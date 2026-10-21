//! A scene without a viewBox (docs/intents/2026-10-scene-canvas/spec.md,
//! R1 to R7, R12): the frame resolved in one place, and the proof that a
//! scene with a viewBox publishes the same bytes it always has (R4).

mod depthlib;

use std::path::Path;

use depthlib::*;
use engine::compile::{compile_scene, Budgets};
use engine::engine::Order;
use engine::preview::{encode_png, render_preview, PreviewColors, Target};
use engine::sha256;

const GOLDENS: &str = include_str!("data/frame-goldens.txt");

fn repo() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// One line per scene of the examples and the fixture: the hash of its compiled SVG and of its preview PNG.
fn golden_lines() -> Vec<String> {
    let colors = PreviewColors { card: [250, 246, 238], grid: [200, 190, 170] };
    let mut docs: Vec<(String, std::path::PathBuf)> =
        ["harbor", "town"].iter().map(|n| (n.to_string(), repo().join("examples").join(format!("{n}.pixellab")))).collect();
    docs.push(("sample".into(), repo().join("fixtures/sample.pixellab")));
    let mut lines = Vec::new();
    for (doc, path) in docs {
        let (sources, errors) = read_sources(&path);
        assert_eq!(errors, Vec::<String>::new());
        for scene in sources.scenes.keys() {
            let svg = compile_scene(&sources, scene, &Budgets::new(), Order::Depth).unwrap().output;
            let (image, _) = render_preview(&sources, Target::Scene(scene), colors).unwrap();
            let png = encode_png(&image, 1);
            lines.push(format!("{doc}/{scene} svg {} png {}", sha256::hex(svg.as_bytes()), sha256::hex(&png)));
        }
    }
    lines
}

#[test]
fn r4_a_scene_with_a_viewbox_publishes_the_same_bytes() {
    let lines = golden_lines();
    if std::env::var("PIXELLAB_WRITE_GOLDENS").is_ok() {
        std::fs::write(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/frame-goldens.txt"), lines.join("\n") + "\n").unwrap();
        return;
    }
    assert_eq!(lines.join("\n") + "\n", GOLDENS, "an existing scene's SVG or image changed");
}

// ------------------------------------------------------------ format (R1, R12)

use engine::engine::validate;
use serde_json::{json, Value};

fn problems(scene: Value) -> Vec<String> {
    validate(&src(&[("b", block())], &[("s", scene)]))
}

#[test]
fn r1_the_starter_scene_has_no_viewbox() {
    let scene = engine::starter::starter_scene();
    assert!(scene.get("viewBox").is_none());
    assert_eq!(problems(scene), Vec::<String>::new());
}

#[test]
fn r12_a_scene_without_a_viewbox_is_valid_and_one_with_a_bad_viewbox_is_not() {
    assert_eq!(problems(json!({ "items": [{ "object": "b", "at": { "tile": [0, 0, 0] } }] })), Vec::<String>::new());
    assert_eq!(problems(json!({ "viewBox": [0, 0, 8, 8], "items": [] })), Vec::<String>::new());
    let bad = problems(json!({ "viewBox": [0, 0, 0, 8], "items": [] }));
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(bad[0].contains("viewBox must be four whole numbers"), "{bad:?}");
    assert!(problems(json!({ "viewBox": null, "items": [] }))[0].contains("viewBox must be four whole numbers"));
}

#[test]
fn r5_the_margin_is_a_whole_number_of_pixels_from_zero() {
    assert_eq!(problems(json!({ "margin": 2, "items": [] })), Vec::<String>::new());
    assert_eq!(problems(json!({ "margin": 0, "items": [] })), Vec::<String>::new());
    for bad in [json!(-1), json!(1.5), json!("2")] {
        let p = problems(json!({ "margin": bad, "items": [] }));
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("margin must be a whole number of pixels, 0 or more"), "{p:?}");
    }
}

#[test]
fn r12_a_scene_keeps_its_keys_in_canonical_order_with_margin_next_to_viewbox() {
    let scene = json!({ "items": [], "origin": [0, 0], "margin": 2, "viewBox": [0, 0, 8, 8], "output": "a.svg" });
    assert_eq!(
        engine::format::serialize_scene(&scene),
        "{\n  \"output\": \"a.svg\",\n  \"viewBox\": [0, 0, 8, 8],\n  \"margin\": 2,\n  \"origin\": [0, 0],\n  \"items\": []\n}\n"
    );
}

// ------------------------------------------------- the fit (R2, R3, R5, R6, R7)

