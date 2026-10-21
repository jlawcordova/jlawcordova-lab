//! PNG previews of pixel art (ports cli/lib/engine/preview.mjs; the site's
//! pixel-art engine spec R29): an object on one tile outline, a scene at its
//! viewBox with only its first data-class variant, both on the document's
//! card color, at frame 0 of every loop; and the palette's swatch sheet.

use crate::engine::{bounds, composite, js_space, render_object, render_scene, CompositeOptions, Order, Sources};
use crate::format::quote;
use crate::iso::{Pixel, TILE_H};
use crate::jsnum;
use crate::omap::OMap;
use crate::png::Image;
use crate::Result;

const MARGIN: i32 = 2;

/// The preview colors a document gets when its manifest names none.
pub const DEFAULT_CARD: &str = "#E9DCC6";
pub const DEFAULT_GRID: &str = "rgba(90, 62, 43, 0.06)";

/// The card color and the grid line color over it, as opaque RGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewColors {
    pub card: [u8; 3],
    pub grid: [u8; 3],
}

/// The card and grid colors from a manifest's `preview` (`previewColors`),
/// or the error message previewColors threw.
pub fn preview_colors(card: Option<&str>, grid: Option<&str>) -> std::result::Result<PreviewColors, String> {
    let card_text = card.unwrap_or(DEFAULT_CARD);
    let grid_text = grid.unwrap_or(DEFAULT_GRID);
    let card_rgb =
        parse_hex(card_text.trim_matches(js_space)).ok_or_else(|| format!("preview.card {} isn't a #RRGGBB color", quote(card_text)))?;
    let grid_trim = grid_text.trim_matches(js_space);
    if let Some((r, g, b, alpha)) = parse_rgba(grid_trim) {
        let mut out = [0u8; 3];
        for (i, v) in [r, g, b].into_iter().enumerate() {
            out[i] = wrap(jsnum::round(v * alpha + card_rgb[i] as f64 * (1.0 - alpha)));
        }
        return Ok(PreviewColors { card: card_rgb, grid: out });
    }
    if let Some(rgb) = parse_hex(grid_trim) {
        return Ok(PreviewColors { card: card_rgb, grid: rgb });
    }
    Err(format!("preview.grid {} isn't a #RRGGBB or rgba(r, g, b, a) color", quote(grid_text)))
}

/// What a typed-array store does with a number: truncate, wrap to a byte.
fn wrap(v: f64) -> u8 {
    if !v.is_finite() {
        return 0;
    }
    (v.trunc().rem_euclid(256.0)) as u8
}

fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let at = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap();
    Some([at(0), at(2), at(4)])
}

/// `^rgba\((\d+),\s*(\d+),\s*(\d+),\s*([\d.]+)\)$`, as numbers (an
/// unparsable alpha, as `Number` would, is NaN).
fn parse_rgba(s: &str) -> Option<(f64, f64, f64, f64)> {
    let inner = s.strip_prefix("rgba(")?.strip_suffix(')')?;
    let mut parts = inner.splitn(4, ',');
    let mut ints = [0f64; 3];
    for (i, v) in ints.iter_mut().enumerate() {
        let part = parts.next()?;
        // Only the commas after the first allow whitespace.
        let digits = if i == 0 { part } else { part.trim_start_matches(js_space) };
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *v = digits.parse().ok()?;
    }
    let alpha = parts.next()?.trim_start_matches(js_space);
    if alpha.is_empty() || !alpha.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }
    Some((ints[0], ints[1], ints[2], alpha.parse().unwrap_or(f64::NAN)))
}

