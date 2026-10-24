//! PNG renders of scenes and objects for scene.render, object.render and the app (ports cli/lib/render.mjs).
//!
//! A scene or object as a transparent PNG at an integer scale, with each
//! item's bounds in output pixels so the app can hit-test clicks. It does
//! what the web lab's stage does: pose the scene's moving items at `time`,
//! show one variant of each run of sibling data-class groups, render with
//! the engine's `render_scene` and composite, and take each item's pixels
//! from `render_object` at its placement point.

use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use engine::depth::Bounds;
use engine::engine::{
    self as eng, composite, js_string, origin_of, render_object_in, render_scene, resolve, scene_at, truthy, Child, ColorName,
    CompositeOptions, Group, Order, Sources, SpriteLayer,
};
use engine::iso::{self, Pixel};
use engine::jsnum::{as_f64, num};
use engine::omap::{OMap, OSet};
use engine::png::Image;
use engine::{edit, motion, preview, states};
use serde_json::{Map, Value};

use crate::errors::{CommandError, Error, Result};

pub const MAX_SCALE: u32 = 16;

/// A path into a scene's items: [5, 2] is "5.2".
pub fn path_string(path: &[usize]) -> String {
    path.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(".")
}

/// Parses "5.2" to [5, 2]; None if it isn't a path.
pub fn parse_path(text: &str) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for part in text.split('.') {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        out.push(part.parse::<f64>().ok()? as usize);
    }
    Some(out)
}

/// Parses `--play <action>@<ms>`: an action's name and the ms into it.
pub fn parse_play(text: &str) -> Result<(String, f64)> {
    let bad = || CommandError::usage(format!("--play {text}: give an action and the ms into it, like hop@120"));
    let (name, at) = text.rsplit_once('@').ok_or_else(bad)?;
    if !engine::engine::is_name(name) || at.is_empty() || !at.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad().into());
    }
    Ok((name.to_string(), at.parse::<f64>().map_err(|_| bad())?))
}

/// Whether any item of a scene (or a layer of an object it places) has the action.
fn scene_has_action(sources: &Sources, items: Option<&Value>, action: &str) -> bool {
    items.and_then(Value::as_array).into_iter().flatten().any(|item| {
        if item.get("group").is_some() {
            return scene_has_action(sources, item.get("items"), action);
        }
        let own = engine::ambient::moves_of(item).is_some_and(|m| m.actions.iter().any(|(n, _)| n == action));
        own || resolve(sources, &js_string(item.get("object")))
            .is_ok_and(|r| r.layer_moves.iter().flatten().any(|m| m.actions.iter().any(|(n, _)| n == action)))
    })
}

/// Where an item's placement point is, in scene pixels.
pub fn placement_point(item: &Value, origin: [f64; 2]) -> [f64; 2] {
    let at = item.get("at");
    match at.and_then(|a| a.get("tile")).filter(|t| truthy(Some(t))) {
        Some(tile) => {
            let n = |i: usize| tile.get(i).and_then(as_f64).unwrap_or(f64::NAN);
            iso::tile_to_px([n(0), n(1), n(2)], origin)
        }
        None => {
            let px = at.and_then(|a| a.get("px"));
            let n = |i: usize| px.and_then(|p| p.get(i)).filter(|v| !v.is_null()).and_then(as_f64).unwrap_or(0.0);
            [n(0), n(1)]
        }
    }
}

fn is_group(item: &Value) -> bool {
    item.as_object().is_some_and(|o| o.contains_key("group"))
}

fn is_variant(item: &Value) -> bool {
    is_group(item) && item.get("group").and_then(Value::as_object).is_some_and(|g| g.contains_key("data-class"))
}

/// An item that is shown: its path into the untrimmed items.
#[derive(Clone, Debug)]
pub struct Leaf {
    pub path: Vec<usize>,
    pub item: Value,
}

/// The scene as the stage shows it (a port of the lab's `visibleItems`): of
/// each run of sibling data-class groups only one shows, the one holding
/// `selected`, else the first. Returns the trimmed items, and every item
/// that shows, in paint order, with its path into the untrimmed items.
/// Items that come from `scene_at` carry a `path`, their path in the
/// scene's own items, which the depth order has moved them from; without
/// one they are where they are listed.
/// An item's path in the scene's own items: the `path` it carries, else where it is listed (`i` under `prefix`).
fn item_path(item: &Value, prefix: &[usize], i: usize) -> Vec<usize> {
    match item.get("path").and_then(Value::as_array) {
        Some(p) => p.iter().map(|n| as_f64(n).unwrap_or(0.0) as usize).collect(),
        None => {
            let mut p = prefix.to_vec();
            p.push(i);
            p
        }
    }
}

