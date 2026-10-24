//! Scene edits: placing, moving, setting, grouping and sorting items (ports cli/lib/scene-ops.mjs).
//!
//! Every function changes a scene document in place and returns what the
//! caller needs (a new path, a message). Paths are index lists into the
//! scene's items: `[5, 2]` is the third item of the group at 5. Functions
//! return a `scene_op_error` (with a stable code) where the lab would
//! announce that nothing could be done.
//!
//! The Node functions took `site` only to reach the engine modules; here they
//! call `engine::iso`, `engine::motion` and `engine::engine` directly.
//! Results that were objects are `serde_json::Value` objects with the same
//! keys in the same order. Where the Node result held the live item, this one
//! holds a copy of the item as the edit left it. `sortScene`, `stayed` and
//! `pathChanges` live with the session.

use serde_json::{json, Map, Value};

use engine::engine::{self as eng, is_class_list as engine_is_class_list, is_name, js_space, js_string, truthy, Sources};
use engine::jsnum;
use engine::serialize::{entries, keys, ordered};
use engine::{iso, motion};

use crate::errors::CommandError;

type Result<T> = std::result::Result<T, CommandError>;

/// Tile steps in scene pixels (iso: 32×16 tiles, 16-pixel levels), as `moveByTiles` uses them.
const HALF_W: f64 = 16.0;
const HALF_H: f64 = 8.0;
const LEVEL_H: f64 = 16.0;

/// A class list: lowercase class names separated by single spaces (`CLASS_LIST`).
pub fn is_class_list(s: &str) -> bool {
    engine_is_class_list(s)
}

/// An item id: lowercase kebab-case (`ID`).
pub fn is_id(s: &str) -> bool {
    is_name(s)
}

/// A group attribute's name: class or data-* (`GROUP_ATTR`).
pub fn is_group_attr(s: &str) -> bool {
    s == "class" || s.strip_prefix("data-").is_some_and(eng::is_class_token)
}

/// `new SceneOpError(message, code)`: a `CommandError` with the scene-op code
/// (`scene_op` when the Node call gave none).
pub fn scene_op_error(message: impl Into<String>, code: &str) -> CommandError {
    CommandError::new(message, code)
}

fn is_group(item: &Value) -> bool {
    item.as_object().is_some_and(|o| o.contains_key("group"))
}

/// `path.join('.')`.
pub fn show(path: &[usize]) -> String {
    path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(".")
}

fn path_value(path: &[usize]) -> Value {
    Value::Array(path.iter().map(|&i| Value::from(i)).collect())
}

fn num_at(v: Option<&Value>, i: usize) -> f64 {
    v.and_then(|v| v.get(i)).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

fn tile_of(item: &Value) -> Option<&Value> {
    item.get("at").and_then(|a| a.get("tile")).filter(|t| truthy(Some(t)))
}

/// `item.at.tile` truthy.
fn has_tile(item: &Value) -> bool {
    tile_of(item).is_some()
}

/// The list that holds the children of the groups at `dirs` (the scene's
/// items for no dirs).
fn list_mut<'a>(doc: &'a mut Value, dirs: &[usize]) -> Option<&'a mut Vec<Value>> {
    let mut list = doc.get_mut("items")?.as_array_mut()?;
    for &i in dirs {
        list = list.get_mut(i)?.get_mut("items")?.as_array_mut()?;
    }
    Some(list)
}

fn list_ref<'a>(doc: &'a Value, dirs: &[usize]) -> Option<&'a Vec<Value>> {
    let mut list = doc.get("items")?.as_array()?;
    for &i in dirs {
        list = list.get(i)?.get("items")?.as_array()?;
    }
    Some(list)
}

/// The list that holds the item at `path`, and its index there (`parentOf`).
/// The list is `None` where a step of the path leads nowhere; an empty path's
/// index is 0.
pub fn parent_of<'a>(doc: &'a mut Value, path: &[usize]) -> (Option<&'a mut Vec<Value>>, usize) {
    let index = path.last().copied().unwrap_or(0);
    let dirs = &path[..path.len().saturating_sub(1)];
    (list_mut(doc, dirs), index)
}

/// The item at `path`, or None (`itemAt`).
pub fn item_at<'a>(doc: &'a Value, path: &[usize]) -> Option<&'a Value> {
    let (&index, dirs) = path.split_last()?;
    list_ref(doc, dirs)?.get(index).filter(|v| !v.is_null())
}

/// `item_at`, mutable.
pub fn item_at_mut<'a>(doc: &'a mut Value, path: &[usize]) -> Option<&'a mut Value> {
    let (&index, dirs) = path.split_last()?;
    list_mut(doc, dirs)?.get_mut(index).filter(|v| !v.is_null())
}

/// The placed item at `path` (not a group), or a not_found / is_group error (`placedAt`).
pub fn placed_at<'a>(doc: &'a Value, path: &[usize]) -> Result<&'a Value> {
    let item = item_at(doc, path).ok_or_else(|| scene_op_error(format!("no item at {}", show(path)), "not_found"))?;
    if is_group(item) {
        return Err(scene_op_error(
            format!("{} is a group, not a placed item; give the path of an item inside it", show(path)),
            "is_group",
        ));
    }
    Ok(item)
}

/// `placed_at`, mutable.
pub fn placed_at_mut<'a>(doc: &'a mut Value, path: &[usize]) -> Result<&'a mut Value> {
    placed_at(doc, path)?;
    Ok(item_at_mut(doc, path).expect("checked"))
}

