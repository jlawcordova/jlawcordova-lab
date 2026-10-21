//! document.info, document.new, document.copy and palette.get: the open
//! document (docs/intents/2026-10-pixellab-document/spec.md, R3, R4), making
//! a new one, saving a copy of it, and its palette (ports
//! cli/lib/commands/document.mjs).

use std::fs;
use std::path::{Path, PathBuf};

use engine::engine::validate;
use serde_json::{json, Map, Value};

use super::common::{obj, opt_str, pad_end, str_of};
use crate::context::{Ctx, DocKind};
use crate::document::{expand_path, home_dir, process_env, EXTENSION, FORMAT};
use crate::errors::{CommandError, Result};
use crate::registry::{Command, Params};

/// JS `path.resolve(dir, p)` without touching the disk.
fn resolve_in(dir: &Path, p: &str) -> PathBuf {
    crate::open_docs::resolve(&dir.join(p))
}

/// The checked-out branch, read from .git/HEAD (no git process, nothing written) (`gitBranch`).
pub fn git_branch(root: &Path) -> Option<String> {
    let mut dir = root.to_path_buf();
    for _ in 0..8 {
        let mut git_dir = dir.join(".git");
        if git_dir.exists() {
            if git_dir.is_file() {
                let text = fs::read_to_string(&git_dir).ok()?;
                let target = text.lines().find_map(|l| {
                    let rest = l.strip_prefix("gitdir:")?;
                    let rest = rest.trim_start();
                    (!rest.is_empty()).then(|| rest.to_string())
                })?;
                git_dir = resolve_in(&dir, target.trim());
            }
            let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
            let head = head.trim();
            let branch = head
                .strip_prefix("ref:")
                .map(str::trim_start)
                .and_then(|r| r.strip_prefix("refs/heads/"))
                .filter(|b| !b.is_empty() && !b.contains('\n'));
            return Some(match branch {
                Some(b) => b.to_string(),
                None => format!("(detached at {})", head.chars().take(7).collect::<String>()),
            });
        }
        match dir.parent() {
            Some(up) if up != dir => dir = up.to_path_buf(),
            _ => break,
        }
    }
    None
}

fn path_text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn info_run(ctx: &mut Ctx, _: Params) -> Result<Value> {
    let loaded = ctx.sources()?;
    let doc = ctx.document()?;
    let mut problems = loaded.load_errors.len();
    problems += validate(&loaded.sources).len();
    let mut tiers = Map::new();
    for (_, c) in loaded.sources.colors.iter() {
        let n = tiers.get(c.tier).and_then(Value::as_i64).unwrap_or(0);
        tiers.insert(c.tier.to_string(), json!(n + 1));
    }
    Ok(Value::Object(obj(vec![
        ("root", json!(path_text(&doc.root))),
        ("from", json!(doc.from.as_str())),
        ("via", json!(doc.via.as_str())),
        ("kind", json!(doc.kind)),
        ("name", json!(doc.name)),
        ("siteRoot", doc.site_root.as_deref().map_or(Value::Null, |p| json!(path_text(p)))),
        ("publishDir", doc.publish_dir.as_deref().map_or(Value::Null, |p| json!(path_text(p)))),
        ("branch", git_branch(&doc.root).map_or(Value::Null, |b| json!(b))),
        ("scenes", json!(loaded.sources.scenes.len())),
        ("objects", json!(loaded.sources.objects.len())),
        ("colors", Value::Object(tiers)),
        ("problems", json!(problems)),
    ])))
}

