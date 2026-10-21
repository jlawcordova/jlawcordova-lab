//! document.migrate (docs/intents/2026-10-rust-engine/spec.md, R8, D3):
//! converts a format-1 document (`.mjs` sources, or a site's legacy
//! `source/` folder) to format 2, in place or as a copy with `--to`. It reads
//! every source with `engine::migrate`, carries comments into notes, builds
//! every scene from the sources as read and from the format-2 text, and
//! refuses unless the SVGs are byte-identical; then it writes the JSON
//! sources and the manifest with `"format": 2`, and in place removes the
//! `.mjs` files. It refuses while the document has unsaved edits open.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use engine::compile::{compile_scene, Budgets};
use engine::engine::{load_sources, Order};
use engine::format::{parse, serialize_file, serialize_manifest};
use engine::migrate::convert;
use serde_json::{json, Map, Value};

use super::common::{obj, opt_str, str_of};
use crate::context::Ctx;
use crate::document::{
    document_name, expand_path, find_document_in_site, home_dir, resolve_document_root, site_budgets, site_colors, Via, EXTENSION,
    SITE_ART_DIR,
};
use crate::errors::{CommandError, Result};
use crate::open_docs::unsaved_in_app;
use crate::registry::{Command, Params};
use crate::state::{state_dir, write_atomic};

/// The format-1 sources of a folder, relative: palette.mjs, objects/*.mjs, scenes/*.mjs.
fn sources_of(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if root.join("palette.mjs").is_file() {
        out.push("palette.mjs".to_string());
    }
    for sub in ["objects", "scenes"] {
        let mut names: Vec<String> = fs::read_dir(root.join(sub))
            .map(|d| {
                d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|f| f.ends_with(".mjs")).collect()
            })
            .unwrap_or_default();
        names.sort();
        out.extend(names.into_iter().map(|f| format!("{sub}/{f}")));
    }
    out
}

fn json_name(file: &str) -> String {
    format!("{}.json", file.strip_suffix(".mjs").unwrap_or(file))
}

/// The sources as engine sources, from `(file, value)` pairs.
fn sources_from(values: &[(String, Value)]) -> engine::engine::Sources {
    let (mut palette, mut objects, mut scenes) = (Value::Null, BTreeMap::new(), BTreeMap::new());
    for (file, value) in values {
        if file == "palette.json" {
            palette = value.clone();
        } else if let Some(name) = file.strip_prefix("objects/").and_then(|f| f.strip_suffix(".json")) {
            objects.insert(name.to_string(), value.clone());
        } else if let Some(name) = file.strip_prefix("scenes/").and_then(|f| f.strip_suffix(".json")) {
            scenes.insert(name.to_string(), value.clone());
        }
    }
    load_sources(palette, objects, scenes)
}

