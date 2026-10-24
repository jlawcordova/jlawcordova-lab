//! Category commands (docs/intents/2026-10-object-libraries/spec.md, R9, D2):
//! `object.category`, and the `category` filter and field of `object.list`
//! and `object.show`, on temp copies of the sample document.

#![allow(clippy::result_large_err)]

mod common;

use std::collections::HashMap;

use pixellab::cli::{main, Io};
use pixellab::context::Ctx;
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::Error;
use pixellab::registry::registry;
use pixellab::session::{Options, Session};
use serde_json::{json, Value};

use common::{make_document, temp_dir, TempDocument};

fn session(doc: &TempDocument) -> Session {
    let mut env = doc.env();
    env.insert("PIXELLAB_STATE".into(), temp_dir("pixellab-state-").to_string_lossy().into_owned());
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    Session::new(document, Options { env, ..Options::default() })
}

fn call(s: &mut Session, name: &str, params: Value) -> Result<Value, Error> {
    let mut ctx = Ctx::new(s);
    registry().call(&mut ctx, name, params, false)
}

fn cli(doc: &TempDocument, args: &[&str]) -> (i32, String, String) {
    let reg = registry();
    let mut env = doc.env();
    env.insert("PIXELLAB_STATE".into(), doc.base.join("state").to_string_lossy().into_owned());
    let mut argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    argv.push("--document".into());
    argv.push(doc.root.to_string_lossy().into_owned());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = {
        let mut io = Io { stdout: &mut out, stderr: &mut err, env: env as HashMap<String, String>, cwd: doc.root.clone(), registry: &reg };
        main(&argv, &mut io)
    };
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn file(doc: &TempDocument, name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(doc.root.join(name)).unwrap()).unwrap()
}

fn code_of(e: &Error) -> String {
    match e {
        Error::Command(c) => c.code.clone(),
        Error::Internal(_) => "internal".into(),
    }
}

fn names(r: &Value) -> Vec<String> {
    r["objects"].as_array().unwrap().iter().map(|o| o["name"].as_str().unwrap().to_string()).collect()
}

#[test]
fn r9_object_category_sets_and_clears_one_objects_category_in_one_edit() {
    let doc = make_document();
    let mut s = session(&doc);
    let r = call(&mut s, "object.category", json!({ "names": ["pebble"], "category": ["Nature", "Stones"] })).unwrap();
    assert_eq!(r["changed"], true);
    assert_eq!(r["category"], json!(["Nature", "Stones"]));
    assert_eq!(r["summary"], "Set pebble's category to Nature › Stones");
    assert_eq!(s.entries.get("object:pebble").unwrap().undo.len(), 1);
    let shown = call(&mut s, "object.show", json!({ "name": "pebble", "doc": true })).unwrap();
    assert_eq!(shown["category"], json!(["Nature", "Stones"]));
    assert_eq!(shown["doc"]["category"], json!(["Nature", "Stones"]));
    // Setting it again changes nothing; clearing removes the field, in one more step.
    assert_eq!(
        call(&mut s, "object.category", json!({ "names": ["pebble"], "category": ["Nature", "Stones"] })).unwrap()["changed"],
        false
    );
    let r = call(&mut s, "object.category", json!({ "names": ["pebble"], "category": null })).unwrap();
    assert_eq!(
        (r["changed"].clone(), r["category"].clone(), r["summary"].clone()),
        (json!(true), Value::Null, json!("Cleared pebble's category"))
    );
    let shown = call(&mut s, "object.show", json!({ "name": "pebble", "doc": true })).unwrap();
    assert!(shown.get("category").is_none() && shown["doc"].get("category").is_none());
    assert_eq!(s.entries.get("object:pebble").unwrap().undo.len(), 2);
    // Undo takes the clear back.
    call(&mut s, "doc.undo", json!({ "doc": "object:pebble" })).unwrap();
    let shown = call(&mut s, "object.show", json!({ "name": "pebble" })).unwrap();
    assert_eq!(shown["category"], json!(["Nature", "Stones"]));
}

