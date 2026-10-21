//! The pixel-art engine (ports cli/lib/engine/engine.mjs; pixel-art engine
//! spec D2–D6). It takes the palette, objects and scenes as plain data,
//! validates them, resolves objects, and renders scenes and objects to layer
//! trees, each list of items in depth order.
//!
//! Documents are `serde_json::Value`s, as the Node engine held plain data.
//! Where the Node engine would have thrown on data of the wrong shape (a
//! `TypeError`), this returns an `EngineError` instead; validation reports
//! the same data first, with the Node engine's messages.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use serde_json::{Map, Value};

use crate::depth::{self, Bounds, Box3, Copy, Mark, Node};
use crate::iso::{self, Pixel, TILE_H, TILE_W};
use crate::motion;
use crate::omap::{OMap, OSet};
use crate::serialize::{entries, keys};
use crate::{jsnum, EngineError, Result};

/// The palette's tiers, in the order the engine reads them.
pub const TIERS: [&str; 2] = ["world", "outfit"];

/// The design language's caps (R26, R27).
pub struct Caps {
    pub world: usize,
    pub outfit: usize,
    pub size: i64,
    pub colors: usize,
    pub character_width: i64,
    pub character_height: i64,
}

pub const CAPS: Caps = Caps { world: 32, outfit: 16, size: 64, colors: 12, character_width: 24, character_height: 32 };

/// A sprite's footprint is whole tiles, 1 to this many each way.
pub const MAX_FOOTPRINT: i64 = 16;
/// A side map is one tile's face wide and one level high.
pub const SIDE_W: i32 = crate::iso::TILE_W / 2;
pub const SIDE_H: i32 = crate::iso::LEVEL_H;

/// A palette color: its hex and its tier ("world" or "outfit").
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteColor {
    pub hex: String,
    pub tier: &'static str,
}

/// What an object needs for the depth order, cached per set of sources
/// (`extentOf`).
#[derive(Clone, Debug, PartialEq)]
pub struct Extent {
    pub bounds: Option<Bounds>,
    pub footprint: [i64; 2],
    pub height: f64,
    pub block: Option<[i64; 3]>,
}

/// The three source kinds, wrapped for the rest of the engine
/// (`loadSources`): objects and scenes in code-point order by name, and the
/// palette's colors by name, world before outfit.
#[derive(Debug, Default)]
pub struct Sources {
    pub palette: Value,
    pub colors: OMap<String, PaletteColor>,
    pub objects: BTreeMap<String, Value>,
    pub scenes: BTreeMap<String, Value>,
    /// `extentOf`'s cache. Whoever changes `objects` clears it.
    pub extents: RefCell<HashMap<String, Extent>>,
}

impl Clone for Sources {
    fn clone(&self) -> Self {
        Sources {
            palette: self.palette.clone(),
            colors: self.colors.clone(),
            objects: self.objects.clone(),
            scenes: self.scenes.clone(),
            extents: RefCell::new(self.extents.borrow().clone()),
        }
    }
}

/// Wraps the three source kinds (`loadSources`).
pub fn load_sources(palette: Value, objects: BTreeMap<String, Value>, scenes: BTreeMap<String, Value>) -> Sources {
    let mut colors = OMap::new();
    for tier in TIERS {
        if let Some(Value::Object(map)) = palette.get(tier) {
            for (name, hex) in entries(map) {
                if !colors.has(name.as_str()) {
                    colors.set(name.clone(), PaletteColor { hex: js_string(Some(hex)), tier });
                }
            }
        }
    }
    Sources { palette, colors, objects, scenes, extents: RefCell::new(HashMap::new()) }
}

/// Code-point order, never locale order (`byCodePoint`).
pub fn by_code_point(a: &str, b: &str) -> std::cmp::Ordering {
    a.cmp(b)
}

// ------------------------------------------------------------ JS helpers

/// `String(v)`, for a JSON value or `undefined` (None).
pub fn js_string(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(Value::Null) => "null".into(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => jsnum::number_string(n),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(list)) => list
            .iter()
            .map(|e| match e {
                Value::Null => String::new(),
                e => js_string(Some(e)),
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(Value::Object(_)) => "[object Object]".into(),
    }
}

/// `'${String(v)}'`: how problems quote a value.
pub fn q(v: Option<&Value>) -> String {
    format!("'{}'", js_string(v))
}

fn qs(s: &str) -> String {
    format!("'{s}'")
}

/// A string's length as JavaScript counts it (UTF-16 code units).
pub fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// JavaScript truthiness of an optional value.
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0 && !x.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

fn is_record(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Object(_)))
}

fn is_int(v: &Value) -> bool {
    v.as_f64().is_some_and(jsnum::is_integer)
}

/// `isWhole(v, n)`: a list of `n` whole numbers.
fn is_whole(v: Option<&Value>, n: usize) -> bool {
    matches!(v, Some(Value::Array(list)) if list.len() == n && list.iter().all(is_int))
}

fn num_at(v: &Value, i: usize) -> f64 {
    v.get(i).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

fn int_at(v: &Value, i: usize) -> i64 {
    num_at(v, i) as i64
}

fn has(map: &Map<String, Value>, key: &str) -> bool {
    map.contains_key(key)
}

fn lowercase_word(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

/// `^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$`: lowercase kebab-case.
pub fn is_name(name: &str) -> bool {
    let mut parts = name.split('-');
    let first = parts.next().unwrap_or("");
    let mut chars = first.chars();
    if !matches!(chars.next(), Some(c) if c.is_ascii_lowercase()) || !chars.all(lowercase_word) {
        return false;
    }
    parts.all(|p| !p.is_empty() && p.chars().all(lowercase_word))
}

/// `^[a-z][a-z0-9-]*$`: one class name.
pub fn is_class_token(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase()) && chars.all(|c| lowercase_word(c) || c == '-')
}

/// `^[a-z][a-z0-9-]*(?: [a-z][a-z0-9-]*)*$`: class names separated by single spaces.
pub fn is_class_list(s: &str) -> bool {
    s.split(' ').all(is_class_token)
}

/// `^#[0-9A-F]{6}$`.
fn is_hex(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c))
}

/// `^[a-z][a-z0-9]*(?:-[a-z0-9]+)*\.svg$`.
fn is_output(s: &str) -> bool {
    s.strip_suffix(".svg").is_some_and(is_name)
}

/// `^data-[a-z][a-z0-9-]*$`.
fn is_data_attr(s: &str) -> bool {
    s.strip_prefix("data-").is_some_and(is_class_token)
}

/// JavaScript's `\s`.
pub fn js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

/// `^\S(?:[^\n\r\t]{0,62}\S)?$`: a layer's name, 1–64 UTF-16 units on one
/// line, no spaces at either end.
pub fn is_layer_name(s: &str) -> bool {
    let (Some(first), Some(last)) = (s.chars().next(), s.chars().last()) else { return false };
    if js_space(first) || js_space(last) || s.contains(['\n', '\r', '\t']) {
        return false;
    }
    // `\S` matches one UTF-16 unit; a name of one astral character is two units and doesn't match.
    let len = js_len(s);
    len == 1 || (2..=64).contains(&len)
}

/// A map key: one printable ASCII character other than space and '.'.
pub fn is_key(k: &str) -> bool {
    let mut chars = k.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c > ' ' && c <= '~' && c != '.')
}

/// `objects/<name>.json`: the file a problem names.
pub fn object_file(name: &str) -> String {
    crate::format::object_file(name)
}

/// `scenes/<name>.json`.
pub fn scene_file(name: &str) -> String {
    crate::format::scene_file(name)
}

// ---------------------------------------------------------------- validation

/// Every problem in the sources, as "<file>: <where>: <rule>" lines. It
/// never stops at the first problem.
pub fn validate(sources: &Sources) -> Vec<String> {
    let mut problems = Vec::new();
    validate_palette(sources, &mut problems);
    let mut broken = HashSet::new();
    for name in sources.objects.keys() {
        let before = problems.len();
        validate_object_shape(sources, name, &mut problems);
        if problems.len() > before {
            broken.insert(name.clone());
        }
    }
    for name in sources.objects.keys() {
        if broken.contains(name) {
            continue;
        }
        if sources.objects[name].get("kind").and_then(Value::as_str) == Some("block") {
            validate_block_resolved(sources, name, &mut problems);
        } else {
            validate_resolved(sources, name, &broken, &mut problems);
        }
    }
    let mut outputs: HashMap<String, String> = HashMap::new();
    for name in sources.scenes.keys() {
        validate_scene(sources, name, &mut outputs, &mut problems);
    }
    problems
}

/// `notes`: a list of strings (R7).
fn notes_ok(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Array(list)) if list.iter().all(Value::is_string))
}

fn validate_palette(sources: &Sources, problems: &mut Vec<String>) {
    let file = crate::format::PALETTE_FILE;
    let Some(palette) = sources.palette.as_object() else {
        problems.push(format!("{file}: must export an object with world and outfit"));
        return;
    };
    for key in keys(palette) {
        if !TIERS.contains(&key.as_str()) && key != "notes" && key != "colorNotes" {
            problems.push(format!("{file}: unknown tier {}", qs(key)));
        }
    }
    if palette.contains_key("notes") && !notes_ok(palette.get("notes")) {
        problems.push(format!("{file}: notes must be a list of strings"));
    }
    let mut seen: HashMap<&str, &str> = HashMap::new();
    for tier in TIERS {
        let Some(Value::Object(colors)) = palette.get(tier) else {
            problems.push(format!("{file}: {tier} must be an object of name: '#RRGGBB'"));
            continue;
        };
        let cap = if tier == "world" { CAPS.world } else { CAPS.outfit };
        let names = keys(colors);
        if names.len() > cap {
            problems.push(format!("{file}: {tier} has {} colors, max {cap}", names.len()));
        }
        for name in names {
            if !is_name(name) {
                problems.push(format!("{file}: {tier} {}: names are lowercase kebab-case", qs(name)));
            }
            let value = &colors[name];
            if !value.as_str().is_some_and(is_hex) {
                problems.push(format!("{file}: {tier} {}: {} must be uppercase #RRGGBB, with no alpha", qs(name), q(Some(value))));
            }
            match seen.get(name.as_str()) {
                Some(other) => problems.push(format!("{file}: {} is in both {other} and {tier}; names must be unique", qs(name))),
                None => {
                    seen.insert(name, tier);
                }
            }
        }
    }
    if let Some(notes) = palette.get("colorNotes") {
        match notes.as_object() {
            Some(map) if map.values().all(Value::is_string) => {
                for name in keys(map) {
                    if !sources.colors.has(name.as_str()) {
                        problems.push(format!("{file}: colorNotes {} is no color of world or outfit", qs(name)));
                    }
                }
            }
            _ => problems.push(format!("{file}: colorNotes must be an object of color name: note")),
        }
    }
}

const OBJECT_PROPS: [&str; 8] = ["notes", "kind", "extends", "anchor", "footprint", "keys", "rows", "layers"];
const BLOCK_PROPS: [&str; 6] = ["notes", "kind", "size", "faces", "surface", "sides"];
const SIDES_PROPS: [&str; 3] = ["keys", "left", "right"];
const FACES: [&str; 4] = ["top", "left", "right", "edge"];
const MAP_LAYER_PROPS: [&str; 2] = ["class", "map"];
const FRAME_LAYER_PROPS: [&str; 3] = ["loop", "prefix", "frames"];

