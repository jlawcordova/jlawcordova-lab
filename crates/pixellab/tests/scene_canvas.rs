//! The program side of docs/intents/2026-10-scene-canvas/spec.md: a scene without a size
//! (R1 to R12) and placing on top of a block (R16, R17, R19), through the registry on a
//! session over a temp copy of the sample document, and through the CLI door.

#![allow(clippy::result_large_err)]

mod common;

use pixellab::cli::{main, Io};
use pixellab::context::{Ctx, DocKind};
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::{CommandError, Error};
use pixellab::registry::registry;
use pixellab::session::{Mode, Options, Session};
use serde_json::{json, Value};

/// A session over a temp copy of the sample document (objects `block` and `pebble`).
struct Lab {
    session: Session,
    doc: common::TempDocument,
    state: std::path::PathBuf,
}

fn lab_with(scenes: &[(&str, Value)], mode: Mode) -> Lab {
    let doc = common::make_document();
    for (name, scene) in scenes {
        doc.write(&format!("scenes/{name}.json"), &engine::format::stringify_pretty(scene));
    }
    let state = common::temp_dir("pixellab-state-");
    let mut env = doc.env();
    env.insert("PIXELLAB_STATE".into(), state.to_string_lossy().into_owned());
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    Lab { session: Session::new(document, Options { mode, env, ..Options::default() }), doc, state }
}

fn lab(scenes: &[(&str, Value)]) -> Lab {
    lab_with(scenes, Mode::Direct)
}

impl Lab {
    fn call(&mut self, name: &str, params: Value) -> Result<Value, CommandError> {
        registry().call(&mut Ctx::new(&mut self.session), name, params, false).map_err(|e| match e {
            Error::Command(c) => c,
            Error::Internal(m) => CommandError::new(m, "internal"),
        })
    }

    fn ok(&mut self, name: &str, params: Value) -> Value {
        self.call(name, params).unwrap_or_else(|e| panic!("{name}: {}", e.message))
    }

    fn scene(&mut self, name: &str) -> Value {
        Ctx::new(&mut self.session).doc(DocKind::Scene, name).unwrap()
    }

    fn cli(&self, args: &[&str]) -> (i32, String, String) {
        let reg = registry();
        let mut argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        argv.push("--document".into());
        argv.push(self.doc.root.to_string_lossy().into_owned());
        let mut env = self.doc.env();
        env.insert("PIXELLAB_STATE".into(), self.state.to_string_lossy().into_owned());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = {
            let mut io = Io { stdout: &mut out, stderr: &mut err, env, cwd: self.doc.root.clone(), registry: &reg };
            main(&argv, &mut io)
        };
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }
}

fn block_item(col: i64, row: i64, level: i64) -> Value {
    json!({ "object": "block", "at": { "tile": [col, row, level] } })
}

fn as_box(v: &Value) -> Vec<i64> {
    v.as_array().unwrap().iter().map(|n| n.as_i64().unwrap()).collect()
}

// ------------------------------------------------------------------ R7

