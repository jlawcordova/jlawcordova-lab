//! The document (ports cli/lib/document.mjs;
//! docs/intents/2026-10-pixellab-document/spec.md, D2): a `.pixellab` folder
//! holding manifest.json, palette.json, objects/*.json and scenes/*.json in
//! format 2. Where it comes from (--document, PIXELLAB_DOC, the config,
//! --site, PIXELLAB_SITE, the default site), whether it looks like one, what
//! its manifest says, and its sources read fresh. A format-1 document gives
//! R9's problem. Filled in by U13.

/// The format this program reads and writes.
pub const FORMAT: i64 = 2;
pub const EXTENSION: &str = ".pixellab";
/// Where a site keeps its document, and the legacy sources it had before documents.
pub const SITE_ART_DIR: &str = "src/assets/pixel-art";
pub const LEGACY_SOURCE_DIR: &str = "src/assets/pixel-art/source";
/// The night theme's page color when a manifest names none.
pub const DEFAULT_DARK_PAGE: &str = "#1C1712";

use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use engine::compile::{Budget, Budgets};
use engine::format::{stringify, Docs};
use engine::serialize::{entries, keys};
use serde_json::{Map, Value};

use crate::errors::CommandError;

/// The environment, as a map (with HOME), passed in so tests control it.
pub type Env = HashMap<String, String>;

/// The default site, before `~` is expanded.
pub const DEFAULT_SITE: &str = "~/Projects/jlawcordova.github.io";

/// A manifest, checked by [`check_manifest`].
pub type Manifest = Value;

type CResult<T> = std::result::Result<T, CommandError>;

/// The process environment as an [`Env`].
pub fn process_env() -> Env {
    std::env::vars().collect()
}

/// The home folder: `env["HOME"]`, else the process's.
pub fn home_dir(env: &Env) -> PathBuf {
    match env.get("HOME") {
        Some(h) => PathBuf::from(h),
        None => PathBuf::from(std::env::var("HOME").unwrap_or_default()),
    }
}

/// The config file: `{"document": "<path>"}` or `{"site": "<checkout>"}`.
pub fn config_file(home: &Path) -> PathBuf {
    home.join(".config").join("pixellab").join("config.json")
}

/// `path.resolve(cwd, path)`: absolute, with `.` and `..` folded away.
fn resolve_path(cwd: &Path, path: &Path) -> PathBuf {
    let full = if path.is_absolute() { path.to_path_buf() } else { cwd.join(path) };
    let mut out = PathBuf::new();
    for c in full.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Expands a leading `~` and makes the path absolute.
pub fn expand_path(path: &str, cwd: &Path, home: &Path) -> PathBuf {
    if path == "~" || path.starts_with("~/") {
        let rest = &path[1..];
        resolve_path(cwd, &resolve_path(Path::new("/"), &PathBuf::from(format!("{}/{}", home.display(), rest))))
    } else {
        resolve_path(cwd, Path::new(path))
    }
}

fn read_config(home: &Path) -> CResult<Map<String, Value>> {
    let file = config_file(home);
    if !file.exists() {
        return Ok(Map::new());
    }
    let text = fs::read_to_string(&file).unwrap_or_default();
    let config: Value = serde_json::from_str(&text).map_err(|err| {
        CommandError::document(format!(
            r#"{} is not valid JSON ({err}); it should be {{"document": "<path to a .pixellab>"}}"#,
            file.display()
        ))
    })?;
    let Value::Object(config) = config else { return Ok(Map::new()) };
    for key in ["document", "site"] {
        if let Some(v) = config.get(key) {
            if !matches!(v, Value::String(s) if !s.is_empty()) {
                return Err(CommandError::document(format!(r#"{}: "{key}" must be a path"#, file.display())));
            }
        }
    }
    Ok(config)
}

/// Where the document's path came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum From {
    Flag,
    Env,
    Config,
    Default,
}

impl From {
    pub fn as_str(self) -> &'static str {
        match self {
            From::Flag => "flag",
            From::Env => "env",
            From::Config => "config",
            From::Default => "default",
        }
    }
}

/// Whether the path names a document or a site checkout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    Doc,
    Site,
}

impl Via {
    pub fn as_str(self) -> &'static str {
        match self {
            Via::Doc => "doc",
            Via::Site => "site",
        }
    }
}

