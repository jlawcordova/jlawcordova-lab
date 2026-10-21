//! R25 / D11: a clean slate for proposals. Temp state dirs only.
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

/// A proposal as an older program wrote it: no `format`.
fn old(dir: &Path, id: &str) {
    fs::create_dir_all(dir.join("proposals")).unwrap();
    let p = json!({ "id": id, "number": 1, "doc": "scene:x", "baseHash": null, "source": "{}", "summary": [], "createdAt": "2026-10-19T11:28:30.100Z", "agent": null });
    fs::write(dir.join("proposals").join(format!("{id}.json")), serde_json::to_string_pretty(&p).unwrap()).unwrap();
}

fn activity(dir: &Path) -> Vec<Value> {
    parse_activity(&fs::read_to_string(dir.join("activity.jsonl")).unwrap_or_default())
}

#[test]
fn r25_d11_old_proposals_move_to_retired_proposals_on_first_start() {
    let dir = temp();
    old(&dir, "20261019-112830-aaaaaa");
    old(&dir, "20261019-112831-bbbbbb");
    let state = create_state(&dir);
    assert!(state.list_proposals().is_empty());
    assert!(!state.has_proposal("20261019-112830-aaaaaa"));
    assert!(dir.join("retired-proposals/20261019-112830-aaaaaa.json").is_file());
    assert!(dir.join("retired-proposals/20261019-112831-bbbbbb.json").is_file());
    assert!(!dir.join("proposals/20261019-112830-aaaaaa.json").exists());
}

#[test]
fn r25_d11_the_activity_line_is_plural_for_three_and_singular_for_one() {
    let dir = temp();
    for id in ["20261019-112830-aaaaaa", "20261019-112831-bbbbbb", "20261019-112832-cccccc"] {
        old(&dir, id);
    }
    create_state(&dir);
    let lines = activity(&dir);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["summary"], "retired 3 proposals from before the format change");
    assert_eq!(lines[0]["ok"], true);
    assert!(lines[0]["time"].is_string() && lines[0]["tool"].is_string() && lines[0]["args"].is_object());

    let one = temp();
    old(&one, "20261019-112830-aaaaaa");
    create_state(&one);
    assert_eq!(activity(&one)[0]["summary"], "retired 1 proposal from before the format change");
}

#[test]
fn r25_d11_a_second_start_retires_nothing_and_logs_nothing() {
    let dir = temp();
    old(&dir, "20261019-112830-aaaaaa");
    create_state(&dir);
    create_state(&dir);
    assert_eq!(activity(&dir).len(), 1);
    assert_eq!(fs::read_dir(dir.join("retired-proposals")).unwrap().count(), 1);
    let empty = temp();
    create_state(&empty);
    assert!(!empty.join("activity.jsonl").exists());
    assert!(!empty.join("retired-proposals").exists());
}

#[test]
fn r25_d11_numbers_carry_on_after_retiring() {
    let dir = temp();
    old(&dir, "20261019-112830-aaaaaa");
    fs::write(dir.join("proposal-numbers.log"), "20261019-112830-aaaaaa\n").unwrap();
    let state = create_state(&dir);
    assert_eq!(fs::read_to_string(dir.join("proposal-numbers.log")).unwrap(), "20261019-112830-aaaaaa\n");
    let written = state
        .write_proposal(&json!({ "id": "20261019-120000-dddddd", "doc": "scene:x", "baseHash": null, "source": "{}", "summary": [], "createdAt": "2026-10-19T12:00:00.000Z", "agent": null }))
        .unwrap();
    assert_eq!(written["number"], 2);
}

#[test]
fn r25_d11_a_format_2_proposal_is_kept_and_carries_format_after_number() {
    let dir = temp();
    old(&dir, "20261019-112830-aaaaaa");
    let state = create_state(&dir);
    state
        .write_proposal(&json!({ "id": "20261019-120000-dddddd", "doc": "scene:x", "baseHash": null, "source": "{}", "summary": [], "createdAt": "2026-10-19T12:00:00.000Z", "agent": null }))
        .unwrap();
    let text = fs::read_to_string(dir.join("proposals/20261019-120000-dddddd.json")).unwrap();
    let keys: Vec<String> = serde_json::from_str::<serde_json::Map<String, Value>>(&text).unwrap().keys().cloned().collect();
    assert_eq!(keys[..3], ["id", "number", "format"]);
    assert!(text.contains("\"format\": 2"));
    // a later start keeps it, and logs nothing more
    let again = create_state(&dir);
    assert_eq!(again.list_proposals().len(), 1);
    assert!(dir.join("proposals/20261019-120000-dddddd.json").is_file());
    assert_eq!(activity(&dir).len(), 1);
}
