//! library.info, library.category, library.export and library.import: object
//! libraries (`.pixellib`) and copying objects between them and a document
//! (docs/intents/2026-10-object-libraries/spec.md, R10–R17, D2–D5). The
//! package, the plans and the pure parts are in `library.rs`; this file is
//! the I/O and the one version an import makes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use engine::engine::{load_sources, validate};
use serde_json::{json, Map, Value};

use super::common::{flag, obj, opt_str, str_of};
use super::history::put_files;
use crate::context::Ctx;
use crate::document::{expand_path, home_dir, process_env};
use crate::errors::{CommandError, Result};
use crate::library::{
    category_from_param, category_of, category_text, check_conversions, imported_objects, library_problems, plan_import, read_library,
    stem, write_library, write_library_object, Exported, Fate, ImportPlan, Library, Selection, LIBRARY_EXT,
};
use crate::registry::{Command, Params};

fn usage(message: impl Into<String>) -> crate::errors::Error {
    CommandError::usage(message).into()
}

/// A path param: `~` and relative paths made absolute.
pub(crate) fn expand(path: &str) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    expand_path(path, &cwd, &home_dir(&process_env()))
}

/// `path`, with `ext` added when it doesn't end in it.
pub(crate) fn with_ext(path: PathBuf, ext: &str) -> PathBuf {
    let text = path.to_string_lossy().into_owned();
    if text.ends_with(ext) {
        path
    } else {
        PathBuf::from(format!("{text}{ext}"))
    }
}

fn library_path(params: &Params, key: &str) -> Result<PathBuf> {
    match opt_str(params, key).filter(|p| !p.is_empty()) {
        Some(p) => Ok(expand(p)),
        None => Err(usage(format!("give the library's path ({key})"))),
    }
}

fn summary_format(r: &Value, _: &Params) -> String {
    str_of(r.get("summary"))
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

fn kind_of(doc: &Value) -> &'static str {
    if doc.get("kind").and_then(Value::as_str) == Some("block") {
        "block"
    } else {
        "sprite"
    }
}

fn category_json(doc: &Value) -> Value {
    category_of(doc).map_or(Value::Null, |c| json!(c))
}

/// The distinct categories of some objects, case-insensitively, each spelled as
/// the alphabetically first object that uses it spells it (spec R2), with how
/// many objects are in it itself, in path order.
pub(crate) fn categories_of<'a>(objects: impl Iterator<Item = (&'a String, &'a Value)>) -> Vec<Value> {
    let mut seen: BTreeMap<String, (Vec<String>, usize)> = BTreeMap::new();
    for (_, doc) in objects {
        if let Some(c) = category_of(doc) {
            let key = c.iter().map(|n| n.to_lowercase()).collect::<Vec<_>>().join("\u{1}");
            seen.entry(key).or_insert_with(|| (c, 0)).1 += 1;
        }
    }
    seen.into_values().map(|(c, n)| json!({ "category": c, "text": category_text(&c), "objects": n })).collect()
}

// ------------------------------------------------------------------ info

fn colors_json(lib: &Library) -> Value {
    let tier = |t: &[(String, String)]| Value::Object(t.iter().map(|(n, h)| (n.clone(), json!(h))).collect());
    json!({ "world": tier(&lib.world), "outfit": tier(&lib.outfit) })
}

fn info_run(_: &mut Ctx, params: Params) -> Result<Value> {
    let path = library_path(&params, "library")?;
    let lib = read_library(&path)?;
    let problems = library_problems(&lib);
    let objects: Vec<Value> = lib
        .objects
        .iter()
        .map(|(name, doc)| {
            let mut row = obj(vec![("name", json!(name)), ("kind", json!(kind_of(doc)))]);
            if let Some(e) = doc.get("extends").filter(|e| e.is_string()) {
                row.insert("extends".into(), e.clone());
            }
            row.insert("category".into(), category_json(doc));
            Value::Object(row)
        })
        .collect();
    let n = objects.len();
    Ok(json!({
        "library": path.to_string_lossy(),
        "name": lib.name,
        "exportedFrom": lib.exported_from,
        "objects": objects,
        "categories": categories_of(lib.objects.iter()),
        "colors": colors_json(&lib),
        "ok": problems.is_empty(),
        "problems": problems,
        "summary": format!("{}: {} from {}", lib.name, plural(n, "object"), lib.exported_from),
    }))
}

