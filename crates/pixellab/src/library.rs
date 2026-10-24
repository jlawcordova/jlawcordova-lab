//! Categories, object libraries (`.pixellib`) and palette files (`.pixelpal`)
//! (docs/intents/2026-10-object-libraries/spec.md, D1–D6).
//!
//! This file holds what the commands share: the category helpers, the two
//! file kinds (read, validated, written canonically) and the import and
//! palette plans, as pure functions over data. The commands in
//! `commands/library.rs` and `commands/palette.rs` do the I/O and the edits.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{json, Value};

use engine::engine::{category_path_problem, load_sources, validate, Sources};

use crate::errors::CommandError;

// ------------------------------------------------------------ categories

/// The separators a typed category path may use (spec R4): `›` or `/`.
const SEPARATORS: [char; 2] = ['›', '/'];

/// Splits typed text into category names: "Nature / Trees" and
/// "Nature › Trees" are `["Nature", "Trees"]`. Spaces around a separator
/// don't count (a name has none at its ends, spec R2).
pub fn split_category(text: &str) -> Vec<String> {
    text.split(SEPARATORS).map(|n| n.trim().to_string()).collect()
}

/// A category from a param: a list of names or of typed paths, joined in
/// order. `["Nature", "Trees"]` and `["Nature/Trees"]` are the same path.
/// Refuses what spec R2 refuses, saying why.
pub fn category_from_param(value: &Value) -> Result<Vec<String>, String> {
    let parts: Vec<&str> = match value {
        Value::String(s) => vec![s.as_str()],
        Value::Array(list) => {
            let mut parts = Vec::new();
            for v in list {
                parts.push(v.as_str().ok_or_else(|| format!("a category name must be text, got {v}"))?);
            }
            parts
        }
        other => return Err(format!("a category is a list of names, got {other}")),
    };
    let names: Vec<String> = parts.into_iter().flat_map(split_category).collect();
    match category_path_problem(&names) {
        Some(why) => Err(why),
        None => Ok(names),
    }
}

/// "Nature › Trees".
pub fn category_text(names: &[String]) -> String {
    names.join(" › ")
}

/// An object source's category, if it has a well-formed one.
pub fn category_of(doc: &Value) -> Option<Vec<String>> {
    let list = doc.get("category")?.as_array()?;
    let names: Vec<String> = list.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
    (!names.is_empty() && names.len() == list.len()).then_some(names)
}

/// Whether `category` is `prefix` or below it, without regard to case (spec R9).
pub fn category_within(category: &[String], prefix: &[String]) -> bool {
    category.len() >= prefix.len() && category.iter().zip(prefix).all(|(a, b)| a.to_lowercase() == b.to_lowercase())
}

// ------------------------------------------------------------ the packages

/// The folder extension of an object library.
pub const LIBRARY_EXT: &str = ".pixellib";
/// The extension of a palette file.
pub const PALETTE_EXT: &str = ".pixelpal";
/// Both file kinds are format 1.
pub const FORMAT: u64 = 1;

/// The library's manifest.
pub const LIBRARY_FILE: &str = "library.json";

/// `name`, if it ends in `ext`, without it: "coast.pixellib" is "coast".
pub fn stem(path: &Path, ext: &str) -> String {
    let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    file.strip_suffix(ext).unwrap_or(&file).to_string()
}

/// A tier's colors in order: name, hex.
pub type Tier = Vec<(String, String)>;

/// A library package, read (spec D3): the objects as a document's files, and
/// the record of the colors they used when it was exported.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub name: String,
    pub exported_from: String,
    pub world: Tier,
    pub outfit: Tier,
    pub objects: BTreeMap<String, Value>,
    /// Object files that didn't load, as `check` words them.
    pub load_errors: Vec<String>,
}

impl Library {
    /// A recorded color: its tier and hex.
    pub fn color(&self, name: &str) -> Option<(&'static str, &str)> {
        fn find<'a>(tier: &'a [(String, String)], name: &str) -> Option<&'a str> {
            tier.iter().find(|(n, _)| n == name).map(|(_, h)| h.as_str())
        }
        find(&self.world, name).map(|h| ("world", h)).or_else(|| find(&self.outfit, name).map(|h| ("outfit", h)))
    }

    /// The recorded colors as a palette, so the engine can check the objects against them.
    fn palette(&self) -> Value {
        let tier = |t: &Tier| Value::Object(t.iter().map(|(n, h)| (n.clone(), json!(h))).collect());
        json!({ "world": tier(&self.world), "outfit": tier(&self.outfit) })
    }

    /// `library.json`'s data.
    pub fn manifest(&self) -> Value {
        let tier = |t: &Tier| Value::Object(t.iter().map(|(n, h)| (n.clone(), json!(h))).collect());
        json!({
            "format": FORMAT,
            "kind": "library",
            "name": self.name,
            "exportedFrom": self.exported_from,
            "colors": { "world": tier(&self.world), "outfit": tier(&self.outfit) },
        })
    }

    /// Every file of the package, canonical: `library.json` first, then the
    /// objects in name order. The same library always gives the same bytes.
    pub fn files(&self) -> Vec<(String, String)> {
        let mut files = vec![(LIBRARY_FILE.to_string(), engine::format::serialize_manifest(&self.manifest()))];
        files.extend(self.objects.iter().map(|(n, o)| (engine::format::object_file(n), engine::format::serialize_object(o))));
        files
    }
}

