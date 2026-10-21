//! Compiling scenes to the compact SVGs a site inlines (ports
//! cli/lib/engine/compile.mjs; the site's pixel-art engine spec D6, R7, R8).
//! Each scene renders to a rect SVG, which `optimize_svg` turns into paths;
//! `verify_lossless` checks every fill group covers exactly the same pixels;
//! then the budget is checked. `head_sources` runs `git` as a subprocess, as
//! compile.mjs did.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use std::collections::{BTreeMap as Tree, HashSet};
use std::process::Command;

use serde_json::Value;

use crate::deflate::gzip;
use crate::engine::{load_sources, render_scene_with, validate, Order, Sources};
use crate::format::{read_docs, stringify};
use crate::motion::items_by_id;
use crate::omap::{OMap, OSet};
use crate::svg::to_rect_svg;
use crate::{jsnum, EngineError, Result};

/// An output's budget, in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    pub raw: u64,
    pub gzip: u64,
}

/// The engine's budget for an output, unless the manifest gives it one.
pub const BUDGET: Budget = Budget { raw: 100 * 1024, gzip: 25 * 1024 };

/// Budgets by output file name, from the document's manifest.
pub type Budgets = BTreeMap<String, Budget>;

/// A node of the sources' markup: an element or text (`parse`).
#[derive(Clone, Debug, PartialEq)]
pub enum Markup {
    Element { name: String, attrs: Vec<(String, String)>, children: Vec<Markup>, self_closing: bool },
    Text(String),
}

fn word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn name_char(b: u8) -> bool {
    word(b) || b == b':' || b == b'-'
}

/// JavaScript's `\s` for ASCII input, which these sources are.
fn space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

/// A tag at `at`, as compile.mjs's TOKEN matches it: closing, name, attrs,
/// self-closing, and where it ends.
fn tag(src: &[u8], at: usize) -> Option<(bool, String, Vec<(String, String)>, bool, usize)> {
    let mut i = at + 1;
    let closing = src.get(i) == Some(&b'/');
    if closing {
        i += 1;
    }
    if !src.get(i).is_some_and(u8::is_ascii_alphabetic) {
        return None;
    }
    let start = i;
    while src.get(i).is_some_and(|&b| name_char(b)) {
        i += 1;
    }
    let name = String::from_utf8_lossy(&src[start..i]).into_owned();
    let mut attrs = Vec::new();
    loop {
        let mut j = i;
        if !src.get(j).is_some_and(|&b| space(b)) {
            break;
        }
        while src.get(j).is_some_and(|&b| space(b)) {
            j += 1;
        }
        let k0 = j;
        while src.get(j).is_some_and(|&b| name_char(b)) {
            j += 1;
        }
        if j == k0 || src.get(j) != Some(&b'=') || src.get(j + 1) != Some(&b'"') {
            break;
        }
        let key = String::from_utf8_lossy(&src[k0..j]).into_owned();
        let v0 = j + 2;
        let mut v = v0;
        while src.get(v).is_some_and(|&b| b != b'"' && b != b'<') {
            v += 1;
        }
        if src.get(v) != Some(&b'"') {
            break;
        }
        attrs.push((key, String::from_utf8_lossy(&src[v0..v]).into_owned()));
        i = v + 1;
    }
    while src.get(i).is_some_and(|&b| space(b)) {
        i += 1;
    }
    let self_closing = src.get(i) == Some(&b'/');
    if self_closing {
        i += 1;
    }
    if src.get(i) != Some(&b'>') {
        return None;
    }
    Some((closing, name, attrs, self_closing, i + 1))
}

fn slice40(source: &str, at: usize) -> String {
    source[at..].chars().take(40).collect()
}

