//! Key order (ports cli/lib/engine/serialize.mjs's tables and `prop()`).
//!
//! The Node engine held documents as JavaScript objects, whose keys iterate in
//! a fixed order: integer-like keys ("0", "12") first, ascending, then every
//! other key in the order it was added. `serde_json` with `preserve_order`
//! keeps insertion order only, so wherever the port iterates an object's keys
//! (`Object.keys`, `Object.entries`, `for…in`), it uses [`keys`] or
//! [`entries`], and the JSON writers in `format.rs` print keys this way too.
//!
//! The tables say in which order the canonical format (D4, `format.rs`)
//! writes each kind of record's known keys; unknown keys follow, in
//! [`keys`] order.

use serde_json::{Map, Value};

/// Whether a key is an array index, which JavaScript lists first: "0" or a
/// number without leading zeros, below 2^32 − 1.
pub fn is_index(key: &str) -> bool {
    if key.is_empty() || key.len() > 10 {
        return false;
    }
    if key == "0" {
        return true;
    }
    let bytes = key.as_bytes();
    if bytes[0] == b'0' || !bytes.iter().all(u8::is_ascii_digit) {
        return false;
    }
    key.parse::<u64>().is_ok_and(|n| n < 4_294_967_295)
}

/// An object's keys in JavaScript's order: index keys ascending, then the
/// rest in insertion order.
pub fn keys(map: &Map<String, Value>) -> Vec<&String> {
    let mut index: Vec<(u64, &String)> = Vec::new();
    let mut rest: Vec<&String> = Vec::new();
    for k in map.keys() {
        if is_index(k) {
            index.push((k.parse().expect("an index key is a number"), k));
        } else {
            rest.push(k);
        }
    }
    index.sort_by_key(|(n, _)| *n);
    index.into_iter().map(|(_, k)| k).chain(rest).collect()
}

/// An object's entries in JavaScript's order ([`keys`]).
pub fn entries(map: &Map<String, Value>) -> Vec<(&String, &Value)> {
    keys(map).into_iter().map(|k| (k, &map[k])).collect()
}

/// A copy of an object with its keys in JavaScript's order, so a later
/// iteration in insertion order sees them as JavaScript would.
pub fn ordered(map: &Map<String, Value>) -> Map<String, Value> {
    entries(map).into_iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

/// The palette: notes first (R7), the two tiers, then the notes on colors.
pub const PALETTE: &[&str] = &["notes", "world", "outfit", "colorNotes"];
/// A sprite object.
pub const SPRITE: &[&str] = &["notes", "kind", "extends", "anchor", "footprint", "keys", "rows", "layers"];
/// A block object.
pub const BLOCK: &[&str] = &["notes", "kind", "size", "faces", "surface", "sides"];
/// A map layer, a frame layer, and a block's surface (keys first).
pub const LAYER: &[&str] = &["keys", "class", "loop", "prefix", "frames", "map", "ambient", "actions"];
/// A block's sides.
pub const SIDES: &[&str] = &["keys", "left", "right"];
/// A scene.
pub const SCENE: &[&str] = &["notes", "output", "image", "viewBox", "origin", "layers", "items", "motion"];
/// A scene's layer entry.
pub const SCENE_LAYER: &[&str] = &["id", "name", "parent"];
/// A placed item.
pub const ITEM: &[&str] = &["object", "id", "class", "layer", "depth", "at", "ambient", "actions"];
/// A group item.
pub const GROUP: &[&str] = &["group", "depth", "items"];
/// An item's placement.
pub const AT: &[&str] = &["tile", "px", "level"];
/// A scene's motion.
pub const MOTION: &[&str] = &["duration", "tracks"];
/// A motion track.
pub const TRACK: &[&str] = &["item", "keys"];
/// A motion key.
pub const KEY: &[&str] = &["t", "move", "show", "pose"];
/// An ambient loop or an action (D6).
pub const LOOP: &[&str] = &["duration", "offset", "keys"];
/// A scene's image (D5).
pub const IMAGE: &[&str] = &["file", "scale"];

/// The keys of `map` in a table's order: the table's keys that are present,
/// then any others in [`keys`] order.
pub fn table_keys<'a>(map: &'a Map<String, Value>, table: &[&str]) -> Vec<&'a String> {
    let mut out: Vec<&String> = Vec::new();
    for k in table {
        if let Some((key, _)) = map.get_key_value(*k) {
            out.push(key);
        }
    }
    for k in keys(map) {
        if !table.contains(&k.as_str()) {
            out.push(k);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn index_keys_come_first() {
        let v = json!({ "b": 1, "10": 2, "a": 3, "2": 4, "01": 5, "-1": 6 });
        let m = v.as_object().unwrap();
        let k: Vec<&str> = keys(m).into_iter().map(String::as_str).collect();
        assert_eq!(k, ["2", "10", "b", "a", "01", "-1"]);
    }

    #[test]
    fn tables_put_known_keys_first() {
        let v = json!({ "at": {}, "zz": 1, "object": "pine", "id": "a" });
        let m = v.as_object().unwrap();
        let k: Vec<&str> = table_keys(m, ITEM).into_iter().map(String::as_str).collect();
        assert_eq!(k, ["object", "id", "at", "zz"]);
    }
}
