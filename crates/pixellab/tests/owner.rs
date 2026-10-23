//! One writer per document (docs/intents/2026-10-autosave/spec.md, R22, R25–R27, D9): the app's
//! server owns the document it has open, a second one is refused, a killed owner frees it at once,
//! and only this user's processes reach it, through a socket beside the lock.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use common::server::start_server;
use common::{make_document, temp_dir};
use pixellab::owner::{self, Claimed};
use serde_json::{json, Value};

/// A connection to an owner's socket, speaking newline JSON-RPC.
struct Client {
    stream: BufReader<UnixStream>,
    id: u64,
}

impl Client {
    fn connect(state: &Path, root: &Path) -> Client {
        let holder = owner::holder(state, root).expect("an owner");
        let stream = UnixStream::connect(holder["socket"].as_str().unwrap()).expect("connect");
        stream.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
        Client { stream: BufReader::new(stream), id: 0 }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        let line = json!({ "jsonrpc": "2.0", "id": self.id, "method": method, "params": params }).to_string();
        writeln!(self.stream.get_mut(), "{line}").unwrap();
        let mut reply = String::new();
        self.stream.read_line(&mut reply).expect("a reply");
        serde_json::from_str(&reply).expect("JSON")
    }
}

#[test]
fn r22_the_apps_server_takes_the_documents_lock_and_a_client_edits_through_it() {
    let doc = make_document();
    let state = temp_dir("pixellab-state-");
    let mut env = doc.env();
    env.insert("PIXELLAB_AUTOSAVE_MS".into(), "100".into());
    let mut app = start_server(&["--stdio", "--document", doc.root.to_str().unwrap()], &env, &state);
    let info = app.request("session.info", json!({}))["result"].clone();
    let holder = owner::holder(&state, &doc.root).expect("an owner");
    assert_eq!(holder["pid"], info["pid"]);
    assert_eq!(holder["protocol"], owner::PROTOCOL);
    let mut client = Client::connect(&state, &doc.root);
    let refused = client.request("scene.place", json!({ "name": "sample", "object": "pebble", "at": [6, 0, 0] }));
    assert_eq!(refused["error"]["data"]["code"], "usage", "hello first: {refused}");
    let hello = client.request("client.hello", json!({ "kind": "mcp", "name": "claude-code 2.1.0", "protocol": owner::PROTOCOL }));
    assert_eq!(hello["result"]["accepted"], true, "{hello}");
    let since = app.notifications().len();
    let placed = client.request("scene.place", json!({ "name": "sample", "object": "pebble", "at": [6, 0, 0] }));
    assert_eq!(placed["result"]["changed"], true, "{placed}");
    // The app sees the edit as it happens, by its author, and saves it.
    let changed =
        app.wait_for(since, "doc.changed by the agent", |m| m["method"] == "doc.changed" && m["params"]["author"]["kind"] == "agent");
    assert_eq!(changed["params"]["undoLabel"], "pebble placed at column 6, row 0, level 0 (Claude)");
    app.wait_for(since, "saved", |m| m["method"] == "doc.changed" && m["params"]["saveState"] == "saved");
    let text = std::fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap();
    assert!(text.contains("[6, 0, 0]"), "{text}");
    let rendered = app.request("scene.items", json!({ "name": "sample" }));
    assert!(rendered.to_string().contains("[6,0,0]"), "the app's copy has it");
    // The app's own requests can't come through a client.
    assert_eq!(client.request("shutdown", json!({}))["error"]["data"]["code"], "not_found");
    assert_eq!(app.close(), 0);
}

#[test]
fn r26_a_second_server_on_an_owned_document_is_refused_with_the_owners_app_pid() {
    let doc = make_document();
    let state = temp_dir("pixellab-state-");
    let mut env = doc.env();
    env.insert("PIXELLAB_APP_PID".into(), "4242".into());
    let mut first = start_server(&["--stdio", "--document", doc.root.to_str().unwrap()], &env, &state);
    first.request("session.info", json!({}));
    env.insert("PIXELLAB_APP_PID".into(), "5151".into());
    let mut second = start_server(&["--stdio", "--document", doc.root.to_str().unwrap()], &env, &state);
    let r = second.request("session.info", json!({}));
    assert_eq!(r["error"]["data"]["code"], "owned", "{r}");
    assert_eq!(r["error"]["data"]["appPid"], 4242);
    assert_eq!(second.exited(), 4);
    // Another document is another owner's business.
    let other = make_document();
    let mut third = start_server(&["--stdio", "--document", other.root.to_str().unwrap()], &other.env(), &state);
    assert!(third.request("session.info", json!({}))["result"].is_object());
}