fn info_format(r: &Value, _: &Params) -> String {
    let mut lines = vec![str_of(r.get("summary"))];
    for o in r["objects"].as_array().into_iter().flatten() {
        let cat = match o["category"].as_array() {
            Some(c) => c.iter().map(|n| str_of(Some(n))).collect::<Vec<_>>().join(" › "),
            None => "(none)".into(),
        };
        let ext = o.get("extends").map(|e| format!("  (extends {})", str_of(Some(e)))).unwrap_or_default();
        lines.push(format!("  {}  {}  [{cat}]{ext}", str_of(o.get("name")), str_of(o.get("kind"))));
    }
    let count = |tier: &str| r["colors"][tier].as_object().map_or(0, Map::len);
    lines.push(format!("colors: {} world, {} outfit", count("world"), count("outfit")));
    for p in r["problems"].as_array().into_iter().flatten() {
        lines.push(format!("problem: {}", str_of(Some(p))));
    }
    lines.join("\n")
}

// -------------------------------------------------------------- category

fn category_run(_: &mut Ctx, params: Params) -> Result<Value> {
    let has = |k: &str| params.get(k).is_some_and(|v| !v.is_null());
    let clear = flag(&params, "clear") || params.get("category").is_some_and(Value::is_null);
    if usize::from(has("category")) + usize::from(has("to")) + usize::from(clear) != 1 {
        return Err(usage("library.category: give one of --to, --clear"));
    }
    let category = if clear {
        None
    } else {
        let value = params.get("category").filter(|v| !v.is_null()).or_else(|| params.get("to")).expect("a category");
        Some(category_from_param(value).map_err(|why| usage(format!("library.category: {why}")))?)
    };
    let path = library_path(&params, "library")?;
    let names: Vec<String> =
        params.get("names").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(String::from).collect();
    if names.is_empty() {
        return Err(usage("library.category: give at least one object name"));
    }
    if !path.exists() {
        return Err(CommandError::not_found(format!("no library at {}", path.display())).into());
    }
    // One writer at a time; the lock is the library folder itself, so no file is left in it.
    let io = |e: std::io::Error| CommandError::new(format!("can't write {}: {e}", path.display()), "io");
    let lock = std::fs::File::open(&path).map_err(io)?;
    lock.lock().map_err(io)?;
    let lib = read_library(&path)?;
    let mut docs: Vec<(String, Value)> = Vec::new();
    for query in &names {
        let bare = query.strip_prefix("objects/").unwrap_or(query);
        let name = bare.strip_suffix(".json").unwrap_or(bare);
        let Some(doc) = lib.objects.get(name) else {
            return Err(CommandError::not_found(format!("no object named {query} in {}", path.display())).into());
        };
        if !docs.iter().any(|(n, _)| n == name) {
            docs.push((name.to_string(), doc.clone()));
        }
    }
    let mut results = Vec::new();
    for (name, mut doc) in docs {
        let before = doc.clone();
        let object = doc.as_object_mut().expect("an object");
        match &category {
            Some(c) => {
                object.insert("category".into(), json!(c));
            }
            None => {
                object.shift_remove("category");
            }
        }
        // The text of the file now decides: a library written by hand is made canonical too.
        let changed = doc != before
            || std::fs::read_to_string(path.join(engine::format::object_file(&name)))
                .map_or(true, |t| t != engine::format::serialize_object(&doc));
        if changed {
            write_library_object(&path, &name, &doc)?;
        }
        results.push(json!({ "name": name, "changed": doc != before }));
    }
    drop(lock);
    let changed = results.iter().any(|r| r["changed"] == json!(true));
    let shown: Vec<&str> = results.iter().filter_map(|r| r["name"].as_str()).collect();
    let what = match &category {
        Some(c) => format!("Set the category of {} to {}", shown.join(", "), category_text(c)),
        None => format!("Cleared the category of {}", shown.join(", ")),
    };
    Ok(json!({
        "library": path.to_string_lossy(),
        "changed": changed,
        "category": category.map_or(Value::Null, |c| json!(c)),
        "objects": results,
        "summary": if changed { what } else { format!("nothing changed ({what})") },
    }))
}

