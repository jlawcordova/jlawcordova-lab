//! Docs in step with disk (ports cli/test/sync.test.mjs;
//! docs/intents/2026-10-mcp-gaps/spec.md, R2–R4, R8): a clean doc follows its
//! file, doc.reload reloads only when nothing is lost, the edits since disk
//! are listed, and doc.merge merges a change on disk into a dirty working
//! copy. Proposals are gone (docs/intents/2026-10-autosave/spec.md, R16), so
//! their settling cases went with them. Sessions run in-process on a temp
//! copy of the sample document.
#![allow(clippy::result_large_err)]

mod common;

use std::path::PathBuf;

use pixellab::author::Author;
use pixellab::context::Ctx;
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::Error;
use pixellab::registry::registry;
use pixellab::session::{Options, Session};
use serde_json::{json, Value};

const SCENE: &str = "scenes/sample.json";
const BLOCK: &str = "{ \"object\": \"block\", \"at\": { \"tile\": [0, 0, 0] } }";

struct Lab {
    doc: common::TempDocument,
    state: PathBuf,
    original: String,
}

impl Lab {
    fn new() -> Lab {
        let doc = common::make_document();
        let state = common::temp_dir("pixellab-state-");
        let original = std::fs::read_to_string(doc.root.join(SCENE)).unwrap();
        Lab { doc, state, original }
    }

    fn session(&self, agent: Option<&str>, on_stale: Option<Box<dyn FnMut(Value)>>) -> Session {
        let mut env = self.doc.env();
        env.insert("PIXELLAB_STATE".into(), self.state.to_string_lossy().into_owned());
        let document = open_document(&OpenOptions {
            doc: Some(self.doc.root.to_string_lossy().into_owned()),
            site: None,
            env: env.clone(),
            cwd: self.doc.root.clone(),
        })
        .unwrap();
        let mut s = Session::new(document, Options { env, agent: agent.map(String::from), on_stale, ..Options::default() });
        if agent.is_some() {
            s.author = Author::agent(agent.map(String::from), None);
        }
        s
    }

    /// An agent's session.
    fn agent(&self) -> Session {
        self.session(Some("test"), None)
    }

    /// The owner's session (direct), sharing the state dir.
    fn owner(&self) -> Session {
        self.session(None, None)
    }

    fn write_scene(&self, text: &str) {
        std::fs::write(self.doc.root.join(SCENE), text).unwrap();
    }
}

fn call(s: &mut Session, name: &str, params: Value) -> Result<Value, Error> {
    registry().call(&mut Ctx::new(s), name, params, false)
}

fn ok(r: Result<Value, Error>) -> Value {
    r.unwrap_or_else(|e| panic!("{e:?}"))
}

/// One MCP tool call: the server syncs first.
fn mcp(s: &mut Session, name: &str, params: Value) -> Value {
    s.sync();
    ok(call(s, name, params))
}

fn command_error(r: Result<Value, Error>) -> pixellab::errors::CommandError {
    match r.expect_err("an error") {
        Error::Command(c) => c,
        Error::Internal(m) => panic!("internal error: {m}"),
    }
}

fn place(s: &mut Session, col: i64) -> Value {
    mcp(s, "scene.place", json!({ "name": "sample", "object": "pebble", "at": [col, 2, 0] }))
}

fn status(s: &mut Session) -> Value {
    mcp(s, "doc.status", json!({ "doc": "scene:sample" }))
}

// ---------------------------------------------------------------- a clean doc follows its file (R2)

#[test]
fn r2_a_clean_doc_reloads_when_its_file_changes_a_dirty_one_that_now_matches_the_file_is_clean() {
    let lab = Lab::new();
    let mut a = lab.agent();
    mcp(&mut a, "doc.open", json!({ "doc": "scene:sample" }));
    lab.write_scene(&lab.original.replace("\"tile\": [0, 0, 1]", "\"tile\": [0, 0, 2]"));
    assert_eq!(mcp(&mut a, "scene.items", json!({ "name": "sample" }))["items"][1]["at"], json!({ "tile": [0, 0, 2] }));
    assert_eq!(status(&mut a)["stale"], false);
    // The owner saves the very edit the agent made.
    place(&mut a, 4);
    let mut o = lab.owner();
    ok(call(&mut o, "scene.place", json!({ "name": "sample", "object": "pebble", "at": [4, 2, 0] })));
    ok(call(&mut o, "doc.save", json!({ "doc": "scene:sample" })));
    let s = status(&mut a);
    assert_eq!(json!([s["dirty"], s["stale"], s["edits"]]), json!([false, false, []]));
}

// ---------------------------------------------------------------- doc.reload (R3)

#[test]
fn r3_reloads_a_clean_stale_doc_and_says_when_there_was_nothing_to_load() {
    let lab = Lab::new();
    let mut a = lab.agent();
    ok(call(&mut a, "doc.open", json!({ "doc": "scene:sample" })));
    assert_eq!(ok(call(&mut a, "doc.reload", json!({ "doc": "scene:sample" })))["reloaded"], false);
    lab.write_scene(&lab.original.replace("\"tile\": [0, 0, 1]", "\"tile\": [0, 0, 3]"));
    let r = ok(call(&mut a, "doc.reload", json!({ "doc": "scene:sample" })));
    assert_eq!(json!([r["reloaded"], r["changed"], r["dirty"]]), json!([true, true, false]));
}

