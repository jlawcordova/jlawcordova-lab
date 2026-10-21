//! Starter sources for new objects, scenes and documents (ports
//! cli/lib/engine/starter.mjs and starter-palette.mjs; pixel-art engine spec
//! R38, D9.6), so a new file is the same whichever door makes it.

use serde_json::Value;

/// Names are at most this long.
pub const MAX_NAME: usize = 64;

/// The palette a new document starts from (starter-palette.mjs), with its
/// comments as notes (R7).
pub const STARTER_PALETTE: &str = include_str!("starter-palette.json");

/// The starter palette as data.
pub fn starter_palette() -> Value {
    todo!("U2: parse STARTER_PALETTE")
}

/// An empty scene: a 128×128 view around tile [0, 0, 0] (`starterScene`).
pub fn starter_scene() -> Value {
    todo!("U2: port starter.mjs starterScene")
}

/// An empty sprite, anchored at its bottom center (`starterSprite`).
pub fn starter_sprite(width: i64, height: i64) -> Value {
    let _ = (width, height);
    todo!("U2: port starter.mjs starterSprite")
}

/// A one-level block (`starterBlock`).
pub fn starter_block() -> Value {
    todo!("U2: port starter.mjs starterBlock")
}

/// An object that extends `base` with no rows overridden yet
/// (`starterExtends`).
pub fn starter_extends(base: &str) -> Value {
    let _ = base;
    todo!("U2: port starter.mjs starterExtends")
}
