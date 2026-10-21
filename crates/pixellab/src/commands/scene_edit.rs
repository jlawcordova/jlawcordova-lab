//! The scene editing commands: scene.place, scene.move, scene.set and the rest (ports cli/lib/commands/scene-edit.mjs).
//!
//! The edits themselves are `scene_ops`; these commands find the scene and
//! the item and run one edit as one undo step through `ctx.edit`. Moving an
//! item at rest keeps its group's data-x/data-y in step. Every scene edit
//! ends with the session putting the items in depth order, so a command
//! reports the paths it names after that sort, and `reordered` when it moved
//! any.

use engine::engine::{self as eng, composite, js_len, render_scene, truthy, CompositeOptions, Order, Sources};
use engine::serialize::keys;
use serde_json::{json, Map, Value};

use super::common::{at_most_one, flag, format_change, need_one, obj, opt_str, str_of};
use super::scene::{find_layer, set_item_layer};
use crate::context::{Ctx, DocKind};
use crate::errors::{CommandError, Error, Result};
use crate::registry::{Command, Params};
use crate::render::path_string;
use crate::scene_ops as ops;
use crate::session::Edit;
use crate::sorting::{remap_json, sort_scene};

fn name_param() -> Value {
    json!({ "type": "string", "description": "scene name (or scenes/<name>)" })
}

fn item_param() -> Value {
    json!({ "type": "string", "description": "the item: its path from scene items (12, 5.2) or its id" })
}

fn tile_param() -> Value {
    json!({ "type": "array", "items": { "type": "integer" }, "minItems": 3, "maxItems": 3, "description": "a tile: col,row,level" })
}

fn px_param() -> Value {
    json!({ "type": "array", "items": { "type": "integer" }, "minItems": 2, "maxItems": 2, "description": "a point in scene pixels: x,y" })
}

fn keep_group_param() -> Value {
    json!({
        "type": "boolean",
        "default": true,
        "description": "keep the item's group in step: data-x/data-y that were its placement point or foot, and data-col/data-row that were its tile, follow it"
    })
}

/// `itemParams`: name and item, then `more`.
fn item_params(more: Vec<(&str, Value)>) -> Value {
    let mut props = Map::new();
    props.insert("name".into(), name_param());
    props.insert("item".into(), item_param());
    for (k, v) in more {
        props.insert(k.into(), v);
    }
    json!({ "type": "object", "properties": Value::Object(props), "required": ["name", "item"] })
}

fn text<'a>(params: &'a Params, key: &str) -> &'a str {
    opt_str(params, key).unwrap_or("")
}

fn given(params: &Params, key: &str) -> bool {
    params.get(key).is_some_and(|v| !v.is_null())
}

fn nums(params: &Params, key: &str) -> Option<Vec<f64>> {
    params.get(key)?.as_array().map(|a| a.iter().map(|v| v.as_f64().unwrap_or(f64::NAN)).collect())
}

fn num_or(params: &Params, key: &str, default: f64) -> f64 {
    params.get(key).and_then(Value::as_f64).unwrap_or(default)
}

/// The path an op returned (`{ path: [..] }`).
fn path_of(r: &Value) -> Vec<usize> {
    r["path"].as_array().map(|a| a.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect()).unwrap_or_default()
}

/// The scene's name, checked.
fn scene_name(ctx: &mut Ctx, query: &str) -> Result<String> {
    let name = ctx.find(query, Some(DocKind::Scene))?.name;
    ctx.doc(DocKind::Scene, &name)?;
    Ok(name)
}

/// Runs one item edit on a scene: finds the item by path or id, calls `op`
/// with the working copy and the item's path, and reports the item's new path.
/// `op` returns `{ path, item, message }`.
fn item_edit(ctx: &mut Ctx, params: &Params, op: impl FnOnce(&mut Value, &[usize]) -> Result<Value>, key: Option<&str>) -> Result<Value> {
    let name = scene_name(ctx, text(params, "name"))?;
    let reference = text(params, "item").to_string();
    ctx.edit(DocKind::Scene, &name, key, |doc, _| {
        let path = ops::find_item(doc, &reference)?;
        let r = op(doc, &path)?;
        let result = obj(vec![("path", json!(path_string(&path_of(&r)))), ("item", r["item"].clone())]);
        Ok(Edit::new(result, str_of(r.get("message"))))
    })
}

/// An item edit that moves the item at rest: like `item_edit`, then its
/// innermost group's attributes follow it (`follow_group`) unless keepGroup
/// is false or the move was keyed past time 0.
fn move_edit(ctx: &mut Ctx, params: &Params, op: impl FnOnce(&mut Value, &[usize]) -> Result<Value>, key: Option<&str>) -> Result<Value> {
    let name = scene_name(ctx, text(params, "name"))?;
    let reference = text(params, "item").to_string();
    let keep_group = params.get("keepGroup").and_then(Value::as_bool) != Some(false);
    ctx.edit(DocKind::Scene, &name, key, |doc, loaded| {
        let sources = &loaded.sources;
        let path = ops::find_item(doc, &reference)?;
        let before = ops::standing(sources, doc, ops::placed_at(doc, &path)?);
        let r = op(doc, &path)?;
        let keyed = r.get("keyed").and_then(Value::as_bool) == Some(true);
        let new_path = path_of(&r);
        let group =
            if keep_group && !keyed { ops::follow_group(doc, &new_path, &before, &ops::standing(sources, doc, &r["item"])) } else { None };
        let mut message = str_of(r.get("message"));
        if let Some(g) = &group {
            let followed: Vec<&str> = g["attrs"]
                .as_object()
                .map(|a| {
                    keys(a)
                        .into_iter()
                        .map(String::as_str)
                        .filter(|k| matches!(*k, "data-x" | "data-y" | "data-col" | "data-row"))
                        .collect()
                })
                .unwrap_or_default();
            message = format!("{message}; its group's {} followed", followed.join(", "));
        }
        let mut result = obj(vec![("path", json!(path_string(&new_path))), ("item", r["item"].clone())]);
        if let Some(g) = group {
            result.insert("group".into(), g);
        }
        Ok(Edit::new(result, message))
    })
}

