//! Edits to object sources (ports cli/lib/engine/edit.mjs; pixel-art engine
//! spec D9.5, R17). Each function changes a source document in place and
//! keeps it in the canonical shape. Painting an object that extends another
//! writes the painted row as an override, and drops the override again when
//! the row matches the base; painting a block writes its `surface` and
//! `sides`.

use std::collections::HashSet;

use serde_json::{Map, Value};

use crate::engine::{is_key, layer_maps, resolve, side_cells, truthy, Resolved, Sources, SpriteLayer, SIDE_H, SIDE_W};
use crate::iso::{block_faces, footprint, tile_pixels, Pixel, TILE_H, TILE_W};
use crate::motion::SITE_LOOPS;
use crate::omap::OMap;
use crate::serialize::{entries, keys as js_keys, ordered};
use crate::{EngineError, Result};

/// Keys tried for a new color after its own initial: lowercase, uppercase, digits, then punctuation.
const KEY_ORDER: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!#$%&*+-=?@^_~:;<>/|()[]{}\"`,\\'";

/// A layer and frame of an object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Where {
    pub layer: usize,
    pub frame: usize,
}

/// The map a layer shows at a frame (`mapAt`): a frame loop shows frame
/// `frame`, or its last frame if it has fewer.
pub fn map_at(layer: &SpriteLayer, frame: usize) -> &Vec<String> {
    let maps = layer_maps(layer);
    maps[frame.min(maps.len() - 1)]
}

fn string_record(v: Option<&Value>) -> OMap<String, String> {
    match v {
        Some(Value::Object(m)) => entries(m).into_iter().map(|(k, v)| (k.clone(), crate::engine::js_string(Some(v)))).collect(),
        _ => OMap::new(),
    }
}

fn rows_value(rows: impl IntoIterator<Item = String>) -> Value {
    Value::Array(rows.into_iter().map(Value::String).collect())
}

fn rows_of(v: &Value) -> Vec<String> {
    v.as_array().map(|l| l.iter().map(|r| r.as_str().unwrap_or_default().to_string()).collect()).unwrap_or_default()
}

/// The key for a color: the one already mapped to it, or a new one added to
/// the document's own `keys` (`keyFor`).
pub fn key_for(doc: &mut Value, keys: &OMap<String, String>, color: &str) -> String {
    if let Some((k, _)) = keys.iter().find(|(_, v)| v.as_str() == color) {
        return k.clone();
    }
    let initial = color.chars().next().unwrap_or('a').to_string();
    let candidates = [initial.clone(), initial.to_uppercase()].into_iter().chain(KEY_ORDER.chars().map(String::from));
    let key = candidates.into_iter().find(|k| is_key(k) && !keys.has(k.as_str())).expect("a free key");
    let obj = doc.as_object_mut().expect("a source document is an object");
    let mut own = match obj.get("keys") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };
    own.insert(key.clone(), Value::String(color.to_string()));
    obj.insert("keys".to_string(), Value::Object(ordered(&own)));
    key
}

/// Sets pixels on one layer and frame to a color, or clears them (None).
/// Returns how many changed (`paint`). `sources` hold `doc` as `name`.
pub fn paint(sources: &Sources, name: &str, doc: &mut Value, at: Where, pixels: &[[i64; 2]], color: Option<&str>) -> Result<usize> {
    let Where { layer, frame } = at;
    let resolved = resolve(sources, name)?;
    let Some(source) = resolved.layers.get(layer) else { return Ok(0) };
    let map = map_at(source, frame);
    let ch = match color {
        None => '.',
        Some(c) => key_for(doc, &resolved.keys, c).chars().next().expect("a key"),
    };
    let mut rows: OMap<usize, Vec<char>> = OMap::new();
    let mut changed = 0;
    for &[x, y] in pixels {
        if y < 0 || y as usize >= map.len() || x < 0 {
            continue;
        }
        let (xu, yu) = (x as usize, y as usize);
        if xu >= map[yu].chars().count() {
            continue;
        }
        let row = rows.get_or_insert_with(yu, || map[yu].chars().collect());
        if row[xu] != ch {
            changed += 1;
        }
        row[xu] = ch;
    }
    if changed == 0 {
        return Ok(0);
    }
    let extends = doc.get("extends").filter(|e| truthy(Some(e))).map(|e| crate::engine::js_string(Some(e)));
    if let Some(extends) = extends {
        if matches!(source, SpriteLayer::Frames { .. }) {
            return Err(EngineError(format!("layer {layer} is a frame loop; an object that extends another can only override map layers")));
        }
        let base_resolved = resolve(sources, &extends)?;
        let base = base_resolved.layers.get(layer).ok_or_else(|| EngineError(format!("object '{extends}' has no layer {layer}")))?;
        let base_map = layer_maps(base)[0];
        let mut overrides = match doc.get("rows") {
            Some(Value::Object(m)) => m.clone(),
            _ => Map::new(),
        };
        let lk = layer.to_string();
        let mut mine = match overrides.get(&lk) {
            Some(Value::Object(m)) => m.clone(),
            _ => Map::new(),
        };
        for (y, row) in rows.iter() {
            let text: String = row.iter().collect();
            if base_map.get(*y) == Some(&text) {
                mine.shift_remove(&y.to_string());
            } else {
                mine.insert(y.to_string(), Value::String(text));
            }
        }
        // Rows in order, so the canonical source lists them top to bottom.
        let sorted = sort_numeric(&mine);
        if !sorted.is_empty() {
            overrides.insert(lk, Value::Object(sorted));
        } else {
            overrides.shift_remove(&lk);
        }
        doc.as_object_mut().expect("a source document is an object").insert("rows".to_string(), Value::Object(sort_numeric(&overrides)));
    } else {
        let layer_value = &mut doc["layers"][layer];
        let target = if layer_value.get("frames").is_some() {
            let frames = layer_value["frames"].as_array_mut().expect("frames");
            let last = frames.len() - 1;
            &mut frames[frame.min(last)]
        } else {
            &mut layer_value["map"]
        };
        for (y, row) in rows.iter() {
            target[*y] = Value::String(row.iter().collect());
        }
    }
    Ok(changed)
}

