//! The program side of docs/intents/2026-10-scene-canvas/spec.md: a scene without a size
//! (R1 to R12) and placing on top of a block (R16, R17, R19), through the registry on a
//! session over a temp copy of the sample document, and through the CLI door.

#![allow(clippy::result_large_err)]

mod common;

use pixellab::cli::{main, Io};
use pixellab::context::{Ctx, DocKind};
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::{CommandError, Error};
use pixellab::registry::registry;
use pixellab::session::{Mode, Options, Session};
use serde_json::{json, Value};

/// A session over a temp copy of the sample document (objects `block` and `pebble`).
struct Lab {
    session: Session,
    doc: common::TempDocument,
    state: std::path::PathBuf,
}

fn lab_with(scenes: &[(&str, Value)], mode: Mode) -> Lab {
    let doc = common::make_document();
    for (name, scene) in scenes {
        doc.write(&format!("scenes/{name}.json"), &engine::format::stringify_pretty(scene));
    }
    let state = common::temp_dir("pixellab-state-");
    let mut env = doc.env();
    env.insert("PIXELLAB_STATE".into(), state.to_string_lossy().into_owned());
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    Lab { session: Session::new(document, Options { mode, env, ..Options::default() }), doc, state }
}

fn lab(scenes: &[(&str, Value)]) -> Lab {
    lab_with(scenes, Mode::Direct)
}

impl Lab {
    fn call(&mut self, name: &str, params: Value) -> Result<Value, CommandError> {
        registry().call(&mut Ctx::new(&mut self.session), name, params, false).map_err(|e| match e {
            Error::Command(c) => c,
            Error::Internal(m) => CommandError::new(m, "internal"),
        })
    }

    fn ok(&mut self, name: &str, params: Value) -> Value {
        self.call(name, params).unwrap_or_else(|e| panic!("{name}: {}", e.message))
    }

    fn scene(&mut self, name: &str) -> Value {
        Ctx::new(&mut self.session).doc(DocKind::Scene, name).unwrap()
    }

    fn cli(&self, args: &[&str]) -> (i32, String, String) {
        let reg = registry();
        let mut argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        argv.push("--document".into());
        argv.push(self.doc.root.to_string_lossy().into_owned());
        let mut env = self.doc.env();
        env.insert("PIXELLAB_STATE".into(), self.state.to_string_lossy().into_owned());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = {
            let mut io = Io { stdout: &mut out, stderr: &mut err, env, cwd: self.doc.root.clone(), registry: &reg };
            main(&argv, &mut io)
        };
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }
}

fn block_item(col: i64, row: i64, level: i64) -> Value {
    json!({ "object": "block", "at": { "tile": [col, row, level] } })
}

fn as_box(v: &Value) -> Vec<i64> {
    v.as_array().unwrap().iter().map(|n| n.as_i64().unwrap()).collect()
}

// ------------------------------------------------------------------ R7

#[test]
fn r7_scene_render_list_and_show_report_the_resolved_box() {
    let mut lab = lab(&[
        ("fit", json!({ "origin": [0, 0], "items": [block_item(0, 0, 0), block_item(4, 0, 0)] })),
        ("fixed", json!({ "viewBox": [-10, -10, 50, 40], "items": [block_item(0, 0, 0)] })),
    ]);
    let sources = Ctx::new(&mut lab.session).sources().unwrap();
    let want = engine::engine::resolve_frame(&sources.sources, "fit").unwrap();
    assert!(!want.fixed);

    let render = lab.ok("scene.render", json!({ "name": "fit" }));
    assert_eq!(as_box(&render["viewBox"]), want.view_box.to_vec());
    assert_eq!(render["frame"], "fit");
    assert_eq!((render["width"].as_i64().unwrap(), render["height"].as_i64().unwrap()), (want.view_box[2], want.view_box[3]));

    let render = lab.ok("scene.render", json!({ "name": "fixed", "scale": 2 }));
    assert_eq!(as_box(&render["viewBox"]), vec![-10, -10, 50, 40]);
    assert_eq!(render["frame"], "fixed");
    assert_eq!((render["width"].as_i64().unwrap(), render["height"].as_i64().unwrap()), (100, 80));

    let show = lab.ok("scene.show", json!({ "name": "fit" }));
    assert_eq!(as_box(&show["viewBox"]), want.view_box.to_vec());
    assert_eq!(show["frame"], "fit");
    let show = lab.ok("scene.show", json!({ "name": "fixed" }));
    assert_eq!(show["frame"], "fixed");

    let list = lab.ok("scene.list", json!({}));
    let find = |name: &str| list["scenes"].as_array().unwrap().iter().find(|s| s["name"] == name).unwrap().clone();
    assert_eq!(as_box(&find("fit")["viewBox"]), want.view_box.to_vec());
    assert_eq!(find("fit")["frame"], "fit");
    assert_eq!(find("fixed")["frame"], "fixed");

    let (code, human, err) = lab.cli(&["scene", "list"]);
    assert_eq!(code, 0, "{err}");
    let size = format!("{}×{}", want.view_box[2], want.view_box[3]);
    assert!(human.lines().any(|l| l.starts_with("fit ") && l.contains(&format!("fits {size}"))), "{human}");
    assert!(human.lines().any(|l| l.starts_with("fixed ") && l.contains("fixed 50×40")), "{human}");
    let (_, human, _) = lab.cli(&["scene", "show", "fit"]);
    assert!(human.lines().any(|l| l == format!("size      fits {size}")), "{human}");
}

