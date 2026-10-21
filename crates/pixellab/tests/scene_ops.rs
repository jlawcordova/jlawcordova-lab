//! scene_ops against the web lab's behavior (ports cli/test/scene-ops.test.mjs):
//! each test states what the lab does and checks the port does the same, on
//! plain scene documents.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};

use engine::engine::{load_sources, Sources};
use engine::jsnum::num;
use engine::{iso, motion};
use pixellab::errors::CommandError;
use pixellab::scene_ops as ops;

fn scene() -> Value {
    json!({
        "viewBox": [0, 0, 200, 200],
        "origin": [100, 50],
        "items": [
            { "object": "a", "at": { "tile": [0, 0, 0] } },
            { "object": "b", "at": { "tile": [1, 0, 0] } },
            { "object": "c", "at": { "px": [10, 10] } },
            { "object": "d", "at": { "tile": [1, 1, 0] } },
            { "object": "e", "at": { "tile": [1, 1, 1] } },
            { "group": { "class": "g" }, "items": [{ "object": "f", "id": "cart", "at": { "tile": [3, 3, 0] } }, { "object": "h", "at": { "px": [5, 6] } }] },
        ],
        "motion": { "duration": 1000, "tracks": [{ "item": "cart", "keys": [{ "t": 0 }, { "t": 500, "move": [32, 16] }] }] },
    })
}

const ORIGIN: [f64; 2] = [100.0, 50.0];

fn keys_of(v: &Value) -> Vec<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

fn code(e: CommandError) -> String {
    e.code
}

/// The scene of real sprites that sortScene needs (the fake objects above don't resolve).
fn real() -> Value {
    json!({
        "viewBox": [-100, -100, 200, 200],
        "origin": [0, 0],
        "items": [
            { "object": "pine", "id": "front", "at": { "tile": [1, 1, 0] } },
            { "object": "pine", "id": "back", "at": { "tile": [0, 0, 0] } },
            { "object": "pine", "id": "middle", "at": { "tile": [1, 0, 0] } },
            { "object": "pine", "id": "far", "at": { "tile": [9, 9, 0] } },
        ],
    })
}

fn ids(doc: &Value) -> Vec<Value> {
    doc["items"].as_array().unwrap().iter().map(|i| i["id"].clone()).collect()
}

/// The sample document's sources (a `pine` sprite is added by the sort tests' own document).
fn sources() -> Sources {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sample.pixellab");
    let docs = engine::format::read_docs(&dir);
    load_sources(
        docs.palette.clone().unwrap_or(Value::Null),
        BTreeMap::from_iter(docs.objects.clone()),
        BTreeMap::from_iter(docs.scenes.clone()),
    )
}

/// sortScene is ported with the session (U14): until then a stand-in that panics.
fn sort_scene(_sources: &Sources, _doc: &mut Value) -> (Vec<Value>, Vec<(String, String)>) {
    unimplemented!("sort_scene comes with the session (U14)")
}

#[test]
#[ignore = "needs U14"]
fn sort_scene_back_to_front_by_column_and_row_then_level_where_items_overlap_what_insert_index_did_for_tiles() {
    let mut doc = real();
    let (moved, remap) = sort_scene(&sources(), &mut doc);
    // "front" was listed first but stands in front of "back" and "middle"; "far" overlaps nothing and keeps its place.
    assert_eq!(ids(&doc), vec![json!("back"), json!("middle"), json!("front"), json!("far")]);
    assert_eq!(moved, vec![json!({ "path": "0", "to": "2", "object": "pine" })], "only the item the sort took out of its place is listed");
    let mut remap = remap;
    remap.sort();
    assert_eq!(
        remap,
        vec![("0".to_string(), "2".to_string()), ("1".to_string(), "0".to_string()), ("2".to_string(), "1".to_string())],
        "remap holds every path that changed"
    );
    // Sorting a sorted scene moves nothing.
    let (again, remap) = sort_scene(&sources(), &mut doc);
    assert_eq!((again, remap.len()), (vec![], 0));
}

