//! Ports of the rest of cli/test/engine.test.mjs: validation, the design
//! language, sprites, scenes, blocks, block sides and the serializer. The
//! grid and block-face tests are in `iso.rs`, the rect SVG ones in
//! `svg_starter.rs`, the previews in `preview.rs` and the editing tests in
//! `edit.rs`. Problems now name `.json` files, and the serializer tests
//! check format 2's text (`engine::format::serialize` and `parse`).

use std::collections::BTreeMap;
use std::time::Instant;

use engine::engine::{
    composite, describe_object, load_sources, render_object, render_scene, resolve, validate, Child, CompositeOptions, Group, Order,
    Sources,
};
use engine::format::{parse, serialize, stringify};
use engine::iso::{block_faces, tile_to_px, Pixel};
use engine::omap::OMap;
use serde_json::{json, Map, Value};

const SEEDS: [u32; 5] = [1, 7, 42, 2026, 31337];

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

/// `Math.floor(r() * n)`.
fn floor(r: &mut (impl FnMut() -> f64 + ?Sized), n: f64) -> i64 {
    (r() * n).floor() as i64
}

// ---------------------------------------------------------------- helpers

fn palette() -> Value {
    json!({
        "world": { "ink": "#2E2418", "cream": "#F4EDE0", "grass-2": "#8FA56E", "grass-4": "#4E6B3A", "wood-3": "#7A4E2D" },
        "outfit": { "teal-1": "#7FA89B" },
    })
}

/// `{...base, ...extra}`: extra's keys replace in place or are added.
fn with(mut base: Value, extra: Value) -> Value {
    for (k, v) in extra.as_object().expect("extra is an object") {
        base[k] = v.clone();
    }
    base
}

