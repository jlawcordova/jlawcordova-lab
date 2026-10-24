//! The object editing commands: object.paint, object.fill, layers, frames and the rest (ports cli/lib/commands/object-edit.mjs).
//!
//! Every change goes through the engine's edit module, as the lab's Object
//! mode does, so the source stays in the shape a person would write by hand.
//! Pixel coordinates are those of object.render's PNG at scale 1: a sprite's
//! map column and row; for a block, the PNG pixel of its render (whose viewBox
//! starts at the block's top-left pixel).

use std::collections::HashSet;

use engine::engine::{
    bounds, composite, is_key, js_len, js_string, render_object, resolve, truthy, used_colors, CompositeOptions, Resolved, Sources,
    SpriteLayer, CAPS, MAX_FOOTPRINT,
};
use engine::format::stringify;
use engine::iso::Pixel;
use engine::motion::SITE_LOOPS;
use engine::omap::OMap;
use engine::serialize::entries;
use engine::{edit, starter};
use serde_json::{json, Map, Value};

use super::common::{flag, format_change, obj, opt_str, str_of};
use super::scene_edit::check_new_name;
use crate::context::{Ctx, DocKind, Loaded};
use crate::errors::{CommandError, Error, Result};
use crate::library::{category_from_param, category_text};
use crate::registry::{Command, Params};
use crate::session::Edit;

fn name_param() -> Value {
    json!({ "type": "string", "description": "object name (or objects/<name>)" })
}

fn layer_param() -> Value {
    json!({ "type": "integer", "minimum": 0, "default": 0, "description": "the layer, from 0 at the bottom" })
}

fn frame_param() -> Value {
    json!({ "type": "integer", "minimum": 0, "default": 0, "description": "the frame of a frame loop (a block: of its surface loop)" })
}

fn pixels_param() -> Value {
    json!({
        "type": "array",
        "items": { "type": "array", "items": { "type": "integer" }, "minItems": 2, "maxItems": 2 },
        "minItems": 1,
        "description": "pixels as [x, y] pairs; on the command line x,y pairs separated by spaces (or the flag repeated)"
    })
}

fn color_param() -> Value {
    json!({ "type": "string", "description": "a palette color name (pixellab palette get)" })
}

fn usage(message: impl Into<String>) -> Error {
    CommandError::usage(message).into()
}

fn failed(message: impl Into<String>, code: &str) -> Error {
    CommandError::new(message, code).into()
}

fn as_int(v: &Value) -> i64 {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).unwrap_or(0)
}

/// An integer param (0 when absent).
fn int(params: &Params, key: &str) -> i64 {
    params.get(key).map_or(0, as_int)
}

fn index(params: &Params, key: &str) -> usize {
    int(params, key).max(0) as usize
}

/// The object's name, checked (`objectAt`).
fn object_at(ctx: &mut Ctx, query: &str) -> Result<String> {
    let name = ctx.find(query, Some(DocKind::Object))?.name;
    ctx.doc(DocKind::Object, &name)?;
    Ok(name)
}

/// The engine's resolve, as a command error when the object doesn't resolve.
fn resolved(sources: &Sources, name: &str) -> Result<Resolved> {
    resolve(sources, name).map_err(|e| {
        failed(format!("object {name} doesn't resolve: {}; fix the problems first (pixellab check {name})", e.0), "resolve_error")
    })
}

/// Whether the object may paint a color: its tier, then the colors cap.
fn check_color(sources: &Sources, r: &Resolved, color: &str) -> Result<()> {
    let Some(entry) = sources.colors.get(color) else {
        return Err(usage(format!("no palette color named {color} (pixellab palette get)")));
    };
    if entry.tier == "outfit" && !r.character {
        return Err(failed(format!("{color} is an outfit color; only objects that extend character may use it"), "color_not_allowed"));
    }
    let used: HashSet<String> = used_colors(sources, r).into_iter().collect();
    if !used.contains(&entry.hex) && used.len() >= CAPS.colors {
        return Err(failed(
            format!("This object already uses {} colors. Pick one it uses, or free one up first.", CAPS.colors),
            "color_cap",
        ));
    }
    Ok(())
}

/// Why painting is off for a layer, or None.
fn paint_blocked(doc: &Value, r: &Resolved, layer: usize) -> Option<String> {
    if doc.get("kind").and_then(Value::as_str) == Some("block") {
        return None;
    }
    let Some(source) = r.layers.get(layer) else {
        return Some(format!("there's no layer {layer}; the object has {} (0 to {})", r.layers.len(), r.layers.len() as i64 - 1));
    };
    if truthy(doc.get("extends")) && matches!(source, SpriteLayer::Frames { .. }) {
        return Some(format!("layer {layer} is a frame loop of {}, which an outfit can't override", js_string(doc.get("extends"))));
    }
    None
}

type BlockPixels = OMap<Pixel, engine::engine::ColorName>;

/// A block's pixels at a frame and its top-left pixel: where its render's PNG starts.
fn block_corner(sources: &Sources, name: &str, frame: usize) -> Result<(BlockPixels, i32, i32)> {
    let root = render_object(sources, name)?;
    let pixels = composite(&root, CompositeOptions { frame, ..CompositeOptions::default() });
    let (min_x, min_y) = bounds(pixels.keys()).map_or((0, 0), |b| (b.min_x, b.min_y));
    Ok((pixels, min_x, min_y))
}

