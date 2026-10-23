//! scene.ambient, scene.action, object.ambient and object.action, and
//! `--play` on both renders (spec R24, D6, D8): over the registry, the
//! command line, `serve --stdio` and `serve --mcp`, on temp copies of the
//! sample document.

#![allow(clippy::result_large_err)]

mod common;

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use common::{make_document, temp_dir, TempDocument};
use pixellab::cli::{main, Io};
use pixellab::context::Ctx;
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::{Error, Result};
use pixellab::registry::registry;
use pixellab::session::{Options, Session};
use serde_json::{json, Value};

const MEADOW: &str = r#"{
  "output": "meadow.svg",
  "viewBox": [0, 0, 64, 48],
  "items": [
    { "object": "dot", "at": { "px": [20, 20] } },
    { "object": "dot", "id": "other", "at": { "px": [40, 20] } }
  ]
}
"#;

/// A 12 by 12 sprite with a 2 by 2 square at its top left, so it has room to move.
const DOT: &str = r#"{
  "kind": "sprite",
  "anchor": [0, 0],
  "keys": { "a": "ink" },
  "layers": [
    { "map": ["aa..........", "aa..........", "............", "............", "............", "............", "............", "............", "............", "............", "............", "............"] },
    { "map": ["............", "............", "............", "............", "............", "............", "............", "............", "............", "............", "......aa....", "......aa...."] }
  ]
}
"#;

const DRIFT: &str = r#"[{"t":0,"move":[0,0]},{"t":500,"move":[8,0]}]"#;
const HOP: &str = r#"[{"t":0,"move":[0,0]},{"t":100,"move":[0,-6]}]"#;

fn doc_with_dot() -> TempDocument {
    let doc = make_document();
    doc.write("scenes/meadow.json", MEADOW);
    doc.write("objects/dot.json", DOT);
    doc
}

fn session(doc: &TempDocument) -> Session {
    let mut env = doc.env();
    env.insert("PIXELLAB_STATE".into(), doc.base.join("state").to_string_lossy().into_owned());
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    Session::new(document, Options { env, ..Options::default() })
}

fn run(s: &mut Session, name: &str, params: Value) -> Result<Value> {
    registry().call(&mut Ctx::new(s), name, params, false)
}

