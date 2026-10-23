//! The session (U14): working copies with undo and redo, saving, versions,
//! reloading, merging and settling, called directly. The command-level
//! suites (sync.test.mjs and the edit commands' tests) run once the
//! commands are ported.

mod common;

use pixellab::author::Author;
use pixellab::context::{Ctx, DocKind};
use pixellab::document::{open_document, OpenOptions};
use pixellab::session::{hash_text, Edit, Options, Session};
use serde_json::{json, Map, Value};

fn session(doc: &common::TempDocument, state: &std::path::Path) -> Session {
    let mut env = doc.env();
    env.insert("PIXELLAB_STATE".into(), state.to_string_lossy().into_owned());
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    Session::new(document, Options { env, agent: Some("test".into()), ..Options::default() })
}

/// Adds an item to the sample scene, as scene.place would.
fn place(s: &mut Session, col: i64) -> Value {
    let mut ctx = Ctx::new(s);
    ctx.edit(DocKind::Scene, "sample", None, |doc, _| {
        doc["items"].as_array_mut().unwrap().push(json!({ "object": "pebble", "at": { "tile": [col, 2, 0] } }));
        let n = doc["items"].as_array().unwrap().len() - 1;
        Ok(Edit::new(Map::from_iter([("path".to_string(), Value::String(n.to_string()))]), format!("pebble placed at column {col}")))
    })
    .unwrap()
}

fn read(doc: &common::TempDocument, file: &str) -> String {
    std::fs::read_to_string(doc.root.join(file)).unwrap()
}

#[test]
fn an_edit_is_one_undo_step_and_undo_and_redo_swap_it() {
    let doc = common::make_document();
    let state = common::temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    let r = place(&mut s, 1);
    assert_eq!(r["doc"], "scene:sample");
    assert_eq!(r["changed"], true);
    assert_eq!(r["summary"], "pebble placed at column 1");
    assert!(s.is_dirty("scene:sample"));
    let u = s.step("scene:sample", true).unwrap();
    assert_eq!(u["summary"], "Undid: pebble placed at column 1");
    assert_eq!(u["dirty"], false);
    let r = s.step("scene:sample", false).unwrap();
    assert_eq!(r["summary"], "Redid: pebble placed at column 1");
    assert_eq!(s.step("scene:sample", false).unwrap()["summary"], "Nothing to redo");
}

#[test]
fn an_edit_that_changes_nothing_is_no_step_and_says_so() {
    let doc = common::make_document();
    let state = common::temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    let r = Ctx::new(&mut s).edit(DocKind::Scene, "sample", None, |_, _| Ok(Edit::new(Map::new(), "moved nothing"))).unwrap();
    assert_eq!(r["changed"], false);
    assert_eq!(r["summary"], "nothing changed (moved nothing)");
    assert_eq!(s.status("scene:sample")["canUndo"], false);
}

#[test]
fn a_failed_edit_leaves_the_doc_as_it_was() {
    let doc = common::make_document();
    let state = common::temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    let before = s.doc(DocKind::Scene, "sample").unwrap();
    let err = Ctx::new(&mut s).edit(DocKind::Scene, "sample", None, |doc, _| {
        doc["items"] = json!([]);
        Err(pixellab::errors::CommandError::new("no", "nope").into())
    });
    assert!(err.is_err());
    assert_eq!(s.entries.get("scene:sample").unwrap().doc, before);
}

#[test]
fn rapid_repeats_with_one_key_coalesce() {
    let doc = common::make_document();
    let state = common::temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    for n in 0..3 {
        Ctx::new(&mut s)
            .edit(DocKind::Object, "pebble", Some("paint"), move |doc, _| {
                let row = doc["layers"][0]["map"][0].as_str().unwrap().to_string();
                let mut chars: Vec<char> = row.chars().collect();
                chars[n] = 'c';
                doc["layers"][0]["map"][0] = Value::String(chars.into_iter().collect());
                Ok(Edit::new(Map::new(), format!("painted {n}")))
            })
            .unwrap();
    }
    let d = s.details("object:pebble");
    assert_eq!(d["undo"], json!(["painted 0", "painted 1", "painted 2"]));
    assert_eq!(s.entries.get("object:pebble").unwrap().undo.len(), 1);
}