// ---- scene.place ----

fn place_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    at_most_one(&params, &["at", "px"], "scene.place")?;
    if given(&params, "level") && !given(&params, "px") {
        return Err(CommandError::usage("scene.place: --level goes with --px; an item on a tile has its level in --at").into());
    }
    if flag(&params, "foot") && !given(&params, "newGroup") {
        return Err(CommandError::usage("scene.place: --foot goes with --new-group").into());
    }
    let name = scene_name(ctx, text(&params, "name"))?;
    let object = text(&params, "object").to_string();
    if !ctx.names(DocKind::Object)?.contains(&object) {
        return Err(CommandError::not_found(format!("no object named {object} (pixellab object list)")).into());
    }
    ctx.edit(DocKind::Scene, &name, None, |doc, loaded| {
        let layer = match opt_str(&params, "layer") {
            Some(l) => Some(find_layer(doc, l)?),
            None => None,
        };
        let at = if given(&params, "px") {
            let mut at = obj(vec![("px", params["px"].clone())]);
            if let Some(level) = params.get("level").filter(|l| !l.is_null()) {
                at.insert("level".into(), level.clone());
            }
            Value::Object(at)
        } else if given(&params, "at") {
            // col,row,level: a level given is used as given. col,row alone leaves the level to stacking.
            let mut tile = params["at"].as_array().cloned().unwrap_or_default();
            tile.truncate(3);
            while tile.len() < 3 {
                tile.push(json!(0));
            }
            json!({ "tile": tile })
        } else {
            // SceneMode/Lab.open: the cursor starts on the tile at the middle of the view. A scene without a
            // viewBox has no view: it starts at the middle of what is in it now, or the origin tile when empty.
            let middle = match doc.get("viewBox").and_then(Value::as_array) {
                Some(vb) => {
                    let n = |i: usize| vb.get(i).and_then(Value::as_f64).unwrap_or(f64::NAN);
                    Some([(n(0) + n(2) / 2.0).floor(), (n(1) + n(3) / 2.0).floor()])
                }
                None => {
                    let mut probe = loaded.sources.clone();
                    probe.scenes.insert(name.clone(), doc.clone());
                    let painted = eng::resolve_frame(&probe, &name).ok().and_then(|f| f.painted);
                    painted.map(|[x, y, w, h]| [(x as f64 + w as f64 / 2.0).floor(), (y as f64 + h as f64 / 2.0).floor()])
                }
            };
            match middle {
                Some(px) => {
                    let tile = engine::iso::px_to_tile(px, 0.0, eng::origin_of(doc));
                    json!({ "tile": tile.iter().map(|&x| engine::jsnum::num(x)).collect::<Vec<_>>() })
                }
                None => json!({ "tile": [0, 0, 0] }),
            }
        };
        // Stacking (scene-canvas spec R17): no level given (a tile as col,row, or no position at all) puts
        // the item on top of what stands there; --on-top says so for a level or a pixel point too.
        let level_given = if given(&params, "px") {
            true
        } else if given(&params, "at") {
            params["at"].as_array().is_some_and(|a| a.len() >= 3)
        } else {
            false
        };
        let mut at = at;
        let mut over: Option<String> = None;
        if flag(&params, "onTop") || !level_given {
            let probe = json!({ "object": object, "at": at });
            let stand = ops::stand_level(&loaded.sources, doc, &probe, None, None);
            ops::put_level(&mut at, stand.level);
            over = stand.over;
        }
        let new_group = params.get("newGroup").and_then(Value::as_object).filter(|_| given(&params, "newGroup"));
        let r = if given(&params, "group") || new_group.is_some() {
            let group = match opt_str(&params, "group") {
                Some(g) => Some(ops::group_path_of(doc, g)?),
                None => None,
            };
            if let Some(attrs) = new_group {
                // Checks the attributes as a group would.
                let mut probe = json!({ "items": [{ "group": {}, "items": [] }] });
                ops::set_group_attrs(&mut probe, &[0], attrs)?;
            }
            ops::place_in_group(doc, &object, &at, group.as_deref(), new_group)?
        } else {
            ops::place(doc, &object, &at)
        };
        let path = path_of(&r);
        let item_group = r
            .get("group")
            .filter(|g| !g.is_null())
            .map(|g| g.as_array().map(|a| a.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect::<Vec<_>>()).unwrap_or_default());
        if let Some(class) = opt_str(&params, "class").filter(|c| !c.is_empty()) {
            ops::set_class(doc, &path, class)?;
        }
        if let Some(id) = opt_str(&params, "id").filter(|c| !c.is_empty()) {
            ops::set_id(doc, &path, id)?;
        }
        if let Some(layer) = &layer {
            if let Some(item) = ops::item_at_mut(doc, &path) {
                set_item_layer(item, layer.get("id").and_then(Value::as_str));
            }
        }
        if let (true, Some(g)) = (flag(&params, "foot"), &item_group) {
            let item = ops::item_at(doc, &path).cloned().unwrap_or(Value::Null);
            let [fx, fy] = ops::foot_of(&loaded.sources, &item, eng::origin_of(doc));
            let attrs = obj(vec![("data-x", json!(engine::jsnum::to_string(fx))), ("data-y", json!(engine::jsnum::to_string(fy)))]);
            ops::set_group_attrs(doc, g, &attrs)?;
        }
        let message = match &layer {
            Some(l) => format!("{} in {}", str_of(r.get("message")), str_of(l.get("name"))),
            None => str_of(r.get("message")),
        };
        let message = match &over {
            Some(o) => format!("{message}, on top of {o}"),
            None => message,
        };
        let mut result = obj(vec![("path", json!(path_string(&path))), ("item", ops::item_at(doc, &path).cloned().unwrap_or(Value::Null))]);
        if let Some(g) = &item_group {
            let attrs = ops::item_at(doc, g).and_then(|i| i.get("group")).cloned().unwrap_or(Value::Null);
            result.insert("group".into(), json!({ "path": path_string(g), "attrs": attrs }));
        }
        Ok(Edit::new(result, message))
    })
}

