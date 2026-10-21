//! scene.list, scene.show, scene.items and scene.render (ports cli/lib/commands/scene.mjs).

use engine::engine::{js_len, js_space, js_string, truthy};
use engine::omap::OSet;
use engine::serialize::{entries, ordered};
use serde_json::{json, Map, Value};

use super::common::{flag, join, obj, opt_num, opt_str, pad_end, pad_start, str_of};
use crate::context::{Ctx, DocKind};
use crate::errors::{CommandError, Result};
use crate::registry::{Command, Params};
use crate::render::{flatten_items, parse_path, render_scene_image, SceneImageOptions, Theme, MAX_SCALE};

fn name_param() -> Value {
    json!({ "type": "string", "description": "scene name (or scenes/<name>)" })
}

/// `RENDER_PARAMS`: scale and time, as the properties of a params schema.
pub fn render_params() -> Value {
    json!({
        "scale": { "type": "integer", "minimum": 1, "maximum": MAX_SCALE, "default": 1, "description": "whole-pixel zoom of the PNG" },
        "time": { "type": "integer", "minimum": 0, "description": "ms into the animation: frame loops and motion tracks (default 0, the rest frame)" }
    })
}

/// scene.render's and proposal.render's theme (`THEME`).
pub fn theme() -> Value {
    json!({
        "type": "string",
        "enum": ["light", "dark"],
        "default": "light",
        "description": "dark draws as the site's dark theme: the hero island's drop shadow (data-enter=\"shadow\") takes the dark page color"
    })
}

// ---- scene layers, as much of layer-ops.mjs as these commands read ----
// (layer_ops.rs is U18's; these are the few functions scene.render and
// scene.place need, and can move there.)

/// The scene's layers (an empty list when it has none) (`layersOf`).
fn layers_of(doc: &Value) -> Vec<&Value> {
    doc.get("layers").and_then(Value::as_array).map(|l| l.iter().collect()).unwrap_or_default()
}

/// A layer by its id, or by its name when only one layer has it (`findLayer`).
/// Returns a copy of the layer.
pub fn find_layer(doc: &Value, reference: &str) -> Result<Value> {
    let layers = layers_of(doc);
    let is = |l: &&Value, key: &str| l.get(key).and_then(Value::as_str) == Some(reference);
    if let Some(l) = layers.iter().find(|l| is(l, "id")) {
        return Ok((*l).clone());
    }
    let named: Vec<&&Value> = layers.iter().filter(|l| is(l, "name")).collect();
    let quoted = engine::format::stringify(&Value::String(reference.to_string()));
    if named.len() == 1 {
        return Ok((**named[0]).clone());
    }
    if named.len() > 1 {
        let ids: Vec<String> = named.iter().map(|l| str_of(l.get("id"))).collect();
        return Err(CommandError::new(
            format!("{} layers are called {quoted}; give one's id ({})", named.len(), ids.join(", ")),
            "ambiguous",
        )
        .into());
    }
    let known: Vec<String> = layers.iter().map(|l| str_of(l.get("id"))).collect();
    let has = if known.is_empty() { "; it has no layers yet".to_string() } else { format!(" (it has {})", known.join(", ")) };
    Err(CommandError::new(format!("no layer {quoted} in this scene{has}"), "not_found").into())
}

/// A layer's id and the ids of every layer inside it (`withInner`).
fn with_inner(doc: &Value, id: &str) -> Vec<String> {
    let mut out = vec![id.to_string()];
    let mut grew = true;
    while grew {
        grew = false;
        for l in layers_of(doc) {
            let (Some(parent), Some(lid)) = (l.get("parent").and_then(Value::as_str), l.get("id").and_then(Value::as_str)) else {
                continue;
            };
            if out.iter().any(|o| o == parent) && !out.iter().any(|o| o == lid) {
                out.push(lid.to_string());
                grew = true;
            }
        }
    }
    out
}

