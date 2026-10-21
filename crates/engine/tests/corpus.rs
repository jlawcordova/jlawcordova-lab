//! The goldens harness (plan step 6): the engine's outputs for the corpus
//! against the Node CLI's, which `node scripts/goldens.mjs --out <dir>`
//! wrote. Run with PIXELLAB_GOLDENS=<dir> cargo test --test corpus; without
//! it, the tests say they were skipped.
//!
//! - every scene's compiled SVG, byte for byte (R3);
//! - every preview PNG, pixel for pixel (decoded RGBA and dimensions).
//!
//! The read commands' JSON results are compared by the program's own corpus
//! test (crates/pixellab/tests/corpus.rs).

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use engine::compile::{compile_scene, Budget, Budgets};
use engine::engine::{load_sources, Order, Sources};
use engine::format::{parse, read_docs};
use engine::png::decode;
use engine::preview::{preview_colors, render_preview, Target};

fn open(root: &Path) -> (Sources, serde_json::Value) {
    let docs = read_docs(root);
    assert!(docs.load_errors.is_empty(), "{}: {:?}", root.display(), docs.load_errors);
    let manifest = parse(&std::fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    (load_sources(docs.palette.unwrap_or_default(), docs.objects, docs.scenes), manifest)
}

fn budgets(manifest: &serde_json::Value) -> Budgets {
    let mut out = BTreeMap::new();
    if let Some(b) = manifest.pointer("/publish/budgets").and_then(|b| b.as_object()) {
        for (file, v) in b {
            out.insert(file.clone(), Budget { raw: v["raw"].as_u64().unwrap(), gzip: v["gzip"].as_u64().unwrap() });
        }
    }
    out
}

#[test]
fn every_scene_compiles_byte_identical() {
    let Some(dir) = common::goldens("every_scene_compiles_byte_identical") else { return };
    // Node's SVGs are today's, before spec R11–R15 (plan.md, "Departures").
    std::env::set_var("PIXELLAB_LEGACY_SVG", "1");
    let mut differ = Vec::new();
    for (name, format2, goldens) in common::corpus(&dir) {
        let (sources, manifest) = open(&format2);
        let budgets = budgets(&manifest);
        for scene in sources.scenes.keys() {
            let want = std::fs::read_to_string(goldens.join("svg").join(format!("{scene}.svg"))).unwrap();
            let got = compile_scene(&sources, scene, &budgets, Order::Depth).map(|c| c.output);
            match got {
                Ok(svg) if svg == want => {}
                Ok(svg) => differ.push(format!(
                    "{name}/{scene}: {} bytes, Node {} (first difference at byte {})",
                    svg.len(),
                    want.len(),
                    first_difference(&svg, &want)
                )),
                Err(e) => differ.push(format!("{name}/{scene}: {e}")),
            }
        }
    }
    assert!(differ.is_empty(), "{} scenes differ:\n{}", differ.len(), differ.join("\n"));
}

#[test]
fn every_preview_is_pixel_identical() {
    let Some(dir) = common::goldens("every_preview_is_pixel_identical") else { return };
    let mut differ = Vec::new();
    for (name, format2, goldens) in common::corpus(&dir) {
        let (sources, manifest) = open(&format2);
        let colors = preview_colors(
            manifest.pointer("/preview/card").and_then(|v| v.as_str()),
            manifest.pointer("/preview/grid").and_then(|v| v.as_str()),
        )
        .unwrap();
        let targets: Vec<(String, Target)> = sources
            .objects
            .keys()
            .map(|o| (format!("object-{o}"), Target::Object(o)))
            .chain(sources.scenes.keys().map(|s| (format!("scene-{s}"), Target::Scene(s))))
            .collect();
        for (file, target) in targets {
            let want = decode(&std::fs::read(goldens.join("png").join(format!("{file}.png"))).unwrap()).unwrap();
            match render_preview(&sources, target, colors) {
                Ok((image, _)) if image == want => {}
                Ok((image, _)) => {
                    differ.push(format!("{name}/{file}: {}×{}, Node {}×{}", image.width, image.height, want.width, want.height))
                }
                Err(e) => differ.push(format!("{name}/{file}: {e}")),
            }
        }
    }
    assert!(differ.is_empty(), "{} previews differ:\n{}", differ.len(), differ.join("\n"));
}

fn first_difference(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()))
}
