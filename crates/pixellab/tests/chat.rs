//! The Agent chat's CLI side (ports cli/test/chat.test.mjs;
//! docs/intents/2026-10-agent-chat/spec.md): the selection block
//! (chat.context, D5), questions (chat.ask, D8), and proposals and activity
//! that know where they came from (D7). Everything runs on temp copies of
//! examples/harbor.pixellab, bootstrapped to format 2 once per run.

#![allow(clippy::result_large_err)]

mod common;

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use pixellab::chat_context::{match_saved, selection_context, SelectionInput, GROUP_PATHS};
use pixellab::cli::{main as cli_main, Io};
use pixellab::context::Ctx;
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::Error;
use pixellab::registry::registry;
use pixellab::session::{Options, Session};
use serde_json::{json, Value};

/// The harbor as format 2, made once with Node's bootstrap.
fn harbor_master() -> &'static PathBuf {
    static MASTER: OnceLock<PathBuf> = OnceLock::new();
    MASTER.get_or_init(|| {
        let base = common::temp_dir("pixellab-chat-harbor-");
        let to = base.join("harbor.pixellab");
        let out = Command::new("node")
            .current_dir(common::repo())
            .args(["scripts/goldens.mjs", "--bootstrap"])
            .arg(common::repo().join("examples/harbor.pixellab"))
            .arg(&to)
            .output()
            .expect("node");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        to
    })
}

struct Harbor {
    base: PathBuf,
    root: PathBuf,
    state: PathBuf,
    env: HashMap<String, String>,
    s: Session,
}

impl Drop for Harbor {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
        let _ = std::fs::remove_dir_all(&self.state);
    }
}

fn open(root: &Path, env: &HashMap<String, String>) -> pixellab::document::Document {
    open_document(&OpenOptions { doc: Some(root.to_string_lossy().into_owned()), site: None, env: env.clone(), cwd: root.to_path_buf() })
        .unwrap()
}

/// A temp copy of the harbor and a session on it (the app's).
fn harbor_with(chat: Option<Value>) -> Harbor {
    let base = common::temp_dir("pixellab-chat-");
    let root = base.join("harbor.pixellab");
    common::copy_dir(harbor_master(), &root);
    let state = common::temp_dir("pixellab-chat-state-");
    let home = base.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let env = HashMap::from([
        ("HOME".to_string(), home.to_string_lossy().into_owned()),
        ("PIXELLAB_STATE".to_string(), state.to_string_lossy().into_owned()),
    ]);
    let s = Session::new(open(&root, &env), Options { env: env.clone(), chat, ..Options::default() });
    Harbor { base, root, state, env, s }
}

fn harbor() -> Harbor {
    harbor_with(None)
}

impl Harbor {
    fn call(&mut self, cmd: &str, params: Value) -> Result<Value, Error> {
        registry().call(&mut Ctx::new(&mut self.s), cmd, params, false)
    }
    fn ok(&mut self, cmd: &str, params: Value) -> Value {
        self.call(cmd, params).unwrap_or_else(|e| panic!("{cmd}: {}", e.message()))
    }
}

fn code(e: Error) -> String {
    match e {
        Error::Command(c) => c.code,
        Error::Internal(m) => m,
    }
}