fn pixel_pairs(params: &Params) -> Vec<[i64; 2]> {
    params
        .get("pixels")
        .and_then(Value::as_array)
        .map(|list| list.iter().map(|p| [p.get(0).map_or(0, as_int), p.get(1).map_or(0, as_int)]).collect())
        .unwrap_or_default()
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// Paints (or clears, color None) pixels as one undo step (`paintEdit`).
#[allow(clippy::too_many_arguments)]
fn paint_edit(
    ctx: &mut Ctx,
    name: &str,
    layer: usize,
    frame: usize,
    pixels_of: impl FnOnce(&Resolved, &Sources) -> Result<Vec<[i64; 2]>>,
    color: Option<&str>,
    verb: &'static str,
    key: Option<String>,
) -> Result<Value> {
    ctx.edit(DocKind::Object, name, key.as_deref(), |doc, loaded: &Loaded| {
        let sources = &loaded.sources;
        let r = resolved(sources, name)?;
        if let Some(blocked) = paint_blocked(doc, &r, layer) {
            return Err(failed(blocked, "paint_blocked"));
        }
        if let Some(c) = color {
            check_color(sources, &r, c)?;
        }
        let pixels = pixels_of(&r, sources)?;
        let count = if doc.get("kind").and_then(Value::as_str) == Some("block") {
            let (_, min_x, min_y) = block_corner(sources, name, frame)?;
            let shifted: Vec<[i64; 2]> = pixels.iter().map(|[x, y]| [x + min_x as i64, y + min_y as i64]).collect();
            edit::paint_block(doc, frame, &shifted, color)?
        } else {
            edit::paint(sources, name, doc, edit::Where { layer, frame }, &pixels, color)?
        };
        let n = plural(count, "pixel");
        let message = if count == 0 {
            "Nothing changed".to_string()
        } else {
            match color {
                None => format!("Erased {n}"),
                Some(c) => format!("{verb} {n} {c}"),
            }
        };
        Ok(Edit::new(obj(vec![("pixels", json!(count))]), message))
    })
}

fn paint_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let name = object_at(ctx, opt_str(&params, "name").unwrap_or(""))?;
    let (layer, frame) = (index(&params, "layer"), index(&params, "frame"));
    let color = opt_str(&params, "color").unwrap_or("").to_string();
    let pixels = pixel_pairs(&params);
    let key = format!("object.paint:{layer}:{frame}:{color}");
    paint_edit(ctx, &name, layer, frame, |_, _| Ok(pixels), Some(&color), "Painted", Some(key))
}

fn erase_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let name = object_at(ctx, opt_str(&params, "name").unwrap_or(""))?;
    let (layer, frame) = (index(&params, "layer"), index(&params, "frame"));
    let pixels = pixel_pairs(&params);
    let key = format!("object.erase:{layer}:{frame}");
    paint_edit(ctx, &name, layer, frame, |_, _| Ok(pixels), None, "Erased", Some(key))
}

fn fill_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let name = object_at(ctx, opt_str(&params, "name").unwrap_or(""))?;
    let (layer, frame) = (index(&params, "layer"), index(&params, "frame"));
    let (x, y) = (int(&params, "x"), int(&params, "y"));
    let color = opt_str(&params, "color").unwrap_or("").to_string();
    let n = name.clone();
    paint_edit(
        ctx,
        &name,
        layer,
        frame,
        |r, sources| {
            let Some(block) = &r.block else {
                let source = r.layers.get(layer).ok_or_else(|| Error::Internal("Cannot read properties of undefined".into()))?;
                return Ok(edit::flood_fill(edit::map_at(source, frame), x, y));
            };
            // In block coordinates, then back to the render's.
            let (pixels, min_x, min_y) = block_corner(sources, &n, frame)?;
            let colors: OMap<Pixel, String> = pixels.iter().map(|(p, c)| (*p, c.to_string())).collect();
            let to = |v: i64| i32::try_from(v).unwrap_or(i32::MAX);
            let cells = edit::block_cells(block.size);
            Ok(edit::block_fill(&cells, &colors, to(x + min_x as i64), to(y + min_y as i64))
                .into_iter()
                .map(|[px, py]| [px - min_x as i64, py - min_y as i64])
                .collect())
        },
        Some(&color),
        "Filled",
        None,
    )
}

fn pick_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let name = object_at(ctx, opt_str(&params, "name").unwrap_or(""))?;
    let (layer, frame) = (index(&params, "layer"), index(&params, "frame"));
    let (x, y) = (int(&params, "x"), int(&params, "y"));
    let loaded = ctx.sources()?;
    let sources = &loaded.sources;
    let r = resolved(sources, &name)?;
    let color: Option<String> = if r.block.is_some() {
        let (pixels, min_x, min_y) = block_corner(sources, &name, frame)?;
        let at = |v: i64, m: i32| i32::try_from(v + m as i64).ok();
        match (at(x, min_x), at(y, min_y)) {
            (Some(px), Some(py)) => pixels.get(&(px, py)).map(|c| c.to_string()),
            _ => None,
        }
    } else {
        if r.layers.get(layer).is_none() {
            return Err(usage(format!("there's no layer {layer}; the object has {}", r.layers.len())));
        }
        edit::color_at(&r, edit::Where { layer, frame }, x, y)
    };
    let hex = color.as_ref().map_or(Value::Null, |c| sources.colors.get(c.as_str()).map_or(Value::Null, |e| json!(e.hex)));
    Ok(json!({ "object": name, "x": x, "y": y, "layer": layer, "frame": frame, "color": color, "hex": hex }))
}

fn pick_format(r: &Value, _: &Params) -> String {
    let what = if truthy(r.get("color")) { format!("{} {}", str_of(r.get("color")), str_of(r.get("hex"))) } else { "empty".into() };
    format!("x {}, y {}: {what}", str_of(r.get("x")), str_of(r.get("y")))
}