use engine::engine::{bounds, composite, render_object, render_scene, resolve_frame, CompositeOptions, EMPTY_FRAME};
use engine::iso::tile_to_px;

/// `[x, y, w, h]` of the pixels the block `b` paints standing on `tile` (origin 0, 0), shifted by `by`.
fn block_box(tile: [f64; 3], by: [i32; 2]) -> (i32, i32, i32, i32) {
    let sources = src(&[("b", block())], &[]);
    let pixels = composite(&render_object(&sources, "b").unwrap(), CompositeOptions::default());
    let b = bounds(pixels.keys()).unwrap();
    let [px, py] = tile_to_px(tile, [0.0, 0.0]);
    (b.min_x + px as i32 + by[0], b.min_y + py as i32 + by[1], b.max_x + px as i32 + by[0], b.max_y + py as i32 + by[1])
}

fn union_box(boxes: &[(i32, i32, i32, i32)]) -> [i64; 4] {
    let x0 = boxes.iter().map(|b| b.0).min().unwrap();
    let y0 = boxes.iter().map(|b| b.1).min().unwrap();
    let x1 = boxes.iter().map(|b| b.2).max().unwrap();
    let y1 = boxes.iter().map(|b| b.3).max().unwrap();
    [x0 as i64, y0 as i64, (x1 - x0 + 1) as i64, (y1 - y0 + 1) as i64]
}

fn item(id: &str, tile: [i32; 3]) -> Value {
    json!({ "object": "b", "id": id, "at": { "tile": tile } })
}

fn frame_of(scene: Value) -> engine::engine::Frame {
    resolve_frame(&src(&[("b", block())], &[("s", scene)]), "s").unwrap()
}

#[test]
fn r2_a_scene_without_a_viewbox_publishes_at_its_painted_bounds() {
    let scene = json!({ "items": [item("a", [0, 0, 0]), item("c", [3, -1, 2])] });
    let frame = frame_of(scene.clone());
    assert!(!frame.fixed);
    assert_eq!(frame.view_box, union_box(&[block_box([0.0, 0.0, 0.0], [0, 0]), block_box([3.0, -1.0, 2.0], [0, 0])]));
    // The box is exactly the pixels: every edge has paint on it.
    let sources = src(&[("b", block())], &[("s", scene)]);
    let rendered = render_scene(&sources, "s", Order::Depth).unwrap();
    let b = bounds(composite(&rendered.root, CompositeOptions::default()).keys()).unwrap();
    let [x, y, w, h] = frame.view_box;
    assert_eq!([b.min_x as i64, b.min_y as i64, b.max_x as i64 - x + 1, b.max_y as i64 - y + 1], [x, y, w, h]);
    // The SVG, the preview and the render all carry it.
    let vb: Vec<i64> = rendered.view_box.iter().map(|v| v.as_i64().unwrap()).collect();
    assert_eq!(vb, frame.view_box.to_vec());
}

#[test]
fn r2_a_scene_with_a_viewbox_is_fixed_and_keeps_it() {
    let frame = frame_of(json!({ "viewBox": [-3, -2, 10, 7], "items": [item("a", [0, 0, 0])] }));
    assert!(frame.fixed);
    assert_eq!(frame.view_box, [-3, -2, 10, 7]);
}

#[test]
fn r3_a_moving_items_every_place_is_inside_the_box() {
    let scene = json!({
        "items": [item("a", [0, 0, 0])],
        "motion": { "duration": 1000, "tracks": [{ "item": "a", "keys": [
            { "t": 0, "move": [0, 0] }, { "t": 400, "move": [40, 20] }, { "t": 700, "move": [-30, 6] }
        ] }] }
    });
    let frame = frame_of(scene);
    let expected = union_box(&[block_box([0.0; 3], [0, 0]), block_box([0.0; 3], [40, 20]), block_box([0.0; 3], [-30, 6])]);
    assert_eq!(frame.view_box, expected);
}

#[test]
fn r3_a_moving_item_that_starts_away_from_its_art_counts_from_where_it_rests() {
    // The art is drawn where the first key puts it; the others move it from there.
    let scene = json!({
        "items": [item("a", [0, 0, 0])],
        "motion": { "duration": 1000, "tracks": [{ "item": "a", "keys": [
            { "t": 0, "move": [10, 0] }, { "t": 500, "move": [30, 0] }
        ] }] }
    });
    let frame = frame_of(scene);
    assert_eq!(frame.view_box, union_box(&[block_box([0.0; 3], [10, 0]), block_box([0.0; 3], [30, 0])]));
}