fn to_map(list: Vec<(&str, Value)>) -> BTreeMap<String, Value> {
    list.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// Sources from inline objects and scenes, on the default palette.
fn src(objects: Vec<(&str, Value)>, scenes: Vec<(&str, Value)>) -> Sources {
    load_sources(palette(), to_map(objects), to_map(scenes))
}

fn src_p(objects: Vec<(&str, Value)>, scenes: Vec<(&str, Value)>, palette: Value) -> Sources {
    load_sources(palette, to_map(objects), to_map(scenes))
}

/// A one-layer sprite on `grass-2`.
fn sprite<S: AsRef<str>>(map: impl IntoIterator<Item = S>) -> Value {
    sprite_k(map, json!({ "g": "grass-2" }), json!({}))
}

fn sprite_k<S: AsRef<str>>(map: impl IntoIterator<Item = S>, keys: Value, extra: Value) -> Value {
    let rows: Vec<String> = map.into_iter().map(|s| s.as_ref().to_string()).collect();
    with(json!({ "kind": "sprite", "keys": keys, "layers": [{ "map": rows }] }), extra)
}

fn scene(items: Value) -> Value {
    json!({ "viewBox": [0, 0, 8, 8], "items": items })
}

fn scene_x(items: Value, extra: Value) -> Value {
    with(scene(items), extra)
}

fn check(got: Vec<String>, want: &[&str]) {
    assert_eq!(got, want.iter().map(|s| s.to_string()).collect::<Vec<_>>());
}

fn rows_of(n: usize, w: usize, ch: &str) -> Vec<String> {
    vec![ch.repeat(w); n]
}

fn pixels_of(sources: &Sources, name: &str) -> OMap<Pixel, engine::engine::ColorName> {
    composite(&render_object(sources, name).unwrap(), CompositeOptions::default())
}

fn name_at(drawn: &OMap<Pixel, engine::engine::ColorName>, p: Pixel) -> Option<&str> {
    drawn.get(&p).map(|c| &**c)
}

/// `c.attrs?.[0]?.[1] ?? 'layer'` for each child of a group.
fn kinds(root: &Group) -> Vec<String> {
    root.children
        .iter()
        .map(|c| match c {
            Child::Layer(_) => "layer".to_string(),
            Child::Group(g) => g.attrs.first().map(|a| a.1.clone()).unwrap_or_else(|| "layer".to_string()),
        })
        .collect()
}

fn root_of(sources: &Sources, name: &str) -> Group {
    render_scene(sources, name, Order::Depth).unwrap().root
}

fn color_keys(n: usize, prefix: &str, digits: usize, start: &str) -> Value {
    let mut m = Map::new();
    for i in 0..n {
        m.insert(format!("{prefix}-{i}"), json!(format!("{start}{i:0digits$}")));
    }
    Value::Object(m)
}

/// The default palette with `n` more world colors `w-0`…, `#<start><i>`.
fn wide(n: usize, start: &str, digits: usize, base: Value) -> Value {
    let mut p = base;
    for (k, v) in color_keys(n, "w", digits, start).as_object().unwrap() {
        p["world"][k] = v.clone();
    }
    p
}

const ERR_SIZE: &str =
    "objects/blk.json: size must be three whole numbers: tiles wide, tiles deep (both at least 1) and levels high (0 is a flat tile)";

// ----------------------------------------------------------- validation (R30)

#[test]
fn r1_a_sprite_object_loads_and_validates_with_no_problems() {
    check(validate(&src(vec![("tree", sprite([".g.", "ggg"]))], vec![])), &[]);
}

#[test]
fn r30_a_ragged_row() {
    check(validate(&src(vec![("tree", sprite(["ggg", "gg", "ggg"]))], vec![])), &["objects/tree.json: layer 0, row 1: 2 wide, expected 3"]);
}

#[test]
fn r30_a_map_with_the_wrong_number_of_rows_in_a_frame() {
    let tree = json!({ "kind": "sprite", "keys": { "g": "grass-2" }, "layers": [{ "map": ["g", "g"] }, { "loop": "wf", "prefix": "w", "frames": [["g", "g"], ["g"]] }] });
    check(validate(&src(vec![("tree", tree)], vec![])), &["objects/tree.json: layer 1, frame 1: 1 rows, expected 2"]);
}

#[test]
fn r30_a_key_that_is_not_in_keys_with_its_column() {
    check(
        validate(&src(vec![("tree", sprite(["ggg", "gqg"]))], vec![])),
        &["objects/tree.json: layer 0, row 1, column 1: key 'q' is not in keys"],
    );
}

#[test]
fn r30_keys_are_case_sensitive() {
    check(validate(&src(vec![("tree", sprite(["gG"]))], vec![])), &["objects/tree.json: layer 0, row 0, column 1: key 'G' is not in keys"]);
}

#[test]
fn r30_a_key_that_is_more_than_one_character_or_a_space() {
    let tree = sprite_k(["g"], json!({ "g": "grass-2", "gg": "ink", " ": "ink" }), json!({}));
    check(
        validate(&src(vec![("tree", tree)], vec![])),
        &[
            "objects/tree.json: key 'gg': keys are one printable character, not space or '.'",
            "objects/tree.json: key ' ': keys are one printable character, not space or '.'",
        ],
    );
}

#[test]
fn r30_a_color_that_is_not_in_the_palette() {
    let tree = sprite_k(["g"], json!({ "g": "grass-9" }), json!({}));
    check(validate(&src(vec![("tree", tree)], vec![])), &["objects/tree.json: key 'g': 'grass-9' is not in the palette"]);
}

#[test]
fn r30_a_scene_that_places_a_missing_object() {
    let problems = validate(&src(vec![], vec![("demo", scene(json!([{ "object": "nope", "at": { "px": [0, 0] } }])))]));
    check(problems, &["scenes/demo.json: items[0]: object 'nope' does not exist"]);
}

#[test]
fn r30_a_missing_object_inside_a_group_names_the_nested_item() {
    let items = json!([{ "group": { "data-class": "0" }, "items": [{ "object": "nope", "at": { "px": [0, 0] } }] }]);
    let problems = validate(&src(vec![], vec![("demo", scene(items))]));
    check(problems, &["scenes/demo.json: items[0].items[0]: object 'nope' does not exist"]);
}

#[test]
fn r30_extends_an_object_that_does_not_exist() {
    check(
        validate(&src(vec![("hat", json!({ "kind": "sprite", "extends": "nope" }))], vec![])),
        &["objects/hat.json: extends 'nope', which does not exist"],
    );
}

#[test]
fn r30_an_extends_cycle() {
    let objects = vec![("a", json!({ "kind": "sprite", "extends": "b" })), ("b", json!({ "kind": "sprite", "extends": "a" }))];
    check(validate(&src(objects, vec![])), &["objects/a.json: extends cycle: a → b → a", "objects/b.json: extends cycle: b → a → b"]);
}

#[test]
fn r30_non_integer_offsets_and_anchors() {
    let tree = sprite_k(["g"], json!({ "g": "grass-2" }), json!({ "anchor": [0.5, 0] }));
    let items = json!([{ "object": "tree", "at": { "px": [1.5, 0] } }, { "object": "tree", "at": { "tile": [0, 0] } }]);
    let problems = validate(&src(vec![("tree", tree)], vec![("demo", scene(items))]));
    check(
        problems,
        &[
            "objects/tree.json: anchor must be two whole numbers",
            "scenes/demo.json: items[0]: at.px must be two whole numbers",
            "scenes/demo.json: items[1]: at.tile must be three whole numbers",
        ],
    );
}

#[test]
fn r30_unknown_properties_and_kinds() {
    let tree = with(sprite(["g"]), json!({ "kind": "blob", "colour": "red" }));
    check(
        validate(&src(vec![("tree", tree)], vec![])),
        &["objects/tree.json: unknown property 'colour'", "objects/tree.json: kind must be 'sprite' or 'block', got 'blob'"],
    );
}

#[test]
fn r30_a_row_override_with_the_wrong_width_or_out_of_range() {
    let objects = vec![
        ("character", sprite(["gg", "gg"])),
        ("hat", json!({ "kind": "sprite", "extends": "character", "rows": { "0": { "1": "ggg" } } })),
        ("cap", json!({ "kind": "sprite", "extends": "character", "rows": { "0": { "2": "gg" } } })),
    ];
    check(
        validate(&src(objects, vec![])),
        &[
            "objects/cap.json: layer 0, row 2 (override): character has rows 0 to 1",
            "objects/hat.json: layer 0, row 1 (override): 3 wide, expected 2",
        ],
    );
}

#[test]
fn r30_every_problem_is_reported_not_only_the_first() {
    check(
        validate(&src(vec![("tree", sprite(["gq", "g"]))], vec![])),
        &["objects/tree.json: layer 0, row 0, column 1: key 'q' is not in keys", "objects/tree.json: layer 0, row 1: 1 wide, expected 2"],
    );
}

#[test]
fn r30_names_must_be_lowercase_kebab_case() {
    check(validate(&src(vec![("Tree", sprite(["g"]))], vec![])), &["objects/Tree.json: names are lowercase kebab-case"]);
}

#[test]
fn r30_two_scenes_cannot_write_the_same_output() {
    let scenes = vec![("a", scene_x(json!([]), json!({ "output": "x.svg" }))), ("b", scene_x(json!([]), json!({ "output": "x.svg" })))];
    check(validate(&src(vec![], scenes)), &["scenes/b.json: output 'x.svg' is also written by scenes/a.json"]);
}

#[test]
fn r30_group_attributes_are_class_or_data() {
    let problems = validate(&src(vec![], vec![("demo", scene(json!([{ "group": { "onclick": "x" }, "items": [] }])))]));
    check(problems, &["scenes/demo.json: items[0]: group attribute 'onclick' must be class or data-*"]);
}

#[test]
fn r30_l4_random_invalid_objects_each_fail_with_their_exact_message() {
    for seed in SEEDS {
        let mut r = rng(seed);
        let w = 2 + floor(&mut r, 30.0) as usize;
        let h = 3 + floor(&mut r, 30.0) as usize; // 3 or more, so the most common width is clear
        let map: Vec<String> = (0..h).map(|_| (0..w).map(|_| if r() < 0.5 { 'g' } else { '.' }).collect()).collect();
        let row = floor(&mut r, h as f64) as usize;
        let col = floor(&mut r, w as f64) as usize;
        let cases: Vec<(Vec<String>, String)> = vec![
            (
                map.iter().enumerate().map(|(i, m)| if i == row { m[1..].to_string() } else { m.clone() }).collect(),
                format!("layer 0, row {row}: {} wide, expected {w}", w - 1),
            ),
            (
                map.iter()
                    .enumerate()
                    .map(|(i, m)| if i == row { format!("{}z{}", &m[..col], &m[col + 1..]) } else { m.clone() })
                    .collect(),
                format!("layer 0, row {row}, column {col}: key 'z' is not in keys"),
            ),
            (map.iter().map(|m| format!("{m}{}", ".".repeat(65 - w))).collect(), format!("65×{h}, max 64×64")),
        ];
        for (bad, message) in cases {
            assert_eq!(validate(&src(vec![("tree", sprite(bad))], vec![])), vec![format!("objects/tree.json: {message}")], "seed {seed}");
        }
    }
}

// ------------------------------------------------- design language (R26, R27)

fn many_colors(n: usize) -> Value {
    let mut m = Map::new();
    for (i, k) in "abcdefghijklm".chars().take(n).enumerate() {
        m.insert(k.to_string(), json!(format!("w-{i}")));
    }
    Value::Object(m)
}

#[test]
fn r27_rejects_a_13th_color_and_allows_12() {
    let wide13 = wide(13, "#0000", 2, palette());
    let twelve = "abcdefghijkl";
    check(validate(&src_p(vec![("tree", sprite_k([twelve], many_colors(12), json!({})))], vec![], wide13.clone())), &[]);
    check(
        validate(&src_p(vec![("tree", sprite_k([format!("{twelve}m")], many_colors(13), json!({})))], vec![], wide13)),
        &["objects/tree.json: uses 13 colors, max 12"],
    );
}

#[test]
fn r27_counts_colors_by_value_so_two_keys_for_one_color_count_once() {
    check(validate(&src(vec![("tree", sprite_k(["gG"], json!({ "g": "grass-2", "G": "grass-2" }), json!({})))], vec![])), &[]);
}

#[test]
fn r27_rejects_a_65x64_object_and_allows_64x64() {
    let map = |w: usize, h: usize| rows_of(h, w, "g");
    check(validate(&src(vec![("tree", sprite(map(64, 64)))], vec![])), &[]);
    check(validate(&src(vec![("tree", sprite(map(65, 64)))], vec![])), &["objects/tree.json: 65×64, max 64×64"]);
    check(validate(&src(vec![("tree", sprite(map(64, 65)))], vec![])), &["objects/tree.json: 64×65, max 64×64"]);
}

#[test]
fn r27_rejects_a_24x33_character_and_an_outfit_that_makes_one() {
    let body = |h: usize, w: usize| rows_of(h, w, "g");
    check(validate(&src(vec![("character", sprite(body(32, 24)))], vec![])), &[]);
    check(
        validate(&src(vec![("character", sprite(body(33, 24)))], vec![])),
        &["objects/character.json: the figure is 24×33; a character is at most 24×32"],
    );
    check(
        validate(&src(vec![("character", sprite(body(32, 25)))], vec![])),
        &["objects/character.json: the figure is 25×32; a character is at most 24×32"],
    );
    let tall = vec![("character", sprite(body(33, 24))), ("hat", json!({ "kind": "sprite", "extends": "character" }))];
    check(
        validate(&src(tall, vec![])),
        &[
            "objects/character.json: the figure is 24×33; a character is at most 24×32",
            "objects/hat.json: the figure is 24×33; a character is at most 24×32",
        ],
    );
}

#[test]
fn r27_a_characters_cap_is_on_the_figure_it_paints_not_its_canvas() {
    let canvas = |figure_width: usize| -> Vec<String> {
        (0..38)
            .map(|y| {
                if y < 32 {
                    format!("{}{}", ".".repeat(10), "g".repeat(figure_width)).chars().chain(std::iter::repeat('.')).take(40).collect()
                } else {
                    ".".repeat(40)
                }
            })
            .collect()
    };
    let base = || ("character", sprite(canvas(24)));
    check(validate(&src(vec![base(), ("coat", json!({ "kind": "sprite", "extends": "character" }))], vec![])), &[]);
    let padded: String = std::iter::once('g').chain(std::iter::repeat('.')).take(40).collect();
    let prop = json!({ "kind": "sprite", "extends": "character", "rows": { "0": { "37": padded } } });
    check(validate(&src(vec![base(), ("coat", prop)], vec![])), &["objects/coat.json: the figure is 34×38; a character is at most 24×32"]);
}

#[test]
fn r26_a_c_hex_name_is_not_in_the_palette() {
    let tree = sprite_k(["g"], json!({ "g": "c-6f8a55" }), json!({}));
    check(validate(&src(vec![("tree", tree)], vec![])), &["objects/tree.json: key 'g': 'c-6f8a55' is not in the palette"]);
}

#[test]
fn retire_legacy_r3_an_object_with_legacy_true_is_rejected() {
    let tree = sprite_k(["g"], json!({ "g": "grass-2" }), json!({ "legacy": true }));
    check(validate(&src(vec![("tree", tree)], vec![])), &["objects/tree.json: unknown property 'legacy'"]);
}

#[test]
fn r26_an_outfit_color_only_in_objects_that_extend_character() {
    let tree = sprite_k(["t"], json!({ "t": "teal-1" }), json!({}));
    check(
        validate(&src(vec![("tree", tree)], vec![])),
        &["objects/tree.json: key 't': 'teal-1' is an outfit color; only objects that extend character may use it"],
    );
    let objects =
        vec![("character", sprite(["g"])), ("coat", json!({ "kind": "sprite", "extends": "character", "keys": { "g": "teal-1" } }))];
    check(validate(&src(objects, vec![])), &[]);
}

#[test]
fn r26_palette_tiers_over_their_caps() {
    let big = |n: usize, prefix: &str| {
        let mut m = Map::new();
        for i in 0..n {
            m.insert(format!("{prefix}-{i}"), json!(format!("#00{i:04}")));
        }
        Value::Object(m)
    };
    let problems = validate(&src_p(vec![], vec![], json!({ "world": big(33, "w"), "outfit": big(17, "o") })));
    check(problems, &["palette.json: world has 33 colors, max 32", "palette.json: outfit has 17 colors, max 16"]);
}

#[test]
fn retire_legacy_r2_the_palette_has_two_tiers_and_a_legacy_tier_is_unknown() {
    // Format 2 also accepts the keys `notes` and `colorNotes`; any other key,
    // `legacy` among them, is still an unknown tier.
    let problems = validate(&src_p(vec![], vec![], json!({ "world": {}, "outfit": {}, "legacy": { "c-6f8a55": "#6F8A55" } })));
    check(problems, &["palette.json: unknown tier 'legacy'"]);
}

#[test]
fn r26_palette_values_are_uppercase_hex_with_no_alpha_and_names_are_unique_across_tiers() {
    let palette = json!({ "world": { "ink": "#2e2418", "cream": "#F4EDE0AA" }, "outfit": { "ink": "#2E2418" } });
    check(
        validate(&src_p(vec![], vec![], palette)),
        &[
            "palette.json: world 'ink': '#2e2418' must be uppercase #RRGGBB, with no alpha",
            "palette.json: world 'cream': '#F4EDE0AA' must be uppercase #RRGGBB, with no alpha",
            "palette.json: 'ink' is in both world and outfit; names must be unique",
        ],
    );
}

#[test]
fn r26_the_same_value_may_be_in_two_tiers_under_different_names() {
    let mut p = palette();
    p["outfit"]["teal-0"] = json!("#2E2418");
    check(validate(&src_p(vec![], vec![], p)), &[]);
}

#[test]
fn r27_l4_random_oversize_objects_fail_with_their_exact_size() {
    for seed in SEEDS {
        let mut r = rng(seed);
        let w = 65 + floor(&mut r, 40.0) as usize;
        let h = 1 + floor(&mut r, 100.0) as usize;
        let message = format!("objects/tree.json: {w}×{h}, max 64×64");
        assert_eq!(validate(&src(vec![("tree", sprite(rows_of(h, w, "g")))], vec![])), vec![message], "seed {seed}");
    }
}

// ------------------------------------------------------------------ sprites

#[test]
fn r1_dot_is_transparent_and_the_anchor_sits_on_the_placement_point() {
    let dot = sprite_k(["...", ".g."], json!({ "g": "grass-2" }), json!({ "anchor": [1, 1] }));
    let sources = src(vec![("dot", dot)], vec![]);
    let drawn: Vec<(Pixel, String)> = pixels_of(&sources, "dot").iter().map(|(p, c)| (*p, c.to_string())).collect();
    assert_eq!(drawn, vec![((0, 0), "grass-2".to_string())]);
}

#[test]
fn r2_extends_replaces_whole_rows_per_layer_then_merges_keys() {
    let objects = vec![
        (
            "character",
            json!({ "kind": "sprite", "anchor": [1, 0], "keys": { "o": "ink", "s": "cream" }, "layers": [{ "map": ["oo", "ss"] }, { "class": "cbob", "map": ["s.", ".s"] }] }),
        ),
        (
            "coat",
            json!({ "kind": "sprite", "extends": "character", "keys": { "s": "grass-2", "t": "teal-1" }, "rows": { "1": { "0": "tt" } } }),
        ),
    ];
    let sources = src(objects.clone(), vec![]);
    let coat = resolve(&sources, "coat").unwrap();
    let keys: Vec<(String, String)> = coat.keys.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    assert_eq!(
        keys,
        vec![("o".to_string(), "ink".to_string()), ("s".to_string(), "grass-2".to_string()), ("t".to_string(), "teal-1".to_string())]
    );
    use engine::engine::SpriteLayer;
    assert_eq!(
        coat.layers,
        vec![
            SpriteLayer::Map { class: None, map: vec!["oo".into(), "ss".into()] },
            SpriteLayer::Map { class: Some("cbob".into()), map: vec!["tt".into(), ".s".into()] },
        ]
    );
    assert_eq!(coat.anchor, [1, 0]);
    assert!(coat.character);
    assert_eq!(sources.objects["character"]["layers"][1]["map"], json!(["s.", ".s"]), "the base is not changed");
}

// ------------------------------------------------------------- scenes (R3)

#[test]
fn r3_items_paint_in_source_order_and_later_paint_wins_in_a_layer_swapped_order() {
    // The composite half of 'R3: items paint in source order…' (the SVG half
    // is in svg_starter.rs).
    let dot = sprite(["g"]);
    let ink = sprite_k(["i"], json!({ "i": "ink" }), json!({}));
    let items = json!([{ "object": "ink", "at": { "px": [1, 1] } }, { "object": "dot", "at": { "px": [1, 1] } }]);
    let sources = src(vec![("dot", dot), ("ink", ink)], vec![("s", scene(items))]);
    let drawn: Vec<(Pixel, String)> =
        composite(&root_of(&sources, "s"), CompositeOptions::default()).iter().map(|(p, c)| (*p, c.to_string())).collect();
    assert_eq!(drawn, vec![((1, 1), "grass-2".to_string())]);
}

// ----------------------------------------------------------- blocks (R28, D3, D4)

fn block() -> Value {
    json!({ "kind": "block", "size": [1, 1, 1], "faces": { "top": "grass-2", "left": "wood-3", "right": "grass-4", "edge": "ink" } })
}

fn flat() -> Value {
    json!({ "kind": "block", "size": [1, 1, 0], "faces": { "top": "grass-2" } })
}

#[test]
fn r28_the_top_is_the_light_shade_the_left_face_the_mid_shade_and_the_right_face_the_shadow_shade() {
    let sources = src(vec![("block", block())], vec![]);
    let faces = block_faces([1, 1, 1], [0, 0]);
    let drawn = pixels_of(&sources, "block");
    for p in &faces.top {
        if !faces.edge.contains(p) {
            assert_eq!(name_at(&drawn, *p), Some("grass-2"), "top {p:?}");
        }
    }
    for p in &faces.left {
        if !faces.edge.contains(p) {
            assert_eq!(name_at(&drawn, *p), Some("wood-3"), "left {p:?}");
        }
    }
    for p in &faces.right {
        if !faces.edge.contains(p) {
            assert_eq!(name_at(&drawn, *p), Some("grass-4"), "right {p:?}");
        }
    }
    for p in &faces.edge {
        assert_eq!(name_at(&drawn, *p), Some("ink"), "edge {p:?}");
    }
    assert_eq!(drawn.len(), faces.top.len() + faces.left.len() + faces.right.len());
}

#[test]
fn r3_a_block_is_placed_by_its_first_tiles_top_face_center_by_tile_or_by_pixel() {
    let items = json!([{ "object": "block", "at": { "tile": [2, 1, 1] } }, { "object": "block", "at": { "px": [5, -3] } }]);
    let sources = src(vec![("block", block())], vec![("s", scene_x(items, json!({ "origin": [10, 20] })))]);
    let placed = composite(&root_of(&sources, "s"), CompositeOptions::default());
    let [tx, ty] = tile_to_px([2.0, 1.0, 1.0], [10.0, 20.0]);
    let mut expected = block_faces([1, 1, 1], [tx as i32, ty as i32]).top;
    expected.extend(block_faces([1, 1, 1], [5, -3]).top);
    for p in expected {
        assert!(placed.has(&p), "{p:?} is drawn");
    }
}

#[test]
fn r5_a_surface_is_clipped_to_the_top_face_and_compiles_to_loop_prefix_n_groups_frame_0_first() {
    let map = |ch: &str| rows_of(16, 32, ch);
    let water = json!({ "kind": "block", "size": [1, 1, 0], "faces": { "top": "grass-2" }, "surface": { "keys": { "a": "cream", "b": "ink" }, "loop": "wf", "prefix": "w", "frames": [map("a"), map("b")] } });
    let sources = src(vec![("water", water)], vec![("s", scene(json!([{ "object": "water", "at": { "px": [0, 0] } }])))]);
    let root = root_of(&sources, "s");
    assert_eq!(kinds(&root), ["layer", "wf w0", "wf w1"]);
    let top: std::collections::HashSet<Pixel> = block_faces([1, 1, 0], [0, 0]).top.into_iter().collect();
    let (Child::Group(g1), Child::Group(g2)) = (&root.children[1], &root.children[2]) else { panic!("frame groups") };
    let Child::Layer(frame0) = &g1.children[0] else { panic!("frame 0 layer") };
    assert!(frame0.pixels.keys().all(|p| top.contains(p)), "frame 0 stays on the top face");
    assert_eq!(frame0.pixels.len(), top.len(), "frame 0 covers the whole top face");
    let flat = composite(&root, CompositeOptions::default());
    assert_eq!(name_at(&flat, (0, 0)), Some("cream"), "only frame 0 shows when flattened");
    assert!(g1.frame == Some(0) && g2.frame == Some(1));
}

#[test]
fn r5_a_surface_repeats_on_every_tile_of_a_larger_block_and_a_static_one_is_a_plain_layer() {
    let slab = json!({ "kind": "block", "size": [2, 1, 0], "faces": { "top": "grass-2" }, "surface": { "keys": { "a": "cream" }, "map": rows_of(16, 32, "a") } });
    let flat = pixels_of(&src(vec![("slab", slab)], vec![]), "slab");
    let top = block_faces([2, 1, 0], [0, 0]).top;
    assert_eq!(flat.len(), top.len());
    assert!(top.iter().all(|p| name_at(&flat, *p) == Some("cream")));
}

#[test]
fn r5_each_tiles_surface_is_clipped_to_that_tiles_own_diamond_not_the_whole_top() {
    // A map that paints only its bottom-right corner, which is outside its own tile's diamond.
    let map: Vec<String> = (0..16).map(|y| if y >= 14 { format!("{}aa", ".".repeat(30)) } else { ".".repeat(32) }).collect();
    let slab =
        json!({ "kind": "block", "size": [2, 1, 0], "faces": { "top": "grass-2" }, "surface": { "keys": { "a": "cream" }, "map": map } });
    let drawn = pixels_of(&src(vec![("slab", slab.clone())], vec![]), "slab");
    let cream: Vec<Pixel> = drawn.keys().filter(|p| name_at(&drawn, **p) == Some("cream")).copied().collect();
    assert_eq!(cream, vec![], "nothing outside a tile's own diamond is painted, even if the next tile covers it");
    // A map that paints a pixel inside its own diamond lands once per tile.
    let dot: Vec<String> =
        (0..16).map(|y| if y == 8 { format!("{}a{}", ".".repeat(16), ".".repeat(15)) } else { ".".repeat(32) }).collect();
    let dotted_slab = with(slab, json!({ "surface": { "keys": { "a": "cream" }, "map": dot } }));
    let dotted = pixels_of(&src(vec![("slab", dotted_slab)], vec![]), "slab");
    let mut cream: Vec<Pixel> = dotted.keys().filter(|p| name_at(&dotted, **p) == Some("cream")).copied().collect();
    cream.sort();
    assert_eq!(cream, vec![(0, 0), (16, 8)]);
}

#[test]
fn r5_the_edge_is_drawn_last_over_the_surface() {
    let slab = json!({ "kind": "block", "size": [1, 1, 0], "faces": { "top": "grass-2", "edge": "ink" }, "surface": { "keys": { "a": "cream" }, "loop": "wf", "prefix": "w", "frames": [rows_of(16, 32, "a")] } });
    let sources = src(vec![("slab", slab)], vec![("s", scene(json!([{ "object": "slab", "at": { "px": [0, 0] } }])))]);
    let root = root_of(&sources, "s");
    assert_eq!(kinds(&root), ["layer", "wf w0", "layer"]);
    let edge0 = block_faces([1, 1, 0], [0, 0]).edge[0];
    assert_eq!(name_at(&composite(&root, CompositeOptions::default()), edge0), Some("ink"));
}

#[test]
fn r30_a_block_with_a_bad_size_missing_faces_or_unknown_properties() {
    let bad = |obj: Value| validate(&src(vec![("blk", obj)], vec![]));
    check(bad(with(block(), json!({ "size": [1, 1] }))), &[ERR_SIZE]);
    check(bad(with(block(), json!({ "size": [0, 1, 1] }))), &[ERR_SIZE]);
    check(
        bad(with(block(), json!({ "faces": { "top": "grass-2" } }))),
        &[
            "objects/blk.json: faces.left is missing; a block with levels needs a left and a right face",
            "objects/blk.json: faces.right is missing; a block with levels needs a left and a right face",
        ],
    );
    check(
        bad(with(flat(), json!({ "faces": { "top": "grass-2", "left": "ink" } }))),
        &["objects/blk.json: faces.left: a flat block (0 levels) has no left face"],
    );
    check(
        bad(with(flat(), json!({ "faces": { "left": "ink" } }))),
        &["objects/blk.json: faces.top is missing", "objects/blk.json: faces.left: a flat block (0 levels) has no left face"],
    );
    check(bad(with(flat(), json!({ "faces": { "top": "grass-9" } }))), &["objects/blk.json: faces.top: 'grass-9' is not in the palette"]);
    check(
        bad(with(flat(), json!({ "faces": { "top": "grass-2", "side": "ink" } }))),
        &["objects/blk.json: faces: unknown face 'side'; use top, left, right or edge"],
    );
    check(bad(with(flat(), json!({ "keys": {} }))), &["objects/blk.json: unknown property 'keys'"]);
}

#[test]
fn r30_a_surface_with_a_ragged_map_an_unknown_key_or_a_bad_frame() {
    let blk = |surface: Value| validate(&src(vec![("blk", with(flat(), json!({ "surface": surface })))], vec![]));
    let a32 = |n: usize| rows_of(n, 32, "a");
    check(blk(json!({ "keys": { "a": "cream" }, "map": a32(16) })), &[]);
    check(blk(json!({ "keys": { "a": "cream" }, "map": a32(15) })), &["objects/blk.json: surface: 15 rows, expected 16, one tile"]);
    let mut ragged = a32(15);
    ragged.push("a".repeat(31));
    check(blk(json!({ "keys": { "a": "cream" }, "map": ragged })), &["objects/blk.json: surface, row 15: 31 wide, expected 32, one tile"]);
    let mut unknown = a32(15);
    unknown.push(format!("{}q", "a".repeat(31)));
    check(
        blk(json!({ "keys": { "a": "cream" }, "map": unknown })),
        &["objects/blk.json: surface, row 15, column 31: key 'q' is not in keys"],
    );
    check(blk(json!({ "keys": { "a": "nope" }, "map": a32(16) })), &["objects/blk.json: surface, key 'a': 'nope' is not in the palette"]);
    check(
        blk(json!({ "keys": { "a": "cream" }, "loop": "wf", "prefix": "w", "frames": [a32(16), a32(2)] })),
        &["objects/blk.json: surface, frame 1: 2 rows, expected 16, one tile"],
    );
    check(
        blk(json!({ "keys": { "a": "cream" }, "loop": "wf", "frames": [a32(16)] })),
        &["objects/blk.json: surface: prefix must be one lowercase class name"],
    );
    check(blk(json!({ "map": a32(16), "keys": { "a": "cream" }, "extra": 1 })), &["objects/blk.json: surface: unknown property 'extra'"]);
}

#[test]
fn r26_r27_a_block_may_use_world_colors_only_at_most_12_colors_and_64x64_pixels() {
    let blk = |obj: Value| validate(&src(vec![("blk", obj)], vec![]));
    let face = |name: &str| with(block(), json!({ "faces": { "top": name, "left": "wood-3", "right": "grass-4", "edge": "ink" } }));
    check(blk(face("c-6f8a55")), &["objects/blk.json: faces.top: 'c-6f8a55' is not in the palette"]);
    check(
        blk(face("teal-1")),
        &["objects/blk.json: faces.top: 'teal-1' is an outfit color; only objects that extend character may use it"],
    );
    check(blk(with(flat(), json!({ "size": [2, 2, 0] }))), &[]);
    check(blk(with(flat(), json!({ "size": [3, 3, 0] }))), &["objects/blk.json: 94×48, max 64×64"]);
    check(blk(with(block(), json!({ "size": [1, 1, 3] }))), &[]);
    check(blk(with(block(), json!({ "size": [1, 1, 4] }))), &["objects/blk.json: 30×80, max 64×64"]);
    let wide10 = wide(10, "#0000", 2, palette());
    let mut keys = Map::new();
    for i in 0..10 {
        keys.insert(i.to_string(), json!(format!("w-{i}")));
    }
    let mut map: Vec<String> = "0123456789".chars().map(|c| c.to_string().repeat(32)).collect();
    map.extend(rows_of(6, 32, "0"));
    let surface = json!({ "keys": Value::Object(keys), "map": map });
    check(
        validate(&src_p(vec![("blk", with(block(), json!({ "surface": surface })))], vec![], wide10)),
        &["objects/blk.json: uses 14 colors, max 12"],
    );
}

#[test]
fn r27_l4_a_blocks_size_is_checked_before_anything_is_drawn_so_a_typo_is_an_error_not_a_crash() {
    for seed in SEEDS {
        let mut r = rng(seed);
        let size = [1 + floor(&mut r, 5000.0), 1 + floor(&mut r, 5000.0), floor(&mut r, 500.0)];
        let w = (size[0] + size[1]) * 16 - 2;
        let h = (size[0] + size[1]) * 8 + size[2] * 16;
        let started = Instant::now();
        let problems = validate(&src(vec![("blk", with(block(), json!({ "size": size })))], vec![]));
        assert_eq!(problems, vec![format!("objects/blk.json: {w}×{h}, max 64×64")], "seed {seed}");
        assert!(started.elapsed().as_millis() < 1000, "seed {seed}: reported at once");
    }
}

#[test]
fn r30_a_sprite_cannot_extend_a_block() {
    let objects = vec![("blk", block()), ("sp", json!({ "kind": "sprite", "extends": "blk" }))];
    check(validate(&src(objects, vec![])), &["objects/sp.json: extends 'blk', which is a block; only a sprite can be extended"]);
}

#[test]
fn r27_check_names_a_blocks_kind_size_and_colors() {
    let sources = src(vec![("block", block())], vec![]);
    assert_eq!(describe_object(&sources, &resolve(&sources, "block").unwrap()), "block 1×1×1 · 30×32 · 4 colors (of 12) · 1 frame");
}

// ------------------------------------------- block sides (hero island detail spec R6)

/// One world color per letter: t, l and r for the faces, a to f for the side
/// textures. Pictures show faces in uppercase and textures in lowercase.
fn side_palette() -> Value {
    let mut world = Map::new();
    for (i, n) in "tlrabcdef".chars().enumerate() {
        world.insert(n.to_string(), json!(format!("#0000{:X}", 16 + i)));
    }
    json!({ "world": Value::Object(world), "outfit": {} })
}

fn side_map(mut cell: impl FnMut(usize, usize) -> Option<char>) -> Vec<String> {
    (0..16).map(|r| (0..16).map(|c| cell(r, c).unwrap_or('.')).collect()).collect()
}

/// A two-row band on the left (with column 15 and one speckle marked) and
/// one row on the right (with one speckle).
fn sides() -> Value {
    let keys: Map<String, Value> = "abcdef".chars().map(|k| (k.to_string(), json!(k.to_string()))).collect();
    let left = side_map(|r, c| match (r, c) {
        (0, _) => Some('a'),
        (1, _) => Some('b'),
        (_, 15) => Some('c'),
        (6, 3) => Some('d'),
        _ => None,
    });
    let right = side_map(|r, c| {
        if r == 0 {
            Some('e')
        } else if r == 4 && c == 10 {
            Some('f')
        } else {
            None
        }
    });
    json!({ "keys": Value::Object(keys), "left": left, "right": right })
}

fn cube(size: [i64; 3], extra: Value) -> Value {
    with(json!({ "kind": "block", "size": size, "faces": { "top": "t", "left": "l", "right": "r" }, "sides": sides() }), extra)
}

/// What an object paints, as text rows from `origin`, `width` wide and
/// `height` high, and how many pixels it paints.
fn picture(sources: &Sources, name: &str, (x0, y0): (i32, i32), width: usize, height: usize) -> (Vec<String>, usize) {
    let drawn = pixels_of(sources, name);
    let rows = (0..height)
        .map(|r| {
            (0..width)
                .map(|c| match name_at(&drawn, (x0 + c as i32, y0 + r as i32)) {
                    Some(n) if ["t", "l", "r"].contains(&n) => n.to_uppercase(),
                    Some(n) => n.to_string(),
                    None => ".".to_string(),
                })
                .collect::<String>()
        })
        .collect();
    (rows, drawn.len())
}

// Checked by eye: each band follows its column's top edge at 2:1, column 15
// shows only where a face is longer than one tile, and the map repeats on
// every level. Then frozen here.
const PICTURE_111: &[&str] = &[
    "..............TT..............",
    "............TTTTTT............",
    "..........TTTTTTTTTT..........",
    "........TTTTTTTTTTTTTT........",
    "......TTTTTTTTTTTTTTTTTT......",
    "....TTTTTTTTTTTTTTTTTTTTTT....",
    "..TTTTTTTTTTTTTTTTTTTTTTTTTT..",
    "TTTTTTTTTTTTTTTTTTTTTTTTTTTTTT",
    "TTTTTTTTTTTTTTTTTTTTTTTTTTTTTT",
    "aaTTTTTTTTTTTTTTTTTTTTTTTTTTee",
    "bbaaTTTTTTTTTTTTTTTTTTTTTTeeRR",
    "LLbbaaTTTTTTTTTTTTTTTTTTeeRRRR",
    "LLLLbbaaTTTTTTTTTTTTTTeeRRRRRR",
    "LLLLLLbbaaTTTTTTTTTTeeRRRRRRRR",
    "LLLLLLLLbbaaTTTTTTeeRRRRRRRRRR",
    "LLLLLLLLLLbbaaTTeeRRRRRRRfRRRR",
    "LLLdLLLLLLLLbbaeRRRRRRRRRRRRRR",
    "LLLLLLLLLLLLLLbRRRRRRRRRRRRRRR",
    "LLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "LLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "LLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "LLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "LLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "LLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "LLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "..LLLLLLLLLLLLLRRRRRRRRRRRRR..",
    "....LLLLLLLLLLLRRRRRRRRRRR....",
    "......LLLLLLLLLRRRRRRRRR......",
    "........LLLLLLLRRRRRRR........",
    "..........LLLLLRRRRR..........",
    "............LLLRRR............",
    "..............LR..............",
];

const PICTURE_212: &[&str] = &[
    "..............TT..............................",
    "............TTTTTT............................",
    "..........TTTTTTTTTT..........................",
    "........TTTTTTTTTTTTTT........................",
    "......TTTTTTTTTTTTTTTTTT......................",
    "....TTTTTTTTTTTTTTTTTTTTTT....................",
    "..TTTTTTTTTTTTTTTTTTTTTTTTTT..................",
    "TTTTTTTTTTTTTTTTTTTTTTTTTTTTTT................",
    "TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT..............",
    "aaTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT............",
    "bbaaTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT..........",
    "LLbbaaTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT........",
    "LLLLbbaaTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT......",
    "LLLLLLbbaaTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT....",
    "LLLLLLLLbbaaTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT..",
    "LLLLLLLLLLbbaaTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT",
    "LLLdLLLLLLLLbbaaTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT",
    "LLLLLLLLLLLLLLbbaaTTTTTTTTTTTTTTTTTTTTTTTTTTee",
    "LLLLLLLLLLLLLLLcbbaaTTTTTTTTTTTTTTTTTTTTTTeeRR",
    "LLLLLLLLLLLLLLLcLLbbaaTTTTTTTTTTTTTTTTTTeeRRRR",
    "LLLLLLLLLLLLLLLcLLLLbbaaTTTTTTTTTTTTTTeeRRRRRR",
    "LLLLLLLLLLLLLLLcLLLLLLbbaaTTTTTTTTTTeeRRRRRRRR",
    "LLLLLLLLLLLLLLLcLLLLLLLLbbaaTTTTTTeeRRRRRRRRRR",
    "LLLLLLLLLLLLLLLcLLLLLLLLLLbbaaTTeeRRRRRRRfRRRR",
    "LLLLLLLLLLLLLLLcLLLdLLLLLLLLbbaeRRRRRRRRRRRRRR",
    "aaLLLLLLLLLLLLLcLLLLLLLLLLLLLLbRRRRRRRRRRRRRRR",
    "bbaaLLLLLLLLLLLcLLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "LLbbaaLLLLLLLLLcLLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "LLLLbbaaLLLLLLLcLLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "LLLLLLbbaaLLLLLcLLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "LLLLLLLLbbaaLLLcLLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "LLLLLLLLLLbbaaLcLLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "LLLdLLLLLLLLbbaaLLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "LLLLLLLLLLLLLLbbaaLLLLLLLLLLLLLRRRRRRRRRRRRRee",
    "LLLLLLLLLLLLLLLcbbaaLLLLLLLLLLLRRRRRRRRRRReeRR",
    "LLLLLLLLLLLLLLLcLLbbaaLLLLLLLLLRRRRRRRRReeRRRR",
    "LLLLLLLLLLLLLLLcLLLLbbaaLLLLLLLRRRRRRReeRRRRRR",
    "LLLLLLLLLLLLLLLcLLLLLLbbaaLLLLLRRRRReeRRRRRRRR",
    "LLLLLLLLLLLLLLLcLLLLLLLLbbaaLLLRRReeRRRRRRRRRR",
    "LLLLLLLLLLLLLLLcLLLLLLLLLLbbaaLReeRRRRRRRfRRRR",
    "LLLLLLLLLLLLLLLcLLLdLLLLLLLLbbaeRRRRRRRRRRRRRR",
    "..LLLLLLLLLLLLLcLLLLLLLLLLLLLLbRRRRRRRRRRRRRRR",
    "....LLLLLLLLLLLcLLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "......LLLLLLLLLcLLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "........LLLLLLLcLLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "..........LLLLLcLLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "............LLLcLLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "..............LcLLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "................LLLLLLLLLLLLLLLRRRRRRRRRRRRRRR",
    "..................LLLLLLLLLLLLLRRRRRRRRRRRRR..",
    "....................LLLLLLLLLLLRRRRRRRRRRR....",
    "......................LLLLLLLLLRRRRRRRRR......",
    "........................LLLLLLLRRRRRRR........",
    "..........................LLLLLRRRRR..........",
    "............................LLLRRR............",
    "..............................LR..............",
];

#[test]
fn r6_pixel_exact_side_textures_for_1_1_1_and_2_1_2() {
    for (size, rows) in [([1, 1, 1], PICTURE_111), ([2, 1, 2], PICTURE_212)] {
        let sources = src_p(vec![("blk", cube(size, json!({})))], vec![], side_palette());
        let (got, count) = picture(&sources, "blk", (-15, -8), rows[0].len(), rows.len());
        assert_eq!(got, rows, "size {size:?}");
        let expected: usize = rows.iter().map(|r| r.chars().filter(|&c| c != '.').count()).sum();
        assert_eq!(count, expected, "size {size:?}: no pixel outside the picture");
    }
}

#[test]
fn r6_l4_a_cubes_side_maps_land_where_the_conversion_formulas_say_pixel_artist_agent_blocks() {
    for seed in SEEDS {
        let mut r = rng(seed);
        let mut random = || -> Vec<String> {
            (0..16)
                .map(|_| {
                    (0..16)
                        .map(|_| if r() < 0.4 { '.' } else { "abcdef".as_bytes()[(r() * 6.0).floor() as usize] as char })
                        .collect::<String>()
                })
                .collect()
        };
        let left = random();
        let right = random();
        let mut s = sides();
        s["left"] = json!(left);
        s["right"] = json!(right);
        let sources = src_p(vec![("blk", cube([1, 1, 1], json!({ "sides": s })))], vec![], side_palette());
        let drawn = pixels_of(&sources, "blk");
        // A 30×32 cube sprite anchored at [15, 8]: left column i, row r is sprite[9 + ⌊i/2⌋ + r][i],
        // and right column j, row r is sprite[16 − ⌈j/2⌉ + r][15 + j].
        let want = |map: &Vec<String>, row: usize, col: usize, face: &str| {
            let c = map[row].as_bytes()[col] as char;
            if c == '.' {
                face.to_string()
            } else {
                c.to_string()
            }
        };
        for col in 0..15usize {
            for row in 0..16usize {
                let (lx, ly) = (col as i32, 9 + (col / 2) as i32 + row as i32);
                let (rx, ry) = (15 + col as i32, 16 - col.div_ceil(2) as i32 + row as i32);
                assert_eq!(
                    name_at(&drawn, (lx - 15, ly - 8)),
                    Some(want(&left, row, col, "l").as_str()),
                    "seed {seed}: left column {col}, row {row}"
                );
                assert_eq!(
                    name_at(&drawn, (rx - 15, ry - 8)),
                    Some(want(&right, row, col, "r").as_str()),
                    "seed {seed}: right column {col}, row {row}"
                );
            }
        }
    }
}

#[test]
fn r6_sides_paint_after_the_faces_under_the_top_surface_and_the_edge() {
    let blk = cube(
        [1, 1, 1],
        json!({
            "faces": { "top": "t", "left": "l", "right": "r", "edge": "d" },
            "surface": { "keys": { "b": "b" }, "loop": "wf", "prefix": "w", "frames": [rows_of(16, 32, "b")] },
            "sides": { "keys": { "a": "a" }, "left": rows_of(16, 16, "a"), "right": rows_of(16, 16, "a") },
        }),
    );
    let sources = src_p(vec![("blk", blk)], vec![("s", scene(json!([{ "object": "blk", "at": { "px": [0, 0] } }])))], side_palette());
    let root = root_of(&sources, "s");
    assert_eq!(kinds(&root), ["layer", "wf w0", "layer"]);
    let drawn = composite(&root, CompositeOptions::default());
    let faces = block_faces([1, 1, 1], [0, 0]);
    let edge: std::collections::HashSet<Pixel> = faces.edge.iter().copied().collect();
    for p in &faces.edge {
        assert_eq!(name_at(&drawn, *p), Some("d"), "edge {p:?}");
    }
    for p in faces.left.iter().chain(&faces.right) {
        if !edge.contains(p) {
            assert_eq!(name_at(&drawn, *p), Some("a"), "side {p:?}");
        }
    }
    for p in &faces.top {
        if !edge.contains(p) {
            assert_eq!(name_at(&drawn, *p), Some("b"), "top {p:?}");
        }
    }
}

#[test]
fn r6_sides_are_refused_on_a_flat_block_and_need_16x16_maps_with_known_keys() {
    let bad = |sides: Value, extra: Value| {
        validate(&src_p(vec![("blk", cube([1, 1, 1], with(json!({ "sides": sides }), extra)))], vec![], side_palette()))
    };
    let s = sides();
    check(bad(s.clone(), json!({})), &[]);
    check(
        bad(s.clone(), json!({ "size": [1, 1, 0], "faces": { "top": "t" } })),
        &["objects/blk.json: sides: a flat block (0 levels) has no sides"],
    );
    check(
        bad(with(s.clone(), json!({ "left": rows_of(15, 16, ".") })), json!({})),
        &["objects/blk.json: sides.left: 15 rows, expected 16, one level"],
    );
    let mut right = rows_of(3, 16, ".");
    right.push(".".repeat(17));
    right.extend(rows_of(12, 16, "."));
    check(
        bad(with(s.clone(), json!({ "right": right })), json!({})),
        &["objects/blk.json: sides.right, row 3: 17 wide, expected 16, one tile's face"],
    );
    let mut left = vec![format!("z{}", ".".repeat(15))];
    left.extend(rows_of(15, 16, "."));
    check(
        bad(with(s.clone(), json!({ "left": left })), json!({})),
        &["objects/blk.json: sides.left, row 0, column 0: key 'z' is not in keys"],
    );
    let mut keys = s["keys"].clone();
    keys["a"] = json!("nope");
    check(bad(with(s.clone(), json!({ "keys": keys })), json!({})), &["objects/blk.json: sides, key 'a': 'nope' is not in the palette"]);
    check(bad(json!({ "keys": s["keys"].clone(), "left": s["left"].clone() }), json!({})), &["objects/blk.json: sides.right is missing"]);
    check(bad(with(s.clone(), json!({ "top": rows_of(16, 16, ".") })), json!({})), &["objects/blk.json: sides: unknown property 'top'"]);
    check(bad(json!("stripes"), json!({})), &["objects/blk.json: sides: must be { keys, left, right }"]);
}

#[test]
fn r6_r26_side_colors_are_world_colors_only_and_count_toward_the_12_color_cap() {
    let blk = |sides: Value| {
        let b = with(
            cube([1, 1, 1], json!({})),
            json!({ "faces": { "top": "grass-2", "left": "wood-3", "right": "grass-4" }, "sides": sides }),
        );
        validate(&src(vec![("blk", b)], vec![]))
    };
    let one = |color: &str| json!({ "keys": { "a": color }, "left": side_map(|r, _| if r == 0 { Some('a') } else { None }), "right": side_map(|_, _| None) });
    check(blk(one("cream")), &[]);
    check(blk(one("c-6f8a55")), &["objects/blk.json: sides, key 'a': 'c-6f8a55' is not in the palette"]);
    check(
        blk(one("teal-1")),
        &["objects/blk.json: sides, key 'a': 'teal-1' is an outfit color; only objects that extend character may use it"],
    );
    let wide10 = wide(10, "#0001", 2, side_palette());
    let mut keys = Map::new();
    for i in 0..10 {
        keys.insert(i.to_string(), json!(format!("w-{i}")));
    }
    let digit = |c: usize| char::from_digit(c as u32, 10);
    let many = json!({ "keys": Value::Object(keys.clone()), "left": side_map(|r, c| if r == 0 && c < 10 { digit(c) } else { None }), "right": side_map(|_, _| None) });
    check(
        validate(&src_p(vec![("blk", cube([1, 1, 1], json!({ "sides": many })))], vec![], wide10.clone())),
        &["objects/blk.json: uses 13 colors, max 12"],
    );
    let unused = json!({ "keys": Value::Object(keys), "left": side_map(|r, c| if r == 0 && c < 9 { digit(c) } else { None }), "right": side_map(|_, _| None) });
    check(validate(&src_p(vec![("blk", cube([1, 1, 1], json!({ "sides": unused })))], vec![], wide10)), &[]);
    // a key no map uses is not counted
}

#[test]
fn r27_check_counts_a_blocks_side_colors() {
    let sources = src_p(vec![("blk", cube([1, 1, 1], json!({})))], vec![], side_palette());
    assert_eq!(describe_object(&sources, &resolve(&sources, "blk").unwrap()), "block 1×1×1 · 30×32 · 9 colors (of 12) · 1 frame");
}

// ---------------------------------------------------------- serializer (R9)
//
// The Node serializer wrote `.mjs` text; format 2 writes JSON, so each test
// keeps its intent on `engine::format::serialize` and `parse`: the canonical
// text for a document, and that a document survives a save and a reload
// byte for byte.

/// Parses canonical text back, the way a reload reads a file.
fn load(text: &str) -> Value {
    parse(text).expect("canonical text parses")
}

#[test]
fn r9_writes_the_canonical_text_for_a_sprite() {
    // Adapted: the expected text is format 2's (JSON, a trailing newline, short
    // arrays such as `anchor` on one line) instead of the `.mjs` module.
    let obj = json!({
        "kind": "sprite",
        "anchor": [5, 13],
        "keys": { "L": "grass-2", "'": "ink" },
        "layers": [{ "map": [".L.", "'L'"] }, { "class": "cbob", "map": ["...", ".L."] }, { "loop": "wf", "prefix": "w", "frames": [["L..", "..."], [".L.", "..."]] }],
    });
    let expected = r#"{
  "kind": "sprite",
  "anchor": [5, 13],
  "keys": {
    "L": "grass-2",
    "'": "ink"
  },
  "layers": [
    {
      "map": [
        ".L.",
        "'L'"
      ]
    },
    {
      "class": "cbob",
      "map": [
        "...",
        ".L."
      ]
    },
    {
      "loop": "wf",
      "prefix": "w",
      "frames": [
        [
          "L..",
          "..."
        ],
        [
          ".L.",
          "..."
        ]
      ]
    }
  ]
}
"#;
    assert_eq!(serialize(&obj), expected);
}

