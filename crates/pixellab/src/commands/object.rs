//! object.list, object.show and object.render: read-only object commands
//! (ports cli/lib/commands/object.mjs).

use engine::engine::{describe_object, layer_maps, resolve, used_colors, used_keys, Sources, SpriteLayer};
use serde_json::{json, Map, Value};

use super::common::{join, obj, opt_num, opt_str, pad_end, str_of};
use crate::context::{Ctx, DocKind};
use crate::errors::{CommandError, Result};
use crate::registry::{Command, Params};
use crate::render::{parse_play, render_object_image, ObjectImageOptions, MAX_SCALE};

fn name_param() -> Value {
    json!({ "type": "string", "description": "object name (or objects/<name>)" })
}

/// A summary of one resolved object, or its error if it doesn't resolve,
/// and the resolved object (`summary`).
fn summary(sources: &Sources, name: &str) -> (Map<String, Value>, Option<engine::engine::Resolved>) {
    let doc = sources.objects.get(name);
    let kind = if doc.and_then(|d| d.get("kind")).and_then(Value::as_str) == Some("block") { "block" } else { "sprite" };
    let mut base = obj(vec![("name", json!(name)), ("kind", json!(kind))]);
    if let Some(ext) = doc.and_then(|d| d.get("extends")).filter(|e| engine::engine::truthy(Some(e))) {
        base.insert("extends".into(), ext.clone());
    }
    let resolved = match resolve(sources, name) {
        Ok(r) => r,
        Err(e) => {
            base.insert("error".into(), json!(e.0));
            return (base, None);
        }
    };
    let frames = resolved.layers.iter().map(|l| layer_maps(l).len()).max().unwrap_or(0).max(1);
    base.insert("size".into(), json!([resolved.width, resolved.height]));
    if let Some(block) = &resolved.block {
        base.insert("blockSize".into(), json!(block.size));
    }
    base.insert("layers".into(), json!(resolved.layers.len()));
    base.insert("frames".into(), json!(frames));
    base.insert("colors".into(), json!(used_colors(sources, &resolved).len()));
    base.insert("description".into(), json!(describe_object(sources, &resolved)));
    (base, Some(resolved))
}

fn list_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let kind = opt_str(&params, "kind");
    let loaded = ctx.sources()?;
    let mut objects = Vec::new();
    for name in loaded.sources.objects.keys() {
        let (s, _) = summary(&loaded.sources, name);
        if kind.is_none() || s.get("kind").and_then(Value::as_str) == kind {
            objects.push(Value::Object(s));
        }
    }
    Ok(json!({ "objects": objects }))
}

