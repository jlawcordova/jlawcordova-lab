//! The activity summaries, the state dir and the proposals (ports cli/test/activity-and-proposals.test.mjs).
#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use pixellab::activity_summary::activity_summary;
use pixellab::context::Ctx;
use pixellab::errors::Result;
use pixellab::registry::{Command, Params};
use pixellab::state::{create_state, iso_time, new_proposal_id, parse_activity, state_dir, write_atomic};
use serde_json::{json, Map, Value};

fn run(_: &mut Ctx, _: Params) -> Result<Value> {
    Ok(Value::Null)
}

fn cmd(name: &'static str) -> Command {
    let mut c = Command::new(name, "x", json!({ "type": "object", "properties": {} }), false, run);
    c.format = Some(|_, _| "formatted\nsecond line".into());
    c
}

fn cmd_with(name: &'static str, format: fn(&Value, &Params) -> String) -> Command {
    let mut c = cmd(name);
    c.format = Some(format);
    c
}

fn sum(c: &Command, r: Value) -> String {
    activity_summary(c, &r, &Map::new())
}

struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("pixellab-state-test-{tag}-{}-{}", std::process::id(), pixellab::state::random_hex(4)));
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn base(id: &str, doc: &str, created: &str) -> Value {
    json!({ "baseHash": null, "source": "", "summary": [], "agent": null, "id": id, "doc": doc, "createdAt": created })
}

#[test]
fn read_tools_get_a_short_summary_of_what_they_did() {
    let empty = || json!({});
    assert_eq!(sum(&cmd("scene.list"), json!({ "scenes": vec![empty(); 15] })), "listed 15 scenes");
    assert_eq!(sum(&cmd("object.list"), json!({ "objects": [empty()] })), "listed 1 object");
    assert_eq!(
        sum(&cmd("scene.render"), json!({ "scene": "hero-island", "scale": 2, "time": 0, "items": [] })),
        "rendered hero-island at 2×"
    );
    assert_eq!(sum(&cmd("scene.render"), json!({ "scene": "hero-island", "scale": 1, "time": 400 })), "rendered hero-island at 1×, 400 ms");
    assert_eq!(sum(&cmd("object.render"), json!({ "object": "house", "scale": 4, "frame": 0 })), "rendered house at 4×");
    assert_eq!(sum(&cmd("scene.items"), json!({ "scene": "meadow", "items": [empty(), empty()] })), "listed 2 items of meadow");
    assert_eq!(
        sum(&cmd("object.show"), json!({ "name": "house", "description": "46×42 · 10 colors" })),
        "showed object house (46×42 · 10 colors)"
    );
    assert_eq!(
        sum(&cmd("check"), json!({ "target": null, "checked": 177, "problems": [] })),
        "checked every source (177 files): no problems"
    );
    assert_eq!(
        sum(&cmd("check"), json!({ "target": { "name": "pine" }, "checked": 2, "problems": [empty()], "summary": "ignored" })),
        "checked pine (2 files): 1 problem"
    );
}

#[test]
fn edits_keep_their_own_summary_other_commands_fall_back_to_the_format() {
    assert_eq!(
        sum(&cmd("scene.place"), json!({ "summary": "pine placed at column 1, row 2, level 0" })),
        "pine placed at column 1, row 2, level 0"
    );
    assert_eq!(sum(&cmd_with("doc.close", |_, _| "scene:x:   closed".into()), json!({})), "scene:x: closed");
    assert_eq!(sum(&cmd("scene.list"), Value::Null), "listed 0 scenes");
    assert_eq!(sum(&cmd("scene.place"), json!({})), "formatted");
}

#[test]
fn proposals_list_oldest_first_by_created_at_even_within_one_second() {
    let tmp = TempDir::new("order");
    let state = create_state(&tmp.0);
    state.write_proposal(&base("20261019-112830-ffffff", "scene:a", "2026-10-19T11:28:30.100Z")).unwrap();
    state.write_proposal(&base("20261019-112830-000000", "object:b", "2026-10-19T11:28:30.900Z")).unwrap();
    let docs: Vec<String> = state.list_proposals().iter().map(|p| p["doc"].as_str().unwrap().to_string()).collect();
    assert_eq!(docs, ["scene:a", "object:b"]);
}

#[test]
fn iso_time_matches_to_iso_string() {
    // Values from `node -e "console.log(new Date(<ms>).toISOString())"`.
    for (ms, want) in [
        (0.0, "1970-01-01T00:00:00.000Z"),
        (951782400000.0, "2000-02-29T00:00:00.000Z"),
        (1792555782000.0, "2026-10-21T04:09:42.000Z"),
        (1792555782123.0, "2026-10-21T04:09:42.123Z"),
        (4102444800000.0, "2100-01-01T00:00:00.000Z"),
        (-1.0, "1969-12-31T23:59:59.999Z"),
        (-86400001.0, "1969-12-30T23:59:59.999Z"),
        (253402300800000.0, "+010000-01-01T00:00:00.000Z"),
        (-62198755200000.0, "-000001-01-01T00:00:00.000Z"),
    ] {
        assert_eq!(iso_time(ms), want, "{ms}");
    }
}

#[test]
fn new_proposal_id_is_sortable_and_file_safe() {
    let id = new_proposal_id(1792555782123.0);
    assert_eq!(id.len(), 22);
    assert!(id.starts_with("20261021-040942-"), "{id}");
    assert!(id[16..].bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)));
    assert_ne!(new_proposal_id(0.0), new_proposal_id(0.0));
}