pub fn visible_items(items: &[Value], selected: Option<&[usize]>, prefix: &[usize]) -> (Vec<Value>, Vec<Leaf>) {
    let mut kept: Vec<Value> = Vec::new();
    let mut leaves: Vec<Leaf> = Vec::new();
    let path_of = |i: usize| -> Vec<usize> { item_path(&items[i], prefix, i) };
    let inner = |item: &Value| -> Vec<Value> { item.get("items").and_then(Value::as_array).cloned().unwrap_or_default() };
    let with_items = |item: &Value, sub: Vec<Value>| -> Value {
        let mut out = item.clone();
        if let Some(o) = out.as_object_mut() {
            o.insert("items".into(), Value::Array(sub));
        }
        out
    };
    let mut run: Vec<usize> = Vec::new();
    let flush = |run: &mut Vec<usize>, kept: &mut Vec<Value>, leaves: &mut Vec<Leaf>| {
        if run.is_empty() {
            return;
        }
        let holder = run.iter().copied().find(|&i| match selected {
            Some(s) if s.len() > prefix.len() => path_of(i).get(prefix.len()) == Some(&s[prefix.len()]),
            _ => false,
        });
        let shown = holder.unwrap_or(run[0]);
        let (sub_kept, sub_leaves) = visible_items(&inner(&items[shown]), selected, &path_of(shown));
        kept.push(with_items(&items[shown], sub_kept));
        leaves.extend(sub_leaves);
        run.clear();
    };
    for (i, item) in items.iter().enumerate() {
        if is_variant(item) {
            run.push(i);
            continue;
        }
        flush(&mut run, &mut kept, &mut leaves);
        if is_group(item) {
            let (sub_kept, sub_leaves) = visible_items(&inner(item), selected, &path_of(i));
            kept.push(with_items(item, sub_kept));
            leaves.extend(sub_leaves);
        } else {
            kept.push(item.clone());
            leaves.push(Leaf { path: path_of(i), item: item.clone() });
        }
    }
    flush(&mut run, &mut kept, &mut leaves);
    (kept, leaves)
}

/// Every item of a scene, flattened in paint order, with groups listed
/// apart: `{ items, groups, layers }`.
pub fn flatten_items(scene: &Value) -> Value {
    let origin = origin_of(scene);
    let mut items: Vec<Value> = Vec::new();
    let mut groups: Vec<Value> = Vec::new();
    fn walk(list: &[Value], prefix: &[usize], group: Option<&str>, origin: [f64; 2], items: &mut Vec<Value>, groups: &mut Vec<Value>) {
        for (i, item) in list.iter().enumerate() {
            let mut path = prefix.to_vec();
            path.push(i);
            let path_text = path_string(&path);
            if is_group(item) {
                let mut g = Map::new();
                g.insert("path".into(), Value::String(path_text.clone()));
                let attrs = match item.get("group") {
                    Some(Value::Object(o)) => Value::Object(o.clone()),
                    _ => Value::Object(Map::new()),
                };
                g.insert("attrs".into(), attrs);
                let children = item.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
                g.insert("items".into(), Value::from(children.len()));
                if let Some(d) = item.get("depth") {
                    g.insert("depth".into(), d.clone());
                }
                if let Some(parent) = group {
                    g.insert("group".into(), Value::String(parent.to_string()));
                }
                groups.push(Value::Object(g));
                walk(&children, &path, Some(&path_text), origin, items, groups);
                continue;
            }
            let mut o = Map::new();
            o.insert("path".into(), Value::String(path_text));
            if let Some(v) = item.get("object") {
                o.insert("object".into(), v.clone());
            }
            if let Some(v) = item.get("at") {
                o.insert("at".into(), v.clone());
            }
            // A pixel item stands on a level of its own (default 0); a tile's level is its third number.
            if truthy(item.get("at").and_then(|a| a.get("px"))) {
                let level = item.pointer("/at/level").filter(|v| !v.is_null()).cloned().unwrap_or_else(|| Value::from(0));
                o.insert("level".into(), level);
            }
            for key in ["depth", "class", "id", "layer", "state"] {
                if let Some(v) = item.get(key) {
                    o.insert(key.into(), v.clone());
                }
            }
            if let Some(parent) = group {
                o.insert("group".into(), Value::String(parent.to_string()));
            }
            let [x, y] = placement_point(item, origin);
            o.insert("point".into(), Value::Array(vec![num(x), num(y)]));
            items.push(Value::Object(o));
        }
    }
    let top = scene.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
    walk(&top, &[], None, origin, &mut items, &mut groups);
    let layers = layer_summary(scene, &items);
    let mut out = Map::new();
    out.insert("items".into(), Value::Array(items));
    out.insert("groups".into(), Value::Array(groups));
    out.insert("layers".into(), Value::Array(layers));
    Value::Object(out)
}

