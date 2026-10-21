//! R18, D5 (docs/intents/2026-10-rust-engine/spec.md): a scene's
//! `"image": { "file", "scale" }` is rendered by `art build` into the
//! manifest's `publish.images` as the same PNG `art preview` makes.
#![allow(clippy::result_large_err)]

mod common;

use std::fs;

use common::{make_document, TempDocument};
use pixellab::cli::{main, Io};
use pixellab::registry::registry;
use serde_json::{json, Value};

fn cli(doc: &TempDocument, args: &[&str]) -> (i32, String, String) {
    let reg = registry();
    let mut argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    argv.push("--document".into());
    argv.push(doc.root.to_string_lossy().into_owned());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = {
        let mut io = Io { stdout: &mut out, stderr: &mut err, env: doc.env(), cwd: doc.root.clone(), registry: &reg };
        main(&argv, &mut io)
    };
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn edit(doc: &TempDocument, file: &str, f: impl FnOnce(&mut Value)) {
    let path = doc.root.join(file);
    let mut v: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    f(&mut v);
    fs::write(path, serde_json::to_string_pretty(&v).unwrap()).unwrap();
}

/// The fixture with sample's image set and `publish.images` pointing at a folder beside it.
fn doc_with_image(image: Value) -> TempDocument {
    let doc = make_document();
    edit(&doc, "scenes/sample.json", |s| s["image"] = image);
    edit(&doc, "manifest.json", |m| m["publish"]["images"] = json!("../sample-images"));
    doc
}

fn images_dir(doc: &TempDocument) -> std::path::PathBuf {
    doc.base.join("sample-images")
}

fn check_problems(doc: &TempDocument) -> Vec<String> {
    let docs = pixellab::document::open_document(&pixellab::document::OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: doc.env(),
        cwd: doc.root.clone(),
    })
    .unwrap()
    .load_docs();
    let sources = engine::engine::load_sources(docs.palette.unwrap_or(Value::Null), docs.objects, docs.scenes);
    docs.load_errors.into_iter().chain(engine::engine::validate(&sources)).collect()
}

fn preview_png(doc: &TempDocument, scale: &str) -> Vec<u8> {
    let out = doc.base.join("preview.png");
    let (code, o, e) = cli(doc, &["art", "preview", "sample", "--scale", scale, "--out", &out.to_string_lossy()]);
    assert_eq!(code, 0, "{o}{e}");
    fs::read(out).unwrap()
}

#[test]
fn r18_art_build_writes_the_image_at_scale_matching_the_preview() {
    let doc = doc_with_image(json!({ "file": "rest.png", "scale": 5 }));
    let (code, out, err) = cli(&doc, &["art", "build", "--json"]);
    assert_eq!(code, 0, "{out}{err}");
    let written = fs::read(images_dir(&doc).join("rest.png")).unwrap();
    assert_eq!(written, preview_png(&doc, "5"), "the same bytes as art preview at that scale");
    let image = engine::png::decode(&written).unwrap();
    assert_eq!((image.width, image.height), (64 * 5, 48 * 5), "the viewBox at 5x");
}

#[test]
fn r18_the_result_lists_the_image_with_the_outputs() {
    let doc = doc_with_image(json!({ "file": "rest.png", "scale": 2 }));
    let (code, out, err) = cli(&doc, &["art", "build", "--json"]);
    assert_eq!(code, 0, "{out}{err}");
    let r: Value = serde_json::from_str(&out).unwrap();
    let outputs = r["outputs"].as_array().unwrap();
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0]["kind"], "svg");
    let png = &outputs[1];
    assert_eq!(png["file"], "rest.png");
    assert_eq!(png["scene"], "sample");
    assert_eq!(png["kind"], "png");
    assert_eq!(png["raw"], fs::metadata(images_dir(&doc).join("rest.png")).unwrap().len());
    assert_eq!(png["warnings"], json!([]));
    assert_eq!(png.as_object().unwrap().keys().cloned().collect::<Vec<_>>(), ["file", "scene", "kind", "raw", "warnings"]);
}