#[test]
fn save_writes_the_canonical_text_and_refuses_problems_and_conflicts() {
    let doc = common::make_document();
    let state = common::temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    place(&mut s, 3);
    let saved = s.save("scene:sample", false).unwrap();
    assert_eq!(saved["wrote"], true);
    let text = read(&doc, "scenes/sample.json");
    assert_eq!(saved["hash"], hash_text(&text));
    assert!(text.contains("{ \"object\": \"pebble\", \"at\": { \"tile\": [3, 2, 0] } }"), "{text}");
    assert_eq!(engine::format::serialize(&engine::format::parse(&text).unwrap()), text);
    // A problem refuses the save.
    Ctx::new(&mut s)
        .edit(DocKind::Scene, "sample", None, |doc, _| {
            doc["items"].as_array_mut().unwrap().push(json!({ "object": "nope", "at": { "tile": [9, 9, 0] } }));
            Ok(Edit::new(Map::new(), "added a missing object"))
        })
        .unwrap();
    let err = s.save("scene:sample", false).unwrap_err();
    assert!(err.message().starts_with("scenes/sample.json has 1 problem; nothing written."), "{err}");
    s.step("scene:sample", true).unwrap();
    // A change on disk is a conflict.
    place(&mut s, 5);
    doc.write("scenes/sample.json", &text.replace("[3, 2, 0]", "[4, 2, 0]"));
    let err = s.save("scene:sample", false).unwrap_err();
    assert_eq!(err.message(), "scenes/sample.json changed on disk since it was opened; nothing written. Revert to load it again, or force the save to overwrite it.");
    assert_eq!(s.save("scene:sample", true).unwrap()["wrote"], true);
}

#[test]
fn r16_an_agents_save_writes_the_document_and_its_version_lists_the_edits() {
    let doc = common::make_document();
    let state = common::temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    s.author = Author::agent(Some("test".into()), None);
    let original = read(&doc, "scenes/sample.json");
    place(&mut s, 1);
    let r = s.save("scene:sample", false).unwrap();
    assert_eq!(r["wrote"], true);
    assert_ne!(read(&doc, "scenes/sample.json"), original);
    place(&mut s, 2);
    s.save("scene:sample", false).unwrap();
    let v = s.history.versions().last().cloned().unwrap();
    assert_eq!(v["author"], json!({ "kind": "agent", "name": "test" }));
    assert_eq!(v["edits"], json!(["pebble placed at column 1", "pebble placed at column 2"]));
}

#[test]
fn sync_reloads_a_clean_doc_whose_file_changed_and_marks_a_matching_dirty_one_clean() {
    let doc = common::make_document();
    let state = common::temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    s.open(pixellab::context::DocKind::Scene, "sample").unwrap();
    // Another writer adds a pebble: the clean doc follows the file.
    let mut other = session(&doc, &state);
    place(&mut other, 4);
    other.save("scene:sample", false).unwrap();
    s.sync();
    assert_eq!(s.entries.get("scene:sample").unwrap().doc["items"].as_array().unwrap().len(), 3);
    assert_eq!(s.details("scene:sample")["dirty"], false);
    // The same edit made here and on disk: the doc is clean.
    place(&mut s, 5);
    place(&mut other, 5);
    other.save("scene:sample", false).unwrap();
    s.sync();
    assert_eq!(s.details("scene:sample")["dirty"], false);
}

#[test]
fn reload_refuses_to_drop_edits_and_revert_drops_them_as_a_step() {
    let doc = common::make_document();
    let state = common::temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    place(&mut s, 1);
    let err = s.reload("scene:sample").unwrap_err();
    assert!(err.message().starts_with("scene:sample has 1 unsaved edit the file doesn't have; reloading would drop it."), "{err}");
    let r = s.revert("scene:sample").unwrap();
    assert_eq!(r["summary"], "Reverted scene:sample to the file on disk");
    assert_eq!(r["changed"], true);
    assert_eq!(r["dirty"], false);
    assert_eq!(r["canUndo"], true);
    assert_eq!(s.reload("scene:sample").unwrap()["summary"], "scene:sample already matches scenes/sample.json");
}