/// The scene's layers (docs/intents/2026-10-scene-layers/spec.md, D4), each
/// with how many items are in it or in the layers inside it.
fn layer_summary(scene: &Value, items: &[Value]) -> Vec<Value> {
    let layers: Vec<&Value> = scene.get("layers").and_then(Value::as_array).map(|l| l.iter().collect()).unwrap_or_default();
    let mut parents: OMap<String, Option<String>> = OMap::new();
    for l in &layers {
        if let Some(id) = l.get("id").and_then(Value::as_str) {
            parents.set(id.to_string(), l.get("parent").and_then(Value::as_str).map(str::to_string));
        }
    }
    let mut counts: OMap<String, usize> = OMap::new();
    for item in items {
        let mut seen: OSet<String> = OSet::new();
        let mut cur = item.get("layer").and_then(Value::as_str).map(str::to_string);
        while let Some(l) = cur {
            if !parents.has(l.as_str()) || seen.has(l.as_str()) {
                break;
            }
            seen.add(l.clone());
            *counts.get_or_insert_with(l.clone(), || 0) += 1;
            cur = parents.get(l.as_str()).cloned().flatten();
        }
    }
    layers
        .iter()
        .map(|l| {
            let mut o = Map::new();
            if let Some(v) = l.get("id") {
                o.insert("id".into(), v.clone());
            }
            if let Some(v) = l.get("name") {
                o.insert("name".into(), v.clone());
            }
            if let Some(v) = l.get("parent") {
                o.insert("parent".into(), v.clone());
            }
            let n = l.get("id").and_then(Value::as_str).and_then(|id| counts.get(id).copied()).unwrap_or(0);
            o.insert("items".into(), Value::from(n));
            Value::Object(o)
        })
        .collect()
}

/// The scene's items with every placed item for which `hidden(item)` holds
/// marked hidden (as a moving item away from the stage is), leaving paths
/// alone. `hidden` is given the item and its path in the scene's own items.
fn hide_where(items: &[Value], prefix: &[usize], hidden: &dyn Fn(&Value, &[usize]) -> bool) -> Vec<Value> {
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let path = item_path(item, prefix, i);
            if is_group(item) {
                let inner = item.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
                let mut out = item.clone();
                out["items"] = Value::Array(hide_where(&inner, &path, hidden));
                return out;
            }
            if truthy(Some(item)) && !truthy(item.get("hidden")) && hidden(item, &path) {
                let mut out = item.clone();
                if let Some(o) = out.as_object_mut() {
                    o.insert("hidden".into(), Value::Bool(true));
                }
                out
            } else {
                item.clone()
            }
        })
        .collect()
}

/// The color name the dark theme's ground pixels get (not a palette name).
const PAGE: &str = "--color-page";
/// The dark page color when a site names none.
pub const DARK_PAGE: &str = "#1C1712";

/// The site's dark-theme page color: --color-page in the
/// [data-theme="dark"] block of src/styles/variables.css, else #1C1712.
/// Always `Some` (every failure gives the default), as the JS returned a
/// string.
pub fn dark_page_color(root: &Path) -> Option<String> {
    let found = (|| {
        let css = std::fs::read_to_string(root.join("src/styles/variables.css")).ok()?;
        let at = css.find("[data-theme=\"dark\"]")?;
        let rest = &css[at..];
        let mut from = 0;
        while let Some(i) = rest[from..].find("--color-page:") {
            let mut j = from + i + "--color-page:".len();
            while let Some(c) = rest[j..].chars().next().filter(|&c| engine::engine::js_space(c)) {
                j += c.len_utf8();
            }
            let tail = &rest[j..];
            let b = tail.as_bytes();
            if b.len() >= 7 && b[0] == b'#' && b[1..7].iter().all(u8::is_ascii_hexdigit) {
                let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
                if !tail[7..].chars().next().is_some_and(word) {
                    return Some(tail[..7].to_ascii_uppercase());
                }
            }
            from += i + 1;
        }
        None
    })();
    Some(found.unwrap_or_else(|| DARK_PAGE.to_string()))
}