#[test]
fn r9_writes_the_canonical_text_for_an_outfit_and_a_scene() {
    // Adapted as above. The outfit's `rows` keys are written as JSON strings,
    // in the order a JavaScript object lists them (whole numbers ascending).
    let outfit = json!({ "kind": "sprite", "extends": "character", "keys": {}, "rows": { "0": { "3": "ab", "10": "cd" }, "1": {} } });
    assert_eq!(
        serialize(&outfit),
        r#"{
  "kind": "sprite",
  "extends": "character",
  "keys": {},
  "rows": {
    "0": {
      "3": "ab",
      "10": "cd"
    },
    "1": {}
  }
}
"#
    );
    let s = json!({
        "output": "range-sprite.svg",
        "viewBox": [-51, -9, 103, 72],
        "origin": [0, 32],
        "items": [
            { "object": "range-island", "at": { "px": [-51, -9] } },
            { "group": { "data-class": "0" }, "items": [{ "object": "outfit-front-end", "class": "a b", "at": { "tile": [1, -2, 0] } }] },
            { "group": {}, "items": [] },
        ],
    });
    assert_eq!(
        serialize(&s),
        r#"{
  "output": "range-sprite.svg",
  "viewBox": [-51, -9, 103, 72],
  "origin": [0, 32],
  "items": [
    { "object": "range-island", "at": { "px": [-51, -9] } },
    {
      "group": { "data-class": "0" },
      "items": [
        { "object": "outfit-front-end", "class": "a b", "at": { "tile": [1, -2, 0] } }
      ]
    },
    {
      "group": {},
      "items": []
    }
  ]
}
"#
    );
}

