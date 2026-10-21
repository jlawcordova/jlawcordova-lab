//! Ports of the 'editing objects (R17)' and 'editing blocks (hero island
//! detail spec R6)' tests in cli/test/engine.test.mjs. The Node tests share
//! one document object with their sources, so an edit shows in the next
//! `resolve`; here `src` rebuilds the sources from the document each time.

use std::collections::BTreeMap;

use engine::edit::{
    add_frame, add_layer, block_cells, block_fill, color_at, delete_frame, delete_layer, duplicate_frame, duplicate_layer, flood_fill,
    move_in, paint, paint_block, resize, BlockCell, Part, Where,
};
use engine::engine::{composite, load_sources, render_object, resolve, validate, CompositeOptions, Sources};
use engine::format::serialize_object;
use engine::iso::{block_faces, Pixel};
use engine::omap::OMap;
use serde_json::{json, Value};

fn palette() -> Value {
    json!({
        "world": { "ink": "#2E2418", "cream": "#F4EDE0", "grass-2": "#8FA56E", "grass-4": "#4E6B3A", "wood-3": "#7A4E2D" },
        "outfit": { "teal-1": "#7FA89B" },
    })
}

/// Sources from the given objects.
fn src(objects: &[(&str, &Value)]) -> Sources {
    let map: BTreeMap<String, Value> = objects.iter().map(|(n, d)| (n.to_string(), (*d).clone())).collect();
    load_sources(palette(), map, BTreeMap::new())
}

/// A one-layer sprite.
fn sprite(map: &[&str], keys: Value, extra: Value) -> Value {
    let mut doc = json!({ "kind": "sprite", "keys": keys, "layers": [{ "map": map }] });
    for (k, v) in extra.as_object().unwrap() {
        doc[k] = v.clone();
    }
    doc
}

/// Deterministic pseudo-random numbers (mulberry32), so a failure can be
/// replayed from its seed.
fn rng(seed: u32) -> impl FnMut() -> f64 {
    let mut a = seed;
    move || {
        a = a.wrapping_add(0x6d2b79f5);
        let mut t = a;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4294967296.0
    }
}

fn at(layer: usize, frame: usize) -> Where {
    Where { layer, frame }
}

fn base() -> Value {
    sprite(
        &["....", ".gg.", ".gg.", "...."],
        json!({ "g": "grass-2" }),
        json!({ "layers": [
            { "map": ["....", ".gg.", ".gg.", "...."] },
            { "loop": "wf", "prefix": "w", "frames": [["....", "....", "....", "...."], ["g...", "....", "....", "...."]] },
        ] }),
    )
}

/// `paint` with sources rebuilt from the document, as the shared object
/// gave the Node test.
fn paint_in(objects: &[(&str, &Value)], name: &str, doc: &mut Value, w: Where, pixels: &[[i64; 2]], color: Option<&str>) -> usize {
    let mut all: Vec<(&str, &Value)> = objects.iter().filter(|(n, _)| *n != name).cloned().collect();
    let snapshot = doc.clone();
    all.push((name, &snapshot));
    paint(&src(&all), name, doc, w, pixels, color).unwrap()
}

fn problems(objects: &[(&str, &Value)]) -> Vec<String> {
    validate(&src(objects))
}

fn keys_of(v: &Value) -> Vec<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

#[test]
fn r17_the_pencil_paints_pixels_and_a_new_color_gets_a_key_named_after_it() {
    let mut doc = base();
    assert_eq!(paint_in(&[], "s", &mut doc, at(0, 0), &[[0, 0], [1, 1]], Some("ink")), 2);
    assert_eq!(doc["keys"], json!({ "g": "grass-2", "i": "ink" }));
    assert_eq!(doc["layers"][0]["map"], json!(["i...", ".ig.", ".gg.", "...."]));
    assert_eq!(paint_in(&[], "s", &mut doc, at(0, 0), &[[0, 0], [9, 9]], Some("ink")), 0, "no change, and outside the map is ignored");
    assert_eq!(problems(&[("s", &doc)]), Vec::<String>::new());
}

