//! The two servers as child processes (ports cli/test/servers.test.mjs):
//! `pixellab serve --stdio` (the app's JSON-RPC session server, with its
//! notifications) and `pixellab serve --mcp` (MCP for agents, whose saves
//! become proposals). Every stdout line must be JSON: the reader thread
//! records anything else and the test fails on it. Every wait has a deadline,
//! so a hang fails instead of blocking.

#![allow(clippy::result_large_err)]

mod common;

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use common::{make_document, make_site, temp_dir};
use pixellab::state::create_state;

const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Default)]
struct Shared {
    lines: Vec<Value>,
    not_json: Vec<String>,
    stderr: String,
}

/// A running server: requests and raw lines in, responses and notifications out.
struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    shared: Arc<(Mutex<Shared>, Condvar)>,
    next_id: u64,
}

fn start_server(args: &[&str], env: &HashMap<String, String>, state: &Path) -> Server {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pixellab"));
    cmd.arg("serve")
        .args(args)
        .env_remove("PIXELLAB_SITE")
        .env_remove("PIXELLAB_DOC")
        .env_remove("PIXELLAB_CHAT")
        .env_remove("PIXELLAB_CHAT_TITLE");
    cmd.envs(env).env("PIXELLAB_STATE", state);
    cmd.current_dir(state).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn the server");
    let stdin = child.stdin.take();
    let stdout = child.stdout.take().expect("stdout");
    let stderr = child.stderr.take().expect("stderr");
    let shared: Arc<(Mutex<Shared>, Condvar)> = Arc::default();
    let out = shared.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let (lock, cv) = &*out;
            let mut s = lock.lock().unwrap();
            match serde_json::from_str::<Value>(&line) {
                Ok(v) => s.lines.push(v),
                Err(_) => s.not_json.push(line),
            }
            cv.notify_all();
        }
    });
    let err = shared.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            let (lock, _) = &*err;
            let mut s = lock.lock().unwrap();
            s.stderr.push_str(&line);
            s.stderr.push('\n');
        }
    });
    Server { child, stdin, shared, next_id: 1 }
}

impl Server {
    /// Every message the server has written so far.
    fn lines(&self) -> Vec<Value> {
        let s = self.shared.0.lock().unwrap();
        assert!(s.not_json.is_empty(), "stdout had a line that is not JSON: {:?}", s.not_json);
        s.lines.clone()
    }

    /// The notifications (messages with a method) so far.
    fn notifications(&self) -> Vec<Value> {
        self.lines().into_iter().filter(|m| m.get("method").is_some()).collect()
    }

    fn stderr(&self) -> String {
        self.shared.0.lock().unwrap().stderr.clone()
    }

    fn send(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        writeln!(stdin, "{line}").expect("write to the server");
        stdin.flush().expect("flush");
    }

    /// Sends a request and returns the whole response message.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string());
        let deadline = Instant::now() + TIMEOUT;
        let (lock, cv) = &*self.shared;
        let mut s = lock.lock().unwrap();
        loop {
            assert!(s.not_json.is_empty(), "stdout had a line that is not JSON: {:?}", s.not_json);
            if let Some(m) = s.lines.iter().find(|m| m.get("id") == Some(&json!(id)) && m.get("method").is_none()) {
                return m.clone();
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "timed out waiting for the response to {method}; got {:?}", s.lines);
            s = cv.wait_timeout(s, left).unwrap().0;
        }
    }

    fn notify(&mut self, method: &str) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": {} }).to_string());
    }

    /// Waits for a notification passing `test`; those before index `since` (of `notifications()`) don't count.
    fn wait_for(&self, since: usize, what: &str, test: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + TIMEOUT;
        let (lock, cv) = &*self.shared;
        let mut s = lock.lock().unwrap();
        loop {
            assert!(s.not_json.is_empty(), "stdout had a line that is not JSON: {:?}", s.not_json);
            let seen: Vec<&Value> = s.lines.iter().filter(|m| m.get("method").is_some()).skip(since).collect();
            if let Some(n) = seen.iter().find(|n| test(n)) {
                return (*n).clone();
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero(),
                "timed out waiting for {what}; got {:?}",
                seen.iter().map(|n| n["method"].clone()).collect::<Vec<_>>()
            );
            s = cv.wait_timeout(s, left).unwrap().0;
        }
    }

    /// The exit code, once the process ends (a deadline fails the test).
    fn exited(&mut self) -> i32 {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().expect("wait") {
                return status.code().unwrap_or(-1);
            }
            assert!(Instant::now() < deadline, "the server did not exit");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Ends the server's input and waits for it to exit.
    fn close(&mut self) -> i32 {
        self.stdin = None;
        self.exited()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// The width of a PNG, from its IHDR chunk.
fn png_width(png: &[u8]) -> u32 {
    assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10], "bad PNG signature");
    u32::from_be_bytes([png[16], png[17], png[18], png[19]])
}