#[test]
fn r7_the_render_view_is_a_window_of_the_scene() {
    let mut lab = lab(&[("fit", json!({ "items": [block_item(0, 0, 0)] }))]);
    let whole = lab.ok("scene.render", json!({ "name": "fit" }));
    let [x, y, w, h] = as_box(&whole["viewBox"])[..] else { panic!() };
    // A window bigger than the scene on every side: the art sits inside it at the right offset.
    let window = lab.ok("scene.render", json!({ "name": "fit", "view": [x - 20, y - 10, w + 40, h + 30] }));
    assert_eq!(as_box(&window["viewBox"]), vec![x - 20, y - 10, w + 40, h + 30]);
    assert_eq!((window["width"].as_i64().unwrap(), window["height"].as_i64().unwrap()), (w + 40, h + 30));
    assert_eq!(window["frame"], "fit");
    let bounds_whole = as_box(&whole["items"][0]["bounds"]);
    let bounds_window = as_box(&window["items"][0]["bounds"]);
    assert_eq!(bounds_window, vec![bounds_whole[0] + 20, bounds_whole[1] + 10, bounds_whole[2], bounds_whole[3]]);
    // A window can be empty space.
    let far = lab.ok("scene.render", json!({ "name": "fit", "view": [5000, 5000, 64, 32] }));
    assert_eq!((far["width"].as_i64().unwrap(), far["height"].as_i64().unwrap()), (64, 32));
    // A bad window is a usage error.
    assert_eq!(lab.call("scene.render", json!({ "name": "fit", "view": [0, 0, 0, 5] })).unwrap_err().exit_code, 2);
    assert_eq!(lab.call("scene.render", json!({ "name": "fit", "view": [0, 0, 100000, 100000] })).unwrap_err().exit_code, 2);
}

#[test]
fn r7_place_with_no_position_starts_at_the_middle_of_the_content_or_the_origin_tile() {
    let mut lab = lab(&[("empty", json!({ "items": [] })), ("full", json!({ "items": [block_item(6, 2, 0)] }))]);
    lab.ok("scene.place", json!({ "name": "empty", "object": "pebble" }));
    assert_eq!(lab.scene("empty")["items"][0]["at"], json!({ "tile": [0, 0, 0] }));
    lab.ok("scene.place", json!({ "name": "full", "object": "pebble" }));
    let placed = lab.scene("full")["items"].as_array().unwrap().iter().find(|i| i["object"] == "pebble").unwrap().clone();
    // The middle of the block at 6,2 is that block's own tile.
    let tile = as_box(&placed["at"]["tile"]);
    assert!((tile[0] - 6).abs() <= 1 && (tile[1] - 2).abs() <= 1 && tile[2] >= 0, "{tile:?}");
}

// ------------------------------------------------------------------ R8