fn info_format(r: &Value, _: &Params) -> String {
    let s = |k: &str| str_of(r.get(k));
    let via_site = s("via") == "site";
    let from = match s("from").as_str() {
        "flag" => (if s("via") == "doc" { "--document" } else { "--site" }).to_string(),
        "env" => (if via_site { "PIXELLAB_SITE" } else { "PIXELLAB_DOC" }).to_string(),
        other => other.to_string(),
    };
    let publish = match r.get("publishDir") {
        Some(Value::String(p)) => p.clone(),
        _ => "none (add publish.dir to manifest.json, or pass --dir to art build)".to_string(),
    };
    let colors = r["colors"]
        .as_object()
        .map(|m| engine::serialize::entries(m).iter().map(|(t, n)| format!("{} {t}", str_of(Some(n)))).collect::<Vec<_>>().join(", "))
        .unwrap_or_default();
    let problems = r["problems"].as_f64().unwrap_or(0.0);
    [
        format!(
            "document  {} ({}from {}{from})",
            s("root"),
            if s("kind") == "legacy-site" { "a legacy source folder, " } else { "" },
            if via_site { "the site checkout given by " } else { "" }
        ),
        format!("name      {}", s("name")),
        format!("publish   {publish}"),
        format!(
            "branch    {}",
            match r.get("branch") {
                Some(Value::String(b)) => b.clone(),
                _ => "unknown".into(),
            }
        ),
        format!("scenes    {}", s("scenes")),
        format!("objects   {}", s("objects")),
        format!("colors    {colors}"),
        format!("problems  {}{}", s("problems"), if problems != 0.0 { " (pixellab check)" } else { "" }),
    ]
    .join("\n")
}

/// `expandPath` over the process's environment and working folder.
fn expand(path: &str) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    expand_path(path, &cwd, &home_dir(&process_env()))
}

fn with_extension(mut root: PathBuf) -> PathBuf {
    if !path_text(&root).ends_with(EXTENSION) {
        root = PathBuf::from(format!("{}{EXTENSION}", path_text(&root)));
    }
    root
}

fn new_run(_: &mut Ctx, params: Params) -> Result<Value> {
    let root = with_extension(expand(opt_str(&params, "path").unwrap_or("")));
    if root.exists() {
        return Err(CommandError::new(
            format!("{} already exists; pick another path, or open it with --document", path_text(&root)),
            "exists",
        )
        .into());
    }
    let doc_name = match opt_str(&params, "name") {
        Some(n) => n.to_string(),
        None => {
            let base = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            base.strip_suffix(EXTENSION).unwrap_or(&base).to_string()
        }
    };
    if doc_name.is_empty() {
        return Err(CommandError::usage("the document needs a name").into());
    }
    fs::create_dir_all(root.join("objects"))?;
    fs::create_dir_all(root.join("scenes"))?;
    let manifest = engine::format::serialize_manifest(&json!({ "format": FORMAT, "name": doc_name }));
    fs::write(root.join("manifest.json"), manifest)?;
    fs::write(root.join(engine::format::PALETTE_FILE), engine::starter::STARTER_PALETTE)?;
    Ok(Value::Object(obj(vec![
        ("root", json!(path_text(&root))),
        ("name", json!(doc_name)),
        ("summary", json!(format!("Made {}", path_text(&root)))),
    ])))
}

