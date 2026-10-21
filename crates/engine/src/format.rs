//! Format 2 (docs/intents/2026-10-rust-engine/spec.md, R6, R7, D4): JSON
//! sources, their canonical text, JSON text as JavaScript writes it, and
//! reading a format-2 document's sources from its folder.
//!
//! The canonical text of a source:
//! - two-space indentation, keys in the order of the tables in
//!   `serialize.rs` (unknown keys after them), and a final newline;
//! - a list of numbers on one line (`"size": [1, 1, 1]`), any other list one
//!   element per line, so a map's rows are one string per line;
//! - one line for each placed item, scene layer and motion key, and for a
//!   group's attributes and a scene's image, as the `.mjs` sources had them,
//!   so a scene merges and diffs a line per item;
//! - an item's `at.level` is left out when it is 0, as `serialize.mjs` did.
//!
//! `scripts/goldens.mjs` writes the same text from Node for the bootstrap
//! copy of the corpus; `serialize(parse(text)) == text` for every file.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::{Map, Value};

use crate::jsnum;
use crate::serialize::{self as order, keys};

// ------------------------------------------------------------------ strings

/// A string as `JSON.stringify` writes it: `"` and `\` escaped, the short
/// escapes for \b \f \n \r \t, `\u00xx` for other control characters, and
/// everything else as it is.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    quote_into(s, &mut out);
    out
}

fn quote_into(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn scalar_into(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&jsnum::number_string(n)),
        Value::String(s) => quote_into(s, out),
        Value::Array(_) | Value::Object(_) => unreachable!("not a scalar"),
    }
}

// ------------------------------------------------------- JSON.stringify(v)

/// `JSON.stringify(v)`: no spaces, keys in JavaScript's order.
pub fn stringify(v: &Value) -> String {
    let mut out = String::new();
    compact_into(v, &mut out);
    out
}

fn compact_into(v: &Value, out: &mut String) {
    match v {
        Value::Array(list) => {
            out.push('[');
            for (i, e) in list.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                compact_into(e, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            for (i, k) in keys(map).into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                quote_into(k, out);
                out.push(':');
                compact_into(&map[k], out);
            }
            out.push('}');
        }
        _ => scalar_into(v, out),
    }
}

/// `JSON.stringify(v, null, 2)`.
pub fn stringify_pretty(v: &Value) -> String {
    let mut out = String::new();
    indented_into(v, 0, &mut out);
    out
}

fn indented_into(v: &Value, depth: usize, out: &mut String) {
    match v {
        Value::Array(list) if !list.is_empty() => {
            out.push_str("[\n");
            for (i, e) in list.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                pad(depth + 1, out);
                indented_into(e, depth + 1, out);
            }
            out.push('\n');
            pad(depth, out);
            out.push(']');
        }
        Value::Object(map) if !map.is_empty() => {
            out.push_str("{\n");
            for (i, k) in keys(map).into_iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                pad(depth + 1, out);
                quote_into(k, out);
                out.push_str(": ");
                indented_into(&map[k], depth + 1, out);
            }
            out.push('\n');
            pad(depth, out);
            out.push('}');
        }
        Value::Array(_) => out.push_str("[]"),
        Value::Object(_) => out.push_str("{}"),
        _ => scalar_into(v, out),
    }
}

fn pad(depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

// ----------------------------------------------------------- canonical text

/// How a value is laid out in the canonical text.
#[derive(Clone, Copy)]
enum Shape {
    /// Lists of numbers on one line, anything else one element or key per line.
    Pretty,
    /// All on one line: `{ "k": v }`, `[a, b]`.
    Inline,
    /// A record on one line, its known keys in a table's order.
    InlineRecord(&'static [&'static str]),
    /// A record whose known keys follow a table, each value by its key.
    Record(&'static [&'static str], fn(&str) -> Shape),
    /// A list whose elements each take a shape.
    List(fn(&Value) -> Shape),
}

fn pretty(_: &str) -> Shape {
    Shape::Pretty
}

