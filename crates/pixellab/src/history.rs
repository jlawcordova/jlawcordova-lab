//! A document's version history, in the state dir (docs/intents/2026-10-autosave/spec.md, D3).
//!
//! ```text
//! history/<key>/            key: the first 16 hex of sha256 of the document's absolute path
//!   document.json           { path, createdAt }
//!   versions.jsonl          one version per line
//!   blobs/<sha256>.json     a source file's text, by its hash
//!   lock                    held (File::lock) while a writer changes this folder
//! ```
//!
//! A version is `{ number, startedAt, endedAt, endedMs, author, edits, changes: [{ file, before, after }], open }`,
//! `before` and `after` being blob hashes or null for a file that didn't exist. Version 0 (author
//! `opened`) holds every scene and object as it was when the document was first opened with no
//! history, so the state at any version is version 0 plus the changes up to it.
//!
//! Every writer records here: the app's server, an MCP server and the CLI when no app owns the
//! document. Writes by one author run into one open version until someone else writes, the author
//! pauses for a minute (IDLE_MS), or the version is marked. Thinning folds versions older than a
//! day into one a day, and older than 30 days into one a week, keeping each kept version's state.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use engine::format::stringify;
use serde_json::{json, Map, Value};

use crate::author::{Author, AuthorKind};
use crate::errors::Result;
use crate::state::{iso_time, write_atomic};

/// How long an author may pause before their next write starts a new version.
pub const IDLE_MS: f64 = 60_000.0;
const DAY_MS: f64 = 86_400_000.0;
const WEEK_MS: f64 = 7.0 * DAY_MS;
/// How many edit labels a version keeps (a long drag is hundreds of steps).
const EDITS_KEPT: usize = 200;

/// The key of a document's history: its absolute path, hashed.
pub fn key_of(root: &Path) -> String {
    let path = crate::state::real_path(root);
    engine::sha256::hex(path.to_string_lossy().as_bytes())[..16].to_string()
}

fn hash_of(text: &str) -> String {
    engine::sha256::hex(text.as_bytes())
}

/// One file's change in a version: its text before and after (None: no file).
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub file: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// A document's history folder.
#[derive(Clone, Debug)]
pub struct History {
    pub dir: PathBuf,
    pub root: PathBuf,
}