#[test]
fn r8_scene_frame_fixes_the_size_and_takes_it_off_again() {
    let mut lab = lab(&[("fit", json!({ "items": [block_item(0, 0, 0)] }))]);
    let before = lab.ok("scene.frame", json!({ "name": "fit" }));
    assert_eq!(before["frame"], "fit");
    assert_eq!(before["changed"], false);
    assert_eq!(before["margin"], 0);
    let size = before["size"].clone();
    assert!(lab.scene("fit").get("viewBox").is_none(), "reporting changes nothing");

    let fixed = lab.ok("scene.frame", json!({ "name": "fit", "box": [-20, -10, 100, 60] }));
    assert_eq!(
        (fixed["frame"].clone(), fixed["viewBox"].clone(), fixed["size"].clone()),
        (json!("fixed"), json!([-20, -10, 100, 60]), json!([100, 60]))
    );
    assert_eq!(lab.scene("fit")["viewBox"], json!([-20, -10, 100, 60]));
    assert_eq!(lab.ok("scene.render", json!({ "name": "fit" }))["frame"], "fixed");
    assert_eq!(lab.ok("scene.frame", json!({ "name": "fit" }))["frame"], "fixed");

    let fitted = lab.ok("scene.frame", json!({ "name": "fit", "fit": true }));
    assert_eq!((fitted["frame"].clone(), fitted["size"].clone()), (json!("fit"), size));
    assert!(lab.scene("fit").get("viewBox").is_none());

    // The margin, and taking it off with 0.
    let padded = lab.ok("scene.frame", json!({ "name": "fit", "margin": 3 }));
    assert_eq!(padded["margin"], 3);
    assert_eq!(padded["size"], json!([before["size"][0].as_i64().unwrap() + 6, before["size"][1].as_i64().unwrap() + 6]));
    assert_eq!(lab.scene("fit")["margin"], 3);
    lab.ok("scene.frame", json!({ "name": "fit", "margin": 0 }));
    assert!(lab.scene("fit").get("margin").is_none());

    // One undo step each.
    lab.ok("scene.frame", json!({ "name": "fit", "box": [0, 0, 8, 8] }));
    lab.ok("doc.undo", json!({ "doc": "scene:fit" }));
    assert!(lab.scene("fit").get("viewBox").is_none());
}

#[test]
fn r8_scene_frame_refuses_a_bad_box_and_both_options_at_once() {
    let mut lab = lab(&[("fit", json!({ "items": [] }))]);
    assert_eq!(lab.call("scene.frame", json!({ "name": "fit", "box": [0, 0, 0, 5] })).unwrap_err().exit_code, 2);
    assert_eq!(lab.call("scene.frame", json!({ "name": "fit", "box": [0, 0, 5, 5], "fit": true })).unwrap_err().exit_code, 2);
    assert_eq!(lab.call("scene.frame", json!({ "name": "fit", "box": [0, 0, 5] })).unwrap_err().exit_code, 2);
    assert_eq!(lab.call("scene.frame", json!({ "name": "fit", "margin": -1 })).unwrap_err().exit_code, 2);
    assert_eq!(lab.call("scene.frame", json!({ "name": "nope" })).unwrap_err().exit_code, 1);
}

#[test]
fn r8_scene_frame_over_the_cli() {
    let lab = lab(&[("fit", json!({ "items": [block_item(0, 0, 0)] }))]);
    let (code, out, err) = lab.cli(&["scene", "frame", "fit", "--box", "-16,-16,64,48"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("fixed 64×48"), "{out}");
    let (code, out, _) = lab.cli(&["scene", "frame", "fit"]);
    assert_eq!(code, 0);
    assert!(out.contains("fixed 64×48"), "{out}");
    let (code, out, err) = lab.cli(&["scene", "frame", "fit", "--fit", "--margin", "2"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("fits its content") && out.contains("margin 2"), "{out}");
    let text = std::fs::read_to_string(lab.doc.root.join("scenes/fit.json")).unwrap();
    assert!(!text.contains("viewBox") && text.contains("\"margin\": 2"), "{text}");
}

#[test]
fn r8_scene_frame_as_a_proposal() {
    let mut lab = lab_with(&[("fit", json!({ "items": [block_item(0, 0, 0)] }))], Mode::Propose);
    let before = std::fs::read_to_string(lab.doc.root.join("scenes/fit.json")).unwrap();
    lab.ok("scene.frame", json!({ "name": "fit", "box": [-16, -16, 64, 48] }));
    let saved = lab.ok("doc.save", json!({ "doc": "scene:fit" }));
    assert_eq!(saved["wrote"], false);
    assert_eq!(std::fs::read_to_string(lab.doc.root.join("scenes/fit.json")).unwrap(), before, "the document is untouched");
    let proposal = lab.session.state.read_proposal(saved["proposal"].as_str().unwrap()).unwrap();
    let source: Value = serde_json::from_str(proposal["source"].as_str().unwrap()).unwrap();
    assert_eq!(source["viewBox"], json!([-16, -16, 64, 48]));
    assert_eq!(proposal["summary"].as_array().unwrap().len(), 1);
}

// ------------------------------------------------------------ R16, R17

fn level_of(item: &Value) -> i64 {
    item["at"]["tile"][2].as_i64().unwrap()
}

fn find<'a>(items: &'a [Value], object: &str, col: i64, row: i64) -> &'a Value {
    items.iter().find(|i| i["object"] == object && i["at"]["tile"][0] == col && i["at"]["tile"][1] == row).unwrap()
}