/// Parses just enough XML for the rect SVGs (`parse`): elements with
/// double-quoted attributes, and text.
pub fn parse(source: &str) -> std::result::Result<Vec<Markup>, String> {
    let src = source.as_bytes();
    // A stack of open elements: (name, attrs, children); the root is first.
    let mut stack: Vec<(String, Vec<(String, String)>, Vec<Markup>)> = vec![(String::new(), vec![], vec![])];
    let mut at = 0;
    while at < src.len() {
        if src[at] != b'<' {
            let end = src[at..].iter().position(|&b| b == b'<').map_or(src.len(), |p| at + p);
            stack.last_mut().unwrap().2.push(Markup::Text(source[at..end].to_string()));
            at = end;
            continue;
        }
        let Some((closing, name, attrs, self_closing, end)) = tag(src, at) else {
            return Err(format!("Unsupported markup at offset {at}: {}", slice40(source, at)));
        };
        if closing {
            if stack.len() == 1 || stack.last().unwrap().0 != name {
                return Err(format!("Unsupported markup: unexpected </{name}> at offset {at}"));
            }
            let (name, attrs, children) = stack.pop().unwrap();
            stack.last_mut().unwrap().2.push(Markup::Element { name, attrs, children, self_closing: false });
        } else if self_closing {
            stack.last_mut().unwrap().2.push(Markup::Element { name, attrs, children: vec![], self_closing: true });
        } else {
            stack.push((name, attrs, vec![]));
        }
        at = end;
    }
    if stack.len() != 1 {
        return Err(format!("Unsupported markup: <{}> is never closed", stack.last().unwrap().0));
    }
    Ok(stack.pop().unwrap().2)
}

/// An element's attribute (`attr`).
pub fn attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

fn attrs_text(attrs: &[(String, String)]) -> String {
    attrs.iter().map(|(k, v)| format!(" {k}=\"{v}\"")).collect()
}

fn serialize(nodes: &[Markup], out: &mut String) {
    for n in nodes {
        match n {
            Markup::Text(t) => out.push_str(t),
            Markup::Element { name, attrs, self_closing: true, .. } => {
                out.push_str(&format!("<{name}{}/>", attrs_text(attrs)));
            }
            Markup::Element { name, attrs, children, .. } => {
                out.push_str(&format!("<{name}{}>", attrs_text(attrs)));
                serialize(children, out);
                out.push_str(&format!("</{name}>"));
            }
        }
    }
}

/// A `<g fill>` whose children are all `<rect>`s, at least one (`isRectGroup`).
pub fn is_rect_group(node: &Markup) -> bool {
    match node {
        Markup::Element { name, attrs, children, .. } => {
            name == "g"
                && attr(attrs, "fill").is_some()
                && !children.is_empty()
                && children.iter().all(|c| matches!(c, Markup::Element { name, .. } if name == "rect"))
        }
        Markup::Text(_) => false,
    }
}

fn whole_number(attrs: &[(String, String)], key: &str) -> std::result::Result<i64, String> {
    let raw = attr(attrs, key).or(if key == "x" || key == "y" { Some("0") } else { None });
    let ok = raw.is_some_and(|r| {
        let digits = r.strip_prefix('-').unwrap_or(r);
        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
    });
    if !ok {
        return Err(format!("Only whole-number rects are supported; got {key}=\"{}\"", raw.unwrap_or("undefined")));
    }
    Ok(raw.unwrap().parse::<i64>().unwrap_or(0))
}

/// The pixels of rects, in the order they're listed (`rectPixels`).
pub fn rect_pixels(rects: &[Markup]) -> std::result::Result<OSet<(i64, i64)>, String> {
    let mut pixels = OSet::new();
    for r in rects {
        let Markup::Element { attrs, .. } = r else { continue };
        let [x, y, w, h] =
            [whole_number(attrs, "x")?, whole_number(attrs, "y")?, whole_number(attrs, "width")?, whole_number(attrs, "height")?];
        for dy in 0..h {
            for dx in 0..w {
                pixels.add((x + dx, y + dy));
            }
        }
    }
    Ok(pixels)
}

