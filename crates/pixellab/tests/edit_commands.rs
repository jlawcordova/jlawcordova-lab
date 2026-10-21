//! The doc, proposal and check parts of cli/test/edit-commands.test.mjs:
//! sessions (undo, redo, revert, saves, conflicts) and proposals. The Node
//! tests edit with scene.place / scene.move / object.paint; those commands
//! belong to other units, so the edits here are made with `ctx.edit` (as the
//! session suite does), and the tests that need the commands are ignored.
#![allow(clippy::result_large_err)]

mod bootstrap;
mod common;

use std::cell::Cell;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

use common::{make_document, TempDocument};
use pixellab::context::{Ctx, DocKind};
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::{Error, Result};
use pixellab::registry::registry;
use pixellab::session::{Edit, Mode, Options, Session};
use serde_json::{json, Map, Value};

fn session(doc: &TempDocument, mode: Mode) -> Session {
    let mut env = doc.env();
    env.insert("PIXELLAB_STATE".into(), doc.base.join("state").to_string_lossy().into_owned());
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    Session::new(document, Options { mode, env, agent: Some("test-agent 1.0".into()), ..Options::default() })
}

fn call(s: &mut Session, name: &str, params: Value) -> Result<Value> {
    registry().call(&mut Ctx::new(s), name, params, false)
}

fn code(e: Error) -> String {
    match e {
        Error::Command(c) => c.code,
        other => panic!("{other}"),
    }
}

/// Places a pebble in the sample scene, as scene.place would.
fn place(s: &mut Session, col: i64) -> Value {
    Ctx::new(s)
        .edit(DocKind::Scene, "sample", None, |doc, _| {
            doc["items"].as_array_mut().unwrap().push(json!({ "object": "pebble", "at": { "tile": [col, 2, 0] } }));
            Ok(Edit::new(Map::new(), format!("pebble placed at column {col}")))
        })
        .unwrap()
}

#[test]
fn undo_and_redo_with_statuses_and_nothing_written_until_save() {
    let doc = make_document();
    let mut s = session(&doc, Mode::Direct);
    let disk = fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap();
    place(&mut s, 1);
    place(&mut s, 2);
    assert_eq!(fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap(), disk, "nothing written");
    let st = call(&mut s, "doc.status", json!({ "doc": "scene:sample" })).unwrap();
    assert_eq!(
        (st["dirty"].clone(), st["canUndo"].clone(), st["canRedo"].clone(), st["undo"].as_array().unwrap().len()),
        (json!(true), json!(true), json!(false), 2)
    );
    let undo = call(&mut s, "doc.undo", json!({ "doc": "sample" })).unwrap();
    assert!(undo["summary"].as_str().unwrap().starts_with("Undid: pebble placed at column 2"));
    call(&mut s, "doc.undo", json!({ "doc": "sample" })).unwrap();
    let st = call(&mut s, "doc.status", json!({ "doc": "scene:sample" })).unwrap();
    assert_eq!((st["dirty"].clone(), st["canUndo"].clone(), st["canRedo"].clone()), (json!(false), json!(false), json!(true)));
    assert_eq!(call(&mut s, "doc.undo", json!({ "doc": "sample" })).unwrap()["summary"], "Nothing to undo");
    call(&mut s, "doc.redo", json!({ "doc": "sample" })).unwrap();
    call(&mut s, "doc.redo", json!({ "doc": "sample" })).unwrap();
    let saved = call(&mut s, "doc.save", json!({ "doc": "scene:sample" })).unwrap();
    assert_eq!(saved["wrote"], true);
    assert_eq!(saved["dirty"], false);
    let written: Value = serde_json::from_str(&fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap()).unwrap();
    assert_eq!(written["items"].as_array().unwrap().len(), 4);
}

#[test]
fn conflict_a_file_changed_on_disk_since_it_was_opened_is_not_overwritten_unless_forced() {
    let doc = make_document();
    let mut s = session(&doc, Mode::Direct);
    place(&mut s, 1);
    let outside = format!("{}\n", fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap());
    fs::write(doc.root.join("scenes/sample.json"), &outside).unwrap();
    assert_eq!(code(call(&mut s, "doc.save", json!({ "doc": "scene:sample" })).unwrap_err()), "conflict");
    assert_eq!(fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap(), outside);
    assert_eq!(call(&mut s, "doc.status", json!({ "doc": "sample" })).unwrap()["stale"], true);
    let forced = call(&mut s, "doc.save", json!({ "doc": "scene:sample", "force": true })).unwrap();
    assert_eq!(forced["wrote"], true);
}

