//! The version history (docs/intents/2026-10-autosave/spec.md, R8–R15, D3, D4).

mod common;

use std::cell::Cell;
use std::rc::Rc;

use pixellab::author::Author;
use pixellab::context::{Ctx, DocKind};
use pixellab::document::{open_document, OpenOptions};
use pixellab::history::{Change, History};
use pixellab::session::{Edit, Options, Session};
use serde_json::{json, Map, Value};

/// A session over a temp document with a clock the test moves.
fn session(doc: &common::TempDocument, clock: &Rc<Cell<f64>>) -> Session {
    let env = doc.env();
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    let c = clock.clone();
    Session::new(document, Options { env, now: Box::new(move || c.get()), ..Options::default() })
}

fn place(s: &mut Session, col: i64) {
    Ctx::new(s)
        .edit(DocKind::Scene, "sample", None, |doc, _| {
            doc["items"].as_array_mut().unwrap().push(json!({ "object": "pebble", "at": { "tile": [col, 2, 0] } }));
            Ok(Edit::new(Map::new(), format!("Placed pebble at {col}")))
        })
        .unwrap();
}

fn save(s: &mut Session) {
    s.save("scene:sample", false).unwrap();
}

const T0: f64 = 1_790_000_000_000.0;

#[test]
fn r8_version_0_holds_the_document_as_it_was_before_the_first_write_and_reading_writes_nothing() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(T0));
    let mut s = session(&doc, &clock);
    s.doc(DocKind::Scene, "sample").unwrap();
    assert!(!s.history.dir.exists(), "a session that only reads writes nothing");
    let sample = std::fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap();
    place(&mut s, 1);
    save(&mut s);
    let versions = s.history.versions();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0]["number"], 0);
    assert_eq!(versions[0]["author"], json!({ "kind": "opened" }));
    assert_eq!(s.history.text_at(0, "scenes/sample.json"), Some(Some(sample)), "the scene before the write");
    assert!(s.history.text_at(0, "objects/pebble.json").is_some(), "and every other doc as it is");
}

#[test]
fn r8_every_write_is_recorded_with_its_author_and_edits() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(T0));
    let mut s = session(&doc, &clock);
    let before = std::fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap();
    place(&mut s, 1);
    save(&mut s);
    let v = s.history.versions().last().cloned().unwrap();
    assert_eq!(v["number"], 1);
    assert_eq!(v["author"], json!({ "kind": "owner" }));
    assert_eq!(v["edits"], json!(["Placed pebble at 1"]));
    assert_eq!(v["changes"].as_array().unwrap().len(), 1);
    assert_eq!(s.history.text_at(0, "scenes/sample.json"), Some(Some(before)));
    let after = std::fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap();
    assert_eq!(s.history.text_at(1, "scenes/sample.json"), Some(Some(after)));
}

#[test]
fn r9_one_authors_writes_run_into_one_version_until_someone_else_writes() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(T0));
    let mut s = session(&doc, &clock);
    place(&mut s, 1);
    save(&mut s);
    clock.set(T0 + 5_000.0);
    place(&mut s, 2);
    save(&mut s);
    assert_eq!(s.history.versions().len(), 2, "the owner's two saves are one version");
    s.author = Author::agent(Some("claude".into()), None);
    place(&mut s, 3);
    save(&mut s);
    let versions = s.history.versions();
    assert_eq!(versions.len(), 3);
    assert_eq!(versions[1]["edits"], json!(["Placed pebble at 1", "Placed pebble at 2"]));
    assert_eq!(versions[1]["open"], false);
    assert_eq!(versions[2]["author"], json!({ "kind": "agent", "name": "claude" }));
}

#[test]
fn r9_a_pause_or_a_mark_starts_a_new_version() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(T0));
    let mut s = session(&doc, &clock);
    place(&mut s, 1);
    save(&mut s);
    clock.set(T0 + 61_000.0);
    place(&mut s, 2);
    save(&mut s);
    assert_eq!(s.history.versions().len(), 3, "a minute's pause starts a new version");
    s.history.mark(None).unwrap();
    place(&mut s, 3);
    save(&mut s);
    assert_eq!(s.history.versions().len(), 4, "a mark starts a new version");
}

