//! The object editing commands (ports cli/test/map-set.test.mjs and the
//! object parts of cli/test/edit-commands.test.mjs), on temp copies of the
//! sample document (format 2).

#![allow(clippy::result_large_err)]

mod common;

use std::collections::HashMap;

use pixellab::cli::{main, Io};
use pixellab::context::Ctx;
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::Error;
use pixellab::registry::registry;
use pixellab::session::{Options, Session};
use serde_json::{json, Value};

use common::{make_document, temp_dir, TempDocument};

fn many() -> String {
    (0..14).map(|i| format!("    \"c-{i}\": \"#{:06X}\"", (0x101010 * (i + 1)) & 0xFFFFFF)).collect::<Vec<_>>().join(",\n")
}

fn palette() -> String {
    format!(
        "{{\n  \"world\": {{\n    \"ink\": \"#2E2418\",\n    \"grass-2\": \"#8FA56E\",\n    \"grass-4\": \"#4E6B3A\",\n    \"soil-2\": \"#8A5A34\",\n    \"soil-3\": \"#5A3E2B\",\n{}\n  }},\n  \"outfit\": {{\n    \"teal-1\": \"#7FA89B\"\n  }}\n}}\n",
        many()
    )
}

const FLAME: &str = r#"{
  "kind": "sprite",
  "anchor": [1, 1],
  "keys": { "a": "ink" },
  "layers": [{ "loop": "ff", "prefix": "f", "frames": [["a.", ".a"], [".a", "a."], ["aa", ".."], ["..", "aa"]] }]
}
"#;
const PEBBLE_TOO: &str = r#"{ "kind": "sprite", "extends": "pebble", "rows": {} }
"#;

fn map_doc() -> TempDocument {
    let doc = make_document();
    doc.write("palette.json", &palette());
    doc.write("objects/flame.json", FLAME);
    doc.write("objects/pebble-too.json", PEBBLE_TOO);
    doc
}

fn session(doc: &TempDocument, state: &std::path::Path) -> Session {
    let mut env = doc.env();
    env.insert("PIXELLAB_STATE".into(), state.to_string_lossy().into_owned());
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    Session::new(document, Options { env, ..Options::default() })
}

fn call(s: &mut Session, name: &str, params: Value) -> Result<Value, Error> {
    let mut ctx = Ctx::new(s);
    registry().call(&mut ctx, name, params, false)
}

fn source(s: &mut Session, name: &str) -> Value {
    let mut ctx = Ctx::new(s);
    ctx.doc(pixellab::context::DocKind::Object, name).unwrap()
}

fn code_of(e: &Error) -> String {
    match e {
        Error::Command(c) => c.code.clone(),
        Error::Internal(_) => "internal".into(),
    }
}

fn rejects(r: Result<Value, Error>, code: Option<&str>, message: Option<&str>) {
    let e = r.expect_err("should be refused");
    if let Some(code) = code {
        assert_eq!(code_of(&e), code, "{}", e.message());
    }
    if let Some(m) = message {
        assert!(e.message().contains(m), "{} should contain {m}", e.message());
    }
}

// ---- map-set.test.mjs ----

#[test]
fn r15_with_a_legend_any_characters_map_to_the_objects_keys_new_colors_get_keys_in_one_undo_step() {
    let doc = map_doc();
    let state = temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    let r = call(
        &mut s,
        "object.map.set",
        json!({ "name": "pebble", "rows": ["xyyx", "y..y", "y..y", "xyyx"], "keys": { "x": "ink", "y": "soil-2" } }),
    )
    .unwrap();
    assert_eq!(
        (r["changed"].clone(), r["size"].clone(), r["colors"].clone(), r["keysAdded"].clone()),
        (json!(true), json!([4, 4]), json!(2), json!(["s: soil-2"]))
    );
    let pebble = source(&mut s, "pebble");
    assert_eq!(pebble["layers"][0]["map"], json!(["cssc", "s..s", "s..s", "cssc"]));
    assert_eq!(pebble["keys"]["s"], "soil-2");
    assert_eq!(s.entries.get("object:pebble").unwrap().undo.len(), 1);
    s.step("object:pebble", true).unwrap();
    assert_eq!(source(&mut s, "pebble")["layers"][0]["map"], json!([".bb.", "baab", "caac", ".cc."]));
}