/// An edit to an object with its own layers: a sprite that doesn't extend another (`layerEdit`).
fn layer_edit(ctx: &mut Ctx, query: &str, f: impl FnOnce(&mut Value) -> Result<Edit>) -> Result<Value> {
    let name = object_at(ctx, query)?;
    let n = name.clone();
    ctx.edit(DocKind::Object, &name, None, move |doc, _| {
        if doc.get("kind").and_then(Value::as_str) != Some("sprite") {
            return Err(failed(format!("{n} is a block; a block has no layers or frames to change"), "not_editable"));
        }
        if truthy(doc.get("extends")) {
            return Err(failed(
                format!("{n} extends {} and takes its layers from it; paint row overrides instead", js_string(doc.get("extends"))),
                "not_editable",
            ));
        }
        f(doc)
    })
}

fn layer_count(doc: &Value) -> usize {
    doc.get("layers").and_then(Value::as_array).map_or(0, Vec::len)
}

/// `layerOf`: refuses a layer the object doesn't have.
fn layer_of(doc: &Value, layer: usize) -> Result<()> {
    let n = layer_count(doc);
    if layer >= n {
        return Err(usage(format!("there's no layer {layer}; the object has {n} (0 to {})", n as i64 - 1)));
    }
    Ok(())
}

/// A frame loop's prefix and frame count (`loopOf`).
struct LoopInfo {
    prefix: String,
    frames: usize,
}

fn loop_of(doc: &Value, layer: usize) -> Result<LoopInfo> {
    layer_of(doc, layer)?;
    let l = &doc["layers"][layer];
    match l.get("frames") {
        Some(frames) => Ok(LoopInfo { prefix: str_of(l.get("prefix")), frames: frames.as_array().map_or(0, Vec::len) }),
        None => Err(failed(format!("layer {layer} is a still map, not a frame loop (object.animate makes it one)"), "not_a_loop")),
    }
}

/// A loop and a frame of it that exist.
fn frame_of(doc: &Value, layer: usize, frame: usize) -> Result<LoopInfo> {
    let l = loop_of(doc, layer)?;
    if frame >= l.frames {
        return Err(usage(format!("there's no frame {frame}; the loop has {}", l.frames)));
    }
    Ok(l)
}

/// Moves a list item by `by` steps; refuses past either end (`moveBy`).
fn move_by(list: &mut [Value], idx: usize, by: i64, what: &str) -> Result<usize> {
    let target = idx as i64 + by;
    if by == 0 {
        return Err(usage("--by must not be 0"));
    }
    if target < 0 || target >= list.len() as i64 {
        return Err(failed(
            format!(
                "{what} {idx} can't move {} {}: there are {} (0 to {})",
                if by > 0 { "up" } else { "down" },
                by.abs(),
                list.len(),
                list.len() as i64 - 1
            ),
            "out_of_range",
        ));
    }
    let mut i = idx;
    while i as i64 != target {
        i = edit::move_in(list, i, if by > 0 { 1 } else { -1 });
    }
    Ok(i)
}

fn result_of(pairs: Vec<(&str, Value)>, message: String) -> Edit {
    Edit::new(obj(pairs), message)
}

fn layer_add_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    layer_edit(ctx, opt_str(&params, "name").unwrap_or(""), |doc| {
        let layer = edit::add_layer(doc);
        Ok(result_of(vec![("layer", json!(layer))], format!("Added layer {layer}")))
    })
}

fn layer_duplicate_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let layer = index(&params, "layer");
    layer_edit(ctx, opt_str(&params, "name").unwrap_or(""), |doc| {
        layer_of(doc, layer)?;
        let next = edit::duplicate_layer(doc, layer);
        Ok(result_of(vec![("layer", json!(next))], format!("Layer {layer} duplicated as layer {next}")))
    })
}

fn layer_delete_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let layer = index(&params, "layer");
    layer_edit(ctx, opt_str(&params, "name").unwrap_or(""), |doc| {
        layer_of(doc, layer)?;
        if layer_count(doc) == 1 {
            return Err(failed(format!("layer {layer} is the only layer, so it stays"), "last_layer"));
        }
        let next = edit::delete_layer(doc, layer);
        Ok(result_of(vec![("layer", json!(next))], format!("Layer {layer} deleted")))
    })
}

fn layer_move_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let (layer, by) = (index(&params, "layer"), int(&params, "by"));
    layer_edit(ctx, opt_str(&params, "name").unwrap_or(""), |doc| {
        layer_of(doc, layer)?;
        let list = doc["layers"].as_array_mut().expect("layers");
        let next = move_by(list, layer, by, "layer")?;
        Ok(result_of(vec![("layer", json!(next))], format!("Layer {layer} {} to layer {next}", if by > 0 { "raised" } else { "lowered" })))
    })
}

fn frame_params(more: Vec<(&str, Value)>) -> Value {
    let mut props = Map::new();
    props.insert("name".into(), name_param());
    props.insert("layer".into(), layer_param());
    props.insert("frame".into(), frame_param());
    for (k, v) in more {
        props.insert(k.into(), v);
    }
    json!({ "type": "object", "properties": props, "required": ["name"] })
}

fn frame_add_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let (layer, frame) = (index(&params, "layer"), index(&params, "frame"));
    layer_edit(ctx, opt_str(&params, "name").unwrap_or(""), |doc| {
        let l = frame_of(doc, layer, frame)?;
        let next = edit::add_frame(doc, layer, frame);
        Ok(result_of(vec![("frame", json!(next))], format!("Added a blank frame, {}{next}", l.prefix)))
    })
}

fn frame_duplicate_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let (layer, frame) = (index(&params, "layer"), index(&params, "frame"));
    layer_edit(ctx, opt_str(&params, "name").unwrap_or(""), |doc| {
        let l = frame_of(doc, layer, frame)?;
        let next = edit::duplicate_frame(doc, layer, frame);
        Ok(result_of(vec![("frame", json!(next))], format!("Frame {p}{frame} duplicated as {p}{next}", p = l.prefix)))
    })
}

