//! Scene layers: finding, making, renaming, ungrouping, assigning and moving them (ports cli/lib/layer-ops.mjs).
//!
//! A scene's `layers` list holds `{ id, name, parent? }`, and a placed item's
//! `layer` names the one it belongs to. They're bookkeeping only: nothing here
//! reorders the scene's items, and the `{ group, items }` wrappers are never
//! read or written. Like `scene_ops`, every function changes a scene document
//! in place and returns a `scene_op_error` with a stable code when nothing can
//! be done.
//!
//! Where the Node functions took `{ path, item }` members, these take the
//! members' paths and look the items up in the document (the Node items were
//! always live references into it). `Placed` carries a copy of an item as it
//! stood. Results that were objects are `serde_json::Value` objects with the
//! same keys in the same order.

use serde_json::{json, Map, Value};

use engine::engine::{self as eng, js_space, js_string, truthy};
use engine::format::stringify;
use engine::jsnum;
use engine::omap::OSet;
use engine::serialize::entries;

use crate::errors::CommandError;
use crate::scene_ops::{item_at, item_at_mut, scene_op_error, show};

type Result<T> = std::result::Result<T, CommandError>;

/// `LAYER_NAME`: 1–64 characters on one line, no spaces at either end.
pub fn is_layer_name(name: &str) -> bool {
    eng::is_layer_name(name)
}

/// `LAYER_ID`: lowercase kebab-case.
pub fn is_layer_id(id: &str) -> bool {
    eng::is_name(id)
}

fn is_group(item: &Value) -> bool {
    item.as_object().is_some_and(|o| o.contains_key("group"))
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// The scene's layers (an empty list when it has none).
pub fn layers_of(doc: &Value) -> &[Value] {
    doc.get("layers").and_then(Value::as_array).map_or(&[], |v| v.as_slice())
}

/// The last layer with this id (a JS `Map` built from the list keeps the last).
fn last_index(doc: &Value, id: &str) -> Option<usize> {
    layers_of(doc).iter().rposition(|l| str_of(l, "id") == Some(id))
}

/// A placed item of the scene with its path.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub path: Vec<usize>,
    pub item: Value,
}

/// Every placed item of the scene with its path, in paint order, wrappers looked through.
pub fn placed_items(doc: &Value) -> Vec<Placed> {
    fn walk(list: Option<&Value>, prefix: &[usize], out: &mut Vec<Placed>) {
        let Some(list) = list.and_then(Value::as_array) else { return };
        for (i, item) in list.iter().enumerate() {
            let mut path = prefix.to_vec();
            path.push(i);
            if is_group(item) {
                walk(item.get("items"), &path, out);
            } else if truthy(Some(item)) {
                out.push(Placed { path, item: item.clone() });
            }
        }
    }
    let mut out = Vec::new();
    walk(doc.get("items"), &[], &mut out);
    out
}

/// The index of a layer by its id, or by its name when only one layer has it.
pub fn find_layer_index(doc: &Value, reference: &str) -> Result<usize> {
    let layers = layers_of(doc);
    if let Some(i) = layers.iter().position(|l| str_of(l, "id") == Some(reference)) {
        return Ok(i);
    }
    let named: Vec<usize> = (0..layers.len()).filter(|&i| str_of(&layers[i], "name") == Some(reference)).collect();
    if named.len() == 1 {
        return Ok(named[0]);
    }
    let quoted = stringify(&Value::String(reference.to_string()));
    if named.len() > 1 {
        let ids: Vec<String> = named.iter().map(|&i| js_string(layers[i].get("id"))).collect();
        return Err(scene_op_error(
            format!("{} layers are called {}; give one's id ({})", named.len(), quoted, ids.join(", ")),
            "ambiguous",
        ));
    }
    let known: Vec<String> = layers.iter().map(|l| js_string(l.get("id"))).collect();
    let tail = if known.is_empty() { "; it has no layers yet".to_string() } else { format!(" (it has {})", known.join(", ")) };
    Err(scene_op_error(format!("no layer {quoted} in this scene{tail}"), "not_found"))
}

