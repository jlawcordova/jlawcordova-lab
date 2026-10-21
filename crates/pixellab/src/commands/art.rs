//! art.build, art.preview and art.sizes: publishing a document's scenes to
//! the SVGs a site inlines, the review PNGs an agent looks at, and where a
//! compiled scene's bytes go (ports cli/lib/commands/art.mjs;
//! docs/intents/2026-10-pixellab-document/spec.md, R7, R8).

use std::path::{Path, PathBuf};
use std::time::Instant;

use engine::compile::{compile_all, drop_head, head_sources, scene_sizes};
use engine::engine::{load_sources, validate, Sources};
use engine::omap::OMap;
use engine::preview::{encode_png, preview_colors, render_palette, render_preview, Target};
use serde_json::{json, Map, Value};

use super::check::{dependencies, file_of};
use super::common::{obj, opt_num, opt_str, pad_end, pad_start, str_of};
use crate::context::{Ctx, DocKind};
use crate::errors::{CommandError, Error, Result};
use crate::registry::{Command, Params};
use crate::render::{base64, MAX_SCALE};

/// Problems in the files a target uses, as the site's `--check` scoped them.
fn problems_for(sources: &Sources, problems: &[String], kind: DocKind, name: &str) -> Vec<String> {
    let files = dependencies(sources, kind, name);
    problems.iter().filter(|p| files.has(&file_of(p))).cloned().collect()
}

fn invalid(what: &str, problems: &[String]) -> Error {
    CommandError::new(
        format!("{what} has {} problem{}:\n{}", problems.len(), if problems.len() == 1 { "" } else { "s" }, problems.join("\n")),
        "invalid",
    )
    .with_details(json!(problems))
    .into()
}

fn path_text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

// ------------------------------------------------------------------ art.build

fn build_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let started = Instant::now();
    let doc = ctx.document()?;
    let out: Option<PathBuf> = match opt_str(&params, "dir").filter(|d| !d.is_empty()) {
        Some(dir) => Some(crate::open_docs::resolve(Path::new(dir))),
        None => doc.publish_dir.clone(),
    };
    let Some(out) = out else {
        return Err(CommandError::new(
            "this document has no publish folder: add \"publish\": { \"dir\": \"<folder>\" } to its manifest.json, or pass --dir <folder>",
            "no_publish_dir",
        )
        .into());
    };
    // The files on disk, not anyone's unsaved working copies.
    let docs = doc.load_docs();
    let budgets = doc.budgets.clone();
    let sources = load_sources(docs.palette.unwrap_or(Value::Null), docs.objects, docs.scenes);
    let problems: Vec<String> = docs.load_errors.into_iter().chain(validate(&sources)).collect();
    let r = compile_all(&sources, &problems, &out, &budgets)?;
    let unsaved: Vec<Value> = match ctx.session.as_deref() {
        Some(s) => s.list().into_iter().filter(|st| st["dirty"] == json!(true)).map(|st| st["doc"].clone()).collect(),
        None => vec![],
    };
    let outputs: Vec<Value> = r
        .outputs
        .iter()
        .map(|o| {
            json!({ "file": o.file, "scene": o.scene, "kind": "svg", "raw": o.raw, "gzip": o.gzip, "over": o.over, "report": o.report, "warnings": o.warnings })
        })
        .collect();
    let preview_only: Vec<Value> = r
        .preview_only
        .iter()
        .map(|p| json!({ "scene": p.scene, "raw": p.raw, "gzip": p.gzip, "over": p.over, "report": p.report, "warnings": p.warnings }))
        .collect();
    // Every output's warnings in one list, "<file>: <warning>" (R4).
    let warnings: Vec<String> = r.outputs.iter().flat_map(|o| o.warnings.iter().map(move |w| format!("{}: {w}", o.file))).collect();
    Ok(Value::Object(obj(vec![
        ("ok", json!(r.ok)),
        ("publishDir", json!(path_text(&out))),
        ("outputs", Value::Array(outputs)),
        ("previewOnly", Value::Array(preview_only)),
        ("problems", json!(r.problems)),
        ("warnings", json!(warnings)),
        ("ms", json!(started.elapsed().as_millis() as u64)),
        ("unsaved", Value::Array(unsaved)),
    ])))
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

