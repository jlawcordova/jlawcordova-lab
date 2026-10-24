//! The state commands (docs/intents/2026-10-object-states/spec.md, D3, R4, R8, R15, R23, R30, R34), through the registry on a temp
//! copy of fixtures/states.pixellab: editing a sprite's states, the frame edits that keep them in step, making states of a
//! sprite's loops, the scene's states, and every new command in all three doors. Nothing here writes to the fixture.

#![allow(clippy::result_large_err)]

mod common;

use std::fs;
use std::path::PathBuf;

use pixellab::context::{Ctx, DocKind};
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::Error;
use pixellab::registry::registry;
use pixellab::session::{Options, Session};
use serde_json::{json, Value};

/// A temp copy of the states fixture, with a state folder and an empty HOME.
struct Fx {
    base: PathBuf,
    root: PathBuf,
    home: PathBuf,
    state: PathBuf,
}

impl Fx {
    fn new() -> Fx {
        let base = common::temp_dir("pixellab-states-");
        let (root, home, state) = (base.join("states.pixellab"), base.join("home"), base.join("state"));
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&state).unwrap();
        common::copy_dir(&common::repo().join("fixtures/states.pixellab"), &root);
        Fx { base, root, home, state }
    }

    fn write(&self, file: &str, value: &Value) {
        common::write(&self.root, file, &engine::format::serialize_file(file, value));
    }

    fn read(&self, file: &str) -> Value {
        serde_json::from_str(&fs::read_to_string(self.root.join(file)).unwrap()).unwrap()
    }

    fn env(&self) -> std::collections::HashMap<String, String> {
        let mut env = std::collections::HashMap::from([("HOME".to_string(), self.home.to_string_lossy().into_owned())]);
        env.insert("PIXELLAB_STATE".into(), self.state.to_string_lossy().into_owned());
        env
    }

    fn session(&self) -> Session {
        let env = self.env();
        let document = open_document(&OpenOptions {
            doc: Some(self.root.to_string_lossy().into_owned()),
            site: None,
            env: env.clone(),
            cwd: self.root.clone(),
        })
        .unwrap();
        Session::new(document, Options { env, agent: Some("test".into()), ..Options::default() })
    }
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn call(s: &mut Session, name: &str, params: Value) -> Result<Value, Error> {
    let mut ctx = Ctx::new(s);
    registry().call(&mut ctx, name, params, false)
}

fn ok(s: &mut Session, name: &str, params: Value) -> Value {
    call(s, name, params).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The code of a command's refusal, or its message when it has none.
fn refused(r: Result<Value, Error>) -> (String, String) {
    match r.expect_err("a refusal") {
        Error::Command(c) => (c.code, c.message),
        Error::Internal(m) => ("internal".into(), m),
    }
}

fn object(s: &mut Session, name: &str) -> Value {
    s.doc(DocKind::Object, name).unwrap()
}

fn scene(s: &mut Session, name: &str) -> Value {
    s.doc(DocKind::Scene, name).unwrap()
}

fn problems(s: &mut Session) -> Vec<String> {
    let r = ok(s, "check", json!({}));
    r["problems"].as_array().unwrap().iter().map(|p| p["text"].as_str().unwrap().to_string()).collect()
}

// ----------------------------------------------------------------------------- R4

/// A sprite with a frame loop of `n` frames named `loop_name`, or a still one (n 0).
fn sprite(loop_name: &str, n: usize) -> Value {
    let frame = |i: usize| json!(if i % 2 == 0 { ["a.", ".a"] } else { [".a", "a."] });
    let layer = if n == 0 {
        json!({ "map": ["a.", ".a"] })
    } else {
        json!({ "loop": loop_name, "prefix": "f", "frames": (0..n).map(frame).collect::<Vec<_>>() })
    };
    json!({ "kind": "sprite", "anchor": [0, 1], "keys": { "a": "ink" }, "layers": [layer] })
}

#[test]
fn r4_the_first_state_is_made_from_the_frames_as_they_play_now() {
    let fx = Fx::new();
    for (name, loop_name, n) in [("fast", "ff", 4), ("slow", "wf", 5), ("long", "hf", 6), ("other", "zz", 3), ("still", "", 0)] {
        fx.write(&format!("objects/{name}.json"), &sprite(loop_name, n));
    }
    let mut s = fx.session();
    // ff plays 4 frames in 600 ms, wf 5 in 1 s, hf 6 in 3 s, and any other loop 200 ms a frame; a still sprite has one frame at 200 ms.
    for (name, frames, ms) in [("fast", 4, 150), ("slow", 5, 200), ("long", 6, 500), ("other", 3, 200), ("still", 1, 200)] {
        let r = ok(&mut s, "object.state.add", json!({ "name": name, "state": "idle" }));
        assert_eq!(r["start"], true, "the first state is the start");
        let o = object(&mut s, name);
        assert_eq!(o["start"], "idle");
        assert_eq!(
            o["states"]["idle"],
            json!({ "frames": (0..frames).collect::<Vec<_>>(), "ms": ms }),
            "{name}: it loops, every frame, at its loop's timing"
        );
    }
    // The art plays the same: a render at a time shows the frame the loop shows.
    let r = ok(&mut s, "object.render", json!({ "name": "fast", "time": 460 }));
    assert_eq!(r["items"][0]["frame"], 3);
    assert_eq!(problems(&mut s), Vec::<String>::new());
}

#[test]
fn d3_a_state_is_added_after_another_and_a_one_shot_hands_over() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = ok(&mut s, "object.state.add", json!({ "name": "lamp", "state": "dim", "frames": [1], "ms": 300, "after": "idle" }));
    assert_eq!(r["states"], json!(["idle", "dim", "flicker"]));
    assert_eq!(r["start"], false);
    let r = ok(
        &mut s,
        "object.state.add",
        json!({ "name": "lamp", "state": "burst", "frames": [0, 1], "ms": 40, "once": true, "then": "dim" }),
    );
    assert_eq!(r["states"], json!(["idle", "dim", "flicker", "burst"]));
    assert_eq!(object(&mut s, "lamp")["states"]["burst"], json!({ "frames": [0, 1], "ms": 40, "loop": false, "then": "dim" }));
    assert_eq!(problems(&mut s), Vec::<String>::new());
}

