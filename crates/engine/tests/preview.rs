//! The PNG previews (ports the 'previews (R29)' block of cli/test/engine.test.mjs,
//! the previewColors error cases, and the palette sheet).

use std::collections::BTreeMap;

use engine::engine::{composite, load_sources, render_scene, CompositeOptions, Order, Sources};
use engine::png::{decode, Image};
use engine::preview::{encode_png, preview_colors, render_palette, render_preview, PreviewColors, Target};
use serde_json::{json, Value};

fn src(objects: Vec<(&str, Value)>, scenes: Vec<(&str, Value)>) -> Sources {
    let palette = json!({
        "world": { "ink": "#2E2418", "cream": "#F4EDE0", "grass-2": "#8FA56E", "grass-4": "#4E6B3A", "wood-3": "#7A4E2D" },
        "outfit": { "teal-1": "#7FA89B" }
    });
    let map = |v: Vec<(&str, Value)>| v.into_iter().map(|(k, v)| (k.to_string(), v)).collect::<BTreeMap<_, _>>();
    load_sources(palette, map(objects), map(scenes))
}

fn sprite(map: &[&str], keys: Value, extra: Value) -> Value {
    let mut v = json!({ "kind": "sprite", "keys": keys, "layers": [{ "map": map }] });
    for (k, e) in extra.as_object().unwrap() {
        v[k] = e.clone();
    }
    v
}

fn grass() -> Value {
    json!({ "g": "grass-2" })
}

fn scene(items: Value, view_box: [i32; 4]) -> Value {
    json!({ "viewBox": view_box, "items": items })
}

/// The decoded PNG's pixels, one `[r, g, b, a]` per pixel.
fn decoded(image: &Image, scale: u32) -> (u32, u32, Vec<[u8; 4]>) {
    let png = encode_png(image, scale);
    assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10], "PNG signature");
    let d = decode(&png).unwrap();
    (d.width, d.height, d.rgba.chunks(4).map(|p| [p[0], p[1], p[2], p[3]]).collect())
}

fn rgb(c: [u8; 3]) -> [u8; 4] {
    [c[0], c[1], c[2], 255]
}

#[test]
fn r29_the_png_writer_makes_a_valid_png_whose_pixels_are_the_image_at_1x_and_3x() {
    let image = Image { width: 2, height: 2, rgba: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 1, 2, 3, 255] };
    let (w, h, one) = decoded(&image, 1);
    assert_eq!((w, h), (2, 2));
    assert_eq!(one, vec![[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [1, 2, 3, 255]]);
    let (w, h, three) = decoded(&image, 3);
    assert_eq!((w, h), (6, 6));
    assert_eq!(three[0], [255, 0, 0, 255]);
    assert_eq!(three[2], [255, 0, 0, 255]);
    assert_eq!(three[3], [0, 255, 0, 255]);
    assert_eq!(three[3 * 6 + 5], [1, 2, 3, 255]);
}

#[test]
fn r29_a_preview_has_the_engines_pixels_on_the_card_color_standing_on_a_tile() {
    let sources = src(vec![("dot", sprite(&["g.", "gg"], grass(), json!({ "anchor": [0, 1] })))], vec![]);
    let card = [0xe9, 0xdc, 0xc6];
    let grid = [0xe0, 0xd3, 0xbd];
    let (image, origin) = render_preview(&sources, Target::Object("dot"), PreviewColors { card, grid }).unwrap();
    let (width, _, pixels) = decoded(&image, 1);
    let at = |x: i32, y: i32| pixels[((y - origin[1]) as u32 * width + (x - origin[0]) as u32) as usize];
    let g = [0x8f, 0xa5, 0x6e, 255];
    assert_eq!(at(0, -1), g);
    assert_eq!(at(0, 0), g);
    assert_eq!(at(1, 0), g);
    assert_eq!(at(1, -1), rgb(card), "transparent pixels show the card");
    assert_eq!(at(-16, 0), rgb(grid), "the tile outline's left corner");
    assert_eq!(at(15, 0), rgb(grid), "the tile outline's right corner");
    assert_eq!(at(0, -8), rgb(grid), "the tile outline's top corner");
    assert_eq!(at(0, 7), rgb(grid), "the tile outline's bottom corner");
}