#[test]
fn r15_without_a_legend_rows_use_the_objects_own_keys_a_frame_of_a_loop_is_set_alone() {
    let doc = map_doc();
    let state = temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    call(&mut s, "object.map.set", json!({ "name": "pebble", "rows": ["aaaa", "bbbb", "cccc", "...."] })).unwrap();
    assert_eq!(source(&mut s, "pebble")["layers"][0]["map"], json!(["aaaa", "bbbb", "cccc", "...."]));
    call(&mut s, "object.map.set", json!({ "name": "flame", "rows": ["..", ".."], "frame": 2 })).unwrap();
    assert_eq!(source(&mut s, "flame")["layers"][0]["frames"], json!([["a.", ".a"], [".a", "a."], ["..", ".."], ["..", "aa"]]));
    rejects(call(&mut s, "object.map.set", json!({ "name": "flame", "rows": ["..", ".."], "frame": 7 })), Some("usage"), None);
    rejects(
        call(&mut s, "object.map.set", json!({ "name": "pebble", "rows": ["aaaa", "bbbb", "cccc", "...."], "frame": 1 })),
        Some("usage"),
        None,
    );
}

#[test]
fn r15_a_size_change_needs_resize_the_size_cap_and_ragged_rows_are_refused() {
    let doc = map_doc();
    let state = temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    let rows = json!(["aaaaaa", "a....a", "aaaaaa"]);
    rejects(call(&mut s, "object.map.set", json!({ "name": "pebble", "rows": rows })), Some("size_mismatch"), None);
    let r = call(&mut s, "object.map.set", json!({ "name": "pebble", "rows": rows, "resize": true })).unwrap();
    assert_eq!(r["size"], json!([6, 3]));
    assert_eq!(source(&mut s, "pebble")["layers"][0]["map"], rows);
    rejects(call(&mut s, "object.map.set", json!({ "name": "pebble", "rows": ["a".repeat(65)], "resize": true })), Some("size_cap"), None);
    rejects(
        call(&mut s, "object.map.set", json!({ "name": "pebble", "rows": ["aaa", "aa"], "resize": true })),
        None,
        Some("row 1 is 2 wide; every row must be 3"),
    );
}

#[test]
fn r15_the_palette_outfit_colors_unknown_characters_and_the_12_color_cap_are_checked() {
    let doc = map_doc();
    let state = temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    let z = json!(["zzzz", "zzzz", "zzzz", "zzzz"]);
    rejects(
        call(&mut s, "object.map.set", json!({ "name": "pebble", "rows": z, "keys": { "z": "no-such-color" } })),
        None,
        Some("no palette color named no-such-color"),
    );
    rejects(
        call(&mut s, "object.map.set", json!({ "name": "pebble", "rows": z, "keys": { "z": "teal-1" } })),
        Some("color_not_allowed"),
        None,
    );
    rejects(
        call(&mut s, "object.map.set", json!({ "name": "pebble", "rows": ["qqqq", "qqqq", "qqqq", "qqqq"] })),
        None,
        Some("\"q\" isn't a key"),
    );
    let legend: serde_json::Map<String, Value> =
        "abcdefghijklm".chars().enumerate().map(|(i, ch)| (ch.to_string(), json!(format!("c-{i}")))).collect();
    rejects(
        call(&mut s, "object.map.set", json!({ "name": "pebble", "rows": ["abcdefghijklm"], "keys": legend, "resize": true })),
        Some("color_cap"),
        None,
    );
    assert_eq!(source(&mut s, "pebble")["layers"][0]["map"], json!([".bb.", "baab", "caac", ".cc."]), "a refused map changes nothing");
    let legend: serde_json::Map<String, Value> =
        "abcdefghijklmnop".chars().enumerate().map(|(i, ch)| (ch.to_string(), json!(format!("c-{}", i % 12)))).collect();
    let r = call(&mut s, "object.map.set", json!({ "name": "pebble", "rows": ["abcd", "efgh", "ijkl", "mnop"], "keys": legend })).unwrap();
    assert_eq!(r["colors"], 12);
}