fn hex_ok(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str).filter(|h| engine::engine::is_hex(h)).map(str::to_string)
}

fn tier_of(colors: Option<&Value>, tier: &str, problems: &mut Vec<String>) -> Tier {
    let Some(Value::Object(map)) = colors.and_then(|c| c.get(tier)) else {
        problems.push(format!("{LIBRARY_FILE}: colors.{tier} must be an object of name: '#RRGGBB'"));
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, hex) in map {
        match hex_ok(Some(hex)) {
            Some(h) => out.push((name.clone(), h)),
            None => problems.push(format!("{LIBRARY_FILE}: colors.{tier}.{name} must be uppercase #RRGGBB")),
        }
    }
    out
}

/// Reads a `.pixellib` folder. A path that isn't one, or a manifest of
/// another format or kind, is a `library` error; objects that don't load are
/// kept as `load_errors` for [`library_problems`].
pub fn read_library(path: &Path) -> Result<Library, CommandError> {
    let shown = path.display();
    if !path.exists() {
        return Err(CommandError::not_found(format!("no library at {shown}")));
    }
    let not_one = |why: &str| CommandError::new(format!("{shown} isn't a Pixel Lab object library: {why}"), "library");
    let manifest_path = path.join(LIBRARY_FILE);
    if !path.is_dir() || !manifest_path.is_file() {
        return Err(not_one("it has no library.json (a .pixellib is a folder)"));
    }
    let text = std::fs::read_to_string(&manifest_path).map_err(|e| not_one(&format!("library.json can't be read: {e}")))?;
    let manifest = engine::format::parse(&text).map_err(|e| not_one(&format!("library.json isn't JSON: {e}")))?;
    match manifest.get("format").and_then(Value::as_u64) {
        Some(FORMAT) => {}
        Some(n) => return Err(not_one(&format!("it is format {n}; this Pixel Lab reads format {FORMAT}"))),
        None => return Err(not_one("library.json has no format")),
    }
    if manifest.get("kind").and_then(Value::as_str) != Some("library") {
        return Err(not_one("library.json's kind is not \"library\""));
    }
    let mut problems = Vec::new();
    let text_of = |key: &str, problems: &mut Vec<String>| match manifest.get(key).and_then(Value::as_str) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            problems.push(format!("{LIBRARY_FILE}: {key} must be a name"));
            String::new()
        }
    };
    let name = text_of("name", &mut problems);
    let exported_from = text_of("exportedFrom", &mut problems);
    let colors = manifest.get("colors");
    let world = tier_of(colors, "world", &mut problems);
    let outfit = tier_of(colors, "outfit", &mut problems);
    if !problems.is_empty() {
        return Err(not_one(&problems.join("; ")));
    }
    // read_docs wants a palette.json, which a library doesn't have.
    let docs = engine::format::read_docs(path);
    let load_errors = docs.load_errors.into_iter().filter(|e| !e.starts_with("palette.json:")).collect();
    Ok(Library { name, exported_from, world, outfit, objects: docs.objects, load_errors })
}

/// What's wrong with a library, as `check --library` reports it: files that
/// didn't load, then the engine's problems with the objects checked against
/// the library's own recorded colors (D3). Empty means it can be imported.
pub fn library_problems(lib: &Library) -> Vec<String> {
    let mut problems: Vec<String> = lib.load_errors.clone();
    let sources = load_sources(lib.palette(), lib.objects.clone(), BTreeMap::new());
    for p in validate(&sources) {
        problems.push(match p.strip_prefix("palette.json: ") {
            Some(rest) => format!("{LIBRARY_FILE}: colors: {rest}"),
            None => p,
        });
    }
    problems
}