/// Where an item's placement point is, in scene pixels (`placementPoint`).
pub fn placement_point(item: &Value, origin: [f64; 2]) -> [f64; 2] {
    if let Some(tile) = tile_of(item) {
        return iso::tile_to_px([num_at(Some(tile), 0), num_at(Some(tile), 1), num_at(Some(tile), 2)], origin);
    }
    let px = item.get("at").and_then(|a| a.get("px"));
    let get = |i: usize| px.and_then(|p| p.get(i)).and_then(Value::as_f64).unwrap_or(0.0);
    [get(0), get(1)]
}

/// Sets a pixel-placed item's level, leaving the key out at 0.
fn set_level(at: &mut Value, level: f64) {
    if let Some(map) = at.as_object_mut() {
        if level != 0.0 && !level.is_nan() {
            map.insert("level".into(), jsnum::num(level));
        } else {
            map.shift_remove("level");
        }
    }
}

/// Moves an item by whole tiles and levels, however it's placed: a
/// pixel-placed item moves by the same screen offset, and its level by the
/// same levels (`moveByTiles`).
pub fn move_by_tiles(item: &mut Value, dc: f64, dr: f64, dl: f64) {
    let Some(at) = item.get_mut("at") else { return };
    if let Some(tile) = at.get("tile").filter(|t| truthy(Some(t))) {
        let moved =
            json!([jsnum::num(num_at(Some(tile), 0) + dc), jsnum::num(num_at(Some(tile), 1) + dr), jsnum::num(num_at(Some(tile), 2) + dl)]);
        at["tile"] = moved;
    } else {
        let px = at.get("px");
        let (x, y) = (num_at(px, 0), num_at(px, 1));
        let moved = json!([jsnum::num(x + (dc - dr) * HALF_W), jsnum::num(y + (dc + dr) * HALF_H - dl * LEVEL_H)]);
        let level = at.get("level").and_then(Value::as_f64).unwrap_or(0.0);
        at["px"] = moved;
        if dl != 0.0 && !dl.is_nan() {
            set_level(at, level + dl);
        }
    }
}

/// "column 3, row 4, level 0", "x 31, y 6" or, for a pixel item standing
/// above level 0, "x 40, y -12, level 4" (`spoken`).
pub fn spoken(item: &Value) -> String {
    if let Some(tile) = tile_of(item) {
        return format!("column {}, row {}, level {}", js_string(tile.get(0)), js_string(tile.get(1)), js_string(tile.get(2)));
    }
    let at = item.get("at");
    let px = at.and_then(|a| a.get("px"));
    let level = at.and_then(|a| a.get("level"));
    let tail = if truthy(level) { format!(", level {}", js_string(level)) } else { String::new() };
    format!("x {}, y {}{}", js_string(px.and_then(|p| p.get(0))), js_string(px.and_then(|p| p.get(1))), tail)
}

fn drop_track_of(doc: &mut Value, id: &Value) {
    if let Some(tracks) = doc.get_mut("motion").and_then(|m| m.get_mut("tracks")).and_then(Value::as_array_mut) {
        tracks.retain(|track| track.get("item") != Some(id));
    }
}

/// Takes an item's track out of the scene's motion (`dropTrack`).
pub fn drop_track(doc: &mut Value, id: &str) {
    drop_track_of(doc, &Value::String(id.to_string()));
}

/// An `at` for a new or moved item: a tile, or a pixel point with the level
/// it stands on (left out at 0) (`atOf`).
fn at_of(at: &Value) -> Value {
    if let Some(tile) = at.get("tile").filter(|t| truthy(Some(t))) {
        return json!({ "tile": tile.clone() });
    }
    let px = at.get("px").cloned().unwrap_or_else(|| json!([]));
    let mut out = Map::new();
    out.insert("px".into(), px);
    if let Some(level) = at.get("level").filter(|l| truthy(Some(l))) {
        out.insert("level".into(), level.clone());
    }
    Value::Object(out)
}

/// Places an object on a tile or at a pixel point, as the newest item of the
/// scene's list; the session's sort then puts it where it stands (`place`).
/// `at` is `{ tile }` or `{ px, level? }`. Returns `{ path, item, message }`.
pub fn place(doc: &mut Value, object: &str, at: &Value) -> Value {
    let item = json!({ "object": object, "at": at_of(at) });
    let message = format!("{object} placed at {}", spoken(&item));
    let list = doc.as_object_mut().map(|o| o.entry("items").or_insert_with(|| json!([]))).and_then(Value::as_array_mut);
    let path: Vec<usize> = match list {
        Some(list) => {
            list.push(item.clone());
            vec![list.len() - 1]
        }
        None => Vec::new(),
    };
    json!({ "path": path_value(&path), "item": item, "message": message })
}