fn refused(r: Result<Value>) -> (String, String) {
    match r {
        Err(Error::Command(c)) => (c.code, c.message),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn cli(doc: &TempDocument, args: &[&str]) -> (i32, String, String) {
    let reg = registry();
    let mut argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    argv.push("--document".into());
    argv.push(doc.root.to_string_lossy().into_owned());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = {
        let mut io = Io { stdout: &mut out, stderr: &mut err, env: doc.env(), cwd: doc.root.clone(), registry: &reg };
        main(&argv, &mut io)
    };
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn read(doc: &TempDocument, file: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(doc.root.join(file)).unwrap()).unwrap()
}

fn png_of(out: &Value) -> engine::png::Image {
    let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in out["png"].as_str().unwrap().bytes().filter(|c| *c != b'=') {
        acc = acc << 6 | table.iter().position(|t| *t == c).unwrap() as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    engine::png::decode(&bytes).unwrap()
}

/// The box of the opaque pixels of a render at scale 1: [min x, min y, max x, max y].
fn ink_box(out: &Value) -> [u32; 4] {
    let png = png_of(out);
    let mut b = [u32::MAX, u32::MAX, 0, 0];
    for y in 0..png.height {
        for x in 0..png.width {
            if png.rgba[((y * png.width + x) * 4 + 3) as usize] > 0 {
                b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
            }
        }
    }
    b
}

fn keys(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}

// ---- scene ambient and action over the registry

#[test]
fn r24_d8_scene_ambient_sets_changes_and_clears_a_loop_and_undo_redo_follow() {
    let doc = doc_with_dot();
    let mut s = session(&doc);
    let set = run(&mut s, "scene.ambient", json!({ "name": "meadow", "item": "0", "duration": 16000, "keys": keys(DRIFT) })).unwrap();
    assert_eq!(set["changed"], true);
    assert_eq!(set["summary"], "Set dot's ambient loop: 16000 ms");
    assert_eq!(set["path"], "0");
    assert_eq!(set["ambient"]["duration"], 16000);
    let changed =
        run(&mut s, "scene.ambient", json!({ "name": "meadow", "item": "0", "duration": 8000, "offset": 2000, "keys": keys(DRIFT) }))
            .unwrap();
    assert_eq!(changed["ambient"]["offset"], 2000);
    let cleared = run(&mut s, "scene.ambient", json!({ "name": "meadow", "item": "0", "clear": true })).unwrap();
    assert_eq!(cleared["summary"], "Cleared dot's ambient loop");
    assert_eq!(cleared["ambient"], Value::Null);
    // Clearing what isn't there changes nothing.
    let again = run(&mut s, "scene.ambient", json!({ "name": "meadow", "item": "0", "clear": true })).unwrap();
    assert_eq!(again["changed"], false);
    // Undo steps back through the clear and the change to the first set, redo forward again.
    let undo = run(&mut s, "doc.undo", json!({ "doc": "scene:meadow" })).unwrap();
    assert!(undo["summary"].as_str().unwrap().starts_with("Undid: Cleared dot's ambient loop"), "{undo}");
    run(&mut s, "doc.undo", json!({ "doc": "scene:meadow" })).unwrap();
    let doc_now = run(&mut s, "scene.items", json!({ "name": "meadow" })).unwrap();
    assert!(doc_now.to_string().contains("dot"));
    run(&mut s, "doc.redo", json!({ "doc": "scene:meadow" })).unwrap();
    let saved = run(&mut s, "doc.save", json!({ "doc": "scene:meadow" })).unwrap();
    assert_eq!(saved["wrote"], true, "{saved}");
    assert_eq!(read(&doc, "scenes/meadow.json")["items"][0]["ambient"]["duration"], 8000);
}

#[test]
fn r24_d8_scene_action_adds_changes_and_deletes_and_a_name_is_per_item() {
    let doc = doc_with_dot();
    let mut s = session(&doc);
    let added =
        run(&mut s, "scene.action", json!({ "name": "meadow", "item": "other", "action": "hop", "duration": 400, "keys": keys(HOP) }))
            .unwrap();
    assert_eq!(added["summary"], "Added hop to other");
    assert_eq!(added["action"]["name"], "hop");
    assert_eq!(added["action"]["duration"], 400);
    let changed =
        run(&mut s, "scene.action", json!({ "name": "meadow", "item": "other", "action": "hop", "duration": 300, "keys": keys(HOP) }))
            .unwrap();
    assert_eq!(changed["summary"], "Changed hop on other");
    let wave = json!({ "name": "meadow", "item": "other", "action": "wave", "duration": 300, "keys": keys(HOP) });
    run(&mut s, "scene.action", wave).unwrap();
    let deleted = run(&mut s, "scene.action", json!({ "name": "meadow", "item": "other", "action": "hop", "delete": true })).unwrap();
    assert_eq!(deleted["summary"], "Deleted hop from other");
    assert_eq!(deleted["action"], Value::Null);
    run(&mut s, "doc.save", json!({ "doc": "scene:meadow" })).unwrap();
    let item = read(&doc, "scenes/meadow.json")["items"][1].clone();
    assert_eq!(item["actions"].as_object().unwrap().keys().collect::<Vec<_>>(), ["wave"]);
    run(&mut s, "scene.action", json!({ "name": "meadow", "item": "other", "action": "wave", "delete": true })).unwrap();
    run(&mut s, "doc.save", json!({ "doc": "scene:meadow" })).unwrap();
    assert!(read(&doc, "scenes/meadow.json")["items"][1].get("actions").is_none(), "no empty actions left");
}

#[test]
fn r24_d8_scene_commands_refuse_what_the_engines_rules_name_and_change_nothing() {
    let doc = doc_with_dot();
    let mut s = session(&doc);
    let past = json!([{ "t": 0, "move": [0, 0] }, { "t": 900, "move": [4, 0] }]);
    let (code, message) = refused(run(&mut s, "scene.ambient", json!({ "name": "meadow", "item": "0", "duration": 800, "keys": past })));
    assert_eq!(code, "invalid");
    assert!(message.contains("t must be from 0 to below the duration"), "{message}");
    let first = json!([{ "t": 5, "move": [0, 0] }]);
    let (_, message) = refused(run(&mut s, "scene.ambient", json!({ "name": "meadow", "item": "0", "duration": 800, "keys": first })));
    assert!(message.contains("the first key must be at t: 0"), "{message}");
    let (_, message) = refused(run(
        &mut s,
        "scene.ambient",
        json!({ "name": "meadow", "item": "0", "duration": 800, "offset": 800, "keys": keys(DRIFT) }),
    ));
    assert!(message.contains("ambient.offset"), "{message}");
    let (_, message) = refused(run(
        &mut s,
        "scene.action",
        json!({ "name": "meadow", "item": "0", "action": "Bad Name", "duration": 800, "keys": keys(HOP) }),
    ));
    assert!(message.contains("names are lowercase words joined by hyphens"), "{message}");
    // One name is one move in a scene.
    run(&mut s, "scene.action", json!({ "name": "meadow", "item": "0", "action": "hop", "duration": 400, "keys": keys(HOP) })).unwrap();
    let (code, message) = refused(run(
        &mut s,
        "scene.action",
        json!({ "name": "meadow", "item": "other", "action": "hop", "duration": 200, "keys": keys(HOP) }),
    ));
    assert_eq!(code, "invalid");
    assert!(message.contains("one name is one move in a scene"), "{message}");
    // An item with scene motion can't have an ambient loop.
    run(&mut s, "scene.motion.set", json!({ "name": "meadow", "duration": 1000 })).unwrap();
    run(&mut s, "scene.track.add", json!({ "name": "meadow", "item": "other" })).unwrap();
    let (_, message) =
        refused(run(&mut s, "scene.ambient", json!({ "name": "meadow", "item": "other", "duration": 800, "keys": keys(DRIFT) })));
    assert!(message.contains("has scene motion and an ambient loop"), "{message}");
    // Missing parts, a missing item and a missing action.
    assert_eq!(refused(run(&mut s, "scene.ambient", json!({ "name": "meadow", "item": "0", "duration": 800 }))).0, "usage");
    assert_eq!(refused(run(&mut s, "scene.ambient", json!({ "name": "meadow", "item": "0", "clear": true, "duration": 8 }))).0, "usage");
    assert_eq!(
        refused(run(&mut s, "scene.ambient", json!({ "name": "meadow", "item": "9", "duration": 8, "keys": keys(DRIFT) }))).0,
        "not_found"
    );
    assert_eq!(
        refused(run(&mut s, "scene.action", json!({ "name": "meadow", "item": "0", "action": "nope", "delete": true }))).0,
        "not_found"
    );
    // Only the hop and the track survived.
    let status = run(&mut s, "doc.status", json!({ "doc": "scene:meadow" })).unwrap();
    assert_eq!(status["edits"].as_array().unwrap().len(), 3, "{status}");
}

// ---- object ambient and action

#[test]
fn r24_d8_object_ambient_and_action_edit_a_map_layer() {
    let doc = doc_with_dot();
    let mut s = session(&doc);
    let set = run(&mut s, "object.ambient", json!({ "name": "dot", "layer": 1, "duration": 1000, "keys": keys(DRIFT) })).unwrap();
    assert_eq!(set["summary"], "Set dot's layer 1 ambient loop: 1000 ms");
    assert_eq!(set["layer"], 1);
    let added =
        run(&mut s, "object.action", json!({ "name": "dot", "layer": 1, "action": "hop", "duration": 400, "keys": keys(HOP) })).unwrap();
    assert_eq!(added["summary"], "Added hop to dot's layer 1");
    let changed =
        run(&mut s, "object.action", json!({ "name": "dot", "layer": 1, "action": "hop", "duration": 300, "keys": keys(HOP) })).unwrap();
    assert_eq!(changed["summary"], "Changed hop on dot's layer 1");
    run(&mut s, "doc.save", json!({ "doc": "object:dot" })).unwrap();
    let layer = read(&doc, "objects/dot.json")["layers"][1].clone();
    assert_eq!(layer["ambient"]["duration"], 1000);
    assert_eq!(layer["actions"]["hop"]["duration"], 300);
    assert!(read(&doc, "objects/dot.json")["layers"][0].get("ambient").is_none());
    let deleted = run(&mut s, "object.action", json!({ "name": "dot", "layer": 1, "action": "hop", "delete": true })).unwrap();
    assert_eq!(deleted["summary"], "Deleted hop from dot's layer 1");
    let cleared = run(&mut s, "object.ambient", json!({ "name": "dot", "layer": 1, "clear": true })).unwrap();
    assert_eq!(cleared["summary"], "Cleared dot's layer 1 ambient loop");
    run(&mut s, "doc.undo", json!({ "doc": "object:dot" })).unwrap();
    run(&mut s, "doc.undo", json!({ "doc": "object:dot" })).unwrap();
    let redo = run(&mut s, "doc.redo", json!({ "doc": "object:dot" })).unwrap();
    assert!(redo["summary"].as_str().unwrap().starts_with("Redid: Deleted hop"), "{redo}");
}

#[test]
fn r24_d8_object_commands_refuse_a_frame_loop_a_block_an_outfit_a_missing_layer_and_bad_keys() {
    let doc = doc_with_dot();
    doc.write(
        "objects/flag.json",
        r#"{ "kind": "sprite", "anchor": [0, 0], "keys": { "a": "ink" }, "layers": [{ "loop": "wf", "prefix": "flag", "frames": [["a"], ["a"]] }] }"#,
    );
    let mut s = session(&doc);
    let args = |name: &str, layer: i64| json!({ "name": name, "layer": layer, "duration": 1000, "keys": keys(DRIFT) });
    let (code, message) = refused(run(&mut s, "object.ambient", args("flag", 0)));
    assert_eq!(code, "not_a_map_layer");
    assert!(message.contains("frame loop"), "{message}");
    let (code, message) = refused(run(&mut s, "object.ambient", args("block", 0)));
    assert_eq!(code, "not_a_map_layer");
    assert!(message.contains("block"), "{message}");
    let (code, message) = refused(run(&mut s, "object.ambient", args("dot", 5)));
    assert_eq!(code, "not_found");
    assert!(message.contains("no layer 5"), "{message}");
    let mut action = args("flag", 0);
    action["action"] = json!("hop");
    assert_eq!(refused(run(&mut s, "object.action", action)).0, "not_a_map_layer");
    let late = json!({ "name": "dot", "layer": 0, "duration": 100, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 100, "move": [1, 0] }] });
    assert_eq!(refused(run(&mut s, "object.ambient", late)).0, "invalid");
    let fractional = json!({ "name": "dot", "layer": 0, "duration": 100, "keys": [{ "t": 0, "move": [0.5, 0] }] });
    assert_eq!(refused(run(&mut s, "object.ambient", fractional)).0, "invalid");
    let status = run(&mut s, "doc.status", json!({ "doc": "object:dot" })).unwrap();
    assert_eq!(status["dirty"], false, "nothing was changed");
}

// ---- the command line

#[test]
fn r24_d8_the_command_line_sets_and_clears_a_loop_and_an_action_and_saves_them() {
    let doc = doc_with_dot();
    let (code, out, err) = cli(&doc, &["scene", "ambient", "meadow", "0", "--duration", "16000", "--keys", DRIFT]);
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with("Set dot's ambient loop: 16000 ms"), "{out}");
    assert_eq!(read(&doc, "scenes/meadow.json")["items"][0]["ambient"]["keys"][1]["move"], json!([8, 0]));
    let (code, out, err) = cli(&doc, &["scene", "action", "meadow", "other", "hop", "--duration", "400", "--keys", HOP]);
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with("Added hop to other"), "{out}");
    let (code, _, err) = cli(&doc, &["object", "ambient", "dot", "1", "--offset", "250", "--duration", "1000", "--keys", DRIFT]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(read(&doc, "objects/dot.json")["layers"][1]["ambient"]["offset"], 250);
    let (code, _, err) = cli(&doc, &["object", "action", "dot", "0", "hop", "--duration", "400", "--keys", HOP]);
    assert_eq!(code, 0, "{err}");
    let (code, _, err) = cli(&doc, &["check"]);
    assert_eq!(code, 0, "{err}");
    let (code, out, _) = cli(&doc, &["scene", "ambient", "meadow", "0", "--clear"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("Cleared dot's ambient loop"), "{out}");
    assert!(read(&doc, "scenes/meadow.json")["items"][0].get("ambient").is_none());
    let (code, _, _) = cli(&doc, &["scene", "action", "meadow", "other", "hop", "--delete"]);
    assert_eq!(code, 0);
    assert!(read(&doc, "scenes/meadow.json")["items"][1].get("actions").is_none());
    // A refusal is a failed command, and the file is not written.
    let before = fs::read_to_string(doc.root.join("scenes/meadow.json")).unwrap();
    let past = r#"[{"t":0,"move":[0,0]},{"t":900,"move":[4,0]}]"#;
    let (code, _, err) = cli(&doc, &["scene", "ambient", "meadow", "0", "--duration", "800", "--keys", past]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("t must be from 0 to below the duration"), "{err}");
    assert_eq!(fs::read_to_string(doc.root.join("scenes/meadow.json")).unwrap(), before);
    let (code, _, err) = cli(&doc, &["scene", "ambient", "meadow", "0", "--duration", "800"]);
    assert_eq!(code, 2, "{err}");
}

// ---- renders

#[test]
fn r24_d8_scene_render_places_an_ambient_loop_at_time_and_play_moves_it_while_the_action_runs() {
    let doc = doc_with_dot();
    let mut s = session(&doc);
    run(&mut s, "scene.ambient", json!({ "name": "meadow", "item": "0", "duration": 1000, "keys": keys(DRIFT) })).unwrap();
    run(&mut s, "scene.action", json!({ "name": "meadow", "item": "0", "action": "hop", "duration": 400, "keys": keys(HOP) })).unwrap();
    let render = |s: &mut Session, extra: Value| {
        let mut params = json!({ "name": "meadow" });
        for (k, v) in extra.as_object().unwrap() {
            params[k] = v.clone();
        }
        run(s, "scene.render", params).unwrap()
    };
    // Both dots are 12 wide; the box of everything is read from the first dot's left edge.
    let rest = ink_box(&render(&mut s, json!({ "time": 0 })));
    assert_eq!(rest[0], 20, "the first dot's left edge at rest");
    let drifted = ink_box(&render(&mut s, json!({ "time": 500 })));
    assert_eq!(drifted[0], 28, "the loop has moved it 8 pixels at 500 ms");
    let looped = ink_box(&render(&mut s, json!({ "time": 1000 })));
    assert_eq!(looped, rest, "and it is back at the loop's start");
    // The action plays over the loop: up 6 while it holds its last key...
    let hopping = ink_box(&render(&mut s, json!({ "time": 500, "play": "hop@300" })));
    assert_eq!([hopping[0], hopping[1]], [rest[0], rest[1] - 6], "the action replaces the loop's move: 6 up, and not 8 right");
    // ...and back on the loop once the action has ended.
    assert_eq!(ink_box(&render(&mut s, json!({ "time": 500, "play": "hop@400" }))), drifted);
    assert_eq!(ink_box(&render(&mut s, json!({ "time": 500, "play": "hop@1000" }))), drifted);
    let without = render(&mut s, json!({ "time": 500 }));
    let ended = render(&mut s, json!({ "time": 500, "play": "hop@400" }));
    assert_eq!(without["png"], ended["png"]);
    // An action nothing has, and a play that isn't one, are refused.
    let mut params = json!({ "name": "meadow", "play": "wave@10" });
    assert_eq!(refused(run(&mut s, "scene.render", params.clone())).0, "not_found");
    params["play"] = json!("hop");
    assert_eq!(refused(run(&mut s, "scene.render", params)).0, "usage");
}

#[test]
fn r24_d8_object_render_places_a_layers_loop_at_time_and_play_moves_it_while_the_action_runs() {
    let doc = doc_with_dot();
    let mut s = session(&doc);
    let nudge = json!([{ "t": 0, "move": [0, 0] }, { "t": 500, "move": [4, 0] }]);
    run(&mut s, "object.ambient", json!({ "name": "dot", "layer": 1, "duration": 1000, "keys": nudge })).unwrap();
    run(&mut s, "object.action", json!({ "name": "dot", "layer": 1, "action": "hop", "duration": 400, "keys": json!([{ "t": 0, "move": [0, 0] }, { "t": 100, "move": [-2, -6] }]) }))
        .unwrap();
    // The second layer's square is the only ink outside the first layer's 4 by 4 corner.
    let at = |s: &mut Session, params: Value| {
        let mut p = json!({ "name": "dot" });
        for (k, v) in params.as_object().unwrap() {
            p[k] = v.clone();
        }
        run(s, "object.render", p).unwrap()
    };
    let png_of_square = |out: &Value| -> Vec<(u32, u32)> {
        let png = png_of(out);
        let mut found = Vec::new();
        for y in 0..png.height {
            for x in 0..png.width {
                if png.rgba[((y * png.width + x) * 4 + 3) as usize] > 0 && (y >= 4 || x >= 4) {
                    found.push((x, y));
                }
            }
        }
        found
    };
    let rest = png_of_square(&at(&mut s, json!({ "time": 0 })));
    assert_eq!(rest[0], (6, 10), "the second layer's square at rest");
    assert_eq!(png_of_square(&at(&mut s, json!({ "time": 1000 }))), rest, "and back at the loop's start");
    let drifted = png_of_square(&at(&mut s, json!({ "time": 500 })));
    assert_eq!(drifted[0], (6 + 4, 10), "moved 4 right by the loop at 500 ms");
    let hop = png_of_square(&at(&mut s, json!({ "time": 500, "play": "hop@300" })));
    assert_eq!(hop[0], (4, 4), "the action's last key: 2 left and 6 up");
    let after = at(&mut s, json!({ "time": 500, "play": "hop@400" }));
    assert_eq!(after["png"], at(&mut s, json!({ "time": 500 }))["png"], "back on the loop once the action ended");
    // --play needs --time, and an action no layer has.
    assert_eq!(refused(run(&mut s, "object.render", json!({ "name": "dot", "play": "hop@10" }))).0, "usage");
    assert_eq!(refused(run(&mut s, "object.render", json!({ "name": "dot", "time": 0, "play": "wave@10" }))).0, "not_found");
}

#[test]
fn r23_a_layer_moved_by_its_action_or_loop_is_fully_inside_the_render_and_the_rest_render_is_unchanged() {
    let doc = doc_with_dot();
    let mut s = session(&doc);
    // Layer 0's square sits at the map's top left: moving it left and up leaves the map's box.
    run(&mut s, "object.action", json!({ "name": "dot", "layer": 0, "action": "lunge", "duration": 400, "keys": json!([{ "t": 0, "move": [0, 0] }, { "t": 100, "move": [-3, -2] }]) })).unwrap();
    let render = |s: &mut Session, extra: Value| {
        let mut p = json!({ "name": "dot" });
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        run(s, "object.render", p).unwrap()
    };
    let rest = render(&mut s, json!({}));
    assert_eq!(rest["viewBox"], json!([0, 0, 12, 12]), "the rest render is the map's box, as before");
    let at_rest_time = render(&mut s, json!({ "time": 0 }));
    assert_eq!(at_rest_time["viewBox"], json!([0, 0, 12, 12]), "nothing moved at time 0");
    assert_eq!(at_rest_time["png"], rest["png"]);
    let moved = render(&mut s, json!({ "time": 0, "play": "lunge@300" }));
    assert_eq!(moved["viewBox"], json!([-3, -2, 15, 14]), "the box grows to hold the moved layer");
    let ink = ink_box(&moved);
    // The square (2 by 2) now covers x -3..-2, y -2..-1: columns 0..1 and rows 0..1 of the grown image.
    assert_eq!([ink[0], ink[1]], [0, 0], "the whole square is inside the render, none of it clipped");
    assert_eq!(moved["width"], json!(15));
    assert_eq!(png_of(&moved).rgba[3], 255, "its first pixel is opaque");
}

// ---- serve --stdio and serve --mcp

struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<Value>,
    seen: Vec<Value>,
    next_id: u64,
}

