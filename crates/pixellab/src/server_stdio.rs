//! `pixellab serve --stdio`: the Mac app's server (ports
//! cli/lib/server-stdio.mjs; docs/cli.md, "The stdio protocol").
//! Newline-delimited JSON-RPC 2.0 on stdin and stdout. Methods are the
//! registry's command names, plus session.info and shutdown; a command's
//! result is the response's result. The server holds one session, so edits
//! are instant, and saves them itself a second after the last one
//! (docs/intents/2026-10-autosave/spec.md, D1).
//!
//! Notifications it sends:
//!   doc.changed        { doc, dirty, canUndo, canRedo }  after each edit, undo, redo, save, revert or reload
//!   doc.stale          { doc, file }                     an open doc with unsaved edits whose file changed on disk
//!   files.changed      { files: ['scenes/x.json', …] }   when sources change on disk (debounced)
//!   history.changed    { versions: [latest] }             the document's version history changed
//!   activity.appended  { entries: [...] }                new lines of activity.jsonl
//!
//! It owns its document (owner.rs, D9): it holds the document's lock in the
//! state dir and listens on a socket, where the CLI and MCP servers send
//! their commands after a client.hello naming who they are. One thread runs
//! the session: it takes lines from stdin, lines from clients and ticks from
//! a timer thread, in order, and polls the document and the state dir on
//! each tick, so requests run one at a time, whoever sends them.

use std::io::{BufRead, Write};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::author::Author;
use crate::context::{Ctx, DocKind};
use crate::document::{open_document_allowing_old, Document, Env, OpenOptions};
use crate::errors::{CommandError, Error};
use crate::jsonrpc::{handle_line, spawn_lines, Failure, Output, RpcError, METHOD_NOT_FOUND};
use crate::owner::{self, Claimed, Incoming};
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
    Remote(Incoming),
    Tick,
}

fn log(text: &str) {
    eprintln!("pixellab: {text}");
}

fn path_string(p: &Option<std::path::PathBuf>) -> Value {
    p.as_ref().map_or(Value::Null, |p| Value::String(p.to_string_lossy().into_owned()))
}

/// The Mac app's pid: PIXELLAB_APP_PID, else the process that started this one (R26).
fn app_pid(env: &Env) -> i64 {
    env.get("PIXELLAB_APP_PID").and_then(|v| v.trim().parse().ok()).unwrap_or_else(|| i64::from(std::os::unix::process::parent_id()))
}

/// The author a client.hello names (D8): an MCP server is an agent (its client's name, and the
/// chat it works for), the CLI is the CLI.
fn author_of_hello(params: &Value) -> Author {
    match params.get("kind").and_then(Value::as_str) {
        Some("cli") => Author::cli(),
        _ => Author::agent(
            params.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()).map(str::to_string),
            params.get("chat").filter(|c| c.is_object()).cloned(),
        ),
    }
}

