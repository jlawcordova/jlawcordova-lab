//! Starter sources for new objects, scenes and documents (ports
//! cli/lib/engine/starter.mjs and starter-palette.mjs; pixel-art engine spec
//! R38, D9.6), so a new file is the same whichever door makes it.

use serde_json::{json, Value};

/// Names are at most this long.
pub const MAX_NAME: usize = 64;

/// The palette a new document starts from (starter-palette.mjs), with its
/// comments as notes (R7).
pub const STARTER_PALETTE: &str = include_str!("starter-palette.json");

/// The starter palette as data.
pub fn starter_palette() -> Value {
    crate::format::parse(STARTER_PALETTE).expect("starter-palette.json is valid")
}

/// An empty scene: a 128×128 view around tile [0, 0, 0] (`starterScene`).
pub fn starter_scene() -> Value {
    json!({ "viewBox": [-64, -64, 128, 128], "origin": [0, 0], "items": [] })
}

/// An empty sprite, anchored at its bottom center (`starterSprite`).
pub fn starter_sprite(width: i64, height: i64) -> Value {
    let rows = (0..height.max(0)).map(|_| ".".repeat(width.max(0) as usize)).collect::<Vec<_>>();
    json!({
        "kind": "sprite",
        "anchor": [width.div_euclid(2), height - 1],
        "keys": { "o": "ink" },
        "layers": [{ "map": rows }],
    })
}

/// A one-level block (`starterBlock`).
pub fn starter_block() -> Value {
    json!({ "kind": "block", "size": [1, 1, 1], "faces": { "top": "grass-2", "left": "soil-2", "right": "soil-3" } })
}

/// An object that extends `base` with no rows overridden yet
/// (`starterExtends`).
pub fn starter_extends(base: &str) -> Value {
    json!({ "kind": "sprite", "extends": base, "keys": {}, "rows": {} })
}
