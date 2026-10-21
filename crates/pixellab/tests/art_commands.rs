//! art.preview and art.sizes (ports cli/test/art-commands.test.mjs;
//! docs/intents/2026-10-pixellab-document/spec.md, R8): the headless toolkit
//! the site's `npm run art -- --preview | --sizes` gave agents, as pixellab
//! commands. Calls go through the registry; the tests that need the command
//! line itself (`--out`, exit codes) wait for the CLI door.
#![allow(clippy::result_large_err)]

mod common;

use std::process::Command as Process;

use common::{make_document, TempDocument};
use pixellab::cli::{main, Io};
use pixellab::context::Ctx;
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::{Error, Result};
use pixellab::registry::registry;
use pixellab::session::{Options, Session};
use serde_json::{json, Value};

fn session(doc: &TempDocument) -> Session {
    let state = doc.base.join("state");
    let mut env = doc.env();
    env.insert("PIXELLAB_STATE".into(), state.to_string_lossy().into_owned());
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    Session::new(document, Options { env, ..Options::default() })
}

fn call(doc: &TempDocument, name: &str, params: Value) -> Result<Value> {
    let mut s = session(doc);
    registry().call(&mut Ctx::new(&mut s), name, params, false)
}

fn human(doc: &TempDocument, name: &str, params: Value) -> String {
    let mut s = session(doc);
    let reg = registry();
    let r = reg.call(&mut Ctx::new(&mut s), name, params.clone(), false).unwrap();
    let cmd = reg.get(name).unwrap();
    let checked = pixellab::params::check_params(&cmd.params, &params, false, Some(name)).unwrap();
    (cmd.format.unwrap())(&r, &checked)
}

fn code_of(e: Error) -> (String, String) {
    match e {
        Error::Command(c) => (c.code, c.message),
        other => panic!("not a command error: {other}"),
    }
}

