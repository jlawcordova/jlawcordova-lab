//! R21 (docs/intents/2026-10-autosave/spec.md, D6): proposals left waiting from before edits
//! applied are set aside, not lost, with a notice the Agent window shows once. Temp state dirs only.
#![allow(clippy::result_large_err)]

use std::fs;
use std::path::Path;

use pixellab::state::{create_state, parse_activity, random_hex};
use serde_json::{json, Value};

fn temp() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pixellab-retire-test-{}-{}", std::process::id(), random_hex(4)));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A waiting proposal, as the program wrote them before edits applied.
fn waiting(dir: &Path, id: &str) {
    fs::create_dir_all(dir.join("proposals")).unwrap();
    let p = json!({ "id": id, "number": 1, "format": 2, "doc": "scene:x", "baseHash": null, "source": "{}", "summary": [], "createdAt": "2026-10-19T11:28:30.100Z", "agent": null });
    fs::write(dir.join("proposals").join(format!("{id}.json")), serde_json::to_string_pretty(&p).unwrap()).unwrap();
}

fn activity(dir: &Path) -> Vec<Value> {
    parse_activity(&fs::read_to_string(dir.join("activity.jsonl")).unwrap_or_default())
}

fn notice(dir: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.join("retired-proposals/notice.json")).unwrap()).unwrap()
}

#[test]
fn r21_waiting_proposals_move_to_retired_proposals_with_a_notice() {
    let dir = temp();
    waiting(&dir, "20261019-112830-aaaaaa");
    waiting(&dir, "20261019-112831-bbbbbb");
    create_state(&dir);
    assert!(!dir.join("proposals").exists(), "the proposals folder is gone");
    let n = notice(&dir);
    assert_eq!(n["count"], 2);
    let moved = Path::new(n["dir"].as_str().unwrap());
    assert!(moved.starts_with(dir.join("retired-proposals")));
    assert!(moved.join("20261019-112830-aaaaaa.json").is_file(), "set aside, not deleted");
    assert!(moved.join("20261019-112831-bbbbbb.json").is_file());
}

#[test]
fn r21_the_activity_line_says_how_many_and_why() {
    let dir = temp();
    for id in ["20261019-112830-aaaaaa", "20261019-112831-bbbbbb", "20261019-112832-cccccc"] {
        waiting(&dir, id);
    }
    create_state(&dir);
    let lines = activity(&dir);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["summary"], "retired 3 waiting proposals: edits apply now");
    assert_eq!(lines[0]["ok"], true);
    assert!(lines[0]["time"].is_string() && lines[0]["tool"].is_string() && lines[0]["args"].is_object());

    let one = temp();
    waiting(&one, "20261019-112830-aaaaaa");
    create_state(&one);
    assert_eq!(activity(&one)[0]["summary"], "retired 1 waiting proposal: edits apply now");
}

#[test]
fn r21_a_second_start_retires_nothing_and_logs_nothing() {
    let dir = temp();
    waiting(&dir, "20261019-112830-aaaaaa");
    create_state(&dir);
    create_state(&dir);
    assert_eq!(activity(&dir).len(), 1);
    assert_eq!(notice(&dir)["count"], 1);
    let empty = temp();
    create_state(&empty);
    assert!(!empty.join("activity.jsonl").exists());
    assert!(!empty.join("retired-proposals").exists());
}

#[test]
fn r21_an_old_numbers_log_is_left_alone() {
    let dir = temp();
    waiting(&dir, "20261019-112830-aaaaaa");
    fs::write(dir.join("proposal-numbers.log"), "20261019-112830-aaaaaa\n").unwrap();
    create_state(&dir);
    assert_eq!(fs::read_to_string(dir.join("proposal-numbers.log")).unwrap(), "20261019-112830-aaaaaa\n");
}
