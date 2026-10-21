//! scene.ambient, scene.action, object.ambient and object.action: the
//! commands that edit ambient loops and actions (spec R24, D6, D8).
//!
//! An ambient loop repeats on its own; an action plays when the page asks
//! (`data-play`). A scene item or an object's map layer carries either. Each
//! command runs as one undo step through `ctx.edit`, and an edit that would
//! leave the doc with a problem the engine's rules name (a key past the
//! duration, a name used for two moves in a scene...) is refused, writing
//! nothing.

use std::collections::HashSet;

use engine::engine::{js_string, load_sources, validate};
use serde_json::{json, Map, Value};

use super::common::{flag, opt_num, opt_str, str_of};
use crate::context::{Ctx, DocKind, Loaded};
use crate::errors::{CommandError, Result};
use crate::registry::{Command, Params};
use crate::render::path_string;
use crate::scene_ops as ops;
use crate::session::Edit;

fn name_param(kind: &str) -> Value {
    json!({ "type": "string", "description": format!("{kind} name (or {kind}s/<name>)") })
}

fn item_param() -> Value {
    json!({ "type": "string", "description": "the item: its path from scene items (12, 5.2) or its id" })
}

fn layer_param() -> Value {
    json!({ "type": "integer", "minimum": 0, "description": "the map layer, from 0 at the bottom (a frame loop or a block has none)" })
}

fn action_param() -> Value {
    json!({ "type": "string", "description": "the action's name: lowercase words joined by hyphens, like hop" })
}

fn duration_param(what: &str) -> Value {
    json!({ "type": "integer", "minimum": 1, "description": format!("{what}'s length in ms, up to 600000") })
}

fn keys_param() -> Value {
    json!({
        "type": "array",
        "items": { "type": "object" },
        "minItems": 1,
        "description": "the keys, a JSON list of { \"t\": ms, \"move\": [x, y] }: the first at t 0, each after the one before and below the duration, moves in whole pixels from where the art is drawn. On the command line, the list as one JSON string."
    })
}

fn usage(message: impl Into<String>) -> crate::errors::Error {
    CommandError::usage(message).into()
}

fn failed(message: impl Into<String>, code: &str) -> crate::errors::Error {
    CommandError::new(message, code).into()
}

fn given(params: &Params, key: &str) -> bool {
    params.get(key).is_some_and(|v| !v.is_null())
}

/// What the edit does to an element.
enum Op {
    SetAmbient(Value),
    ClearAmbient,
    SetAction(String, Value),
    DeleteAction(String),
}

/// The element a command edits, from `{ duration, offset?, keys }`.
fn loop_value(params: &Params, command: &str) -> Result<Value> {
    if !given(params, "duration") {
        return Err(usage(format!("{command}: give --duration and --keys")));
    }
    if !given(params, "keys") {
        return Err(usage(format!("{command}: give --keys, a JSON list of {{ \"t\", \"move\" }}")));
    }
    let mut out = Map::new();
    out.insert("duration".into(), params["duration"].clone());
    if given(params, "offset") {
        out.insert("offset".into(), params["offset"].clone());
    }
    out.insert("keys".into(), params["keys"].clone());
    Ok(Value::Object(out))
}

/// The action's value: `{ duration, keys }`.
fn action_value(params: &Params, command: &str) -> Result<Value> {
    if given(params, "offset") {
        return Err(usage(format!("{command}: an action has no offset")));
    }
    let Value::Object(mut v) = loop_value(params, command)? else { unreachable!() };
    v.shift_remove("offset");
    Ok(Value::Object(v))
}