/// An object's entries ordered by their numeric keys.
fn sort_numeric(m: &Map<String, Value>) -> Map<String, Value> {
    let mut list: Vec<(f64, &String)> = js_keys(m).into_iter().map(|k| (crate::jsnum::parse(k), k)).collect();
    list.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    ordered(&list.into_iter().map(|(_, k)| (k.clone(), m[k].clone())).collect())
}

/// Which part of a block a pixel lands in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Surface,
    Left,
    Right,
}

/// A cell of a block's source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockCell {
    pub part: Part,
    pub col: i32,
    pub row: i32,
}

/// Where each pixel of a block lands in its source, relative to its first
/// tile's top-face center (`blockCells`).
pub fn block_cells(size: [i64; 3]) -> OMap<Pixel, BlockCell> {
    let mut cells = OMap::new();
    for [tx, ty] in footprint([size[0], size[1]], [0, 0]) {
        for p in tile_pixels(tx, ty) {
            cells.set(p, BlockCell { part: Part::Surface, col: p.0 - tx + TILE_W / 2, row: p.1 - ty + TILE_H / 2 });
        }
    }
    let faces = block_faces(size, [0, 0]);
    for (part, face) in [(Part::Left, &faces.left), (Part::Right, &faces.right)] {
        for c in side_cells(face) {
            cells.set(c.p, BlockCell { part, col: c.col, row: c.row });
        }
    }
    cells
}

fn blank_map(width: i32, height: i32) -> Value {
    rows_value((0..height).map(|_| ".".repeat(width as usize)))
}

/// Paints a block's surface and sides (`paintBlock`); returns how many changed.
pub fn paint_block(doc: &mut Value, frame: usize, pixels: &[[i64; 2]], color: Option<&str>) -> Result<usize> {
    let size = doc
        .get("size")
        .and_then(Value::as_array)
        .map(|a| [0, 1, 2].map(|i| a.get(i).and_then(Value::as_i64).unwrap_or(0)))
        .unwrap_or([0; 3]);
    let cells = block_cells(size);
    let mut changed = 0;
    for &[x, y] in pixels {
        let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y)) else { continue };
        let Some(&cell) = cells.get(&(x, y)) else { continue };
        let surface = cell.part == Part::Surface;
        let field = if surface { "surface" } else { "sides" };
        if color.is_none() && !truthy(doc.get(field)) {
            continue;
        }
        // Created in canonical order: keys first.
        if doc.get(field).is_none_or(Value::is_null) {
            let made = if surface {
                serde_json::json!({ "keys": {}, "map": blank_map(TILE_W, TILE_H) })
            } else {
                serde_json::json!({ "keys": {}, "left": blank_map(SIDE_W, SIDE_H), "right": blank_map(SIDE_W, SIDE_H) })
            };
            doc.as_object_mut().expect("a block is an object").insert(field.to_string(), made);
        }
        let owner = &mut doc[field];
        let ch = match color {
            None => '.',
            Some(c) => {
                let keys = string_record(owner.get("keys"));
                key_for(owner, &keys, c).chars().next().expect("a key")
            }
        };
        let map: &mut Value = if surface {
            if owner.get("frames").is_some() {
                let frames = owner["frames"].as_array_mut().expect("frames");
                let last = frames.len() - 1;
                &mut frames[frame.min(last)]
            } else {
                &mut owner["map"]
            }
        } else {
            &mut owner[if cell.part == Part::Left { "left" } else { "right" }]
        };
        let (col, row) = (cell.col as usize, cell.row as usize);
        let Some(text) = map.get(row).and_then(Value::as_str) else { continue };
        let mut chars: Vec<char> = text.chars().collect();
        if chars.get(col) == Some(&ch) {
            continue;
        }
        if col < chars.len() {
            chars[col] = ch;
        }
        map[row] = Value::String(chars.into_iter().collect());
        changed += 1;
    }
    Ok(changed)
}