/// Holds a history folder's lock while it lives.
struct Locked(#[allow(dead_code)] fs::File);

impl History {
    /// The history of the document at `root`, in `state_dir`. Nothing is written until a version is.
    pub fn new(state_dir: &Path, root: &Path) -> History {
        History { dir: state_dir.join("history").join(key_of(root)), root: crate::state::real_path(root) }
    }

    fn versions_file(&self) -> PathBuf {
        self.dir.join("versions.jsonl")
    }

    fn blobs(&self) -> PathBuf {
        self.dir.join("blobs")
    }

    fn lock(&self) -> Result<Locked> {
        fs::create_dir_all(self.blobs())?;
        let file = fs::OpenOptions::new().create(true).truncate(false).write(true).open(self.dir.join("lock"))?;
        file.lock()?;
        Ok(Locked(file))
    }

    /// Every version, oldest first.
    pub fn versions(&self) -> Vec<Value> {
        let Ok(text) = fs::read_to_string(self.versions_file()) else { return Vec::new() };
        text.lines().filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect()
    }

    fn write_versions(&self, versions: &[Value]) -> Result<()> {
        let mut text = String::new();
        for v in versions {
            text.push_str(&stringify(v));
            text.push('\n');
        }
        write_atomic(&self.versions_file(), &text)
    }

    fn put_blob(&self, text: &str) -> Result<String> {
        let hash = hash_of(text);
        let path = self.blobs().join(format!("{hash}.json"));
        if !path.exists() {
            write_atomic(&path, text)?;
        }
        Ok(hash)
    }

    /// A blob's text.
    pub fn blob(&self, hash: &str) -> Option<String> {
        fs::read_to_string(self.blobs().join(format!("{hash}.json"))).ok()
    }

    fn blob_of(&self, text: &Option<String>) -> Result<Value> {
        Ok(match text {
            Some(t) => Value::String(self.put_blob(t)?),
            None => Value::Null,
        })
    }

    /// Writes version 0 from the document's sources when there's no history yet.
    pub fn ensure_base(&self, source_dir: &Path, now: f64) -> Result<()> {
        if self.versions_file().exists() {
            return Ok(());
        }
        let _lock = self.lock()?;
        if self.versions_file().exists() {
            return Ok(());
        }
        let mut changes = Vec::new();
        for (file, text) in source_files(source_dir) {
            changes.push(json!({ "file": file, "before": Value::Null, "after": self.put_blob(&text)? }));
        }
        write_atomic(
            &self.dir.join("document.json"),
            &stringify(&json!({ "path": self.root.to_string_lossy(), "createdAt": iso_time(now) })),
        )?;
        let base = json!({
            "number": 0,
            "startedAt": iso_time(now),
            "endedAt": iso_time(now),
            "endedMs": now,
            "author": Author::of(AuthorKind::Opened).to_json(),
            "edits": ["Opened"],
            "changes": changes,
            "open": false,
        });
        self.write_versions(&[base])
    }

    /// Records changes by an author: the open version grows when it's the same author's and
    /// they haven't paused; otherwise a new version starts. Returns the version.
    pub fn record(&self, changes: &[Change], author: &Author, edits: &[String], now: f64) -> Result<Value> {
        let _lock = self.lock()?;
        let mut versions = self.versions();
        let extend = versions.last().is_some_and(|last| {
            last["open"] == json!(true)
                && Author::from_json(last.get("author")).same(author)
                && now - last["endedMs"].as_f64().unwrap_or(0.0) < IDLE_MS
        });
        if !extend {
            if let Some(last) = versions.last_mut() {
                last["open"] = json!(false);
            }
            let number = versions.last().and_then(|v| v["number"].as_u64()).map_or(0, |n| n + 1);
            versions.push(json!({
                "number": number,
                "startedAt": iso_time(now),
                "endedAt": iso_time(now),
                "endedMs": now,
                "author": author.to_json(),
                "edits": [],
                "changes": [],
                "open": true,
            }));
        }
        let mut version = versions.pop().expect("a version");
        for change in changes {
            let before = self.blob_of(&change.before)?;
            let after = self.blob_of(&change.after)?;
            let list = version["changes"].as_array_mut().expect("changes");
            match list.iter_mut().find(|c| c["file"] == json!(change.file)) {
                Some(c) => c["after"] = after,
                None => list.push(json!({ "file": change.file, "before": before, "after": after })),
            }
        }
        // A file whose edits cancelled out isn't a change.
        version["changes"].as_array_mut().expect("changes").retain(|c| c["before"] != c["after"]);
        let list = version["edits"].as_array_mut().expect("edits");
        list.extend(edits.iter().map(|e| json!(e)));
        if list.len() > EDITS_KEPT {
            let extra = list.len() - EDITS_KEPT;
            list.drain(..extra);
        }
        version["endedAt"] = json!(iso_time(now));
        version["endedMs"] = json!(now);
        versions.push(version.clone());
        self.write_versions(&versions)?;
        Ok(version)
    }

    /// Closes the open version, so the next write starts a new one. Returns it, if there was one.
    pub fn mark(&self, label: Option<&str>) -> Result<Option<Value>> {
        if !self.versions_file().exists() {
            return Ok(None);
        }
        let _lock = self.lock()?;
        let mut versions = self.versions();
        let Some(last) = versions.last_mut() else { return Ok(None) };
        if last["open"] != json!(true) {
            return Ok(None);
        }
        last["open"] = json!(false);
        if let Some(l) = label.filter(|l| !l.trim().is_empty()) {
            last["label"] = json!(l);
        }
        let out = last.clone();
        self.write_versions(&versions)?;
        Ok(Some(out))
    }

    /// The version with this number.
    pub fn version(&self, number: u64) -> Option<Value> {
        self.versions().into_iter().find(|v| v["number"].as_u64() == Some(number))
    }

    /// The newest version's number.
    pub fn latest(&self) -> Option<u64> {
        self.versions().last().and_then(|v| v["number"].as_u64())
    }

    /// Every file's blob after a version (None: the file didn't exist then).
    pub fn files_at(&self, number: u64) -> BTreeMap<String, Option<String>> {
        let mut out = BTreeMap::new();
        for v in self.versions() {
            if v["number"].as_u64().is_some_and(|n| n > number) {
                break;
            }
            for c in v["changes"].as_array().into_iter().flatten() {
                let file = c["file"].as_str().unwrap_or_default().to_string();
                out.insert(file, c["after"].as_str().map(str::to_string));
            }
        }
        out
    }

    /// A file's text after a version: Some(None) when it didn't exist, None when the history never saw it.
    pub fn text_at(&self, number: u64, file: &str) -> Option<Option<String>> {
        let files = self.files_at(number);
        let hash = files.get(file)?;
        Some(hash.as_ref().and_then(|h| self.blob(h)))
    }

    /// Folds old versions together (R10) and removes blobs no version needs. Returns how many versions went.
    pub fn thin(&self, now: f64) -> Result<usize> {
        if !self.versions_file().exists() {
            return Ok(0);
        }
        let _lock = self.lock()?;
        let versions = self.versions();
        let bucket = |v: &Value| -> Option<(char, i64)> {
            if v["number"].as_u64() == Some(0) || v["open"] == json!(true) {
                return None;
            }
            let ended = v["endedMs"].as_f64().unwrap_or(now);
            let age = now - ended;
            if age < DAY_MS {
                None
            } else if age < 30.0 * DAY_MS {
                Some(('d', (ended / DAY_MS).floor() as i64))
            } else {
                Some(('w', (ended / WEEK_MS).floor() as i64))
            }
        };
        let mut out: Vec<Value> = Vec::new();
        let mut last_bucket: Option<(char, i64)> = None;
        for v in versions.iter().cloned() {
            let b = bucket(&v);
            match (b, last_bucket, out.last_mut()) {
                (Some(b), Some(lb), Some(prev)) if b == lb => fold_into(prev, &v),
                _ => out.push(v),
            }
            last_bucket = b;
        }
        let gone = versions.len() - out.len();
        if gone > 0 {
            self.write_versions(&out)?;
        }
        let used: BTreeSet<String> = out
            .iter()
            .flat_map(|v| v["changes"].as_array().cloned().unwrap_or_default())
            .flat_map(|c| [c["before"].as_str().map(str::to_string), c["after"].as_str().map(str::to_string)])
            .flatten()
            .collect();
        if let Ok(list) = fs::read_dir(self.blobs()) {
            for e in list.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if let Some(hash) = name.strip_suffix(".json") {
                    if !used.contains(hash) {
                        let _ = fs::remove_file(e.path());
                    }
                }
            }
        }
        Ok(gone)
    }
}