// ---- scene.move ----

fn move_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    need_one(&params, &["by", "nudge"], "scene.move")?;
    at_most_one(&params, &["by", "nudge"], "scene.move")?;
    let time = num_or(&params, "time", 0.0);
    let by = nums(&params, "by");
    let nudge = nums(&params, "nudge");
    let key = format!(
        "scene.move:{}:{}:{}",
        text(&params, "item"),
        if given(&params, "by") { "by" } else { "nudge" },
        engine::jsnum::to_string(time)
    );
    move_edit(
        ctx,
        &params,
        |doc, path| {
            Ok(match (&by, &nudge) {
                (Some(by), _) => ops::move_item(doc, path, by[0], by[1], by.get(2).copied().unwrap_or(0.0), time)?,
                (None, Some(nudge)) => ops::nudge_item(doc, path, nudge[0], nudge[1], time)?,
                (None, None) => Value::Null,
            })
        },
        Some(&key),
    )
}

// ---- scene.set ----

const FIELDS: [&str; 5] = ["col", "row", "level", "x", "y"];

fn set_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let mut wanted: Vec<&str> = vec!["tile", "px"];
    wanted.extend(FIELDS);
    wanted.extend(["depth", "class", "id"]);
    need_one(&params, &wanted, "scene.set")?;
    at_most_one(&params, &["tile", "px"], "scene.set")?;
    let others = FIELDS.iter().chain(["tile", "px", "class", "id"].iter()).any(|k| given(&params, k));
    if given(&params, "depth") && !others {
        // The override is the one field a group takes.
        // A group is named by its path, which the sort that ends the edit can change: say it with the path the result returns.
        let name = scene_name(ctx, text(&params, "name"))?;
        let reference = text(&params, "item").to_string();
        let depth = text(&params, "depth").to_string();
        return ctx.edit(DocKind::Scene, &name, None, |doc, _| {
            let path = ops::find_item(doc, &reference)?;
            let r = ops::set_depth(doc, &path, &depth)?;
            let result = obj(vec![("path", json!(path_string(&r.path))), ("item", r.item.clone())]);
            Ok(Edit::later(result, move |result| r.say(&str_of(result.get("path")))))
        });
    }
    match set_edit(ctx, &params) {
        // Only the override applies to a group, and the rest is scene.group.set's.
        Err(Error::Command(e)) if e.code == "is_group" => Err(CommandError::usage(format!(
            "scene.set: {} is a group; only --depth applies to one (its attributes are scene.group.set)",
            text(&params, "item")
        ))
        .into()),
        other => other,
    }
}

/// scene.set's edit of a placed item.
fn set_edit(ctx: &mut Ctx, params: &Params) -> Result<Value> {
    move_edit(
        ctx,
        params,
        |doc, path| {
            let mut said: Vec<String> = Vec::new();
            let mut r: Option<Value> = None;
            let has_tile = given(params, "tile");
            let has_px = given(params, "px");
            if has_tile || has_px {
                let at = if has_tile { json!({ "tile": params["tile"].clone() }) } else { json!({ "px": params["px"].clone() }) };
                let set = ops::set_at(doc, path, &at)?;
                said.push(str_of(set.get("message")));
                r = Some(set);
            }
            for f in FIELDS {
                if let Some(v) = params.get(f).filter(|v| !v.is_null()) {
                    r = Some(ops::set_field(doc, path, f, v.as_f64().unwrap_or(f64::NAN))?);
                }
            }
            if let (Some(r), false, false) = (&r, has_tile, has_px) {
                said.push(str_of(r.get("message")));
            }
            if let Some(d) = opt_str(params, "depth") {
                said.push(ops::set_depth(doc, path, d)?.message);
            }
            if let Some(c) = opt_str(params, "class") {
                said.push(str_of(ops::set_class(doc, path, c)?.get("message")));
            }
            if let Some(i) = opt_str(params, "id") {
                said.push(str_of(ops::set_id(doc, path, i)?.get("message")));
            }
            Ok(
                json!({ "path": path.to_vec(), "item": ops::item_at(doc, path).cloned().unwrap_or(Value::Null), "message": said.join("; ") }),
            )
        },
        None,
    )
}