/// Writes a files list into a new folder beside `to`, then renames it into
/// place, so a failed write leaves nothing half made. An existing `to` is
/// replaced only with `replace`, and put back if the swap fails.
fn write_folder(to: &Path, files: &[(String, String)], replace: bool, what: &str) -> Result<(), CommandError> {
    let io = |e: std::io::Error| CommandError::new(format!("can't write {}: {e}", to.display()), "io");
    if to.exists() && !replace {
        return Err(CommandError::new(format!("{} already exists; pick another path, or replace it", to.display()), "exists"));
    }
    let parent = to.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(io)?;
    let tag = crate::state::random_hex(4);
    let temp = parent.join(format!(".{what}-{}-{tag}.tmp", std::process::id()));
    let made = (|| -> std::io::Result<()> {
        for (file, text) in files {
            let path = temp.join(file);
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(path, text)?;
        }
        Ok(())
    })();
    if let Err(e) = made {
        let _ = std::fs::remove_dir_all(&temp);
        return Err(io(e));
    }
    let backup = parent.join(format!(".{what}-{}-{tag}.old", std::process::id()));
    let had = to.exists();
    if had {
        if let Err(e) = std::fs::rename(to, &backup) {
            let _ = std::fs::remove_dir_all(&temp);
            return Err(io(e));
        }
    }
    if let Err(e) = std::fs::rename(&temp, to) {
        if had {
            let _ = std::fs::rename(&backup, to);
        }
        let _ = std::fs::remove_dir_all(&temp);
        return Err(io(e));
    }
    if had {
        let _ = std::fs::remove_dir_all(&backup);
    }
    Ok(())
}

/// Writes a new `.pixellib` at `to` (`replace`: over an existing one).
pub fn write_library(to: &Path, lib: &Library, replace: bool) -> Result<(), CommandError> {
    write_folder(to, &lib.files(), replace, "pixellib")
}

/// Writes one object of a library in place, canonically (`library.category`).
pub fn write_library_object(path: &Path, name: &str, object: &Value) -> Result<(), CommandError> {
    let file = path.join(engine::format::object_file(name));
    crate::state::write_atomic(&file, &engine::format::serialize_object(object))
        .map_err(|e| CommandError::new(format!("can't write {}: {}", file.display(), e.message()), "io"))
}

// ------------------------------------------------------------ export

/// Which of a document's objects to export (spec R11).
#[derive(Clone, Debug)]
pub enum Selection {
    All,
    Names(Vec<String>),
    /// A category and everything below it.
    Category(Vec<String>),
}

/// An exported object: `base` is true for one added because a chosen object extends it.
#[derive(Clone, Debug, PartialEq)]
pub struct Exported {
    pub name: String,
    pub base: bool,
}

/// The color names an object's source uses: its keys, a block's faces, its
/// surface's keys and its sides' keys, once each, in the order found.
pub fn color_refs(doc: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |v: Option<&Value>| {
        if let Some(Value::Object(map)) = v {
            for c in map.values().filter_map(Value::as_str) {
                if !out.iter().any(|o| o == c) {
                    out.push(c.to_string());
                }
            }
        }
    };
    add(doc.get("keys"));
    add(doc.get("faces"));
    add(doc.get("surface").and_then(|s| s.get("keys")));
    add(doc.get("sides").and_then(|s| s.get("keys")));
    out
}

/// Rewrites every color name of an object's source through `convert`
/// (spec R15): its keys, a block's faces, its surface's and sides' keys.
fn convert_colors(doc: &mut Value, convert: &BTreeMap<String, String>) {
    fn map(v: Option<&mut Value>, convert: &BTreeMap<String, String>) {
        if let Some(Value::Object(m)) = v {
            for c in m.values_mut() {
                if let Some(to) = c.as_str().and_then(|s| convert.get(s)) {
                    *c = json!(to);
                }
            }
        }
    }
    map(doc.get_mut("keys"), convert);
    map(doc.get_mut("faces"), convert);
    map(doc.get_mut("surface").and_then(|s| s.get_mut("keys")), convert);
    map(doc.get_mut("sides").and_then(|s| s.get_mut("keys")), convert);
}