fn is_version(v: &str) -> bool {
    v.len() == 8 && v.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// Runs the CLI in process: (exit code, stdout, stderr).
fn run_cli(h: &Harbor, args: &[&str]) -> (i32, String, String) {
    let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let reg = registry();
    let code = {
        let mut io = Io { stdout: &mut out, stderr: &mut err, env: h.env.clone(), cwd: h.base.clone(), registry: &reg };
        cli_main(&argv, &mut io)
    };
    (code, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

#[derive(Clone)]
struct Sink(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for Sink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Runs the MCP server in process over these requests (method, params); returns each response's `result`.
fn mcp(h: &Harbor, env_extra: &[(&str, &str)], requests: &[(&str, Value)]) -> Vec<Value> {
    let mut env = h.env.clone();
    for (k, v) in env_extra {
        env.insert(k.to_string(), v.to_string());
    }
    let mut input = String::new();
    for (i, (method, params)) in requests.iter().enumerate() {
        input += &format!("{}\n", json!({ "jsonrpc": "2.0", "id": i + 1, "method": method, "params": params }));
    }
    let sink = Arc::new(Mutex::new(Vec::new()));
    let reg = registry();
    let document = open(&h.root, &env);
    pixellab::server_mcp::start(document, env, &reg, false, Box::new(Cursor::new(input.into_bytes())), Box::new(Sink(sink.clone())));
    let text = String::from_utf8(sink.lock().unwrap().clone()).unwrap();
    text.lines().map(|l| serde_json::from_str::<Value>(l).unwrap()["result"].clone()).collect()
}

fn init() -> (&'static str, Value) {
    (
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "claude-code", "version": "2.1.290" } }),
    )
}

fn activity(h: &Harbor) -> Vec<Value> {
    std::fs::read_to_string(h.state.join("activity.jsonl")).unwrap().trim().split('\n').map(|l| serde_json::from_str(l).unwrap()).collect()
}

// --- chat.context: the selection block (R10) ---

#[test]
fn r10_a_group_with_its_saved_paths_items_with_object_position_and_group_pins_marked() {
    let mut h = harbor();
    let r = h.ok("chat.context", json!({ "name": "harbor", "layers": ["trees"], "items": ["14.0", "51"], "pinned": ["layer:trees"] }));
    assert_eq!(r["document"], "harbor");
    assert_eq!(r["scene"], "harbor");
    assert!(is_version(r["version"].as_str().unwrap()));
    let text = r["text"].as_str().unwrap();
    let lines: Vec<&str> = text.split('\n').collect();
    assert_eq!(lines[0], format!("<selection document=\"harbor\" scene=\"harbor\" version=\"{}\">", r["version"].as_str().unwrap()));
    assert_eq!(lines[1], "  group trees \"Trees\" items=1.0,14.0,21.0,28.0,39.0,49.0,50.0,80.0 pinned");
    assert_eq!(lines[2], "  item 14.0 object=pine at=tile 3,0,0 group=trees");
    assert_eq!(lines[3], "  item 51 object=rock at=tile 0,7,0");
    assert_eq!(*lines.last().unwrap(), "</selection>");
    assert_eq!(r["unsaved"], json!([]));
    let saved: Vec<&Value> = r["items"].as_array().unwrap().iter().map(|i| &i["savedPath"]).collect();
    assert_eq!(saved, [&json!("14.0"), &json!("51")]);
    assert_eq!(r["layers"][0]["saved"], true);
}

#[test]
fn r10_nothing_selected_says_the_whole_scene_is_in_view() {
    let mut h = harbor();
    let r = h.ok("chat.context", json!({ "name": "harbor" }));
    assert_eq!(r["text"].as_str().unwrap().split('\n').nth(1).unwrap(), "  (nothing selected: the whole scene is in view)");
}

#[test]
fn r10_a_layer_nested_in_the_selected_one_counts_and_a_long_group_lists_60_paths_then_says_how_many_more() {
    let mut h = harbor();
    let ground = h.ok("chat.context", json!({ "name": "harbor", "layers": ["ground"] }));
    let text = ground["text"].as_str().unwrap().to_string();
    let line = text.split('\n').nth(1).unwrap();
    assert!(line.starts_with("  group ground \"Ground\" items="), "{line}");
    assert_eq!(line.split("items=").nth(1).unwrap().split(' ').next().unwrap().split(',').count(), GROUP_PATHS);
    assert!(line.ends_with("…and 2 more (scene_items lists them)"), "the harbor's ground holds 62 items, in its four layers: {line}");
}

#[test]
fn r10_an_object_names_the_whole_object() {
    let mut h = harbor();
    let r = h.ok("chat.context", json!({ "object": "pine" }));
    let text = r["text"].as_str().unwrap();
    let version = r["version"].as_str().unwrap();
    assert!(is_version(version));
    assert_eq!(
        text,
        format!("<selection document=\"harbor\" object=\"pine\" version=\"{version}\">\n  (the whole object is in view)\n</selection>")
    );
}

#[test]
fn r10_an_unknown_item_or_layer_is_not_found_not_not_saved() {
    let mut h = harbor();
    assert_eq!(code(h.call("chat.context", json!({ "name": "harbor", "items": ["3"] })).unwrap_err()), "not_found");
    assert_eq!(code(h.call("chat.context", json!({ "name": "harbor", "layers": ["nope"] })).unwrap_err()), "not_found");
}

#[test]
fn r10_the_cli_prints_the_block() {
    let h = harbor();
    let root = h.root.to_str().unwrap().to_string();
    let r = run_cli(&h, &["chat", "context", "harbor", "--items", "14.0", "--document", &root]);
    assert_eq!(r.0, 0, "{}", r.2);
    let lines: Vec<&str> = r.1.split('\n').collect();
    assert!(lines[0].starts_with("<selection document=\"harbor\" scene=\"harbor\" version=\"") && lines[0].ends_with("\">"), "{}", r.1);
    assert!(is_version(&lines[0]["<selection document=\"harbor\" scene=\"harbor\" version=\"".len()..lines[0].len() - 2]));
    assert_eq!(lines[1..], ["  item 14.0 object=pine at=tile 3,0,0 group=trees", "</selection>", ""]);
}

// --- chat.context: what is not saved is left out (R11) ---

#[test]
fn r11_an_item_placed_since_the_save_is_left_out_items_it_pushed_down_the_list_keep_their_saved_paths() {
    let mut h = harbor();
    let placed = h.ok("scene.place", json!({ "name": "harbor", "object": "rock", "at": [0, 0, 1] }));
    let new_path = placed["path"]
        .as_str()
        .map(str::to_string)
        .or_else(|| placed["item"]["path"].as_str().map(str::to_string))
        .unwrap_or_else(|| panic!("{placed}"));
    let items = h.ok("scene.items", json!({ "name": "harbor" }));
    let pine =
        items["items"].as_array().unwrap().iter().find(|i| i["object"] == "pine" && i["at"]["tile"] == json!([5, 6, 0])).unwrap().clone();
    let pine_path = pine["path"].as_str().unwrap().to_string();
    assert_ne!(pine_path, "80.0", "the placed rock sorted in before the pine, so its working path moved");
    let r = h.ok("chat.context", json!({ "name": "harbor", "items": [new_path, pine_path] }));
    assert_eq!(r["unsaved"], json!([new_path]));
    assert_eq!(r["items"][1]["savedPath"], "80.0", "the pine is named by its path in the saved file");
    let text = r["text"].as_str().unwrap();
    assert!(text.contains("\n  item 80.0 object=pine at=tile 5,6,0 group=trees\n"), "{text}");
    assert!(text.contains("\n  (1 selected item is left out: it isn't saved yet)\n"), "{text}");
    assert!(!text.contains(&format!("item {new_path} ")));
}

#[test]
fn r11_a_moved_item_a_new_layer_and_a_layer_whose_items_changed_are_left_out_saving_brings_them_back() {
    let mut h = harbor();
    h.ok("scene.move", json!({ "name": "harbor", "item": "14.0", "by": [1, 0] }));
    h.ok("scene.layer.new", json!({ "name": "harbor", "layerName": "Rocks", "items": ["51"] }));
    h.ok("scene.layer.assign", json!({ "name": "harbor", "to": "trees", "items": ["85.0"] }));
    let items = h.ok("scene.items", json!({ "name": "harbor" }));
    let moved = items["items"].as_array().unwrap().iter().find(|i| i["object"] == "pine" && i["at"]["tile"] == json!([4, 0, 0])).unwrap()
        ["path"]
        .as_str()
        .unwrap()
        .to_string();
    let r = h.ok("chat.context", json!({ "name": "harbor", "layers": ["rocks", "trees", "boat"], "items": [moved] }));
    assert_eq!(r["unsaved"], json!(["layer:rocks", "layer:trees", moved]));
    let text = r["text"].as_str().unwrap();
    assert!(text.contains("\n  group boat \"Boat\" items=41\n"), "{text}");
    assert!(text.contains("(2 selected groups and 1 selected item are left out: they aren't saved yet)"), "{text}");
    h.ok("doc.save", json!({ "doc": "scene:harbor" }));
    let after = h.ok("chat.context", json!({ "name": "harbor", "layers": ["rocks", "trees"], "items": [moved] }));
    assert_eq!(after["unsaved"], json!([]));
    assert_ne!(after["version"], r["version"], "the version is the saved file's");
}

#[test]
fn r11_identical_items_match_in_order() {
    let a = json!({ "object": "a", "at": { "tile": [0, 0, 0] } });
    let saved = json!({ "items": [a, a] });
    let working = json!({ "items": [{ "object": "b", "at": { "tile": [9, 9, 0] } }, a, a, a] });
    let got: Vec<(String, Option<String>)> = match_saved(&working, &saved).iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let s = |x: &str| Some(x.to_string());
    assert_eq!(got, [("0".to_string(), None), ("1".to_string(), s("0")), ("2".to_string(), s("1")), ("3".to_string(), None)]);
}

#[test]
fn r11_an_item_whose_svg_group_changed_attributes_is_not_the_saved_one() {
    let saved = json!({ "items": [{ "group": { "class": "enter" }, "items": [{ "object": "a", "at": { "tile": [0, 0, 0] } }] }] });
    let working =
        json!({ "items": [{ "group": { "class": "enter", "data-x": "3" }, "items": [{ "object": "a", "at": { "tile": [0, 0, 0] } }] }] });
    let r = selection_context(&SelectionInput {
        document: "d",
        scene: "s",
        version: "v",
        working: &working,
        saved: &saved,
        layers: &[],
        items: &["0.0".to_string()],
        pinned: &[],
    })
    .unwrap();
    assert_eq!(r["unsaved"], json!(["0.0"]));
}

// --- chat.ask: a question with answer buttons (R15, R23) ---

#[test]
fn r15_from_a_chat_it_is_asked_from_a_terminal_it_says_to_ask_in_the_conversation() {
    let mut chat = harbor_with(Some(json!({ "id": "c1", "title": "Pines" })));
    let asked = chat.ok("chat.ask", json!({ "question": "Every pine, or only these 2?", "options": ["Only these 2", "Every pine (5)"] }));
    assert_eq!(asked["asked"], true);
    assert_eq!(asked["options"], json!(["Only these 2", "Every pine (5)"]));
    assert!(asked["note"].as_str().unwrap().contains("their next message"));
    assert_eq!(asked["chat"], json!({ "id": "c1", "title": "Pines" }));
    let mut terminal = harbor();
    let not = terminal.ok("chat.ask", json!({ "question": "Which?", "options": ["A", "B"] }));
    assert_eq!(not["asked"], false);
    assert!(not["note"].as_str().unwrap().contains("ask in your own conversation"));
}

#[test]
fn r15_two_to_four_answers_each_a_short_label() {
    let mut h = harbor();
    let long = "x".repeat(61);
    for options in [json!(["A"]), json!(["A", "B", "C", "D", "E"]), json!(["A", long])] {
        let e = h.call("chat.ask", json!({ "question": "Which?", "options": options })).unwrap_err();
        assert_eq!(code(e), "usage", "{options}");
    }
}

#[test]
fn r23_over_mcp_with_pixellab_chat_chat_ask_and_chat_context_are_tools_the_question_is_asked_and_the_activity_line_says_so() {
    let h = harbor();
    let out = mcp(
        &h,
        &[("PIXELLAB_CHAT", "chat-7"), ("PIXELLAB_CHAT_TITLE", "Forest by the house")],
        &[
            init(),
            ("tools/list", json!({})),
            ("tools/call", json!({ "name": "chat_ask", "arguments": { "question": "Every pine?", "options": ["Yes", "No"] } })),
        ],
    );
    assert!(out[0]["instructions"].as_str().unwrap().contains("chat_ask"));
    let names: Vec<&str> = out[1]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for name in ["chat_ask", "chat_context"] {
        assert!(names.contains(&name), "{name}");
    }
    assert_eq!(out[2]["structuredContent"]["asked"], true);
    assert_eq!(out[2]["structuredContent"]["chat"], json!({ "id": "chat-7", "title": "Forest by the house" }));
    let lines = activity(&h);
    let last = lines.last().unwrap();
    assert_eq!(last["summary"], "asked: Every pine?");
    assert_eq!(last["agent"], "claude-code 2.1.290");
    assert_eq!(last["chat"], json!({ "id": "chat-7", "title": "Forest by the house" }));
}

// --- proposals and activity know where they came from (R24) ---

#[test]
fn r24_a_chats_proposal_records_the_chat_and_a_number_the_activity_lines_record_the_agent_and_the_chat() {
    let mut h = harbor();
    let out = mcp(
        &h,
        &[("PIXELLAB_CHAT", "chat-1"), ("PIXELLAB_CHAT_TITLE", "Move the pines")],
        &[
            init(),
            ("tools/call", json!({ "name": "scene_move", "arguments": { "name": "harbor", "item": "14.0", "by": [1, 0] } })),
            ("tools/call", json!({ "name": "doc_save", "arguments": { "doc": "scene:harbor" } })),
        ],
    );
    assert!(out[1].get("isError").is_none(), "{}", out[1]);
    let saved = out[2]["structuredContent"].clone();
    assert_eq!(saved["number"], 1);
    let summary = saved["summary"].as_str().unwrap();
    let pre = "Proposed scene:harbor as #1 (";
    assert!(summary.starts_with(pre) && summary.ends_with(") (1 edit)"), "{summary}");
    let id = saved["proposal"].as_str().unwrap().to_string();
    assert_eq!(&summary[pre.len()..summary.len() - ") (1 edit)".len()], id);
    let p: Value = serde_json::from_str(&std::fs::read_to_string(h.state.join("proposals").join(format!("{id}.json"))).unwrap()).unwrap();
    assert_eq!(p["number"], 1);
    assert_eq!(p["chat"], json!({ "id": "chat-1", "title": "Move the pines" }));
    assert_eq!(p["agent"], "claude-code 2.1.290");
    let listed = h.ok("proposal.list", json!({}))["proposals"][0].clone();
    assert_eq!(listed["number"], 1);
    assert_eq!(listed["chat"], json!({ "id": "chat-1", "title": "Move the pines" }));
    let shown = h.ok("proposal.show", json!({ "id": id }));
    assert_eq!(shown["number"], 1);
    assert_eq!(shown["chat"], listed["chat"]);
    for line in activity(&h) {
        assert_eq!(line["agent"], "claude-code 2.1.290");
        assert_eq!(line["chat"], json!({ "id": "chat-1", "title": "Move the pines" }));
    }
    let root = h.root.to_str().unwrap().to_string();
    let r = run_cli(&h, &["proposal", "list", "--document", &root]);
    let want_tail = "  scene:harbor  chat \"Move the pines\"  1 edit";
    assert!(
        r.1.lines().any(|l| {
            l.strip_prefix("#1  ").is_some_and(|rest| {
                let (pid, tail) = rest.split_at(rest.find("  ").unwrap_or(0));
                pid == id && tail == want_tail
            })
        }),
        "{}",
        r.1
    );
}

#[test]
fn r24_numbers_count_up_a_rewrite_keeps_its_number_and_servers_proposing_at_once_get_their_own() {
    let dir = common::temp_dir("pixellab-chat-numbers-");
    let state = Arc::new(pixellab::state::create_state(dir.clone()));
    let base = |id: &str| json!({ "id": id, "doc": "scene:a", "baseHash": null, "source": "", "summary": [], "createdAt": "2026-10-21T00:00:00.000Z", "agent": null });
    let ids: Vec<String> = (0..8).map(|i| format!("20261021-000000-00000{i}")).collect();
    let threads: Vec<_> = ids
        .iter()
        .map(|id| {
            let (dir, p) = (dir.clone(), base(id));
            std::thread::spawn(move || pixellab::state::create_state(dir).write_proposal(&p).unwrap())
        })
        .collect();
    let written: Vec<Value> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    let mut numbers: Vec<u64> = written.iter().map(|p| p["number"].as_u64().unwrap()).collect();
    numbers.sort();
    assert_eq!(numbers, [1, 2, 3, 4, 5, 6, 7, 8], "eight at once, eight numbers");
    let mut rewrite = base(&ids[3]);
    rewrite["summary"] = json!(["rewritten"]);
    let again = state.write_proposal(&rewrite).unwrap();
    assert_eq!(again["number"], written[3]["number"]);
    let next = state.write_proposal(&base("20261021-000001-aaaaaa")).unwrap();
    assert_eq!(next["number"], 9);
    assert_eq!(std::fs::read_to_string(dir.join("proposal-numbers.log")).unwrap().trim().split('\n').count(), 9);
    assert_eq!(std::fs::read_dir(dir.join("proposals")).unwrap().count(), 9, "numbers.log is beside the proposals, not among them");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn r24_an_older_proposal_without_a_number_or_a_chat_still_lists_shows_and_is_rejected() {
    let mut h = harbor();
    std::fs::create_dir_all(h.state.join("proposals")).unwrap();
    let id = "20261019-120000-abcdef";
    let source = std::fs::read_to_string(h.root.join("scenes/harbor.json")).unwrap();
    let old = json!({ "id": id, "doc": "scene:harbor", "baseHash": null, "source": source, "summary": ["old"], "createdAt": "2026-10-19T12:00:00.000Z", "agent": "old-agent" });
    std::fs::write(h.state.join("proposals").join(format!("{id}.json")), engine::format::stringify(&old)).unwrap();
    let listed = h.ok("proposal.list", json!({}))["proposals"].clone();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert!(listed[0].get("number").is_none());
    assert!(listed[0].get("chat").is_none());
    assert_eq!(h.ok("proposal.show", json!({ "id": id }))["agent"], "old-agent");
    h.ok("proposal.reject", json!({ "id": id }));
    assert_eq!(h.ok("proposal.list", json!({}))["proposals"], json!([]));
}