fn is_string_list(v: &Value) -> bool {
    matches!(v, Value::Array(list) if list.iter().all(Value::is_string))
}

fn rows_of(v: &Value) -> Vec<&str> {
    v.as_array().map(|l| l.iter().filter_map(Value::as_str).collect()).unwrap_or_default()
}

/// Checks the keys of a record of key → color name.
fn check_keys(sources: &Sources, map: &Map<String, Value>, at: &str, add: &mut dyn FnMut(String)) {
    for (key, color) in entries(map) {
        if !is_key(key) {
            add(format!("{at}key {}: keys are one printable character, not space or '.'", qs(key)));
        }
        if !color.as_str().is_some_and(|c| sources.colors.has(c)) {
            add(format!("{at}key {}: {} is not in the palette", qs(key), q(Some(color))));
        }
    }
}

/// Checks one object on its own: its fields, keys and maps.
fn validate_object_shape(sources: &Sources, name: &str, problems: &mut Vec<String>) {
    let file = object_file(name);
    let mut add = |msg: String| problems.push(format!("{file}: {msg}"));
    let obj = &sources.objects[name];
    if !is_name(name) {
        add("names are lowercase kebab-case".into());
    }
    let Some(o) = obj.as_object() else { return add("must export an object".into()) };
    if obj.get("kind").and_then(Value::as_str) == Some("block") {
        validate_block_shape(sources, o, &mut add);
        if has(o, "sides") {
            validate_sides(sources, o, &mut add);
        }
        return;
    }
    for key in keys(o) {
        if !OBJECT_PROPS.contains(&key.as_str()) {
            add(format!("unknown property {}", qs(key)));
        }
    }
    if has(o, "notes") && !notes_ok(o.get("notes")) {
        add("notes must be a list of strings".into());
    }
    if o.get("kind").and_then(Value::as_str) != Some("sprite") {
        add(format!("kind must be 'sprite' or 'block', got {}", q(o.get("kind"))));
    }
    if has(o, "anchor") && !is_whole(o.get("anchor"), 2) {
        add("anchor must be two whole numbers".into());
    }
    if has(o, "footprint") {
        let fp = o.get("footprint");
        let ok =
            is_whole(fp, 2) && fp.unwrap().as_array().unwrap().iter().all(|n| (1.0..=MAX_FOOTPRINT as f64).contains(&n.as_f64().unwrap()));
        if !ok {
            add(format!("footprint must be two whole numbers, tiles wide and deep, from 1 to {MAX_FOOTPRINT}"));
        }
    }
    let empty = Map::new();
    let keys_map = match o.get("keys") {
        Some(Value::Object(m)) => m,
        _ => &empty,
    };
    if has(o, "keys") && !is_record(o.get("keys")) {
        add("keys must be an object of key: color name".into());
    } else {
        check_keys(sources, keys_map, "", &mut add);
    }

    if has(o, "extends") {
        let ext = o.get("extends");
        match ext.and_then(Value::as_str) {
            Some(base) if sources.objects.contains_key(base) => {
                if sources.objects[base].get("kind").and_then(Value::as_str) == Some("block") {
                    add(format!("extends {}, which is a block; only a sprite can be extended", q(ext)));
                }
            }
            _ => add(format!("extends {}, which does not exist", q(ext))),
        }
        if has(o, "layers") {
            add("an object that extends another gives rows, not layers".into());
        }
        let rows_shape = "rows must be an object of layer: { row: string }";
        if !has(o, "rows") {
            return;
        }
        let Some(rows) = o.get("rows").and_then(Value::as_object) else { return add(rows_shape.into()) };
        for (li, layer_rows) in entries(rows) {
            let ok = matches!(layer_rows, Value::Object(m) if m.values().all(Value::is_string));
            if !ok {
                add(format!("layer {li} (override): {rows_shape}"));
            }
        }
        return;
    }
    if has(o, "rows") {
        add("rows is only for an object that extends another".into());
    }
    let layers = match o.get("layers") {
        Some(Value::Array(list)) if !list.is_empty() => list,
        _ => return add("layers must be a list of at least one layer".into()),
    };

    let mut maps: Vec<(String, Vec<&str>)> = Vec::new();
    for (li, layer) in layers.iter().enumerate() {
        let place = format!("layer {li}");
        let Some(l) = layer.as_object() else {
            add(format!("{place}: must be an object with a map, or a loop, prefix and frames"));
            continue;
        };
        let framed = l.contains_key("frames");
        let allowed: &[&str] = if framed { &FRAME_LAYER_PROPS } else { &MAP_LAYER_PROPS };
        for key in keys(l) {
            if !allowed.contains(&key.as_str()) {
                add(format!("{place}: unknown property {}", qs(key)));
            }
        }
        let layer_maps: Vec<Option<&Value>> = if framed {
            if !l.get("loop").and_then(Value::as_str).is_some_and(is_class_token) {
                add(format!("{place}: loop must be one lowercase class name"));
            }
            if !l.get("prefix").and_then(Value::as_str).is_some_and(is_class_token) {
                add(format!("{place}: prefix must be one lowercase class name"));
            }
            match l.get("frames") {
                Some(Value::Array(frames)) if !frames.is_empty() => frames.iter().map(Some).collect(),
                _ => {
                    add(format!("{place}: frames must be a list of at least one map"));
                    continue;
                }
            }
        } else {
            if has(l, "class") && !l.get("class").and_then(Value::as_str).is_some_and(is_class_list) {
                add(format!("{place}: class must be lowercase class names separated by single spaces"));
            }
            vec![l.get("map")]
        };
        for (fi, map) in layer_maps.into_iter().enumerate() {
            let at = if framed { format!("{place}, frame {fi}") } else { place.clone() };
            match map {
                Some(m @ Value::Array(list)) if !list.is_empty() && is_string_list(m) => maps.push((at, rows_of(m))),
                _ => add(format!("{at}: map must be a list of at least one string")),
            }
        }
    }
    if maps.is_empty() {
        return;
    }
    // Every map shares one size: the most common one.
    let width = most_common(&maps.iter().flat_map(|(_, m)| m.iter().map(|r| js_len(r))).collect::<Vec<_>>());
    let height = most_common(&maps.iter().map(|(_, m)| m.len()).collect::<Vec<_>>());
    if width == 0 {
        add("maps must be at least 1 wide".into());
    }
    for (at, map) in &maps {
        if map.len() != height {
            add(format!("{at}: {} rows, expected {height}", map.len()));
        }
        for (ri, row) in map.iter().enumerate() {
            if js_len(row) != width {
                add(format!("{at}, row {ri}: {} wide, expected {width}", js_len(row)));
            }
            check_row(row, &|k| keys_map.contains_key(k), &mut |col, msg| add(format!("{at}, row {ri}, column {col}: {msg}")));
        }
    }
}

/// Checks a block on its own: its size, faces and surface (spec D3).
fn validate_block_shape(sources: &Sources, o: &Map<String, Value>, add: &mut dyn FnMut(String)) {
    for key in keys(o) {
        if key == "footprint" {
            add("footprint is for sprites; a block's size is its footprint".into());
        } else if !BLOCK_PROPS.contains(&key.as_str()) {
            add(format!("unknown property {}", qs(key)));
        }
    }
    if has(o, "notes") && !notes_ok(o.get("notes")) {
        add("notes must be a list of strings".into());
    }
    let size = o.get("size");
    let sized = is_whole(size, 3) && {
        let s = size.unwrap();
        num_at(s, 0) >= 1.0 && num_at(s, 1) >= 1.0 && num_at(s, 2) >= 0.0
    };
    if !sized {
        add("size must be three whole numbers: tiles wide, tiles deep (both at least 1) and levels high (0 is a flat tile)".into());
    }
    match o.get("faces") {
        Some(Value::Object(faces)) => {
            for (face, color) in entries(faces) {
                if !FACES.contains(&face.as_str()) {
                    add(format!("faces: unknown face {}; use top, left, right or edge", qs(face)));
                } else if !color.as_str().is_some_and(|c| sources.colors.has(c)) {
                    add(format!("faces.{face}: {} is not in the palette", q(Some(color))));
                }
            }
            if !faces.contains_key("top") {
                add("faces.top is missing".into());
            }
            if sized {
                let flat = num_at(size.unwrap(), 2) == 0.0;
                for face in ["left", "right"] {
                    if !flat && !faces.contains_key(face) {
                        add(format!("faces.{face} is missing; a block with levels needs a left and a right face"));
                    }
                    if flat && faces.contains_key(face) {
                        add(format!("faces.{face}: a flat block (0 levels) has no {face} face"));
                    }
                }
            }
        }
        _ => add("faces must be an object of face: color name".into()),
    }
    if !has(o, "surface") {
        return;
    }
    let place = "surface";
    let Some(surface) = o.get("surface").and_then(Value::as_object) else {
        return add(format!("{place}: must be {{ keys, map }} or {{ keys, loop, prefix, frames }}"));
    };
    let looped = surface.contains_key("frames");
    for key in keys(surface) {
        let ok = key == "keys" || if looped { FRAME_LAYER_PROPS.contains(&key.as_str()) } else { key == "map" };
        if !ok {
            add(format!("{place}: unknown property {}", qs(key)));
        }
    }
    let empty = Map::new();
    let keys_map = match surface.get("keys") {
        Some(Value::Object(m)) => {
            check_keys(sources, m, "surface, ", add);
            m
        }
        _ => {
            add(format!("{place}: keys must be an object of key: color name"));
            &empty
        }
    };
    let maps: Vec<Option<&Value>> = if looped {
        if !surface.get("loop").and_then(Value::as_str).is_some_and(is_class_token) {
            add(format!("{place}: loop must be one lowercase class name"));
        }
        if !surface.get("prefix").and_then(Value::as_str).is_some_and(is_class_token) {
            add(format!("{place}: prefix must be one lowercase class name"));
        }
        match surface.get("frames") {
            Some(Value::Array(frames)) if !frames.is_empty() => frames.iter().map(Some).collect(),
            _ => return add(format!("{place}: frames must be a list of at least one map")),
        }
    } else {
        vec![surface.get("map")]
    };
    for (fi, map) in maps.into_iter().enumerate() {
        let at = if looped { format!("{place}, frame {fi}") } else { place.to_string() };
        let Some(m) = map.filter(|m| is_string_list(m)) else {
            add(format!("{at}: map must be a list of strings"));
            continue;
        };
        let rows = rows_of(m);
        if rows.len() != TILE_H as usize {
            add(format!("{at}: {} rows, expected {TILE_H}, one tile", rows.len()));
        }
        for (ri, row) in rows.iter().enumerate() {
            if js_len(row) != TILE_W as usize {
                add(format!("{at}, row {ri}: {} wide, expected {TILE_W}, one tile", js_len(row)));
            }
            check_row(row, &|k| keys_map.contains_key(k), &mut |col, msg| add(format!("{at}, row {ri}, column {col}: {msg}")));
        }
    }
}

