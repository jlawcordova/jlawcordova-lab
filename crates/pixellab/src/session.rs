//! A session (ports cli/lib/session.mjs): open documents held in memory as
//! working copies, each with its own undo and redo stacks, standing in for
//! what's on disk in every command's ctx. Mutating commands edit through
//! `ctx.edit`, which opens the doc if it isn't open yet, so every command
//! works the same in a one-shot CLI run, the app's stdio server and the MCP
//! server. Nothing is written until `save`.
//!
//! Undo: a step is a snapshot of the doc before the edit, at most 200 are
//! kept, and an edit that changes nothing is no step. Rapid repeats of the
//! same edit within COALESCE_MS merge into one step.
//!
//! Saving: validate with the engine and refuse a doc with problems unless
//! forced; write the canonical format-2 text; refuse with a conflict if the
//! file changed on disk since the doc was opened (by hash) unless forced;
//! write atomically, and record the write in the version history
//! (docs/intents/2026-10-autosave/spec.md, D3). Every save is a write: agents'
//! edits apply as anyone's do (R16). `autosave` saves a doc a while after its
//! last change (D1).
//!
//! Edits since disk (docs/intents/2026-10-mcp-gaps/spec.md, D1): every undo
//! step has a sequence number, and an entry's base_seq is the number when
//! the doc last matched its file; a version lists the steps after it.
//! `sync` reloads clean docs whose files changed. `merge` (D4) brings a
//! change on disk into a dirty working copy, three ways, in the sources'
//! trailing-comma form (merge.rs).
//!
//! Depth order (docs/intents/2026-10-depth-order/spec.md, R4): an edit of a
//! scene ends by putting every list of its items in depth order, inside the
//! same undo step; the result's paths follow the sort and `reordered` says
//! which paths moved.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use engine::engine::{load_sources, validate};
use engine::format::{self, stringify, Docs};
use serde_json::{json, Map, Value};

use crate::author::{Author, AuthorKind};
use crate::context::{doc_file, DocKind, Found, Loaded};
use crate::diff::unified_diff;
use crate::document::{Document, Env};
use crate::errors::{CommandError, Error, Result};
use crate::history::{Change, History};
use crate::merge::{fold_scene, merge3, trailing, untrailing};
use crate::sorting::{path_changes, remap_json, sort_scene, Sorted};
use crate::state::{create_state, state_dir, write_atomic, State};

pub const UNDO_LIMIT: usize = 200;
pub const COALESCE_MS: f64 = 1000.0;

/// sha256 of a source's text, hex (`hashText`).
pub fn hash_text(text: &str) -> String {
    engine::sha256::hex(text.as_bytes())
}

/// "scene:hero-island" (`docRef`).
pub fn doc_ref(kind: &str, name: &str) -> String {
    format!("{kind}:{name}")
}

/// The data a source text holds (`importSource`): its JSON.
pub fn import_source(text: &str) -> std::result::Result<Value, String> {
    format::parse(text)
}

/// A source file on disk now (`readSource`).
#[derive(Clone, Debug, PartialEq)]
pub struct Disk {
    pub file: String,
    pub path: PathBuf,
    pub exists: bool,
    pub text: String,
    pub hash: Option<String>,
}

/// The file on disk now.
pub fn read_source(document: &Document, kind: DocKind, name: &str) -> Disk {
    let file = doc_file(kind, name);
    let path = document.source_dir.join(&file);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let hash = Some(hash_text(&text));
            Disk { file, path, exists: true, text, hash }
        }
        Err(_) => Disk { file, path, exists: false, text: String::new(), hash: None },
    }
}

/// The engine's problems for one doc's own file, load errors included (`problemsIn`).
pub fn problems_in(loaded: &Loaded, file: &str) -> Vec<String> {
    let prefix = format!("{file}: ");
    loaded.load_errors.iter().cloned().chain(validate(&loaded.sources)).filter(|p| p.starts_with(&prefix)).collect()
}

/// An undo or redo step: the doc before, what it did, its sequence number.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub before: String,
    pub labels: Vec<String>,
    pub seq: u64,
    /// Who made it (docs/intents/2026-10-autosave/spec.md, D7).
    pub author: Author,
}

impl Step {
    /// What the Edit menu calls it: its last label, the count when it holds
    /// several, and whose it is for anyone but the owner ("Place pine (Claude)").
    pub fn menu_label(&self) -> String {
        let what = match self.labels.len() {
            0 => String::new(),
            1 => self.labels[0].clone(),
            n => format!("{} ({n} edits)", self.labels.last().expect("a label")),
        };
        format!("{what}{}", self.author.label_suffix())
    }
}

/// What a doc was when it last matched its file.
#[derive(Clone, Debug, PartialEq)]
pub struct Base {
    pub exists: bool,
    pub hash: Option<String>,
    pub text: String,
}

/// An open doc (`Entry`).
#[derive(Clone, Debug)]
pub struct Entry {
    pub kind: DocKind,
    pub name: String,
    pub ref_: String,
    pub file: String,
    pub doc: Value,
    pub clean: String,
    pub base: Base,
    pub undo: Vec<Step>,
    pub redo: Vec<Step>,
    pub last: Option<(String, f64)>,
    pub base_seq: u64,
    pub undone: Vec<String>,
    pub created: Option<String>,
    /// When the doc last changed, for autosave's pause (docs/intents/2026-10-autosave/spec.md, D1).
    pub changed_at: Option<f64>,
    /// Why autosave last couldn't write it, until it changes again.
    pub blocked: Option<Blocked>,
    /// Who changed it last: its next write is that author's version, whoever saves it (D3, D8).
    pub changed_by: Option<Author>,
}

/// Why autosave couldn't write a doc (D1).
#[derive(Clone, Debug, PartialEq)]
pub enum Blocked {
    /// It has this many problems.
    Problems(usize),
    /// Its file changed on disk and the change clashes with the open copy.
    Conflict,
}

/// What an edit's function returns: its result's fields, and its message
/// (text, or made from the result after the sort moved its paths).
pub struct Edit {
    pub result: Map<String, Value>,
    pub message: Message,
}

/// An edit's message.
pub enum Message {
    None,
    Text(String),
    From(Box<dyn FnOnce(&Map<String, Value>) -> String>),
}

impl Edit {
    /// An edit with a result and a message.
    pub fn new(result: Map<String, Value>, message: impl Into<String>) -> Edit {
        Edit { result, message: Message::Text(message.into()) }
    }

    /// An edit whose message is made from its result after the sort.
    pub fn later(result: Map<String, Value>, message: impl FnOnce(&Map<String, Value>) -> String + 'static) -> Edit {
        Edit { result, message: Message::From(Box::new(message)) }
    }

