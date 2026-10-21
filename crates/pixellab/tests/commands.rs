//! Every read-only command, called directly through the registry and through
//! the CLI door, against a temp copy of the sample document (ports
//! cli/test/commands.test.mjs). The commands are ported by U22 to U26, so
//! every test here is ignored until then. The Node suite ran on a copy of the
//! site's art (hero-island, pine, ...); these run on the sample document
//! (scene `sample`, objects `block` and `pebble`), format 2.

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

use common::{make_document, TempDocument};

/// A scene that breaks two rules, and an object that breaks one.
const BROKEN_SCENE: &str = r#"{
  "viewBox": [0, 0, 10, 10],
  "items": [
    { "object": "no-such-object", "at": { "tile": [0, 0, 0] } },
    { "object": "pebble", "at": { "px": [1, 2, 3] } }
  ]
}
"#;
const BROKEN_OBJECT: &str = r#"{
  "kind": "sprite",
  "keys": { "a": "not-a-color" },
  "layers": [{ "map": ["a"] }]
}
"#;

fn broken() -> TempDocument {
    let doc = make_document();
    doc.write("scenes/zz-broken.json", BROKEN_SCENE);
    doc.write("objects/zz-bad.json", BROKEN_OBJECT);
    doc
}

fn env_of(doc: &TempDocument) -> HashMap<String, String> {
    doc.env()
}

fn open(doc: &TempDocument) -> Session {
    let env = env_of(doc);
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    Session::new(document, Options { env, ..Options::default() })
}

/// `registry.call(ctx, name, params)` on a fresh session over the document.
fn call(doc: &TempDocument, name: &str, params: Value) -> Result<Value, Error> {
    let mut session = open(doc);
    let mut ctx = Ctx::new(&mut session);
    registry().call(&mut ctx, name, params, false)
}

