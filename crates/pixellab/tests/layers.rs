//! Scene layers (ports cli/test/layers.test.mjs; docs/intents/2026-10-scene-layers/spec.md, R1–R5):
//! named, nestable parts of a scene, stored in the scene file as a `layers`
//! list and a `layer` on each item. They never change paint order or what's
//! published, they're apart from the { group } wrappers, and every action is a
//! command.
//!
//! The operations run directly on scene documents; the tests that go through
//! the layer commands, the CLI or a server are written and ignored until
//! those are ported.

#![allow(clippy::result_large_err)]

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{json, Map, Value};

use engine::engine::{load_sources, validate};
use engine::format::{read_docs, stringify};
use engine::serialize::ordered;
use pixellab::context::{Ctx, DocKind};
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::{CommandError, Error};
use pixellab::layer_ops as ops;
use pixellab::layer_ops::NewLayer;
use pixellab::registry::registry;
use pixellab::scene_ops;
use pixellab::session::{Mode, Options, Session};

// Two ground cubes in entrance wrappers whose data-* point at them, two
// pebbles on top, an animated pebble and one placed by pixel.
fn yard() -> Value {
    let wrapper = |c: i64| {
        json!({
            "group": { "class": "enter", "data-enter": "ground", "data-col": c.to_string(), "data-row": "0", "data-x": (c * 16).to_string(), "data-y": (c * 8).to_string() },
            "items": [{ "object": "block", "at": { "tile": [c, 0, 0] } }],
        })
    };
    json!({
        "output": "yard.svg",
        "viewBox": [-48, -32, 128, 96],
        "items": [
            wrapper(0),
            wrapper(1),
            { "object": "pebble", "at": { "tile": [0, 0, 1] } },
            { "object": "pebble", "id": "rock", "at": { "tile": [1, 0, 1] } },
            { "object": "pebble", "at": { "px": [8, 20] } },
        ],
        "motion": { "duration": 1000, "tracks": [{ "item": "rock", "keys": [{ "t": 0 }, { "t": 500, "move": [2, 0] }] }] },
    })
}

/// `yard` with layers: Ground (the cubes), Stones (the pebbles) and Rock inside Stones.
fn layered() -> Value {
    let mut s = yard();
    let mut map = Map::new();
    for (k, v) in s.as_object().unwrap() {
        if k == "items" {
            map.insert(
                "layers".into(),
                json!([{ "id": "ground", "name": "Ground" }, { "id": "stones", "name": "Stones" }, { "id": "rock", "name": "Rock", "parent": "stones" }]),
            );
        }
        map.insert(k.clone(), v.clone());
    }
    s = Value::Object(map);
    s["items"][0]["items"][0] = json!({ "object": "block", "layer": "ground", "at": { "tile": [0, 0, 0] } });
    s["items"][1]["items"][0] = json!({ "object": "block", "layer": "ground", "at": { "tile": [1, 0, 0] } });
    s["items"][2] = json!({ "object": "pebble", "layer": "stones", "at": { "tile": [0, 0, 1] } });
    s["items"][3] = json!({ "object": "pebble", "id": "rock", "layer": "rock", "at": { "tile": [1, 0, 1] } });
    s
}

fn sample_dir() -> std::path::PathBuf {
    common::fixture()
}

/// The engine's problems for a scene given as data, next to the sample document's palette and objects.
fn problems(data: Value) -> Vec<String> {
    let docs = read_docs(&sample_dir());
    let objects = BTreeMap::from_iter(docs.objects.clone());
    let scenes = BTreeMap::from([("s".to_string(), data)]);
    let all = load_sources(docs.palette.clone().unwrap_or(Value::Null), objects, scenes);
    validate(&all).into_iter().filter_map(|p| p.strip_prefix("scenes/s.json: ").map(str::to_string)).collect()
}

fn scene(extra: Value) -> Value {
    let mut s = json!({ "viewBox": [0, 0, 16, 16], "items": [{ "object": "pebble", "at": { "px": [8, 8] } }] });
    for (k, v) in extra.as_object().unwrap() {
        s[k] = v.clone();
    }
    s
}

fn code(e: CommandError) -> String {
    e.code
}

/// The objects and placements in paint order, and the wrappers' attributes: what layers must never change.
fn paint_order(scene: &Value) -> String {
    fn strip(v: &Value) -> Value {
        match v {
            Value::Object(m) => {
                Value::Object(m.iter().filter(|(k, _)| k.as_str() != "layer").map(|(k, v)| (k.clone(), strip(v))).collect())
            }
            Value::Array(a) => Value::Array(a.iter().map(strip).collect()),
            other => other.clone(),
        }
    }
    stringify(&strip(&scene["items"]))
}

fn p(path: &[usize]) -> Vec<usize> {
    path.to_vec()
}

fn layer_ids(doc: &Value) -> Vec<String> {
    ops::layers_of(doc).iter().map(|l| l["id"].as_str().unwrap().to_string()).collect()
}

