//! Autosave and authors (docs/intents/2026-10-autosave/spec.md): who made
//! each undo step, and the Edit menu's labels.

mod common;

use pixellab::author::Author;
use pixellab::context::{Ctx, DocKind};
use pixellab::document::{open_document, OpenOptions};
use pixellab::session::{Edit, Options, Session};
use serde_json::{json, Map, Value};

fn session(doc: &common::TempDocument) -> Session {
    let env = doc.env();
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    Session::new(document, Options { env, ..Options::default() })
}

fn place(s: &mut Session, col: i64, key: Option<&str>) -> Value {
    let mut ctx = Ctx::new(s);
    ctx.edit(DocKind::Scene, "sample", key, |doc, _| {
        doc["items"].as_array_mut().unwrap().push(json!({ "object": "pebble", "at": { "tile": [col, 2, 0] } }));
        Ok(Edit::new(Map::new(), format!("Placed pebble at {col}")))
    })
    .unwrap()
}

#[test]
fn r20_the_edit_menu_names_an_agents_step() {
    let doc = common::make_document();
    let mut s = session(&doc);
    place(&mut s, 1, None);
    let live = s.live_status("scene:sample");
    assert_eq!(live["undoLabel"], "Placed pebble at 1");
    s.author = Author::agent(Some("claude-code 2.1.0".into()), None);
    place(&mut s, 2, None);
    let live = s.live_status("scene:sample");
    assert_eq!(live["undoLabel"], "Placed pebble at 2 (Claude)");
    assert_eq!(live["author"], json!({ "kind": "agent", "name": "claude-code 2.1.0" }));
    s.author = Author::owner();
    s.step("scene:sample", true).unwrap();
    let live = s.live_status("scene:sample");
    assert_eq!(live["undoLabel"], "Placed pebble at 1");
    assert_eq!(live["redoLabel"], "Placed pebble at 2 (Claude)");
}

#[test]
fn r20_edits_by_two_authors_never_coalesce() {
    let doc = common::make_document();
    let mut s = session(&doc);
    place(&mut s, 1, Some("drag"));
    s.author = Author::agent(Some("claude".into()), None);
    place(&mut s, 2, Some("drag"));
    assert_eq!(s.entries.get("scene:sample").unwrap().undo.len(), 2);
}

// ------------------------------------------------------------- autosave (D1)

use std::cell::Cell;
use std::rc::Rc;

fn timed(doc: &common::TempDocument, clock: &Rc<Cell<f64>>) -> Session {
    let env = doc.env();
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    let c = clock.clone();
    Session::new(document, Options { env, now: Box::new(move || c.get()), autosave_ms: Some(1000.0), ..Options::default() })
}

fn file(doc: &common::TempDocument) -> String {
    std::fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap()
}

#[test]
fn r1_an_edit_saves_itself_a_second_after_the_last_edit() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(1000.0));
    let mut s = timed(&doc, &clock);
    let before = file(&doc);
    place(&mut s, 1, None);
    assert_eq!(s.save_state("scene:sample").0, "pending");
    clock.set(1500.0);
    s.autosave(clock.get(), false);
    assert_eq!(file(&doc), before, "not before a second has passed");
    clock.set(2100.0);
    s.autosave(clock.get(), false);
    assert_ne!(file(&doc), before);
    assert_eq!(s.save_state("scene:sample").0, "saved");
    assert_eq!(s.live_status("scene:sample")["saveState"], "saved");
}

#[test]
fn r2_autosave_never_writes_an_invalid_doc_and_saves_once_it_is_valid() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(1000.0));
    let mut s = timed(&doc, &clock);
    let before = file(&doc);
    Ctx::new(&mut s)
        .edit(DocKind::Scene, "sample", None, |doc, _| {
            doc["items"].as_array_mut().unwrap().push(json!({ "object": "nothing-by-this-name", "at": { "tile": [1, 1, 0] } }));
            Ok(Edit::new(Map::new(), "Placed a ghost"))
        })
        .unwrap();
    clock.set(3000.0);
    s.autosave(clock.get(), false);
    assert_eq!(file(&doc), before, "an invalid doc isn't written");
    let (state, n) = s.save_state("scene:sample");
    assert_eq!((state, n), ("problems", 1));
    // Nothing retries until it changes; undoing the bad edit makes it valid again.
    s.step("scene:sample", true).unwrap();
    place(&mut s, 2, None);
    clock.set(5000.0);
    s.autosave(clock.get(), false);
    assert_ne!(file(&doc), before);
    assert_eq!(s.save_state("scene:sample").0, "saved");
}

