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

// ----------------------------------------------------------------- R6: the run

use engine::ambient::{action_at, moves_of};
use engine::states::{at, changes, run, Pose, Settle};

fn walker_states() -> std::rc::Rc<States> {
    resolve(&docs().sources(), "walker").unwrap().states.unwrap()
}

fn pose(frame: usize, moved: [f64; 2]) -> Pose {
    Pose { frame, moved }
}

fn states_of(v: Value) -> std::rc::Rc<States> {
    States::of(&v).expect("states")
}

#[test]
fn r6_a_looping_state_loops_from_its_first_frame_and_its_move_steps_back() {
    let s = walker_states();
    let r = run(&s, "idle");
    assert_eq!(r.segments.len(), 1);
    assert_eq!(r.settle, Settle::Loop { from: 0.0, period: 1000.0 });
    for (t, want) in
        [(0.0, [0.0, 0.0]), (499.0, [0.0, 0.0]), (500.0, [0.0, -1.0]), (999.0, [0.0, -1.0]), (1000.0, [0.0, 0.0]), (1500.0, [0.0, -1.0])]
    {
        assert_eq!(at(&s, &r, t), pose(0, want), "{t} ms into idle");
    }
    let w = run(&s, "walk");
    let frames: Vec<usize> = [0.0, 149.0, 150.0, 300.0, 450.0, 599.0, 600.0, 750.0].iter().map(|t| at(&s, &w, *t).frame).collect();
    assert_eq!(frames, [0, 0, 1, 2, 1, 1, 0, 1], "walk's frames repeat every 600 ms");
}

#[test]
fn r6_a_state_that_holds_shows_its_last_frame_from_when_it_has_played() {
    let s = walker_states();
    let r = run(&s, "dead");
    assert_eq!(r.settle, Settle::Hold { from: 200.0, frame: 2, moved: [0.0, 0.0] });
    let frames: Vec<usize> = [0.0, 99.0, 100.0, 199.0, 200.0, 99999.0].iter().map(|t| at(&s, &r, *t).frame).collect();
    assert_eq!(frames, [1, 1, 2, 2, 2, 2]);
}

#[test]
fn r6_a_then_hands_over_to_the_next_state_and_the_run_ends_in_its_loop() {
    let s = walker_states();
    let r = run(&s, "hop");
    assert_eq!(
        r.segments.iter().map(|g| (g.state.as_str(), g.from, g.to)).collect::<Vec<_>>(),
        [("hop", 0.0, 480.0), ("idle", 480.0, 1480.0)]
    );
    assert_eq!(r.settle, Settle::Loop { from: 480.0, period: 1000.0 });
    assert_eq!(at(&s, &r, 0.0), pose(1, [0.0, 0.0]));
    assert_eq!(at(&s, &r, 479.0), pose(1, [0.0, -2.0]), "hop holds its last key until it hands over");
    assert_eq!(at(&s, &r, 480.0), pose(0, [0.0, 0.0]), "idle starts from its first frame and key");
    assert_eq!(at(&s, &r, 980.0), pose(0, [0.0, -1.0]));
    assert_eq!(at(&s, &r, 1480.0), pose(0, [0.0, 0.0]), "and loops");
    assert_eq!(at(&s, &r, 480.0 + 5000.0 + 500.0), pose(0, [0.0, -1.0]));
}

#[test]
fn r6_a_chain_of_one_shot_states_that_leads_back_to_itself_is_a_loop() {
    let s = walker_states();
    let r = run(&s, "ping");
    assert_eq!(r.segments.len(), 2);
    assert_eq!(r.settle, Settle::Loop { from: 0.0, period: 400.0 });
    let frames: Vec<usize> =
        [0.0, 99.0, 100.0, 200.0, 299.0, 300.0, 399.0, 400.0, 500.0, 1000.0].iter().map(|t| at(&s, &r, *t).frame).collect();
    assert_eq!(frames, [0, 0, 1, 1, 1, 0, 0, 0, 1, 1]);
    // Entered at pong, the cycle starts there.
    let r = run(&s, "pong");
    assert_eq!(r.settle, Settle::Loop { from: 0.0, period: 400.0 });
    assert_eq!(at(&s, &r, 0.0).frame, 1);
    assert_eq!(at(&s, &r, 200.0).frame, 0);
}

