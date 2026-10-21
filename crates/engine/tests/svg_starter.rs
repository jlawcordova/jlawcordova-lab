//! The rect SVG and the starters (ports of the toRectSvg and starter tests in
//! cli/test/engine.test.mjs), with direct tests of `to_rect_svg` on
//! hand-built trees whose expected text came from running the Node
//! `toRectSvg` on the same trees.

use std::collections::BTreeMap;
use std::rc::Rc;

use engine::engine as eng;
use engine::engine::{composite, Child, ColorName, CompositeOptions, Group, Layer, Order, Rendered};
use engine::format::{parse, serialize_palette};
use engine::starter::{starter_block, starter_extends, starter_palette, starter_scene, starter_sprite, STARTER_PALETTE};
use engine::svg::to_rect_svg;
use serde_json::{json, Value};

// ---------------------------------------------------------------- helpers

fn hex(name: &str) -> String {
    match name {
        "a" | "c" => "#111111",
        "b" => "#222222",
        "d" => "#333333",
        other => panic!("no hex for {other}"),
    }
    .to_string()
}

fn layer(pixels: &[(i32, i32, &str)], names: &[&str]) -> Layer {
    let mut l = Layer::default();
    for (x, y, n) in pixels {
        l.pixels.set((*x, *y), Rc::<str>::from(*n) as ColorName);
    }
    for n in names {
        l.colors.add(Rc::<str>::from(*n) as ColorName);
    }
    l
}

fn rendered(view_box: Value, style: Option<&str>, children: Vec<Child>) -> Rendered {
    Rendered {
        view_box: view_box.as_array().unwrap().clone(),
        root: Group { children, ..Group::default() },
        style: style.map(str::to_string),
        scope: None,
    }
}

// ----------------------------------------------------- to_rect_svg directly

#[test]
fn two_colors_with_the_same_hex_share_a_group() {
    let r = rendered(json!([0, 0, 8, 8]), None, vec![Child::Layer(layer(&[(0, 0, "a"), (1, 0, "c"), (0, 1, "b")], &["b", "a", "c"]))]);
    assert_eq!(
        to_rect_svg(&r, &hex),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\" viewBox=\"0 0 8 8\" shape-rendering=\"crispEdges\"><g fill=\"#222222\"><rect x=\"0\" y=\"1\" width=\"1\" height=\"1\"/></g><g fill=\"#111111\"><rect x=\"0\" y=\"0\" width=\"2\" height=\"1\"/></g></svg>"
    );
}

#[test]
fn a_run_breaks_at_a_gap_and_at_a_new_row() {
    let r = rendered(
        json!([-1.5, 0, 8, 8]),
        None,
        vec![Child::Layer(layer(&[(0, 0, "a"), (1, 0, "a"), (3, 0, "a"), (0, 1, "a"), (1, 1, "a"), (2, 1, "a")], &["a", "d"]))],
    );
    assert_eq!(
        to_rect_svg(&r, &hex),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\" viewBox=\"-1.5 0 8 8\" shape-rendering=\"crispEdges\"><g fill=\"#111111\"><rect x=\"0\" y=\"0\" width=\"2\" height=\"1\"/><rect x=\"3\" y=\"0\" width=\"1\" height=\"1\"/><rect x=\"0\" y=\"1\" width=\"3\" height=\"1\"/></g></svg>"
    );
}

#[test]
fn attributes_are_escaped_and_a_style_comes_first() {
    let group = Group {
        attrs: vec![("class".into(), "a&b\"c<d".into()), ("data-x".into(), "1".into())],
        children: vec![Child::Layer(layer(&[(2, 2, "d")], &["d"]))],
        ..Group::default()
    };
    let r = rendered(json!([0, 0, 4, 4]), Some(".x{opacity:0}"), vec![Child::Group(group)]);
    assert_eq!(
        to_rect_svg(&r, &hex),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\" viewBox=\"0 0 4 4\" shape-rendering=\"crispEdges\"><style>.x{opacity:0}</style><g class=\"a&amp;b&quot;c&lt;d\" data-x=\"1\"><g fill=\"#333333\"><rect x=\"2\" y=\"2\" width=\"1\" height=\"1\"/></g></g></svg>"
    );
}