#[test]
#[ignore = "needs U14"]
fn sort_scene_sorts_a_group_as_one_object_and_its_members_inside_it_a_scene_that_does_not_draw_is_left_alone() {
    let mut doc = real();
    doc["items"] = json!([
        { "group": { "class": "g" }, "items": [{ "object": "pine", "id": "g-front", "at": { "tile": [1, 1, 0] } }, { "object": "pine", "id": "g-back", "at": { "tile": [0, 0, 0] } }] },
        { "object": "pine", "id": "behind-all", "at": { "tile": [-1, -1, 0] } },
    ]);
    let (moved, remap) = sort_scene(&sources(), &mut doc);
    assert_eq!(doc["items"][0]["id"], "behind-all", "the group stands in front of it");
    let inner: Vec<&Value> = doc["items"][1]["items"].as_array().unwrap().iter().map(|i| &i["id"]).collect();
    assert_eq!(inner, vec!["g-back", "g-front"]);
    let mut remap = remap;
    remap.sort();
    let want = [("0", "1"), ("0.0", "1.1"), ("0.1", "1.0"), ("1", "0")];
    assert_eq!(remap, want.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>());
    assert!(moved.iter().any(|m| m["group"] == json!(true) || m["object"] == "pine"));
    let mut broken = real();
    broken["items"] =
        json!([{ "object": "no-such-object", "at": { "tile": [1, 1, 0] } }, { "object": "pine", "at": { "tile": [0, 0, 0] } }]);
    let (left, remap) = sort_scene(&sources(), &mut broken);
    assert_eq!((left, remap.len(), broken["items"][0]["object"].clone()), (vec![], 0, json!("no-such-object")));
}

#[test]
fn place_added_last_on_a_tile_or_by_pixel_the_session_then_sorts_it_lab_place() {
    let mut doc = scene();
    let r = ops::place(&mut doc, "pine", &json!({ "tile": [0, 1, 0] }));
    assert_eq!(r["path"], json!([doc["items"].as_array().unwrap().len() - 1]));
    assert_eq!(doc["items"].as_array().unwrap().last().unwrap(), &json!({ "object": "pine", "at": { "tile": [0, 1, 0] } }));
    assert_eq!(r["message"], "pine placed at column 0, row 1, level 0");
    let p = ops::place(&mut doc, "rock", &json!({ "px": [1, 2] }));
    assert_eq!(p["path"], json!([doc["items"].as_array().unwrap().len() - 1]));
    assert_eq!(p["message"], "rock placed at x 1, y 2");
    // A pixel item can stand on a level; level 0 is left out and not spoken.
    let high = ops::place(&mut doc, "rock", &json!({ "px": [1, 2], "level": 4 }));
    assert_eq!(doc["items"].as_array().unwrap().last().unwrap()["at"], json!({ "px": [1, 2], "level": 4 }));
    assert_eq!(high["message"], "rock placed at x 1, y 2, level 4");
    assert_eq!(ops::place(&mut doc, "rock", &json!({ "px": [1, 2], "level": 0 }))["item"]["at"], json!({ "px": [1, 2] }));
}

#[test]
fn move_by_tiles_a_tile_item_by_tiles_a_pixel_item_by_the_same_screen_offset_lab_move_by_tiles() {
    let mut tile = json!({ "at": { "tile": [1, 2, 0] } });
    ops::move_by_tiles(&mut tile, 1.0, -1.0, 2.0);
    assert_eq!(tile["at"]["tile"], json!([2, 1, 2]));
    let mut px = json!({ "at": { "px": [10, 10] } });
    ops::move_by_tiles(&mut px, 1.0, 0.0, 0.0);
    assert_eq!(px["at"]["px"], json!([26, 18])); // (dc - dr) * 16, (dc + dr) * 8
    ops::move_by_tiles(&mut px, 0.0, 1.0, 1.0);
    assert_eq!(px["at"]["px"], json!([10, 10])); // -16, +8 - 16
    assert_eq!(px["at"], json!({ "px": [10, 10], "level": 1 }), "a pixel item stands a level higher too, over the same tile");
    ops::move_by_tiles(&mut px, 0.0, 0.0, -1.0);
    assert_eq!(px["at"], json!({ "px": [10, 26] }), "back to level 0, which is left out");
}