/// The pixels a fill from (x, y) covers on a block (`blockFill`): the
/// 4-connected pixels of the start pixel's color on the same part.
pub fn block_fill(cells: &OMap<Pixel, BlockCell>, colors: &OMap<Pixel, String>, x: i32, y: i32) -> Vec<[i64; 2]> {
    let Some(start) = cells.get(&(x, y)) else { return vec![] };
    let color = colors.get(&(x, y));
    let mut seen: HashSet<Pixel> = HashSet::from([(x, y)]);
    let mut out = Vec::new();
    let mut stack = vec![(x, y)];
    while let Some((px, py)) = stack.pop() {
        out.push([px as i64, py as i64]);
        for p in [(px + 1, py), (px - 1, py), (px, py + 1), (px, py - 1)] {
            if seen.contains(&p) || cells.get(&p).map(|c| c.part) != Some(start.part) || colors.get(&p) != color {
                continue;
            }
            seen.insert(p);
            stack.push(p);
        }
    }
    out
}

/// The pixels a fill from (x, y) covers on a map (`floodFill`).
pub fn flood_fill(map: &[String], x: i64, y: i64) -> Vec<[i64; 2]> {
    let grid: Vec<Vec<char>> = map.iter().map(|r| r.chars().collect()).collect();
    let inside = |x: i64, y: i64| y >= 0 && (y as usize) < grid.len() && x >= 0 && (x as usize) < grid[y as usize].len();
    if !inside(x, y) {
        return vec![];
    }
    let target = grid[y as usize][x as usize];
    let mut seen: HashSet<(i64, i64)> = HashSet::from([(x, y)]);
    let mut out = Vec::new();
    let mut stack = vec![(x, y)];
    while let Some((px, py)) = stack.pop() {
        out.push([px, py]);
        for (nx, ny) in [(px + 1, py), (px - 1, py), (px, py + 1), (px, py - 1)] {
            if !inside(nx, ny) || grid[ny as usize][nx as usize] != target || seen.contains(&(nx, ny)) {
                continue;
            }
            seen.insert((nx, ny));
            stack.push((nx, ny));
        }
    }
    out
}

/// The color name at (x, y), or None (`colorAt`).
pub fn color_at(resolved: &Resolved, at: Where, x: i64, y: i64) -> Option<String> {
    let source = resolved.layers.get(at.layer)?;
    let row = map_at(source, at.frame).get(usize::try_from(y).ok()?)?;
    let ch = row.chars().nth(usize::try_from(x).ok()?)?;
    if ch == '.' {
        return None;
    }
    resolved.keys.get(ch.to_string().as_str()).cloned()
}

/// The rows an object overrides, per layer, top to bottom (`overriddenRows`).
pub fn overridden_rows(doc: &Value) -> Vec<(usize, Vec<usize>)> {
    let Some(Value::Object(rows)) = doc.get("rows") else { return vec![] };
    let mut out: Vec<(usize, Vec<usize>)> = entries(rows)
        .into_iter()
        .map(|(layer, rows)| {
            let mut ys: Vec<usize> =
                rows.as_object().map(|r| js_keys(r).into_iter().map(|k| k.parse().unwrap_or(0)).collect()).unwrap_or_default();
            ys.sort();
            (layer.parse().unwrap_or(0), ys)
        })
        .collect();
    out.sort_by_key(|(l, _)| *l);
    out
}

/// Pads or crops every map of a sprite to `width` × `height` (`resize`).
pub fn resize(doc: &mut Value, width: usize, height: usize) {
    let fit = |map: &Value| {
        let rows = rows_of(map);
        rows_value((0..height).map(|y| {
            let mut row: String = rows.get(y).map(String::as_str).unwrap_or("").chars().take(width).collect();
            while row.chars().count() < width {
                row.push('.');
            }
            row
        }))
    };
    for layer in doc["layers"].as_array_mut().into_iter().flatten() {
        if let Some(frames) = layer.get("frames").and_then(Value::as_array) {
            let fitted = Value::Array(frames.iter().map(fit).collect());
            layer["frames"] = fitted;
        } else {
            let fitted = fit(&layer["map"]);
            layer["map"] = fitted;
        }
    }
}

