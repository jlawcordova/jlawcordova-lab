//! Object states (docs/intents/2026-10-object-states/spec.md): the format and its checks (R1, R2,
//! R5, R7), what a state's run shows at a time (R6) and the renders of a sprite with states.

mod common;

use std::collections::BTreeMap;

use engine::engine::{load_sources, resolve, validate, Sources};
use engine::format::{parse, read_docs, serialize_file};
use engine::states::{End, States};
use serde_json::{json, Value};

/// The committed fixture: one sprite per kind of state, and a scene that places them.
fn states_fixture() -> std::path::PathBuf {
    common::repo().join("fixtures/states.pixellab")
}

struct Docs {
    palette: Value,
    objects: BTreeMap<String, Value>,
    scenes: BTreeMap<String, Value>,
}

fn docs() -> Docs {
    let d = read_docs(&states_fixture());
    assert!(d.load_errors.is_empty(), "{:?}", d.load_errors);
    Docs { palette: d.palette.expect("a palette"), objects: d.objects, scenes: d.scenes }
}

impl Docs {
    fn sources(self) -> Sources {
        load_sources(self.palette, self.objects, self.scenes)
    }
}

/// The problems of the fixture after `change`.
fn problems(change: impl FnOnce(&mut Docs)) -> Vec<String> {
    let mut d = docs();
    change(&mut d);
    validate(&d.sources())
}

fn walker(d: &mut Docs) -> &mut Value {
    d.objects.get_mut("walker").expect("the walker")
}

fn one(found: &[String], needle: &str) {
    assert!(found.iter().any(|p| p.contains(needle)), "no problem with {needle:?} in {found:#?}");
}

// ------------------------------------------------------------------- R1: the format

#[test]
fn r1_the_fixture_has_every_kind_of_state_and_checks_clean() {
    let d = docs();
    assert_eq!(validate(&d.sources()), Vec::<String>::new());
    let sources = docs().sources();
    let states = resolve(&sources, "walker").unwrap().states.expect("the walker has states");
    assert_eq!(states.names(), ["idle", "walk", "dead", "hop", "ping", "pong"]);
    assert_eq!(states.start, "idle");
    assert_eq!(states.get("idle").unwrap().end, End::Loop);
    assert_eq!(states.get("dead").unwrap().end, End::Hold);
    assert_eq!(states.get("hop").unwrap().end, End::Then("idle".into()));
    assert_eq!(states.get("hop").unwrap().length(), 480.0);
    assert_eq!(states.get("walk").unwrap().frames, [0, 1, 2, 1]);
    assert_eq!(states.get("idle").unwrap().moves.len(), 2);
    assert!(resolve(&sources, "pebble").unwrap().states.is_none(), "a sprite without states has none");
}

#[test]
fn r1_the_fixture_is_canonical_and_a_state_is_one_line() {
    let root = states_fixture();
    for file in common::sources(&root) {
        let text = std::fs::read_to_string(root.join(&file)).unwrap();
        assert_eq!(serialize_file(&file, &parse(&text).unwrap()), text, "{file} is canonical");
    }
    let text = std::fs::read_to_string(root.join("objects/walker.json")).unwrap();
    assert!(text.contains("\"start\": \"idle\",\n  \"states\": {\n"), "start and states follow the layers");
    assert!(
        text.contains(
            "    \"hop\": { \"frames\": [1], \"ms\": 480, \"loop\": false, \"then\": \"idle\", \"move\": [{ \"t\": 0, \"move\": [0, 0] }, { \"t\": 44, \"move\": [0, -2] }] },\n"
        ),
        "{text}"
    );
    let scene = std::fs::read_to_string(root.join("scenes/states.json")).unwrap();
    assert!(
        scene.contains("{ \"object\": \"walker\", \"state\": \"walk\", \"at\": { \"tile\": [1, 0, 0] } }"),
        "an item's state follows depth"
    );
    assert!(scene.contains("{ \"t\": 1000, \"state\": \"flicker\" }"));
}