/// Past time 0, moving an animated item moves it at that time: the key there,
/// made if there isn't one, takes the move. Returns None when the item isn't
/// animated (or time is 0), for the caller to move its placement; else the
/// message (`keyedMove`). `item` is a copy of the item, not borrowed from
/// `doc`. `time` is in ms, rounded as the lab rounds its pane time.
pub fn keyed_move(doc: &mut Value, item: &Value, dx: f64, dy: f64, time: f64) -> Option<String> {
    let t = jsnum::round(time);
    let id = item.get("id")?;
    let motion = doc.get_mut("motion").filter(|m| truthy(Some(m)))?;
    if t <= 0.0 || t.is_nan() {
        return None;
    }
    let duration = motion.get("duration").and_then(Value::as_f64).unwrap_or(f64::NAN);
    let tracks = motion.get_mut("tracks")?.as_array_mut()?;
    let ti = tracks.iter().position(|track| track.get("item") == Some(id))?;
    let object = js_string(item.get("object"));
    let start = motion::state_at(&tracks[ti], &object, duration, t).moved;
    let keys = tracks[ti].get_mut("keys")?.as_array_mut()?;
    let found = keys.iter().position(|k| k.get("t").and_then(Value::as_f64) == Some(t));
    let index = match found {
        Some(i) => i,
        None => {
            let before = keys.iter().filter(|k| k.get("t").and_then(Value::as_f64).is_some_and(|kt| kt < t)).count();
            keys.insert(before, json!({ "t": jsnum::num(t) }));
            before
        }
    };
    keys[index]["move"] = json!([jsnum::num(start[0] + dx), jsnum::num(start[1] + dy)]);
    Some(format!("{} moved at {} s", object, jsnum::to_fixed(t / 1000.0, 1)))
}

/// Moves an item by tiles and levels: past time 0 an animated item's key
/// moves instead (`moveItem`). Returns `{ path, item, message, keyed }`.
pub fn move_item(doc: &mut Value, path: &[usize], dc: f64, dr: f64, dl: f64, time: f64) -> Result<Value> {
    let item = placed_at(doc, path)?.clone();
    if let Some(message) = keyed_move(doc, &item, (dc - dr) * HALF_W, (dc + dr) * HALF_H - dl * LEVEL_H, time) {
        return Ok(json!({ "path": path_value(path), "item": item, "message": message, "keyed": true }));
    }
    let item = placed_at_mut(doc, path)?;
    move_by_tiles(item, dc, dr, dl);
    let item = item.clone();
    let message = format!("{} moved to {}", js_string(item.get("object")), spoken(&item));
    Ok(json!({ "path": path_value(path), "item": item, "message": message, "keyed": false }))
}

/// Nudges an item by pixels; an item on a tile is then placed by pixel
/// (`nudgeItem`). Returns `{ path, item, message, keyed }`.
pub fn nudge_item(doc: &mut Value, path: &[usize], dx: f64, dy: f64, time: f64) -> Result<Value> {
    let item = placed_at(doc, path)?.clone();
    if let Some(message) = keyed_move(doc, &item, dx, dy, time) {
        return Ok(json!({ "path": path_value(path), "item": item, "message": message, "keyed": true }));
    }
    let [x, y] = placement_point(&item, eng::origin_of(doc));
    // It keeps the level it stands on, so nudging it doesn't change what it stands behind.
    let level = match tile_of(&item) {
        Some(tile) => tile.get(2).cloned(),
        None => item.get("at").and_then(|a| a.get("level")).cloned(),
    };
    let mut target = json!({ "px": [jsnum::num(x + dx), jsnum::num(y + dy)] });
    if let Some(level) = level {
        target["level"] = level;
    }
    let item = placed_at_mut(doc, path)?;
    item["at"] = at_of(&target);
    let item = item.clone();
    let message = format!("{} nudged to {}", js_string(item.get("object")), spoken(&item));
    Ok(json!({ "path": path_value(path), "item": item, "message": message, "keyed": false }))
}

/// Sets one coordinate of an item: col, row or level of an item on a tile, x
/// or y of a pixel-placed one, or the level a pixel-placed one stands on
/// (`setField`). Where the lab ignores a field that doesn't match the
/// placement, this refuses. Returns `{ path, item, message }`.
pub fn set_field(doc: &mut Value, path: &[usize], field: &str, value: f64) -> Result<Value> {
    placed_at(doc, path)?;
    if !jsnum::is_integer(value) {
        return Err(scene_op_error(format!("{field} must be a whole number"), "usage"));
    }
    let item = placed_at_mut(doc, path)?;
    let object = js_string(item.get("object"));
    let on_tile = has_tile(item);
    let at = item.get_mut("at").ok_or_else(|| scene_op_error("an item has no at", "wrong_placement"))?;
    if field == "level" && at.get("px").is_some_and(|p| truthy(Some(p))) {
        set_level(at, value);
    } else {
        let (key, index) = match field {
            "col" => ("tile", 0),
            "row" => ("tile", 1),
            "level" => ("tile", 2),
            "x" => ("px", 0),
            "y" => ("px", 1),
            _ => ("", 0),
        };
        let list = if key.is_empty() { None } else { at.get_mut(key).filter(|l| truthy(Some(l))).and_then(Value::as_array_mut) };
        let Some(list) = list else {
            let placed = if on_tile { "on a tile (col, row, level)" } else { "by pixel (x, y, level)" };
            return Err(scene_op_error(
                format!("{object} at {} is placed {placed}, so it has no {field}; set tile or px to change how it's placed", show(path)),
                "wrong_placement",
            ));
        };
        if list.len() <= index {
            list.resize(index + 1, Value::Null);
        }
        list[index] = jsnum::num(value);
    }
    let item = item.clone();
    let message = format!("{} moved to {}", object, spoken(&item));
    Ok(json!({ "path": path_value(path), "item": item, "message": message }))
}

