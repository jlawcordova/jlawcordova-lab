//! The version history's commands (docs/intents/2026-10-autosave/spec.md, D4): list, show and
//! render a version, restore the document or one doc to it, undo just what it changed, and mark
//! the open version closed. Restore and undo are one undo step per doc, saved at once, and become
//! a version of their own (author `restore`), so going back can be undone too.

use engine::engine::{load_sources, Sources};
use serde_json::{json, Map, Value};

use super::common::{flag, obj, opt_num, opt_str, str_of};
use crate::author::{Author, AuthorKind};
use crate::context::{doc_file, Ctx, DocKind, Loaded};
use crate::diff::unified_diff;
use crate::errors::{CommandError, Result};
use crate::history::summary;
use crate::merge::{fold_scene, merge3, trailing, untrailing};
use crate::registry::{Command, Params};
use crate::render::{render_object_image, render_scene_image, ObjectImageOptions, SceneImageOptions, Theme, MAX_SCALE};
use crate::session::{canonical, import_source, Session};

fn version_param() -> Value {
    json!({ "type": "integer", "minimum": 0, "description": "the version's number (history.list)" })
}

fn doc_param() -> Value {
    json!({ "type": "string", "description": "a scene or object: its name, scene:<name> or object:<name>, or scenes/<name>.json" })
}

/// A doc's kind and name from a file in the document (`scenes/x.json`).
pub fn doc_of_file(file: &str) -> Option<(DocKind, String)> {
    if file == engine::format::PALETTE_FILE {
        return Some((DocKind::Palette, String::new()));
    }
    let (dir, rest) = file.split_once('/')?;
    let name = rest.strip_suffix(".json")?.to_string();
    match dir {
        "scenes" => Some((DocKind::Scene, name)),
        "objects" => Some((DocKind::Object, name)),
        _ => None,
    }
}

/// The file a doc param names: scene:<name>, object:<name>, a file, or a name the session finds
/// (a doc that's gone since is found by its file in the history).
fn file_of(session: &mut Session, query: &str) -> Result<String> {
    if let Some((kind, name)) = query.split_once(':') {
        if let Some(k) = DocKind::parse(kind).filter(|k| *k != DocKind::Palette) {
            return Ok(doc_file(k, name));
        }
    }
    if doc_of_file(query).is_some() {
        return Ok(query.to_string());
    }
    let with_json = if query.ends_with(".json") { query.to_string() } else { format!("{query}.json") };
    if doc_of_file(&with_json).is_some() {
        return Ok(with_json);
    }
    match session.find(query, None) {
        Ok(found) => Ok(found.file),
        Err(e) => {
            let known = session.history.latest().map(|n| session.history.files_at(n)).unwrap_or_default();
            for kind in [DocKind::Scene, DocKind::Object] {
                let f = doc_file(kind, query);
                if known.contains_key(&f) {
                    return Ok(f);
                }
            }
            Err(e)
        }
    }
}

fn the_version(session: &Session, params: &Params) -> Result<Value> {
    let n = opt_num(params, "version").unwrap_or(-1.0);
    if n < 0.0 {
        return Err(CommandError::usage("say which version (history.list lists them)").into());
    }
    session
        .history
        .version(n as u64)
        .ok_or_else(|| CommandError::not_found(format!("no version {n} in this document's history (history.list lists them)")).into())
}

fn requester(session: &Session) -> Author {
    let who = session.author.clone();
    Author { kind: AuthorKind::Restore, name: Some(who.display()), chat: who.chat.clone() }
}

// ------------------------------------------------------------------ list

fn list_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let limit = opt_num(&params, "limit").unwrap_or(50.0) as usize;
    let session = ctx.session()?;
    let file = match opt_str(&params, "doc") {
        Some(d) => Some(file_of(session, d)?),
        None => None,
    };
    let mut list: Vec<Value> = session
        .history
        .versions()
        .iter()
        .rev()
        .filter(|v| file.as_ref().is_none_or(|f| v["changes"].as_array().into_iter().flatten().any(|c| c["file"].as_str() == Some(f))))
        .map(summary)
        .collect();
    let total = list.len();
    list.truncate(limit);
    Ok(json!({ "versions": list, "total": total }))
}

