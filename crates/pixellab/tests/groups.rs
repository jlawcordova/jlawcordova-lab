//! Groups (ports cli/test/groups.test.mjs; docs/intents/2026-10-mcp-gaps/spec.md,
//! R9–R11): a group's data-x/data-y and data-col/data-row follow the item they
//! point at, scene.group.set edits a group's attributes, and scene.place
//! places inside an existing or a new group. The scene is of entrance groups
//! like the hero island's, with a 4×4 sprite anchored at its top-left corner,
//! so its foot isn't its placement point.
//!
//! The tests that go through the commands and a session are written and
//! ignored until those units land; the ones named `ops:` run the same rules on
//! `scene_ops` directly.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use engine::engine::{load_sources, Sources};
use pixellab::errors::CommandError;
use pixellab::scene_ops as ops;

fn stone() -> Value {
    json!({
        "kind": "sprite",
        "anchor": [0, 0],
        "keys": { "a": "ink" },
        "layers": [{ "map": ["aaaa", "aaaa", "aaaa", "aaaa"] }],
    })
}

fn groups_scene() -> Value {
    json!({
        "viewBox": [-32, -16, 96, 64],
        "items": [
            {
                "group": { "class": "enter", "data-enter": "ground", "data-col": "1", "data-row": "0", "data-x": "16", "data-y": "8" },
                "items": [{ "object": "block", "at": { "tile": [1, 0, 0] } }],
            },
            {
                "group": { "class": "enter", "data-enter": "object", "data-x": "42", "data-y": "13" },
                "items": [{ "object": "stone", "id": "rock", "at": { "px": [40, 10] } }],
            },
            {
                "group": { "class": "enter", "data-enter": "object", "data-x": "99", "data-y": "99" },
                "items": [{ "object": "pebble", "at": { "px": [8, 30] } }],
            },
            { "object": "pebble", "at": { "tile": [3, 3, 0] } },
        ],
        "motion": { "duration": 1000, "tracks": [{ "item": "rock", "keys": [{ "t": 0 }] }] },
    })
}

/// The sample document's sources, with the stone and the groups scene added.
fn sources() -> Sources {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sample.pixellab");
    let docs = engine::format::read_docs(&dir);
    let mut objects = BTreeMap::from_iter(docs.objects.clone());
    objects.insert("stone".into(), stone());
    let mut scenes = BTreeMap::from_iter(docs.scenes.clone());
    scenes.insert("groups".into(), groups_scene());
    load_sources(docs.palette.clone().unwrap_or(Value::Null), objects, scenes)
}