/// `{ root, from, via }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub root: PathBuf,
    pub from: From,
    pub via: Via,
}

fn from_text(from: From, via: Via) -> &'static str {
    match (from, via) {
        (From::Flag, Via::Doc) => "--document",
        (From::Flag, Via::Site) => "--site",
        (From::Env, Via::Doc) => "PIXELLAB_DOC",
        (From::Env, Via::Site) => "PIXELLAB_SITE",
        (From::Config, Via::Doc) => r#"~/.config/pixellab/config.json ("document")"#,
        (From::Config, Via::Site) => r#"~/.config/pixellab/config.json ("site")"#,
        (From::Default, Via::Doc) => "the default",
        (From::Default, Via::Site) => "the default site",
    }
}

const HOW: &str = r#"Pass --document <path.pixellab> (or --site <checkout>), set PIXELLAB_DOC, or put {"document": "<path>"} in ~/.config/pixellab/config.json."#;

/// Where the document comes from: --document, else PIXELLAB_DOC, else the
/// config's "document", else --site, PIXELLAB_SITE, the config's "site",
/// else the default site. Doesn't check the folder.
pub fn resolve_document_root(doc: Option<&str>, site: Option<&str>, env: &Env, cwd: &Path) -> CResult<Resolved> {
    let home = home_dir(env);
    let set = |k: &str| env.get(k).filter(|v| !v.is_empty());
    let r = |path: &str, from, via| Resolved { root: expand_path(path, cwd, &home), from, via };
    if let Some(d) = doc.filter(|d| !d.is_empty()) {
        return Ok(r(d, From::Flag, Via::Doc));
    }
    if let Some(d) = set("PIXELLAB_DOC") {
        return Ok(r(d, From::Env, Via::Doc));
    }
    let config = read_config(&home)?;
    let cfg = |k: &str| config.get(k).and_then(Value::as_str).filter(|v| !v.is_empty());
    if let Some(d) = cfg("document") {
        return Ok(r(d, From::Config, Via::Doc));
    }
    if let Some(s) = site.filter(|s| !s.is_empty()) {
        return Ok(r(s, From::Flag, Via::Site));
    }
    if let Some(s) = set("PIXELLAB_SITE") {
        return Ok(r(s, From::Env, Via::Site));
    }
    if let Some(s) = cfg("site") {
        return Ok(r(s, From::Config, Via::Site));
    }
    Ok(r(DEFAULT_SITE, From::Default, Via::Site))
}

fn is_dir(path: &Path) -> bool {
    path.is_dir()
}

/// The document inside a site checkout: the first `*.pixellab` folder under
/// src/assets/pixel-art/ (by name), else the legacy source/ folder
/// (`true` in the second field).
pub fn find_document_in_site(site_root: &Path) -> Option<(PathBuf, bool)> {
    let art = site_root.join(SITE_ART_DIR);
    if is_dir(&art) {
        let mut bundles: Vec<String> = fs::read_dir(&art)
            .map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| e.file_name().into_string().ok()).collect())
            .unwrap_or_default();
        bundles.retain(|f| f.ends_with(EXTENSION) && is_dir(&art.join(f)));
        bundles.sort();
        if let Some(first) = bundles.first() {
            return Some((art.join(first), false));
        }
    }
    let legacy = site_root.join(LEGACY_SOURCE_DIR);
    is_dir(&legacy).then_some((legacy, true))
}

/// The budgets the site's compiler gave two outputs; a legacy source folder keeps them.
pub fn site_budgets() -> Budgets {
    let mut b = Budgets::new();
    b.insert("hero-island.svg".into(), Budget { raw: 750 * 1024, gzip: 150 * 1024 });
    b.insert("range-sprite.svg".into(), Budget { raw: 200 * 1024, gzip: 50 * 1024 });
    b
}

const KNOWN_TOP: [&str; 5] = ["format", "name", "publish", "preview", "theme"];
const KNOWN_PUBLISH: [&str; 3] = ["dir", "budgets", "images"];
const KNOWN_PREVIEW: [&str; 2] = ["card", "grid"];
const KNOWN_THEME: [&str; 1] = ["darkPage"];