#[test]
fn r17_a_key_already_taken_falls_back_to_the_uppercase_then_the_fixed_order() {
    let mut doc = sprite(&[".."], json!({ "g": "grass-2", "G": "grass-4" }), json!({}));
    paint_in(&[], "s", &mut doc, at(0, 0), &[[0, 0]], Some("grass-1"));
    assert_eq!(keys_of(&doc["keys"]), ["g", "G", "a"]);
}

#[test]
fn r17_the_eraser_clears_pixels_on_the_frame_being_edited() {
    let mut doc = base();
    assert_eq!(paint_in(&[], "s", &mut doc, at(0, 0), &[[1, 1]], None), 1);
    assert_eq!(doc["layers"][0]["map"], json!(["....", "..g.", ".gg.", "...."]));
    paint_in(&[], "s", &mut doc, at(1, 1), &[[0, 0]], None);
    paint_in(&[], "s", &mut doc, at(1, 0), &[[3, 3]], Some("grass-2"));
    assert_eq!(doc["layers"][1]["frames"], json!([["....", "....", "....", "...g"], ["....", "....", "....", "...."]]));
}

#[test]
fn r17_painting_an_object_that_extends_another_overrides_whole_rows_and_drops_a_row_that_matches_the_base_again() {
    let body = sprite(&["....", ".gg.", ".gg."], json!({ "g": "grass-2" }), json!({}));
    let mut outfit = json!({ "kind": "sprite", "extends": "body", "keys": {}, "rows": {} });
    let all = |o: &Value| src(&[("body", &body), ("outfit", o)]);
    let mut p = |outfit: &mut Value, pixels: &[[i64; 2]], color: Option<&str>| {
        let sources = all(&outfit.clone());
        paint(&sources, "outfit", outfit, at(0, 0), pixels, color).unwrap()
    };
    p(&mut outfit, &[[0, 2], [3, 0]], Some("ink"));
    assert_eq!(outfit["rows"], json!({ "0": { "0": "...i", "2": "igg." } }));
    assert_eq!(outfit["keys"], json!({ "i": "ink" }));
    p(&mut outfit, &[[0, 2]], None);
    assert_eq!(outfit["rows"], json!({ "0": { "0": "...i" } }));
    p(&mut outfit, &[[3, 0]], None);
    assert_eq!(outfit["rows"], json!({}));
    assert_eq!(
        serialize_object(&outfit),
        serialize_object(&json!({ "kind": "sprite", "extends": "body", "keys": { "i": "ink" }, "rows": {} }))
    );
}

#[test]
fn r17_an_object_that_extends_another_cant_paint_a_frame_loop() {
    let body = base();
    let mut outfit = json!({ "kind": "sprite", "extends": "body", "rows": {} });
    let sources = src(&[("body", &body), ("outfit", &outfit.clone())]);
    let err = paint(&sources, "outfit", &mut outfit, at(1, 0), &[[0, 0]], Some("ink")).unwrap_err();
    assert!(err.0.contains("frame loop"), "{}", err.0);
}

#[test]
fn r17_fill_takes_4_connected_pixels_of_the_same_key_only() {
    let map: Vec<String> = ["gg.g", "g..g", ".g.g"].iter().map(|s| s.to_string()).collect();
    let sorted = |x, y| {
        let mut v: Vec<String> = flood_fill(&map, x, y).iter().map(|p| format!("{},{}", p[0], p[1])).collect();
        v.sort();
        v
    };
    assert_eq!(sorted(0, 0), ["0,0", "0,1", "1,0"]);
    assert_eq!(sorted(1, 1), ["1,1", "2,0", "2,1", "2,2"]);
    assert!(flood_fill(&map, 9, 0).is_empty());
}

#[test]
fn r17_the_picker_reads_the_color_under_a_pixel_on_the_frame_shown() {
    let doc = base();
    let resolved = resolve(&src(&[("s", &doc)]), "s").unwrap();
    assert_eq!(color_at(&resolved, at(0, 0), 1, 1).as_deref(), Some("grass-2"));
    assert_eq!(color_at(&resolved, at(0, 0), 0, 0), None);
    assert_eq!(color_at(&resolved, at(1, 1), 0, 0).as_deref(), Some("grass-2"));
    assert_eq!(color_at(&resolved, at(1, 5), 0, 0).as_deref(), Some("grass-2"), "past the last frame shows the last frame");
}