#[test]
fn r16_drop_lands_on_the_top_of_the_block_under_it() {
    let mut lab = lab(&[(
        "stack",
        json!({ "items": [
            block_item(2, 2, 0), block_item(2, 2, 1),
            { "object": "pebble", "id": "high", "at": { "tile": [2, 2, 5] } },
            { "object": "pebble", "id": "lost", "at": { "tile": [8, 8, 4] } },
            { "object": "block", "id": "floater", "at": { "tile": [4, 4, 3] } },
            block_item(4, 4, 0),
            { "object": "pebble", "id": "rests", "at": { "tile": [6, 6, 0] } },
            block_item(6, 6, 0),
        ] }),
    )]);
    // Over two stacked blocks: onto the top one (level 1).
    let r = lab.ok("scene.drop", json!({ "name": "stack", "item": "high" }));
    assert!(r["summary"].as_str().unwrap().contains("dropped to level 1, on top of block"), "{r}");
    let items = lab.scene("stack")["items"].as_array().unwrap().clone();
    assert_eq!(level_of(items.iter().find(|i| i["id"] == "high").unwrap()), 1);
    // Over nothing: the ground.
    lab.ok("scene.drop", json!({ "name": "stack", "item": "lost" }));
    let items = lab.scene("stack")["items"].as_array().unwrap().clone();
    assert_eq!(level_of(items.iter().find(|i| i["id"] == "lost").unwrap()), 0);
    // A block rests with its bottom on the top of the block under it: level 1, not 0.
    lab.ok("scene.drop", json!({ "name": "stack", "item": "floater" }));
    let items = lab.scene("stack")["items"].as_array().unwrap().clone();
    assert_eq!(level_of(items.iter().find(|i| i["id"] == "floater").unwrap()), 1);
    // Already resting: it stays, and nothing else moves.
    let before = lab.scene("stack");
    let r = lab.ok("scene.drop", json!({ "name": "stack", "item": "rests" }));
    assert!(r["summary"].as_str().unwrap().contains("already rests at level 0"), "{r}");
    assert_eq!(lab.scene("stack"), before);
    // A group is not an item to drop; an unknown item is not found.
    assert_eq!(lab.call("scene.drop", json!({ "name": "stack", "item": "nope" })).unwrap_err().exit_code, 1);
}

#[test]
fn r16_drop_is_one_undo_step_and_ignores_what_is_above_and_the_item_itself() {
    let mut lab = lab(&[(
        "tower",
        json!({ "items": [
            block_item(1, 1, 0),
            { "object": "block", "id": "mid", "at": { "tile": [1, 1, 1] } },
            { "object": "pebble", "id": "top", "at": { "tile": [1, 1, 2] } },
        ] }),
    )]);
    // The block in the middle, with a pebble above it, stays resting on the block below.
    lab.ok("scene.drop", json!({ "name": "tower", "item": "mid" }));
    let items = lab.scene("tower")["items"].as_array().unwrap().clone();
    assert_eq!(level_of(items.iter().find(|i| i["id"] == "mid").unwrap()), 1);
    // The pebble rests on it (level 1 is the top of the middle block); at 2 it floats one level, so it drops to 1.
    lab.ok("scene.drop", json!({ "name": "tower", "item": "top" }));
    let items = lab.scene("tower")["items"].as_array().unwrap().clone();
    assert_eq!(level_of(items.iter().find(|i| i["id"] == "top").unwrap()), 1);
    lab.ok("doc.undo", json!({ "doc": "scene:tower" }));
    let items = lab.scene("tower")["items"].as_array().unwrap().clone();
    assert_eq!(level_of(items.iter().find(|i| i["id"] == "top").unwrap()), 2, "one undo step");
}