/// Builds the library a document exports (spec R11, D4): the chosen objects
/// and every object they extend, with the colors they use recorded at the
/// document's values. `name` is the package's name, `from` the document's.
/// Refuses (as `invalid`) a library that wouldn't pass `check --library`.
pub fn build_library(sources: &Sources, name: &str, from: &str, selection: &Selection) -> Result<(Library, Vec<Exported>), CommandError> {
    let mut chosen: Vec<String> = match selection {
        Selection::All => sources.objects.keys().cloned().collect(),
        Selection::Names(names) => {
            for n in names {
                if !sources.objects.contains_key(n) {
                    return Err(CommandError::not_found(format!("no object named {n}")));
                }
            }
            names.clone()
        }
        Selection::Category(prefix) => sources
            .objects
            .iter()
            .filter(|(_, o)| category_of(o).is_some_and(|c| category_within(&c, prefix)))
            .map(|(n, _)| n.clone())
            .collect(),
    };
    chosen.sort();
    chosen.dedup();
    if chosen.is_empty() {
        let what = match selection {
            Selection::Category(c) => format!("no objects in the category {}", category_text(c)),
            _ => "the document has no objects to export".to_string(),
        };
        return Err(CommandError::new(what, "empty"));
    }
    let mut exported: Vec<Exported> = chosen.iter().map(|n| Exported { name: n.clone(), base: false }).collect();
    let mut at = 0;
    while at < exported.len() {
        let base = sources.objects[&exported[at].name].get("extends").and_then(Value::as_str).map(str::to_string);
        if let Some(base) = base.filter(|b| sources.objects.contains_key(b) && !exported.iter().any(|e| &e.name == b)) {
            exported.push(Exported { name: base, base: true });
        }
        at += 1;
    }
    exported.sort_by(|a, b| a.name.cmp(&b.name));
    let objects: BTreeMap<String, Value> = exported.iter().map(|e| (e.name.clone(), sources.objects[&e.name].clone())).collect();
    let used: Vec<String> = objects.values().flat_map(color_refs).collect();
    let mut lib = Library { name: name.to_string(), exported_from: from.to_string(), objects, ..Library::default() };
    for (color, entry) in sources.colors.iter() {
        if used.iter().any(|u| u == color) {
            let tier = if entry.tier == "world" { &mut lib.world } else { &mut lib.outfit };
            tier.push((color.clone(), entry.hex.clone()));
        }
    }
    let problems = library_problems(&lib);
    if !problems.is_empty() {
        return Err(CommandError::new(
            format!("these objects don't pass check, so they can't be exported:\n{}", problems.join("\n")),
            "invalid",
        )
        .with_details(Value::from(problems)));
    }
    Ok((lib, exported))
}

// ------------------------------------------------------------ import

/// How an imported object is handled (spec R13, D5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fate {
    /// Copied under its own name.
    New,
    /// Copied under a new name, because the document has another object by its name.
    Renamed,
    /// Not copied: the document has the same object (apart from its category).
    Same,
    /// Not copied: the library's `character` differs from the document's, whose figures use it instead.
    Swapped,
}

impl Fate {
    pub fn as_str(self) -> &'static str {
        match self {
            Fate::New => "new",
            Fate::Renamed => "renamed",
            Fate::Same => "same",
            Fate::Swapped => "character",
        }
    }
    fn copied(self) -> bool {
        matches!(self, Fate::New | Fate::Renamed)
    }
}

/// One object of a library, in the plan.
#[derive(Clone, Debug)]
pub struct PlannedObject {
    pub name: String,
    /// The name it has in the document (its own when not copied: the document's object).
    pub as_name: String,
    pub fate: Fate,
}

/// A color name the library and the document give different values (spec R14).
#[derive(Clone, Debug, PartialEq)]
pub struct ColorDifference {
    pub name: String,
    pub tier: &'static str,
    pub library: String,
    pub document: String,
}

/// A color the document doesn't have (spec R15), and the document's nearest of its tier.
#[derive(Clone, Debug, PartialEq)]
pub struct MissingColor {
    pub name: String,
    pub tier: &'static str,
    pub hex: String,
    pub nearest: Option<(String, String)>,
    /// The library's objects (copied ones) that use it.
    pub used_by: Vec<String>,
}

/// What importing a library into a document would do (D5), computed without changing anything.
#[derive(Clone, Debug)]
pub struct ImportPlan {
    pub objects: Vec<PlannedObject>,
    pub differing: Vec<ColorDifference>,
    /// The library's `character` differs from the document's: the figures that will use the document's.
    pub character_swapped: bool,
    pub figures: Vec<String>,
    pub missing: Vec<MissingColor>,
}

impl ImportPlan {
    pub fn renames(&self) -> Vec<(&str, &str)> {
        self.objects.iter().filter(|o| o.fate == Fate::Renamed).map(|o| (o.name.as_str(), o.as_name.as_str())).collect()
    }
    pub fn copied(&self) -> Vec<&PlannedObject> {
        self.objects.iter().filter(|o| o.fate.copied()).collect()
    }
    /// Library name → name in the document, for every object.
    pub fn names(&self) -> BTreeMap<&str, &str> {
        self.objects.iter().map(|o| (o.name.as_str(), o.as_name.as_str())).collect()
    }
    /// Whether the import needs `acceptDifferences`: colors that differ, or a swapped `character`.
    pub fn differs(&self) -> bool {
        !self.differing.is_empty() || self.character_swapped
    }
}

/// An object without its category, for comparing two objects apart from it (spec R13).
fn without_category(doc: &Value) -> Value {
    let mut doc = doc.clone();
    if let Value::Object(m) = &mut doc {
        m.shift_remove("category");
    }
    doc
}

fn same_object(a: &Value, b: &Value) -> bool {
    engine::format::serialize_object(&without_category(a)) == engine::format::serialize_object(&without_category(b))
}