/// Checks a block's side textures (hero island detail spec R6).
fn validate_sides(sources: &Sources, o: &Map<String, Value>, add: &mut dyn FnMut(String)) {
    let Some(sides) = o.get("sides").and_then(Value::as_object) else { return add("sides: must be { keys, left, right }".into()) };
    if is_whole(o.get("size"), 3) && num_at(&o["size"], 2) == 0.0 {
        add("sides: a flat block (0 levels) has no sides".into());
    }
    for key in keys(sides) {
        if !SIDES_PROPS.contains(&key.as_str()) {
            add(format!("sides: unknown property {}", qs(key)));
        }
    }
    let empty = Map::new();
    let keys_map = match sides.get("keys") {
        Some(Value::Object(m)) => {
            check_keys(sources, m, "sides, ", add);
            m
        }
        _ => {
            add("sides: keys must be an object of key: color name".into());
            &empty
        }
    };
    for face in ["left", "right"] {
        let at = format!("sides.{face}");
        let Some(map) = sides.get(face) else {
            add(format!("{at} is missing"));
            continue;
        };
        if !is_string_list(map) {
            add(format!("{at}: map must be a list of strings"));
            continue;
        }
        let rows = rows_of(map);
        if rows.len() != SIDE_H as usize {
            add(format!("{at}: {} rows, expected {SIDE_H}, one level", rows.len()));
        }
        for (ri, row) in rows.iter().enumerate() {
            if js_len(row) != SIDE_W as usize {
                add(format!("{at}, row {ri}: {} wide, expected {SIDE_W}, one tile's face", js_len(row)));
            }
            check_row(row, &|k| keys_map.contains_key(k), &mut |col, msg| add(format!("{at}, row {ri}, column {col}: {msg}")));
        }
    }
}

/// Checks a block against the design language (R26, R27).
fn validate_block_resolved(sources: &Sources, name: &str, problems: &mut Vec<String>) {
    let file = object_file(name);
    let mut add = |msg: String| problems.push(format!("{file}: {msg}"));
    let resolved = match resolve(sources, name) {
        Ok(r) => r,
        Err(e) => return add(e.0),
    };
    let block = resolved.block.as_ref().expect("a block resolves to a block");
    let mut named: Vec<(String, String)> = block.faces.iter().map(|(f, c)| (format!("faces.{f}"), c.clone())).collect();
    for key in used_keys(&resolved) {
        named.push((format!("surface, key {}", qs(&key)), resolved.keys.get(&key).cloned().unwrap_or_default()));
    }
    if let Some(sides) = &block.sides {
        for key in used_side_keys(block) {
            named.push((format!("sides, key {}", qs(&key)), sides.keys.get(&key).cloned().unwrap_or_default()));
        }
    }
    for (place, color) in named {
        if sources.colors.get(&color).map(|c| c.tier) == Some("outfit") {
            add(format!("{place}: {} is an outfit color; only objects that extend character may use it", qs(&color)));
        }
    }
    if resolved.width > CAPS.size || resolved.height > CAPS.size {
        add(format!("{}×{}, max {}×{}", resolved.width, resolved.height, CAPS.size, CAPS.size));
    }
    let colors = used_colors(sources, &resolved).len();
    if colors > CAPS.colors {
        add(format!("uses {colors} colors, max {}", CAPS.colors));
    }
}

/// The most common value, and the earliest one on a tie.
fn most_common(values: &[usize]) -> usize {
    let mut counts: OMap<usize, usize> = OMap::new();
    for v in values {
        let n = counts.get(v).copied().unwrap_or(0);
        counts.set(*v, n + 1);
    }
    let mut best = values[0];
    for (v, n) in counts.iter() {
        if *n > counts.get(&best).copied().unwrap_or(0) {
            best = *v;
        }
    }
    best
}

fn check_row(row: &str, has_key: &dyn Fn(&str) -> bool, report: &mut dyn FnMut(usize, String)) {
    for (x, ch) in row.chars().enumerate() {
        let key = ch.to_string();
        if ch != '.' && !has_key(&key) {
            report(x, format!("key {} is not in keys", qs(&key)));
        }
    }
}

/// Checks what needs the `extends` chain (R26, R27).
fn validate_resolved(sources: &Sources, name: &str, broken: &HashSet<String>, problems: &mut Vec<String>) {
    let file = object_file(name);
    let mut add = |msg: String| problems.push(format!("{file}: {msg}"));
    let (names, cycle) = extends_chain(sources, name);
    if cycle {
        return add(format!("extends cycle: {}", names.join(" → ")));
    }
    if names[1..].iter().any(|n| broken.contains(n)) {
        return;
    }
    let obj = &sources.objects[name];
    let resolved = match resolve(sources, name) {
        Ok(r) => r,
        Err(e) => return add(e.0),
    };
    if truthy(obj.get("extends")) {
        if let Some(rows) = obj.get("rows").and_then(Value::as_object) {
            for (li, layer_rows) in entries(rows) {
                let Some(layer_rows) = layer_rows.as_object() else { continue };
                for (ri, row) in entries(layer_rows) {
                    let row = row.as_str().unwrap_or("");
                    if js_len(row) as i64 != resolved.width {
                        add(format!("layer {li}, row {ri} (override): {} wide, expected {}", js_len(row), resolved.width));
                    }
                    check_row(row, &|k| resolved.keys.has(k), &mut |col, msg| {
                        add(format!("layer {li}, row {ri}, column {col} (override): {msg}"))
                    });
                }
            }
        }
    }
    for key in used_keys(&resolved) {
        let color = resolved.keys.get(&key).cloned().unwrap_or_default();
        if sources.colors.get(&color).map(|c| c.tier) == Some("outfit") && !resolved.character {
            add(format!("key {}: {} is an outfit color; only objects that extend character may use it", qs(&key), qs(&color)));
        }
    }
    if resolved.width > CAPS.size || resolved.height > CAPS.size {
        add(format!("{}×{}, max {}×{}", resolved.width, resolved.height, CAPS.size, CAPS.size));
    }
    let figure = painted_size(&resolved);
    if resolved.character && (figure[0] > CAPS.character_width || figure[1] > CAPS.character_height) {
        add(format!(
            "the figure is {}×{}; a character is at most {}×{}",
            figure[0], figure[1], CAPS.character_width, CAPS.character_height
        ));
    }
    let colors = used_colors(sources, &resolved).len();
    if colors > CAPS.colors {
        add(format!("uses {colors} colors, max {}", CAPS.colors));
    }
}

fn extends_chain(sources: &Sources, name: &str) -> (Vec<String>, bool) {
    let next_of = |n: &str| sources.objects.get(n).and_then(|o| o.get("extends")).and_then(Value::as_str).map(str::to_string);
    let mut names = vec![name.to_string()];
    let mut next = next_of(name);
    while let Some(n) = next.filter(|n| sources.objects.contains_key(n)) {
        names.push(n.clone());
        if names.iter().position(|x| *x == n) != Some(names.len() - 1) {
            return (names, true);
        }
        next = next_of(&n);
    }
    (names, false)
}

const DEPTHS: [&str; 2] = ["front", "back"];
const SCENE_PROPS: [&str; 7] = ["notes", "output", "viewBox", "origin", "layers", "items", "motion"];
const LAYER_PROPS: [&str; 3] = ["id", "name", "parent"];

/// A scene's layers: returns the ids of the valid ones.
fn validate_layers(layers: Option<&Value>, add: &mut dyn FnMut(String)) -> HashSet<String> {
    let mut ids = HashSet::new();
    let Some(list) = layers.and_then(Value::as_array) else {
        add("layers must be a list of { id, name, parent }".into());
        return ids;
    };
    let mut parents: OMap<String, Value> = OMap::new();
    for (i, layer) in list.iter().enumerate() {
        let place = format!("layers[{i}]");
        let Some(l) = layer.as_object() else {
            add(format!("{place}: must be {{ id, name, parent }}"));
            continue;
        };
        for key in keys(l) {
            if !LAYER_PROPS.contains(&key.as_str()) {
                add(format!("{place}: unknown property {}", qs(key)));
            }
        }
        match l.get("id").and_then(Value::as_str) {
            Some(id) if is_name(id) => {
                if ids.contains(id) {
                    add(format!("{place}: id {} is already used by another layer", qs(id)));
                } else {
                    ids.insert(id.to_string());
                    if let Some(p) = l.get("parent") {
                        parents.set(id.to_string(), p.clone());
                    }
                }
            }
            _ => add(format!("{place}: id must be lowercase kebab-case")),
        }
        if !l.get("name").and_then(Value::as_str).is_some_and(is_layer_name) {
            add(format!("{place}: name must be 1 to 64 characters on one line, without spaces at either end"));
        }
    }
    for (i, layer) in list.iter().enumerate() {
        let Some(parent) = layer.as_object().and_then(|l| l.get("parent")) else { continue };
        if !parent.as_str().is_some_and(|p| ids.contains(p)) {
            add(format!("layers[{i}]: parent {} is no layer's id", q(Some(parent))));
        }
    }
    for id in parents.keys() {
        let mut seen = HashSet::from([id.clone()]);
        let mut next = parents.get(id).cloned();
        while let Some(n) = next.as_ref().and_then(Value::as_str).filter(|n| parents.has(*n) && !seen.contains(*n)).map(str::to_string) {
            seen.insert(n.clone());
            next = parents.get(&n).cloned();
        }
        if next.as_ref().and_then(Value::as_str) == Some(id.as_str()) {
            add(format!("layers: layer {} is inside itself (a layer's parents can't come back to it)", qs(id)));
            break;
        }
    }
    ids
}

fn validate_scene(sources: &Sources, name: &str, outputs: &mut HashMap<String, String>, problems: &mut Vec<String>) {
    let file = scene_file(name);
    let mut add = |msg: String| problems.push(format!("{file}: {msg}"));
    let scene = &sources.scenes[name];
    if !is_name(name) {
        add("names are lowercase kebab-case".into());
    }
    let Some(s) = scene.as_object() else { return add("must export an object".into()) };
    for key in keys(s) {
        if !SCENE_PROPS.contains(&key.as_str()) {
            add(format!("unknown property {}", qs(key)));
        }
    }
    if has(s, "notes") && !notes_ok(s.get("notes")) {
        add("notes must be a list of strings".into());
    }
    if has(s, "output") {
        match s.get("output").and_then(Value::as_str) {
            Some(out) if is_output(out) => match outputs.get(out) {
                Some(other) => add(format!("output {} is also written by {}", qs(out), scene_file(other))),
                None => {
                    outputs.insert(out.to_string(), name.to_string());
                }
            },
            _ => add("output must be a kebab-case file name ending in .svg".into()),
        }
    }
    let vb = s.get("viewBox");
    if !is_whole(vb, 4) || num_at(vb.unwrap(), 2) <= 0.0 || num_at(vb.unwrap(), 3) <= 0.0 {
        add("viewBox must be four whole numbers: x, y, width, height (width and height above 0)".into());
    }
    if has(s, "origin") && !is_whole(s.get("origin"), 2) {
        add("origin must be two whole numbers".into());
    }
    let layers = if has(s, "layers") { validate_layers(s.get("layers"), &mut add) } else { HashSet::new() };
    let Some(items) = s.get("items").and_then(Value::as_array) else { return add("items must be a list".into()) };
    let mut ids = HashSet::new();
    validate_items(sources, items, "items", &mut add, &mut ids, &layers);
    if has(s, "motion") {
        motion::validate_motion(sources, scene, &mut add);
        validate_copy_classes(sources, name, scene, &mut add);
    }
}

