//! The scene layer commands: scene.layer.new, rename, ungroup, assign and move (ports cli/lib/commands/scene-layers.mjs).
//!
//! Layer commands (docs/intents/2026-10-scene-layers/spec.md, D4): make,
//! rename, ungroup, assign and move layers. The logic is `layer_ops`; each
//! command is one undo step through `ctx.edit`. Layers never reorder the
//! scene's items, so nothing here changes what's published (moving items is
//! the exception: the session's depth sort may reorder them).

use serde_json::{json, Map, Value};

use super::common::{format_change, obj};
use crate::context::{Ctx, DocKind};
use crate::errors::{CommandError, Result};
use crate::layer_ops as l;
use crate::registry::{Command, Params};
use crate::scene_ops as ops;
use crate::session::Edit;

fn name_param() -> Value {
    json!({ "type": "string", "description": "scene name (or scenes/<name>)" })
}

fn layer_param() -> Value {
    json!({ "type": "string", "description": "a layer: its id, or its name when only one layer has it (scene items lists them)" })
}

fn items_param() -> Value {
    json!({ "type": "array", "items": { "type": "string" }, "description": "items: their paths from scene items (12, 5.2) or ids, comma-separated on the command line" })
}

fn layers_param() -> Value {
    json!({ "type": "array", "items": { "type": "string" }, "description": "layers: their ids (or names), comma-separated on the command line" })
}

/// The scene's name, checked (`sceneName`).
fn scene_name(ctx: &mut Ctx, params: &Params) -> Result<String> {
    let query = params.get("name").and_then(Value::as_str).unwrap_or("");
    let name = ctx.find(query, Some(DocKind::Scene))?.name;
    ctx.doc(DocKind::Scene, &name)?;
    Ok(name)
}

/// A string-array param (empty when absent).
fn strings(params: &Params, key: &str) -> Vec<String> {
    params.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// Items named by path or id, as paths (each must be a placed item, not a group) (`itemsOf`).
fn items_of(doc: &Value, refs: &[String]) -> Result<Vec<Vec<usize>>> {
    let mut out = Vec::new();
    for r in refs {
        let path = ops::find_item(doc, r)?;
        ops::placed_at(doc, &path)?;
        out.push(path);
    }
    Ok(out)
}

/// Layers named by id or name, as ids (`layersOf`).
fn layers_of(doc: &Value, refs: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for r in refs {
        out.push(l::find_layer(doc, r)?.get("id").and_then(Value::as_str).unwrap_or("").to_string());
    }
    Ok(out)
}

/// '' is the top level; anything else a layer (`levelOf`).
fn level_of(doc: &Value, r: &str) -> Result<Option<String>> {
    if r.is_empty() {
        return Ok(None);
    }
    Ok(Some(l::find_layer(doc, r)?.get("id").and_then(Value::as_str).unwrap_or("").to_string()))
}

/// The result of a layer op without its message, and the message.
fn edit_of(r: Value, keys: &[&str]) -> Edit {
    let message = r.get("message").and_then(Value::as_str).unwrap_or("").to_string();
    let mut m = Map::new();
    for k in keys {
        if let Some(v) = r.get(*k) {
            m.insert((*k).to_string(), v.clone());
        }
    }
    Edit::new(m, message)
}

fn new_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let name = scene_name(ctx, &params)?;
    ctx.edit(DocKind::Scene, &name, None, move |doc, _| {
        let items = items_of(doc, &strings(&params, "items"))?;
        let layers = layers_of(doc, &strings(&params, "layers"))?;
        let parent = match params.get("parent").and_then(Value::as_str) {
            None => None,
            Some(p) => Some(level_of(doc, p)?),
        };
        let r = l::new_layer(
            doc,
            l::NewLayer {
                name: params.get("layerName").filter(|v| !v.is_null()).cloned(),
                id: params.get("id").and_then(Value::as_str).map(str::to_string),
                items,
                layers,
                parent,
            },
        )?;
        Ok(edit_of(r, &["layer", "items", "layers"]))
    })
}

fn rename_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let name = scene_name(ctx, &params)?;
    ctx.edit(DocKind::Scene, &name, None, move |doc, _| {
        let layer = params.get("layer").and_then(Value::as_str).unwrap_or("");
        let r = l::rename_layer(doc, layer, params.get("to").unwrap_or(&Value::Null))?;
        Ok(edit_of(r, &["layer"]))
    })
}

fn ungroup_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let name = scene_name(ctx, &params)?;
    ctx.edit(DocKind::Scene, &name, None, move |doc, _| {
        let layer = params.get("layer").and_then(Value::as_str).unwrap_or("");
        let r = l::ungroup_layer(doc, layer)?;
        Ok(edit_of(r, &["parent", "layers", "items"]))
    })
}