#[test]
fn d3_adding_a_state_is_refused_when_it_would_break_a_rule() {
    let fx = Fx::new();
    let mut s = fx.session();
    let add = |s: &mut Session, params: Value| refused(call(s, "object.state.add", params));
    assert_eq!(add(&mut s, json!({ "name": "lamp", "state": "idle" })).0, "state_exists");
    assert_eq!(add(&mut s, json!({ "name": "lamp", "state": "Two Words" })).0, "usage");
    assert!(add(&mut s, json!({ "name": "lamp", "state": "clear" })).1.contains("--state clear"));
    assert_eq!(add(&mut s, json!({ "name": "lamp", "state": "x", "frames": [2] })).1, "there's no frame 2; the sprite has 2 (0 to 1)");
    assert_eq!(add(&mut s, json!({ "name": "lamp", "state": "x", "ms": 19 })).0, "usage");
    assert_eq!(add(&mut s, json!({ "name": "lamp", "state": "x", "then": "idle" })).0, "usage");
    assert_eq!(
        add(&mut s, json!({ "name": "lamp", "state": "x", "once": true, "then": "nope" })).1,
        "lamp has no state nope (it has idle, flicker)"
    );
    assert_eq!(add(&mut s, json!({ "name": "lamp", "state": "x", "frames": vec![0; 100], "ms": 10000 })).0, "usage", "over 600000 ms");
    assert_eq!(add(&mut s, json!({ "name": "block", "state": "x" })).0, "not_editable");
    // A sprite that extends another has the base's states.
    s.create(DocKind::Object, "lamp-hat", json!({ "kind": "sprite", "extends": "lamp" }), "made");
    assert!(add(&mut s, json!({ "name": "lamp-hat", "state": "x" })).1.contains("extends lamp and has its states"));
    for i in 0..30 {
        ok(&mut s, "object.state.add", json!({ "name": "lamp", "state": format!("more-{i}") }));
    }
    assert_eq!(add(&mut s, json!({ "name": "lamp", "state": "one-too-many" })).0, "too_many");
    assert_eq!(object(&mut s, "lamp")["states"].as_object().unwrap().len(), 32);
}

// ----------------------------------------------------------------------- set, delete

#[test]
fn d3_a_state_is_changed_in_place() {
    let fx = Fx::new();
    let mut s = fx.session();
    let set = |s: &mut Session, params: Value| {
        let mut p = json!({ "name": "walker", "state": "hop" });
        for (k, v) in params.as_object().unwrap() {
            p[k] = v.clone();
        }
        call(s, "object.state.set", p)
    };
    ok(&mut s, "object.state.set", json!({ "name": "walker", "state": "walk", "ms": 100, "frames": [0, 1] }));
    assert_eq!(object(&mut s, "walker")["states"]["walk"], json!({ "frames": [0, 1], "ms": 100 }));
    set(&mut s, json!({ "hold": true })).unwrap();
    assert_eq!(object(&mut s, "walker")["states"]["hop"]["loop"], false);
    assert!(object(&mut s, "walker")["states"]["hop"].get("then").is_none(), "a hold has no then");
    set(&mut s, json!({ "then": "walk" })).unwrap();
    assert_eq!(object(&mut s, "walker")["states"]["hop"]["then"], "walk");
    set(&mut s, json!({ "loop": true })).unwrap();
    let hop = object(&mut s, "walker")["states"]["hop"].clone();
    assert!(hop.get("loop").is_none() && hop.get("then").is_none(), "{hop}");
    ok(&mut s, "object.state.set", json!({ "name": "walker", "state": "walk", "start": true }));
    assert_eq!(object(&mut s, "walker")["start"], "walk");
    // Refusals.
    assert_eq!(refused(set(&mut s, json!({ "then": "walk" }))).0, "usage", "then on a looping state");
    assert_eq!(refused(set(&mut s, json!({ "loop": true, "once": true }))).0, "usage");
    assert_eq!(refused(set(&mut s, json!({ "rename": "walk" }))).0, "state_exists");
    assert_eq!(refused(set(&mut s, json!({}))).0, "usage");
    // hop's move has a key at 44 ms: a state shorter than that cuts it off.
    assert_eq!(refused(set(&mut s, json!({ "ms": 20, "frames": [1] }))).0, "out_of_range");
    assert_eq!(problems(&mut s), Vec::<String>::new());
}