/// A layer by its id, or by its name when only one layer has it (a copy).
pub fn find_layer(doc: &Value, reference: &str) -> Result<Value> {
    Ok(layers_of(doc)[find_layer_index(doc, reference)?].clone())
}

/// The layers that hold a layer, outermost first, ending with the layer itself.
pub fn chain_of(doc: &Value, id: Option<&str>) -> Vec<String> {
    let mut chain: Vec<String> = Vec::new();
    let mut next = id.map(str::to_string);
    while let Some(cur) = next {
        let Some(i) = last_index(doc, &cur) else { break };
        if chain.contains(&cur) {
            break;
        }
        next = str_of(&layers_of(doc)[i], "parent").map(str::to_string);
        chain.insert(0, cur);
    }
    chain
}

/// A layer's id and the ids of every layer inside it.
pub fn with_inner(doc: &Value, id: &str) -> OSet<String> {
    let mut out = OSet::new();
    out.add(id.to_string());
    let mut grew = true;
    while grew {
        grew = false;
        for l in layers_of(doc) {
            let (Some(parent), Some(lid)) = (str_of(l, "parent"), str_of(l, "id")) else { continue };
            if out.has(parent) && !out.has(lid) {
                out.add(lid.to_string());
                grew = true;
            }
        }
    }
    out
}

/// Every layer id in `ids` and inside them.
pub fn closure(doc: &Value, ids: &[String]) -> OSet<String> {
    let mut out = OSet::new();
    for id in ids {
        for inner in with_inner(doc, id).iter() {
            out.add(inner.clone());
        }
    }
    out
}

/// The placed items in the given layers, or inside them, in paint order.
pub fn items_in_layers(doc: &Value, ids: &[String]) -> Vec<Placed> {
    let set = closure(doc, ids);
    placed_items(doc).into_iter().filter(|p| str_of(&p.item, "layer").is_some_and(|l| set.has(l))).collect()
}

/// Sets (or with `None` takes off) an item's layer, keeping the canonical key
/// order object, id, class, layer, depth, at.
pub fn set_item_layer(item: &mut Value, layer: Option<&str>) {
    let Some(map) = item.as_object_mut() else { return };
    let mut old = std::mem::take(map);
    let mut take = |key: &str| old.shift_remove(key);
    let (object, id, class, _old_layer, at, depth) = (take("object"), take("id"), take("class"), take("layer"), take("at"), take("depth"));
    if let Some(v) = object {
        map.insert("object".into(), v);
    }
    if let Some(v) = id {
        map.insert("id".into(), v);
    }
    if let Some(v) = class {
        map.insert("class".into(), v);
    }
    if let Some(l) = layer {
        map.insert("layer".into(), Value::String(l.to_string()));
    }
    if let Some(v) = depth {
        map.insert("depth".into(), v);
    }
    if let Some(v) = at {
        map.insert("at".into(), v);
    }
    for (k, v) in entries(&old) {
        map.insert(k.clone(), v.clone());
    }
}

/// Checks a layer name and returns it trimmed.
pub fn check_name(name: &Value) -> Result<String> {
    let trimmed = name.as_str().map_or("", |s| s.trim_matches(js_space));
    if !is_layer_name(trimmed) {
        return Err(scene_op_error("A layer name is 1 to 64 characters on one line, like Railway", "usage"));
    }
    Ok(trimmed.to_string())
}