#[test]
fn r29_a_scene_preview_shows_only_the_first_of_sibling_data_class_variants() {
    let items: Vec<Value> = (0..2)
        .map(|i| json!({ "group": { "data-class": i.to_string() }, "items": [{ "object": if i == 1 { "ink" } else { "dot" }, "at": { "px": [i, 0] } }] }))
        .collect();
    let sources = src(
        vec![("dot", sprite(&["g"], grass(), json!({}))), ("ink", sprite(&["i"], json!({ "i": "ink" }), json!({})))],
        vec![("s", scene(Value::Array(items), [0, 0, 2, 1]))],
    );
    let card = [1, 2, 3];
    let (image, _) = render_preview(&sources, Target::Scene("s"), PreviewColors { card, grid: [4, 5, 6] }).unwrap();
    assert_eq!(decoded(&image, 1).2, vec![[0x8f, 0xa5, 0x6e, 255], rgb(card)]);
    let rendered = render_scene(&sources, "s", Order::Depth).unwrap();
    assert_eq!(composite(&rendered.root, CompositeOptions::default()).len(), 2, "composite draws every variant by default");
}

#[test]
fn r29_a_scene_preview_is_its_viewbox_at_frame_0() {
    let fall =
        json!({ "kind": "sprite", "keys": { "w": "cream" }, "layers": [{ "loop": "wf", "prefix": "w", "frames": [["w."], [".w"]] }] });
    let sources = src(vec![("fall", fall)], vec![("s", scene(json!([{ "object": "fall", "at": { "px": [0, 0] } }]), [0, 0, 3, 1]))]);
    let card = [1, 2, 3];
    let (image, _) = render_preview(&sources, Target::Scene("s"), PreviewColors { card, grid: [4, 5, 6] }).unwrap();
    assert_eq!(decoded(&image, 1).2, vec![[0xf4, 0xed, 0xe0, 255], rgb(card), rgb(card)]);
}

#[test]
fn preview_colors_default_to_the_site_card_and_grid() {
    let c = preview_colors(None, None).unwrap();
    assert_eq!(c.card, [0xe9, 0xdc, 0xc6]);
    // 90, 62, 43 at alpha 0.06 over the card.
    assert_eq!(c.grid, [0xe0, 0xd3, 0xbd]);
}

#[test]
fn preview_colors_read_hex_and_rgba_and_throw_with_the_text_quoted() {
    let c = preview_colors(Some(" #102030 "), Some("rgba(0, 0, 0, 0.5)")).unwrap();
    assert_eq!(c.card, [0x10, 0x20, 0x30]);
    assert_eq!(c.grid, [8, 16, 24]);
    let c = preview_colors(Some("#102030"), Some("#AABBCC")).unwrap();
    assert_eq!(c.grid, [0xaa, 0xbb, 0xcc]);
    assert_eq!(preview_colors(Some("red"), None).unwrap_err(), r#"preview.card "red" isn't a #RRGGBB color"#);
    assert_eq!(preview_colors(Some("#12345"), None).unwrap_err(), r##"preview.card "#12345" isn't a #RRGGBB color"##);
    assert_eq!(
        preview_colors(None, Some("rgb(1, 2, 3)")).unwrap_err(),
        r#"preview.grid "rgb(1, 2, 3)" isn't a #RRGGBB or rgba(r, g, b, a) color"#
    );
    assert_eq!(
        preview_colors(None, Some("rgba( 1,2,3,0.5)")).unwrap_err(),
        r#"preview.grid "rgba( 1,2,3,0.5)" isn't a #RRGGBB or rgba(r, g, b, a) color"#
    );
}

#[test]
fn the_palette_sheet_has_a_swatch_per_name_and_leaves_missing_ones_as_the_card() {
    let sources = src(vec![], vec![]);
    let card = [1, 2, 3];
    let image = render_palette(&sources, PreviewColors { card, grid: [100, 100, 100] });
    assert_eq!((image.width, image.height), (74, 2 + 8 * 18));
    let (_, _, px) = decoded(&image, 1);
    let at = |x: usize, y: usize| px[y * 74 + x];
    assert_eq!(at(0, 0), rgb(card));
    // Row 0 is grass: grass-1 is missing, grass-2 is the second swatch.
    assert_eq!(at(2 + 8, 2 + 8), rgb(card));
    assert_eq!(at(2 + 18 + 8, 2 + 8), [0x8f, 0xa5, 0x6e, 255]);
    assert_eq!(at(2 + 18, 2), [75, 75, 75, 255], "the swatch's edge is the grid's color, darker");
}