/// `^(.+)-z(\d+)$`.
fn copy_of(id: &str) -> Option<(String, f64)> {
    let at = id.rfind("-z")?;
    let (base, digits) = (&id[..at], &id[at + 2..]);
    if base.is_empty() || digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((base.to_string(), jsnum::parse(digits)))
}

/// An item drawn in copies has classes `mv-<id>-z<k>`; a track for an item
/// whose id is `<id>-z<k>` would have that same class.
fn validate_copy_classes(sources: &Sources, name: &str, scene: &Value, add: &mut dyn FnMut(String)) {
    let tracks = scene.pointer("/motion/tracks").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut ids: OSet<String> = OSet::new();
    for t in &tracks {
        if let Some(id) = t.get("item").and_then(Value::as_str) {
            ids.add(id.to_string());
        }
    }
    let clashes: Vec<(String, String, f64)> =
        ids.iter().filter_map(|id| copy_of(id).filter(|(base, _)| ids.has(base)).map(|(base, k)| (id.clone(), base, k))).collect();
    if clashes.is_empty() {
        return;
    }
    // The sources have other problems when this fails, which are reported on their own.
    let Ok(copies) = motion_copies(sources, name) else { return };
    for (id, base, k) in clashes {
        let n = copies.iter().find(|(item, _)| *item == base).map_or(0, |(_, n)| *n);
        if n as f64 > k {
            add(format!(
                "motion: item {} has the class of copy {} of {} (mv-{id}); give one of them another id",
                qs(&id),
                jsnum::to_string(k),
                qs(&base)
            ));
        }
    }
}

fn validate_items(
    sources: &Sources,
    items: &[Value],
    path: &str,
    add: &mut dyn FnMut(String),
    ids: &mut HashSet<String>,
    layers: &HashSet<String>,
) {
    for (i, item) in items.iter().enumerate() {
        let place = format!("{path}[{i}]");
        let Some(it) = item.as_object() else {
            add(format!("{place}: must be {{ object, at }} or {{ group, items }}"));
            continue;
        };
        if it.contains_key("group") {
            for key in keys(it) {
                if !["group", "depth", "items"].contains(&key.as_str()) {
                    add(format!("{place}: unknown property {}", qs(key)));
                }
            }
            if has(it, "depth") && !it["depth"].as_str().is_some_and(|d| DEPTHS.contains(&d)) {
                add(format!("{place}: depth must be 'front' or 'back'"));
            }
            match it.get("group") {
                Some(Value::Object(attrs)) => {
                    for (attr, value) in entries(attrs) {
                        if attr != "class" && !is_data_attr(attr) {
                            add(format!("{place}: group attribute {} must be class or data-*", qs(attr)));
                        }
                        if !value.as_str().is_some_and(|v| !v.contains(['"', '<', '>', '&'])) {
                            add(format!("{place}: group attribute {} must be a string without \" < > &", qs(attr)));
                        }
                        if attr == "class" && value.as_str().is_some_and(|v| !is_class_list(v)) {
                            add(format!("{place}: class must be lowercase class names separated by single spaces"));
                        }
                    }
                }
                _ => add(format!("{place}: group must be an object of attributes")),
            }
            let Some(inner) = it.get("items").and_then(Value::as_array) else {
                add(format!("{place}: items must be a list"));
                continue;
            };
            validate_items(sources, inner, &format!("{place}.items"), add, ids, layers);
            continue;
        }
        for key in keys(it) {
            if !["object", "id", "class", "layer", "depth", "at"].contains(&key.as_str()) {
                add(format!("{place}: unknown property {}", qs(key)));
            }
        }
        if has(it, "depth") && !it["depth"].as_str().is_some_and(|d| DEPTHS.contains(&d)) {
            add(format!("{place}: depth must be 'front' or 'back'"));
        }
        if has(it, "layer") && !it["layer"].as_str().is_some_and(|l| layers.contains(l)) {
            add(format!("{place}: layer {} is no layer of this scene", q(it.get("layer"))));
        }
        if has(it, "id") {
            match it["id"].as_str() {
                Some(id) if is_name(id) => {
                    if ids.contains(id) {
                        add(format!("{place}: id {} is already used in this scene", qs(id)));
                    } else {
                        ids.insert(id.to_string());
                    }
                }
                _ => add(format!("{place}: id must be lowercase kebab-case")),
            }
        }
        if !it.get("object").and_then(Value::as_str).is_some_and(|o| sources.objects.contains_key(o)) {
            add(format!("{place}: object {} does not exist", q(it.get("object"))));
        }
        if has(it, "class") && !it["class"].as_str().is_some_and(is_class_list) {
            add(format!("{place}: class must be lowercase class names separated by single spaces"));
        }
        let at = it.get("at").and_then(Value::as_object);
        let shape_ok = at.is_some_and(|a| {
            let placed: Vec<&String> = keys(a).into_iter().filter(|k| *k != "level").collect();
            placed.len() == 1 && (placed[0] == "px" || placed[0] == "tile") && keys(a).iter().all(|k| *k == "level" || *k == placed[0])
        });
        if !shape_ok {
            add(format!("{place}: at must be {{ px: [x, y], level? }} or {{ tile: [col, row, level] }}"));
        } else {
            let a = at.unwrap();
            if a.contains_key("px") && !is_whole(a.get("px"), 2) {
                add(format!("{place}: at.px must be two whole numbers"));
            } else if a.contains_key("tile") && !is_whole(a.get("tile"), 3) {
                add(format!("{place}: at.tile must be three whole numbers"));
            }
        }
        if let Some(a) = at {
            if a.contains_key("level") {
                if a.contains_key("tile") {
                    add(format!("{place}: at.level is only for at.px; a tile's level is its third number"));
                } else if !is_int(&a["level"]) {
                    add(format!("{place}: at.level must be a whole number"));
                }
            }
        }
    }
}

// ------------------------------------------------------------------- resolve

/// A sprite's layer: a map (with an optional class), or a frame loop.
#[derive(Clone, Debug, PartialEq)]
pub enum SpriteLayer {
    Map { class: Option<String>, map: Vec<String> },
    Frames { loop_name: String, prefix: String, frames: Vec<Vec<String>> },
}

/// A block's side textures.
#[derive(Clone, Debug, PartialEq)]
pub struct Sides {
    pub keys: OMap<String, String>,
    pub left: Vec<String>,
    pub right: Vec<String>,
}

/// A resolved block: its size, faces, surface (as written) and sides.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub size: [i64; 3],
    pub faces: OMap<String, String>,
    pub surface: Option<Value>,
    pub sides: Option<Sides>,
}

/// An object with `extends` applied (`resolve`).
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub name: String,
    pub block: Option<Block>,
    pub character: bool,
    pub anchor: [i64; 2],
    pub footprint: [i64; 2],
    /// Key → color name, in the order the keys paint.
    pub keys: OMap<String, String>,
    pub layers: Vec<SpriteLayer>,
    pub width: i64,
    pub height: i64,
}

fn shape_error(name: &str, what: &str) -> EngineError {
    EngineError(format!("object '{name}': {what}"))
}

/// A record of strings, in JavaScript's key order.
fn string_record(v: Option<&Value>) -> OMap<String, String> {
    match v {
        Some(Value::Object(m)) => entries(m).into_iter().map(|(k, v)| (k.clone(), js_string(Some(v)))).collect(),
        _ => OMap::new(),
    }
}

fn string_rows(v: &Value) -> Option<Vec<String>> {
    v.as_array().map(|l| l.iter().map(|r| js_string(Some(r))).collect())
}

/// A layer (or a block's surface) as a `SpriteLayer`.
pub fn sprite_layer(name: &str, layer: &Value) -> Result<SpriteLayer> {
    let l = layer.as_object().ok_or_else(|| shape_error(name, "a layer is not an object"))?;
    if l.contains_key("frames") {
        let frames = l["frames"]
            .as_array()
            .ok_or_else(|| shape_error(name, "frames is not a list"))?
            .iter()
            .map(|m| string_rows(m).ok_or_else(|| shape_error(name, "a frame is not a list")))
            .collect::<Result<Vec<_>>>()?;
        Ok(SpriteLayer::Frames { loop_name: js_string(l.get("loop")), prefix: js_string(l.get("prefix")), frames })
    } else {
        let map = l.get("map").and_then(string_rows).ok_or_else(|| shape_error(name, "a layer has no map"))?;
        let class = l.get("class").filter(|c| truthy(Some(c))).map(|c| js_string(Some(c)));
        Ok(SpriteLayer::Map { class, map })
    }
}

fn own_footprint(obj: &Value) -> Option<[i64; 2]> {
    obj.get("footprint").filter(|f| f.is_array()).map(|f| [int_at(f, 0), int_at(f, 1)])
}

/// Applies `extends` (`resolve`): the base's layers with whole rows
/// replaced, then the keys merged (the base's order first, new keys after).
/// Errs on a missing object or a cycle.
pub fn resolve(sources: &Sources, name: &str) -> Result<Resolved> {
    resolve_seen(sources, name, &[])
}

fn resolve_seen(sources: &Sources, name: &str, seen: &[String]) -> Result<Resolved> {
    let obj = sources.objects.get(name).ok_or_else(|| EngineError(format!("object '{name}' does not exist")))?;
    if seen.iter().any(|s| s == name) {
        let mut chain: Vec<&str> = seen.iter().map(String::as_str).collect();
        chain.push(name);
        return Err(EngineError(format!("extends cycle: {}", chain.join(" → "))));
    }
    if obj.get("kind").and_then(Value::as_str) == Some("block") {
        return resolve_block(name, obj);
    }
    let anchor = obj.get("anchor").filter(|a| truthy(Some(a))).map(|a| [int_at(a, 0), int_at(a, 1)]);
    if !truthy(obj.get("extends")) {
        let layers = obj
            .get("layers")
            .and_then(Value::as_array)
            .ok_or_else(|| shape_error(name, "layers is not a list"))?
            .iter()
            .map(|l| sprite_layer(name, l))
            .collect::<Result<Vec<_>>>()?;
        let first = layers.first().ok_or_else(|| shape_error(name, "it has no layers"))?;
        let first = layer_maps(first).first().map(|m| (*m).clone()).unwrap_or_default();
        let width = first.first().map_or(0, |r| js_len(r)) as i64;
        return Ok(Resolved {
            name: name.to_string(),
            block: None,
            character: name == "character",
            anchor: anchor.unwrap_or([0, 0]),
            footprint: own_footprint(obj).unwrap_or([1, 1]),
            keys: string_record(obj.get("keys")),
            layers,
            width,
            height: first.len() as i64,
        });
    }
    let base_name = js_string(obj.get("extends"));
    let mut chain = seen.to_vec();
    chain.push(name.to_string());
    let base = resolve_seen(sources, &base_name, &chain)?;
    let mut layers = base.layers.clone();
    if let Some(Value::Object(rows)) = obj.get("rows") {
        for (li, layer_rows) in entries(rows) {
            let index = jsnum::parse(li);
            let layer = if jsnum::is_integer(index) && index >= 0.0 { layers.get_mut(index as usize) } else { None };
            let map = match layer {
                Some(SpriteLayer::Map { map, .. }) => map,
                _ => return Err(EngineError(format!("layer {li} (override): {base_name} has no map layer {li}"))),
            };
            if let Value::Object(layer_rows) = layer_rows {
                for (ri, row) in entries(layer_rows) {
                    let r = jsnum::parse(ri);
                    if !jsnum::is_integer(r) || r < 0.0 || r >= map.len() as f64 {
                        return Err(EngineError(format!(
                            "layer {li}, row {ri} (override): {base_name} has rows 0 to {}",
                            map.len() as i64 - 1
                        )));
                    }
                    map[r as usize] = js_string(Some(row));
                }
            }
        }
    }
    let mut keys = base.keys.clone();
    for (k, v) in string_record(obj.get("keys")).into_entries() {
        keys.set(k, v);
    }
    Ok(Resolved {
        name: name.to_string(),
        block: None,
        character: name == "character" || base.character,
        anchor: anchor.unwrap_or(base.anchor),
        footprint: own_footprint(obj).unwrap_or(base.footprint),
        keys: js_ordered(keys),
        layers,
        width: base.width,
        height: base.height,
    })
}

