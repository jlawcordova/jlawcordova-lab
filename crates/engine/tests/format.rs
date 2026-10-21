//! The canonical text (plan step 5, spec D4): every bootstrapped file is the
//! canonical serialization of itself, so the Rust writer and the Node
//! bootstrap (scripts/goldens.mjs) agree byte for byte.

mod common;

use std::path::Path;

use engine::format::{parse, serialize_file};

fn check_document(root: &Path) -> usize {
    let mut n = 0;
    for file in common::sources(root) {
        let text = std::fs::read_to_string(root.join(&file)).unwrap();
        let doc = parse(&text).unwrap_or_else(|e| panic!("{}: {e}", root.join(&file).display()));
        assert_eq!(serialize_file(&file, &doc), text, "{} isn't canonical", root.join(&file).display());
        n += 1;
    }
    n
}

#[test]
fn the_fixture_is_canonical() {
    assert_eq!(check_document(&common::fixture()), 5);
}

#[test]
fn every_bootstrapped_file_is_canonical() {
    let Some(dir) = common::goldens("every_bootstrapped_file_is_canonical") else { return };
    for (name, format2, _) in common::corpus(&dir) {
        let n = check_document(&format2);
        eprintln!("{name}: {n} files canonical");
    }
}