/// Path data for a pixel set (`pathData`): horizontal runs per row, then
/// runs with the same x and width in consecutive rows merged.
fn path_data(pixels: &OSet<(i64, i64)>) -> String {
    let mut rows: Tree<i64, Vec<i64>> = Tree::new();
    for &(x, y) in pixels.iter() {
        rows.entry(y).or_default().push(x);
    }
    let mut runs: Tree<i64, Vec<(i64, i64)>> = Tree::new();
    for (y, xs) in rows.iter_mut() {
        xs.sort();
        let mut row = Vec::new();
        let mut i = 0;
        while i < xs.len() {
            let mut j = i;
            while j + 1 < xs.len() && xs[j + 1] == xs[j] + 1 {
                j += 1;
            }
            row.push((xs[i], (j - i + 1) as i64));
            i = j + 1;
        }
        runs.insert(*y, row);
    }
    let mut taken: HashSet<(i64, i64, i64)> = HashSet::new();
    let mut d = String::new();
    for (&y, row) in &runs {
        for &(x, w) in row {
            if taken.contains(&(y, x, w)) {
                continue;
            }
            let mut h = 1;
            while runs.get(&(y + h)).is_some_and(|r| r.contains(&(x, w))) && !taken.contains(&(y + h, x, w)) {
                taken.insert((y + h, x, w));
                h += 1;
            }
            d.push_str(&format!("M{x} {y}h{w}v{h}h-{w}z"));
        }
    }
    d
}

fn optimize_nodes(nodes: &[Markup]) -> std::result::Result<Vec<Markup>, String> {
    nodes
        .iter()
        .map(|n| match n {
            Markup::Text(_) => Ok(n.clone()),
            Markup::Element { attrs, children, .. } if is_rect_group(n) => {
                let mut attrs = attrs.clone();
                attrs.push(("d".into(), path_data(&rect_pixels(children)?)));
                Ok(Markup::Element { name: "path".into(), attrs, children: vec![], self_closing: true })
            }
            Markup::Element { name, attrs, children, self_closing } => Ok(Markup::Element {
                name: name.clone(),
                attrs: attrs.clone(),
                children: optimize_nodes(children)?,
                self_closing: *self_closing,
            }),
        })
        .collect()
}

/// Rect form → path form (`optimizeSvg`). Errs on markup it doesn't understand.
pub fn optimize_svg(source: &str) -> std::result::Result<String, String> {
    let mut out = String::with_capacity(source.len() / 2);
    serialize(&optimize_nodes(&parse(source)?)?, &mut out);
    Ok(out)
}

fn path_pixels(d: &str) -> std::result::Result<HashSet<(i64, i64)>, String> {
    let mut pixels = HashSet::new();
    let b = d.as_bytes();
    let mut i = 0;
    let num = |i: &mut usize, signed: bool| -> Option<i64> {
        let start = *i;
        if signed && b.get(*i) == Some(&b'-') {
            *i += 1;
        }
        let d0 = *i;
        while b.get(*i).is_some_and(u8::is_ascii_digit) {
            *i += 1;
        }
        if *i == d0 {
            return None;
        }
        d[start..*i].parse().ok()
    };
    let lit = |i: &mut usize, s: &str| -> bool {
        if d[*i..].starts_with(s) {
            *i += s.len();
            true
        } else {
            false
        }
    };
    while i < b.len() {
        let mut j = i;
        let parsed = (|| {
            if !lit(&mut j, "M") {
                return None;
            }
            let x = num(&mut j, true)?;
            if !lit(&mut j, " ") {
                return None;
            }
            let y = num(&mut j, true)?;
            if !lit(&mut j, "h") {
                return None;
            }
            let w = num(&mut j, false)?;
            if !lit(&mut j, "v") {
                return None;
            }
            let h = num(&mut j, false)?;
            if !lit(&mut j, "h-") {
                return None;
            }
            let w2 = num(&mut j, false)?;
            if !lit(&mut j, "z") {
                return None;
            }
            Some((x, y, w, h, w2))
        })();
        match parsed {
            // A failed sticky match resets lastIndex to 0, which the message slices from.
            None => return Err(format!("Unexpected path data: {}", &d[..d.len().min(40)])),
            Some((_, _, w, _, w2)) if w != w2 => return Err(format!("Unexpected path data: {}", &d[j..d.len().min(j + 40)])),
            Some((x, y, w, h, _)) => {
                for dy in 0..h {
                    for dx in 0..w {
                        pixels.insert((x + dx, y + dy));
                    }
                }
            }
        }
        i = j;
    }
    Ok(pixels)
}