fn author_text(a: &Value) -> String {
    Author::from_json(Some(a)).display()
}

fn list_format(r: &Value, _: &Params) -> String {
    let lines: Vec<String> = r["versions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| {
            let edits = v["edits"].as_array().cloned().unwrap_or_default();
            let first = edits.first().map(|e| str_of(Some(e))).unwrap_or_default();
            let more = if edits.len() > 1 { format!(" (+{} more)", edits.len() - 1) } else { String::new() };
            format!(
                "#{}  {}  {}{}  {}{more}  [{}]",
                str_of(v.get("number")),
                str_of(v.get("endedAt")),
                author_text(&v["author"]),
                if v["open"] == json!(true) { " (now)" } else { "" },
                first,
                v["files"].as_array().map(|f| f.iter().map(|x| str_of(Some(x))).collect::<Vec<_>>().join(", ")).unwrap_or_default()
            )
        })
        .collect();
    if lines.is_empty() {
        "no versions".into()
    } else {
        lines.join("\n")
    }
}

// ------------------------------------------------------------------ show

fn show_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let session = ctx.session()?;
    let v = the_version(session, &params)?;
    let only = match opt_str(&params, "doc") {
        Some(d) => Some(file_of(session, d)?),
        None => None,
    };
    let mut diffs = Vec::new();
    for c in v["changes"].as_array().into_iter().flatten() {
        let file = str_of(c.get("file"));
        if only.as_ref().is_some_and(|o| *o != file) {
            continue;
        }
        let text = |h: &Value| h.as_str().and_then(|h| session.history.blob(h)).unwrap_or_default();
        let diff = unified_diff(&text(&c["before"]), &text(&c["after"]), &format!("a/{file}"), &format!("b/{file}"), 3);
        diffs.push(json!({ "file": file, "created": c["before"].is_null(), "deleted": c["after"].is_null(), "diff": diff }));
    }
    let mut out = match summary(&v) {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    out.insert("diffs".into(), Value::Array(diffs));
    Ok(Value::Object(out))
}

fn show_format(r: &Value, _: &Params) -> String {
    let mut lines = vec![format!("#{}  {}  {}", str_of(r.get("number")), str_of(r.get("endedAt")), author_text(&r["author"]))];
    for e in r["edits"].as_array().into_iter().flatten() {
        lines.push(format!("  {}", str_of(Some(e))));
    }
    for d in r["diffs"].as_array().into_iter().flatten() {
        lines.push(str_of(d.get("diff")).trim_end().to_string());
    }
    lines.join("\n")
}

// ---------------------------------------------------------------- render

/// The session's sources with one doc as it was at a version.
fn overlay(session: &mut Session, kind: DocKind, name: &str, data: Option<Value>) -> Result<Loaded> {
    let loaded = session.loaded()?;
    let mut objects = loaded.sources.objects.clone();
    let mut scenes = loaded.sources.scenes.clone();
    let list = if kind == DocKind::Scene { &mut scenes } else { &mut objects };
    match data {
        Some(d) => {
            list.insert(name.to_string(), d);
        }
        None => {
            list.remove(name);
        }
    }
    let sources: Sources = load_sources(loaded.sources.palette.clone(), objects, scenes);
    Ok(Loaded { sources, load_errors: loaded.load_errors.clone() })
}