/// `name`, or `name-2`, `name-3`, ...: the first not in `taken` (spec R13).
fn free_name(name: &str, taken: &BTreeSet<String>) -> String {
    (2..).map(|n| format!("{name}-{n}")).find(|c| !taken.contains(c)).expect("a free suffix")
}

/// The library's objects with the bases first, in name order otherwise.
fn bases_first(objects: &BTreeMap<String, Value>) -> Vec<String> {
    fn visit(name: &str, objects: &BTreeMap<String, Value>, seen: &mut BTreeSet<String>, out: &mut Vec<String>) {
        if !seen.insert(name.to_string()) {
            return;
        }
        if let Some(base) = objects.get(name).and_then(|o| o.get("extends")).and_then(Value::as_str).filter(|b| objects.contains_key(*b)) {
            visit(base, objects, seen, out);
        }
        out.push(name.to_string());
    }
    let (mut seen, mut out) = (BTreeSet::new(), Vec::new());
    for name in objects.keys() {
        visit(name, objects, &mut seen, &mut out);
    }
    out
}

fn rgb(hex: &str) -> [i64; 3] {
    let n = |i: usize| i64::from_str_radix(hex.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
    [n(1), n(3), n(5)]
}

/// The document's color of a tier nearest to `hex`: the smallest squared RGB
/// distance, ties to the earlier in the palette (spec R15).
pub fn nearest_color(doc: &Sources, tier: &str, hex: &str) -> Option<(String, String)> {
    let want = rgb(hex);
    let mut best: Option<(i64, &String, &String)> = None;
    for (name, c) in doc.colors.iter().filter(|(_, c)| c.tier == tier) {
        let have = rgb(&c.hex);
        let d: i64 = (0..3).map(|i| (want[i] - have[i]).pow(2)).sum();
        if best.is_none_or(|(b, _, _)| d < b) {
            best = Some((d, name, &c.hex));
        }
    }
    best.map(|(_, n, h)| (n.clone(), h.clone()))
}

/// Plans importing `lib` into the document `doc` (spec R13–R15, D5), as pure
/// data. The library should have passed [`library_problems`].
pub fn plan_import(doc: &Sources, lib: &Library) -> ImportPlan {
    let mut taken: BTreeSet<String> = doc.objects.keys().chain(lib.objects.keys()).cloned().collect();
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let mut objects: Vec<PlannedObject> = Vec::new();
    for name in bases_first(&lib.objects) {
        let mut object = lib.objects[&name].clone();
        if let Some(to) = object.get("extends").and_then(Value::as_str).and_then(|b| names.get(b)).cloned() {
            object["extends"] = json!(to);
        }
        let (fate, as_name) = match doc.objects.get(&name) {
            None => (Fate::New, name.clone()),
            Some(mine) if same_object(&object, mine) => (Fate::Same, name.clone()),
            // The figures of the library use the document's character (D5).
            Some(_) if name == "character" => (Fate::Swapped, name.clone()),
            Some(_) => {
                let to = free_name(&name, &taken);
                taken.insert(to.clone());
                (Fate::Renamed, to)
            }
        };
        names.insert(name.clone(), as_name.clone());
        objects.push(PlannedObject { name, as_name, fate });
    }
    objects.sort_by(|a, b| a.name.cmp(&b.name));
    let fate_of = |n: &str| objects.iter().find(|o| o.name == n).map(|o| o.fate);
    let copied: Vec<&str> = objects.iter().filter(|o| o.fate.copied()).map(|o| o.name.as_str()).collect();

    // The objects whose look the import depends on: the copied ones, and the
    // equal objects they extend (the document's colors draw those).
    let mut relevant: BTreeSet<&str> = copied.iter().copied().collect();
    for name in &copied {
        let mut at = *name;
        while let Some(base) = lib.objects[at].get("extends").and_then(Value::as_str) {
            if fate_of(base) != Some(Fate::Same) {
                break;
            }
            relevant.insert(base);
            at = base;
        }
    }
    let mut differing: Vec<ColorDifference> = Vec::new();
    let mut missing: Vec<MissingColor> = Vec::new();
    for tier in [&lib.world, &lib.outfit] {
        for (color, hex) in tier {
            let users: Vec<&str> = relevant.iter().copied().filter(|n| color_refs(&lib.objects[*n]).contains(color)).collect();
            if users.is_empty() {
                continue;
            }
            let tier_name = lib.color(color).map_or("world", |(t, _)| t);
            match doc.colors.get(color.as_str()) {
                Some(mine) if mine.hex != *hex => differing.push(ColorDifference {
                    name: color.clone(),
                    tier: tier_name,
                    library: hex.clone(),
                    document: mine.hex.clone(),
                }),
                Some(_) => {}
                None => {
                    let used_by: Vec<String> = users.iter().filter(|n| copied.contains(n)).map(|n| n.to_string()).collect();
                    if !used_by.is_empty() {
                        missing.push(MissingColor {
                            name: color.clone(),
                            tier: tier_name,
                            hex: hex.clone(),
                            nearest: nearest_color(doc, tier_name, hex),
                            used_by,
                        });
                    }
                }
            }
        }
    }
    let character_swapped = fate_of("character") == Some(Fate::Swapped);
    // The copied objects that extend `character`, directly or through the objects they extend.
    let extends_character = |start: &str| {
        let mut at = start.to_string();
        for _ in 0..64 {
            match lib.objects.get(&at).and_then(|o| o.get("extends")).and_then(Value::as_str) {
                Some("character") => return true,
                Some(b) => at = b.to_string(),
                None => return false,
            }
        }
        false
    };
    let figures =
        if character_swapped { copied.iter().filter(|n| extends_character(n)).map(|n| n.to_string()).collect() } else { Vec::new() };
    ImportPlan { objects, differing, character_swapped, figures, missing }
}

/// Why a conversion is refused, with the command's code.
#[derive(Clone, Debug, PartialEq)]
pub struct ConvertProblem {
    pub code: &'static str,
    pub message: String,
}

/// Checks the conversions the owner chose against the plan (spec R15): each
/// names a missing color, converts it to a color the document has, of its tier.
pub fn check_conversions(doc: &Sources, plan: &ImportPlan, convert: &BTreeMap<String, String>) -> Result<(), ConvertProblem> {
    for (from, to) in convert {
        let Some(missing) = plan.missing.iter().find(|m| &m.name == from) else {
            return Err(ConvertProblem { code: "usage", message: format!("{from} isn't a missing color of this import") });
        };
        let Some(target) = doc.colors.get(to.as_str()) else {
            return Err(ConvertProblem { code: "usage", message: format!("the document has no color named {to} to convert {from} to") });
        };
        if target.tier != missing.tier {
            return Err(ConvertProblem {
                code: "wrong_tier",
                message: format!(
                    "{from} is {} color and {to} is {} color; a color converts only within its tier",
                    a_tier(missing.tier),
                    a_tier(target.tier)
                ),
            });
        }
    }
    Ok(())
}

fn a_tier(tier: &str) -> String {
    format!("{} {tier}", if tier == "outfit" { "an" } else { "a" })
}

/// The objects an import adds, under their final names (spec D5): the copied
/// objects, with `extends` following the renames, converted colors rewritten
/// in their keys, and their categories kept.
pub fn imported_objects(plan: &ImportPlan, lib: &Library, convert: &BTreeMap<String, String>) -> Vec<(String, Value)> {
    let names = plan.names();
    // Bases first, so each object is saved after the one it extends.
    bases_first(&lib.objects)
        .into_iter()
        .filter_map(|name| plan.copied().into_iter().find(|o| o.name == name))
        .map(|o| {
            let mut object = lib.objects[&o.name].clone();
            if let Some(to) = object.get("extends").and_then(Value::as_str).and_then(|b| names.get(b)) {
                object["extends"] = json!(to);
            }
            convert_colors(&mut object, convert);
            (o.as_name.clone(), object)
        })
        .collect()
}

// ------------------------------------------------------------ palette files

/// A `.pixelpal`, read (spec R19): a palette plus `format` and `kind`.
#[derive(Clone, Debug, Default)]
pub struct PaletteFile {
    pub world: Tier,
    pub outfit: Tier,
    pub notes: Option<Value>,
    pub color_notes: BTreeMap<String, String>,
}

/// The data of a `.pixelpal` made from a document's palette: `format` and `kind` first.
pub fn palette_file_value(palette: &Value) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("format".into(), json!(FORMAT));
    out.insert("kind".into(), json!("palette"));
    if let Value::Object(m) = palette {
        for (k, v) in m {
            out.insert(k.clone(), v.clone());
        }
    }
    Value::Object(out)
}