fn new_format(r: &Value, _: &Params) -> String {
    format!("{}\nNext: pixellab --document {} scene new <name>, or open it in Pixel Lab.", str_of(r.get("summary")), str_of(r.get("root")))
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if entry.file_type()?.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

fn copy_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let dest = with_extension(expand(opt_str(&params, "to").unwrap_or("")));
    let doc = ctx.document()?;
    if dest.exists() {
        return Err(CommandError::new(format!("{} already exists; pick another path", path_text(&dest)), "exists").into());
    }
    let from = doc.root.clone();
    let (dest_text, from_text) = (path_text(&dest), path_text(&from));
    if dest_text == from_text || dest_text.starts_with(&format!("{from_text}/")) {
        return Err(CommandError::usage(format!("{dest_text} is inside the document itself")).into());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    copy_tree(&from, &dest)?;
    let mut wrote_manifest = false;
    if !dest.join("manifest.json").exists() {
        let mut manifest = doc.manifest.clone();
        let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if let Value::Object(m) = &mut manifest {
            m.insert("name".into(), json!(name.strip_suffix(EXTENSION).unwrap_or(&name)));
        }
        fs::write(dest.join("manifest.json"), engine::format::serialize_manifest(&manifest))?;
        wrote_manifest = true;
    }
    Ok(Value::Object(obj(vec![
        ("from", json!(from_text)),
        ("to", json!(dest_text)),
        ("wroteManifest", json!(wrote_manifest)),
        (
            "summary",
            json!(format!("Copied {from_text} to {dest_text}{}", if wrote_manifest { " (with a manifest.json for it)" } else { "" })),
        ),
    ])))
}

fn summary_format(r: &Value, _: &Params) -> String {
    str_of(r.get("summary"))
}

fn palette_run(ctx: &mut Ctx, _: Params) -> Result<Value> {
    let loaded = ctx.sources()?;
    let doc = ctx.doc(DocKind::Palette, "")?;
    let tier = |key: &str| match doc.get(key) {
        Some(Value::Object(m)) => Value::Object(engine::serialize::entries(m).into_iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
        _ => json!({}),
    };
    let colors: Vec<Value> = loaded.sources.colors.iter().map(|(name, c)| json!({ "name": name, "hex": c.hex, "tier": c.tier })).collect();
    Ok(json!({ "world": tier("world"), "outfit": tier("outfit"), "colors": colors }))
}

fn palette_format(r: &Value, _: &Params) -> String {
    r["colors"]
        .as_array()
        .map(|cs| {
            cs.iter()
                .map(|c| format!("{} {}  {}", pad_end(&str_of(c.get("tier")), 7), str_of(c.get("hex")), str_of(c.get("name"))))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut info = Command::new(
        "document.info",
        "Show the document in use: where it came from, its publish folder and what it holds",
        json!({ "type": "object", "properties": {} }),
        false,
        info_run,
    );
    info.description = Some(concat!(
        "root is the .pixellab folder (or a legacy source/ folder in a site checkout, kind legacy-site); from says how it was named (--document, --site, PIXELLAB_DOC, ",
        "PIXELLAB_SITE, the config file or the default); publishDir is where art build writes (null when the manifest names none); ",
        "then the git branch, and how many scenes, objects and colors it has, with the count of validation problems."
    ));
    info.format = Some(info_format);

    let mut make = Command::new(
        "document.new",
        "Make a new, empty .pixellab document with the starter palette",
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "where to make it, e.g. ~/Art/meadow.pixellab" },
                "name": { "type": "string", "description": "the document's name (the folder's, by default)" }
            },
            "required": ["path"]
        }),
        true,
        new_run,
    );
    make.description = Some("Writes manifest.json, palette.json and empty objects/ and scenes/ folders at path (.pixellab is added when missing). Refuses an existing path. The new document is not opened: name it with --document.");
    make.args = vec!["path"];
    make.writes_direct = true;
    make.needs_document = false;
    make.examples = vec!["pixellab document new ~/Art/meadow.pixellab"];
    make.format = Some(new_format);

    let mut copy = Command::new(
        "document.copy",
        "Save a copy of the open document at another path (Save As)",
        json!({
            "type": "object",
            "properties": { "to": { "type": "string", "description": "the new path (.pixellab is added when missing)" } },
            "required": ["to"]
        }),
        true,
        copy_run,
    );
    copy.description = Some(concat!(
        "Copies the whole .pixellab folder (manifest, palette, objects, scenes) to --to. Refuses an existing path. The copy keeps its publish.dir, so a relative one now points next to the copy. ",
        "A legacy source/ folder has no manifest: the copy gets one with the site's budgets, publish folder (..) and colors, so it publishes as the site did."
    ));
    copy.writes_direct = true;
    copy.examples = vec!["pixellab document copy --to ~/Art/meadow-v2.pixellab"];
    copy.format = Some(summary_format);

    let mut palette = Command::new(
        "palette.get",
        "List the palette: every color name, hex and tier",
        json!({ "type": "object", "properties": {} }),
        false,
        palette_run,
    );
    palette.description = Some("World colors are for every object; outfit colors only for objects that extend character.");
    palette.format = Some(palette_format);

    vec![info, make, copy, palette]
}