#[test]
fn r6_undo_works_across_autosaves() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(1000.0));
    let mut s = timed(&doc, &clock);
    let before = file(&doc);
    place(&mut s, 1, None);
    clock.set(3000.0);
    s.autosave(clock.get(), false);
    assert_ne!(file(&doc), before);
    s.step("scene:sample", true).unwrap();
    clock.set(5000.0);
    s.autosave(clock.get(), false);
    assert_eq!(
        engine::format::parse(&file(&doc)).unwrap(),
        engine::format::parse(&before).unwrap(),
        "the undo saved too, and the file is as it was"
    );
}

#[test]
fn r7_a_change_from_outside_merges_into_a_doc_with_a_save_pending() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(1000.0));
    let mut s = timed(&doc, &clock);
    place(&mut s, 5, None);
    // Someone else widens the view on disk meanwhile.
    let mut v: Value = serde_json::from_str(&file(&doc)).unwrap();
    v["viewBox"] = json!([-16, -16, 96, 48]);
    std::fs::write(doc.root.join("scenes/sample.json"), engine::format::stringify_pretty(&v) + "\n").unwrap();
    clock.set(3000.0);
    let results = s.autosave(clock.get(), false);
    assert_eq!(results[0]["saved"], true, "{results:?}");
    let now: Value = serde_json::from_str(&file(&doc)).unwrap();
    assert_eq!(now["viewBox"], json!([-16, -16, 96, 48]), "theirs kept");
    assert!(now["items"].as_array().unwrap().iter().any(|i| i["at"]["tile"][0] == 5), "ours kept");
}

#[test]
fn r7_a_clash_blocks_the_doc_and_reports_it_stale() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(1000.0));
    let mut s = timed(&doc, &clock);
    let stale: Rc<Cell<bool>> = Rc::default();
    let seen = stale.clone();
    s.on_stale = Some(Box::new(move |_| seen.set(true)));
    Ctx::new(&mut s)
        .edit(DocKind::Scene, "sample", None, |doc, _| {
            doc["viewBox"] = json!([-16, -16, 80, 48]);
            Ok(Edit::new(Map::new(), "Widened the view"))
        })
        .unwrap();
    let mut v: Value = serde_json::from_str(&file(&doc)).unwrap();
    v["viewBox"] = json!([-16, -16, 96, 48]);
    std::fs::write(doc.root.join("scenes/sample.json"), engine::format::stringify_pretty(&v) + "\n").unwrap();
    clock.set(3000.0);
    s.autosave(clock.get(), false);
    assert_eq!(s.save_state("scene:sample").0, "conflict");
    assert!(stale.get());
    let now: Value = serde_json::from_str(&file(&doc)).unwrap();
    assert_eq!(now["viewBox"], json!([-16, -16, 96, 48]), "the file is left as it is");
}

#[test]
fn r5_doc_save_saves_and_closes_the_version() {
    let doc = common::make_document();
    let clock = Rc::new(Cell::new(1000.0));
    let mut s = timed(&doc, &clock);
    place(&mut s, 1, None);
    let reg = pixellab::registry::registry();
    reg.call(&mut Ctx::new(&mut s), "doc.save", json!({ "doc": "scene:sample" }), false).unwrap();
    assert_eq!(s.history.versions().last().unwrap()["open"], false);
    let r = reg.call(&mut Ctx::new(&mut s), "doc.flush", json!({}), false).unwrap();
    assert_eq!(r["saved"], true);
}

// ------------------------------------------------------------ the app's server

#[test]
fn r1_the_apps_server_writes_an_edit_without_doc_save() {
    let doc = common::make_document();
    let state = common::temp_dir("pixellab-state-");
    let mut env = doc.env();
    env.insert("PIXELLAB_AUTOSAVE_MS".into(), "100".into());
    let mut server = common::server::start_server(&["--stdio", "--document", doc.root.to_str().unwrap()], &env, &state);
    let before = file(&doc);
    let since = server.notifications().len();
    server.request("scene.place", json!({ "name": "sample", "object": "pebble", "at": [3, 0, 0] }));
    server.wait_for(since, "a saved doc.changed", |n| n["method"] == "doc.changed" && n["params"]["saveState"] == "saved");
    assert_ne!(file(&doc), before);
}

#[test]
fn r24_the_apps_server_saves_what_is_pending_when_it_stops() {
    let doc = common::make_document();
    let state = common::temp_dir("pixellab-state-");
    let mut env = doc.env();
    env.insert("PIXELLAB_AUTOSAVE_MS".into(), "60000".into());
    let mut server = common::server::start_server(&["--stdio", "--document", doc.root.to_str().unwrap()], &env, &state);
    let before = file(&doc);
    server.request("scene.place", json!({ "name": "sample", "object": "pebble", "at": [3, 0, 0] }));
    assert_eq!(file(&doc), before);
    assert_eq!(server.close(), 0);
    assert_ne!(file(&doc), before, "saved on the way out");
}