#[test]
fn proposal_numbering_carries_on_across_writes_and_a_rewrite_keeps_its_number() {
    let tmp = TempDir::new("numbers");
    let state = create_state(&tmp.0);
    let a = state.write_proposal(&base("20261019-112830-aaaaaa", "scene:a", "2026-10-19T11:28:30.100Z")).unwrap();
    let b = state.write_proposal(&base("20261019-112831-bbbbbb", "scene:b", "2026-10-19T11:28:31.100Z")).unwrap();
    assert_eq!(a["number"], 1);
    assert_eq!(b["number"], 2);
    // key order: id, number, then the proposal's own keys
    let keys: Vec<&String> = a.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["id", "number", "format", "baseHash", "source", "summary", "agent", "doc", "createdAt"]);
    let again = state.write_proposal(&base("20261019-112830-aaaaaa", "scene:a2", "2026-10-19T11:28:30.100Z")).unwrap();
    assert_eq!(again["number"], 1);
    assert_eq!(state.number_of("20261019-112830-aaaaaa"), Some(1));
    assert_eq!(state.number_of("20261019-999999-ffffff"), None);
    let c = state.write_proposal(&base("20261019-112832-cccccc", "scene:c", "2026-10-19T11:28:32.100Z")).unwrap();
    assert_eq!(c["number"], 3);
    assert_eq!(fs::read_to_string(tmp.0.join("proposal-numbers.log")).unwrap().lines().count(), 3);
    let read = state.read_proposal("20261019-112830-aaaaaa").unwrap();
    assert_eq!(read["doc"], "scene:a2");
    assert_eq!(read["number"], 1);
    assert!(fs::read_to_string(tmp.0.join("proposals/20261019-112830-aaaaaa.json")).unwrap().ends_with("}\n"));
}

#[test]
fn proposals_validate_ids_read_and_remove() {
    let tmp = TempDir::new("ids");
    let state = create_state(&tmp.0);
    let bad = state.read_proposal("nope").unwrap_err();
    assert_eq!(bad.message(), "no proposal nope; ids look like 20261019-142301-a1b2c3 (pixellab proposal list)");
    let missing = state.read_proposal("20261019-112830-aaaaaa").unwrap_err();
    assert_eq!(missing.message(), "no proposal 20261019-112830-aaaaaa (pixellab proposal list)");
    assert!(!state.has_proposal("nope"));
    state.write_proposal(&base("20261019-112830-aaaaaa", "scene:a", "2026-10-19T11:28:30.100Z")).unwrap();
    assert!(state.has_proposal("20261019-112830-aaaaaa"));
    fs::write(tmp.0.join("proposals/20261019-112831-bbbbbb.json"), "{ not json").unwrap();
    assert_eq!(state.list_proposals().len(), 1);
    assert!(state
        .read_proposal("20261019-112831-bbbbbb")
        .unwrap_err()
        .message()
        .starts_with("proposal 20261019-112831-bbbbbb is not valid JSON: "));
    state.remove_proposal("20261019-112830-aaaaaa").unwrap();
    assert!(!state.has_proposal("20261019-112830-aaaaaa"));
    assert_eq!(
        state.remove_proposal("20261019-112830-aaaaaa").unwrap_err().message(),
        "no proposal 20261019-112830-aaaaaa (pixellab proposal list)"
    );
}

#[test]
fn activity_appends_and_tails_oldest_first_skipping_broken_lines() {
    let tmp = TempDir::new("activity");
    let state = create_state(tmp.0.join("nested"));
    assert!(state.tail_activity(50).is_empty());
    for i in 0..5 {
        state.append_activity(&json!({ "time": "t", "tool": "scene.list", "n": i })).unwrap();
    }
    let tail: Vec<i64> = state.tail_activity(2).iter().map(|e| e["n"].as_i64().unwrap()).collect();
    assert_eq!(tail, [3, 4]);
    assert_eq!(state.tail_activity(50).len(), 5);
    assert_eq!(parse_activity("{\"a\":1}\nbroken\n\n{\"b\":2}\n"), vec![json!({ "a": 1 }), json!({ "b": 2 })]);
    let line = fs::read_to_string(tmp.0.join("nested/activity.jsonl")).unwrap();
    assert!(line.starts_with("{\"time\":\"t\",\"tool\":\"scene.list\",\"n\":0}\n"));
}

#[test]
fn state_dir_resolves_the_environment() {
    let mut env = HashMap::new();
    env.insert("HOME".to_string(), "/Users/x".to_string());
    assert_eq!(state_dir(&env), PathBuf::from("/Users/x/Library/Application Support/Pixel Lab"));
    env.insert("PIXELLAB_STATE".to_string(), "/tmp/a/../b/".to_string());
    assert_eq!(state_dir(&env), PathBuf::from("/tmp/b"));
}

#[test]
fn write_atomic_leaves_no_temp_file() {
    let tmp = TempDir::new("atomic");
    let file = tmp.0.join("deep/er/file.txt");
    write_atomic(&file, "one").unwrap();
    write_atomic(&file, "two").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "two");
    assert_eq!(fs::read_dir(file.parent().unwrap()).unwrap().count(), 1);
}