fn write(v: &Value, shape: Shape, depth: usize, out: &mut String) {
    match (shape, v) {
        (Shape::Inline, _) => inline_into(v, None, out),
        (Shape::InlineRecord(table), _) => inline_into(v, Some(table), out),
        (Shape::Record(table, by_key), Value::Object(map)) => record_into(map, table, by_key, depth, out),
        (Shape::List(each), Value::Array(list)) => {
            if list.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (i, e) in list.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                pad(depth + 1, out);
                write(e, each(e), depth + 1, out);
            }
            out.push('\n');
            pad(depth, out);
            out.push(']');
        }
        (_, Value::Array(list)) if !list.is_empty() && list.iter().all(Value::is_number) => inline_into(v, None, out),
        (_, Value::Array(list)) if !list.is_empty() => {
            out.push_str("[\n");
            for (i, e) in list.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                pad(depth + 1, out);
                write(e, Shape::Pretty, depth + 1, out);
            }
            out.push('\n');
            pad(depth, out);
            out.push(']');
        }
        (_, Value::Object(map)) => record_into(map, &[], pretty, depth, out),
        (_, Value::Array(_)) => out.push_str("[]"),
        _ => scalar_into(v, out),
    }
}

fn record_into(map: &Map<String, Value>, table: &[&str], by_key: fn(&str) -> Shape, depth: usize, out: &mut String) {
    if map.is_empty() {
        out.push_str("{}");
        return;
    }
    out.push_str("{\n");
    for (i, k) in order::table_keys(map, table).into_iter().enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        pad(depth + 1, out);
        quote_into(k, out);
        out.push_str(": ");
        write(&map[k], by_key(k), depth + 1, out);
    }
    out.push('\n');
    pad(depth, out);
    out.push('}');
}

/// One line. A record's keys follow `table` when one is given; an item's
/// `at` drops a level of 0.
/// Not a key order: an item's `actions`, a record of named actions, each in
/// the loop's order.
const ACTIONS: &[&str] = &["\u{0}actions"];

fn inline_into(v: &Value, table: Option<&[&str]>, out: &mut String) {
    match v {
        Value::Array(list) => {
            out.push('[');
            for (i, e) in list.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                // A list of records (a loop's keys) puts each in the table's order.
                inline_into(e, table, out);
            }
            out.push(']');
        }
        Value::Object(map) if map.is_empty() => out.push_str("{}"),
        Value::Object(map) => {
            let names = match table {
                Some(t) if t != ACTIONS => order::table_keys(map, t),
                _ => keys(map),
            };
            out.push_str("{ ");
            let mut first = true;
            for k in names {
                let value = &map[k];
                if table == Some(order::AT) && k == "level" && value.as_f64() == Some(0.0) {
                    continue;
                }
                if !first {
                    out.push_str(", ");
                }
                first = false;
                quote_into(k, out);
                out.push_str(": ");
                let inner = match (table, k.as_str()) {
                    (Some(t), "at") if t == order::ITEM => Some(order::AT),
                    (Some(t), "ambient") if t == order::ITEM => Some(order::LOOP),
                    (Some(t), "actions") if t == order::ITEM => Some(ACTIONS),
                    (Some(t), _) if t == ACTIONS => Some(order::LOOP),
                    (Some(t), "keys") if t == order::LOOP => Some(order::KEY),
                    _ => None,
                };
                inline_into(value, inner, out);
            }
            out.push_str(" }");
        }
        _ => scalar_into(v, out),
    }
}

fn item_shape(v: &Value) -> Shape {
    match v {
        Value::Object(map) if map.contains_key("group") => Shape::Record(order::GROUP, group_key),
        _ => Shape::InlineRecord(order::ITEM),
    }
}

fn group_key(k: &str) -> Shape {
    match k {
        "group" => Shape::Inline,
        "items" => Shape::List(item_shape),
        _ => Shape::Pretty,
    }
}

fn scene_key(k: &str) -> Shape {
    match k {
        "layers" => Shape::List(|_| Shape::InlineRecord(order::SCENE_LAYER)),
        "items" => Shape::List(item_shape),
        "motion" => Shape::Record(order::MOTION, motion_key),
        "image" => Shape::InlineRecord(order::IMAGE),
        _ => Shape::Pretty,
    }
}

fn motion_key(k: &str) -> Shape {
    match k {
        "tracks" => Shape::List(|_| Shape::Record(order::TRACK, track_key)),
        _ => Shape::Pretty,
    }
}