/// A palette file's canonical text.
pub fn palette_file_text(palette: &Value) -> String {
    engine::format::serialize_palette(&palette_file_value(palette))
}

fn tier_list(palette: &Value, tier: &str) -> Tier {
    match palette.get(tier) {
        Some(Value::Object(m)) => m.iter().filter_map(|(n, h)| h.as_str().map(|h| (n.clone(), h.to_string()))).collect(),
        _ => Vec::new(),
    }
}

/// Reads a `.pixelpal` and checks it the way the engine checks a palette
/// (names, hex values, caps, notes). A file that isn't one is a `palette_file` error.
pub fn read_palette_file(path: &Path) -> Result<PaletteFile, CommandError> {
    let shown = path.display();
    if !path.exists() {
        return Err(CommandError::not_found(format!("no palette file at {shown}")));
    }
    let not_one = |why: String| CommandError::new(format!("{shown} isn't a Pixel Lab palette file: {why}"), "palette_file");
    let text = std::fs::read_to_string(path).map_err(|e| not_one(format!("it can't be read: {e}")))?;
    let data = engine::format::parse(&text).map_err(|e| not_one(format!("it isn't JSON: {e}")))?;
    match data.get("format").and_then(Value::as_u64) {
        Some(FORMAT) => {}
        Some(n) => return Err(not_one(format!("it is format {n}; this Pixel Lab reads format {FORMAT}"))),
        None => return Err(not_one("it has no format".into())),
    }
    if data.get("kind").and_then(Value::as_str) != Some("palette") {
        return Err(not_one("its kind is not \"palette\"".into()));
    }
    let mut palette = data.clone();
    if let Value::Object(m) = &mut palette {
        m.shift_remove("format");
        m.shift_remove("kind");
    }
    let problems: Vec<String> = validate(&load_sources(palette.clone(), BTreeMap::new(), BTreeMap::new()))
        .into_iter()
        .map(|p| p.strip_prefix("palette.json: ").map_or(p.clone(), str::to_string))
        .collect();
    if !problems.is_empty() {
        return Err(not_one(problems.join("; ")));
    }
    let color_notes = match palette.get("colorNotes") {
        Some(Value::Object(m)) => m.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string()))).collect(),
        _ => BTreeMap::new(),
    };
    Ok(PaletteFile {
        world: tier_list(&palette, "world"),
        outfit: tier_list(&palette, "outfit"),
        notes: palette.get("notes").cloned(),
        color_notes,
    })
}