#[test]
fn r1_state_names_follow_the_name_rule() {
    let found = problems(|d| {
        let o = walker(d);
        let idle = o["states"]["idle"].clone();
        o["states"]["Bad Name"] = idle;
    });
    one(&found, "states.Bad Name: names are lowercase words joined by hyphens");
}

#[test]
fn r1_at_most_32_states_and_none_on_a_block() {
    let found = problems(|d| {
        let o = walker(d);
        for i in 0..30 {
            o["states"][format!("extra-{i}")] = json!({ "frames": [0], "ms": 100 });
        }
    });
    one(&found, "at most 32 states, not 36");
    let found = problems(|d| {
        d.objects.get_mut("block").unwrap()["states"] = json!({ "idle": { "frames": [0], "ms": 100 } });
    });
    one(&found, "objects/block.json: states is for sprites; a block has no states");
}

// ------------------------------------------------------------ R2: one frame on every layer

#[test]
fn r2_the_frame_loops_of_a_sprite_with_states_are_as_long_as_each_other() {
    let found = problems(|d| {
        let o = walker(d);
        let frames = o["layers"][1]["frames"].clone();
        o["layers"].as_array_mut().unwrap().push(json!({ "loop": "wk2", "prefix": "v", "frames": [frames[0], frames[1]] }));
    });
    one(&found, "layer 2: 2 frames, but layer 1 has 3; the frame loops of a sprite with states are as long as each other");
    // A sprite without states keeps loops of any length.
    let found = problems(|d| {
        let o = d.objects.get_mut("pebble").unwrap();
        let frames = o["layers"][0]["frames"].clone();
        o["layers"].as_array_mut().unwrap().push(json!({ "loop": "wk2", "prefix": "v", "frames": [frames[0], frames[1]] }));
    });
    assert_eq!(found, Vec::<String>::new());
}

// -------------------------------------------------------------------- R5: extends

#[test]
fn r5_a_sprite_that_extends_another_has_the_bases_states() {
    let mut d = docs();
    d.objects.insert("walker-hat".into(), json!({ "kind": "sprite", "extends": "walker", "rows": { "0": { "0": "c..." } } }));
    let sources = d.sources();
    assert_eq!(validate(&sources), Vec::<String>::new());
    let base = resolve(&sources, "walker").unwrap().states.unwrap();
    let outfit = resolve(&sources, "walker-hat").unwrap().states.unwrap();
    assert_eq!(base, outfit);
}

#[test]
fn r5_a_sprite_that_extends_another_cant_have_its_own_states() {
    let found = problems(|d| {
        d.objects.insert(
            "walker-hat".into(),
            json!({ "kind": "sprite", "extends": "walker", "start": "idle", "states": { "idle": { "frames": [0], "ms": 100 } } }),
        );
    });
    one(&found, "objects/walker-hat.json: an object that extends another has the states of 'walker'; it can't have its own");
}

// ---------------------------------------------------------------------- R7: checks

#[test]
fn r7_start_names_one_of_the_states() {
    one(&problems(|d| walker(d)["start"] = json!("sprint")), "start 'sprint' is not one of its states (idle, walk, dead, hop, ping, pong)");
    one(&problems(|d| drop(walker(d).as_object_mut().unwrap().shift_remove("start"))), "start is missing: name the state it starts in");
    one(
        &problems(|d| {
            let o = d.objects.get_mut("pebble").unwrap();
            o["start"] = json!("idle");
        }),
        "start names a state, but there are no states",
    );
}

#[test]
fn r7_states_is_not_empty() {
    one(&problems(|d| walker(d)["states"] = json!({})), "states must be an object of named states (leave it out for none)");
}

#[test]
fn r7_then_names_a_state_of_the_sprite_and_not_on_a_looping_state() {
    one(&problems(|d| walker(d)["states"]["hop"]["then"] = json!("fly")), "states.hop: then 'fly' is not one of the states");
    one(
        &problems(|d| walker(d)["states"]["idle"]["then"] = json!("hop")),
        "states.idle: a looping state has no then; set loop to false to play once and hand over",
    );
}