#[test]
fn r17_placing_on_a_block_puts_the_item_on_top_of_it() {
    let mut lab = lab(&[("build", json!({ "items": [block_item(3, 1, 0)] }))]);
    // A tile as col,row has no level of its own: on the block, level 0 is its top.
    lab.ok("scene.place", json!({ "name": "build", "object": "pebble", "at": [3, 1] }));
    let items = lab.scene("build")["items"].as_array().unwrap().clone();
    assert_eq!(level_of(find(&items, "pebble", 3, 1)), 0);
    // A block on a block rests its bottom on the top: level 1.
    let r = lab.ok("scene.place", json!({ "name": "build", "object": "block", "at": [3, 1] }));
    assert!(r["summary"].as_str().unwrap().ends_with("on top of block"), "{r}");
    let items = lab.scene("build")["items"].as_array().unwrap().clone();
    assert_eq!(items.iter().filter(|i| i["object"] == "block").map(level_of).max(), Some(1));
    // And another: level 2.
    lab.ok("scene.place", json!({ "name": "build", "object": "block", "at": [3, 1] }));
    let items = lab.scene("build")["items"].as_array().unwrap().clone();
    assert_eq!(items.iter().filter(|i| i["object"] == "block").map(level_of).max(), Some(2));
    // Bare ground is level 0, for a block too, and a sprite is not a floor.
    lab.ok("scene.place", json!({ "name": "build", "object": "block", "at": [9, 9] }));
    lab.ok("scene.place", json!({ "name": "build", "object": "pebble", "at": [3, 8] }));
    lab.ok("scene.place", json!({ "name": "build", "object": "pebble", "at": [3, 8] }));
    let items = lab.scene("build")["items"].as_array().unwrap().clone();
    assert_eq!(level_of(find(&items, "block", 9, 9)), 0);
    assert!(items.iter().filter(|i| i["at"]["tile"][0] == 3 && i["at"]["tile"][1] == 8).all(|i| level_of(i) == 0));
}

#[test]
fn r17_a_level_given_wins_and_on_top_overrides_it() {
    let mut lab = lab(&[("build", json!({ "items": [block_item(3, 1, 0)] }))]);
    lab.ok("scene.place", json!({ "name": "build", "object": "pebble", "at": [3, 1, 0], "id": "given" }));
    lab.ok("scene.place", json!({ "name": "build", "object": "pebble", "at": [3, 1, 7], "id": "high" }));
    lab.ok("scene.place", json!({ "name": "build", "object": "pebble", "at": [3, 1, 7], "id": "pinned", "onTop": true }));
    let items = lab.scene("build")["items"].as_array().unwrap().clone();
    let level = |id: &str| level_of(items.iter().find(|i| i["id"] == id).unwrap());
    assert_eq!((level("given"), level("high"), level("pinned")), (0, 7, 0));
    // A pixel point: no level, as before; --on-top stacks on the block under its tile.
    lab.ok("scene.place", json!({ "name": "build", "object": "block", "px": [3, 1] }));
    lab.ok("scene.place", json!({ "name": "build", "object": "pebble", "px": [0, 0], "onTop": true, "id": "px" }));
    let items = lab.scene("build")["items"].as_array().unwrap().clone();
    let px = items.iter().find(|i| i["id"] == "px").unwrap();
    assert!(px["at"].get("level").is_none() || px["at"]["level"].as_i64().unwrap() >= 0, "{px}");
    // With no position at all the item stacks where the cursor starts.
    let middle = lab.ok("scene.place", json!({ "name": "build", "object": "block" }));
    assert!(middle["item"]["at"]["tile"][2].as_i64().unwrap() >= 0);
    // The CLI takes --at col,row and --on-top.
    let (code, out, err) = lab.cli(&["scene", "place", "build", "block", "--at", "3,1", "--on-top"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("placed at column 3, row 1, level"), "{out}");
}

#[test]
fn r17_one_place_is_one_undo_step() {
    let mut lab = lab(&[("build", json!({ "items": [block_item(3, 1, 0)] }))]);
    lab.ok("scene.place", json!({ "name": "build", "object": "block", "at": [3, 1] }));
    assert_eq!(lab.scene("build")["items"].as_array().unwrap().len(), 2);
    lab.ok("doc.undo", json!({ "doc": "scene:build" }));
    assert_eq!(lab.scene("build")["items"].as_array().unwrap().len(), 1);
}

// ----------------------------------------------------------------- R11

fn warnings(lab: &mut Lab, name: &str) -> Vec<Value> {
    lab.ok("check", json!({ "name": name }))["warnings"].as_array().unwrap().clone()
}

fn coded<'a>(warnings: &'a [Value], code: &str) -> Vec<&'a Value> {
    warnings.iter().filter(|w| w["code"] == code).collect()
}

