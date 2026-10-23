//! The session commands: doc.open, doc.save, doc.undo, doc.redo and the rest (ports cli/lib/commands/doc.mjs).

use serde_json::{json, Map, Value};

use super::common::{doc_param, flag, opt_str, resolve_doc, str_of};
use crate::context::Ctx;
use crate::errors::Result;
use crate::registry::{Command, Params};

fn doc_params(more: Vec<(&str, Value)>) -> Value {
    let mut properties = Map::new();
    properties.insert("doc".into(), doc_param());
    for (k, v) in more {
        properties.insert(k.into(), v);
    }
    json!({ "type": "object", "properties": properties, "required": ["doc"] })
}

fn force_param(description: &str) -> (&'static str, Value) {
    ("force", json!({ "type": "boolean", "default": false, "description": description }))
}

/// The doc's ref, once it's open; opening it first if `open` (`entryOf`).
fn entry_of(ctx: &mut Ctx, query: &str, open: bool) -> Result<String> {
    let found = resolve_doc(ctx, query)?;
    let session = ctx.session()?;
    if open {
        session.open(found.kind, &found.name)?;
    }
    session.entry(&found.ref_)?;
    Ok(found.ref_)
}

fn status_line(s: &Value) -> String {
    format!(
        "{}: {}{}{}",
        str_of(s.get("doc")),
        if s["dirty"] == json!(true) { "unsaved changes" } else { "saved" },
        if s["canUndo"] == json!(true) { ", can undo" } else { "" },
        if s["canRedo"] == json!(true) { ", can redo" } else { "" }
    )
}

fn doc_of(params: &Params) -> &str {
    opt_str(params, "doc").unwrap_or("")
}

fn open_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let ref_ = entry_of(ctx, doc_of(&params), true)?;
    Ok(ctx.session()?.details(&ref_))
}

fn open_format(r: &Value, _: &Params) -> String {
    status_line(r)
}

fn status_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let Some(doc) = opt_str(&params, "doc") else {
        let session = ctx.session()?;
        return Ok(json!({ "docs": session.list() }));
    };
    let ref_ = entry_of(ctx, doc, false)?;
    Ok(ctx.session()?.details(&ref_))
}

fn status_format(r: &Value, _: &Params) -> String {
    if let Some(docs) = r.get("docs").and_then(Value::as_array) {
        let lines = docs.iter().map(status_line).collect::<Vec<_>>().join("\n");
        return if lines.is_empty() { "no docs open".to_string() } else { lines };
    }
    format!("{}{}", status_line(r), if r["stale"] == json!(true) { ", changed on disk" } else { "" })
}

fn undo_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let ref_ = entry_of(ctx, doc_of(&params), false)?;
    ctx.session()?.step(&ref_, true)
}

fn redo_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let ref_ = entry_of(ctx, doc_of(&params), false)?;
    ctx.session()?.step(&ref_, false)
}

fn summary_format(r: &Value, _: &Params) -> String {
    str_of(r.get("summary"))
}

fn save_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let ref_ = entry_of(ctx, doc_of(&params), false)?;
    let session = ctx.session()?;
    let saved = session.save(&ref_, flag(&params, "force"))?;
    // Saving by hand (⌘S) closes the version, as a Mac app's Save saves a version (R5).
    let _ = session.history.mark(None);
    Ok(saved)
}

/// doc.flush: saves now what autosave would save soon (docs/intents/2026-10-autosave/spec.md, D1).
fn flush_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let only = match opt_str(&params, "doc") {
        Some(d) => Some(resolve_doc(ctx, d)?.ref_),
        None => None,
    };
    let session = ctx.session()?;
    let refs: Vec<String> = match &only {
        Some(r) if session.entries.has(r) => vec![r.clone()],
        Some(_) => vec![],
        None => session.entries.keys().cloned().collect(),
    };
    let mut docs = Vec::new();
    for r in refs {
        if session.is_dirty(&r) {
            session.save_now(&r);
        }
        let (state, problems) = session.save_state(&r);
        docs.push(json!({ "doc": r, "saveState": state, "problems": problems }));
    }
    let saved = docs.iter().all(|d| d["saveState"] == "saved");
    Ok(json!({ "saved": saved, "docs": docs }))
}

fn flush_format(r: &Value, _: &Params) -> String {
    let lines: Vec<String> = r["docs"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| match str_of(d.get("saveState")).as_str() {
            "problems" => format!(
                "{}: not saved, {} problem{}",
                str_of(d.get("doc")),
                str_of(d.get("problems")),
                if d["problems"] == 1 { "" } else { "s" }
            ),
            "conflict" => format!("{}: not saved, its file changed on disk and clashes", str_of(d.get("doc"))),
            other => format!("{}: {other}", str_of(d.get("doc"))),
        })
        .collect();
    if lines.is_empty() {
        "no docs open".into()
    } else {
        lines.join("\n")
    }
}

fn save_format(r: &Value, _: &Params) -> String {
    let file = str_of(r.get("file"));
    format!(
        "{}: {}",
        str_of(r.get("doc")),
        if engine::engine::truthy(r.get("wrote")) { format!("wrote {file}") } else { format!("{file} unchanged") }
    )
}

fn revert_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let ref_ = entry_of(ctx, doc_of(&params), false)?;
    ctx.session()?.revert(&ref_)
}

fn reload_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let ref_ = entry_of(ctx, doc_of(&params), false)?;
    ctx.session()?.reload(&ref_)
}

fn merge_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let ref_ = entry_of(ctx, doc_of(&params), false)?;
    ctx.session()?.merge(&ref_)
}

fn close_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let ref_ = entry_of(ctx, doc_of(&params), false)?;
    let r = ctx.session()?.close(&ref_, flag(&params, "force"))?;
    ctx.invalidate();
    Ok(r)
}