#[test]
fn d3_renaming_a_state_follows_it_through_every_scene_as_one_version() {
    let fx = Fx::new();
    // A second scene that names the walker's states too.
    let mut second = fx.read("scenes/states.json");
    second["output"] = json!("states-two.svg");
    second["items"] = json!([{ "object": "walker", "state": "walk", "at": { "tile": [0, 0, 0] } }]);
    second.as_object_mut().unwrap().shift_remove("motion");
    fx.write("scenes/states-two.json", &second);
    let mut s = fx.session();
    // (The history starts with the document as it was, when it first changes; an edit that is its own version makes that so.)
    ok(&mut s, "object.state.add", json!({ "name": "walker", "state": "extra" }));
    ok(&mut s, "doc.save", json!({ "doc": "object:walker" }));
    let before = ok(&mut s, "history.list", json!({}))["total"].as_u64().unwrap();
    let r = ok(&mut s, "object.state.set", json!({ "name": "walker", "state": "walk", "rename": "stroll" }));
    assert_eq!(r["state"], "stroll");
    assert_eq!(r["scenes"], json!([{ "scene": "states", "changed": 1 }, { "scene": "states-two", "changed": 1 }]), "{r}");
    let list = ok(&mut s, "history.list", json!({}));
    assert_eq!(list["total"].as_u64().unwrap(), before + 1, "one version");
    let version = &list["versions"][0];
    let files: Vec<&str> = version["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect();
    assert!(
        files.contains(&"objects/walker.json") && files.contains(&"scenes/states.json") && files.contains(&"scenes/states-two.json"),
        "{files:?}"
    );
    assert_eq!(
        fx.read("objects/walker.json")["states"].as_object().unwrap().keys().collect::<Vec<_>>(),
        ["idle", "stroll", "dead", "hop", "ping", "pong", "extra"],
        "in its place"
    );
    assert_eq!(fx.read("scenes/states.json")["items"][2]["state"], "stroll");
    assert_eq!(fx.read("scenes/states-two.json")["items"][0]["state"], "stroll");
    assert_eq!(problems(&mut s), Vec::<String>::new());
    // A key's state is renamed too, and `start` and `then` follow in the object.
    let r = ok(&mut s, "object.state.set", json!({ "name": "lamp", "state": "flicker", "rename": "flash" }));
    assert_eq!(r["scenes"], json!([{ "scene": "states", "changed": 1 }]), "only the key at 1000 ms names flicker: {r}");
    let keys = fx.read("scenes/states.json")["motion"]["tracks"][0]["keys"].clone();
    assert_eq!(keys[1]["state"], "flash");
    ok(&mut s, "object.state.set", json!({ "name": "lamp", "state": "idle", "rename": "rest" }));
    let lamp = fx.read("objects/lamp.json");
    assert_eq!(lamp["start"], "rest");
    assert_eq!(lamp["states"]["flash"]["then"], "rest");
    assert_eq!(fx.read("scenes/states.json")["motion"]["tracks"][0]["keys"][2]["state"], "rest");
    assert_eq!(problems(&mut s), Vec::<String>::new());
}

#[test]
fn d3_a_state_that_is_still_named_is_not_deleted_and_the_last_one_takes_the_states_off() {
    let fx = Fx::new();
    let mut s = fx.session();
    let delete =
        |s: &mut Session, name: &str, state: &str| refused(call(s, "object.state.delete", json!({ "name": name, "state": state })));
    let (code, message) = delete(&mut s, "lamp", "flicker");
    assert_eq!(code, "in_use");
    assert!(message.contains("scenes/states.json: lamp-b's key at 1000 ms"), "{message}");
    assert_eq!(delete(&mut s, "lamp", "idle").0, "in_use", "the start, and a hand-over, and a key");
    assert_eq!(delete(&mut s, "walker", "idle").0, "in_use");
    let (_, message) = delete(&mut s, "walker", "pong");
    assert!(message.contains("ping hands over to it"), "{message}");
    // pong is the end of a cycle; once the scene and the then let go, it goes.
    ok(&mut s, "object.state.set", json!({ "name": "walker", "state": "ping", "hold": true }));
    ok(&mut s, "object.state.delete", json!({ "name": "walker", "state": "pong" }));
    assert!(object(&mut s, "walker")["states"].get("pong").is_none());
    // A sprite's last state takes states and start off, and its frame loops play on their own again.
    fx.write("objects/solo.json", &sprite("ff", 4));
    let mut s = fx.session();
    ok(&mut s, "object.state.add", json!({ "name": "solo", "state": "only" }));
    ok(&mut s, "object.state.delete", json!({ "name": "solo", "state": "only" }));
    let o = object(&mut s, "solo");
    assert!(o.get("states").is_none() && o.get("start").is_none());
}

#[test]
fn d3_a_state_is_duplicated_after_itself_and_moved_in_the_list() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = ok(&mut s, "object.state.duplicate", json!({ "name": "walker", "state": "walk" }));
    assert_eq!(r["state"], "walk-2");
    assert_eq!(r["states"], json!(["idle", "walk", "walk-2", "dead", "hop", "ping", "pong"]));
    let r = ok(&mut s, "object.state.duplicate", json!({ "name": "walker", "state": "walk", "as": "jog" }));
    assert_eq!(r["states"], json!(["idle", "walk", "jog", "walk-2", "dead", "hop", "ping", "pong"]));
    assert_eq!(
        refused(call(&mut s, "object.state.duplicate", json!({ "name": "walker", "state": "walk", "as": "idle" }))).0,
        "state_exists"
    );
    let r = ok(&mut s, "object.state.move", json!({ "name": "walker", "state": "jog", "by": 3 }));
    assert_eq!(r["states"], json!(["idle", "walk", "walk-2", "dead", "hop", "jog", "ping", "pong"]));
    assert_eq!(refused(call(&mut s, "object.state.move", json!({ "name": "walker", "state": "idle", "by": -1 }))).0, "out_of_range");
}

// --------------------------------------------------------------- a state's frames and move

#[test]
fn r27_a_states_frames_are_added_moved_and_deleted_by_position() {
    let fx = Fx::new();
    let mut s = fx.session();
    let walk = |s: &mut Session| object(s, "walker")["states"]["walk"]["frames"].clone();
    ok(&mut s, "object.state.frame.add", json!({ "name": "walker", "state": "walk", "frame": 2, "at": 1 }));
    assert_eq!(walk(&mut s), json!([0, 2, 1, 2, 1]));
    ok(&mut s, "object.state.frame.add", json!({ "name": "walker", "state": "walk" }));
    assert_eq!(walk(&mut s), json!([0, 2, 1, 2, 1, 1]), "by default a copy of the frame it ends on, last");
    let r = ok(&mut s, "object.state.frame.move", json!({ "name": "walker", "state": "walk", "at": 0, "by": 2 }));
    assert_eq!(r["at"], 2);
    assert_eq!(walk(&mut s), json!([2, 1, 0, 2, 1, 1]));
    ok(&mut s, "object.state.frame.delete", json!({ "name": "walker", "state": "walk", "at": 5 }));
    assert_eq!(walk(&mut s), json!([2, 1, 0, 2, 1]));
    assert_eq!(refused(call(&mut s, "object.state.frame.add", json!({ "name": "walker", "state": "walk", "frame": 3 }))).0, "usage");
    assert_eq!(refused(call(&mut s, "object.state.frame.delete", json!({ "name": "walker", "state": "walk", "at": 9 }))).0, "usage");
    assert_eq!(refused(call(&mut s, "object.state.frame.delete", json!({ "name": "walker", "state": "idle", "at": 0 }))).0, "last_frame");
    assert_eq!(
        refused(call(&mut s, "object.state.frame.move", json!({ "name": "walker", "state": "walk", "at": 4, "by": 1 }))).0,
        "out_of_range"
    );
    assert_eq!(problems(&mut s), Vec::<String>::new());
}

#[test]
fn r27_a_states_move_has_keys_that_are_made_pinned_moved_and_deleted() {
    let fx = Fx::new();
    let mut s = fx.session();
    let keys = |s: &mut Session| object(s, "walker")["states"]["walk"]["move"].clone();
    // The first key is at 0: a key made later brings it.
    let r = ok(&mut s, "object.state.key.set", json!({ "name": "walker", "state": "walk", "time": 150, "move": [0, -2] }));
    assert_eq!(r["key"], json!({ "t": 150, "move": [0, -2] }));
    assert_eq!(r["pinned"], false);
    assert_eq!(keys(&mut s), json!([{ "t": 0, "move": [0, 0] }, { "t": 150, "move": [0, -2] }]));
    // With no move, a key pins the sprite where it is then (a walk of 600 ms steps up two pixels in 150 ms, then holds... and back).
    let r = ok(&mut s, "object.state.key.set", json!({ "name": "walker", "state": "walk", "time": 100 }));
    assert_eq!(r["pinned"], true);
    assert_eq!(r["key"]["t"], 100);
    let again = ok(&mut s, "object.state.key.set", json!({ "name": "walker", "state": "walk", "time": 100 }));
    assert_eq!((again["changed"].clone(), again["pinned"].clone()), (json!(false), json!(false)), "a time that has a key is left alone");
    let r = ok(&mut s, "object.state.key.move", json!({ "name": "walker", "state": "walk", "time": 100, "to": 110 }));
    assert_eq!(r["key"]["t"], 110);
    assert_eq!(
        refused(call(&mut s, "object.state.key.move", json!({ "name": "walker", "state": "walk", "time": 0, "to": 5 }))).0,
        "first_key"
    );
    assert_eq!(
        refused(call(&mut s, "object.state.key.move", json!({ "name": "walker", "state": "walk", "time": 110, "to": 150 }))).0,
        "key_exists"
    );
    assert_eq!(
        refused(call(&mut s, "object.state.key.set", json!({ "name": "walker", "state": "walk", "time": 600, "move": [1, 1] }))).0,
        "out_of_range"
    );
    ok(&mut s, "object.state.key.delete", json!({ "name": "walker", "state": "walk", "time": 110 }));
    assert_eq!(refused(call(&mut s, "object.state.key.delete", json!({ "name": "walker", "state": "walk", "time": 0 }))).0, "first_key");
    ok(&mut s, "object.state.key.delete", json!({ "name": "walker", "state": "walk", "time": 150 }));
    ok(&mut s, "object.state.key.delete", json!({ "name": "walker", "state": "walk", "time": 0 }));
    assert!(object(&mut s, "walker")["states"]["walk"].get("move").is_none(), "the last key takes the move off");
    assert_eq!(problems(&mut s), Vec::<String>::new());
}