fn png_of(out: &Value) -> engine::png::Image {
    let b64 = out["png"].as_str().unwrap();
    let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in b64.bytes().filter(|c| *c != b'=') {
        acc = acc << 6 | table.iter().position(|t| *t == c).unwrap() as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    engine::png::decode(&bytes).unwrap()
}

// art.preview (R8)

#[test]
fn r8_an_object_previews_on_one_tile_outline_on_the_card_color_with_a_2_pixel_margin() {
    let doc = make_document();
    let out = call(&doc, "art.preview", json!({ "name": "pebble" })).unwrap();
    assert_eq!(out["kind"], "object");
    assert_eq!(out["scale"], 1);
    let png = png_of(&out);
    assert_eq!([png.width as i64, png.height as i64], [out["width"].as_i64().unwrap(), out["height"].as_i64().unwrap()]);
    // The tile outline is 32 wide; the pebble sits on it; 2 pixels of margin all round.
    assert_eq!(png.width, 36);
    assert_eq!(png.rgba[..4], [0xe9, 0xdc, 0xc6, 255], "the corner is the card color from the manifest");
}

#[test]
fn r8_scale_multiplies_the_size_and_out_writes_the_png() {
    let doc = make_document();
    let out = call(&doc, "art.preview", json!({ "name": "pebble", "scale": 4 })).unwrap();
    assert_eq!(out["width"], 144);
    assert_eq!(png_of(&out).width, 144);
    let line = human(&doc, "art.preview", json!({ "name": "pebble", "scale": 4 }));
    assert!(line.starts_with("pebble: 144×") && line.ends_with(" preview PNG at 4×"), "{line}");
}

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

#[test]
fn r8_out_writes_the_png_on_the_command_line() {
    let doc = make_document();
    let file = doc.base.join("pebble@4x.png");
    let (code, stdout, stderr) = cli(&doc, &["art", "preview", "pebble", "--scale", "4", "--out", &file.to_string_lossy()]);
    assert_eq!(code, 0, "{stderr}");
    let lines: Vec<&str> = stdout.split_inclusive('\n').collect();
    assert_eq!(lines.len(), 2, "{stdout}");
    assert!(lines[0].starts_with("pebble: 144\u{d7}") && lines[0].ends_with(" preview PNG at 4\u{d7}\n"), "{stdout}");
    assert!(lines[1].starts_with("wrote ") && lines[1].contains("pebble@4x.png (144\u{d7}"), "{stdout}");
    let png = engine::png::decode(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(png.width, 144);
}

#[test]
fn r8_a_scene_previews_at_its_viewbox_first_variant() {
    let doc = make_document();
    let out = call(&doc, "art.preview", json!({ "name": "scenes/sample" })).unwrap();
    assert_eq!(out["kind"], "scene");
    assert_eq!([out["width"].as_i64().unwrap(), out["height"].as_i64().unwrap()], [64, 48]);
}

#[test]
fn r8_palette_renders_the_swatch_sheet_four_across_a_row_per_material_and_one_for_ink_cream_and_skin() {
    let doc = make_document();
    let out = call(&doc, "art.preview", json!({ "name": "palette" })).unwrap();
    assert_eq!(out["kind"], "palette");
    assert_eq!([out["width"].as_i64().unwrap(), out["height"].as_i64().unwrap()], [2 + 4 * 18, 2 + 8 * 18]);
}

#[test]
fn r8_a_doc_with_problems_or_one_that_uses_a_broken_object_gets_no_preview_code_invalid_exit_1() {
    let doc = make_document();
    doc.write("objects/zz-bad.json", r#"{ "kind": "sprite", "keys": { "a": "nope" }, "layers": [{ "map": ["a"] }] }"#);
    doc.write("scenes/zz-uses-bad.json", r#"{ "viewBox": [0, 0, 8, 8], "items": [{ "object": "zz-bad", "at": { "px": [0, 0] } }] }"#);
    for name in ["zz-bad", "zz-uses-bad"] {
        let e = call(&doc, "art.preview", json!({ "name": name })).unwrap_err();
        let Error::Command(c) = e else { panic!("not a command error") };
        assert_eq!((c.code.as_str(), c.exit_code), ("invalid", 1), "{name}");
        let details = c.details.unwrap();
        assert!(details.as_array().unwrap().iter().any(|p| p.as_str().unwrap().starts_with("objects/zz-bad.json: ")), "{details}");
    }
    assert!(call(&doc, "art.preview", json!({ "name": "pebble" })).is_ok(), "an object that does not use it still previews");
}

#[test]
fn r8_an_unknown_name_exits_1() {
    let doc = make_document();
    let (code, message) = code_of(call(&doc, "art.preview", json!({ "name": "nope" })).unwrap_err());
    assert_eq!(code, "not_found");
    assert!(message.contains("no object or scene named nope"), "{message}");
}

// art.sizes (R8)

#[test]
fn r8_prints_the_scenes_bytes_and_each_objects_and_colors_and_says_when_there_is_no_head() {
    let doc = make_document();
    let text = human(&doc, "art.sizes", json!({ "name": "sample" }));
    let mut lines = text.lines();
    let first = lines.next().unwrap();
    assert!(first.starts_with("sample.svg: ") && first.ends_with(" B gzip") && first.contains(" B raw, "), "{first}");
    assert_eq!(lines.next().unwrap(), "HEAD: not available (not in a git repository with a HEAD commit)");
    assert!(
        text.contains("\nobject  placed  raw  Δ raw  gzip  Δ gzip\n")
            || text.lines().any(|l| l.starts_with("object") && l.contains("placed") && l.contains("Δ raw") && l.ends_with("Δ gzip")),
        "{text}"
    );
    assert!(text.lines().any(|l| l.starts_with("block") && l.contains("  1  ") && l.ends_with('—')), "{text}");
    assert!(text.lines().any(|l| l.starts_with("color") && l.contains("Δ raw")), "{text}");
    assert!(text.contains("grass-2 #8FA56E"), "{text}");
    let out = call(&doc, "art.sizes", json!({ "name": "sample" })).unwrap();
    assert_eq!(out["output"], "sample.svg");
    assert_eq!(out["head"], json!({ "reason": "not in a git repository with a HEAD commit" }));
    let mut objects: Vec<&str> = out["objects"].as_array().unwrap().iter().map(|o| o["object"].as_str().unwrap()).collect();
    objects.sort();
    assert_eq!(objects, ["block", "pebble"]);
}

fn git(root: &std::path::Path, args: &[&str]) {
    let ok = Process::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

#[test]
fn r8_compares_with_the_sources_at_head_of_the_repository_the_document_is_in() {
    let doc = make_document();
    git(&doc.root, &["init", "-q"]);
    git(&doc.root, &["add", "-A"]);
    git(&doc.root, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "base"]);
    // Place the pebble a second time: more bytes than HEAD.
    let path = doc.root.join("scenes/sample.json");
    let scene = std::fs::read_to_string(&path).unwrap();
    let placed = r#"{ "object": "pebble", "at": { "tile": [0, 0, 1] } }"#;
    assert!(scene.contains(placed));
    std::fs::write(&path, scene.replace(placed, &format!(r#"{placed},{}"#, r#"{ "object": "pebble", "at": { "tile": [1, 0, 0] } }"#)))
        .unwrap();
    let out = call(&doc, "art.sizes", json!({ "name": "scenes/sample" })).unwrap();
    let (head, now) = (out["head"]["raw"].as_i64().unwrap(), out["now"]["raw"].as_i64().unwrap());
    assert!(head > 0 && now > head, "{now} > {head}");
    let pebble = out["objects"].as_array().unwrap().iter().find(|o| o["object"] == "pebble").unwrap();
    assert_eq!(pebble["placed"], 2);
    assert!(pebble["raw"].as_i64().unwrap() > pebble["headRaw"].as_i64().unwrap());
    let text = human(&doc, "art.sizes", json!({ "name": "sample" }));
    let line = text.lines().nth(1).unwrap();
    assert!(line.starts_with("HEAD: ") && line.contains(" B raw, ") && line.contains(" (+"), "{line}");
}

#[test]
fn r8_takes_a_scene_not_an_object() {
    let doc = make_document();
    let (code, message) = code_of(call(&doc, "art.sizes", json!({ "name": "pebble" })).unwrap_err());
    assert_eq!(code, "not_found");
    assert!(message.contains("no scene named pebble"), "{message}");
}
