//! The isometric grid (ports cli/lib/engine/iso.mjs): 2:1 dimetric, 32×16
//! tiles, 16-pixel levels. Grid positions only; block faces come with the
//! block kind.
//!
//! Positions are doubles, as JavaScript held them; pixels (`Pixel`) are
//! whole numbers.

/// A pixel, (x, y): what the Node engine wrote as the key "x,y".
pub type Pixel = (i32, i32);

pub const TILE_W: i32 = 32;
pub const TILE_H: i32 = 16;
pub const LEVEL_H: i32 = 16;

/// The top-face center of the tile at [col, row, level], in scene pixels.
/// `origin` is where tile [0, 0, 0] sits.
pub fn tile_to_px(tile: [f64; 3], origin: [f64; 2]) -> [f64; 2] {
    let _ = (tile, origin);
    todo!("U1: port iso.mjs tileToPx")
}

/// The tile at `level` whose top face holds the pixel (x, y): the inverse
/// of `tile_to_px` (`pxToTile`, with `Math.round` from `jsnum`).
pub fn px_to_tile(pixel: [f64; 2], level: f64, origin: [f64; 2]) -> [f64; 3] {
    let _ = (pixel, level, origin);
    todo!("U1: port iso.mjs pxToTile")
}

/// Where the pixel (x, y) stands at `level`, in tile space, unrounded
/// (`pxToTileExact`).
pub fn px_to_tile_exact(pixel: [f64; 2], level: f64, origin: [f64; 2]) -> [f64; 2] {
    let _ = (pixel, level, origin);
    todo!("U1: port iso.mjs pxToTileExact")
}

/// The pixels of the tile whose top-face center is (cx, cy), in the order
/// `tilePixels` lists them.
pub fn tile_pixels(cx: i32, cy: i32) -> Vec<Pixel> {
    let _ = (cx, cy);
    todo!("U1: port iso.mjs tilePixels")
}

/// The top-face centers of a block's footprint (`footprint`): `size` is
/// [tiles wide, tiles deep], (cx, cy) the first tile's center.
pub fn footprint(size: [i64; 2], center: [i32; 2]) -> Vec<[i32; 2]> {
    let _ = (size, center);
    todo!("U1: port iso.mjs footprint")
}

/// The width and height of a block's silhouette, from its size alone
/// (`blockSize`).
pub fn block_size(size: [i64; 3]) -> [i64; 2] {
    let _ = size;
    todo!("U1: port iso.mjs blockSize")
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
    let _ = (size, center);
    todo!("U1: port iso.mjs blockFaces")
}
