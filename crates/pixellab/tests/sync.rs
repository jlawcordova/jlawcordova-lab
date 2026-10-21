//! Docs in step with disk (ports cli/test/sync.test.mjs;
//! docs/intents/2026-10-mcp-gaps/spec.md, R1–R8): an MCP session settles its
//! proposals once the owner applies or rejects them, doc.reload reloads only
//! when nothing is lost, proposals list the edits since disk, the app's open
//! docs are visible to agents, and doc.merge and proposal.apply merge a
//! change on disk into a dirty working copy. Sessions run in-process on a
//! temp copy of the sample document.
#![allow(clippy::result_large_err)]

mod common;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use pixellab::context::Ctx;
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::Error;
use pixellab::registry::registry;
use pixellab::session::{hash_text, Mode, Options, Session};
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

    fn session(&self, mode: Mode, agent: Option<&str>, on_stale: Option<Box<dyn FnMut(Value)>>) -> Session {
        let mut env = self.doc.env();
        env.insert("PIXELLAB_STATE".into(), self.state.to_string_lossy().into_owned());
        let document = open_document(&OpenOptions {
            doc: Some(self.doc.root.to_string_lossy().into_owned()),
            site: None,
            env: env.clone(),
            cwd: self.doc.root.clone(),
        })
        .unwrap();
        Session::new(document, Options { mode, env, agent: agent.map(String::from), on_stale, ..Options::default() })
    }

    /// An agent's session (proposals).
    fn agent(&self) -> Session {
        self.session(Mode::Propose, Some("test"), None)
    }

    /// The owner's session (direct), sharing the state dir.
    fn owner(&self) -> Session {
        self.session(Mode::Direct, None, None)
    }

    fn write_scene(&self, text: &str) {
        std::fs::write(self.doc.root.join(SCENE), text).unwrap();
    }

    fn scene_text(&self) -> String {
        std::fs::read_to_string(self.doc.root.join(SCENE)).unwrap()
    }

    /// The scene's items as they are on disk.
    fn items(&self) -> Vec<Value> {
        let v: Value = serde_json::from_str(&self.scene_text()).unwrap();
        v["items"].as_array().unwrap().clone()
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

// ---------------------------------------------------------------- settling proposals (R1, R4)

#[test]
fn r1_applied_with_no_later_edits_the_doc_is_clean_not_stale_and_the_next_proposal_starts_from_the_file() {
    let lab = Lab::new();
    let mut a = lab.agent();
    place(&mut a, 1);
    let first = mcp(&mut a, "doc.save", json!({ "doc": "scene:sample" }));
    ok(call(&mut lab.owner(), "proposal.apply", json!({ "id": first["proposal"] })));
    let s = status(&mut a);
    assert_eq!(s["dirty"], false);
    assert_eq!(s["stale"], false);
    assert_eq!(s["synced"], json!({ "doc": "scene:sample", "proposal": first["proposal"], "outcome": "applied", "action": "clean" }));
    assert!(s.get("proposal").is_none());
    // R4: the next proposal lists only the new edit, has a new id and the file's hash as its base.
    mcp(&mut a, "scene.move", json!({ "name": "sample", "item": "0", "by": [1, 0] }));
    let second = mcp(&mut a, "doc.save", json!({ "doc": "scene:sample" }));
    assert_ne!(second["proposal"], first["proposal"]);
    assert_eq!(second["edits"], json!(["block moved to column 1, row 0, level 0"]));
    let path = lab.state.join("proposals").join(format!("{}.json", second["proposal"].as_str().unwrap()));
    let p: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(p["summary"], json!(["block moved to column 1, row 0, level 0"]));
    assert_eq!(p["baseHash"], hash_text(&lab.scene_text()));
    ok(call(&mut lab.owner(), "proposal.apply", json!({ "id": second["proposal"] })));
    assert_eq!(lab.items().len(), 3, "both proposals landed");
}

#[test]
fn r1_rejected_with_no_later_edits_the_doc_reloads_from_the_file() {
    let lab = Lab::new();
    let mut a = lab.agent();
    place(&mut a, 1);
    let saved = mcp(&mut a, "doc.save", json!({ "doc": "scene:sample" }));
    ok(call(&mut lab.owner(), "proposal.reject", json!({ "id": saved["proposal"] })));
    let s = status(&mut a);
    assert_eq!(
        json!([s["dirty"], s["stale"], s["canUndo"], s["synced"]["outcome"], s["synced"]["action"]]),
        json!([false, false, false, "rejected", "reloaded"])
    );
    assert_eq!(
        mcp(&mut a, "scene.items", json!({ "name": "sample" }))["items"].as_array().unwrap().len(),
        2,
        "the rejected pebble is gone"
    );
}

#[test]
fn r1_r4_applied_with_later_edits_rebased_keeping_only_the_later_edits_as_unsaved() {
    let lab = Lab::new();
    let mut a = lab.agent();
    place(&mut a, 1);
    let saved = mcp(&mut a, "doc.save", json!({ "doc": "scene:sample" }));
    mcp(&mut a, "scene.move", json!({ "name": "sample", "item": "0", "by": [0, 1] }));
    ok(call(&mut lab.owner(), "proposal.apply", json!({ "id": saved["proposal"] })));
    let s = status(&mut a);
    assert_eq!(json!([s["dirty"], s["stale"], s["synced"]["action"]]), json!([true, false, "rebased"]));
    assert_eq!(s["edits"], json!(["block moved to column 0, row 1, level 0"]));
    let again = mcp(&mut a, "doc.save", json!({ "doc": "scene:sample" }));
    assert_eq!(again["edits"], json!(["block moved to column 0, row 1, level 0"]));
    let diff = again["diff"].as_str().unwrap();
    assert!(diff.lines().any(|l| l == "-    { \"object\": \"block\", \"at\": { \"tile\": [0, 0, 0] } },"), "{diff}");
    assert!(
        !diff.lines().any(|l| l.starts_with('+') && l.contains("pebble") && l.contains("[1, 2, 0]")),
        "the applied pebble is not in the new diff"
    );
}

#[test]
fn r1_rejected_with_later_edits_kept_as_it_is_and_doc_status_says_so() {
    let lab = Lab::new();
    let mut a = lab.agent();
    place(&mut a, 1);
    let saved = mcp(&mut a, "doc.save", json!({ "doc": "scene:sample" }));
    place(&mut a, 3);
    ok(call(&mut lab.owner(), "proposal.reject", json!({ "id": saved["proposal"] })));
    let s = status(&mut a);
    assert_eq!(json!([s["dirty"], s["stale"], s["synced"]["outcome"], s["synced"]["action"]]), json!([true, false, "rejected", "kept"]));
    assert_eq!(s["edits"].as_array().unwrap().len(), 2);
}

#[test]
fn r1_a_new_doc_whose_proposal_was_rejected_is_discarded() {
    let lab = Lab::new();
    let mut a = lab.agent();
    mcp(&mut a, "scene.new", json!({ "name": "meadow" }));
    let saved = mcp(&mut a, "doc.save", json!({ "doc": "scene:meadow" }));
    ok(call(&mut lab.owner(), "proposal.reject", json!({ "id": saved["proposal"] })));
    let all = mcp(&mut a, "doc.status", json!({}));
    assert_eq!(all["docs"], json!([]));
    assert_eq!(
        all["settled"].as_array().unwrap().last().unwrap(),
        &json!({ "doc": "scene:meadow", "proposal": saved["proposal"], "outcome": "rejected", "action": "discarded" })
    );
}

#[test]
fn r1_a_proposal_still_waiting_leaves_the_doc_alone() {
    let lab = Lab::new();
    let mut a = lab.agent();
    place(&mut a, 1);
    let saved = mcp(&mut a, "doc.save", json!({ "doc": "scene:sample" }));
    let s = status(&mut a);
    assert_eq!(s["dirty"], true);
    assert_eq!(s["proposal"], saved["proposal"]);
    assert!(s.get("synced").is_none());
}

// ---------------------------------------------------------------- clean docs follow the file (R2)

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
fn r3_refuses_unsaved_edits_code_dirty_the_edits_in_details_and_a_waiting_proposal_code_proposed() {
    let lab = Lab::new();
    let mut a = lab.agent();
    ok(call(&mut a, "scene.place", json!({ "name": "sample", "object": "pebble", "at": [1, 2, 0] })));
    let e = command_error(call(&mut a, "doc.reload", json!({ "doc": "scene:sample" })));
    assert_eq!(e.code, "dirty");
    assert_eq!(e.details, Some(json!(["pebble placed at column 1, row 2, level 0"])));
    let saved = ok(call(&mut a, "doc.save", json!({ "doc": "scene:sample" })));
    assert_eq!(command_error(call(&mut a, "doc.reload", json!({ "doc": "scene:sample" }))).code, "proposed");
    // Once applied, the edits are on disk, so reloading drops nothing.
    ok(call(&mut lab.owner(), "proposal.apply", json!({ "id": saved["proposal"] })));
    let r = ok(call(&mut a, "doc.reload", json!({ "doc": "scene:sample" })));
    assert_eq!(json!([r["reloaded"], r["changed"], r["dirty"]]), json!([true, false, false]));
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

// ---------------------------------------------------------------- the app's open docs (R7)

/// An open-docs file for a process, as the app's server writes it.
fn write_open_docs(lab: &Lab, pid: i64, document: &std::path::Path, docs: Value) {
    let dir = lab.state.join("open");
    std::fs::create_dir_all(&dir).unwrap();
    let data = json!({ "pid": pid, "document": document.to_string_lossy(), "updatedAt": "2026-10-21T00:00:00.000Z", "docs": docs });
    std::fs::write(dir.join(format!("{pid}.json")), serde_json::to_string_pretty(&data).unwrap()).unwrap();
}

#[test]
fn r7_doc_status_lists_what_the_app_has_unsaved_and_a_proposal_for_one_of_them_warns() {
    let lab = Lab::new();
    // A live process that isn't this one stands in for the app's server.
    let parent = std::os::unix::process::parent_id() as i64;
    write_open_docs(
        &lab,
        parent,
        &lab.doc.root,
        json!([{ "doc": "scene:sample", "dirty": true }, { "doc": "object:pebble", "dirty": false }]),
    );
    let mut a = lab.agent();
    assert_eq!(ok(call(&mut a, "doc.status", json!({})))["unsavedInApp"], json!(["scene:sample"]));
    ok(call(&mut a, "scene.place", json!({ "name": "sample", "object": "pebble", "at": [1, 2, 0] })));
    assert_eq!(ok(call(&mut a, "doc.status", json!({ "doc": "scene:sample" })))["unsavedInApp"], true);
    let saved = ok(call(&mut a, "doc.save", json!({ "doc": "scene:sample" })));
    assert!(saved["warnings"][0].as_str().unwrap().starts_with("scene:sample has unsaved edits in Pixel Lab"), "{saved}");
    // Another document's app, or a dead one, doesn't count.
    write_open_docs(
        &lab,
        std::process::id() as i64 + 100000,
        &lab.doc.base.join("other.pixellab"),
        json!([{ "doc": "object:pebble", "dirty": true }]),
    );
    assert_eq!(ok(call(&mut a, "doc.status", json!({})))["unsavedInApp"], json!(["scene:sample"]));
    std::fs::remove_file(lab.state.join("open").join(format!("{parent}.json"))).unwrap();
    assert_eq!(ok(call(&mut a, "doc.status", json!({})))["unsavedInApp"], json!([]));
}

// ---------------------------------------------------------------- merging a change on disk (R6, R8)

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

#[test]
fn r6_approving_a_proposal_merges_it_into_the_owners_dirty_copy_or_reports_the_clash() {
    let lab = Lab::new();
    let mut a = lab.agent();
    ok(call(&mut a, "scene.place", json!({ "name": "sample", "object": "pebble", "at": [1, 2, 0] })));
    let p1 = ok(call(&mut a, "doc.save", json!({ "doc": "scene:sample" })));
    let stale: Rc<RefCell<Vec<Value>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = stale.clone();
    let mut o = lab.session(Mode::Direct, None, Some(Box::new(move |x| sink.borrow_mut().push(x))));
    ok(call(&mut o, "scene.place", json!({ "name": "sample", "object": "pebble", "at": [6, 6, 0] })));
    let applied = ok(call(&mut o, "proposal.apply", json!({ "id": p1["proposal"] })));
    assert_eq!(
        json!([applied["wrote"], applied["reloaded"], applied["merged"], applied.get("conflict").cloned().unwrap_or(Value::Null)]),
        json!([true, false, true, null])
    );
    assert!(applied["summary"].as_str().unwrap().contains("merged into the open copy"), "{applied}");
    assert_eq!(
        ok(call(&mut o, "scene.items", json!({ "name": "sample" })))["items"].as_array().unwrap().len(),
        4,
        "the agent's pebble and the owner's"
    );
    assert_eq!(ok(call(&mut o, "doc.status", json!({ "doc": "scene:sample" })))["stale"], false);

    // A second proposal that clashes with the owner's edit: written, but the open copy stays and is stale.
    let mut b = lab.agent();
    ok(call(&mut b, "scene.move", json!({ "name": "sample", "item": "0", "by": [0, 1] })));
    let p2 = ok(call(&mut b, "doc.save", json!({ "doc": "scene:sample" })));
    ok(call(&mut o, "scene.move", json!({ "name": "sample", "item": "0", "by": [1, 0] })));
    let clash = ok(call(&mut o, "proposal.apply", json!({ "id": p2["proposal"] })));
    assert_eq!(json!([clash["wrote"], clash.get("merged").cloned().unwrap_or(Value::Null), clash["conflict"]]), json!([true, null, true]));
    assert_eq!(*stale.borrow(), vec![json!({ "doc": "scene:sample", "file": SCENE })]);
    assert_eq!(ok(call(&mut o, "doc.status", json!({ "doc": "scene:sample" })))["stale"], true);
}
