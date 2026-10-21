//! Migration's reader (R8, D3): a format-1 source's comments become notes.

mod common;

use engine::migrate::convert;

#[test]
fn the_fixtures_palette_comment_becomes_its_notes() {
    let text = std::fs::read_to_string(common::repo().join("fixtures/format1/sample.pixellab/palette.mjs")).unwrap();
    let value = convert("palette.mjs", &text).unwrap();
    assert_eq!(
        value["notes"],
        serde_json::json!(["The sample document's palette (fixtures/format1): a few world colors and one outfit color."])
    );
}
