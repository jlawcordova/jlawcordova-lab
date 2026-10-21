//! Rendering: valid PNGs at the right size, transparent backgrounds, and item
//! bounds that match the pixels each item paints, in output pixels (ports
//! cli/test/render.test.mjs).
//!
//! The Node suite ran on a site with a pine, a tile, a water and a character
//! sprite; these run on the format-2 fixture plus the objects and scenes
//! each test adds (`with`). Tests that go through the CLI are written and
//! ignored until it exists. The parity test compares every result with what
//! Node's `renderSceneImage` and `renderObjectImage` gave for the format-1
//! fixture (tests/data/render/expected.json, made by the script named in
//! the report).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use engine::engine::{load_sources, validate, Sources};
use engine::format::read_docs;
use engine::iso;
use engine::omap::OSet;
use engine::png::{decode, Image};
use pixellab::render::*;
use serde_json::{json, Value};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn fixture() -> PathBuf {
    repo().join("fixtures/sample.pixellab")
}

fn data(file: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/render").join(file);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The fixture's sources, plus these objects and scenes.
fn with(objects: &[(&str, Value)], scenes: &[(&str, Value)]) -> Sources {
    let docs = read_docs(&fixture());
    assert!(docs.load_errors.is_empty(), "{:?}", docs.load_errors);
    let mut objs: BTreeMap<String, Value> = docs.objects;
    let mut scns: BTreeMap<String, Value> = docs.scenes;
    for (n, v) in objects {
        objs.insert(n.to_string(), v.clone());
    }
    for (n, v) in scenes {
        scns.insert(n.to_string(), v.clone());
    }
    load_sources(docs.palette.unwrap(), objs, scns)
}

/// The fixture and the scenes the parity script added.
fn with_extras() -> Sources {
    let extra = data("extra-scenes.json");
    let scenes: Vec<(String, Value)> = extra.as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let refs: Vec<(&str, Value)> = scenes.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    with(&[], &refs)
}

/// A single-item scene.
fn scene(items: Value, view_box: Value, extra: Value) -> Value {
    let mut s = json!({ "viewBox": view_box, "origin": [0, 0], "items": items });
    if let Some(e) = extra.as_object() {
        for (k, v) in e {
            s[k] = v.clone();
        }
    }
    s
}

fn view80() -> Value {
    json!([-40, -40, 80, 80])
}

struct Png {
    r: Value,
    image: Image,
    types: Vec<String>,
    color_type: u8,
    bit_depth: u8,
}

impl Png {
    fn alpha(&self, x: u32, y: u32) -> u8 {
        self.image.rgba[((y * self.image.width + x) * 4 + 3) as usize]
    }
    fn rgba(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.image.width + x) * 4) as usize;
        self.image.rgba[i..i + 4].try_into().unwrap()
    }
    /// The box around every non-transparent pixel, as [x, y, w, h].
    fn opaque_box(&self) -> Option<[u32; 4]> {
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0, 0);
        let mut any = false;
        for y in 0..self.image.height {
            for x in 0..self.image.width {
                if self.alpha(x, y) == 0 {
                    continue;
                }
                any = true;
                (min_x, min_y, max_x, max_y) = (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y));
            }
        }
        any.then(|| [min_x, min_y, max_x - min_x + 1, max_y - min_y + 1])
    }
}

fn unbase64(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let mut out = Vec::new();
    for chunk in text.as_bytes().chunks(4) {
        let n: Vec<u32> = chunk.iter().filter(|&&c| c != b'=').map(|&c| value(c) as u32).collect();
        let bits = n.iter().fold(0u32, |a, &v| a << 6 | v) << (6 * (4 - n.len()));
        for i in 0..n.len() - 1 {
            out.push((bits >> (16 - 8 * i)) as u8);
        }
    }
    out
}

/// Decodes a render's PNG, checking its chunk layout on the way.
fn decode_png(r: &Value, key: &str) -> Png {
    let bytes = unbase64(r[key].as_str().unwrap());
    let mut types = Vec::new();
    let mut off = 8;
    while off < bytes.len() {
        let len = u32::from_be_bytes(bytes[off..off + 4].try_into().unwrap()) as usize;
        types.push(String::from_utf8_lossy(&bytes[off + 4..off + 8]).into_owned());
        off += 12 + len;
    }
    Png { r: r.clone(), image: decode(&bytes).unwrap(), types, bit_depth: bytes[24], color_type: bytes[25] }
}