#[test]
fn r27_the_run_command_reads_what_a_state_plays() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = ok(&mut s, "object.state.run", json!({ "name": "walker", "state": "hop" }));
    assert_eq!(r["segments"], json!([{ "state": "hop", "from": 0, "to": 480 }, { "state": "idle", "from": 480, "to": 1480 }]));
    assert_eq!(r["settle"], json!({ "kind": "loop", "from": 480, "period": 1000 }));
    assert_eq!(r["changes"][0], json!({ "t": 0, "frame": 1, "move": [0, 0] }));
    assert_eq!(r["changes"].as_array().unwrap().len(), 5);
    let dead = ok(&mut s, "object.state.run", json!({ "name": "walker", "state": "dead" }));
    assert_eq!(dead["settle"], json!({ "kind": "hold", "from": 200, "frame": 2, "move": [0, 0] }));
    assert_eq!(refused(call(&mut s, "object.state.run", json!({ "name": "walker", "state": "fly" }))).0, "not_found");
    assert_eq!(refused(call(&mut s, "object.state.run", json!({ "name": "pebble", "state": "idle" }))).0, "no_states");
}

// ------------------------------------------------------------------ R8: states make

/// A sprite shaped like the site's character: layer 0 empty, layer 1 painted, its moves on layer 1.
fn character_like() -> Value {
    json!({
        "kind": "sprite", "anchor": [2, 3], "keys": { "a": "ink", "b": "grass-2" },
        "layers": [
            { "map": ["....", "....", "....", "...."] },
            {
                "map": ["abba", "baab", "baab", "abba"],
                "ambient": { "duration": 1600, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 800, "move": [0, -1] }] },
                "actions": {
                    "hop": { "duration": 480, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 120, "move": [0, -4] }, { "t": 240, "move": [0, -6] }, { "t": 360, "move": [0, -4] }] },
                    "wave": { "duration": 600, "keys": [{ "t": 0, "move": [1, 0] }] }
                }
            }
        ]
    })
}

#[test]
fn r8_the_ambient_loop_and_actions_become_states_and_the_art_stays_the_same() {
    let fx = Fx::new();
    fx.write("objects/character-like.json", &character_like());
    let mut s = fx.session();
    let render = |s: &mut Session, params: Value| {
        let mut p = json!({ "name": "character-like", "scale": 1 });
        for (k, v) in params.as_object().unwrap() {
            p[k] = v.clone();
        }
        let r = ok(s, "object.render", p);
        (r["png"].clone(), r["viewBox"].clone())
    };
    let ambient_before: Vec<(Value, Value)> = [0, 700, 800, 1500, 1700].iter().map(|t| render(&mut s, json!({ "time": t }))).collect();
    let hop_before: Vec<(Value, Value)> =
        [0, 130, 250, 370, 470].iter().map(|t| render(&mut s, json!({ "time": 100, "play": format!("hop@{t}") }))).collect();
    let r = ok(&mut s, "object.states.make", json!({ "name": "character-like" }));
    assert_eq!(r["states"], json!(["idle", "hop", "wave"]));
    let o = object(&mut s, "character-like");
    assert_eq!(o["start"], "idle");
    assert_eq!(
        o["states"]["idle"],
        json!({ "frames": [0], "ms": 1600, "move": [{ "t": 0, "move": [0, 0] }, { "t": 800, "move": [0, -1] }] })
    );
    assert_eq!(o["states"]["hop"]["frames"], json!([0]));
    assert_eq!(
        (o["states"]["hop"]["ms"].clone(), o["states"]["hop"]["loop"].clone(), o["states"]["hop"]["then"].clone()),
        (json!(480), json!(false), json!("idle"))
    );
    assert!(o["layers"][1].get("ambient").is_none() && o["layers"][1].get("actions").is_none(), "the layer's moves are the states' now");
    assert_eq!(problems(&mut s), Vec::<String>::new());
    // The art is the same: the ambient loop at each time, and the hop at each time into it.
    let ambient_after: Vec<(Value, Value)> = [0, 700, 800, 1500, 1700].iter().map(|t| render(&mut s, json!({ "time": t }))).collect();
    assert_eq!(ambient_before, ambient_after, "idle plays the ambient loop");
    let hop_after: Vec<(Value, Value)> =
        [0, 130, 250, 370, 470].iter().map(|t| render(&mut s, json!({ "state": "hop", "time": t }))).collect();
    assert_eq!(
        hop_before.iter().map(|r| &r.0).collect::<Vec<_>>(),
        hop_after.iter().map(|r| &r.0).collect::<Vec<_>>(),
        "hop plays the action"
    );
    // Making states again says there are some.
    assert_eq!(refused(call(&mut s, "object.states.make", json!({ "name": "character-like" }))).0, "has_states");
}