/// A block with an ambient loop and a hop action.
fn playing_item() -> Value {
    json!({
        "object": "b", "id": "a", "at": { "tile": [1, 1, 0] },
        "ambient": { "duration": 1000, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 500, "move": [-8, 4] }] },
        "actions": { "hop": { "duration": 400, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 200, "move": [6, -12] }] } }
    })
}

#[test]
fn r3_an_ambient_loops_reach_is_inside_the_box() {
    let scene = json!({ "items": [playing_item()] });
    let sources = src(&[("b", block())], &[("s", scene)]);
    let frame = resolve_frame(&sources, "s").unwrap().view_box;
    let expected =
        union_box(&[block_box([1.0, 1.0, 0.0], [0, 0]), block_box([1.0, 1.0, 0.0], [-8, 4]), block_box([1.0, 1.0, 0.0], [6, -12])]);
    assert_eq!(frame, expected);
    // Sampled over the loop, and over the action, nothing paints outside it.
    let rendered = render_scene(&sources, "s", Order::Depth).unwrap();
    let inside = |pixels: &engine::omap::OMap<engine::iso::Pixel, engine::engine::ColorName>| {
        pixels.keys().all(|&(x, y)| {
            (x as i64) >= frame[0] && (x as i64) < frame[0] + frame[2] && (y as i64) >= frame[1] && (y as i64) < frame[1] + frame[3]
        })
    };
    for t in (0..1000).step_by(50) {
        let options = CompositeOptions { time: Some(t as f64), ..Default::default() };
        assert!(inside(&composite(&rendered.root, options)), "ambient at {t}ms");
        let options = CompositeOptions { time: Some(t as f64), play: Some(("hop".into(), (t % 400) as f64)), ..Default::default() };
        assert!(inside(&composite(&rendered.root, options)), "hop at {t}ms");
    }
}

#[test]
fn r5_the_margin_is_added_on_every_side() {
    let items = json!([item("a", [0, 0, 0])]);
    let bare = frame_of(json!({ "items": items })).view_box;
    let padded = frame_of(json!({ "margin": 3, "items": items })).view_box;
    assert_eq!(padded, [bare[0] - 3, bare[1] - 3, bare[2] + 6, bare[3] + 6]);
    assert_eq!(frame_of(json!({ "margin": 0, "items": items })).view_box, bare);
    // A fixed box takes no margin.
    assert_eq!(frame_of(json!({ "viewBox": [0, 0, 9, 9], "margin": 3, "items": items })).view_box, [0, 0, 9, 9]);
}

#[test]
fn r6_an_empty_scene_has_one_tile_around_the_origin() {
    assert_eq!(EMPTY_FRAME, [-16, -8, 32, 16]);
    let frame = frame_of(json!({ "items": [] }));
    assert_eq!(frame.view_box, [-16, -8, 32, 16]);
    assert_eq!(frame.painted, None);
    let sources = src(&[("b", block())], &[("s", json!({ "items": [] }))]);
    // It compiles, renders and previews.
    let compiled = compile_scene(&sources, "s", &Budgets::new(), Order::Depth).unwrap();
    assert!(compiled.output.contains("viewBox=\"-16 -8 32 16\""), "{}", compiled.output);
    let colors = PreviewColors { card: [1, 2, 3], grid: [4, 5, 6] };
    let (image, at) = render_preview(&sources, Target::Scene("s"), colors).unwrap();
    assert_eq!((image.width, image.height, at), (32, 16, [-16, -8]));
}

#[test]
fn r7_the_image_and_the_preview_use_the_resolved_box() {
    let scene = json!({ "margin": 2, "items": [item("a", [0, 0, 0]), item("c", [2, 2, 0])] });
    let sources = src(&[("b", block())], &[("s", scene)]);
    let [x, y, w, h] = resolve_frame(&sources, "s").unwrap().view_box;
    let colors = PreviewColors { card: [1, 2, 3], grid: [4, 5, 6] };
    let (image, at) = render_preview(&sources, Target::Scene("s"), colors).unwrap();
    assert_eq!((image.width as i64, image.height as i64, at), (w, h, [x as i32, y as i32]));
    // The margin shows as card around the art: the corner pixel is the card color.
    assert_eq!(&image.rgba[..3], &[1, 2, 3]);
    let svg = compile_scene(&sources, "s", &Budgets::new(), Order::Depth).unwrap().output;
    assert!(svg.contains(&format!("viewBox=\"{x} {y} {w} {h}\"")), "{svg}");
}
