//! The scene motion commands: tracks, keys, the loop's length and the path
//! helper (ports cli/lib/commands/scene-motion.mjs).
//!
//! The edits are `motion_ops`; each runs as one undo step through
//! `ctx.edit`, and the key edits return the off-grid warnings of the track
//! they changed.

use serde_json::{json, Map, Value};

use super::common::{format_change, need_one, str_of};
use crate::context::{Ctx, DocKind};
use crate::errors::{CommandError, Result};
use crate::motion_ops as m;
use crate::registry::{Command, Params};
use crate::render::path_string;
use crate::scene_ops as ops;
use crate::session::Edit;

fn name_param() -> Value {
    json!({ "type": "string", "description": "scene name (or scenes/<name>)" })
}

fn item_param() -> Value {
    json!({ "type": "string", "description": "the item: its path from scene items (12, 5.2) or its id; an item needs an id to have a track (scene.set --id)" })
}

fn time_param() -> Value {
    json!({ "type": "integer", "minimum": 0, "description": "ms into the loop" })
}

fn move_param() -> Value {
    json!({ "type": "array", "items": { "type": "integer" }, "minItems": 2, "maxItems": 2, "description": "pixels from where the scene places the item: x,y" })
}

fn s(params: &Params, key: &str) -> String {
    params.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn n(params: &Params, key: &str) -> f64 {
    params.get(key).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

/// The id of the item `reference` names, which must have one (`idOf`).
fn id_of(doc: &Value, reference: &str) -> Result<String> {
    let path = ops::find_item(doc, reference)?;
    let item = ops::placed_at(doc, &path)?;
    match item.get("id") {
        None => Err(ops::scene_op_error(
            format!(
                "{} at {} has no id, so it can't have a track; give it one with scene.set --id",
                str_of(item.get("object")),
                path_string(&path)
            ),
            "no_id",
        )
        .into()),
        Some(id) => Ok(str_of(Some(id))),
    }
}

/// The off-grid warnings of one item's track (`warningsFor`).
fn warnings_for(doc: &Value, name: &str, id: &str) -> Vec<Value> {
    m::off_grid_moves(doc)
        .iter()
        .filter(|w| w.get("item").and_then(Value::as_str) == Some(id))
        .map(|w| json!(m::off_grid_text(&engine::format::scene_file(name), w)))
        .collect()
}

/// An edit from a result with a `message`, which leaves the rest in order
/// (`Edit::from_value` removes with a swap, which would reorder the keys).
fn edit_from(v: Value) -> Edit {
    let mut result = v.as_object().cloned().unwrap_or_default();
    let message = match result.shift_remove("message") {
        Some(Value::String(t)) => t,
        _ => "changed".to_string(),
    };
    Edit::new(result, message)
}

/// One motion edit on a scene as one undo step (`motionEdit`).
fn motion_edit(ctx: &mut Ctx, query: &str, f: impl FnOnce(&mut Value, &str) -> Result<Value>) -> Result<Value> {
    let name = ctx.find(query, Some(DocKind::Scene))?.name;
    ctx.doc(DocKind::Scene, &name)?;
    ctx.edit(DocKind::Scene, &name, None, |doc, _| f(doc, &name).map(edit_from))
}

/// `{ message, ...rest }` from an op's result: the keys of `r` in `keys`, then `extra`.
fn result(r: &Value, keys: &[&str], extra: Vec<(&str, Value)>) -> Value {
    let mut out = Map::new();
    out.insert("message".into(), r.get("message").cloned().unwrap_or(Value::Null));
    for k in keys {
        out.insert((*k).to_string(), r.get(*k).cloned().unwrap_or(Value::Null));
    }
    for (k, v) in extra {
        out.insert(k.to_string(), v);
    }
    Value::Object(out)
}

fn add_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    motion_edit(ctx, &s(&params, "name"), |doc, _| {
        let id = id_of(doc, &s(&params, "item"))?;
        let r = m::add_track(doc, &id, params.get("duration").and_then(Value::as_f64))?;
        let duration = doc["motion"]["duration"].clone();
        Ok(result(&r, &["track"], vec![("duration", duration)]))
    })
}

fn remove_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    motion_edit(ctx, &s(&params, "name"), |doc, _| {
        let id = id_of(doc, &s(&params, "item"))?;
        Ok(m::remove_track(doc, &id)?)
    })
}

