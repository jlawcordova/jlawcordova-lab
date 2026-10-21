//! check: the engine's validation problems, for every source or for one doc
//! and what it uses, and warnings that don't fail it, each with a code
//! (ports cli/lib/commands/check.mjs): off_grid motion, out_of_order scenes
//! and the copies a moving item is drawn in.

use engine::engine::{describe_object, is_in_depth_order, motion_copies, resolve, validate, Sources};
use engine::omap::{OMap, OSet};
use serde_json::{json, Map, Value};

use super::common::{obj, opt_str, str_of};
use crate::context::{Ctx, DocKind};
use crate::errors::Result;
use crate::motion_ops::{off_grid_moves, off_grid_text};
use crate::registry::{Command, Params};

/// The files a check of one doc covers: palette.json, the doc, and every
/// object it uses (by extends, items and motion poses) (`dependencies`).
pub fn dependencies(sources: &Sources, kind: DocKind, name: &str) -> OSet<String> {
    let mut files: OSet<String> = OSet::new();
    files.add(engine::format::PALETTE_FILE.to_string());
    fn add_object(sources: &Sources, files: &mut OSet<String>, object: Option<&Value>) {
        let mut n = object.and_then(Value::as_str).map(str::to_string);
        while let Some(name) = n {
            let file = engine::format::object_file(&name);
            if files.has(&file) {
                break;
            }
            files.add(file);
            n = sources.objects.get(&name).and_then(|o| o.get("extends")).and_then(Value::as_str).map(str::to_string);
        }
    }
    fn add_items(sources: &Sources, files: &mut OSet<String>, items: Option<&Value>) {
        for item in items.and_then(Value::as_array).into_iter().flatten() {
            if engine::engine::truthy(item.get("group")) {
                add_items(sources, files, item.get("items"));
            } else if item.get("object").is_some_and(Value::is_string) {
                add_object(sources, files, item.get("object"));
            }
        }
    }
    if kind == DocKind::Object {
        add_object(sources, &mut files, Some(&json!(name)));
    } else {
        files.add(engine::format::scene_file(name));
        let scene = sources.scenes.get(name);
        add_items(sources, &mut files, scene.and_then(|s| s.get("items")));
        let tracks = scene.and_then(|s| s.get("motion")).and_then(|m| m.get("tracks")).and_then(Value::as_array);
        for track in tracks.into_iter().flatten() {
            for key in track.get("keys").and_then(Value::as_array).into_iter().flatten() {
                add_object(sources, &mut files, key.get("pose"));
            }
        }
    }
    files
}

/// `slice(0, indexOf(': '))`: the file a problem line starts with.
pub fn file_of(problem: &str) -> String {
    match problem.find(": ") {
        Some(at) => problem[..at].to_string(),
        None => {
            let mut units: Vec<u16> = problem.encode_utf16().collect();
            units.pop();
            String::from_utf16_lossy(&units)
        }
    }
}

/// How many items a sort would move in a scene, as scene.sort counts them
/// (`misplaced`).
fn misplaced(sources: &Sources, scene: &Value) -> engine::Result<usize> {
    let mut tracks: OMap<String, Value> = OMap::new();
    for track in scene.get("motion").and_then(|m| m.get("tracks")).and_then(Value::as_array).into_iter().flatten() {
        if let Some(item) = track.get("item").and_then(Value::as_str) {
            tracks.set(item.to_string(), track.clone());
        }
    }
    let origin = engine::engine::origin_of(scene);
    fn walk(sources: &Sources, items: &[Value], origin: [f64; 2], tracks: &OMap<String, Value>) -> engine::Result<usize> {
        let mut n = engine::depth::displaced(&engine::engine::depth_order(sources, items, origin, tracks)?);
        for item in items {
            if engine::engine::truthy(item.get("group")) {
                let inner = item.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
                n += walk(sources, &inner, origin, tracks)?;
            }
        }
        Ok(n)
    }
    let items = scene.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
    walk(sources, &items, origin, &tracks)
}

fn entry(text: &str) -> Map<String, Value> {
    let file = file_of(text);
    let message = text.get(file.len() + 2..).unwrap_or("").to_string();
    obj(vec![("file", json!(file)), ("message", json!(message)), ("text", json!(text))])
}