// ---- scene.duplicate, scene.remove, scene.snap ----

fn duplicate_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    item_edit(ctx, &params, |doc, path| Ok(ops::duplicate_item(doc, path)?), None)
}

fn remove_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    item_edit(ctx, &params, |doc, path| Ok(ops::remove_item(doc, path)?), None)
}

fn snap_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    move_edit(ctx, &params, |doc, path| Ok(ops::snap_item(doc, path)?), None)
}

// ---- scene.sort ----

/// How many pixels differ, at time 0, between a scene drawn in the order its
/// file lists its items and in depth order (what the site sees when it adopts
/// this engine: the published render no longer depends on the list's order).
fn changed_pixels(sources: &Sources, name: &str) -> engine::Result<usize> {
    let draw = |order: Order| -> engine::Result<_> {
        let rendered = render_scene(sources, name, order)?;
        Ok(composite(&rendered.root, CompositeOptions { first_variant: true, frame: 0, time: Some(0.0), play: None }))
    };
    let (was, now) = (draw(Order::List)?, draw(Order::Depth)?);
    let mut n = 0;
    for (p, color) in now.iter() {
        if was.get(p) != Some(color) {
            n += 1;
        }
    }
    for p in was.keys() {
        if !now.has(p) {
            n += 1;
        }
    }
    Ok(n)
}

fn sort_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let all = flag(&params, "all");
    if all && given(&params, "name") {
        return Err(CommandError::usage("scene.sort: give a scene or --all, not both").into());
    }
    if !all && !given(&params, "name") {
        return Err(CommandError::usage("scene.sort: give a scene, or --all for every scene").into());
    }
    let names = if all { ctx.names(DocKind::Scene)? } else { vec![scene_name(ctx, text(&params, "name"))?] };
    let mut moved: Vec<Value> = Vec::new();
    // (scene, moved, changedPixels)
    let mut scenes: Vec<(String, usize, usize)> = Vec::new();
    let mut docs: Vec<String> = Vec::new();
    let mut reordered = Value::Object(Map::new());
    for name in &names {
        let mut slot: Option<(crate::sorting::Sorted, usize)> = None;
        let r = ctx.edit(DocKind::Scene, name, None, |doc, loaded| {
            // a scene that doesn't draw (a missing object) can't be compared; the sort leaves it alone
            let pixels = changed_pixels(&loaded.sources, name).unwrap_or(0);
            let sorted = sort_scene(&loaded.sources, doc);
            let n = sorted.moved.len();
            let message = format!("Sorted {name} by depth: {n} item{} moved", if n == 1 { "" } else { "s" });
            slot = Some((sorted, pixels));
            Ok(Edit::new(Map::new(), message))
        })?;
        let (sorted, pixels) = slot.expect("the edit ran");
        if r["changed"] == json!(true) {
            docs.push(str_of(r.get("doc")));
        }
        reordered = remap_json(&sorted.remap);
        for m in &sorted.moved {
            let mut entry = obj(vec![("scene", json!(name))]);
            if let Value::Object(m) = m {
                entry.extend(m.clone());
            }
            moved.push(Value::Object(entry));
        }
        scenes.push((name.clone(), sorted.moved.len(), pixels));
    }
    let total = moved.len();
    let pixels: usize = scenes.iter().map(|s| s.2).sum();
    let sorted_scenes: Vec<&(String, usize, usize)> = scenes.iter().filter(|s| s.1 > 0).collect();
    let s = |n: usize| if n == 1 { "" } else { "s" };
    let summary = if total > 0 {
        format!(
            "Sorted {} by depth: {total} item{} moved, {pixels} pixel{} change{}",
            if sorted_scenes.len() == 1 { sorted_scenes[0].0.clone() } else { format!("{} scenes", sorted_scenes.len()) },
            s(total),
            s(pixels),
            if pixels == 1 { "s" } else { "" }
        )
    } else {
        format!("Already in depth order{}", if all { String::new() } else { format!(": {}", names[0]) })
    };
    let mut out = obj(vec![
        ("changed", json!(!docs.is_empty())),
        ("summary", json!(summary)),
        ("moved", Value::Array(moved)),
        ("changedPixels", json!(pixels)),
        (
            "scenes",
            Value::Array(
                scenes.iter().map(|(scene, moved, pixels)| json!({ "scene": scene, "moved": moved, "changedPixels": pixels })).collect(),
            ),
        ),
    ]);
    // Every path the sort changed, for a caller holding paths into the one scene sorted.
    if !all {
        out.insert("reordered".into(), reordered);
    }
    Ok(Value::Object(out))
}

fn sort_format(r: &Value, _: &Params) -> String {
    let mut lines = vec![str_of(r.get("summary"))];
    for s in r["scenes"].as_array().into_iter().flatten() {
        if truthy(s.get("moved")) {
            lines.push(format!(
                "{}: {} moved, {} pixels change",
                str_of(s.get("scene")),
                str_of(s.get("moved")),
                str_of(s.get("changedPixels"))
            ));
        }
    }
    lines.join("\n")
}

// ---- scene.new ----