#[test]
fn r7_scene_render_list_and_show_report_the_resolved_box() {
    let mut lab = lab(&[
        ("fit", json!({ "origin": [0, 0], "items": [block_item(0, 0, 0), block_item(4, 0, 0)] })),
        ("fixed", json!({ "viewBox": [-10, -10, 50, 40], "items": [block_item(0, 0, 0)] })),
    ]);
    let sources = Ctx::new(&mut lab.session).sources().unwrap();
    let want = engine::engine::resolve_frame(&sources.sources, "fit").unwrap();
    assert!(!want.fixed);

    let render = lab.ok("scene.render", json!({ "name": "fit" }));
    assert_eq!(as_box(&render["viewBox"]), want.view_box.to_vec());
    assert_eq!(render["frame"], "fit");
    assert_eq!((render["width"].as_i64().unwrap(), render["height"].as_i64().unwrap()), (want.view_box[2], want.view_box[3]));

    let render = lab.ok("scene.render", json!({ "name": "fixed", "scale": 2 }));
    assert_eq!(as_box(&render["viewBox"]), vec![-10, -10, 50, 40]);
    assert_eq!(render["frame"], "fixed");
    assert_eq!((render["width"].as_i64().unwrap(), render["height"].as_i64().unwrap()), (100, 80));

    let show = lab.ok("scene.show", json!({ "name": "fit" }));
    assert_eq!(as_box(&show["viewBox"]), want.view_box.to_vec());
    assert_eq!(show["frame"], "fit");
    let show = lab.ok("scene.show", json!({ "name": "fixed" }));
    assert_eq!(show["frame"], "fixed");

    let list = lab.ok("scene.list", json!({}));
    let find = |name: &str| list["scenes"].as_array().unwrap().iter().find(|s| s["name"] == name).unwrap().clone();
    assert_eq!(as_box(&find("fit")["viewBox"]), want.view_box.to_vec());
    assert_eq!(find("fit")["frame"], "fit");
    assert_eq!(find("fixed")["frame"], "fixed");

    let (code, human, err) = lab.cli(&["scene", "list"]);
    assert_eq!(code, 0, "{err}");
    let size = format!("{}×{}", want.view_box[2], want.view_box[3]);
    assert!(human.lines().any(|l| l.starts_with("fit ") && l.contains(&format!("fits {size}"))), "{human}");
    assert!(human.lines().any(|l| l.starts_with("fixed ") && l.contains("fixed 50×40")), "{human}");
    let (_, human, _) = lab.cli(&["scene", "show", "fit"]);
    assert!(human.lines().any(|l| l == format!("size      fits {size}")), "{human}");
}

#[test]
fn r7_the_render_view_is_a_window_of_the_scene() {
    let mut lab = lab(&[("fit", json!({ "items": [block_item(0, 0, 0)] }))]);
    let whole = lab.ok("scene.render", json!({ "name": "fit" }));
    let [x, y, w, h] = as_box(&whole["viewBox"])[..] else { panic!() };
    // A window bigger than the scene on every side: the art sits inside it at the right offset.
    let window = lab.ok("scene.render", json!({ "name": "fit", "view": [x - 20, y - 10, w + 40, h + 30] }));
    assert_eq!(as_box(&window["viewBox"]), vec![x - 20, y - 10, w + 40, h + 30]);
    assert_eq!((window["width"].as_i64().unwrap(), window["height"].as_i64().unwrap()), (w + 40, h + 30));
    assert_eq!(window["frame"], "fit");
    let bounds_whole = as_box(&whole["items"][0]["bounds"]);
    let bounds_window = as_box(&window["items"][0]["bounds"]);
    assert_eq!(bounds_window, vec![bounds_whole[0] + 20, bounds_whole[1] + 10, bounds_whole[2], bounds_whole[3]]);
    // A window can be empty space.
    let far = lab.ok("scene.render", json!({ "name": "fit", "view": [5000, 5000, 64, 32] }));
    assert_eq!((far["width"].as_i64().unwrap(), far["height"].as_i64().unwrap()), (64, 32));
    // A bad window is a usage error.
    assert_eq!(lab.call("scene.render", json!({ "name": "fit", "view": [0, 0, 0, 5] })).unwrap_err().exit_code, 2);
    assert_eq!(lab.call("scene.render", json!({ "name": "fit", "view": [0, 0, 100000, 100000] })).unwrap_err().exit_code, 2);
}

#[test]
fn r7_place_with_no_position_starts_at_the_middle_of_the_content_or_the_origin_tile() {
    let mut lab = lab(&[("empty", json!({ "items": [] })), ("full", json!({ "items": [block_item(6, 2, 0)] }))]);
    lab.ok("scene.place", json!({ "name": "empty", "object": "pebble" }));
    assert_eq!(lab.scene("empty")["items"][0]["at"], json!({ "tile": [0, 0, 0] }));
    lab.ok("scene.place", json!({ "name": "full", "object": "pebble" }));
    let placed = lab.scene("full")["items"].as_array().unwrap().iter().find(|i| i["object"] == "pebble").unwrap().clone();
    // The middle of the block at 6,2 is that block's own tile.
    let tile = as_box(&placed["at"]["tile"]);
    assert!((tile[0] - 6).abs() <= 1 && (tile[1] - 2).abs() <= 1 && tile[2] >= 0, "{tile:?}");
}
