//! check: the engine's validation problems, for every source or for one doc
//! and what it uses, and warnings that don't fail it, each with a code
//! (ports cli/lib/commands/check.mjs): off_grid motion, out_of_order scenes
//! and the copies a moving item is drawn in.

use engine::compile::{scene_budget_warnings, Budgets};
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

/// A published scene's size against its budget (R4): a warning, never a problem.
fn budget_warnings(sources: &Sources, all: &[String], n: &str, budgets: &Budgets) -> Vec<Value> {
    let Some(output) = sources.scenes.get(n).and_then(|s| s.get("output")).and_then(Value::as_str).filter(|o| !o.is_empty()) else {
        return vec![];
    };
    let deps = dependencies(sources, DocKind::Scene, n);
    if deps.iter().any(|f| all.iter().any(|p| file_of(p) == *f)) {
        return vec![];
    }
    let Ok(over) = scene_budget_warnings(sources, n, budgets) else { return vec![] };
    over.into_iter()
        .map(|w| {
            let text = format!("{}: {output} is {w}", engine::format::scene_file(n));
            with(entry(&text), vec![("code", json!("over_budget")), ("scene", json!(n)), ("output", json!(output))])
        })
        .collect()
}

/// A fixed viewBox that cuts off painted pixels (scene-canvas spec R11): a warning, never a problem, so a
/// document that has always cropped on purpose still builds. `scene.frame --fit` would fit it to its content.
fn cropped_warnings(sources: &Sources, all: &[String], n: &str) -> Vec<Value> {
    let deps = dependencies(sources, DocKind::Scene, n);
    if deps.iter().any(|f| all.iter().any(|p| file_of(p) == *f)) {
        return vec![];
    }
    let Ok(outside) = engine::engine::pixels_outside_frame(sources, n) else { return vec![] };
    if outside == 0 {
        return vec![];
    }
    let text = format!(
        "{}: {outside} pixel{} outside its viewBox (the published SVG cuts {} off; pixellab scene frame {n} --fit fits it to its content)",
        engine::format::scene_file(n),
        if outside == 1 { "" } else { "s" },
        if outside == 1 { "it" } else { "them" }
    );
    vec![with(entry(&text), vec![("code", json!("cropped")), ("scene", json!(n)), ("pixels", json!(outside))])]
}

/// A scene with nothing in it publishes one tile around the origin (scene-canvas spec R6): a note.
fn empty_warnings(sources: &Sources, all: &[String], n: &str) -> Vec<Value> {
    let Some(scene) = sources.scenes.get(n) else { return vec![] };
    let deps = dependencies(sources, DocKind::Scene, n);
    if deps.iter().any(|f| all.iter().any(|p| file_of(p) == *f)) || scene.get("viewBox").is_some_and(Value::is_array) {
        return vec![];
    }
    match engine::engine::resolve_frame(sources, n) {
        Ok(frame) if frame.painted.is_none() => {
            let text = format!("{}: is empty; it publishes one tile around the origin", engine::format::scene_file(n));
            vec![with(entry(&text), vec![("code", json!("empty")), ("scene", json!(n))])]
        }
        _ => vec![],
    }
}

/// An object that has states and also layer ambient loops or actions (R8): the states decide what plays, so those never do.
fn states_warnings(sources: &Sources, all: &[String], name: &str) -> Vec<Value> {
    let Some(doc) = sources.objects.get(name) else { return vec![] };
    let deps = dependencies(sources, DocKind::Object, name);
    if deps.iter().any(|f| all.iter().any(|p| file_of(p) == *f)) {
        return vec![];
    }
    let loops: Vec<usize> = doc
        .get("layers")
        .and_then(Value::as_array)
        .map(|l| l.iter().enumerate().filter(|(_, l)| l.get("ambient").is_some() || l.get("actions").is_some()).map(|(i, _)| i).collect())
        .unwrap_or_default();
    if doc.get("states").is_none() || loops.is_empty() {
        return vec![];
    }
    let layers = loops.iter().map(|i| format!("layer {i}")).collect::<Vec<_>>().join(", ");
    let text = format!(
        "{}: has states and an ambient loop or actions on {layers}; the states decide what plays, so those never play (pixellab object states make turns them into states)",
        engine::format::object_file(name)
    );
    vec![with(entry(&text), vec![("code", json!("states_and_loops")), ("object", json!(name))])]
}

/// An item whose track keys a state belongs to the scene (R17): a page's `data-state` doesn't change it. A group that carries
/// `data-state` or `data-play` around one is a page's switch that can't reach it, so that is a warning.
fn owned_state_warnings(sources: &Sources, all: &[String], n: &str) -> Vec<Value> {
    let Some(scene) = sources.scenes.get(n) else { return vec![] };
    let deps = dependencies(sources, DocKind::Scene, n);
    if deps.iter().any(|f| all.iter().any(|p| file_of(p) == *f)) {
        return vec![];
    }
    let keyed: Vec<String> = scene
        .pointer("/motion/tracks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|t| engine::states::keys_states(t))
        .filter_map(|t| t.get("item").and_then(Value::as_str).map(str::to_string))
        .collect();
    if keyed.is_empty() {
        return vec![];
    }
    fn walk(list: &[Value], path: &str, switch: Option<&str>, keyed: &[String], n: &str, out: &mut Vec<Value>) {
        for (i, item) in list.iter().enumerate() {
            let here = if path.is_empty() { i.to_string() } else { format!("{path}.{i}") };
            if let Some(attrs) = item.get("group").and_then(Value::as_object) {
                let own = ["data-state", "data-play"].into_iter().find(|a| attrs.contains_key(*a));
                let inner = item.get("items").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
                walk(inner, &here, own.map(|_| here.as_str()).or(switch), keyed, n, out);
            } else if let (Some(group), Some(id)) =
                (switch, item.get("id").and_then(Value::as_str).filter(|id| keyed.iter().any(|k| k == id)))
            {
                let text = format!(
                    "{}: {id}'s track keys a state, so the scene plays it and the data-state or data-play on the group at {group} doesn't switch it",
                    engine::format::scene_file(n)
                );
                out.push(with(entry(&text), vec![("code", json!("scene_owns_state")), ("scene", json!(n)), ("item", json!(id))]));
            }
        }
    }
    let mut out = Vec::new();
    walk(scene.get("items").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]), "", None, &keyed, n, &mut out);
    out
}

