//! The compiled SVG's own `<style>` (spec R11, R14, R15, D7): frame loops
//! and scene motion play in the SVG itself, scoped to its scene, crisp
//! without the site's CSS, and a scene with nothing to play has no style.

use std::collections::BTreeMap;

use engine::compile::{compile_scene, Budgets};
use engine::engine::{load_sources, Order, Sources};
use serde_json::{json, Value};

fn palette() -> Value {
    json!({ "world": { "ink": "#2E2418", "cream": "#F4EDE0", "grass-2": "#8FA56E" }, "outfit": {} })
}

fn sources(objects: Vec<(&str, Value)>, scenes: Vec<(&str, Value)>) -> Sources {
    let map = |v: Vec<(&str, Value)>| v.into_iter().map(|(k, v)| (k.to_string(), v)).collect::<BTreeMap<_, _>>();
    load_sources(palette(), map(objects), map(scenes))
}

/// A sprite with a still layer and a frame loop of `frames` frames.
fn waving(loop_name: &str, prefix: &str, frames: usize) -> Value {
    let frame = |i: usize| json!(if i % 2 == 0 { ["c."] } else { [".c"] });
    json!({
        "kind": "sprite", "anchor": [0, 1], "keys": { "g": "grass-2", "c": "cream" },
        "layers": [{ "map": ["gg"] }, { "loop": loop_name, "prefix": prefix, "frames": (0..frames).map(frame).collect::<Vec<_>>() }]
    })
}

fn still() -> Value {
    json!({ "kind": "sprite", "anchor": [0, 0], "keys": { "g": "grass-2" }, "layers": [{ "map": ["gg"] }] })
}

fn scene(items: Value) -> Value {
    json!({ "output": "s.svg", "viewBox": [-8, -8, 16, 16], "items": items })
}

fn svg(sources: &Sources, name: &str) -> String {
    compile_scene(sources, name, &Budgets::new(), Order::Depth).unwrap().output
}

fn style(svg: &str) -> &str {
    let start = svg.find("<style>").map(|i| i + "<style>".len()).expect("a style");
    &svg[start..svg.find("</style>").unwrap()]
}

#[test]
fn r14_r15_the_root_names_its_scene_and_is_crisp_on_its_own() {
    let s = sources(vec![("stone", still())], vec![("meadow", scene(json!([{ "object": "stone", "at": { "tile": [0, 0, 0] } }])))]);
    let out = svg(&s, "meadow");
    assert!(
        out.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\" viewBox=\"-8 -8 16 16\" shape-rendering=\"crispEdges\" data-pixellab=\"meadow\">"),
        "{out}"
    );
    assert!(!out.contains(" class=\"") || !out.starts_with("<svg class"), "the root never gains a class");
}

#[test]
fn d7_a_scene_with_nothing_to_play_has_no_style() {
    let s = sources(vec![("stone", still())], vec![("meadow", scene(json!([{ "object": "stone", "at": { "tile": [0, 0, 0] } }])))]);
    assert!(!svg(&s, "meadow").contains("<style>"));
}

#[test]
fn r11_a_site_frame_loop_plays_as_pixel_art_css_plays_it_with_frame_0_under_reduced_motion() {
    let s =
        sources(vec![("flag", waving("wf", "w", 5))], vec![("pond", scene(json!([{ "object": "flag", "at": { "tile": [0, 0, 0] } }])))]);
    let out = svg(&s, "pond");
    assert_eq!(
        style(&out),
        concat!(
            "@keyframes pond--wf{0%{opacity:1}20%{opacity:0}100%{opacity:0}}",
            "[data-pixellab=\"pond\"] .wf{opacity:0;animation:pond--wf 1000ms step-end infinite}",
            "[data-pixellab=\"pond\"] .wf.w1{animation-delay:-800ms}",
            "[data-pixellab=\"pond\"] .wf.w2{animation-delay:-600ms}",
            "[data-pixellab=\"pond\"] .wf.w3{animation-delay:-400ms}",
            "[data-pixellab=\"pond\"] .wf.w4{animation-delay:-200ms}",
            "@media (prefers-reduced-motion:reduce){[data-pixellab=\"pond\"] .wf{animation:none}[data-pixellab=\"pond\"] .wf.w0{opacity:1}}",
        )
    );
    // The frames keep their classes, unscoped, as the site's pages know them.
    assert!(out.contains("<g class=\"wf w0\">") && out.contains("<g class=\"wf w4\">"), "{out}");
}