#[test]
fn r8_states_make_refuses_when_it_could_not_keep_the_art_the_same() {
    let fx = Fx::new();
    let mut offset = character_like();
    offset["layers"][1]["ambient"]["offset"] = json!(200);
    let mut two = character_like();
    two["layers"][0] = json!({ "map": ["....", "....", "....", "...."], "ambient": { "duration": 1000, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 500, "move": [1, 0] }] } });
    let mut pixels = character_like();
    pixels["layers"][0] = json!({ "map": ["ab..", "....", "....", "...."] });
    let mut odd = character_like();
    odd["layers"][1]["actions"]["wave"]["duration"] = json!(30000);
    let mut looped = sprite("ff", 4);
    looped["layers"][0]["frames"] = json!([["a."], ["a."], ["a."], ["a."]]);
    looped["layers"][0]["ambient"] = json!({ "duration": 700, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 350, "move": [0, -1] }] });
    // (A frame layer has no ambient: write it as a map layer beside the loop.)
    let mut beside = sprite("ff", 4);
    beside["layers"][0]["frames"] = json!([["..", ".."], ["..", ".."], ["..", ".."], ["..", ".."]]);
    beside["layers"].as_array_mut().unwrap().push(
        json!({ "map": ["a."], "ambient": { "duration": 700, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 350, "move": [0, -1] }] } }),
    );
    for (name, doc) in [("offset", offset), ("two", two), ("pixels", pixels), ("odd", odd), ("beside", beside)] {
        fx.write(&format!("objects/{name}.json"), &doc);
    }
    let mut s = fx.session();
    let make = |s: &mut Session, name: &str| refused(call(s, "object.states.make", json!({ "name": name })));
    let before = (object(&mut s, "offset"), object(&mut s, "two"));
    assert_eq!(make(&mut s, "offset").0, "ambient_offset");
    assert!(make(&mut s, "offset").1.contains("starts 200 ms in"));
    assert_eq!(make(&mut s, "two").0, "moves_on_many_layers");
    assert_eq!(make(&mut s, "pixels").0, "moves_other_layers");
    assert_eq!(make(&mut s, "odd").0, "duration");
    // 700 ms is not a whole number of the frame loop's 600 ms cycles.
    assert_eq!(make(&mut s, "beside").0, "duration");
    assert_eq!(make(&mut s, "pebble").0, "nothing_to_make");
    assert_eq!(make(&mut s, "block").0, "not_editable");
    assert_eq!((object(&mut s, "offset"), object(&mut s, "two")), before, "a refusal changes nothing");
}

// ------------------------------------------------------------------- R30: frames follow

/// A sprite with two frame loops of 4 frames and states that play some of them.
fn frames_fx() -> Fx {
    let fx = Fx::new();
    let mut o = sprite("ff", 4);
    o["layers"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "loop": "gg", "prefix": "g", "frames": [["b.", ".b"], ["b.", ".b"], ["b.", ".b"], ["b.", ".b"]] }));
    o["keys"]["b"] = json!("grass-2");
    o["start"] = json!("a");
    o["states"] = json!({
        "a": { "frames": [0, 1, 2, 3], "ms": 100 },
        "b": { "frames": [3, 1], "ms": 100, "loop": false },
        "c": { "frames": [2], "ms": 100 }
    });
    fx.write("objects/cols.json", &o);
    fx
}

#[test]
fn r30_a_frame_added_or_duplicated_is_added_to_every_loop_and_the_states_after_it_are_renumbered() {
    let fx = frames_fx();
    let mut s = fx.session();
    let r = ok(&mut s, "object.frame.add", json!({ "name": "cols", "frame": 1 }));
    assert_eq!(r["frame"], 2);
    let o = object(&mut s, "cols");
    assert_eq!(o["layers"][0]["frames"].as_array().unwrap().len(), 5);
    assert_eq!(o["layers"][1]["frames"].as_array().unwrap().len(), 5, "every frame loop gets the column");
    assert_eq!(o["states"]["a"]["frames"], json!([0, 1, 3, 4]), "the frames after the new one are renumbered");
    assert_eq!(o["states"]["b"]["frames"], json!([4, 1]));
    assert_eq!(o["states"]["c"]["frames"], json!([3]));
    assert_eq!(r["states"], json!(["a", "b", "c"]));
    assert_eq!(o["layers"][0]["frames"][2], json!(["..", ".."]), "the new frame is blank");
    ok(&mut s, "object.frame.duplicate", json!({ "name": "cols", "frame": 0 }));
    assert_eq!(
        object(&mut s, "cols")["states"]["a"]["frames"],
        json!([0, 2, 4, 5]),
        "a duplicate doesn't change what a state plays, only the numbers"
    );
    assert_eq!(problems(&mut s), Vec::<String>::new());
}

#[test]
fn r30_a_frame_moved_takes_the_states_with_it() {
    let fx = frames_fx();
    let mut s = fx.session();
    let r = ok(&mut s, "object.frame.move", json!({ "name": "cols", "frame": 0, "by": 2 }));
    assert_eq!(r["frame"], 2);
    let o = object(&mut s, "cols");
    assert_eq!(o["states"]["a"]["frames"], json!([2, 0, 1, 3]), "frame 0 is now 2, and 1 and 2 are 0 and 1");
    assert_eq!(o["states"]["b"]["frames"], json!([3, 0]));
    assert_eq!(o["layers"][0]["frames"].as_array().unwrap().len(), 4);
    ok(&mut s, "object.frame.move", json!({ "name": "cols", "frame": 3, "by": -3 }));
    assert_eq!(object(&mut s, "cols")["states"]["a"]["frames"], json!([3, 1, 2, 0]));
    assert_eq!(refused(call(&mut s, "object.frame.move", json!({ "name": "cols", "frame": 3, "by": 1 }))).0, "out_of_range");
}

#[test]
fn r30_a_deleted_frame_leaves_the_states_and_a_state_left_with_none_refuses_the_delete() {
    let fx = frames_fx();
    let mut s = fx.session();
    // Frame 2 is all that c plays.
    let before = object(&mut s, "cols");
    let (code, message) = refused(call(&mut s, "object.frame.delete", json!({ "name": "cols", "frame": 2 })));
    assert_eq!(code, "state_needs_frame");
    assert!(message.contains("state c plays only that frame"), "{message}");
    assert_eq!(object(&mut s, "cols"), before, "nothing changed");
    ok(&mut s, "object.state.frame.add", json!({ "name": "cols", "state": "c", "frame": 0 }));
    let r = ok(&mut s, "object.frame.delete", json!({ "name": "cols", "frame": 2 }));
    assert_eq!(r["states"], json!(["a", "b", "c"]));
    let o = object(&mut s, "cols");
    assert_eq!(o["states"]["a"]["frames"], json!([0, 1, 2]), "frame 2 left it, 3 became 2");
    assert_eq!(o["states"]["b"]["frames"], json!([2, 1]));
    assert_eq!(o["states"]["c"]["frames"], json!([0]));
    assert_eq!(o["layers"][1]["frames"].as_array().unwrap().len(), 3);
    assert_eq!(problems(&mut s), Vec::<String>::new());
}