/// Places an item at an absolute tile or pixel point, replacing how it was
/// placed (`setAt`). `at` is `{ tile }` or `{ px, level? }`. Returns
/// `{ path, item, message }`.
pub fn set_at(doc: &mut Value, path: &[usize], at: &Value) -> Result<Value> {
    let item = placed_at_mut(doc, path)?;
    // A pixel point keeps the level the item already stands on, unless the caller gives one.
    let target = if truthy(at.get("tile")) {
        at.clone()
    } else {
        let level = at.get("level").filter(|l| !l.is_null()).or_else(|| item.get("at").and_then(|a| a.get("level"))).cloned();
        let mut t = json!({ "px": at.get("px").cloned().unwrap_or(Value::Null) });
        if let Some(level) = level {
            t["level"] = level;
        }
        t
    };
    item["at"] = at_of(&target);
    let item = item.clone();
    let message = format!("{} moved to {}", js_string(item.get("object")), spoken(&item));
    Ok(json!({ "path": path_value(path), "item": item, "message": message }))
}

/// Snaps a pixel-placed item to the tile under its placement point at the
/// level it stands on (level 0 unless it has one) (`snapItem`). Returns
/// `{ path, item, message }`.
pub fn snap_item(doc: &mut Value, path: &[usize]) -> Result<Value> {
    let origin = eng::origin_of(doc);
    let item = placed_at_mut(doc, path)?;
    let object = js_string(item.get("object"));
    if has_tile(item) {
        return Err(scene_op_error(format!("{object} is already on a tile"), "already_on_tile"));
    }
    let at = item.get("at");
    let px = at.and_then(|a| a.get("px"));
    let level = at.and_then(|a| a.get("level")).and_then(Value::as_f64).unwrap_or(0.0);
    let tile = iso::px_to_tile([num_at(px, 0), num_at(px, 1)], level, origin);
    item["at"] = json!({ "tile": [jsnum::num(tile[0]), jsnum::num(tile[1]), jsnum::num(tile[2])] });
    let item = item.clone();
    let message = format!("{} snapped to {}", object, spoken(&item));
    Ok(json!({ "path": path_value(path), "item": item, "message": message }))
}

/// Duplicates an item one column over, listed right after it until the
/// session's sort puts it where it stands. The copy takes no id, so no track
/// either (`duplicateItem`). Returns `{ path, item, message }`.
pub fn duplicate_item(doc: &mut Value, path: &[usize]) -> Result<Value> {
    let item = placed_at(doc, path)?.clone();
    let mut twin = item.clone();
    // An id names one item, so the copy doesn't take it, or its track.
    if let Some(map) = twin.as_object_mut() {
        map.shift_remove("id");
    }
    move_by_tiles(&mut twin, 1.0, 0.0, 0.0);
    let index = path[path.len() - 1] + 1;
    if let (Some(list), _) = parent_of(doc, path) {
        let at = index.min(list.len());
        list.insert(at, twin.clone());
    }
    let mut new_path = path[..path.len() - 1].to_vec();
    new_path.push(index);
    let message = format!("{} duplicated at {}", js_string(item.get("object")), spoken(&twin));
    Ok(json!({ "path": path_value(&new_path), "item": twin, "message": message }))
}

/// Removes an item, and its motion track if it has an id (`removeItem`).
/// Returns `{ path, item, message }`.
pub fn remove_item(doc: &mut Value, path: &[usize]) -> Result<Value> {
    let item = placed_at(doc, path)?.clone();
    if let (Some(list), index) = parent_of(doc, path) {
        if index < list.len() {
            list.remove(index);
        }
    }
    if let Some(id) = item.get("id") {
        if doc.get("motion").is_some_and(|m| truthy(Some(m))) {
            drop_track_of(doc, id);
        }
    }
    let message = format!("{} removed", js_string(item.get("object")));
    Ok(json!({ "path": path_value(path), "item": item, "message": message }))
}

/// Rebuilds an item's keys in the canonical order: object, id, class, layer,
/// depth, at (a layer stays as it was); keys outside that list go
/// (`reorderKeys`).
fn reorder_keys(item: &mut Value, id: Option<Value>, class: Option<Value>) {
    let Some(map) = item.as_object_mut() else { return };
    let (object, at, layer, depth, state) = (
        map.get("object").cloned(),
        map.get("at").cloned(),
        map.get("layer").cloned(),
        map.get("depth").cloned(),
        map.get("state").cloned(),
    );
    map.clear();
    for (key, value) in [("object", object), ("id", id), ("class", class), ("layer", layer), ("depth", depth), ("state", state), ("at", at)]
    {
        if let Some(value) = value {
            map.insert(key.into(), value);
        }
    }
}

/// What `set_depth` did; `say` words the message for a group by the path the
/// caller gives, since the sort that ends an edit can change it.
#[derive(Clone, Debug, PartialEq)]
pub struct DepthEdit {
    pub path: Vec<usize>,
    pub item: Value,
    pub message: String,
    /// The edited thing is a group.
    pub group: bool,
    /// 'front' or 'back', or None for auto.
    pub depth: Option<String>,
}

impl DepthEdit {
    /// The message, naming a group by `at` (`say`).
    pub fn say(&self, at: &str) -> String {
        let what = if self.group { format!("Group {at}") } else { js_string(self.item.get("object")) };
        match self.depth.as_deref() {
            None => format!("{what} draws by where it stands"),
            Some(d) => format!("{what} draws {} everything else in its list", if d == "front" { "in front of" } else { "behind" }),
        }
    }

    /// `{ path, item, message }`.
    pub fn to_json(&self) -> Value {
        json!({ "path": path_value(&self.path), "item": self.item, "message": self.message })
    }
}