fn close_format(r: &Value, _: &Params) -> String {
    format!("{}: closed", str_of(r.get("doc")))
}

fn session_command(
    name: &'static str,
    summary: &'static str,
    description: Option<&'static str>,
    params: Value,
    mutates: bool,
    run: fn(&mut Ctx, Params) -> Result<Value>,
    format: fn(&Value, &Params) -> String,
) -> Command {
    let mut c = Command::new(name, summary, params, mutates, run);
    c.description = description;
    c.args = vec!["doc"];
    c.session = true;
    c.format = Some(format);
    c
}

const SORT_NOTE: &str = "A scene edit and its depth sort are one step, so undoing it can give items other paths. When it does, the result has reordered: every path that changed, old → new (an item whose placement the step changed pairs with itself), for a caller holding paths.";

/// This file's commands.
pub fn commands() -> Vec<Command> {
    vec![
        session_command(
            "doc.open",
            "Open a scene or object as a working copy, with undo and redo",
            Some("Edits to an open doc save themselves (the app's server: about a second after the last one; MCP and the CLI: after each command). Editing a doc that isn't open opens it, so this is only needed to start from a clean status."),
            doc_params(vec![]),
            false,
            open_run,
            open_format,
        ),
        session_command(
            "doc.status",
            "Whether open docs have unsaved changes, and can undo or redo",
            Some(concat!(
                "With a doc: { doc, dirty, canUndo, canRedo, undoLabel, redoLabel, author, saveState, problems, file, exists, stale, undo: [edits], redo: [edits], edits }. ",
                "saveState: saved, pending (a save is coming), problems (it can't be saved until they're fixed) or conflict (its file changed on disk and clashes). ",
                "undoLabel and redoLabel name the step and, for anyone but the owner, whose it is. stale: the file changed on disk since the doc was opened or last matched it. ",
                "edits: the edits since the doc last matched its file. Without a doc: { docs: [{ doc, dirty, canUndo, canRedo }] }."
            )),
            json!({ "type": "object", "properties": { "doc": doc_param() } }),
            false,
            status_run,
            status_format,
        ),
        session_command(
            "doc.undo",
            "Undo the last edit to an open doc",
            Some(SORT_NOTE),
            doc_params(vec![]),
            true,
            undo_run,
            summary_format,
        ),
        session_command(
            "doc.redo",
            "Redo the last undone edit to an open doc",
            Some(SORT_NOTE),
            doc_params(vec![]),
            true,
            redo_run,
            summary_format,
        ),
        session_command(
            "doc.save",
            "Save an open doc now and close its version: validate, serialize with the engine and write its source",
            Some(concat!(
                "Edits save themselves, so this is the Mac's Save: it writes now and starts a new version for the next edit. ",
                "Refuses a doc with validation problems (code invalid, details: the problems) and a file that changed on disk since the doc was opened ",
                "(code conflict), unless --force. Returns { doc, file, wrote, hash, diff, dirty, canUndo, canRedo }."
            )),
            doc_params(vec![force_param("save despite problems or a conflict")]),
            true,
            save_run,
            save_format,
        ),
        session_command(
            "doc.flush",
            "Save every open doc with changes now (or one), as autosave would soon",
            Some(concat!(
                "Edits save themselves about a second after the last one; this saves now. A doc with problems isn't written (saveState problems), ",
                "and one whose file changed on disk is merged first (saveState conflict when that clashes). Returns { saved, docs: [{ doc, saveState, problems }] }."
            )),
            json!({ "type": "object", "properties": { "doc": doc_param() } }),
            true,
            flush_run,
            flush_format,
        ),
        session_command(
            "doc.revert",
            "Load an open doc from disk again, dropping its changes (as one undo step)",
            Some("A doc that was never saved (made by scene.new or object.new) is discarded. To reload without dropping anything, use doc.reload."),
            doc_params(vec![]),
            true,
            revert_run,
            summary_format,
        ),
        session_command(
            "doc.reload",
            "Load an open doc from disk again, only if no unsaved work is lost",
            Some(concat!(
                "Reloads when the working copy has no edits its file lacks: it is clean, or it matches the file. ",
                "Refuses with code dirty (details: the edits that would be lost) when there are unsaved edits. ",
                "Returns { doc, reloaded, changed, summary, dirty, canUndo, canRedo }; reloaded is false when the doc already matched the file. ",
                "A scene whose file is not in depth order (a hand edit) is sorted as a step of its own: the result has sorted (how many items moved), the doc is then unsaved, ",
                "and one undo gives back exactly the file. doc.revert is the one that drops edits."
            )),
            doc_params(vec![]),
            true,
            reload_run,
            summary_format,
        ),
        session_command(
            "doc.merge",
            "Merge the changes to a doc's file on disk into its working copy, keeping the unsaved edits",
            Some(concat!(
                "A three-way line merge: the file as the doc was opened or last saved, the file now, and the working copy. One undo step; the doc stays dirty with its own edits. ",
                "A doc with no unsaved edits just reloads. When both sides changed the same lines it refuses with code merge_conflict ",
                "(details: [{ base: [from, to], ours: [lines], theirs: [lines] }]) and changes nothing. The merged scene is put in depth order, in the same step ",
                "(a line merge cannot judge overlaps): sorted says how many items moved. Returns { doc, merged, changed, summary, dirty, canUndo, canRedo, sorted? }."
            )),
            doc_params(vec![]),
            true,
            merge_run,
            summary_format,
        ),
        session_command(
            "doc.close",
            "Close an open doc (refused with unsaved changes unless --force)",
            None,
            doc_params(vec![force_param("drop unsaved changes")]),
            false,
            close_run,
            close_format,
        ),
    ]
}