#[test]
fn move_item_and_nudge_item_lab_move_selected_nudge_selected() {
    let mut doc = scene();
    assert_eq!(ops::move_item(&mut doc, &[0], 2.0, 1.0, 0.0, 0.0).unwrap()["message"], "a moved to column 2, row 1, level 0");
    assert_eq!(doc["items"][0]["at"], json!({ "tile": [2, 1, 0] }));
    // A nudge places a tile item by pixel, from its placement point (iso tileToPx).
    let [x, y] = iso::tile_to_px([1.0, 0.0, 0.0], ORIGIN);
    let r = ops::nudge_item(&mut doc, &[1], 1.0, -2.0, 0.0).unwrap();
    assert_eq!(doc["items"][1]["at"], json!({ "px": [num(x + 1.0), num(y - 2.0)] }));
    assert_eq!(r["message"], format!("b nudged to x {}, y {}", x + 1.0, y - 2.0));
    assert_eq!(code(ops::move_item(&mut doc, &[5], 1.0, 0.0, 0.0, 0.0).unwrap_err()), "is_group");
    assert_eq!(code(ops::move_item(&mut doc, &[9], 1.0, 0.0, 0.0, 0.0).unwrap_err()), "not_found");
    // A nudge keeps the level the item stands on: a tile item's, or a pixel item's own.
    doc["items"].as_array_mut().unwrap().push(json!({ "object": "x", "at": { "tile": [1, 0, 2] } }));
    let last = doc["items"].as_array().unwrap().len() - 1;
    ops::nudge_item(&mut doc, &[last], 1.0, 0.0, 0.0).unwrap();
    assert_eq!(doc["items"][last]["at"]["level"], 2);
    ops::nudge_item(&mut doc, &[last], 1.0, 0.0, 0.0).unwrap();
    assert_eq!(doc["items"][last]["at"]["level"], 2);
    let [hx, hy] = iso::tile_to_px([1.0, 0.0, 2.0], ORIGIN);
    assert_eq!(doc["items"][last]["at"]["px"], json!([num(hx + 2.0), num(hy)]));
}

#[test]
fn keyed_move_past_time_0_an_animated_item_moves_its_key_made_if_needed_lab_keyed_move() {
    let mut doc = scene();
    // At t 250 the cart is half-way to its t 500 key: the new key takes that point plus the move.
    let start = motion::state_at(&doc["motion"]["tracks"][0], "f", 1000.0, 250.0).moved;
    let r = ops::move_item(&mut doc, &[5, 0], 1.0, 0.0, 0.0, 250.0).unwrap();
    assert_eq!(r["keyed"], true);
    assert_eq!(r["message"], "f moved at 0.3 s");
    assert_eq!(
        doc["motion"]["tracks"][0]["keys"],
        json!([{ "t": 0 }, { "t": 250, "move": [num(start[0] + 16.0), num(start[1] + 8.0)] }, { "t": 500, "move": [32, 16] }])
    );
    assert_eq!(doc["items"][5]["items"][0]["at"], json!({ "tile": [3, 3, 0] }), "the placement stays");
    // An existing key moves; time 0 moves the placement.
    ops::nudge_item(&mut doc, &[5, 0], 2.0, 0.0, 500.0).unwrap();
    assert_eq!(doc["motion"]["tracks"][0]["keys"][2], json!({ "t": 500, "move": [34, 16] }));
    ops::move_item(&mut doc, &[5, 0], 1.0, 0.0, 0.0, 0.0).unwrap();
    assert_eq!(doc["items"][5]["items"][0]["at"], json!({ "tile": [4, 3, 0] }));
}