/// Sets an item's or a group's depth override: 'front' draws it over
/// everything else in its list, 'back' under everything, and 'auto' takes the
/// override off, so it draws by where it stands. Keeps the canonical key
/// order (`setDepth`).
pub fn set_depth(doc: &mut Value, path: &[usize], depth: &str) -> Result<DepthEdit> {
    if item_at(doc, path).is_none() {
        return Err(scene_op_error(format!("no item at {}", show(path)), "not_found"));
    }
    if depth != "front" && depth != "back" && depth != "auto" {
        return Err(scene_op_error("depth is 'front', 'back' or 'auto'", "usage"));
    }
    let value = if depth == "auto" { None } else { Some(depth.to_string()) };
    let item = item_at_mut(doc, path).expect("checked");
    let group = is_group(item);
    if group {
        let map = item.as_object_mut().expect("group");
        let old = std::mem::take(map);
        let mut rebuilt = Map::new();
        rebuilt.insert("group".into(), old.get("group").cloned().unwrap_or(Value::Null));
        if let Some(v) = &value {
            rebuilt.insert("depth".into(), Value::String(v.clone()));
        }
        if let Some(items) = old.get("items") {
            rebuilt.insert("items".into(), items.clone());
        }
        for key in keys(&old) {
            if key != "group" && key != "items" && key != "depth" {
                rebuilt.insert(key.clone(), old[key].clone());
            }
        }
        *map = rebuilt;
    } else {
        let map = item.as_object_mut().expect("item");
        match &value {
            None => {
                map.shift_remove("depth");
            }
            Some(v) => {
                map.insert("depth".into(), Value::String(v.clone()));
            }
        }
        let (id, class) = (item.get("id").cloned(), item.get("class").cloned());
        reorder_keys(item, id, class);
    }
    let mut edit = DepthEdit { path: path.to_vec(), item: item.clone(), message: String::new(), group, depth: value };
    edit.message = edit.say(&show(path));
    Ok(edit)
}

/// Sets an item's class list, or removes it with '' (`setClass`), keeping the
/// canonical key order object, id, class, at. Returns `{ path, item, message }`.
pub fn set_class(doc: &mut Value, path: &[usize], value: &str) -> Result<Value> {
    placed_at(doc, path)?;
    let cls = value.trim_matches(js_space);
    if !cls.is_empty() && !is_class_list(cls) {
        return Err(scene_op_error("A class is lowercase class names separated by single spaces", "usage"));
    }
    let item = placed_at_mut(doc, path)?;
    let id = item.get("id").cloned();
    reorder_keys(item, id, if cls.is_empty() { None } else { Some(Value::String(cls.to_string())) });
    let message = if cls.is_empty() { "Class removed".to_string() } else { format!("Class set to {cls}") };
    Ok(json!({ "path": path_value(path), "item": item.clone(), "message": message }))
}

/// Sets an item's id, or removes it with '' (`setId`). Follows the engine's
/// rules (lowercase kebab-case, unique in the scene) and the lab's habits: the
/// canonical key order, and an item's track goes with its id (renamed with
/// it; dropped with it). Returns `{ path, item, message }`.
pub fn set_id(doc: &mut Value, path: &[usize], value: &str) -> Result<Value> {
    let current = placed_at(doc, path)?.get("id").cloned();
    let id = value.trim_matches(js_space);
    if !id.is_empty() && !is_id(id) {
        return Err(scene_op_error("An id is lowercase kebab-case, like crane-arm", "usage"));
    }
    let id_value = Value::String(id.to_string());
    if !id.is_empty() && current.as_ref() != Some(&id_value) && motion::items_by_id(doc.get("items").unwrap_or(&Value::Null)).has(id) {
        return Err(scene_op_error(format!("id '{id}' is already used in this scene"), "id_taken"));
    }
    if let Some(old) = &current {
        if doc.get("motion").is_some_and(|m| truthy(Some(m))) && old != &id_value {
            if !id.is_empty() {
                if let Some(tracks) = doc.get_mut("motion").and_then(|m| m.get_mut("tracks")).and_then(Value::as_array_mut) {
                    for track in tracks {
                        if track.get("item") == Some(old) {
                            track["item"] = id_value.clone();
                        }
                    }
                }
            } else {
                drop_track_of(doc, old);
            }
        }
    }
    let item = placed_at_mut(doc, path)?;
    let class = item.get("class").cloned();
    reorder_keys(item, if id.is_empty() { None } else { Some(id_value) }, class);
    let message = if id.is_empty() { "Id removed".to_string() } else { format!("Id set to {id}") };
    Ok(json!({ "path": path_value(path), "item": item.clone(), "message": message }))
}

/// Sets the state a placed item starts in, or clears it with None (the object's own start then): it's left out when it is the
/// object's start. Refuses a state the object doesn't have. Returns `{ path, item, message }`.
pub fn set_state(doc: &mut Value, path: &[usize], state: Option<&str>, sources: &Sources) -> Result<Value> {
    let object = js_string(placed_at(doc, path)?.get("object"));
    let states = eng::resolve(sources, &object).ok().and_then(|r| r.states);
    let message = match state {
        None => {
            placed_at_mut(doc, path)?.as_object_mut().expect("an item").shift_remove("state");
            format!("{object} starts in the state it has by default")
        }
        Some(name) => {
            let Some(states) = states else {
                return Err(scene_op_error(format!("{object} has no states (object.state.add makes the first)"), "no_states"));
            };
            if !states.has(name) {
                return Err(scene_op_error(format!("{object} has no state {name} (it has {})", states.names().join(", ")), "not_found"));
            }
            let item = placed_at_mut(doc, path)?.as_object_mut().expect("an item");
            if name == states.start {
                item.shift_remove("state");
                format!("{object} starts in {name}, the object's start")
            } else {
                item.insert("state".into(), Value::String(name.to_string()));
                format!("{object} starts in {name}")
            }
        }
    };
    let item = item_at(doc, path).cloned().unwrap_or(Value::Null);
    Ok(json!({ "path": path_value(path), "item": item, "message": message }))
}