fn render_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let session = ctx.session()?;
    let v = the_version(session, &params)?;
    let n = v["number"].as_u64().unwrap_or(0);
    let file = file_of(session, opt_str(&params, "doc").unwrap_or(""))?;
    let Some((kind, name)) = doc_of_file(&file).filter(|(k, _)| *k != DocKind::Palette) else {
        return Err(CommandError::usage(format!("{file} isn't a scene or an object")).into());
    };
    let Some(Some(text)) = session.history.text_at(n, &file) else {
        return Err(CommandError::not_found(format!("{file} didn't exist at version {n}")).into());
    };
    let data = import_source(&text).map_err(|e| CommandError::new(format!("{file} at version {n} doesn't load: {e}"), "load_error"))?;
    let view = overlay(session, kind, &name, Some(data))?;
    let scale = opt_num(&params, "scale").unwrap_or(1.0) as u32;
    let time = opt_num(&params, "time");
    let image = if kind == DocKind::Scene {
        let theme = if opt_str(&params, "theme") == Some("dark") { Theme::Dark } else { Theme::Light };
        let options = SceneImageOptions { scale, time: time.unwrap_or(0.0), theme, ..SceneImageOptions::default() };
        render_scene_image(&view.sources, &name, &session.document.dark_page, &options)?
    } else {
        let options = ObjectImageOptions {
            scale,
            time,
            frame: opt_num(&params, "frame").unwrap_or(0.0) as usize,
            layers: params
                .get("layers")
                .and_then(Value::as_array)
                .map(|l| l.iter().filter_map(Value::as_u64).map(|n| n as usize).collect()),
            play: None,
        };
        render_object_image(&view.sources, &name, &options)?
    };
    let mut out = obj(vec![("version", json!(n)), ("doc", json!(format!("{}:{name}", kind.as_str()))), ("kind", json!(kind.as_str()))]);
    if let Value::Object(m) = image {
        out.extend(m);
    }
    Ok(Value::Object(out))
}

fn render_format(r: &Value, _: &Params) -> String {
    format!(
        "{} at version {}: {}×{} PNG at {}×",
        str_of(r.get("doc")),
        str_of(r.get("version")),
        str_of(r.get("width")),
        str_of(r.get("height")),
        str_of(r.get("scale"))
    )
}

// --------------------------------------------------------------- restore

/// Puts each file to its text (None: removes it), saves, and records it all as one version by
/// the requester. Returns the docs written and the files removed. `once`: only the first file
/// carries the label, so the version lists it once however many files it holds (an import).
pub(crate) fn put_files(
    session: &mut Session,
    targets: Vec<(String, Option<String>)>,
    label: &str,
    force: bool,
    once: bool,
) -> Result<(Vec<String>, Vec<String>)> {
    let author = requester(session);
    let who = std::mem::replace(&mut session.author, author);
    let _ = session.history.mark(None);
    let result = (|| -> Result<(Vec<String>, Vec<String>)> {
        let mut written = Vec::new();
        let mut removed = Vec::new();
        for (n, (file, text)) in targets.into_iter().enumerate() {
            let Some((kind, name)) = doc_of_file(&file) else { continue };
            let label = if once && n > 0 { "" } else { label };
            match text {
                Some(t) => {
                    let data = import_source(&t).map_err(|e| CommandError::new(format!("{file} doesn't load: {e}"), "load_error"))?;
                    let ref_ = session.set_doc(kind, &name, data, label)?;
                    session.save(&ref_, force)?;
                    written.push(ref_);
                }
                // The palette is never deleted: a document always has one.
                None if kind == DocKind::Palette => {}
                None => {
                    session.delete_doc(kind, &name, label)?;
                    removed.push(file);
                }
            }
        }
        Ok((written, removed))
    })();
    let _ = session.history.mark(None);
    session.author = who;
    result
}

fn same_text(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x == y || canonical(x).is_some_and(|c| Some(c) == canonical(y)),
        _ => false,
    }
}

