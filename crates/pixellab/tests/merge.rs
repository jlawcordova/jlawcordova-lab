//! merge3 (docs/intents/2026-10-mcp-gaps/spec.md, D4): a three-way line merge
//! of canonical sources (ports cli/test/merge.test.mjs), and the diffs it's
//! built on.

use pixellab::diff::unified_diff;
use pixellab::merge::{fold_scene, hunks, merge3, trailing, untrailing, Conflict, Hunk, Merged};

fn lines(l: &[&str]) -> String {
    l.join("\n")
}

const BASE: [&str; 5] = ["a", "b", "c", "d", "e"];

fn hunk(from: usize, to: usize, lines: &[&str]) -> Hunk {
    Hunk { from, to, lines: lines.iter().map(|s| s.to_string()).collect() }
}

#[test]
fn r8_hunks_are_the_base_ranges_each_side_replaced() {
    assert_eq!(hunks(&["a", "b", "c"], &["a", "x", "c"]), vec![hunk(1, 2, &["x"])]);
    assert_eq!(hunks(&["a", "b"], &["a", "n", "b"]), vec![hunk(1, 1, &["n"])]);
    assert_eq!(hunks(&["a", "b"], &["b"]), vec![hunk(0, 1, &[])]);
}

#[test]
fn r8_one_side_changed_take_it() {
    let base = lines(&BASE);
    let x = lines(&["a", "x", "c", "d", "e"]);
    assert_eq!(merge3(&base, &base, &x, false), Merged { text: x.clone(), conflicts: vec![] });
    assert_eq!(merge3(&base, &x, &base, false), Merged { text: x, conflicts: vec![] });
}

#[test]
fn r8_changes_to_different_lines_both_apply_adjacent_ones_too() {
    let base = lines(&BASE);
    let m = |a: &[&str], b: &[&str]| merge3(&base, &lines(a), &lines(b), false).text;
    assert_eq!(m(&["a", "B", "c", "d", "e"], &["a", "b", "c", "D", "e"]), lines(&["a", "B", "c", "D", "e"]));
    assert_eq!(m(&["a", "B", "c", "d", "e"], &["a", "b", "C", "d", "e"]), lines(&["a", "B", "C", "d", "e"]));
    assert_eq!(m(&["a", "b", "c", "d", "e", "f"], &["z", "a", "b", "c", "d", "e"]), lines(&["z", "a", "b", "c", "d", "e", "f"]));
    assert_eq!(m(&["a", "c", "d", "e"], &["a", "b", "c", "e"]), lines(&["a", "c", "e"]));
}

#[test]
fn r8_the_same_change_on_both_sides_applies_once() {
    let base = lines(&BASE);
    let x = lines(&["a", "x", "c", "d", "e"]);
    assert_eq!(merge3(&base, &x, &x, false).text, x);
    let xd = lines(&["a", "x", "c", "D", "e"]);
    assert_eq!(merge3(&base, &xd, &x, false), Merged { text: xd, conflicts: vec![] });
}

#[test]
fn r8_different_changes_to_the_same_line_clash_with_both_sides_shown() {
    let base = lines(&BASE);
    let r = merge3(&base, &lines(&["a", "x", "c", "d", "e"]), &lines(&["a", "y", "c", "d", "e"]), false);
    assert_eq!(
        r,
        Merged { text: String::new(), conflicts: vec![Conflict { base: [2, 2], ours: vec!["x".into()], theirs: vec!["y".into()] }] }
    );
    // One side deletes a line the other changes.
    assert_eq!(merge3(&base, &lines(&["a", "c", "d", "e"]), &lines(&["a", "y", "c", "d", "e"]), false).conflicts.len(), 1);
}

#[test]
fn r8_insertions_at_the_same_point_clash_unless_union_keeps_both_ours_first() {
    let base = lines(&BASE);
    let ours = lines(&["a", "b", "c", "d", "e", "mine"]);
    let theirs = lines(&["a", "b", "c", "d", "e", "yours"]);
    assert_eq!(merge3(&base, &ours, &theirs, false).conflicts.len(), 1);
    assert_eq!(merge3(&base, &ours, &theirs, true), Merged { text: lines(&["a", "b", "c", "d", "e", "mine", "yours"]), conflicts: vec![] });
    // union never joins a replacement with an insertion.
    assert_eq!(
        merge3(&base, &lines(&["a", "B", "c", "d", "e"]), &lines(&["a", "n", "b", "c", "d", "e"]), true).text,
        lines(&["a", "n", "B", "c", "d", "e"])
    );
    assert_eq!(merge3(&base, &lines(&["a", "B", "c", "d", "e"]), &lines(&["a", "y", "c", "d", "e"]), true).conflicts.len(), 1);
}