/// `^\d+(\.\d+)*$`: a path like "12" or "5.2".
fn parse_path(text: &str) -> Option<Vec<usize>> {
    text.split('.')
        .map(|part| {
            if !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()) {
                Some(part.parse::<usize>().unwrap_or(usize::MAX))
            } else {
                None
            }
        })
        .collect()
}

fn find_id(list: &[Value], id: &str, prefix: &mut Vec<usize>) -> Option<Vec<usize>> {
    for (i, item) in list.iter().enumerate() {
        prefix.push(i);
        if is_group(item) {
            let children = item.get("items").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
            if let Some(found) = find_id(children, id, prefix) {
                return Some(found);
            }
        } else if item.get("id").and_then(Value::as_str) == Some(id) {
            return Some(prefix.clone());
        }
        prefix.pop();
    }
    None
}

/// Resolves an item reference: a path ("12", "5.2") or an item id (`findItem`).
pub fn find_item(doc: &Value, reference: &str) -> Result<Vec<usize>> {
    if let Some(path) = parse_path(reference) {
        return Ok(path);
    }
    let id = reference.strip_prefix('#').unwrap_or(reference);
    let items = doc.get("items").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    find_id(items, id, &mut Vec::new())
        .ok_or_else(|| scene_op_error(format!("no item with id {id}; give a path like 12 or 5.2, or an id"), "not_found"))
}

// Groups: the lab never edited groups; these follow the site's conventions
// for them: a group's data-x and data-y are its first item's foot, data-col
// and data-row its tile.

/// The path of a group: `reference` names the group itself (its path), or an
/// item inside it (a path or an id), whose innermost group it is
/// (`groupPathOf`).
pub fn group_path_of(doc: &Value, reference: &str) -> Result<Vec<usize>> {
    let path = if let Some(path) = parse_path(reference) {
        let at = item_at(doc, &path).ok_or_else(|| scene_op_error(format!("no item or group at {}", show(&path)), "not_found"))?;
        if is_group(at) {
            return Ok(path);
        }
        path
    } else {
        find_item(doc, reference)?
    };
    if path.len() < 2 {
        return Err(scene_op_error(format!("{reference} is at the top level, not inside a group"), "not_in_group"));
    }
    Ok(path[..path.len() - 1].to_vec())
}

/// An item's foot in scene pixels, as the site's entrance groups record it:
/// an item on a tile stands on its tile's top-face center (its placement
/// point); a sprite placed by pixel on the center of its bottom row
/// (`footOf`).
pub fn foot_of(sources: &Sources, item: &Value, origin: [f64; 2]) -> [f64; 2] {
    let [x, y] = placement_point(item, origin);
    if has_tile(item) {
        return [x, y];
    }
    let Ok(r) = eng::resolve(sources, &js_string(item.get("object"))) else { return [x, y] };
    if r.block.is_some() {
        return [x, y];
    }
    [jsnum::round(x - r.anchor[0] as f64 + r.width as f64 / 2.0), y - r.anchor[1] as f64 + r.height as f64 - 1.0]
}

/// Where an item stands, for `follow_group`: its point, foot and tile.
#[derive(Clone, Debug, PartialEq)]
pub struct Standing {
    pub point: [f64; 2],
    pub foot: [f64; 2],
    pub tile: Option<Vec<f64>>,
}

/// Where an item stands (`standing`).
pub fn standing(sources: &Sources, doc: &Value, item: &Value) -> Standing {
    let origin = eng::origin_of(doc);
    let tile = tile_of(item).map(|t| t.as_array().map(|a| a.iter().map(|v| v.as_f64().unwrap_or(f64::NAN)).collect()).unwrap_or_default());
    Standing { point: placement_point(item, origin), foot: foot_of(sources, item, origin), tile }
}

/// `int`: a string of digits (with an optional minus) as a number.
fn int(v: Option<&Value>) -> Option<f64> {
    let s = v?.as_str()?;
    let digits = s.strip_prefix('-').unwrap_or(s);
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        Some(jsnum::parse(s))
    } else {
        None
    }
}