#[test]
fn r9_a_change_from_outside_is_its_own_version() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(T0));
    let mut s = session(&doc, &clock);
    s.open(DocKind::Scene, "sample").unwrap();
    let file = doc.root.join("scenes/sample.json");
    let text = std::fs::read_to_string(&file).unwrap();
    let mut v: Value = serde_json::from_str(&text).unwrap();
    v["items"].as_array_mut().unwrap().pop();
    std::fs::write(&file, engine::format::stringify_pretty(&v) + "\n").unwrap();
    assert!(s.reload_if_clean("scene:sample"));
    let last = s.history.versions().last().cloned().unwrap();
    assert_eq!(last["author"], json!({ "kind": "outside" }));
    assert_eq!(last["edits"], json!(["Changed outside Pixel Lab"]));
}

#[test]
fn r10_old_versions_thin_to_one_a_day_and_one_a_week_keeping_their_state() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(T0));
    let mut s = session(&doc, &clock);
    let day = 86_400_000.0;
    // Two versions on one day, 40 days ago; two on one day, 3 days ago; one today.
    for (t, col) in [(T0, 1), (T0 + 2.0 * 3_600_000.0, 2), (T0 + 37.0 * day, 3), (T0 + 37.0 * day + 3_600_000.0, 4), (T0 + 40.0 * day, 5)] {
        clock.set(t);
        place(&mut s, col);
        save(&mut s);
        s.history.mark(None).unwrap();
    }
    let n = s.history.versions().len();
    assert_eq!(n, 6);
    let last_of_day = s.history.versions()[2]["number"].as_u64().unwrap();
    let state_then = s.history.text_at(last_of_day, "scenes/sample.json");
    let gone = s.history.thin(T0 + 40.0 * day + 1000.0).unwrap();
    assert_eq!(gone, 2);
    let versions = s.history.versions();
    assert_eq!(versions.len(), 4);
    assert_eq!(versions[1]["edits"], json!(["Placed pebble at 1", "Placed pebble at 2"]));
    assert_eq!(s.history.text_at(last_of_day, "scenes/sample.json"), state_then, "a kept version's state is kept");
    let blobs = std::fs::read_dir(s.history.dir.join("blobs")).unwrap().count();
    let changes: usize = versions.iter().map(|v| v["changes"].as_array().unwrap().len()).sum();
    assert!(blobs <= changes * 2, "blobs no version needs are gone");
}

#[test]
fn r11_history_lives_in_the_state_dir_and_nothing_is_added_to_the_document() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(T0));
    let mut s = session(&doc, &clock);
    let listing = |root: &std::path::Path| {
        let mut all: Vec<String> = walk(root);
        all.sort();
        all
    };
    let before = listing(&doc.root);
    place(&mut s, 1);
    save(&mut s);
    let mut after = listing(&doc.root);
    after.retain(|f| !before.contains(f));
    assert!(after.is_empty(), "new files in the document: {after:?}");
    assert!(s.history.dir.starts_with(&s.state.dir));
}

fn walk(dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p.to_string_lossy().into_owned());
        }
    }
    out
}

#[test]
fn r11_history_is_keyed_by_the_documents_real_path() {
    let doc = common::make_document();
    let state = common::temp_dir("pixellab-state-");
    let a = History::new(&state, &doc.root, &doc.root);
    let b = History::new(&state, &doc.root.join("..").join("sample.pixellab"), &doc.root);
    assert_eq!(a.dir, b.dir);
    a.ensure_base(T0).unwrap();
    a.record(&[Change { file: "scenes/x.json".into(), before: None, after: Some("{}".into()) }], &Author::cli(), &[], T0).unwrap();
    assert_eq!(b.latest(), Some(1));
}

// ------------------------------------------------------------ commands (D4)

fn call(s: &mut Session, name: &str, params: Value) -> Result<Value, pixellab::errors::Error> {
    let reg = pixellab::registry::registry();
    let mut ctx = Ctx::new(s);
    reg.call(&mut ctx, name, params, false)
}

fn sample(doc: &common::TempDocument) -> String {
    std::fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap()
}

#[test]
fn r13_restore_puts_the_document_back_as_one_undoable_step_and_a_version() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(T0));
    let mut s = session(&doc, &clock);
    let original = sample(&doc);
    place(&mut s, 1);
    save(&mut s);
    let at1 = sample(&doc);
    s.history.mark(None).unwrap();
    place(&mut s, 2);
    save(&mut s);
    assert_ne!(sample(&doc), at1);
    let r = call(&mut s, "history.restore", json!({ "version": 1 })).unwrap();
    assert_eq!(r["changed"], true, "{r}");
    assert_eq!(sample(&doc), at1);
    let last = s.history.versions().last().cloned().unwrap();
    assert_eq!(last["author"]["kind"], "restore");
    assert_eq!(last["author"]["name"], "you");
    // One undo step takes the restore back; it saves like any edit.
    s.step("scene:sample", true).unwrap();
    s.save("scene:sample", false).unwrap();
    assert_ne!(sample(&doc), at1);
    // Version 0 is the document as it was first opened.
    call(&mut s, "history.restore", json!({ "version": 0 })).unwrap();
    assert_eq!(engine::format::parse(&sample(&doc)).unwrap(), engine::format::parse(&original).unwrap());
}