/// Folds `next` into `prev`: the first `before` and last `after` of each file, every edit, and
/// `several` authors when they differ. The folded version keeps `next`'s number and end.
fn fold_into(prev: &mut Value, next: &Value) {
    let mut changes: Vec<Value> = prev["changes"].as_array().cloned().unwrap_or_default();
    for c in next["changes"].as_array().into_iter().flatten() {
        match changes.iter_mut().find(|p| p["file"] == c["file"]) {
            Some(p) => p["after"] = c["after"].clone(),
            None => changes.push(c.clone()),
        }
    }
    changes.retain(|c| c["before"] != c["after"]);
    prev["changes"] = Value::Array(changes);
    let mut edits: Vec<Value> = prev["edits"].as_array().cloned().unwrap_or_default();
    edits.extend(next["edits"].as_array().cloned().unwrap_or_default());
    if edits.len() > EDITS_KEPT {
        let extra = edits.len() - EDITS_KEPT;
        edits.drain(..extra);
    }
    prev["edits"] = Value::Array(edits);
    let a = Author::from_json(prev.get("author"));
    let b = Author::from_json(next.get("author"));
    if !a.same(&b) {
        let mut authors: Vec<Value> = match prev.get("authors").and_then(Value::as_array) {
            Some(list) => list.clone(),
            None => vec![a.to_json()],
        };
        if !authors.iter().any(|x| Author::from_json(Some(x)).same(&b)) {
            authors.push(b.to_json());
        }
        prev["author"] = Author::of(AuthorKind::Several).to_json();
        prev["authors"] = Value::Array(authors);
    }
    for k in ["number", "endedAt", "endedMs"] {
        prev[k] = next[k].clone();
    }
}

/// The scenes and objects of a source dir, by their file name in it.
pub fn source_files(source_dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for dir in ["objects", "scenes"] {
        let Ok(list) = fs::read_dir(source_dir.join(dir)) else { continue };
        let mut names: Vec<String> = list
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".json") && !n.starts_with('.'))
            .collect();
        names.sort();
        for n in names {
            if let Ok(text) = fs::read_to_string(source_dir.join(dir).join(&n)) {
                out.push((format!("{dir}/{n}"), text));
            }
        }
    }
    out
}

/// A version as `history.list` shows it: without its blob hashes.
pub fn summary(v: &Value) -> Value {
    let mut m = Map::new();
    for k in ["number", "startedAt", "endedAt", "author", "authors", "edits", "open", "label"] {
        if let Some(x) = v.get(k) {
            m.insert(k.into(), x.clone());
        }
    }
    let files: Vec<Value> = v["changes"].as_array().into_iter().flatten().map(|c| c["file"].clone()).collect();
    m.insert("files".into(), Value::Array(files));
    Value::Object(m)
}