/// Keeps the item's innermost group in step with a move: data-x and data-y
/// that were its placement point (or its foot) take the new one, and
/// data-col, data-row that were its tile take the new tile. Returns the
/// group's path and attributes (`{ path, attrs }`) when anything changed,
/// else None (`followGroup`).
pub fn follow_group(doc: &mut Value, path: &[usize], before: &Standing, after: &Standing) -> Option<Value> {
    if path.len() < 2 {
        return None;
    }
    let group_path = &path[..path.len() - 1];
    let group = item_at_mut(doc, group_path)?;
    if !is_group(group) {
        return None;
    }
    let attrs = group.get_mut("group").filter(|g| truthy(Some(g)))?.as_object_mut()?;
    let mut changed = false;
    if let (Some(gx), Some(gy)) = (int(attrs.get("data-x")), int(attrs.get("data-y"))) {
        let pick = if gx == before.point[0] && gy == before.point[1] {
            Some(after.point)
        } else if gx == before.foot[0] && gy == before.foot[1] {
            Some(after.foot)
        } else {
            None
        };
        if let Some(pick) = pick.filter(|p| p[0] != gx || p[1] != gy) {
            attrs.insert("data-x".into(), Value::String(jsnum::to_string(pick[0])));
            attrs.insert("data-y".into(), Value::String(jsnum::to_string(pick[1])));
            changed = true;
        }
    }
    if let (Some(gc), Some(gr)) = (int(attrs.get("data-col")), int(attrs.get("data-row"))) {
        if let (Some(b), Some(a)) = (&before.tile, &after.tile) {
            let get = |t: &Vec<f64>, i: usize| t.get(i).copied().unwrap_or(f64::NAN);
            if gc == get(b, 0) && gr == get(b, 1) && (gc != get(a, 0) || gr != get(a, 1)) {
                attrs.insert("data-col".into(), Value::String(jsnum::to_string(get(a, 0))));
                attrs.insert("data-row".into(), Value::String(jsnum::to_string(get(a, 1))));
                changed = true;
            }
        }
    }
    changed.then(|| json!({ "path": show(group_path), "attrs": Value::Object(ordered(attrs)) }))
}

/// Sets (or with '' removes) a group's attributes. New ones go after the
/// existing ones. Refuses names and values the engine would
/// (`setGroupAttrs`). Returns `{ path, group, message }`.
pub fn set_group_attrs(doc: &mut Value, path: &[usize], attrs: &Map<String, Value>) -> Result<Value> {
    let group = item_at_mut(doc, path)
        .filter(|g| is_group(g))
        .ok_or_else(|| scene_op_error(format!("{} is not a group", show(path)), "not_a_group"))?;
    for (name, value) in entries(attrs) {
        if !is_group_attr(name) {
            return Err(scene_op_error(
                format!(
                    "A group attribute is class or data-*, like data-enter; not {}",
                    engine::format::stringify(&Value::String(name.clone()))
                ),
                "usage",
            ));
        }
        if !matches!(value, Value::String(s) if !s.contains(['"', '<', '>', '&'])) {
            return Err(scene_op_error(format!("group attribute {name} must be a string without \" < > &"), "usage"));
        }
    }
    let mut current = match group.get("group") {
        Some(Value::Object(m)) => ordered(m),
        _ => Map::new(),
    };
    let mut said: Vec<String> = Vec::new();
    for (name, value) in entries(attrs) {
        let text = value.as_str().unwrap_or("");
        if text.is_empty() {
            if current.contains_key(name) {
                said.push(format!("{name} removed"));
            }
            current.shift_remove(name);
        } else {
            current.insert(name.clone(), value.clone());
            said.push(format!("{name} {text}"));
        }
    }
    group["group"] = Value::Object(current);
    let message = format!("Group {}: {}", show(path), if said.is_empty() { "unchanged".to_string() } else { said.join(", ") });
    Ok(json!({ "path": path_value(path), "group": group.clone(), "message": message }))
}

/// The first placed item in a list, depth first, with its path below
/// `prefix` (`firstPlaced`).
pub fn first_placed(list: &[Value], prefix: &[usize]) -> Option<(Value, Vec<usize>)> {
    for (i, item) in list.iter().enumerate() {
        let mut path = prefix.to_vec();
        path.push(i);
        if is_group(item) {
            let children = item.get("items").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
            if let Some(found) = first_placed(children, &path) {
                return Some(found);
            }
        } else if truthy(Some(item)) {
            return Some((item.clone(), path));
        }
    }
    None
}

/// Places an object inside a group: an existing one at `group`, and/or a new
/// one with `new_group` attributes. The item (or the new group) goes last in
/// the list it joins; the session's sort then puts it where it stands
/// (`placeInGroup`). Returns `{ path, group, item, message }`.
pub fn place_in_group(
    doc: &mut Value,
    object: &str,
    at: &Value,
    group: Option<&[usize]>,
    new_group: Option<&Map<String, Value>>,
) -> Result<Value> {
    let item = json!({ "object": object, "at": at_of(at) });
    if group.is_none() && new_group.is_none() {
        return Err(scene_op_error("placing into a group needs a group or a new group", "usage"));
    }
    if let Some(g) = group {
        if !item_at(doc, g).is_some_and(is_group) {
            return Err(scene_op_error(format!("{} is not a group", show(g)), "not_a_group"));
        }
        let target = item_at_mut(doc, g).expect("checked");
        if target.get("items").is_none_or(Value::is_null) {
            target["items"] = json!([]);
        }
    }
    let dirs = group.unwrap_or(&[]);
    let list = match group {
        Some(_) => list_mut(doc, dirs),
        None => doc.as_object_mut().map(|o| o.entry("items").or_insert_with(|| json!([]))).and_then(Value::as_array_mut),
    };
    let list = list.ok_or_else(|| scene_op_error("the scene has no items", "usage"))?;
    if let Some(attrs) = new_group {
        list.push(json!({ "group": Value::Object(ordered(attrs)), "items": [item.clone()] }));
        let mut gpath = dirs.to_vec();
        gpath.push(list.len() - 1);
        let mut path = gpath.clone();
        path.push(0);
        let message = format!("{object} placed at {} in a new group", spoken(&item));
        return Ok(json!({ "path": path_value(&path), "group": path_value(&gpath), "item": item, "message": message }));
    }
    list.push(item.clone());
    let mut path = dirs.to_vec();
    path.push(list.len() - 1);
    let message = format!("{object} placed at {} in group {}", spoken(&item), show(dirs));
    Ok(json!({ "path": path_value(&path), "group": path_value(dirs), "item": item, "message": message }))
}