/// What `normalize('NFKD')` leaves of one lowercase character, as far as the
/// `[^a-z0-9]` replacement after it can tell: ASCII stays, a letter with marks
/// is its base letter and then a separator (the marks), a compatibility
/// character is its expansion. Covers Latin-1, Latin Extended-A, the common
/// Latin Extended-B and Vietnamese letters, ligatures, super/subscript digits
/// and fullwidth forms; any other character is passed on (and ends up a
/// separator unless it is ASCII).
fn fold(c: char, out: &mut String) {
    let s: &str = match c {
        'à'..='å' | 'ā' | 'ă' | 'ą' | 'ǎ' | 'ǟ' | 'ǡ' | 'ǻ' | 'ȁ' | 'ȃ' | 'ȧ' | 'ḁ' | 'ạ'..='ặ' => "a ",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c ",
        'ď' | 'ḋ' | 'ḍ' | 'ḏ' | 'ḑ' | 'ḓ' => "d ",
        'è'..='ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' | 'ȅ' | 'ȇ' | 'ȩ' | 'ẹ'..='ệ' => "e ",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' | 'ǧ' | 'ǵ' => "g ",
        'ĥ' | 'ȟ' => "h ",
        'ì'..='ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ǐ' | 'ȉ' | 'ȋ' | 'ỉ' | 'ị' => "i ",
        'ĵ' | 'ǰ' => "j ",
        'ķ' | 'ǩ' => "k ",
        'ĺ' | 'ļ' | 'ľ' => "l ",
        'ñ' | 'ń' | 'ņ' | 'ň' | 'ǹ' => "n ",
        'ò'..='ö' | 'ō' | 'ŏ' | 'ő' | 'ơ' | 'ǒ' | 'ǫ' | 'ǭ' | 'ȍ' | 'ȏ' | 'ȯ' | 'ọ'..='ợ' => "o ",
        'ŕ' | 'ŗ' | 'ř' | 'ȑ' | 'ȓ' => "r ",
        'ś' | 'ŝ' | 'ş' | 'š' | 'ș' => "s ",
        'ţ' | 'ť' | 'ț' => "t ",
        'ù'..='ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' | 'ư' | 'ǔ' | 'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' | 'ȕ' | 'ȗ' | 'ụ'..='ự' => {
            "u "
        }
        'ŵ' => "w ",
        'ý' | 'ÿ' | 'ŷ' | 'ȳ' | 'ỳ' | 'ỵ' | 'ỷ' | 'ỹ' => "y ",
        'ź' | 'ż' | 'ž' => "z ",
        'ĳ' => "ij",
        'ŀ' => "l ",
        'ŉ' => " n",
        'ſ' => "s",
        'ª' => "a",
        'º' => "o",
        '\u{fb00}' => "ff",
        '\u{fb01}' => "fi",
        '\u{fb02}' => "fl",
        '\u{fb03}' => "ffi",
        '\u{fb04}' => "ffl",
        '\u{fb05}' | '\u{fb06}' => "st",
        '¹' | '₁' => "1",
        '²' | '₂' => "2",
        '³' | '₃' => "3",
        '\u{2070}' | '\u{2080}' => "0",
        '\u{2074}'..='\u{2079}' => {
            out.push(char::from_u32('4' as u32 + (c as u32 - 0x2074)).unwrap_or(' '));
            return;
        }
        '\u{2084}'..='\u{2089}' => {
            out.push(char::from_u32('4' as u32 + (c as u32 - 0x2084)).unwrap_or(' '));
            return;
        }
        '\u{ff10}'..='\u{ff19}' | '\u{ff41}'..='\u{ff5a}' => {
            out.push(char::from_u32(c as u32 - 0xff01 + 0x21).unwrap_or(' '));
            return;
        }
        _ => {
            out.push(c);
            return;
        }
    };
    out.push_str(s);
}

/// An id for a new layer, made from its name: lowercase, words joined by
/// dashes, `layer-…` when it doesn't start with a letter, and -2, -3… when it's taken.
pub fn id_for(doc: &Value, name: &str) -> String {
    let mut folded = String::new();
    for c in name.to_lowercase().chars() {
        fold(c, &mut folded);
    }
    let spaced: String = folded.chars().map(|c| if c.is_ascii_lowercase() || c.is_ascii_digit() { c } else { ' ' }).collect();
    let joined = spaced.split_whitespace().collect::<Vec<_>>().join("-");
    let mut base: String = joined.chars().take(40).collect();
    base.truncate(base.trim_end_matches('-').len());
    if !base.starts_with(|c: char| c.is_ascii_lowercase()) {
        base = if base.is_empty() { "layer".into() } else { format!("layer-{base}") };
    }
    let taken: Vec<&str> = layers_of(doc).iter().filter_map(|l| str_of(l, "id")).collect();
    if !taken.contains(&base.as_str()) {
        return base;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !taken.contains(&candidate.as_str()) {
            return candidate;
        }
        n += 1;
    }
}

