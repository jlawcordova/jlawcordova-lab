//! Depth order in editing (ports cli/test/depth-edit.test.mjs;
//! docs/intents/2026-10-depth-order/spec.md, R4 and R6–R8, R11, R13): every
//! scene edit leaves each list of items in depth order inside the same undo
//! step, results name the paths after the sort, merges and reloads sort what
//! they adopt, and the commands that came with depth (the override, a pixel
//! item's level, scene.sort, footprints) work through the CLI, serve --stdio
//! and MCP. On temp copies of the sample document with scenes of blocks that
//! overlap on purpose.
#![allow(clippy::result_large_err)]

mod bootstrap;
mod common;

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use engine::engine::{composite, is_in_depth_order, render_scene, CompositeOptions, Order};
use pixellab::cli::{main, Io};
use pixellab::context::Ctx;
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::Error;
use pixellab::registry::registry;
use pixellab::session::{Mode, Options, Session};
use serde_json::{json, Value};

// Four blocks side by side, listed back to front, and a pebble nothing overlaps.
fn stand_items() -> Vec<Value> {
    vec![
        json!({ "object": "block", "id": "a", "at": { "tile": [0, 0, 0] } }),
        json!({ "object": "block", "id": "b", "at": { "tile": [1, 0, 0] } }),
        json!({ "object": "block", "id": "c", "at": { "tile": [0, 1, 0] } }),
        json!({ "object": "block", "id": "d", "at": { "tile": [1, 1, 0] } }),
        json!({ "object": "pebble", "id": "far", "at": { "tile": [9, 9, 0] } }),
    ]
}

fn scene_of(items: Vec<Value>) -> Value {
    json!({ "viewBox": [-48, -32, 128, 96], "origin": [0, 0], "items": items })
}

fn stand() -> Value {
    scene_of(stand_items())
}

// A block in front of another that moves: its first key puts it somewhere else.
fn moving() -> Value {
    let mut s = scene_of(vec![
        json!({ "object": "block", "id": "a", "at": { "tile": [0, 0, 0] } }),
        json!({ "object": "block", "id": "cart", "at": { "tile": [0, 2, 0] } }),
    ]);
    s["motion"] = json!({ "duration": 1000, "tracks": [{ "item": "cart", "keys": [{ "t": 0 }, { "t": 500, "move": [16, 8] }] }] });
    s
}

// A group around a block that stands in front of the block beside it.
fn grouped() -> Value {
    json!({
        "viewBox": [-48, -32, 128, 96],
        "items": [
            { "group": { "class": "enter" }, "items": [{ "object": "block", "id": "x", "at": { "tile": [1, 1, 0] } }] },
            { "object": "block", "id": "y", "at": { "tile": [0, 0, 0] } },
        ],
    })
}

// STAND written out of order: d is listed first, so drawn first, under the blocks it stands in front of.
fn tangled_items() -> Vec<Value> {
    let mut items = stand_items();
    let d = items.remove(3);
    items.insert(0, d);
    items
}

fn tangled() -> Value {
    scene_of(tangled_items())
}

struct Lab {
    doc: common::TempDocument,
    state: PathBuf,
}

impl Lab {
    fn new() -> Lab {
        let doc = common::make_document();
        let state = common::temp_dir("pixellab-state-");
        let lab = Lab { doc, state };
        for (name, scene) in [("stand", stand()), ("moving", moving()), ("tangled", tangled()), ("grouped", grouped())] {
            lab.write_scene(name, &scene);
        }
        lab
    }

    fn write_scene(&self, name: &str, scene: &Value) {
        self.doc.write(&format!("scenes/{name}.json"), &engine::format::serialize_scene(scene));
    }

    fn env(&self) -> std::collections::HashMap<String, String> {
        let mut env = self.doc.env();
        env.insert("PIXELLAB_STATE".into(), self.state.to_string_lossy().into_owned());
        env
    }

    fn session(&self, mode: Mode, agent: Option<&str>) -> Session {
        let env = self.env();
        let document = open_document(&OpenOptions {
            doc: Some(self.doc.root.to_string_lossy().into_owned()),
            site: None,
            env: env.clone(),
            cwd: self.doc.root.clone(),
        })
        .unwrap();
        Session::new(document, Options { mode, env, agent: agent.map(String::from), ..Options::default() })
    }

    fn owner(&self) -> Session {
        self.session(Mode::Direct, None)
    }

    fn agent(&self) -> Session {
        self.session(Mode::Propose, Some("test"))
    }

    fn file(&self, name: &str) -> PathBuf {
        self.doc.root.join(format!("scenes/{name}.json"))
    }

    fn text(&self, name: &str) -> String {
        std::fs::read_to_string(self.file(name)).unwrap()
    }

    /// `pixellab <args> --document <doc>`: (exit code, stdout, stderr).
    fn cli(&self, args: &[&str]) -> (i32, String, String) {
        let reg = registry();
        let mut argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        argv.push("--document".into());
        argv.push(self.doc.root.to_string_lossy().into_owned());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = {
            let mut io = Io { stdout: &mut out, stderr: &mut err, env: self.env(), cwd: self.doc.root.clone(), registry: &reg };
            main(&argv, &mut io)
        };
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }
}

/// `registry.call(s.context(), name, params)`.
fn run(s: &mut Session, name: &str, params: Value) -> Result<Value, Error> {
    registry().call(&mut Ctx::new(s), name, params, false)
}

/// `registry.call(s.context(), name, { name: 'stand', ...params })`; a `null` param drops the key.
fn call(s: &mut Session, name: &str, params: Value) -> Result<Value, Error> {
    let mut p = json!({ "name": "stand" });
    for (k, v) in params.as_object().unwrap() {
        if v.is_null() {
            p.as_object_mut().unwrap().remove(k);
        } else {
            p[k] = v.clone();
        }
    }
    run(s, name, p)
}

fn ok(r: Result<Value, Error>) -> Value {
    r.unwrap_or_else(|e| panic!("{}", text_of(&e)))
}

fn text_of(e: &Error) -> String {
    match e {
        Error::Command(c) => c.message.clone(),
        Error::Internal(m) => m.clone(),
    }
}

fn code_of(e: &Error) -> String {
    match e {
        Error::Command(c) => c.code.clone(),
        Error::Internal(_) => "internal".into(),
    }
}

/// An error with this code whose message contains `message`.
fn rejects(r: Result<Value, Error>, code: &str, message: &str) {
    let e = r.expect_err("an error");
    assert_eq!(code_of(&e), code, "{}", text_of(&e));
    assert!(text_of(&e).contains(message), "{}", text_of(&e));
}