#[test]
fn r9_the_cli_form_takes_names_and_a_path_with_either_separator_and_saves() {
    let doc = make_document();
    let (code, out, err) = cli(&doc, &["object", "category", "pebble", "block", "--to", "Nature / Stones"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("Set the category of 2 objects (pebble, block) to Nature › Stones"), "{out}");
    for object in ["pebble", "block"] {
        assert_eq!(file(&doc, &format!("objects/{object}.json"))["category"], json!(["Nature", "Stones"]));
    }
    let (code, _, err) = cli(&doc, &["object", "category", "pebble", "--to", "Terrain › Rocks"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(file(&doc, "objects/pebble.json")["category"], json!(["Terrain", "Rocks"]));
    let (code, _, err) = cli(&doc, &["object", "category", "pebble", "--clear"]);
    assert_eq!(code, 0, "{err}");
    assert!(file(&doc, "objects/pebble.json").get("category").is_none());
    // The key sits after notes and before kind in the saved file.
    let text = std::fs::read_to_string(doc.root.join("objects/block.json")).unwrap();
    assert!(text.find("\"category\"").unwrap() < text.find("\"kind\"").unwrap(), "{text}");
}

#[test]
fn r9_a_refusal_changes_nothing() {
    let doc = make_document();
    let mut s = session(&doc);
    for (params, code) in [
        (json!({ "names": ["pebble"], "category": ["All"] }), "usage"),
        (json!({ "names": ["pebble"], "category": ["Nature", "Trees!"] }), "usage"),
        (json!({ "names": ["pebble"], "category": [] }), "usage"),
        (json!({ "names": ["pebble", "no-such"], "category": ["Nature"] }), "not_found"),
        (json!({ "names": ["pebble"] }), "usage"),
        (json!({ "names": ["pebble"], "category": ["A"], "clear": true }), "usage"),
    ] {
        let e = call(&mut s, "object.category", params.clone()).expect_err("refused");
        assert_eq!(code_of(&e), code, "{params}: {}", e.message());
    }
    assert!(s.entries.values().all(|e| e.undo.is_empty()), "nothing was edited");
    let e = call(&mut s, "object.category", json!({ "names": ["pebble"], "category": ["Others"] })).expect_err("refused");
    assert!(e.message().contains("tab of the Library"), "{}", e.message());
}

#[test]
fn r9_object_list_filters_to_a_category_and_everything_below_it_without_regard_to_case() {
    let doc = make_document();
    let mut s = session(&doc);
    call(&mut s, "object.category", json!({ "names": ["pebble"], "category": ["Nature", "Stones"] })).unwrap();
    call(&mut s, "object.category", json!({ "names": ["block"], "category": ["Nature"] })).unwrap();
    let all = call(&mut s, "object.list", json!({})).unwrap();
    let row = |name: &str| all["objects"].as_array().unwrap().iter().find(|o| o["name"] == name).unwrap().clone();
    assert_eq!(row("pebble")["category"], json!(["Nature", "Stones"]));
    assert_eq!(row("block")["category"], json!(["Nature"]));
    let mut under = |c: Value| names(&call(&mut s, "object.list", json!({ "category": c })).unwrap());
    assert_eq!(under(json!(["nature"])), ["block", "pebble"]);
    assert_eq!(under(json!(["NATURE", "stones"])), ["pebble"]);
    assert_eq!(under(json!(["Nature/Stones"])), ["pebble"]);
    assert_eq!(under(json!(["Stones"])), Vec::<String>::new(), "a path starts at the outermost name");
    let both = call(&mut s, "object.list", json!({ "category": ["Nature"], "kind": "block" })).unwrap();
    assert_eq!(names(&both), ["block"]);
    // An uncategorized object has no category field, and the CLI prints it.
    call(&mut s, "object.category", json!({ "names": ["block"], "clear": true })).unwrap();
    assert!(call(&mut s, "object.list", json!({})).unwrap()["objects"]
        .as_array()
        .unwrap()
        .iter()
        .any(|o| o["name"] == "block" && o.get("category").is_none()));
    let (code, out, err) = cli(&doc, &["object", "category", "pebble", "--to", "Nature/Stones"]);
    assert_eq!(code, 0, "{err}{out}");
    let (_, out, _) = cli(&doc, &["object", "list", "--category", "nature"]);
    assert!(out.lines().count() == 1 && out.contains("[Nature › Stones]"), "{out}");
}