// ---------------------------------------------------------------- export

fn export_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let has = |k: &str| params.get(k).is_some_and(|v| !v.is_null());
    if has("objects") && has("category") {
        return Err(usage("library.export: give only one of objects, category (or neither, for every object)"));
    }
    let selection = if has("category") {
        Selection::Category(category_from_param(&params["category"]).map_err(|why| usage(format!("library.export: {why}")))?)
    } else if has("objects") {
        let names: Vec<String> = params["objects"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect();
        if names.is_empty() {
            return Err(usage("library.export: objects is an empty list"));
        }
        Selection::Names(names.iter().map(|n| n.strip_prefix("objects/").unwrap_or(n).to_string()).collect())
    } else {
        Selection::All
    };
    let to = with_ext(expand(opt_str(&params, "to").unwrap_or("")), LIBRARY_EXT);
    if opt_str(&params, "to").unwrap_or("").is_empty() {
        return Err(usage("library.export: give the library's path (to)"));
    }
    let name = stem(&to, LIBRARY_EXT);
    let from = ctx.document()?.name.clone();
    let root = ctx.document()?.root.clone();
    let (to_text, root_text) = (to.to_string_lossy().into_owned(), root.to_string_lossy().into_owned());
    if to_text == root_text || to_text.starts_with(&format!("{root_text}/")) {
        return Err(usage(format!("{to_text} is inside the document itself")));
    }
    let loaded = ctx.sources()?;
    let (lib, exported) = crate::library::build_library(&loaded.sources, &name, &from, &selection)?;
    write_library(&to, &lib, flag(&params, "replace"))?;
    let rows: Vec<Value> = exported
        .iter()
        .map(|Exported { name, base }| {
            let doc = &lib.objects[name];
            let mut row = obj(vec![("name", json!(name)), ("kind", json!(kind_of(doc))), ("category", category_json(doc))]);
            if *base {
                row.insert("base".into(), json!(true));
            }
            Value::Object(row)
        })
        .collect();
    let bases = exported.iter().filter(|e| e.base).count();
    let n = exported.len();
    let summary = format!(
        "Exported {} to {to_text}{}",
        plural(n, "object"),
        if bases > 0 { format!(" ({bases} added because others extend them)") } else { String::new() }
    );
    Ok(json!({
        "to": to_text,
        "name": name,
        "count": n,
        "objects": rows,
        "colors": colors_json(&lib),
        "summary": summary,
    }))
}

// ---------------------------------------------------------------- import

fn tier_text(tier: &str) -> String {
    format!("{} {tier}", if tier == "outfit" { "an" } else { "a" })
}

/// The plan, as the result and the app's sheet read it (spec D5).
fn plan_json(plan: &ImportPlan, lib: &Library, path: &Path) -> Value {
    let objects: Vec<Value> = plan
        .objects
        .iter()
        .map(|o| {
            let doc = &lib.objects[&o.name];
            json!({
                "name": o.name,
                "as": o.as_name,
                "fate": o.fate.as_str(),
                "kind": kind_of(doc),
                "category": category_json(doc),
            })
        })
        .collect();
    let renames: Vec<Value> = plan.renames().iter().map(|(from, to)| json!({ "from": from, "to": to })).collect();
    let same: Vec<&str> = plan.objects.iter().filter(|o| o.fate == Fate::Same).map(|o| o.name.as_str()).collect();
    let differing: Vec<Value> =
        plan.differing.iter().map(|d| json!({ "name": d.name, "tier": d.tier, "library": d.library, "document": d.document })).collect();
    let missing: Vec<Value> = plan
        .missing
        .iter()
        .map(|m| {
            json!({
                "name": m.name,
                "tier": m.tier,
                "hex": m.hex,
                "nearest": m.nearest.as_ref().map_or(Value::Null, |(n, h)| json!({ "name": n, "hex": h })),
                "usedBy": m.used_by,
            })
        })
        .collect();
    let copied = plan.copied().len();
    json!({
        "library": { "path": path.to_string_lossy(), "name": lib.name, "exportedFrom": lib.exported_from },
        "objects": objects,
        "count": copied,
        "renames": renames,
        "same": same,
        "differing": differing,
        "character": { "swapped": plan.character_swapped, "figures": plan.figures },
        "missing": missing,
        "needsAcceptDifferences": plan.differs(),
        "ready": !plan.differs() && plan.missing.is_empty(),
    })
}