fn frame_delete_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let (layer, frame) = (index(&params, "layer"), index(&params, "frame"));
    layer_edit(ctx, opt_str(&params, "name").unwrap_or(""), |doc| {
        let l = frame_of(doc, layer, frame)?;
        if l.frames == 1 {
            return Err(failed("A loop keeps at least one frame", "last_frame"));
        }
        let next = edit::delete_frame(doc, layer, frame);
        Ok(result_of(vec![("frame", json!(next))], format!("Frame {}{frame} deleted", l.prefix)))
    })
}

fn frame_move_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let (layer, frame, by) = (index(&params, "layer"), index(&params, "frame"), int(&params, "by"));
    layer_edit(ctx, opt_str(&params, "name").unwrap_or(""), |doc| {
        let l = frame_of(doc, layer, frame)?;
        let list = doc["layers"][layer]["frames"].as_array_mut().expect("frames");
        let next = move_by(list, frame, by, "frame")?;
        Ok(result_of(
            vec![("frame", json!(next))],
            format!("Frame {p}{frame} moved {}, to {p}{next}", if by > 0 { "later" } else { "earlier" }, p = l.prefix),
        ))
    })
}

fn animate_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let layer = index(&params, "layer");
    let loop_name = opt_str(&params, "loop").unwrap_or("ff").to_string();
    layer_edit(ctx, opt_str(&params, "name").unwrap_or(""), |doc| {
        layer_of(doc, layer)?;
        if doc["layers"][layer].get("frames").is_some() {
            edit::set_loop(doc, layer, &loop_name);
        } else {
            edit::animate_layer(doc, layer, &loop_name);
        }
        let frames = SITE_LOOPS.iter().find(|l| l.name == loop_name).map_or(0, |l| l.frames);
        let count = doc["layers"][layer]["frames"].as_array().map_or(0, Vec::len);
        Ok(result_of(vec![("frames", json!(count))], format!("Layer {layer} plays as {loop_name}, {frames} frames")))
    })
}

fn still_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let (layer, frame) = (index(&params, "layer"), index(&params, "frame"));
    layer_edit(ctx, opt_str(&params, "name").unwrap_or(""), |doc| {
        frame_of(doc, layer, frame)?;
        edit::still_layer(doc, layer, frame);
        Ok(result_of(vec![], format!("Still, keeping frame {frame}")))
    })
}

fn resize_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let given = |k: &str| params.get(k).is_some_and(|v| !v.is_null());
    if !given("width") && !given("height") {
        return Err(usage("object.resize: give --width, --height or both"));
    }
    let query = opt_str(&params, "name").unwrap_or("");
    let name = object_at(ctx, query)?;
    let loaded = ctx.sources()?;
    let r = resolved(&loaded.sources, &name)?;
    let width = if given("width") { int(&params, "width") } else { r.width };
    let height = if given("height") { int(&params, "height") } else { r.height };
    layer_edit(ctx, query, |doc| {
        edit::resize(doc, width.max(0) as usize, height.max(0) as usize);
        Ok(result_of(vec![("size", json!([width, height]))], format!("Resized to {width}×{height}")))
    })
}

/// A sprite's footprint: whole tiles wide and deep, 1 to 16.
fn footprint_param() -> Value {
    json!({
        "type": "array",
        "items": { "type": "integer", "minimum": 1, "maximum": MAX_FOOTPRINT },
        "minItems": 2,
        "maxItems": 2,
        "description": "tiles wide and deep: w,d (1 to 16 each)"
    })
}

fn footprint_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let to = params.get("to").filter(|v| !v.is_null()).cloned();
    let clear = flag(&params, "clear");
    if to.is_some() && clear {
        return Err(usage("object.footprint: give only one of --to, --clear"));
    }
    if to.is_none() && !clear {
        return Err(usage("object.footprint: give --to or --clear"));
    }
    let name = object_at(ctx, opt_str(&params, "name").unwrap_or(""))?;
    let n = name.clone();
    ctx.edit(DocKind::Object, &name, None, move |doc, _| {
        if doc.get("kind").and_then(Value::as_str) == Some("block") {
            let size: Vec<String> =
                doc.get("size").and_then(Value::as_array).map(|a| a.iter().take(2).map(|v| str_of(Some(v))).collect()).unwrap_or_default();
            return Err(usage(format!("{n} is a block; its size is its footprint ({} tiles)", size.join("×"))));
        }
        let object = doc.as_object_mut().expect("an object source is an object");
        if clear {
            object.shift_remove("footprint");
            return Ok(Edit::new(obj(vec![("footprint", Value::Null)]), format!("Footprint of {n} cleared")));
        }
        let to = to.expect("to");
        let text = to.as_array().map(|a| a.iter().map(|v| str_of(Some(v))).collect::<Vec<_>>().join("×")).unwrap_or_default();
        object.insert("footprint".into(), to.clone());
        Ok(Edit::new(obj(vec![("footprint", to)]), format!("Footprint of {n} set to {text}")))
    })
}