/// The site's dark theme in a rendered tree: every pixel under a group with
/// data-enter="shadow" (the hero island's dithered drop shadow) takes the
/// page color. The sources are untouched; only this render changes.
fn dark_theme(group: &mut Group, inside: bool) {
    for child in group.children.iter_mut() {
        match child {
            Child::Layer(layer) => {
                if inside {
                    let page: ColorName = Rc::from(PAGE);
                    for color in layer.pixels.values_mut() {
                        *color = page.clone();
                    }
                }
            }
            Child::Group(g) => {
                let shadow = inside || g.attrs.iter().any(|(k, v)| k == "data-enter" && v == "shadow");
                dark_theme(g, shadow);
            }
        }
    }
}

/// parseInt(text, 16): the leading hex digits, 0 if none (NaN stores as 0).
fn hex_byte(text: &str) -> u8 {
    let digits: String = text.chars().take_while(char::is_ascii_hexdigit).collect();
    u8::from_str_radix(&digits, 16).unwrap_or(0)
}

/// An RGBA buffer of the pixels inside a box [x, y, width, height];
/// everything else transparent. `extra` is the hex of color names outside
/// the palette.
fn to_image(sources: &Sources, pixels: &OMap<Pixel, ColorName>, view: [i64; 4], extra: &[(&str, &str)]) -> Image {
    let [bx, by, width, height] = view;
    let (w, h) = (width.max(0), height.max(0));
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let mut rgb: HashMap<&str, [u8; 3]> = HashMap::new();
    for ((px, py), name) in pixels.iter() {
        let x = *px as i64 - bx;
        let y = *py as i64 - by;
        if x < 0 || y < 0 || x >= w || y >= h {
            continue;
        }
        let color = rgb.entry(name).or_insert_with(|| {
            let hex = sources
                .colors
                .get(&**name)
                .map(|c| c.hex.as_str())
                .or_else(|| extra.iter().find(|(k, _)| k == &&**name).map(|(_, v)| *v))
                .unwrap_or("#000000");
            let part = |i: usize| hex.get(i..i + 2).map_or(0, hex_byte);
            [part(1), part(3), part(5)]
        });
        let at = ((y * w + x) * 4) as usize;
        rgba[at..at + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
    }
    Image { width: w as u32, height: h as u32, rgba }
}

/// The engine's pixels for one object drawn alone with its anchor at (0, 0),
/// and their bounding box.
fn object_pixels(
    sources: &Sources,
    name: &str,
    time: Option<f64>,
    frame: usize,
    play: Option<(String, f64)>,
    state: Option<&str>,
) -> eng_result::Result<(OMap<Pixel, ColorName>, Option<Bounds>)> {
    let tree = render_object_in(sources, name, state)?;
    let pixels = composite(&tree, CompositeOptions { first_variant: false, frame, time, play });
    let box_ = eng::bounds(pixels.keys());
    Ok((pixels, box_))
}

mod eng_result {
    pub use engine::Result;
}

/// Scene-pixel box to [x, y, w, h] in output pixels (null if none).
fn to_output(b: Option<Bounds>, view: [i64; 4], scale: u32) -> Value {
    match b {
        None => Value::Null,
        Some(b) => {
            let s = scale as i64;
            let n = |v: i64| Value::from(v);
            Value::Array(vec![
                n((b.min_x as i64 - view[0]) * s),
                n((b.min_y as i64 - view[1]) * s),
                n((b.max_x as i64 - b.min_x as i64 + 1) * s),
                n((b.max_y as i64 - b.min_y as i64 + 1) * s),
            ])
        }
    }
}

/// Standard base64 with padding.
pub fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

/// A render's theme: `dark` draws as the site's dark theme does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

/// Options for `render_scene_image`. `hide` is a set of layer ids and item paths whose
/// items aren't drawn or listed; with `focus` (a set of layer ids) the
/// result adds `focusPng`, the same render with only those layers' items.
#[derive(Clone, Debug)]
pub struct SceneImageOptions {
    pub scale: u32,
    pub time: f64,
    pub show: Option<Vec<usize>>,
    pub theme: Theme,
    pub hide: Option<OSet<String>>,
    pub focus: Option<OSet<String>>,
    /// An action playing, and ms into it (`--play hop@120`): its elements take its move instead of their ambient loop.
    pub play: Option<(String, f64)>,
    /// A window of the scene to render, `[x, y, w, h]` in scene pixels, instead of its box (scene-canvas
    /// spec R9): the app asks for what is on the stage, so an unbounded scene is never rendered whole.
    pub view: Option<[i64; 4]>,
}

impl Default for SceneImageOptions {
    fn default() -> Self {
        SceneImageOptions { scale: 1, time: 0.0, show: None, theme: Theme::Light, hide: None, focus: None, play: None, view: None }
    }
}

/// The most pixels a `view` window may hold (4 bytes each): a stage is never this big.
const MAX_VIEW_PIXELS: i128 = 100_000_000;

fn original<'a>(scene: &'a Value, path: &[usize]) -> Option<&'a Value> {
    let mut list = scene.get("items")?;
    let mut found = None;
    for (k, &i) in path.iter().enumerate() {
        let item = list.get(i)?;
        found = Some(item);
        if k + 1 < path.len() {
            list = item.get("items")?;
        }
    }
    found
}

