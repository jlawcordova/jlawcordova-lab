//! `pixellab serve --stdio`: the Mac app's server (ports
//! cli/lib/server-stdio.mjs; docs/cli.md, "The stdio protocol").
//! Newline-delimited JSON-RPC 2.0 on stdin and stdout. Methods are the
//! registry's command names, plus session.info and shutdown; a command's
//! result is the response's result. The server holds one session, so edits
//! are instant and stay in memory until doc.save writes them.
//!
//! Notifications it sends:
//!   doc.changed        { doc, dirty, canUndo, canRedo }  after each edit, undo, redo, save, revert or reload
//!   doc.stale          { doc, file }                     an open doc with unsaved edits whose file changed on disk
//!   files.changed      { files: ['scenes/x.json', …] }   when sources change on disk (debounced)
//!   history.changed    { versions: [latest] }             the document's version history changed
//!   activity.appended  { entries: [...] }                new lines of activity.jsonl
//!
//! It keeps open/<pid>.json in the state dir current (open_docs.rs). One
//! thread runs the session: it takes lines from a reader thread and ticks
//! from a timer thread, in order, and polls the document and the state dir
//! on each tick (spec D2), so requests run one at a time.

use std::io::{BufRead, Write};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::context::{Ctx, DocKind};
use crate::document::{open_document_allowing_old, Document, Env, OpenOptions};
use crate::errors::{CommandError, Error};
use crate::jsonrpc::{handle_line, spawn_lines, Failure, Output, RpcError, METHOD_NOT_FOUND};
use crate::open_docs::OpenDocsWriter;
use crate::registry::Registry;
use crate::session::{is_stale, read_source, Options, Session};
use crate::state::now_ms;
use crate::watch::{SourcesWatcher, StateWatcher, POLL_MS};

pub const PROTOCOL_VERSION: i64 = 1;

/// The program's version: the release's (the tag's), stamped at build time
/// as PIXELLAB_VERSION; 0.0.0-dev otherwise.
pub fn version() -> &'static str {
    option_env!("PIXELLAB_VERSION").filter(|v| !v.is_empty()).unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// How long after a doc's last change the server saves it: PIXELLAB_AUTOSAVE_MS (the script hook
/// uses 100), else a second (docs/intents/2026-10-autosave/spec.md, R1).
pub fn autosave_ms(env: &Env) -> f64 {
    env.get("PIXELLAB_AUTOSAVE_MS").and_then(|v| v.trim().parse::<f64>().ok()).filter(|v| *v >= 0.0).unwrap_or(1000.0)
}

enum Event {
    Line(Option<String>),
    Tick,
}

fn log(text: &str) {
    eprintln!("pixellab: {text}");
}

fn path_string(p: &Option<std::path::PathBuf>) -> Value {
    p.as_ref().map_or(Value::Null, |p| Value::String(p.to_string_lossy().into_owned()))
}

