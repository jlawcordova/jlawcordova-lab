//! Copies of the examples (format 2, made by `scripts/make-examples.mjs`), for the
//! tests that need the harbor or the town and no site checkout.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use crate::common;

/// A copy of `examples/<name>.pixellab` at `<base>/<name>.pixellab`, so a test can edit it.
pub fn example(base: &Path, name: &str) -> PathBuf {
    let to = base.join(format!("{name}.pixellab"));
    copy_dir(&common::repo().join("examples").join(format!("{name}.pixellab")), &to);
    to
}

/// Copies a folder and everything in it.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}