fn restore_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let session = ctx.session()?;
    let v = the_version(session, &params)?;
    let n = v["number"].as_u64().unwrap_or(0);
    let only = match opt_str(&params, "doc") {
        Some(d) => Some(file_of(session, d)?),
        None => None,
    };
    let then = session.history.files_at(n);
    let mut files: std::collections::BTreeSet<String> = then.keys().cloned().collect();
    if let Some(latest) = session.history.latest() {
        files.extend(session.history.files_at(latest).into_keys());
    }
    files.extend(crate::history::source_files(&session.document.source_dir).into_iter().map(|(f, _)| f));
    let mut targets = Vec::new();
    for file in files {
        if only.as_ref().is_some_and(|o| *o != file) {
            continue;
        }
        let Some((kind, name)) = doc_of_file(&file) else { continue };
        let target = then.get(&file).cloned().flatten().and_then(|h| session.history.blob(&h));
        let now = session.current_text(kind, &name);
        if !same_text(&target, &now) {
            targets.push((file, target));
        }
    }
    let label = format!("Restored to version {n}");
    let (written, removed) = put_files(session, targets, &label, true, false)?;
    let changed = !written.is_empty() || !removed.is_empty();
    let summary = if changed {
        format!("{label}: {} doc{} written, {} removed", written.len(), if written.len() == 1 { "" } else { "s" }, removed.len())
    } else {
        format!("Already as it was at version {n}")
    };
    Ok(json!({ "version": n, "changed": changed, "written": written, "removed": removed, "summary": summary }))
}

fn summary_format(r: &Value, _: &Params) -> String {
    str_of(r.get("summary"))
}

// ------------------------------------------------------------------ undo

fn undo_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let session = ctx.session()?;
    let v = the_version(session, &params)?;
    let n = v["number"].as_u64().unwrap_or(0);
    if n == 0 {
        return Err(CommandError::new(
            "version 0 is the document as it was first opened; restore an earlier state with history.restore",
            "usage",
        )
        .into());
    }
    let mut targets = Vec::new();
    let mut clashes = Vec::new();
    for c in v["changes"].as_array().into_iter().flatten() {
        let file = str_of(c.get("file"));
        let Some((kind, name)) = doc_of_file(&file) else { continue };
        let text = |h: &Value| h.as_str().and_then(|h| session.history.blob(h));
        let (before, after) = (text(&c["before"]), text(&c["after"]));
        let now = session.current_text(kind, &name);
        if same_text(&now, &after) {
            targets.push((file, before));
            continue;
        }
        match (&before, &after, &now) {
            (Some(theirs), Some(base), Some(mine)) => {
                let (base, mine, theirs) = (
                    canonical(base).unwrap_or(base.clone()),
                    canonical(mine).unwrap_or(mine.clone()),
                    canonical(theirs).unwrap_or(theirs.clone()),
                );
                let is_scene = kind == DocKind::Scene;
                let merged = merge3(&trailing(&base), &trailing(&mine), &trailing(&theirs), is_scene);
                let folded =
                    if is_scene { fold_scene(&merged.text) } else { crate::merge::Folded { text: merged.text.clone(), clashes: vec![] } };
                let text = untrailing(&folded.text);
                if !merged.conflicts.is_empty() || !folded.clashes.is_empty() || import_source(&text).is_err() {
                    clashes.push(file);
                } else {
                    targets.push((file, Some(text)));
                }
            }
            // Made, deleted or changed again in ways a line merge can't take back.
            _ => clashes.push(file),
        }
    }
    if !clashes.is_empty() {
        return Err(CommandError::new(
            format!(
                "Version {n}'s changes can't be taken back: {} changed the same lines since. Restore a version instead (history.restore), or undo those edits first.",
                clashes.join(", ")
            ),
            "clash",
        )
        .with_details(Value::from(clashes))
        .into());
    }
    let first = v["edits"].as_array().and_then(|e| e.first()).map(|e| str_of(Some(e))).unwrap_or_default();
    let label = if first.is_empty() { format!("Undid version {n}") } else { format!("Undid version {n}: {first}") };
    let (written, removed) = put_files(session, targets, &label, flag(&params, "force"), false)?;
    let changed = !written.is_empty() || !removed.is_empty();
    Ok(json!({ "version": n, "changed": changed, "written": written, "removed": removed, "summary": label }))
}

// ------------------------------------------------------------------ mark