#[test]
fn r15_an_object_that_extends_another_gets_row_overrides_blocks_are_refused() {
    let doc = map_doc();
    let state = temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    call(&mut s, "object.map.set", json!({ "name": "pebble-too", "rows": [".bb.", "cccc", "caac", ".cc."] })).unwrap();
    assert_eq!(source(&mut s, "pebble-too")["rows"], json!({ "0": { "1": "cccc" } }));
    rejects(
        call(&mut s, "object.map.set", json!({ "name": "pebble-too", "rows": ["..", ".."], "resize": true })),
        Some("size_mismatch"),
        None,
    );
    rejects(call(&mut s, "object.map.set", json!({ "name": "block", "rows": [".."] })), Some("not_editable"), None);
}

#[test]
fn r15_an_object_that_extends_another_gets_row_overrides_and_check_finds_no_problems() {
    let doc = map_doc();
    let state = temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    call(&mut s, "object.map.set", json!({ "name": "pebble-too", "rows": [".bb.", "cccc", "caac", ".cc."] })).unwrap();
    assert_eq!(call(&mut s, "check", json!({ "name": "pebble-too" })).unwrap()["problems"], json!([]));
}

// ---- edit-commands.test.mjs: the object commands, one-shot ----

const SPRITE: &str = r#"{
  "kind": "sprite",
  "anchor": [2, 2],
  "keys": { "o": "ink" },
  "layers": [
    { "map": ["oo..", "o...", "...."] },
    {
      "loop": "ff",
      "prefix": "f",
      "frames": [
        ["....", ".o..", "...."],
        ["....", "..o.", "...."],
        ["....", "...o", "...."],
        ["....", "....", "...o"]
      ]
    }
  ]
}
"#;
const FILE: &str = "objects/zz-sprite.json";

fn cli_doc() -> TempDocument {
    let doc = make_document();
    doc.write(FILE, SPRITE);
    doc
}