/// object.category: sets or clears the category of one or more objects
/// (docs/intents/2026-10-object-libraries/spec.md, R9, D2). Each object is
/// one session edit; the command checks every name and the path first, so a
/// refusal changes nothing.
fn category_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let has = |k: &str| params.get(k).is_some_and(|v| !v.is_null());
    // `category: null` is the JSON form of --clear.
    let clear = flag(&params, "clear") || params.get("category").is_some_and(Value::is_null);
    if usize::from(has("category")) + usize::from(has("to")) + usize::from(clear) != 1 {
        return Err(usage("object.category: give one of --to, --clear"));
    }
    let category = if clear {
        None
    } else {
        let value = params.get("category").filter(|v| !v.is_null()).or_else(|| params.get("to")).expect("a category");
        Some(category_from_param(value).map_err(|why| usage(format!("object.category: {why}")))?)
    };
    let queries: Vec<String> =
        params.get("names").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(String::from).collect();
    if queries.is_empty() {
        return Err(usage("object.category: give at least one object name"));
    }
    let mut names: Vec<String> = Vec::new();
    for query in &queries {
        let name = object_at(ctx, query)?;
        if !names.contains(&name) {
            names.push(name);
        }
    }
    let mut results: Vec<Value> = Vec::new();
    for name in &names {
        let n = name.clone();
        let set = category.clone();
        let label = match &category {
            Some(c) => format!("Set {n}'s category to {}", category_text(c)),
            None => format!("Cleared {n}'s category"),
        };
        let r = ctx.edit(DocKind::Object, name, None, move |doc, _| {
            let object = doc.as_object_mut().expect("an object source is an object");
            match &set {
                Some(c) => {
                    object.insert("category".into(), json!(c));
                }
                None => {
                    object.shift_remove("category");
                }
            }
            Ok(Edit::new(obj(vec![("category", set.as_ref().map_or(Value::Null, |c| json!(c)))]), label))
        })?;
        results.push(json!({ "name": name, "doc": r["doc"], "changed": r["changed"] }));
    }
    let changed = results.iter().any(|r| r["changed"] == json!(true));
    let summary = match (names.as_slice(), &category) {
        ([one], Some(c)) => format!("Set {one}'s category to {}", category_text(c)),
        ([one], None) => format!("Cleared {one}'s category"),
        (many, Some(c)) => format!("Set the category of {} objects ({}) to {}", many.len(), many.join(", "), category_text(c)),
        (many, None) => format!("Cleared the category of {} objects ({})", many.len(), many.join(", ")),
    };
    let summary = if changed { summary } else { format!("nothing changed ({summary})") };
    Ok(json!({
        "changed": changed,
        "category": category.map_or(Value::Null, |c| json!(c)),
        "objects": results,
        "summary": summary,
    }))
}

fn new_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let name = opt_str(&params, "name").unwrap_or("").to_string();
    let kind = opt_str(&params, "kind").unwrap_or("sprite");
    check_new_name(ctx, &name, "object.new")?;
    let has = |k: &str| truthy(params.get(k));
    let mut doc = if kind == "block" {
        if has("extends") || has("width") || has("height") {
            return Err(usage(
                "object.new --kind block doesn't take --width, --height or --extends; a block's size is in its file, in tiles and levels",
            ));
        }
        if has("footprint") {
            return Err(usage("object.new --kind block doesn't take --footprint; a block's size is its footprint"));
        }
        starter::starter_block()
    } else if has("extends") {
        if has("width") || has("height") {
            return Err(usage("object.new: --width/--height and --extends don't mix: an object that extends another has its size"));
        }
        let base = str_of(params.get("extends"));
        if !ctx.names(DocKind::Object)?.contains(&base) {
            return Err(usage(format!("object.new --extends {base}: no such object")));
        }
        starter::starter_extends(&base)
    } else {
        let size = |k: &str| if has(k) { int(&params, k) } else { 16 };
        starter::starter_sprite(size("width"), size("height"))
    };
    if has("footprint") {
        doc.as_object_mut().expect("a starter is an object").insert("footprint".into(), params["footprint"].clone());
    }
    let label = format!("Created object {name}");
    ctx.session()?.create(DocKind::Object, &name, doc, &label);
    Ok(json!({ "doc": format!("object:{name}"), "changed": true, "summary": label, "file": engine::format::object_file(&name) }))
}

/// Rows → the object's own keys: legend characters become the object's key
/// for their color (made as painting makes one), the rest must be its keys (`rekey`).
fn rekey(
    sources: &Sources,
    r: &Resolved,
    doc: &mut Value,
    rows: &[String],
    legend: &Map<String, Value>,
) -> Result<(Vec<String>, Vec<String>)> {
    // `{ ...r.keys, ...(doc.keys ?? {}) }`, in JavaScript's key order.
    let known_of = |doc: &Value| -> OMap<String, String> {
        let mut m: Map<String, Value> = r.keys.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        if let Some(Value::Object(own)) = doc.get("keys") {
            for (k, v) in own {
                m.insert(k.clone(), v.clone());
            }
        }
        entries(&m).into_iter().map(|(k, v)| (k.clone(), js_string(Some(v)))).collect()
    };
    let mut to: Vec<(String, String)> = Vec::new();
    let mut added: Vec<String> = Vec::new();
    for (ch, color) in entries(legend) {
        if !is_key(ch) {
            return Err(usage(format!(
                "object.map.set: legend key {} must be one printable character, not space or '.'",
                stringify(&json!(ch))
            )));
        }
        let Some(color) = color.as_str() else {
            return Err(usage(format!("object.map.set: legend key {ch} must name a palette color")));
        };
        let Some(entry) = sources.colors.get(color) else {
            return Err(usage(format!("object.map.set: no palette color named {color} (pixellab palette get)")));
        };
        if entry.tier == "outfit" && !r.character {
            return Err(failed(format!("{color} is an outfit color; only objects that extend character may use it"), "color_not_allowed"));
        }
        let known = known_of(doc);
        let key = edit::key_for(doc, &known, color);
        if !known.has(key.as_str()) {
            added.push(format!("{key}: {color}"));
        }
        to.push((ch.clone(), key));
    }
    let own = known_of(doc);
    let mut unknown: Vec<String> = Vec::new();
    let out: Vec<String> = rows
        .iter()
        .map(|row| {
            row.chars()
                .map(|c| {
                    let ch = c.to_string();
                    if ch == "." {
                        return ch;
                    }
                    if let Some((_, key)) = to.iter().find(|(k, _)| *k == ch) {
                        return key.clone();
                    }
                    if own.has(ch.as_str()) {
                        return ch;
                    }
                    if !unknown.contains(&ch) {
                        unknown.push(ch.clone());
                    }
                    ch
                })
                .collect::<String>()
        })
        .collect();
    if !unknown.is_empty() {
        let list = unknown.iter().map(|c| stringify(&json!(c))).collect::<Vec<_>>().join(", ");
        return Err(usage(format!(
            "object.map.set: {list} {} of this object or the legend; give them in --keys (char → color)",
            if unknown.len() == 1 { "isn't a key" } else { "aren't keys" }
        )));
    }
    Ok((out, added))
}