fn assign_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    if strings(&params, "items").is_empty() && strings(&params, "layers").is_empty() {
        return Err(CommandError::usage("scene.layer.assign: give --items, --layers or both").into());
    }
    let name = scene_name(ctx, &params)?;
    ctx.edit(DocKind::Scene, &name, None, move |doc, _| {
        let to = level_of(doc, params.get("to").and_then(Value::as_str).unwrap_or(""))?;
        let items = items_of(doc, &strings(&params, "items"))?;
        let layers = layers_of(doc, &strings(&params, "layers"))?;
        let r = l::assign_layer(doc, to.as_deref(), &items, &layers)?;
        Ok(edit_of(r, &["to", "items", "layers"]))
    })
}

fn sign(n: f64) -> String {
    if n > 0.0 {
        format!("+{}", engine::jsnum::to_string(n))
    } else {
        engine::jsnum::to_string(n)
    }
}

fn move_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let layer = params.get("layer").and_then(Value::as_str).map(str::to_string);
    if layer.is_none() && strings(&params, "layers").is_empty() && strings(&params, "items").is_empty() {
        return Err(CommandError::usage("scene.layer.move: give a layer, --layers or --items").into());
    }
    let name = scene_name(ctx, &params)?;
    let by: Vec<f64> =
        params.get("by").and_then(Value::as_array).map(|a| a.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect()).unwrap_or_default();
    let (dc, dr, dl) = (by.first().copied().unwrap_or(0.0), by.get(1).copied().unwrap_or(0.0), by.get(2).copied().unwrap_or(0.0));
    let mut key_parts: Vec<String> = vec![layer.clone().unwrap_or_default()];
    key_parts.extend(strings(&params, "layers"));
    key_parts.push("|".into());
    key_parts.extend(strings(&params, "items"));
    let key = format!("scene.layer.move:{}", key_parts.join(","));
    ctx.edit(DocKind::Scene, &name, Some(&key), move |doc, loaded| {
        let sources = &loaded.sources;
        let mut ids: Vec<String> = Vec::new();
        if let Some(layer) = &layer {
            ids.push(l::find_layer(doc, layer)?.get("id").and_then(Value::as_str).unwrap_or("").to_string());
        }
        ids.extend(layers_of(doc, &strings(&params, "layers"))?);
        let mut paths: Vec<(String, Vec<usize>)> = Vec::new();
        let mut put = |path: Vec<usize>| {
            let k = path.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(".");
            match paths.iter_mut().find(|(pk, _)| *pk == k) {
                Some(e) => e.1 = path,
                None => paths.push((k, path)),
            }
        };
        for p in l::items_in_layers(doc, &ids) {
            put(p.path);
        }
        for path in items_of(doc, &strings(&params, "items"))? {
            put(path);
        }
        let keep_group = params.get("keepGroup").and_then(Value::as_bool) != Some(false);
        let mut followed: Vec<Value> = Vec::new();
        for (_, path) in &paths {
            let before = ops::standing(sources, doc, ops::placed_at(doc, path)?);
            ops::move_by_tiles(ops::placed_at_mut(doc, path)?, dc, dr, dl);
            if !keep_group {
                continue;
            }
            let after = ops::standing(sources, doc, ops::placed_at(doc, path)?);
            if let Some(group) = ops::follow_group(doc, path, &before, &after) {
                followed.push(group);
            }
        }
        let mut names: Vec<String> = Vec::new();
        for id in &ids {
            names.push(engine::engine::js_string(l::find_layer(doc, id)?.get("name")));
        }
        let n = paths.len();
        let what = if names.len() == 1 && strings(&params, "items").is_empty() {
            names[0].clone()
        } else {
            format!("{n} item{}", if n == 1 { "" } else { "s" })
        };
        let message = if n > 0 {
            let level = if dl != 0.0 { format!(", {} level", sign(dl)) } else { String::new() };
            let follow = if followed.is_empty() {
                String::new()
            } else {
                format!("; {} group{} followed", followed.len(), if followed.len() == 1 { "" } else { "s" })
            };
            format!("Moved {what} {} col, {} row{level}{follow}", sign(dc), sign(dr))
        } else {
            format!("{what} has no items to move")
        };
        let mut result = obj(vec![
            ("moved", json!(n)),
            ("paths", Value::Array(paths.iter().map(|(k, _)| json!(k)).collect())),
            ("by", json!([engine::jsnum::num(dc), engine::jsnum::num(dr), engine::jsnum::num(dl)])),
        ]);
        if !followed.is_empty() {
            result.insert("groups".into(), Value::Array(followed));
        }
        Ok(Edit::new(result, message))
    })
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut create = Command::new(
        "scene.layer.new",
        "Make a layer (a named, nestable part of a scene) and move items and layers into it",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "layerName": { "type": "string", "description": "the layer's name, 1 to 64 characters (default \"Group n\")" },
                "id": { "type": "string", "description": "its id, lowercase kebab-case (default: made from the name)" },
                "items": items_param(),
                "layers": layers_param(),
                "parent": { "type": "string", "description": "the layer to make it in ('' for the top level); default: the members' common parent" }
            },
            "required": ["name"]
        }),
        true,
        new_run,
    );
    create.description = Some(
        "A layer names part of a scene (the Mine, the Railway, the Train inside it) so it can be selected, moved and placed into as one. It never changes paint order or what is published: items keep their place in the scene, and their { group } wrappers stay as they are. Without --layer-name it is \"Group n\". Its parent is --parent ('' for the top level), else the deepest layer that holds every member, else the top level. A member already inside another member stays where it is. Its id is made from the name unless --id gives one. Returns the layer, how many items joined it and which layers.",
    );
    create.args = vec!["name"];
    create.examples = vec![
        "pixellab scene layer new hero-island --layer-name Railway --items 140.0,141.0,142.0",
        "pixellab scene layer new hero-island --layer-name Train --items cart --parent railway",
    ];
    create.format = Some(format_change);

    let mut rename = Command::new(
        "scene.layer.rename",
        "Rename a layer",
        json!({
            "type": "object",
            "properties": { "name": name_param(), "layer": layer_param(), "to": { "type": "string", "description": "the new name, 1 to 64 characters" } },
            "required": ["name", "layer", "to"]
        }),
        true,
        rename_run,
    );
    rename.description = Some("Its id stays the same, so the scene file changes on one line.");
    rename.args = vec!["name", "layer", "to"];
    rename.examples = vec!["pixellab scene layer rename hero-island group-1 \"Island trees\""];
    rename.format = Some(format_change);

    let mut ungroup = Command::new(
        "scene.layer.ungroup",
        "Ungroup a layer: what it held moves up one level and stays where it is",
        json!({ "type": "object", "properties": { "name": name_param(), "layer": layer_param() }, "required": ["name", "layer"] }),
        true,
        ungroup_run,
    );
    ungroup.description = Some(
        "Takes the layer away; its layers and items now belong to its parent (or the top level). Nothing moves on the stage. Returns the parent, and the layers and items (paths) it held.",
    );
    ungroup.args = vec!["name", "layer"];
    ungroup.examples = vec!["pixellab scene layer ungroup hero-island railway"];
    ungroup.format = Some(format_change);

    let mut assign = Command::new(
        "scene.layer.assign",
        "Move items and layers into a layer, or to the top level",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "to": { "type": "string", "description": "the layer to move them into: its id or name, or '' for the top level" },
                "items": items_param(),
                "layers": layers_param()
            },
            "required": ["name", "to"]
        }),
        true,
        assign_run,
    );
    assign.description = Some(
        "--to is the layer ('' for the top level). Items keep their place in the scene (paint order doesn't change); a layer can't go inside itself or a layer within it.",
    );
    assign.args = vec!["name"];
    assign.examples = vec![
        "pixellab scene layer assign hero-island --to railway --items 150.0,151.0",
        "pixellab scene layer assign hero-island --to '' --layers train",
    ];
    assign.format = Some(format_change);

    let mut mv = Command::new(
        "scene.layer.move",
        "Move layers (every item in them, and in the layers inside them) and items by tiles, together",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "layer": layer_param(),
                "layers": layers_param(),
                "items": items_param(),
                "by": { "type": "array", "items": { "type": "integer" }, "minItems": 2, "maxItems": 3, "description": "tiles to move: col,row[,level]" },
                "keepGroup": { "type": "boolean", "default": true, "description": "keep each item's group attributes in step, as scene.move does" }
            },
            "required": ["name", "by"]
        }),
        true,
        move_run,
    );
    mv.description = Some(
        "As dragging a selection on the stage: every item moves by --by col,row[,level] tiles (a pixel-placed item by the same screen offset), each once, in one undo step; rapid repeats coalesce. Placements move at rest: an animated item's keys are offsets from its placement, so its motion moves with it. Each item's group keeps its data-x/data-y and data-col/data-row in step, as scene.move does (--no-keep-group leaves them). Each item then stands at a new depth, so the scene is put in depth order: paths lists the moved items' paths after that, and reordered says which other paths moved. Layers themselves never change what is published.",
    );
    mv.args = vec!["name", "layer"];
    mv.examples = vec![
        "pixellab scene layer move hero-island railway --by 1,0",
        "pixellab scene layer move hero-island --layers railway,mine --items 12.0 --by 0,-1",
    ];
    mv.format = Some(format_change);

    vec![create, rename, ungroup, assign, mv]
}