fn png_size(png: &[u8]) -> (u64, u64) {
    (u64::from(png_width(png)), u64::from(u32::from_be_bytes([png[20], png[21], png[22], png[23]])))
}

fn b64(text: &str) -> Vec<u8> {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in text.bytes().filter(|c| *c != b'=') {
        acc = (acc << 6) | T.iter().position(|t| *t == c).expect("base64") as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

fn str_of(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

fn keys(v: &Value) -> Vec<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

fn pebble_at(doc: &Value, tile: Value) -> Option<Value> {
    doc["items"].as_array().unwrap().iter().find(|i| i["object"] == "pebble" && i["at"]["tile"] == tile).cloned()
}

// ----------------------------------------------------------------- serve --stdio

/// One `serve --stdio` over a temp site checkout (the document inside it).
struct Stdio_ {
    site: common::TempSite,
    state: PathBuf,
    server: Server,
}

fn stdio_fixture() -> Stdio_ {
    let site = make_site();
    let state = temp_dir("pixellab-state-");
    let server = start_server(&["--stdio", "--site", site.root.to_str().unwrap()], &site.env(), &state);
    Stdio_ { site, state, server }
}

#[test]
fn serve_stdio_session_info() {
    let mut f = stdio_fixture();
    let result = f.server.request("session.info", json!({}))["result"].clone();
    assert_eq!(result["name"], "pixellab");
    assert_eq!(result["protocol"], 1);
    assert_eq!(result["site"], f.site.document.to_str().unwrap(), "site is the document root");
    assert_eq!(
        result["document"],
        json!({
            "root": result["site"],
            "name": "sample",
            "kind": "document",
            "publishDir": f.site.document.parent().unwrap().join("sample-out").to_str().unwrap(),
            "siteRoot": f.site.root.to_str().unwrap(),
        })
    );
    assert_eq!(result["stateDir"], f.state.to_str().unwrap());
    assert_eq!(result["saves"], "direct");
    assert_eq!(result["docs"], json!([]));
}

#[test]
fn serve_stdio_a_render_png_scale_view_box_size_and_item_bounds() {
    let mut f = stdio_fixture();
    let result = f.server.request("scene.render", json!({ "name": "sample", "scale": 2 }))["result"].clone();
    let png = b64(str_of(&result["png"]));
    assert_eq!([json!(png_size(&png).0), json!(png_size(&png).1)], [result["width"].clone(), result["height"].clone()]);
    assert_eq!(result["scale"], 2);
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
    let mut k = keys(&result["items"][0]);
    k.sort();
    assert_eq!(k, ["at", "bounds", "object", "path"]);
}

/// A doc.changed's status fields, without the labels and author the app shows (autosave spec, D11).
fn status_core(v: &Value) -> Value {
    json!({ "doc": v["doc"], "dirty": v["dirty"], "canUndo": v["canUndo"], "canRedo": v["canRedo"] })
}

#[test]
fn serve_stdio_place_undo_redo_save_with_doc_changed_after_each() {
    let mut f = stdio_fixture();
    let file = f.site.document.join("scenes/sample.json");
    let disk = fs::read_to_string(&file).unwrap();
    let changed = |n: &Value| n["method"] == "doc.changed";
    let mut since = f.server.notifications().len();
    let placed = f.server.request("scene.place", json!({ "name": "sample", "object": "pebble", "at": [1, 0, 0] }));
    assert_eq!(placed["result"]["changed"], true);
    assert_eq!(
        status_core(&f.server.wait_for(since, "doc.changed", changed)["params"]),
        json!({ "doc": "scene:sample", "dirty": true, "canUndo": true, "canRedo": false })
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), disk, "nothing written before doc.save");

    since = f.server.notifications().len();
    let undo = f.server.request("doc.undo", json!({ "doc": "scene:sample" }));
    assert!(str_of(&undo["result"]["summary"]).starts_with("Undid: pebble placed"), "{undo}");
    assert_eq!(
        status_core(&f.server.wait_for(since, "doc.changed", changed)["params"]),
        json!({ "doc": "scene:sample", "dirty": false, "canUndo": false, "canRedo": true })
    );

    since = f.server.notifications().len();
    f.server.request("doc.redo", json!({ "doc": "scene:sample" }));
    assert_eq!(
        status_core(&f.server.wait_for(since, "doc.changed", changed)["params"]),
        json!({ "doc": "scene:sample", "dirty": true, "canUndo": true, "canRedo": false })
    );

    let status = f.server.request("doc.status", json!({ "doc": "scene:sample" }))["result"].clone();
    let mut summary = status.clone();
    summary["undo"] = json!(status["undo"].as_array().unwrap().len());
    assert_eq!(
        summary,
        json!({
            "doc": "scene:sample", "dirty": true, "canUndo": true, "canRedo": false,
            "undoLabel": "pebble placed at column 1, row 0, level 0", "redoLabel": null, "author": { "kind": "owner" },
            "file": "scenes/sample.json", "exists": true, "stale": false,
            "undo": 1, "redo": [], "edits": ["pebble placed at column 1, row 0, level 0"], "unsavedInApp": false,
        })
    );

    since = f.server.notifications().len();
    let saved = f.server.request("doc.save", json!({ "doc": "scene:sample" }));
    assert_eq!(saved["result"]["wrote"], true);
    assert_eq!(
        status_core(&f.server.wait_for(since, "doc.changed", changed)["params"]),
        json!({ "doc": "scene:sample", "dirty": false, "canUndo": true, "canRedo": false })
    );
    // The session sorted the scene as part of the edit, so the pebble is where it stands, not last.
    assert_eq!(pebble_at(&read_json(&file), json!([1, 0, 0])), Some(json!({ "object": "pebble", "at": { "tile": [1, 0, 0] } })));
    let info = f.server.request("session.info", json!({}));
    assert_eq!(info["result"]["docs"], json!([{ "doc": "scene:sample", "dirty": false, "canUndo": true, "canRedo": false }]));
}

#[test]
fn serve_stdio_errors_unknown_method_bad_params_a_failed_command_a_line_that_is_not_json() {
    let mut f = stdio_fixture();
    let unknown = f.server.request("scene.nope", json!({}));
    assert_eq!(unknown["error"]["code"], -32601);
    assert_eq!(unknown["error"]["data"]["code"], "not_found");
    let usage = f.server.request("scene.place", json!({ "name": "sample" }));
    assert_eq!(usage["error"]["code"], -32602);
    assert_eq!(usage["error"]["data"]["code"], "usage");
    assert!(str_of(&usage["error"]["message"]).contains("object is required"), "{usage}");
    let failed = f.server.request("scene.show", json!({ "name": "no-such-scene" }));
    assert_eq!(failed["error"]["code"], -32000);
    assert_eq!(failed["error"]["data"]["code"], "not_found");
    let count = f.server.lines().len();
    f.server.send("this is not json");
    let ping = f.server.request("session.info", json!({}));
    assert!(ping.get("result").is_some());
    let parse_error = f.server.lines().into_iter().skip(count).find(|l| l["error"]["code"] == -32700).expect("a parse error response");
    assert_eq!(parse_error["id"], Value::Null);
}

#[test]
fn serve_stdio_a_command_that_panics_on_a_malformed_object_is_an_internal_error_and_the_server_keeps_its_open_docs() {
    let mut f = stdio_fixture();
    f.server.request("scene.place", json!({ "name": "sample", "object": "pebble", "at": [1, 0, 0] }));
    let broken = r#"{ "kind": "sprite", "anchor": [0, 0], "keys": { "a": "ink" }, "layers": "" }"#;
    fs::write(f.site.document.join("objects/broken.json"), broken).unwrap();
    let failed = f.server.request("object.layer.add", json!({ "name": "broken" }));
    assert_eq!(failed["error"]["code"], -32603, "{failed}");
    let status = f.server.request("doc.status", json!({ "doc": "scene:sample" }));
    assert_eq!(status["result"]["dirty"], true, "the unsaved place is still there: {status}");
    assert_eq!(status["result"]["edits"], json!(["pebble placed at column 1, row 0, level 0"]));
}

#[test]
fn serve_stdio_files_changed_when_a_source_changes_on_disk_and_a_clean_open_doc_reloads() {
    let mut f = stdio_fixture();
    f.server.request("doc.open", json!({ "doc": "object:pebble" }));
    let file = f.site.document.join("objects/pebble.json");
    let since = f.server.notifications().len();
    let text = fs::read_to_string(&file).unwrap().replace("\"keys\": {\n", "\"keys\": {\n    \"z\": \"ink\",\n");
    assert!(text.contains("\"z\": \"ink\""));
    fs::write(&file, text).unwrap();
    let n = f.server.wait_for(since, "files.changed", |m| {
        m["method"] == "files.changed" && m["params"]["files"].as_array().is_some_and(|a| a.contains(&json!("objects/pebble.json")))
    });
    assert!(n.is_object());
    let reloaded = f
        .server
        .wait_for(since, "doc.changed for object:pebble", |m| m["method"] == "doc.changed" && m["params"]["doc"] == "object:pebble");
    assert_eq!(reloaded["params"]["dirty"], false);
    let shown = f.server.request("object.show", json!({ "name": "pebble", "doc": true }));
    assert_eq!(shown["result"]["doc"]["keys"]["z"], "ink");
}

#[test]
fn serve_stdio_proposals_changed_and_activity_appended_when_the_state_dir_changes() {
    let mut f = stdio_fixture();
    // Same as the other tests: the server is up once it answers.
    f.server.request("session.info", json!({}));
    let since = f.server.notifications().len();
    let store = create_state(&f.state);
    let disk = fs::read_to_string(f.site.document.join("scenes/sample-preview.json")).unwrap();
    store
        .write_proposal(&json!({
            "id": "20261019-120000-abcdef", "doc": "scene:sample-preview", "baseHash": null, "source": disk, "summary": ["nothing"],
            "createdAt": pixellab::state::iso_time(pixellab::state::now_ms()), "agent": "test",
        }))
        .unwrap();
    let p = f.server.wait_for(since, "proposals.changed", |m| m["method"] == "proposals.changed");
    let ids: Vec<&Value> = p["params"]["proposals"].as_array().unwrap().iter().map(|x| &x["id"]).collect();
    assert_eq!(ids, [&json!("20261019-120000-abcdef")]);
    let entry = json!({ "time": "t", "tool": "scene_list", "args": {}, "ok": true, "summary": "s" });
    let mut log = fs::OpenOptions::new().create(true).append(true).open(f.state.join("activity.jsonl")).unwrap();
    writeln!(log, "{entry}").unwrap();
    let a = f.server.wait_for(since, "activity.appended", |m| m["method"] == "activity.appended");
    assert_eq!(a["params"]["entries"], json!([entry]));
}

#[test]
fn r25_d11_serve_stdio_proposal_list_does_not_list_retired_proposals() {
    let site = make_site();
    let state = temp_dir("pixellab-state-");
    let id = "20261019-112830-aaaaaa";
    fs::create_dir_all(state.join("proposals")).unwrap();
    let old = json!({ "id": id, "doc": "scene:sample-preview", "baseHash": null, "source": "{}", "summary": [], "createdAt": "2026-10-19T11:28:30.100Z", "agent": null });
    fs::write(state.join("proposals").join(format!("{id}.json")), old.to_string()).unwrap();
    let mut server = start_server(&["--stdio", "--site", site.root.to_str().unwrap()], &site.env(), &state);
    let listed = server.request("proposal.list", json!({}))["result"]["proposals"].clone();
    assert_eq!(listed, json!([]));
    assert!(state.join("retired-proposals").join(format!("{id}.json")).is_file());
    let activity = server.request("activity.list", json!({}));
    assert!(activity.to_string().contains("retired 1 proposal from before the format change"), "{activity}");
}

#[test]
fn serve_stdio_shutdown_answers_then_the_server_exits_0() {
    let mut f = stdio_fixture();
    let r = f.server.request("shutdown", json!({}));
    assert_eq!(r["result"], json!({ "ok": true }));
    assert_eq!(f.server.exited(), 0);
}

// ----------------------------------------------------------------- serve --mcp

struct Mcp {
    site: common::TempSite,
    state: PathBuf,
    server: Server,
    scene_before: String,
}

fn mcp_fixture(direct: bool) -> Mcp {
    let site = make_site();
    let state = temp_dir("pixellab-state-");
    let scene_before = fs::read_to_string(site.document.join("scenes/sample.json")).unwrap();
    let mut args = vec!["--mcp"];
    if direct {
        args.push("--direct");
    }
    args.extend(["--site", site.root.to_str().unwrap()]);
    let server = start_server(&args, &site.env(), &state);
    Mcp { site, state, server, scene_before }
}

fn initialize(server: &mut Server, name: &str, version: &str) -> Value {
    server.request(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": name, "version": version } }),
    )
}

fn call(server: &mut Server, name: &str, args: Value) -> Value {
    server.request("tools/call", json!({ "name": name, "arguments": args }))["result"].clone()
}

fn activity(state: &Path) -> Vec<Value> {
    fs::read_to_string(state.join("activity.jsonl")).unwrap().trim().split('\n').map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn mcp_initialized() -> Mcp {
    let mut m = mcp_fixture(false);
    initialize(&mut m.server, "test-agent", "9.9");
    m
}

#[test]
fn serve_mcp_initialize_notifications_initialized_ping() {
    let mut m = mcp_fixture(false);
    let server = &mut m.server;
    let init = initialize(server, "test-agent", "9.9");
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(init["result"]["capabilities"], json!({ "tools": { "listChanged": false } }));
    assert_eq!(init["result"]["serverInfo"]["name"], "pixellab");
    assert!(init["result"]["instructions"].is_string());
    let count = server.lines().len();
    server.notify("notifications/initialized");
    assert_eq!(server.request("ping", json!({}))["result"], json!({}));
    assert_eq!(server.lines().len(), count + 1, "no response to a notification");
}

#[test]
fn serve_mcp_a_tool_that_panics_on_a_malformed_object_is_an_internal_error_and_the_server_keeps_running() {
    let mut m = mcp_initialized();
    call(&mut m.server, "scene_place", json!({ "name": "sample", "object": "pebble", "at": [1, 0, 0] }));
    let broken = r#"{ "kind": "sprite", "anchor": [0, 0], "keys": { "a": "ink" }, "layers": "" }"#;
    fs::write(m.site.document.join("objects/broken.json"), broken).unwrap();
    let failed = call(&mut m.server, "object_layer_add", json!({ "name": "broken" }));
    assert_eq!(failed["isError"], true, "{failed}");
    assert_eq!(failed["structuredContent"]["error"]["code"], "internal");
    let status = call(&mut m.server, "doc_status", json!({ "doc": "scene:sample" }));
    assert_eq!(status["structuredContent"]["dirty"], true, "{status}");
}

#[test]
fn serve_mcp_tools_list_the_commands_as_tools_with_input_schemas() {
    let mut m = mcp_initialized();
    let tools = m.server.request("tools/list", json!({}))["result"]["tools"].clone();
    let tools = tools.as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| str_of(&t["name"])).collect();
    for name in
        ["scene_place", "scene_frame", "scene_render", "object_paint", "object_layer_add", "doc_save", "doc_undo", "proposal_list", "check"]
    {
        assert!(names.contains(&name), "{name}");
    }
    for name in ["proposal_apply", "proposal_reject", "art_build"] {
        assert!(!names.contains(&name), "{name}");
    }
    for t in tools {
        assert!(str_of(&t["name"]).bytes().all(|b| b.is_ascii_lowercase() || b == b'_') && !str_of(&t["name"]).is_empty());
        assert_eq!(t["inputSchema"]["type"], "object");
        assert!(t["description"].is_string());
    }
    let place = tools.iter().find(|t| t["name"] == "scene_place").unwrap();
    assert_eq!(place["inputSchema"]["required"], json!(["name", "object"]));
    assert_eq!(place["inputSchema"]["properties"]["at"]["items"], json!({ "type": "integer" }));
    assert!(str_of(&tools.iter().find(|t| t["name"] == "doc_save").unwrap()["description"]).contains("writes a proposal"));
}

#[test]
fn serve_mcp_tools_call_a_render_comes_back_as_an_image() {
    let mut m = mcp_initialized();
    let r = call(&mut m.server, "scene_render", json!({ "name": "sample", "scale": 2 }));
    assert!(r.get("isError").is_none(), "{r}");
    assert_eq!(r["content"][0]["type"], "image");
    assert_eq!(r["content"][0]["mimeType"], "image/png");
    assert!(png_width(&b64(str_of(&r["content"][0]["data"]))) > 0);
    assert_eq!(r["content"][1]["type"], "text");
    assert!(r["structuredContent"].get("png").is_none());
    assert_eq!(r["structuredContent"]["scale"], 2);
}

/// Places a pebble and saves: the proposal's id.
fn place_and_save(server: &mut Server) -> (Value, Value) {
    let placed = call(server, "scene_place", json!({ "name": "sample", "object": "pebble", "at": [1, 0, 0] }));
    let saved = call(server, "doc_save", json!({ "doc": "scene:sample" }));
    (placed, saved)
}

#[test]
fn serve_mcp_tools_call_edit_then_doc_save_writes_a_proposal_and_leaves_the_site_alone() {
    let mut m = mcp_initialized();
    let (state, scene_before) = (m.state.clone(), m.scene_before.clone());
    let (placed, saved) = place_and_save(&mut m.server);
    assert!(placed.get("isError").is_none(), "{placed}");
    assert_eq!(placed["structuredContent"]["changed"], true);
    assert_eq!(serde_json::from_str::<Value>(str_of(&placed["content"][0]["text"])).unwrap(), placed["structuredContent"]);
    assert!(saved.get("isError").is_none(), "{}", saved["content"][0]["text"]);
    let proposal = str_of(&saved["structuredContent"]["proposal"]).to_string();
    let files: Vec<String> =
        fs::read_dir(state.join("proposals")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(files, [format!("{proposal}.json")]);
    let p = read_json(&state.join("proposals").join(&files[0]));
    // A number (#n) too (docs/intents/2026-10-agent-chat/spec.md, R24); no chat from a terminal.
    assert_eq!(keys(&p), ["id", "number", "format", "doc", "baseHash", "source", "summary", "createdAt", "agent"]);
    assert_eq!(p["doc"], "scene:sample");
    assert_eq!(p["agent"], "test-agent 9.9");
    assert_eq!(p["summary"], json!(["pebble placed at column 1, row 0, level 0"]));
    let proposed: Value = serde_json::from_str(str_of(&p["source"])).unwrap();
    assert_eq!(pebble_at(&proposed, json!([1, 0, 0])), Some(json!({ "object": "pebble", "at": { "tile": [1, 0, 0] } })));
    assert_eq!(fs::read_to_string(m.site.document.join("scenes/sample.json")).unwrap(), scene_before, "the site is untouched");
}

#[test]
fn r19_agents_can_place_on_top_and_drop_over_mcp_as_proposals() {
    let mut m = mcp_initialized();
    let tools = m.server.request("tools/list", json!({}))["result"]["tools"].clone();
    let tools = tools.as_array().unwrap();
    for name in ["scene_drop", "scene_place", "scene_frame"] {
        assert!(tools.iter().any(|t| t["name"] == name), "{name}");
    }
    let place = tools.iter().find(|t| t["name"] == "scene_place").unwrap();
    assert_eq!(place["inputSchema"]["properties"]["onTop"]["type"], "boolean");
    assert_eq!(place["inputSchema"]["properties"]["at"]["minItems"], 2);
    // The sample's pebble floats a level above the block at 0,0; the block is its floor.
    let dropped = call(&mut m.server, "scene_drop", json!({ "name": "sample", "item": "1" }));
    assert!(dropped.get("isError").is_none(), "{dropped}");
    assert!(str_of(&dropped["structuredContent"]["summary"]).contains("dropped to level 0, on top of block"), "{dropped}");
    let placed = call(&mut m.server, "scene_place", json!({ "name": "sample", "object": "block", "at": [0, 0], "onTop": true }));
    assert!(placed.get("isError").is_none(), "{placed}");
    let saved = call(&mut m.server, "doc_save", json!({ "doc": "scene:sample" }));
    let proposal = read_json(&m.state.join("proposals").join(format!("{}.json", str_of(&saved["structuredContent"]["proposal"]))));
    let source: Value = serde_json::from_str(str_of(&proposal["source"])).unwrap();
    assert_eq!(pebble_at(&source, json!([0, 0, 0])).map(|p| p["object"].clone()), Some(json!("pebble")));
    let stacked =
        source["items"].as_array().unwrap().iter().filter(|i| i["object"] == "block").any(|i| i["at"]["tile"] == json!([0, 0, 1]));
    assert!(stacked, "{source}");
    assert_eq!(fs::read_to_string(m.site.document.join("scenes/sample.json")).unwrap(), m.scene_before, "the site is untouched");
}

fn failing_calls(server: &mut Server) -> (Value, Value, Value) {
    let failed = call(server, "scene_place", json!({ "name": "sample", "object": "no-such-object" }));
    let bad = call(server, "scene_place", json!({ "name": "sample", "object": "pebble", "at": "here" }));
    let unknown = server.request("tools/call", json!({ "name": "proposal_apply", "arguments": {} }));
    (failed, bad, unknown)
}

#[test]
fn serve_mcp_tools_call_errors_a_failed_command_and_bad_arguments_are_is_error_results_an_unknown_tool_is_a_protocol_error() {
    let mut m = mcp_initialized();
    let (failed, bad, unknown) = failing_calls(&mut m.server);
    assert_eq!(failed["isError"], true);
    assert!(str_of(&failed["content"][0]["text"]).contains("no object named no-such-object"), "{failed}");
    assert_eq!(failed["structuredContent"]["error"]["code"], "not_found");
    assert_eq!(bad["isError"], true);
    assert_eq!(bad["structuredContent"]["error"]["code"], "usage");
    assert_eq!(unknown["error"]["code"], -32602);
    assert!(str_of(&unknown["error"]["message"]).contains("for a person"), "{unknown}");
}

#[test]
fn serve_mcp_every_tools_call_is_in_activity_jsonl() {
    // The same calls, in the same order, as the tests above make on their own servers.
    let mut m = mcp_initialized();
    let state = m.state.clone();
    call(&mut m.server, "scene_render", json!({ "name": "sample", "scale": 2 }));
    place_and_save(&mut m.server);
    failing_calls(&mut m.server);
    let entries = activity(&state);
    let seen: Vec<(String, bool)> = entries.iter().map(|e| (str_of(&e["tool"]).to_string(), e["ok"].as_bool().unwrap())).collect();
    let expected: Vec<(String, bool)> = [
        ("scene_render", true),
        ("scene_place", true),
        ("doc_save", true),
        ("scene_place", false),
        ("scene_place", false),
        ("proposal_apply", false),
    ]
    .iter()
    .map(|(t, ok)| (t.to_string(), *ok))
    .collect();
    assert_eq!(seen, expected);
    // The client's name and version too (docs/intents/2026-10-agent-chat/spec.md, R24); no chat from a terminal.
    for e in &entries {
        assert_eq!(keys(e), ["time", "tool", "args", "ok", "summary", "agent"]);
    }
    assert_eq!(entries[1]["summary"], "pebble placed at column 1, row 0, level 0");
    assert_eq!(entries[1]["args"], json!({ "name": "sample", "object": "pebble", "at": [1, 0, 0] }));
}

#[test]
fn serve_mcp_the_server_exits_when_its_input_ends() {
    let mut m = mcp_initialized();
    assert_eq!(m.server.close(), 0);
    assert_eq!(m.server.stderr(), "");
}

#[test]
fn serve_mcp_direct_doc_save_writes_the_site_and_art_build_is_a_tool() {
    let mut m = mcp_fixture(true);
    let server = &mut m.server;
    initialize(server, "t", "1");
    let tools = server.request("tools/list", json!({}))["result"]["tools"].clone();
    let names: Vec<&str> = tools.as_array().unwrap().iter().map(|t| str_of(&t["name"])).collect();
    assert!(names.contains(&"art_build"));
    assert!(!names.contains(&"proposal_apply"));
    call(server, "scene_place", json!({ "name": "sample", "object": "pebble", "at": [1, 0, 0] }));
    let saved = call(server, "doc_save", json!({ "doc": "scene:sample" }));
    assert_eq!(saved["structuredContent"]["wrote"], true);
    assert_eq!(
        pebble_at(&read_json(&m.site.document.join("scenes/sample.json")), json!([1, 0, 0])),
        Some(json!({ "object": "pebble", "at": { "tile": [1, 0, 0] } }))
    );
    let proposals: Vec<_> = fs::read_dir(m.state.join("proposals")).map(|d| d.flatten().collect()).unwrap_or_default();
    assert!(proposals.is_empty());
}

// ----------------------------------------------------------------- servers and disk

fn run_cli(args: &[&str], env: &HashMap<String, String>, state: &Path) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_pixellab"))
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

#[test]
fn r5_r7_the_stdio_server_sends_doc_stale_for_a_dirty_doc_whose_file_changed_and_keeps_open_pid_json() {
    let doc = make_document();
    let state = temp_dir("pixellab-state-");
    let mut server = start_server(&["--stdio", "--document", doc.root.to_str().unwrap()], &doc.env(), &state);
    let info = server.request("session.info", json!({}))["result"].clone();
    let open_file = state.join("open").join(format!("{}.json", info["pid"]));
    server.request("scene.place", json!({ "name": "sample", "object": "pebble", "at": [3, 3, 0] }));
    let listed = read_json(&open_file);
    assert_eq!(
        [listed["pid"].clone(), listed["document"].clone(), listed["docs"].clone()],
        [info["pid"].clone(), json!(doc.root.to_str().unwrap()), json!([{ "doc": "scene:sample", "dirty": true }])]
    );
    let since = server.notifications().len();
    let file = doc.root.join("scenes/sample.json");
    let text = fs::read_to_string(&file).unwrap().replace("\"tile\": [0, 0, 1]", "\"tile\": [0, 0, 2]");
    assert!(text.contains("[0, 0, 2]"));
    fs::write(&file, text).unwrap();
    let stale = server.wait_for(since, "doc.stale", |m| m["method"] == "doc.stale");
    assert_eq!(stale["params"], json!({ "doc": "scene:sample", "file": "scenes/sample.json" }));
    let merged = server.request("doc.merge", json!({ "doc": "scene:sample" }));
    assert_eq!([merged["result"]["merged"].clone(), merged["result"]["dirty"].clone()], [json!(true), json!(true)]);
    server.request("doc.close", json!({ "doc": "scene:sample", "force": true }));
    assert_eq!(read_json(&open_file)["docs"], json!([]), "closing updates the list");
    server.request("shutdown", json!({}));
    assert_eq!(server.exited(), 0);
    assert!(!open_file.exists(), "removed when the server stops");
}

#[test]
fn r1_r3_an_mcp_session_settles_its_applied_proposal_before_the_next_call_doc_reload_and_doc_revert_annotations() {
    let doc = make_document();
    let state = temp_dir("pixellab-state-");
    let mut server = start_server(&["--mcp", "--document", doc.root.to_str().unwrap()], &doc.env(), &state);
    initialize(&mut server, "t", "1");
    let tools = server.request("tools/list", json!({}))["result"]["tools"].clone();
    let tool = |n: &str| tools.as_array().unwrap().iter().find(|t| t["name"] == n).cloned().unwrap_or_else(|| panic!("no tool {n}"));
    assert_eq!(tool("doc_revert")["annotations"]["destructiveHint"], true);
    assert_eq!(
        [tool("doc_reload")["annotations"]["destructiveHint"].clone(), tool("doc_reload")["annotations"]["readOnlyHint"].clone()],
        [json!(false), json!(false)]
    );
    assert!(tool("doc_merge").is_object());
    let structured = |server: &mut Server, name: &str, args: Value| call(server, name, args)["structuredContent"].clone();
    structured(&mut server, "scene_place", json!({ "name": "sample", "object": "pebble", "at": [3, 3, 0] }));
    let saved = structured(&mut server, "doc_save", json!({ "doc": "scene:sample" }));
    let applied = run_cli(&["proposal", "apply", str_of(&saved["proposal"]), "--document", doc.root.to_str().unwrap()], &doc.env(), &state);
    assert_eq!(applied.0, 0, "{}", applied.2);
    let status = structured(&mut server, "doc_status", json!({ "doc": "scene:sample" }));
    assert_eq!(
        [status["dirty"].clone(), status["stale"].clone(), status["synced"]["action"].clone(), status["unsavedInApp"].clone()],
        [json!(false), json!(false), json!("clean"), json!(false)]
    );
    assert_eq!(structured(&mut server, "doc_reload", json!({ "doc": "scene:sample" }))["reloaded"], false);
}

// ----------------------------------------------------------------- serve usage

#[test]
fn serve_usage_serve_needs_stdio_or_mcp() {
    let state = temp_dir("pixellab-state-");
    let env = HashMap::new();
    let r = run_cli(&["serve"], &env, &state);
    assert_eq!(r.0, 2);
    assert!(r.2.contains("give --stdio or --mcp"), "{}", r.2);
    let help = run_cli(&["serve", "--help"], &env, &state);
    assert_eq!(help.0, 0);
    // The Node help names `node <repo>/cli/bin/pixellab.mjs`; the program is the binary now.
    assert!(help.1.contains("claude mcp add pixellab -- "), "{}", help.1);
}