#[test]
fn r17_resize_pads_and_crops_every_map_at_the_right_and_bottom() {
    let mut doc = base();
    resize(&mut doc, 2, 5);
    assert_eq!(doc["layers"][0]["map"], json!(["..", ".g", ".g", "..", ".."]));
    assert_eq!(doc["layers"][1]["frames"][1], json!(["g.", "..", "..", "..", ".."]));
    assert_eq!(problems(&[("s", &doc)]), Vec::<String>::new());
}

#[test]
fn r17_layers_and_frames_can_be_added_duplicated_moved_and_deleted_but_never_to_none() {
    let mut doc = base();
    assert_eq!(add_layer(&mut doc), 2);
    assert_eq!(doc["layers"][2]["map"], json!(["....", "....", "....", "...."]));
    assert_eq!(duplicate_layer(&mut doc, 0), 1);
    assert_eq!(doc["layers"][1], doc["layers"][0]);
    let layers = doc["layers"].as_array_mut().unwrap();
    assert_eq!(move_in(layers, 1, 1), 2);
    assert_eq!(move_in(layers, 0, -1), 0, "already first");
    assert_eq!(delete_layer(&mut doc, 3), 2);
    assert_eq!(doc["layers"].as_array().unwrap().len(), 3);
    let mut one = sprite(&["g"], json!({ "g": "grass-2" }), json!({}));
    assert_eq!(delete_layer(&mut one, 0), 0);
    assert_eq!(one["layers"].as_array().unwrap().len(), 1);

    let mut looped = base();
    assert_eq!(add_frame(&mut looped, 1, 0), 1);
    let firsts: Vec<&Value> = looped["layers"][1]["frames"].as_array().unwrap().iter().map(|f| &f[0]).collect();
    assert_eq!(firsts, [&json!("...."), &json!("...."), &json!("g...")]);
    assert_eq!(duplicate_frame(&mut looped, 1, 2), 3);
    assert_eq!(looped["layers"][1]["frames"][3], looped["layers"][1]["frames"][2]);
    assert_eq!(delete_frame(&mut looped, 1, 3), 2);
    assert_eq!(looped["layers"][1]["frames"].as_array().unwrap().len(), 3);
    assert_eq!(problems(&[("s", &looped)]), Vec::<String>::new());
}

// -------------------------------------------------------------------- blocks

fn cube(size: [i64; 3]) -> Value {
    json!({ "kind": "block", "size": size, "faces": { "top": "grass-2", "left": "wood-3", "right": "grass-4" } })
}

/// What the block shows on `frame`, "x,y" → color.
fn drawn(doc: &Value, frame: usize) -> OMap<Pixel, String> {
    let sources = src(&[("blk", doc)]);
    let group = render_object(&sources, "blk").unwrap();
    let shown = composite(&group, CompositeOptions { frame, ..Default::default() });
    let mut out = OMap::new();
    for (p, c) in shown.iter() {
        out.set(*p, c.to_string());
    }
    out
}

#[test]
fn r6_every_block_pixel_maps_to_one_cell_the_top_to_the_surface_the_faces_to_their_side_maps() {
    let cells = block_cells([1, 1, 1]);
    let faces = block_faces([1, 1, 1], [0, 0]);
    assert_eq!(cells.len(), faces.top.len() + faces.left.len() + faces.right.len());
    assert_eq!(cells.get(&(0, 0)), Some(&BlockCell { part: Part::Surface, col: 16, row: 8 }));
    assert_eq!(cells.get(&(-15, 0)), Some(&BlockCell { part: Part::Surface, col: 1, row: 8 }));
    // Left column i starts at y = 1 + floor(i/2), right column j at y = 8 - ceil(j/2) (pixel-artist agent, Blocks).
    assert_eq!(cells.get(&(-10, 5)), Some(&BlockCell { part: Part::Left, col: 5, row: 2 }));
    assert_eq!(cells.get(&(10, 7)), Some(&BlockCell { part: Part::Right, col: 10, row: 4 }));
    assert_eq!(cells.get(&(20, 0)), None, "outside the block");
}