#[test]
fn an_empty_style_writes_nothing() {
    let r = rendered(json!([0, 0, 4, 4]), Some(""), vec![]);
    assert_eq!(
        to_rect_svg(&r, &hex),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\" viewBox=\"0 0 4 4\" shape-rendering=\"crispEdges\"></svg>"
    );
}

// ------------------------------------------------ ports needing the renderer

const PALETTE: &str = r##"{ "world": { "ink": "#2E2418", "cream": "#F4EDE0", "grass-2": "#8FA56E", "grass-4": "#4E6B3A", "wood-3": "#7A4E2D" }, "outfit": { "teal-1": "#7FA89B" } }"##;

fn src(objects: Vec<(&str, Value)>, scenes: Vec<(&str, Value)>) -> eng::Sources {
    let to_map = |v: Vec<(&str, Value)>| v.into_iter().map(|(k, d)| (k.to_string(), d)).collect::<BTreeMap<_, _>>();
    eng::load_sources(parse(PALETTE).unwrap(), to_map(objects), to_map(scenes))
}

fn sprite(map: &[&str], keys: Value) -> Value {
    json!({ "kind": "sprite", "keys": keys, "layers": [{ "map": map }] })
}

fn scene(items: Value, extra: Value) -> Value {
    let mut s = json!({ "viewBox": [0, 0, 8, 8], "items": items });
    for (k, v) in extra.as_object().into_iter().flatten() {
        s[k] = v.clone();
    }
    s
}

/// The flattened pixels of a scene, sorted: `[...composite(root)].sort()`.
fn flat_of(sources: &eng::Sources, name: &str) -> Vec<(i32, i32, String)> {
    let root = eng::render_scene(sources, name, Order::Depth).unwrap().root;
    let mut out: Vec<_> = composite(&root, CompositeOptions::default()).iter().map(|(p, c)| (p.0, p.1, c.to_string())).collect();
    out.sort();
    out
}

fn svg_of(sources: &eng::Sources, name: &str) -> String {
    let rendered = eng::render_scene(sources, name, Order::Depth).unwrap();
    to_rect_svg(&rendered, &|n| sources.colors.get(n).unwrap().hex.clone())
}

/// `.replace(/^<svg[^>]*>|<\/svg>$/g, '')`.
fn inner(svg: &str) -> String {
    let start = svg.find('>').unwrap() + 1;
    svg[start..svg.len() - "</svg>".len()].to_string()
}