/// Refuses a name that isn't kebab-case, is too long, or is taken by any
/// object or scene (`checkNewName`).
pub fn check_new_name(ctx: &mut Ctx, name: &str, command: &str) -> Result<()> {
    let max = engine::starter::MAX_NAME;
    if js_len(name) > max {
        return Err(CommandError::usage(format!("{command}: name is {} characters long, max {max}", js_len(name))).into());
    }
    if !eng::is_name(name) {
        return Err(CommandError::usage(format!(
            "{command}: name {} must be lowercase kebab-case, like small-rock",
            engine::format::stringify(&Value::String(name.to_string()))
        ))
        .into());
    }
    let taken = ctx.names(DocKind::Object)?.iter().any(|n| n == name)
        || ctx.names(DocKind::Scene)?.iter().any(|n| n == name)
        || ctx.load_error(DocKind::Object, name)?.is_some_and(|e| !e.is_empty())
        || ctx.load_error(DocKind::Scene, name)?.is_some_and(|e| !e.is_empty());
    if taken {
        return Err(CommandError::new(format!("there's already an object or scene called {name}"), "exists").into());
    }
    Ok(())
}

fn new_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let name = text(&params, "name").to_string();
    check_new_name(ctx, &name, "scene.new")?;
    let label = format!("Created scene {name}");
    ctx.session()?.create(DocKind::Scene, &name, engine::starter::starter_scene(), &label);
    Ok(Value::Object(obj(vec![
        ("doc", json!(format!("scene:{name}"))),
        ("changed", json!(true)),
        ("summary", json!(label)),
        ("file", json!(engine::format::scene_file(&name))),
    ])))
}

// ---- scene.group.set ----

fn group_set_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    if !given(&params, "attrs") && !flag(&params, "foot") {
        return Err(CommandError::usage("scene.group.set: give --attrs, --foot or both").into());
    }
    let name = scene_name(ctx, text(&params, "name"))?;
    ctx.edit(DocKind::Scene, &name, None, |doc, loaded| {
        let path = ops::group_path_of(doc, text(&params, "group"))?;
        let mut attrs = params.get("attrs").and_then(Value::as_object).cloned().unwrap_or_default();
        if flag(&params, "foot") {
            let list = ops::item_at(doc, &path).and_then(|g| g.get("items")).and_then(Value::as_array).cloned().unwrap_or_default();
            let Some((first, _)) = ops::first_placed(&list, &[]) else {
                return Err(
                    CommandError::new(format!("group {} has no items to take a foot from", path_string(&path)), "empty_group").into()
                );
            };
            let [fx, fy] = ops::foot_of(&loaded.sources, &first, eng::origin_of(doc));
            attrs.insert("data-x".into(), json!(engine::jsnum::to_string(fx)));
            attrs.insert("data-y".into(), json!(engine::jsnum::to_string(fy)));
        }
        let r = ops::set_group_attrs(doc, &path, &attrs)?;
        let result = obj(vec![("group", json!({ "path": path_string(&path), "attrs": r["group"]["group"].clone() }))]);
        Ok(Edit::new(result, str_of(r.get("message"))))
    })
}

// ---- scene.drop ----

fn drop_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let loaded = ctx.sources()?;
    move_edit(ctx, &params, |doc, path| Ok(ops::drop_item(&loaded.sources, doc, path)?), None)
}

// ---- scene.frame ----

/// The frame a scene would publish at with this source, as result fields (scene-canvas spec R7, R8).
fn frame_fields(sources: &Sources, name: &str, scene: &Value) -> Map<String, Value> {
    let mut probe = sources.clone();
    probe.scenes.insert(name.to_string(), scene.clone());
    let frame = eng::resolve_frame(&probe, name);
    let own = scene.get("viewBox").and_then(Value::as_array).cloned();
    let fixed = own.is_some();
    let view_box = match (&frame, own) {
        (_, Some(vb)) => Some(Value::Array(vb)),
        (Ok(f), None) => Some(json!(f.view_box)),
        (Err(_), None) => None,
    };
    let mut out = Map::new();
    out.insert("scene".into(), json!(name));
    out.insert("frame".into(), json!(if fixed { "fixed" } else { "fit" }));
    if let Some(vb) = view_box {
        out.insert("size".into(), json!([vb.get(2).cloned().unwrap_or(Value::Null), vb.get(3).cloned().unwrap_or(Value::Null)]));
        out.insert("viewBox".into(), vb);
    }
    out.insert("margin".into(), scene.get("margin").cloned().unwrap_or_else(|| json!(0)));
    out
}

fn size_phrase(fields: &Map<String, Value>) -> String {
    let size = match fields.get("size").and_then(Value::as_array) {
        Some(s) => format!("{}×{}", str_of(s.first()), str_of(s.get(1))),
        None => "?".to_string(),
    };
    let vb = fields.get("viewBox").map(|v| format!(" at {}", str_of(v.get(0)) + ", " + &str_of(v.get(1)))).unwrap_or_default();
    if fields.get("frame").and_then(Value::as_str) == Some("fixed") {
        format!("fixed {size}{vb}")
    } else {
        format!("fits its content: {size}{vb}, margin {}", str_of(fields.get("margin")))
    }
}

