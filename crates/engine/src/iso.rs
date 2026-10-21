//! The isometric grid (ports cli/lib/engine/iso.mjs): 2:1 dimetric, 32×16
//! tiles, 16-pixel levels. Grid positions only; block faces come with the
//! block kind.
//!
//! Positions are doubles, as JavaScript held them; pixels (`Pixel`) are
//! whole numbers.

/// A pixel, (x, y): what the Node engine wrote as the key "x,y".
pub type Pixel = (i32, i32);

use crate::jsnum;
use crate::omap::{OMap, OSet};

pub const TILE_W: i32 = 32;
pub const TILE_H: i32 = 16;
pub const LEVEL_H: i32 = 16;

/// The top-face center of the tile at [col, row, level], in scene pixels.
/// `origin` is where tile [0, 0, 0] sits.
pub fn tile_to_px(tile: [f64; 3], origin: [f64; 2]) -> [f64; 2] {
    let [col, row, level] = tile;
    let [ox, oy] = origin;
    [(col - row) * (TILE_W as f64 / 2.0) + ox, (col + row) * (TILE_H as f64 / 2.0) - level * LEVEL_H as f64 + oy]
}

/// The tile at `level` whose top face holds the pixel (x, y): the inverse
/// of `tile_to_px` (`pxToTile`, with `Math.round` from `jsnum`). `+ 0`
/// turns a -0 into 0.
pub fn px_to_tile(pixel: [f64; 2], level: f64, origin: [f64; 2]) -> [f64; 3] {
    let [x, y] = pixel;
    let [ox, oy] = origin;
    let u = (x + 0.5 - ox) / TILE_W as f64;
    let v = (y + 0.5 - oy + level * LEVEL_H as f64) / TILE_H as f64;
    [jsnum::round(v + u) + 0.0, jsnum::round(v - u) + 0.0, level]
}

/// Where the pixel (x, y) stands at `level`, in tile space, unrounded
/// (`pxToTileExact`).
pub fn px_to_tile_exact(pixel: [f64; 2], level: f64, origin: [f64; 2]) -> [f64; 2] {
    let [x, y] = pixel;
    let [ox, oy] = origin;
    let sum = (y - oy + level * LEVEL_H as f64) / (TILE_H as f64 / 2.0);
    let diff = (x - ox) / (TILE_W as f64 / 2.0);
    [(sum + diff) / 2.0 + 0.0, (sum - diff) / 2.0 + 0.0]
}

/// The pixels of the tile whose top-face center is (cx, cy), in the order
/// `tilePixels` lists them.
pub fn tile_pixels(cx: i32, cy: i32) -> Vec<Pixel> {
    let mut out = Vec::new();
    for y in (cy - TILE_H / 2)..(cy + TILE_H / 2) {
        let half = TILE_W as f64 / 2.0 - 2.0 * (y as f64 + 0.5 - cy as f64).abs();
        for x in (cx - TILE_W / 2)..(cx + TILE_W / 2) {
            if (x as f64 + 0.5 - cx as f64).abs() < half {
                out.push((x, y));
            }
        }
    }
    out
}

/// The top-face centers of a block's footprint (`footprint`): `size` is
/// [tiles wide, tiles deep], (cx, cy) the first tile's center.
pub fn footprint(size: [i64; 2], center: [i32; 2]) -> Vec<[i32; 2]> {
    let [width, depth] = size;
    let [cx, cy] = center;
    let mut out = Vec::new();
    for j in 0..depth {
        for i in 0..width {
            out.push([cx + (i - j) as i32 * (TILE_W / 2), cy + (i + j) as i32 * (TILE_H / 2)]);
        }
    }
    out
}

/// The width and height of a block's silhouette, from its size alone
/// (`blockSize`).
pub fn block_size(size: [i64; 3]) -> [i64; 2] {
    let [width, depth, levels] = size;
    [(width + depth) * (TILE_W as i64 / 2) - 2, (width + depth) * (TILE_H as i64 / 2) + levels * LEVEL_H as i64]
}

/// A block's faces, each in the order `blockFaces` lists its pixels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Faces {
    pub top: Vec<Pixel>,
    pub left: Vec<Pixel>,
    pub right: Vec<Pixel>,
    pub edge: Vec<Pixel>,
}

/// The faces of a block whose first tile's top-face center is `center`
/// (`blockFaces`).
pub fn block_faces(size: [i64; 3], center: [i32; 2]) -> Faces {
    let [width, depth, levels] = size;
    let cx = center[0];
    let mut spans: OMap<i32, (i32, i32)> = OMap::new();
    let mut top = Vec::new();
    for [tx, ty] in footprint([width, depth], center) {
        for p in tile_pixels(tx, ty) {
            top.push(p);
            let (x, y) = p;
            match spans.get_mut(&x) {
                Some(span) => {
                    span.0 = span.0.min(y);
                    span.1 = span.1.max(y);
                }
                None => spans.set(x, (y, y)),
            }
        }
    }
    let split = cx as i64 + (width - depth) * (TILE_W as i64 / 2);
    let mut left = Vec::new();
    let mut right = Vec::new();
    for (&x, &(_, bottom)) in spans.iter() {
        for y in (bottom + 1)..=(bottom + (levels * LEVEL_H as i64) as i32) {
            if (x as i64) < split {
                left.push((x, y));
            } else {
                right.push((x, y));
            }
        }
    }
    let solid: OSet<Pixel> = top.iter().chain(&left).chain(&right).copied().collect();
    let edge =
        solid.iter().filter(|&&(x, y)| [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)].iter().any(|n| !solid.has(n))).copied().collect();
    Faces { top, left, right, edge }
}