fn track_key(k: &str) -> Shape {
    match k {
        "keys" => Shape::List(|_| Shape::InlineRecord(order::KEY)),
        _ => Shape::Pretty,
    }
}

fn layer_key(k: &str) -> Shape {
    match k {
        "ambient" => Shape::Record(order::LOOP, loop_key),
        // Named actions, each written as an ambient loop is.
        "actions" => Shape::Record(&[], |_| Shape::Record(order::LOOP, loop_key)),
        _ => Shape::Pretty,
    }
}

fn loop_key(k: &str) -> Shape {
    match k {
        "keys" => Shape::List(|_| Shape::InlineRecord(order::KEY)),
        _ => Shape::Pretty,
    }
}

fn object_key(k: &str) -> Shape {
    match k {
        "layers" => Shape::List(|_| Shape::Record(order::LAYER, layer_key)),
        "surface" => Shape::Record(order::LAYER, layer_key),
        "sides" => Shape::Record(order::SIDES, pretty),
        _ => Shape::Pretty,
    }
}

fn finish(v: &Value, shape: Shape) -> String {
    let mut out = String::new();
    write(v, shape, 0, &mut out);
    out.push('\n');
    out
}

/// The canonical text of an object (it has a `kind`) or a scene.
pub fn serialize(doc: &Value) -> String {
    match doc {
        Value::Object(map) if map.contains_key("kind") => serialize_object(doc),
        _ => serialize_scene(doc),
    }
}

/// The canonical text of an object, a sprite or a block.
pub fn serialize_object(doc: &Value) -> String {
    let block = doc.get("kind").and_then(Value::as_str) == Some("block");
    finish(doc, Shape::Record(if block { order::BLOCK } else { order::SPRITE }, object_key))
}

/// The canonical text of a scene.
pub fn serialize_scene(doc: &Value) -> String {
    finish(doc, Shape::Record(order::SCENE, scene_key))
}

/// The canonical text of a palette.
pub fn serialize_palette(doc: &Value) -> String {
    finish(doc, Shape::Record(order::PALETTE, pretty))
}

/// A manifest's text: `JSON.stringify(manifest, null, 2)` and a newline, as
/// `document new` and `document copy` write it.
pub fn serialize_manifest(manifest: &Value) -> String {
    let mut out = stringify_pretty(manifest);
    out.push('\n');
    out
}

/// The canonical text of a source, by the file it's in: `palette.json`,
/// `objects/<name>.json` or `scenes/<name>.json`.
pub fn serialize_file(file: &str, doc: &Value) -> String {
    if file == "palette.json" {
        serialize_palette(doc)
    } else if file.starts_with("objects/") {
        serialize_object(doc)
    } else {
        serialize_scene(doc)
    }
}

// ------------------------------------------------------------------ reading

/// Parses JSON text, keeping the order of keys as written.
pub fn parse(text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}

/// A file's size and modification time, as `fs.stat` gives them.
#[derive(Clone, Debug, PartialEq)]
pub struct FileStat {
    pub path: PathBuf,
    /// Milliseconds since the epoch, with the fraction `stat` gives.
    pub mtime_ms: f64,
    pub size: u64,
}

/// A format-2 document's sources, as `loadDocs` read them: the palette, the
/// objects and scenes by name (in name order), the files that couldn't be
/// read as "<file>: cannot be loaded: <message>" (sorted), and every file
/// read, with its stat.
#[derive(Clone, Debug, Default)]
pub struct Docs {
    pub palette: Option<Value>,
    pub objects: BTreeMap<String, Value>,
    pub scenes: BTreeMap<String, Value>,
    pub load_errors: Vec<String>,
    pub files: BTreeMap<String, FileStat>,
}

/// The file name of a source: `palette.json`, `objects/<name>.json`,
/// `scenes/<name>.json`.
pub const PALETTE_FILE: &str = "palette.json";
pub fn object_file(name: &str) -> String {
    format!("objects/{name}.json")
}
pub fn scene_file(name: &str) -> String {
    format!("scenes/{name}.json")
}

/// A stat of `path`, or None.
pub fn stat(path: &Path) -> Option<FileStat> {
    let meta = fs::metadata(path).ok()?;
    let mtime_ms = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0.0, |d| d.as_secs_f64() * 1000.0);
    Some(FileStat { path: path.to_path_buf(), mtime_ms, size: meta.len() })
}