fn render_error(kind: &str, name: &str, message: &str) -> Error {
    CommandError::new(format!("{kind} {name} can't render: {message}; run pixellab check {name}"), "render_error").into()
}

/// Renders a scene at `time` ms to a PNG, with its items' bounds. With
/// `theme: Dark` it draws as the site's dark theme does (`dark_page` is the
/// color the shadow pixels take).
pub fn render_scene_image(sources: &Sources, name: &str, dark_page: &str, options: &SceneImageOptions) -> Result<Value> {
    let scene = sources
        .scenes
        .get(name)
        .ok_or_else(|| Error::Internal(format!("Cannot read properties of undefined (reading 'origin'): {name}")))?;
    if let Some((action, _)) = &options.play {
        if !scene_has_action(sources, scene.get("items"), action) {
            return Err(CommandError::new(
                format!("scene {name} has no action {action}; give one of its items an action (pixellab scene action)"),
                "not_found",
            )
            .into());
        }
    }
    // The box the scene publishes at: its own viewBox, else the fit to what it paints (spec R2, R7).
    let frame = engine::engine::resolve_frame(sources, name).map_err(|e| render_error("scene", name, &e.0))?;
    let view = match options.view {
        Some([_, _, w, h]) if w <= 0 || h <= 0 => {
            return Err(CommandError::usage("scene.render: view must be x,y,w,h with w and h above 0").into());
        }
        Some([_, _, w, h]) if (w as i128) * (h as i128) > MAX_VIEW_PIXELS => {
            return Err(CommandError::usage(format!("scene.render: view is larger than {MAX_VIEW_PIXELS} pixels")).into());
        }
        Some(window) => window,
        None => frame.view_box,
    };
    let mut image = inner_scene_image(sources, name, scene, view, dark_page, options).map_err(|m| render_error("scene", name, &m))?;
    if let Some(out) = image.as_object_mut() {
        out.insert("viewBox".into(), Value::Array(view.iter().map(|&n| Value::from(n)).collect()));
        out.insert("frame".into(), Value::from(if frame.fixed { "fixed" } else { "fit" }));
    }
    Ok(image)
}

/// What a scene shows at times (every item, light theme), pixel by pixel: the paint a scene render at
/// that time draws, for comparing two versions of a scene (poses becoming states, R38). It keeps one
/// working copy of the sources, so a render at each of many times doesn't copy the document.
pub struct ScenePixels {
    local: Sources,
    name: String,
    scene: Value,
    /// The tree drawn for the last posed scene (times come in order): between two times where nothing
    /// is placed, posed or ordered differently, only what plays inside the tree (loops, states) changes.
    last: Option<(String, Rc<Group>)>,
}

impl ScenePixels {
    pub fn new(sources: &Sources, name: &str) -> ScenePixels {
        let scene = sources.scenes.get(name).cloned().unwrap_or(Value::Null);
        ScenePixels { local: sources.clone(), name: name.to_string(), scene, last: None }
    }

