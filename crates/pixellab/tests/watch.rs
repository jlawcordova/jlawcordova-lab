//! Watching by polling (cli/lib/watch.mjs has no Node tests; these cover its behavior).

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use pixellab::state::State;
use pixellab::watch::{SourcesWatcher, StateWatcher, DEBOUNCE_MS};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pixellab-watch-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn doc(name: &str) -> PathBuf {
    let dir = temp(name);
    fs::create_dir_all(dir.join("objects")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    fs::write(dir.join("palette.json"), "{}").unwrap();
    fs::write(dir.join("objects/pine.json"), "{}").unwrap();
    fs::write(dir.join("scenes/hero.json"), "{}").unwrap();
    dir
}

fn bump(path: &std::path::Path, secs: u64) {
    let f = fs::File::options().write(true).open(path).unwrap();
    f.set_modified(SystemTime::now() + Duration::from_secs(secs)).unwrap();
}

#[test]
fn a_source_edit_is_reported_once_after_the_debounce() {
    let dir = doc("edit");
    let mut w = SourcesWatcher::new(&dir);
    assert!(w.poll(0.0).is_empty());
    fs::write(dir.join("objects/pine.json"), "{\"a\":1}").unwrap();
    assert!(w.poll(1000.0).is_empty());
    assert!(w.poll(1000.0 + DEBOUNCE_MS - 1.0).is_empty());
    assert_eq!(w.poll(1000.0 + DEBOUNCE_MS), vec!["objects/pine.json"]);
    assert!(w.poll(2000.0).is_empty());
}

#[test]
fn an_edit_that_keeps_the_size_is_seen_by_its_mtime() {
    let dir = doc("mtime");
    let mut w = SourcesWatcher::new(&dir);
    bump(&dir.join("palette.json"), 5);
    w.poll(0.0);
    assert_eq!(w.poll(200.0), vec!["palette.json"]);
}

#[test]
fn several_edits_within_the_debounce_are_one_report() {
    let dir = doc("burst");
    let mut w = SourcesWatcher::new(&dir);
    fs::write(dir.join("scenes/hero.json"), "{\"a\":1}").unwrap();
    assert!(w.poll(0.0).is_empty());
    fs::write(dir.join("objects/pine.json"), "{\"b\":22}").unwrap();
    assert!(w.poll(100.0).is_empty());
    assert!(w.poll(200.0).is_empty()); // 100 since the latest change
    assert_eq!(w.poll(250.0), vec!["objects/pine.json", "scenes/hero.json"]);
    assert!(w.poll(500.0).is_empty());
}

#[test]
fn a_file_added_and_a_file_removed_are_reported() {
    let dir = doc("addrm");
    let mut w = SourcesWatcher::new(&dir);
    fs::write(dir.join("scenes/new.json"), "{}").unwrap();
    fs::remove_file(dir.join("objects/pine.json")).unwrap();
    w.poll(0.0);
    assert_eq!(w.poll(150.0), vec!["objects/pine.json", "scenes/new.json"]);
}

#[test]
fn files_that_are_not_sources_are_ignored() {
    let dir = doc("ignore");
    let mut w = SourcesWatcher::new(&dir);
    fs::write(dir.join("manifest.json"), "{}").unwrap();
    fs::write(dir.join("objects/old.mjs"), "x").unwrap();
    fs::create_dir_all(dir.join("objects/nested")).unwrap();
    fs::write(dir.join("objects/nested/deep.json"), "{}").unwrap();
    fs::write(dir.join("notes.json"), "{}").unwrap();
    w.poll(0.0);
    assert!(w.poll(1000.0).is_empty());
}

#[test]
fn a_proposal_written_and_removed_is_reported() {
    let state = State::new(temp("proposals"));
    let mut w = StateWatcher::new(&state);
    assert!(state.proposals_dir.is_dir());
    assert!(!w.poll(0.0).proposals);
    fs::write(state.proposals_dir.join("p1.json"), "{}").unwrap();
    assert!(!w.poll(100.0).proposals);
    assert!(w.poll(250.0).proposals);
    assert!(!w.poll(500.0).proposals);
    fs::remove_file(state.proposals_dir.join("p1.json")).unwrap();
    assert!(!w.poll(600.0).proposals);
    assert!(w.poll(750.0).proposals);
}

#[test]
fn activity_lines_appended_are_returned_once_each_and_a_partial_line_waits() {
    let state = State::new(temp("activity"));
    fs::write(&state.activity_file, "{\"tool\":\"old\"}\n").unwrap();
    let mut w = StateWatcher::new(&state);
    assert!(w.poll(0.0).activity.is_empty()); // starts at the end
    let mut text = fs::read_to_string(&state.activity_file).unwrap();
    text.push_str("{\"tool\":\"a\"}\n{\"tool\":\"b\"}\n{\"tool\":\"c\"");
    fs::write(&state.activity_file, &text).unwrap();
    let got = w.poll(10.0).activity;
    assert_eq!(got.iter().map(|v| v["tool"].as_str().unwrap()).collect::<Vec<_>>(), ["a", "b"]);
    assert!(w.poll(20.0).activity.is_empty());
    text.push_str("}\n");
    fs::write(&state.activity_file, &text).unwrap();
    let got = w.poll(30.0).activity;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0]["tool"], "c");
    assert!(w.poll(40.0).activity.is_empty());
    fs::write(&state.activity_file, "{\"tool\":\"z\"}\n").unwrap(); // truncated: starts over
    assert_eq!(w.poll(50.0).activity[0]["tool"], "z");
}

#[test]
fn polling_a_folder_of_300_source_files_takes_under_a_millisecond() {
    let dir = doc("timing");
    for i in 0..150 {
        fs::write(dir.join(format!("objects/o{i}.json")), "{}").unwrap();
        fs::write(dir.join(format!("scenes/s{i}.json")), "{}").unwrap();
    }
    let mut w = SourcesWatcher::new(&dir);
    w.poll(0.0);
    let start = Instant::now();
    for i in 0..100 {
        assert!(w.poll(i as f64).is_empty());
    }
    let avg = start.elapsed().as_secs_f64() * 1000.0 / 100.0;
    assert!(avg < 1.0, "a poll took {avg} ms on average");
}