/// Copies a folder's files, leaving out the format-1 sources and the manifest.
fn copy_others(from: &Path, to: &Path, rel: &str) -> std::io::Result<()> {
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        let target = to.join(&name);
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&target)?;
            copy_others(&entry.path(), &target, &path)?;
        } else if path != "manifest.json" && !((path == "palette.mjs") || ((rel == "objects" || rel == "scenes") && name.ends_with(".mjs")))
        {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn migrate_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    // Where the document is: the CLI's flags and environment, or a session's open document.
    let options = match (&ctx.open, &ctx.session) {
        (Some(o), _) => o.clone(),
        (None, Some(s)) => crate::document::OpenOptions {
            doc: Some(s.document.root.to_string_lossy().into_owned()),
            site: None,
            env: s.env.clone(),
            cwd: s.document.root.clone(),
        },
        (None, None) => crate::document::OpenOptions::default(),
    };
    let resolved = resolve_document_root(options.doc.as_deref(), options.site.as_deref(), &options.env, &options.cwd)?;
    let (root, site_root, legacy) = if resolved.via == Via::Site {
        match find_document_in_site(&resolved.root) {
            Some((found, legacy)) => (found, Some(resolved.root.clone()), legacy),
            None => {
                return Err(crate::document::check_document(&resolved.root, resolved.from, resolved.via)
                    .err()
                    .map_or_else(|| CommandError::document("no document to migrate"), |e| e)
                    .into())
            }
        }
    } else {
        (resolved.root.clone(), None, false)
    };
    if !root.is_dir() {
        return Err(crate::document::check_document(&root, resolved.from, Via::Doc)
            .err()
            .map_or_else(|| CommandError::document("no document to migrate"), |e| e)
            .into());
    }
    let manifest_file = root.join("manifest.json");
    let manifest: Option<Value> = fs::read_to_string(&manifest_file).ok().and_then(|t| serde_json::from_str(&t).ok());
    let name = document_name(&root, site_root.as_deref(), manifest.as_ref());
    if !root.join("palette.mjs").is_file() {
        if root.join("palette.json").is_file() {
            return Err(CommandError::new(format!("{name} is already in format 2; there's nothing to migrate"), "already_migrated").into());
        }
        return Err(CommandError::document(format!(
            "{} isn't a Pixel Lab document: it has no palette.mjs or palette.json",
            root.display()
        ))
        .into());
    }
    // Unsaved edits open in Pixel Lab (or in this session) would be lost.
    let mut dirty = unsaved_in_app(&state_dir(&options.env), &root);
    if let Some(s) = ctx.session.as_deref() {
        dirty.extend(s.dirty_refs());
    }
    if !dirty.is_empty() {
        return Err(CommandError::new(
            format!("{name} has unsaved edits open ({}); save or revert them before migrating", dirty.join(", ")),
            "dirty",
        )
        .with_details(json!(dirty))
        .into());
    }
    // Read every source.
    let mut converted: Vec<(String, Value)> = Vec::new();
    let mut refused = Vec::new();
    let mut notes = Vec::new();
    for file in sources_of(&root) {
        let text = fs::read_to_string(root.join(&file)).unwrap_or_default();
        match convert(&file, &text) {
            Ok(value) => {
                let target = json_name(&file);
                for n in value.get("notes").and_then(Value::as_array).into_iter().flatten() {
                    notes.push(Value::String(format!("{target}: {}", str_of(Some(n)))));
                }
                if let Some(Value::Object(cn)) = value.get("colorNotes") {
                    for (color, note) in cn {
                        notes.push(Value::String(format!("{target}: {color}: {}", str_of(Some(note)))));
                    }
                }
                converted.push((target, value));
            }
            Err(e) => refused.push(e),
        }
    }
    if !refused.is_empty() {
        let n = refused.len();
        return Err(CommandError::new(
            format!("{n} source{} can't be read as data; nothing written:\n{}", if n == 1 { "" } else { "s" }, refused.join("\n")),
            "migrate_refused",
        )
        .with_details(json!(refused))
        .into());
    }
    // The format-2 text, and every scene built both ways.
    let texts: Vec<(String, String)> = converted.iter().map(|(f, v)| (f.clone(), serialize_file(f, v))).collect();
    let reread: Vec<(String, Value)> = texts.iter().map(|(f, t)| (f.clone(), parse(t).expect("canonical text parses"))).collect();
    let (before, after) = (sources_from(&converted), sources_from(&reread));
    let budgets = Budgets::new();
    for scene in before.scenes.keys() {
        let a = compile_scene(&before, scene, &budgets, Order::Depth).map(|c| c.output);
        let b = compile_scene(&after, scene, &budgets, Order::Depth).map(|c| c.output);
        if a != b {
            return Err(CommandError::new(
                format!("scenes/{scene}.json wouldn't build the same SVG as scenes/{scene}.mjs; nothing written"),
                "not_lossless",
            )
            .into());
        }
    }
    // The manifest.
    let mut m: Map<String, Value> = match manifest {
        Some(Value::Object(m)) => m,
        _ if legacy => {
            let site = site_root.clone().expect("a legacy document is in a site");
            let (card, grid, dark) = site_colors(&site);
            let budgets: Map<String, Value> =
                site_budgets().iter().map(|(k, b)| (k.clone(), json!({ "raw": b.raw, "gzip": b.gzip }))).collect();
            let mut preview = Map::new();
            if let Some(c) = card {
                preview.insert("card".into(), json!(c));
            }
            if let Some(g) = grid {
                preview.insert("grid".into(), json!(g));
            }
            let theme = dark.map_or_else(|| json!({}), |d| json!({ "darkPage": d }));
            obj(vec![
                ("format", json!(2)),
                ("name", json!(name)),
                ("publish", json!({ "dir": "..", "budgets": budgets })),
                ("preview", Value::Object(preview)),
                ("theme", theme),
            ])
        }
        _ => Map::new(),
    };
    m.insert("format".into(), json!(2));
    let manifest_text = serialize_manifest(&Value::Object(m));
    // Where it goes: the copy, a legacy folder's new bundle, or in place.
    let home = home_dir(&options.env);
    let target: PathBuf = match opt_str(&params, "to") {
        Some(to) => expand_path(to, &options.cwd, &home),
        None if legacy => site_root.clone().expect("a site").join(SITE_ART_DIR).join(format!("{name}{EXTENSION}")),
        None => root.clone(),
    };
    let copying = opt_str(&params, "to").is_some();
    if copying {
        // A copy inside the document would copy itself (as document.copy refuses).
        let (to_text, from_text) = (target.to_string_lossy().into_owned(), root.to_string_lossy().into_owned());
        if to_text != from_text && to_text.starts_with(&format!("{}/", from_text.trim_end_matches('/'))) {
            return Err(CommandError::usage(format!("{to_text} is inside the document itself")).into());
        }
    }
    if (copying || target != root) && target.exists() {
        return Err(CommandError::new(format!("{} already exists; pick another path", target.display()), "exists").into());
    }
    let io = |e: std::io::Error| CommandError::new(format!("writing {} failed: {e}", target.display()), "write_failed");
    let write = |file: &str, text: &str| -> Result<()> {
        write_atomic(&target.join(file), text)
            .map_err(|e| CommandError::new(format!("writing {} failed: {}", target.join(file).display(), e.message()), "write_failed").into())
    };
    if target != root {
        fs::create_dir_all(&target).map_err(io)?;
        if copying {
            copy_others(&root, &target, "").map_err(io)?;
        }
    }
    let mut files = Vec::new();
    for dir in ["objects", "scenes"] {
        fs::create_dir_all(target.join(dir)).map_err(io)?;
    }
    // The sources first and the manifest last: until the manifest says
    // format 2, a document interrupted here still opens as format 1.
    for (file, text) in &texts {
        write(file, text)?;
    }
    write("manifest.json", &manifest_text)?;
    files.push(json!("manifest.json"));
    files.extend(texts.iter().map(|(file, _)| json!(file)));
    // In place (or out of a legacy folder): the .mjs files go.
    let mut removed = Vec::new();
    if !copying {
        for file in sources_of(&root) {
            fs::remove_file(root.join(&file)).map_err(io)?;
            removed.push(json!(if legacy { format!("{}/{file}", crate::document::LEGACY_SOURCE_DIR) } else { file }));
        }
        if legacy {
            for dir in ["objects", "scenes", ""] {
                let _ = fs::remove_dir(root.join(dir));
            }
        }
    }
    Ok(json!({ "ok": true, "root": target.to_string_lossy(), "files": files, "notes": notes, "removed": removed }))
}