    /// An edit from a JSON object: its `message` (a string) is the message.
    pub fn from_value(v: Value) -> Edit {
        let mut result = match v {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        let message = match result.shift_remove("message") {
            Some(Value::String(s)) => Message::Text(s),
            _ => Message::None,
        };
        Edit { result, message }
    }
}

/// How a session starts.
pub struct Options {
    pub env: Env,
    pub agent: Option<String>,
    pub chat: Option<Value>,
    pub on_change: Option<Box<dyn FnMut(Value)>>,
    /// `{ doc, file }` when an open doc with unsaved edits can't take what changed on disk (proposal.apply's conflict).
    pub on_stale: Option<Box<dyn FnMut(Value)>>,
    pub now: Box<dyn Fn() -> f64>,
    /// Save a doc this long after its last change (the app's server); None: only when asked.
    pub autosave_ms: Option<f64>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            env: crate::document::process_env(),
            agent: None,
            chat: None,
            on_change: None,
            on_stale: None,
            now: Box::new(crate::state::now_ms),
            autosave_ms: None,
        }
    }
}

/// The paths a result names, moved to where a sort put them (`followSort`):
/// path, paths, group.path, groups[].path, and scene.layer.ungroup's items.
fn follow_sort(result: Map<String, Value>, remap: &engine::omap::OMap<String, String>) -> Map<String, Value> {
    let to = |v: &Value| match v {
        Value::String(s) => Value::String(remap.get(s).cloned().unwrap_or_else(|| s.clone())),
        v => v.clone(),
    };
    let is_path = |s: &str| !s.is_empty() && s.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    let mut out = result;
    if let Some(p) = out.get("path").cloned() {
        out.insert("path".into(), to(&p));
    }
    if let Some(Value::Array(list)) = out.get("paths").cloned() {
        out.insert("paths".into(), Value::Array(list.iter().map(to).collect()));
    }
    if let Some(Value::Object(mut g)) = out.get("group").cloned() {
        if let Some(p) = g.get("path").cloned() {
            g.insert("path".into(), to(&p));
            out.insert("group".into(), Value::Object(g));
        }
    }
    if let Some(Value::Array(list)) = out.get("groups").cloned() {
        let mapped = list
            .into_iter()
            .map(|g| match g {
                Value::Object(mut m) if m.contains_key("path") => {
                    let p = m["path"].clone();
                    m.insert("path".into(), to(&p));
                    Value::Object(m)
                }
                g => g,
            })
            .collect();
        out.insert("groups".into(), Value::Array(mapped));
    }
    if let Some(Value::Array(list)) = out.get("items").cloned() {
        if list.iter().all(|p| p.as_str().is_some_and(is_path)) {
            out.insert("items".into(), Value::Array(list.iter().map(to).collect()));
        }
    }
    out
}