#[test]
fn r1_fold_scene_folds_both_sides_first_layers_into_one_list_ours_first_it_refuses_an_id_or_another_key_both_added() {
    let scene = lines(&[
        "{",
        "  \"viewBox\": [0, 0, 16, 16],",
        "  \"layers\": [",
        "    { \"id\": \"group-1\", \"name\": \"Group 1\" },",
        "  ],",
        "  \"layers\": [",
        "    { \"id\": \"sky\", \"name\": \"Sky\" },",
        "  ],",
        "  \"items\": [],",
        "}",
    ]);
    let folded = fold_scene(&scene);
    assert_eq!(
        folded.text,
        lines(&[
            "{",
            "  \"viewBox\": [0, 0, 16, 16],",
            "  \"layers\": [",
            "    { \"id\": \"group-1\", \"name\": \"Group 1\" },",
            "    { \"id\": \"sky\", \"name\": \"Sky\" },",
            "  ],",
            "  \"items\": [],",
            "}",
        ])
    );
    assert!(folded.clashes.is_empty());
    assert_eq!(fold_scene(&scene.replace("\"sky\", \"name\": \"Sky\"", "\"group-1\", \"name\": \"Rocks\"")).clashes, ["layer group-1"]);
    let motions = lines(&[
        "{",
        "  \"items\": [],",
        "  \"motion\": {",
        "    \"duration\": 100,",
        "  },",
        "  \"motion\": {",
        "    \"duration\": 200,",
        "  },",
        "}",
    ]);
    assert_eq!(fold_scene(&motions).clashes, ["motion"]);
    let one = lines(&["{", "  \"layers\": [", "    { \"id\": \"a\", \"name\": \"A\" },", "  ],", "  \"items\": [],", "}"]);
    assert_eq!(fold_scene(&one).text, one);
    assert!(fold_scene(&one).clashes.is_empty());
}

/// A scene's canonical text with these item lines.
fn scene(items: &[&str]) -> String {
    let body: Vec<String> = items.iter().map(|i| format!("    {i}")).collect();
    format!("{{\n  \"viewBox\": [0, 0, 16, 16],\n  \"items\": [\n{}\n  ]\n}}\n", body.join(",\n"))
}

#[test]
fn trailing_commas_round_trip_the_canonical_text() {
    let text = scene(&["{ \"object\": \"a\", \"at\": { \"px\": [1, 2] } }", "{ \"object\": \"b\", \"at\": { \"px\": [3, 4] } }"]);
    let t = trailing(&text);
    assert!(t.contains("\"b\", \"at\": { \"px\": [3, 4] } },\n  ],\n}"), "{t}");
    assert_eq!(untrailing(&t), text);
    for file in [
        "fixtures/sample.pixellab/scenes/sample.json",
        "fixtures/sample.pixellab/objects/pebble.json",
        "fixtures/sample.pixellab/palette.json",
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(file);
        let text = std::fs::read_to_string(path).unwrap();
        assert_eq!(untrailing(&trailing(&text)), text, "{file}");
    }
}

#[test]
fn items_added_at_the_end_by_both_sides_merge_in_trailing_form() {
    let a = "{ \"object\": \"a\", \"at\": { \"px\": [1, 2] } }";
    let base = scene(&[a]);
    let ours = scene(&[a, "{ \"object\": \"mine\", \"at\": { \"px\": [5, 5] } }"]);
    let theirs = scene(&[a, "{ \"object\": \"yours\", \"at\": { \"px\": [6, 6] } }"]);
    // As JSON, both sides changed the line before their new item.
    assert_eq!(merge3(&base, &ours, &theirs, true).conflicts.len(), 1);
    let merged = merge3(&trailing(&base), &trailing(&ours), &trailing(&theirs), true);
    assert!(merged.conflicts.is_empty());
    assert_eq!(
        untrailing(&merged.text),
        scene(&[a, "{ \"object\": \"mine\", \"at\": { \"px\": [5, 5] } }", "{ \"object\": \"yours\", \"at\": { \"px\": [6, 6] } }"])
    );
}

#[test]
fn a_unified_diff_has_hunks_with_context() {
    assert_eq!(unified_diff("a\nb\n", "a\nb\n", "a/x", "b/x", 3), "");
    let before: String = (1..=12).map(|n| format!("{n}\n")).collect();
    let after = before.replacen("2\n", "two\n", 1).replacen("11\n", "eleven\n", 1);
    assert_eq!(
        unified_diff(&before, &after, "a/f", "b/f", 3),
        "--- a/f\n+++ b/f\n@@ -1,5 +1,5 @@\n 1\n-2\n+two\n 3\n 4\n 5\n@@ -8,5 +8,5 @@\n 8\n 9\n 10\n-11\n+eleven\n 12\n"
    );
    assert_eq!(unified_diff("", "a\n", "a/n", "b/n", 3), "--- a/n\n+++ b/n\n@@ -0,0 +1,1 @@\n+a\n");
}