#[test]
fn set_field_sets_one_coordinate_of_the_matching_placement_lab_set_field() {
    let mut doc = scene();
    ops::set_field(&mut doc, &[0], "level", 3.0).unwrap();
    assert_eq!(doc["items"][0]["at"]["tile"], json!([0, 0, 3]));
    ops::set_field(&mut doc, &[2], "y", 40.0).unwrap();
    assert_eq!(doc["items"][2]["at"]["px"], json!([10, 40]));
    // The lab ignores x on a tile item; the port refuses.
    assert_eq!(code(ops::set_field(&mut doc, &[0], "x", 1.0).unwrap_err()), "wrong_placement");
    // A pixel item has a level too, left out at 0.
    ops::set_field(&mut doc, &[2], "level", 4.0).unwrap();
    assert_eq!(doc["items"][2]["at"], json!({ "px": [10, 40], "level": 4 }));
    assert_eq!(ops::set_field(&mut doc, &[2], "level", 0.0).unwrap()["message"], "c moved to x 10, y 40");
    assert_eq!(doc["items"][2]["at"], json!({ "px": [10, 40] }));
    assert_eq!(code(ops::set_field(&mut doc, &[2], "col", 1.0).unwrap_err()), "wrong_placement");
}

#[test]
fn snap_item_to_the_tile_under_the_placement_point_at_the_items_level_lab_snap_selected() {
    let mut doc = scene();
    let point = iso::tile_to_px([2.0, 1.0, 0.0], ORIGIN);
    doc["items"][2]["at"] = json!({ "px": [num(point[0] + 3.0), num(point[1] - 2.0)] });
    assert_eq!(ops::snap_item(&mut doc, &[2]).unwrap()["message"], "c snapped to column 2, row 1, level 0");
    assert_eq!(doc["items"][2]["at"], json!({ "tile": [2, 1, 0] }));
    assert!(ops::snap_item(&mut doc, &[2]).unwrap_err().message.contains("already on a tile"));
    // A pixel item snaps to the tile at the level it stands on.
    let high = iso::tile_to_px([2.0, 1.0, 3.0], ORIGIN);
    doc["items"][2]["at"] = json!({ "px": [num(high[0] + 3.0), num(high[1] - 2.0)], "level": 3 });
    assert_eq!(ops::snap_item(&mut doc, &[2]).unwrap()["message"], "c snapped to column 2, row 1, level 3");
    assert_eq!(doc["items"][2]["at"], json!({ "tile": [2, 1, 3] }));
}

#[test]
fn set_depth_front_back_or_auto_on_an_item_or_a_group_keeping_the_key_order_replaces_raise_and_lower() {
    let mut doc = scene();
    doc["items"][0]["layer"] = json!("l");
    doc["items"][0]["id"] = json!("first");
    let r = ops::set_depth(&mut doc, &[0], "front").unwrap();
    assert_eq!(r.message, "a draws in front of everything else in its list");
    assert_eq!(keys_of(&doc["items"][0]), ["object", "id", "layer", "depth", "at"]);
    assert_eq!(ops::set_depth(&mut doc, &[0], "back").unwrap().item["depth"], "back");
    assert_eq!(ops::set_depth(&mut doc, &[0], "auto").unwrap().message, "a draws by where it stands");
    assert_eq!(keys_of(&doc["items"][0]), ["object", "id", "layer", "at"]);
    // A group takes it too, and writes it after the group's attributes.
    assert_eq!(ops::set_depth(&mut doc, &[5], "back").unwrap().message, "Group 5 draws behind everything else in its list");
    assert_eq!(keys_of(&doc["items"][5]), ["group", "depth", "items"]);
    ops::set_depth(&mut doc, &[5], "auto").unwrap();
    assert_eq!(keys_of(&doc["items"][5]), ["group", "items"]);
    // And the rest of an item's edits keep the override in its place.
    ops::set_depth(&mut doc, &[5, 0], "front").unwrap();
    ops::set_class(&mut doc, &[5, 0], "sway").unwrap();
    assert_eq!(keys_of(&doc["items"][5]["items"][0]), ["object", "id", "class", "depth", "at"]);
    assert_eq!(code(ops::set_depth(&mut doc, &[0], "forward").unwrap_err()), "usage");
    assert_eq!(code(ops::set_depth(&mut doc, &[9], "front").unwrap_err()), "not_found");
}