fn attrs(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

fn group_attrs(doc: &Value, i: usize) -> &Value {
    &doc["items"][i]["group"]
}

/// Moves the item at `path` with `edit` and lets its group follow, as scene.move does.
fn moved(doc: &mut Value, path: &[usize], edit: impl FnOnce(&mut Value)) -> Option<Value> {
    let sources = sources();
    let before = ops::standing(&sources, doc, ops::placed_at(doc, path).unwrap());
    edit(ops::placed_at_mut(doc, path).unwrap());
    let after = ops::standing(&sources, doc, ops::placed_at(doc, path).unwrap());
    ops::follow_group(doc, path, &before, &after)
}

// ---- through the commands and a session (U14, U22) ----

mod common;

use pixellab::context::Ctx;
use pixellab::document::{open_document, OpenOptions};
use pixellab::session::Options;

/// A session over a temp copy of the sample document with the stone and the groups scene added.
struct Session {
    session: pixellab::session::Session,
    _doc: common::TempDocument,
}

fn session() -> Session {
    let doc = common::make_document();
    doc.write("objects/stone.json", &engine::format::stringify_pretty(&stone()));
    doc.write("scenes/groups.json", &engine::format::stringify_pretty(&groups_scene()));
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
    let session = pixellab::session::Session::new(document, Options { env, ..Options::default() });
    Session { session, _doc: doc }
}

/// Calls a command on the groups scene, as the Node suite's `call` does.
fn call(s: &mut Session, name: &str, params: Value) -> Result<Value, CommandError> {
    // `{ name: 'groups', ...params }`: the name goes first.
    let mut merged = Map::new();
    merged.insert("name".into(), json!("groups"));
    merged.extend(params.as_object().unwrap().clone());
    let params = Value::Object(merged);
    pixellab::registry::registry().call(&mut Ctx::new(&mut s.session), name, params, false).map_err(|e| match e {
        pixellab::errors::Error::Command(c) => c,
        pixellab::errors::Error::Internal(m) => CommandError::new(m, "internal"),
    })
}

fn groups(s: &mut Session) -> Vec<Value> {
    call(s, "scene.items", json!({})).unwrap()["groups"].as_array().unwrap().iter().map(|g| g["attrs"].clone()).collect()
}

#[test]
fn r9_a_cube_moved_by_tiles_takes_its_ground_groups_data_col_data_row_data_x_and_data_y_with_it() {
    let mut s = session();
    let r = call(&mut s, "scene.move", json!({ "item": "0.0", "by": [1, 0] })).unwrap();
    assert_eq!(
        r["group"],
        json!({ "path": "0", "attrs": { "class": "enter", "data-enter": "ground", "data-col": "2", "data-row": "0", "data-x": "32", "data-y": "16" } })
    );
    assert!(r["summary"].as_str().unwrap().ends_with("its group's data-col, data-row, data-x, data-y followed"));
}

#[test]
fn r9_a_sprite_placed_by_pixel_takes_its_groups_foot_with_it_scene_set_and_scene_snap_follow_too() {
    let mut s = session();
    let nudged = call(&mut s, "scene.move", json!({ "item": "rock", "nudge": [1, -1] })).unwrap();
    assert_eq!(
        (nudged["group"]["attrs"]["data-x"].clone(), nudged["group"]["attrs"]["data-y"].clone()),
        (json!("43"), json!("12")),
        "the foot: x − 0 + 4/2, y − 0 + 4 − 1"
    );
    let set = call(&mut s, "scene.set", json!({ "item": "rock", "px": [10, 20] })).unwrap();
    assert_eq!((set["group"]["attrs"]["data-x"].clone(), set["group"]["attrs"]["data-y"].clone()), (json!("12"), json!("23")));
    let snapped = call(&mut s, "scene.snap", json!({ "item": "rock" })).unwrap();
    assert!(!snapped["group"].is_null(), "snapping moves the foot to the tile center, and the group follows");
    let tile = &snapped["item"]["at"]["tile"];
    let (c, r) = (tile[0].as_i64().unwrap(), tile[1].as_i64().unwrap());
    assert_eq!(
        (snapped["group"]["attrs"]["data-x"].clone(), snapped["group"]["attrs"]["data-y"].clone()),
        (json!(((c - r) * 16).to_string()), json!(((c + r) * 8).to_string()))
    );
}

#[test]
fn r9_a_group_that_points_elsewhere_no_keep_group_and_a_keyed_move_leave_the_attributes_alone() {
    let mut s = session();
    let other = call(&mut s, "scene.move", json!({ "item": "2.0", "nudge": [1, 0] })).unwrap();
    assert!(other.get("group").is_none());
    let off = call(&mut s, "scene.move", json!({ "item": "0.0", "by": [0, 1], "keepGroup": false })).unwrap();
    assert!(off.get("group").is_none());
    assert_eq!(groups(&mut s)[0]["data-row"], "0");
    let keyed = call(&mut s, "scene.move", json!({ "item": "rock", "nudge": [2, 0], "time": 500 })).unwrap();
    assert!(keyed.get("group").is_none());
    assert_eq!(groups(&mut s)[1]["data-x"], "42");
}

#[test]
fn r10_sets_and_removes_attributes_by_group_path_or_by_an_item_inside_it() {
    let mut s = session();
    let r = call(&mut s, "scene.group.set", json!({ "group": "2", "attrs": { "data-enter": "sky", "data-x": "" } })).unwrap();
    assert_eq!(r["group"], json!({ "path": "2", "attrs": { "class": "enter", "data-enter": "sky", "data-y": "99" } }));
    let by_item = call(&mut s, "scene.group.set", json!({ "group": "rock", "attrs": { "data-wave": "2" } })).unwrap();
    assert_eq!(by_item["group"]["path"], "1");
    assert_eq!(by_item["group"]["attrs"]["data-wave"], "2");
}

#[test]
fn r10_foot_sets_data_x_and_data_y_to_the_first_items_foot() {
    let mut s = session();
    let r = call(&mut s, "scene.group.set", json!({ "group": "2", "foot": true })).unwrap();
    assert_eq!(
        (r["group"]["attrs"]["data-x"].clone(), r["group"]["attrs"]["data-y"].clone()),
        (json!("8"), json!("30")),
        "the pebble's anchor is its foot"
    );
}

#[test]
fn r10_refuses_attributes_the_engine_would_a_top_level_item_and_a_call_with_nothing_to_set() {
    let mut s = session();
    assert!(call(&mut s, "scene.group.set", json!({ "group": "0", "attrs": { "style": "x" } }))
        .unwrap_err()
        .message
        .contains("class or data-*"));
    assert!(call(&mut s, "scene.group.set", json!({ "group": "0", "attrs": { "data-x": "a\"b" } }))
        .unwrap_err()
        .message
        .contains("without \" < > &"));
    assert_eq!(call(&mut s, "scene.group.set", json!({ "group": "3", "foot": true })).unwrap_err().code, "not_in_group");
    assert_eq!(call(&mut s, "scene.group.set", json!({ "group": "0" })).unwrap_err().code, "usage");
}

#[test]
fn r11_into_an_existing_group_where_it_stands_among_that_groups_items() {
    let mut s = session();
    let r = call(&mut s, "scene.place", json!({ "object": "pebble", "at": [1, 0, 1], "group": "0" })).unwrap();
    assert_eq!((r["path"].clone(), r["group"]["path"].clone()), (json!("0.1"), json!("0")), "nothing it overlaps: it joins the end");
    // Below the cube it is behind it: it is listed first, and the cube moves to second.
    let behind = call(&mut session(), "scene.place", json!({ "object": "pebble", "at": [1, 0, -1], "group": "0" })).unwrap();
    assert_eq!((behind["path"].clone(), behind["group"]["path"].clone()), (json!("0.0"), json!("0")));
    assert_eq!(behind["reordered"]["0.0"], "0.1", "the cube moved down one");
    // Into the group around the rock, with nothing it overlaps: it joins last, and the rock keeps its place.
    let mut fresh = session();
    let far = call(&mut fresh, "scene.place", json!({ "object": "pebble", "px": [5, 5], "group": "rock" })).unwrap();
    assert_eq!(far["path"], "1.1");
    let items = call(&mut fresh, "scene.items", json!({})).unwrap();
    let rock = items["items"].as_array().unwrap().iter().find(|i| i["id"] == "rock").unwrap().clone();
    assert_eq!(rock["path"], "1.0");
}

#[test]
fn r11_into_a_new_group_with_its_foot_the_group_goes_where_its_item_stands_among_the_groups() {
    let mut s = session();
    let r = call(
        &mut s,
        "scene.place",
        json!({ "object": "stone", "px": [0, 0], "newGroup": { "class": "enter", "data-enter": "object" }, "foot": true }),
    )
    .unwrap();
    assert_eq!(r["group"]["attrs"], json!({ "class": "enter", "data-enter": "object", "data-x": "2", "data-y": "3" }));
    assert_eq!(r["path"], format!("{}.0", r["group"]["path"].as_str().unwrap()));
    let items = call(&mut s, "scene.items", json!({})).unwrap();
    let stone = items["items"].as_array().unwrap().iter().find(|i| i["group"] == r["group"]["path"]).unwrap().clone();
    assert_eq!(stone["path"], r["path"], "the path it returns is where it is after the sort");
    // On a tile, a new group goes where its item stands: the cube at 0,0,0 is behind the cube at 1,0,0, so its group is listed first.
    let t = call(&mut s, "scene.place", json!({ "object": "block", "at": [0, 0, 0], "newGroup": { "data-enter": "ground" } })).unwrap();
    let items = call(&mut s, "scene.items", json!({})).unwrap()["items"].as_array().unwrap().clone();
    let new0 = items.iter().find(|i| i["path"] == t["path"]).unwrap();
    let cube1 = items.iter().find(|i| i["object"] == "block" && i["at"]["tile"] == json!([1, 0, 0])).unwrap();
    let first = |p: &Value| p["path"].as_str().unwrap().split('.').next().unwrap().parse::<usize>().unwrap();
    assert!(first(new0) < first(cube1), "before the group whose cube is at 1,0,0");
    let nested = call(
        &mut s,
        "scene.place",
        json!({ "object": "pebble", "at": [1, 0, 1], "group": t["group"]["path"], "newGroup": { "data-part": "top" } }),
    )
    .unwrap();
    assert_eq!(nested["path"].as_str().unwrap().split('.').count(), 3);
}

#[test]
fn r6_r11_a_top_level_item_goes_where_it_stands_index_is_gone_and_the_refusals() {
    let mut s = session();
    let r = call(&mut s, "scene.place", json!({ "object": "pebble", "at": [9, 9, 0] })).unwrap();
    assert_eq!(r["path"], "4", "nothing overlaps it, so it joins the end of the list");
    assert!(call(&mut s, "scene.place", json!({ "object": "pebble", "at": [9, 9, 0], "index": 0 }))
        .unwrap_err()
        .message
        .contains("unknown param index"));
    assert_eq!(call(&mut s, "scene.place", json!({ "object": "pebble", "at": [9, 9, 0], "group": "4" })).unwrap_err().code, "not_in_group");
    assert!(call(&mut s, "scene.place", json!({ "object": "pebble", "at": [9, 9, 0], "newGroup": { "onclick": "x" } }))
        .unwrap_err()
        .message
        .contains("class or data-*"));
    assert_eq!(call(&mut s, "scene.place", json!({ "object": "pebble", "at": [9, 9, 0], "foot": true })).unwrap_err().code, "usage");
}

// ---- the same rules on scene_ops ----

#[test]
fn ops_r9_a_cube_moved_by_tiles_takes_its_ground_groups_data_col_data_row_data_x_and_data_y_with_it() {
    let mut doc = groups_scene();
    let group = moved(&mut doc, &[0, 0], |item| ops::move_by_tiles(item, 1.0, 0.0, 0.0)).unwrap();
    assert_eq!(
        group,
        json!({ "path": "0", "attrs": { "class": "enter", "data-enter": "ground", "data-col": "2", "data-row": "0", "data-x": "32", "data-y": "16" } })
    );
    assert_eq!(group_attrs(&doc, 0), &group["attrs"]);
}

#[test]
fn ops_r9_a_sprite_placed_by_pixel_takes_its_groups_foot_with_it() {
    let sources = sources();
    let mut doc = groups_scene();
    let before = ops::standing(&sources, &doc, ops::placed_at(&doc, &[1, 0]).unwrap());
    assert_eq!(before.foot, [42.0, 13.0]);
    let nudged = ops::nudge_item(&mut doc, &[1, 0], 1.0, -1.0, 0.0).unwrap();
    let after = ops::standing(&sources, &doc, &nudged["item"]);
    let group = ops::follow_group(&mut doc, &[1, 0], &before, &after).unwrap();
    assert_eq!(
        (group["attrs"]["data-x"].clone(), group["attrs"]["data-y"].clone()),
        (json!("43"), json!("12")),
        "the foot: x − 0 + 4/2, y − 0 + 4 − 1"
    );
    // scene.set: the new point's foot.
    let before = after;
    let set = ops::set_at(&mut doc, &[1, 0], &json!({ "px": [10, 20] })).unwrap();
    let after = ops::standing(&sources, &doc, &set["item"]);
    let group = ops::follow_group(&mut doc, &[1, 0], &before, &after).unwrap();
    assert_eq!((group["attrs"]["data-x"].clone(), group["attrs"]["data-y"].clone()), (json!("12"), json!("23")));
}

#[test]
fn ops_r9_a_group_that_points_elsewhere_leaves_the_attributes_alone() {
    let mut doc = groups_scene();
    let other = moved(&mut doc, &[2, 0], |item| {
        let px = item["at"]["px"].clone();
        item["at"]["px"] = json!([px[0].as_i64().unwrap() + 1, px[1]]);
    });
    assert_eq!(other, None);
    assert_eq!(group_attrs(&doc, 2)["data-x"], "99");
}

#[test]
fn ops_r10_sets_and_removes_attributes_by_group_path_or_by_an_item_inside_it() {
    let mut doc = groups_scene();
    let r = ops::set_group_attrs(&mut doc, &[2], &attrs(json!({ "data-enter": "sky", "data-x": "" }))).unwrap();
    assert_eq!(r["group"]["group"], json!({ "class": "enter", "data-enter": "sky", "data-y": "99" }));
    assert_eq!(r["message"], "Group 2: data-enter sky, data-x removed");
    let path = ops::group_path_of(&doc, "rock").unwrap();
    assert_eq!(path, vec![1]);
    let by_item = ops::set_group_attrs(&mut doc, &path, &attrs(json!({ "data-wave": "2" }))).unwrap();
    assert_eq!(by_item["path"], json!([1]));
    assert_eq!(by_item["group"]["group"]["data-wave"], "2");
    assert_eq!(ops::set_group_attrs(&mut doc, &[1], &attrs(json!({}))).unwrap()["message"], "Group 1: unchanged");
}

#[test]
fn ops_r10_foot_sets_data_x_and_data_y_to_the_first_items_foot() {
    let sources = sources();
    let doc = groups_scene();
    let (first, _) = ops::first_placed(doc["items"][2]["items"].as_array().unwrap(), &[2]).unwrap();
    assert_eq!(ops::foot_of(&sources, &first, [0.0, 0.0]), [8.0, 30.0], "the pebble's anchor is its foot");
    let (found, path) = ops::first_placed(doc["items"].as_array().unwrap(), &[]).unwrap();
    assert_eq!((found["object"].clone(), path), (json!("block"), vec![0, 0]));
}

#[test]
fn ops_r10_refuses_attributes_the_engine_would_a_top_level_item_and_a_missing_group() {
    let mut doc = groups_scene();
    assert!(ops::set_group_attrs(&mut doc, &[0], &attrs(json!({ "style": "x" }))).unwrap_err().message.contains("class or data-*"));
    assert_eq!(
        ops::set_group_attrs(&mut doc, &[0], &attrs(json!({ "style": "x" }))).unwrap_err().message,
        "A group attribute is class or data-*, like data-enter; not \"style\""
    );
    assert!(ops::set_group_attrs(&mut doc, &[0], &attrs(json!({ "data-x": "a\"b" }))).unwrap_err().message.contains("without \" < > &"));
    assert_eq!(ops::group_path_of(&doc, "3").unwrap_err().code, "not_in_group");
    assert_eq!(ops::set_group_attrs(&mut doc, &[3], &attrs(json!({}))).unwrap_err().code, "not_a_group");
    assert_eq!(ops::group_path_of(&doc, "9").unwrap_err().code, "not_found");
}

#[test]
fn ops_r11_into_an_existing_group_or_a_new_one_the_item_goes_last() {
    let mut doc = groups_scene();
    let r = ops::place_in_group(&mut doc, "pebble", &json!({ "tile": [1, 0, 1] }), Some(&[0]), None).unwrap();
    assert_eq!((r["path"].clone(), r["group"].clone()), (json!([0, 1]), json!([0])));
    assert_eq!(r["message"], "pebble placed at column 1, row 0, level 1 in group 0");
    let new = ops::place_in_group(
        &mut doc,
        "stone",
        &json!({ "px": [0, 0] }),
        None,
        Some(&attrs(json!({ "class": "enter", "data-enter": "object" }))),
    )
    .unwrap();
    assert_eq!((new["path"].clone(), new["group"].clone()), (json!([4, 0]), json!([4])));
    assert_eq!(new["message"], "stone placed at x 0, y 0 in a new group");
    assert_eq!(
        doc["items"][4],
        json!({ "group": { "class": "enter", "data-enter": "object" }, "items": [{ "object": "stone", "at": { "px": [0, 0] } }] })
    );
    let nested =
        ops::place_in_group(&mut doc, "pebble", &json!({ "tile": [1, 0, 1] }), Some(&[4]), Some(&attrs(json!({ "data-part": "top" }))))
            .unwrap();
    assert_eq!(nested["path"], json!([4, 1, 0]));
    assert_eq!(ops::place_in_group(&mut doc, "pebble", &json!({ "tile": [9, 9, 0] }), Some(&[3]), None).unwrap_err().code, "not_a_group");
}