/// A refusal carrying the plan, so an agent or the app can read what to answer.
fn refuse(code: &str, message: String, plan: &Value) -> crate::errors::Error {
    CommandError::new(message, code).with_details(plan.clone()).into()
}

fn convert_param(params: &Params) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    match params.get("convert") {
        None | Some(Value::Null) => {}
        Some(Value::Object(m)) => {
            for (from, to) in m {
                let Some(to) = to.as_str() else { return Err(usage(format!("convert: {from} must convert to a color name"))) };
                out.insert(from.clone(), to.to_string());
            }
        }
        Some(_) => return Err(usage("convert is an object of missing color: the document's color, like {\"wood-5\": \"wood-3\"}")),
    }
    Ok(out)
}

fn import_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let path = library_path(&params, "from")?;
    let lib = read_library(&path)?;
    let problems = library_problems(&lib);
    if !problems.is_empty() {
        return Err(CommandError::new(
            format!("{} doesn't pass check --library, so it can't be imported:\n{}", path.display(), problems.join("\n")),
            "invalid",
        )
        .with_details(Value::from(problems))
        .into());
    }
    let convert = convert_param(&params)?;
    let loaded = ctx.sources()?;
    let doc = &loaded.sources;
    let plan = plan_import(doc, &lib);
    let mut shown = plan_json(&plan, &lib, &path);
    if flag(&params, "dryRun") {
        let Value::Object(m) = &mut shown else { unreachable!() };
        m.insert("dryRun".into(), json!(true));
        let n = plan.copied().len();
        m.insert("summary".into(), json!(format!("Would import {} from {}; nothing changed", plural(n, "object"), lib.name)));
        return Ok(shown);
    }
    if let Err(p) = check_conversions(doc, &plan, &convert) {
        return Err(refuse(p.code, p.message, &shown));
    }
    if plan.differs() && !flag(&params, "acceptDifferences") {
        let mut why: Vec<String> =
            plan.differing.iter().map(|d| format!("{} is {} here and {} in the library", d.name, d.document, d.library)).collect();
        if plan.character_swapped {
            why.push(format!(
                "the library's character differs from this document's, so {} will extend this document's character and may look different",
                if plan.figures.is_empty() { "its figures".to_string() } else { plan.figures.join(", ") }
            ));
        }
        return Err(refuse(
            "differs",
            format!(
                "the imported objects would look different from the library: {}. Give acceptDifferences to import anyway.",
                why.join("; ")
            ),
            &shown,
        ));
    }
    let unconverted: Vec<&crate::library::MissingColor> = plan.missing.iter().filter(|m| !convert.contains_key(&m.name)).collect();
    if !unconverted.is_empty() {
        let list: Vec<String> = unconverted
            .iter()
            .map(|m| {
                format!(
                    "{} ({} {}, nearest {})",
                    m.name,
                    tier_text(m.tier),
                    m.hex,
                    m.nearest.as_ref().map_or("none".to_string(), |(n, h)| format!("{n} {h}"))
                )
            })
            .collect();
        return Err(refuse(
            "missing_colors",
            format!(
                "the document doesn't have {}: {}. Give convert, a color of the same tier for each, to import; converting changes how the imported objects look.",
                if list.len() == 1 { "this color" } else { "these colors" },
                list.join(", ")
            ),
            &shown,
        ));
    }
    let imported = imported_objects(&plan, &lib, &convert);
    if imported.is_empty() {
        let Value::Object(m) = &mut shown else { unreachable!() };
        m.insert("changed".into(), json!(false));
        m.insert("imported".into(), json!([]));
        m.insert("version".into(), Value::Null);
        m.insert("summary".into(), json!(format!("Nothing to import: the document already has every object of {}", lib.name)));
        return Ok(shown);
    }
    // The document with the import in it must still pass check, before anything is written.
    let mut objects = doc.objects.clone();
    for (name, object) in &imported {
        objects.insert(name.clone(), object.clone());
    }
    let after = load_sources(doc.palette.clone(), objects, doc.scenes.clone());
    let broken: Vec<String> = validate(&after)
        .into_iter()
        .filter(|p| imported.iter().any(|(n, _)| p.starts_with(&format!("{}: ", engine::format::object_file(n)))))
        .collect();
    if !broken.is_empty() {
        return Err(CommandError::new(
            format!("the imported objects don't pass check in this document, so nothing was imported:\n{}", broken.join("\n")),
            "invalid",
        )
        .with_details(Value::from(broken))
        .into());
    }
    let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let label = format!("Imported {} from {file}", plural(imported.len(), "object"));
    let targets: Vec<(String, Option<String>)> =
        imported.iter().map(|(n, o)| (engine::format::object_file(n), Some(engine::format::serialize_object(o)))).collect();
    let session = ctx.session()?;
    put_files(session, targets, &label, false, true)?;
    let version = session.history.latest();
    let Value::Object(m) = &mut shown else { unreachable!() };
    m.insert("changed".into(), json!(true));
    m.insert("imported".into(), json!(imported.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()));
    m.insert("version".into(), version.map_or(Value::Null, |v| json!(v)));
    m.insert("summary".into(), json!(label));
    Ok(shown)
}

