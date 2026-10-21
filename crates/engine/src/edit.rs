//! Edits to object sources (ports cli/lib/engine/edit.mjs; pixel-art engine
//! spec D9.5, R17). Each function changes a source document in place and
//! keeps it in the canonical shape. Painting an object that extends another
//! writes the painted row as an override, and drops the override again when
//! the row matches the base; painting a block writes its `surface` and
//! `sides`.

use serde_json::Value;

use crate::engine::{Resolved, Sources, SpriteLayer};
use crate::iso::Pixel;
use crate::omap::OMap;
use crate::Result;

/// A layer and frame of an object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Where {
    pub layer: usize,
    pub frame: usize,
}

/// The map a layer shows at a frame (`mapAt`).
pub fn map_at(layer: &SpriteLayer, frame: usize) -> &Vec<String> {
    let _ = (layer, frame);
    todo!("U6: port edit.mjs mapAt")
}

/// The key for a color: the one already mapped to it, or a new one added to
/// the document's own `keys` (`keyFor`).
pub fn key_for(doc: &mut Value, keys: &OMap<String, String>, color: &str) -> String {
    let _ = (doc, keys, color);
    todo!("U6: port edit.mjs keyFor")
}

/// Sets pixels on one layer and frame to a color, or clears them (None).
/// Returns how many changed (`paint`). `sources` hold `doc` as `name`.
pub fn paint(sources: &Sources, name: &str, doc: &mut Value, at: Where, pixels: &[[i64; 2]], color: Option<&str>) -> Result<usize> {
    let _ = (sources, name, doc, at, pixels, color);
    todo!("U6: port edit.mjs paint")
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

/// Where each pixel of a block lands in its source (`blockCells`).
pub fn block_cells(size: [i64; 3]) -> OMap<Pixel, BlockCell> {
    let _ = size;
    todo!("U6: port edit.mjs blockCells")
}

/// Paints a block's surface and sides (`paintBlock`); returns how many changed.
pub fn paint_block(doc: &mut Value, frame: usize, pixels: &[[i64; 2]], color: Option<&str>) -> Result<usize> {
    let _ = (doc, frame, pixels, color);
    todo!("U6: port edit.mjs paintBlock")
}

/// The pixels a fill from (x, y) covers on a block (`blockFill`).
pub fn block_fill(cells: &OMap<Pixel, BlockCell>, colors: &OMap<Pixel, String>, x: i32, y: i32) -> Vec<[i64; 2]> {
    let _ = (cells, colors, x, y);
    todo!("U6: port edit.mjs blockFill")
}

/// The pixels a fill from (x, y) covers on a map (`floodFill`).
pub fn flood_fill(map: &[String], x: i64, y: i64) -> Vec<[i64; 2]> {
    let _ = (map, x, y);
    todo!("U6: port edit.mjs floodFill")
}

/// The color name at (x, y), or None (`colorAt`).
pub fn color_at(resolved: &Resolved, at: Where, x: i64, y: i64) -> Option<String> {
    let _ = (resolved, at, x, y);
    todo!("U6: port edit.mjs colorAt")
}

/// The rows an object overrides, per layer, top to bottom (`overriddenRows`).
pub fn overridden_rows(doc: &Value) -> Vec<(usize, Vec<usize>)> {
    let _ = doc;
    todo!("U6: port edit.mjs overriddenRows")
}

/// Pads or crops every map of a sprite to `width` × `height` (`resize`).
pub fn resize(doc: &mut Value, width: usize, height: usize) {
    let _ = (doc, width, height);
    todo!("U6: port edit.mjs resize")
}

/// Adds a blank map layer; returns its index (`addLayer`).
pub fn add_layer(doc: &mut Value) -> usize {
    let _ = doc;
    todo!("U6: port edit.mjs addLayer")
}

/// Duplicates a layer after itself; returns the copy's index (`duplicateLayer`).
pub fn duplicate_layer(doc: &mut Value, index: usize) -> usize {
    let _ = (doc, index);
    todo!("U6: port edit.mjs duplicateLayer")
}

/// Deletes a layer unless it's the last; returns the layer to select (`deleteLayer`).
pub fn delete_layer(doc: &mut Value, index: usize) -> usize {
    let _ = (doc, index);
    todo!("U6: port edit.mjs deleteLayer")
}

/// Moves an element of a list by `delta`; returns its new index (`move`).
pub fn move_in(list: &mut [Value], index: usize, delta: i64) -> usize {
    let _ = (list, index, delta);
    todo!("U6: port edit.mjs move")
}

/// Adds a blank frame after `index`; returns its index (`addFrame`).
pub fn add_frame(doc: &mut Value, layer: usize, index: usize) -> usize {
    let _ = (doc, layer, index);
    todo!("U6: port edit.mjs addFrame")
}

/// Duplicates a frame after itself (`duplicateFrame`).
pub fn duplicate_frame(doc: &mut Value, layer: usize, index: usize) -> usize {
    let _ = (doc, layer, index);
    todo!("U6: port edit.mjs duplicateFrame")
}

/// Deletes a frame unless it's the last (`deleteFrame`).
pub fn delete_frame(doc: &mut Value, layer: usize, index: usize) -> usize {
    let _ = (doc, layer, index);
    todo!("U6: port edit.mjs deleteFrame")
}

/// Turns a map layer into one of the site's frame loops (`animateLayer`).
pub fn animate_layer(doc: &mut Value, layer: usize, loop_name: &str) {
    let _ = (doc, layer, loop_name);
    todo!("U6: port edit.mjs animateLayer")
}

/// Switches a frame loop to another of the site's loops (`setLoop`).
pub fn set_loop(doc: &mut Value, layer: usize, loop_name: &str) {
    let _ = (doc, layer, loop_name);
    todo!("U6: port edit.mjs setLoop")
}

/// Turns a frame loop back into a map layer, keeping one frame (`stillLayer`).
pub fn still_layer(doc: &mut Value, layer: usize, frame: usize) {
    let _ = (doc, layer, frame);
    todo!("U6: port edit.mjs stillLayer")
}