/// A color the file has and the document doesn't (spec R21).
#[derive(Clone, Debug, PartialEq)]
pub struct NewColor {
    pub name: String,
    pub tier: &'static str,
    pub hex: String,
}

/// A color both have, in the same tier, with different values (spec R22).
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteDifference {
    pub name: String,
    pub tier: &'static str,
    pub theirs: String,
    pub mine: String,
}

/// A color both have, in different tiers (spec R22). It stays as the document has it.
#[derive(Clone, Debug, PartialEq)]
pub struct TierConflict {
    pub name: String,
    /// The tier the document has it in.
    pub tier: &'static str,
    pub theirs_tier: &'static str,
    pub theirs: String,
    pub mine: String,
}

/// What importing a palette file would do (D6), computed without changing anything.
#[derive(Clone, Debug, Default)]
pub struct PalettePlan {
    pub new: Vec<NewColor>,
    pub differing: Vec<PaletteDifference>,
    pub other_tier: Vec<TierConflict>,
}

/// Plans importing `file` into a document's palette (spec R21, R22).
pub fn plan_palette(doc: &Sources, file: &PaletteFile) -> PalettePlan {
    let mut plan = PalettePlan::default();
    for (tier, list) in [("world", &file.world), ("outfit", &file.outfit)] {
        for (name, hex) in list {
            match doc.colors.get(name.as_str()) {
                None => plan.new.push(NewColor { name: name.clone(), tier, hex: hex.clone() }),
                Some(mine) if mine.tier != tier => plan.other_tier.push(TierConflict {
                    name: name.clone(),
                    tier: mine.tier,
                    theirs_tier: tier,
                    theirs: hex.clone(),
                    mine: mine.hex.clone(),
                }),
                Some(mine) if mine.hex != *hex => {
                    plan.differing.push(PaletteDifference { name: name.clone(), tier, theirs: hex.clone(), mine: mine.hex.clone() })
                }
                Some(_) => {}
            }
        }
    }
    plan
}

/// How many colors adding `new` would put over each tier's cap: (world, outfit) (spec R23).
pub fn over_cap(doc: &Sources, new: &[NewColor]) -> (usize, usize) {
    let count = |tier: &str| doc.colors.iter().filter(|(_, c)| c.tier == tier).count();
    let added = |tier: &str| new.iter().filter(|c| c.tier == tier).count();
    let over = |tier: &str, cap: usize| (count(tier) + added(tier)).saturating_sub(cap);
    (over("world", engine::engine::CAPS.world), over("outfit", engine::engine::CAPS.outfit))
}

