//! Object libraries and palette files (docs/intents/2026-10-object-libraries/spec.md,
//! R10–R23): `library.export`, `library.import`, `library.info`, `library.category`,
//! `check --library`, `palette.export` and `palette.import`, on temp copies of the sample
//! document. Nothing here touches the real site or `examples/`.

#![allow(clippy::result_large_err)]

mod common;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use pixellab::cli::{main, Io};
use pixellab::context::Ctx;
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::Error;
use pixellab::registry::registry;
use pixellab::session::{Options, Session};
use serde_json::{json, Value};

use common::{make_document, make_library, temp_dir, TempDocument};

// ------------------------------------------------------------------ helpers

fn session(doc: &TempDocument) -> Session {
    let mut env = doc.env();
    env.insert("PIXELLAB_STATE".into(), doc.base.join("state").to_string_lossy().into_owned());
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

fn ok(s: &mut Session, name: &str, params: Value) -> Value {
    call(s, name, params.clone()).unwrap_or_else(|e| panic!("{name} {params}: {}", e.message()))
}

fn cli_in(doc: Option<&TempDocument>, cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let reg = registry();
    let mut env: HashMap<String, String> = HashMap::new();
    let mut argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    if let Some(doc) = doc {
        env = doc.env();
        env.insert("PIXELLAB_STATE".into(), doc.base.join("state").to_string_lossy().into_owned());
        argv.push("--document".into());
        argv.push(doc.root.to_string_lossy().into_owned());
    }
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = {
        let mut io = Io { stdout: &mut out, stderr: &mut err, env, cwd: cwd.to_path_buf(), registry: &reg };
        main(&argv, &mut io)
    };
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn cli(doc: &TempDocument, args: &[&str]) -> (i32, String, String) {
    cli_in(Some(doc), &doc.root, args)
}

fn code_of(e: &Error) -> String {
    match e {
        Error::Command(c) => c.code.clone(),
        Error::Internal(_) => "internal".into(),
    }
}

fn details_of(e: &Error) -> Value {
    match e {
        Error::Command(c) => c.details.clone().unwrap_or(Value::Null),
        Error::Internal(_) => Value::Null,
    }
}

fn read(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// The top-level keys of a canonical JSON text, in the order written.
fn top_keys(text: &str) -> Vec<String> {
    text.lines().filter(|l| l.starts_with("  \"")).map(|l| l.trim().split('"').nth(1).unwrap().to_string()).collect()
}

fn text(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

/// Every file under `dir`, as relative path → bytes.
fn tree(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().to_string_lossy().into_owned(), fs::read(&path).unwrap());
            }
        }
    }
    let mut out = Default::default();
    walk(dir, dir, &mut out);
    out
}

fn sprite(color: &str) -> Value {
    json!({ "kind": "sprite", "anchor": [0, 0], "keys": { "a": color }, "layers": [{ "map": ["a"] }] })
}

fn extending(base: &str, color: &str) -> Value {
    json!({ "kind": "sprite", "extends": base, "keys": { "a": color } })
}

fn block(top: &str, left: &str, right: &str) -> Value {
    json!({ "kind": "block", "size": [1, 1, 1], "faces": { "top": top, "left": left, "right": right } })
}

fn categorized(mut object: Value, category: Value) -> Value {
    object["category"] = category;
    object
}

/// The sample document with these objects added (as files).
fn doc_with(objects: &[(&str, Value)]) -> TempDocument {
    let doc = make_document();
    for (name, object) in objects {
        doc.write(&format!("objects/{name}.json"), &engine::format::serialize_object(object));
    }
    doc
}

fn lib_dir() -> PathBuf {
    temp_dir("pixellab-libs-")
}

/// The sample palette's colors, as a library records them.
const INK: (&str, &str, &str) = ("ink", "#2E2418", "world");
const GRASS2: (&str, &str, &str) = ("grass-2", "#8FA56E", "world");
const GRASS4: (&str, &str, &str) = ("grass-4", "#4E6B3A", "world");
const SOIL3: (&str, &str, &str) = ("soil-3", "#5A3E2B", "world");
const WOOD3: (&str, &str, &str) = ("wood-3", "#7A4E2D", "world");
const TEAL1: (&str, &str, &str) = ("teal-1", "#7FA89B", "outfit");

fn names(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|n| n.as_str().unwrap().to_string()).collect()
}

fn versions(s: &mut Session) -> usize {
    ok(s, "history.list", json!({}))["total"].as_u64().unwrap() as usize
}

// ------------------------------------------------------------------ R10, R11: export