/// Applies the op to one element; returns the sentence for the change.
/// `who` names the element: "boat", "pebble's layer 1".
fn apply(element: &mut Value, op: &Op, who: &str, possessive: &str) -> Result<String> {
    let Some(map) = element.as_object_mut() else {
        return Err(failed(format!("{who} isn't an element that can carry moves"), "not_an_element"));
    };
    Ok(match op {
        Op::SetAmbient(value) => {
            let ms = str_of(value.get("duration"));
            map.insert("ambient".into(), value.clone());
            format!("Set {possessive} ambient loop: {ms} ms")
        }
        Op::ClearAmbient => {
            if map.shift_remove("ambient").is_some() {
                format!("Cleared {possessive} ambient loop")
            } else {
                format!("{who} has no ambient loop to clear")
            }
        }
        Op::SetAction(name, value) => {
            let actions = map.entry("actions").or_insert_with(|| Value::Object(Map::new()));
            let Some(actions) = actions.as_object_mut() else {
                return Err(failed(format!("{who}'s actions aren't an object; fix them first (pixellab check)"), "invalid"));
            };
            let existed = actions.contains_key(name);
            actions.insert(name.clone(), value.clone());
            if existed {
                format!("Changed {name} on {who}")
            } else {
                format!("Added {name} to {who}")
            }
        }
        Op::DeleteAction(name) => {
            let removed = map.get_mut("actions").and_then(Value::as_object_mut).and_then(|a| a.shift_remove(name));
            if removed.is_none() {
                return Err(failed(format!("{who} has no action {name}"), "not_found"));
            }
            if map.get("actions").and_then(Value::as_object).is_some_and(Map::is_empty) {
                map.shift_remove("actions");
            }
            format!("Deleted {name} from {who}")
        }
    })
}

/// What the doc's problems are with `doc` as the named scene or object.
fn problems_with(loaded: &Loaded, kind: DocKind, name: &str, doc: &Value) -> Vec<String> {
    let mut objects = loaded.sources.objects.clone();
    let mut scenes = loaded.sources.scenes.clone();
    if kind == DocKind::Scene {
        scenes.insert(name.to_string(), doc.clone());
    } else {
        objects.insert(name.to_string(), doc.clone());
    }
    validate(&load_sources(loaded.sources.palette.clone(), objects, scenes))
}

/// Refuses a result with problems the doc did not have (D6's rules).
fn refuse_new_problems(loaded: &Loaded, kind: DocKind, name: &str, before: &Value, after: &Value) -> Result<()> {
    let had: HashSet<String> = problems_with(loaded, kind, name, before).into_iter().collect();
    let new: Vec<String> = problems_with(loaded, kind, name, after).into_iter().filter(|p| !had.contains(p)).collect();
    if new.is_empty() {
        return Ok(());
    }
    Err(CommandError::new(format!("{}; nothing changed", new.join("; ")), "invalid")
        .with_details(Value::Array(new.iter().map(|p| Value::String(p.clone())).collect()))
        .into())
}

fn result_of(first: (&str, Value), element: &Value, action: Option<&str>) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert(first.0.into(), first.1);
    match action {
        None => {
            out.insert("ambient".into(), element.get("ambient").cloned().unwrap_or(Value::Null));
        }
        Some(name) => {
            // The action as it stands now: { name, duration, keys }, or null once deleted.
            let now = element.get("actions").and_then(|a| a.get(name)).and_then(Value::as_object).map(|a| {
                let mut m = Map::new();
                m.insert("name".into(), Value::String(name.to_string()));
                m.extend(a.clone());
                Value::Object(m)
            });
            out.insert("action".into(), now.unwrap_or(Value::Null));
        }
    }
    out
}

fn action_of(op: &Op) -> Option<&str> {
    match op {
        Op::SetAction(n, _) | Op::DeleteAction(n) => Some(n),
        _ => None,
    }
}

// ---- scenes ----

fn scene_edit(ctx: &mut Ctx, params: &Params, op: Op) -> Result<Value> {
    let name = ctx.find(opt_str(params, "name").unwrap_or(""), Some(DocKind::Scene))?.name;
    ctx.doc(DocKind::Scene, &name)?;
    let reference = opt_str(params, "item").unwrap_or("").to_string();
    ctx.edit(DocKind::Scene, &name, None, |doc, loaded| {
        let path = ops::find_item(doc, &reference)?;
        let item = ops::placed_at(doc, &path)?;
        let who = match item.get("id") {
            Some(id) => str_of(Some(id)),
            None => js_string(item.get("object")),
        };
        let before = doc.clone();
        let message = apply(ops::placed_at_mut(doc, &path)?, &op, &who, &format!("{who}'s"))?;
        refuse_new_problems(loaded, DocKind::Scene, &name, &before, doc)?;
        let element = ops::placed_at(doc, &path)?;
        Ok(Edit::new(result_of(("path", json!(path_string(&path))), element, action_of(&op)), message))
    })
}

