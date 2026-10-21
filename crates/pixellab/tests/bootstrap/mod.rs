//! Format-2 copies of the examples, made with Node's `goldens.mjs --bootstrap`
//! (the tests need the harbor or the town, and no site checkout).
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::common;

/// A format-2 copy of `examples/<name>.pixellab` at `<base>/<name>.pixellab`.
pub fn example(base: &Path, name: &str) -> PathBuf {
    let repo = common::repo();
    let to = base.join(format!("{name}.pixellab"));
    let out = Command::new("node")
        .arg(repo.join("scripts/goldens.mjs"))
        .arg("--bootstrap")
        .arg(repo.join("examples").join(format!("{name}.pixellab")))
        .arg(&to)
        .output()
        .expect("run node");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    to
}