/// Keys in JavaScript's object order (index keys first), as spreading into
/// an object gives them.
fn js_ordered(map: OMap<String, String>) -> OMap<String, String> {
    let obj: Map<String, Value> = map.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect();
    string_record(Some(&Value::Object(obj)))
}

fn resolve_block(name: &str, obj: &Value) -> Result<Resolved> {
    let surface = obj.get("surface").filter(|s| truthy(Some(s)));
    let layers = match surface {
        Some(s) => vec![sprite_layer(name, s)?],
        None => vec![],
    };
    let size = obj.get("size").ok_or_else(|| shape_error(name, "a block has no size"))?;
    let size = [int_at(size, 0), int_at(size, 1), int_at(size, 2)];
    let [width, height] = iso::block_size(size);
    let sides = obj.get("sides").filter(|s| truthy(Some(s))).map(|s| Sides {
        keys: string_record(s.get("keys")),
        left: s.get("left").and_then(string_rows).unwrap_or_default(),
        right: s.get("right").and_then(string_rows).unwrap_or_default(),
    });
    Ok(Resolved {
        name: name.to_string(),
        block: Some(Block { size, faces: string_record(obj.get("faces")), surface: surface.cloned(), sides }),
        character: false,
        anchor: [0, 0],
        footprint: [size[0], size[1]],
        keys: string_record(surface.and_then(|s| s.get("keys"))),
        layers,
        width,
        height,
    })
}

/// A layer's maps: one, or one per frame (`layerMaps`).
pub fn layer_maps(layer: &SpriteLayer) -> Vec<&Vec<String>> {
    match layer {
        SpriteLayer::Map { map, .. } => vec![map],
        SpriteLayer::Frames { frames, .. } => frames.iter().collect(),
    }
}

/// The keys some map uses, in key order (`usedKeys`).
pub fn used_keys(resolved: &Resolved) -> Vec<String> {
    let mut used = HashSet::new();
    for layer in &resolved.layers {
        for map in layer_maps(layer) {
            for row in map {
                used.extend(row.chars().map(|c| c.to_string()));
            }
        }
    }
    resolved.keys.keys().filter(|k| used.contains(*k)).cloned().collect()
}

/// The keys a block's side maps use, in key order (`usedSideKeys`).
pub fn used_side_keys(block: &Block) -> Vec<String> {
    let Some(sides) = &block.sides else { return vec![] };
    let used: HashSet<String> = sides.left.iter().chain(&sides.right).flat_map(|r| r.chars().map(|c| c.to_string())).collect();
    sides.keys.keys().filter(|k| used.contains(*k)).cloned().collect()
}

/// The width and height of the pixels an object paints (`paintedSize`).
pub fn painted_size(resolved: &Resolved) -> [i64; 2] {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
    for layer in &resolved.layers {
        for map in layer_maps(layer) {
            for (y, row) in map.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    if ch == '.' {
                        continue;
                    }
                    min_x = min_x.min(x as i64);
                    min_y = min_y.min(y as i64);
                    max_x = max_x.max(x as i64);
                    max_y = max_y.max(y as i64);
                }
            }
        }
    }
    if min_x == i64::MAX {
        [0, 0]
    } else {
        [max_x - min_x + 1, max_y - min_y + 1]
    }
}

/// The distinct palette colors (by hex) an object paints (`usedColors`).
pub fn used_colors(sources: &Sources, resolved: &Resolved) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    if let Some(block) = &resolved.block {
        names.extend(block.faces.values().cloned());
        if let Some(sides) = &block.sides {
            names.extend(used_side_keys(block).iter().filter_map(|k| sides.keys.get(k).cloned()));
        }
    }
    names.extend(used_keys(resolved).iter().filter_map(|k| resolved.keys.get(k).cloned()));
    let set: OSet<String> = names.into_iter().map(|n| sources.colors.get(&n).map_or(n, |c| c.hex.clone())).collect();
    set.iter().cloned().collect()
}

/// The one-line summary `check` prints for an object (`describeObject`).
pub fn describe_object(sources: &Sources, resolved: &Resolved) -> String {
    let colors = used_colors(sources, resolved).len();
    let frames = resolved.layers.iter().map(|l| layer_maps(l).len()).max().unwrap_or(0).max(1);
    let plural = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
    let cap = format!("of {}", CAPS.colors);
    if let Some(block) = &resolved.block {
        let [w, d, h] = block.size;
        return [
            format!("block {w}×{d}×{h}"),
            format!("{}×{}", resolved.width, resolved.height),
            format!("{} ({cap})", plural(colors, "color")),
            plural(frames, "frame"),
        ]
        .join(" · ");
    }
    [
        format!("{}×{}", resolved.width, resolved.height),
        format!("{} ({cap})", plural(colors, "color")),
        plural(resolved.layers.len(), "layer"),
        plural(frames, "frame"),
    ]
    .join(" · ")
}

/// A pixel of a side face and the cell of its side map (`sideCells`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SideCell {
    pub p: Pixel,
    pub col: i32,
    pub row: i32,
}

/// Where each pixel of a side face falls in its side map (`sideCells`).
pub fn side_cells(face: &[Pixel]) -> Vec<SideCell> {
    let Some(x0) = face.iter().map(|p| p.0).min() else { return vec![] };
    let mut first: HashMap<i32, i32> = HashMap::new();
    for &(x, y) in face {
        let f = first.entry(x).or_insert(y);
        *f = (*f).min(y);
    }
    face.iter().map(|&(x, y)| SideCell { p: (x, y), col: (x - x0) % SIDE_W, row: (y - first[&x]) % SIDE_H }).collect()
}

// -------------------------------------------------------------------- trees

/// A color name, shared by every pixel painted with it.
pub type ColorName = Rc<str>;

/// Paint: pixels in the order first painted (later paint wins), and the
/// colors in the order first used.
#[derive(Clone, Debug, Default)]
pub struct Layer {
    pub pixels: OMap<Pixel, ColorName>,
    pub colors: OSet<ColorName>,
}

/// A `<g>`: its attributes, a frame loop's frame and count, whether it is
/// hidden at rest, and its children.
#[derive(Clone, Debug, Default)]
pub struct Group {
    pub attrs: Vec<(String, String)>,
    pub frame: Option<usize>,
    pub frames: Option<usize>,
    pub hidden: bool,
    pub children: Vec<Child>,
    /// A group the engine made to play motion (a moving item, a pose, a
    /// copy): a compiled SVG scopes its class to the scene (spec R15).
    pub generated: bool,
}

/// A group's child: paint, or a group.
#[derive(Clone, Debug)]
pub enum Child {
    Layer(Layer),
    Group(Group),
}

/// A rendered scene: its viewBox, its tree, and its motion's CSS.
#[derive(Clone, Debug, Default)]
pub struct Rendered {
    pub view_box: Vec<Value>,
    pub root: Group,
    pub style: Option<String>,
    /// The scene a compiled SVG is scoped to (`data-pixellab`, spec D7), or
    /// None for the unscoped output (`render_scene`).
    pub scope: Option<String>,
}

/// The order a scene's items paint in: depth order, or as the file lists
/// them (what `scene.sort` compares against).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    Depth,
    List,
}

fn class_group(class: String) -> Group {
    Group { attrs: vec![("class".into(), class)], ..Group::default() }
}

/// The layer unclassed paint goes into: the group's last child if it's a
/// layer, so consecutive paint shares one layer and later paint wins.
fn open_layer(group: &mut Group) -> &mut Layer {
    if !matches!(group.children.last(), Some(Child::Layer(_))) {
        group.children.push(Child::Layer(Layer::default()));
    }
    match group.children.last_mut() {
        Some(Child::Layer(l)) => l,
        _ => unreachable!("a layer was just pushed"),
    }
}

/// Interns color names, so every pixel of one color shares its name.
#[derive(Default)]
struct Names(HashMap<String, ColorName>);

impl Names {
    fn get(&mut self, name: &str) -> ColorName {
        if let Some(n) = self.0.get(name) {
            return n.clone();
        }
        let n: ColorName = Rc::from(name);
        self.0.insert(name.to_string(), n.clone());
        n
    }
}

/// Paints a map key by key, in key order.
fn paint(
    layer: &mut Layer,
    map: &[String],
    keys: &OMap<String, String>,
    origin: (i32, i32),
    clip: Option<&HashSet<Pixel>>,
    names: &mut Names,
) {
    let (ox, oy) = origin;
    let mut by_key: HashMap<char, Vec<Pixel>> = HashMap::new();
    for (y, row) in map.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let p = (ox + x as i32, oy + y as i32);
            if ch == '.' || clip.is_some_and(|c| !c.contains(&p)) {
                continue;
            }
            by_key.entry(ch).or_default().push(p);
        }
    }
    lay_by_key(layer, &by_key, keys, names);
}

/// Lays pixels grouped by key onto a layer, in key order.
fn lay_by_key(layer: &mut Layer, by_key: &HashMap<char, Vec<Pixel>>, keys: &OMap<String, String>, names: &mut Names) {
    for (key, color) in keys.iter() {
        let mut chars = key.chars();
        let (Some(ch), None) = (chars.next(), chars.next()) else { continue };
        let Some(pixels) = by_key.get(&ch) else { continue };
        let name = names.get(color);
        layer.colors.add(name.clone());
        for p in pixels {
            layer.pixels.set(*p, name.clone());
        }
    }
}

fn frame_group(loop_name: &str, prefix: &str, i: usize, count: usize, layer: Layer) -> Child {
    Child::Group(Group {
        attrs: vec![("class".into(), format!("{loop_name} {prefix}{i}"))],
        frame: Some(i),
        frames: Some(count),
        hidden: false,
        children: vec![Child::Layer(layer)],
        generated: false,
    })
}