fn scene_ambient_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let op = if flag(&params, "clear") {
        if given(&params, "duration") || given(&params, "offset") || given(&params, "keys") {
            return Err(usage("scene.ambient: --clear takes no --duration, --offset or --keys"));
        }
        Op::ClearAmbient
    } else {
        Op::SetAmbient(loop_value(&params, "scene.ambient")?)
    };
    scene_edit(ctx, &params, op)
}

fn scene_action_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let name = opt_str(&params, "action").unwrap_or("").to_string();
    let op = if flag(&params, "delete") {
        if given(&params, "duration") || given(&params, "keys") {
            return Err(usage("scene.action: --delete takes no --duration or --keys"));
        }
        Op::DeleteAction(name)
    } else {
        Op::SetAction(name, action_value(&params, "scene.action")?)
    };
    scene_edit(ctx, &params, op)
}

// ---- objects ----

fn object_edit(ctx: &mut Ctx, params: &Params, op: Op) -> Result<Value> {
    let name = ctx.find(opt_str(params, "name").unwrap_or(""), Some(DocKind::Object))?.name;
    ctx.doc(DocKind::Object, &name)?;
    let layer = opt_num(params, "layer").unwrap_or(0.0) as usize;
    ctx.edit(DocKind::Object, &name, None, |doc, loaded| {
        if doc.get("kind").and_then(Value::as_str) == Some("block") {
            return Err(failed(
                format!("{name} is a block; only a sprite's map layers carry ambient loops and actions"),
                "not_a_map_layer",
            ));
        }
        let Some(layers) = doc.get("layers").and_then(Value::as_array) else {
            return Err(failed(
                format!("{name} extends another sprite and has no layers of its own; edit the one it extends"),
                "not_a_map_layer",
            ));
        };
        let Some(l) = layers.get(layer) else {
            return Err(failed(
                format!("{name} has no layer {layer}; it has {} (0 to {})", layers.len(), layers.len() as i64 - 1),
                "not_found",
            ));
        };
        if l.get("map").is_none() {
            return Err(failed(
                format!("layer {layer} of {name} is a frame loop; only a map layer carries ambient loops and actions"),
                "not_a_map_layer",
            ));
        }
        let who = format!("{name}'s layer {layer}");
        let before = doc.clone();
        let element = &mut doc["layers"][layer];
        let message = apply(element, &op, &who, &who)?;
        refuse_new_problems(loaded, DocKind::Object, &name, &before, doc)?;
        Ok(Edit::new(result_of(("layer", json!(layer)), &doc["layers"][layer], action_of(&op)), message))
    })
}

fn object_ambient_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let op = if flag(&params, "clear") {
        if given(&params, "duration") || given(&params, "offset") || given(&params, "keys") {
            return Err(usage("object.ambient: --clear takes no --duration, --offset or --keys"));
        }
        Op::ClearAmbient
    } else {
        Op::SetAmbient(loop_value(&params, "object.ambient")?)
    };
    object_edit(ctx, &params, op)
}

fn object_action_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let name = opt_str(&params, "action").unwrap_or("").to_string();
    let op = if flag(&params, "delete") {
        if given(&params, "duration") || given(&params, "keys") {
            return Err(usage("object.action: --delete takes no --duration or --keys"));
        }
        Op::DeleteAction(name)
    } else {
        Op::SetAction(name, action_value(&params, "object.action")?)
    };
    object_edit(ctx, &params, op)
}