fn is_hex6(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// JS truthiness of a JSON value.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// `Object.keys(v)` for an object or array, in JS order.
fn object_keys(v: &Value) -> Vec<String> {
    match v {
        Value::Object(m) => keys(m).into_iter().cloned().collect(),
        Value::Array(a) => (0..a.len()).map(|i| i.to_string()).collect(),
        _ => vec![],
    }
}

/// `Number.isInteger(v) && v > 0`.
fn positive_integer(v: Option<&Value>) -> bool {
    v.and_then(Value::as_f64).is_some_and(|f| f.fract() == 0.0 && f > 0.0)
}

/// Checks a manifest's shape; the first problem is a DocumentError.
pub fn check_manifest(m: &Value, file: &str) -> CResult<Manifest> {
    let bad = |what: &str| CommandError::document(format!("{file}: {what}"));
    let Value::Object(top) = m else { return Err(bad("must be a JSON object")) };
    match top.get("format").and_then(Value::as_f64) {
        Some(f) if f == FORMAT as f64 => {}
        _ => {
            let got = match top.get("format") {
                None => " (missing)".to_string(),
                Some(v) => format!(" (got {})", stringify(v)),
            };
            return Err(bad(&format!("\"format\" must be {FORMAT}{got}")));
        }
    }
    let unknown = |obj: &Value, known: &[&str], at: &str| -> CResult<()> {
        match object_keys(obj).into_iter().find(|k| !known.contains(&k.as_str())) {
            Some(k) => Err(bad(&format!("unknown key {at}{}", stringify(&Value::String(k))))),
            None => Ok(()),
        }
    };
    unknown(m, &KNOWN_TOP, "")?;
    if let Some(name) = top.get("name") {
        if !matches!(name, Value::String(s) if !s.is_empty()) {
            return Err(bad("\"name\" must be a non-empty string"));
        }
    }
    let path_string = |v: &Value| matches!(v, Value::String(s) if !s.is_empty());
    if let Some(publish) = top.get("publish") {
        if !truthy(publish) || !matches!(publish, Value::Object(_) | Value::Array(_)) {
            return Err(bad("\"publish\" must be an object"));
        }
        unknown(publish, &KNOWN_PUBLISH, "publish.")?;
        if let Some(dir) = publish.get("dir") {
            if !path_string(dir) {
                return Err(bad("\"publish.dir\" must be a path"));
            }
        }
        if let Some(images) = publish.get("images") {
            if !path_string(images) {
                return Err(bad("\"publish.images\" must be a path"));
            }
        }
        if let Some(budgets) = publish.get("budgets") {
            let Some(map) = budgets.as_object() else {
                return Err(bad(r#""publish.budgets" must be an object of { "<output>.svg": { raw, gzip } }"#));
            };
            for (out, b) in entries(map) {
                let ok = b.as_object().is_some_and(|o| {
                    positive_integer(o.get("raw")) && positive_integer(o.get("gzip")) && o.keys().all(|k| k == "raw" || k == "gzip")
                });
                if !ok {
                    return Err(bad(&format!("\"publish.budgets.{out}\" must be {{ raw: <bytes>, gzip: <bytes> }}")));
                }
            }
        }
    }
    if let Some(preview) = top.get("preview") {
        if !truthy(preview) || !matches!(preview, Value::Object(_) | Value::Array(_)) {
            return Err(bad("\"preview\" must be an object"));
        }
        unknown(preview, &KNOWN_PREVIEW, "preview.")?;
        let card = preview.get("card").and_then(Value::as_str);
        let grid = preview.get("grid").and_then(Value::as_str);
        if let Err(message) = engine::preview::preview_colors(card, grid) {
            return Err(bad(&message));
        }
    }
    if let Some(theme) = top.get("theme") {
        if !truthy(theme) || !matches!(theme, Value::Object(_) | Value::Array(_)) {
            return Err(bad("\"theme\" must be an object"));
        }
        unknown(theme, &KNOWN_THEME, "theme.")?;
        if let Some(dark) = theme.get("darkPage") {
            if !matches!(dark, Value::String(s) if is_hex6(s)) {
                return Err(bad("\"theme.darkPage\" must be a #RRGGBB color"));
            }
        }
    }
    Ok(m.clone())
}

/// The colors a legacy site's variables.css gives previews and the night
/// theme, when it has them: `(card, grid, dark_page)`.
pub fn site_colors(site_root: &Path) -> (Option<String>, Option<String>, Option<String>) {
    let Ok(css) = fs::read_to_string(site_root.join("src/styles/variables.css")) else { return (None, None, None) };
    // `--name:\s*` then the value up to the terminator.
    let after = |text: &str, name: &str| -> Option<String> {
        let at = text.find(name)?;
        Some(text[at + name.len()..].trim_start().to_string())
    };
    let card = after(&css, "--color-card:").and_then(|r| {
        let hex = r.get(..7)?;
        (is_hex6(hex) && r[7..].starts_with(';')).then(|| hex.to_string())
    });
    let grid = after(&css, "--color-grid:").and_then(|r| {
        let body = r.strip_prefix("rgba(")?;
        let end = body.find(')')?;
        body[end + 1..].starts_with(';').then(|| format!("rgba({})", &body[..end]))
    });
    let dark = css.find("[data-theme=\"dark\"]").and_then(|at| {
        let r = after(&css[at..], "--color-page:")?;
        let hex = r.get(..7)?;
        let boundary = r[7..].chars().next().is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
        (is_hex6(hex) && boundary).then(|| hex.to_uppercase())
    });
    (card, grid, dark)
}

/// A checked document root: `(root, site_root, legacy, manifest)`.
#[derive(Clone, Debug)]
pub struct Checked {
    pub root: PathBuf,
    pub site_root: Option<PathBuf>,
    pub legacy: bool,
    pub manifest: Manifest,
}

fn base_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The name of a document: the manifest's, else the folder's without
/// `.pixellab`, else (for a bare folder) its site's folder name.
pub fn document_name(root: &Path, site_root: Option<&Path>, manifest: Option<&Value>) -> String {
    if let Some(n) = manifest.and_then(|m| m.get("name")).and_then(Value::as_str).filter(|n| !n.is_empty()) {
        return n.to_string();
    }
    let b = base_name(root);
    if let Some(stem) = b.strip_suffix(EXTENSION) {
        return stem.to_string();
    }
    match site_root {
        Some(s) => base_name(s),
        None => base_name(root.parent().unwrap_or(Path::new(""))),
    }
}

fn old_format(name: &str) -> CommandError {
    CommandError::document(format!("{name} uses the old format. Migrate it with `pixellab document migrate`, or in the app."))
}

/// Checks that `root` is a document (or, via a site, finds the one in it)
/// and reads its manifest.
pub fn check_document(root: &Path, from: From, via: Via) -> CResult<Checked> {
    let where_ = format!("(from {})", from_text(from, via));
    if !is_dir(root) {
        let expected = if via == Via::Site { "a checkout of the site" } else { "a .pixellab document" };
        let what = if via == Via::Site { "site" } else { "document" };
        return Err(CommandError::document(format!(
            "{what} not found: {} {where_} is not a folder. Expected {expected}. {HOW}",
            root.display()
        )));
    }
    let mut doc_root = root.to_path_buf();
    let mut site_root = None;
    let mut legacy = false;
    if via == Via::Site {
        let Some((found, is_legacy)) = find_document_in_site(root) else {
            return Err(CommandError::document(format!(
                "{} {where_} doesn't look like the site checkout: no *.pixellab or source/ under {SITE_ART_DIR}. {HOW}",
                root.display()
            )));
        };
        site_root = Some(root.to_path_buf());
        doc_root = found;
        legacy = is_legacy;
    }
    if legacy {
        return Err(old_format(&document_name(&doc_root, site_root.as_deref(), None)));
    }
    let manifest_file = doc_root.join("manifest.json");
    // The manifest, parsed leniently first: R9 only needs its format and name.
    let parsed: Option<Value> = if manifest_file.exists() {
        match fs::read_to_string(&manifest_file)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
        {
            Ok(v) => Some(v),
            Err(err) => {
                if doc_root.join("palette.json").exists() {
                    return Err(CommandError::document(format!("{} is not valid JSON ({err})", manifest_file.display())));
                }
                None
            }
        }
    } else {
        None
    };
    let is_old = parsed.as_ref().and_then(|m| m.get("format")).and_then(Value::as_f64) == Some(1.0);
    if is_old || (!doc_root.join("palette.json").exists() && doc_root.join("palette.mjs").exists()) {
        return Err(old_format(&document_name(&doc_root, site_root.as_deref(), parsed.as_ref())));
    }
    if !doc_root.join("palette.json").exists() {
        return Err(CommandError::document(format!(
            "{} {where_} isn't a Pixel Lab document: it has no palette.json. {HOW}",
            doc_root.display()
        )));
    }
    let manifest = match parsed {
        Some(p) => check_manifest(&p, &manifest_file.to_string_lossy())?,
        None => serde_json::json!({ "format": FORMAT }),
    };
    Ok(Checked { root: doc_root, site_root, legacy, manifest })
}

/// How to open a document.
#[derive(Clone, Debug)]
pub struct OpenOptions {
    pub doc: Option<String>,
    pub site: Option<String>,
    pub env: Env,
    pub cwd: PathBuf,
}

impl Default for OpenOptions {
    fn default() -> Self {
        OpenOptions { doc: None, site: None, env: process_env(), cwd: std::env::current_dir().unwrap_or_default() }
    }
}

/// An open document: its manifest now, its sources on each `load_docs`.
#[derive(Clone, Debug)]
pub struct Document {
    pub root: PathBuf,
    pub from: From,
    pub via: Via,
    /// "document" or "legacy-site".
    pub kind: &'static str,
    pub site_root: Option<PathBuf>,
    pub name: String,
    pub source_dir: PathBuf,
    pub manifest: Manifest,
    pub publish_dir: Option<PathBuf>,
    pub images_dir: Option<PathBuf>,
    pub budgets: Budgets,
    pub preview_card: Option<String>,
    pub preview_grid: Option<String>,
    pub dark_page: String,
}

impl Document {
    /// The sources, read fresh.
    pub fn load_docs(&self) -> Docs {
        load_docs(&self.root)
    }
}

/// Reads palette.json, objects/*.json and scenes/*.json, freshly.
pub fn load_docs(dir: &Path) -> Docs {
    engine::format::read_docs(dir)
}

/// Finds, checks and opens the document.
pub fn open_document(options: &OpenOptions) -> CResult<Document> {
    let Resolved { root, from, via } = resolve_document_root(options.doc.as_deref(), options.site.as_deref(), &options.env, &options.cwd)?;
    let checked = check_document(&root, from, via)?;
    Ok(document_at(checked, from, via))
}

fn document_at(Checked { root, site_root, legacy, manifest }: Checked, from: From, via: Via) -> Document {
    let resolve = |key: &str| -> Option<PathBuf> {
        let dir = manifest.get("publish")?.get(key)?.as_str()?;
        Some(if Path::new(dir).is_absolute() { PathBuf::from(dir) } else { resolve_path(&root, Path::new(dir)) })
    };
    let publish_dir = resolve("dir");
    let images_dir = resolve("images");
    let name = document_name(&root, site_root.as_deref(), Some(&manifest));
    let mut budgets = Budgets::new();
    if let Some(map) = manifest.get("publish").and_then(|p| p.get("budgets")).and_then(Value::as_object) {
        for (out, b) in entries(map) {
            let n = |k: &str| b.get(k).and_then(Value::as_f64).unwrap_or(0.0) as u64;
            budgets.insert(out.clone(), Budget { raw: n("raw"), gzip: n("gzip") });
        }
    }
    let text = |a: &str, b: &str| manifest.get(a).and_then(|o| o.get(b)).and_then(Value::as_str).map(str::to_string);
    Document {
        from,
        via,
        kind: if legacy { "legacy-site" } else { "document" },
        site_root,
        name,
        source_dir: root.clone(),
        publish_dir,
        images_dir,
        budgets,
        preview_card: text("preview", "card"),
        preview_grid: text("preview", "grid"),
        dark_page: text("theme", "darkPage").unwrap_or_else(|| DEFAULT_DARK_PAGE.to_string()),
        manifest,
        root,
    }
}