/// A step of a flattened tree, for the lossless check.
enum Step {
    Text(String),
    Pixels(HashSet<(i64, i64)>, Vec<(String, String)>),
    Open(String),
    Close(String),
}

impl Step {
    /// JSON.stringify of the Node step (a Set prints as {}).
    fn json(&self) -> String {
        match self {
            Step::Text(t) => format!("{{\"text\":{}}}", stringify(&Value::String(t.clone()))),
            Step::Pixels(_, attrs) => {
                let list: Vec<Value> =
                    attrs.iter().map(|(k, v)| Value::Array(vec![Value::String(k.clone()), Value::String(v.clone())])).collect();
                format!("{{\"pixels\":{{}},\"attrs\":{}}}", stringify(&Value::Array(list)))
            }
            Step::Open(s) => format!("{{\"open\":{}}}", stringify(&Value::String(s.clone()))),
            Step::Close(s) => format!("{{\"close\":{}}}", stringify(&Value::String(s.clone()))),
        }
    }
}

fn steps(nodes: &[Markup], out: &mut Vec<Step>) -> std::result::Result<(), String> {
    for n in nodes {
        match n {
            Markup::Text(t) => out.push(Step::Text(t.clone())),
            Markup::Element { attrs, children, .. } if is_rect_group(n) => {
                out.push(Step::Pixels(rect_pixels(children)?.iter().copied().collect(), attrs.clone()))
            }
            Markup::Element { name, attrs, .. } if name == "path" && attr(attrs, "d").is_some() => {
                let rest = attrs.iter().filter(|(k, _)| k != "d").cloned().collect();
                out.push(Step::Pixels(path_pixels(attr(attrs, "d").unwrap())?, rest));
            }
            Markup::Element { name, attrs, children, .. } => {
                out.push(Step::Open(format!("<{name}{}>", attrs_text(attrs))));
                steps(children, out)?;
                out.push(Step::Close(name.clone()));
            }
        }
    }
    Ok(())
}

/// Errs unless `output` draws exactly the same pixels, groups and structure
/// as `source` (`verifyLossless`).
pub fn verify_lossless(source: &str, output: &str) -> std::result::Result<(), String> {
    let (mut a, mut b) = (Vec::new(), Vec::new());
    steps(&parse(source)?, &mut a)?;
    steps(&parse(output)?, &mut b)?;
    if a.len() != b.len() {
        return Err(format!("Structure changed: {} steps before, {} after", a.len(), b.len()));
    }
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        match (x, y) {
            (Step::Pixels(xp, xa), Step::Pixels(yp, ya)) => {
                let label = format!("fill group {} (step {i})", attr(xa, "fill").map_or_else(|| attrs_text(xa), str::to_string));
                if attrs_text(xa) != attrs_text(ya) {
                    return Err(format!("Attributes changed on {label}"));
                }
                if xp != yp {
                    return Err(format!("Pixels changed in {label}: {} before, {} after", xp.len(), yp.len()));
                }
            }
            _ => {
                if x.json() != y.json() {
                    return Err(format!("Structure changed at step {i}: {} → {}", x.json(), y.json()));
                }
            }
        }
    }
    Ok(())
}

/// "12.3 KB" (`kb`).
pub fn kb(bytes: u64) -> String {
    format!("{} KB", jsnum::to_fixed(bytes as f64 / 1024.0, 1))
}

/// A compiled scene (`compileScene`'s result).
#[derive(Clone, Debug, PartialEq)]
pub struct Compiled {
    pub output: String,
    pub raw: u64,
    pub gzip: u64,
    pub over: bool,
    pub budget: Budget,
    pub report: String,
}

fn hex_of(sources: &Sources) -> impl Fn(&str) -> String + '_ {
    |color: &str| sources.colors.get(color).map_or_else(|| "undefined".to_string(), |c| c.hex.clone())
}