fn migrate_format(r: &Value, _: &Params) -> String {
    let count = |k: &str| r[k].as_array().map_or(0, Vec::len);
    let removed = count("removed");
    format!(
        "Migrated to format 2 in {}: {} files written, {} note{} kept{}",
        str_of(r.get("root")),
        count("files"),
        count("notes"),
        if count("notes") == 1 { "" } else { "s" },
        if removed > 0 { format!(", {removed} .mjs file{} removed", if removed == 1 { "" } else { "s" }) } else { String::new() }
    )
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut migrate = Command::new(
        "document.migrate",
        "Migrate a document from format 1 (.mjs sources) to format 2 (JSON sources), in place or as a copy",
        json!({
            "type": "object",
            "properties": {
                "to": { "type": "string", "description": "write the migrated document to this new folder instead of converting it in place" }
            }
        }),
        true,
        migrate_run,
    );
    migrate.description = Some(
        "Reads every .mjs source as data (comments become notes; anything that isn't plain data is refused with its file, line and column), checks that every scene builds the same SVG from the JSON sources, then writes palette.json, objects/*.json, scenes/*.json and a manifest with \"format\": 2. In place it removes the .mjs files (a site's legacy source/ folder becomes src/assets/pixel-art/<name>.pixellab); with --to it writes a copy and leaves the document as it is. Refuses while the document has unsaved edits open in Pixel Lab.",
    );
    migrate.writes_direct = true;
    migrate.needs_document = false;
    migrate.examples = vec!["pixellab document migrate --document ~/art/old.pixellab", "pixellab document migrate --to /tmp/copy.pixellab"];
    migrate.format = Some(migrate_format);
    vec![migrate]
}