fn build_format(r: &Value, _: &Params) -> String {
    let list = |k: &str| r[k].as_array().cloned().unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    for o in list("outputs") {
        lines.push(format!("{}: {}", str_of(o.get("file")), str_of(o.get("report"))));
    }
    for p in list("previewOnly") {
        lines.push(format!("{} (preview only, not written): {}", str_of(p.get("scene")), str_of(p.get("report"))));
    }
    let problems = list("problems");
    lines.extend(problems.iter().map(|p| str_of(Some(p))));
    let outputs = list("outputs").len();
    if !problems.is_empty() {
        lines.push(format!("{}; nothing written.", plural(problems.len(), "problem")));
    } else {
        let secs = engine::jsnum::to_fixed(r["ms"].as_f64().unwrap_or(0.0) / 1000.0, 1);
        lines.push(format!("published {} to {} in {secs} s", plural(outputs, "file"), str_of(r.get("publishDir"))));
    }
    let unsaved = list("unsaved");
    if !unsaved.is_empty() {
        lines.push(format!("not built (unsaved): {}", unsaved.iter().map(|u| str_of(Some(u))).collect::<Vec<_>>().join(", ")));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------- art.preview

fn preview_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let name = opt_str(&params, "name").unwrap_or("").to_string();
    let scale = opt_num(&params, "scale").unwrap_or(1.0) as u32;
    let loaded = ctx.sources()?;
    let doc = ctx.document()?;
    let sources = &loaded.sources;
    let problems: Vec<String> = loaded.load_errors.iter().cloned().chain(validate(sources)).collect();
    let colors = preview_colors(doc.preview_card.as_deref(), doc.preview_grid.as_deref()).map_err(Error::Internal)?;
    let mut found = "palette".to_string();
    let image = if name == "palette" {
        let prefix = format!("{}: ", engine::format::PALETTE_FILE);
        let mine: Vec<String> = problems.iter().filter(|p| p.starts_with(&prefix)).cloned().collect();
        if !mine.is_empty() {
            return Err(invalid(engine::format::PALETTE_FILE, &mine));
        }
        render_palette(sources, colors)
    } else {
        let kind = opt_str(&params, "kind").and_then(DocKind::parse);
        let target = ctx.find(&name, kind)?;
        let mine = problems_for(sources, &problems, target.kind, &target.name);
        if !mine.is_empty() {
            return Err(invalid(&target.file, &mine));
        }
        found = target.kind.as_str().to_string();
        let t = if target.kind == DocKind::Object { Target::Object(&target.name) } else { Target::Scene(&target.name) };
        render_preview(sources, t, colors)?.0
    };
    Ok(Value::Object(obj(vec![
        ("name", json!(name)),
        ("kind", json!(found)),
        ("scale", json!(scale)),
        ("width", json!(image.width * scale)),
        ("height", json!(image.height * scale)),
        ("png", json!(base64(&encode_png(&image, scale)))),
    ])))
}

fn preview_format(r: &Value, _: &Params) -> String {
    format!("{}: {}×{} preview PNG at {}×", str_of(r.get("name")), str_of(r.get("width")), str_of(r.get("height")), str_of(r.get("scale")))
}

// ------------------------------------------------------------------ art.sizes

/// One row of the objects or colors table.
#[derive(Clone, Copy)]
struct Row {
    placed: Option<usize>,
    raw: i64,
    gzip: i64,
}

fn rows(now: &OMap<String, Row>, head: Option<&OMap<String, Row>>, key: &str) -> Vec<Value> {
    let mut keys: Vec<String> = now.keys().cloned().collect();
    if let Some(h) = head {
        for k in h.keys() {
            if !keys.contains(k) {
                keys.push(k.clone());
            }
        }
    }
    let raw = |k: &String| now.get(k).map_or(-1, |r| r.raw);
    let head_raw = |k: &String| head.and_then(|h| h.get(k)).map_or(0, |r| r.raw);
    keys.sort_by(|a, b| raw(b).cmp(&raw(a)).then_with(|| head_raw(b).cmp(&head_raw(a))).then_with(|| a.cmp(b)));
    keys.iter()
        .map(|k| {
            let (n, h) = (now.get(k), head.and_then(|h| h.get(k)));
            let mut m = Map::new();
            m.insert(key.to_string(), json!(k));
            if let Some(p) = n.and_then(|n| n.placed) {
                m.insert("placed".into(), json!(p));
            }
            if let Some(n) = n {
                m.insert("raw".into(), json!(n.raw));
                m.insert("gzip".into(), json!(n.gzip));
            }
            if let Some(h) = h {
                m.insert("headRaw".into(), json!(h.raw));
                m.insert("headGzip".into(), json!(h.gzip));
            }
            Value::Object(m)
        })
        .collect()
}

fn object_rows(s: &engine::compile::SceneSizes) -> OMap<String, Row> {
    let mut m = OMap::new();
    for (k, c) in s.objects.iter() {
        m.set(k.clone(), Row { placed: Some(c.placed), raw: c.raw, gzip: c.gzip });
    }
    m
}

fn color_rows(s: &engine::compile::SceneSizes) -> OMap<String, Row> {
    let mut m = OMap::new();
    for (k, c) in s.colors.iter() {
        m.set(k.clone(), Row { placed: None, raw: c.raw, gzip: c.gzip });
    }
    m
}

fn sizes_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let loaded = ctx.sources()?;
    let target = ctx.find(opt_str(&params, "name").unwrap_or(""), Some(DocKind::Scene))?;
    let sources = &loaded.sources;
    let all: Vec<String> = loaded.load_errors.iter().cloned().chain(validate(sources)).collect();
    let problems = problems_for(sources, &all, target.kind, &target.name);
    if !problems.is_empty() {
        return Err(invalid(&target.file, &problems));
    }
    let now = scene_sizes(sources, &target.name)?;
    let root = ctx.document()?.root.clone();
    let head = head_sources(&root);
    let mut reason: Option<String> = None;
    let mut before: Option<engine::compile::SceneSizes> = None;
    let result = (|| -> engine::Result<()> {
        match &head {
            Err(r) => reason = Some(r.clone()),
            Ok(h) => {
                if !h.sources.scenes.contains_key(&target.name) {
                    reason = Some(format!("{} isn't at HEAD", target.file));
                } else if !problems_for(&h.sources, &h.problems, target.kind, &target.name).is_empty() {
                    reason = Some("its sources at HEAD have problems".into());
                } else {
                    before = Some(scene_sizes(&h.sources, &target.name)?);
                }
            }
        }
        Ok(())
    })();
    if let Ok(h) = &head {
        drop_head(h);
    }
    result?;
    let scene = sources.scenes.get(&target.name);
    let output = scene.and_then(|s| s.get("output")).cloned().unwrap_or(Value::Null);
    let head_value = match (&reason, &before) {
        (Some(r), _) => json!({ "reason": r }),
        (None, Some(b)) => json!({ "raw": b.raw, "gzip": b.gzip }),
        _ => json!({}),
    };
    let head_objects = before.as_ref().map(object_rows);
    let head_colors = before.as_ref().map(color_rows);
    let budget = engine::compile::scene_budget(sources, &target.name, &ctx.document()?.budgets);
    let warnings = engine::compile::budget_warnings(budget, now.raw, now.gzip);
    Ok(Value::Object(obj(vec![
        ("scene", json!(target.name)),
        ("output", output),
        ("now", json!({ "raw": now.raw, "gzip": now.gzip })),
        ("head", head_value),
        ("objects", Value::Array(rows(&object_rows(&now), head_objects.as_ref(), "object"))),
        ("colors", Value::Array(rows(&color_rows(&now), head_colors.as_ref(), "color"))),
        ("warnings", json!(warnings)),
    ])))
}

