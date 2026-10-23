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