#[test]
fn r6_l4_painting_any_pixel_of_a_block_makes_that_pixel_show_the_color() {
    for size in [[1, 1, 1], [2, 1, 2], [1, 2, 0]] {
        let pixels: Vec<Pixel> = block_cells(size).keys().cloned().collect();
        let mut r = rng(size.iter().fold(0, |a, b| a * 10 + b) as u32);
        for _ in 0..60 {
            let p = pixels[(r() * pixels.len() as f64).floor() as usize];
            let mut doc = cube(size);
            if size[2] == 0 {
                doc["faces"] = json!({ "top": "grass-2" });
            }
            assert_eq!(paint_block(&mut doc, 0, &[[p.0 as i64, p.1 as i64]], Some("cream")).unwrap(), 1, "{size:?}: {p:?}");
            assert_eq!(drawn(&doc, 0).get(&p).map(String::as_str), Some("cream"), "{size:?}: {p:?} shows the paint");
            assert_eq!(problems(&[("blk", &doc)]), Vec::<String>::new(), "{size:?}: {p:?} gives a valid block");
        }
    }
}

#[test]
fn r6_painting_adds_a_surface_or_sides_with_a_key_for_the_color_and_erasing_clears_the_cell() {
    let mut doc = cube([1, 1, 1]);
    assert_eq!(paint_block(&mut doc, 0, &[[0, 0], [-10, 5], [10, 7]], Some("roof-2")).unwrap(), 3);
    let blank = |w: usize| vec![".".repeat(w); 16];
    let put = |mut map: Vec<String>, col: usize, row: usize| {
        map[row].replace_range(col..=col, "r");
        map
    };
    assert_eq!(doc["surface"], json!({ "keys": { "r": "roof-2" }, "map": put(blank(32), 16, 8) }));
    assert_eq!(doc["sides"], json!({ "keys": { "r": "roof-2" }, "left": put(blank(16), 5, 2), "right": put(blank(16), 10, 4) }));
    assert_eq!(paint_block(&mut doc, 0, &[[-10, 5]], Some("roof-2")).unwrap(), 0, "the same color again changes nothing");
    assert_eq!(
        paint_block(&mut doc, 0, &[[-10, 5], [-15, 30]], None).unwrap(),
        1,
        "the eraser clears a cell; outside the block is ignored"
    );
    assert_eq!(doc["sides"]["left"], json!(blank(16)));
    let mut plain = cube([1, 1, 1]);
    assert_eq!(paint_block(&mut plain, 0, &[[0, 0], [-10, 5]], None).unwrap(), 0);
    assert_eq!(plain, cube([1, 1, 1]), "erasing a block with no surface or sides adds neither");
}

#[test]
fn r6_painting_a_looped_surface_paints_the_frame_being_edited() {
    let map = |ch: &str| vec![ch.repeat(32); 16];
    let mut doc = cube([1, 1, 0]);
    doc["faces"] = json!({ "top": "grass-2" });
    doc["surface"] = json!({ "keys": { "a": "cream" }, "loop": "wf", "prefix": "w", "frames": [map("."), map(".")] });
    paint_block(&mut doc, 1, &[[0, 0]], Some("cream")).unwrap();
    assert_eq!(doc["surface"]["frames"][0][8].as_str().unwrap().as_bytes()[16], b'.');
    assert_eq!(doc["surface"]["frames"][1][8].as_str().unwrap().as_bytes()[16], b'a');
    assert_eq!(drawn(&doc, 1).get(&(0, 0)).map(String::as_str), Some("cream"), "frame 1 shows it");
    assert_eq!(drawn(&doc, 0).get(&(0, 0)).map(String::as_str), Some("grass-2"), "frame 0 does not");
}

#[test]
fn r6_fill_on_a_block_covers_the_start_pixels_color_on_that_part_only() {
    let mut doc = cube([1, 1, 1]);
    let cells = block_cells([1, 1, 1]);
    let left = block_fill(&cells, &drawn(&doc, 0), -10, 5);
    assert_eq!(left.len(), block_faces([1, 1, 1], [0, 0]).left.len(), "the whole left face, not the top or the right face");
    paint_block(&mut doc, 0, &[[-10, 5], [-9, 5], [-8, 5]], Some("cream")).unwrap();
    assert_eq!(block_fill(&cells, &drawn(&doc, 0), -10, 5).len(), 3, "a patch fills only itself");
    assert!(block_fill(&cells, &drawn(&doc, 0), 40, 40).is_empty(), "outside the block");
}
