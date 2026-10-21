//! Shared by the engine's integration tests: the repo, the committed
//! fixture.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// The committed format-2 fixture.
pub fn fixture() -> PathBuf {
    repo().join("fixtures/sample.pixellab")
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