/// Every layer id in `ids` and inside them (`closure`).
pub fn closure(doc: &Value, ids: &[String]) -> OSet<String> {
    let mut out = OSet::new();
    for id in ids {
        for inner in with_inner(doc, id) {
            out.add(inner);
        }
    }
    out
}

/// Sets (or with None takes off) an item's layer, keeping the canonical key
/// order object, id, class, layer, depth, at (`setItemLayer`).
pub fn set_item_layer(item: &mut Value, layer: Option<&str>) {
    let Some(map) = item.as_object_mut() else { return };
    let old = std::mem::take(map);
    let mut rebuilt = Map::new();
    let layer_value = layer.map(|l| Value::String(l.to_string()));
    for (key, value) in [
        ("object", old.get("object").cloned()),
        ("id", old.get("id").cloned()),
        ("class", old.get("class").cloned()),
        ("layer", layer_value),
        ("depth", old.get("depth").cloned()),
        ("at", old.get("at").cloned()),
    ] {
        if let Some(v) = value {
            rebuilt.insert(key.into(), v);
        }
    }
    for (key, value) in old {
        if !["object", "id", "class", "layer", "depth", "at"].contains(&key.as_str()) {
            rebuilt.insert(key, value);
        }
    }
    *map = rebuilt;
}

// ---- the commands ----

fn count_items(items: Option<&Value>) -> usize {
    items
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|item| if item.as_object().is_some_and(|o| o.contains_key("group")) { count_items(item.get("items")) } else { 1 })
                .sum()
        })
        .unwrap_or(0)
}

/// The scene's data, by name or scenes/<name>.
fn scene_doc(ctx: &mut Ctx, query: &str) -> Result<(String, Value)> {
    let name = ctx.find(query, Some(DocKind::Scene))?.name;
    let scene = ctx.doc(DocKind::Scene, &name)?;
    Ok((name, scene))
}

fn at(a: Option<&Value>) -> String {
    let Some(a) = a else { return "?".into() };
    if truthy(a.get("tile")) {
        format!("tile {}", join(a.get("tile"), ","))
    } else if truthy(a.get("px")) {
        let level = if truthy(a.get("level")) { format!(" level {}", str_of(a.get("level"))) } else { String::new() };
        format!("px {}{level}", join(a.get("px"), ","))
    } else {
        "?".into()
    }
}

fn or_null(v: Option<&Value>) -> Value {
    v.filter(|v| !v.is_null()).cloned().unwrap_or(Value::Null)
}

fn list_run(ctx: &mut Ctx, _: Params) -> Result<Value> {
    let loaded = ctx.sources()?;
    let scenes: Vec<Value> = loaded
        .sources
        .scenes
        .iter()
        .map(|(name, scene)| {
            let mut m = obj(vec![
                ("name", json!(name)),
                ("file", json!(engine::format::scene_file(name))),
                ("output", or_null(scene.get("output"))),
            ]);
            if let Some(vb) = scene.get("viewBox") {
                m.insert("viewBox".into(), vb.clone());
            }
            m.insert("origin".into(), scene.get("origin").filter(|v| !v.is_null()).cloned().unwrap_or_else(|| json!([0, 0])));
            m.insert("items".into(), json!(count_items(scene.get("items"))));
            m.insert("motion".into(), json!(truthy(scene.get("motion"))));
            Value::Object(m)
        })
        .collect();
    Ok(json!({ "scenes": scenes }))
}