#[test]
fn r9_l4_random_objects_and_scenes_survive_save_and_reload_byte_for_byte() {
    let chars: Vec<char> = "abcXYZ019#@'\\\"%!~".chars().collect();
    for seed in SEEDS {
        let mut r = rng(seed);
        let w = 1 + floor(&mut r, 64.0) as usize;
        let h = 1 + floor(&mut r, 24.0) as usize;
        let key_count = 1 + floor(&mut r, 12.0);
        let mut key_list: Vec<char> = Vec::new();
        for _ in 0..key_count {
            let k = chars[floor(&mut r, chars.len() as f64) as usize];
            if !key_list.contains(&k) {
                key_list.push(k);
            }
        }
        let map = |r: &mut dyn FnMut() -> f64| -> Vec<String> {
            (0..h)
                .map(|_| {
                    (0..w)
                        .map(|_| if r() < 0.3 { '.' } else { key_list[(r() * key_list.len() as f64).floor() as usize] })
                        .collect::<String>()
                })
                .collect()
        };
        let mut layers = vec![json!({ "map": map(&mut r) })];
        if r() < 0.5 {
            layers.push(json!({ "class": "cbob", "map": map(&mut r) }));
        }
        if r() < 0.5 {
            let n = 1 + floor(&mut r, 5.0);
            let frames: Vec<Vec<String>> = (0..n).map(|_| map(&mut r)).collect();
            layers.push(json!({ "loop": "hf", "prefix": "h", "frames": frames }));
        }
        let anchor = [floor(&mut r, w as f64), floor(&mut r, h as f64)];
        let keys: Map<String, Value> = key_list.iter().map(|k| (k.to_string(), json!("ink"))).collect();
        let obj = json!({ "kind": "sprite", "anchor": anchor, "keys": Value::Object(keys), "layers": layers });
        let text = serialize(&obj);
        let loaded = load(&text);
        assert_eq!(loaded, obj, "seed {seed}");
        assert_eq!(stringify(&loaded), stringify(&obj), "seed {seed}: same key order");
        assert_eq!(serialize(&loaded), text, "seed {seed}");

        let item = |r: &mut dyn FnMut() -> f64| -> Value {
            if r() < 0.5 {
                let (x, y) = (floor(&mut *r, 200.0) - 100, floor(&mut *r, 200.0) - 100);
                json!({ "object": "dot", "at": { "px": [x, y] } })
            } else {
                let (c, d, l) = (floor(&mut *r, 9.0) - 4, floor(&mut *r, 9.0), floor(&mut *r, 3.0));
                json!({ "object": "dot", "class": "pcloud pc1", "at": { "tile": [c, d, l] } })
            }
        };
        let first = item(&mut r);
        let second = item(&mut r);
        let third = item(&mut r);
        let fourth = item(&mut r);
        let s = json!({ "viewBox": [-10, -10, 20, 20], "items": [first, { "group": { "data-class": seed.to_string() }, "items": [second, third] }, fourth] });
        let scene_text = serialize(&s);
        assert_eq!(serialize(&load(&scene_text)), scene_text, "seed {seed}");
        assert_eq!(load(&scene_text), s, "seed {seed}");
        assert_eq!(stringify(&load(&scene_text)), stringify(&s), "seed {seed}: same key order");
    }
}