/// `PIXELLAB_LEGACY_SVG=1` makes the compiler write the SVGs as the Node CLI
/// did, unscoped and with no frame loops in them, so the parity check (spec
/// R3) can still compare them until cutover. Test-only; read here alone.
fn legacy_svg() -> bool {
    std::env::var("PIXELLAB_LEGACY_SVG").is_ok_and(|v| v == "1")
}

fn rect_svg(sources: &Sources, name: &str, order: Order) -> Result<String> {
    let scope = if legacy_svg() { None } else { Some(name) };
    Ok(to_rect_svg(&render_scene_with(sources, name, order, scope)?, &hex_of(sources)))
}

/// Renders a scene and runs it through the optimizer, lossless check and
/// budget (`compileScene`). A preview-only scene, or one without its own
/// budget, gets the engine's.
pub fn compile_scene(sources: &Sources, name: &str, budgets: &Budgets, order: Order) -> Result<Compiled> {
    let rects = rect_svg(sources, name, order)?;
    let output = optimize_svg(&rects).map_err(EngineError)?;
    verify_lossless(&rects, &output).map_err(EngineError)?;
    let (raw, gz) = (output.len() as u64, gzip(output.as_bytes()).len() as u64);
    let (src_raw, src_gz) = (rects.len() as u64, gzip(rects.as_bytes()).len() as u64);
    let file = sources.scenes.get(name).and_then(|s| s.get("output")).and_then(Value::as_str).filter(|f| !f.is_empty());
    let budget = file.and_then(|f| budgets.get(f)).copied().unwrap_or(BUDGET);
    let over = raw > budget.raw || gz > budget.gzip;
    let report = format!(
        "{} / {} gzip → {} / {} gzip, lossless{}",
        kb(src_raw),
        kb(src_gz),
        kb(raw),
        kb(gz),
        if over { " — OVER BUDGET" } else { "" }
    );
    Ok(Compiled { output, raw, gzip: gz, over, budget, report })
}

/// One scene written to the publish folder.
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub file: String,
    pub scene: String,
    pub raw: u64,
    pub gzip: u64,
    pub over: bool,
    pub report: String,
}

/// A scene without an output.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewOnly {
    pub scene: String,
    pub raw: u64,
    pub gzip: u64,
    pub over: bool,
    pub report: String,
}

/// What `compile_all` did.
#[derive(Clone, Debug, PartialEq)]
pub struct Build {
    pub ok: bool,
    pub outputs: Vec<Output>,
    pub preview_only: Vec<PreviewOnly>,
    pub problems: Vec<String>,
}

/// Compiles every scene and writes those with an `output` into `out`;
/// nothing is written when any source has a problem (`compileAll`).
pub fn compile_all(sources: &Sources, problems: &[String], out: &Path, budgets: &Budgets) -> Result<Build> {
    if !problems.is_empty() {
        return Ok(Build { ok: false, outputs: vec![], preview_only: vec![], problems: problems.to_vec() });
    }
    let (mut outputs, mut preview_only, mut over) = (Vec::new(), Vec::new(), false);
    for (name, scene) in &sources.scenes {
        let c = compile_scene(sources, name, budgets, Order::Depth)?;
        over |= c.over;
        match scene.get("output").and_then(Value::as_str).filter(|f| !f.is_empty()) {
            Some(file) => {
                std::fs::create_dir_all(out).map_err(|e| EngineError(e.to_string()))?;
                std::fs::write(out.join(file), &c.output).map_err(|e| EngineError(e.to_string()))?;
                outputs.push(Output {
                    file: file.to_string(),
                    scene: name.clone(),
                    raw: c.raw,
                    gzip: c.gzip,
                    over: c.over,
                    report: c.report,
                });
            }
            None => preview_only.push(PreviewOnly { scene: name.clone(), raw: c.raw, gzip: c.gzip, over: c.over, report: c.report }),
        }
    }
    Ok(Build { ok: !over, outputs, preview_only, problems: vec![] })
}

/// How many items a scene places, groups flattened (`countItems`).
pub fn count_items(items: &[Value]) -> usize {
    items
        .iter()
        .map(|item| match item.get("group").filter(|g| crate::engine::truthy(Some(g))) {
            Some(_) => count_items(item.get("items").and_then(Value::as_array).map_or(&[][..], |v| v.as_slice())),
            None => 1,
        })
        .sum()
}