fn import_format(r: &Value, _: &Params) -> String {
    let mut lines = vec![str_of(r.get("summary"))];
    for o in r["objects"].as_array().into_iter().flatten() {
        match o["fate"].as_str() {
            Some("renamed") => lines.push(format!("  {} -> {}", str_of(o.get("name")), str_of(o.get("as")))),
            Some("same") => lines.push(format!("  {} is already here; used as it is", str_of(o.get("name")))),
            Some("character") => lines.push(format!("  {}: this document's is used instead", str_of(o.get("name")))),
            _ => {}
        }
    }
    for d in r["differing"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  {} differs: {} here, {} in the library",
            str_of(d.get("name")),
            str_of(d.get("document")),
            str_of(d.get("library"))
        ));
    }
    for m in r["missing"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  {} ({}) is missing; nearest {} {}",
            str_of(m.get("name")),
            str_of(m.get("tier")),
            str_of(m["nearest"].get("name")),
            str_of(m["nearest"].get("hex"))
        ));
    }
    lines.join("\n")
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut info = Command::new(
        "library.info",
        "Show an object library: its objects, their categories and the colors they used",
        json!({
            "type": "object",
            "properties": { "library": { "type": "string", "description": "the .pixellib folder" } },
            "required": ["library"]
        }),
        false,
        info_run,
    );
    info.description = Some("Lists each object (name, kind, extends, category), the distinct categories with how many objects each holds, the colors recorded at export (never a palette) and the problems check --library finds. Needs no document.");
    info.args = vec!["library"];
    info.needs_document = false;
    info.examples = vec!["pixellab library info ~/Art/explainer.pixellib"];
    info.format = Some(info_format);

    let mut category = Command::new(
        "library.category",
        "Set or clear the category of objects in a .pixellib, in place",
        json!({
            "type": "object",
            "properties": {
                "library": { "type": "string", "description": "the .pixellib folder" },
                "names": { "type": "array", "items": { "type": "string" }, "minItems": 1, "description": "the objects" },
                "category": {
                    "type": ["array", "null"],
                    "items": { "type": "string" },
                    "description": "the category as names, outermost first, or null to clear it"
                },
                "to": { "type": "string", "description": "the category as a path: Nature/Trees or Nature › Trees" },
                "clear": { "type": "boolean", "default": false, "description": "remove the category" }
            },
            "required": ["library", "names"]
        }),
        true,
        category_run,
    );
    category.description = Some("Writes the library's object files in place, canonical, while holding a lock on the folder. A library has no undo and no history: it is a file, and this is the only way to edit one (no painting, adding or removing objects). The rules for a category are object.category's.");
    category.args = vec!["library", "names"];
    category.writes_direct = true;
    category.needs_document = false;
    category.examples = vec!["pixellab library category ~/Art/explainer.pixellib cat cat-white --to \"Animals/Cats\""];
    category.format = Some(summary_format);

    let mut export = Command::new(
        "library.export",
        "Export the document's objects to a new .pixellib",
        json!({
            "type": "object",
            "properties": {
                "to": { "type": "string", "description": "the new library (.pixellib is added when missing)" },
                "objects": { "type": "array", "items": { "type": "string" }, "description": "only these objects" },
                "category": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "only the objects in this category and below it: names outermost first, or a path such as Nature/Trees"
                },
                "replace": { "type": "boolean", "default": false, "description": "replace a library that is already there" }
            },
            "required": ["to"]
        }),
        true,
        export_run,
    );
    export.description = Some("Copies all of the document's objects, the objects of one category and everything below it, or a named list into a package of the same object files, with a record of the palette colors they use (never read as a palette). Every object that a chosen one extends comes too, marked base: true in the result, so the library stands on its own. Writes a temp folder beside the target and renames it into place; refuses an existing library (exists) unless replace. The document doesn't change.");
    export.args = vec!["to"];
    export.writes_direct = true;
    export.examples = vec![
        "pixellab library export ~/Art/everything.pixellib",
        "pixellab library export ~/Art/trees.pixellib --category Nature/Trees",
        "pixellab library export ~/Art/some.pixellib --objects pine,tree",
    ];
    export.format = Some(summary_format);

    let mut import = Command::new(
        "library.import",
        "Import the objects of a .pixellib into the document, by copying",
        json!({
            "type": "object",
            "properties": {
                "from": { "type": "string", "description": "the .pixellib folder" },
                "acceptDifferences": { "type": "boolean", "default": false, "description": "import although colors differ from the library's record, or its character differs" },
                "convert": {
                    "type": "object",
                    "additionalProperties": { "type": "string" },
                    "description": "for each color the document lacks, the document's color of the same tier to use instead: {\"wood-5\": \"wood-3\"}"
                },
                "dryRun": { "type": "boolean", "default": false, "description": "return the plan and change nothing" }
            },
            "required": ["from"]
        }),
        true,
        import_run,
    );
    import.description = Some(concat!(
        "Copies the library's objects, with their categories, into the document; nothing links it to the library afterwards, and the same library can be imported again. ",
        "An imported object whose name is taken gets the first free suffix (pine-2) and every extends in the import follows; the document's own objects are never renamed or changed. ",
        "One that equals the document's same-named object, apart from its category, isn't copied (fate same). A library's character that differs from the document's isn't copied: its figures extend the document's character, which counts as a difference. ",
        "The plan (dryRun returns it): objects with their fate (new, renamed, same, character), renames, differing colors, the character swap, and missing colors with each one's tier and the document's nearest color of that tier. ",
        "Refusals, each carrying the plan in its details: differs (colors that differ from the library's record, or the character swap) unless acceptDifferences; missing_colors unless convert names a color for every missing one; wrong_tier when a conversion crosses tiers (an outfit color converts only to an outfit color); invalid when the library or the result doesn't pass check. ",
        "An agent that hits differs or missing_colors asks the owner with chat_ask before accepting or converting. ",
        "The import never changes the palette, the existing objects or the scenes. It is one version in the history (Imported 12 objects from explainer.pixellib), whose number is in the result, and one undo step: history_undo takes it back."
    ));
    import.args = vec!["from"];
    import.writes_direct = true;
    import.examples = vec![
        "pixellab library import ~/Art/explainer.pixellib --dry-run",
        "pixellab library import ~/Art/explainer.pixellib --accept-differences --convert '{\"wood-5\":\"wood-3\"}'",
    ];
    import.format = Some(import_format);

    vec![info, category, export, import]
}