#[test]
fn r9_a_block_serializes_to_its_canonical_text_with_and_without_a_surface() {
    // Adapted: the block's JSON text, line by line, as the Node test checked
    // the `.mjs` lines (no header comment line in format 2).
    let rows: Vec<String> = (0..16).map(|i| format!("{}a{}", ".".repeat(i), ".".repeat(31 - i))).collect();
    let text = serialize(
        &json!({ "kind": "block", "size": [1, 1, 0], "faces": { "top": "water-2" }, "surface": { "keys": { "a": "water-1" }, "loop": "wf", "prefix": "w", "frames": [rows.clone()] } }),
    );
    let lines: Vec<&str> = text.split('\n').collect();
    assert_eq!(
        lines[..15],
        [
            "{",
            "  \"kind\": \"block\",",
            "  \"size\": [1, 1, 0],",
            "  \"faces\": {",
            "    \"top\": \"water-2\"",
            "  },",
            "  \"surface\": {",
            "    \"keys\": {",
            "      \"a\": \"water-1\"",
            "    },",
            "    \"loop\": \"wf\",",
            "    \"prefix\": \"w\",",
            "    \"frames\": [",
            "      [",
            &format!("        \"{}\",", rows[0]),
        ]
    );
    assert_eq!(lines[lines.len() - 5..].join("\n"), "      ]\n    ]\n  }\n}\n");
    let plain = serialize(&json!({ "kind": "block", "size": [1, 1, 1], "faces": { "top": "a", "left": "b", "right": "c", "edge": "d" } }));
    assert_eq!(
        plain,
        "{\n  \"kind\": \"block\",\n  \"size\": [1, 1, 1],\n  \"faces\": {\n    \"top\": \"a\",\n    \"left\": \"b\",\n    \"right\": \"c\",\n    \"edge\": \"d\"\n  }\n}\n"
    );
    let still = serialize(
        &json!({ "kind": "block", "size": [1, 1, 0], "faces": { "top": "a" }, "surface": { "keys": { "a": "b" }, "map": ["a"] } }),
    );
    assert!(still.contains("  \"surface\": {\n    \"keys\": {\n      \"a\": \"b\"\n    },\n    \"map\": [\n      \"a\"\n    ]\n  }\n"));
}