#[test]
fn r30_layer_edits_never_touch_states_and_still_follows_the_last_loop() {
    let fx = frames_fx();
    let mut s = fx.session();
    let states = object(&mut s, "cols")["states"].clone();
    ok(&mut s, "object.layer.add", json!({ "name": "cols" }));
    ok(&mut s, "object.layer.move", json!({ "name": "cols", "layer": 2, "by": -1 }));
    ok(&mut s, "object.layer.delete", json!({ "name": "cols", "layer": 1 }));
    assert_eq!(object(&mut s, "cols")["states"], states, "layers organize the painting; a move is the whole sprite's");
    // Turning a loop still while another loop remains leaves the frames; the last one leaves one frame.
    let fx2 = frames_fx();
    let mut s2 = fx2.session();
    ok(&mut s2, "object.still", json!({ "name": "cols", "layer": 1, "frame": 0 }));
    assert_eq!(object(&mut s2, "cols")["states"], states, "the other loop still has its four frames");
    let (code, message) = refused(call(&mut s2, "object.still", json!({ "name": "cols", "layer": 0, "frame": 2 })));
    assert_eq!(code, "state_needs_frame", "{message}");
    ok(&mut s2, "object.state.frame.add", json!({ "name": "cols", "state": "a", "frame": 2 }));
    ok(&mut s2, "object.state.frame.add", json!({ "name": "cols", "state": "b", "frame": 2 }));
    let r = ok(&mut s2, "object.still", json!({ "name": "cols", "layer": 0, "frame": 2 }));
    assert_eq!(r["states"], json!(["a", "b", "c"]));
    let o = object(&mut s2, "cols");
    assert_eq!(
        (o["states"]["a"]["frames"].clone(), o["states"]["b"]["frames"].clone(), o["states"]["c"]["frames"].clone()),
        (json!([0, 0]), json!([0]), json!([0]))
    );
}

#[test]
fn r30_animating_a_layer_of_a_sprite_with_states_makes_a_loop_as_long_as_the_others() {
    let fx = frames_fx();
    let mut o = fx.read("objects/cols.json");
    o["layers"].as_array_mut().unwrap().push(json!({ "map": ["a.", ".a"] }));
    // Three frames in the other loops: the new ff would have four.
    o["layers"][0]["frames"].as_array_mut().unwrap().pop();
    o["layers"][1]["frames"].as_array_mut().unwrap().pop();
    o["states"] = json!({ "a": { "frames": [0, 1, 2], "ms": 100 } });
    fx.write("objects/cols.json", &o);
    let mut s = fx.session();
    ok(&mut s, "object.animate", json!({ "name": "cols", "layer": 2, "loop": "hf" }));
    let o = object(&mut s, "cols");
    assert_eq!(o["layers"][2]["frames"].as_array().unwrap().len(), 3);
    assert_eq!(o["layers"][2]["loop"], "hf");
    assert_eq!(problems(&mut s), Vec::<String>::new());
    // Switching a loop keeps its length.
    ok(&mut s, "object.animate", json!({ "name": "cols", "layer": 0, "loop": "wf" }));
    assert_eq!(object(&mut s, "cols")["layers"][0]["frames"].as_array().unwrap().len(), 3);
    assert_eq!(problems(&mut s), Vec::<String>::new());
}

// ----------------------------------------------------------------- the scene's states

#[test]
fn r25_an_item_starts_in_a_state_it_names_and_start_puts_it_back() {
    let fx = Fx::new();
    let mut s = fx.session();
    let items = ok(&mut s, "scene.items", json!({ "name": "states" }));
    let walker = items["items"].as_array().unwrap().iter().find(|i| i["state"] == "walk").expect("the walker that starts in walk").clone();
    assert_eq!(walker["object"], "walker");
    let path = walker["path"].as_str().unwrap().to_string();
    let r = ok(&mut s, "scene.set", json!({ "name": "states", "item": path, "state": "hop" }));
    assert_eq!(scene(&mut s, "states")["items"][2]["state"], "hop", "{r}");
    // The object's own start is left out.
    ok(&mut s, "scene.set", json!({ "name": "states", "item": path, "state": "idle" }));
    assert!(scene(&mut s, "states")["items"][2].get("state").is_none());
    ok(&mut s, "scene.set", json!({ "name": "states", "item": path, "state": "dead" }));
    ok(&mut s, "scene.set", json!({ "name": "states", "item": path, "state": "start" }));
    assert!(scene(&mut s, "states")["items"][2].get("state").is_none());
    assert_eq!(refused(call(&mut s, "scene.set", json!({ "name": "states", "item": path, "state": "fly" }))).0, "not_found");
    let pebble = items["items"].as_array().unwrap().iter().find(|i| i["object"] == "pebble").unwrap()["path"].as_str().unwrap().to_string();
    assert_eq!(refused(call(&mut s, "scene.set", json!({ "name": "states", "item": pebble, "state": "idle" }))).0, "no_states");
    // An item's other fields keep its state when they change.
    ok(&mut s, "scene.set", json!({ "name": "states", "item": path, "state": "walk" }));
    ok(&mut s, "scene.set", json!({ "name": "states", "item": path, "id": "stroller" }));
    assert_eq!(scene(&mut s, "states")["items"][2]["state"], "walk");
}

#[test]
fn r15_a_key_changes_the_state_and_with_no_values_it_pins_the_item() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = ok(&mut s, "scene.key.set", json!({ "name": "states", "item": "lamp-b", "time": 2500, "state": "flicker" }));
    assert_eq!(r["key"], json!({ "t": 2500, "state": "flicker" }));
    ok(&mut s, "scene.key.set", json!({ "name": "states", "item": "lamp-b", "time": 2500, "state": "clear" }));
    assert_eq!(scene(&mut s, "states")["motion"]["tracks"][0]["keys"][3], json!({ "t": 2500 }));
    assert_eq!(
        refused(call(&mut s, "scene.key.set", json!({ "name": "states", "item": "lamp-b", "time": 2500, "state": "hop" }))).0,
        "no_state"
    );
    // A state of the pose: the key says walker, so walker's states are the ones that count.
    ok(&mut s, "scene.key.set", json!({ "name": "states", "item": "lamp-b", "time": 2500, "pose": "walker", "state": "hop" }));
    assert_eq!(scene(&mut s, "states")["motion"]["tracks"][0]["keys"][3], json!({ "t": 2500, "pose": "walker", "state": "hop" }));
    assert_eq!(
        refused(call(&mut s, "scene.key.set", json!({ "name": "states", "item": "lamp-b", "time": 2700, "state": "flicker" }))).0,
        "no_state",
        "the pose carries on"
    );
    ok(&mut s, "scene.key.set", json!({ "name": "states", "item": "lamp-b", "time": 2500, "clear": ["pose", "state"] }));
    assert_eq!(problems(&mut s), Vec::<String>::new());
    // Pinning: a key with the item's move at that time, and a time that has a key is left alone.
    let r = ok(&mut s, "scene.key.set", json!({ "name": "states", "item": "lamp-b", "time": 700 }));
    assert_eq!((r["pinned"].clone(), r["key"].clone()), (json!(true), json!({ "t": 700, "move": [0, 0] })));
    let again = ok(&mut s, "scene.key.set", json!({ "name": "states", "item": "lamp-b", "time": 700 }));
    assert_eq!((again["pinned"].clone(), again["changed"].clone()), (json!(false), json!(false)));
    let r = ok(&mut s, "scene.key.set", json!({ "name": "states", "item": "lamp-b", "time": 1000 }));
    assert_eq!((r["pinned"].clone(), r["changed"].clone()), (json!(false), json!(false)), "the key at 1000 ms is the flicker key");
    ok(&mut s, "scene.key.set", json!({ "name": "states", "item": "lamp-b", "time": 800, "move": [4, 2] }));
    let r = ok(&mut s, "scene.key.set", json!({ "name": "states", "item": "lamp-b", "time": 900 }));
    assert_eq!(r["key"], json!({ "t": 900, "move": [4, 2] }), "pinned where it stands then");
}