/// `^<prefix>(\d+)$`: the number after a prefix, when only digits follow.
fn counted(text: &str, prefix: &str) -> Option<f64> {
    let digits = text.strip_prefix(prefix)?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<f64>().ok()
}

/// "Group n": one more than the highest n of the scene's "Group n" names and
/// group-n ids, so the count goes on after a group is renamed.
pub fn next_group_name(doc: &Value) -> String {
    let mut high = 0.0_f64;
    for l in layers_of(doc) {
        for n in
            [str_of(l, "name").and_then(|s| counted(s, "Group ")), str_of(l, "id").and_then(|s| counted(s, "group-"))].into_iter().flatten()
        {
            high = high.max(n);
        }
    }
    format!("Group {}", jsnum::to_string(high + 1.0))
}

/// The deepest layer that holds every one of `parents` (each a layer id, or
/// `None` for the top level), or `None`.
pub fn common_parent(doc: &Value, parents: &[Option<String>]) -> Option<String> {
    if parents.is_empty() {
        return None;
    }
    let chains: Vec<Vec<String>> = parents.iter().map(|p| p.as_deref().map_or_else(Vec::new, |p| chain_of(doc, Some(p)))).collect();
    let mut common = None;
    let mut i = 0;
    loop {
        let Some(id) = chains[0].get(i) else { return common };
        if !chains.iter().all(|c| c.get(i) == Some(id)) {
            return common;
        }
        common = Some(id.clone());
        i += 1;
    }
}

/// The layer an item is in (`None` for the top level), if that layer exists.
fn layer_of_item(doc: &Value, item: &Value) -> Option<String> {
    let layer = str_of(item, "layer")?;
    layers_of(doc).iter().any(|l| str_of(l, "id") == Some(layer)).then(|| layer.to_string())
}

fn no_item(path: &[usize]) -> CommandError {
    scene_op_error(format!("no item at {}", show(path)), "not_found")
}

/// Drops members that are inside another member: a layer inside a member
/// layer, and an item in (or inside) a member layer stay where they are.
fn top_members(doc: &Value, items: &[Vec<usize>], layers: &[String]) -> Result<(Vec<Vec<usize>>, Vec<String>)> {
    let mut unique: Vec<String> = Vec::new();
    for id in layers {
        if !unique.contains(id) {
            unique.push(id.clone());
        }
    }
    let kept_layers: Vec<String> = unique
        .iter()
        .filter(|id| {
            let chain = chain_of(doc, Some(id));
            !unique.iter().any(|other| other != *id && chain[..chain.len().saturating_sub(1)].contains(other))
        })
        .cloned()
        .collect();
    let inner = closure(doc, &kept_layers);
    let mut seen: Vec<String> = Vec::new();
    let mut kept_items = Vec::new();
    for path in items {
        let key = show(path);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let item = item_at(doc, path).ok_or_else(|| no_item(path))?;
        if layer_of_item(doc, item).is_none_or(|l| !inner.has(&l)) {
            kept_items.push(path.clone());
        }
    }
    Ok((kept_items, kept_layers))
}