#[test]
fn r10_a_library_is_a_package_of_objects_with_the_colors_they_used_and_nothing_else() {
    let doc = doc_with(&[("lamp", categorized(sprite("wood-3"), json!(["Props", "Lamps"])))]);
    let mut s = session(&doc);
    let out = lib_dir().join("everything.pixellib");
    let r = ok(&mut s, "library.export", json!({ "to": out.to_str().unwrap() }));
    assert_eq!(r["count"], 3);
    assert_eq!(r["summary"], format!("Exported 3 objects to {}", out.display()));
    let files: Vec<String> = tree(&out).keys().cloned().collect();
    assert_eq!(files, ["library.json", "objects/block.json", "objects/lamp.json", "objects/pebble.json"]);
    let manifest = read(&out.join("library.json"));
    assert_eq!(manifest["format"], 1);
    assert_eq!(manifest["kind"], "library");
    assert_eq!(manifest["name"], "everything");
    assert_eq!(manifest["exportedFrom"], "sample");
    // The record holds the colors the objects use at the document's values, in palette order, and nothing else.
    assert_eq!(
        manifest["colors"],
        json!({ "world": { "grass-2": "#8FA56E", "grass-4": "#4E6B3A", "ink": "#2E2418", "soil-2": "#8A5A34", "soil-3": "#5A3E2B", "wood-3": "#7A4E2D" }, "outfit": {} })
    );
    assert_eq!(read(&out.join("objects/lamp.json"))["category"], json!(["Props", "Lamps"]));
    // No dates and no paths: the same export again gives the same bytes. The file is canonical.
    let again = lib_dir().join("everything.pixellib");
    ok(&mut s, "library.export", json!({ "to": again.to_str().unwrap() }));
    assert_eq!(tree(&out), tree(&again));
    let key_order = top_keys(&text(&out.join("library.json")));
    assert_eq!(key_order, ["format", "kind", "name", "exportedFrom", "colors"]);
    // The package passes its own check, with a document or without one.
    let (code, out_text, err) = cli_in(None, &lib_dir(), &["check", "--library", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}{out_text}");
    assert!(out_text.contains("ok · 3 objects from sample"), "{out_text}");
}

#[test]
fn r11_export_all_a_category_and_everything_below_it_or_a_named_list_with_every_base() {
    let doc = doc_with(&[
        ("character", sprite("grass-2")),
        ("coat", categorized(extending("character", "teal-1"), json!(["People"]))),
        ("cloak", categorized(extending("coat", "teal-1"), json!(["People", "Capes"]))),
        ("lamp", categorized(sprite("wood-3"), json!(["Props"]))),
    ]);
    let mut s = session(&doc);
    let dir = lib_dir();
    let to = |n: &str| dir.join(n).to_str().unwrap().to_string();
    // A category and below it: the cloak is under People, and brings its bases (coat, character).
    let r = ok(&mut s, "library.export", json!({ "to": to("people"), "category": ["people"] }));
    let rows: Vec<(String, bool)> =
        r["objects"].as_array().unwrap().iter().map(|o| (o["name"].as_str().unwrap().to_string(), o["base"] == true)).collect();
    assert_eq!(rows, [("character".to_string(), true), ("cloak".to_string(), false), ("coat".to_string(), false)]);
    assert!(dir.join("people.pixellib/objects/character.json").exists());
    assert!(!dir.join("people.pixellib/objects/lamp.json").exists());
    // A deeper category brings the whole chain of bases.
    let r = ok(&mut s, "library.export", json!({ "to": to("capes.pixellib"), "category": ["People/Capes"] }));
    assert_eq!(r["count"], 3);
    assert_eq!(r["summary"].as_str().unwrap().split(" (").nth(1).unwrap(), "2 added because others extend them)");
    // A named list: the lamp alone has no bases; the coat brings the character.
    assert_eq!(ok(&mut s, "library.export", json!({ "to": to("one"), "objects": ["lamp"] }))["count"], 1);
    let r = ok(&mut s, "library.export", json!({ "to": to("two"), "objects": ["coat", "lamp"] }));
    assert_eq!(r["count"], 3);
    assert_eq!(r["objects"][0], json!({ "name": "character", "kind": "sprite", "category": null, "base": true }));
    // The colors it records are the ones its objects use, outfit colors in the outfit tier.
    let manifest = read(&dir.join("two.pixellib/library.json"));
    assert_eq!(manifest["colors"]["outfit"], json!({ "teal-1": "#7FA89B" }));
    // Exporting doesn't change the document.
    assert!(s.entries.is_empty() || s.dirty_refs().is_empty());
}

#[test]
fn r11_export_refuses_what_it_cannot_do_and_leaves_nothing_half_made() {
    let doc = make_document();
    let mut s = session(&doc);
    let dir = lib_dir();
    let to = dir.join("one.pixellib");
    let t = to.to_str().unwrap();
    ok(&mut s, "library.export", json!({ "to": t, "objects": ["block"] }));
    let before = tree(&to);
    let e = call(&mut s, "library.export", json!({ "to": t })).expect_err("exists");
    assert_eq!(code_of(&e), "exists");
    assert_eq!(tree(&to), before, "the library that was there is untouched");
    // replace swaps it in whole: the old pebble-less package becomes the all-objects one.
    ok(&mut s, "library.export", json!({ "to": t, "replace": true }));
    assert!(to.join("objects/pebble.json").exists());
    for (params, code) in [
        (json!({ "to": t, "replace": true, "objects": ["nope"] }), "not_found"),
        (json!({ "to": t, "replace": true, "category": ["Nothing"] }), "empty"),
        (json!({ "to": t, "replace": true, "category": ["All"] }), "usage"),
        (json!({ "to": t, "replace": true, "objects": ["block"], "category": ["A"] }), "usage"),
        (json!({ "to": doc.root.join("inside").to_str().unwrap() }), "usage"),
    ] {
        let e = call(&mut s, "library.export", params.clone()).expect_err("refused");
        assert_eq!(code_of(&e), code, "{params}: {}", e.message());
    }
    assert!(to.join("objects/pebble.json").exists(), "a refused replace keeps the old library");
    // Nothing else is left beside it: no temp folder.
    let beside: Vec<String> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(beside, ["one.pixellib"]);
}

#[test]
fn r11_exporting_an_object_that_does_not_pass_check_is_refused() {
    let doc = doc_with(&[("bad", sprite("nope-9"))]);
    let mut s = session(&doc);
    let to = lib_dir().join("bad.pixellib");
    let e = call(&mut s, "library.export", json!({ "to": to.to_str().unwrap() })).expect_err("invalid");
    assert_eq!(code_of(&e), "invalid");
    assert!(!to.exists());
}

// ------------------------------------------------------------------ R12, R13: import

#[test]
fn r12_import_copies_objects_with_their_categories_as_one_version_and_nothing_links_back() {
    let doc = make_document();
    let mut s = session(&doc);
    let dir = lib_dir();
    let lib = make_library(
        &dir,
        "yard",
        &[GRASS2, WOOD3, INK],
        vec![
            ("lamp", categorized(sprite("wood-3"), json!(["Props", "Lamps"]))),
            ("bench", categorized(sprite("grass-2"), json!(["Props"]))),
            ("sign", sprite("ink")),
        ],
    );
    let l = lib.to_str().unwrap();
    // An earlier edit, saved, so the history has its first version before the import.
    ok(&mut s, "object.category", json!({ "names": ["block"], "to": "Ground" }));
    ok(&mut s, "doc.save", json!({ "doc": "object:block" }));
    ok(&mut s, "history.mark", json!({}));
    let before = versions(&mut s);
    let plan = ok(&mut s, "library.import", json!({ "from": l, "dryRun": true }));
    assert_eq!((plan["dryRun"].clone(), plan["count"].clone(), plan["ready"].clone()), (json!(true), json!(3), json!(true)));
    assert_eq!(versions(&mut s), before, "a dry run changes nothing");
    assert!(!doc.root.join("objects/lamp.json").exists());

    let r = ok(&mut s, "library.import", json!({ "from": l }));
    assert_eq!(r["changed"], true);
    assert_eq!(r["summary"], "Imported 3 objects from yard.pixellib");
    assert_eq!(names(&r["imported"]), ["bench", "lamp", "sign"]);
    assert_eq!(read(&doc.root.join("objects/lamp.json"))["category"], json!(["Props", "Lamps"]));
    assert!(read(&doc.root.join("objects/sign.json")).get("category").is_none());
    // One version, whose number is in the result; its label is listed once.
    assert_eq!(versions(&mut s), before + 1);
    let n = r["version"].as_u64().unwrap();
    let v = ok(&mut s, "history.show", json!({ "version": n }));
    let edits: Vec<&str> = v["edits"].as_array().unwrap().iter().map(|e| e.as_str().unwrap()).collect();
    assert_eq!(edits, ["Imported 3 objects from yard.pixellib"]);
    let files: Vec<&str> = v["files"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
    assert_eq!(files, ["objects/bench.json", "objects/lamp.json", "objects/sign.json"]);
    // History undo takes the whole import back.
    ok(&mut s, "history.undo", json!({ "version": n }));
    assert!(!doc.root.join("objects/lamp.json").exists() && !doc.root.join("objects/bench.json").exists());
    // The document names nothing of the library.
    let manifest = text(&doc.root.join("manifest.json"));
    assert!(!manifest.contains("yard") && !manifest.contains("pixellib"));
    // Importing the same library again, and again: the objects are the document's now, so there is nothing to copy.
    ok(&mut s, "library.import", json!({ "from": l }));
    let again = ok(&mut s, "library.import", json!({ "from": l }));
    assert_eq!(again["changed"], false);
    assert_eq!(
        again["objects"].as_array().unwrap().iter().map(|o| o["fate"].clone()).collect::<Vec<_>>(),
        vec![json!("same"), json!("same"), json!("same")]
    );
}

#[test]
fn r13_a_name_clash_keeps_both_with_the_first_free_suffix_and_extends_follow_two_levels() {
    // The document has base, mid and base-2; the library's own base-2 keeps its name, so base is base-3.
    let doc = doc_with(&[
        ("base", sprite("grass-2")),
        ("mid", extending("base", "grass-4")),
        ("base-2", sprite("ink")),
        ("keep", categorized(sprite("soil-2"), json!(["Mine"]))),
    ]);
    let mut s = session(&doc);
    let dir = lib_dir();
    let lib = make_library(
        &dir,
        "chain",
        &[GRASS2, GRASS4, SOIL3, WOOD3, INK],
        vec![
            ("base", sprite("wood-3")),
            ("mid", extending("base", "soil-3")),
            ("top", categorized(extending("mid", "wood-3"), json!(["Stack"]))),
            ("base-2", sprite("wood-3")),
        ],
    );
    let own: HashMap<String, Vec<u8>> = tree(&doc.root).into_iter().collect();
    let plan = ok(&mut s, "library.import", json!({ "from": lib.to_str().unwrap(), "dryRun": true }));
    let renames: Vec<(String, String)> = plan["renames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["from"].as_str().unwrap().to_string(), r["to"].as_str().unwrap().to_string()))
        .collect();
    // The library's own base-2 differs from the document's base-2, so it is renamed too (base-2-2).
    assert_eq!(
        renames,
        [
            ("base".to_string(), "base-3".to_string()),
            ("base-2".to_string(), "base-2-2".to_string()),
            ("mid".to_string(), "mid-2".to_string())
        ]
    );
    assert_eq!(plan["count"], 4, "base-3, base-2-2, mid-2 and top");
    ok(&mut s, "library.import", json!({ "from": lib.to_str().unwrap() }));
    let o = |n: &str| read(&doc.root.join(format!("objects/{n}.json")));
    assert_eq!(o("top")["extends"], "mid-2");
    assert_eq!(o("mid-2")["extends"], "base-3");
    assert_eq!(o("top")["category"], json!(["Stack"]));
    assert!(doc.root.join("objects/base-3.json").exists());
    // The document's own objects are never renamed or changed.
    for (file, bytes) in &own {
        if file.starts_with("objects/") {
            assert_eq!(&fs::read(doc.root.join(file)).unwrap(), bytes, "{file}");
        }
    }
    assert_eq!(o("mid")["extends"], "base");
}

#[test]
fn r13_a_second_import_of_a_clashing_library_takes_the_next_suffix() {
    let doc = doc_with(&[("pine", sprite("grass-2"))]);
    let mut s = session(&doc);
    let lib = make_library(&lib_dir(), "pines", &[GRASS4], vec![("pine", sprite("grass-4"))]);
    let l = lib.to_str().unwrap();
    assert_eq!(ok(&mut s, "library.import", json!({ "from": l }))["renames"][0]["to"], "pine-2");
    // Now pine-2 is the library's pine, identical: it is the document's, so it is not copied again.
    let again = ok(&mut s, "library.import", json!({ "from": l, "dryRun": true }));
    assert_eq!(again["renames"].as_array().unwrap().len(), 1, "pine differs from the document's pine, so it is copied as pine-3");
    assert_eq!(again["renames"][0]["to"], "pine-3");
    assert_eq!(read(&doc.root.join("objects/pine.json")), sprite("grass-2"));
}

#[test]
fn r13_an_object_identical_apart_from_its_category_is_not_copied_and_references_use_the_documents() {
    let doc = doc_with(&[("base", sprite("grass-2"))]);
    let mut s = session(&doc);
    let lib = make_library(
        &lib_dir(),
        "shared",
        &[GRASS2, GRASS4],
        vec![("base", categorized(sprite("grass-2"), json!(["Things"]))), ("topper", extending("base", "grass-4"))],
    );
    let l = lib.to_str().unwrap();
    let plan = ok(&mut s, "library.import", json!({ "from": l, "dryRun": true }));
    let fates: Vec<(String, String)> = plan["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| (o["name"].as_str().unwrap().to_string(), o["fate"].as_str().unwrap().to_string()))
        .collect();
    assert_eq!(fates, [("base".to_string(), "same".to_string()), ("topper".to_string(), "new".to_string())]);
    assert_eq!(names(&plan["same"]), ["base"]);
    assert!(plan["renames"].as_array().unwrap().is_empty());
    let before = text(&doc.root.join("objects/base.json"));
    let r = ok(&mut s, "library.import", json!({ "from": l }));
    assert_eq!(r["summary"], "Imported 1 object from shared.pixellib");
    assert_eq!(read(&doc.root.join("objects/topper.json"))["extends"], "base");
    assert_eq!(text(&doc.root.join("objects/base.json")), before, "the document's base keeps no category");
}

#[test]
fn r13_a_library_character_that_differs_is_not_copied_and_counts_as_a_difference() {
    // The site-shaped case: the document's character differs from the library's, and the library's figures extend it.
    let doc = doc_with(&[("character", sprite("grass-2"))]);
    let mut s = session(&doc);
    let lib = make_library(
        &lib_dir(),
        "cast",
        &[GRASS4, TEAL1],
        vec![
            ("character", sprite("grass-4")),
            ("chef", categorized(extending("character", "teal-1"), json!(["People"]))),
            ("waiter", categorized(extending("chef", "teal-1"), json!(["People"]))),
        ],
    );
    let l = lib.to_str().unwrap();
    let plan = ok(&mut s, "library.import", json!({ "from": l, "dryRun": true }));
    assert_eq!(plan["character"], json!({ "swapped": true, "figures": ["chef", "waiter"] }));
    assert_eq!(plan["needsAcceptDifferences"], true);
    assert_eq!(plan["ready"], false);
    let fate = |n: &str| plan["objects"].as_array().unwrap().iter().find(|o| o["name"] == n).unwrap()["fate"].clone();
    assert_eq!((fate("character"), fate("chef")), (json!("character"), json!("new")));
    // Refused without acceptDifferences, with the plan.
    let e = call(&mut s, "library.import", json!({ "from": l })).expect_err("differs");
    assert_eq!(code_of(&e), "differs");
    assert!(e.message().contains("character differs"), "{}", e.message());
    assert_eq!(details_of(&e)["character"]["swapped"], true);
    assert!(!doc.root.join("objects/chef.json").exists());
    // Accepted: the figures extend the document's character, which stays as it was, and no character-2 appears.
    let r = ok(&mut s, "library.import", json!({ "from": l, "acceptDifferences": true }));
    assert_eq!(r["imported"], json!(["chef", "waiter"]));
    assert_eq!(read(&doc.root.join("objects/chef.json"))["extends"], "character");
    assert_eq!(read(&doc.root.join("objects/waiter.json"))["extends"], "chef");
    assert_eq!(read(&doc.root.join("objects/character.json")), sprite("grass-2"));
    assert!(!doc.root.join("objects/character-2.json").exists());
    let check = ok(&mut s, "check", json!({}));
    assert_eq!(check["ok"], true, "{check}");
}

#[test]
fn r13_a_library_character_the_same_or_new_is_no_difference() {
    // Same as the document's: no difference. Imported into a document without one: it keeps its name.
    let doc = doc_with(&[("character", sprite("grass-2"))]);
    let mut s = session(&doc);
    let same = make_library(
        &lib_dir(),
        "same",
        &[GRASS2, TEAL1],
        vec![("character", sprite("grass-2")), ("chef", extending("character", "teal-1"))],
    );
    let plan = ok(&mut s, "library.import", json!({ "from": same.to_str().unwrap(), "dryRun": true }));
    assert_eq!((plan["character"]["swapped"].clone(), plan["ready"].clone()), (json!(false), json!(true)));
    ok(&mut s, "library.import", json!({ "from": same.to_str().unwrap() }));
    assert_eq!(read(&doc.root.join("objects/chef.json"))["extends"], "character");

    let doc = make_document();
    let mut s = session(&doc);
    let r = ok(&mut s, "library.import", json!({ "from": same.to_str().unwrap() }));
    assert_eq!(names(&r["imported"]), ["character", "chef"]);
    assert_eq!(read(&doc.root.join("objects/character.json")), sprite("grass-2"));
}

// ------------------------------------------------------------------ R14, R15: colors

#[test]
fn r14_colors_that_differ_are_a_warning_and_the_command_refuses_unless_told() {
    let doc = make_document();
    let mut s = session(&doc);
    // The library's wood-3 was darker than the document's.
    let lib = make_library(
        &lib_dir(),
        "dark",
        &[("wood-3", "#3A2410", "world"), INK],
        vec![("post", sprite("wood-3")), ("plain", sprite("ink"))],
    );
    let l = lib.to_str().unwrap();
    let plan = ok(&mut s, "library.import", json!({ "from": l, "dryRun": true }));
    assert_eq!(plan["differing"], json!([{ "name": "wood-3", "tier": "world", "library": "#3A2410", "document": "#7A4E2D" }]));
    assert_eq!((plan["needsAcceptDifferences"].clone(), plan["ready"].clone()), (json!(true), json!(false)));
    let e = call(&mut s, "library.import", json!({ "from": l })).expect_err("differs");
    assert_eq!(code_of(&e), "differs");
    assert_eq!(details_of(&e)["differing"][0]["name"], "wood-3");
    assert!(e.message().contains("acceptDifferences"), "{}", e.message());
    assert!(!doc.root.join("objects/post.json").exists() && !doc.root.join("objects/plain.json").exists());
    // Accepted: the objects keep their color names, so they use the document's value.
    ok(&mut s, "library.import", json!({ "from": l, "acceptDifferences": true }));
    assert_eq!(read(&doc.root.join("objects/post.json"))["keys"]["a"], "wood-3");
    // A library whose differing color no imported object uses has nothing to warn about.
    let doc = make_document();
    let mut s = session(&doc);
    let quiet = make_library(&lib_dir(), "quiet", &[("wood-3", "#3A2410", "world"), INK], vec![("plain", sprite("ink"))]);
    let plan = ok(&mut s, "library.import", json!({ "from": quiet.to_str().unwrap(), "dryRun": true }));
    assert_eq!(plan["differing"], json!([]));
}

#[test]
fn r15_missing_colors_are_never_added_and_each_needs_a_conversion_in_its_tier() {
    let doc = make_document();
    let mut s = session(&doc);
    let lib = make_library(
        &lib_dir(),
        "odd",
        &[("rust-9", "#8B5A33", "world"), ("sand-1", "#E0D0A0", "world"), GRASS2, ("plum-1", "#8A6A9A", "outfit"), TEAL1],
        vec![
            ("pole", sprite("rust-9")),
            ("slab", block("sand-1", "rust-9", "grass-2")),
            ("dress", json!({ "kind": "sprite", "extends": "character", "keys": { "a": "plum-1" } })),
            ("character", sprite("grass-2")),
        ],
    );
    let l = lib.to_str().unwrap();
    let plan = ok(&mut s, "library.import", json!({ "from": l, "dryRun": true }));
    let missing = plan["missing"].as_array().unwrap();
    let get = |n: &str| missing.iter().find(|m| m["name"] == n).unwrap().clone();
    assert_eq!(missing.len(), 3);
    // rust-9 (#8B5A33) is closest to soil-2 (#8A5A34) among the document's world colors.
    assert_eq!(get("rust-9")["nearest"], json!({ "name": "soil-2", "hex": "#8A5A34" }));
    assert_eq!(get("rust-9")["tier"], "world");
    assert_eq!(get("rust-9")["usedBy"], json!(["pole", "slab"]));
    assert_eq!(get("plum-1")["tier"], "outfit");
    assert_eq!(get("plum-1")["nearest"]["name"], "teal-1");
    assert_eq!(get("sand-1")["nearest"]["name"], "grass-1");
    // The command refuses with the list and the nearest colors.
    let e = call(&mut s, "library.import", json!({ "from": l })).expect_err("missing");
    assert_eq!(code_of(&e), "missing_colors");
    assert!(e.message().contains("rust-9") && e.message().contains("soil-2"), "{}", e.message());
    assert_eq!(details_of(&e)["missing"].as_array().unwrap().len(), 3);
    // A partial conversion is still a refusal.
    let e = call(&mut s, "library.import", json!({ "from": l, "convert": { "rust-9": "soil-2" } })).expect_err("missing");
    assert_eq!(code_of(&e), "missing_colors");
    // An outfit color converts only to an outfit color, and a world color only to a world one.
    let all = |sand: &str, rust: &str, plum: &str| json!({ "from": l, "convert": { "sand-1": sand, "rust-9": rust, "plum-1": plum } });
    let e = call(&mut s, "library.import", all("cream", "soil-2", "wood-3")).expect_err("wrong tier");
    assert_eq!(code_of(&e), "wrong_tier");
    let e = call(&mut s, "library.import", all("teal-1", "soil-2", "teal-1")).expect_err("wrong tier");
    assert_eq!(code_of(&e), "wrong_tier");
    for convert in [json!({ "nope-1": "soil-2" }), json!({ "grass-2": "soil-2" }), json!({ "rust-9": "no-such-color" })] {
        let e = call(&mut s, "library.import", json!({ "from": l, "convert": convert })).expect_err("usage");
        assert_eq!(code_of(&e), "usage", "{convert}: {}", e.message());
    }
    assert!(!doc.root.join("objects/pole.json").exists(), "refusals changed nothing");
    // The conversion rewrites the imported objects' keys and a block's faces, and nothing else.
    let before = text(&doc.root.join("palette.json"));
    ok(&mut s, "library.import", all("cream", "soil-2", "teal-1"));
    let o = |n: &str| read(&doc.root.join(format!("objects/{n}.json")));
    assert_eq!(o("pole")["keys"]["a"], "soil-2");
    assert_eq!(o("slab")["faces"], json!({ "top": "cream", "left": "soil-2", "right": "grass-2" }));
    assert_eq!(o("dress")["keys"]["a"], "teal-1");
    assert_eq!(text(&doc.root.join("palette.json")), before, "no color is added to the palette");
    assert_eq!(ok(&mut s, "check", json!({}))["ok"], true);
}

// ------------------------------------------------------------------ R16: existing art

#[test]
fn r16_an_import_never_changes_the_palette_the_existing_objects_the_scenes_or_what_publish_writes() {
    let doc = make_document();
    let build = |doc: &TempDocument| {
        let (code, _, err) = cli(doc, &["art", "build"]);
        assert_eq!(code, 0, "{err}");
        tree(&doc.base.join("sample-out"))
    };
    let published = build(&doc);
    assert!(!published.is_empty());
    let own = tree(&doc.root);
    let lib = make_library(
        &lib_dir(),
        "more",
        &[("rust-9", "#8B5A33", "world"), ("wood-3", "#101010", "world"), GRASS2],
        vec![
            ("block", block("grass-2", "rust-9", "wood-3")),
            ("pebble", sprite("rust-9")),
            ("extra", categorized(sprite("wood-3"), json!(["Extras"]))),
        ],
    );
    let (code, out, err) =
        cli(&doc, &["library", "import", lib.to_str().unwrap(), "--accept-differences", "--convert", "{\"rust-9\":\"soil-2\"}"]);
    assert_eq!(code, 0, "{err}{out}");
    let now = tree(&doc.root);
    for (file, bytes) in &own {
        assert_eq!(now.get(file), Some(bytes), "{file} is unchanged");
    }
    let added: Vec<&String> = now.keys().filter(|k| !own.contains_key(*k)).collect();
    assert_eq!(added, ["objects/block-2.json", "objects/extra.json", "objects/pebble-2.json"]);
    assert_eq!(build(&doc), published, "the published SVGs are byte-identical");
}

// ------------------------------------------------------------------ R17: info, category, check

#[test]
fn r17_library_info_lists_objects_categories_and_recorded_colors_and_category_edits_it_in_place() {
    let dir = lib_dir();
    let lib = make_library(
        &dir,
        "kit",
        &[GRASS2, INK],
        vec![
            ("a", categorized(sprite("grass-2"), json!(["Nature", "Trees"]))),
            ("b", categorized(sprite("ink"), json!(["nature", "trees"]))),
            ("c", sprite("ink")),
        ],
    );
    let l = lib.to_str().unwrap();
    let doc = make_document();
    let mut s = session(&doc);
    let info = ok(&mut s, "library.info", json!({ "library": l }));
    assert_eq!(info["name"], "kit");
    assert_eq!(info["ok"], true);
    assert_eq!(info["objects"][0], json!({ "name": "a", "kind": "sprite", "category": ["Nature", "Trees"] }));
    assert_eq!(info["objects"][2]["category"], Value::Null);
    // One category, spelled as its alphabetically first object spells it.
    assert_eq!(info["categories"], json!([{ "category": ["Nature", "Trees"], "text": "Nature › Trees", "objects": 2 }]));
    assert_eq!(info["colors"], json!({ "world": { "grass-2": "#8FA56E", "ink": "#2E2418" }, "outfit": {} }));
    let (code, out, err) = cli_in(None, &dir, &["library", "info", l]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("a  sprite  [Nature › Trees]") && out.contains("c  sprite  [(none)]"), "{out}");

    // library.category sets several objects at once, canonical, and leaves the others' bytes alone.
    let before = tree(&lib);
    let r = ok(&mut s, "library.category", json!({ "library": l, "names": ["c", "b"], "to": "Misc / Bits" }));
    assert_eq!(r["changed"], true);
    assert_eq!(r["category"], json!(["Misc", "Bits"]));
    let after = tree(&lib);
    assert_eq!(after["objects/a.json"], before["objects/a.json"]);
    assert_eq!(read(&lib.join("objects/c.json"))["category"], json!(["Misc", "Bits"]));
    assert_eq!(read(&lib.join("objects/b.json"))["category"], json!(["Misc", "Bits"]));
    assert_eq!(text(&lib.join("objects/c.json")), engine::format::serialize_object(&read(&lib.join("objects/c.json"))));
    assert_eq!(ok(&mut s, "library.category", json!({ "library": l, "names": ["c"], "to": "Misc/Bits" }))["changed"], false);
    ok(&mut s, "library.category", json!({ "library": l, "names": ["c", "b"], "clear": true }));
    assert!(read(&lib.join("objects/c.json")).get("category").is_none());
    // Refusals change nothing.
    let now = tree(&lib);
    for (params, code) in [
        (json!({ "library": l, "names": ["nope"], "to": "X" }), "not_found"),
        (json!({ "library": l, "names": ["a"], "to": "All" }), "usage"),
        (json!({ "library": l, "names": ["a"], "to": "Bad!" }), "usage"),
        (json!({ "library": l, "names": ["a"] }), "usage"),
        (json!({ "library": dir.join("no.pixellib").to_str().unwrap(), "names": ["a"], "to": "X" }), "not_found"),
    ] {
        let e = call(&mut s, "library.category", params.clone()).expect_err("refused");
        assert_eq!(code_of(&e), code, "{params}: {}", e.message());
    }
    assert_eq!(tree(&lib), now);
    // The CLI form.
    let (code, out, err) = cli_in(None, &dir, &["library", "category", l, "a", "--to", "Nature/Plants"]);
    assert_eq!(code, 0, "{err}{out}");
    assert_eq!(read(&lib.join("objects/a.json"))["category"], json!(["Nature", "Plants"]));
}

#[test]
fn r10_a_folder_that_is_not_a_library_and_a_library_that_fails_check_are_told_so() {
    let dir = lib_dir();
    let doc = make_document();
    let mut s = session(&doc);
    // Not a library: a document, a plain folder, a missing path, another format, another kind.
    fs::create_dir_all(dir.join("plain.pixellib")).unwrap();
    let wrong = dir.join("future.pixellib");
    fs::create_dir_all(&wrong).unwrap();
    fs::write(wrong.join("library.json"), "{\"format\": 2, \"kind\": \"library\"}").unwrap();
    let other = dir.join("other.pixellib");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("library.json"), "{\"format\": 1, \"kind\": \"palette\"}").unwrap();
    for (path, code, says) in [
        (doc.root.to_str().unwrap().to_string(), "library", "no library.json"),
        (dir.join("plain.pixellib").to_string_lossy().into_owned(), "library", "no library.json"),
        (wrong.to_string_lossy().into_owned(), "library", "format 2"),
        (other.to_string_lossy().into_owned(), "library", "kind"),
        (dir.join("none.pixellib").to_string_lossy().into_owned(), "not_found", "no library"),
    ] {
        for command in ["library.info", "library.import"] {
            let key = if command == "library.info" { "library" } else { "from" };
            let e = call(&mut s, command, json!({ key: path })).expect_err("refused");
            assert_eq!(code_of(&e), code, "{command} {path}");
            assert!(e.message().contains(says), "{command} {path}: {}", e.message());
        }
    }
    // A library whose object breaks a rule: check --library lists it, import refuses it.
    let lib = make_library(&dir, "broken", &[GRASS2], vec![("ok", sprite("grass-2")), ("bad", sprite("grass-2"))]);
    let mut bad = sprite("grass-2");
    bad["category"] = json!(["All"]);
    bad["extends"] = json!("ghost");
    fs::write(lib.join("objects/bad.json"), engine::format::serialize_object(&bad)).unwrap();
    fs::write(lib.join("objects/worse.json"), "{ not json").unwrap();
    let (code, out, _) = cli_in(None, &dir, &["check", "--library", lib.to_str().unwrap()]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("objects/bad.json: category: 'All' is a tab of the Library"), "{out}");
    assert!(out.contains("objects/bad.json: extends 'ghost'") && out.contains("objects/worse.json: cannot be loaded"), "{out}");
    let e = call(&mut s, "library.import", json!({ "from": lib.to_str().unwrap() })).expect_err("invalid");
    assert_eq!(code_of(&e), "invalid");
    assert!(!doc.root.join("objects/ok.json").exists());
    // An object using a color its library didn't record is a problem of the library too.
    let unrecorded = make_library(&dir, "unrecorded", &[GRASS2], vec![("x", sprite("wood-3"))]);
    let (code, out, _) = cli_in(None, &dir, &["check", "--library", unrecorded.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(out.contains("objects/x.json") && out.contains("wood-3"), "{out}");
}

#[test]
fn r13_an_import_that_would_not_pass_check_in_the_document_is_refused_whole() {
    // An outfit color used by an object that is not a figure: the library's own check can't see the document's character.
    let doc = make_document();
    let mut s = session(&doc);
    let lib =
        make_library(&lib_dir(), "fig", &[GRASS2, TEAL1], vec![("good", sprite("grass-2")), ("figure", extending("character", "teal-1"))]);
    let e = call(&mut s, "library.import", json!({ "from": lib.to_str().unwrap() })).expect_err("invalid");
    assert_eq!(code_of(&e), "invalid");
    assert!(e.message().contains("figure"), "{}", e.message());
    assert!(!doc.root.join("objects/good.json").exists(), "nothing was imported");
}

#[test]
fn r12_the_cli_imports_a_library_and_dry_run_means_the_plan() {
    let doc = make_document();
    let dir = lib_dir();
    let lib = make_library(&dir, "cli", &[WOOD3, SOIL3], vec![("hut", categorized(block("wood-3", "soil-3", "soil-3"), json!(["Homes"])))]);
    let l = lib.to_str().unwrap();
    let (code, out, err) = cli(&doc, &["library", "import", l, "--dry-run"]);
    assert_eq!(code, 0, "{err}{out}");
    assert!(out.starts_with("Would import 1 object from cli; nothing changed"), "{out}");
    assert!(!doc.root.join("objects/hut.json").exists());
    let (code, out, err) = cli(&doc, &["library", "import", l, "--json"]);
    assert_eq!(code, 0, "{err}{out}");
    let r: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(r["summary"], "Imported 1 object from cli.pixellib");
    assert_eq!(read(&doc.root.join("objects/hut.json"))["category"], json!(["Homes"]));
    // A refusal exits 1 and says what to do.
    let doc2 = make_document();
    let odd = make_library(&dir, "odd", &[("rust-9", "#8B5A33", "world")], vec![("pole", sprite("rust-9"))]);
    let (code, _, err) = cli(&doc2, &["library", "import", odd.to_str().unwrap()]);
    assert_ne!(code, 0);
    assert!(err.contains("rust-9") && err.contains("convert"), "{err}");
    // And exports from the CLI.
    let to = dir.join("out.pixellib");
    let (code, out, err) = cli(&doc, &["library", "export", to.to_str().unwrap(), "--category", "Homes"]);
    assert_eq!(code, 0, "{err}{out}");
    assert!(to.join("objects/hut.json").exists() && !to.join("objects/block.json").exists());
}

// ------------------------------------------------------------------ R19, R20: palette export

fn palette_file(dir: &Path, name: &str, world: &[(&str, &str)], outfit: &[(&str, &str)], notes: &[(&str, &str)]) -> PathBuf {
    let tier = |t: &[(&str, &str)]| Value::Object(t.iter().map(|(n, h)| (n.to_string(), json!(h))).collect());
    let mut p = json!({ "format": 1, "kind": "palette", "world": tier(world), "outfit": tier(outfit) });
    if !notes.is_empty() {
        p["colorNotes"] = Value::Object(notes.iter().map(|(n, t)| (n.to_string(), json!(t))).collect());
    }
    let path = dir.join(format!("{name}.pixelpal"));
    fs::write(&path, engine::format::serialize_palette(&p)).unwrap();
    path
}

#[test]
fn r20_a_document_exports_its_palette_as_one_file_and_replaces_only_when_told() {
    let doc = make_document();
    let mut s = session(&doc);
    let dir = lib_dir();
    let to = dir.join("sample");
    let r = ok(&mut s, "palette.export", json!({ "to": to.to_str().unwrap() }));
    let file = dir.join("sample.pixelpal");
    assert_eq!(r["to"], file.to_str().unwrap());
    assert_eq!((r["world"].clone(), r["outfit"].clone()), (json!(10), json!(1)));
    let exported = read(&file);
    assert_eq!(exported["format"], 1);
    assert_eq!(exported["kind"], "palette");
    let mut palette = read(&doc.root.join("palette.json"));
    palette.as_object_mut().unwrap().insert("format".into(), json!(1));
    assert_eq!(exported["world"], palette["world"]);
    assert_eq!(exported["outfit"], palette["outfit"]);
    let keys = top_keys(&text(&file));
    assert_eq!(keys, ["format", "kind", "world", "outfit"]);
    let first = text(&file);
    let e = call(&mut s, "palette.export", json!({ "to": file.to_str().unwrap() })).expect_err("exists");
    assert_eq!(code_of(&e), "exists");
    assert_eq!(text(&file), first);
    ok(&mut s, "palette.export", json!({ "to": file.to_str().unwrap(), "replace": true }));
    assert_eq!(text(&file), first, "the same palette gives the same bytes");
    assert!(s.dirty_refs().is_empty(), "exporting doesn't change the document");
}

// ------------------------------------------------------------------ R21–R23: palette import

#[test]
fn r21_import_adds_new_colors_at_the_end_of_their_tier_as_one_edit_and_one_version() {
    let doc = make_document();
    let mut s = session(&doc);
    let dir = lib_dir();
    let file = palette_file(
        &dir,
        "coast",
        &[("ink", "#2E2418"), ("sand-1", "#E0D0A0"), ("grass-2", "#8FA56E"), ("sea-1", "#3B6E8F")],
        &[("teal-1", "#7FA89B"), ("plum-1", "#8A6A9A")],
        &[("sand-1", "Dry sand."), ("ink", "Their note for ink."), ("plum-1", "Dusk coats.")],
    );
    let f = file.to_str().unwrap();
    ok(&mut s, "object.category", json!({ "names": ["block"], "to": "Ground" }));
    ok(&mut s, "doc.save", json!({ "doc": "object:block" }));
    ok(&mut s, "history.mark", json!({}));
    let before = versions(&mut s);
    let plan = ok(&mut s, "palette.import", json!({ "from": f, "dryRun": true }));
    assert_eq!(plan["new"].as_array().unwrap().len(), 3);
    assert_eq!(plan["new"][0], json!({ "name": "sand-1", "tier": "world", "hex": "#E0D0A0", "selected": true, "note": "Dry sand." }));
    assert_eq!(plan["tiers"]["world"], json!({ "have": 10, "adding": 2, "cap": 32, "over": 0 }));
    assert_eq!(plan["ready"], true);
    assert_eq!(versions(&mut s), before);
    assert!(s.dirty_refs().is_empty());

    let r = ok(&mut s, "palette.import", json!({ "from": f }));
    assert_eq!(r["summary"], "Imported 3 colors from coast.pixelpal");
    assert_eq!(r["added"], json!(["sand-1", "sea-1", "plum-1"]));
    ok(&mut s, "doc.save", json!({ "doc": "palette:" }));
    let after = read(&doc.root.join("palette.json"));
    let tier = |t: &str| after[t].as_object().unwrap().keys().cloned().collect::<Vec<_>>();
    let world = tier("world");
    assert_eq!(&world[world.len() - 2..], ["sand-1", "sea-1"], "at the end of the tier, in the file's order");
    assert_eq!(world.len(), 12);
    assert_eq!(tier("outfit"), ["teal-1", "plum-1"]);
    // The notes of the new colors only: ink had none and keeps none.
    assert_eq!(after["colorNotes"], json!({ "sand-1": "Dry sand.", "plum-1": "Dusk coats." }));
    // One undo step on the palette doc, one version.
    assert_eq!(s.entries.get("palette:").unwrap().undo.len(), 1);
    assert_eq!(versions(&mut s), before + 1);
    let number = ok(&mut s, "history.list", json!({}))["versions"][0]["number"].as_u64().unwrap();
    let v = ok(&mut s, "history.show", json!({ "version": number }));
    assert_eq!(v["edits"], json!(["Imported 3 colors from coast.pixelpal"]));
    // Again: nothing to add.
    let again = ok(&mut s, "palette.import", json!({ "from": f }));
    assert_eq!(again["changed"], false);
    // Undo takes it all back.
    ok(&mut s, "history.undo", json!({ "version": number }));
    assert_eq!(read(&doc.root.join("palette.json")), {
        let mut p = after.clone();
        p["world"].as_object_mut().unwrap().shift_remove("sand-1");
        p["world"].as_object_mut().unwrap().shift_remove("sea-1");
        p["outfit"].as_object_mut().unwrap().shift_remove("plum-1");
        p.as_object_mut().unwrap().shift_remove("colorNotes");
        p
    });
}

#[test]
fn r21_the_import_never_removes_a_color_and_a_cli_import_saves_the_palette() {
    let doc = make_document();
    let dir = lib_dir();
    let file = palette_file(&dir, "tiny", &[("sea-1", "#3B6E8F")], &[], &[]);
    let before = read(&doc.root.join("palette.json"));
    let (code, out, err) = cli(&doc, &["palette", "import", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}{out}");
    assert!(out.contains("Imported 1 color from tiny.pixelpal") && out.contains("wrote palette.json"), "{out}{err}");
    let after = read(&doc.root.join("palette.json"));
    for tier in ["world", "outfit"] {
        for (name, hex) in before[tier].as_object().unwrap() {
            assert_eq!(&after[tier][name], hex, "{name} is kept");
        }
    }
    assert_eq!(after["world"]["sea-1"], "#3B6E8F");
    // The saved file is canonical.
    assert_eq!(text(&doc.root.join("palette.json")), engine::format::serialize_palette(&after));
    let (code, out, _) = cli(&doc, &["palette", "import", file.to_str().unwrap(), "--dry-run"]);
    assert_eq!(code, 0);
    assert!(out.contains("Would import 0 new colors"), "{out}");
}

#[test]
fn r22_differing_colors_are_the_owners_choice_and_the_command_refuses_unless_told() {
    let doc = make_document();
    let mut s = session(&doc);
    let dir = lib_dir();
    // teal-1 is a world color in the file but an outfit color here; grass-2 the other way round.
    let file = palette_file(
        &dir,
        "diff",
        &[("wood-3", "#101010"), ("ink", "#000000"), ("sea-1", "#3B6E8F"), ("teal-1", "#00FF00")],
        &[("grass-2", "#8FA56E")],
        &[],
    );
    let f = file.to_str().unwrap();
    let plan = ok(&mut s, "palette.import", json!({ "from": f, "dryRun": true }));
    assert_eq!(
        plan["differing"],
        json!([
            { "name": "wood-3", "tier": "world", "theirs": "#101010", "mine": "#7A4E2D" },
            { "name": "ink", "tier": "world", "theirs": "#000000", "mine": "#2E2418" }
        ])
    );
    assert_eq!(
        plan["otherTier"],
        json!([
            { "name": "teal-1", "tier": "outfit", "theirsTier": "world", "theirs": "#00FF00", "mine": "#7FA89B" },
            { "name": "grass-2", "tier": "world", "theirsTier": "outfit", "theirs": "#8FA56E", "mine": "#8FA56E" }
        ])
    );
    assert_eq!(plan["ready"], false);
    let e = call(&mut s, "palette.import", json!({ "from": f })).expect_err("differs");
    assert_eq!(code_of(&e), "palette_differs");
    assert_eq!(details_of(&e)["differing"].as_array().unwrap().len(), 2);
    let e = call(&mut s, "palette.import", json!({ "from": f, "keep": ["wood-3"] })).expect_err("one is left");
    assert_eq!(code_of(&e), "palette_differs");
    assert!(e.message().contains("ink") && !e.message().contains("wood-3"), "{}", e.message());
    for params in [
        json!({ "from": f, "keep": ["wood-3", "ink"], "useTheirs": ["wood-3"] }),
        json!({ "from": f, "keep": ["sea-1"] }),
        json!({ "from": f, "keep": ["wood-3", "ink"], "useTheirs": ["teal-1"] }),
        json!({ "from": f, "keep": ["wood-3", "ink"], "only": ["wood-3"] }),
    ] {
        let e = call(&mut s, "palette.import", params.clone()).expect_err("usage");
        assert_eq!(code_of(&e), "usage", "{params}");
    }
    assert!(s.dirty_refs().is_empty(), "refusals changed nothing");
    // Keep one, use theirs for the other: the new color is added, the other-tier colors stay as the document has them.
    let r = ok(&mut s, "palette.import", json!({ "from": f, "keep": ["wood-3"], "useTheirs": ["ink"] }));
    assert_eq!(r["summary"], "Imported 2 colors from diff.pixelpal");
    assert_eq!((r["added"].clone(), r["usedTheirs"].clone()), (json!(["sea-1"]), json!(["ink"])));
    assert_eq!(ok(&mut s, "doc.save", json!({ "doc": "palette:" }))["wrote"], true);
    let after = read(&doc.root.join("palette.json"));
    assert_eq!(after["world"]["wood-3"], "#7A4E2D");
    assert_eq!(after["world"]["ink"], "#000000");
    assert_eq!(after["world"]["sea-1"], "#3B6E8F");
    assert_eq!(after["outfit"]["teal-1"], "#7FA89B");
    assert!(after["world"]["teal-1"].is_null() && after["outfit"]["grass-2"].is_null());
    assert_eq!(s.entries.get("palette:").unwrap().undo.len(), 1);
}

#[test]
fn r23_a_palette_import_that_would_pass_a_cap_adds_nothing_and_says_how_many_are_over() {
    let doc = make_document();
    let mut s = session(&doc);
    let dir = lib_dir();
    // 10 world colors here; 25 new ones makes 35, three over the cap of 32. Outfit: 1 here plus 16 new is one over 16.
    let world: Vec<(String, String)> =
        (0..25).map(|i| (format!("tone-{i}"), format!("#{:02X}{:02X}{:02X}", 10 + i, 20 + i, 30 + i))).collect();
    let outfit: Vec<(String, String)> = (0..16).map(|i| (format!("garb-{i}"), format!("#{:02X}0000", 100 + i))).collect();
    let w: Vec<(&str, &str)> = world.iter().map(|(n, h)| (n.as_str(), h.as_str())).collect();
    let o: Vec<(&str, &str)> = outfit.iter().map(|(n, h)| (n.as_str(), h.as_str())).collect();
    let file = palette_file(&dir, "big", &w, &o, &[]);
    let f = file.to_str().unwrap();
    let before = text(&doc.root.join("palette.json"));
    let plan = ok(&mut s, "palette.import", json!({ "from": f, "dryRun": true }));
    assert_eq!(
        (plan["over"].clone(), plan["tiers"]["world"]["over"].clone(), plan["tiers"]["outfit"]["over"].clone()),
        (json!(4), json!(3), json!(1))
    );
    let e = call(&mut s, "palette.import", json!({ "from": f })).expect_err("full");
    assert_eq!(code_of(&e), "palette_full");
    assert!(
        e.message().contains("3 over the world tier's cap of 32") && e.message().contains("1 over the outfit tier's cap of 16"),
        "{}",
        e.message()
    );
    assert_eq!(details_of(&e)["over"], 4);
    assert!(s.dirty_refs().is_empty());
    assert_eq!(text(&doc.root.join("palette.json")), before);
    // Narrowed to a list that fits: 22 world and 15 outfit colors.
    let only: Vec<String> = world.iter().take(22).map(|c| c.0.clone()).chain(outfit.iter().take(15).map(|c| c.0.clone())).collect();
    let plan = ok(&mut s, "palette.import", json!({ "from": f, "only": only, "dryRun": true }));
    assert_eq!(plan["over"], 0);
    assert_eq!(plan["new"].as_array().unwrap().iter().filter(|c| c["selected"] == true).count(), 37);
    let r = ok(&mut s, "palette.import", json!({ "from": f, "only": only }));
    assert_eq!(r["summary"], "Imported 37 colors from big.pixelpal");
    let e = call(&mut s, "palette.import", json!({ "from": f, "only": ["nope"] })).expect_err("usage");
    assert_eq!(code_of(&e), "usage");
    let saved = ok(&mut s, "doc.save", json!({ "doc": "palette:" }));
    assert_eq!(saved["wrote"], true);
    assert_eq!(read(&doc.root.join("palette.json"))["world"].as_object().unwrap().len(), 32);
}

#[test]
fn r19_a_file_that_is_not_a_palette_file_is_told_so() {
    let doc = make_document();
    let mut s = session(&doc);
    let dir = lib_dir();
    fs::write(dir.join("a.pixelpal"), "not json").unwrap();
    fs::write(dir.join("b.pixelpal"), "{\"format\": 1, \"kind\": \"library\"}").unwrap();
    fs::write(dir.join("c.pixelpal"), "{\"format\": 3, \"kind\": \"palette\", \"world\": {}, \"outfit\": {}}").unwrap();
    fs::write(dir.join("d.pixelpal"), "{\"format\": 1, \"kind\": \"palette\", \"world\": {\"x\": \"red\"}, \"outfit\": {}}").unwrap();
    for (name, says) in [("a", "isn't JSON"), ("b", "kind"), ("c", "format 3"), ("d", "x")] {
        let path = dir.join(format!("{name}.pixelpal"));
        let e = call(&mut s, "palette.import", json!({ "from": path.to_str().unwrap() })).expect_err("refused");
        assert_eq!(code_of(&e), "palette_file", "{name}");
        assert!(e.message().contains(says), "{name}: {}", e.message());
    }
    let e = call(&mut s, "palette.import", json!({ "from": dir.join("none.pixelpal").to_str().unwrap() })).expect_err("missing");
    assert_eq!(code_of(&e), "not_found");
    // A file over the cap is itself not a palette.
    let world: Vec<(String, String)> = (0..33).map(|i| (format!("t-{i}"), "#101010".to_string())).collect();
    let w: Vec<(&str, &str)> = world.iter().map(|(n, h)| (n.as_str(), h.as_str())).collect();
    let big = palette_file(&dir, "e", &w, &[], &[]);
    let e = call(&mut s, "palette.import", json!({ "from": big.to_str().unwrap() })).expect_err("over");
    assert_eq!(code_of(&e), "palette_file");
}