fn key_set_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    need_one(&params, &["move", "show", "pose", "clear"], "scene.key.set")?;
    if let Some(pose) = params.get("pose").and_then(Value::as_str) {
        if !ctx.names(DocKind::Object)?.iter().any(|o| o == pose) {
            return Err(CommandError::usage(format!("scene.key.set: no object named {pose} to pose as (pixellab object list)")).into());
        }
    }
    motion_edit(ctx, &s(&params, "name"), |doc, name| {
        let id = id_of(doc, &s(&params, "item"))?;
        let values = m::KeyValues {
            move_: params.get("move").and_then(Value::as_array).map(|a| [a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0)]),
            show: params.get("show").and_then(Value::as_bool),
            pose: params.get("pose").and_then(Value::as_str).map(str::to_string),
            clear: params
                .get("clear")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
        };
        let r = m::set_key(doc, &id, n(&params, "time"), &values)?;
        Ok(result(&r, &["key"], vec![("warnings", Value::Array(warnings_for(doc, name, &id)))]))
    })
}

fn key_move_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    motion_edit(ctx, &s(&params, "name"), |doc, name| {
        let id = id_of(doc, &s(&params, "item"))?;
        let r = m::move_key(doc, &id, n(&params, "time"), n(&params, "to"))?;
        Ok(result(&r, &["key"], vec![("warnings", Value::Array(warnings_for(doc, name, &id)))]))
    })
}

fn key_delete_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    motion_edit(ctx, &s(&params, "name"), |doc, name| {
        let id = id_of(doc, &s(&params, "item"))?;
        let r = m::delete_key(doc, &id, n(&params, "time"))?;
        Ok(result(&r, &[], vec![("warnings", Value::Array(warnings_for(doc, name, &id)))]))
    })
}

fn motion_set_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    motion_edit(ctx, &s(&params, "name"), |doc, _| {
        let r = m::set_duration(doc, n(&params, "duration"))?;
        let mut out = r.as_object().cloned().unwrap_or_default();
        out.insert("duration".into(), params.get("duration").cloned().unwrap_or(Value::Null));
        Ok(Value::Object(out))
    })
}

fn path_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    need_one(&params, &["speed", "end"], "scene.motion.path")?;
    let given = |k: &str| params.get(k).is_some_and(|v| !v.is_null());
    if given("speed") && given("end") {
        return Err(CommandError::usage("scene.motion.path: give --speed or --end, not both").into());
    }
    if given("speed") && n(&params, "speed") <= 0.0 {
        return Err(CommandError::usage("scene.motion.path: --speed must be more than 0").into());
    }
    motion_edit(ctx, &s(&params, "name"), |doc, name| {
        let id = id_of(doc, &s(&params, "item"))?;
        if m::track_of(doc, &id).is_none() {
            if !engine::engine::truthy(doc.get("motion")) {
                return Err(
                    ops::scene_op_error("the scene has no motion yet; make it with scene.motion.set --duration", "no_motion").into()
                );
            }
            m::add_track(doc, &id, None)?;
        }
        let points: Vec<[f64; 2]> = params
            .get("points")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|p| [p.get(0).and_then(Value::as_f64).unwrap_or(0.0), p.get(1).and_then(Value::as_f64).unwrap_or(0.0)])
                    .collect()
            })
            .unwrap_or_default();
        let timing = m::PathTiming {
            start: params.get("start").and_then(Value::as_f64).unwrap_or(0.0),
            speed: params.get("speed").and_then(Value::as_f64),
            end: params.get("end").and_then(Value::as_f64),
        };
        let r = m::motion_path(doc, &id, &points, timing)?;
        Ok(result(&r, &["keys", "end"], vec![("warnings", Value::Array(warnings_for(doc, name, &id)))]))
    })
}