/// Puts the working doc where the sources can see it (JS: `ctx.invalidate()` then `ctx.sources()`).
fn refresh(sources: &mut Sources, name: &str, doc: &Value) {
    sources.objects.insert(name.to_string(), doc.clone());
    sources.extents.borrow_mut().clear();
}

fn set_map_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let name = object_at(ctx, opt_str(&params, "name").unwrap_or(""))?;
    let rows: Vec<String> =
        params.get("rows").and_then(Value::as_array).map(|l| l.iter().map(|r| js_string(Some(r))).collect()).unwrap_or_default();
    let (layer, frame) = (index(&params, "layer"), index(&params, "frame"));
    let width = rows.first().map_or(0, |r| js_len(r));
    let bad = rows.iter().position(|row| js_len(row) != width);
    if width == 0 {
        return Err(usage("object.map.set: rows are empty"));
    }
    if let Some(bad) = bad {
        return Err(usage(format!("object.map.set: row {bad} is {} wide; every row must be {width}, like row 0", js_len(&rows[bad]))));
    }
    let height = rows.len();
    if width as i64 > CAPS.size || height as i64 > CAPS.size {
        return Err(failed(format!("{width}×{height} is over the size cap, {}×{}", CAPS.size, CAPS.size), "size_cap"));
    }
    let want_resize = flag(&params, "resize");
    let legend = params.get("keys").and_then(Value::as_object).cloned().unwrap_or_default();
    ctx.edit(DocKind::Object, &name, None, |doc, loaded| {
        if doc.get("kind").and_then(Value::as_str) == Some("block") {
            return Err(failed(
                format!("{name} is a block; its maps are its surface and sides, which object.paint paints"),
                "not_editable",
            ));
        }
        let mut sources = loaded.sources.clone();
        let mut r = resolved(&sources, &name)?;
        if let Some(blocked) = paint_blocked(doc, &r, layer) {
            return Err(failed(blocked, "paint_blocked"));
        }
        let (is_loop, loop_len) = match &r.layers[layer] {
            SpriteLayer::Frames { frames, .. } => (true, frames.len()),
            SpriteLayer::Map { .. } => (false, 1),
        };
        if is_loop && frame >= loop_len {
            return Err(usage(format!("there's no frame {frame}; the loop has {loop_len}")));
        }
        if !is_loop && frame > 0 {
            return Err(usage(format!("layer {layer} is a still map, so it has only frame 0")));
        }
        if width as i64 != r.width || height as i64 != r.height {
            if truthy(doc.get("extends")) {
                return Err(failed(
                    format!("{name} extends {} and takes its size ({}×{}) from it", js_string(doc.get("extends")), r.width, r.height),
                    "size_mismatch",
                ));
            }
            if !want_resize {
                return Err(failed(
                    format!(
                        "{name} is {}×{}; these rows are {width}×{height}. Give --resize to resize every map first.",
                        r.width, r.height
                    ),
                    "size_mismatch",
                ));
            }
            edit::resize(doc, width, height);
            refresh(&mut sources, &name, doc);
            r = resolved(&sources, &name)?;
        }
        let (keyed, added) = rekey(&sources, &r, doc, &rows, &legend)?;
        if truthy(doc.get("extends")) {
            let base_name = js_string(doc.get("extends"));
            let base = resolve(&sources, &base_name)?;
            let base_layer =
                base.layers.get(layer).ok_or_else(|| Error::Internal("Cannot read properties of undefined (reading 'map')".into()))?;
            let base_map = edit::map_at(base_layer, 0);
            let mut mine = Map::new();
            for (y, row) in keyed.iter().enumerate() {
                if base_map.get(y) != Some(row) {
                    mine.insert(y.to_string(), json!(row));
                }
            }
            let mut overrides = match doc.get("rows") {
                Some(Value::Object(m)) => m.clone(),
                _ => Map::new(),
            };
            let lk = layer.to_string();
            if !mine.is_empty() {
                overrides.insert(lk, Value::Object(mine));
            } else {
                overrides.shift_remove(&lk);
            }
            let mut list: Vec<(f64, String)> =
                engine::serialize::keys(&overrides).into_iter().map(|k| (engine::jsnum::parse(k), k.clone())).collect();
            list.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let sorted: Map<String, Value> = list.into_iter().map(|(_, k)| (k.clone(), overrides[&k].clone())).collect();
            doc.as_object_mut().expect("an object source is an object").insert("rows".into(), Value::Object(sorted));
        } else if is_loop {
            doc["layers"][layer]["frames"][frame] = json!(keyed);
        } else {
            doc["layers"][layer]["map"] = json!(keyed);
        }
        refresh(&mut sources, &name, doc);
        let colors = used_colors(&sources, &resolved(&sources, &name)?).len();
        if colors > CAPS.colors {
            return Err(failed(format!("That map gives {name} {colors} colors; an object paints at most {}", CAPS.colors), "color_cap"));
        }
        let at = format!("layer {layer}{}", if is_loop { format!(", frame {frame}") } else { String::new() });
        Ok(Edit::new(
            obj(vec![("size", json!([width, height])), ("colors", json!(colors)), ("keysAdded", json!(added))]),
            format!("Set the map of {at}: {width}×{height}, {} in the object", plural(colors, "color")),
        ))
    })
}

