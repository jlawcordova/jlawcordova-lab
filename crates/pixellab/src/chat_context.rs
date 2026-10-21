//! The selection block a message in Pixel Lab's Agent chat carries
//! (ports cli/lib/chat-context.mjs; docs/intents/2026-10-agent-chat/spec.md,
//! D5): what the owner selected, as the saved document has it. Claude works
//! from the file on disk, so a target is named by its path in the saved file,
//! and one that isn't in the saved file as it is in the app (placed, moved or
//! changed since the save) is left out of the block.
//!
//! Working items are matched to saved ones by their content (object, at, id,
//! class, layer, depth and their SVG group's attributes): the k-th working
//! item with a given content is the k-th saved item with it. A layer is saved
//! when the file has a layer with its id holding the same items.

use std::collections::{HashMap, HashSet};

use engine::engine::{js_string, truthy};
use engine::omap::OMap;
use serde_json::{json, Map, Value};

use crate::document::Env;
use crate::errors::{CommandError, Result};
use crate::layer_ops::{closure, layers_of};

/// The Agent chat a server works for (`chatOf`): PIXELLAB_CHAT and
/// PIXELLAB_CHAT_TITLE in its env.
pub fn chat_of(env: &Env) -> Option<Value> {
    let id = env.get("PIXELLAB_CHAT").map(|s| s.trim()).filter(|s| !s.is_empty())?;
    let title = env.get("PIXELLAB_CHAT_TITLE").map(|s| s.trim()).filter(|s| !s.is_empty()).unwrap_or("Chat");
    Some(json!({ "id": id, "title": title }))
}

/// A group lists at most this many item paths in the block.
pub const GROUP_PATHS: usize = 60;

/// A placed item with its path and its wrapper's attributes.
#[derive(Clone, Debug, PartialEq)]
pub struct Walked {
    pub path: String,
    pub item: Value,
    pub wrapper: Value,
}

fn is_group(item: &Value) -> bool {
    item.as_object().is_some_and(|o| o.contains_key("group"))
}

/// Every placed item with its path and its wrapper's attributes, in paint order (`walkItems`).
pub fn walk_items(scene: &Value) -> Vec<Walked> {
    fn walk(list: Option<&Value>, prefix: &[usize], wrapper: &Value, out: &mut Vec<Walked>) {
        let Some(list) = list.and_then(Value::as_array) else { return };
        for (i, item) in list.iter().enumerate() {
            let mut path = prefix.to_vec();
            path.push(i);
            if is_group(item) {
                let mut w = Map::new();
                if let Some(g) = item.get("group").and_then(Value::as_object) {
                    for (k, v) in g {
                        w.insert(k.clone(), v.clone());
                    }
                }
                if let Some(d) = item.get("depth") {
                    w.insert("depth".into(), d.clone());
                }
                walk(item.get("items"), &path, &Value::Object(w), out);
            } else if truthy(Some(item)) {
                // Paths as scene.items writes them: "12" at the top level, "5.2" inside a wrapper.
                let text = path.iter().map(usize::to_string).collect::<Vec<_>>().join(".");
                out.push(Walked { path: text, item: item.clone(), wrapper: wrapper.clone() });
            }
        }
    }
    let mut out = Vec::new();
    walk(scene.get("items"), &[], &Value::Null, &mut out);
    out
}

fn or_null(item: &Value, key: &str) -> Value {
    item.get(key).cloned().unwrap_or(Value::Null)
}

/// What makes two items the same item, wherever they are in the list (`contentKey`).
pub fn content_key(item: &Value, wrapper: &Value) -> String {
    let key = json!([
        or_null(item, "object"),
        or_null(item, "at"),
        or_null(item, "id"),
        or_null(item, "class"),
        or_null(item, "layer"),
        or_null(item, "depth"),
        wrapper
    ]);
    engine::format::stringify(&key)
}

/// Maps each working item's path to its saved path, or None when the saved
/// file has no such item (`matchSaved`).
pub fn match_saved(working: &Value, saved: &Value) -> OMap<String, Option<String>> {
    let mut saved_by_key: HashMap<String, Vec<String>> = HashMap::new();
    for w in walk_items(saved) {
        saved_by_key.entry(content_key(&w.item, &w.wrapper)).or_default().push(w.path);
    }
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out = OMap::new();
    for w in walk_items(working) {
        let key = content_key(&w.item, &w.wrapper);
        let k = *seen.get(&key).unwrap_or(&0);
        seen.insert(key.clone(), k + 1);
        out.set(w.path, saved_by_key.get(&key).and_then(|l| l.get(k)).cloned());
    }
    out
}