#[test]
fn r3_refuses_unsaved_edits_code_dirty_the_edits_in_details_and_reloads_once_they_are_saved() {
    let lab = Lab::new();
    let mut a = lab.agent();
    ok(call(&mut a, "scene.place", json!({ "name": "sample", "object": "pebble", "at": [1, 2, 0] })));
    let e = command_error(call(&mut a, "doc.reload", json!({ "doc": "scene:sample" })));
    assert_eq!(e.code, "dirty");
    assert_eq!(e.details, Some(json!(["pebble placed at column 1, row 2, level 0"])));
    ok(call(&mut a, "doc.save", json!({ "doc": "scene:sample" })));
    // Once saved, the edits are on disk, so reloading drops nothing.
    let r = ok(call(&mut a, "doc.reload", json!({ "doc": "scene:sample" })));
    assert_eq!(json!([r["reloaded"], r["changed"], r["dirty"]]), json!([false, false, false]));
}

// ---------------------------------------------------------------- edits since disk (R4)

#[test]
fn r4_after_a_save_only_later_edits_are_listed_undoing_a_saved_edit_is_listed_as_an_undo() {
    let lab = Lab::new();
    let mut o = lab.owner();
    let st = |o: &mut Session| ok(call(o, "doc.status", json!({ "doc": "scene:sample" })));
    ok(call(&mut o, "scene.place", json!({ "name": "sample", "object": "pebble", "at": [1, 2, 0] })));
    ok(call(&mut o, "doc.save", json!({ "doc": "scene:sample" })));
    assert_eq!(st(&mut o)["edits"], json!([]));
    ok(call(&mut o, "doc.undo", json!({ "doc": "scene:sample" })));
    assert_eq!(st(&mut o)["edits"], json!(["Undid: pebble placed at column 1, row 2, level 0"]));
    ok(call(&mut o, "doc.redo", json!({ "doc": "scene:sample" })));
    let s = st(&mut o);
    assert_eq!(json!([s["edits"], s["dirty"]]), json!([[], false]));
    // A repeat right after a save is its own step, not merged into the saved one.
    ok(call(&mut o, "scene.move", json!({ "name": "sample", "item": "0", "by": [1, 0] })));
    ok(call(&mut o, "doc.save", json!({ "doc": "scene:sample" })));
    ok(call(&mut o, "scene.move", json!({ "name": "sample", "item": "0", "by": [1, 0] })));
    assert_eq!(st(&mut o)["edits"], json!(["block moved to column 2, row 0, level 0"]));
}

// ---------------------------------------------------------------- doc.merge (R8)

fn move_block_on_disk(lab: &Lab) {
    lab.write_scene(&lab.original.replace(BLOCK, "{ \"object\": \"block\", \"at\": { \"tile\": [0, -1, 0] } }"));
}

#[test]
fn r8_doc_merge_brings_the_files_change_into_a_dirty_copy_as_one_undo_step_keeping_its_edits() {
    let lab = Lab::new();
    let mut o = lab.owner();
    ok(call(&mut o, "scene.place", json!({ "name": "sample", "object": "pebble", "at": [5, 5, 0] })));
    // Someone else moves the block on disk.
    assert!(lab.original.contains(BLOCK));
    move_block_on_disk(&lab);
    assert_eq!(ok(call(&mut o, "doc.status", json!({ "doc": "scene:sample" })))["stale"], true);
    let r = ok(call(&mut o, "doc.merge", json!({ "doc": "scene:sample" })));
    assert_eq!(json!([r["merged"], r["dirty"]]), json!([true, true]));
    let s = ok(call(&mut o, "doc.status", json!({ "doc": "scene:sample" })));
    assert_eq!(json!([s["stale"], s["edits"]]), json!([false, ["pebble placed at column 5, row 5, level 0"]]));
    let listed: Vec<Value> = ok(call(&mut o, "scene.items", json!({ "name": "sample" })))["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| json!([i["object"], i["at"]["tile"]]))
        .collect();
    assert_eq!(Value::Array(listed), json!([["block", [0, -1, 0]], ["pebble", [0, 0, 1]], ["pebble", [5, 5, 0]]]));
    ok(call(&mut o, "doc.undo", json!({ "doc": "scene:sample" })));
    assert_eq!(
        ok(call(&mut o, "scene.items", json!({ "name": "sample" })))["items"][0]["at"],
        json!({ "tile": [0, 0, 0] }),
        "undo takes the merge back out"
    );
    ok(call(&mut o, "doc.redo", json!({ "doc": "scene:sample" })));
    let saved = ok(call(&mut o, "doc.save", json!({ "doc": "scene:sample" })));
    assert_eq!(saved["wrote"], true, "no conflict: the base is the merged file");
}

#[test]
fn r8_a_clash_changes_nothing_and_says_where() {
    let lab = Lab::new();
    let mut o = lab.owner();
    ok(call(&mut o, "scene.move", json!({ "name": "sample", "item": "0", "by": [1, 0] })));
    move_block_on_disk(&lab);
    let e = command_error(call(&mut o, "doc.merge", json!({ "doc": "scene:sample" })));
    assert_eq!(e.code, "merge_conflict");
    let details = e.details.unwrap();
    assert_eq!(details.as_array().unwrap().len(), 1);
    assert!(details[0]["ours"][0].as_str().unwrap().contains("\"tile\": [1, 0, 0]"), "{details}");
    assert!(details[0]["theirs"][0].as_str().unwrap().contains("\"tile\": [0, -1, 0]"), "{details}");
    let s = ok(call(&mut o, "doc.status", json!({ "doc": "scene:sample" })));
    assert_eq!(json!([s["dirty"], s["stale"]]), json!([true, true]));
}