/// The scene's items back to front, by id (or object), as scene.items lists them.
fn order_of(s: &mut Session, name: &str) -> Vec<String> {
    let r = ok(call(s, "scene.items", json!({ "name": name })));
    r["items"].as_array().unwrap().iter().map(|i| i.get("id").unwrap_or(&i["object"]).as_str().unwrap().to_string()).collect()
}

fn order(s: &mut Session) -> Vec<String> {
    order_of(s, "stand")
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// Whether the working copy has every list in depth order, by the engine's own test.
fn in_order_of(s: &mut Session, name: &str) -> bool {
    let loaded = Ctx::new(s).sources().unwrap();
    is_in_depth_order(&loaded.sources, &loaded.sources.scenes[name]).unwrap()
}

fn in_order(s: &mut Session) -> bool {
    in_order_of(s, "stand")
}

fn items_of(s: &mut Session, name: &str) -> Vec<Value> {
    ok(call(s, "scene.items", json!({ "name": name })))["items"].as_array().unwrap().clone()
}

// ---------------------------------------------------------------- R4

#[test]
fn r4_placing_puts_the_item_where_it_stands_and_the_path_it_returns_is_after_the_sort() {
    let lab = Lab::new();
    let mut s = lab.owner();
    let r = ok(call(&mut s, "scene.place", json!({ "object": "block", "at": [-1, -1, 0], "id": "e" })));
    // e is behind a, b and c, which would wait for it; the sort moves e (one item) up in front of them and leaves "far" where it was.
    assert_eq!(order(&mut s), ids(&["e", "a", "b", "c", "d", "far"]));
    assert_eq!(r["path"], "0");
    assert_eq!(
        r["reordered"],
        json!({ "0": "1", "1": "2", "2": "3", "3": "4", "4": "5", "5": "0" }),
        "every path the sort changed, old -> new; e was added last"
    );
    assert_eq!(r["item"]["id"], "e");
    assert!(in_order(&mut s));
    // Nothing it overlaps: it stays last and nothing is reordered.
    let near = ok(call(&mut s, "scene.place", json!({ "object": "pebble", "at": [8, 9, 0] })));
    assert_eq!(near["path"], "6");
    assert!(near.get("reordered").is_none());
}

#[test]
fn r4_moving_by_command_re_sorts_the_result_names_the_new_path_and_what_moved() {
    let lab = Lab::new();
    let mut s = lab.owner();
    let r = ok(call(&mut s, "scene.move", json!({ "item": "d", "by": [-2, -2] })));
    assert_eq!(r["path"], "0");
    assert_eq!(order(&mut s), ids(&["d", "a", "b", "c", "far"]));
    assert_eq!(r["reordered"], json!({ "0": "1", "1": "2", "2": "3", "3": "0" }));
    assert_eq!(r["summary"], "block moved to column -1, row -1, level 0");
    // Moving it back puts it back in its place.
    let back = ok(call(&mut s, "scene.move", json!({ "item": "d", "by": [2, 2] })));
    assert_eq!(back["path"], "3");
    assert_eq!(order(&mut s), ids(&["a", "b", "c", "d", "far"]));
}

#[test]
fn r4_moving_by_a_layer_scene_layer_move_re_sorts_and_lists_the_moved_items_paths_after_the_sort() {
    let lab = Lab::new();
    let mut s = lab.owner();
    ok(call(&mut s, "scene.layer.new", json!({ "layerName": "Corner", "items": ["d"] })));
    let r = ok(call(&mut s, "scene.layer.move", json!({ "layer": "corner", "by": [-2, -2] })));
    assert_eq!(r["paths"], json!(["0"]));
    assert_eq!(r["reordered"], json!({ "0": "1", "1": "2", "2": "3", "3": "0" }));
    assert_eq!(order(&mut s), ids(&["d", "a", "b", "c", "far"]));
    assert_eq!(items_of(&mut s, "stand")[0]["layer"], "corner");
}

#[test]
fn r4_setting_a_field_a_tile_or_a_pixel_point_snapping_duplicating_and_removing_keep_the_order() {
    let lab = Lab::new();
    let mut s = lab.owner();
    assert_eq!(ok(call(&mut s, "scene.set", json!({ "item": "d", "col": -1, "row": -1 })))["path"], "0");
    assert_eq!(order(&mut s), ids(&["d", "a", "b", "c", "far"]));
    assert_eq!(ok(call(&mut s, "scene.set", json!({ "item": "d", "tile": [1, 1, 0] })))["path"], "3");
    let px = ok(call(&mut s, "scene.set", json!({ "item": "d", "px": [0, -16] })));
    assert_eq!(order(&mut s), ids(&["d", "a", "b", "c", "far"]), "a block placed by pixel at the point of tile -1,-1");
    assert_eq!(px["path"], "0");
    let snapped = ok(call(&mut s, "scene.snap", json!({ "item": "d" })));
    assert_eq!((snapped["path"].clone(), snapped["item"]["at"].clone()), (json!("0"), json!({ "tile": [-1, -1, 0] })));
    let twin = ok(call(&mut s, "scene.duplicate", json!({ "item": "d" })));
    let at: usize = twin["path"].as_str().unwrap().parse().unwrap();
    assert_eq!(items_of(&mut s, "stand")[at]["at"]["tile"], json!([0, -1, 0]));
    assert!(in_order(&mut s));
    ok(call(&mut s, "scene.remove", json!({ "item": "d" })));
    assert_eq!(order(&mut s), ids(&["block", "a", "b", "c", "far"]));
    assert!(in_order(&mut s));
}

#[test]
fn r4_setting_an_override_scene_set_depth_re_sorts_and_so_does_clearing_it() {
    let lab = Lab::new();
    let mut s = lab.owner();
    let front = ok(call(&mut s, "scene.set", json!({ "item": "a", "depth": "front" })));
    assert_eq!(front["path"], "4");
    assert_eq!(order(&mut s), ids(&["b", "c", "d", "far", "a"]));
    assert_eq!(items_of(&mut s, "stand").last().unwrap()["depth"], "front");
    // Back where it stands, it is behind b and c again: it alone moves back up, past them.
    let auto = ok(call(&mut s, "scene.set", json!({ "item": "a", "depth": "auto" })));
    assert_eq!(order(&mut s), ids(&["a", "b", "c", "d", "far"]));
    assert_eq!(auto["path"], "0");
    assert!(items_of(&mut s, "stand")[0].get("depth").is_none());
}

#[test]
fn r4_undo_and_redo_give_the_edit_and_its_sort_back_together_in_one_step() {
    let lab = Lab::new();
    let mut s = lab.owner();
    let doc = json!({ "doc": "scene:stand" });
    ok(call(&mut s, "scene.move", json!({ "item": "d", "by": [-2, -2] })));
    assert_eq!(order(&mut s), ids(&["d", "a", "b", "c", "far"]));
    let undone = ok(run(&mut s, "doc.undo", doc.clone()));
    assert_eq!(undone["dirty"], false, "one undo is the whole edit: back to the file");
    assert_eq!(order(&mut s), ids(&["a", "b", "c", "d", "far"]));
    ok(run(&mut s, "doc.redo", doc));
    assert_eq!(order(&mut s), ids(&["d", "a", "b", "c", "far"]));
    assert!(in_order(&mut s));
}

#[test]
fn r4_undo_of_a_sorting_edit_says_where_items_went_and_redo_says_it_back() {
    let lab = Lab::new();
    let mut s = lab.owner();
    let doc = json!({ "doc": "scene:stand" });
    ok(call(&mut s, "scene.move", json!({ "item": "d", "by": [-2, -2] })));
    // d went to the front of the list, so a, b and c are one later; undo puts d back at 3 and the rest a place earlier.
    let undone = ok(run(&mut s, "doc.undo", doc.clone()));
    assert_eq!(
        undone["reordered"],
        json!({ "0": "3", "1": "0", "2": "1", "3": "2" }),
        "old path -> new path, d included though its placement changed"
    );
    let redone = ok(run(&mut s, "doc.redo", doc.clone()));
    assert_eq!(redone["reordered"], json!({ "3": "0", "0": "1", "1": "2", "2": "3" }));
    // A step that moves no paths says nothing.
    ok(call(&mut s, "scene.set", json!({ "item": "a", "class": "x" })));
    assert!(ok(run(&mut s, "doc.undo", doc))["reordered"].is_null());
}

#[test]
fn r4_undo_of_a_sorting_edit_moves_a_group_with_its_first_item() {
    let lab = Lab::new();
    let mut s = lab.owner();
    // The group (x at 1,1) is listed before y (0,0), which it stands in front of: the first edit sorts the scene.
    let r = ok(call(&mut s, "scene.set", json!({ "name": "grouped", "item": "1", "class": "z" })));
    assert_eq!(r["reordered"], json!({ "0": "1", "0.0": "1.0", "1": "0" }));
    let undone = ok(run(&mut s, "doc.undo", json!({ "doc": "scene:grouped" })));
    assert_eq!(undone["reordered"], json!({ "1": "0", "1.0": "0.0", "0": "1" }), "the group and its member, old -> new");
}

#[test]
fn r4_a_motion_edit_that_moves_a_tracks_first_key_re_sorts_the_item_rests_somewhere_else() {
    let lab = Lab::new();
    let mut s = lab.owner();
    assert_eq!(order_of(&mut s, "moving"), ids(&["a", "cart"]));
    // The cart rests at its first key: moved up the screen to tile -1,-1 it is behind block a.
    let r = ok(call(&mut s, "scene.key.set", json!({ "name": "moving", "item": "cart", "time": 0, "move": [32, -32] })));
    assert_eq!(order_of(&mut s, "moving"), ids(&["cart", "a"]));
    assert!(in_order_of(&mut s, "moving"));
    assert_eq!(r["reordered"]["0"], "1");
    // Another key doesn't change where it rests.
    ok(call(&mut s, "scene.key.set", json!({ "name": "moving", "item": "cart", "time": 500, "move": [0, 0] })));
    assert_eq!(order_of(&mut s, "moving"), ids(&["cart", "a"]));
}

#[test]
fn r4_merging_a_change_from_disk_into_a_dirty_copy_leaves_it_sorted() {
    let lab = Lab::new();
    let mut s = lab.owner();
    // The owner moves d behind everything (unsaved); meanwhile the file gets a new block at the front, on disk.
    ok(call(&mut s, "scene.move", json!({ "item": "d", "by": [-2, -2] })));
    let mut items = stand_items();
    items.insert(4, json!({ "object": "block", "id": "e", "at": { "tile": [2, 2, 0] } }));
    lab.write_scene("stand", &scene_of(items));
    let merged = ok(run(&mut s, "doc.merge", json!({ "doc": "scene:stand" })));
    assert_eq!(merged["merged"], true);
    assert_eq!(order(&mut s), ids(&["d", "a", "b", "c", "e", "far"]));
    assert!(in_order(&mut s));
    assert_eq!(merged["dirty"], true, "the owner's edit is still unsaved");
}

#[test]
fn r4_a_merge_that_brings_an_out_of_order_list_together_comes_out_sorted_and_says_so() {
    let lab = Lab::new();
    let mut s = lab.owner();
    ok(call(&mut s, "scene.place", json!({ "object": "pebble", "at": [8, 9, 0], "id": "near" })));
    // The file, edited by hand and merged by git, now lists d before the blocks it stands in front of.
    lab.write_scene("stand", &tangled());
    let merged = ok(run(&mut s, "doc.merge", json!({ "doc": "scene:stand" })));
    assert_eq!(merged["merged"], true);
    assert_eq!(merged["sorted"], 1);
    assert!(merged["summary"].as_str().unwrap().contains("the result was sorted by depth"), "{}", merged["summary"]);
    assert_eq!(order(&mut s), ids(&["a", "b", "c", "d", "far", "near"]));
}

#[test]
fn r4_two_edits_of_a_sorted_scene_one_of_which_moved_an_item_still_merge_the_same_item_changed_both_ways_clashes() {
    let lab = Lab::new();
    let mut s = lab.owner();
    ok(call(&mut s, "scene.move", json!({ "item": "d", "by": [-2, -2] })));
    // On disk someone else gave block a a class: a different line from the ones the move touched.
    let mut items = stand_items();
    items[0] = json!({ "object": "block", "id": "a", "class": "sway", "at": { "tile": [0, 0, 0] } });
    lab.write_scene("stand", &scene_of(items));
    let merged = ok(run(&mut s, "doc.merge", json!({ "doc": "scene:stand" })));
    assert_eq!((merged["merged"].clone(), merged.get("sorted").cloned()), (json!(true), None), "nothing needed sorting again");
    assert_eq!(order(&mut s), ids(&["d", "a", "b", "c", "far"]));
    let a = items_of(&mut s, "stand").into_iter().find(|i| i["id"] == "a").unwrap();
    assert_eq!(a["class"], "sway");
    // Both sides changed d's own line: that is a clash, as before.
    let lab = Lab::new();
    let mut t = lab.owner();
    ok(call(&mut t, "scene.move", json!({ "item": "d", "by": [-2, -2] })));
    let mut items = stand_items();
    items[3] = json!({ "object": "block", "id": "d", "at": { "tile": [3, 3, 0] } });
    lab.write_scene("stand", &scene_of(items));
    let e = run(&mut t, "doc.merge", json!({ "doc": "scene:stand" })).unwrap_err();
    assert_eq!(code_of(&e), "merge_conflict", "{}", text_of(&e));
}

#[test]
fn r4_a_file_found_out_of_order_on_reload_is_sorted_as_a_step_of_its_own_unsaved_and_one_undo_gives_the_file_back() {
    let lab = Lab::new();
    let mut s = lab.owner();
    ok(run(&mut s, "doc.open", json!({ "doc": "scene:stand" })));
    lab.write_scene("stand", &tangled());
    let reloaded = ok(run(&mut s, "doc.reload", json!({ "doc": "scene:stand" })));
    assert_eq!(
        [reloaded["reloaded"].clone(), reloaded["sorted"].clone(), reloaded["dirty"].clone(), reloaded["canUndo"].clone()],
        [json!(true), json!(1), json!(true), json!(true)]
    );
    assert!(reloaded["summary"].as_str().unwrap().contains("wasn't in depth order, so stand was sorted"), "{}", reloaded["summary"]);
    assert_eq!(order(&mut s), ids(&["a", "b", "c", "d", "far"]));
    let status = ok(run(&mut s, "doc.status", json!({ "doc": "scene:stand" })));
    assert_eq!(status["edits"], json!(["Sorted stand by depth"]));
    let undone = ok(run(&mut s, "doc.undo", json!({ "doc": "scene:stand" })));
    assert_eq!(undone["dirty"], false);
    assert_eq!(order(&mut s), ids(&["d", "a", "b", "c", "far"]), "exactly the file");
}

#[test]
fn r4_a_sorted_file_reloads_as_it_is_with_no_extra_step() {
    let lab = Lab::new();
    let mut s = lab.owner();
    ok(run(&mut s, "doc.open", json!({ "doc": "scene:stand" })));
    let mut items = stand_items();
    items[4] = json!({ "object": "pebble", "id": "far", "at": { "tile": [8, 9, 0] } });
    lab.write_scene("stand", &scene_of(items));
    let reloaded = ok(run(&mut s, "doc.reload", json!({ "doc": "scene:stand" })));
    assert_eq!(
        [
            reloaded["reloaded"].clone(),
            reloaded.get("sorted").cloned().unwrap_or(Value::Null),
            reloaded["dirty"].clone(),
            reloaded["canUndo"].clone()
        ],
        [json!(true), Value::Null, json!(false), json!(false)]
    );
}

#[test]
fn r4_a_proposal_approved_into_a_copy_with_unsaved_edits_merges_and_leaves_it_sorted() {
    let lab = Lab::new();
    let mut a = lab.agent();
    let mut o = lab.owner();
    // The owner has an unsaved edit; the agent proposes a different one, and the owner approves it.
    ok(call(&mut o, "scene.move", json!({ "item": "c", "by": [0, -2] })));
    a.sync();
    ok(call(&mut a, "scene.place", json!({ "object": "block", "at": [-1, -1, 0], "id": "e" })));
    let proposed = ok(run(&mut a, "doc.save", json!({ "doc": "scene:stand" })));
    let applied = ok(run(&mut o, "proposal.apply", json!({ "id": proposed["proposal"] })));
    assert_eq!(applied["merged"], true);
    assert!(in_order(&mut o));
    let list = order(&mut o);
    let at = |id: &str| list.iter().position(|x| x == id).unwrap();
    assert!(at("e") < at("a"), "the proposed block stands behind a");
    let text = lab.text("stand");
    assert!(text.find("\"id\": \"e\"").unwrap() < text.find("\"id\": \"a\"").unwrap(), "and the file the proposal wrote lists it so");
    let doc: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(doc["items"].as_array().unwrap().len(), 6);
}

// ---------------------------------------------------------------- R6

#[test]
fn r6_scene_place_has_no_index_and_the_override_puts_an_item_over_or_under_everything_in_its_list() {
    let lab = Lab::new();
    let mut s = lab.owner();
    let e = call(&mut s, "scene.place", json!({ "object": "pebble", "at": [8, 9, 0], "index": 0 })).unwrap_err();
    assert!(text_of(&e).contains("unknown param index"), "{}", text_of(&e));
    // Back: d stands in front of a, b and c, but draws before all of them.
    ok(call(&mut s, "scene.set", json!({ "item": "d", "depth": "back" })));
    assert_eq!(order(&mut s)[0], "d");
    // Front on a, which stands behind b, c and d: it draws over them.
    let front = ok(call(&mut s, "scene.set", json!({ "item": "a", "depth": "front" })));
    assert_eq!(order(&mut s).last().unwrap(), "a");
    assert_eq!(front["item"]["depth"], "front");
    let items = items_of(&mut s, "stand");
    assert_eq!([items[0]["depth"].clone(), items.last().unwrap()["depth"].clone()], [json!("back"), json!("front")]);
}

#[test]
fn r6_the_override_takes_a_group_by_its_path_and_no_other_field_a_groups_attributes_are_scene_group_set() {
    let lab = Lab::new();
    let mut s = lab.owner();
    let mut g = |params: Value| {
        let mut p = params;
        p["name"] = json!("grouped");
        run(&mut s, "scene.set", p)
    };
    let r = ok(g(json!({ "item": "0", "depth": "back" })));
    assert_eq!(r["item"]["depth"], "back");
    assert_eq!(r["item"].as_object().unwrap().keys().collect::<Vec<_>>(), ["group", "depth", "items"]);
    assert_eq!(r["path"], "0", "the group stands in front of y, but it draws first");
    assert_eq!(r["item"]["items"][0]["id"], "x");
    rejects(
        g(json!({ "item": "0", "depth": "front", "class": "x" })),
        "usage",
        "is a group; only --depth applies to one (its attributes are scene.group.set)",
    );
    rejects(g(json!({ "item": "0", "col": 2 })), "usage", "scene.group.set");
    rejects(g(json!({ "item": "0", "id": "q" })), "usage", "");
    let e = g(json!({ "item": "0", "depth": "sideways" })).unwrap_err();
    assert!(text_of(&e).contains("depth"), "{}", text_of(&e));
    let items = ok(call(&mut s, "scene.items", json!({ "name": "grouped" })));
    let groups: Vec<Value> = items["groups"].as_array().unwrap().iter().map(|x| json!([x["path"], x["depth"]])).collect();
    assert_eq!(groups, [json!(["0", "back"])]);
}

#[test]
fn r6_a_groups_summary_names_it_by_its_path_after_the_sort_the_one_the_result_returns() {
    let lab = Lab::new();
    let mut s = lab.owner();
    let mut g = |params: Value| {
        let mut p = params;
        p["name"] = json!("grouped");
        run(&mut s, "scene.set", p)
    };
    ok(g(json!({ "item": "0", "depth": "back" })));
    // Clearing the override lets the group stand in front of y again, so the sort moves it from path 0 to path 1.
    let r = ok(g(json!({ "item": "0", "depth": "auto" })));
    assert_eq!(r["path"], "1");
    assert_eq!(r["reordered"]["0"], "1");
    assert_eq!(r["summary"], "Group 1 draws by where it stands");
    assert_eq!(ok(run(&mut s, "doc.undo", json!({ "doc": "scene:grouped" })))["summary"], "Undid: Group 1 draws by where it stands");
}

// ---------------------------------------------------------------- R7

#[test]
fn r7_scene_place_px_level_scene_set_level_and_scene_items_show_it_level_0_is_not_written() {
    let lab = Lab::new();
    let mut s = lab.owner();
    let placed = ok(call(&mut s, "scene.place", json!({ "object": "pebble", "px": [40, -12], "level": 4, "id": "jib" })));
    assert_eq!(placed["item"]["at"], json!({ "px": [40, -12], "level": 4 }));
    assert_eq!(placed["summary"], "pebble placed at x 40, y -12, level 4");
    let listed = items_of(&mut s, "stand").into_iter().find(|i| i["id"] == "jib").unwrap();
    assert_eq!([listed["level"].clone(), listed["at"].clone()], [json!(4), json!({ "px": [40, -12], "level": 4 })]);
    let set = ok(call(&mut s, "scene.set", json!({ "item": "jib", "level": 2 })));
    assert_eq!(set["item"]["at"], json!({ "px": [40, -12], "level": 2 }));
    assert_eq!(set["summary"], "pebble moved to x 40, y -12, level 2");
    let flat = ok(call(&mut s, "scene.set", json!({ "item": "jib", "level": 0 })));
    assert_eq!(flat["item"]["at"], json!({ "px": [40, -12] }));
    let tile = ok(call(&mut s, "scene.place", json!({ "object": "pebble", "at": [3, 3, 1] })));
    let found = items_of(&mut s, "stand").into_iter().find(|i| i["path"] == tile["path"]).unwrap();
    assert!(found.get("level").is_none(), "a tile item's level is its tile's third number");
    rejects(call(&mut s, "scene.place", json!({ "object": "pebble", "at": [3, 3, 1], "level": 2 })), "usage", "--level goes with --px");
}

#[test]
fn r7_scene_move_by_moves_a_pixel_items_level_with_it_nudge_and_px_keep_it_scene_snap_snaps_at_it() {
    let lab = Lab::new();
    let mut s = lab.owner();
    ok(call(&mut s, "scene.place", json!({ "object": "pebble", "px": [40, -12], "level": 4, "id": "jib" })));
    let up = ok(call(&mut s, "scene.move", json!({ "item": "jib", "by": [0, 0, 1] })));
    assert_eq!(up["item"]["at"], json!({ "px": [40, -28], "level": 5 }), "a level higher over the same tile: 16 pixels up");
    let down = ok(call(&mut s, "scene.move", json!({ "item": "jib", "by": [1, 0, -1] })));
    assert_eq!(down["item"]["at"], json!({ "px": [40 + 16, -28 + 8 + 16], "level": 4 }));
    let nudged = ok(call(&mut s, "scene.move", json!({ "item": "jib", "nudge": [1, 1] })));
    assert_eq!(nudged["item"]["at"], json!({ "px": [57, -3], "level": 4 }));
    let placed = ok(call(&mut s, "scene.set", json!({ "item": "jib", "px": [10, 10] })));
    assert_eq!(placed["item"]["at"], json!({ "px": [10, 10], "level": 4 }), "it still stands on level 4");
    let snapped = ok(call(&mut s, "scene.snap", json!({ "item": "jib" })));
    assert_eq!(snapped["item"]["at"]["tile"][2], 4, "snapped to the tile at its own level");
    assert!(snapped["summary"].as_str().unwrap().ends_with("level 4"));
    // A tile item nudged becomes a pixel item that keeps the level of its tile.
    let lifted = ok(call(&mut s, "scene.place", json!({ "object": "pebble", "at": [5, 5, 2], "id": "lifted" })));
    let nudge = ok(call(&mut s, "scene.move", json!({ "item": "lifted", "nudge": [1, 0] })));
    assert_eq!(nudge["item"]["at"]["level"], 2);
    assert_eq!(lifted["item"]["at"]["tile"][2], 2);
}

// ---------------------------------------------------------------- R8

/// The human text `object.show` prints for these params.
fn human(s: &mut Session, name: &str, params: Value) -> String {
    let r = ok(run(s, name, params.clone()));
    let reg = registry();
    let cmd = reg.get(name).unwrap();
    let checked = pixellab::params::check_params(&cmd.params, &params, false, Some(name)).unwrap();
    (cmd.format.unwrap())(&r, &checked)
}

#[test]
fn r8_object_footprint_sets_and_clears_it_after_the_anchor_in_the_file_blocks_refuse() {
    let lab = Lab::new();
    let mut s = lab.owner();
    let r = ok(run(&mut s, "object.footprint", json!({ "name": "pebble", "to": [2, 1] })));
    assert_eq!(r["footprint"], json!([2, 1]));
    assert_eq!(r["summary"], "Footprint of pebble set to 2\u{d7}1");
    ok(run(&mut s, "doc.save", json!({ "doc": "object:pebble" })));
    let text = std::fs::read_to_string(lab.doc.root.join("objects/pebble.json")).unwrap();
    let at = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("{needle} in {text}"));
    assert!(at("\"anchor\"") < at("\"footprint\": [2, 1]") && at("\"footprint\"") < at("\"keys\""), "after the anchor, before the keys");
    let shown = ok(run(&mut s, "object.show", json!({ "name": "pebble" })));
    assert_eq!(shown["footprint"], json!([2, 1]));
    assert!(human(&mut s, "object.show", json!({ "name": "pebble" })).lines().any(|l| l == "footprint 2\u{d7}1 tiles"));
    let cleared = ok(run(&mut s, "object.footprint", json!({ "name": "pebble", "clear": true })));
    assert!(cleared["footprint"].is_null());
    assert_eq!(ok(run(&mut s, "object.show", json!({ "name": "pebble" })))["footprint"], json!([1, 1]));
    rejects(run(&mut s, "object.footprint", json!({ "name": "block", "to": [2, 2] })), "usage", "its size is its footprint");
    let e = run(&mut s, "object.footprint", json!({ "name": "pebble" })).unwrap_err();
    assert!(text_of(&e).contains("give --to or --clear"), "{}", text_of(&e));
    rejects(run(&mut s, "object.footprint", json!({ "name": "pebble", "to": [0, 1] })), "usage", "");
    rejects(run(&mut s, "object.footprint", json!({ "name": "pebble", "to": [17, 1] })), "usage", "");
    let e = run(&mut s, "object.footprint", json!({ "name": "pebble", "to": [2, 1], "clear": true })).unwrap_err();
    assert!(text_of(&e).contains("only one of"), "{}", text_of(&e));
    assert!(ok(run(&mut s, "object.show", json!({ "name": "block" })))["footprint"].is_null(), "a block's size is its footprint");
}