fn start(args: &[&str], doc: &TempDocument, state: &std::path::Path) -> Server {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pixellab"))
        .arg("serve")
        .args(args)
        .arg("--document")
        .arg(&doc.root)
        .env_remove("PIXELLAB_SITE")
        .env_remove("PIXELLAB_DOC")
        .envs(doc.env())
        .env("PIXELLAB_STATE", state)
        .current_dir(state)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn");
    let stdout = child.stdout.take().unwrap();
    let stdin = child.stdin.take();
    let (tx, lines) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(|l| l.ok()) {
            let _ = tx.send(serde_json::from_str::<Value>(&line).expect("every stdout line is JSON"));
        }
    });
    Server { child, stdin, lines, seen: Vec::new(), next_id: 1 }
}

impl Server {
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string();
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{line}").unwrap();
        stdin.flush().unwrap();
        loop {
            let m =
                self.lines.recv_timeout(Duration::from_secs(15)).unwrap_or_else(|_| panic!("timed out on {method}; saw {:?}", self.seen));
            self.seen.push(m.clone());
            if m.get("id") == Some(&json!(id)) && m.get("method").is_none() {
                return m;
            }
        }
    }

    /// The next notification with this method, after the ones already seen.
    fn notification(&mut self, method: &str, test: impl Fn(&Value) -> bool) -> Value {
        if let Some(i) = self.seen.iter().position(|m| m["method"] == method && test(m)) {
            return self.seen.remove(i);
        }
        loop {
            let m =
                self.lines.recv_timeout(Duration::from_secs(15)).unwrap_or_else(|_| panic!("timed out on {method}; saw {:?}", self.seen));
            if m["method"] == method && test(&m) {
                return m;
            }
            self.seen.push(m);
        }
    }

    fn call(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({ "name": name, "arguments": args }))["result"].clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stdin = None;
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn r24_d8_over_stdio_each_command_edits_notifies_doc_changed_and_undoes() {
    let doc = doc_with_dot();
    let state = temp_dir("pixellab-state-");
    let mut server = start(&["--stdio"], &doc, &state);
    let dirty = |n: &Value| n["params"]["doc"] == "scene:meadow" && n["params"]["dirty"] == true;
    let r = server.request("scene.ambient", json!({ "name": "meadow", "item": "0", "duration": 16000, "keys": keys(DRIFT) }));
    assert_eq!(r["result"]["summary"], "Set dot's ambient loop: 16000 ms", "{r}");
    server.notification("doc.changed", dirty);
    let r =
        server.request("scene.action", json!({ "name": "meadow", "item": "other", "action": "hop", "duration": 400, "keys": keys(HOP) }));
    assert_eq!(r["result"]["summary"], "Added hop to other");
    let r = server.request("object.ambient", json!({ "name": "dot", "layer": 1, "duration": 1000, "keys": keys(DRIFT) }));
    assert_eq!(r["result"]["summary"], "Set dot's layer 1 ambient loop: 1000 ms");
    server.notification("doc.changed", |n| n["params"]["doc"] == "object:dot" && n["params"]["dirty"] == true);
    let r = server.request("object.action", json!({ "name": "dot", "layer": 1, "action": "hop", "duration": 400, "keys": keys(HOP) }));
    assert_eq!(r["result"]["summary"], "Added hop to dot's layer 1");
    let undo = server.request("doc.undo", json!({ "doc": "object:dot" }));
    assert!(undo["result"]["summary"].as_str().unwrap().starts_with("Undid: Added hop to dot's layer 1"), "{undo}");
    // A refusal is an error response; the doc stays as it was.
    let bad = server.request("scene.ambient", json!({ "name": "meadow", "item": "0", "duration": 100, "keys": keys(DRIFT) }));
    assert_eq!(bad["error"]["data"]["code"], "invalid", "{bad}");
    let saved = server.request("doc.save", json!({ "doc": "scene:meadow" }));
    assert_eq!(saved["result"]["wrote"], true, "{saved}");
    let item = read(&doc, "scenes/meadow.json")["items"].clone();
    assert_eq!([item[0]["ambient"]["duration"].clone(), item[1]["actions"]["hop"]["duration"].clone()], [json!(16000), json!(400)]);
    let rendered = server.request("scene.render", json!({ "name": "meadow", "time": 500, "play": "hop@300" }));
    assert_eq!(rendered["result"]["time"], 500);
}

