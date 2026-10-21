//! Shared by `depth.rs`, `depth_motion.rs`, `motion.rs` and `compile.rs`: the
//! mulberry32 generator (JS-exact, so a seed gives Node's cases), the small
//! palette and objects the Node suites use, the example documents in format 2,
//! and a few helpers for reading rendered trees.
#![allow(dead_code)]

pub mod css;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

use engine::depth::{block_box, sprite_box, sprite_height, Box3};
use engine::engine::{composite, load_sources, render_object, Child, ColorName, CompositeOptions, Group, Sources};
use engine::iso::Pixel;
use engine::omap::OMap;
use serde_json::{json, Value};

pub const SEEDS: [u32; 5] = [1, 7, 42, 2026, 31337];

/// Deterministic pseudo-random numbers (mulberry32), so a failure can be
/// replayed from its seed. Calls give the same sequence as the Node suites'.
pub struct Rng(u32);

pub fn rng(seed: u32) -> Rng {
    Rng(seed)
}

impl Rng {
    /// The next number in [0, 1), `r()`.
    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4294967296.0
    }

    /// `Math.floor(r() * n)`.
    pub fn floor(&mut self, n: f64) -> i64 {
        (self.next() * n).floor() as i64
    }

    /// `list[Math.floor(r() * list.length)]`.
    pub fn pick<'a, T>(&mut self, list: &'a [T]) -> &'a T {
        &list[self.floor(list.len() as f64) as usize]
    }
}

pub fn palette() -> Value {
    json!({
        "world": { "ink": "#2E2418", "cream": "#F4EDE0", "grass-2": "#8FA56E", "grass-4": "#4E6B3A", "wood-3": "#7A4E2D" },
        "outfit": {},
    })
}

/// `{...base, ...extra}`: extra's keys replace in place or are added.
pub fn with(mut base: Value, extra: Value) -> Value {
    for (k, v) in extra.as_object().expect("extra is an object") {
        base[k] = v.clone();
    }
    base
}

pub fn sprite(map: &[&str]) -> Value {
    sprite_x(map, json!({}))
}

pub fn sprite_x(map: &[&str], extra: Value) -> Value {
    with(json!({ "kind": "sprite", "keys": { "g": "grass-2", "i": "ink" }, "layers": [{ "map": map }] }), extra)
}

pub fn block() -> Value {
    block_x(json!({}))
}

pub fn block_x(extra: Value) -> Value {
    with(json!({ "kind": "block", "size": [1, 1, 1], "faces": { "top": "grass-2", "left": "grass-4", "right": "wood-3" } }), extra)
}

pub fn scene(items: Value) -> Value {
    scene_x(items, json!({}))
}

pub fn scene_x(items: Value, extra: Value) -> Value {
    with(json!({ "viewBox": [0, 0, 8, 8], "items": items }), extra)
}