/// Draws a block with its first tile's top-face center at (cx, cy).
fn draw_block(block: &Block, keys: &OMap<String, String>, cx: i32, cy: i32, group: &mut Group, names: &mut Names) -> Result<()> {
    let shapes = iso::block_faces(block.size, [cx, cy]);
    {
        let base = open_layer(group);
        for (face, pixels) in [("top", &shapes.top), ("left", &shapes.left), ("right", &shapes.right)] {
            let Some(color) = block.faces.get(face).filter(|c| !c.is_empty()) else { continue };
            let name = names.get(color);
            base.colors.add(name.clone());
            for p in pixels {
                base.pixels.set(*p, name.clone());
            }
        }
        if let Some(sides) = &block.sides {
            let mut by_key: HashMap<char, Vec<Pixel>> = HashMap::new();
            for (face, rows) in [(&shapes.left, &sides.left), (&shapes.right, &sides.right)] {
                for SideCell { p, col, row } in side_cells(face) {
                    let ch = rows.get(row as usize).and_then(|r| r.chars().nth(col as usize));
                    let Some(ch) = ch else { return Err(EngineError(format!("sides: no cell at row {row}, column {col}"))) };
                    if ch != '.' {
                        by_key.entry(ch).or_default().push(p);
                    }
                }
            }
            lay_by_key(base, &by_key, &sides.keys, names);
        }
    }
    if let Some(surface) = &block.surface {
        let clips: Vec<(i32, i32, HashSet<Pixel>)> = iso::footprint([block.size[0], block.size[1]], [cx, cy])
            .into_iter()
            .map(|[tx, ty]| (tx, ty, iso::tile_pixels(tx, ty).into_iter().collect()))
            .collect();
        let lay = |layer: &mut Layer, map: &[String], names: &mut Names| {
            // Each tile's map is clipped to that tile's own diamond.
            for (tx, ty, clip) in &clips {
                paint(layer, map, keys, (tx - TILE_W / 2, ty - TILE_H / 2), Some(clip), names);
            }
        };
        match sprite_layer("surface", surface)? {
            SpriteLayer::Frames { loop_name, prefix, frames } => {
                for (i, map) in frames.iter().enumerate() {
                    let mut layer = Layer::default();
                    lay(&mut layer, map, names);
                    group.children.push(frame_group(&loop_name, &prefix, i, frames.len(), layer));
                }
            }
            SpriteLayer::Map { map, .. } => lay(open_layer(group), &map, names),
        }
    }
    if let Some(edge) = block.faces.get("edge").filter(|c| !c.is_empty()) {
        let name = names.get(edge);
        let layer = open_layer(group);
        layer.colors.add(name.clone());
        for p in &shapes.edge {
            layer.pixels.set(*p, name.clone());
        }
    }
    Ok(())
}

/// Draws a resolved object with its map's top-left pixel at (ox, oy).
fn draw(obj: &Resolved, ox: i32, oy: i32, group: &mut Group, names: &mut Names) -> Result<()> {
    if let Some(block) = &obj.block {
        return draw_block(block, &obj.keys, ox, oy, group, names);
    }
    for layer in &obj.layers {
        match layer {
            SpriteLayer::Frames { loop_name, prefix, frames } => {
                for (i, map) in frames.iter().enumerate() {
                    let mut out = Layer::default();
                    paint(&mut out, map, &obj.keys, (ox, oy), None, names);
                    group.children.push(frame_group(loop_name, prefix, i, frames.len(), out));
                }
            }
            SpriteLayer::Map { class: Some(class), map } => {
                let mut out = Layer::default();
                paint(&mut out, map, &obj.keys, (ox, oy), None, names);
                let mut g = class_group(class.clone());
                g.children.push(Child::Layer(out));
                group.children.push(Child::Group(g));
            }
            SpriteLayer::Map { class: None, map } => paint(open_layer(group), map, &obj.keys, (ox, oy), None, names),
        }
    }
    Ok(())
}

// ------------------------------------------------------------------- tracks

/// A key for a track's item or an item's id, as a JavaScript `Map` tells
/// them apart (primitives by value; anything else never matches).
fn track_key(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(format!("#n:{}", jsnum::number_string(n))),
        Value::Bool(b) => Some(format!("#b:{b}")),
        Value::Null => Some("#null".into()),
        _ => None,
    }
}

/// A scene's motion tracks by item id (`tracksOf`).
pub fn tracks_of(scene: &Value) -> OMap<String, Value> {
    let mut out = OMap::new();
    if let Some(Value::Array(tracks)) = scene.pointer("/motion/tracks") {
        for track in tracks {
            if let Some(k) = track.get("item").and_then(track_key) {
                out.set(k, track.clone());
            }
        }
    }
    out
}

/// The track of an item (by its id), if it has one.
fn track_for<'a>(item: &Value, tracks: &'a OMap<String, Value>) -> Option<&'a Value> {
    item.get("id").and_then(track_key).and_then(|k| tracks.get(&k))
}

/// A scene's origin, [0, 0] when it names none.
pub fn origin_of(scene: &Value) -> [f64; 2] {
    match scene.get("origin") {
        Some(o) if !o.is_null() => [num_at(o, 0), num_at(o, 1)],
        _ => [0.0, 0.0],
    }
}

fn duration_of(scene: &Value) -> f64 {
    scene.pointer("/motion/duration").and_then(Value::as_f64).unwrap_or(f64::NAN)
}

/// An item's placement point in pixels, and the level it stands on.
pub fn placement(item: &Value, origin: [f64; 2]) -> Result<([f64; 2], f64)> {
    let at = item.get("at").ok_or_else(|| EngineError("an item has no at".into()))?;
    if let Some(tile) = at.get("tile").filter(|t| truthy(Some(t))) {
        let t = [num_at(tile, 0), num_at(tile, 1), num_at(tile, 2)];
        return Ok((iso::tile_to_px(t, origin), t[2]));
    }
    let px = at.get("px").ok_or_else(|| EngineError("an item's at has no px or tile".into()))?;
    let level = at.get("level").and_then(Value::as_f64).unwrap_or(0.0);
    Ok(([num_at(px, 0), num_at(px, 1)], level))
}

fn object_of(item: &Value) -> String {
    js_string(item.get("object"))
}

// --------------------------------------------------------------------- depth

/// Where an object's pixels fall with its anchor at (0, 0), and the sizes
/// its box comes from (`extentOf`), cached per set of sources.
fn extent_of(sources: &Sources, name: &str) -> Result<Extent> {
    if let Some(found) = sources.extents.borrow().get(name) {
        return Ok(found.clone());
    }
    let obj = resolve(sources, name)?;
    let found = Extent {
        bounds: tree_bounds(&render_object(sources, name)?),
        footprint: obj.footprint,
        height: depth::sprite_height(obj.anchor[1] as f64),
        block: obj.block.as_ref().map(|b| b.size),
    };
    sources.extents.borrow_mut().insert(name.to_string(), found.clone());
    Ok(found)
}

/// The bounds of every pixel in a tree, in every group.
fn tree_bounds(group: &Group) -> Option<Bounds> {
    let mut out = None;
    for child in &group.children {
        let b = match child {
            Child::Layer(l) => bounds(l.pixels.keys()),
            Child::Group(g) => tree_bounds(g),
        };
        out = depth::union_bounds(&[out, b]);
    }
    out
}

/// A placed object's node for the sort, as a function of where it stands
/// (`nodeMaker`): a move in pixels from its placement gives its box and the
/// pixels it can draw on.
struct NodeMaker {
    point: [f64; 2],
    level: f64,
    tile: Option<[f64; 2]>,
    tracked: bool,
    rest: [f64; 2],
    origin: [f64; 2],
    extent: Extent,
    drawn: Option<Bounds>,
}

impl NodeMaker {
    fn new(sources: &Sources, item: &Value, origin: [f64; 2], track: Option<&Value>) -> Result<NodeMaker> {
        let (point, level) = placement(item, origin)?;
        let tile = item.pointer("/at/tile").filter(|t| truthy(Some(t))).map(|t| [num_at(t, 0), num_at(t, 1)]);
        let object = object_of(item);
        let (mut standing, mut poses, mut rest) = (object.clone(), vec![object.clone()], [0.0, 0.0]);
        if let Some(track) = track {
            let first = motion::filled_keys(track, &object).into_iter().next().ok_or_else(|| EngineError("a track has no keys".into()))?;
            rest = first.moved;
            standing = first.pose;
            poses = motion::poses_of(track, &object);
        }
        let extent = extent_of(sources, &standing)?;
        let drawn = depth::union_bounds(&poses.iter().map(|p| extent_of(sources, p).map(|e| e.bounds)).collect::<Result<Vec<_>>>()?);
        Ok(NodeMaker { point, level, tile, tracked: track.is_some(), rest, origin, extent, drawn })
    }

    fn at_rest(&self) -> (Option<Box3>, Option<Bounds>) {
        self.make(self.rest)
    }

    fn make(&self, moved: [f64; 2]) -> (Option<Box3>, Option<Bounds>) {
        let (px, py) = (self.point[0] + moved[0], self.point[1] + moved[1]);
        let [c, r] = match self.tile {
            Some(t) if !self.tracked => t,
            _ => iso::px_to_tile_exact([px, py], self.level, self.origin),
        };
        let boxed = match self.extent.block {
            Some(b) => depth::block_box(c, r, self.level, [b[0] as f64, b[1] as f64, b[2] as f64]),
            None => {
                depth::sprite_box(c, r, self.level, [self.extent.footprint[0] as f64, self.extent.footprint[1] as f64], self.extent.height)
            }
        };
        let bounds = self.drawn.map(|d| Bounds {
            min_x: d.min_x + px as i32,
            min_y: d.min_y + py as i32,
            max_x: d.max_x + px as i32,
            max_y: d.max_y + py as i32,
        });
        (Some(boxed), bounds)
    }
}

/// A group that is one variant of a run (the site shows one at a time).
fn is_variant_group(item: &Value) -> bool {
    matches!(item.get("group"), Some(Value::Object(g)) if g.contains_key("data-class"))
}

fn is_group(item: &Value) -> bool {
    item.as_object().is_some_and(|o| o.contains_key("group"))
}

/// A list's entry for the sort (`Part`).
#[derive(Clone, Debug)]
struct Part {
    index: usize,
    item: Value,
    boxed: Option<Box3>,
    bounds: Option<Bounds>,
    depth: Option<Mark>,
    inner: Option<Box<Arranged>>,
}

/// One list in depth order (`Arranged`): its parts in the new order, the
/// order as old indices, whether it and every list inside it were already in
/// order, and the box and bounds of the whole list.
#[derive(Clone, Debug)]
struct Arranged {
    order: Vec<usize>,
    sorted: bool,
    boxed: Option<Box3>,
    bounds: Option<Bounds>,
    parts: Vec<Part>,
}

impl Arranged {
    fn items(&self) -> Vec<Value> {
        self.parts.iter().map(|p| p.item.clone()).collect()
    }
}

fn mark_of(item: &Value) -> Option<Mark> {
    item.get("depth").and_then(Value::as_str).and_then(Mark::parse)
}

/// Consecutive variant groups make a run; every other part is a run of one.
fn runs_of(list: &[&Part]) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = Vec::new();
    for (i, part) in list.iter().enumerate() {
        if is_variant_group(&part.item) && i > 0 && is_variant_group(&list[i - 1].item) {
            out.last_mut().expect("a run before").push(i);
        } else {
            out.push(vec![i]);
        }
    }
    out
}

fn unit_node(parts: &[&Part], unit: &[usize], moving: bool) -> Node {
    Node {
        boxed: depth::union_box(&unit.iter().map(|&i| parts[i].boxed).collect::<Vec<_>>()),
        bounds: depth::union_bounds(&unit.iter().map(|&i| parts[i].bounds).collect::<Vec<_>>()),
        depth: parts[unit[0]].depth,
        moving,
    }
}