#[test]
fn revert_loads_the_file_again_as_an_undo_step_and_close_refuses_unsaved_changes() {
    let doc = make_document();
    let mut s = session(&doc, Mode::Direct);
    place(&mut s, 1);
    let r = call(&mut s, "doc.revert", json!({ "doc": "sample" })).unwrap();
    assert_eq!((r["dirty"].clone(), r["canUndo"].clone()), (json!(false), json!(true)));
    place(&mut s, 2);
    assert_eq!(code(call(&mut s, "doc.close", json!({ "doc": "sample" })).unwrap_err()), "dirty");
    assert_eq!(call(&mut s, "doc.close", json!({ "doc": "sample", "force": true })).unwrap()["closed"], true);
    assert_eq!(code(call(&mut s, "doc.status", json!({ "doc": "scene:sample" })).unwrap_err()), "not_found");
}

#[test]
fn propose_mode_doc_save_writes_a_proposal_not_the_document_apply_and_reject() {
    let doc = make_document();
    let mut agent = session(&doc, Mode::Propose);
    let disk = fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap();
    place(&mut agent, 3);
    let saved = call(&mut agent, "doc.save", json!({ "doc": "sample" })).unwrap();
    assert_eq!(saved["wrote"], json!(false));
    assert_eq!(fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap(), disk, "the document is untouched");
    let proposal: Value = serde_json::from_str(&fs::read_to_string(saved["file"].as_str().unwrap()).unwrap()).unwrap();
    let keys: Vec<&String> = proposal.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["id", "number", "doc", "baseHash", "source", "summary", "createdAt", "agent"]);
    assert_eq!(proposal["agent"], "test-agent 1.0");

    let mut person = session(&doc, Mode::Direct);
    let list = call(&mut person, "proposal.list", json!({})).unwrap();
    let p = &list["proposals"][0];
    assert_eq!(
        (p["id"].clone(), p["baseMatches"].clone(), p["summary"].as_array().unwrap().len()),
        (proposal["id"].clone(), json!(true), 1)
    );
    let shown = call(&mut person, "proposal.show", json!({ "id": proposal["id"] })).unwrap();
    assert!(
        shown["diff"].as_str().unwrap().contains("\"tile\": [3, 2, 0]") || shown["diff"].as_str().unwrap().contains("[3, 2, 0]"),
        "{}",
        shown["diff"]
    );
    call(&mut person, "doc.open", json!({ "doc": "sample" })).unwrap();
    let applied = call(&mut person, "proposal.apply", json!({ "id": proposal["id"] })).unwrap();
    assert_eq!(applied["reloaded"], true);
    assert_eq!(fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap(), proposal["source"].as_str().unwrap());
    assert!(!std::path::Path::new(saved["file"].as_str().unwrap()).exists(), "the proposal is removed");
    assert_eq!(call(&mut person, "proposal.list", json!({})).unwrap()["proposals"], json!([]));

    // A proposal on a base that changed is refused unless forced; reject deletes.
    let mut agent = session(&doc, Mode::Propose);
    place(&mut agent, 4);
    let second = call(&mut agent, "doc.save", json!({ "doc": "sample" })).unwrap();
    let now = fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap();
    fs::write(doc.root.join("scenes/sample.json"), format!("{now}\n")).unwrap();
    assert_eq!(call(&mut person, "proposal.list", json!({})).unwrap()["proposals"][0]["baseMatches"], false);
    assert_eq!(code(call(&mut person, "proposal.apply", json!({ "id": second["proposal"] })).unwrap_err()), "conflict");
    let rejected = call(&mut person, "proposal.reject", json!({ "id": second["proposal"] })).unwrap();
    assert_eq!(rejected["rejected"], true);
    assert_eq!(code(call(&mut person, "proposal.show", json!({ "id": second["proposal"] })).unwrap_err()), "not_found");
    assert_eq!(code(call(&mut person, "proposal.show", json!({ "id": "../../etc/passwd" })).unwrap_err()), "not_found");
}

#[test]
fn proposal_render_the_proposed_doc_over_the_files_on_disk_rendered_without_writing() {
    let doc = make_document();
    let mut agent = session(&doc, Mode::Propose);
    let mut person = session(&doc, Mode::Direct);
    let text = fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap();
    place(&mut agent, -2);
    let scene = call(&mut agent, "doc.save", json!({ "doc": "sample" })).unwrap();
    let r = call(&mut person, "proposal.render", json!({ "id": scene["proposal"], "scale": 2 })).unwrap();
    assert_eq!(
        (r["kind"].clone(), r["doc"].clone(), r["baseMatches"].clone(), r["scale"].clone()),
        (json!("scene"), json!("scene:sample"), json!(true), json!(2))
    );
    assert_eq!(r["items"].as_array().unwrap().len(), 3);
    assert_eq!(fs::read_to_string(doc.root.join("scenes/sample.json")).unwrap(), text, "nothing written");
    assert_eq!(code(call(&mut person, "proposal.render", json!({ "id": "20990101-000000-abcdef" })).unwrap_err()), "not_found");
    call(&mut person, "proposal.reject", json!({ "id": scene["proposal"] })).unwrap();
}