#[test]
fn r9_l4_a_random_block_with_and_without_sides_survives_save_and_reload_byte_for_byte() {
    let mut with_sides = 0;
    for seed in SEEDS {
        let mut r = rng(seed);
        let size = [1 + floor(&mut r, 3.0), 1 + floor(&mut r, 3.0), floor(&mut r, 3.0)];
        let mut obj = json!({ "kind": "block", "size": size, "faces": { "top": "grass-2" } });
        if size[2] > 0 {
            obj["faces"]["left"] = json!("wood-3");
            obj["faces"]["right"] = json!("grass-4");
        }
        if r() < 0.5 {
            obj["faces"]["edge"] = json!("ink");
        }
        if size[2] > 0 {
            let mut map = || -> Vec<String> {
                (0..16)
                    .map(|_| {
                        (0..16)
                            .map(|_| if r() < 0.5 { '.' } else { "ab'%".as_bytes()[(r() * 4.0).floor() as usize] as char })
                            .collect::<String>()
                    })
                    .collect()
            };
            let left = map();
            let right = map();
            obj["sides"] = json!({ "keys": { "a": "cream", "b": "ink", "'": "grass-4", "%": "wood-3" }, "left": left, "right": right });
            with_sides += 1;
        }
        let text = serialize(&obj);
        let loaded = load(&text);
        assert_eq!(loaded, obj, "seed {seed}");
        assert_eq!(stringify(&loaded), stringify(&obj), "seed {seed}: same key order");
        assert_eq!(serialize(&loaded), text, "seed {seed}");
        check(validate(&src(vec![("blk", loaded)], vec![])), &[]);
    }
    assert!(with_sides > 0, "some random blocks have sides");
}