#[test]
fn r25_a_killed_owner_frees_the_document_at_once() {
    let doc = make_document();
    let state = temp_dir("pixellab-state-");
    let mut app = start_server(&["--stdio", "--document", doc.root.to_str().unwrap()], &doc.env(), &state);
    app.request("session.info", json!({}));
    assert!(owner::holder(&state, &doc.root).is_some());
    app.child.kill().unwrap(); // SIGKILL: no cleanup runs
    app.child.wait().unwrap();
    assert!(owner::holder(&state, &doc.root).is_none(), "the OS let go of the lock");
    // The next owner takes over, the dead socket file and all.
    match owner::claim(&state, &doc.root, None, "test").unwrap() {
        Claimed::Owned(c) => assert!(c.socket.exists()),
        Claimed::Held(h) => panic!("still held: {h}"),
    }
}

#[test]
fn r27_the_owners_folder_and_socket_are_the_users_alone() {
    let doc = make_document();
    let state = temp_dir("pixellab-state-");
    let Claimed::Owned(c) = owner::claim(&state, &doc.root, None, "test").unwrap() else { panic!("held") };
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&owner::owners_dir(&state)), 0o700);
    assert_eq!(mode(&c.socket), 0o600);
    let stream = UnixStream::connect(&c.socket).unwrap();
    assert!(owner::same_user(&stream), "this user's own process");
}

#[test]
fn r27_a_socket_path_too_long_for_macos_goes_in_tmpdir() {
    let doc = make_document();
    let deep = temp_dir("pixellab-state-").join("a".repeat(60)).join("b".repeat(60));
    let path = owner::socket_path(&deep, &doc.root);
    assert!(path.as_os_str().len() <= 104, "{}", path.display());
    assert!(!path.starts_with(&deep));
    let Claimed::Owned(c) = owner::claim(&deep, &doc.root, None, "test").unwrap() else { panic!("held") };
    let holder = owner::holder(&deep, &doc.root).expect("held");
    assert_eq!(holder["socket"], json!(c.socket.to_str().unwrap()), "the lock says where it listens");
    assert!(UnixStream::connect(&c.socket).is_ok());
}

// ------------------------------------------------------------- clients (D10)

fn cli(args: &[&str], env: &std::collections::HashMap<String, String>, state: &Path) -> (i32, String, String) {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_pixellab"))
        .args(args)
        .env_remove("PIXELLAB_SITE")
        .env_remove("PIXELLAB_DOC")
        .envs(env)
        .env("PIXELLAB_STATE", state)
        .current_dir(state)
        .output()
        .unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

fn mcp_call(server: &mut common::server::Server, name: &str, args: Value) -> Value {
    server.request("tools/call", json!({ "name": name, "arguments": args }))["result"].clone()
}

fn mcp_server(doc: &common::TempDocument, state: &Path) -> common::server::Server {
    let mut m = start_server(&["--mcp", "--document", doc.root.to_str().unwrap()], &doc.env(), state);
    m.request(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "claude-code", "version": "2.1.0" } }),
    );
    m
}

fn sample(doc: &common::TempDocument) -> String {
    std::fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap()
}