#[test]
fn merge_brings_a_change_on_disk_into_a_dirty_copy() {
    let doc = common::make_document();
    let state = common::temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    let original = read(&doc, "scenes/sample.json");
    // The working copy adds an item; the file moves another.
    place(&mut s, 3);
    doc.write("scenes/sample.json", &original.replace("[0, 0, 1]", "[1, 0, 1]"));
    let r = s.merge("scene:sample").unwrap();
    assert_eq!(r["merged"], true);
    assert_eq!(r["summary"], "Merged the changes to scenes/sample.json into scene:sample");
    let items = s.entries.get("scene:sample").unwrap().doc["items"].clone();
    let text = engine::format::stringify(&items);
    assert!(text.contains("[1,0,1]") && text.contains("[3,2,0]"), "{text}");
    assert_eq!(r["dirty"], true);
    // Both sides changing one line is a conflict.
    let mut s = session(&doc, &state);
    Ctx::new(&mut s)
        .edit(DocKind::Scene, "sample", None, |doc, _| {
            doc["viewBox"] = json!([0, 0, 10, 10]);
            Ok(Edit::new(Map::new(), "viewBox"))
        })
        .unwrap();
    let now = read(&doc, "scenes/sample.json");
    doc.write("scenes/sample.json", &now.replace("[-16, -16, 64, 48]", "[0, 0, 20, 20]"));
    let err = s.merge("scene:sample").unwrap_err();
    assert!(err.message().starts_with("Both the open copy and scenes/sample.json changed the same line (1 place)"), "{err}");
}

#[test]
fn a_scene_edit_ends_in_depth_order_and_its_paths_follow_the_sort() {
    let doc = common::make_document();
    let state = common::temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    // A pebble listed last that draws before the pebble on the block: the sort moves it, and says where things went.
    let r = Ctx::new(&mut s)
        .edit(DocKind::Scene, "sample", None, |doc, _| {
            doc["items"].as_array_mut().unwrap().push(json!({ "object": "pebble", "at": { "tile": [-1, -1, 0] } }));
            Ok(Edit::later(Map::from_iter([("path".to_string(), json!("2"))]), |r| format!("placed at {}", r["path"].as_str().unwrap())))
        })
        .unwrap();
    // As the Node session gives it.
    assert_eq!(
        engine::format::stringify(&r),
        r#"{"doc":"scene:sample","changed":true,"summary":"placed at 1","path":"1","reordered":{"1":"2","2":"1"}}"#
    );
    // Undo gives back the paths too.
    let u = s.step("scene:sample", true).unwrap();
    assert_eq!(
        engine::format::stringify(&u),
        r#"{"doc":"scene:sample","changed":true,"summary":"Undid: placed at 1","dirty":false,"canUndo":false,"canRedo":true,"reordered":{"2":"1"}}"#
    );
}

#[test]
fn find_names_objects_and_scenes_and_says_what_is_missing() {
    let doc = common::make_document();
    let state = common::temp_dir("pixellab-state-");
    let mut s = session(&doc, &state);
    let mut ctx = Ctx::new(&mut s);
    assert_eq!(ctx.find("pebble", None).unwrap().file, "objects/pebble.json");
    assert_eq!(ctx.find("scenes/sample.json", None).unwrap().name, "sample");
    assert_eq!(ctx.find("nope", None).unwrap_err().message(), "no object or scene named nope");
    assert_eq!(ctx.find("objects/sample", Some(DocKind::Scene)).unwrap_err().message(), "objects/sample is not a scene");
    assert_eq!(ctx.doc(DocKind::Scene, "nope").unwrap_err().message(), "no scene named nope");
}