#[test]
fn r7_frames_are_frames_of_the_sprite_and_a_state_has_some() {
    one(
        &problems(|d| walker(d)["states"]["walk"]["frames"] = json!([0, 3])),
        "states.walk: frame 3 is not a frame of the sprite (it has 3, from 0 to 2)",
    );
    one(&problems(|d| walker(d)["states"]["walk"]["frames"] = json!([])), "states.walk: frames must be a list of 1 to 256 frame numbers");
    one(&problems(|d| walker(d)["states"]["walk"]["frames"] = json!([0.5])), "states.walk: frame 0.5 is not a frame");
    one(
        &problems(|d| drop(walker(d)["states"]["walk"].as_object_mut().unwrap().shift_remove("frames"))),
        "states.walk: frames must be a list",
    );
    one(&problems(|d| walker(d)["states"]["walk"]["frames"] = json!(vec![0; 257])), "states.walk: frames must be a list of 1 to 256");
}

#[test]
fn r7_a_still_sprite_has_one_frame() {
    let found = problems(|d| {
        let o = d.objects.get_mut("block").unwrap();
        *o = json!({ "kind": "sprite", "keys": { "a": "ink" }, "layers": [{ "map": ["a"] }],
            "start": "only", "states": { "only": { "frames": [1], "ms": 100 } } });
    });
    one(&found, "states.only: frame 1 is not a frame of the sprite (it has 1, from 0 to 0)");
}

#[test]
fn r7_the_frame_length_is_20_to_10000_ms_and_a_state_is_at_most_600000_ms() {
    for ms in [json!(19), json!(10001), json!(100.5), json!("100")] {
        one(&problems(|d| walker(d)["states"]["walk"]["ms"] = ms), "states.walk: ms must be a whole number from 20 to 10000");
    }
    assert_eq!(problems(|d| walker(d)["states"]["walk"]["ms"] = json!(20)), Vec::<String>::new());
    assert_eq!(problems(|d| walker(d)["states"]["walk"]["ms"] = json!(10000)), Vec::<String>::new());
    // 100 frames of 10000 ms is 1000000 ms.
    let found = problems(|d| {
        walker(d)["states"]["walk"]["ms"] = json!(10000);
        walker(d)["states"]["walk"]["frames"] = json!(vec![0; 61]);
    });
    one(&found, "states.walk: lasts 610000 ms; at most 600000");
}

#[test]
fn r7_move_keys_follow_the_ambient_key_rules_within_the_states_length() {
    let walk_move = |keys: Value| move |d: &mut Docs| walker(d)["states"]["walk"]["move"] = keys;
    one(&problems(walk_move(json!([{ "t": 10, "move": [0, 0] }]))), "states.walk: move[0]: the first key must be at t: 0");
    one(
        &problems(walk_move(json!([{ "t": 0, "move": [0, 0] }, { "t": 0, "move": [0, 1] }]))),
        "move[1]: t must come after the key before it",
    );
    one(
        &problems(walk_move(json!([{ "t": 0, "move": [0, 0] }, { "t": 600, "move": [0, 1] }]))),
        "move[1]: t must be from 0 to below the duration",
    );
    one(&problems(walk_move(json!([{ "t": 0, "move": [0.5, 0] }]))), "move[0]: move must be two whole numbers");
    one(&problems(walk_move(json!([]))), "states.walk: move must be a list of at least one key");
    assert_eq!(problems(walk_move(json!([{ "t": 0, "move": [0, 0] }, { "t": 599, "move": [0, 1] }]))), Vec::<String>::new());
}

