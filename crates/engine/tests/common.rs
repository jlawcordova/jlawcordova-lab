//! Shared by the engine's integration tests: the repo, the committed
//! fixture, and the corpus `scripts/goldens.mjs --out` wrote (when
//! PIXELLAB_GOLDENS names it).

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde_json::Value;

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// The committed format-2 fixture.
pub fn fixture() -> PathBuf {
    repo().join("fixtures/sample.pixellab")
}

/// The goldens folder, or None (and a note on stderr) when it isn't set.
pub fn goldens(test: &str) -> Option<PathBuf> {
    match std::env::var_os("PIXELLAB_GOLDENS") {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => {
            eprintln!("{test}: skipped (set PIXELLAB_GOLDENS to the folder `node scripts/goldens.mjs --out <dir>` wrote)");
            None
        }
    }
}

/// The corpus's documents: (name, format-2 folder, goldens folder).
pub fn corpus(dir: &Path) -> Vec<(String, PathBuf, PathBuf)> {
    let text = std::fs::read_to_string(dir.join("corpus.json")).expect("corpus.json");
    let corpus: Value = serde_json::from_str(&text).unwrap();
    corpus["docs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            let name = d["name"].as_str().unwrap().to_string();
            (name.clone(), PathBuf::from(d["format2"].as_str().unwrap()), dir.join("goldens").join(&name))
        })
        .collect()
}

/// Every source file of a format-2 document, relative to it.
pub fn sources(root: &Path) -> Vec<String> {
    let mut out = vec!["palette.json".to_string()];
    for sub in ["objects", "scenes"] {
        let mut names: Vec<String> = std::fs::read_dir(root.join(sub))
            .map(|d| {
                d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|f| f.ends_with(".json")).collect()
            })
            .unwrap_or_default();
        names.sort();
        out.extend(names.into_iter().map(|f| format!("{sub}/{f}")));
    }
    out
}