#[test]
fn r18_the_human_output_has_one_line_per_image() {
    let doc = doc_with_image(json!({ "file": "rest.png", "scale": 2 }));
    let (code, out, err) = cli(&doc, &["art", "build"]);
    assert_eq!(code, 0, "{out}{err}");
    let size = fs::metadata(images_dir(&doc).join("rest.png")).unwrap().len();
    assert!(out.contains(&format!("rest.png: PNG, {size} bytes\n")), "{out}");
    assert!(out.contains("published 2 files to "), "{out}");
}

#[test]
fn r18_a_bad_file_scale_or_key_is_a_problem() {
    let cases = [
        (json!({ "file": "a/b.png", "scale": 2 }), "scenes/sample.json: image.file must be a plain file name ending in .png"),
        (json!({ "file": "rest.jpg", "scale": 2 }), "scenes/sample.json: image.file must be a plain file name ending in .png"),
        (json!({ "file": ".png", "scale": 2 }), "scenes/sample.json: image.file must be a plain file name ending in .png"),
        (json!({ "scale": 2 }), "scenes/sample.json: image.file must be a plain file name ending in .png"),
        (json!({ "file": "a.png", "scale": 0 }), "scenes/sample.json: image.scale must be a whole number from 1 to 16"),
        (json!({ "file": "a.png", "scale": 17 }), "scenes/sample.json: image.scale must be a whole number from 1 to 16"),
        (json!({ "file": "a.png", "scale": 2.5 }), "scenes/sample.json: image.scale must be a whole number from 1 to 16"),
        (json!({ "file": "a.png", "scale": "2" }), "scenes/sample.json: image.scale must be a whole number from 1 to 16"),
        (json!({ "file": "a.png", "scale": 2, "dpi": 3 }), "scenes/sample.json: image: unknown property 'dpi'"),
        (json!("rest.png"), "scenes/sample.json: image must be { file: \"<name>.png\", scale: <1 to 16> }"),
    ];
    for (image, want) in cases {
        let doc = doc_with_image(image.clone());
        assert_eq!(check_problems(&doc), [want], "{image}");
    }
    let ok = doc_with_image(json!({ "file": "a.png", "scale": 16 }));
    assert_eq!(check_problems(&ok), Vec::<String>::new());
}

#[test]
fn r18_an_image_without_publish_images_refuses_like_a_missing_publish_dir() {
    let doc = doc_with_image(json!({ "file": "rest.png", "scale": 2 }));
    edit(&doc, "manifest.json", |m| {
        m["publish"].as_object_mut().unwrap().remove("images");
    });
    let (code, out, err) = cli(&doc, &["art", "build"]);
    assert_eq!(code, 1, "{out}");
    assert!(err.contains("this document has no publish folder for images: add \"publish\": { \"images\": \"<folder>\" }"), "{err}");
    assert!(!doc.base.join("sample-out").join("sample.svg").exists(), "nothing is written");
}

#[test]
fn r18_dir_stands_in_for_publish_images_when_the_manifest_gives_none() {
    let doc = doc_with_image(json!({ "file": "rest.png", "scale": 1 }));
    edit(&doc, "manifest.json", |m| {
        m["publish"].as_object_mut().unwrap().remove("images");
    });
    let dir = doc.base.join("elsewhere");
    let (code, out, err) = cli(&doc, &["art", "build", "--dir", &dir.to_string_lossy()]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(dir.join("rest.png").exists() && dir.join("sample.svg").exists());
}

#[test]
fn r18_a_scene_with_an_image_round_trips_in_canonical_text() {
    let text = "{\n  \"output\": \"sample.svg\",\n  \"image\": { \"file\": \"rest.png\", \"scale\": 5 },\n  \"viewBox\": [-16, -16, 64, 48],\n  \"items\": []\n}\n";
    let doc = engine::format::parse(text).unwrap();
    assert_eq!(engine::format::serialize_file("scenes/sample.json", &doc), text);
    let shuffled = "{ \"viewBox\": [-16, -16, 64, 48], \"items\": [], \"image\": { \"scale\": 5, \"file\": \"rest.png\" }, \"output\": \"sample.svg\" }";
    let doc = engine::format::parse(shuffled).unwrap();
    assert_eq!(engine::format::serialize_file("scenes/sample.json", &doc), text);
}