fn mark_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let session = ctx.session()?;
    let marked = session.history.mark(opt_str(&params, "label"))?;
    Ok(match marked {
        Some(v) => json!({ "marked": true, "version": v["number"], "summary": format!("Closed version {}", str_of(v.get("number"))) }),
        None => json!({ "marked": false, "summary": "No version was open" }),
    })
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut list = Command::new(
        "history.list",
        "List the document's versions, newest first",
        json!({
            "type": "object",
            "properties": {
                "doc": doc_param(),
                "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 50, "description": "how many versions" }
            }
        }),
        false,
        list_run,
    );
    list.description = Some(concat!(
        "Each is { number, startedAt, endedAt, author: { kind, name?, chat? }, edits, files, open }. A version is a stretch of one author's writes ",
        "(owner, agent, cli, outside, restore; several once thinned). open marks the version still growing. With doc, only versions that changed it."
    ));
    list.format = Some(list_format);

    let mut show = Command::new(
        "history.show",
        "Show a version: its edits and a diff of each file it changed",
        json!({ "type": "object", "properties": { "version": version_param(), "doc": doc_param() }, "required": ["version"] }),
        false,
        show_run,
    );
    show.args = vec!["version"];
    show.format = Some(show_format);

    let mut render = Command::new(
        "history.render",
        "Render a scene or object as it was after a version, without writing anything",
        json!({
            "type": "object",
            "properties": {
                "version": version_param(),
                "doc": doc_param(),
                "scale": { "type": "integer", "minimum": 1, "maximum": MAX_SCALE, "default": 1, "description": "whole-pixel zoom of the PNG" },
                "time": { "type": "integer", "minimum": 0, "description": "ms into the motion and loops (a scene: default 0; an object: overrides frame)" },
                "frame": { "type": "integer", "minimum": 0, "default": 0, "description": "an object's frame" },
                "layers": { "type": "array", "items": { "type": "integer", "minimum": 0 }, "description": "only these layers of a sprite" },
                "theme": { "type": "string", "enum": ["light", "dark"], "default": "light", "description": "a scene's theme" }
            },
            "required": ["version", "doc"]
        }),
        false,
        render_run,
    );
    render.description = Some(
        "The doc as it was after that version, over the rest of the document as it is now, rendered as scene.render or object.render renders (same params, same result shape), plus { version, doc, kind }. Pixel Lab draws it over the stage as a ghost.",
    );
    render.args = vec!["version", "doc"];
    render.writes_png = true;
    render.format = Some(render_format);

    let mut restore = Command::new(
        "history.restore",
        "Put the document (or one doc) back as it was after a version",
        json!({ "type": "object", "properties": { "version": version_param(), "doc": doc_param() }, "required": ["version"] }),
        true,
        restore_run,
    );
    restore.description = Some(concat!(
        "Every scene and object (or just doc) goes back to its text at that version: one undo step per doc, saved at once, and a version of its own, ",
        "so a restore can be undone too. A doc made since is removed; one removed since comes back."
    ));
    restore.args = vec!["version"];
    restore.writes_direct = true;
    restore.format = Some(summary_format);

    let mut undo = Command::new(
        "history.undo",
        "Take back just what one version changed, keeping everything since",
        json!({
            "type": "object",
            "properties": {
                "version": version_param(),
                "force": { "type": "boolean", "default": false, "description": "save even when the result has problems" }
            },
            "required": ["version"]
        }),
        true,
        undo_run,
    );
    undo.description = Some(concat!(
        "Each file the version changed is merged three ways (its after as the base, the doc now, its before), so later edits stay. ",
        "Refuses (code clash, the files in details) when later edits changed the same lines, writing nothing. One undo step per doc, saved, and a version of its own."
    ));
    undo.args = vec!["version"];
    undo.writes_direct = true;
    undo.format = Some(summary_format);

    let mut mark = Command::new(
        "history.mark",
        "Close the open version, so the next edit starts a new one",
        json!({ "type": "object", "properties": { "label": { "type": "string", "description": "a name for the version" } } }),
        true,
        mark_run,
    );
    mark.writes_direct = true;
    mark.format = Some(summary_format);

    vec![list, show, render, restore, undo, mark]
}