// --- the scene format (R1) ---

#[test]
fn r1_a_scene_with_nested_layers_and_items_in_them_is_valid() {
    let s = scene(json!({
        "layers": [{ "id": "yard", "name": "Yard" }, { "id": "crates", "name": "Crates and boxes", "parent": "yard" }],
        "items": [{ "object": "pebble", "layer": "crates", "at": { "px": [8, 8] } }],
    }));
    assert_eq!(problems(s), Vec::<String>::new());
}

#[test]
fn r1_the_engine_refuses_malformed_layers_and_items_in_missing_layers() {
    let one = |text: &str| vec![text.to_string()];
    assert_eq!(problems(scene(json!({ "layers": "yard" }))), one("layers must be a list of { id, name, parent }"));
    assert_eq!(
        problems(scene(json!({ "layers": [{ "id": "yard", "name": "Yard", "color": "red" }] }))),
        one("layers[0]: unknown property 'color'")
    );
    assert_eq!(problems(scene(json!({ "layers": [{ "id": "Yard", "name": "Yard" }] }))), one("layers[0]: id must be lowercase kebab-case"));
    assert_eq!(
        problems(scene(json!({ "layers": [{ "id": "yard", "name": "Yard" }, { "id": "yard", "name": "Two" }] }))),
        one("layers[1]: id 'yard' is already used by another layer")
    );
    let bad_name = one("layers[0]: name must be 1 to 64 characters on one line, without spaces at either end");
    for name in [json!(""), json!(" Yard"), json!("Yard "), json!("Two\nlines"), json!("x".repeat(65)), json!(7)] {
        assert_eq!(problems(scene(json!({ "layers": [{ "id": "yard", "name": name.clone() }] }))), bad_name, "{name}");
    }
    assert_eq!(problems(scene(json!({ "layers": [{ "id": "yard", "name": "x".repeat(64) }] }))), Vec::<String>::new());
    assert_eq!(
        problems(scene(json!({ "layers": [{ "id": "yard", "name": "Yard", "parent": "barn" }] }))),
        one("layers[0]: parent 'barn' is no layer's id")
    );
    let cycle = |id: &str| one(&format!("layers: layer '{id}' is inside itself (a layer's parents can't come back to it)"));
    assert_eq!(
        problems(scene(json!({ "layers": [{ "id": "a", "name": "A", "parent": "b" }, { "id": "b", "name": "B", "parent": "a" }] }))),
        cycle("a")
    );
    assert_eq!(problems(scene(json!({ "layers": [{ "id": "a", "name": "A", "parent": "a" }] }))), cycle("a"));
    assert_eq!(
        problems(scene(json!({ "items": [{ "object": "pebble", "layer": "yard", "at": { "px": [8, 8] } }] }))),
        one("items[0]: layer 'yard' is no layer of this scene")
    );
    assert_eq!(
        problems(scene(json!({ "layers": [], "items": [{ "object": "pebble", "layer": 3, "at": { "px": [8, 8] } }] }))),
        one("items[0]: layer '3' is no layer of this scene")
    );
}

#[test]
fn r1_layers_serialize_before_items_one_per_line_and_an_items_layer_before_at_the_source_round_trips() {
    // The source is JSON now: the canonical key order is what the serializer writes.
    let s = layered();
    let map = ordered(s.as_object().unwrap());
    let keys: Vec<&String> = map.keys().collect();
    let layers_at = keys.iter().position(|k| k.as_str() == "layers").unwrap();
    let items_at = keys.iter().position(|k| k.as_str() == "items").unwrap();
    assert!(layers_at + 1 == items_at, "layers come right before items: {keys:?}");
    assert_eq!(stringify(&Value::Object(map)), stringify(&s), "the source round-trips");
    let rock: Vec<&String> = s["items"][3].as_object().unwrap().keys().collect();
    assert_eq!(rock, ["object", "id", "layer", "at"]);
}

#[test]
#[ignore = "needs U23"]
fn r1_scene_items_lists_the_layers_what_each_holds_layers_inside_included_and_each_items_layer() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = call(&mut s, "scene.items", json!({}), "layered").unwrap();
    assert_eq!(
        r["layers"],
        json!([
            { "id": "ground", "name": "Ground", "items": 2 },
            { "id": "stones", "name": "Stones", "items": 2 },
            { "id": "rock", "name": "Rock", "parent": "stones", "items": 1 },
        ])
    );
    let listed: Vec<Value> =
        r["items"].as_array().unwrap().iter().map(|i| json!([i["path"], i.get("layer").cloned().unwrap_or(Value::Null)])).collect();
    assert_eq!(
        listed,
        [json!(["0.0", "ground"]), json!(["1.0", "ground"]), json!(["2", "stones"]), json!(["3", "rock"]), json!(["4", null])]
    );
    let shown = call(&mut s, "scene.show", json!({}), "layered").unwrap();
    assert_eq!([shown["layers"].clone(), shown["layerNames"].clone()], [json!(3), json!(["Ground", "Stones", "Rock"])]);
}