#[test]
fn r22_the_cli_edits_through_the_app_and_the_app_saves_before_it_answers() {
    let doc = make_document();
    let state = temp_dir("pixellab-state-");
    let mut env = doc.env();
    env.insert("PIXELLAB_AUTOSAVE_MS".into(), "60000".into());
    let mut app = start_server(&["--stdio", "--document", doc.root.to_str().unwrap()], &env, &state);
    app.request("session.info", json!({}));
    let since = app.notifications().len();
    let (code, out, err) =
        cli(&["scene", "place", "sample", "pebble", "--at", "7,0,0", "--document", doc.root.to_str().unwrap()], &env, &state);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("saved by Pixel Lab: scenes/sample.json"), "{out}");
    assert!(sample(&doc).contains("[7, 0, 0]"), "on disk before the CLI answered, a minute's pause or not");
    let changed = app.wait_for(since, "doc.changed by the CLI", |m| m["method"] == "doc.changed" && m["params"]["author"]["kind"] == "cli");
    assert_eq!(changed["params"]["undoLabel"], "pebble placed at column 7, row 0, level 0 (CLI)");
    // A read sees the app's copy.
    let (code, out, _) = cli(&["scene", "items", "sample", "--json", "--document", doc.root.to_str().unwrap()], &env, &state);
    assert_eq!(code, 0);
    assert!(out.contains("7,") || out.contains("7, 0, 0"), "{out}");
    // The version is the CLI's.
    let v = app.request("history.list", json!({}))["result"]["versions"][0].clone();
    assert_eq!(v["author"], json!({ "kind": "cli" }));
}

#[test]
fn r22_r20_an_agent_edits_through_the_app_and_undoes_only_its_own_step() {
    let doc = make_document();
    let state = temp_dir("pixellab-state-");
    let mut app = start_server(&["--stdio", "--document", doc.root.to_str().unwrap()], &doc.env(), &state);
    app.request("session.info", json!({}));
    let mut agent = mcp_server(&doc, &state);
    let placed = mcp_call(&mut agent, "scene_place", json!({ "name": "sample", "object": "pebble", "at": [8, 0, 0] }));
    assert!(placed.get("isError").is_none(), "{placed}");
    assert!(app.request("scene.items", json!({ "name": "sample" })).to_string().contains("[8,0,0]"), "the app has the agent's edit");
    let undone = mcp_call(&mut agent, "doc_undo", json!({ "doc": "scene:sample" }));
    assert!(undone.get("isError").is_none(), "its own step: {undone}");
    mcp_call(&mut agent, "scene_place", json!({ "name": "sample", "object": "pebble", "at": [9, 0, 0] }));
    // The owner edits after it: the agent's undo would take back the owner's step, so it refuses.
    app.request("scene.place", json!({ "name": "sample", "object": "pebble", "at": [10, 0, 0] }));
    let refused = mcp_call(&mut agent, "doc_undo", json!({ "doc": "scene:sample" }));
    assert_eq!(refused["structuredContent"]["error"]["code"], "not-yours", "{refused}");
    let revert = mcp_call(&mut agent, "doc_revert", json!({ "doc": "scene:sample" }));
    assert_eq!(revert["structuredContent"]["error"]["code"], "owned", "{revert}");
    let close = mcp_call(&mut agent, "doc_close", json!({ "doc": "scene:sample" }));
    assert_eq!(close["structuredContent"]["closed"], false);
    // The app's ⌘Z takes the latest step, whoever made it.
    let u = app.request("doc.undo", json!({ "doc": "scene:sample" }))["result"].clone();
    assert!(u["summary"].as_str().unwrap().contains("column 10"), "{u}");
}

#[test]
fn r24_when_the_app_closes_the_document_the_agent_writes_it_itself() {
    let doc = make_document();
    let state = temp_dir("pixellab-state-");
    let mut app = start_server(&["--stdio", "--document", doc.root.to_str().unwrap()], &doc.env(), &state);
    app.request("session.info", json!({}));
    let mut agent = mcp_server(&doc, &state);
    mcp_call(&mut agent, "scene_place", json!({ "name": "sample", "object": "pebble", "at": [8, 0, 0] }));
    assert_eq!(app.close(), 0);
    assert!(sample(&doc).contains("[8, 0, 0]"), "the app saved it on the way out");
    assert!(owner::holder(&state, &doc.root).is_none());
    let placed = mcp_call(&mut agent, "scene_place", json!({ "name": "sample", "object": "pebble", "at": [9, 0, 0] }));
    assert_eq!(placed["structuredContent"]["saved"][0]["doc"], "scene:sample", "written by the agent: {placed}");
    assert!(sample(&doc).contains("[8, 0, 0]") && sample(&doc).contains("[9, 0, 0]"), "{}", sample(&doc));
    // And when the app opens it again, the agent goes through it again.
    let mut again = start_server(&["--stdio", "--document", doc.root.to_str().unwrap()], &doc.env(), &state);
    again.request("session.info", json!({}));
    let since = again.notifications().len();
    mcp_call(&mut agent, "scene_place", json!({ "name": "sample", "object": "pebble", "at": [11, 0, 0] }));
    again.wait_for(since, "the agent's edit in the app", |m| m["method"] == "doc.changed" && m["params"]["author"]["kind"] == "agent");
}