#[test]
fn r8_object_new_footprint_gives_a_new_sprite_its_footprint_a_block_refuses_it() {
    let lab = Lab::new();
    let mut s = lab.owner();
    let made = ok(run(&mut s, "object.new", json!({ "name": "shed", "width": 12, "height": 8, "footprint": [2, 1] })));
    assert_eq!(made["changed"], true);
    assert_eq!(ok(run(&mut s, "object.show", json!({ "name": "shed" })))["footprint"], json!([2, 1]));
    rejects(run(&mut s, "object.new", json!({ "name": "slab", "kind": "block", "footprint": [2, 1] })), "usage", "size is its footprint");
    let extended = ok(run(&mut s, "object.new", json!({ "name": "big-shed", "extends": "shed", "footprint": [3, 2] })));
    assert_eq!(extended["changed"], true);
    assert_eq!(ok(run(&mut s, "object.show", json!({ "name": "big-shed" })))["footprint"], json!([3, 2]));
}

#[test]
fn r8_an_object_that_extends_a_sprite_has_its_footprint_and_a_footprint_changes_where_its_scene_sorts() {
    let lab = Lab::new();
    let mut s = lab.owner();
    ok(run(&mut s, "object.new", json!({ "name": "shed", "width": 12, "height": 8 })));
    ok(run(&mut s, "object.footprint", json!({ "name": "shed", "to": [4, 4] })));
    ok(run(&mut s, "object.new", json!({ "name": "shed-two", "extends": "shed" })));
    assert_eq!(ok(run(&mut s, "object.show", json!({ "name": "shed-two" })))["footprint"], json!([4, 4]));
}