// --- merging a scene with layers (R1, D1) ---

fn merging() -> Value {
    let mut s = yard();
    s.as_object_mut().unwrap().shift_remove("output");
    s
}

fn merging_layered() -> Value {
    let mut s = layered();
    s.as_object_mut().unwrap().shift_remove("output");
    s
}

/// Another session's edit, saved to the file.
fn on_disk(fx: &Fx, command: &str, params: Value) {
    let mut other = fx.session();
    call(&mut other, command, params, "merging").unwrap();
    let doc = other.doc(DocKind::Scene, "merging").unwrap();
    std::fs::write(fx.doc.root.join("scenes/merging.json"), engine::format::stringify_pretty(&doc)).unwrap();
}

fn merge(s: &mut Session) -> Result<Value, Error> {
    let mut ctx = Ctx::new(s);
    registry().call(&mut ctx, "doc.merge", json!({ "doc": "scene:merging" }), false)
}

fn working(s: &mut Session, name: &str) -> Value {
    s.doc(DocKind::Scene, name).unwrap()
}

#[test]
#[ignore = "needs U23"]
fn r1_when_the_open_copy_and_the_file_each_make_a_scenes_first_layers_a_merge_keeps_both_lists() {
    let fx = Fx::new();
    fx.write_scene("merging", &merging());
    let mut mine = fx.session();
    call(&mut mine, "scene.layer.new", json!({ "items": ["0.0"] }), "merging").unwrap();
    on_disk(&fx, "scene.layer.new", json!({ "layerName": "Sky", "items": ["4"] }));
    assert_eq!(merge(&mut mine).unwrap()["merged"], json!(true));
    let scene = working(&mut mine, "merging");
    assert_eq!(scene["layers"], json!([{ "id": "group-1", "name": "Group 1" }, { "id": "sky", "name": "Sky" }]));
    assert_eq!([scene["items"][0]["items"][0]["layer"].clone(), scene["items"][4]["layer"].clone()], [json!("group-1"), json!("sky")]);
}