fn render(sources: &Sources, name: &str, options: SceneImageOptions) -> Png {
    let r = render_scene_image(sources, name, "#1C1712", &options).unwrap_or_else(|e| panic!("{name}: {e}"));
    decode_png(&r, "png")
}

fn render_at(sources: &Sources, name: &str, scale: u32) -> Png {
    render(sources, name, SceneImageOptions { scale, ..SceneImageOptions::default() })
}

fn object(sources: &Sources, name: &str, options: ObjectImageOptions) -> Png {
    let r = render_object_image(sources, name, &options).unwrap_or_else(|e| panic!("{name}: {e}"));
    decode_png(&r, "png")
}

fn ints(v: &Value) -> Vec<i64> {
    v.as_array().unwrap().iter().map(|n| n.as_i64().unwrap()).collect()
}

fn pebble_box(sources: &Sources) -> (i64, i64, i64, i64, [i64; 2]) {
    let pebble = &sources.objects["pebble"];
    let map: Vec<&str> = pebble["layers"][0]["map"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect();
    let cols: Vec<i64> = map.iter().flat_map(|row| row.chars().enumerate().filter(|(_, c)| *c != '.').map(|(x, _)| x as i64)).collect();
    let rows: Vec<i64> = map.iter().enumerate().filter(|(_, r)| r.chars().any(|c| c != '.')).map(|(y, _)| y as i64).collect();
    let anchor = ints(&pebble["anchor"]);
    (
        *cols.iter().min().unwrap(),
        *cols.iter().max().unwrap(),
        *rows.iter().min().unwrap(),
        *rows.iter().max().unwrap(),
        [anchor[0], anchor[1]],
    )
}

fn zz_sources() -> Sources {
    with(
        &[],
        &[
            ("zz-pine", scene(json!([{ "object": "pebble", "at": { "tile": [0, 0, 0] } }]), view80(), json!({}))),
            ("zz-px", scene(json!([{ "object": "pebble", "at": { "px": [-13, 21] } }]), view80(), json!({}))),
            (
                "zz-two",
                scene(
                    json!([{ "object": "pebble", "at": { "px": [-20, 0] } }, { "object": "pebble", "id": "mover", "at": { "px": [20, 0] } }]),
                    view80(),
                    json!({ "motion": { "duration": 1000, "tracks": [{ "item": "mover", "keys": [{ "t": 0 }, { "t": 500, "move": [0, 10], "show": true }] }] } }),
                ),
            ),
            (
                "zz-hide",
                scene(
                    json!([{ "object": "pebble", "id": "blink", "at": { "px": [0, 0] } }]),
                    view80(),
                    json!({ "motion": { "duration": 1000, "tracks": [{ "item": "blink", "keys": [{ "t": 0 }, { "t": 500, "show": false }] }] } }),
                ),
            ),
        ],
    )
}

#[test]
fn the_fixtures_are_valid_sources() {
    let sources = zz_sources();
    for name in ["zz-pine", "zz-px", "zz-two", "zz-hide"] {
        let problems: Vec<String> = validate(&sources).into_iter().filter(|p| p.contains(name)).collect();
        assert!(problems.is_empty(), "{name}: {problems:?}");
    }
    let extras = with_extras();
    assert_eq!(validate(&extras), Vec::<String>::new());
}

#[test]
fn a_png_signature_ihdr_size_at_the_scale_rgba_transparent_background() {
    let sources = with_extras();
    for scale in [1, 2, 3] {
        let png = render_at(&sources, "sample", scale);
        let vb = ints(&sources.scenes["sample"]["viewBox"]);
        assert_eq!(png.types, ["IHDR", "IDAT", "IEND"]);
        assert_eq!(png.image.width as i64, vb[2] * scale as i64);
        assert_eq!(png.image.height as i64, vb[3] * scale as i64);
        assert_eq!(png.r["width"], json!(png.image.width));
        assert_eq!(png.r["height"], json!(png.image.height));
        assert_eq!(png.r["viewBox"], sources.scenes["sample"]["viewBox"]);
        assert_eq!(png.r["scale"], json!(scale));
        assert_eq!(png.color_type, 6);
        assert_eq!(png.bit_depth, 8);
        assert_eq!(png.alpha(0, 0), 0, "the corner is transparent");
        assert!(png.opaque_box().is_some(), "something is painted");
    }
}

#[test]
fn a_tile_placed_sprites_bounds_tile_to_px_minus_its_anchor_plus_its_painted_box() {
    let sources = zz_sources();
    let (min_c, max_c, min_r, max_r, anchor) = pebble_box(&sources);
    for scale in [1u32, 2, 4] {
        let png = render_at(&sources, "zz-pine", scale);
        let s = scale as i64;
        // tile [0, 0, 0] at origin [0, 0] is (0, 0); the viewBox starts at (-40, -40).
        let expected =
            [(0 - anchor[0] + min_c + 40) * s, (0 - anchor[1] + min_r + 40) * s, (max_c - min_c + 1) * s, (max_r - min_r + 1) * s];
        let items = png.r["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0], json!({ "path": "0", "object": "pebble", "at": { "tile": [0, 0, 0] }, "bounds": expected }));
        assert_eq!(png.opaque_box().map(|b| b.map(i64::from)), Some(expected));
        // Every painted pixel is one of the pebble's colors, fully opaque.
        let pebble = &sources.objects["pebble"];
        let rgbs: Vec<[u8; 4]> = pebble["keys"]
            .as_object()
            .unwrap()
            .values()
            .map(|name| {
                let hex = &sources.colors.get(name.as_str().unwrap()).unwrap().hex;
                let p = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap();
                [p(1), p(3), p(5), 255]
            })
            .collect();
        for y in expected[1]..expected[1] + expected[3] {
            for x in expected[0]..expected[0] + expected[2] {
                if png.alpha(x as u32, y as u32) != 0 {
                    assert!(rgbs.contains(&png.rgba(x as u32, y as u32)), "pixel {x},{y}");
                }
            }
        }
    }
}

#[test]
fn a_flat_tile_block_30_16_around_its_top_face_center() {
    let sources = with(
        &[("tile", json!({ "kind": "block", "size": [1, 1, 0], "faces": { "top": "grass-2" } }))],
        &[("zz-tile", scene(json!([{ "object": "tile", "at": { "tile": [1, 0, 0] } }]), view80(), json!({})))],
    );
    let png = render_at(&sources, "zz-tile", 2);
    let [cx, cy] = iso::tile_to_px([1.0, 0.0, 0.0], [0.0, 0.0]);
    let expected = [(cx as i64 - 15 + 40) * 2, (cy as i64 - 8 + 40) * 2, 30 * 2, 16 * 2];
    assert_eq!(ints(&png.r["items"][0]["bounds"]), expected);
    assert_eq!(png.opaque_box().map(|b| b.map(i64::from)), Some(expected));
}

#[test]
fn a_px_placed_item_at_px_minus_its_anchor() {
    let sources = zz_sources();
    let at_tile = render_at(&sources, "zz-pine", 1);
    let png = render_at(&sources, "zz-px", 1);
    let (dx, dy) = (-13, 21);
    let b = ints(&at_tile.r["items"][0]["bounds"]);
    // Clip to the 80×80 view, as the PNG is.
    let x0 = 0.max(b[0] + dx);
    let y0 = 0.max(b[1] + dy);
    let x1 = 80.min(b[0] + dx + b[2]);
    let y1 = 80.min(b[1] + dy + b[3]);
    assert_eq!(ints(&png.r["items"][0]["bounds"]), [b[0] + dx, b[1] + dy, b[2], b[3]]);
    assert_eq!(png.opaque_box().map(|b| b.map(i64::from)), Some([x0, y0, x1 - x0, y1 - y0]));
}

#[test]
fn every_painted_pixel_of_a_real_scene_is_inside_some_item_bounds() {
    let sources = with_extras();
    let cases: [(&str, f64, u32); 7] = [
        ("sample", 0.0, 2),
        ("sample-preview", 0.0, 1),
        ("zz-group", 0.0, 2),
        ("zz-layers", 0.0, 1),
        ("zz-two", 0.0, 1),
        ("zz-two", 500.0, 1),
        ("zz-hide", 700.0, 1),
    ];
    for (name, time, scale) in cases {
        let png = render(&sources, name, SceneImageOptions { scale, time, ..SceneImageOptions::default() });
        let boxes: Vec<Vec<i64>> =
            png.r["items"].as_array().unwrap().iter().filter(|i| !i["bounds"].is_null()).map(|i| ints(&i["bounds"])).collect();
        let mut uncovered = 0;
        for y in 0..png.image.height {
            for x in 0..png.image.width {
                if png.alpha(x, y) == 0 {
                    continue;
                }
                let (x, y) = (x as i64, y as i64);
                if !boxes.iter().any(|b| x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3]) {
                    uncovered += 1;
                }
            }
        }
        assert_eq!(uncovered, 0, "{name} {time}");
        // Every bounds box is whole pixels at the scale.
        for b in &boxes {
            assert_eq!(b[2] % scale as i64, 0);
        }
    }
}

#[test]
fn theme_dark_the_islands_drop_shadow_takes_the_dark_page_color_nothing_else_changes() {
    let sources = with_extras();
    let light = render(&sources, "zz-layers", SceneImageOptions { scale: 2, ..SceneImageOptions::default() });
    let dark = render(&sources, "zz-layers", SceneImageOptions { scale: 2, theme: Theme::Dark, ..SceneImageOptions::default() });
    let groups = flatten_items(&sources.scenes["zz-layers"]);
    let shadow_groups: Vec<String> = groups["groups"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|g| g["attrs"]["data-enter"] == "shadow")
        .map(|g| format!("{}.", g["path"].as_str().unwrap()))
        .collect();
    let shadows: Vec<Vec<i64>> = light.r["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| shadow_groups.iter().any(|g| i["path"].as_str().unwrap().starts_with(g.as_str())))
        .map(|i| ints(&i["bounds"]))
        .collect();
    assert!(!shadows.is_empty());
    let mut changed = 0;
    for y in 0..light.image.height {
        for x in 0..light.image.width {
            let (a, b) = (light.rgba(x, y), dark.rgba(x, y));
            if a == b {
                continue;
            }
            changed += 1;
            assert_eq!(b, [0x1c, 0x17, 0x12, 255], "pixel {x},{y}");
            let (xi, yi) = (x as i64, y as i64);
            assert!(
                shadows.iter().any(|q| xi >= q[0] && xi < q[0] + q[2] && yi >= q[1] && yi < q[1] + q[3]),
                "pixel {x},{y} outside the shadow"
            );
        }
    }
    assert!(changed > 10, "{changed} pixels changed");
    assert_eq!(dark.r["theme"], "dark");
}

#[test]
fn time_moves_a_tracked_item_its_bounds_follow_and_hidden_items_drop_out() {
    let sources = zz_sources();
    let at = |name: &str, time: f64| render(&sources, name, SceneImageOptions { time, ..SceneImageOptions::default() });
    let rest = at("zz-two", 0.0);
    let moved = at("zz-two", 500.0);
    let (a, b) = (&rest.r["items"][1], &moved.r["items"][1]);
    assert_eq!(a["id"], "mover");
    let (ab, bb) = (ints(&a["bounds"]), ints(&b["bounds"]));
    assert_eq!(bb, [ab[0], ab[1] + 10, ab[2], ab[3]]);
    assert_eq!(moved.r["items"][0]["bounds"], rest.r["items"][0]["bounds"], "the untracked item stays");
    assert_ne!(moved.opaque_box(), rest.opaque_box());
    assert_eq!(moved.r["time"], 500);

    let shown = at("zz-hide", 0.0);
    let hidden = at("zz-hide", 700.0);
    assert_eq!(shown.r["items"].as_array().unwrap().len(), 1);
    assert_eq!(hidden.r["items"].as_array().unwrap().len(), 0);
    assert_eq!(hidden.opaque_box(), None);
}

#[test]
fn time_changes_frame_loops() {
    let sources = with(
        &[(
            "water",
            json!({
                "kind": "sprite",
                "anchor": [0, 0],
                "keys": { "a": "water-2", "b": "cream" },
                "layers": [{ "loop": "wf", "prefix": "w", "frames": [["aab", "baa"], ["baa", "aab"]] }]
            }),
        )],
        &[],
    );
    let mut frames = std::collections::BTreeSet::new();
    for time in [0.0, 200.0, 400.0, 600.0, 800.0] {
        let r = render_object_image(&sources, "water", &ObjectImageOptions { time: Some(time), ..ObjectImageOptions::default() }).unwrap();
        frames.insert(r["png"].as_str().unwrap().to_string());
    }
    assert!(frames.len() > 1, "water looks different over time");
    let f0 = render_object_image(&sources, "water", &ObjectImageOptions { frame: 0, ..ObjectImageOptions::default() }).unwrap();
    let t0 = render_object_image(&sources, "water", &ObjectImageOptions { time: Some(0.0), ..ObjectImageOptions::default() }).unwrap();
    assert_eq!(f0["png"], t0["png"]);
}

#[test]
fn variants_the_first_shows_or_the_one_holding_show() {
    let sources = with_extras();
    let flat = flatten_items(&sources.scenes["zz-group"]);
    let variants: Vec<&Value> =
        flat["groups"].as_array().unwrap().iter().filter(|g| g["attrs"].as_object().unwrap().contains_key("data-class")).collect();
    assert!(variants.len() > 1);
    let second = variants[1]["path"].as_str().unwrap();
    let first = render_at(&sources, "zz-group", 1);
    let shown: Vec<&str> = first.r["items"].as_array().unwrap().iter().map(|i| i["path"].as_str().unwrap()).collect();
    assert!(!shown.iter().any(|p| p.starts_with(&format!("{second}."))), "the second variant is hidden by default");
    let in_second =
        flat["items"].as_array().unwrap().iter().find(|i| i["path"].as_str().unwrap().starts_with(&format!("{second}."))).unwrap();
    let path = in_second["path"].as_str().unwrap();
    let picked = render(&sources, "zz-group", SceneImageOptions { show: parse_path(path), ..SceneImageOptions::default() });
    assert!(picked.r["items"].as_array().unwrap().iter().any(|i| i["path"] == path));
    assert_ne!(picked.r["png"], first.r["png"]);
}

#[test]
fn object_render_a_sprites_viewbox_is_its_map_the_anchor_at_0_0() {
    let sources = with_extras();
    let png = object(&sources, "pebble", ObjectImageOptions { scale: 2, ..ObjectImageOptions::default() });
    let pebble = &sources.objects["pebble"];
    let map: Vec<&str> = pebble["layers"][0]["map"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect();
    let anchor = ints(&pebble["anchor"]);
    assert_eq!(ints(&png.r["viewBox"]), [-anchor[0], -anchor[1], map[0].len() as i64, map.len() as i64]);
    assert_eq!(png.image.width as i64, png.r["viewBox"][2].as_i64().unwrap() * 2);
    // Map pixel (x, y) is PNG pixel (x, y) at 1x: check every cell's alpha.
    let one = object(&sources, "pebble", ObjectImageOptions::default());
    for (y, row) in map.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            assert_eq!(one.alpha(x as u32, y as u32) > 0, c != '.', "pixel {x},{y}");
        }
    }
    assert_eq!(ints(&png.r["items"][0]["bounds"]), png.opaque_box().unwrap().map(i64::from));
    let block = object(&sources, "block", ObjectImageOptions::default());
    assert_eq!(ints(&block.r["items"][0]["bounds"]), [0, 0, block.r["width"].as_i64().unwrap(), block.r["height"].as_i64().unwrap()]);
}

#[test]
fn object_render_layers_only_those_layers_as_the_lab_stage_shows_them() {
    let sources = with(
        &[(
            "character",
            json!({
                "kind": "sprite",
                "anchor": [1, 3],
                "keys": { "a": "ink", "b": "teal-1" },
                "layers": [{ "map": ["aaa", "a.a", "a.a", "..."] }, { "map": ["...", ".b.", "...", "bbb"] }]
            }),
        )],
        &[],
    );
    let png = |layers: Option<Vec<usize>>| object(&sources, "character", ObjectImageOptions { layers, ..ObjectImageOptions::default() });
    let all = png(None);
    let both = png(Some(vec![0, 1]));
    let bottom = png(Some(vec![0]));
    let top = png(Some(vec![1]));
    let none = png(Some(vec![]));
    assert_eq!(none.opaque_box(), None);
    assert_eq!(both.image.width, all.image.width);
    let mut differs = false;
    for y in 0..all.image.height {
        for x in 0..all.image.width {
            assert_eq!(both.alpha(x, y) > 0, all.alpha(x, y) > 0, "pixel {x},{y}");
            assert_eq!(all.alpha(x, y) > 0, bottom.alpha(x, y) > 0 || top.alpha(x, y) > 0, "pixel {x},{y} is in a layer");
            if bottom.alpha(x, y) != all.alpha(x, y) {
                differs = true;
            }
        }
    }
    assert!(differs, "the top layer adds pixels");
}

#[test]
#[ignore = "needs U21"]
fn the_cli_writes_the_png_with_out() {
    // pixellab scene render zz-pine --scale 3 --out <file> prints
    // "zz-pine: 240×240 PNG at 3×, 1 items shown, time 0 ms\nwrote … (240×240)\n";
    // object render pine --json --out <file> has png equal to the file's base64.
    unreachable!("needs the CLI (U21), and scene.render (U22) and object.render (U24)");
}

#[test]
#[ignore = "needs U22"]
fn render_params_are_checked_scale_range_time_show() {
    // scene render zz-pine --scale 0|17|1.5, --time -1, --show x: exit 2, "pixellab: scene.render: …";
    // scene list --out x.png: exit 2, "--out goes with a render command".
    unreachable!("needs scene.render (U22) and the CLI (U21)");
}

// ------------------------------------------------------------------ helpers

#[test]
fn render_helpers_paths_base64_and_the_dark_page_color() {
    assert_eq!(path_string(&[5, 2]), "5.2");
    assert_eq!(parse_path("5.2"), Some(vec![5, 2]));
    assert_eq!(parse_path("0"), Some(vec![0]));
    for bad in ["", "x", "5.", ".5", "5..2", "-1", "1.5x", " 1"] {
        assert_eq!(parse_path(bad), None, "{bad:?}");
    }
    assert_eq!(MAX_SCALE, 16);
    for (text, want) in
        [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")]
    {
        assert_eq!(base64(text.as_bytes()), want);
    }
    let dir = std::env::temp_dir().join(format!("pixellab-render-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("src/styles")).unwrap();
    assert_eq!(dark_page_color(&dir).as_deref(), Some("#1C1712"), "no file");
    let css = |text: &str| std::fs::write(dir.join("src/styles/variables.css"), text).unwrap();
    css(":root { --color-page: #FFFFFF; }\n[data-theme=\"dark\"] {\n  --color-page:   #1a2b3c;\n}\n");
    assert_eq!(dark_page_color(&dir).as_deref(), Some("#1A2B3C"));
    css(":root { --color-page: #FFFFFF; }\n");
    assert_eq!(dark_page_color(&dir).as_deref(), Some("#1C1712"), "no dark block");
    css("[data-theme=\"dark\"] { --color-page: #1a2b3cd; --color-page: #223344; }");
    assert_eq!(dark_page_color(&dir).as_deref(), Some("#223344"), "a longer run is skipped, as \\b does");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn flatten_items_lists_items_groups_and_layers() {
    let sources = with_extras();
    let flat = flatten_items(&sources.scenes["zz-layers"]);
    assert_eq!(
        flat["layers"],
        json!([{ "id": "rail", "name": "Rail", "items": 2 }, { "id": "train", "name": "Train", "parent": "rail", "items": 1 }])
    );
    assert_eq!(flat["groups"], json!([{ "path": "3", "attrs": { "data-enter": "shadow" }, "items": 1, "depth": "back" }]));
    assert_eq!(flat["items"][3]["group"], "3");
    assert_eq!(flat["items"][2]["level"], 0);
}

#[test]
fn visible_items_shows_one_variant_per_run() {
    let items: Vec<Value> = vec![
        json!({ "group": { "data-class": "a" }, "items": [{ "object": "x" }] }),
        json!({ "group": { "data-class": "b" }, "items": [{ "object": "y" }] }),
        json!({ "object": "z" }),
    ];
    let (kept, leaves) = visible_items(&items, None, &[]);
    assert_eq!(kept.len(), 2);
    let paths: Vec<String> = leaves.iter().map(|l| path_string(&l.path)).collect();
    assert_eq!(paths, ["0.0", "2"]);
    let (_, leaves) = visible_items(&items, Some(&[1, 0]), &[]);
    let paths: Vec<String> = leaves.iter().map(|l| path_string(&l.path)).collect();
    assert_eq!(paths, ["1.0", "2"]);
}

// ------------------------------------------------------------------- parity

/// Every result equals what Node gave: the JSON without the PNGs, and the
/// decoded pixels of the PNGs.
#[test]
fn parity_renders_match_nodes_pixel_for_pixel() {
    let sources = with_extras();
    let expected = data("expected.json");
    let cases = data("cases.json");
    let strip = |v: &Value| {
        let mut v = v.clone();
        if let Some(o) = v.as_object_mut() {
            o.shift_remove("png");
            o.shift_remove("focusPng");
        }
        v
    };
    let mut count = 0;
    for c in cases.as_array().unwrap() {
        let key = c["key"].as_str().unwrap();
        let name = c["name"].as_str().unwrap();
        let o = &c["options"];
        let get = |k: &str| o.get(k).and_then(Value::as_f64);
        let set = |k: &str| {
            o.get(k).map(|a| {
                a.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).fold(OSet::new(), |mut s, v| {
                    s.add(v);
                    s
                })
            })
        };
        let got = if c["kind"] == "scene" {
            let options = SceneImageOptions {
                scale: get("scale").unwrap() as u32,
                time: get("time").unwrap_or(0.0),
                show: o.get("show").map(|a| a.as_array().unwrap().iter().map(|n| n.as_u64().unwrap() as usize).collect()),
                theme: if o.get("theme").and_then(Value::as_str) == Some("dark") { Theme::Dark } else { Theme::Light },
                hide: set("hide"),
                focus: set("focus"),
            };
            render_scene_image(&sources, name, "#1C1712", &options).unwrap()
        } else {
            let options = ObjectImageOptions {
                scale: get("scale").unwrap() as u32,
                time: get("time"),
                frame: get("frame").unwrap_or(0.0) as usize,
                layers: o.get("layers").map(|a| a.as_array().unwrap().iter().map(|n| n.as_u64().unwrap() as usize).collect()),
            };
            render_object_image(&sources, name, &options).unwrap()
        };
        let want = &expected[key];
        assert_eq!(strip(&got), strip(want), "{key}");
        assert_eq!(serde_json::to_string(&strip(&got)).unwrap(), serde_json::to_string(&strip(want)).unwrap(), "{key}: key order");
        for png in ["png", "focusPng"] {
            if want.get(png).is_none() {
                assert!(got.get(png).is_none(), "{key}: {png}");
                continue;
            }
            let (a, b) = (decode(&unbase64(got[png].as_str().unwrap())).unwrap(), decode(&unbase64(want[png].as_str().unwrap())).unwrap());
            assert_eq!((a.width, a.height), (b.width, b.height), "{key} {png}");
            assert!(a.rgba == b.rgba, "{key} {png}: pixels differ");
        }
        count += 1;
    }
    assert!(count >= 20);
    let mut flattened = 0;
    for (key, want) in expected.as_object().unwrap() {
        if let Some(name) = key.strip_prefix("flatten:") {
            let got = flatten_items(&sources.scenes[name]);
            assert_eq!(serde_json::to_string(&got).unwrap(), serde_json::to_string(want).unwrap(), "{key}");
            flattened += 1;
        }
    }
    assert!(flattened >= 6);
}

/// For every scene of the fixture, scale 1 and 2 at time 0.
#[test]
fn parity_every_scene_of_the_fixture() {
    let sources = with(&[], &[]);
    let expected = data("expected.json");
    for name in sources.scenes.keys() {
        for scale in [1, 2] {
            let key = format!("scene:{name}:{scale}");
            let got = render_scene_image(&sources, name, "#1C1712", &SceneImageOptions { scale, ..SceneImageOptions::default() }).unwrap();
            let want = &expected[&key];
            let (a, b) =
                (decode(&unbase64(got["png"].as_str().unwrap())).unwrap(), decode(&unbase64(want["png"].as_str().unwrap())).unwrap());
            assert!(a == b, "{key}");
        }
    }
}