// ----------------------------------------------------------- stacking (scene-canvas spec R16, R17)

/// Where an item rests: the level it stands at, and the object it rests on (None for the ground).
#[derive(Clone, Debug, PartialEq)]
pub struct Stand {
    pub level: i64,
    pub over: Option<String>,
}

/// Every placed item with its path, groups flattened, in list order.
fn placed_items<'a>(items: &'a [Value], dirs: &mut Vec<usize>, out: &mut Vec<(Vec<usize>, &'a Value)>) {
    for (i, item) in items.iter().enumerate() {
        dirs.push(i);
        if is_group(item) {
            if let Some(inner) = item.get("items").and_then(Value::as_array) {
                placed_items(inner, dirs, out);
            }
        } else {
            out.push((dirs.clone(), item));
        }
        dirs.pop();
    }
}

/// The level an item rests at over its column and row: the top of the highest block that covers any of the
/// tiles its own block size or footprint covers, in whole levels; level 0 (the ground) when none does. A sprite
/// stands at that level; a block of height h rests with its bottom on it, so its level (the level of its
/// top face) is that plus h. `ignore` leaves one item out (the one being dropped); `below` counts only
/// items whose top is no higher than that level (what is under an item, not on it).
pub fn stand_level(sources: &Sources, doc: &Value, item: &Value, ignore: Option<&[usize]>, below: Option<f64>) -> Stand {
    let origin = eng::origin_of(doc);
    let ground = Stand { level: 0, over: None };
    let Ok(me) = eng::item_box(sources, item, origin) else { return ground };
    let height = match eng::resolve(sources, &js_string(item.get("object"))) {
        Ok(r) => r.block.map(|b| b.size[2] as f64),
        Err(_) => None,
    };
    let mut found: Option<(f64, String)> = None;
    let mut all = Vec::new();
    placed_items(doc.get("items").and_then(Value::as_array).map_or(&[][..], |v| v.as_slice()), &mut Vec::new(), &mut all);
    for (path, other) in all {
        if ignore.is_some_and(|p| p == path.as_slice()) {
            continue;
        }
        // Only a block holds something up: a sprite standing on a tile (a pine, a pebble) is not a floor.
        if !eng::resolve(sources, &js_string(other.get("object"))).is_ok_and(|r| r.block.is_some()) {
            continue;
        }
        let Ok(o) = eng::item_box(sources, other, origin) else { continue };
        let covers = o.c0 < me.c1 && o.c1 > me.c0 && o.r0 < me.r1 && o.r1 > me.r0;
        if !covers || below.is_some_and(|limit| o.l1 > limit + 1e-9) {
            continue;
        }
        if found.as_ref().is_none_or(|(top, _)| o.l1 > *top) {
            found = Some((o.l1, js_string(other.get("object"))));
        }
    }
    match found {
        // Whole levels.
        Some((top, over)) => Stand { level: (top - 1e-9).ceil() as i64 + height.map_or(0, |h| h as i64), over: Some(over) },
        None => ground,
    }
}

/// Sets the level an item at `at` stands at: the third number of a tile, or a pixel item's `level`
/// (left out at 0).
pub fn put_level(at: &mut Value, level: i64) {
    if let Some(tile) = at.get_mut("tile").and_then(Value::as_array_mut) {
        while tile.len() < 3 {
            tile.push(json!(0));
        }
        tile[2] = json!(level);
    } else if let Some(map) = at.as_object_mut() {
        if level == 0 {
            map.shift_remove("level");
        } else {
            map.insert("level".into(), json!(level));
        }
    }
}

/// Drops an item onto whatever it stands over: down to the level where it rests on the top of the
/// block under it, or on the ground. Never up. Returns `{ path, item, message }`.
pub fn drop_item(sources: &Sources, doc: &mut Value, path: &[usize]) -> Result<Value> {
    let item = placed_at(doc, path)?.clone();
    let object = js_string(item.get("object"));
    let at = item.get("at").cloned().unwrap_or(Value::Null);
    let current = match tile_of(&item) {
        Some(tile) => num_at(Some(tile), 2),
        None => at.get("level").and_then(Value::as_f64).unwrap_or(0.0),
    };
    let current = if current.is_nan() { 0.0 } else { current };
    // What is under it: the item's own base is its level (a sprite), or its level less its height (a block).
    let base = eng::item_box(sources, &item, eng::origin_of(doc)).map_or(current, |b| b.l0.min(current));
    let stand = stand_level(sources, doc, &item, Some(path), Some(base));
    let level = (stand.level as f64).min(current) as i64;
    let moved = (level as f64) != current;
    if let Some(target) = item_at_mut(doc, path) {
        put_level(&mut target["at"], level);
    }
    let item = item_at(doc, path).cloned().unwrap_or(Value::Null);
    let rests = match &stand.over {
        Some(over) if level == stand.level => format!("on top of {over}"),
        _ => "on the ground".to_string(),
    };
    let message = if moved {
        format!("{object} dropped to level {level}, {rests}")
    } else {
        format!("{object} already rests at level {level}, {rests}")
    };
    Ok(json!({ "path": path_value(path), "item": item, "message": message, "over": stand.over, "changed": moved }))
}