/// Runs the server until its input ends or it's told to shut down; returns
/// the exit code.
pub fn start(
    document: Document,
    env: Env,
    registry: &Registry,
    input: Box<dyn BufRead + Send>,
    output: Box<dyn Write + Send>,
    watch: bool,
) -> i32 {
    let rpc = Output::new(output);
    let notify = rpc.clone();
    // The app's server notices stale docs itself (the watcher) and when autosave can't merge one.
    let stale = rpc.clone();
    let autosave_ms = autosave_ms(&env);
    let mut session = Session::new(
        document,
        Options {
            env,
            autosave_ms: Some(autosave_ms),
            on_change: Some(Box::new(move |status| notify.notify("doc.changed", status))),
            on_stale: Some(Box::new(move |doc| stale.notify("doc.stale", doc))),
            ..Options::default()
        },
    );
    let mut open_docs = OpenDocsWriter::new(&session.state.dir, &session.document.root);
    let (tx, rx) = mpsc::channel::<Event>();
    let lines = tx.clone();
    spawn_lines(input, move |line| {
        let _ = lines.send(Event::Line(line));
    });
    let mut sources = None;
    let mut state = None;
    if watch {
        sources = Some(SourcesWatcher::new(&session.document.source_dir));
        state = Some(StateWatcher::new(&session.state, session.history.dir.join("versions.jsonl")));
        let ticks = tx.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(POLL_MS));
            if ticks.send(Event::Tick).is_err() {
                break;
            }
        });
    }
    drop(tx);
    let mut shutting_down = false;
    let mut reopened = false;
    let docs_of = |s: &Session| -> Vec<(String, bool)> { s.entries.keys().map(|r| (r.clone(), s.is_dirty(r))).collect() };
    while let Ok(event) = rx.recv() {
        match event {
            Event::Line(None) => break,
            Event::Line(Some(line)) => {
                let mut handle = |method: &str, params: Value, _: &Value| -> Result<Value, Failure> {
                    if method == "session.info" {
                        let d = &session.document;
                        return Ok(json!({
                            "name": "pixellab",
                            "version": version(),
                            "protocol": PROTOCOL_VERSION,
                            "pid": std::process::id(),
                            "document": { "root": d.root.to_string_lossy(), "name": d.name, "kind": d.kind, "publishDir": path_string(&d.publish_dir), "siteRoot": path_string(&d.site_root) },
                            "site": d.root.to_string_lossy(),
                            "sourceDir": d.source_dir.to_string_lossy(),
                            "stateDir": session.state.dir.to_string_lossy(),
                            "saves": "direct",
                            "docs": session.list(),
                        }));
                    }
                    if method == "shutdown" {
                        shutting_down = true;
                        return Ok(json!({ "ok": true }));
                    }
                    if !registry.has(method) {
                        return Err(RpcError::new(METHOD_NOT_FOUND, format!("no method {method}"))
                            .with_data(json!({ "code": "not_found" }))
                            .into());
                    }
                    // A format-1 document (R9): only the commands that don't need it run,
                    // document.migrate among them, which converts it and reopens it here (R8).
                    let old = session.document.old_format.clone().map(|problem| {
                        let d = &session.document;
                        let legacy = d.kind == "legacy-site";
                        let open = OpenOptions {
                            doc: if legacy { None } else { Some(d.root.to_string_lossy().into_owned()) },
                            site: if legacy { d.site_root.as_ref().map(|r| r.to_string_lossy().into_owned()) } else { None },
                            env: session.env.clone(),
                            cwd: d.root.clone(),
                        };
                        (problem, open)
                    });
                    if let Some((problem, _)) = &old {
                        if registry.get(method).is_some_and(|c| c.needs_document) {
                            return Err(Failure::from(Error::Command(CommandError::document(problem.clone()))));
                        }
                    }
                    let mut ctx = Ctx::new(&mut session);
                    ctx.open = old.as_ref().map(|(_, open)| open.clone());
                    let result = registry.call(&mut ctx, method, params, false);
                    if let (Some((_, open)), Ok(_), "document.migrate") = (&old, &result, method) {
                        match open_document_allowing_old(open) {
                            Ok(document) if document.old_format.is_none() => {
                                session.document = document;
                                session.invalidate();
                                reopened = true;
                            }
                            Ok(_) => {}
                            Err(e) => log(&format!("reopening the migrated document failed: {}", e.message)),
                        }
                    }
                    // Opening and closing docs changes the list without a doc.changed; written before the response.
                    open_docs.update(&docs_of(&session), &mut |t| log(t));
                    result.map_err(Failure::from)
                };
                if let Some(reply) = handle_line(&line, &mut handle, &mut |t| log(t)) {
                    rpc.send(&reply);
                }
                if std::mem::take(&mut reopened) && sources.is_some() {
                    sources = Some(SourcesWatcher::new(&session.document.source_dir));
                }
                if shutting_down {
                    break;
                }
            }
            Event::Tick => {
                let now = now_ms();
                if let Some(w) = sources.as_mut() {
                    let files = w.poll(now);
                    if !files.is_empty() {
                        for file in &files {
                            let Some((dir, rest)) = file.split_once('/') else { continue };
                            let Some(name) = rest.strip_suffix(".json") else { continue };
                            let kind = if dir == "objects" { DocKind::Object } else { DocKind::Scene };
                            let ref_ = format!("{}:{name}", kind.as_str());
                            if session.reload_if_clean(&ref_) {
                                continue;
                            }
                            let Some(entry) = session.entries.get(&ref_) else { continue };
                            if !session.is_dirty(&ref_) {
                                continue;
                            }
                            let disk = read_source(&session.document, entry.kind, &entry.name);
                            // A doc with a save pending takes the change, three ways, and autosave
                            // writes the result (R7); only a clash is left to the owner.
                            if is_stale(entry, &disk) && session.merge(&ref_).is_err() {
                                rpc.notify("doc.stale", json!({ "doc": ref_, "file": file }));
                            }
                        }
                        rpc.notify("files.changed", json!({ "files": files }));
                        open_docs.update(&docs_of(&session), &mut |t| log(t));
                    }
                }
                session.autosave(now, false);
                if let Some(w) = state.as_mut() {
                    let changes = w.poll(now);
                    if changes.history {
                        let latest: Vec<Value> = session.history.versions().last().map(crate::history::summary).into_iter().collect();
                        rpc.notify("history.changed", json!({ "versions": latest }));
                    }
                    if !changes.activity.is_empty() {
                        rpc.notify("activity.appended", json!({ "entries": changes.activity }));
                    }
                }
            }
        }
    }
    // Everything that can be saved is, and the open version closes (R24).
    session.autosave(now_ms(), true);
    let _ = session.history.mark(None);
    rpc.stop();
    open_docs.remove();
    0
}