fn obj(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// `{ ...a, ...b }`: b's keys after a's, b's values winning in place.
pub fn spread(mut a: Map<String, Value>, b: Map<String, Value>) -> Map<String, Value> {
    for (k, v) in b {
        a.insert(k, v);
    }
    a
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn ids_of(layers: &Value) -> std::collections::HashSet<String> {
    layers.as_array().into_iter().flatten().filter_map(|l| l.get("id").and_then(Value::as_str).map(str::to_string)).collect()
}

/// Every placed item of a scene, in groups too.
fn placed(doc: &Value) -> Vec<Value> {
    fn walk(items: Option<&Value>, out: &mut Vec<Value>) {
        for item in items.and_then(Value::as_array).into_iter().flatten() {
            if item.as_object().is_some_and(|o| o.contains_key("group")) {
                walk(item.get("items"), out);
            } else {
                out.push(item.clone());
            }
        }
    }
    let mut out = Vec::new();
    walk(doc.get("items"), &mut out);
    out
}

/// The open docs of one door's run.
pub struct Session {
    pub document: Document,
    pub env: Env,
    pub agent: Option<String>,
    /// The Agent chat this session works for (docs/intents/2026-10-agent-chat/spec.md, D7).
    pub chat: Option<Value>,
    /// Who the request being run comes from (docs/intents/2026-10-autosave/spec.md, D8).
    pub author: Author,
    /// Whether that request came from a client of the owner (D10), not the app itself.
    pub remote: bool,
    pub on_change: Option<Box<dyn FnMut(Value)>>,
    pub on_stale: Option<Box<dyn FnMut(Value)>>,
    pub now: Box<dyn Fn() -> f64>,
    pub autosave_ms: Option<f64>,
    pub state: State,
    /// The document's version history (docs/intents/2026-10-autosave/spec.md, D3).
    pub history: History,
    /// Open docs by ref, in the order they were opened.
    pub entries: engine::omap::OMap<String, Entry>,
    pub seq: u64,
    disk: Option<Rc<Docs>>,
    loaded: Option<Rc<Loaded>>,
}

impl Session {
    pub fn new(document: Document, options: Options) -> Session {
        let state = create_state(state_dir(&options.env));
        // The version history starts at the first write (version 0 is the document before it), so
        // a session that only reads writes nothing. One that can't be written never stops an edit.
        let history = History::new(&state.dir, &document.root, &document.source_dir);
        if let Err(e) = history.thin((options.now)()) {
            eprintln!("pixellab: can't thin the version history: {}", e.message());
        }
        Session {
            document,
            env: options.env,
            agent: options.agent,
            chat: options.chat,
            author: Author::owner(),
            remote: false,
            on_change: options.on_change,
            on_stale: options.on_stale,
            now: options.now,
            autosave_ms: options.autosave_ms,
            state,
            history,
            entries: engine::omap::OMap::new(),
            seq: 0,
            disk: None,
            loaded: None,
        }
    }

    // ------------------------------------------------------------ sources

    /// A new call: the disk is read again on the next `loaded`.
    pub fn begin_call(&mut self) {
        self.disk = None;
        self.loaded = None;
    }

    /// The working copies changed: the next `loaded` merges them again.
    pub fn invalidate(&mut self) {
        self.loaded = None;
    }

    /// The document's files as on disk, read once per call.
    pub fn disk(&mut self) -> Result<Rc<Docs>> {
        if self.disk.is_none() {
            self.disk = Some(Rc::new(self.document.load_docs()));
        }
        Ok(self.disk.clone().expect("just read"))
    }

    /// The disk's docs with the working copies in their place (`merged`, `ctx.sources`).
    pub fn loaded(&mut self) -> Result<Rc<Loaded>> {
        if let Some(l) = &self.loaded {
            return Ok(l.clone());
        }
        let disk = self.disk()?;
        let mut objects: BTreeMap<String, Value> = disk.objects.clone();
        let mut scenes: BTreeMap<String, Value> = disk.scenes.clone();
        for entry in self.entries.values() {
            match entry.kind {
                DocKind::Object => objects.insert(entry.name.clone(), entry.doc.clone()),
                DocKind::Scene => scenes.insert(entry.name.clone(), entry.doc.clone()),
                DocKind::Palette => None,
            };
        }
        let sources = load_sources(disk.palette.clone().unwrap_or(Value::Null), objects, scenes);
        let loaded = Rc::new(Loaded { sources, load_errors: disk.load_errors.clone() });
        self.loaded = Some(loaded.clone());
        Ok(loaded)
    }

    /// The load error of a doc's file, if it exists but didn't load.
    pub fn load_error(&mut self, kind: DocKind, name: &str) -> Result<Option<String>> {
        let loaded = self.loaded()?;
        let prefix = format!("{}: cannot be loaded: ", doc_file(kind, name));
        Ok(loaded.load_errors.iter().find(|e| e.starts_with(&prefix)).cloned())
    }

    /// One document's data (`ctx.doc`). NotFoundError if there's no such doc.
    pub fn doc(&mut self, kind: DocKind, name: &str) -> Result<Value> {
        let loaded = self.loaded()?;
        let found = match kind {
            DocKind::Palette => (!loaded.sources.palette.is_null()).then(|| loaded.sources.palette.clone()),
            DocKind::Object => loaded.sources.objects.get(name).cloned(),
            DocKind::Scene => loaded.sources.scenes.get(name).cloned(),
        };
        if let Some(doc) = found {
            return Ok(doc);
        }
        if let Some(e) = self.load_error(kind, name)? {
            return Err(CommandError::new(e, "load_error").into());
        }
        Err(CommandError::not_found(if kind == DocKind::Palette {
            format!("{} is missing", format::PALETTE_FILE)
        } else {
            format!("no {} named {name}", kind.as_str())
        })
        .into())
    }

    /// Finds an object or scene by name, or by objects/<name> or
    /// scenes/<name>, a trailing .json allowed (`ctx.find`).
    pub fn find(&mut self, query: &str, kind: Option<DocKind>) -> Result<Found> {
        let (asked_dir, name) = match query.split_once('/') {
            Some((dir @ ("objects" | "scenes"), rest)) if !rest.is_empty() => {
                (Some(dir), rest.strip_suffix(".json").filter(|n| !n.is_empty()).unwrap_or(rest))
            }
            _ => (None, query.strip_suffix(".json").unwrap_or(query)),
        };
        let asked = match asked_dir {
            Some("objects") => Some(DocKind::Object),
            Some(_) => Some(DocKind::Scene),
            None => kind,
        };
        if let (Some(_), Some(k)) = (asked_dir, kind) {
            if asked != Some(k) {
                return Err(CommandError::not_found(format!("{query} is not a {}", k.as_str())).into());
            }
        }
        let loaded = self.loaded()?;
        let object_there = loaded.sources.objects.contains_key(name) || self.load_error(DocKind::Object, name)?.is_some();
        let scene_there = loaded.sources.scenes.contains_key(name) || self.load_error(DocKind::Scene, name)?.is_some();
        let is_object = (asked.is_none() || asked == Some(DocKind::Object)) && object_there;
        let is_scene = (asked.is_none() || asked == Some(DocKind::Scene)) && scene_there;
        if is_object && is_scene {
            return Err(CommandError::new(
                format!("{name} is both an object and a scene; say objects/{name} or scenes/{name}"),
                "ambiguous",
            )
            .into());
        }
        if !is_object && !is_scene {
            let what = asked.map_or("object or scene", DocKind::as_str);
            return Err(CommandError::not_found(format!("no {what} named {query}")).into());
        }
        let found = if is_object { DocKind::Object } else { DocKind::Scene };
        Ok(Found { kind: found, name: name.to_string(), file: doc_file(found, name) })
    }

    // ------------------------------------------------------------ entries

    pub fn entry(&mut self, ref_: &str) -> Result<&mut Entry> {
        match self.entries.get_mut(ref_) {
            Some(e) => Ok(e),
            None => Err(CommandError::not_found(format!("{ref_} is not open in this session (doc.open opens it)")).into()),
        }
    }

    /// Opens a doc as a working copy, or returns it if it's open.
    pub fn open(&mut self, kind: DocKind, name: &str) -> Result<&mut Entry> {
        let ref_ = doc_ref(kind.as_str(), name);
        if !self.entries.has(&ref_) {
            let data = self.doc(kind, name)?;
            let disk = read_source(&self.document, kind, name);
            let entry = Entry {
                kind,
                name: name.to_string(),
                ref_: ref_.clone(),
                file: doc_file(kind, name),
                clean: serialize(&data).unwrap_or_default(),
                doc: data,
                base: Base { exists: disk.exists, hash: disk.hash, text: disk.text },
                undo: vec![],
                redo: vec![],
                last: None,
                base_seq: self.seq,
                undone: vec![],
                created: None,
                changed_at: None,
                blocked: None,
                changed_by: None,
            };
            self.entries.set(ref_.clone(), entry);
            self.invalidate();
        }
        Ok(self.entries.get_mut(&ref_).expect("just opened"))
    }

    /// Adds a new doc (not on disk yet) as a working copy. It's dirty until saved.
    pub fn create(&mut self, kind: DocKind, name: &str, doc: Value, label: &str) {
        let ref_ = doc_ref(kind.as_str(), name);
        let entry = Entry {
            kind,
            name: name.to_string(),
            ref_: ref_.clone(),
            file: doc_file(kind, name),
            doc,
            clean: String::new(),
            base: Base { exists: false, hash: None, text: String::new() },
            undo: vec![],
            redo: vec![],
            last: None,
            base_seq: self.seq,
            undone: vec![],
            created: Some(label.to_string()),
            changed_at: None,
            blocked: None,
            changed_by: None,
        };
        self.entries.set(ref_.clone(), entry);
        self.invalidate();
        self.changed(&ref_);
    }

    /// Puts a scene's lists of items in depth order, in place, judging
    /// objects as the session has them.
    fn sort_doc(&mut self, doc: &mut Value) -> Result<Sorted> {
        let loaded = self.loaded()?;
        Ok(sort_scene(&loaded.sources, doc))
    }

    /// Edits a doc as one undo step (opening it first if needed). `f` gets
    /// the doc and the sources as they were before the edit.
    pub fn edit(
        &mut self,
        kind: DocKind,
        name: &str,
        key: Option<&str>,
        f: impl FnOnce(&mut Value, &Loaded) -> Result<Edit>,
    ) -> Result<Value> {
        let ref_ = self.open(kind, name)?.ref_.clone();
        let loaded = self.loaded()?;
        let before_doc = self.entries.get(&ref_).expect("open").doc.clone();
        let before = stringify(&before_doc);
        let mut doc = before_doc;
        let edit = f(&mut doc, &loaded);
        drop(loaded);
        let edit = match edit {
            Ok(e) => e,
            Err(err) => {
                self.invalidate();
                return Err(err);
            }
        };
        self.entries.get_mut(&ref_).expect("open").doc = doc;
        self.invalidate();
        let mut result = edit.result;
        // Sorting is part of the edit: the same undo step holds both.
        if kind == DocKind::Scene {
            let mut doc = self.entries.get(&ref_).expect("open").doc.clone();
            let sorted = self.sort_doc(&mut doc)?;
            if !sorted.remap.is_empty() {
                self.entries.get_mut(&ref_).expect("open").doc = doc;
                self.invalidate();
                result = follow_sort(result, &sorted.remap);
                result.insert("reordered".into(), remap_json(&sorted.remap));
            }
        }
        let message = match edit.message {
            Message::From(f) => f(&result),
            Message::Text(t) => t,
            Message::None => "changed".into(),
        };
        let time = (self.now)();
        let next_seq = self.seq + 1;
        let entry = self.entries.get_mut(&ref_).expect("open");
        let changed = stringify(&entry.doc) != before;
        if changed {
            let coalesce = key.is_some()
                && entry.last.as_ref().is_some_and(|(k, t)| Some(k.as_str()) == key && time - t < COALESCE_MS)
                && entry.undo.last().is_some_and(|s| s.author.same(&self.author));
            if coalesce {
                entry.undo.last_mut().expect("a step").labels.push(message.clone());
            } else {
                entry.undo.push(Step { before, labels: vec![message.clone()], seq: next_seq, author: self.author.clone() });
                if entry.undo.len() > UNDO_LIMIT {
                    entry.undo.remove(0);
                }
            }
            entry.last = key.map(|k| (k.to_string(), time));
            entry.redo.clear();
            if !coalesce {
                self.seq = next_seq;
            }
            self.changed(&ref_);
        }
        let summary = if changed { message.clone() } else { format!("nothing changed ({message})") };
        let head = obj(vec![("doc", Value::String(ref_)), ("changed", Value::Bool(changed)), ("summary", Value::String(summary))]);
        Ok(Value::Object(spread(head, result)))
    }

    /// `{ doc, dirty, canUndo, canRedo }` (`status`).
    pub fn status(&self, ref_: &str) -> Map<String, Value> {
        let entry = self.entries.get(ref_).expect("an open doc");
        let text = serialize(&entry.doc);
        obj(vec![
            ("doc", Value::String(ref_.to_string())),
            ("dirty", Value::Bool(text.as_deref() != Some(entry.clean.as_str()))),
            ("canUndo", Value::Bool(!entry.undo.is_empty())),
            ("canRedo", Value::Bool(!entry.redo.is_empty())),
        ])
    }

    /// Whether a doc has edits its file doesn't.
    pub fn is_dirty(&self, ref_: &str) -> bool {
        self.status(ref_)["dirty"] == Value::Bool(true)
    }

    /// The full status doc.status shows (`details`).
    pub fn details(&self, ref_: &str) -> Value {
        let entry = self.entries.get(ref_).expect("an open doc");
        let disk = read_source(&self.document, entry.kind, &entry.name);
        let mut out = self.live_status(ref_);
        out.insert("file".into(), Value::String(entry.file.clone()));
        out.insert("exists".into(), Value::Bool(entry.base.exists));
        out.insert("stale".into(), Value::Bool(is_stale(entry, &disk)));
        out.insert("undo".into(), Value::from(entry.undo.iter().flat_map(|s| s.labels.clone()).collect::<Vec<_>>()));
        let mut redo: Vec<String> = entry.redo.iter().flat_map(|s| s.labels.clone()).collect();
        redo.reverse();
        out.insert("redo".into(), Value::from(redo));
        out.insert("edits".into(), Value::from(edits(entry)));
        Value::Object(out)
    }

    /// The status plus what the app shows of it (docs/intents/2026-10-autosave/spec.md, D1, D11):
    /// the Edit menu's labels and who made the latest change.
    pub fn live_status(&self, ref_: &str) -> Map<String, Value> {
        let mut out = self.status(ref_);
        let entry = self.entries.get(ref_).expect("an open doc");
        let label = |s: Option<&Step>| s.map_or(Value::Null, |s| Value::String(s.menu_label()));
        out.insert("undoLabel".into(), label(entry.undo.last()));
        out.insert("redoLabel".into(), label(entry.redo.last()));
        out.insert("author".into(), self.author.to_json());
        let (state, problems) = self.save_state(ref_);
        out.insert("saveState".into(), json!(state));
        out.insert("problems".into(), json!(problems));
        out
    }

    /// `saved`, `pending` (a save is coming), `problems` or `conflict` (autosave can't write it), and
    /// how many problems (D1).
    pub fn save_state(&self, ref_: &str) -> (&'static str, usize) {
        let Some(entry) = self.entries.get(ref_) else { return ("saved", 0) };
        match &entry.blocked {
            Some(Blocked::Problems(n)) => ("problems", *n),
            Some(Blocked::Conflict) => ("conflict", 0),
            None if self.is_dirty(ref_) => ("pending", 0),
            None => ("saved", 0),
        }
    }

    /// Saves every doc whose last change is at least `autosave_ms` old (`force_all`: every dirty
    /// doc now, a blocked one too). A doc with problems stays unsaved and is blocked until it
    /// changes; one whose file changed on disk is merged, then saved; a clash blocks it and is
    /// reported stale. Returns each doc tried, with what happened.
    pub fn autosave(&mut self, now: f64, force_all: bool) -> Vec<Value> {
        let wait = self.autosave_ms;
        let due: Vec<String> = self
            .entries
            .values()
            .filter(|e| {
                let dirty = serialize(&e.doc).as_deref() != Some(e.clean.as_str());
                dirty && (force_all || (e.blocked.is_none() && wait.is_some_and(|w| e.changed_at.is_none_or(|t| now - t >= w))))
            })
            .map(|e| e.ref_.clone())
            .collect();
        let mut out = Vec::new();
        for ref_ in due {
            out.push(self.save_now(&ref_));
        }
        out
    }

    /// A change on disk clashed with the doc's pending edits: autosave stops until it changes, and
    /// the status says why (R7).
    pub fn block_conflict(&mut self, ref_: &str) {
        if let Some(e) = self.entries.get_mut(ref_) {
            e.blocked = Some(Blocked::Conflict);
        }
        self.notify(ref_);
    }

    /// One autosave of a doc (D1).
    pub fn save_now(&mut self, ref_: &str) -> Value {
        let mut result = self.save(ref_, false);
        if let Err(Error::Command(e)) = &result {
            if e.code == "conflict" {
                result = match self.merge(ref_) {
                    Ok(_) => self.save(ref_, false),
                    Err(err) => Err(err),
                };
            }
        }
        match result {
            Ok(r) => json!({ "doc": ref_, "saved": true, "wrote": r.get("wrote").cloned().unwrap_or(Value::Bool(false)) }),
            Err(err) => {
                let (code, n) = match &err {
                    Error::Command(e) => (e.code.clone(), e.details.as_ref().and_then(Value::as_array).map_or(1, Vec::len)),
                    Error::Internal(_) => ("internal".to_string(), 0),
                };
                let blocked = if code == "invalid" { Blocked::Problems(n.max(1)) } else { Blocked::Conflict };
                let stale = blocked == Blocked::Conflict;
                let file = self.entries.get(ref_).map(|e| e.file.clone()).unwrap_or_default();
                if let Some(e) = self.entries.get_mut(ref_) {
                    e.blocked = Some(blocked);
                }
                self.notify(ref_);
                if stale {
                    if let Some(f) = self.on_stale.as_mut() {
                        f(json!({ "doc": ref_, "file": file }));
                    }
                }
                json!({ "doc": ref_, "saved": false, "code": code, "message": err.message() })
            }
        }
    }

    /// Tells whoever listens that a doc's status changed, and notes when, for autosave.
    fn changed(&mut self, ref_: &str) {
        let now = (self.now)();
        let author = self.author.clone();
        if let Some(e) = self.entries.get_mut(ref_) {
            if serialize(&e.doc).as_deref() != Some(e.clean.as_str()) {
                e.changed_by = Some(author);
            }
            e.changed_at = Some(now);
            e.blocked = None;
        }
        self.notify(ref_);
    }

    /// Sends a doc's live status to whoever listens.
    fn notify(&mut self, ref_: &str) {
        if self.on_change.is_some() && self.entries.has(ref_) {
            let status = Value::Object(self.live_status(ref_));
            (self.on_change.as_mut().expect("a listener"))(status);
        }
    }

    /// Undoes or redoes one step (`step`).
    pub fn step(&mut self, ref_: &str, undo: bool) -> Result<Value> {
        let entry = self.entry(ref_)?;
        let (from, to) = if undo { (&mut entry.undo, &mut entry.redo) } else { (&mut entry.redo, &mut entry.undo) };
        let Some(snap) = from.pop() else {
            let summary = if undo { "Nothing to undo" } else { "Nothing to redo" };
            let head = obj(vec![
                ("doc", Value::String(ref_.to_string())),
                ("changed", Value::Bool(false)),
                ("summary", Value::String(summary.into())),
            ]);
            return Ok(Value::Object(spread(head, self.status(ref_))));
        };
        to.push(Step { before: stringify(&entry.doc), labels: snap.labels.clone(), seq: snap.seq, author: snap.author.clone() });
        // A step from before the doc last matched its file: undoing it is itself a change from disk.
        if snap.seq <= entry.base_seq {
            if undo {
                entry.undone.extend(snap.labels.iter().map(|l| format!("Undid: {l}")));
            } else {
                let keep = entry.undone.len().saturating_sub(snap.labels.len());
                entry.undone.truncate(keep);
            }
        }
        let restored = format::parse(&snap.before).map_err(Error::Internal)?;
        let left = std::mem::replace(&mut entry.doc, restored);
        entry.last = None;
        let reordered = if entry.kind == DocKind::Scene { path_changes(&left, &entry.doc) } else { Default::default() };
        self.invalidate();
        self.changed(ref_);
        let what = if snap.labels.len() == 1 {
            snap.labels[0].clone()
        } else {
            format!("{} ({} edits)", snap.labels.last().expect("a label"), snap.labels.len())
        };
        let head = obj(vec![
            ("doc", Value::String(ref_.to_string())),
            ("changed", Value::Bool(true)),
            ("summary", Value::String(format!("{}: {what}", if undo { "Undid" } else { "Redid" }))),
        ]);
        let mut out = spread(head, self.status(ref_));
        if !reordered.is_empty() {
            out.insert("reordered".into(), remap_json(&reordered));
        }
        Ok(Value::Object(out))
    }

    /// Marks the doc as matching its file (`matchDisk`): the next proposal
    /// lists only later edits, and no edit coalesces across the line.
    fn match_disk(&mut self, ref_: &str, disk: Base, clean: String, seq: Option<u64>) {
        let seq = seq.unwrap_or(self.seq);
        let entry = self.entries.get_mut(ref_).expect("open");
        entry.base = disk;
        entry.clean = clean;
        entry.base_seq = seq;
        entry.undone.clear();
        entry.last = None;
        entry.created = None;
        entry.changed_by = None;
    }

    /// Replaces the working copy with the file, clearing its history, then
    /// sorts it if the file is out of depth order (`loadFromDisk`).
    fn load_from_disk(&mut self, ref_: &str, disk: &Disk) -> Result<Option<Sorted>> {
        let data = import_source(&disk.text).map_err(Error::Internal)?;
        let clean = serialize(&data).unwrap_or_default();
        {
            let entry = self.entries.get_mut(ref_).expect("open");
            entry.doc = data;
            entry.undo.clear();
            entry.redo.clear();
        }
        self.match_disk(ref_, base_of(disk), clean, None);
        self.invalidate();
        self.sort_loaded(ref_)
    }

    /// A scene just adopted from its file is put in depth order if it isn't,
    /// as a step of its own (`sortLoaded`).
    fn sort_loaded(&mut self, ref_: &str) -> Result<Option<Sorted>> {
        let entry = self.entries.get(ref_).expect("open");
        if entry.kind != DocKind::Scene {
            return Ok(None);
        }
        let before = stringify(&entry.doc);
        let name = entry.name.clone();
        let mut doc = entry.doc.clone();
        let sorted = self.sort_doc(&mut doc)?;
        if sorted.moved.is_empty() {
            return Ok(None);
        }
        self.seq += 1;
        let seq = self.seq;
        let entry = self.entries.get_mut(ref_).expect("open");
        entry.doc = doc;
        entry.undo.push(Step { before, labels: vec![format!("Sorted {name} by depth")], seq, author: self.author.clone() });
        self.invalidate();
        self.changed(ref_);
        Ok(Some(sorted))
    }

    /// What saving would write, and whether it may (`plan`).
    pub fn plan(&mut self, ref_: &str) -> Result<Plan> {
        let entry = self.entry(ref_)?.clone();
        let Some(text) = serialize(&entry.doc) else {
            return Err(CommandError::new(
                format!("{} can't be serialized; fix its shape first (pixellab check {})", entry.ref_, entry.name),
                "invalid",
            )
            .into());
        };
        let loaded = self.loaded()?;
        let problems = problems_in(&loaded, &entry.file);
        let disk = read_source(&self.document, entry.kind, &entry.name);
        let conflict = if entry.base.exists {
            if !disk.exists {
                Some("deleted")
            } else if disk.hash != entry.base.hash {
                Some("changed")
            } else {
                None
            }
        } else if disk.exists {
            Some("created")
        } else {
            None
        };
        let diff = unified_diff(&disk.text, &text, &format!("a/{}", entry.file), &format!("b/{}", entry.file), 3);
        Ok(Plan { text, disk, problems, conflict, diff })
    }

    /// Puts a doc's content to `data` as one undo step labelled `label`, opening it, or adding it
    /// when it isn't in the document (history.restore and history.undo). Returns its ref.
    pub fn set_doc(&mut self, kind: DocKind, name: &str, data: Value, label: &str) -> Result<String> {
        let ref_ = doc_ref(kind.as_str(), name);
        let exists = self.entries.has(&ref_) || read_source(&self.document, kind, name).exists;
        if exists {
            let label = label.to_string();
            self.edit(kind, name, None, move |doc, _| {
                *doc = data;
                Ok(Edit::new(Map::new(), label))
            })?;
        } else {
            self.create(kind, name, data, label);
        }
        Ok(ref_)
    }

    /// Deletes a doc's file and closes it, recording the change (history.restore and history.undo).
    pub fn delete_doc(&mut self, kind: DocKind, name: &str, label: &str) -> Result<()> {
        let ref_ = doc_ref(kind.as_str(), name);
        let disk = read_source(&self.document, kind, name);
        if disk.exists {
            std::fs::remove_file(&disk.path)?;
            self.record(Change { file: disk.file.clone(), before: Some(disk.text.clone()), after: None }, &[label.to_string()]);
        }
        self.entries.delete(&ref_);
        self.invalidate();
        Ok(())
    }

    /// A doc's text as the session has it now: its working copy when open, else its file (None: no file).
    pub fn current_text(&self, kind: DocKind, name: &str) -> Option<String> {
        let ref_ = doc_ref(kind.as_str(), name);
        if let Some(e) = self.entries.get(&ref_) {
            return serialize(&e.doc);
        }
        let disk = read_source(&self.document, kind, name);
        disk.exists.then_some(disk.text)
    }

    /// Records a write in the version history, by this request's author. A history that can't be
    /// written doesn't fail the save that's already on disk.
    pub fn record(&mut self, change: Change, labels: &[String]) {
        let author = self.author.clone();
        self.record_as(&author, change, labels);
    }

    /// Records a write in the version history, by an author.
    pub fn record_as(&mut self, author: &Author, change: Change, labels: &[String]) {
        if let Err(e) = self.history.record(&[change], author, labels, (self.now)()) {
            eprintln!("pixellab: can't record the version: {}", e.message());
        }
    }

    /// Records a file that changed on disk without this session, unless the history already has it
    /// as it is now (another Pixel Lab writer recorded it).
    fn note_outside(&mut self, file: &str, before: Option<String>, disk: &Disk) {
        let now_hash = disk.hash.clone();
        let latest = self.history.latest().map(|n| self.history.files_at(n));
        if latest.as_ref().and_then(|f| f.get(file)).is_some_and(|h| *h == now_hash) {
            return;
        }
        let change = Change { file: file.to_string(), before, after: disk.exists.then(|| disk.text.clone()) };
        let author = Author::of(AuthorKind::Outside);
        if let Err(e) = self.history.record(&[change], &author, &["Changed outside Pixel Lab".to_string()], (self.now)()) {
            eprintln!("pixellab: can't record the version: {}", e.message());
        }
    }

    /// Saves a doc: validates it and writes its file, recording the version.
    pub fn save(&mut self, ref_: &str, force: bool) -> Result<Value> {
        let plan = self.plan(ref_)?;
        let file = self.entry(ref_)?.file.clone();
        if !plan.problems.is_empty() && !force {
            let n = plan.problems.len();
            let them = if n == 1 { "it" } else { "them" };
            return Err(CommandError::new(
                format!(
                    "{file} has {n} problem{}; nothing written. Fix {them}, or force the save:\n{}",
                    plural(n),
                    plan.problems.join("\n")
                ),
                "invalid",
            )
            .with_details(Value::from(plan.problems.clone()))
            .into());
        }
        if let Some(conflict) = plan.conflict.filter(|_| !force) {
            let how = match conflict {
                "changed" => "changed on disk",
                "deleted" => "was deleted",
                _ => "was created on disk",
            };
            return Err(CommandError::new(
                format!("{file} {how} since it was opened; nothing written. Revert to load it again, or force the save to overwrite it."),
                "conflict",
            )
            .with_details(json!({ "conflict": conflict }))
            .into());
        }
        let wrote = plan.text != plan.disk.text || !plan.disk.exists;
        if wrote {
            write_atomic(&plan.disk.path, &plan.text)?;
            let entry = self.entry(ref_)?;
            let labels = edits(entry);
            // The version is the last editor's, not whoever runs the save (the app's autosave of an agent's edit).
            let author = entry.changed_by.clone().unwrap_or_else(|| self.author.clone());
            let before = plan.disk.exists.then(|| plan.disk.text.clone());
            self.record_as(&author, Change { file: file.clone(), before, after: Some(plan.text.clone()) }, &labels);
        }
        let hash = hash_text(&plan.text);
        self.match_disk(ref_, Base { exists: true, hash: Some(hash.clone()), text: plan.text.clone() }, plan.text.clone(), None);
        self.invalidate();
        self.changed(ref_);
        let mut out = obj(vec![
            ("doc", Value::String(ref_.to_string())),
            ("file", Value::String(file)),
            ("wrote", Value::Bool(wrote)),
            ("hash", Value::String(hash)),
            ("bytes", Value::from(plan.text.len())),
            ("diff", Value::String(plan.diff)),
        ]);
        if !plan.problems.is_empty() {
            out.insert("forcedProblems".into(), Value::from(plan.problems));
        }
        Ok(Value::Object(spread(out, self.status(ref_))))
    }

    /// Loads the doc from disk again, as one undo step; a new doc is dropped (`revert`).
    pub fn revert(&mut self, ref_: &str) -> Result<Value> {
        let entry = self.entry(ref_)?.clone();
        let disk = read_source(&self.document, entry.kind, &entry.name);
        if !disk.exists {
            self.entries.delete(ref_);
            self.invalidate();
            return Ok(
                json!({ "doc": ref_, "changed": true, "summary": format!("Discarded {ref_}, which isn't on disk"), "closed": true, "dirty": false, "canUndo": false, "canRedo": false }),
            );
        }
        let data =
            import_source(&disk.text).map_err(|e| CommandError::new(format!("{}: cannot be loaded: {e}", entry.file), "load_error"))?;
        let before = stringify(&entry.doc);
        let changed = stringify(&data) != before;
        let clean = serialize(&data).unwrap_or_default();
        if changed {
            self.seq += 1;
        }
        let seq = self.seq;
        {
            let e = self.entries.get_mut(ref_).expect("open");
            e.doc = data;
            if changed {
                e.undo.push(Step {
                    before,
                    labels: vec![format!("Reverted {ref_} to the file on disk")],
                    seq,
                    author: self.author.clone(),
                });
                if e.undo.len() > UNDO_LIMIT {
                    e.undo.remove(0);
                }
                e.redo.clear();
            }
        }
        self.match_disk(ref_, base_of(&disk), clean, None);
        self.invalidate();
        self.changed(ref_);
        let head = obj(vec![
            ("doc", Value::String(ref_.to_string())),
            ("changed", Value::Bool(changed)),
            ("summary", Value::String(format!("Reverted {ref_} to the file on disk"))),
        ]);
        Ok(Value::Object(spread(head, self.status(ref_))))
    }

    /// Re-reads a clean doc whose file changed on disk; a dirty doc is left
    /// alone. Returns whether it reloaded (`reloadIfClean`).
    pub fn reload_if_clean(&mut self, ref_: &str) -> bool {
        let Some(entry) = self.entries.get(ref_) else { return false };
        let disk = read_source(&self.document, entry.kind, &entry.name);
        if !disk.exists || disk.hash == entry.base.hash || self.is_dirty(ref_) {
            return false;
        }
        let (file, before) = (entry.file.clone(), entry.base.exists.then(|| entry.base.text.clone()));
        self.note_outside(&file, before, &disk);
        if self.load_from_disk(ref_, &disk).is_err() {
            return false;
        }
        self.changed(ref_);
        true
    }

    /// Brings every open doc in line with disk where nothing is lost (`sync`): a clean doc whose
    /// file changed reloads, and a dirty one that now matches its file is clean.
    pub fn sync(&mut self) {
        let refs: Vec<String> = self.entries.keys().cloned().collect();
        for ref_ in refs {
            // An unreadable file or a broken source is left for doc.status and save to report.
            let _ = self.settle(&ref_);
        }
    }

    fn settle(&mut self, ref_: &str) -> Result<()> {
        let Some(entry) = self.entries.get(ref_).cloned() else { return Ok(()) };
        let disk = read_source(&self.document, entry.kind, &entry.name);
        let text = serialize(&entry.doc);
        if !disk.exists || !is_stale(&entry, &disk) {
            return Ok(());
        }
        if text.as_deref() == Some(entry.clean.as_str()) {
            self.reload_if_clean(ref_);
            return Ok(());
        }
        let canon = canonical(&disk.text);
        if canon.is_some() && canon == text {
            self.match_disk(ref_, base_of(&disk), canon.expect("checked"), None);
            self.changed(ref_);
        }
        Ok(())
    }

    /// Loads a doc from disk, but only if no unsaved work is lost (`reload`).
    pub fn reload(&mut self, ref_: &str) -> Result<Value> {
        let entry = self.entry(ref_)?.clone();
        let disk = read_source(&self.document, entry.kind, &entry.name);
        let text = serialize(&entry.doc);
        let dirty = text.as_deref() != Some(entry.clean.as_str());
        let canon = if disk.exists { canonical(&disk.text) } else { None };
        if dirty && (canon.is_none() || canon != text) {
            let list = edits(&entry);
            let n = list.len();
            let count = if n == 0 { "some".to_string() } else { n.to_string() };
            let them = if n == 1 { "it" } else { "them" };
            return Err(CommandError::new(
                format!("{ref_} has {count} unsaved edit{} the file doesn't have; reloading would drop {them}. Save {them} (doc.save), or revert to drop {them} (doc.revert).", plural(n)),
                "dirty",
            )
            .with_details(Value::from(list))
            .into());
        }
        if !disk.exists {
            self.entries.delete(ref_);
            self.invalidate();
            return Ok(
                json!({ "doc": ref_, "reloaded": false, "closed": true, "changed": true, "summary": format!("Closed {ref_}: its file isn't on disk any more"), "dirty": false, "canUndo": false, "canRedo": false }),
            );
        }
        if canon.is_none() {
            return Err(CommandError::new(
                format!("{} on disk doesn't load; fix it first (pixellab check {})", entry.file, entry.name),
                "load_error",
            )
            .into());
        }
        if !is_stale(&entry, &disk) && !dirty {
            let head = obj(vec![
                ("doc", Value::String(ref_.to_string())),
                ("reloaded", Value::Bool(false)),
                ("changed", Value::Bool(false)),
                ("summary", Value::String(format!("{ref_} already matches {}", entry.file))),
            ]);
            return Ok(Value::Object(spread(head, self.status(ref_))));
        }
        let before = stringify(&entry.doc);
        self.note_outside(&entry.file, entry.base.exists.then(|| entry.base.text.clone()), &disk);
        let sorted = self.load_from_disk(ref_, &disk)?;
        self.invalidate();
        self.changed(ref_);
        let now_text = stringify(&self.entries.get(ref_).expect("open").doc);
        let note = if sorted.is_some() {
            format!("; it wasn't in depth order, so {} was sorted (one undo gives back the file)", entry.name)
        } else {
            String::new()
        };
        let mut out = obj(vec![
            ("doc", Value::String(ref_.to_string())),
            ("reloaded", Value::Bool(true)),
            ("changed", Value::Bool(now_text != before)),
            ("summary", Value::String(format!("Reloaded {ref_} from {}{note}", entry.file))),
        ]);
        if let Some(s) = &sorted {
            out.insert("sorted".into(), Value::from(s.moved.len()));
        }
        Ok(Value::Object(spread(out, self.status(ref_))))
    }

    /// The problems of `data` as this doc that none of `others` has (`newProblems`).
    fn new_problems(&mut self, entry: &Entry, data: &Value, others: &[Value]) -> Result<Vec<String>> {
        let loaded = self.loaded()?;
        let prefix = format!("{}: ", entry.file);
        let problems_of = |doc: &Value| -> Vec<String> {
            let mut objects = loaded.sources.objects.clone();
            let mut scenes = BTreeMap::new();
            match entry.kind {
                DocKind::Scene => {
                    scenes.insert(entry.name.clone(), doc.clone());
                }
                _ => {
                    objects.insert(entry.name.clone(), doc.clone());
                }
            }
            validate(&load_sources(loaded.sources.palette.clone(), objects, scenes))
                .into_iter()
                .filter(|p| p.starts_with(&prefix))
                .collect()
        };
        let had: std::collections::HashSet<String> = others.iter().flat_map(problems_of).collect();
        Ok(problems_of(data).into_iter().filter(|p| !had.contains(p)).collect())
    }

    /// Merges the file's changes since the doc's base into its working copy,
    /// three ways, as one undo step (`merge`). A clean doc just reloads; a
    /// clash changes nothing and is a merge_conflict.
    pub fn merge(&mut self, ref_: &str) -> Result<Value> {
        let entry = self.entry(ref_)?.clone();
        let disk = read_source(&self.document, entry.kind, &entry.name);
        if !disk.exists {
            return Err(CommandError::new(format!("{} isn't on disk; there's nothing to merge", entry.file), "not_found").into());
        }
        if !is_stale(&entry, &disk) {
            let head = obj(vec![
                ("doc", Value::String(ref_.to_string())),
                ("merged", Value::Bool(false)),
                ("changed", Value::Bool(false)),
                ("summary", Value::String(format!("{} hasn't changed since {ref_} was opened", entry.file))),
            ]);
            return Ok(Value::Object(spread(head, self.status(ref_))));
        }
        let Some(theirs) = canonical(&disk.text) else {
            return Err(CommandError::new(
                format!("{} on disk doesn't load, so it can't be merged; fix it first (pixellab check {})", entry.file, entry.name),
                "load_error",
            )
            .into());
        };
        let Some(ours) = serialize(&entry.doc) else {
            return Err(CommandError::new(
                format!("{ref_} can't be serialized; fix its shape first (pixellab check {})", entry.name),
                "invalid",
            )
            .into());
        };
        self.note_outside(&entry.file, entry.base.exists.then(|| entry.base.text.clone()), &disk);
        if ours == entry.clean {
            let sorted = self.load_from_disk(ref_, &disk)?;
            self.invalidate();
            self.changed(ref_);
            let note = if sorted.is_some() {
                format!("; it wasn't in depth order, so {} was sorted (one undo gives back the file)", entry.name)
            } else {
                String::new()
            };
            let mut out = obj(vec![
                ("doc", Value::String(ref_.to_string())),
                ("merged", Value::Bool(true)),
                ("changed", Value::Bool(true)),
                ("summary", Value::String(format!("Reloaded {ref_}: it had no unsaved edits{note}"))),
            ]);
            if let Some(s) = &sorted {
                out.insert("sorted".into(), Value::from(s.moved.len()));
            }
            return Ok(Value::Object(spread(out, self.status(ref_))));
        }
        let base = if entry.base.exists { canonical(&entry.base.text).unwrap_or_else(|| entry.base.text.clone()) } else { String::new() };
        let is_scene = entry.kind == DocKind::Scene;
        let merged = merge3(&trailing(&base), &trailing(&ours), &trailing(&theirs), is_scene);
        if !merged.conflicts.is_empty() {
            let n = merged.conflicts.len();
            let details: Vec<Value> =
                merged.conflicts.iter().map(|c| json!({ "base": c.base, "ours": c.ours, "theirs": c.theirs })).collect();
            return Err(CommandError::new(
                format!(
                    "Both the open copy and {} changed the same line{} ({n} place{}); nothing merged. Reload the file (dropping the unsaved edits), or keep the open copy and save over the file.",
                    entry.file,
                    plural(n),
                    plural(n)
                ),
                "merge_conflict",
            )
            .with_details(Value::from(details))
            .into());
        }
        // Both sides' first layers fold into one list; what can't be folded clashes.
        let folded = if is_scene { fold_scene(&merged.text) } else { crate::merge::Folded { text: merged.text.clone(), clashes: vec![] } };
        if !folded.clashes.is_empty() {
            return Err(CommandError::new(
                format!(
                    "Both the open copy and {} added {}; nothing merged. Reload the file (dropping the unsaved edits), or keep the open copy and save over the file.",
                    entry.file,
                    folded.clashes.join(", ")
                ),
                "merge_conflict",
            )
            .with_details(Value::from(folded.clashes.clone()))
            .into());
        }
        let text = untrailing(&folded.text);
        let theirs_data = import_source(&theirs).map_err(Error::Internal)?;
        if is_scene {
            // A layer both sides made under the same id must hold the same items.
            let base_ids = if base.is_empty() {
                Default::default()
            } else {
                import_source(&base).map(|b| ids_of(b.get("layers").unwrap_or(&Value::Null))).unwrap_or_default()
            };
            let their_ids = ids_of(theirs_data.get("layers").unwrap_or(&Value::Null));
            let members = |doc: &Value, id: &str| -> String {
                let mut list: Vec<String> = placed(doc)
                    .into_iter()
                    .filter(|item| item.get("layer").and_then(Value::as_str) == Some(id))
                    .map(|item| stringify(&item))
                    .collect();
                list.sort();
                list.join("\n")
            };
            let ours_ids: Vec<String> = entry
                .doc
                .get("layers")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|l| l.get("id").and_then(Value::as_str).map(str::to_string))
                .collect();
            let differ: Vec<String> = ours_ids
                .into_iter()
                .filter(|id| their_ids.contains(id) && !base_ids.contains(id))
                .filter(|id| members(&entry.doc, id) != members(&theirs_data, id))
                .collect();
            if !differ.is_empty() {
                return Err(CommandError::new(
                    format!(
                        "Both the open copy and {} made layer {}, holding different items; nothing merged. Reload the file (dropping the unsaved edits), or keep the open copy and save over the file.",
                        entry.file,
                        differ.join(", ")
                    ),
                    "merge_conflict",
                )
                .with_details(Value::from(differ.iter().map(|id| format!("layer {id}")).collect::<Vec<_>>()))
                .into());
            }
        }
        let mut data = import_source(&text).map_err(|e| {
            CommandError::new(
                format!("the merge of {} doesn't load ({}); nothing merged", entry.file, e.lines().next().unwrap_or("")),
                "merge_invalid",
            )
        })?;
        // A merge that loads but breaks the doc is refused.
        let added = self.new_problems(&entry, &data, &[entry.doc.clone(), theirs_data])?;
        if !added.is_empty() {
            return Err(CommandError::new(
                format!(
                    "the merge of {} would add {} problem{} ({}); nothing merged",
                    entry.file,
                    added.len(),
                    plural(added.len()),
                    added[0]
                ),
                "merge_invalid",
            )
            .with_details(Value::from(added))
            .into());
        }
        let before = stringify(&entry.doc);
        // The merge keeps the list in depth order: a line merge can't judge overlaps.
        let sorted = if is_scene { Some(self.sort_doc(&mut data)?) } else { None };
        let changed = stringify(&data) != before;
        // The base is the file now; the merge's step sits at the base, so undoing it is a change.
        let keep = entry.base_seq;
        let undone = entry.undone.clone();
        {
            let e = self.entries.get_mut(ref_).expect("open");
            e.doc = data;
            e.undo.push(Step {
                before,
                labels: vec![format!("Merged the changes to {} from disk", entry.file)],
                seq: keep,
                author: Author::of(crate::author::AuthorKind::Outside),
            });
            if e.undo.len() > UNDO_LIMIT {
                e.undo.remove(0);
            }
            e.redo.clear();
        }
        self.match_disk(ref_, base_of(&disk), theirs, Some(keep));
        self.entries.get_mut(ref_).expect("open").undone = undone;
        self.invalidate();
        self.changed(ref_);
        let moved = sorted.as_ref().map_or(0, |s| s.moved.len());
        let note = if moved > 0 { "; the result was sorted by depth" } else { "" };
        let mut out = obj(vec![
            ("doc", Value::String(ref_.to_string())),
            ("merged", Value::Bool(true)),
            ("changed", Value::Bool(changed)),
            ("summary", Value::String(format!("Merged the changes to {} into {ref_}{note}", entry.file))),
        ]);
        if moved > 0 {
            out.insert("sorted".into(), Value::from(moved));
        }
        Ok(Value::Object(spread(out, self.status(ref_))))
    }

    /// Closes a doc (`close`); a dirty one only with `force`.
    pub fn close(&mut self, ref_: &str, force: bool) -> Result<Value> {
        self.entry(ref_)?;
        if self.is_dirty(ref_) && !force {
            return Err(CommandError::new(
                format!("{ref_} has unsaved changes; save or revert it, or force the close to drop them"),
                "dirty",
            )
            .into());
        }
        self.entries.delete(ref_);
        self.invalidate();
        Ok(json!({ "doc": ref_, "closed": true }))
    }

    /// Every open doc's status (`list`).
    pub fn list(&self) -> Vec<Value> {
        self.entries.keys().map(|r| Value::Object(self.status(r))).collect()
    }

    /// The refs of the open docs with unsaved edits.
    pub fn dirty_refs(&self) -> Vec<String> {
        self.entries.keys().filter(|r| self.is_dirty(r)).cloned().collect()
    }
}

/// What saving would write (`plan`'s result).
#[derive(Clone, Debug)]
pub struct Plan {
    pub text: String,
    pub disk: Disk,
    pub problems: Vec<String>,
    pub conflict: Option<&'static str>,
    pub diff: String,
}

fn base_of(disk: &Disk) -> Base {
    Base { exists: disk.exists, hash: disk.hash.clone(), text: disk.text.clone() }
}

/// The canonical text of a doc, or None if it isn't a record (`serialize`).
pub fn serialize(doc: &Value) -> Option<String> {
    doc.is_object().then(|| format::serialize(doc))
}

/// A source text as the canonical format would write it, or None if it
/// doesn't load (`canonical`).
pub fn canonical(text: &str) -> Option<String> {
    import_source(text).ok().and_then(|v| serialize(&v))
}

/// Whether the file changed on disk since the doc's base (`isStale`).
pub fn is_stale(entry: &Entry, disk: &Disk) -> bool {
    if entry.base.exists {
        disk.hash != entry.base.hash
    } else {
        disk.exists
    }
}

/// The edits since the doc last matched its file, in order (`edits`).
pub fn edits(entry: &Entry) -> Vec<String> {
    let mut out: Vec<String> = entry.created.iter().cloned().collect();
    out.extend(entry.undone.iter().cloned());
    out.extend(entry.undo.iter().filter(|s| s.seq > entry.base_seq).flat_map(|s| s.labels.iter().cloned()));
    out
}