/// `n.toLocaleString('en-US')` for a whole number.
fn locale(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

fn int(v: Option<&Value>) -> Option<i64> {
    v.and_then(Value::as_i64)
}

fn bytes(n: Option<i64>) -> String {
    n.map_or("—".to_string(), locale)
}

fn delta(a: Option<i64>, b: Option<i64>) -> String {
    match (a, b) {
        (Some(a), Some(b)) => format!("{}{}", if a - b >= 0 { "+" } else { "" }, locale(a - b)),
        _ => "—".to_string(),
    }
}

fn table(lines: &mut Vec<String>, title: &str, caption: &str, list: &[Value], placed: bool) {
    let mut header: Vec<String> = vec![title.to_string()];
    if placed {
        header.push("placed".into());
    }
    header.extend(["raw", "Δ raw", "gzip", "Δ gzip"].map(String::from));
    let body: Vec<Vec<String>> = list
        .iter()
        .map(|row| {
            let mut cells = vec![str_of(row.get(title))];
            if placed {
                cells.push(bytes(int(row.get("placed"))));
            }
            cells.push(bytes(int(row.get("raw"))));
            cells.push(delta(int(row.get("raw")), int(row.get("headRaw"))));
            cells.push(bytes(int(row.get("gzip"))));
            cells.push(delta(int(row.get("gzip")), int(row.get("headGzip"))));
            cells
        })
        .collect();
    let widths: Vec<usize> = (0..header.len())
        .map(|i| std::iter::once(&header).chain(body.iter()).map(|l| engine::engine::js_len(&l[i])).max().unwrap_or(0))
        .collect();
    lines.push(String::new());
    lines.push(caption.to_string());
    for l in std::iter::once(&header).chain(body.iter()) {
        lines.push(
            l.iter()
                .enumerate()
                .map(|(i, c)| if i == 0 { pad_end(c, widths[i]) } else { pad_start(c, widths[i]) })
                .collect::<Vec<_>>()
                .join("  "),
        );
    }
}

fn sizes_format(r: &Value, _: &Params) -> String {
    let now = &r["now"];
    let head = &r["head"];
    let title = match r.get("output") {
        Some(Value::Null) | None => format!("{} (preview only)", str_of(r.get("scene"))),
        Some(o) => str_of(Some(o)),
    };
    let mut lines = vec![format!("{title}: {} B raw, {} B gzip", bytes(int(now.get("raw"))), bytes(int(now.get("gzip"))))];
    if engine::engine::truthy(head.get("reason")) {
        lines.push(format!("HEAD: not available ({})", str_of(head.get("reason"))));
    } else {
        lines.push(format!(
            "HEAD: {} B raw, {} B gzip ({} raw, {} gzip)",
            bytes(int(head.get("raw"))),
            bytes(int(head.get("gzip"))),
            delta(int(now.get("raw")), int(head.get("raw"))),
            delta(int(now.get("gzip")), int(head.get("gzip")))
        ));
    }
    let objects = r["objects"].as_array().cloned().unwrap_or_default();
    let colors = r["colors"].as_array().cloned().unwrap_or_default();
    table(&mut lines, "object", "Bytes each object adds (the scene compiled without it), against HEAD:", &objects, true);
    table(&mut lines, "color", "Bytes each color's path takes, against HEAD:", &colors, false);
    let warnings = r["warnings"].as_array().cloned().unwrap_or_default();
    lines.extend(warnings.iter().map(|w| format!("warning: {}: {}", str_of(r.get("scene")), str_of(Some(w)))));
    lines.join("\n")
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut build = Command::new(
        "art.build",
        "Publish: validate every source, compile each scene with an output and write the SVGs",
        json!({ "type": "object", "properties": { "dir": { "type": "string", "description": "the folder to write the SVGs to (else the manifest's publish.dir)" } } }),
        false,
        build_run,
    );
    build.description = Some(concat!(
        "Validates the document's files on disk (unsaved working copies aren't in it; unsaved lists them), renders each scene with an `output` to a rect SVG, ",
        "merges each color group into one <path>, checks the result lossless and within its budget (100 KB raw / 25 KB gzip, or the manifest's), and writes <publish dir>/<output>. ",
        "The publish folder is the manifest's publish.dir (relative to the document), or --dir; with neither it refuses. Nothing is written when any source has a problem. ",
        "Returns { ok, publishDir, outputs: [{ file, scene, raw, gzip, over, report }], previewOnly: [{ scene, raw, gzip, over, report }], problems, ms, unsaved }."
    ));
    build.writes_direct = true;
    build.examples = vec!["pixellab art build", "pixellab art build --dir ~/Desktop/art"];
    build.format = Some(build_format);

    let mut preview = Command::new(
        "art.preview",
        "Render the review PNG of an object (standing on a tile) or a scene, on the card color; `palette` for the swatch sheet",
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "an object or scene (or objects/<name>, scenes/<name>), or palette" },
                "scale": { "type": "integer", "minimum": 1, "maximum": MAX_SCALE, "default": 1, "description": "whole-pixel zoom of the PNG" },
                "kind": { "type": "string", "enum": ["object", "scene"], "description": "which kind name is, when both exist" }
            },
            "required": ["name"]
        }),
        false,
        preview_run,
    );
    preview.description = Some(concat!(
        "The picture an agent or a person checks art by: an object on one 32×16 tile outline, a scene at its viewBox (first variant, frame 0), both on the document's card color (manifest preview.card). ",
        "Refuses (code invalid) when the doc or what it uses has problems. Returns { name, kind, scale, width, height, png } with the PNG as base64; --out writes it."
    ));
    preview.args = vec!["name"];
    preview.writes_png = true;
    preview.examples =
        vec!["pixellab art preview pine --scale 4 --out /tmp/pine@4x.png", "pixellab art preview palette --out /tmp/palette.png"];
    preview.format = Some(preview_format);

    let mut sizes = Command::new(
        "art.sizes",
        "Bytes each object and each color adds to a compiled scene, against the document's git HEAD",
        json!({
            "type": "object",
            "properties": { "name": { "type": "string", "description": "a scene (or scenes/<name>)" } },
            "required": ["name"]
        }),
        false,
        sizes_run,
    );
    sizes.description = Some(concat!(
        "The bytes a compiled scene loses, raw and gzip, when an object's items are left out or a color's paths are dropped (costs overlap a little), now and as committed at HEAD of the repository the document is in. ",
        "Returns { scene, output, now: { raw, gzip }, head: { raw, gzip } | { reason }, objects: [{ object, placed, raw, gzip, headRaw, headGzip }], colors: [{ color, raw, gzip, headRaw, headGzip }] }."
    ));
    sizes.args = vec!["name"];
    sizes.examples = vec!["pixellab art sizes hero-island"];
    sizes.format = Some(sizes_format);

    vec![build, preview, sizes]
}