/// Reads `palette.json`, `objects/*.json` and `scenes/*.json` from a
/// format-2 document folder.
pub fn read_docs(dir: &Path) -> Docs {
    let mut docs = Docs::default();
    let load = |file: &str, docs: &mut Docs| -> Option<Value> {
        let path = dir.join(file);
        match stat(&path) {
            Some(s) => {
                docs.files.insert(file.to_string(), s);
            }
            None => {
                docs.load_errors.push(format!("{file}: cannot be loaded: no such file"));
                return None;
            }
        }
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                docs.load_errors.push(format!("{file}: cannot be loaded: {e}"));
                return None;
            }
        };
        match parse(&text) {
            Ok(v) => Some(v),
            Err(e) => {
                docs.load_errors.push(format!("{file}: cannot be loaded: {}", e.lines().next().unwrap_or("")));
                None
            }
        }
    };
    docs.palette = load(PALETTE_FILE, &mut docs);
    for sub in ["objects", "scenes"] {
        let mut names: Vec<String> = match fs::read_dir(dir.join(sub)) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|f| f.ends_with(".json"))
                .collect(),
            Err(_) => continue,
        };
        names.sort();
        for f in names {
            let file = format!("{sub}/{f}");
            if let Some(v) = load(&file, &mut docs) {
                let name = f[..f.len() - ".json".len()].to_string();
                if sub == "objects" {
                    docs.objects.insert(name, v);
                } else {
                    docs.scenes.insert(name, v);
                }
            }
        }
    }
    docs.load_errors.sort();
    docs
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stringifies_like_javascript() {
        let v = json!({ "b": [1, 2.5, "x\n\"y\u{1}"], "1": null, "a": {}, "c": [] });
        assert_eq!(stringify(&v), r#"{"1":null,"b":[1,2.5,"x\n\"y\u0001"],"a":{},"c":[]}"#);
        assert_eq!(
            stringify_pretty(&v),
            "{\n  \"1\": null,\n  \"b\": [\n    1,\n    2.5,\n    \"x\\n\\\"y\\u0001\"\n  ],\n  \"a\": {},\n  \"c\": []\n}"
        );
    }

    #[test]
    fn writes_a_scene_canonically() {
        let scene = json!({
            "items": [
                { "at": { "tile": [0, 0, 0] }, "object": "block" },
                { "group": { "class": "x" }, "items": [{ "object": "p", "at": { "px": [3, 4], "level": 0 } }], "depth": "front" }
            ],
            "viewBox": [-16, -16, 64, 48],
            "output": "sample.svg",
            "layers": [{ "name": "A", "id": "a" }],
            "motion": { "duration": 1000, "tracks": [{ "item": "a", "keys": [{ "t": 0, "move": [0, 0] }] }] }
        });
        let text = serialize(&scene);
        assert_eq!(
            text,
            r#"{
  "output": "sample.svg",
  "viewBox": [-16, -16, 64, 48],
  "layers": [
    { "id": "a", "name": "A" }
  ],
  "items": [
    { "object": "block", "at": { "tile": [0, 0, 0] } },
    {
      "group": { "class": "x" },
      "depth": "front",
      "items": [
        { "object": "p", "at": { "px": [3, 4] } }
      ]
    }
  ],
  "motion": {
    "duration": 1000,
    "tracks": [
      {
        "item": "a",
        "keys": [
          { "t": 0, "move": [0, 0] }
        ]
      }
    ]
  }
}
"#
        );
        assert_eq!(serialize(&parse(&text).unwrap()), text);
    }

    #[test]
    fn writes_an_object_canonically() {
        let obj = json!({
            "layers": [{ "map": [".a", "a."], "class": "x" }],
            "keys": { "a": "ink" },
            "anchor": [1, 2],
            "kind": "sprite"
        });
        let text = serialize(&obj);
        assert_eq!(
            text,
            "{\n  \"kind\": \"sprite\",\n  \"anchor\": [1, 2],\n  \"keys\": {\n    \"a\": \"ink\"\n  },\n  \"layers\": [\n    {\n      \"class\": \"x\",\n      \"map\": [\n        \".a\",\n        \"a.\"\n      ]\n    }\n  ]\n}\n"
        );
    }
}