#[test]
fn r23_an_item_without_an_id_gets_one_from_its_object_made_unique() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = ok(&mut s, "scene.track.add", json!({ "name": "states", "item": "5" }));
    assert_eq!(r["id"], "pebble");
    assert!(r["summary"].as_str().unwrap().contains("the item had no id, so it got pebble"));
    assert_eq!(scene(&mut s, "states")["items"][5]["id"], "pebble");
    // Another walker without an id gets walker-2... walker, since walker-a and the others already carry other ids.
    let items = ok(&mut s, "scene.items", json!({ "name": "states" }));
    let bare = items["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["object"] == "walker" && i.get("id").is_none())
        .map(|i| i["path"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(bare.len(), 2);
    assert_eq!(ok(&mut s, "scene.track.add", json!({ "name": "states", "item": bare[0] }))["id"], "walker");
    assert_eq!(ok(&mut s, "scene.track.add", json!({ "name": "states", "item": bare[1] }))["id"], "walker-2");
    // An item that has an id keeps it.
    assert_eq!(ok(&mut s, "scene.track.add", json!({ "name": "states", "item": "walker-a" }))["id"], "walker-a");
    assert_eq!(problems(&mut s), Vec::<String>::new());
}

// ------------------------------------------------------------------------ object.show

#[test]
fn d3_show_and_list_say_what_states_an_object_has() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = ok(&mut s, "object.show", json!({ "name": "walker" }));
    assert_eq!(r["start"], "idle");
    let states = r["states"].as_array().unwrap();
    assert_eq!(states.iter().map(|s| s["name"].as_str().unwrap()).collect::<Vec<_>>(), ["idle", "walk", "dead", "hop", "ping", "pong"]);
    let hop = &states[3];
    assert_eq!(
        (hop["frames"].clone(), hop["ms"].clone(), hop["length"].clone(), hop["end"].clone(), hop["then"].clone()),
        (json!([1]), json!(480), json!(480), json!("then"), json!("idle"))
    );
    assert_eq!(hop["move"], json!([{ "t": 0, "move": [0, 0] }, { "t": 44, "move": [0, -2] }]));
    assert_eq!(states[2]["end"], "hold");
    assert_eq!(states[0]["end"], "loop");
    let list = ok(&mut s, "object.list", json!({}));
    let walker = list["objects"].as_array().unwrap().iter().find(|o| o["name"] == "walker").unwrap();
    assert_eq!(walker["start"], "idle");
    assert_eq!(walker["states"][3], json!({ "name": "hop", "length": 480 }));
    let pebble = list["objects"].as_array().unwrap().iter().find(|o| o["name"] == "pebble").unwrap();
    assert!(pebble.get("states").is_none());
    assert!(refused(call(&mut s, "object.render", json!({ "name": "walker", "state": "fly" })))
        .1
        .contains("has no state fly (it has idle, walk"));
    assert!(refused(call(&mut s, "object.render", json!({ "name": "pebble", "state": "idle" }))).1.contains("has no states"));
}

// ----------------------------------------------------------------------------- check

#[test]
fn r8_check_warns_about_states_beside_layer_loops_and_r17_about_a_page_switch_that_cannot_reach_a_keyed_item() {
    let fx = Fx::new();
    let mut o = fx.read("objects/lamp.json");
    o["layers"].as_array_mut().unwrap().push(
        json!({ "map": ["a", "a"], "ambient": { "duration": 400, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 200, "move": [1, 0] }] } }),
    );
    // lamp's frames are two rows of four: the map layer must match.
    o["layers"][1]["map"] = json!(["cccc", "....", "....", "...."]);
    fx.write("objects/lamp.json", &o);
    let mut scn = fx.read("scenes/states.json");
    scn["items"].as_array_mut().unwrap().push(json!({
        "group": { "data-state": "flicker" },
        "items": [{ "object": "lamp", "id": "lamp-c", "at": { "tile": [4, 4, 0] } }]
    }));
    scn["motion"]["tracks"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "item": "lamp-c", "keys": [{ "t": 0 }, { "t": 500, "state": "flicker" }] }));
    fx.write("scenes/states.json", &scn);
    let mut s = fx.session();
    let r = ok(&mut s, "check", json!({}));
    assert_eq!(r["problems"], json!([]), "{r}");
    let codes: Vec<(&str, String)> =
        r["warnings"].as_array().unwrap().iter().map(|w| (w["code"].as_str().unwrap(), w["text"].as_str().unwrap().to_string())).collect();
    let find = |code: &str| codes.iter().find(|(c, _)| *c == code).map(|(_, t)| t.clone());
    assert!(
        find("states_and_loops").unwrap().contains("objects/lamp.json: has states and an ambient loop or actions on layer 1"),
        "{codes:?}"
    );
    assert!(find("scene_owns_state").unwrap().contains("lamp-c's track keys a state"), "{codes:?}");
    // The one-object check gives the object's warning, and a scene without a data-state on a group around a keyed item is quiet.
    let one = ok(&mut s, "check", json!({ "name": "lamp" }));
    assert_eq!(one["warnings"][0]["code"], "states_and_loops");
    let quiet = Fx::new();
    let mut q = quiet.session();
    assert!(ok(&mut q, "check", json!({}))["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .all(|w| w["code"] != "scene_owns_state" && w["code"] != "states_and_loops"));
}

// ---------------------------------------------------------------- R34: all three doors

const NEW_COMMANDS: [&str; 13] = [
    "object.state.add",
    "object.state.set",
    "object.state.delete",
    "object.state.duplicate",
    "object.state.move",
    "object.state.frame.add",
    "object.state.frame.delete",
    "object.state.frame.move",
    "object.state.key.set",
    "object.state.key.delete",
    "object.state.key.move",
    "object.states.make",
    "object.state.run",
];

#[test]
fn r34_every_new_command_is_in_the_registry_the_mcp_tool_list_and_the_stdio_server() {
    let reg = registry();
    for name in NEW_COMMANDS {
        assert!(reg.has(name), "{name}");
    }
    let fx = Fx::new();
    fx.write("objects/character-like.json", &character_like());
    let mut mcp = common::server::start_server(&["--mcp", "--document", fx.root.to_str().unwrap()], &fx.env(), &fx.state);
    mcp.request(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } }),
    );
    let listed = mcp.request("tools/list", json!({}))["result"]["tools"].clone();
    let tools: Vec<&str> = listed.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for name in NEW_COMMANDS {
        assert!(tools.contains(&name.replace('.', "_").as_str()), "{name}");
    }
    let run = listed.as_array().unwrap().iter().find(|t| t["name"] == "object_state_run").unwrap();
    assert_eq!(run["annotations"]["readOnlyHint"], true);
    assert_eq!(listed.as_array().unwrap().iter().find(|t| t["name"] == "object_state_add").unwrap()["annotations"]["readOnlyHint"], false);
    // The agent's instructions say what a state is and how a scene keys one.
    let init = mcp.request(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } }),
    );
    let text = init["result"]["instructions"].as_str().unwrap().to_string();
    for needle in ["named states", "data-state", "scene_key_set state", "object_states_make", "object_state_run", "scene_set state"] {
        assert!(text.contains(needle), "{needle}");
    }
    // One call of each over MCP, each as the agent that made it.
    let call_tool = |mcp: &mut common::server::Server, name: &str, args: Value| {
        let r = mcp.request("tools/call", json!({ "name": name, "arguments": args }))["result"].clone();
        assert_ne!(r["isError"], true, "{name}: {r}");
        r
    };
    call_tool(&mut mcp, "object_state_add", json!({ "name": "lamp", "state": "dim", "frames": [1] }));
    call_tool(&mut mcp, "object_state_set", json!({ "name": "lamp", "state": "dim", "ms": 300 }));
    call_tool(&mut mcp, "object_state_duplicate", json!({ "name": "lamp", "state": "dim", "as": "dim-2" }));
    call_tool(&mut mcp, "object_state_move", json!({ "name": "lamp", "state": "dim-2", "by": -1 }));
    call_tool(&mut mcp, "object_state_frame_add", json!({ "name": "lamp", "state": "dim", "frame": 0 }));
    call_tool(&mut mcp, "object_state_frame_move", json!({ "name": "lamp", "state": "dim", "at": 0, "by": 1 }));
    call_tool(&mut mcp, "object_state_frame_delete", json!({ "name": "lamp", "state": "dim", "at": 0 }));
    call_tool(&mut mcp, "object_state_key_set", json!({ "name": "lamp", "state": "dim", "time": 100, "move": [1, 0] }));
    call_tool(&mut mcp, "object_state_key_move", json!({ "name": "lamp", "state": "dim", "time": 100, "to": 110 }));
    call_tool(&mut mcp, "object_state_key_delete", json!({ "name": "lamp", "state": "dim", "time": 110 }));
    call_tool(&mut mcp, "object_state_delete", json!({ "name": "lamp", "state": "dim-2" }));
    call_tool(&mut mcp, "object_states_make", json!({ "name": "character-like" }));
    let r = call_tool(&mut mcp, "object_state_run", json!({ "name": "character-like", "state": "hop" }));
    assert!(r["structuredContent"]["changes"].is_array() || r["content"][0]["text"].as_str().unwrap_or("").contains("changes"), "{r}");
    call_tool(&mut mcp, "scene_key_set", json!({ "name": "states", "item": "lamp-b", "time": 1500 }));
    call_tool(&mut mcp, "scene_set", json!({ "name": "states", "item": "walker-a", "state": "walk" }));
    call_tool(&mut mcp, "scene_track_add", json!({ "name": "states", "item": "5" }));
    drop(mcp);
    // And the same over `serve --stdio`, which the app uses: the command's name is the method.
    let fx2 = Fx::new();
    fx2.write("objects/character-like.json", &character_like());
    let mut stdio = common::server::start_server(&["--stdio", "--document", fx2.root.to_str().unwrap()], &fx2.env(), &fx2.state);
    let mut rpc = |method: &str, params: Value| {
        let r = stdio.request(method, params);
        assert!(r.get("error").is_none(), "{method}: {r}");
        r["result"].clone()
    };
    rpc("object.state.add", json!({ "name": "lamp", "state": "dim", "frames": [1] }));
    rpc("object.state.set", json!({ "name": "lamp", "state": "dim", "ms": 300 }));
    rpc("object.state.duplicate", json!({ "name": "lamp", "state": "dim", "as": "dim-2" }));
    rpc("object.state.move", json!({ "name": "lamp", "state": "dim-2", "by": -1 }));
    rpc("object.state.frame.add", json!({ "name": "lamp", "state": "dim", "frame": 0 }));
    rpc("object.state.frame.move", json!({ "name": "lamp", "state": "dim", "at": 0, "by": 1 }));
    rpc("object.state.frame.delete", json!({ "name": "lamp", "state": "dim", "at": 0 }));
    rpc("object.state.key.set", json!({ "name": "lamp", "state": "dim", "time": 100, "move": [1, 0] }));
    rpc("object.state.key.move", json!({ "name": "lamp", "state": "dim", "time": 100, "to": 110 }));
    rpc("object.state.key.delete", json!({ "name": "lamp", "state": "dim", "time": 110 }));
    rpc("object.state.delete", json!({ "name": "lamp", "state": "dim-2" }));
    rpc("object.states.make", json!({ "name": "character-like" }));
    let run = rpc("object.state.run", json!({ "name": "character-like", "state": "hop" }));
    assert_eq!(run["settle"]["kind"], "loop");
    let frame = rpc("object.render", json!({ "name": "character-like", "state": "hop", "time": 130 }));
    assert_eq!(frame["items"][0]["state"], "hop");
    let keyed = rpc("scene.render", json!({ "name": "states", "time": 1100 }));
    let lamp = keyed["items"].as_array().unwrap().iter().find(|i| i["id"] == "lamp-b").unwrap().clone();
    assert_eq!((lamp["state"].clone(), lamp["frame"].clone()), (json!("flicker"), json!(1)), "100 ms into flicker: its second frame");
    // Edits through the app's door are undoable steps of the session, not written until saved.
    let status = rpc("doc.status", json!({ "doc": "object:lamp" }));
    assert_eq!(status["dirty"], true);
}