/// Puts one list in depth order (`arrange`).
fn arrange(sources: &Sources, items: &[Value], origin: [f64; 2], tracks: &OMap<String, Value>) -> Result<Arranged> {
    let mut inside = true;
    let mut parts: Vec<Part> = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        if is_group(item) {
            let inner_items = item.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
            let inner = arrange(sources, &inner_items, origin, tracks)?;
            inside &= inner.sorted;
            let mut value = item.clone();
            if !inner.sorted {
                value["items"] = Value::Array(inner.items());
            }
            parts.push(Part {
                index,
                item: value,
                boxed: inner.boxed,
                bounds: inner.bounds,
                depth: mark_of(item),
                inner: Some(Box::new(inner)),
            });
        } else {
            let (boxed, bounds) = NodeMaker::new(sources, item, origin, track_for(item, tracks))?.at_rest();
            parts.push(Part { index, item: item.clone(), boxed, bounds, depth: mark_of(item), inner: None });
        }
    }
    // Sorting can bring two variant groups side by side, which then make a
    // run of their own; sort again until the runs stop merging.
    let mut list: Vec<usize> = (0..parts.len()).collect();
    loop {
        let refs: Vec<&Part> = list.iter().map(|&i| &parts[i]).collect();
        let units = runs_of(&refs);
        let nodes: Vec<Node> = units.iter().map(|u| unit_node(&refs, u, false)).collect();
        let order = depth::sort_nodes(&nodes);
        let next: Vec<usize> = order.iter().flat_map(|&u| units[u].iter().map(|&k| list[k])).collect();
        let merged = runs_of(&next.iter().map(|&i| &parts[i]).collect::<Vec<_>>()).len();
        list = next;
        if merged == units.len() {
            break;
        }
    }
    let boxed = depth::union_box(&parts.iter().map(|p| p.boxed).collect::<Vec<_>>());
    let bounds = depth::union_bounds(&parts.iter().map(|p| p.bounds).collect::<Vec<_>>());
    let order: Vec<usize> = list.iter().map(|&i| parts[i].index).collect();
    let sorted = inside && order.iter().enumerate().all(|(i, &index)| index == i);
    let mut slots: Vec<Option<Part>> = parts.into_iter().map(Some).collect();
    let parts = list.iter().map(|&i| slots[i].take().expect("each part once")).collect();
    Ok(Arranged { order, sorted, boxed, bounds, parts })
}

// --------------------------------------------------------------- while moving

/// What a scene draws, once arranged: an item, a group with its entries, or
/// one copy of an item drawn in several places (`{ copy, item }`).
#[derive(Clone, Debug)]
enum Entry {
    Item(Value),
    Group(Value, Vec<Entry>),
    Copy(String, Box<Entry>),
}

fn entries_of(items: &[Value]) -> Vec<Entry> {
    items
        .iter()
        .map(|item| {
            if is_group(item) {
                let inner = item.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
                Entry::Group(item.clone(), entries_of(&inner))
            } else {
                Entry::Item(item.clone())
            }
        })
        .collect()
}

/// Each run that is a lone item with a track that moves it, as that track
/// (`moversOf`).
fn movers_of<'a>(parts: &[&Part], units: &[Vec<usize>], tracks: &'a OMap<String, Value>) -> Vec<Option<&'a Value>> {
    units
        .iter()
        .map(|unit| {
            let part = parts[unit[0]];
            if unit.len() != 1 || part.inner.is_some() {
                return None;
            }
            track_for(&part.item, tracks).filter(|t| motion::is_moving(t, &object_of(&part.item)))
        })
        .collect()
}

/// A moving unit's node maker, its runs over the loop, and the node it has
/// in each run (made once, so `moving_order` keeps its relations).
struct Mover {
    maker: NodeMaker,
    runs: Vec<motion::Run>,
    made: HashMap<usize, Rc<Node>>,
}

/// The list a scene draws, from an arranged one (`withCopies`): a moving
/// item that takes several places in the order over the loop is drawn once
/// for each, and the items with copies are added to `copies` by id.
fn with_copies(
    sources: &Sources,
    arranged: &Arranged,
    origin: [f64; 2],
    tracks: &OMap<String, Value>,
    duration: f64,
    copies: &mut OMap<String, Vec<Copy>>,
) -> Result<Vec<Entry>> {
    let entry = |part: &Part, copies: &mut OMap<String, Vec<Copy>>| -> Result<Entry> {
        match &part.inner {
            Some(inner) => Ok(Entry::Group(part.item.clone(), with_copies(sources, inner, origin, tracks, duration, copies)?)),
            None => Ok(Entry::Item(part.item.clone())),
        }
    };
    let parts: Vec<&Part> = arranged.parts.iter().collect();
    let units = runs_of(&parts);
    let movers = movers_of(&parts, &units, tracks);
    let moving: Vec<usize> = (0..units.len()).filter(|&i| movers[i].is_some()).collect();
    if moving.is_empty() {
        let mut out = Vec::new();
        for unit in &units {
            for &k in unit {
                out.push(entry(parts[k], copies)?);
            }
        }
        return Ok(out);
    }
    let nodes: Vec<Node> = units.iter().enumerate().map(|(i, u)| unit_node(&parts, u, movers[i].is_some())).collect();
    let mut order_at = depth::moving_order(&nodes);
    let mut state: HashMap<usize, Mover> = HashMap::new();
    for &i in &moving {
        let item = &parts[units[i][0]].item;
        let track = movers[i].expect("a moving unit has a track");
        let maker = NodeMaker::new(sources, item, origin, Some(track))?;
        let runs = motion::move_runs(track, &object_of(item), duration);
        state.insert(i, Mover { maker, runs, made: HashMap::new() });
    }
    // The cuts: wherever any moving item takes a step (those within rounding of each other count as one).
    let mut ats: Vec<f64> = moving.iter().flat_map(|i| state[i].runs.iter().map(|r| r.at)).collect();
    ats.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut cuts = vec![0.0];
    for at in ats {
        if at - cuts[cuts.len() - 1] > 1e-9 {
            cuts.push(at);
        }
    }
    cuts.push(100.0);
    let spans = cuts.clone();
    let mut places_at = |span: usize| -> Vec<(usize, depth::Place)> {
        let middle = (spans[span] + spans[span + 1]) / 2.0;
        let current: Vec<(usize, Rc<Node>)> = moving
            .iter()
            .map(|&i| {
                let mover = state.get_mut(&i).expect("a mover per moving unit");
                let run = run_at(&mover.runs, middle);
                let moved = mover.runs[run].moved;
                let maker = &mover.maker;
                let node = mover
                    .made
                    .entry(run)
                    .or_insert_with(|| {
                        let (boxed, bounds) = maker.make(moved);
                        Rc::new(Node { boxed, bounds, depth: nodes[i].depth, moving: true })
                    })
                    .clone();
                (i, node)
            })
            .collect();
        depth::places_in(&order_at.order(&current), &nodes)
    };
    let windows = depth::place_windows(&cuts, &mut places_at);
    let mut gaps: OMap<i64, Vec<(usize, usize, usize, Option<String>)>> = OMap::new();
    for &i in &moving {
        let own = windows.iter().find(|(n, _)| *n == i).map(|(_, c)| c.clone()).unwrap_or_default();
        let id = js_string(parts[units[i][0]].item.get("id"));
        if own.len() > 1 {
            copies.set(id.clone(), own.clone());
        }
        for (k, place) in own.iter().enumerate() {
            let copy = if own.len() > 1 { Some(motion::copy_class(&id, k)) } else { None };
            gaps.get_or_insert_with(place.follows, Vec::new).push((place.rank, i, k, copy));
        }
    }
    let mut out = Vec::new();
    let gap = |follows: i64, out: &mut Vec<Entry>, copies: &mut OMap<String, Vec<Copy>>| -> Result<()> {
        let mut list = gaps.get(&follows).cloned().unwrap_or_default();
        list.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        for (_, unit, _, copy) in list {
            let e = entry(parts[units[unit][0]], copies)?;
            out.push(match copy {
                None => e,
                Some(c) => Entry::Copy(c, Box::new(e)),
            });
        }
        Ok(())
    };
    gap(-1, &mut out, copies)?;
    for (i, unit) in units.iter().enumerate() {
        if movers[i].is_some() {
            continue;
        }
        for &k in unit {
            out.push(entry(parts[k], copies)?);
        }
        gap(i as i64, &mut out, copies)?;
    }
    Ok(out)
}

/// The run a list gives at percentage `p` (`moveAtPercent`): the last one
/// begun by then.
fn run_at(runs: &[motion::Run], p: f64) -> usize {
    let mut found = 0;
    for (i, run) in runs.iter().enumerate() {
        if run.at > p {
            break;
        }
        found = i;
    }
    found
}

fn scene_or_err<'a>(sources: &'a Sources, name: &str) -> Result<&'a Value> {
    sources.scenes.get(name).ok_or_else(|| EngineError(format!("scene '{name}' does not exist")))
}

fn items_of(scene: &Value) -> Vec<Value> {
    scene.get("items").and_then(Value::as_array).cloned().unwrap_or_default()
}

/// Renders a scene to a tree that mirrors its groups, placement classes and
/// frame loops (`renderScene`). Every list of items paints in depth order,
/// back to front; a scene with motion also returns the CSS that plays it.
pub fn render_scene(sources: &Sources, name: &str, order: Order) -> Result<Rendered> {
    render_scene_with(sources, name, order, None)
}

/// `render_scene`, scoped to `scope` for a compiled SVG (spec D7): the
/// style plays the motion and the frame loops under `[data-pixellab]`, with
/// generated names `<scope>--…`, and a scene with nothing to play has none.
pub fn render_scene_with(sources: &Sources, name: &str, order: Order, scope: Option<&str>) -> Result<Rendered> {
    let scene = scene_or_err(sources, name)?;
    let mut root = Group::default();
    let tracks = tracks_of(scene);
    let origin = origin_of(scene);
    let mut copies: OMap<String, Vec<Copy>> = OMap::new();
    let items = items_of(scene);
    let list = if order == Order::List {
        entries_of(&items)
    } else {
        let arranged = arrange(sources, &items, origin, &tracks)?;
        if tracks.is_empty() {
            entries_of(&arranged.items())
        } else {
            with_copies(sources, &arranged, origin, &tracks, duration_of(scene), &mut copies)?
        }
    };
    let mut names = Names::default();
    place_items(sources, &list, &mut root, origin, &tracks, &mut names)?;
    let view_box = scene.get("viewBox").and_then(Value::as_array).cloned().unwrap_or_default();
    let ids = motion::items_by_id(scene.get("items").unwrap_or(&Value::Null));
    let motion = scene.get("motion").unwrap_or(&Value::Null);
    if let Some(scope) = scope {
        let mut loops = Vec::new();
        frame_loops(&root, &mut loops);
        let style = motion::scene_css(scope, if tracks.is_empty() { None } else { Some((motion, &ids, &copies)) }, &loops);
        return Ok(Rendered { view_box, root, style: Some(style).filter(|s| !s.is_empty()), scope: Some(scope.to_string()) });
    }
    if tracks.is_empty() {
        return Ok(Rendered { view_box, root, style: None, scope: None });
    }
    let style = motion::motion_css(motion, &ids, &copies);
    Ok(Rendered { view_box, root, style: Some(style), scope: None })
}