#[test]
fn r24_d8_over_mcp_the_four_edits_are_tools_and_they_apply() {
    let doc = doc_with_dot();
    let state = temp_dir("pixellab-state-");
    let mut server = start(&["--mcp"], &doc, &state);
    server.request(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test-agent", "version": "9.9" } }),
    );
    let tools = server.request("tools/list", json!({}))["result"]["tools"].clone();
    for name in ["scene_ambient", "scene_action", "object_ambient", "object_action"] {
        let tool = tools.as_array().unwrap().iter().find(|t| t["name"] == name).unwrap_or_else(|| panic!("no tool {name}"));
        assert!(tool["description"].as_str().unwrap().len() > 20);
        assert!(tool["inputSchema"]["properties"].get("keys").is_some() || tool["inputSchema"]["properties"].get("delete").is_some());
    }
    let play = tools.as_array().unwrap().iter().find(|t| t["name"] == "scene_render").unwrap();
    assert!(play["inputSchema"]["properties"].get("play").is_some());
    let scene_before = fs::read_to_string(doc.root.join("scenes/meadow.json")).unwrap();
    let r = server.call("scene_ambient", json!({ "name": "meadow", "item": "0", "duration": 16000, "keys": keys(DRIFT) }));
    assert!(r.get("isError").is_none(), "{r}");
    assert_eq!(r["structuredContent"]["summary"], "Set dot's ambient loop: 16000 ms");
    let r = server.call("scene_action", json!({ "name": "meadow", "item": "0", "action": "hop", "duration": 400, "keys": keys(HOP) }));
    assert_eq!(r["structuredContent"]["summary"], "Added hop to dot");
    let r = server.call("object_action", json!({ "name": "dot", "layer": 0, "action": "hop", "duration": 400, "keys": keys(HOP) }));
    assert_eq!(r["structuredContent"]["summary"], "Added hop to dot's layer 0");
    let r = server.call("object_ambient", json!({ "name": "dot", "layer": 0, "clear": true }));
    assert_eq!(r["structuredContent"]["changed"], false);
    let r = server.call("scene_ambient", json!({ "name": "meadow", "item": "0", "duration": 100, "keys": keys(DRIFT) }));
    assert_eq!(r["isError"], true, "{r}");
    // Each edit applied and saved as the agent's (autosave spec, R16).
    let source: Value = serde_json::from_str(&fs::read_to_string(doc.root.join("scenes/meadow.json")).unwrap()).unwrap();
    assert_ne!(fs::read_to_string(doc.root.join("scenes/meadow.json")).unwrap(), scene_before);
    assert_eq!(source["items"][0]["ambient"]["duration"], 16000);
    assert_eq!(source["items"][0]["actions"]["hop"]["duration"], 400);
    // One version: the agent's run of edits, the scene's and the object's.
    let history = server.call("history_list", json!({ "doc": "scene:meadow" }));
    let v = &history["structuredContent"]["versions"][0];
    assert_eq!(v["edits"], json!(["Set dot's ambient loop: 16000 ms", "Added hop to dot", "Added hop to dot's layer 0"]));
    assert_eq!(v["files"], json!(["scenes/meadow.json", "objects/dot.json"]));
    let dot: Value = serde_json::from_str(&fs::read_to_string(doc.root.join("objects/dot.json")).unwrap()).unwrap();
    assert!(dot.to_string().contains("hop"), "{dot}");
}