fn format_edit(r: &Value, _: &Params) -> String {
    str_of(r.get("summary"))
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let ambient_props = |target: Value| -> Value {
        let mut props = Map::new();
        for (k, v) in target.as_object().cloned().unwrap_or_default() {
            props.insert(k, v);
        }
        props.insert("duration".into(), duration_param("the loop"));
        props.insert(
            "offset".into(),
            json!({ "type": "integer", "minimum": 0, "description": "start the loop this many ms in (below the duration)" }),
        );
        props.insert("keys".into(), keys_param());
        props.insert("clear".into(), json!({ "type": "boolean", "default": false, "description": "remove the ambient loop instead" }));
        Value::Object(props)
    };
    let action_props = |target: Value| -> Value {
        let mut props = Map::new();
        for (k, v) in target.as_object().cloned().unwrap_or_default() {
            props.insert(k, v);
        }
        props.insert("action".into(), action_param());
        props.insert("duration".into(), duration_param("the action"));
        props.insert("keys".into(), keys_param());
        props.insert("delete".into(), json!({ "type": "boolean", "default": false, "description": "delete the action instead" }));
        Value::Object(props)
    };

    let mut scene_ambient = Command::new(
        "scene.ambient",
        "Set or clear a scene item's ambient loop: a move that repeats on its own",
        json!({
            "type": "object",
            "properties": ambient_props(json!({ "name": name_param("scene"), "item": item_param() })),
            "required": ["name", "item"]
        }),
        true,
        scene_ambient_run,
    );
    scene_ambient.description = Some(
        "The loop lasts --duration ms and starts --offset ms in. --keys is a JSON list of { \"t\", \"move\" }: the first at t 0, each later one after the one before and below the duration; the item is moved by [x, y] whole pixels from where the scene places it, stepping a pixel at a time between keys, and after the last key it steps back to the first over the rest of the duration. It plays in the SVG on its own, and scene render --time places it. Setting replaces the loop the item has. --clear removes it. An item with scene motion can't also have an ambient loop. An edit that would leave the scene with a problem is refused, and nothing changes.",
    );
    scene_ambient.args = vec!["name", "item"];
    scene_ambient.examples = vec![
        "pixellab scene ambient meadow cloud --duration 16000 --keys '[{\"t\":0,\"move\":[0,0]},{\"t\":8000,\"move\":[12,0]}]'",
        "pixellab scene ambient meadow cloud --clear",
    ];
    scene_ambient.format = Some(format_edit);

    let mut scene_action = Command::new(
        "scene.action",
        "Add, change or delete a scene item's action: a one-shot move the page plays by name",
        json!({
            "type": "object",
            "properties": action_props(json!({ "name": name_param("scene"), "item": item_param() })),
            "required": ["name", "item", "action"]
        }),
        true,
        scene_action_run,
    );
    scene_action.description = Some(
        "An action plays when the page names it in data-play (the site contract's actions), replaces the item's ambient loop while it plays, holds its last key until --duration ms, then the item is back at rest. --keys is as for scene ambient. Giving a name the item has changes it; --delete removes it. An item has at most 16 actions, and a name is one move in a scene. scene render --play <action>@<ms> shows a frame of it.",
    );
    scene_action.args = vec!["name", "item", "action"];
    scene_action.examples = vec![
        "pixellab scene action meadow frog hop --duration 480 --keys '[{\"t\":0,\"move\":[0,0]},{\"t\":120,\"move\":[0,-4]}]'",
        "pixellab scene action meadow frog hop --delete",
    ];
    scene_action.format = Some(format_edit);

    let mut object_ambient = Command::new(
        "object.ambient",
        "Set or clear a sprite map layer's ambient loop",
        json!({
            "type": "object",
            "properties": ambient_props(json!({ "name": name_param("object"), "layer": layer_param() })),
            "required": ["name", "layer"]
        }),
        true,
        object_ambient_run,
    );
    object_ambient.description = Some(
        "As scene ambient, for one layer of the sprite, which moves wherever the sprite is placed. A frame loop layer, a block and an outfit (which has no layers of its own) refuse. object render --time places the loop.",
    );
    object_ambient.args = vec!["name", "layer"];
    object_ambient.examples = vec![
        "pixellab object ambient pine 1 --duration 4000 --keys '[{\"t\":0,\"move\":[0,0]},{\"t\":2000,\"move\":[1,0]}]'",
        "pixellab object ambient pine 1 --clear",
    ];
    object_ambient.format = Some(format_edit);

    let mut object_action = Command::new(
        "object.action",
        "Add, change or delete an action of a sprite map layer",
        json!({
            "type": "object",
            "properties": action_props(json!({ "name": name_param("object"), "layer": layer_param() })),
            "required": ["name", "layer", "action"]
        }),
        true,
        object_action_run,
    );
    object_action.description = Some(
        "As scene action, for one layer of the sprite. A frame loop layer, a block and an outfit refuse. object render --time T --play <action>@<ms> shows a frame of it.",
    );
    object_action.args = vec!["name", "layer", "action"];
    object_action.examples = vec![
        "pixellab object action character 1 hop --duration 480 --keys '[{\"t\":0,\"move\":[0,0]},{\"t\":120,\"move\":[0,-4]}]'",
        "pixellab object action character 1 hop --delete",
    ];
    object_action.format = Some(format_edit);

    vec![scene_ambient, scene_action, object_ambient, object_action]
}