#[test]
fn r11_ff_and_hf_take_their_cycles_and_another_loop_steps_every_200_ms() {
    let s = sources(
        vec![("fire", waving("ff", "f", 4)), ("hearth", waving("hf", "h", 6)), ("glow", waving("glow", "g", 3))],
        vec![(
            "camp",
            scene(json!([
                { "object": "fire", "at": { "tile": [0, 0, 0] } },
                { "object": "hearth", "at": { "tile": [1, 0, 0] } },
                { "object": "glow", "at": { "tile": [2, 0, 0] } }
            ])),
        )],
    );
    let css = style(&svg(&s, "camp")).to_string();
    for rule in [
        "@keyframes camp--ff{0%{opacity:1}25%{opacity:0}100%{opacity:0}}",
        "[data-pixellab=\"camp\"] .ff{opacity:0;animation:camp--ff 600ms step-end infinite}",
        "[data-pixellab=\"camp\"] .ff.f3{animation-delay:-150ms}",
        "@keyframes camp--hf{0%{opacity:1}16.667%{opacity:0}100%{opacity:0}}",
        "[data-pixellab=\"camp\"] .hf.h5{animation-delay:-500ms}",
        "@keyframes camp--glow{0%{opacity:1}33.333%{opacity:0}100%{opacity:0}}",
        "[data-pixellab=\"camp\"] .glow{opacity:0;animation:camp--glow 600ms step-end infinite}",
        "[data-pixellab=\"camp\"] .glow.g2{animation-delay:-200ms}",
        "[data-pixellab=\"camp\"] .ff,[data-pixellab=\"camp\"] .hf,[data-pixellab=\"camp\"] .glow{animation:none}",
    ] {
        assert!(css.contains(rule), "missing {rule} in {css}");
    }
}

fn moving_scene(name: &str) -> (String, Value) {
    (
        name.to_string(),
        json!({
            "output": format!("{name}.svg"), "viewBox": [-8, -8, 16, 16],
            "items": [{ "object": "stone", "id": "cart", "class": "mv-mine", "at": { "tile": [0, 0, 0] } }],
            "motion": { "duration": 1000, "tracks": [{ "item": "cart", "keys": [{ "t": 0 }, { "t": 500, "move": [2, 0] }] }] }
        }),
    )
}

#[test]
fn r15_scene_motion_names_are_scoped_to_the_scene_and_two_scenes_share_none() {
    let (a, b) = (moving_scene("north"), moving_scene("south"));
    let s = sources(vec![("stone", still())], vec![(&a.0, a.1.clone()), (&b.0, b.1.clone())]);
    let (n, so) = (svg(&s, "north"), svg(&s, "south"));
    assert!(n.contains("<g class=\"north--mv-cart\">"), "{n}");
    assert!(style(&n).contains("@keyframes north--mv-cart{"));
    assert!(style(&n).contains("[data-pixellab=\"north\"] .north--mv-cart{animation:north--mv-cart 1000ms step-end infinite}"));
    assert!(style(&n).ends_with("@media (prefers-reduced-motion:reduce){[data-pixellab=\"north\"] [class*=\"north--\"]{animation:none}}"));
    // Every generated name and every selector belongs to its own scene.
    let names = |css: &str, scene: &str| {
        css.split("@keyframes ").skip(1).map(|r| r.split('{').next().unwrap().to_string()).all(|k| k.starts_with(&format!("{scene}--")))
    };
    assert!(names(style(&n), "north") && names(style(&so), "south"));
    assert!(!style(&n).contains("south") && !style(&so).contains("north"));
    // A document's own class passes through unchanged, even one that looks generated.
    assert!(n.contains("class=\"mv-mine\""), "{n}");
}

#[test]
fn r3_r15_the_scoped_svg_draws_the_same_pixels_as_before() {
    // Scoping changes names and adds the style; the paths are the same.
    let (a, _) = (moving_scene("north"), ());
    let s = sources(vec![("stone", still()), ("flag", waving("wf", "w", 5))], vec![(&a.0, a.1.clone())]);
    let out = svg(&s, "north");
    let paths = |svg: &str| svg.split("<path").skip(1).map(|p| p.split("/>").next().unwrap().to_string()).collect::<Vec<_>>();
    let rendered = engine::engine::render_scene(&s, "north", Order::Depth).unwrap();
    let plain =
        engine::compile::optimize_svg(&engine::svg::to_rect_svg(&rendered, &|c: &str| s.colors.get(c).unwrap().hex.clone())).unwrap();
    assert_eq!(paths(&out), paths(&plain));
}