#[test]
fn propose_mode_validates_too_a_proposal_with_problems_is_refused_unless_forced() {
    let doc = make_document();
    let mut s = session(&doc, Mode::Propose);
    Ctx::new(&mut s)
        .edit(DocKind::Scene, "sample", None, |d, _| {
            d["items"][0]["object"] = json!("no-such-object");
            Ok(Edit::new(Map::new(), "broke it"))
        })
        .unwrap();
    assert_eq!(code(call(&mut s, "doc.save", json!({ "doc": "sample" })).unwrap_err()), "invalid");
}

#[test]
fn rapid_repeats_coalesce_into_one_undo_step_other_edits_do_not() {
    let doc = make_document();
    let clock = Rc::new(Cell::new(1000.0));
    let c = clock.clone();
    let mut env = doc.env();
    env.insert("PIXELLAB_STATE".into(), doc.base.join("state").to_string_lossy().into_owned());
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    let mut s = Session::new(document, Options { mode: Mode::Direct, env, now: Box::new(move || c.get()), ..Options::default() });
    for _ in 0..3 {
        call(&mut s, "scene.move", json!({ "name": "sample", "item": "0", "by": [1, 0] })).unwrap();
        clock.set(clock.get() + 100.0);
    }
    clock.set(clock.get() + 5000.0);
    call(&mut s, "scene.move", json!({ "name": "sample", "item": "0", "by": [1, 0] })).unwrap();
    let st = call(&mut s, "doc.status", json!({ "doc": "sample" })).unwrap();
    assert_eq!(st["undo"].as_array().unwrap().len(), 4, "every edit is listed");
    assert!(call(&mut s, "doc.undo", json!({ "doc": "sample" })).unwrap()["summary"].as_str().unwrap().starts_with("Undid: block moved"));
    let second = call(&mut s, "doc.undo", json!({ "doc": "sample" })).unwrap();
    assert!(second["summary"].as_str().unwrap().ends_with("(3 edits)"), "{}", second["summary"]);
    assert_eq!(second["canUndo"], false);
    assert_eq!(second["dirty"], false);
}

#[test]
fn a_failed_edit_leaves_the_working_copy_as_it_was() {
    let doc = make_document();
    let mut s = session(&doc, Mode::Direct);
    call(&mut s, "doc.open", json!({ "doc": "scene:sample" })).unwrap();
    let before = call(&mut s, "doc.status", json!({ "doc": "sample" })).unwrap();
    let err = call(&mut s, "scene.set", json!({ "name": "sample", "item": "0", "tile": [5, 5, 0], "id": "Bad Id" })).unwrap_err();
    assert!(err.to_string().contains("kebab-case"), "{err}");
    let after = call(&mut s, "doc.status", json!({ "doc": "sample" })).unwrap();
    assert_eq!(after["canUndo"], false);
    assert_eq!(after["dirty"], false);
    assert_eq!(before["undo"], after["undo"]);
}

/// R20: no site checkout. The subject is generic (the engine serializer is the authority for
/// every source), so it runs over the fixture and format-2 copies of the two examples.
#[test]
fn every_scene_and_object_serializes_to_its_file_byte_for_byte_and_saves_unchanged() {
    let base = common::temp_dir("pixellab-roundtrip-");
    let docs = vec![common::fixture(), bootstrap::example(&base, "harbor"), bootstrap::example(&base, "town")];
    let mut n = 0;
    for root in docs {
        let env = HashMap::from([
            ("HOME".to_string(), base.to_string_lossy().into_owned()),
            ("PIXELLAB_STATE".to_string(), base.join("state").to_string_lossy().into_owned()),
        ]);
        let document =
            open_document(&OpenOptions { doc: Some(root.to_string_lossy().into_owned()), site: None, env: env.clone(), cwd: root.clone() })
                .unwrap();
        let mut s = Session::new(document, Options { mode: Mode::Direct, env, ..Options::default() });
        for kind in [DocKind::Scene, DocKind::Object] {
            let names = Ctx::new(&mut s).names(kind).unwrap();
            for name in names {
                let (file, ref_, same) = {
                    let entry = s.open(kind, &name).unwrap();
                    (entry.file.clone(), entry.ref_.clone(), entry.clean == entry.base.text)
                };
                n += 1;
                assert!(same, "{file} differs from serialize() of itself");
                let r = call(&mut s, "doc.save", json!({ "doc": ref_ })).unwrap();
                assert_eq!(r["wrote"], false, "{file} was rewritten");
            }
        }
    }
    assert!(n > 10, "{n} docs");
    let _ = fs::remove_dir_all(base);
}