fn to_map(list: &[(&str, Value)]) -> BTreeMap<String, Value> {
    list.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

/// Sources from inline objects and scenes.
pub fn src(objects: &[(&str, Value)], scenes: &[(&str, Value)]) -> Sources {
    load_sources(palette(), to_map(objects), to_map(scenes))
}

/// A scene's items, as a vector.
pub fn items(sources: &Sources, scene: &str) -> Vec<Value> {
    sources.scenes[scene]["items"].as_array().unwrap().clone()
}

/// The `at.tile` of an item as "c,r,l".
pub fn tile_str(item: &Value) -> String {
    item["at"]["tile"].as_array().map(|t| t.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(",")).unwrap_or_default()
}

/// The pixels an object paints standing at a point, in the order painted.
pub struct Painted {
    pub order: Vec<Pixel>,
    pub map: HashMap<Pixel, String>,
}

pub fn painted_at(sources: &Sources, object: &str, at: [f64; 2]) -> Painted {
    let mut p = Painted { order: Vec::new(), map: HashMap::new() };
    for (px, color) in composite(&render_object(sources, object).unwrap(), CompositeOptions::default()).iter() {
        let q = (px.0 + at[0] as i32, px.1 + at[1] as i32);
        p.order.push(q);
        p.map.insert(q, color.to_string());
    }
    p
}

/// A composite as a sorted list, the way an unordered deep-equal sees a Map.
pub fn sorted(pixels: &OMap<Pixel, ColorName>) -> Vec<(Pixel, String)> {
    let mut v: Vec<(Pixel, String)> = pixels.iter().map(|(p, c)| (*p, c.to_string())).collect();
    v.sort();
    v
}

/// A rendered tree as text with every layer's pixels sorted and its colors
/// sorted, so two trees compare as Node's `deepEqual` does (Map and Set order
/// doesn't count).
pub fn tree_text(group: &Group) -> String {
    let mut out = format!("{:?}|{:?}|{:?}|{}[", group.attrs, group.frame, group.frames, group.hidden);
    for child in &group.children {
        match child {
            Child::Layer(l) => {
                let mut colors: Vec<String> = l.colors.iter().map(|c| c.to_string()).collect();
                colors.sort();
                out.push_str(&format!("layer {:?} {:?};", sorted(&l.pixels), colors));
            }
            Child::Group(g) => {
                out.push_str(&tree_text(g));
                out.push(';');
            }
        }
    }
    out.push(']');
    out
}

/// Whether a group child is a group.
pub fn as_group(child: &Child) -> Option<&Group> {
    match child {
        Child::Group(g) => Some(g),
        Child::Layer(_) => None,
    }
}

/// The classes of `mv-` groups in a tree, in tree order.
pub fn mv_classes(group: &Group) -> Vec<String> {
    let mut out = Vec::new();
    for child in &group.children {
        if let Child::Group(g) = child {
            let cls = g.attrs.iter().find(|(k, _)| k == "class").map_or("", |(_, v)| v.as_str());
            if cls.starts_with("mv-") {
                out.push(cls.to_string());
            }
            out.extend(mv_classes(g));
        }
    }
    out
}

/// A format-2 copy of `examples/<name>.pixellab` at `<base>/<name>.pixellab`,
/// made with Node's `goldens.mjs --bootstrap`.
pub fn example(base: &Path, name: &str) -> PathBuf {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let to = base.join(format!("{name}.pixellab"));
    let out = Command::new("node")
        .arg(repo.join("scripts/goldens.mjs"))
        .arg("--bootstrap")
        .arg(repo.join("examples").join(format!("{name}.pixellab")))
        .arg(&to)
        .output()
        .expect("run node");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    to
}

/// The sources of a format-2 document folder, as `readSources` gives them.
pub fn read_sources(dir: &Path) -> (Sources, Vec<String>) {
    let docs = engine::format::read_docs(dir);
    (load_sources(docs.palette.unwrap_or(Value::Null), docs.objects, docs.scenes), docs.load_errors)
}

/// A temp folder removed on drop.
pub struct Temp(pub PathBuf);

impl Temp {
    pub fn new() -> Temp {
        Temp(engine::compile::temp_dir("pixellab-engine-test-").unwrap())
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Random boxes, mostly unit blocks and sprites on a small grid so keys tie and boxes interpenetrate.
pub fn random_boxes(r: &mut Rng, n: usize, span: f64) -> Vec<Box3> {
    (0..n)
        .map(|_| {
            let (c, row, l) = (r.floor(span) as f64, r.floor(span) as f64, (r.floor(3.0) - 1) as f64);
            if r.next() < 0.5 {
                let size = [1.0 + r.floor(2.0) as f64, 1.0 + r.floor(2.0) as f64, r.floor(3.0) as f64];
                block_box(c, row, l, size)
            } else {
                let (dc, dr) = (r.floor(32.0) as f64 / 32.0, r.floor(32.0) as f64 / 32.0);
                let fp = [1.0 + r.floor(2.0) as f64, 1.0];
                sprite_box(c + dc, row + dr, l, fp, sprite_height(r.floor(40.0) as f64))
            }
        })
        .collect()
}