fn layer_set(item: &Value, set: &engine::omap::OSet<String>) -> bool {
    item.get("layer").and_then(Value::as_str).is_some_and(|l| set.has(l))
}

/// The items in a layer or inside it, as their content keys (sorted).
fn layer_keys(scene: &Value, id: &str) -> Vec<String> {
    let set = closure(scene, &[id.to_string()]);
    let mut keys: Vec<String> =
        walk_items(scene).iter().filter(|w| layer_set(&w.item, &set)).map(|w| content_key(&w.item, &w.wrapper)).collect();
    keys.sort();
    keys
}

fn join_values(list: &[Value]) -> String {
    list.iter().map(|v| if v.is_null() { String::new() } else { js_string(Some(v)) }).collect::<Vec<_>>().join(",")
}

/// "tile 3,-1,1", "px 31,6 level 0" (`atText`).
pub fn at_text(at: &Value) -> String {
    if let Some(tile) = at.get("tile").and_then(Value::as_array) {
        return format!("tile {}", join_values(tile));
    }
    if let Some(px) = at.get("px").and_then(Value::as_array) {
        let level = at.get("level").map(|l| format!(" level {}", js_string(Some(l)))).unwrap_or_default();
        return format!("px {}{level}", join_values(px));
    }
    engine::format::stringify(at)
}

/// A string in double quotes, inner quotes escaped.
fn quoted(s: &str) -> String {
    engine::format::stringify(&Value::String(s.to_string()))
}

fn quoted_value(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => quoted(s),
        Some(v) => engine::format::stringify(v),
        None => "undefined".into(),
    }
}

/// What `selectionContext` takes. `layers` and `items` are as the app has
/// them (working copy); `pinned` holds `layer:<id>` or paths.
pub struct SelectionInput<'a> {
    pub document: &'a str,
    pub scene: &'a str,
    pub version: &'a str,
    pub working: &'a Value,
    pub saved: &'a Value,
    pub layers: &'a [String],
    pub items: &'a [String],
    pub pinned: &'a [String],
}

fn unique(list: &[String]) -> Vec<&String> {
    let mut seen = HashSet::new();
    list.iter().filter(|s| seen.insert(s.as_str())).collect()
}