/// What a preview shows: an object or a scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target<'a> {
    Object(&'a str),
    Scene(&'a str),
}

/// Renders an object (on a tile) or a scene (at its viewBox) to RGBA
/// (`renderPreview`), with the image's top-left in scene pixels.
pub fn render_preview(sources: &Sources, target: Target<'_>, colors: PreviewColors) -> Result<(Image, [i32; 2])> {
    let PreviewColors { card, grid } = colors;
    let mut paint: OMap<Pixel, [u8; 3]> = OMap::new();
    let rgb = |name: &str| -> [u8; 3] {
        let hex = sources.colors.get(name).map(|c| c.hex.as_str()).unwrap_or("");
        let at = |i: usize| hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()).unwrap_or(0);
        [at(1), at(3), at(5)]
    };
    let (min_x, min_y, max_x, max_y);
    match target {
        Target::Scene(name) => {
            let rendered = render_scene(sources, name, Order::Depth)?;
            for (p, color) in composite(&rendered.root, CompositeOptions { first_variant: true, ..Default::default() }).iter() {
                paint.set(*p, rgb(color));
            }
            let n = |i: usize| rendered.view_box.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0) as i32;
            let (x, y, w, h) = (n(0), n(1), n(2), n(3));
            (min_x, min_y, max_x, max_y) = (x, y, x + w - 1, y + h - 1);
        }
        Target::Object(name) => {
            for p in tile_outline() {
                paint.set(p, grid);
            }
            for (p, color) in composite(&render_object(sources, name)?, CompositeOptions::default()).iter() {
                paint.set(*p, rgb(color));
            }
            let b = bounds(paint.keys()).expect("an object paints at least the tile outline");
            (min_x, min_y, max_x, max_y) = (b.min_x - MARGIN, b.min_y - MARGIN, b.max_x + MARGIN, b.max_y + MARGIN);
        }
    }
    let width = (max_x - min_x + 1).max(0) as usize;
    let height = (max_y - min_y + 1).max(0) as usize;
    let mut rgba = vec![0u8; width * height * 4];
    for px in rgba.as_chunks_mut::<4>().0 {
        px[..3].copy_from_slice(&card);
        px[3] = 255;
    }
    for (&(x, y), color) in paint.iter() {
        if x < min_x || x > max_x || y < min_y || y > max_y {
            continue;
        }
        let at = ((y - min_y) as usize * width + (x - min_x) as usize) * 4;
        rgba[at..at + 3].copy_from_slice(color);
        rgba[at + 3] = 255;
    }
    Ok((Image { width: width as u32, height: height as u32, rgba }, [min_x, min_y]))
}

/// The outline of the tile whose top-face center is (0, 0): rows of width 4,
/// 8, … 32, 32, … 8, 4, so every edge steps 2 across for 1 down.
fn tile_outline() -> Vec<Pixel> {
    let rows = TILE_H;
    let half = |r: i32| if r < rows / 2 { 2 * (r + 1) } else { 2 * (rows - r) };
    let inside = |x: i32, r: i32| r >= 0 && r < rows && x >= -half(r) && x < half(r);
    let mut out = Vec::new();
    for r in 0..rows {
        for x in -half(r)..half(r) {
            if !inside(x, r - 1) || !inside(x, r + 1) || !inside(x - 1, r) || !inside(x + 1, r) {
                out.push((x, r - rows / 2));
            }
        }
    }
    out
}

/// The materials of the palette sheet's rows (`MATERIALS`).
pub const MATERIALS: [&str; 7] = ["grass", "soil", "wood", "path", "water", "roof", "gold"];

/// The world palette's swatch sheet (`renderPalette`).
pub fn render_palette(sources: &Sources, colors: PreviewColors) -> Image {
    const SWATCH: usize = 16;
    const GAP: usize = 2;
    let PreviewColors { card, grid } = colors;
    let line: Vec<u8> = grid.iter().map(|&v| wrap(jsnum::round(v as f64 * 0.75))).collect();
    let mut rows: Vec<Vec<String>> = MATERIALS.iter().map(|m| (1..=4).map(|n| format!("{m}-{n}")).collect()).collect();
    rows.push(["ink", "cream", "skin-1", "skin-2"].iter().map(|s| s.to_string()).collect());
    let width = GAP + 4 * (SWATCH + GAP);
    let height = GAP + rows.len() * (SWATCH + GAP);
    let mut rgba = vec![0u8; width * height * 4];
    for px in rgba.as_chunks_mut::<4>().0 {
        px[..3].copy_from_slice(&card);
        px[3] = 255;
    }
    for (r, names) in rows.iter().enumerate() {
        for (c, name) in names.iter().enumerate() {
            let Some(color) = sources.colors.get(name.as_str()) else { continue };
            if color.hex.is_empty() {
                continue;
            }
            let at = |i: usize| color.hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()).unwrap_or(0);
            let rgb = [at(1), at(3), at(5)];
            for y in 0..SWATCH {
                for x in 0..SWATCH {
                    // A 1-pixel line in the grid's color keeps a swatch as light as the card visible.
                    let edge = x == 0 || y == 0 || x == SWATCH - 1 || y == SWATCH - 1;
                    let at = ((GAP + r * (SWATCH + GAP) + y) * width + GAP + c * (SWATCH + GAP) + x) * 4;
                    rgba[at..at + 3].copy_from_slice(if edge { &line } else { &rgb });
                    rgba[at + 3] = 255;
                }
            }
        }
    }
    Image { width: width as u32, height: height as u32, rgba }
}

/// An RGBA PNG of the image, scaled up by whole-pixel repetition
/// (`encodePng`).
pub fn encode_png(image: &Image, scale: u32) -> Vec<u8> {
    crate::png::encode(image, scale)
}
