//! Migration's reader (R8, D3) against the bootstrap: every format-1 source
//! of the corpus, read by `migrate::convert`, is the bootstrap's format-2
//! text apart from notes (which the bootstrap, importing with Node, can't
//! keep).

mod common;

use engine::format::{parse, serialize_file};
use engine::migrate::convert;

#[test]
fn migrating_the_corpus_gives_the_bootstraps_text_apart_from_notes() {
    let Some(dir) = common::goldens("migrating_the_corpus_gives_the_bootstraps_text_apart_from_notes") else { return };
    let corpus: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("corpus.json")).unwrap()).unwrap();
    let mut notes = 0;
    for doc in corpus["docs"].as_array().unwrap() {
        let format1 = std::path::Path::new(doc["format1"].as_str().unwrap());
        let format2 = std::path::Path::new(doc["format2"].as_str().unwrap());
        for file in common::sources(format2) {
            let mjs = file.replace(".json", ".mjs");
            let text = std::fs::read_to_string(format1.join(&mjs)).unwrap();
            let mut value = convert(&mjs, &text).unwrap_or_else(|e| panic!("{e}"));
            if let Some(m) = value.as_object_mut() {
                notes += m.remove("notes").map_or(0, |n| n.as_array().map_or(0, Vec::len));
                notes += m.remove("colorNotes").map_or(0, |n| n.as_object().map_or(0, |o| o.len()));
            }
            let want = std::fs::read_to_string(format2.join(&file)).unwrap();
            assert_eq!(serialize_file(&file, &value), want, "{}", format1.join(&mjs).display());
            assert_eq!(parse(&want).unwrap(), parse(&serialize_file(&file, &value)).unwrap());
        }
    }
    eprintln!("{notes} notes carried");
}

#[test]
fn the_fixtures_palette_comment_becomes_its_notes() {
    let text = std::fs::read_to_string(common::repo().join("cli/test/fixtures/sample.pixellab/palette.mjs")).unwrap();
    let value = convert("palette.mjs", &text).unwrap();
    assert_eq!(
        value["notes"],
        serde_json::json!(["The sample document's palette (cli/test/fixtures): a few world colors and one outfit color."])
    );
}