// ---------------------------------------------------------------- R11

/// How many pixels differ at time 0 between a scene drawn in its listed order and in depth order, counted here from the engine.
fn pixels_that_change(s: &mut Session, name: &str) -> usize {
    let loaded = Ctx::new(s).sources().unwrap();
    let draw = |order: Order| {
        let rendered = render_scene(&loaded.sources, name, order).unwrap();
        composite(&rendered.root, CompositeOptions { first_variant: true, frame: 0, time: Some(0.0), play: None })
    };
    let (was, now) = (draw(Order::List), draw(Order::Depth));
    now.iter().filter(|(p, c)| was.get(*p) != Some(*c)).count() + was.keys().filter(|p| !now.has(*p)).count()
}

#[test]
fn r11_sorts_one_scene_as_one_undo_step_and_says_what_moved_and_how_many_pixels_change() {
    let lab = Lab::new();
    let mut s = lab.owner();
    let expected = pixels_that_change(&mut s, "tangled");
    assert!(expected > 0, "drawn in list order, d paints under the blocks it stands in front of");
    let r = ok(call(&mut s, "scene.sort", json!({ "name": "tangled" })));
    assert_eq!(r["moved"], json!([{ "scene": "tangled", "path": "0", "to": "3", "object": "block" }]));
    assert_eq!(r["changedPixels"], expected);
    assert_eq!(r["scenes"], json!([{ "scene": "tangled", "moved": 1, "changedPixels": expected }]));
    assert_eq!(r["reordered"], json!({ "0": "3", "1": "0", "2": "1", "3": "2" }));
    assert_eq!(r["changed"], true);
    let summary = r["summary"].as_str().unwrap();
    let rest = summary.strip_prefix("Sorted tangled by depth: 1 item moved, ").unwrap_or_else(|| panic!("{summary}"));
    assert!(
        rest.starts_with(|c: char| c.is_ascii_digit()) && (rest.ends_with(" pixel changes") || rest.ends_with(" pixels change")),
        "{summary}"
    );
    assert_eq!(order_of(&mut s, "tangled"), ids(&["a", "b", "c", "d", "far"]));
    assert!(in_order_of(&mut s, "tangled"));
    let undone = ok(run(&mut s, "doc.undo", json!({ "doc": "scene:tangled" })));
    assert_eq!(undone["dirty"], false, "one undo gives the file back");
    assert_eq!(order_of(&mut s, "tangled"), ids(&["d", "a", "b", "c", "far"]));
}