#[test]
#[ignore = "needs U23"]
fn r1_a_layer_id_both_sides_made_for_different_layers_clashes_and_nothing_merges() {
    let fx = Fx::new();
    fx.write_scene("merging", &merging());
    let mut mine = fx.session();
    call(&mut mine, "scene.layer.new", json!({ "layerName": "Trees", "id": "trees", "items": ["0.0"] }), "merging").unwrap();
    on_disk(&fx, "scene.layer.new", json!({ "layerName": "Rocks", "id": "trees", "items": ["4"] }));
    let before = stringify(&working(&mut mine, "merging"));
    match merge(&mut mine) {
        Err(Error::Command(e)) => assert!(e.code == "merge_conflict" && e.message.contains("added layer trees"), "{e:?}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(stringify(&working(&mut mine, "merging")), before);
}

#[test]
#[ignore = "needs U23"]
fn r1_two_different_layers_both_sides_made_under_the_same_default_name_clash_and_nothing_merges() {
    for start in [merging(), merging_layered()] {
        let fx = Fx::new();
        fx.write_scene("merging", &start);
        let mut mine = fx.session();
        call(&mut mine, "scene.layer.new", json!({ "items": ["0.0"] }), "merging").unwrap();
        on_disk(&fx, "scene.layer.new", json!({ "items": ["4"] }));
        let before = stringify(&working(&mut mine, "merging"));
        match merge(&mut mine) {
            Err(Error::Command(e)) => {
                assert!(e.code == "merge_conflict" && e.message.contains("made layer group-1, holding different items"), "{e:?}")
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(stringify(&working(&mut mine, "merging")), before);
    }
}

#[test]
#[ignore = "needs U23"]
fn r1_the_same_layer_made_on_both_sides_the_same_change_twice_still_merges() {
    let fx = Fx::new();
    fx.write_scene("merging", &merging());
    let mut mine = fx.session();
    call(&mut mine, "scene.layer.new", json!({ "items": ["0.0"] }), "merging").unwrap();
    call(&mut mine, "scene.move", json!({ "item": "2", "by": [1, 0] }), "merging").unwrap();
    on_disk(&fx, "scene.layer.new", json!({ "items": ["0.0"] }));
    assert_eq!(merge(&mut mine).unwrap()["merged"], json!(true));
    let scene = working(&mut mine, "merging");
    assert_eq!(scene["layers"], json!([{ "id": "group-1", "name": "Group 1" }]));
    assert_eq!(
        [scene["items"][0]["items"][0]["layer"].clone(), scene["items"][2]["at"].clone()],
        [json!("group-1"), json!({ "tile": [1, 0, 1] })]
    );
}

#[test]
#[ignore = "needs U23"]
fn r1_a_merge_that_would_leave_an_item_in_a_layer_neither_side_kept_is_refused() {
    let fx = Fx::new();
    fx.write_scene("merging", &merging_layered());
    let mut mine = fx.session();
    call(&mut mine, "scene.layer.assign", json!({ "to": "stones", "items": ["4"] }), "merging").unwrap();
    on_disk(&fx, "scene.layer.ungroup", json!({ "layer": "stones" }));
    let before = stringify(&working(&mut mine, "merging"));
    match merge(&mut mine) {
        Err(Error::Command(e)) => assert!(e.code == "merge_invalid" && e.message.contains("layer 'stones' is no layer"), "{e:?}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(stringify(&working(&mut mine, "merging")), before);
}

// --- paint order and output (R2) ---

#[test]
#[ignore = "needs U28"]
fn r2_art_build_of_a_document_with_layers_is_byte_identical_to_the_same_document_without_them() {
    let strip = |mut s: Value| {
        s.as_object_mut().unwrap().shift_remove("layers");
        fn items(v: &mut Value) {
            match v {
                Value::Object(m) => {
                    m.shift_remove("layer");
                    m.values_mut().for_each(items);
                }
                Value::Array(a) => a.iter_mut().for_each(items),
                _ => {}
            }
        }
        items(&mut s);
        s
    };
    assert_eq!(stringify(&strip(layered())), stringify(&yard()), "the stripped scene is the scene without layers");
    let mut outs: Vec<BTreeMap<String, Vec<u8>>> = Vec::new();
    for scene in [strip(layered()), layered()] {
        let fx = Fx::new();
        fx.write_scene("layered", &scene);
        let out = fx.doc.base.join("out");
        let r = cli(&fx, &["art", "build", "--dir", out.to_str().unwrap(), "--document", fx.doc.root.to_str().unwrap()]);
        assert_eq!(r.0, 0, "{}", r.2);
        let mut files = BTreeMap::new();
        for e in std::fs::read_dir(&out).unwrap().flatten() {
            files.insert(e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap());
        }
        outs.push(files);
    }
    assert_eq!(outs[1].keys().cloned().collect::<Vec<_>>(), ["sample.svg", "yard.svg"]);
    for (file, bytes) in &outs[0] {
        assert!(bytes == &outs[1][file], "{file} differs");
    }
}

#[test]
#[ignore = "needs U23"]
fn r2_scene_render_draws_the_same_png_with_and_without_layers() {
    let fx = Fx::new();
    let mut s = fx.session();
    let a = call(&mut s, "scene.render", json!({ "scale": 2 }), "yard").unwrap();
    let b = call(&mut s, "scene.render", json!({ "scale": 2 }), "layered").unwrap();
    assert_eq!(a["png"], b["png"]);
    assert_eq!(a["items"], b["items"]);
}

#[test]
fn r2_making_renaming_assigning_and_ungrouping_layers_never_reorder_or_move_items() {
    let mut s = yard();
    let before = paint_order(&s);
    ops::new_layer(&mut s, NewLayer { name: Some(json!("Ground")), items: vec![p(&[0, 0]), p(&[1, 0])], ..Default::default() }).unwrap();
    ops::new_layer(&mut s, NewLayer { name: Some(json!("Stones")), items: vec![p(&[2]), p(&[3]), p(&[4])], ..Default::default() }).unwrap();
    ops::new_layer(&mut s, NewLayer { items: vec![p(&[3])], ..Default::default() }).unwrap();
    ops::rename_layer(&mut s, "Group 1", &json!("Rock")).unwrap();
    ops::assign_layer(&mut s, Some("ground"), &[p(&[4])], &[]).unwrap();
    ops::assign_layer(&mut s, None, &[], &["group-1".to_string()]).unwrap();
    assert_eq!(paint_order(&s), before);
    for id in ["group-1", "stones", "ground"] {
        ops::ungroup_layer(&mut s, id).unwrap();
    }
    assert_eq!(paint_order(&s), before);
}

#[test]
#[ignore = "needs U28"]
fn r2_depth_order_r4_a_layer_edit_on_a_scene_whose_file_is_out_of_depth_order_sorts_it_and_publishes_the_same_bytes() {
    // The two ground cubes listed the wrong way round: the cube at 1,0 stands in front of the one at 0,0 but is listed first.
    let mut swapped = yard();
    let items = swapped["items"].as_array_mut().unwrap();
    items.swap(0, 1);
    let fx = Fx::new();
    fx.write_scene("yard", &swapped);
    let out = fx.doc.base.join("out");
    let build = || {
        let r = cli(&fx, &["art", "build", "--dir", out.to_str().unwrap(), "--document", fx.doc.root.to_str().unwrap()]);
        assert_eq!(r.0, 0, "{}", r.2);
        std::fs::read(out.join("yard.svg")).unwrap()
    };
    let before = build();
    let made = cli(
        &fx,
        &["scene", "layer", "new", "yard", "--layer-name", "Ground", "--items", "0.0,1.0", "--document", fx.doc.root.to_str().unwrap()],
    );
    assert_eq!(made.0, 0, "{}", made.2);
    let text = std::fs::read_to_string(fx.doc.root.join("scenes/yard.json")).unwrap();
    let doc: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(doc["items"][0]["items"][0]["at"]["tile"], json!([0, 0, 0]), "the edit sorted the scene: the cube behind is listed first");
    assert!(build() == before, "and the SVG is the same bytes: the engine draws in depth order whatever the file lists");
}

#[test]
fn r2_ungrouping_the_last_layer_takes_the_layers_list_away_so_the_file_is_what_it_was() {
    let mut s = yard();
    ops::new_layer(&mut s, NewLayer { name: Some(json!("Ground")), items: vec![p(&[0, 0]), p(&[1, 0])], ..Default::default() }).unwrap();
    ops::ungroup_layer(&mut s, "ground").unwrap();
    assert!(s.get("layers").is_none());
    assert_eq!(stringify(&s), stringify(&yard()));
}

// --- wrappers (R3) ---

#[test]
fn r3_layer_commands_leave_the_group_wrappers_alone_and_scene_group_set_works_on_a_layered_scene() {
    let mut s = layered();
    let groups = |s: &Value| -> Vec<Value> { s["items"].as_array().unwrap().iter().filter_map(|i| i.get("group").cloned()).collect() };
    let before = groups(&s);
    ops::new_layer(&mut s, NewLayer { layers: vec!["ground".into(), "stones".into()], ..Default::default() }).unwrap();
    ops::ungroup_layer(&mut s, "stones").unwrap();
    assert_eq!(groups(&s), before);
    let attrs = json!({ "data-enter": "object" });
    let r = scene_ops::set_group_attrs(&mut s, &[0], attrs.as_object().unwrap()).unwrap();
    assert_eq!(r["group"]["group"]["data-enter"], json!("object"));
    assert_eq!(s["items"][0]["items"][0]["layer"], json!("ground"));
}

#[test]
#[ignore = "needs U23"]
fn r3_moving_a_layer_keeps_each_items_wrapper_data_x_data_y_and_data_col_data_row_in_step_no_keep_group_leaves_them() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = call(&mut s, "scene.layer.move", json!({ "layer": "ground", "by": [1, 0] }), "layered").unwrap();
    assert_eq!(r["moved"], json!(2));
    let items = working(&mut s, "layered")["items"].clone();
    assert_eq!(
        items[0]["group"],
        json!({ "class": "enter", "data-enter": "ground", "data-col": "1", "data-row": "0", "data-x": "16", "data-y": "8" })
    );
    assert_eq!(
        items[1]["group"],
        json!({ "class": "enter", "data-enter": "ground", "data-col": "2", "data-row": "0", "data-x": "32", "data-y": "16" })
    );
    call(&mut s, "scene.layer.move", json!({ "layer": "ground", "by": [0, 1], "keepGroup": false }), "layered").unwrap();
    assert_eq!(working(&mut s, "layered")["items"][0]["group"]["data-row"], json!("0"));
    assert_eq!(working(&mut s, "layered")["items"][0]["items"][0]["at"], json!({ "tile": [1, 1, 0] }));
}

// --- existing documents (R4) ---

fn sample_scene() -> Value {
    read_docs(&sample_dir()).scenes["sample"].clone()
}

#[test]
fn r4_a_scene_without_layers_lists_none_and_every_item_is_at_the_top_level() {
    let s = sample_scene();
    assert!(ops::layers_of(&s).is_empty());
    assert!(ops::placed_items(&s).iter().all(|i| i.item.get("layer").is_none()));
}

#[test]
fn r4_editing_a_scene_without_layers_writes_nothing_about_layers() {
    let mut s = sample_scene();
    scene_ops::place(&mut s, "pebble", &json!({ "tile": [2, 2, 1] }));
    scene_ops::move_item(&mut s, &[0], 1.0, 0.0, 0.0, 0.0).unwrap();
    scene_ops::set_class(&mut s, &[1], "sway").unwrap();
    assert!(!stringify(&s).contains("layer"));
}

// --- commands (R5) ---

#[test]
fn r5_scene_layer_new_names_layers_group_1_group_2_and_makes_ids_from_names() {
    let mut s = yard();
    let a = ops::new_layer(&mut s, NewLayer { items: vec![p(&[0, 0])], ..Default::default() }).unwrap();
    let b = ops::new_layer(&mut s, NewLayer { items: vec![p(&[1, 0])], ..Default::default() }).unwrap();
    assert_eq!(
        [a["layer"].clone(), b["layer"].clone()],
        [json!({ "id": "group-1", "name": "Group 1" }), json!({ "id": "group-2", "name": "Group 2" })]
    );
    assert_eq!(a["message"], json!("Grouped 1 item as Group 1"));
    ops::rename_layer(&mut s, "group-2", &json!("Ground")).unwrap();
    assert_eq!(
        ops::new_layer(&mut s, NewLayer::default()).unwrap()["layer"]["name"],
        json!("Group 3"),
        "one more than the highest Group n, renamed ones (group-2) counted"
    );
    let named =
        |s: &mut Value, n: &str| ops::new_layer(s, NewLayer { name: Some(json!(n)), ..Default::default() }).unwrap()["layer"]["id"].clone();
    assert_eq!(named(&mut s, "Ground"), json!("ground"));
    assert_eq!(named(&mut s, "Ground"), json!("ground-2"));
    assert_eq!(named(&mut s, "3 Trees"), json!("layer-3-trees"));
    assert_eq!(code(ops::new_layer(&mut s, NewLayer { id: Some("ground".into()), ..Default::default() }).unwrap_err()), "exists");
    assert_eq!(code(ops::new_layer(&mut s, NewLayer { name: Some(json!(" ")), ..Default::default() }).unwrap_err()), "usage");
}

#[test]
fn r5_a_new_layer_goes_under_the_members_common_parent_a_given_parent_or_the_top_level() {
    let mut s = layered();
    let in_stones = ops::new_layer(&mut s, NewLayer { items: vec![p(&[2])], layers: vec!["rock".into()], ..Default::default() }).unwrap();
    assert_eq!(in_stones["layer"]["parent"], json!("stones"), "an item in Stones and a layer in Stones");
    let top = ops::new_layer(&mut s, NewLayer { items: vec![p(&[0, 0]), p(&[4])], ..Default::default() }).unwrap();
    assert_eq!(top["layer"].get("parent"), None, "Ground and the top level share only the top level");
    let given =
        ops::new_layer(&mut s, NewLayer { name: Some(json!("Inner")), parent: Some(Some("ground".into())), ..Default::default() }).unwrap();
    assert_eq!(given["layer"]["parent"], json!("ground"));
    let err = ops::new_layer(&mut s, NewLayer { layers: vec!["ground".into()], parent: Some(Some("inner".into())), ..Default::default() })
        .unwrap_err();
    assert_eq!(code(err), "layer_cycle");
    assert_eq!(
        layer_ids(&s),
        ["group-2", "ground", "stones", "group-1", "rock", "inner"],
        "each goes before its first member layer, else before the first layer beside it, else last"
    );
}

#[test]
fn r5_a_member_inside_another_member_stays_where_it_is() {
    let mut s = layered();
    let r = ops::new_layer(
        &mut s,
        NewLayer { layers: vec!["stones".into(), "rock".into()], items: vec![p(&[3]), p(&[2])], ..Default::default() },
    )
    .unwrap();
    assert_eq!([r["layers"].clone(), r["items"].clone()], [json!(["stones"]), json!(0)]);
    let rock = ops::find_layer(&s, "rock").unwrap();
    assert_eq!(rock["parent"], json!("stones"));
    assert_eq!(s["items"][3]["layer"], json!("rock"));
}

#[test]
fn r5_scene_layer_ungroup_moves_what_a_layer_held_up_one_level() {
    let mut s = layered();
    let r = ops::ungroup_layer(&mut s, "stones").unwrap();
    assert_eq!([r["parent"].clone(), r["layers"].clone(), r["items"].clone()], [Value::Null, json!(["rock"]), json!(["2"])]);
    let pairs: Vec<Value> = ops::layers_of(&s).iter().map(|l| json!([l["id"], l.get("parent").cloned().unwrap_or(Value::Null)])).collect();
    assert_eq!(pairs, [json!(["ground", null]), json!(["rock", null])]);
    assert!(s["items"][2].get("layer").is_none());
    ops::new_layer(&mut s, NewLayer { name: Some(json!("Outer")), layers: vec!["rock".into()], ..Default::default() }).unwrap();
    ops::new_layer(&mut s, NewLayer { name: Some(json!("Middle")), layers: vec!["rock".into()], ..Default::default() }).unwrap();
    let inner = ops::ungroup_layer(&mut s, "middle").unwrap();
    assert_eq!(inner["parent"], json!("outer"));
    assert_eq!(ops::find_layer(&s, "rock").unwrap()["parent"], json!("outer"));
}

#[test]
fn r5_scene_layer_assign_moves_items_and_layers_into_a_layer_or_the_top_level_and_refuses_cycles() {
    let mut s = layered();
    let to = ops::find_layer(&s, "Rock").unwrap()["id"].as_str().unwrap().to_string();
    let r = ops::assign_layer(&mut s, Some(&to), &[p(&[4]), p(&[0, 0])], &[]).unwrap();
    assert_eq!(r["message"], json!("Moved 2 items into Rock"));
    ops::assign_layer(&mut s, None, &[], &["rock".to_string()]).unwrap();
    assert_eq!([s["items"][4]["layer"].clone(), s["items"][0]["items"][0]["layer"].clone()], [json!("rock"), json!("rock")]);
    assert_eq!(ops::find_layer(&s, "rock").unwrap().get("parent"), None);
    ops::assign_layer(&mut s, Some("stones"), &[], &["rock".to_string()]).unwrap();
    assert_eq!(code(ops::assign_layer(&mut s, Some("rock"), &[], &["stones".to_string()]).unwrap_err()), "layer_cycle");
    assert_eq!(code(ops::assign_layer(&mut s, Some("stones"), &[], &["stones".to_string()]).unwrap_err()), "layer_cycle");
    assert_eq!(code(ops::find_layer_index(&s, "barn").unwrap_err()), "not_found");
}

#[test]
#[ignore = "needs U23"]
fn r5_scene_layer_move_moves_every_item_in_the_layers_and_inside_them_and_the_items_once_in_one_undo_step() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = call(&mut s, "scene.layer.move", json!({ "layer": "stones", "items": ["2", "4"], "by": [1, -1] }), "layered").unwrap();
    assert_eq!(
        [r["moved"].clone(), r["by"].clone(), r["summary"].clone()],
        [json!(3), json!([1, -1, 0]), json!("Moved 3 items +1 col, -1 row")]
    );
    let scene = working(&mut s, "layered");
    let ats: Vec<Value> = scene["items"].as_array().unwrap()[2..].iter().map(|i| i["at"].clone()).collect();
    assert_eq!(ats, [json!({ "tile": [1, -1, 1] }), json!({ "tile": [2, -1, 1] }), json!({ "px": [8 + 32, 20] })]);
    assert_eq!(
        scene["motion"]["tracks"][0]["keys"][1]["move"],
        json!([2, 0]),
        "an animated item's keys are offsets: its motion moves with it"
    );
    {
        let mut ctx = Ctx::new(&mut s);
        registry().call(&mut ctx, "doc.undo", json!({ "doc": "scene:layered" }), false).unwrap();
    }
    assert_eq!(working(&mut s, "layered")["items"][2]["at"], json!({ "tile": [0, 0, 1] }));
    let level = call(&mut s, "scene.layer.move", json!({ "layer": "Ground", "by": [0, 0, 1] }), "layered").unwrap();
    assert_eq!(level["summary"], json!("Moved Ground 0 col, 0 row, +1 level; 2 groups followed"));
}

#[test]
#[ignore = "needs U22"]
fn r5_scene_place_layer_puts_the_new_item_in_a_layer_an_unknown_layer_places_nothing() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = call(&mut s, "scene.place", json!({ "object": "pebble", "at": [2, 0, 1], "layer": "Rock" }), "layered").unwrap();
    assert_eq!(r["item"]["layer"], json!("rock"));
    assert!(r["summary"].as_str().unwrap().ends_with(" in Rock"));
    let count = working(&mut s, "layered")["items"].as_array().unwrap().len();
    let err = call(&mut s, "scene.place", json!({ "object": "pebble", "at": [2, 2, 1], "layer": "barn" }), "layered").unwrap_err();
    assert_eq!(err_code(err), "not_found");
    assert_eq!(working(&mut s, "layered")["items"].as_array().unwrap().len(), count);
    let dup = call(&mut s, "scene.duplicate", json!({ "item": r["path"] }), "layered").unwrap();
    assert_eq!(dup["item"]["layer"], json!("rock"), "a duplicate stays in its layer");
    call(&mut s, "scene.set", json!({ "item": r["path"], "class": "sway", "id": "new-stone" }), "layered").unwrap();
    let index: usize = r["path"].as_str().unwrap().parse().unwrap();
    let set = working(&mut s, "layered")["items"][index].clone();
    let keys: Vec<&String> = set.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["object", "id", "class", "layer", "at"], "class and id edits keep the layer, in order");
}

#[test]
#[ignore = "needs U22"]
fn r5_scene_render_hide_leaves_hidden_layers_out_focus_adds_the_layers_own_render() {
    let fx = Fx::new();
    let mut s = fx.session();
    let all = call(&mut s, "scene.render", json!({}), "layered").unwrap();
    let hidden = call(&mut s, "scene.render", json!({ "hide": ["stones"] }), "layered").unwrap();
    let paths: Vec<Value> = hidden["items"].as_array().unwrap().iter().map(|i| i["path"].clone()).collect();
    assert_eq!(paths, [json!("0.0"), json!("1.0"), json!("4")], "Stones and Rock inside it");
    assert_ne!(hidden["png"], all["png"]);
    let focused = call(&mut s, "scene.render", json!({ "focus": "rock" }), "layered").unwrap();
    assert_eq!(focused["png"], all["png"]);
    // The node test also decodes focusPng and checks its opaque box equals the rock's bounds; that needs a base64 decoder.
    assert!(focused["focusPng"].is_string());
    assert!(all.get("focusPng").is_none());
    let err = call(&mut s, "scene.render", json!({ "focus": "barn" }), "layered").unwrap_err();
    assert_eq!(err_code(err), "not_found");
}

#[test]
#[ignore = "needs U28"]
fn r5_the_cli_runs_the_layer_commands_and_writes_the_scene() {
    let fx = Fx::new();
    fx.write_scene("yard", &yard());
    let root = fx.doc.root.to_str().unwrap();
    let made = cli(&fx, &["scene", "layer", "new", "yard", "--layer-name", "Ground", "--items", "0.0,1.0", "--document", root]);
    assert_eq!(made.0, 0, "{}", made.2);
    assert!(made.1.contains("Grouped 2 items as Ground"), "{}", made.1);
    let listed = cli(&fx, &["scene", "items", "yard", "--document", root]);
    assert!(
        listed
            .1
            .lines()
            .any(|l| l.starts_with("0.0") && l.contains("block") && l.contains("tile 0,0,0") && l.trim_end().ends_with("^ground")),
        "{}",
        listed.1
    );
    let text = std::fs::read_to_string(fx.doc.root.join("scenes/yard.json")).unwrap();
    let doc: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(doc["layers"], json!([{ "id": "ground", "name": "Ground" }]));
}

#[test]
#[ignore = "needs U28"]
fn r5_over_mcp_the_layer_commands_are_tools_and_their_edits_become_a_proposal() {
    use std::io::{BufRead, BufReader, Write};
    let fx = Fx::new();
    fx.write_scene("yard", &yard());
    let before = std::fs::read_to_string(fx.doc.root.join("scenes/yard.json")).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_pixellab"))
        .args(["serve", "--mcp", "--document", fx.doc.root.to_str().unwrap()])
        .envs(fx.doc.env())
        .env("PIXELLAB_STATE", &fx.state)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut id = 0;
    let mut request = |method: &str, params: Value| -> Value {
        id += 1;
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).unwrap();
        let mut line = String::new();
        out.read_line(&mut line).unwrap();
        serde_json::from_str::<Value>(&line).unwrap()["result"].clone()
    };
    request("initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } }));
    let tools = request("tools/list", json!({}));
    let names: Vec<&str> = tools["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for n in ["scene_layer_new", "scene_layer_rename", "scene_layer_ungroup", "scene_layer_assign", "scene_layer_move"] {
        assert!(names.contains(&n), "{n}");
    }
    let made = request(
        "tools/call",
        json!({ "name": "scene_layer_new", "arguments": { "name": "yard", "layerName": "Ground", "items": ["0.0", "1.0"] } }),
    );
    assert!(made.get("isError").is_none(), "{made}");
    request("tools/call", json!({ "name": "scene_layer_move", "arguments": { "name": "yard", "layer": "ground", "by": [1, 0] } }));
    let saved = request("tools/call", json!({ "name": "doc_save", "arguments": { "doc": "scene:yard" } }))["structuredContent"].clone();
    assert!(saved.get("proposal").is_some(), "a proposal");
    assert_eq!(saved["edits"], json!(["Grouped 2 items as Ground", "Moved Ground +1 col, 0 row; 2 groups followed"]));
    assert_eq!(
        std::fs::read_to_string(fx.doc.root.join("scenes/yard.json")).unwrap(),
        before,
        "the document is untouched until the owner approves"
    );
    let _ = child.kill();
    let _ = child.wait();
}

// --- helpers for the command-level tests ---

/// A temp copy of the sample document with the yard and layered scenes added, and a state folder.
struct Fx {
    doc: common::TempDocument,
    state: std::path::PathBuf,
}

impl Fx {
    fn new() -> Fx {
        let doc = common::make_document();
        let state = common::temp_dir("pixellab-state-");
        let fx = Fx { doc, state };
        fx.write_scene("yard", &yard());
        fx.write_scene("layered", &layered());
        fx
    }

    fn write_scene(&self, name: &str, scene: &Value) {
        self.doc.write(&format!("scenes/{name}.json"), &engine::format::stringify_pretty(scene));
    }

    fn session(&self) -> Session {
        let mut env = self.doc.env();
        env.insert("PIXELLAB_STATE".into(), self.state.to_string_lossy().into_owned());
        let document = open_document(&OpenOptions {
            doc: Some(self.doc.root.to_string_lossy().into_owned()),
            site: None,
            env: env.clone(),
            cwd: self.doc.root.clone(),
        })
        .unwrap();
        Session::new(document, Options { mode: Mode::Direct, env, agent: Some("test".into()), ..Options::default() })
    }
}

fn call(s: &mut Session, name: &str, params: Value, scene: &str) -> Result<Value, Error> {
    let mut p = json!({ "name": scene });
    for (k, v) in params.as_object().unwrap() {
        p[k] = v.clone();
    }
    let mut ctx = Ctx::new(s);
    registry().call(&mut ctx, name, p, false)
}

fn err_code(e: Error) -> String {
    match e {
        Error::Command(c) => c.code,
        Error::Internal(m) => m,
    }
}

/// Runs the program: (exit code, stdout, stderr).
fn cli(fx: &Fx, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_pixellab"))
        .args(args)
        .envs(fx.doc.env())
        .env("PIXELLAB_STATE", &fx.state)
        .current_dir(Path::new(&fx.doc.base))
        .output()
        .unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}