    pub fn at(&mut self, time: f64) -> std::result::Result<OMap<Pixel, ColorName>, String> {
        let e = |err: engine::EngineError| err.0;
        self.local.scenes.insert(self.name.clone(), self.scene.clone());
        let mut still = scene_at(&self.local, &self.name, time).map_err(e)?;
        let items = match still.as_object_mut().and_then(|o| o.shift_remove("items")) {
            Some(Value::Array(a)) => a,
            _ => Vec::new(),
        };
        if let Some(o) = still.as_object_mut() {
            o.insert("items".into(), Value::Array(motion::shown_items(&items)));
        }
        let key = still.to_string();
        let root = match &self.last {
            Some((k, root)) if *k == key => root.clone(),
            _ => {
                self.local.scenes.insert(self.name.clone(), still);
                let root = Rc::new(render_scene(&self.local, &self.name, Order::List).map_err(e)?.root);
                self.last = Some((key, root.clone()));
                root
            }
        };
        Ok(composite(&root, CompositeOptions { first_variant: false, frame: 0, time: Some(time), play: None }))
    }
}

fn inner_scene_image(
    sources: &Sources,
    name: &str,
    scene: &Value,
    view: [i64; 4],
    dark_page: &str,
    options: &SceneImageOptions,
) -> std::result::Result<Value, String> {
    let e = |err: engine::EngineError| err.0;
    let SceneImageOptions { scale, time, show, theme, hide, focus, play, view: _ } = options;
    let (scale, time) = (*scale, *time);
    let origin = origin_of(scene);
    // The scene as it stands at `time`: every list in the order it draws in then (an item that moves among others
    // changes place), each tracked item posed. Hiding a layer or showing one variant leaves the rest where it is,
    // so what's drawn here is drawn in the order given (order 'list').
    let mut still = scene_at(sources, name, time).map_err(e)?;
    let ordered: Vec<Value> = match still.as_object_mut().and_then(|o| o.shift_remove("items")) {
        Some(Value::Array(a)) => a,
        _ => Vec::new(),
    };
    let mut posed = ordered;
    if let Some(hide) = hide.as_ref().filter(|h| !h.is_empty()) {
        // A layer's id, or an item's path (like 145.0): the items in those layers, and those items.
        posed = hide_where(&posed, &[], &|item, path| {
            item.get("layer").and_then(Value::as_str).is_some_and(|l| hide.has(l)) || hide.has(&path_string(path))
        });
    }
    let (kept, leaves) = visible_items(&posed, show.as_deref(), &[]);
    let dark = *theme == Theme::Dark;
    let extra: Vec<(&str, &str)> = if dark { vec![(PAGE, dark_page)] } else { vec![] };
    let mut local = sources.clone();
    // The PNG of these items (trimmed to the shown variants), as the scene draws them.
    let mut paint = |shown: &[Value]| -> std::result::Result<Image, String> {
        let mut scene_view = still.clone();
        if let Some(o) = scene_view.as_object_mut() {
            o.insert("items".into(), Value::Array(motion::shown_items(shown)));
        }
        local.scenes.clear();
        local.scenes.insert(name.to_string(), scene_view);
        let mut rendered = render_scene(&local, name, Order::List).map_err(e)?;
        if dark {
            dark_theme(&mut rendered.root, false);
        }
        let pixels = composite(&rendered.root, CompositeOptions { first_variant: false, frame: 0, time: Some(time), play: play.clone() });
        Ok(to_image(sources, &pixels, view, &extra))
    };
    let image = paint(&kept)?;
    let focus_image = match focus {
        Some(focus) => {
            let only = hide_where(&kept, &[], &|item, _| item.get("layer").and_then(Value::as_str).is_none_or(|l| !focus.has(l)));
            Some(paint(&only)?)
        }
        None => None,
    };
    let mut arts: HashMap<String, Option<Bounds>> = HashMap::new();
    let mut state_cache: HashMap<String, Option<Rc<states::States>>> = HashMap::new();
    let mut items: Vec<Value> = Vec::new();
    for Leaf { path, item } in leaves.iter().filter(|l| !truthy(l.item.get("hidden"))) {
        let source = original(scene, path).ok_or_else(|| "Cannot read properties of undefined".to_string())?;
        let [x, y] = placement_point(item, origin);
        let object = js_string(item.get("object"));
        // A sprite with states is in the state the item is in, entered `since` ms before time 0 (R15), so its box is
        // the one that state shows now: the frame and the move.
        let states = state_cache.entry(object.clone()).or_insert_with(|| resolve(sources, &object).ok().and_then(|r| r.states)).clone();
        let stated = states.as_ref().map(|states| {
            let name = states.first(item.get("state").and_then(Value::as_str)).name.clone();
            (states.clone(), name, item.get("since").and_then(as_f64).unwrap_or(0.0))
        });
        let (key, state, at) = match &stated {
            Some((_, name, since)) => (format!("{object} {name} {since}"), Some(name.as_str()), time - since),
            None => (object.clone(), None, time),
        };
        let art = match arts.get(&key) {
            Some(b) => *b,
            None => {
                let (_, b) = object_pixels(sources, &object, Some(at), 0, None, state).map_err(e)?;
                arts.insert(key, b);
                b
            }
        };
        // Bounds shift by the (whole-pixel) placement point, as JS added the doubles.
        let shifted = art.map(|b| Bounds {
            min_x: (b.min_x as f64 + x) as i32,
            min_y: (b.min_y as f64 + y) as i32,
            max_x: (b.max_x as f64 + x) as i32,
            max_y: (b.max_y as f64 + y) as i32,
        });
        let mut o = Map::new();
        o.insert("path".into(), Value::String(path_string(path)));
        if let Some(v) = source.get("object") {
            o.insert("object".into(), v.clone());
        }
        if item.get("object") != source.get("object") {
            if let Some(v) = item.get("object") {
                o.insert("pose".into(), v.clone());
            }
        }
        if let Some(v) = source.get("at") {
            o.insert("at".into(), v.clone());
        }
        if let Some(v) = source.get("id") {
            o.insert("id".into(), v.clone());
        }
        // A sprite with states says which state it is in and which frame that shows at this time (R15).
        if let Some((states, name, since)) = stated {
            let frame = states::at(&states, &states::run(&states, &name), time - since).frame;
            o.insert("state".into(), Value::String(name));
            o.insert("frame".into(), Value::from(frame));
        }
        o.insert("bounds".into(), to_output(shifted, view, scale));
        items.push(Value::Object(o));
    }
    let mut out = Map::new();
    out.insert("png".into(), Value::String(base64(&preview::encode_png(&image, scale))));
    out.insert("scale".into(), Value::from(scale));
    out.insert("viewBox".into(), Value::Null);
    out.insert("width".into(), Value::from(image.width as u64 * scale as u64));
    out.insert("height".into(), Value::from(image.height as u64 * scale as u64));
    out.insert("time".into(), num(time));
    if dark {
        out.insert("theme".into(), Value::String("dark".into()));
    }
    if let Some(f) = focus_image {
        out.insert("focusPng".into(), Value::String(base64(&preview::encode_png(&f, scale))));
    }
    out.insert("items".into(), Value::Array(items));
    Ok(Value::Object(out))
}