#[test]
fn r6_a_chain_that_comes_back_to_a_later_state_loops_from_there() {
    let s = states_of(json!({ "start": "a", "states": {
        "a": { "frames": [0], "ms": 100, "loop": false, "then": "b" },
        "b": { "frames": [1], "ms": 200, "loop": false, "then": "c" },
        "c": { "frames": [2], "ms": 300, "loop": false, "then": "b" }
    } }));
    let r = run(&s, "a");
    assert_eq!(r.segments.len(), 3);
    assert_eq!(r.settle, Settle::Loop { from: 100.0, period: 500.0 });
    let frames: Vec<usize> = [0.0, 100.0, 299.0, 300.0, 599.0, 600.0, 800.0, 1099.0, 1100.0].iter().map(|t| at(&s, &r, *t).frame).collect();
    assert_eq!(frames, [0, 1, 1, 2, 2, 1, 2, 2, 1]);
}

#[test]
fn r6_a_then_that_names_no_state_holds_where_it_stands() {
    let s = states_of(json!({ "start": "a", "states": { "a": { "frames": [0, 1], "ms": 100, "loop": false, "then": "gone" } } }));
    let r = run(&s, "a");
    assert_eq!(r.settle, Settle::Hold { from: 200.0, frame: 1, moved: [0.0, 0.0] });
}

#[test]
fn r6_a_move_steps_a_pixel_at_a_time_and_the_one_at_44_ms_in_480_is_where_the_css_puts_it() {
    let s = walker_states();
    let r = run(&s, "hop");
    // 0 → 44 ms goes up two pixels, a step each: the first at 22 ms, the second at 44 ms (a hair after, as the CSS rounds 9.1666…% to 9.167%).
    assert_eq!(at(&s, &r, 21.0).moved, [0.0, 0.0]);
    assert_eq!(at(&s, &r, 23.0).moved, [0.0, -1.0]);
    assert_eq!(at(&s, &r, 43.0).moved, [0.0, -1.0]);
    assert_eq!(at(&s, &r, 45.0).moved, [0.0, -2.0]);
}

#[test]
fn r6_at_agrees_with_ambients_action_at_for_a_converted_action() {
    // The site's hop, as an action and as a state that hands over to a resting idle.
    let keys = json!([{ "t": 0, "move": [0, 0] }, { "t": 43, "move": [0, 0] }, { "t": 44, "move": [0, -2] }, { "t": 436, "move": [0, -2] }, { "t": 437, "move": [0, -1] }]);
    let action = moves_of(&json!({ "actions": { "hop": { "duration": 480, "keys": keys } } })).unwrap();
    let a = &action.actions[0].1;
    let s = states_of(json!({ "start": "idle", "states": {
        "idle": { "frames": [0], "ms": 1000 },
        "hop": { "frames": [0], "ms": 480, "loop": false, "then": "idle", "move": keys }
    } }));
    let r = run(&s, "hop");
    for t in 0..480 {
        let want = action_at(a, t as f64).expect("playing");
        assert_eq!(at(&s, &r, t as f64).moved, want, "{t} ms into the hop");
        assert_eq!(at(&s, &r, t as f64 + 0.5).moved, action_at(a, t as f64 + 0.5).unwrap(), "{t}.5 ms into the hop");
    }
    assert_eq!(action_at(a, 480.0), None, "the action has ended: the element is at rest");
    assert_eq!(at(&s, &r, 480.0).moved, [0.0, 0.0], "and idle's rest is where the state hands over to");
}

#[test]
fn r6_the_same_run_every_time_and_changes_lists_when_the_frame_or_the_move_changes() {
    let s = walker_states();
    let r = run(&s, "hop");
    assert_eq!(run(&s, "hop"), r);
    let list = changes(&s, &r, None);
    let got: Vec<(f64, usize, [f64; 2])> = list.iter().map(|c| ((c.t * 1000.0).round() / 1000.0, c.frame, c.moved)).collect();
    assert_eq!(
        got,
        [(0.0, 1, [0.0, 0.0]), (22.001, 1, [0.0, -1.0]), (44.002, 1, [0.0, -2.0]), (480.0, 0, [0.0, 0.0]), (980.0, 0, [0.0, -1.0])]
    );
    // Further out, the loop goes on repeating.
    let more = changes(&s, &r, Some(3500.0));
    assert_eq!(more.last().map(|c| (c.t, c.frame, c.moved)), Some((3480.0, 0, [0.0, 0.0])), "idle's sixth start, before the horizon");
    assert!(more.iter().any(|c| c.t == 1480.0 && c.moved == [0.0, 0.0]), "idle starts over at 1480");
    assert!(more.windows(2).all(|w| w[0].t < w[1].t), "in time order");
    // Every change is what `at` says from there on, and nothing between two changes differs.
    for pair in more.windows(2) {
        let mid = (pair[0].t + pair[1].t) / 2.0;
        assert_eq!(at(&s, &r, mid), pose(pair[0].frame, pair[0].moved));
    }
    let ping = changes(&s, &run(&s, "ping"), None);
    assert_eq!(ping.iter().map(|c| (c.t, c.frame)).collect::<Vec<_>>(), [(0.0, 0), (100.0, 1), (300.0, 0)]);
    let dead = changes(&s, &run(&s, "dead"), None);
    assert_eq!(dead.iter().map(|c| (c.t, c.frame)).collect::<Vec<_>>(), [(0.0, 1), (100.0, 2)]);
}