fn list_format(r: &Value, _: &Params) -> String {
    let objects = r["objects"].as_array().cloned().unwrap_or_default();
    let w = objects.iter().map(|o| engine::engine::js_len(o["name"].as_str().unwrap_or(""))).max().unwrap_or(0).max(4);
    objects
        .iter()
        .map(|o| {
            let what = match o.get("error") {
                Some(e) => format!("error: {}", str_of(Some(e))),
                None => str_of(o.get("description")),
            };
            let ext = o.get("extends").map(|e| format!("  (extends {})", str_of(Some(e)))).unwrap_or_default();
            format!("{}  {}  {what}{ext}", pad_end(o["name"].as_str().unwrap_or(""), w), pad_end(o["kind"].as_str().unwrap_or(""), 6))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn show_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let query = opt_str(&params, "name").unwrap_or("");
    let name = ctx.find(query, Some(DocKind::Object))?.name;
    let doc = ctx.doc(DocKind::Object, &name)?;
    let loaded = ctx.sources()?;
    let sources = &loaded.sources;
    let (rest, resolved) = summary(sources, &name);
    let Some(resolved) = resolved else {
        return Err(CommandError::new(
            format!("object {name} doesn't resolve: {}; run pixellab check {name}", str_of(rest.get("error"))),
            "resolve_error",
        )
        .into());
    };
    let used: std::collections::HashSet<String> = used_keys(&resolved).into_iter().collect();
    let color = |name: &str| -> Map<String, Value> {
        let c = sources.colors.get(name);
        obj(vec![
            ("name", json!(name)),
            ("hex", c.map_or(Value::Null, |c| json!(c.hex))),
            ("tier", c.map_or(Value::Null, |c| json!(c.tier))),
        ])
    };
    let keys: Vec<Value> = resolved
        .keys
        .iter()
        .map(|(key, c)| {
            let mut m = obj(vec![("key", json!(key))]);
            m.extend(color(c));
            m.insert("used".into(), json!(used.contains(key)));
            Value::Object(m)
        })
        .collect();
    let layers: Vec<Value> = resolved
        .layers
        .iter()
        .enumerate()
        .map(|(index, l)| {
            let mut m = obj(vec![("index", json!(index))]);
            if resolved.block.is_some() {
                m.insert("surface".into(), json!(true));
            }
            m.insert("frames".into(), json!(layer_maps(l).len()));
            match l {
                SpriteLayer::Map { class: Some(class), .. } if resolved.block.is_none() => {
                    m.insert("class".into(), json!(class));
                }
                SpriteLayer::Frames { loop_name, prefix, .. } if !loop_name.is_empty() => {
                    m.insert("loop".into(), json!(loop_name));
                    m.insert("prefix".into(), json!(prefix));
                }
                _ => {}
            }
            Value::Object(m)
        })
        .collect();
    let mut out = rest;
    out.insert("file".into(), json!(engine::format::object_file(&name)));
    out.insert("anchor".into(), json!(resolved.anchor));
    // The tiles a sprite covers, for depth order (a block's size is its footprint).
    if resolved.block.is_none() {
        out.insert("footprint".into(), json!(resolved.footprint));
    }
    out.insert("character".into(), json!(resolved.character));
    if let Some(block) = &resolved.block {
        let faces: Map<String, Value> = block.faces.iter().map(|(f, c)| (f.clone(), Value::Object(color(c)))).collect();
        let sides = block.sides.as_ref().map_or(Value::Null, |s| {
            Value::Array(
                s.keys
                    .iter()
                    .map(|(key, c)| {
                        let mut m = obj(vec![("key", json!(key))]);
                        m.extend(color(c));
                        Value::Object(m)
                    })
                    .collect(),
            )
        });
        out.insert("block".into(), json!({ "size": block.size, "faces": faces, "sides": sides }));
    }
    out.insert("layers".into(), Value::Array(layers));
    out.insert("keys".into(), Value::Array(keys));
    out.insert("usedColors".into(), json!(used_colors(sources, &resolved)));
    if params.get("doc").and_then(Value::as_bool).unwrap_or(false) {
        out.insert("doc".into(), doc);
    }
    Ok(Value::Object(out))
}

fn show_format(r: &Value, _: &Params) -> String {
    let s = |k: &str| str_of(r.get(k));
    let mut lines = vec![
        format!("{}  ({})", s("name"), s("file")),
        format!(
            "kind      {}{}{}",
            s("kind"),
            r.get("extends").map(|e| format!(", extends {}", str_of(Some(e)))).unwrap_or_default(),
            if r["character"] == json!(true) { ", character" } else { "" }
        ),
        format!(
            "size      {}{}",
            join(r.get("size"), "×"),
            r.get("block").map(|b| format!(" (block {})", join(b.get("size"), "×"))).unwrap_or_default()
        ),
        format!("anchor    {}", join(r.get("anchor"), ", ")),
    ];
    if let Some(fp) = r.get("footprint") {
        let n = fp[0].as_f64().unwrap_or(0.0) * fp[1].as_f64().unwrap_or(0.0);
        lines.push(format!("footprint {} tile{}", join(Some(fp), "×"), if n == 1.0 { "" } else { "s" }));
    }
    lines.push(format!("summary   {}", s("description")));
    if let Some(Value::Object(faces)) = r.get("block").and_then(|b| b.get("faces")) {
        lines.push(format!(
            "faces     {}",
            engine::serialize::entries(faces).iter().map(|(f, c)| format!("{f} {}", str_of(c.get("name")))).collect::<Vec<_>>().join(", ")
        ));
    }
    for l in r["layers"].as_array().into_iter().flatten() {
        let mut what: Vec<String> = Vec::new();
        if l.get("surface").is_some() {
            what.push("surface".into());
        }
        if let Some(c) = l.get("class") {
            what.push(format!("class {}", str_of(Some(c))));
        }
        if let Some(lp) = l.get("loop") {
            what.push(format!("loop {} ({}0…)", str_of(Some(lp)), str_of(l.get("prefix"))));
        }
        let frames = l["frames"].as_f64().unwrap_or(0.0);
        let what = if what.is_empty() { String::new() } else { format!(", {}", what.join(", ")) };
        lines.push(format!(
            "layer {}   {} frame{}{what}",
            str_of(l.get("index")),
            str_of(l.get("frames")),
            if frames == 1.0 { "" } else { "s" }
        ));
    }
    let keys = r["keys"].as_array().cloned().unwrap_or_default();
    if !keys.is_empty() {
        lines.push(format!(
            "keys      {}",
            keys.iter()
                .map(|k| format!(
                    "{} {}{}",
                    str_of(k.get("key")),
                    str_of(k.get("name")),
                    if k["used"] == json!(true) { "" } else { " (unused)" }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    lines.join("\n")
}

fn render_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let query = opt_str(&params, "name").unwrap_or("");
    let name = ctx.find(query, Some(DocKind::Object))?.name;
    ctx.doc(DocKind::Object, &name)?;
    let loaded = ctx.sources()?;
    let options = ObjectImageOptions {
        scale: opt_num(&params, "scale").unwrap_or(1.0) as u32,
        time: opt_num(&params, "time"),
        frame: opt_num(&params, "frame").unwrap_or(0.0) as usize,
        layers: params.get("layers").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_u64).map(|n| n as usize).collect()),
        play: match opt_str(&params, "play") {
            Some(p) => Some(parse_play(p)?),
            None => None,
        },
    };
    let image = render_object_image(&loaded.sources, &name, &options)?;
    let mut out = obj(vec![("object", json!(name))]);
    if let Value::Object(m) = image {
        out.extend(m);
    }
    Ok(Value::Object(out))
}

fn render_format(r: &Value, _: &Params) -> String {
    format!(
        "{}: {}×{} PNG at {}×, viewBox {}",
        str_of(r.get("object")),
        str_of(r.get("width")),
        str_of(r.get("height")),
        str_of(r.get("scale")),
        join(r.get("viewBox"), ", ")
    )
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut list = Command::new(
        "object.list",
        "List every object with its kind, size, layers, frames and colors",
        json!({ "type": "object", "properties": { "kind": { "type": "string", "enum": ["sprite", "block"], "description": "only objects of this kind" } } }),
        false,
        list_run,
    );
    list.format = Some(list_format);

    let mut show = Command::new(
        "object.show",
        "Show an object's kind, size, anchor, footprint, layers, frames, colors and extends",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "doc": { "type": "boolean", "default": false, "description": "also return the object's source data as doc" }
            },
            "required": ["name"]
        }),
        false,
        show_run,
    );
    show.args = vec!["name"];
    show.format = Some(show_format);

    let mut render = Command::new(
        "object.render",
        "Render one object to a transparent PNG",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "scale": { "type": "integer", "minimum": 1, "maximum": MAX_SCALE, "default": 1, "description": "whole-pixel zoom of the PNG" },
                "frame": { "type": "integer", "minimum": 0, "default": 0, "description": "the frame every loop shows (loops shorter than this show nothing)" },
                "time": { "type": "integer", "minimum": 0, "description": "ms into the loops; overrides frame" },
                "play": super::scene::play_param(),
                "layers": {
                    "type": "array",
                    "items": { "type": "integer", "minimum": 0 },
                    "description": "only these layers of a sprite, each at --frame or its last frame, as the lab's stage shows them (hidden layers, onion skin)"
                }
            },
            "required": ["name"]
        }),
        false,
        render_run,
    );
    render.description = Some(
        "The anchor is at (0, 0). A sprite's viewBox is its whole map, so at scale 1 PNG pixel (x, y) is map column x, row y; a block's is the box around its pixels. Returns the same shape as scene.render, with one item. --time also places the layers' ambient loops, and --play <action>@<ms> (with --time) places the layers that have that action where it has them <ms> into it; with --time the viewBox grows to hold the moved layers (the map's box, plus what moved outside it), and viewBox says where. On the command line, --out <file.png> writes the PNG.",
    );
    render.args = vec!["name"];
    render.writes_png = true;
    render.examples = vec![
        "pixellab object render pine --scale 4 --out /tmp/pine.png",
        "pixellab object render character --time 0 --play hop@120 --out /tmp/hop.png",
    ];
    render.format = Some(render_format);

    vec![list, show, render]
}