/// The block and what went into it (`selectionContext`).
pub fn selection_context(input: &SelectionInput) -> Result<Value> {
    let SelectionInput { document, scene, version, working, saved, layers, items, pinned } = input;
    let map = match_saved(working, saved);
    let working_items: HashMap<String, Walked> = walk_items(working).into_iter().map(|w| (w.path.clone(), w)).collect();
    let mut saved_layers: HashMap<&str, &Value> = HashMap::new();
    for l in layers_of(saved) {
        if let Some(id) = l.get("id").and_then(Value::as_str) {
            saved_layers.insert(id, l);
        }
    }
    let mut working_layers: HashMap<&str, &Value> = HashMap::new();
    for l in layers_of(working) {
        if let Some(id) = l.get("id").and_then(Value::as_str) {
            working_layers.insert(id, l);
        }
    }
    let pins: HashSet<&str> = pinned.iter().map(String::as_str).collect();
    for id in layers.iter() {
        if !working_layers.contains_key(id.as_str()) {
            return Err(CommandError::not_found(format!("no layer {} in {scene} (scene items lists them)", quoted(id))).into());
        }
    }
    for path in items.iter() {
        if !working_items.contains_key(path) {
            return Err(CommandError::not_found(format!("no item {path} in {scene} (scene items lists the paths)")).into());
        }
    }
    let mut unsaved: Vec<String> = Vec::new();
    let mut lines: Vec<String> = Vec::new();

    let saved_walk = walk_items(saved);
    let mut out_layers = Vec::new();
    for id in unique(layers) {
        let layer = working_layers.get(id.as_str()).copied();
        let on_disk = saved_layers.get(id.as_str()).copied();
        let ok = layer.is_some() && on_disk.is_some() && layer_keys(working, id) == layer_keys(saved, id);
        let paths: Vec<String> = if on_disk.is_some() {
            let set = closure(saved, std::slice::from_ref(id));
            saved_walk.iter().filter(|w| layer_set(&w.item, &set)).map(|w| w.path.clone()).collect()
        } else {
            Vec::new()
        };
        let name = [layer.and_then(|l| l.get("name")), on_disk.and_then(|l| l.get("name"))]
            .into_iter()
            .flatten()
            .find(|n| !n.is_null())
            .cloned()
            .unwrap_or_else(|| json!(id));
        out_layers.push(json!({ "id": id, "name": name, "items": if ok { json!(paths) } else { json!([]) }, "saved": ok }));
        if !ok {
            unsaved.push(format!("layer:{id}"));
            continue;
        }
        let shown = paths.iter().take(GROUP_PATHS).cloned().collect::<Vec<_>>().join(",");
        let more = if paths.len() > GROUP_PATHS {
            format!(" …and {} more (scene_items lists them)", paths.len() - GROUP_PATHS)
        } else {
            String::new()
        };
        let pin = if pins.contains(format!("layer:{id}").as_str()) { " pinned" } else { "" };
        lines.push(format!("  group {id} {} items={shown}{more}{pin}", quoted_value(on_disk.and_then(|l| l.get("name")))));
    }

    let mut out_items = Vec::new();
    for path in unique(items) {
        let entry = working_items.get(path.as_str());
        let saved_path = entry.and_then(|_| map.get(path).cloned().flatten());
        let item = entry.map(|e| &e.item);
        let mut o = Map::new();
        o.insert("path".into(), json!(path));
        o.insert("savedPath".into(), saved_path.as_ref().map_or(Value::Null, |p| json!(p)));
        o.insert("object".into(), item.map_or(Value::Null, |i| or_null(i, "object")));
        o.insert("at".into(), item.map_or(Value::Null, |i| or_null(i, "at")));
        if let Some(layer) = item.and_then(|i| i.get("layer")).filter(|l| truthy(Some(l))) {
            o.insert("layer".into(), layer.clone());
        }
        out_items.push(Value::Object(o));
        let (Some(saved_path), Some(item)) = (saved_path, item) else {
            unsaved.push(path.clone());
            continue;
        };
        let group = item.get("layer").filter(|l| truthy(Some(l))).map(|l| format!(" group={}", js_string(Some(l)))).unwrap_or_default();
        let pin = if pins.contains(path.as_str()) { " pinned" } else { "" };
        lines.push(format!(
            "  item {saved_path} object={} at={}{group}{pin}",
            js_string(item.get("object")),
            at_text(item.get("at").unwrap_or(&Value::Null))
        ));
    }

    if layers.is_empty() && items.is_empty() {
        lines.push("  (nothing selected: the whole scene is in view)".into());
    }
    if !unsaved.is_empty() {
        let what = |n: usize, word: &str| format!("{n} selected {word}{}", if n == 1 { "" } else { "s" });
        let nl = unsaved.iter().filter(|t| t.starts_with("layer:")).count();
        let ni = unsaved.len() - nl;
        let parts = [if nl > 0 { what(nl, "group") } else { String::new() }, if ni > 0 { what(ni, "item") } else { String::new() }]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" and ");
        let one = unsaved.len() == 1;
        lines.push(format!(
            "  ({parts} {} left out: {} saved yet)",
            if one { "is" } else { "are" },
            if one { "it isn't" } else { "they aren't" }
        ));
    }
    let head = format!("<selection document={} scene={} version={}>", quoted(document), quoted(scene), quoted(version));
    let mut all = vec![head];
    all.extend(lines);
    all.push("</selection>".into());
    Ok(json!({
        "document": document,
        "scene": scene,
        "version": version,
        "text": all.join("\n"),
        "layers": out_layers,
        "items": out_items,
        "unsaved": unsaved,
    }))
}

/// The block for an object (object-mode context is the whole object; spec, D4).
pub fn object_context(document: &str, object: &str, version: &str) -> Value {
    json!({
        "document": document,
        "object": object,
        "version": version,
        "text": format!("<selection document={} object={} version={}>\n  (the whole object is in view)\n</selection>", quoted(document), quoted(object), quoted(version)),
        "layers": [],
        "items": [],
        "unsaved": [],
    })
}