// ------------------------------------------------------------- R9: one name, one thing

#[test]
fn r9_a_one_shot_state_and_an_action_of_one_name_in_a_scene_is_a_problem() {
    // A one-shot state ends with an animation named <scene>--<state>, which is what an action's keyframes are called.
    let found = problems(|d| {
        d.scenes.get_mut("states").unwrap()["items"][5]["actions"] =
            json!({ "hop": { "duration": 200, "keys": [{ "t": 0, "move": [0, -1] }] } });
    });
    one(&found, "walker's state 'hop' plays once and ends as <scene>--hop, which is also the action 'hop' on items[5]");
    // A looping state of that name is no clash, and neither is an action of another name.
    let found = problems(|d| {
        d.scenes.get_mut("states").unwrap()["items"][5]["actions"] =
            json!({ "walk": { "duration": 200, "keys": [{ "t": 0, "move": [0, -1] }] } });
    });
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn r12_a_one_shot_state_and_a_frame_loop_of_one_name_in_a_scene_is_a_problem() {
    // A frame loop plays as keyframes <scene>--<loop>, the name a one-shot state's end has: the later one would win.
    let pebble_loop = |d: &mut Docs, name: &str| d.objects.get_mut("pebble").unwrap()["layers"][0]["loop"] = json!(name);
    let found = problems(|d| pebble_loop(d, "flicker"));
    one(&found, "lamp's state 'flicker' plays once and ends as <scene>--flicker, which is also the frame loop 'flicker' that pebble plays");
    // The scene draws a track's poses too.
    let found = problems(|d| {
        pebble_loop(d, "flicker");
        let scene = d.scenes.get_mut("states").unwrap();
        scene["items"].as_array_mut().unwrap().pop();
        scene["motion"]["tracks"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "item": "walker-a", "keys": [{ "t": 0 }, { "t": 500, "pose": "pebble" }] }));
    });
    one(&found, "which is also the frame loop 'flicker' that pebble plays");
    // A looping state of a loop's name ends with nothing, and a sprite with states draws its frames as its states, not as a loop.
    assert_eq!(problems(|d| pebble_loop(d, "walk")), Vec::<String>::new());
    assert_eq!(problems(|d| d.objects.get_mut("lamp").unwrap()["layers"][0]["loop"] = json!("flicker")), Vec::<String>::new());
}

#[test]
fn r7_a_state_is_not_named_as_the_compiler_names_what_it_makes() {
    // mv-<id> is scene motion, a<n> and am-<n> ambient loops, s<n>… a stated item: all <scene>--… beside a one-shot state's end.
    for name in ["mv", "mv-boat", "am-1", "a1", "a12-b", "s2", "s10-f0"] {
        let found = problems(|d| {
            let states = walker(d)["states"].as_object_mut().unwrap();
            let dead = states.shift_remove("dead").unwrap();
            states.insert(name.to_string(), dead);
        });
        one(&found, &format!("states.{name}: a name that starts {} is one the compiler makes", name.split('-').next().unwrap()));
    }
    for name in ["amber", "sway", "s", "a", "a-b", "mvx", "spin2"] {
        assert_eq!(engine::states::reserved(name), None, "{name}");
    }
}