fn warnings_of(sources: &Sources, all: &[String], names: &[String], budgets: &Budgets) -> Vec<Value> {
    let mut out = Vec::new();
    for n in names {
        if let Some(scene) = sources.scenes.get(n) {
            for w in off_grid_moves(scene) {
                let text = off_grid_text(&engine::format::scene_file(n), &w);
                out.push(with(entry(&text), vec![("code", json!("off_grid")), ("scene", json!(n))]));
            }
        }
        out.extend(owned_state_warnings(sources, all, n));
        out.extend(depth_warnings(sources, all, n));
        out.extend(cropped_warnings(sources, all, n));
        out.extend(empty_warnings(sources, all, n));
        out.extend(budget_warnings(sources, all, n, budgets));
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

/// `check --library`: a `.pixellib` against its own recorded colors (object-libraries spec, D3).
fn library_run(path: &str) -> Result<Value> {
    let path = super::library::expand(path);
    let lib = crate::library::read_library(&path)?;
    let problems = crate::library::library_problems(&lib);
    let file = path.to_string_lossy().into_owned();
    let checked = 1 + lib.objects.len();
    let n = lib.objects.len();
    Ok(Value::Object(obj(vec![
        ("ok", json!(problems.is_empty())),
        ("target", json!({ "kind": "library", "name": lib.name, "file": file })),
        ("checked", json!(checked)),
        ("problems", Value::Array(problems.iter().map(|p| Value::Object(entry(p))).collect())),
        ("warnings", json!([])),
        ("summary", json!(format!("{n} object{} from {}", if n == 1 { "" } else { "s" }, lib.exported_from))),
    ])))
}

fn run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    if let Some(library) = opt_str(&params, "library").filter(|l| !l.is_empty()) {
        if opt_str(&params, "name").is_some() {
            return Err(crate::errors::CommandError::usage("check: give a name or --library, not both").into());
        }
        return library_run(library);
    }
    let loaded = ctx.sources()?;
    let budgets = ctx.document()?.budgets.clone();
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
            (
                "warnings",
                Value::Array(
                    sources
                        .objects
                        .keys()
                        .flat_map(|n| states_warnings(sources, &all, n))
                        .chain(warnings_of(sources, &all, &names, &budgets))
                        .chain(not_canonical.unwrap_or_default())
                        .collect(),
                ),
            ),
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
            // The size it publishes at: its own viewBox, else the fit to what it paints (scene-canvas spec R7).
            let frame = engine::engine::resolve_frame(sources, &target.name).ok();
            let vb = |i: usize| match &frame {
                Some(f) => f.view_box[i].to_string(),
                None => str_of(scene.get("viewBox").and_then(|v| v.get(i))),
            };
            let output = match scene.get("output") {
                Some(o) if engine::engine::truthy(Some(o)) => format!("output {}", str_of(Some(o))),
                _ => "preview only".to_string(),
            };
            summary = Some(format!("{}×{} · {} items · {objects} objects · {output}", vb(2), vb(3), count_items(&items)));
        }
    }
    let mut warnings = match target.kind {
        DocKind::Scene => warnings_of(sources, &all, std::slice::from_ref(&target.name), &budgets),
        DocKind::Object => states_warnings(sources, &all, &target.name),
        DocKind::Palette => vec![],
    };
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
                "kind": { "type": "string", "enum": ["object", "scene"], "description": "which kind name is, when both exist" },
                "library": { "type": "string", "description": "check this .pixellib instead of the document: its objects against the colors it recorded, extends within it, categories" }
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
        "copies: a moving item that takes several places among the others as it moves is drawn once for each place in the SVG, which adds to its size; the warning says how many. ",
        "cropped: a scene with a fixed viewBox paints pixels outside it, which the published SVG cuts off; the warning says how many (scene.frame --fit fits it to its content). ",
        "empty: a scene with nothing in it and no viewBox, which publishes one tile around the origin. ",
        "states_and_loops: a sprite with states that also has layer ambient loops or actions, which never play since the states decide what plays (object.states.make turns them into states). ",
        "scene_owns_state: an item whose motion track keys a state sits in a group with data-state or data-play: the scene plays it, so the page's attribute can't switch it. ",
        "With library, it checks an object library instead (no document needed): its objects against the colors it recorded at export, extends resolving inside it, and categories."
    ));
    check.no_document_with = Some("library");
    check.args = vec!["name"];
    check.examples = vec![
        "pixellab check",
        "pixellab check hero-island",
        "pixellab check objects/pine --json",
        "pixellab check --library ~/Art/explainer.pixellib",
    ];
    check.format = Some(format);
    vec![check]
}
