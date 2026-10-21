//! `PIXELLAB_LEGACY_SVG=1` (plan.md, "Departures"): the compiler writes the
//! SVGs as the Node CLI did, so the parity check still compares them until
//! cutover. Its own test binary, since it sets the environment.

use std::collections::BTreeMap;

use engine::compile::{compile_scene, Budgets};
use engine::engine::{load_sources, Order};
use serde_json::json;

#[test]
fn the_legacy_switch_writes_the_unscoped_svg_with_no_frame_loops_in_it() {
    let palette = json!({ "world": { "grass-2": "#8FA56E", "cream": "#F4EDE0" }, "outfit": {} });
    let flag = json!({ "kind": "sprite", "anchor": [0, 1], "keys": { "g": "grass-2", "c": "cream" },
        "layers": [{ "map": ["gg"] }, { "loop": "wf", "prefix": "w", "frames": [["c."], [".c"]] }] });
    let scene = json!({ "output": "pond.svg", "viewBox": [-8, -8, 16, 16], "items": [{ "object": "flag", "at": { "tile": [0, 0, 0] } }] });
    let s = load_sources(palette, BTreeMap::from([("flag".to_string(), flag)]), BTreeMap::from([("pond".to_string(), scene)]));
    let scoped = compile_scene(&s, "pond", &Budgets::new(), Order::Depth).unwrap().output;
    assert!(scoped.contains("data-pixellab=\"pond\"") && scoped.contains("<style>"));
    std::env::set_var("PIXELLAB_LEGACY_SVG", "1");
    let legacy = compile_scene(&s, "pond", &Budgets::new(), Order::Depth).unwrap().output;
    std::env::remove_var("PIXELLAB_LEGACY_SVG");
    assert!(
        legacy.starts_with(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\" viewBox=\"-8 -8 16 16\" shape-rendering=\"crispEdges\"><path"
        ),
        "{legacy}"
    );
    assert!(!legacy.contains("<style>") && !legacy.contains("data-pixellab"));
}