#[test]
fn r7_a_state_has_only_its_own_properties_and_loop_is_false_or_absent() {
    one(&problems(|d| walker(d)["states"]["walk"]["speed"] = json!(2)), "states.walk: unknown property 'speed'");
    one(&problems(|d| walker(d)["states"]["walk"]["loop"] = json!(true)), "states.walk: loop must be false (leave it out to loop)");
    one(&problems(|d| walker(d)["states"]["walk"] = json!("fast")), "states.walk: must be { frames, ms, loop?, then?, move? }");
}

#[test]
fn r7_an_items_state_names_a_state_of_the_object_it_shows() {
    one(
        &problems(|d| d.scenes.get_mut("states").unwrap()["items"][1]["state"] = json!("fly")),
        "items[1]: state 'fly' is no state of walker (it has idle, walk, dead, hop, ping, pong)",
    );
    one(
        &problems(|d| d.scenes.get_mut("states").unwrap()["items"][5]["state"] = json!("idle")),
        "items[5]: state 'idle' is no state of pebble, which has no states",
    );
    one(
        &problems(|d| d.scenes.get_mut("states").unwrap()["items"][1]["state"] = json!(3)),
        "items[1]: state must be the name of one of the object's states",
    );
    assert_eq!(problems(|d| d.scenes.get_mut("states").unwrap()["items"][1]["state"] = json!("hop")), Vec::<String>::new());
}

#[test]
fn r7_a_keys_state_names_a_state_of_the_object_shown_at_that_key() {
    let key = |at: usize, state: Value| {
        move |d: &mut Docs| d.scenes.get_mut("states").unwrap()["motion"]["tracks"][0]["keys"][at]["state"] = state
    };
    one(&problems(key(1, json!("fly"))), "motion.tracks[0].keys[1]: state 'fly' is no state of lamp (it has idle, flicker)");
    one(&problems(key(1, json!(7))), "keys[1]: state must be the name of one of the object's states");
    // The object shown is the pose once a key poses the item: walker's states aren't the lamp's.
    let found = problems(|d| {
        let keys = &mut d.scenes.get_mut("states").unwrap()["motion"]["tracks"][0]["keys"];
        keys[1]["pose"] = json!("walker");
        keys[1]["state"] = json!("flicker");
    });
    one(&found, "keys[1]: state 'flicker' is no state of walker");
    // ...and it is the pose's states that count: hop is walker's.
    let found = problems(|d| {
        let keys = &mut d.scenes.get_mut("states").unwrap()["motion"]["tracks"][0]["keys"];
        keys[1]["pose"] = json!("walker");
        keys[1]["state"] = json!("hop");
    });
    assert_eq!(found, Vec::<String>::new());
    // A key after a pose key still shows the pose.
    let found = problems(|d| {
        let keys = &mut d.scenes.get_mut("states").unwrap()["motion"]["tracks"][0]["keys"];
        keys[1]["pose"] = json!("walker");
        keys[2]["state"] = json!("flicker");
    });
    one(&found, "keys[2]: state 'flicker' is no state of walker");
    one(
        &problems(|d| {
            let keys = &mut d.scenes.get_mut("states").unwrap()["motion"]["tracks"][0]["keys"];
            keys[1]["pose"] = json!("pebble");
            keys[1]["state"] = json!("idle");
        }),
        "keys[1]: state 'idle' is no state of pebble, which has no states",
    );
}

#[test]
fn r7_states_of_a_valid_source_parse_and_a_malformed_one_is_left_out() {
    let v = json!({ "start": "a", "states": { "a": { "frames": [0], "ms": 100 }, "b": { "frames": [], "ms": 100 }, "c": { "frames": [1], "ms": 50, "loop": false } } });
    let s = States::of(&v).unwrap();
    assert_eq!(s.names(), ["a", "c"]);
    assert_eq!(s.get("c").unwrap().end, End::Hold);
    let v = json!({ "start": "zzz", "states": { "a": { "frames": [0], "ms": 100 } } });
    assert_eq!(States::of(&v).unwrap().start, "a", "a start that names nothing falls back to the first state");
    assert!(States::of(&json!({ "layers": [] })).is_none());
}