/// A scene's compiled output and size, raw and gzip, without the lossless check (`measure`).
fn measure(sources: &Sources, name: &str) -> Result<(String, u64, u64)> {
    let output = optimize_svg(&rect_svg(sources, name, Order::Depth)?).map_err(EngineError)?;
    let gz = gzip(output.as_bytes()).len() as u64;
    Ok((output.clone(), output.len() as u64, gz))
}

fn is_group_item(item: &Value) -> bool {
    item.get("group").is_some_and(|g| crate::engine::truthy(Some(g)))
}

/// The scene's items without any that place `object` (`withoutObject`).
fn without_object(items: &[Value], object: &str) -> Vec<Value> {
    items
        .iter()
        .filter(|item| item.get("object").and_then(Value::as_str) != Some(object))
        .map(|item| {
            if is_group_item(item) {
                let mut out = item.clone();
                let inner = item.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
                out["items"] = Value::Array(without_object(&inner, object));
                out
            } else {
                item.clone()
            }
        })
        .collect()
}

/// How many times each object is placed (`placements`).
fn placements(items: &[Value], counts: &mut OMap<String, usize>) {
    for item in items {
        if is_group_item(item) {
            placements(item.get("items").and_then(Value::as_array).map_or(&[][..], |v| v.as_slice()), counts);
        } else {
            let object = crate::engine::js_string(item.get("object"));
            let n = counts.get(&object).copied().unwrap_or(0);
            counts.set(object, n + 1);
        }
    }
}

/// Bytes, raw and gzip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cost {
    pub raw: i64,
    pub gzip: i64,
}

/// What an object costs in a scene, and how many times it's placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectCost {
    pub placed: usize,
    pub raw: i64,
    pub gzip: i64,
}

/// What each object and each color costs in a compiled scene
/// (`sceneSizes`); colors are named "<name> <hex>".
#[derive(Clone, Debug, PartialEq)]
pub struct SceneSizes {
    pub raw: u64,
    pub gzip: u64,
    pub objects: OMap<String, ObjectCost>,
    pub colors: OMap<String, Cost>,
}

/// What each object and color costs in a compiled scene (`sceneSizes`).
pub fn scene_sizes(sources: &Sources, name: &str) -> Result<SceneSizes> {
    let scene = sources.scenes.get(name).ok_or_else(|| EngineError(format!("scene '{name}' does not exist")))?;
    let (output, raw, gz) = measure(sources, name)?;
    let items = scene.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut counts = OMap::new();
    placements(&items, &mut counts);
    let mut probe = sources.clone();
    let mut objects = OMap::new();
    for (object, placed) in counts.iter() {
        let left_items = without_object(&items, object);
        // A motion track moves an item by id, so the tracks of the items just taken out go with them.
        let left = items_by_id(&Value::Array(left_items.clone()));
        let mut probe_scene = scene.as_object().cloned().unwrap_or_default();
        probe_scene.shift_remove("output");
        probe_scene.insert("items".into(), Value::Array(left_items));
        if let Some(motion) = scene.get("motion").filter(|m| crate::engine::truthy(Some(m))) {
            let mut m = motion.clone();
            let tracks: Vec<Value> = motion
                .get("tracks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|t| t.get("item").and_then(Value::as_str).is_some_and(|i| left.has(i)))
                .cloned()
                .collect();
            m["tracks"] = Value::Array(tracks);
            probe_scene.insert("motion".into(), m);
        }
        probe.scenes.insert("sizes-probe".into(), Value::Object(probe_scene));
        let (_, less_raw, less_gz) = measure(&probe, "sizes-probe")?;
        objects.set(object.clone(), ObjectCost { placed: *placed, raw: raw as i64 - less_raw as i64, gzip: gz as i64 - less_gz as i64 });
    }
    // A hex can have several names; the first tier's (world before outfit) wins.
    let mut names: OMap<String, String> = OMap::new();
    for (color, c) in sources.colors.iter() {
        if !names.has(&c.hex) {
            names.set(c.hex.clone(), color.clone());
        }
    }
    let mut hexes: OSet<String> = OSet::new();
    let needle = "<path fill=\"";
    let mut from = 0;
    while let Some(p) = output[from..].find(needle) {
        let at = from + p + needle.len();
        let hex = &output[at..output.len().min(at + 7)];
        if hex.len() == 7
            && hex.starts_with('#')
            && hex[1..].chars().all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c))
            && output[at + 7..].starts_with('"')
        {
            hexes.add(hex.to_string());
        }
        from = at;
    }
    let mut colors = OMap::new();
    for hex in hexes.iter() {
        let less = drop_paths(&output, hex);
        let less_gz = gzip(less.as_bytes()).len() as i64;
        let key = format!("{} {hex}", names.get(hex).map_or("?", String::as_str));
        colors.set(key, Cost { raw: raw as i64 - less.len() as i64, gzip: gz as i64 - less_gz });
    }
    Ok(SceneSizes { raw, gzip: gz, objects, colors })
}