fn with(mut m: Map<String, Value>, more: Vec<(&str, Value)>) -> Value {
    for (k, v) in more {
        m.insert(k.to_string(), v);
    }
    Value::Object(m)
}

/// The depth warnings of a scene whose sources are sound (`depthWarnings`).
fn depth_warnings(sources: &Sources, all: &[String], n: &str) -> Vec<Value> {
    let file = engine::format::scene_file(n);
    let Some(scene) = sources.scenes.get(n) else { return vec![] };
    let deps = dependencies(sources, DocKind::Scene, n);
    if deps.iter().any(|f| all.iter().any(|p| file_of(p) == *f)) {
        return vec![];
    }
    let inner = || -> engine::Result<Vec<Value>> {
        let mut out = Vec::new();
        if !is_in_depth_order(sources, scene)? {
            let moves = misplaced(sources, scene)?;
            out.push(with(
                entry(&format!("{file}: items are not in depth order: {moves} would move (pixellab scene sort {n} puts them right)")),
                vec![("code", json!("out_of_order")), ("scene", json!(n)), ("moves", json!(moves))],
            ));
        }
        for (item, copies) in motion_copies(sources, n)? {
            out.push(with(
                entry(&format!("{file}: {item} is drawn {copies} times as it moves among other items (once for each place it takes in the depth order)")),
                vec![("code", json!("copies")), ("scene", json!(n)), ("item", json!(item)), ("copies", json!(copies))],
            ));
        }
        Ok(out)
    };
    inner().unwrap_or_default()
}

fn warnings_of(sources: &Sources, all: &[String], names: &[String]) -> Vec<Value> {
    let mut out = Vec::new();
    for n in names {
        if let Some(scene) = sources.scenes.get(n) {
            for w in off_grid_moves(scene) {
                let text = off_grid_text(&engine::format::scene_file(n), &w);
                out.push(with(entry(&text), vec![("code", json!("off_grid")), ("scene", json!(n))]));
            }
        }
        out.extend(depth_warnings(sources, all, n));
    }
    out
}

/// The sources on disk whose text isn't what saving them writes (`serialize(parse(text)) != text`),
/// among `only` when it is given (the files a check of one doc covers).
fn canonical_warnings(ctx: &mut Ctx, only: Option<&OSet<String>>) -> Result<Vec<Value>> {
    let dir = ctx.document()?.source_dir.clone();
    let mut out = Vec::new();
    for file in ctx.files()?.keys().filter(|f| f.ends_with(".json") && only.is_none_or(|o| o.has(*f))) {
        let Ok(text) = std::fs::read_to_string(dir.join(file)) else { continue };
        let Ok(doc) = engine::format::parse(&text) else { continue };
        if engine::format::serialize_file(file, &doc) != text {
            let message = "isn't in canonical form; saving it in Pixel Lab or pixellab rewrites it";
            out.push(with(entry(&format!("{file}: {message}")), vec![("code", json!("not_canonical"))]));
        }
    }
    Ok(out)
}

fn count_items(items: &[Value]) -> usize {
    items
        .iter()
        .map(|item| {
            if engine::engine::truthy(item.get("group")) {
                count_items(item.get("items").and_then(Value::as_array).map_or(&[][..], |v| v.as_slice()))
            } else {
                1
            }
        })
        .sum()
}