fn cli(doc: &TempDocument, args: &[&str]) -> (i32, String, String) {
    let reg = registry();
    let state = doc.base.join("state");
    let mut env = doc.env();
    env.insert("PIXELLAB_STATE".into(), state.to_string_lossy().into_owned());
    let mut argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    argv.push("--document".into());
    argv.push(doc.root.to_string_lossy().into_owned());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = {
        let mut io = Io { stdout: &mut out, stderr: &mut err, env: env as HashMap<String, String>, cwd: doc.root.clone(), registry: &reg };
        main(&argv, &mut io)
    };
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn text(doc: &TempDocument, file: &str) -> Option<String> {
    std::fs::read_to_string(doc.root.join(file)).ok()
}

fn read_doc(doc: &TempDocument, file: &str) -> Value {
    serde_json::from_str(&text(doc, file).unwrap()).unwrap()
}

/// Runs a command with --dry-run (nothing written, a diff shown), then for real, and returns the doc after.
fn edit(doc: &TempDocument, args: &[&str], file: &str) -> Value {
    let before = text(doc, file);
    let mut dry_args = args.to_vec();
    dry_args.push("--dry-run");
    let (code, stdout, stderr) = cli(doc, &dry_args);
    assert_eq!(code, 0, "{args:?} --dry-run: {stderr}");
    assert!(stdout.contains(&format!("--- a/{file}\n+++ b/{file}\n@@ ")), "{args:?}: {stdout}");
    assert!(stdout.contains("dry run: nothing written"), "{stdout}");
    assert_eq!(text(doc, file), before, "{args:?} --dry-run wrote {file}");
    let (code, stdout, stderr) = cli(doc, args);
    assert_eq!(code, 0, "{args:?}: {stderr}");
    assert!(stdout.lines().any(|l| l == format!("wrote {file}")), "{args:?}: {stdout}");
    assert_ne!(text(doc, file), before, "{args:?} changed nothing");
    read_doc(doc, file)
}

#[test]
fn scene_new_and_object_new() {
    let doc = cli_doc();
    assert_eq!(cli(&doc, &["object", "new", "zz-rock", "--width", "5", "--height", "3"]).0, 0);
    let rock = read_doc(&doc, "objects/zz-rock.json");
    assert_eq!(rock["anchor"], json!([2, 2]));
    assert_eq!(rock["layers"][0]["map"], json!([".....", ".....", "....."]));
    assert_eq!(cli(&doc, &["object", "new", "zz-cube", "--kind", "block"]).0, 0);
    assert_eq!(read_doc(&doc, "objects/zz-cube.json")["kind"], "block");
    assert_eq!(cli(&doc, &["object", "new", "zz-outfit", "--extends", "pebble"]).0, 0);
    assert_eq!(read_doc(&doc, "objects/zz-outfit.json"), json!({ "kind": "sprite", "extends": "pebble", "keys": {}, "rows": {} }));
    assert_eq!(cli(&doc, &["object", "new", "zz-x", "--kind", "block", "--width", "3"]).0, 2);
    let (code, _, err) = cli(&doc, &["object", "new", "zz-rock"]);
    assert_eq!(code, 1);
    assert!(err.contains("already an object or scene called zz-rock"), "{err}");
    assert_eq!(cli(&doc, &["object", "new", "Bad_Name"]).0, 2);
}

#[test]
fn object_paint_object_erase_object_fill_object_pick() {
    let doc = cli_doc();
    let mut d = edit(&doc, &["object", "paint", "zz-sprite", "2,0", "3,2", "--color", "grass-2"], FILE);
    assert_eq!(d["keys"], json!({ "o": "ink", "g": "grass-2" }));
    assert_eq!(d["layers"][0]["map"], json!(["oog.", "o...", "...g"]));
    d = edit(&doc, &["object", "erase", "zz-sprite", "--pixels", "0,0 1,0"], FILE);
    assert_eq!(d["layers"][0]["map"][0], "..g.");
    d = edit(&doc, &["object", "fill", "zz-sprite", "--x", "1", "--y", "1", "--color", "grass-2"], FILE);
    assert_eq!(d["layers"][0]["map"], json!(["gggg", "oggg", "gggg"]));
    d = edit(&doc, &["object", "paint", "zz-sprite", "0,0", "--color", "ink", "--layer", "1", "--frame", "2"], FILE);
    assert_eq!(d["layers"][1]["frames"][2][0], "o...");
    let (_, out, _) = cli(&doc, &["object", "pick", "zz-sprite", "--x", "0", "--y", "1", "--json"]);
    let pick: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(pick["color"], "ink");
    assert!(pick["hex"].is_string());
    assert_eq!(cli(&doc, &["object", "paint", "zz-sprite", "0,0", "--color", "teal-1"]).0, 1);
    assert_eq!(cli(&doc, &["object", "paint", "zz-sprite", "0,0", "--color", "no-such-color"]).0, 2);
}

#[test]
fn object_layer_add_duplicate_move_delete() {
    let doc = cli_doc();
    let mut d = edit(&doc, &["object", "layer", "add", "zz-sprite"], FILE);
    assert_eq!(d["layers"].as_array().unwrap().len(), 3);
    assert_eq!(d["layers"][2]["map"], json!(["....", "....", "...."]));
    d = edit(&doc, &["object", "layer", "duplicate", "zz-sprite", "0"], FILE);
    assert_eq!(d["layers"].as_array().unwrap().len(), 4);
    assert_eq!(d["layers"][1], d["layers"][0]);
    d = edit(&doc, &["object", "layer", "move", "zz-sprite", "3", "--by", "-3"], FILE);
    assert_eq!(d["layers"][0]["map"], json!(["....", "....", "...."]));
    d = edit(&doc, &["object", "layer", "delete", "zz-sprite", "0"], FILE);
    assert_eq!(d["layers"].as_array().unwrap().len(), 3);
    assert_eq!(cli(&doc, &["object", "new", "zz-outfit", "--extends", "zz-sprite"]).0, 0);
    assert_eq!(cli(&doc, &["object", "layer", "add", "zz-outfit"]).0, 1, "an outfit takes its layers from its base");
}

#[test]
fn object_frame_add_duplicate_move_delete() {
    let doc = cli_doc();
    let l = "1";
    let mut d = edit(&doc, &["object", "frame", "add", "zz-sprite", "--layer", l, "--frame", "0"], FILE);
    assert_eq!(d["layers"][1]["frames"].as_array().unwrap().len(), 5);
    assert_eq!(d["layers"][1]["frames"][1], json!(["....", "....", "...."]));
    d = edit(&doc, &["object", "frame", "duplicate", "zz-sprite", "--layer", l, "--frame", "0"], FILE);
    assert_eq!(d["layers"][1]["frames"][1], d["layers"][1]["frames"][0]);
    d = edit(&doc, &["object", "frame", "move", "zz-sprite", "--layer", l, "--frame", "2", "--by", "-2"], FILE);
    assert_eq!(d["layers"][1]["frames"][0], json!(["....", "....", "...."]));
    d = edit(&doc, &["object", "frame", "delete", "zz-sprite", "--layer", l, "--frame", "0"], FILE);
    assert_eq!(d["layers"][1]["frames"].as_array().unwrap().len(), 5);
    let (code, _, err) = cli(&doc, &["object", "frame", "add", "zz-sprite", "--layer", "0"]);
    assert_eq!(code, 1);
    assert!(err.contains("still map"), "{err}");
}

#[test]
fn object_animate_object_still_object_resize() {
    let doc = cli_doc();
    let mut d = edit(&doc, &["object", "animate", "zz-sprite", "--layer", "0", "--loop", "wf"], FILE);
    assert_eq!(d["layers"][0]["loop"], "wf");
    assert_eq!(d["layers"][0]["frames"].as_array().unwrap().len(), 5);
    d = edit(&doc, &["object", "animate", "zz-sprite", "--layer", "0", "--loop", "hf"], FILE);
    assert_eq!(
        (d["layers"][0]["loop"].clone(), d["layers"][0]["prefix"].clone(), d["layers"][0]["frames"].as_array().unwrap().len()),
        (json!("hf"), json!("h"), 6)
    );
    d = edit(&doc, &["object", "still", "zz-sprite", "--layer", "0", "--frame", "1"], FILE);
    assert!(d["layers"][0].get("map").is_some());
    d = edit(&doc, &["object", "resize", "zz-sprite", "--width", "6"], FILE);
    for l in d["layers"].as_array().unwrap() {
        let maps: Vec<&Value> = match l.get("frames") {
            Some(f) => f.as_array().unwrap().iter().collect(),
            None => vec![&l["map"]],
        };
        for map in maps {
            let widths: Vec<usize> = map.as_array().unwrap().iter().map(|r| r.as_str().unwrap().chars().count()).collect();
            assert_eq!(widths, vec![6, 6, 6]);
        }
    }
}

#[test]
fn object_footprint_sets_and_clears_and_refuses_blocks() {
    let doc = cli_doc();
    let d = edit(&doc, &["object", "footprint", "zz-sprite", "--to", "2,1"], FILE);
    assert_eq!(d["footprint"], json!([2, 1]));
    let d = edit(&doc, &["object", "footprint", "zz-sprite", "--clear"], FILE);
    assert!(d.get("footprint").is_none());
    assert_eq!(cli(&doc, &["object", "footprint", "block", "--to", "2,2"]).0, 2);
    assert_eq!(cli(&doc, &["object", "footprint", "zz-sprite"]).0, 2);
}

#[test]
fn an_edit_that_changes_nothing_writes_nothing() {
    let doc = cli_doc();
    let before = text(&doc, FILE);
    let (code, out, err) = cli(&doc, &["object", "erase", "zz-sprite", "99,99"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("nothing written"), "{out}");
    assert_eq!(text(&doc, FILE), before);
}