type Run = fn(&mut Ctx, Params) -> Result<Value>;

fn command(name: &'static str, summary: &'static str, params: Value, run: Run, args: Vec<&'static str>) -> Command {
    let mut c = Command::new(name, summary, params, true, run);
    c.args = args;
    c.format = Some(format_change);
    c
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut paint = command(
        "object.paint",
        "Paint pixels of an object a palette color, on a layer and frame",
        json!({
            "type": "object",
            "properties": { "name": name_param(), "pixels": pixels_param(), "color": color_param(), "layer": layer_param(), "frame": frame_param() },
            "required": ["name", "pixels", "color"]
        }),
        paint_run,
        vec!["name", "pixels"],
    );
    paint.description = Some(
        "Pixels are object.render coordinates at scale 1 (a sprite's map column and row). A new color gets a key, as the lab gives one. An object that extends another paints row overrides; a block paints its top surface and side maps. Outfit colors are only for objects that extend character, and an object paints at most 12 colors. Rapid repeats coalesce into one undo step.",
    );
    paint.examples =
        vec!["pixellab object paint pine 3,4 4,4 --color grass-2", "pixellab object paint pine --pixels 3,4 --color ink --layer 1"];

    let erase = command(
        "object.erase",
        "Erase pixels of an object (make them transparent), on a layer and frame",
        json!({
            "type": "object",
            "properties": { "name": name_param(), "pixels": pixels_param(), "layer": layer_param(), "frame": frame_param() },
            "required": ["name", "pixels"]
        }),
        erase_run,
        vec!["name", "pixels"],
    );

    let mut fill = command(
        "object.fill",
        "Flood-fill from a pixel with a palette color, on a layer and frame",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "x": { "type": "integer" },
                "y": { "type": "integer" },
                "color": color_param(),
                "layer": layer_param(),
                "frame": frame_param()
            },
            "required": ["name", "x", "y", "color"]
        }),
        fill_run,
        vec!["name"],
    );
    fill.description = Some(
        "As the lab's Fill: the 4-connected pixels that share the start pixel's key (edit.floodFill); on a block, those of the start pixel's color on the same face (edit.blockFill).",
    );
    fill.examples = vec!["pixellab object fill pine --x 3 --y 4 --color grass-1"];

    let mut pick = Command::new(
        "object.pick",
        "The color at a pixel of an object, on a layer and frame (the Picker)",
        json!({
            "type": "object",
            "properties": { "name": name_param(), "x": { "type": "integer" }, "y": { "type": "integer" }, "layer": layer_param(), "frame": frame_param() },
            "required": ["name", "x", "y"]
        }),
        false,
        pick_run,
    );
    pick.description =
        Some("A sprite's color on that layer; a block's as it's drawn. Returns { color, hex }, both null for a transparent pixel.");
    pick.args = vec!["name"];
    pick.format = Some(pick_format);

    let layer_add = command(
        "object.layer.add",
        "Add a blank layer on top of an object",
        json!({ "type": "object", "properties": { "name": name_param() }, "required": ["name"] }),
        layer_add_run,
        vec!["name"],
    );
    let layer_duplicate = command(
        "object.layer.duplicate",
        "Duplicate a layer, just above it",
        json!({ "type": "object", "properties": { "name": name_param(), "layer": layer_param() }, "required": ["name"] }),
        layer_duplicate_run,
        vec!["name", "layer"],
    );
    let layer_delete = command(
        "object.layer.delete",
        "Delete a layer (not the only one)",
        json!({ "type": "object", "properties": { "name": name_param(), "layer": layer_param() }, "required": ["name", "layer"] }),
        layer_delete_run,
        vec!["name", "layer"],
    );
    let layer_move = command(
        "object.layer.move",
        "Move a layer up (--by 1, raise) or down (--by -1, lower)",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "layer": layer_param(),
                "by": { "type": "integer", "description": "steps up (positive) or down (negative)" }
            },
            "required": ["name", "layer", "by"]
        }),
        layer_move_run,
        vec!["name", "layer"],
    );

    let frame_add =
        command("object.frame.add", "Add a blank frame after a frame of a layer's loop", frame_params(vec![]), frame_add_run, vec!["name"]);
    let frame_duplicate = command(
        "object.frame.duplicate",
        "Duplicate a frame of a loop, just after it",
        frame_params(vec![]),
        frame_duplicate_run,
        vec!["name"],
    );
    let frame_delete =
        command("object.frame.delete", "Delete a frame of a loop (not the only one)", frame_params(vec![]), frame_delete_run, vec!["name"]);
    let mut move_params =
        frame_params(vec![("by", json!({ "type": "integer", "description": "steps later (positive) or earlier (negative)" }))]);
    move_params["required"] = json!(["name", "by"]);
    let frame_move = command(
        "object.frame.move",
        "Move a frame of a loop later (--by 1) or earlier (--by -1)",
        move_params,
        frame_move_run,
        vec!["name"],
    );

    let mut animate = command(
        "object.animate",
        "Turn a still layer into one of the site's frame loops, or switch a loop's kind",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "layer": layer_param(),
                "loop": { "type": "string", "enum": ["ff", "wf", "hf"], "default": "ff", "description": "ff: 4 frames, 600 ms; wf: 5, 1 s; hf: 6, 3 s" }
            },
            "required": ["name"]
        }),
        animate_run,
        vec!["name"],
    );
    animate.description = Some(
        "As the lab's Animate this layer: the map becomes frame 0 and is copied into every other frame of the loop (edit.animateLayer). On a layer that's already a loop, it plays as the other loop instead, gaining copies of its last frame if it needs more (edit.setLoop).",
    );

    let still = command(
        "object.still",
        "Turn a frame loop back into a still layer, keeping one frame",
        frame_params(vec![]),
        still_run,
        vec!["name"],
    );

    let resize = command(
        "object.resize",
        "Pad or crop a sprite's maps to a width and height (at the right and bottom)",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "width": { "type": "integer", "minimum": 1, "maximum": 64, "description": "the new width (default: as it is)" },
                "height": { "type": "integer", "minimum": 1, "maximum": 64, "description": "the new height (default: as it is)" }
            },
            "required": ["name"]
        }),
        resize_run,
        vec!["name"],
    );

    let mut footprint = command(
        "object.footprint",
        "Set how many tiles a sprite covers (--to w,d), or clear it (--clear)",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "to": footprint_param(),
                "clear": { "type": "boolean", "default": false, "description": "take the footprint off (a sprite then covers 1,1, or what it extends)" }
            },
            "required": ["name"]
        }),
        footprint_run,
        vec!["name"],
    );
    footprint.description = Some(
        "A sprite sorts against its neighbours in depth order by the ground it covers: a footprint of [wide, deep] tiles from the tile it's placed on, so a house, a locomotive or the crane mast go in front of and behind things by the tiles under them. The default is 1,1. A block's size is its footprint, so blocks refuse this. An object that extends another has the other's footprint unless it sets its own. The scenes that use it are put in order again at their next edit; check warns until then (scene.sort does it now).",
    );
    footprint.examples = vec!["pixellab object footprint house --to 2,2", "pixellab object footprint house --clear"];

    let mut category = command(
        "object.category",
        "Set or clear the category of one or more objects",
        json!({
            "type": "object",
            "properties": {
                "names": { "type": "array", "items": { "type": "string" }, "minItems": 1, "description": "the objects (names, or objects/<name>)" },
                "category": {
                    "type": ["array", "null"],
                    "items": { "type": "string" },
                    "description": "the category as names, outermost first, or null to clear it"
                },
                "to": { "type": "string", "description": "the category as a path: Nature/Trees or Nature › Trees" },
                "clear": { "type": "boolean", "default": false, "description": "remove the category" }
            },
            "required": ["names"]
        }),
        category_run,
        vec!["names"],
    );
    category.description = Some(
        "A category is a path of names, outermost first (Nature › Trees), that groups an object under a tab and heading in the Library. A name is 1 to 32 letters, digits, spaces, &, ' or -, with no space at either end or doubled, and All and Others can't be the outermost (they are the Library's own tabs). Names compare without regard to case. A category never changes how an object renders or what Publish writes. Give --to on the command line, or category (a list, or null to clear) in JSON. Each object is its own edit and undo step; the command checks every object and the path first.",
    );
    category.examples = vec!["pixellab object category pine tree --to \"Nature/Trees\"", "pixellab object category pine --clear"];

    let mut footprint_new = footprint_param();
    footprint_new["description"] = json!("the tiles a new sprite covers, for depth order: w,d (default 1,1)");
    let mut create = command(
        "object.new",
        "Make a new object from a starter: a blank sprite, a block, or one that extends another",
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "the new object's name (lowercase kebab-case)" },
                "kind": { "type": "string", "enum": ["sprite", "block"], "default": "sprite" },
                "extends": { "type": "string", "description": "the object it extends (a sprite, such as character)" },
                "width": { "type": "integer", "minimum": 1, "maximum": 64, "description": "a new sprite's width (default 16)" },
                "height": { "type": "integer", "minimum": 1, "maximum": 64, "description": "a new sprite's height (default 16)" },
                "footprint": footprint_new
            },
            "required": ["name"]
        }),
        new_run,
        vec!["name"],
    );
    create.description = Some(
        "The same starters as the lab's New dialog and `npm run art -- --new` (starter.rs): a sprite of --width × --height (16×16 by default) anchored at its bottom center (--footprint says how many tiles it covers, 1,1 unless given), a one-level grass block, or an object that --extends another with no rows overridden yet.",
    );
    create.examples = vec![
        "pixellab object new small-rock --width 12 --height 8",
        "pixellab object new shed --width 40 --height 32 --footprint 2,1",
        "pixellab object new tile-sand --kind block",
        "pixellab object new range-hat --extends character",
    ];

    let mut set_map = command(
        "object.map.set",
        "Replace a whole map of a sprite's layer (or frame of a loop) from rows, in one undo step",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "rows": {
                    "type": "array",
                    "items": { "type": "string" },
                    "minItems": 1,
                    "description": "the map's rows, top to bottom; on the command line each row a positional or the flag repeated"
                },
                "keys": { "type": "object", "description": "a legend: character → palette color, as JSON on the command line" },
                "layer": layer_param(),
                "frame": frame_param(),
                "resize": { "type": "boolean", "default": false, "description": "resize the object to the rows' size first" }
            },
            "required": ["name", "rows"]
        }),
        set_map_run,
        vec!["name", "rows"],
    );
    set_map.description = Some(
        "Rows are strings of one character per pixel, top to bottom, '.' transparent. With --keys (a legend, { char: color }), each legend character becomes the object's key for that color, made if it has none, so a generated map can use any characters; without, the characters are the object's own keys. Checks the palette, outfit colors (only for objects that extend character), the 12-color cap and the 64×64 size cap. A size other than the object's is refused unless --resize, which pads or crops every map first (as object.resize). On an object that extends another, a still layer's rows become row overrides. Blocks are refused: paint their surface and sides.",
    );
    set_map.examples = vec!["pixellab object map set pebble .gg. gwwg gwwg .gg. --keys '{\"g\": \"grass-2\", \"w\": \"ink\"}'"];

    vec![
        paint,
        erase,
        fill,
        pick,
        layer_add,
        layer_duplicate,
        layer_delete,
        layer_move,
        frame_add,
        frame_duplicate,
        frame_delete,
        frame_move,
        animate,
        still,
        resize,
        footprint,
        category,
        create,
        set_map,
    ]
}