#[test]
fn duplicate_item_one_column_over_listed_right_after_it_the_session_sorts_without_its_id_lab_duplicate_selected() {
    let mut doc = scene();
    let r = ops::duplicate_item(&mut doc, &[5, 0]).unwrap();
    assert_eq!(r["path"], json!([5, 1]));
    assert_eq!(doc["items"][5]["items"][1], json!({ "object": "f", "at": { "tile": [4, 3, 0] } }));
    assert_eq!(doc["items"][5]["items"][0]["id"], "cart");
    assert_eq!(doc["motion"]["tracks"].as_array().unwrap().len(), 1);
    let px = ops::duplicate_item(&mut doc, &[2]).unwrap();
    assert_eq!(doc["items"][3], json!({ "object": "c", "at": { "px": [26, 18] } }));
    assert_eq!(px["message"], "c duplicated at x 26, y 18");
}

#[test]
fn remove_item_drops_the_track_of_an_item_with_an_id_lab_remove_selected() {
    let mut doc = scene();
    ops::remove_item(&mut doc, &[5, 0]).unwrap();
    let objects: Vec<&Value> = doc["items"][5]["items"].as_array().unwrap().iter().map(|i| &i["object"]).collect();
    assert_eq!(objects, vec!["h"]);
    assert_eq!(doc["motion"]["tracks"], json!([]));
}

#[test]
fn set_class_keeps_the_key_order_object_id_class_at_lab_class_field() {
    let mut doc = scene();
    ops::set_class(&mut doc, &[5, 0], "sway slow").unwrap();
    assert_eq!(keys_of(&doc["items"][5]["items"][0]), ["object", "id", "class", "at"]);
    assert_eq!(ops::set_class(&mut doc, &[5, 0], "").unwrap()["message"], "Class removed");
    assert_eq!(keys_of(&doc["items"][5]["items"][0]), ["object", "id", "at"]);
    assert!(ops::set_class(&mut doc, &[0], "Bad Class").unwrap_err().message.contains("lowercase class names"));
}

#[test]
fn set_id_renames_or_drops_the_track_with_the_id_and_refuses_a_taken_id() {
    let mut doc = scene();
    ops::set_class(&mut doc, &[5, 0], "x").unwrap();
    ops::set_id(&mut doc, &[5, 0], "wagon").unwrap();
    assert_eq!(keys_of(&doc["items"][5]["items"][0]), ["object", "id", "class", "at"]);
    assert_eq!(doc["motion"]["tracks"][0]["item"], "wagon");
    ops::set_id(&mut doc, &[0], "first").unwrap();
    assert_eq!(code(ops::set_id(&mut doc, &[1], "wagon").unwrap_err()), "id_taken");
    assert!(ops::set_id(&mut doc, &[1], "Not An Id").unwrap_err().message.contains("kebab-case"));
    ops::set_id(&mut doc, &[5, 0], "").unwrap();
    assert_eq!(doc["items"][5]["items"][0].get("id"), None);
    assert_eq!(doc["motion"]["tracks"], json!([]));
}

#[test]
fn find_item_a_path_or_an_id_anywhere_in_the_groups() {
    let doc = scene();
    assert_eq!(ops::find_item(&doc, "5.1").unwrap(), vec![5, 1]);
    assert_eq!(ops::find_item(&doc, "cart").unwrap(), vec![5, 0]);
    assert_eq!(ops::find_item(&doc, "#cart").unwrap(), vec![5, 0]);
    assert_eq!(code(ops::find_item(&doc, "nope").unwrap_err()), "not_found");
}