fn list_format(r: &Value, _: &Params) -> String {
    let scenes = r["scenes"].as_array().cloned().unwrap_or_default();
    let w = scenes.iter().map(|s| js_len(s["name"].as_str().unwrap_or(""))).max().unwrap_or(0).max(4);
    scenes
        .iter()
        .map(|s| {
            let vb = s.get("viewBox").filter(|v| !v.is_null());
            let (vw, vh) = match vb {
                Some(vb) => (str_of(vb.get(2)), str_of(vb.get(3))),
                None => ("0".to_string(), "0".to_string()),
            };
            let output = match s.get("output") {
                Some(Value::String(o)) => o.clone(),
                _ => "(preview only)".to_string(),
            };
            format!(
                "{}  {} {} items  {}{}",
                pad_end(s["name"].as_str().unwrap_or(""), w),
                pad_end(&format!("{vw}×{vh}"), 9),
                pad_start(&str_of(s.get("items")), 4),
                output,
                if truthy(s.get("motion")) { "  · motion" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn show_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let (name, scene) = scene_doc(ctx, opt_str(&params, "name").unwrap_or(""))?;
    let flat = flatten_items(&scene);
    let items = flat["items"].as_array().cloned().unwrap_or_default();
    let mut counts = Map::new();
    for item in &items {
        let key = js_string(item.get("object"));
        let n = counts.get(&key).and_then(Value::as_u64).unwrap_or(0) + 1;
        counts.insert(key, json!(n));
    }
    let layers = flat["layers"].as_array().cloned().unwrap_or_default();
    let motion = if truthy(scene.get("motion")) {
        let motion = &scene["motion"];
        let tracks: Vec<Value> = motion
            .get("tracks")
            .and_then(Value::as_array)
            .map(|t| {
                t.iter()
                    .map(|t| {
                        let mut m = Map::new();
                        if let Some(item) = t.get("item") {
                            m.insert("item".into(), item.clone());
                        }
                        m.insert("keys".into(), json!(t.get("keys").and_then(Value::as_array).map_or(0, Vec::len)));
                        Value::Object(m)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut m = Map::new();
        if let Some(d) = motion.get("duration") {
            m.insert("duration".into(), d.clone());
        }
        m.insert("tracks".into(), Value::Array(tracks));
        Value::Object(m)
    } else {
        Value::Null
    };
    let mut out =
        obj(vec![("name", json!(name)), ("file", json!(engine::format::scene_file(&name))), ("output", or_null(scene.get("output")))]);
    if let Some(vb) = scene.get("viewBox") {
        out.insert("viewBox".into(), vb.clone());
    }
    out.insert("origin".into(), scene.get("origin").filter(|v| !v.is_null()).cloned().unwrap_or_else(|| json!([0, 0])));
    out.insert("items".into(), json!(items.len()));
    out.insert("groups".into(), json!(flat["groups"].as_array().map_or(0, Vec::len)));
    out.insert("layers".into(), json!(layers.len()));
    out.insert("layerNames".into(), Value::Array(layers.iter().filter_map(|l| l.get("name").cloned()).collect()));
    out.insert("objects".into(), Value::Object(ordered(&counts)));
    out.insert("motion".into(), motion);
    if flag(&params, "doc") {
        out.insert("doc".into(), scene);
    }
    Ok(Value::Object(out))
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn show_format(r: &Value, _: &Params) -> String {
    let s = |k: &str| str_of(r.get(k));
    let vb = r.get("viewBox").filter(|v| !v.is_null());
    let vb = |i: usize| str_of(vb.and_then(|v| v.get(i)));
    let layers = r["layers"].as_f64().unwrap_or(0.0);
    let objects = r["objects"].as_object().cloned().unwrap_or_default();
    let output = match r.get("output") {
        Some(Value::String(o)) => o.clone(),
        _ => "(preview only)".to_string(),
    };
    let groups = r["groups"].as_u64().unwrap_or(0) as usize;
    let mut lines = vec![
        format!("{}  ({})", s("name"), s("file")),
        format!("output    {output}"),
        format!("viewBox   {}, {}, {}×{}", vb(0), vb(1), vb(2), vb(3)),
        format!("origin    {}", join(r.get("origin"), ", ")),
        format!("items     {} in {} group{}", s("items"), groups, plural(groups)),
        format!(
            "layers    {}{}",
            s("layers"),
            if layers != 0.0 { format!(": {}", join(r.get("layerNames"), ", ")) } else { String::new() }
        ),
        format!(
            "objects   {}: {}",
            objects.len(),
            entries(&objects)
                .iter()
                .map(|(o, n)| if n.as_f64().unwrap_or(0.0) > 1.0 { format!("{o} ×{}", str_of(Some(n))) } else { (*o).clone() })
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ];
    if let Some(m) = r.get("motion").filter(|m| !m.is_null()) {
        let tracks = m["tracks"].as_array().cloned().unwrap_or_default();
        let items: Vec<String> = tracks.iter().map(|t| str_of(t.get("item"))).collect();
        lines.push(format!(
            "motion    {} ms, {} track{}: {}",
            str_of(m.get("duration")),
            tracks.len(),
            plural(tracks.len()),
            items.join(", ")
        ));
    }
    lines.join("\n")
}

fn items_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let (name, scene) = scene_doc(ctx, opt_str(&params, "name").unwrap_or(""))?;
    let mut out = obj(vec![("scene", json!(name))]);
    if let Value::Object(flat) = flatten_items(&scene) {
        out.extend(flat);
    }
    Ok(Value::Object(out))
}

fn items_format(r: &Value, _: &Params) -> String {
    let items = r["items"].as_array().cloned().unwrap_or_default();
    let w = items.iter().map(|i| js_len(i["path"].as_str().unwrap_or(""))).max().unwrap_or(0).max(4);
    let o = items.iter().map(|i| js_len(&str_of(i.get("object")))).max().unwrap_or(0).max(6);
    items
        .iter()
        .map(|i| {
            let mut extra: Vec<String> = Vec::new();
            if truthy(i.get("id")) {
                extra.push(format!("#{}", str_of(i.get("id"))));
            }
            if truthy(i.get("depth")) {
                extra.push(format!("[{}]", str_of(i.get("depth"))));
            }
            if truthy(i.get("class")) {
                extra.push(format!(".{}", str_of(i.get("class")).replace(' ', ".")));
            }
            if truthy(i.get("layer")) {
                extra.push(format!("^{}", str_of(i.get("layer"))));
            }
            let extra = extra.join(" ");
            let line = format!(
                "{}  {}  {}{}",
                pad_end(i["path"].as_str().unwrap_or(""), w),
                pad_end(&str_of(i.get("object")), o),
                pad_end(&at(i.get("at")), 16),
                if extra.is_empty() { String::new() } else { format!(" {extra}") }
            );
            line.trim_end_matches(js_space).to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let query = opt_str(&params, "name").unwrap_or("");
    let name = ctx.find(query, Some(DocKind::Scene))?.name;
    let scene = ctx.doc(DocKind::Scene, &name)?;
    let show = opt_str(&params, "show");
    let path = show.and_then(parse_path);
    if let (Some(show), None) = (show, &path) {
        return Err(CommandError::usage(format!("--show {show}: give an item path like 12 or 5.2")).into());
    }
    let hide_refs: Vec<&str> =
        params.get("hide").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let hidden = if hide_refs.is_empty() {
        None
    } else {
        let mut ids = Vec::new();
        for r in &hide_refs {
            ids.push(str_of(find_layer(&scene, r)?.get("id")));
        }
        Some(closure(&scene, &ids))
    };
    let focused = match opt_str(&params, "focus") {
        Some(f) => Some(closure(&scene, &[str_of(find_layer(&scene, f)?.get("id"))])),
        None => None,
    };
    let loaded = ctx.sources()?;
    let dark_page = ctx.document()?.dark_page.clone();
    let options = SceneImageOptions {
        scale: opt_num(&params, "scale").unwrap_or(1.0) as u32,
        time: opt_num(&params, "time").unwrap_or(0.0),
        show: path,
        theme: if opt_str(&params, "theme") == Some("dark") { Theme::Dark } else { Theme::Light },
        hide: hidden,
        focus: focused,
    };
    let image = render_scene_image(&loaded.sources, &name, &dark_page, &options)?;
    let mut out = obj(vec![("scene", json!(name))]);
    if let Value::Object(m) = image {
        out.extend(m);
    }
    Ok(Value::Object(out))
}

fn render_format(r: &Value, _: &Params) -> String {
    format!(
        "{}: {}×{} PNG at {}×, {} items shown, time {} ms",
        str_of(r.get("scene")),
        str_of(r.get("width")),
        str_of(r.get("height")),
        str_of(r.get("scale")),
        r["items"].as_array().map_or(0, Vec::len),
        str_of(r.get("time"))
    )
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut list = Command::new("scene.list", "List every scene", json!({ "type": "object", "properties": {} }), false, list_run);
    list.format = Some(list_format);

    let mut show = Command::new(
        "scene.show",
        "Show a scene's settings, size, objects and motion",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "doc": { "type": "boolean", "default": false, "description": "also return the scene's source data as doc" }
            },
            "required": ["name"]
        }),
        false,
        show_run,
    );
    show.args = vec!["name"];
    show.format = Some(show_format);

    let mut items = Command::new(
        "scene.items",
        "List a scene's items in depth order (back to front), with stable paths",
        json!({ "type": "object", "properties": { "name": name_param() }, "required": ["name"] }),
        false,
        items_run,
    );
    items.description = Some(concat!(
        r#"Paths index into the scene's items: "3" is the fourth top-level item, "5.2" the third item inside the group at "5". The list is in depth order "#,
        r#"(every edit keeps it so; scene.sort fixes a file that isn't): back to front, by where each item stands. "#,
        r#"Each item has its object, at ({ tile: [col, row, level] } or { px: [x, y], level? }), level (a pixel item's, 0 unless it says), depth (front or back, when it has the override), "#,
        r#"class, id, layer, enclosing group and placement point in scene pixels. A group in groups has its path, attrs, item count and depth. "#,
        r#"layers lists the scene's layers ({ id, name, parent, items }: items counts what is in it and in the layers inside it), in the order the Layers panel shows them."#
    ));
    items.args = vec!["name"];
    items.format = Some(items_format);

    let mut props = Map::new();
    props.insert("name".into(), name_param());
    if let Value::Object(rp) = render_params() {
        props.extend(rp);
    }
    props.insert("theme".into(), theme());
    props.insert(
        "show".into(),
        json!({ "type": "string", "pattern": "^\\d+(\\.\\d+)*$", "description": "an item path whose variant group to show, like 12.0" }),
    );
    props.insert(
        "hide".into(),
        json!({ "type": "array", "items": { "type": "string" }, "description": "layers (ids or names) whose items are left out" }),
    );
    props.insert(
        "focus".into(),
        json!({ "type": "string", "description": "a layer: the result adds focusPng, the render of only its items (and those of the layers inside it)" }),
    );
    let mut render = Command::new(
        "scene.render",
        "Render a scene to a transparent PNG, with item bounds",
        json!({ "type": "object", "properties": Value::Object(props), "required": ["name"] }),
        false,
        render_run,
    );
    render.description = Some(concat!(
        "Returns { png (base64), scale, viewBox, width, height, time, items: [{ path, object, at, id, bounds: [x, y, w, h] }] }. ",
        "Bounds are in output pixels (scale included), for hit-testing. Of each run of data-class variant groups only the first shows, ",
        "or the one holding --show. --hide leaves out the items in those layers (and the layers inside them), as hiding them in Pixel Lab does; ",
        "--focus adds focusPng, the same render with only that layer's items. On the command line, --out <file.png> writes the PNG."
    ));
    render.args = vec!["name"];
    render.writes_png = true;
    render.examples = vec![
        "pixellab scene render hero-island --scale 2 --out /tmp/hero.png",
        "pixellab scene render range-sprite --time 400 --json",
        "pixellab scene render hero-island --theme dark --out /tmp/hero-dark.png",
    ];
    render.format = Some(render_format);

    vec![list, show, items, render]
}