fn frame_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let fit = flag(&params, "fit");
    if fit && given(&params, "box") {
        return Err(CommandError::usage("scene.frame: give only one of --box, --fit").into());
    }
    let name = scene_name(ctx, text(&params, "name"))?;
    let new_box: Option<Vec<i64>> = nums(&params, "box").map(|b| b.iter().map(|&n| n as i64).collect());
    if let Some(b) = &new_box {
        if b.len() != 4 || b[2] <= 0 || b[3] <= 0 {
            return Err(CommandError::usage("scene.frame: --box must be x,y,w,h with w and h above 0").into());
        }
    }
    let margin = params.get("margin").and_then(Value::as_i64);
    if margin.is_some_and(|m| m < 0) {
        return Err(CommandError::usage("scene.frame: --margin must be 0 or more").into());
    }
    if new_box.is_none() && !fit && margin.is_none() {
        // No option: report the frame.
        let loaded = ctx.sources()?;
        let scene = ctx.doc(DocKind::Scene, &name)?;
        let mut out = frame_fields(&loaded.sources, &name, &scene);
        out.insert("changed".into(), json!(false));
        out.insert("summary".into(), json!(format!("{name}: {}", size_phrase(&out))));
        return Ok(Value::Object(out));
    }
    ctx.edit(DocKind::Scene, &name, None, |doc, loaded| {
        let map = doc.as_object_mut().ok_or_else(|| Error::Internal("a scene is an object".into()))?;
        if let Some(b) = &new_box {
            map.insert("viewBox".into(), json!(b));
        }
        if fit {
            map.shift_remove("viewBox");
        }
        if let Some(m) = margin {
            if m == 0 {
                map.shift_remove("margin");
            } else {
                map.insert("margin".into(), json!(m));
            }
        }
        let mut out = frame_fields(&loaded.sources, &name, doc);
        let message = format!("{name}: {}", size_phrase(&out));
        out.insert("changed".into(), json!(true));
        Ok(Edit::new(out, message))
    })
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut place_props = Map::new();
    for (k, v) in [
        ("name", name_param()),
        ("object", json!({ "type": "string", "description": "the object to place" })),
        (
            "at",
            json!({ "type": "array", "items": { "type": "integer" }, "minItems": 2, "maxItems": 3, "description": "a tile: col,row[,level]; without a level the item goes on top of what stands there" }),
        ),
        ("px", px_param()),
        ("level", json!({ "type": "integer", "description": "with --px: how high the item stands, for depth order (default 0)" })),
        (
            "onTop",
            json!({ "type": "boolean", "default": false, "description": "put the item on top of the block on its tile, whatever level --at or --px give" }),
        ),
        ("class", json!({ "type": "string", "description": "a class list for the item" })),
        ("id", json!({ "type": "string", "description": "an id for the item (lowercase kebab-case, unique in the scene)" })),
        (
            "group",
            json!({ "type": "string", "description": "an existing group to place it in: its path, or an item (path or id) inside it" }),
        ),
        (
            "newGroup",
            json!({ "type": "object", "description": "attributes of a new group around the item (class, data-*), as JSON on the command line" }),
        ),
        (
            "foot",
            json!({ "type": "boolean", "default": false, "description": "with --new-group: set its data-x and data-y to the item's foot" }),
        ),
        (
            "layer",
            json!({ "type": "string", "description": "a layer of the scene for the new item to join: its id, or its name (scene items lists them)" }),
        ),
    ] {
        place_props.insert(k.into(), v);
    }
    let mut place = Command::new(
        "scene.place",
        "Place an object on a tile (or at a pixel point) in a scene",
        json!({ "type": "object", "properties": Value::Object(place_props), "required": ["name", "object"] }),
        true,
        place_run,
    );
    place.description = Some(concat!(
        "Like the lab's Place tool, but the item goes where it stands: it is added to its list and the scene's items are put in depth order, ",
        "so it draws behind what is in front of it and over what is behind (scene.items shows the order). With neither --at nor --px, ",
        "it goes on the tile at the middle of the view, where the lab's cursor starts (the middle of the scene's content when it has no viewBox). ",
        "With no level given (--at col,row, or no position) it stands on top of whatever block covers that tile: on the highest one there, ",
        "in whole levels, or on the ground (level 0); a block of height h rests with its bottom on that top. --at col,row,level is used as given ",
        "unless --on-top is set, which also works for --px. --px takes --level, how high the item stands ",
        "(default 0), which is only for depth: its pixel point is where its foot is drawn. ",
        "--group puts it inside an existing group (the group's path, or an item inside it); --new-group wraps it in a new group with those attributes ",
        r#"(class and data-*, like {"class": "enter", "data-enter": "object"}), and --foot sets that group's data-x and data-y to the item's foot. "#,
        "--layer puts it in a scene layer (its id or name; layers are apart from groups and never change paint order). ",
        "Returns the new item's path after the sort, and its group's; reordered says which other paths the sort moved."
    ));
    place.args = vec!["name", "object"];
    place.examples = vec![
        "pixellab scene place hero-island pine --at 14,6,1",
        "pixellab scene place range-sprite boxes --px 40,12 --dry-run",
        "pixellab scene place hero-island crane-jib --px 40,-12 --level 4",
        r#"pixellab scene place hero-island crate --at 3,1,1 --new-group '{"class": "enter", "data-enter": "object"}' --foot"#,
        "pixellab scene place hero-island pine --at 3,1,1 --layer railway",
    ];
    place.format = Some(format_change);

    let mut mv = Command::new(
        "scene.move",
        "Move an item by tiles and levels (--by) or by pixels (--nudge)",
        item_params(vec![
            ("keepGroup", keep_group_param()),
            (
                "by",
                json!({ "type": "array", "items": { "type": "integer" }, "minItems": 2, "maxItems": 3, "description": "tiles to move: col,row[,level]" }),
            ),
            (
                "nudge",
                json!({ "type": "array", "items": { "type": "integer" }, "minItems": 2, "maxItems": 2, "description": "pixels to move: x,y" }),
            ),
            (
                "time",
                json!({ "type": "integer", "minimum": 0, "default": 0, "description": "ms into the scene's motion; past 0 an animated item's key moves" }),
            ),
        ]),
        true,
        move_run,
    );
    mv.description = Some(concat!(
        "As the lab's arrow keys: --by col,row[,level] moves an item on a tile by whole tiles (a pixel-placed item moves by the same screen offset, ",
        "and its level by the same levels, so it stands over the same tile a level higher); ",
        "--nudge x,y moves it by pixels, and an item on a tile is then placed by pixel at the level it stood on. With --time past 0, an animated item (one with an id ",
        "and a motion track) moves at that time instead: the key there, made if needed, takes the move. Rapid repeats coalesce into one undo step. ",
        "A move at rest keeps the item's group in step: data-x/data-y that were its placement point or foot, and data-col/data-row that were its tile, ",
        "follow it (--no-keep-group leaves them); the result has the group when they did. The scene is then put in depth order: the result has the item's path after that, ",
        "and reordered says which other paths moved."
    ));
    mv.args = vec!["name", "item"];
    mv.examples = vec!["pixellab scene move hero-island 0.0 --by 1,0", "pixellab scene move hero-island jib --nudge 0,-1 --dry-run"];
    mv.format = Some(format_change);

    let mut set = Command::new(
        "scene.set",
        "Set an item's tile or pixel point, one coordinate, its level, depth override, class or id",
        item_params(vec![
            ("keepGroup", keep_group_param()),
            ("tile", tile_param()),
            ("px", px_param()),
            ("col", json!({ "type": "integer" })),
            ("row", json!({ "type": "integer" })),
            (
                "level",
                json!({ "type": "integer", "description": "the level of an item on a tile, or how high a pixel-placed item stands" }),
            ),
            ("x", json!({ "type": "integer" })),
            ("y", json!({ "type": "integer" })),
            (
                "depth",
                json!({ "type": "string", "enum": ["front", "back", "auto"], "description": "front or back draws it over or under everything else in its list; auto clears the override" }),
            ),
            ("class", json!({ "type": "string", "description": "a class list, or '' to remove it" })),
            ("id", json!({ "type": "string", "description": "an id, or '' to remove it" })),
        ]),
        true,
        set_run,
    );
    set.description = Some(concat!(
        "--tile col,row,level or --px x,y place the item there (changing how it's placed if need be; a pixel point keeps the level the item stands on); ",
        "--col, --row, --level (an item on a tile) and --x, --y (a pixel-placed item) set one coordinate, like the lab's Selected fields. ",
        "--level also sets how high a pixel-placed item stands, which only matters for depth order. ",
        "--depth is the override: front draws the item over everything else in its list, back under everything, auto takes the override off ",
        "so it draws by where it stands. It also takes a group's path, as the only field (a group's attributes are scene.group.set). ",
        "--class sets the class list and --id the id; an empty value removes it. An id's motion track follows it when it's renamed and goes with it ",
        "when it's removed. All in one undo step, after which the scene is in depth order (the result has the item's path after that; reordered says what moved). ",
        "Moving it keeps its group's data-x/data-y and data-col/data-row in step, as scene.move does (--no-keep-group leaves them)."
    ));
    set.args = vec!["name", "item"];
    set.examples = vec![
        "pixellab scene set hero-island 0.0 --tile 3,4,0",
        "pixellab scene set hero-island 0.0 --class 'sway slow'",
        "pixellab scene set hero-island 140 --depth back",
    ];
    set.format = Some(format_change);

    let mut duplicate = Command::new(
        "scene.duplicate",
        "Duplicate an item one column over, where the copy stands in depth order",
        item_params(vec![]),
        true,
        duplicate_run,
    );
    duplicate.description = Some(concat!(
        "As the lab's Duplicate: the copy is moved one column (a tile, or the same screen offset by pixel) and takes no id, so no motion track. ",
        "It is listed where it sorts, which is usually next to the original; the result has its path after the sort."
    ));
    duplicate.args = vec!["name", "item"];
    duplicate.format = Some(format_change);

    let mut remove =
        Command::new("scene.remove", "Remove an item (and its motion track, if it has an id)", item_params(vec![]), true, remove_run);
    remove.args = vec!["name", "item"];
    remove.format = Some(format_change);

    let mut snap = Command::new(
        "scene.snap",
        "Snap a pixel-placed item to the tile under its placement point, at the level it stands on",
        item_params(vec![("keepGroup", keep_group_param())]),
        true,
        snap_run,
    );
    snap.description = Some(
        "A pixel item with no level snaps to a level-0 tile. Its group's data-x/data-y and data-col/data-row follow it, as scene.move keeps them (--no-keep-group leaves them).",
    );
    snap.args = vec!["name", "item"];
    snap.format = Some(format_change);

    let mut sort = Command::new(
        "scene.sort",
        "Put a scene's items (or every scene's) in depth order, and say what moved",
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "scene name (or scenes/<name>); leave out with --all" },
                "all": { "type": "boolean", "default": false, "description": "sort every scene" }
            }
        }),
        true,
        sort_run,
    );
    sort.description = Some(concat!(
        "Every edit to a scene already keeps its items in depth order (back to front, by where each stands); this is for a scene whose file isn't, ",
        "such as one written by hand or made before depth order. A group sorts as one object and its members sort inside it. One undo step per scene. ",
        "Returns { moved: [{ scene, path, to, object | group }], changedPixels, scenes: [{ scene, moved, changedPixels }] }: moved lists the items the sort took out ",
        "of their place (paths before and after), and changedPixels counts the pixels that differ at time 0 between the scene drawn in its file's list order ",
        "(as engines before depth order publish it) and in depth order, which is what the site sees when it adopts this engine. ",
        "Give a scene or --all. For one scene, reordered maps every path the sort changed to its new path. ",
        "With --dry-run on the command line nothing is written and the diff shows the new order."
    ));
    sort.args = vec!["name"];
    sort.examples = vec!["pixellab scene sort hero-island --dry-run", "pixellab scene sort --all"];
    sort.format = Some(sort_format);

    let mut create = Command::new(
        "scene.new",
        "Make a new, empty scene (the starter: a 128×128 view around tile 0,0,0)",
        json!({
            "type": "object",
            "properties": { "name": { "type": "string", "description": "the new scene's name (lowercase kebab-case)" } },
            "required": ["name"]
        }),
        true,
        new_run,
    );
    create.args = vec!["name"];
    create.format = Some(format_change);

    let mut group_set = Command::new(
        "scene.group.set",
        "Set or remove a group's class and data attributes, or set its data-x and data-y to its first item's foot",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "group": { "type": "string", "description": "the group: its path, or an item (path or id) inside it" },
                "attrs": { "type": "object", "description": "attribute values; '' removes one (JSON on the command line)" },
                "foot": { "type": "boolean", "default": false, "description": "set data-x and data-y to the first item's foot" }
            },
            "required": ["name", "group"]
        }),
        true,
        group_set_run,
    );
    group_set.description = Some(concat!(
        "--group is the group's path (scene items lists groups), or an item (path or id) inside it. --attrs is an object of attribute values ",
        r#"({"data-enter": "object", "data-x": "96"}); '' removes one. --foot sets data-x and data-y to the foot of the group's first item: "#,
        "an item on a tile stands on its tile's center, a sprite placed by pixel on the center of its bottom row. One undo step."
    ));
    group_set.args = vec!["name", "group"];
    group_set.examples = vec![
        "pixellab scene group set hero-island 42 --foot",
        r#"pixellab scene group set hero-island truck-a --attrs '{"data-enter": "object"}'"#,
    ];
    group_set.format = Some(format_change);

    let mut drop = Command::new(
        "scene.drop",
        "Drop an item onto whatever it stands over: down to the top of the block under it, or the ground",
        item_params(vec![("keepGroup", keep_group_param())]),
        true,
        drop_run,
    );
    drop.description = Some(concat!(
        "As the lab's Drop key: the item's level becomes the one where it rests on the highest block under its column and row (a block of height h ",
        "rests with its bottom on it), or level 0, the ground, when nothing is under it. It never moves up; an item already resting stays. ",
        "One undo step. The item's group's data-x/data-y follow, as scene.move keeps them (--no-keep-group leaves them), and the scene is put in depth order."
    ));
    drop.args = vec!["name", "item"];
    drop.examples = vec!["pixellab scene drop hero-island 12", "pixellab scene drop hero-island crate --dry-run"];
    drop.format = Some(format_change);

    let mut frame = Command::new(
        "scene.frame",
        "Show a scene's published size, fix it (--box), fit it to its content (--fit) or set its margin",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "box": { "type": "array", "items": { "type": "integer" }, "minItems": 4, "maxItems": 4, "description": "fix the size: x,y,w,h in scene pixels (w and h above 0)" },
                "fit": { "type": "boolean", "default": false, "description": "take the fixed size off, so the scene publishes at the box that fits what is in it" },
                "margin": { "type": "integer", "minimum": 0, "description": "pixels left around the painted pixels of a fit scene (0 or more; 0 takes it off)" }
            },
            "required": ["name"]
        }),
        true,
        frame_run,
    );
    frame.description = Some(concat!(
        "A scene with a viewBox publishes at exactly that box (fixed). One without publishes at the smallest whole-pixel box that holds everything it paints, ",
        "over its whole loop (scene motion, ambient loops and actions included), plus its margin (fit). --box x,y,w,h fixes the size; ",
        "--fit removes the viewBox so the scene fits its content again; --margin n sets the pixels left around a fit scene's art (a fixed box ignores it). ",
        "With none of them it reports the frame: { frame: \"fixed\" | \"fit\", viewBox, size: [w, h], margin }. One undo step; over MCP a proposal like every edit."
    ));
    frame.args = vec!["name"];
    frame.examples = vec![
        "pixellab scene frame hero-island",
        "pixellab scene frame hero-island --box -16,-16,272,200",
        "pixellab scene frame hero-island --fit --margin 2",
    ];
    frame.format = Some(format_change);

    vec![place, mv, set, duplicate, remove, snap, sort, create, group_set, drop, frame]
}