fn cli(doc: &TempDocument, args: &[&str]) -> (i32, String, String) {
    let reg = registry();
    let mut argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    argv.push("--document".into());
    argv.push(doc.root.to_string_lossy().into_owned());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = {
        let mut io = Io { stdout: &mut out, stderr: &mut err, env: env_of(doc), cwd: doc.root.clone(), registry: &reg };
        main(&argv, &mut io)
    };
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn cli_json(doc: &TempDocument, args: &[&str]) -> Value {
    let mut all = args.to_vec();
    all.push("--json");
    let (code, out, err) = cli(doc, &all);
    assert_eq!(code, 0, "{err}");
    serde_json::from_str(&out).unwrap()
}

fn code_of(e: &Error) -> String {
    match e {
        Error::Command(c) => c.code.clone(),
        Error::Internal(_) => "internal".into(),
    }
}

#[test]
// The format-2 meaning of the Node test: a document given by --document resolves to its folder.
fn document_info_spec_r3_r4_a_site_checkout_resolves_to_its_legacy_source_folder() {
    let doc = make_document();
    let r = call(&doc, "document.info", json!({})).unwrap();
    assert_eq!(r["root"], doc.root.to_string_lossy().as_ref());
    assert_eq!(r["name"], "sample");
    assert_eq!(r["from"], "flag");
    assert_eq!(r["scenes"], 2);
    assert_eq!(r["objects"], 2);
    assert_eq!(r["problems"], 0);
    assert_eq!(cli_json(&doc, &["document", "info"]), r);
    let (_, human, _) = cli(&doc, &["document", "info"]);
    assert!(human.starts_with("document  "), "{human}");
    assert!(human.ends_with("problems  0\n"), "{human}");
}

#[test]
fn document_info_spec_r3_r4_a_site_checkout_resolves_to_its_document_folder() {
    let site = common::make_site();
    let run = |extra: &[&str]| {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_pixellab"))
            .args(["document", "info", "--site"])
            .arg(&site.root)
            .args(extra)
            .envs(site.env())
            .current_dir(&site.base)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    let r: Value = serde_json::from_str(&run(&["--json"])).unwrap();
    assert_eq!(r["root"], site.document.to_string_lossy().as_ref());
    assert_eq!(r["siteRoot"], site.root.to_string_lossy().as_ref());
    assert_eq!(r["kind"], "document");
    assert_eq!(r["from"], "flag");
    assert_eq!(r["via"], "site");
    assert_eq!(r["publishDir"], site.document.parent().unwrap().join("sample-out").to_string_lossy().as_ref());
    assert_eq!((r["scenes"].as_i64().unwrap() >= 1, r["objects"].as_i64().unwrap() >= 2), (true, true));
    assert_eq!(r["colors"]["world"], 10);
    assert_eq!(r["problems"], 0);
    assert_eq!(r["branch"], Value::Null, "the temp copy is not a git checkout");
    let human = run(&[]);
    assert!(human.starts_with("document  ") && human.contains("(from the site checkout given by --site)\n"), "{human}");
    assert!(human.ends_with("problems  0\n"), "{human}");
}

#[test]
fn palette_get() {
    let doc = make_document();
    let r = call(&doc, "palette.get", json!({})).unwrap();
    let grass = r["colors"].as_array().unwrap().iter().find(|c| c["name"] == "grass-1").unwrap();
    assert_eq!(r["world"]["grass-1"], grass["hex"]);
    for c in r["colors"].as_array().unwrap() {
        let hex = c["hex"].as_str().unwrap();
        assert!(
            hex.len() == 7 && hex.starts_with('#') && hex[1..].chars().all(|d| d.is_ascii_digit() || ('A'..='F').contains(&d)),
            "{hex}"
        );
    }
    assert_eq!(cli_json(&doc, &["palette", "get"]), r);
    let (_, human, _) = cli(&doc, &["palette", "get"]);
    assert!(human.lines().any(|l| l.starts_with("world   #") && l.ends_with("  grass-1")), "{human}");
}

#[test]
fn scene_list() {
    let doc = make_document();
    let r = call(&doc, "scene.list", json!({})).unwrap();
    let scene = r["scenes"].as_array().unwrap().iter().find(|s| s["name"] == "sample").unwrap();
    assert_eq!(scene["output"], "sample.svg");
    assert_eq!(scene["file"], "scenes/sample.json");
    assert_eq!(scene["viewBox"].as_array().unwrap().len(), 4);
    assert_eq!(scene["items"], 2);
    assert_eq!(cli_json(&doc, &["scene", "list"]), r);
    let (_, human, _) = cli(&doc, &["scene", "list"]);
    assert!(human.lines().any(|l| l.starts_with("sample ") && l.ends_with("  sample.svg")), "{human}");
}

#[test]
fn scene_show() {
    let doc = make_document();
    let r = call(&doc, "scene.show", json!({ "name": "sample" })).unwrap();
    assert_eq!(r["name"], "sample");
    assert!(r["objects"]["pebble"].as_i64().unwrap() > 0);
    assert!(r.get("doc").is_none());
    let with_doc = call(&doc, "scene.show", json!({ "name": "scenes/sample", "doc": true })).unwrap();
    assert!(with_doc["doc"]["items"].is_array());
    assert_eq!(cli_json(&doc, &["scene", "show", "sample"]), r);
    let (_, human, _) = cli(&doc, &["scene", "show", "sample"]);
    assert!(human.starts_with("sample  (scenes/sample.json)\noutput    sample.svg\n"), "{human}");
    let err = call(&doc, "scene.show", json!({ "name": "pebble" })).unwrap_err();
    assert_eq!(code_of(&err), "not_found");
    assert!(err.message().contains("no scene named pebble"));
    let (code, _, stderr) = cli(&doc, &["scene", "show", "nope"]);
    assert_eq!((code, stderr.as_str()), (1, "pixellab: no scene named nope\n"));
}

#[test]
fn scene_items_flattened_with_stable_paths_into_groups() {
    let doc = make_document();
    let r = call(&doc, "scene.items", json!({ "name": "sample" })).unwrap();
    assert_eq!(r["scene"], "sample");
    let items = r["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    for (i, item) in items.iter().enumerate() {
        assert_eq!(item["path"], i.to_string());
        assert_eq!(item["point"].as_array().unwrap().len(), 2);
    }
    assert_eq!(items[0]["object"], "block");
    assert_eq!(items[1]["at"]["tile"], json!([0, 0, 1]));
    assert_eq!(cli_json(&doc, &["scene", "items", "sample"]), r);
    let (_, human, _) = cli(&doc, &["scene", "items", "sample"]);
    assert_eq!(human.trim().lines().count(), items.len());
    assert!(human.starts_with("0 "), "{human}");
}

#[test]
fn scene_render_shape_pixels_and_bounds_are_in_render_test_mjs() {
    let doc = make_document();
    let r = call(&doc, "scene.render", json!({ "name": "sample", "scale": 2 })).unwrap();
    for key in ["png", "scale", "viewBox", "width", "height", "items"] {
        assert!(r.get(key).is_some(), "{key}");
    }
    assert_eq!(r["width"].as_i64().unwrap(), r["viewBox"][2].as_i64().unwrap() * 2);
    assert_eq!(cli_json(&doc, &["scene", "render", "sample", "--scale", "2"]), r);
    let (_, human, _) = cli(&doc, &["scene", "render", "sample"]);
    assert!(human.starts_with("sample: "), "{human}");
    assert!(human.contains("\n(add --out"), "{human}");
}

#[test]
fn object_list() {
    let doc = make_document();
    let r = call(&doc, "object.list", json!({})).unwrap();
    let objects = r["objects"].as_array().unwrap();
    let pebble = objects.iter().find(|o| o["name"] == "pebble").unwrap();
    assert_eq!(pebble["kind"], "sprite");
    assert_eq!(pebble["size"].as_array().unwrap().len(), 2);
    assert_eq!(pebble["layers"], 1);
    assert!(pebble["colors"].as_i64().unwrap() > 0);
    assert!(pebble.get("resolved").is_none());
    let blocks = call(&doc, "object.list", json!({ "kind": "block" })).unwrap();
    let blocks = blocks["objects"].as_array().unwrap();
    assert!(!blocks.is_empty() && blocks.iter().all(|o| o["kind"] == "block" && o["blockSize"].as_array().unwrap().len() == 3));
    assert_eq!(cli_json(&doc, &["object", "list"]), r);
    let (_, human, _) = cli(&doc, &["object", "list", "--kind", "block"]);
    assert!(human.lines().any(|l| l.starts_with("block ") && l.contains("block 1×1×1 · ")), "{human}");
}

#[test]
fn object_show_kind_size_layers_frames_colors_extends() {
    let doc = make_document();
    let pebble = call(&doc, "object.show", json!({ "name": "pebble" })).unwrap();
    assert_eq!(pebble["kind"], "sprite");
    assert_eq!(pebble["file"], "objects/pebble.json");
    assert_eq!(pebble["anchor"], json!([2, 3]));
    assert_eq!(pebble["layers"].as_array().unwrap().len(), 1);
    assert_eq!(pebble["frames"], 1);
    for k in pebble["keys"].as_array().unwrap() {
        assert_eq!(k["tier"], "world");
    }
    let block = call(&doc, "object.show", json!({ "name": "block" })).unwrap();
    assert_eq!(block["kind"], "block");
    assert_eq!(block["block"]["size"], json!([1, 1, 1]));
    assert_eq!(cli_json(&doc, &["object", "show", "pebble"]), pebble);
    let (_, human, _) = cli(&doc, &["object", "show", "block"]);
    assert!(human.lines().any(|l| l == "kind      block"), "{human}");
}

#[test]
fn object_render_shape() {
    let doc = make_document();
    let r = call(&doc, "object.render", json!({ "name": "pebble", "scale": 3 })).unwrap();
    let pebble = call(&doc, "object.show", json!({ "name": "pebble" })).unwrap();
    let a = pebble["anchor"].as_array().unwrap();
    let size = pebble["size"].as_array().unwrap();
    assert_eq!(r["viewBox"], json!([-a[0].as_i64().unwrap(), -a[1].as_i64().unwrap(), size[0], size[1]]));
    assert_eq!(r["width"].as_i64().unwrap(), size[0].as_i64().unwrap() * 3);
    assert_eq!(r["items"].as_array().unwrap().len(), 1);
    assert_eq!(cli_json(&doc, &["object", "render", "pebble", "--scale", "3"]), r);
}

#[test]
fn check_everything_one_doc_and_problems_with_exit_code_1() {
    let doc = make_document();
    let all = call(&doc, "check", json!({})).unwrap();
    assert_eq!(all["ok"], true);
    assert_eq!(all["problems"], json!([]));
    let one = call(&doc, "check", json!({ "name": "sample" })).unwrap();
    assert_eq!(one["ok"], true);
    assert_eq!(one["target"], json!({ "kind": "scene", "name": "sample", "file": "scenes/sample.json" }));
    assert!(one["summary"].as_str().unwrap().ends_with("output sample.svg"));
    let (code, out, _) = cli(&doc, &["check", "pebble"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("objects/pebble.json: ok · "), "{out}");

    let bad = broken();
    let everything = call(&bad, "check", json!({})).unwrap();
    assert_eq!(everything["ok"], false);
    let problems = everything["problems"].as_array().unwrap();
    assert!(problems.iter().any(|p| p["file"] == "scenes/zz-broken.json" && p["message"].as_str().unwrap().contains("no-such-object")));
    assert!(problems.iter().any(|p| p["file"] == "objects/zz-bad.json"));
    let scoped = call(&bad, "check", json!({ "name": "zz-broken" })).unwrap();
    let scoped = scoped["problems"].as_array().unwrap();
    assert!(scoped.len() >= 2 && scoped.iter().all(|p| p["file"] == "scenes/zz-broken.json"));
    // Another doc that's fine is fine, even though the sources have problems.
    assert_eq!(call(&bad, "check", json!({ "name": "pebble" })).unwrap()["ok"], true);

    let (code, out, _) = cli(&bad, &["check", "zz-broken"]);
    assert_eq!(code, 1);
    assert!(out.lines().any(|l| l == "scenes/zz-broken.json: items[0]: object 'no-such-object' does not exist"), "{out}");
    assert!(out.lines().any(|l| l.ends_with(" problems")), "{out}");
    let (code, out, _) = cli(&bad, &["check", "--json"]);
    assert_eq!(code, 1);
    assert_eq!(serde_json::from_str::<Value>(&out).unwrap()["ok"], false);
}

#[test]
fn r11_r12_check_warns_with_a_code_off_the_grid_a_scene_out_of_depth_order_and_the_copies_a_moving_item_is_drawn_in() {
    let house = json!({ "kind": "block", "size": [1, 1, 1], "faces": { "top": "grass-2", "left": "grass-4", "right": "wood-3" } });
    let walker = json!({ "kind": "sprite", "keys": { "g": "grass-2", "i": "ink" }, "anchor": [2, 4], "layers": [{ "map": [".gg.", "gggg", "gggg", ".ii."] }] });
    let scene = |items: Value, tracks: Value| {
        let mut doc = json!({ "viewBox": [-80, -20, 160, 140], "items": items });
        if tracks.as_array().is_some_and(|t| !t.is_empty()) {
            doc["motion"] = json!({ "duration": 10000, "tracks": tracks });
        }
        engine::format::serialize(&doc)
    };
    let doc = make_document();
    doc.write("objects/house.json", &engine::format::serialize(&house));
    doc.write("objects/walker.json", &engine::format::serialize(&walker));
    // The walker at tile 0,1,0 is behind the house's walls at 1,1,1, but is listed after them.
    doc.write(
        "scenes/out-of-order.json",
        &scene(json!([{ "object": "house", "at": { "tile": [1, 1, 1] } }, { "object": "walker", "at": { "tile": [0, 1, 0] } }]), json!([])),
    );
    doc.write(
        "scenes/in-order.json",
        &scene(json!([{ "object": "walker", "at": { "tile": [0, 1, 0] } }, { "object": "house", "at": { "tile": [1, 1, 1] } }]), json!([])),
    );
    // The walker passes behind the house and then in front of it, so it is drawn in two places.
    doc.write(
        "scenes/crossing.json",
        &scene(
            json!([{ "object": "house", "at": { "tile": [2, 2, 1] } }, { "object": "walker", "id": "w", "at": { "px": [0, 0] } }]),
            json!([{ "item": "w", "keys": [{ "t": 0 }, { "t": 5000, "move": [0, 64] }] }]),
        ),
    );
    doc.write(
        "scenes/off-grid.json",
        &scene(
            json!([{ "object": "walker", "id": "w", "at": { "px": [0, 0] } }]),
            json!([{ "item": "w", "keys": [{ "t": 0 }, { "t": 5000, "move": [3, 5] }] }]),
        ),
    );
    let check = |params: Value| call(&doc, "check", params).unwrap();
    let codes = |r: &Value| -> Vec<String> {
        r["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| format!("{} {}", w["code"].as_str().unwrap(), w["scene"].as_str().unwrap()))
            .collect()
    };
    let order = check(json!({ "name": "out-of-order" }));
    assert_eq!(order["ok"], true);
    assert_eq!(codes(&order), ["out_of_order out-of-order"]);
    assert_eq!(order["warnings"][0]["moves"], 1, "the walker moves up in front of the house; the house is not counted");
    assert_eq!(order["warnings"][0]["file"], "scenes/out-of-order.json");
    assert_eq!(
        order["warnings"][0]["text"],
        "scenes/out-of-order.json: items are not in depth order: 1 would move (pixellab scene sort out-of-order puts them right)"
    );
    assert_eq!(check(json!({ "name": "in-order" }))["warnings"], json!([]), "a scene in order has none");
    let crossing = check(json!({ "name": "crossing" }));
    assert_eq!(codes(&crossing), ["copies crossing"]);
    assert_eq!((&crossing["warnings"][0]["item"], &crossing["warnings"][0]["copies"]), (&json!("w"), &json!(2)));
    assert!(crossing["warnings"][0]["text"]
        .as_str()
        .unwrap()
        .starts_with("scenes/crossing.json: w is drawn 2 times as it moves among other items"));
    assert_eq!(codes(&check(json!({ "name": "off-grid" }))), ["off_grid off-grid", "off_grid off-grid"]);
    assert_eq!(check(json!({ "name": "walker" }))["warnings"], json!([]), "an object has none");
    assert_eq!(codes(&check(json!({}))), ["copies crossing", "off_grid off-grid", "off_grid off-grid", "out_of_order out-of-order"]);
    let (code, out, _) = cli(&doc, &["check", "crossing"]);
    assert_eq!(code, 0, "warnings keep the exit code 0");
    assert!(out.lines().any(|l| l.starts_with("warning: scenes/crossing.json: w is drawn 2 times")), "{out}");
    assert!(out.lines().any(|l| l == "1 warning"), "{out}");
}

#[test]
fn r11_a_scene_with_a_problem_gets_no_depth_warnings_only_its_problems() {
    let bad = broken();
    let r = call(&bad, "check", json!({ "name": "zz-broken" })).unwrap();
    assert!(r["problems"].as_array().unwrap().len() >= 2);
    assert_eq!(r["warnings"], json!([]));
}

#[test]
fn rendering_a_broken_scene_is_a_clear_command_error() {
    let bad = broken();
    let err = call(&bad, "scene.render", json!({ "name": "zz-broken" })).unwrap_err();
    assert_eq!(code_of(&err), "render_error");
    assert!(err.message().contains("run pixellab check zz-broken"), "{}", err.message());
}

#[test]
fn an_ambiguous_name_asks_for_objects_or_scenes() {
    let both = make_document();
    both.write("scenes/pebble.json", "{ \"viewBox\": [0, 0, 4, 4], \"items\": [] }\n");
    let (code, _, err) = cli(&both, &["check", "pebble"]);
    assert_eq!(code, 1);
    assert!(err.contains("pebble is both an object and a scene; say objects/pebble or scenes/pebble"), "{err}");
    let scoped = cli_json(&both, &["check", "scenes/pebble"]);
    assert_eq!(scoped["target"]["kind"], "scene");
    let kind = cli_json(&both, &["check", "pebble", "--kind", "object"]);
    assert_eq!(kind["target"]["kind"], "object");
}