/// Runs one registry command for whoever sent it (`reopened`: a format-1 document was migrated
/// and reopened here).
fn run_command(session: &mut Session, registry: &Registry, method: &str, params: Value, reopened: &mut bool) -> Result<Value, Failure> {
    if !registry.has(method) {
        return Err(RpcError::new(METHOD_NOT_FOUND, format!("no method {method}")).with_data(json!({ "code": "not_found" })).into());
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
    let mut ctx = Ctx::new(session);
    ctx.open = old.as_ref().map(|(_, open)| open.clone());
    let result = registry.call(&mut ctx, method, params, false);
    if let (Some((_, open)), Ok(_), "document.migrate") = (&old, &result, method) {
        match open_document_allowing_old(open) {
            Ok(document) if document.old_format.is_none() => {
                session.document = document;
                session.invalidate();
                *reopened = true;
            }
            Ok(_) => {}
            Err(e) => log(&format!("reopening the migrated document failed: {}", e.message)),
        }
    }
    result.map_err(Failure::from)
}

/// Answers every request with `owned` until the first one, then stops: another server owns this
/// document (R26). Returns the exit code.
fn refuse_owned(holder: &Value, input: Box<dyn BufRead + Send>, rpc: &Output) -> i32 {
    let app = holder.get("appPid").cloned().unwrap_or(Value::Null);
    let pid = holder.get("pid").cloned().unwrap_or(Value::Null);
    let message = "Another Pixel Lab has this document open; bring that window forward instead";
    for line in input.lines() {
        let Ok(line) = line else { break };
        let mut handle = |_: &str, _: Value, _: &Value| -> Result<Value, Failure> {
            Err(RpcError::new(-32000, message).with_data(json!({ "code": "owned", "appPid": app, "pid": pid, "message": message })).into())
        };
        if let Some(reply) = handle_line(&line, &mut handle, &mut |t| log(t)) {
            rpc.send(&reply);
            break;
        }
    }
    rpc.stop();
    4
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
    let app = app_pid(&env);
    let state_dir = crate::state::state_dir(&env);
    // The document is this server's to write, or someone else's (D9).
    let claim = match owner::claim(&state_dir, &document.root, Some(app), version()) {
        Ok(Claimed::Owned(c)) => Some(c),
        Ok(Claimed::Held(holder)) => return refuse_owned(&holder, input, &rpc),
        Err(e) => {
            log(&format!("can't own the document, so clients can't reach this server: {e}"));
            None
        }
    };
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
    let opened_version = session.history.latest().unwrap_or(0);
    let (tx, rx) = mpsc::channel::<Event>();
    let lines = tx.clone();
    spawn_lines(input, move |line| {
        let _ = lines.send(Event::Line(line));
    });
    let clients = match &claim {
        Some(c) => match owner::listen(c, tx.clone(), Event::Remote) {
            Ok(list) => Some(list),
            Err(e) => {
                log(&format!("can't listen for clients: {e}"));
                None
            }
        },
        None => None,
    };
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
    let mut hellos: std::collections::HashMap<u64, (Author, bool)> = std::collections::HashMap::new();
    let handle_remote =
        |session: &mut Session, hellos: &mut std::collections::HashMap<u64, (Author, bool)>, conn: u64, line: &str, reopened: &mut bool| {
            let Some(clients) = &clients else { return };
            let mut handle = |method: &str, params: Value, _: &Value| -> Result<Value, Failure> {
                if method == "client.hello" {
                    let protocol = params.get("protocol").and_then(Value::as_i64);
                    hellos.insert(conn, (author_of_hello(&params), protocol == Some(owner::PROTOCOL)));
                    let d = &session.document;
                    return Ok(json!({
                        "protocol": owner::PROTOCOL,
                        "version": version(),
                        "accepted": protocol == Some(owner::PROTOCOL),
                        "document": { "root": d.root.to_string_lossy(), "name": d.name },
                    }));
                }
                let Some((author, accepted)) = hellos.get(&conn).cloned() else {
                    return Err(RpcError::new(-32600, "say client.hello first").with_data(json!({ "code": "usage" })).into());
                };
                // A client of another protocol may ask for a save, so it can read the files (R28).
                if !accepted && method != "doc.flush" {
                    return Err(RpcError::new(-32000, "this pixellab speaks another protocol than Pixel Lab's")
                        .with_data(json!({ "code": "version" }))
                        .into());
                }
                if method == "shutdown" || method == "session.info" {
                    return Err(RpcError::new(METHOD_NOT_FOUND, format!("{method} is the app's"))
                        .with_data(json!({ "code": "not_found" }))
                        .into());
                }
                session.author = author;
                session.remote = true;
                let result = run_command(session, registry, method, params, reopened);
                session.author = Author::owner();
                session.remote = false;
                result
            };
            if let Some(reply) = handle_line(line, &mut handle, &mut |t| log(t)) {
                owner::reply(clients, conn, &reply);
            }
        };
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
                            "owner": claim.as_ref().map_or(Value::Null, |c| c.contents.clone()),
                            "openedVersion": opened_version,
                            "docs": session.list(),
                        }));
                    }
                    if method == "shutdown" {
                        shutting_down = true;
                        return Ok(json!({ "ok": true }));
                    }
                    run_command(&mut session, registry, method, params, &mut reopened)
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
            Event::Remote(Incoming::Line { conn, line }) => {
                handle_remote(&mut session, &mut hellos, conn, &line, &mut reopened);
                if std::mem::take(&mut reopened) && sources.is_some() {
                    sources = Some(SourcesWatcher::new(&session.document.source_dir));
                }
            }
            Event::Remote(Incoming::Gone { conn }) => {
                hellos.remove(&conn);
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
    // Clients' requests already sent are answered (R24); then everything that can be saved is,
    // the open version closes, and the document is free for the CLI and agents.
    while let Ok(event) = rx.try_recv() {
        if let Event::Remote(Incoming::Line { conn, line }) = event {
            handle_remote(&mut session, &mut hellos, conn, &line, &mut reopened);
        }
    }
    session.autosave(now_ms(), true);
    let _ = session.history.mark(None);
    if let Some(c) = &clients {
        owner::close_all(c);
    }
    drop(claim);
    rpc.stop();
    0
}