fn run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let loaded = ctx.sources()?;
    let sources = &loaded.sources;
    let query = opt_str(&params, "name");
    let not_canonical = if query.is_none() { Some(canonical_warnings(ctx, None)?) } else { None };
    let all: Vec<String> = loaded.load_errors.iter().cloned().chain(validate(sources)).collect();
    let Some(query) = query else {
        let names: Vec<String> = sources.scenes.keys().cloned().collect();
        return Ok(Value::Object(obj(vec![
            ("ok", json!(all.is_empty())),
            ("target", Value::Null),
            ("checked", json!(1 + sources.objects.len() + sources.scenes.len())),
            ("problems", Value::Array(all.iter().map(|p| Value::Object(entry(p))).collect())),
            ("warnings", Value::Array(warnings_of(sources, &all, &names).into_iter().chain(not_canonical.unwrap_or_default()).collect())),
        ])));
    };
    let target = ctx.find(query, opt_str(&params, "kind").and_then(DocKind::parse))?;
    let files = dependencies(sources, target.kind, &target.name);
    let mine: Vec<&String> = all.iter().filter(|p| files.has(&file_of(p))).collect();
    let mut summary: Option<String> = None;
    if mine.is_empty() {
        if target.kind == DocKind::Object {
            summary = Some(describe_object(sources, &resolve(sources, &target.name)?));
        } else if let Some(scene) = sources.scenes.get(&target.name) {
            let items = scene.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
            let objects = files.iter().filter(|f| f.starts_with("objects/")).count();
            let vb = |i: usize| str_of(scene.get("viewBox").and_then(|v| v.get(i)));
            let output = match scene.get("output") {
                Some(o) if engine::engine::truthy(Some(o)) => format!("output {}", str_of(Some(o))),
                _ => "preview only".to_string(),
            };
            summary = Some(format!("{}×{} · {} items · {objects} objects · {output}", vb(2), vb(3), count_items(&items)));
        }
    }
    let mut warnings = if target.kind == DocKind::Scene { warnings_of(sources, &all, std::slice::from_ref(&target.name)) } else { vec![] };
    warnings.extend(canonical_warnings(ctx, Some(&files))?);
    let mut out = obj(vec![
        ("ok", json!(mine.is_empty())),
        ("target", json!({ "kind": target.kind.as_str(), "name": target.name, "file": target.file })),
        ("checked", json!(files.len())),
        ("problems", Value::Array(mine.iter().map(|p| Value::Object(entry(p))).collect())),
        ("warnings", Value::Array(warnings)),
    ]);
    if let Some(s) = summary.filter(|s| !s.is_empty()) {
        out.insert("summary".into(), json!(s));
    }
    Ok(Value::Object(out))
}

fn format(r: &Value, _: &Params) -> String {
    let warnings = r["warnings"].as_array().cloned().unwrap_or_default();
    let w = warnings.len();
    let mut tail: Vec<String> = warnings.iter().map(|p| format!("warning: {}", str_of(p.get("text")))).collect();
    if w > 0 {
        tail.push(format!("{w} warning{}", if w == 1 { "" } else { "s" }));
    }
    let problems = r["problems"].as_array().cloned().unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    if r["ok"] == json!(true) {
        lines.push(match r.get("target").filter(|t| engine::engine::truthy(Some(t))) {
            Some(t) => format!("{}: ok · {}", str_of(t.get("file")), str_of(r.get("summary"))),
            None => format!("ok · {} files, no problems", str_of(r.get("checked"))),
        });
    } else {
        lines.extend(problems.iter().map(|p| str_of(p.get("text"))));
        let n = problems.len();
        lines.push(format!("{n} problem{}", if n == 1 { "" } else { "s" }));
    }
    lines.extend(tail);
    lines.join("\n")
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut check = Command::new(
        "check",
        "Validate the sources with the engine, all of them or one doc",
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "an object or scene (or objects/<name>, scenes/<name>); every source when left out" },
                "kind": { "type": "string", "enum": ["object", "scene"], "description": "which kind name is, when both exist" }
            }
        }),
        false,
        run,
    );
    check.description = Some(concat!(
        "Problems are the engine's \"<file>: <where>: <rule>\" lines. With a name, only the problems in that doc and the files it uses ",
        "(palette.json, the objects it places or extends) are listed. On the command line, the exit code is 1 when there are problems. ",
        "Warnings don't fail the check, and each has a code. off_grid: a scene's key-to-key moves (the loop back to the first key included) whose steps ",
        "aren't 1 px straight, 1:1 or 2:1, so the item jumps instead of walking the grid; scene.motion.path makes on-grid keys. ",
        "out_of_order: a scene whose file lists its items out of depth order (it still publishes in depth order; scene.sort writes the file in order), with how many items would move. ",
        "copies: a moving item that takes several places among the others as it moves is drawn once for each place in the SVG, which adds to its size; the warning says how many."
    ));
    check.args = vec!["name"];
    check.examples = vec!["pixellab check", "pixellab check hero-island", "pixellab check objects/pine --json"];
    check.format = Some(format);
    vec![check]
}