/// Applies a palette import to a document's palette: the new colors at the
/// end of their tiers with their notes, and the chosen values of `theirs`.
pub fn apply_palette(palette: &mut Value, file: &PaletteFile, add: &[NewColor], theirs: &[PaletteDifference]) {
    for c in add {
        if let Some(Value::Object(tier)) = palette.get_mut(c.tier) {
            tier.insert(c.name.clone(), json!(c.hex));
        }
    }
    for d in theirs {
        if let Some(Value::Object(tier)) = palette.get_mut(d.tier) {
            tier.insert(d.name.clone(), json!(d.theirs));
        }
    }
    let notes: Vec<(&NewColor, &String)> = add.iter().filter_map(|c| file.color_notes.get(&c.name).map(|n| (c, n))).collect();
    if !notes.is_empty() {
        if let Value::Object(m) = palette {
            let entry = m.entry("colorNotes").or_insert_with(|| json!({}));
            if let Value::Object(map) = entry {
                for (c, note) in notes {
                    map.insert(c.name.clone(), json!(note));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(world: Value, objects: Vec<(&str, Value)>) -> Sources {
        load_sources(
            json!({ "world": world, "outfit": {} }),
            objects.into_iter().map(|(n, o)| (n.to_string(), o)).collect(),
            BTreeMap::new(),
        )
    }

    fn sprite(color: &str) -> Value {
        json!({ "kind": "sprite", "anchor": [0, 0], "keys": { "a": color }, "layers": [{ "map": ["a"] }] })
    }

    #[test]
    fn r4_a_typed_path_splits_on_either_separator_and_trims() {
        assert_eq!(split_category("Nature / Trees"), ["Nature", "Trees"]);
        assert_eq!(split_category("Nature › Trees"), ["Nature", "Trees"]);
        assert_eq!(category_from_param(&json!(["Nature", "Big/Trees"])).unwrap(), ["Nature", "Big", "Trees"]);
        assert!(category_from_param(&json!("Nature//Trees")).is_err(), "an empty name is refused");
        assert!(category_from_param(&json!("all")).is_err());
    }

    #[test]
    fn r9_a_category_is_within_its_prefix_without_regard_to_case() {
        let names = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(category_within(&names(&["Nature", "Trees"]), &names(&["nature"])));
        assert!(category_within(&names(&["Nature"]), &names(&["NATURE"])));
        assert!(!category_within(&names(&["Nature"]), &names(&["Nature", "Trees"])));
        assert!(!category_within(&names(&["Trees"]), &names(&["Nature"])));
    }

    #[test]
    fn r15_the_nearest_color_is_the_smallest_distance_in_the_tier_and_ties_go_to_the_earlier() {
        let d = doc(json!({ "dark": "#101010", "mid-a": "#808080", "mid-b": "#808080", "light": "#F0F0F0" }), vec![]);
        assert_eq!(nearest_color(&d, "world", "#7F7F7F"), Some(("mid-a".into(), "#808080".into())));
        assert_eq!(nearest_color(&d, "world", "#000000").unwrap().0, "dark");
        assert_eq!(nearest_color(&d, "outfit", "#000000"), None, "an empty tier has none");
    }

    #[test]
    fn r13_a_rename_takes_the_first_free_suffix_and_an_equal_object_is_the_documents() {
        let d = doc(json!({ "grass": "#00FF00" }), vec![("pine", sprite("grass")), ("pine-2", sprite("grass"))]);
        let mut lib =
            Library { name: "l".into(), exported_from: "x".into(), world: vec![("grass".into(), "#00FF00".into())], ..Library::default() };
        let mut other = sprite("grass");
        other["anchor"] = json!([1, 1]);
        lib.objects.insert("pine".into(), other);
        lib.objects.insert("fir".into(), sprite("grass"));
        lib.objects.insert("pine-2".into(), sprite("grass"));
        let plan = plan_import(&d, &lib);
        let fate = |n: &str| plan.objects.iter().find(|o| o.name == n).map(|o| (o.fate, o.as_name.clone())).unwrap();
        assert_eq!(fate("pine"), (Fate::Renamed, "pine-3".into()), "pine-2 is taken in the document");
        assert_eq!(fate("pine-2"), (Fate::Same, "pine-2".into()));
        assert_eq!(fate("fir"), (Fate::New, "fir".into()));
        assert!(plan.differing.is_empty() && plan.missing.is_empty() && !plan.differs());
    }

    #[test]
    fn r22_a_palette_plan_sorts_new_differing_and_other_tier_colors() {
        let d = doc(json!({ "a": "#111111", "b": "#222222" }), vec![]);
        let file = PaletteFile {
            world: vec![("a".into(), "#111111".into()), ("b".into(), "#999999".into()), ("c".into(), "#333333".into())],
            outfit: vec![("d".into(), "#444444".into())],
            ..PaletteFile::default()
        };
        let plan = plan_palette(&d, &file);
        assert_eq!(plan.new.iter().map(|c| (c.name.as_str(), c.tier)).collect::<Vec<_>>(), [("c", "world"), ("d", "outfit")]);
        assert_eq!(plan.differing.len(), 1);
        assert_eq!(plan.differing[0].name, "b");
        assert!(plan.other_tier.is_empty());
        assert_eq!(over_cap(&d, &plan.new), (0, 0));
    }
}