#[test]
fn r11_check_warns_when_a_fixed_viewbox_cuts_off_painted_pixels() {
    let mut lab = lab(&[
        ("cut", json!({ "viewBox": [0, 0, 10, 10], "items": [block_item(0, 0, 0)] })),
        ("whole", json!({ "viewBox": [-100, -100, 200, 200], "items": [block_item(0, 0, 0)] })),
        ("fit", json!({ "items": [block_item(0, 0, 0)] })),
    ]);
    let cut = warnings(&mut lab, "cut");
    let found = coded(&cut, "cropped");
    assert_eq!(found.len(), 1, "{cut:?}");
    assert_eq!(found[0]["scene"], "cut");
    let outside = found[0]["pixels"].as_u64().unwrap();
    assert!(outside > 0);
    assert!(found[0]["text"].as_str().unwrap().contains(&format!("{outside} pixels outside its viewBox")), "{}", found[0]["text"]);
    // The exact count: the block's pixels, less those inside the 10x10 box.
    let sources = Ctx::new(&mut lab.session).sources().unwrap();
    let rendered = engine::engine::render_scene(&sources.sources, "cut", engine::engine::Order::Depth).unwrap();
    let pixels = engine::engine::composite(&rendered.root, engine::engine::CompositeOptions::default());
    let expected = pixels.keys().filter(|&&(x, y)| !(0..10).contains(&x) || !(0..10).contains(&y)).count();
    assert_eq!(outside as usize, expected);
    // It is a warning: the check still passes.
    assert_eq!(lab.ok("check", json!({ "name": "cut" }))["ok"], true);
    assert_eq!(lab.ok("check", json!({}))["ok"], true);
    // A box that holds everything, and a scene that fits, don't warn.
    assert!(coded(&warnings(&mut lab, "whole"), "cropped").is_empty());
    assert!(coded(&warnings(&mut lab, "fit"), "cropped").is_empty());
    let (code, out, _) = lab.cli(&["check", "cut"]);
    assert_eq!(code, 0);
    assert!(out.contains("warning: scenes/cut.json: ") && out.contains("outside its viewBox"), "{out}");
}

#[test]
fn r11_a_moving_item_that_leaves_the_box_is_counted() {
    let scene = json!({
        "viewBox": [-40, -30, 80, 60],
        "items": [{ "object": "pebble", "id": "p", "at": { "tile": [0, 0, 0] } }],
        "motion": { "duration": 1000, "tracks": [{ "item": "p", "keys": [{ "t": 0, "move": [0, 0] }, { "t": 500, "move": [200, 0] }] }] }
    });
    let mut lab = lab(&[("moves", scene)]);
    let found = warnings(&mut lab, "moves");
    let found = coded(&found, "cropped");
    assert_eq!(found.len(), 1);
    assert!(found[0]["pixels"].as_u64().unwrap() > 0);
}

#[test]
fn r6_check_notes_an_empty_scene_without_failing() {
    let mut lab = lab(&[("empty", json!({ "items": [] }))]);
    let found = warnings(&mut lab, "empty");
    assert_eq!(coded(&found, "empty").len(), 1, "{found:?}");
    assert_eq!(lab.ok("check", json!({ "name": "empty" }))["ok"], true);
    assert!(lab.ok("check", json!({ "name": "empty" }))["summary"].as_str().unwrap().starts_with("32×16 ·"));
}