/// Some of a sprite's layers at a frame (each clamped to its last frame),
/// with its anchor at (0, 0), and their box. Port of the lab's
/// ObjectMode.pixelsAt.
fn layer_pixels(resolved: &eng::Resolved, layers: &[usize], frame: usize) -> (OMap<Pixel, ColorName>, Option<Bounds>) {
    let mut pixels: OMap<Pixel, ColorName> = OMap::new();
    let [ax, ay] = resolved.anchor;
    for (i, layer) in resolved.layers.iter().enumerate() {
        if !layers.contains(&i) {
            continue;
        }
        let layer: &SpriteLayer = layer;
        for (y, row) in edit::map_at(layer, frame).iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                if c == '.' {
                    continue;
                }
                let color = resolved.keys.get(c.to_string().as_str()).filter(|k| !k.is_empty());
                if let Some(color) = color {
                    pixels.set(((x as i64 - ax) as i32, (y as i64 - ay) as i32), Rc::from(color.as_str()));
                }
            }
        }
    }
    let b = eng::bounds(pixels.keys());
    (pixels, b)
}

/// Options for `render_object_image`: with `layers` (a sprite's layer
/// indexes) only those layers show, each at `frame` clamped to its own last
/// frame (hidden layers and the onion skin); a block ignores it. With `time`
/// the frame loops play to it, else `frame` shows.
#[derive(Clone, Debug)]
pub struct ObjectImageOptions {
    pub scale: u32,
    pub time: Option<f64>,
    pub frame: usize,
    pub layers: Option<Vec<usize>>,
    /// An action playing at `time`, and ms into it (`--play hop@120`).
    pub play: Option<(String, f64)>,
}

impl Default for ObjectImageOptions {
    fn default() -> Self {
        ObjectImageOptions { scale: 1, time: None, frame: 0, layers: None, play: None }
    }
}

