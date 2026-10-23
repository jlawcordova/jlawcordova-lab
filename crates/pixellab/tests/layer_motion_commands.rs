//! Command-level tests of the scene layer and motion commands (U23) that the
//! other suites leave to the commands: scene.layer.move's one-step move and
//! the refusals of the layer commands. Their Node sources are
//! cli/test/layers.test.mjs (R5) and cli/test/motion-commands.test.mjs.

#![allow(clippy::result_large_err)]

mod common;

use pixellab::context::{Ctx, DocKind};
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::Error;
use pixellab::registry::registry;
use pixellab::session::{Options, Session};
use serde_json::{json, Map, Value};

fn yard() -> Value {
    let wrapper = |c: i64| {
        json!({
            "group": { "class": "enter", "data-enter": "ground", "data-col": c.to_string(), "data-row": "0", "data-x": (c * 16).to_string(), "data-y": (c * 8).to_string() },
            "items": [{ "object": "block", "at": { "tile": [c, 0, 0] } }],
        })
    };
    json!({
        "output": "yard.svg",
        "viewBox": [-48, -32, 128, 96],
        "items": [
            wrapper(0),
            wrapper(1),
            { "object": "pebble", "at": { "tile": [0, 0, 1] } },
            { "object": "pebble", "id": "rock", "at": { "tile": [1, 0, 1] } },
            { "object": "pebble", "at": { "px": [8, 20] } },
        ],
        "motion": { "duration": 1000, "tracks": [{ "item": "rock", "keys": [{ "t": 0 }, { "t": 500, "move": [2, 0] }] }] },
    })
}

/// `yard` with layers: Ground (the cubes), Stones (the pebbles) and Rock inside Stones.
fn layered() -> Value {
    let mut s = yard();
    let mut map = Map::new();
    for (k, v) in s.as_object().unwrap() {
        if k == "items" {
            map.insert(
                "layers".into(),
                json!([{ "id": "ground", "name": "Ground" }, { "id": "stones", "name": "Stones" }, { "id": "rock", "name": "Rock", "parent": "stones" }]),
            );
        }
        map.insert(k.clone(), v.clone());
    }
    s = Value::Object(map);
    s["items"][0]["items"][0] = json!({ "object": "block", "layer": "ground", "at": { "tile": [0, 0, 0] } });
    s["items"][1]["items"][0] = json!({ "object": "block", "layer": "ground", "at": { "tile": [1, 0, 0] } });
    s["items"][2] = json!({ "object": "pebble", "layer": "stones", "at": { "tile": [0, 0, 1] } });
    s["items"][3] = json!({ "object": "pebble", "id": "rock", "layer": "rock", "at": { "tile": [1, 0, 1] } });
    s
}

struct Fx {
    doc: common::TempDocument,
    state: std::path::PathBuf,
}

impl Fx {
    fn new() -> Fx {
        let doc = common::make_document();
        let state = common::temp_dir("pixellab-state-");
        doc.write("scenes/layered.json", &engine::format::stringify_pretty(&layered()));
        Fx { doc, state }
    }

    fn session(&self) -> Session {
        let mut env = self.doc.env();
        env.insert("PIXELLAB_STATE".into(), self.state.to_string_lossy().into_owned());
        let document = open_document(&OpenOptions {
            doc: Some(self.doc.root.to_string_lossy().into_owned()),
            site: None,
            env: env.clone(),
            cwd: self.doc.root.clone(),
        })
        .unwrap();
        Session::new(document, Options { env, agent: Some("test".into()), ..Options::default() })
    }
}

fn call(s: &mut Session, name: &str, params: Value) -> Result<Value, Error> {
    let mut p = json!({ "name": "layered" });
    for (k, v) in params.as_object().unwrap() {
        p[k] = v.clone();
    }
    let mut ctx = Ctx::new(s);
    registry().call(&mut ctx, name, p, false)
}

fn message(r: Result<Value, Error>) -> (i32, String) {
    match r.expect_err("an error") {
        Error::Command(c) => (c.exit_code, c.message),
        Error::Internal(m) => (-1, m),
    }
}

#[test]
fn r5_scene_layer_move_moves_every_item_in_the_layers_and_inside_them_and_the_items_once() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = call(&mut s, "scene.layer.move", json!({ "layer": "stones", "items": ["2", "4"], "by": [1, -1] })).unwrap();
    assert_eq!(
        [r["moved"].clone(), r["by"].clone(), r["summary"].clone()],
        [json!(3), json!([1, -1, 0]), json!("Moved 3 items +1 col, -1 row")]
    );
    let scene = s.doc(DocKind::Scene, "layered").unwrap();
    let ats: Vec<Value> = scene["items"].as_array().unwrap()[2..].iter().map(|i| i["at"].clone()).collect();
    assert_eq!(ats, [json!({ "tile": [1, -1, 1] }), json!({ "tile": [2, -1, 1] }), json!({ "px": [8 + 32, 20] })]);
    assert_eq!(scene["motion"]["tracks"][0]["keys"][1]["move"], json!([2, 0]));
}

#[test]
fn r5_a_level_move_names_the_groups_that_followed() {
    let fx = Fx::new();
    let mut s = fx.session();
    let level = call(&mut s, "scene.layer.move", json!({ "layer": "Ground", "by": [0, 0, 1] })).unwrap();
    assert_eq!(level["summary"], json!("Moved Ground 0 col, 0 row, +1 level; 2 groups followed"));
    assert_eq!(level["groups"].as_array().unwrap().len(), 2);
}

#[test]
fn r5_the_layer_commands_refuse_a_call_that_names_nothing() {
    let fx = Fx::new();
    let mut s = fx.session();
    assert_eq!(
        message(call(&mut s, "scene.layer.assign", json!({ "to": "stones" }))),
        (2, "scene.layer.assign: give --items, --layers or both".to_string())
    );
    assert_eq!(
        message(call(&mut s, "scene.layer.move", json!({ "by": [1, 0] }))),
        (2, "scene.layer.move: give a layer, --layers or --items".to_string())
    );
}
