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
//!   proposals.changed  { proposals: [...] }              when the state dir's proposals change (as proposal.list)
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
use crate::document::{Document, Env};
use crate::jsonrpc::{handle_line, spawn_lines, Failure, Output, RpcError, METHOD_NOT_FOUND};
use crate::open_docs::OpenDocsWriter;
use crate::registry::Registry;
use crate::session::{is_stale, read_source, Mode, Options, Session};
use crate::state::now_ms;
use crate::watch::{SourcesWatcher, StateWatcher, POLL_MS};

pub const PROTOCOL_VERSION: i64 = 1;

/// The program's version: the release's (the tag's), stamped at build time
/// as PIXELLAB_VERSION; 0.0.0-dev otherwise.
pub fn version() -> &'static str {
    option_env!("PIXELLAB_VERSION").filter(|v| !v.is_empty()).unwrap_or(env!("CARGO_PKG_VERSION"))
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
    // The app's server notices stale docs itself (the watcher), so proposal.apply can tell it.
    let stale = rpc.clone();
    let mut session = Session::new(
        document,
        Options {
            env,
            mode: Mode::Direct,
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
        state = Some(StateWatcher::new(&session.state));
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
                    let result = registry.call(&mut Ctx::new(&mut session), method, params, false);
                    // Opening and closing docs changes the list without a doc.changed; written before the response.
                    open_docs.update(&docs_of(&session), &mut |t| log(t));
                    result.map_err(Failure::from)
                };
                if let Some(reply) = handle_line(&line, &mut handle, &mut |t| log(t)) {
                    rpc.send(&reply);
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
                            if is_stale(entry, &disk) {
                                rpc.notify("doc.stale", json!({ "doc": ref_, "file": file }));
                            }
                        }
                        rpc.notify("files.changed", json!({ "files": files }));
                        open_docs.update(&docs_of(&session), &mut |t| log(t));
                    }
                }
                if let Some(w) = state.as_mut() {
                    let changes = w.poll(now);
                    if changes.proposals {
                        match registry.call(&mut Ctx::new(&mut session), "proposal.list", json!({}), false) {
                            Ok(r) => {
                                rpc.notify("proposals.changed", json!({ "proposals": r.get("proposals").cloned().unwrap_or(Value::Null) }))
                            }
                            Err(e) => log(&format!("listing proposals failed: {}", e.message())),
                        }
                    }
                    if !changes.activity.is_empty() {
                        rpc.notify("activity.appended", json!({ "entries": changes.activity }));
                    }
                }
            }
        }
    }
    rpc.stop();
    open_docs.remove();
    0
}
