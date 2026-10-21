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