#[test]
fn r5_layers_paint_bottom_to_top_and_a_class_layer_gets_its_own_group() {
    let coat = json!({ "kind": "sprite", "keys": { "g": "grass-2", "i": "ink" }, "layers": [{ "map": ["gg"] }, { "class": "cbob", "map": [".i"] }] });
    let sources = src(vec![("coat", coat)], vec![("s", scene(json!([{ "object": "coat", "at": { "px": [0, 0] } }]), json!({})))]);
    assert_eq!(
        svg_of(&sources, "s"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\" viewBox=\"0 0 8 8\" shape-rendering=\"crispEdges\"><g fill=\"#8FA56E\"><rect x=\"0\" y=\"0\" width=\"2\" height=\"1\"/></g><g class=\"cbob\"><g fill=\"#2E2418\"><rect x=\"1\" y=\"0\" width=\"1\" height=\"1\"/></g></g></svg>"
    );
    assert_eq!(flat_of(&sources, "s"), vec![(0, 0, "grass-2".to_string()), (1, 0, "ink".to_string())]);
}

#[test]
fn r5_a_frame_loop_compiles_to_loop_prefix_n_groups_and_frame_0_is_the_static_frame() {
    let fall = json!({ "kind": "sprite", "keys": { "w": "cream" }, "layers": [{ "loop": "wf", "prefix": "w", "frames": [["w."], [".w"], [".."]] }] });
    let sources = src(vec![("fall", fall)], vec![("s", scene(json!([{ "object": "fall", "at": { "px": [0, 0] } }]), json!({})))]);
    assert_eq!(
        inner(&svg_of(&sources, "s")),
        "<g class=\"wf w0\"><g fill=\"#F4EDE0\"><rect x=\"0\" y=\"0\" width=\"1\" height=\"1\"/></g></g><g class=\"wf w1\"><g fill=\"#F4EDE0\"><rect x=\"1\" y=\"0\" width=\"1\" height=\"1\"/></g></g><g class=\"wf w2\"></g>"
    );
    assert_eq!(flat_of(&sources, "s"), vec![(0, 0, "cream".to_string())]);
}

#[test]
fn r5_colors_are_first_painted_in_key_order_and_runs_are_sorted_by_y_then_x() {
    let tree = sprite(&["gi", "ig", "gg"], json!({ "i": "ink", "g": "grass-2" }));
    let sources = src(vec![("tree", tree)], vec![("s", scene(json!([{ "object": "tree", "at": { "px": [0, 0] } }]), json!({})))]);
    assert_eq!(
        inner(&svg_of(&sources, "s")),
        "<g fill=\"#2E2418\"><rect x=\"1\" y=\"0\" width=\"1\" height=\"1\"/><rect x=\"0\" y=\"1\" width=\"1\" height=\"1\"/></g><g fill=\"#8FA56E\"><rect x=\"0\" y=\"0\" width=\"1\" height=\"1\"/><rect x=\"1\" y=\"1\" width=\"1\" height=\"1\"/><rect x=\"0\" y=\"2\" width=\"2\" height=\"1\"/></g>"
    );
}

#[test]
fn r3_items_paint_in_source_order_and_later_paint_wins_in_a_layer() {
    let dot = sprite(&["g"], json!({ "g": "grass-2" }));
    let ink = sprite(&["i"], json!({ "i": "ink" }));
    let items = json!([{ "object": "dot", "at": { "px": [1, 1] } }, { "object": "ink", "at": { "px": [1, 1] } }]);
    let sources = src(vec![("dot", dot), ("ink", ink)], vec![("s", scene(items, json!({})))]);
    let out = svg_of(&sources, "s");
    assert!(out.ends_with("<g fill=\"#2E2418\"><rect x=\"1\" y=\"1\" width=\"1\" height=\"1\"/></g></svg>"));
    assert!(!out.contains("#8FA56E"), "a fully covered color leaves no group");
}

#[test]
fn r3_groups_carry_their_attributes_in_source_order_and_placement_classes_wrap_the_object() {
    let dot = sprite(&["g"], json!({ "g": "grass-2" }));
    let ink = sprite(&["i"], json!({ "i": "ink" }));
    let items = json!([
        { "group": { "data-class": "0", "class": "variant" }, "items": [{ "object": "dot", "at": { "px": [0, 0] } }] },
        { "object": "ink", "class": "itruck it1", "at": { "px": [2, 0] } },
    ]);
    let sources = src(vec![("dot", dot), ("ink", ink)], vec![("s", scene(items, json!({})))]);
    assert_eq!(
        inner(&svg_of(&sources, "s")),
        "<g data-class=\"0\" class=\"variant\"><g fill=\"#8FA56E\"><rect x=\"0\" y=\"0\" width=\"1\" height=\"1\"/></g></g><g class=\"itruck it1\"><g fill=\"#2E2418\"><rect x=\"2\" y=\"0\" width=\"1\" height=\"1\"/></g></g>"
    );
}

#[test]
fn r3_the_root_carries_the_scene_viewbox_in_todays_attribute_order() {
    let sources = src(vec![], vec![("s", scene(json!([]), json!({ "viewBox": [-51, -9, 103, 72] })))]);
    assert_eq!(
        svg_of(&sources, "s"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\" viewBox=\"-51 -9 103 72\" shape-rendering=\"crispEdges\"></svg>"
    );
}

// ----------------------------------------------------------------- starters

#[test]
fn r17_the_editors_starters_are_the_same_documents_as_npm_run_art_new_writes() {
    assert_eq!(
        starter_sprite(4, 3),
        json!({ "kind": "sprite", "anchor": [2, 2], "keys": { "o": "ink" }, "layers": [{ "map": ["....", "....", "...."] }] })
    );
    assert_eq!(starter_scene(), json!({ "viewBox": [-64, -64, 128, 128], "origin": [0, 0], "items": [] }));
    assert_eq!(starter_extends("body"), json!({ "kind": "sprite", "extends": "body", "keys": {}, "rows": {} }));
    assert_eq!(
        starter_block(),
        json!({ "kind": "block", "size": [1, 1, 1], "faces": { "top": "grass-2", "left": "soil-2", "right": "soil-3" } })
    );
}

#[test]
fn the_starters_keep_the_key_order_of_the_node_documents() {
    assert_eq!(
        engine::format::stringify(&starter_sprite(5, 2)),
        r#"{"kind":"sprite","anchor":[2,1],"keys":{"o":"ink"},"layers":[{"map":[".....","....."]}]}"#
    );
    assert_eq!(engine::format::stringify(&starter_scene()), r#"{"viewBox":[-64,-64,128,128],"origin":[0,0],"items":[]}"#);
    assert_eq!(engine::format::stringify(&starter_extends("body")), r#"{"kind":"sprite","extends":"body","keys":{},"rows":{}}"#);
    assert_eq!(
        engine::format::stringify(&starter_block()),
        r#"{"kind":"block","size":[1,1,1],"faces":{"top":"grass-2","left":"soil-2","right":"soil-3"}}"#
    );
}

#[test]
fn the_starter_palette_file_is_canonical() {
    assert_eq!(serialize_palette(&parse(STARTER_PALETTE).unwrap()), STARTER_PALETTE);
}

#[test]
fn the_starter_palette_has_the_same_world_and_outfit_colors_in_order_as_starter_palette_mjs() {
    // JSON.stringify of the default export of cli/lib/engine/starter-palette.mjs.
    let expected = r##"{"world":{"grass-1":"#B5C79C","grass-2":"#8FA56E","grass-3":"#6F8F55","grass-4":"#4E6B3A","soil-1":"#9C6B42","soil-2":"#8A5A34","soil-3":"#5A3E2B","soil-4":"#3F2B1E","wood-1":"#C8A27A","wood-2":"#A07A52","wood-3":"#7A4E2D","wood-4":"#4A3324","path-1":"#E9DCC6","path-2":"#C9B79A","path-3":"#A8957A","path-4":"#8A7A60","water-1":"#C9DDD3","water-2":"#9DC1B4","water-3":"#5F8C7E","water-4":"#3E5E55","roof-1":"#C25A6A","roof-2":"#9E3B4B","roof-3":"#7B2D3B","roof-4":"#5A2230","gold-1":"#E3C57E","gold-2":"#D8B66A","gold-3":"#B08F48","gold-4":"#7A5E2A","ink":"#2E2418","cream":"#F4EDE0","skin-1":"#E9B98A","skin-2":"#A0524A"},"outfit":{"silver-1":"#E2E6EA","silver-2":"#B4BCC4","silver-3":"#858F99","silver-4":"#5A636C","violet-1":"#B49CC8","violet-2":"#8A6AA6","violet-3":"#5F4679","slate-1":"#6B7380","slate-2":"#474E5B","slate-3":"#2F343E","orange-1":"#E8A15A","orange-2":"#C2733A","sky-1":"#C3D8E6","sky-2":"#8FB0C9","navy-1":"#5B6F8E","navy-2":"#3D4C66"}}"##;
    let palette = starter_palette();
    let got = json!({ "world": palette["world"], "outfit": palette["outfit"] });
    assert_eq!(engine::format::stringify(&got), expected);
    assert_eq!(palette["colorNotes"]["soil-3"], "--color-earth");
    assert_eq!(palette["colorNotes"].as_object().unwrap().len(), 4);
    assert_eq!(palette["notes"].as_array().unwrap().len(), 9);
}
