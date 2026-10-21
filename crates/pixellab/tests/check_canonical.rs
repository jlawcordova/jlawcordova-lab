//! R20: `check` warns about a source file that isn't in canonical form, and
//! stays ok (a warning never fails it).

#![allow(clippy::result_large_err)]

mod common;

use pixellab::cli::{main, Io};
use pixellab::context::Ctx;
use pixellab::document::{open_document, OpenOptions};
use pixellab::registry::registry;
use pixellab::session::{Options, Session};
use serde_json::{json, Value};

use common::{make_document, TempDocument};

fn call(doc: &TempDocument, params: Value) -> Value {
    let env = doc.env();
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    let mut session = Session::new(document, Options { env, ..Options::default() });
    let mut ctx = Ctx::new(&mut session);
    registry().call(&mut ctx, "check", params, false).unwrap()
}

fn codes(r: &Value) -> Vec<String> {
    r["warnings"].as_array().unwrap().iter().map(|w| w["code"].as_str().unwrap().to_string()).collect()
}

#[test]
fn r20_check_warns_about_a_source_file_that_is_not_in_canonical_form_and_stays_ok() {
    let doc = make_document();
    // The sample's pebble stands partly outside its viewBox, which `check` now warns about (scene-canvas
    // spec R11); a wider box keeps this test about the canonical form.
    let wide = json!({ "output": "sample.svg", "viewBox": [-64, -64, 192, 128], "items": [
        { "object": "block", "at": { "tile": [0, 0, 0] } }, { "object": "pebble", "at": { "tile": [0, 0, 1] } }] });
    doc.write("scenes/sample.json", &engine::format::serialize_scene(&wide));
    let clean = call(&doc, json!({}));
    assert_eq!(clean["warnings"], json!([]), "the sample is canonical");
    // The same scene, hand-edited out of canonical form: one line, different spacing.
    doc.write("scenes/sample.json", r#"{"output":"sample.svg","viewBox":[-64,-64,192,128],"items":[{"object":"block","at":{"tile":[0,0,0]}},{"object":"pebble","at":{"tile":[0,0,1]}}]}"#);
    let r = call(&doc, json!({}));
    assert_eq!(r["ok"], true);
    assert_eq!(codes(&r), vec!["not_canonical"]);
    assert_eq!(r["warnings"][0]["file"], "scenes/sample.json");
    assert_eq!(r["warnings"][0]["text"], "scenes/sample.json: isn't in canonical form; saving it in Pixel Lab or pixellab rewrites it");
    // A check of that scene names it; a check of another doc doesn't.
    assert_eq!(codes(&call(&doc, json!({ "name": "sample" }))), vec!["not_canonical"]);
    assert_eq!(call(&doc, json!({ "name": "pebble" }))["warnings"], json!([]));
    // On the command line it is a warning line, and the exit code stays 0.
    let reg = registry();
    let argv: Vec<String> = ["check", "--document", &doc.root.to_string_lossy()].iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = {
        let mut io = Io { stdout: &mut out, stderr: &mut err, env: doc.env(), cwd: doc.root.clone(), registry: &reg };
        main(&argv, &mut io)
    };
    let out = String::from_utf8(out).unwrap();
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("warning: scenes/sample.json: isn't in canonical form"), "{out}");
    assert!(out.ends_with("1 warning\n") || out.ends_with("1 warning"), "{out}");
}