/// The frame loops a tree plays, in the order they first appear: (loop, frame class prefix, frame count).
fn frame_loops(group: &Group, out: &mut Vec<motion::FrameLoop>) {
    for child in &group.children {
        if let Child::Group(g) = child {
            if let (Some(frame), Some(count)) = (g.frame, g.frames) {
                let class = g.attrs.iter().find(|(k, _)| k == "class").map(|(_, v)| v.as_str()).unwrap_or("");
                let mut words = class.split(' ');
                let name = words.next().unwrap_or("").to_string();
                let word = words.next().unwrap_or("");
                let prefix = word.strip_suffix(frame.to_string().as_str()).unwrap_or(word).to_string();
                match out.iter_mut().find(|l| l.name == name) {
                    Some(l) => l.frames = l.frames.max(count),
                    None => out.push(motion::FrameLoop { name, prefix, frames: count }),
                }
            }
            frame_loops(g, out);
        }
    }
}

/// Every item that moves among others into several places of the depth
/// order, with how many copies the SVG draws of it (`motionCopies`).
pub fn motion_copies(sources: &Sources, name: &str) -> Result<Vec<(String, usize)>> {
    let scene = scene_or_err(sources, name)?;
    let tracks = tracks_of(scene);
    if tracks.is_empty() {
        return Ok(vec![]);
    }
    let origin = origin_of(scene);
    let mut copies = OMap::new();
    let arranged = arrange(sources, &items_of(scene), origin, &tracks)?;
    with_copies(sources, &arranged, origin, &tracks, duration_of(scene), &mut copies)?;
    Ok(copies.iter().map(|(item, own)| (item.clone(), own.len())).collect())
}

/// What `scene_at`'s lists need, at one time.
struct AtTime<'a> {
    sources: &'a Sources,
    tracks: &'a OMap<String, Value>,
    origin: [f64; 2],
    motion: Option<&'a Value>,
    duration: f64,
    time: f64,
}

impl AtTime<'_> {
    fn point_of(&self, item: &Value) -> [f64; 2] {
        placement(item, self.origin).map_or([f64::NAN, f64::NAN], |(p, _)| p)
    }

    fn list(&self, arranged: &Arranged, prefix: &[usize]) -> Result<Vec<Value>> {
        let parts: Vec<&Part> = arranged.parts.iter().collect();
        let units = runs_of(&parts);
        let movers = movers_of(&parts, &units, self.tracks);
        let mut order: Vec<usize> = (0..units.len()).collect();
        if movers.iter().any(Option::is_some) {
            let nodes: Vec<Node> = units.iter().enumerate().map(|(i, u)| unit_node(&parts, u, movers[i].is_some())).collect();
            let mut current = Vec::new();
            for (i, unit) in units.iter().enumerate() {
                let Some(track) = movers[i] else { continue };
                let item = &parts[unit[0]].item;
                let state = motion::state_at(track, &object_of(item), self.duration, self.time);
                let (boxed, bounds) = NodeMaker::new(self.sources, item, self.origin, Some(track))?.make(state.moved);
                current.push((i, Rc::new(Node { boxed, bounds, depth: nodes[i].depth, moving: true })));
            }
            order = depth::moving_order(&nodes).order(&current);
        }
        let point_of = |item: &Value| self.point_of(item);
        let mut out = Vec::new();
        for i in order {
            for &k in &units[i] {
                let part = parts[k];
                let mut path: Vec<usize> = prefix.to_vec();
                path.push(part.index);
                let mut value = match &part.inner {
                    Some(inner) => {
                        let mut v = part.item.clone();
                        v["items"] = Value::Array(self.list(inner, &path)?);
                        v
                    }
                    None => motion::pose_items(std::slice::from_ref(&part.item), self.motion, self.time, &point_of).remove(0),
                };
                if let Value::Object(m) = &mut value {
                    m.insert("path".into(), Value::Array(path.iter().map(|&n| Value::from(n)).collect()));
                }
                out.push(value);
            }
        }
        Ok(out)
    }
}

/// A scene as it stands at `time` ms, still (`sceneAt`): every list in the
/// order it draws in then, each tracked item posed, no motion, and each
/// item and group with its `path` in the scene's own items.
pub fn scene_at(sources: &Sources, name: &str, time: f64) -> Result<Value> {
    let scene = scene_or_err(sources, name)?;
    let tracks = tracks_of(scene);
    let at = AtTime { sources, tracks: &tracks, origin: origin_of(scene), motion: scene.get("motion"), duration: duration_of(scene), time };
    let arranged = arrange(sources, &items_of(scene), at.origin, &tracks)?;
    let items = at.list(&arranged, &[])?;
    let mut still = scene.as_object().cloned().unwrap_or_default();
    still.shift_remove("motion");
    still.insert("items".into(), Value::Array(items));
    Ok(Value::Object(still))
}

/// The order one list of items draws in, as indices into it (`depthOrder`):
/// its groups count as one object each.
pub fn depth_order(sources: &Sources, items: &[Value], origin: [f64; 2], tracks: &OMap<String, Value>) -> Result<Vec<usize>> {
    Ok(arrange(sources, items, origin, tracks)?.order)
}

/// A copy of the scene with every list of items in depth order
/// (`sceneInOrder`).
pub fn scene_in_order(sources: &Sources, scene: &Value) -> Result<Value> {
    let mut copy = scene.clone();
    let arranged = arrange(sources, &items_of(scene), origin_of(scene), &tracks_of(scene))?;
    copy["items"] = Value::Array(arranged.items());
    Ok(copy)
}

/// Whether every list of the scene's items is in depth order already
/// (`isInDepthOrder`).
pub fn is_in_depth_order(sources: &Sources, scene: &Value) -> Result<bool> {
    Ok(arrange(sources, &items_of(scene), origin_of(scene), &tracks_of(scene))?.sorted)
}

fn place_items(
    sources: &Sources,
    list: &[Entry],
    group: &mut Group,
    origin: [f64; 2],
    tracks: &OMap<String, Value>,
    names: &mut Names,
) -> Result<()> {
    for e in list {
        match e {
            Entry::Copy(class, inner) => {
                let mut copy = Group { generated: true, ..class_group(class.clone()) };
                place_items(sources, std::slice::from_ref(inner), &mut copy, origin, tracks, names)?;
                group.children.push(Child::Group(copy));
            }
            Entry::Group(item, inner) => {
                let attrs = match item.get("group") {
                    Some(Value::Object(m)) => entries(m).into_iter().map(|(k, v)| (k.clone(), js_string(Some(v)))).collect(),
                    _ => vec![],
                };
                let mut child = Group { attrs, ..Group::default() };
                place_items(sources, inner, &mut child, origin, tracks, names)?;
                group.children.push(Child::Group(child));
            }
            Entry::Item(item) => place_item(sources, item, group, origin, tracks, names)?,
        }
    }
    Ok(())
}

fn place_item(
    sources: &Sources,
    item: &Value,
    group: &mut Group,
    origin: [f64; 2],
    tracks: &OMap<String, Value>,
    names: &mut Names,
) -> Result<()> {
    let ([mut x, mut y], _) = placement(item, origin)?;
    let object = object_of(item);
    let class = item.get("class").filter(|c| truthy(Some(c))).map(|c| js_string(Some(c)));
    let draw_into = |name: &str, parent: &mut Group, x: f64, y: f64, names: &mut Names| -> Result<()> {
        let obj = resolve(sources, name)?;
        let (ox, oy) = ((x - obj.anchor[0] as f64) as i32, (y - obj.anchor[1] as f64) as i32);
        match &class {
            Some(c) => {
                let mut target = class_group(c.clone());
                draw(&obj, ox, oy, &mut target, names)?;
                parent.children.push(Child::Group(target));
            }
            None => draw(&obj, ox, oy, parent, names)?,
        }
        Ok(())
    };
    let Some(track) = track_for(item, tracks) else { return draw_into(&object, group, x, y, names) };
    let id = js_string(item.get("id"));
    let rest = motion::filled_keys(track, &object).into_iter().next().ok_or_else(|| EngineError("a track has no keys".into()))?;
    x += rest.moved[0];
    y += rest.moved[1];
    let mut moving =
        Group { attrs: vec![("class".into(), motion::motion_class(&id))], hidden: !rest.show, generated: true, ..Group::default() };
    let poses = motion::poses_of(track, &object);
    if poses.len() == 1 {
        draw_into(&object, &mut moving, x, y, names)?;
    } else {
        for (k, pose) in poses.iter().enumerate() {
            let mut posed = Group {
                attrs: vec![("class".into(), motion::pose_class(&id, k))],
                hidden: *pose != rest.pose,
                generated: true,
                ..Group::default()
            };
            draw_into(pose, &mut posed, x, y, names)?;
            moving.children.push(Child::Group(posed));
        }
    }
    group.children.push(Child::Group(moving));
    Ok(())
}

/// Renders one object on its own, with its anchor at (0, 0)
/// (`renderObject`).
pub fn render_object(sources: &Sources, name: &str) -> Result<Group> {
    let obj = resolve(sources, name)?;
    let mut root = Group::default();
    let mut names = Names::default();
    draw(&obj, -obj.anchor[0] as i32, -obj.anchor[1] as i32, &mut root, &mut names)?;
    Ok(root)
}

/// What `composite` shows: only the first of each run of variant groups,
/// each loop on `frame`, or on the frame it shows at `time`.
#[derive(Clone, Copy, Debug, Default)]
pub struct CompositeOptions {
    pub first_variant: bool,
    pub frame: usize,
    pub time: Option<f64>,
}

/// Flattens a tree to what shows (`composite`): pixel → color name, later
/// paint winning. Hidden groups never draw.
pub fn composite(root: &Group, options: CompositeOptions) -> OMap<Pixel, ColorName> {
    fn is_variant(c: &Child) -> bool {
        matches!(c, Child::Group(g) if g.attrs.iter().any(|(k, _)| k == "data-class"))
    }
    fn walk(group: &Group, options: &CompositeOptions, out: &mut OMap<Pixel, ColorName>) {
        for (i, child) in group.children.iter().enumerate() {
            match child {
                Child::Layer(l) => {
                    for (p, color) in l.pixels.iter() {
                        out.set(*p, color.clone());
                    }
                }
                Child::Group(g) => {
                    if g.hidden {
                        continue;
                    }
                    if let Some(frame) = g.frame {
                        let shown = match options.time {
                            None => options.frame,
                            Some(t) => motion::loop_frame(&loop_of(g), g.frames.unwrap_or(1), t),
                        };
                        if frame != shown {
                            continue;
                        }
                    }
                    if options.first_variant && is_variant(child) && i > 0 && is_variant(&group.children[i - 1]) {
                        continue;
                    }
                    walk(g, options, out);
                }
            }
        }
    }
    let mut out = OMap::new();
    walk(root, &options, &mut out);
    out
}

/// A frame group's loop name, the first of its classes.
fn loop_of(group: &Group) -> String {
    group.attrs.iter().find(|(k, _)| k == "class").map(|(_, v)| v.split(' ').next().unwrap_or("").to_string()).unwrap_or_default()
}

/// The bounding box of a set of pixels, or None if it's empty (`bounds`).
pub fn bounds<'a>(pixels: impl IntoIterator<Item = &'a Pixel>) -> Option<Bounds> {
    let mut out: Option<Bounds> = None;
    for &(x, y) in pixels {
        out = Some(match out {
            None => Bounds { min_x: x, min_y: y, max_x: x, max_y: y },
            Some(b) => Bounds { min_x: b.min_x.min(x), min_y: b.min_y.min(y), max_x: b.max_x.max(x), max_y: b.max_y.max(y) },
        });
    }
    out
}