fn format_warnings(r: &Value, _: &Params) -> String {
    let mut lines = vec![str_of(r.get("summary"))];
    for w in r.get("warnings").and_then(Value::as_array).into_iter().flatten() {
        lines.push(format!("warning: {}", str_of(Some(w))));
    }
    lines.join("\n")
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut add = Command::new(
        "scene.track.add",
        "Add a motion track for an item (it needs an id): one key at 0, the rest frame",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "item": item_param(),
                "duration": { "type": "integer", "minimum": 100, "description": "the loop's length in ms, when the scene has no motion yet" }
            },
            "required": ["name", "item"]
        }),
        true,
        add_run,
    );
    add.description =
        Some("Makes the scene's motion (one loop of --duration ms) when it has none. Then scene.key.set or scene.motion.path keys it.");
    add.args = vec!["name", "item"];
    add.examples = vec!["pixellab scene track add hero-island truck-a", "pixellab scene track add meadow bird --duration 4000"];
    add.format = Some(format_change);

    let mut remove = Command::new(
        "scene.track.remove",
        "Remove an item's motion track, with all its keys",
        json!({ "type": "object", "properties": { "name": name_param(), "item": item_param() }, "required": ["name", "item"] }),
        true,
        remove_run,
    );
    remove.args = vec!["name", "item"];
    remove.format = Some(format_change);

    let mut key_set = Command::new(
        "scene.key.set",
        "Set a key of an item's track: its move, show and pose at a time (made if there's no key there)",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "item": item_param(),
                "time": time_param(),
                "move": move_param(),
                "show": { "type": "boolean", "description": "whether the item shows from this key on" },
                "pose": { "type": "string", "description": "the object the item draws from this key on" },
                "clear": { "type": "array", "items": { "type": "string", "enum": ["move", "show", "pose"] }, "description": "values to remove from the key" }
            },
            "required": ["name", "item", "time"]
        }),
        true,
        key_set_run,
    );
    key_set.description = Some(
        "--move x,y is pixels from where the scene places the item; --show false hides it from that key on; --pose <object> draws another object from that key on. --clear move,show,pose removes those values from the key, so they carry on from the key before. Returns the key and the track's off-grid warnings (moves whose steps aren't 1 px straight, 1:1 or 2:1).",
    );
    key_set.args = vec!["name", "item"];
    key_set.examples = vec![
        "pixellab scene key set hero-island truck-a --time 4200 --move 32,16",
        "pixellab scene key set hero-island crane-hook --time 9000 --pose crane-hook-down",
        "pixellab scene key set hero-island parcel --time 12000 --no-show",
    ];
    key_set.format = Some(format_warnings);

    let mut key_move = Command::new(
        "scene.key.move",
        "Move a key of an item's track to another time",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "item": item_param(),
                "time": time_param(),
                "to": { "type": "integer", "minimum": 0, "description": "its new time, ms into the loop" }
            },
            "required": ["name", "item", "time", "to"]
        }),
        true,
        key_move_run,
    );
    key_move.description = Some("The key at 0 (the rest frame) stays; a key can't land on another.");
    key_move.args = vec!["name", "item"];
    key_move.format = Some(format_warnings);

    let mut key_delete = Command::new(
        "scene.key.delete",
        "Delete a key of an item's track (not the one at 0: remove the track for that)",
        json!({ "type": "object", "properties": { "name": name_param(), "item": item_param(), "time": time_param() }, "required": ["name", "item", "time"] }),
        true,
        key_delete_run,
    );
    key_delete.args = vec!["name", "item"];
    key_delete.format = Some(format_warnings);

    let mut motion_set = Command::new(
        "scene.motion.set",
        "Set the length of a scene's motion loop (making the motion if it has none)",
        json!({
            "type": "object",
            "properties": { "name": name_param(), "duration": { "type": "integer", "minimum": 100, "description": "the loop's length in ms" } },
            "required": ["name", "duration"]
        }),
        true,
        motion_set_run,
    );
    motion_set.description = Some("Refuses a length that would cut off keys.");
    motion_set.args = vec!["name"];
    motion_set.format = Some(format_change);

    let mut path = Command::new(
        "scene.motion.path",
        "Key an item along a polyline on the grid, at a steady speed or between two times",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "item": item_param(),
                "points": {
                    "type": "array",
                    "items": { "type": "array", "items": { "type": "integer" }, "minItems": 2, "maxItems": 2 },
                    "minItems": 2,
                    "description": "x,y moves along the path; on the command line x,y pairs separated by spaces"
                },
                "start": { "type": "integer", "minimum": 0, "description": "when the path starts, ms into the loop", "default": 0 },
                "speed": { "type": "number", "description": "pixels per second along the path" },
                "end": { "type": "integer", "minimum": 0, "description": "when the path ends, ms into the loop (instead of --speed)" }
            },
            "required": ["name", "item", "points"]
        }),
        true,
        path_run,
    );
    path.description = Some(
        "--points are moves (pixels from where the scene places the item), the first at --start. Each segment is split into runs that step 1 px straight, 1:1 or 2:1, each ending in a key; times follow the distance at --speed px per second, or spread from --start to --end. Keys inside the path lose their move (and go, if that leaves nothing); a key there with show or pose keeps its place on the path. Makes the track if the item (with an id) has none. Returns the keys written and the track's remaining off-grid warnings (such as the step from the path's end to the next key).",
    );
    path.args = vec!["name", "item", "points"];
    path.examples = vec!["pixellab scene motion path hero-island truck-a 0,0 16,-3 48,-3 --start 2000 --speed 20"];
    path.format = Some(format_warnings);

    vec![add, remove, key_set, key_move, key_delete, motion_set, path]
}