#[test]
fn r7_a_state_written_twice_is_a_problem() {
    // JSON keeps the last of a key, so only the text can tell.
    use engine::format::duplicate_states;
    let text = r#"{ "layers": [{ "states": { "a": 1, "a": 2 } }], "states": { "idle": { "frames": [0], "ms": 1000, "move": [{ "t": 0 }] }, "say \"hi\"": {}, "flicker": {}, "idle": {}, "flicker": {}, "idle": {} } }"#;
    assert_eq!(duplicate_states(text), ["idle", "flicker"]);
    assert_eq!(duplicate_states(r#"{ "states": { "a": { "a": 1, "b": { "a": 2 } }, "b": [{ "a": 1 }, "a"] } }"#), Vec::<String>::new());
    // Reading the document reports it, and the object still loads (as its last).
    let dir = std::env::temp_dir().join(format!("pixellab-states-twice-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for file in common::sources(&states_fixture()) {
        let to = dir.join(&file);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(states_fixture().join(&file), to).unwrap();
    }
    let lamp = dir.join("objects/lamp.json");
    let text = std::fs::read_to_string(&lamp).unwrap();
    std::fs::write(&lamp, text.replacen("    \"flicker\"", "    \"idle\": { \"frames\": [1], \"ms\": 200 },\n    \"flicker\"", 1)).unwrap();
    let d = read_docs(&dir);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(
        d.load_errors,
        ["objects/lamp.json: states.idle: written twice; the last one counts, and saving keeps only it, so remove or rename one"]
    );
    assert_eq!(d.objects["lamp"]["states"]["idle"], json!({ "frames": [1], "ms": 200 }));
}

#[test]
fn r15_a_track_that_only_changes_pose_leaves_each_object_in_the_state_it_starts_in_from_time_0() {
    // With no state key the track isn't baked, so a page's data-state still reaches the item (R17), and each pose plays
    // its object's start from time 0, as the CSS and a render play it: entered at 0 whatever the key's time.
    let sources = docs().sources();
    let states_of = |n: &str| resolve(&sources, n).ok().and_then(|r| r.states);
    let track = json!({ "item": "p", "keys": [{ "t": 0 }, { "t": 1250, "pose": "walker" }, { "t": 2000, "pose": "lamp" }] });
    let segs = engine::states::timeline(&track, "pebble", None, 3000.0, &states_of);
    let got: Vec<(f64, &str, Option<&str>, f64)> = segs.iter().map(|s| (s.from, s.pose.as_str(), s.state.as_deref(), s.entered)).collect();
    assert_eq!(got, [(0.0, "pebble", None, 0.0), (1250.0, "walker", Some("idle"), 0.0), (2000.0, "lamp", Some("idle"), 0.0)]);
    // The item's own state is its own object's.
    let track = json!({ "item": "w", "keys": [{ "t": 0 }, { "t": 1000, "pose": "lamp" }, { "t": 2000, "pose": "walker" }] });
    let segs = engine::states::timeline(&track, "walker", Some("walk"), 3000.0, &states_of);
    let got: Vec<Option<&str>> = segs.iter().map(|s| s.state.as_deref()).collect();
    assert_eq!(got, [Some("walk"), Some("idle"), Some("walk")]);
    assert!(engine::states::schedule(&track, "walker", Some("walk"), 3000.0, "walker", &states_of).is_none(), "nothing is baked");
}

#[test]
fn r16_a_long_loop_keeps_its_baked_changes_within_a_ms_of_the_run() {
    // Three decimals of a percent is 3 ms of a 300 s loop: the stops get as many more as they need to stay within 1 ms.
    let sources = docs().sources();
    let states_of = |n: &str| resolve(&sources, n).ok().and_then(|r| r.states);
    let track = json!({ "item": "lamp-b", "keys": [{ "t": 0 }, { "t": 1001, "state": "flicker" }] });
    let keyed = engine::states::schedule(&track, "lamp", None, 300_000.0, "lamp", &states_of).unwrap();
    // flicker shows frame 1 from 80 ms in: 1081 ms.
    assert_eq!(keyed.at(1080.0).frame, 0);
    assert_eq!(keyed.at(1081.0).frame, 1);
    let stop = keyed.stops.iter().find(|s| s.frame == 1).unwrap();
    assert_eq!(stop.pct, 0.3603);
    assert!(stop.pct * 3000.0 <= 1081.0 && stop.pct * 3000.0 > 1080.0);
    // A loop of 100 s or less keeps 3 decimals, as before.
    let keyed = engine::states::schedule(&track, "lamp", None, 3000.0, "lamp", &states_of).unwrap();
    assert!(keyed.stops.iter().all(|s| (s.pct * 1000.0 - (s.pct * 1000.0).round()).abs() < 1e-9), "{:?}", keyed.stops);
}