#[test]
fn r13_restore_of_one_doc_leaves_the_others() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(T0));
    let mut s = session(&doc, &clock);
    place(&mut s, 1);
    save(&mut s);
    let r = call(&mut s, "history.restore", json!({ "version": 0, "doc": "scene:sample" })).unwrap();
    assert_eq!(r["written"], json!(["scene:sample"]));
    let r = call(&mut s, "history.restore", json!({ "version": 0, "doc": "sample" })).unwrap();
    assert_eq!(r["changed"], false);
}

#[test]
fn r14_undo_these_changes_takes_back_an_agents_version_and_keeps_later_edits() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(T0));
    let mut s = session(&doc, &clock);
    s.author = Author::agent(Some("claude".into()), None);
    place(&mut s, 7);
    save(&mut s);
    let agent_version = s.history.latest().unwrap();
    s.author = Author::owner();
    // The owner edits another part of the scene afterwards.
    Ctx::new(&mut s)
        .edit(DocKind::Scene, "sample", None, |doc, _| {
            doc["viewBox"] = json!([-16, -16, 80, 48]);
            Ok(Edit::new(Map::new(), "Widened the view"))
        })
        .unwrap();
    save(&mut s);
    let r = call(&mut s, "history.undo", json!({ "version": agent_version })).unwrap();
    assert_eq!(r["changed"], true, "{r}");
    let now: Value = serde_json::from_str(&sample(&doc)).unwrap();
    let cols: Vec<i64> = now["items"].as_array().unwrap().iter().filter_map(|i| i["at"]["tile"][0].as_i64()).collect();
    assert!(!cols.contains(&7), "the agent's pebble is gone: {cols:?}");
    assert_eq!(now["viewBox"], json!([-16, -16, 80, 48]), "the owner's later edit stays");
}

#[test]
fn r13_undo_refuses_when_later_edits_touched_the_same_lines() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(T0));
    let mut s = session(&doc, &clock);
    s.author = Author::agent(Some("claude".into()), None);
    place(&mut s, 7);
    save(&mut s);
    let agent_version = s.history.latest().unwrap();
    s.author = Author::owner();
    // The owner moves the agent's pebble: the same line.
    Ctx::new(&mut s)
        .edit(DocKind::Scene, "sample", None, |doc, _| {
            for item in doc["items"].as_array_mut().unwrap() {
                if item["at"]["tile"][0] == json!(7) {
                    item["at"]["tile"][0] = json!(8);
                }
            }
            Ok(Edit::new(Map::new(), "Moved pebble"))
        })
        .unwrap();
    save(&mut s);
    let before = sample(&doc);
    let err = call(&mut s, "history.undo", json!({ "version": agent_version })).unwrap_err();
    let pixellab::errors::Error::Command(e) = err else { panic!("an internal error") };
    assert_eq!(e.code, "clash", "{}", e.message);
    assert_eq!(sample(&doc), before, "nothing written");
}

#[test]
fn r15_history_reaches_the_cli() {
    let doc = common::make_document();
    let reg = pixellab::registry::registry();
    let run = |args: &[&str]| {
        let mut argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        argv.extend(["--document".to_string(), doc.root.to_string_lossy().into_owned()]);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = {
            let mut io = pixellab::cli::Io { stdout: &mut out, stderr: &mut err, env: doc.env(), cwd: doc.root.clone(), registry: &reg };
            pixellab::cli::main(&argv, &mut io)
        };
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    };
    let (code, _, err) = run(&["scene", "place", "sample", "pebble", "--at", "3,0,0"]);
    assert_eq!(code, 0, "{err}");
    let (code, out, err) = run(&["history", "list"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("#1") && out.contains("the CLI"), "{out}");
    let (code, out, err) = run(&["history", "show", "1"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("pebble"), "{out}");
    let (code, out, err) = run(&["history", "restore", "0"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("Restored to version 0"), "{out}");
}