/// The output without the paths of one color: what
/// `replace(new RegExp(`<path fill="${hex}"[^>]*/>`, 'g'), '')` leaves.
fn drop_paths(output: &str, hex: &str) -> String {
    let needle = format!("<path fill=\"{hex}\"");
    let mut out = String::with_capacity(output.len());
    let mut rest = output;
    while let Some(p) = rest.find(&needle) {
        let after = &rest[p + needle.len()..];
        match after.find('>') {
            Some(gt) if gt > 0 && after.as_bytes()[gt - 1] == b'/' => {
                out.push_str(&rest[..p]);
                rest = &after[gt + 1..];
            }
            _ => {
                out.push_str(&rest[..p + needle.len()]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The sources as committed at HEAD (`headSources`): copied from git into
/// a temp folder, which `drop_head` removes.
pub struct Head {
    pub temp: PathBuf,
    pub sources: Sources,
    pub problems: Vec<String>,
}

/// The sources at HEAD of the git repository holding `dir`, or the reason
/// they can't be read (`headSources`).
pub fn head_sources(dir: &Path) -> std::result::Result<Head, String> {
    let git = |args: &[&str]| -> Option<Vec<u8>> {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        out.status.success().then_some(out.stdout)
    };
    let Some(list) = git(&["ls-tree", "-r", "--name-only", "HEAD", "--", "."]) else {
        return Err("not in a git repository with a HEAD commit".into());
    };
    let is_source = |f: &str| {
        f == "palette.json"
            || f.strip_prefix("objects/").is_some_and(|n| n.ends_with(".json") && !n.contains('/'))
            || f.strip_prefix("scenes/").is_some_and(|n| n.ends_with(".json") && !n.contains('/'))
    };
    let files: Vec<String> = String::from_utf8_lossy(&list).lines().filter(|f| is_source(f)).map(str::to_string).collect();
    if files.is_empty() {
        return Err("no sources at HEAD".into());
    }
    let temp = temp_dir("pixel-art-head-").map_err(|e| e.to_string())?;
    for file in &files {
        let path = temp.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let text = git(&["show", &format!("HEAD:./{file}")]).unwrap_or_default();
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
    }
    let docs = read_docs(&temp);
    let sources = load_sources(docs.palette.unwrap_or(Value::Null), docs.objects, docs.scenes);
    let mut problems = docs.load_errors;
    problems.extend(validate(&sources));
    Ok(Head { temp, sources, problems })
}

/// A new, empty temp folder named with `prefix` (`mkdtemp`).
pub fn temp_dir(prefix: &str) -> std::io::Result<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    for _ in 0..100 {
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        let name = format!("{prefix}{:x}{:x}{n:x}", std::process::id(), nanos);
        let path = std::env::temp_dir().join(name);
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("couldn't make a temp folder"))
}

/// Removes a `head_sources` temp folder (`dropHead`).
pub fn drop_head(head: &Head) {
    let _ = std::fs::remove_dir_all(&head.temp);
}