/// Renders one object to a PNG with its anchor at (0, 0). A sprite's viewBox
/// is its whole map, so output pixel (x, y) at 1x is map column x, row y; a
/// block's is the box around its painted pixels.
pub fn render_object_image(sources: &Sources, name: &str, options: &ObjectImageOptions) -> Result<Value> {
    render_object_image_in(sources, name, options, None)
}

/// `render_object_image` with a sprite that has states in `state` (its start state when None): a
/// `time` is then ms after entering it, and the item says which state and frame it shows (R6).
pub fn render_object_image_in(sources: &Sources, name: &str, options: &ObjectImageOptions, state: Option<&str>) -> Result<Value> {
    if let Some((action, _)) = &options.play {
        if options.time.is_none() {
            return Err(CommandError::usage("--play needs --time: the action plays over the loops at that time").into());
        }
        let has = resolve(sources, name)
            .map(|r| r.layer_moves.iter().flatten().any(|m| m.actions.iter().any(|(n, _)| n == action)))
            .unwrap_or(false);
        if !has {
            return Err(CommandError::new(
                format!("object {name} has no action {action}; give a layer one (pixellab object action)"),
                "not_found",
            )
            .into());
        }
    }
    inner_object_image(sources, name, options, state).map_err(|m| render_error("object", name, &m))
}

fn inner_object_image(
    sources: &Sources,
    name: &str,
    options: &ObjectImageOptions,
    state: Option<&str>,
) -> std::result::Result<Value, String> {
    let ObjectImageOptions { scale, time, frame, layers, play } = options;
    let (scale, frame) = (*scale, *frame);
    let resolved = resolve(sources, name).map_err(|e| e.0)?;
    let (pixels, b) = match layers {
        Some(layers) if resolved.block.is_none() => layer_pixels(&resolved, layers, frame),
        _ => object_pixels(sources, name, *time, frame, play.clone(), state).map_err(|e| e.0)?,
    };
    let view: [i64; 4] = if resolved.block.is_none() {
        let map = [-resolved.anchor[0], -resolved.anchor[1], resolved.width, resolved.height];
        match b {
            // At a `time` (ambient loops, a playing action) the layers move: the box grows to hold them.
            Some(b) if time.is_some() => {
                let (x0, y0) = ((b.min_x as i64).min(map[0]), (b.min_y as i64).min(map[1]));
                let (x1, y1) = ((b.max_x as i64 + 1).max(map[0] + map[2]), (b.max_y as i64 + 1).max(map[1] + map[3]));
                [x0, y0, x1 - x0, y1 - y0]
            }
            _ => map,
        }
    } else if let Some(b) = b {
        [b.min_x as i64, b.min_y as i64, (b.max_x - b.min_x + 1) as i64, (b.max_y - b.min_y + 1) as i64]
    } else {
        [0, 0, 1, 1]
    };
    let image = to_image(sources, &pixels, view, &[]);
    let mut out = Map::new();
    out.insert("png".into(), Value::String(base64(&preview::encode_png(&image, scale))));
    out.insert("scale".into(), Value::from(scale));
    out.insert("viewBox".into(), Value::Array(view.iter().map(|&n| Value::from(n)).collect()));
    out.insert("width".into(), Value::from(image.width as u64 * scale as u64));
    out.insert("height".into(), Value::from(image.height as u64 * scale as u64));
    match time {
        Some(t) => out.insert("time".into(), num(*t)),
        None => out.insert("frame".into(), Value::from(frame)),
    };
    let anchor = if resolved.block.is_some() { [0, 0] } else { resolved.anchor };
    out.insert("anchor".into(), Value::Array(vec![Value::from(anchor[0]), Value::from(anchor[1])]));
    let mut item = Map::new();
    item.insert("path".into(), Value::Null);
    item.insert("object".into(), Value::String(name.to_string()));
    let mut at = Map::new();
    at.insert("px".into(), Value::Array(vec![Value::from(0), Value::from(0)]));
    item.insert("at".into(), Value::Object(at));
    // A sprite with states says which state it shows and which frame that has at this time (R6).
    if let Some(states) = &resolved.states {
        let name = states.first(state).name.clone();
        let shown = match time {
            Some(t) => states::at(states, &states::run(states, &name), *t).frame,
            None => frame,
        };
        item.insert("state".into(), Value::String(name));
        item.insert("frame".into(), Value::from(shown));
    }
    item.insert("bounds".into(), to_output(b, view, scale));
    out.insert("items".into(), Value::Array(vec![Value::Object(item)]));
    Ok(Value::Object(out))
}