/// A stand-in owner in this process: it holds the lock and answers `client.hello` with
/// `accepted`, then closes each connection on its next request (or answers doc.flush).
fn fake_owner(state: &Path, root: &Path, accepted: bool) -> std::thread::JoinHandle<()> {
    let Claimed::Owned(claim) = owner::claim(state, root, None, "9.9.9").unwrap() else { panic!("held") };
    std::thread::spawn(move || {
        let claim = claim;
        for stream in claim.listener.incoming() {
            let Ok(stream) = stream else { break };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut out = stream;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                let m: Value = serde_json::from_str(&line).unwrap();
                let reply = match m["method"].as_str() {
                    Some("client.hello") => {
                        json!({ "jsonrpc": "2.0", "id": m["id"], "result": { "protocol": 99, "version": "9.9.9", "accepted": accepted } })
                    }
                    Some("doc.flush") => json!({ "jsonrpc": "2.0", "id": m["id"], "result": { "saved": true, "docs": [] } }),
                    _ => break, // goes away mid-call
                };
                writeln!(out, "{reply}").unwrap();
            }
        }
    })
}

#[test]
fn r24_an_owner_that_goes_away_mid_call_is_owner_gone_and_nothing_is_retried() {
    let doc = make_document();
    let state = temp_dir("pixellab-state-");
    let _owner = fake_owner(&state, &doc.root, true);
    let before = sample(&doc);
    let (code, _, err) = cli(
        &["scene", "place", "sample", "pebble", "--at", "7,0,0", "--json", "--document", doc.root.to_str().unwrap()],
        &doc.env(),
        &state,
    );
    assert_eq!(code, 1);
    assert!(err.contains("\"code\":\"owner-gone\""), "{err}");
    assert_eq!(sample(&doc), before, "not run again locally");
}

#[test]
fn r28_a_mismatched_owner_refuses_edits_and_reads_come_from_disk() {
    let doc = make_document();
    let state = temp_dir("pixellab-state-");
    let _owner = fake_owner(&state, &doc.root, false);
    let (code, _, err) = cli(
        &["scene", "place", "sample", "pebble", "--at", "7,0,0", "--json", "--document", doc.root.to_str().unwrap()],
        &doc.env(),
        &state,
    );
    assert_eq!(code, 1);
    assert!(err.contains("\"code\":\"version\"") && err.contains("Pixel Lab (9.9.9)"), "{err}");
    let (code, out, err) = cli(&["scene", "items", "sample", "--document", doc.root.to_str().unwrap()], &doc.env(), &state);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("block"), "{out}");
}

#[test]
fn d10_doc_adopt_merges_a_clients_unsaved_copy_as_one_step() {
    let doc = make_document();
    let env = doc.env();
    let document = pixellab::document::open_document(&pixellab::document::OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    let mut s = pixellab::session::Session::new(document, pixellab::session::Options { env, ..Default::default() });
    let base = sample(&doc);
    let theirs = base.replace("[-16, -16, 64, 48]", "[-16, -16, 96, 48]");
    let reg = pixellab::registry::registry();
    let r = reg
        .call(
            &mut pixellab::context::Ctx::new(&mut s),
            "doc.adopt",
            json!({ "doc": "scene:sample", "base": base, "source": theirs }),
            false,
        )
        .unwrap();
    assert_eq!(r["adopted"], true);
    let doc_now = s.doc(pixellab::context::DocKind::Scene, "sample").unwrap();
    assert_eq!(doc_now["viewBox"], json!([-16, -16, 96, 48]));
    assert_eq!(s.entries.get("scene:sample").unwrap().undo.len(), 1, "one step");
}