#[test]
fn r11_all_sorts_every_scene_a_sorted_scene_is_nothing_to_do() {
    let lab = Lab::new();
    let mut s = lab.owner();
    let all = ok(run(&mut s, "scene.sort", json!({ "all": true })));
    assert_eq!(all["changed"], true);
    // tangled is listed out of order; grouped is (its group stands in front of y but is listed first).
    let moved: Vec<&str> = all["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["moved"].as_i64().unwrap() > 0)
        .map(|x| x["scene"].as_str().unwrap())
        .collect();
    assert_eq!(moved, ["grouped", "tangled"]);
    let listed = ok(run(&mut s, "scene.list", json!({})));
    assert_eq!(all["scenes"].as_array().unwrap().len(), listed["scenes"].as_array().unwrap().len());
    let mut seen: Vec<&str> = Vec::new();
    for m in all["moved"].as_array().unwrap() {
        let scene = m["scene"].as_str().unwrap();
        if !seen.contains(&scene) {
            seen.push(scene);
        }
    }
    assert_eq!(seen, ["grouped", "tangled"]);
    assert!(all["reordered"].is_null(), "paths are for one scene");
    let again = ok(run(&mut s, "scene.sort", json!({ "all": true })));
    assert_eq!([again["changed"].clone(), again["moved"].clone(), again["changedPixels"].clone()], [json!(false), json!([]), json!(0)]);
    assert_eq!(again["summary"], "Already in depth order");
    let one = ok(call(&mut s, "scene.sort", json!({ "name": "stand" })));
    assert_eq!(
        [one["changed"].clone(), one["moved"].clone(), one["summary"].clone()],
        [json!(false), json!([]), json!("Already in depth order: stand")]
    );
    rejects(call(&mut s, "scene.sort", json!({ "name": null })), "usage", "give a scene");
    rejects(call(&mut s, "scene.sort", json!({ "all": true })), "usage", "not both");
}

#[test]
fn r11_the_clis_dry_run_shows_the_new_order_and_writes_nothing() {
    let lab = Lab::new();
    let before = lab.text("tangled");
    let (code, out, err) = lab.cli(&["scene", "sort", "tangled", "--dry-run"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.lines().any(|l| l == "-    { \"object\": \"block\", \"id\": \"d\", \"at\": { \"tile\": [1, 1, 0] } },"), "{out}");
    assert!(out.contains("dry run: nothing written (scenes/tangled.json would change)"), "{out}");
    assert!(out.lines().any(|l| l.starts_with("Sorted tangled by depth: 1 item moved, ")), "{out}");
    assert_eq!(lab.text("tangled"), before);
    let (code, _, err) = lab.cli(&["scene", "sort", "tangled"]);
    assert_eq!(code, 0, "{err}");
    let doc: Value = serde_json::from_str(&lab.text("tangled")).unwrap();
    let got: Vec<&str> = doc["items"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap()).collect();
    assert_eq!(got, ["a", "b", "c", "d", "far"]);
    assert!(lab.cli(&["scene", "sort", "tangled"]).1.starts_with("Already in depth order: tangled"));
}

// ---------------------------------------------------------------- R13

/// A server over stdin and stdout, one JSON-RPC message per line.
struct Server {
    child: Child,
    stdin: ChildStdin,
    out: BufReader<ChildStdout>,
    id: u64,
}

impl Server {
    fn start(lab: &Lab, mode: &str) -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pixellab"))
            .args(["serve", mode, "--document", lab.doc.root.to_str().unwrap()])
            .envs(lab.env())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let out = BufReader::new(child.stdout.take().unwrap());
        Server { child, stdin, out, id: 0 }
    }

    /// The whole response message.
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        writeln!(self.stdin, "{}", json!({ "jsonrpc": "2.0", "id": self.id, "method": method, "params": params })).unwrap();
        loop {
            let mut line = String::new();
            assert!(self.out.read_line(&mut line).unwrap() > 0, "the server closed");
            let msg: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line:?}"));
            // Notifications (no id) come between the answers.
            if msg.get("id") == Some(&json!(self.id)) {
                return msg;
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn r13_through_serve_stdio_what_the_app_uses() {
    let lab = Lab::new();
    let mut server = Server::start(&lab, "--stdio");
    let sorted = server.request("scene.sort", json!({ "name": "tangled" }))["result"].clone();
    assert_eq!((sorted["moved"].as_array().unwrap().len(), sorted["reordered"]["0"].clone()), (1, json!("3")));
    let moved = server.request("scene.move", json!({ "name": "stand", "item": "d", "by": [-2, -2] }))["result"].clone();
    assert_eq!((moved["path"].clone(), moved["reordered"]["3"].clone()), (json!("0"), json!("0")));
    let depth = server.request("scene.set", json!({ "name": "stand", "item": "a", "depth": "front" }))["result"].clone();
    assert_eq!(depth["path"], "4");
    let level = server.request("scene.place", json!({ "name": "stand", "object": "pebble", "px": [5, 5], "level": 3 }))["result"].clone();
    assert_eq!(level["item"]["at"], json!({ "px": [5, 5], "level": 3 }));
    let fp = server.request("object.footprint", json!({ "name": "pebble", "to": [2, 2] }))["result"].clone();
    assert_eq!(fp["footprint"], json!([2, 2]));
    let gone = server.request("scene.raise", json!({ "name": "stand", "item": "a" }));
    assert_eq!(gone["error"]["data"]["code"], "not_found");
}

#[test]
fn r13_through_mcp_the_tools_exist_raise_and_lower_do_not_and_an_edit_becomes_a_proposal() {
    let lab = Lab::new();
    let mut server = Server::start(&lab, "--mcp");
    let init = server.request(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test-agent", "version": "1" } }),
    )["result"]
        .clone();
    let instructions = init["instructions"].as_str().unwrap();
    assert!(instructions.contains("There is no raise or lower: scene_set depth"), "{instructions}");
    assert!(instructions.contains("scene_sort"));
    let tools = server.request("tools/list", json!({}))["result"]["tools"].clone();
    let names: Vec<&str> = tools.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for name in ["scene_sort", "object_footprint", "scene_set"] {
        assert!(names.contains(&name), "{name}");
    }
    for name in ["scene_raise", "scene_lower"] {
        assert!(!names.contains(&name), "{name}");
    }
    let mut tool = |name: &str, args: Value| server.request("tools/call", json!({ "name": name, "arguments": args }))["result"].clone();
    let sorted = tool("scene_sort", json!({ "name": "tangled" }));
    assert!(sorted.get("isError").is_none(), "{sorted}");
    assert_eq!(sorted["structuredContent"]["moved"].as_array().unwrap().len(), 1);
    let before = lab.text("tangled");
    let saved = tool("doc_save", json!({ "doc": "scene:tangled" }));
    assert!(saved.get("isError").is_none(), "{saved}");
    assert_eq!(lab.text("tangled"), before, "the document is untouched until the owner approves");
    let file = lab.state.join("proposals").join(format!("{}.json", saved["structuredContent"]["proposal"].as_str().unwrap()));
    let p: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    assert_eq!(p["summary"], json!(["Sorted tangled by depth: 1 item moved"]));
    let source = p["source"].as_str().unwrap();
    let d_line = source.find("\"id\": \"d\"").unwrap();
    let after = &source[d_line..];
    let next = &after[after.find('\n').unwrap() + 1..];
    assert!(next.starts_with("    { \"object\": \"pebble\""), "the proposal lists d after c, before the pebble: {source}");
    let footprint = tool("object_footprint", json!({ "name": "pebble", "to": [2, 1] }));
    assert_eq!(footprint["structuredContent"]["footprint"], json!([2, 1]));
    let depth = tool("scene_set", json!({ "name": "stand", "item": "a", "depth": "back" }));
    assert_eq!(depth["structuredContent"]["item"]["depth"], "back");
}

// ---------------------------------------------------------------- the harbor example

/// A session over a format-2 temp copy of an example.
fn example_session(prefix: &str, name: &str) -> (PathBuf, Session) {
    let tmp = common::temp_dir(prefix);
    let root = bootstrap::example(&tmp, name);
    let env = std::collections::HashMap::from([
        ("HOME".to_string(), tmp.to_string_lossy().into_owned()),
        ("PIXELLAB_STATE".to_string(), tmp.join("state").to_string_lossy().into_owned()),
    ]);
    let document =
        open_document(&OpenOptions { doc: Some(root.to_string_lossy().into_owned()), site: None, env: env.clone(), cwd: root }).unwrap();
    (tmp, Session::new(document, Options { env, ..Options::default() }))
}

fn unbase64(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let mut out = Vec::new();
    for chunk in text.as_bytes().chunks(4) {
        let n: Vec<u32> = chunk.iter().filter(|&&c| c != b'=').map(|&c| value(c) as u32).collect();
        let bits = n.iter().fold(0u32, |a, &v| a << 6 | v) << (6 * (4 - n.len()));
        for i in 0..n.len() - 1 {
            out.push((bits >> (16 - 8 * i)) as u8);
        }
    }
    out
}

#[test]
fn r15_moved_5_5_to_tile_0_1_0_it_draws_behind_the_red_house_moved_back_it_draws_in_front_again_as_it_did() {
    let (tmp, mut s) = example_session("pixellab-harbor-", "harbor");
    let sources = Ctx::new(&mut s).sources().unwrap().sources.clone();
    let h = |s: &mut Session, name: &str, params: Value| {
        let mut p = params;
        p["name"] = json!("harbor-flat");
        ok(run(s, name, p))
    };
    // The color name at a scene pixel of the PNG scene.render makes, by palette lookup.
    let color_at = |s: &mut Session, x: i64, y: i64| -> Option<String> {
        let view = h(s, "scene.show", json!({}))["viewBox"].clone();
        let png = engine::png::decode(&unbase64(h(s, "scene.render", json!({}))["png"].as_str().unwrap())).unwrap();
        let (px, py) = ((x - view[0].as_i64().unwrap()) as usize, (y - view[1].as_i64().unwrap()) as usize);
        let at = (py * png.width as usize + px) * 4;
        let hex = format!("#{:02X}{:02X}{:02X}", png.rgba[at], png.rgba[at + 1], png.rgba[at + 2]);
        sources.colors.iter().find(|(_, c)| c.hex.to_uppercase() == hex).map(|(name, _)| name.clone())
    };
    h(&mut s, "scene.sort", json!({}));
    let find_pine = |items: &Value, tile: &str| -> Value {
        items["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| {
                i["object"] == "pine"
                    && i["at"]["tile"].as_array().map(|t| t.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(","))
                        == Some(tile.to_string())
            })
            .unwrap()
            .clone()
    };
    let items = h(&mut s, "scene.items", json!({}));
    let start = find_pine(&items, "5,6,0");
    // Standing alone in front of the island, its trunk is the front-most thing at its foot (x -16, y 88).
    assert_eq!(color_at(&mut s, -16, 88).as_deref(), Some("wood-3"));
    let start_picture = h(&mut s, "scene.render", json!({}))["png"].clone();
    let out = h(&mut s, "scene.move", json!({ "item": start["path"], "by": [-5, -5] }));
    assert_eq!(out["item"]["at"], json!({ "tile": [0, 1, 0] }));
    // The pine's crown at x -14, y -6 is behind the red house's roof, so that pixel shows the roof.
    assert_eq!(color_at(&mut s, -14, -6).as_deref(), Some("roof-2"), "the red house is in front of the pine");
    assert_eq!(color_at(&mut s, -15, -8).as_deref(), Some("roof-4"));
    // Marking the pine's wrapper to draw in front puts the pine over the house at the same pixel, so that pixel was the house hiding it.
    let wrapper = out["path"].as_str().unwrap().split('.').next().unwrap().to_string();
    let fronted = h(&mut s, "scene.set", json!({ "item": wrapper, "depth": "front" }));
    assert_eq!(color_at(&mut s, -14, -6).as_deref(), Some("grass-4"), "the pine's own color");
    // The wrapper now draws last, so its path is the one the result gives, not the old one.
    h(&mut s, "scene.set", json!({ "item": fronted["path"], "depth": "auto" }));
    assert_eq!(color_at(&mut s, -14, -6).as_deref(), Some("roof-2"));
    // Moving it back: the scene draws as it did, and the pine is in front again.
    let items = h(&mut s, "scene.items", json!({}));
    let back = h(&mut s, "scene.move", json!({ "item": find_pine(&items, "0,1,0")["path"], "by": [5, 5] }));
    assert_eq!(back["item"]["at"], json!({ "tile": [5, 6, 0] }));
    assert_eq!(h(&mut s, "scene.render", json!({}))["png"], start_picture, "out and back draws the scene as it was, pixel for pixel");
    assert_eq!(color_at(&mut s, -16, 88).as_deref(), Some("wood-3"));
    let _ = std::fs::remove_dir_all(tmp);
}

#[test]
fn r8_object_footprint_tree_to_3_3_makes_check_report_out_of_order_and_scene_sort_then_moves_the_items() {
    let (tmp, mut s) = example_session("pixellab-harbor-footprint-", "harbor");
    let out_of_order = |s: &mut Session| -> Vec<Value> {
        ok(run(s, "check", json!({})))["warnings"].as_array().unwrap().iter().filter(|w| w["code"] == "out_of_order").cloned().collect()
    };
    assert!(out_of_order(&mut s).is_empty(), "the example is in depth order as it stands");
    ok(run(&mut s, "object.footprint", json!({ "name": "tree", "to": [3, 3] })));
    let warned = out_of_order(&mut s);
    let mut scenes: Vec<&str> = warned.iter().map(|w| w["scene"].as_str().unwrap()).collect();
    scenes.sort();
    assert_eq!(scenes, ["harbor", "harbor-flat"]);
    assert!(warned.iter().all(|w| w["moves"].as_i64().unwrap() > 0), "each scene says how many items would move");
    let sorted = ok(run(&mut s, "scene.sort", json!({ "all": true })));
    assert_eq!(sorted["changed"], true);
    let mut names: Vec<&str> = sorted["scenes"].as_array().unwrap().iter().map(|x| x["scene"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["harbor", "harbor-flat"]);
    assert!(sorted["scenes"].as_array().unwrap().iter().all(|x| x["moved"].as_i64().unwrap() > 0), "the sort moves items in both scenes");
    assert!(out_of_order(&mut s).is_empty(), "sorted again, check is quiet");
    let _ = std::fs::remove_dir_all(tmp);
}