/// The options of [`new_layer`].
#[derive(Clone, Debug, Default)]
pub struct NewLayer {
    /// The layer's name (a generated "Group n" when `None`).
    pub name: Option<Value>,
    /// Its id (made from the name when `None`).
    pub id: Option<String>,
    /// Paths of the items to move into it.
    pub items: Vec<Vec<usize>>,
    /// Ids of the layers to move into it.
    pub layers: Vec<String>,
    /// Its parent: `None` to work it out, `Some(None)` for the top level, `Some(Some(id))` for a layer.
    pub parent: Option<Option<String>>,
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Makes a layer and moves items and layers into it. Its parent is `parent`
/// (the top level for `Some(None)`), else the deepest layer that holds every
/// member, else the top level.
pub fn new_layer(doc: &mut Value, options: NewLayer) -> Result<Value> {
    let NewLayer { name, id, items, layers, parent } = options;
    let (member_items, member_layers) = top_members(doc, &items, &layers)?;
    let layer_name = match &name {
        None => next_group_name(doc),
        Some(n) => check_name(n)?,
    };
    if let Some(id) = &id {
        if !is_layer_id(id) {
            return Err(scene_op_error("A layer id is lowercase kebab-case, like railway", "usage"));
        }
        if layers_of(doc).iter().any(|l| str_of(l, "id") == Some(id)) {
            return Err(scene_op_error(format!("layer id {id} is already used in this scene"), "exists"));
        }
    }
    let new_id = id.unwrap_or_else(|| id_for(doc, &layer_name));
    let under: Option<String> = match parent {
        None => {
            let mut parents: Vec<Option<String>> = member_layers
                .iter()
                .map(|l| last_index(doc, l).and_then(|i| str_of(&layers_of(doc)[i], "parent")).map(str::to_string))
                .collect();
            for path in &member_items {
                parents.push(layer_of_item(doc, item_at(doc, path).ok_or_else(|| no_item(path))?));
            }
            common_parent(doc, &parents)
        }
        Some(under) => {
            if let Some(u) = &under {
                let chain = chain_of(doc, Some(u));
                if let Some(in_member) = member_layers.iter().find(|m| chain.contains(m)) {
                    return Err(scene_op_error(format!("the new layer can't go inside {in_member}, which it would hold"), "layer_cycle"));
                }
            }
            under
        }
    };
    let mut list: Vec<Value> = layers_of(doc).to_vec();
    let mut at = list.iter().position(|l| str_of(l, "id").is_some_and(|i| member_layers.iter().any(|m| m == i)));
    if at.is_none() {
        at = list.iter().position(|l| str_of(l, "parent").map(str::to_string) == under);
    }
    let mut layer = Map::new();
    layer.insert("id".into(), Value::String(new_id.clone()));
    layer.insert("name".into(), Value::String(layer_name.clone()));
    if let Some(u) = &under {
        layer.insert("parent".into(), Value::String(u.clone()));
    }
    let layer = Value::Object(layer);
    list.insert(at.unwrap_or(list.len()), layer.clone());
    for l in &mut list {
        if str_of(l, "id").is_some_and(|i| member_layers.iter().any(|m| m == i)) {
            if let Some(o) = l.as_object_mut() {
                o.insert("parent".into(), Value::String(new_id.clone()));
            }
        }
    }
    doc["layers"] = Value::Array(list);
    for path in &member_items {
        set_item_layer(item_at_mut(doc, path).ok_or_else(|| no_item(path))?, Some(&new_id));
    }
    let inside = items_in_layers(doc, std::slice::from_ref(&new_id)).len();
    let what = if member_layers.len() + member_items.len() == 0 {
        format!("Made an empty layer, {layer_name}")
    } else {
        format!("Grouped {inside} item{} as {layer_name}", plural(inside))
    };
    Ok(json!({
        "layer": layer,
        "items": member_items.len(),
        "layers": member_layers,
        "inside": inside,
        "message": what,
    }))
}

/// Renames the layer `reference` to `to`.
pub fn rename_layer(doc: &mut Value, reference: &str, to: &Value) -> Result<Value> {
    let index = find_layer_index(doc, reference)?;
    let name = check_name(to)?;
    let layer = &mut doc["layers"][index];
    let old = js_string(layer.get("name"));
    layer["name"] = Value::String(name.clone());
    let message = if old == name { format!("{name} is already called that") } else { format!("Renamed {old} to {name}") };
    Ok(json!({ "layer": layer.clone(), "message": message }))
}

/// Takes a layer away and moves what it held up one level: its layers and
/// items now belong to its parent, and stay where they are.
pub fn ungroup_layer(doc: &mut Value, reference: &str) -> Result<Value> {
    let index = find_layer_index(doc, reference)?;
    let layer = layers_of(doc)[index].clone();
    let layer_id = str_of(&layer, "id").map(str::to_string);
    let parent: Option<String> = str_of(&layer, "parent").map(str::to_string);
    let mut list: Vec<Value> = layers_of(doc).to_vec();
    list.remove(index);
    let mut moved_layers: Vec<String> = Vec::new();
    for l in &mut list {
        if str_of(l, "parent").map(str::to_string) != layer_id || layer_id.is_none() {
            continue;
        }
        if let Some(o) = l.as_object_mut() {
            match &parent {
                None => {
                    o.shift_remove("parent");
                }
                Some(p) => {
                    o.insert("parent".into(), Value::String(p.clone()));
                }
            }
        }
        moved_layers.push(js_string(l.get("id")));
    }
    let mut moved_items: Vec<String> = Vec::new();
    for Placed { path, item } in placed_items(doc) {
        if str_of(&item, "layer").map(str::to_string) != layer_id || layer_id.is_none() {
            continue;
        }
        if let Some(target) = item_at_mut(doc, &path) {
            set_item_layer(target, parent.as_deref());
        }
        moved_items.push(show(&path));
    }
    if let Some(map) = doc.as_object_mut() {
        if list.is_empty() {
            map.shift_remove("layers");
        } else {
            map.insert("layers".into(), Value::Array(list));
        }
    }
    let parent_name = match &parent {
        None => "the top level".to_string(),
        Some(p) => js_string(find_layer(doc, p)?.get("name")),
    };
    let n = moved_layers.len() + moved_items.len();
    let parts = if n == 1 { "its 1 part moved up to".to_string() } else { format!("its {n} parts moved up to") };
    let message = format!(
        "Ungrouped {}: {parts} {parent_name} and {}",
        js_string(layer.get("name")),
        if n == 1 { "stays where it is" } else { "stay where they are" }
    );
    Ok(json!({
        "parent": parent,
        "layers": moved_layers,
        "items": moved_items,
        "message": message,
    }))
}

/// Moves items and layers into a layer (`None` for the top level). Refuses to
/// put a layer inside itself, or inside a layer within it. A layer id or an
/// item path the scene doesn't have is a `not_found` error (the Node code
/// would have thrown a TypeError there).
pub fn assign_layer(doc: &mut Value, to: Option<&str>, items: &[Vec<usize>], layers: &[String]) -> Result<Value> {
    let chain = chain_of(doc, to);
    for id in layers {
        if chain.contains(id) {
            let message = if Some(id.as_str()) == to {
                format!("layer {id} can't go inside itself")
            } else {
                format!("layer {id} can't go inside {}, which is inside it", to.unwrap_or(""))
            };
            return Err(scene_op_error(message, "layer_cycle"));
        }
    }
    let mut unique_layers: Vec<String> = Vec::new();
    for id in layers {
        if !unique_layers.contains(id) {
            unique_layers.push(id.clone());
        }
    }
    for id in &unique_layers {
        let i = last_index(doc, id)
            .ok_or_else(|| scene_op_error(format!("no layer {} in this scene", stringify(&Value::String(id.clone()))), "not_found"))?;
        if let Some(o) = doc["layers"][i].as_object_mut() {
            match to {
                None => {
                    o.shift_remove("parent");
                }
                Some(t) => {
                    o.insert("parent".into(), Value::String(t.to_string()));
                }
            }
        }
    }
    let mut seen: Vec<String> = Vec::new();
    for path in items {
        let key = show(path);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        set_item_layer(item_at_mut(doc, path).ok_or_else(|| no_item(path))?, to);
    }
    let where_ = match to {
        None => "the top level".to_string(),
        Some(t) => {
            let i = last_index(doc, t).ok_or_else(|| {
                scene_op_error(format!("no layer {} in this scene", stringify(&Value::String(t.to_string()))), "not_found")
            })?;
            js_string(layers_of(doc)[i].get("name"))
        }
    };
    let mut said: Vec<String> = Vec::new();
    if !seen.is_empty() {
        said.push(format!("{} item{}", seen.len(), plural(seen.len())));
    }
    if !unique_layers.is_empty() {
        said.push(format!("{} layer{}", unique_layers.len(), plural(unique_layers.len())));
    }
    let message =
        if said.is_empty() { format!("Nothing to move into {where_}") } else { format!("Moved {} into {where_}", said.join(" and ")) };
    Ok(json!({
        "to": to,
        "items": seen.len(),
        "layers": unique_layers,
        "message": message,
    }))
}