/// A blank map the size of the object's others.
fn blank(doc: &Value) -> Value {
    let first = &doc["layers"][0];
    let map = if first.get("frames").is_some() { &first["frames"][0] } else { &first["map"] };
    let rows = rows_of(map);
    let width = rows.first().map_or(0, |r| r.chars().count());
    rows_value(rows.iter().map(|_| ".".repeat(width)))
}

fn layers_mut(doc: &mut Value) -> &mut Vec<Value> {
    doc["layers"].as_array_mut().expect("layers")
}

fn frames_mut(doc: &mut Value, layer: usize) -> &mut Vec<Value> {
    doc["layers"][layer]["frames"].as_array_mut().expect("frames")
}

/// Adds a blank map layer; returns its index (`addLayer`).
pub fn add_layer(doc: &mut Value) -> usize {
    let map = blank(doc);
    let layers = layers_mut(doc);
    layers.push(serde_json::json!({ "map": map }));
    layers.len() - 1
}

/// Duplicates a layer after itself; returns the copy's index (`duplicateLayer`).
pub fn duplicate_layer(doc: &mut Value, index: usize) -> usize {
    let layers = layers_mut(doc);
    let copy = layers[index].clone();
    layers.insert(index + 1, copy);
    index + 1
}

/// Deletes a layer unless it's the last; returns the layer to select (`deleteLayer`).
pub fn delete_layer(doc: &mut Value, index: usize) -> usize {
    let layers = layers_mut(doc);
    if layers.len() <= 1 {
        return index;
    }
    layers.remove(index);
    index.min(layers.len() - 1)
}

/// Moves an element of a list by `delta`; returns its new index (`move`).
pub fn move_in(list: &mut [Value], index: usize, delta: i64) -> usize {
    let target = index as i64 + delta;
    if target < 0 || target >= list.len() as i64 {
        return index;
    }
    list.swap(index, target as usize);
    target as usize
}

/// Adds a blank frame after `index`; returns its index (`addFrame`).
pub fn add_frame(doc: &mut Value, layer: usize, index: usize) -> usize {
    let frames = frames_mut(doc, layer);
    let blank_frame = rows_value(rows_of(&frames[index]).iter().map(|r| ".".repeat(r.chars().count())));
    frames.insert(index + 1, blank_frame);
    index + 1
}

/// Duplicates a frame after itself (`duplicateFrame`).
pub fn duplicate_frame(doc: &mut Value, layer: usize, index: usize) -> usize {
    let frames = frames_mut(doc, layer);
    let copy = frames[index].clone();
    frames.insert(index + 1, copy);
    index + 1
}

/// Deletes a frame unless it's the last (`deleteFrame`).
pub fn delete_frame(doc: &mut Value, layer: usize, index: usize) -> usize {
    let frames = frames_mut(doc, layer);
    if frames.len() <= 1 {
        return index;
    }
    frames.remove(index);
    index.min(frames.len() - 1)
}

fn site_loop(loop_name: &str) -> &'static crate::motion::SiteLoop {
    SITE_LOOPS.iter().find(|l| l.name == loop_name).expect("one of the site's loops")
}

/// Turns a map layer into one of the site's frame loops (`animateLayer`).
pub fn animate_layer(doc: &mut Value, layer: usize, loop_name: &str) {
    let l = site_loop(loop_name);
    let map = doc["layers"][layer]["map"].clone();
    doc["layers"][layer] = serde_json::json!({ "loop": loop_name, "prefix": l.prefix, "frames": vec![map; l.frames] });
}

/// Switches a frame loop to another of the site's loops (`setLoop`).
pub fn set_loop(doc: &mut Value, layer: usize, loop_name: &str) {
    let l = site_loop(loop_name);
    let mut list = frames_mut(doc, layer).clone();
    while list.len() < l.frames {
        let last = list.last().expect("a frame").clone();
        list.push(last);
    }
    doc["layers"][layer] = serde_json::json!({ "loop": loop_name, "prefix": l.prefix, "frames": list });
}

/// Turns a frame loop back into a map layer, keeping one frame (`stillLayer`).
pub fn still_layer(doc: &mut Value, layer: usize, frame: usize) {
    let map = doc["layers"][layer]["frames"][frame].clone();
    doc["layers"][layer] = serde_json::json!({ "map": map });
}
