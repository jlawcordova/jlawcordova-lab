//! The canonical text (spec D4): every file of the committed fixture is the
//! canonical serialization of itself.

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