#[test]
fn r9_a_block_with_sides_serializes_to_its_canonical_text_sides_after_the_surface() {
    // Adapted as the block test above: JSON lines, no header comment, and the
    // slice offsets are those of the JSON layout.
    let surface = json!({ "keys": { "a": "cream" }, "map": rows_of(16, 32, ".") });
    let s = sides();
    let text = serialize(
        &json!({ "kind": "block", "size": [1, 1, 1], "faces": { "top": "t", "left": "l", "right": "r" }, "surface": surface, "sides": s }),
    );
    let lines: Vec<&str> = text.split('\n').collect();
    let at = lines.iter().position(|l| *l == "  \"sides\": {").unwrap();
    assert!(at > lines.iter().position(|l| *l == "  \"surface\": {").unwrap(), "sides come after the surface");
    let left0 = s["left"][0].as_str().unwrap();
    assert_eq!(
        lines[at..at + 11],
        [
            "  \"sides\": {",
            "    \"keys\": {",
            "      \"a\": \"a\",",
            "      \"b\": \"b\",",
            "      \"c\": \"c\",",
            "      \"d\": \"d\",",
            "      \"e\": \"e\",",
            "      \"f\": \"f\"",
            "    },",
            "    \"left\": [",
            &format!("      \"{left0}\","),
        ]
    );
    let right0 = s["right"][0].as_str().unwrap();
    assert_eq!(lines[at + 26..at + 29], ["    ],", "    \"right\": [", &format!("      \"{right0}\",")]);
    assert_eq!(lines[lines.len() - 4..].join("\n"), "    ]\n  }\n}\n");
}
