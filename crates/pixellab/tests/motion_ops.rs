//! Ports cli/test/motion-commands.test.mjs onto direct calls of the motion
//! operations (`pixellab::motion_ops`). Expectations marked "from Node" were
//! computed by running the Node functions once.

#![allow(clippy::result_large_err)]

mod common;

use std::collections::BTreeMap;

use engine::engine::{load_sources, validate};
use engine::format::read_docs;
use pixellab::context::{Ctx, DocKind};
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::{CommandError, Error};
use pixellab::motion_ops::{self, KeyValues, PathTiming};
use pixellab::registry::registry;
use pixellab::session::{Options, Session};
use serde_json::{json, Value};

fn moving() -> Value {
    json!({
        "viewBox": [-32, -16, 96, 64],
        "items": [
            { "object": "block", "at": { "tile": [0, 0, 0] } },
            { "object": "pebble", "id": "cart", "at": { "px": [0, 0] } },
            { "object": "pebble", "id": "bird", "at": { "px": [20, 0] } },
            { "object": "pebble", "at": { "px": [40, 0] } },
        ],
        "motion": {
            "duration": 2000,
            "tracks": [{ "item": "cart", "keys": [{ "t": 0 }, { "t": 1000, "move": [16, -3] }] }],
        },
    })
}

fn still() -> Value {
    json!({ "viewBox": [0, 0, 16, 16], "items": [{ "object": "pebble", "id": "lone", "at": { "px": [8, 12] } }] })
}

fn keys_of(doc: &Value, id: &str) -> Value {
    motion_ops::track_of(doc, id).expect("a track")["keys"].clone()
}

fn code(r: Result<Value, CommandError>) -> String {
    r.expect_err("an error").code
}

fn vals(f: impl FnOnce(&mut KeyValues)) -> KeyValues {
    let mut v = KeyValues::default();
    f(&mut v);
    v
}

const FILE: &str = "scenes/moving.json";

#[test]
fn r12_add_a_track_an_item_needs_an_id_set_move_show_and_pose_keys_move_and_delete_them_remove_the_track() {
    let mut s = moving();
    assert_eq!(code(motion_ops::add_track(&mut s, "cart", None)), "track_exists");
    let added = motion_ops::add_track(&mut s, "bird", None).unwrap();
    assert_eq!(added["track"], json!({ "item": "bird", "keys": [{ "t": 0 }] }));
    assert_eq!(added["message"], "Added a track for bird");

    let moved = motion_ops::set_key(&mut s, "bird", 500.0, &vals(|v| v.move_ = Some([4.0, -2.0]))).unwrap();
    assert_eq!(moved["key"], json!({ "t": 500, "move": [4, -2] }));
    assert_eq!(moved["message"], "Made bird's key at 500 ms: move 4, -2");
    let both = vals(|v| {
        v.show = Some(false);
        v.pose = Some("block".into());
    });
    let r = motion_ops::set_key(&mut s, "bird", 500.0, &both).unwrap();
    assert_eq!(r["message"], "Set bird's key at 500 ms: show false, pose block");
    motion_ops::set_key(&mut s, "bird", 1500.0, &vals(|v| v.show = Some(true))).unwrap();
    assert_eq!(
        keys_of(&s, "bird"),
        json!([{ "t": 0 }, { "t": 500, "move": [4, -2], "show": false, "pose": "block" }, { "t": 1500, "show": true }])
    );
    let r = motion_ops::set_key(&mut s, "bird", 500.0, &vals(|v| v.clear = vec!["pose".into()])).unwrap();
    assert_eq!(r["message"], "Set bird's key at 500 ms: cleared pose");
    assert_eq!(keys_of(&s, "bird")[1], json!({ "t": 500, "move": [4, -2], "show": false }));

    let r = motion_ops::move_key(&mut s, "bird", 1500.0, 1200.0).unwrap();
    assert_eq!(r["key"], json!({ "t": 1200, "show": true }));
    assert_eq!(r["message"], "Moved bird's key from 1500 ms to 1200 ms");
    let times: Vec<Value> = keys_of(&s, "bird").as_array().unwrap().iter().map(|k| k["t"].clone()).collect();
    assert_eq!(times, vec![json!(0), json!(500), json!(1200)]);
    assert_eq!(code(motion_ops::move_key(&mut s, "bird", 1200.0, 500.0)), "key_exists");
    assert_eq!(code(motion_ops::move_key(&mut s, "bird", 0.0, 100.0)), "first_key");
    assert_eq!(code(motion_ops::set_key(&mut s, "bird", 2000.0, &vals(|v| v.show = Some(true)))), "out_of_range");
    let r = motion_ops::delete_key(&mut s, "bird", 1200.0).unwrap();
    assert_eq!(r["message"], "Deleted bird's key at 1200 ms");
    assert_eq!(code(motion_ops::delete_key(&mut s, "bird", 0.0)), "first_key");

    let r = motion_ops::remove_track(&mut s, "bird").unwrap();
    assert_eq!(r["message"], "Removed bird's track");
    assert_eq!(s["motion"]["tracks"].as_array().unwrap().iter().map(|t| t["item"].clone()).collect::<Vec<_>>(), vec![json!("cart")]);
    assert_eq!(code(motion_ops::remove_track(&mut s, "bird")), "no_track");
}

#[test]
fn r12_add_a_track_an_item_needs_an_id_set_move_show_and_pose_keys_move_and_delete_them_remove_the_track_through_the_commands() {
    let fx = Fx::new();
    let mut s = fx.session();
    // An item without an id gets one when it is given a track (R23): its object's name, made unique.
    {
        let fx2 = Fx::new();
        let mut s2 = fx2.session();
        let r = call(&mut s2, "scene.track.add", json!({ "item": "3" }), "moving").unwrap();
        assert_eq!(r["id"], "pebble");
        assert_eq!(r["track"], json!({ "item": "pebble", "keys": [{ "t": 0 }] }));
        assert!(r["summary"].as_str().unwrap().contains("the item had no id, so it got pebble"));
        assert_eq!(
            rejects(
                call(&mut s2, "scene.key.set", json!({ "item": "pebble", "time": 100, "move": [1, 0] }), "moving")
                    .and(Err(Error::Internal("ok".into())))
            ),
            "ok"
        );
    }
    assert_eq!(rejects(call(&mut s, "scene.track.add", json!({ "item": "cart" }), "moving")), "track_exists");
    let added = call(&mut s, "scene.track.add", json!({ "item": "bird" }), "moving").unwrap();
    assert_eq!(added["track"], json!({ "item": "bird", "keys": [{ "t": 0 }] }));

    let moved = call(&mut s, "scene.key.set", json!({ "item": "bird", "time": 500, "move": [4, -2] }), "moving").unwrap();
    assert_eq!(moved["key"], json!({ "t": 500, "move": [4, -2] }));
    assert_eq!(moved["warnings"], json!([]));
    call(&mut s, "scene.key.set", json!({ "item": "bird", "time": 500, "show": false, "pose": "block" }), "moving").unwrap();
    call(&mut s, "scene.key.set", json!({ "item": "bird", "time": 1500, "show": true }), "moving").unwrap();
    assert_eq!(
        keys_of_scene(&mut s, "bird"),
        json!([{ "t": 0 }, { "t": 500, "move": [4, -2], "show": false, "pose": "block" }, { "t": 1500, "show": true }])
    );
    call(&mut s, "scene.key.set", json!({ "item": "bird", "time": 500, "clear": ["pose"] }), "moving").unwrap();
    assert_eq!(keys_of_scene(&mut s, "bird")[1], json!({ "t": 500, "move": [4, -2], "show": false }));

    call(&mut s, "scene.key.move", json!({ "item": "bird", "time": 1500, "to": 1200 }), "moving").unwrap();
    let times: Vec<Value> = keys_of_scene(&mut s, "bird").as_array().unwrap().iter().map(|k| k["t"].clone()).collect();
    assert_eq!(times, [json!(0), json!(500), json!(1200)]);
    assert_eq!(rejects(call(&mut s, "scene.key.move", json!({ "item": "bird", "time": 1200, "to": 500 }), "moving")), "key_exists");
    assert_eq!(rejects(call(&mut s, "scene.key.move", json!({ "item": "bird", "time": 0, "to": 100 }), "moving")), "first_key");
    assert_eq!(rejects(call(&mut s, "scene.key.set", json!({ "item": "bird", "time": 2000, "show": true }), "moving")), "out_of_range");
    call(&mut s, "scene.key.delete", json!({ "item": "bird", "time": 1200 }), "moving").unwrap();
    assert_eq!(rejects(call(&mut s, "scene.key.delete", json!({ "item": "bird", "time": 0 }), "moving")), "first_key");
    assert_eq!(rejects(call(&mut s, "scene.key.set", json!({ "item": "bird", "time": 100, "pose": "no-such-object" }), "moving")), "usage");
    assert_eq!(problems(&mut s, "moving"), Vec::<String>::new(), "what the commands wrote validates");

    call(&mut s, "scene.track.remove", json!({ "item": "bird" }), "moving").unwrap();
    let items: Vec<Value> = tracks(&mut s).as_array().unwrap().iter().map(|t| t["item"].clone()).collect();
    assert_eq!(items, [json!("cart")]);
    assert_eq!(rejects(call(&mut s, "scene.track.remove", json!({ "item": "bird" }), "moving")), "no_track");
}

#[test]
fn r12_a_scene_with_no_motion_gets_one_from_scene_track_add_duration_or_scene_motion_set_the_loops_length_cant_cut_off_keys() {
    let mut s = still();
    assert_eq!(code(motion_ops::add_track(&mut s, "lone", None)), "no_motion");
    motion_ops::add_track(&mut s, "lone", Some(3000.0)).unwrap();
    assert_eq!(s["motion"], json!({ "duration": 3000, "tracks": [{ "item": "lone", "keys": [{ "t": 0 }] }] }));
    let mut m = moving();
    let r = motion_ops::set_duration(&mut m, 900.0).unwrap_err();
    assert_eq!(r.code, "out_of_range");
    assert_eq!(r.message, "a 900 ms loop would cut off key: cart at 1000 ms; move or delete it first");
    let r = motion_ops::set_duration(&mut m, 4000.0).unwrap();
    assert_eq!(r["message"], "The loop is 4000 ms long");
    assert_eq!(m["motion"]["duration"], 4000);
}

#[test]
fn r12_the_commands_are_one_shot_edits_on_the_command_line_too() {
    let fx = Fx::new();
    let (code, out, err) = cli(&fx, &["scene", "key", "set", "moving", "cart", "--time", "1500", "--no-show"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("Made cart's key at 1500 ms: show false"), "{out}");
    assert!(out.contains("warning: scenes/moving.json: motion.tracks[0] (cart): 0 \u{2192} 1000 ms moves (16, -3) in 1 step"), "{out}");
    assert!(out.contains("wrote scenes/moving.json"), "{out}");
}

#[test]
fn r13_on_the_grid_means_each_step_is_1_px_straight_1_1_or_2_1() {
    for (dx, dy) in [(5.0, 0.0), (0.0, -3.0), (4.0, 4.0), (-6.0, 3.0), (8.0, -4.0), (0.0, 0.0)] {
        assert!(motion_ops::on_grid(dx, dy), "{dx},{dy}");
    }
    for (dx, dy) in [(16.0, -3.0), (6.0, 2.0), (2.0, 4.0), (3.0, 2.0)] {
        assert!(!motion_ops::on_grid(dx, dy), "{dx},{dy}");
    }
}

#[test]
fn r13_check_warns_about_each_off_grid_move_the_loop_back_included_without_failing() {
    let s = moving();
    let moves = motion_ops::off_grid_moves(&s);
    // From Node.
    assert_eq!(
        Value::Array(moves.clone()),
        json!([
            { "track": 0, "item": "cart", "from": 0, "to": 1000, "move": [16, -3], "steps": 1, "step": [16, -3] },
            { "track": 0, "item": "cart", "from": 1000, "to": 2000, "move": [-16, 3], "steps": 1, "step": [-16, 3] },
        ])
    );
    assert_eq!(
        moves.iter().map(|w| motion_ops::off_grid_text(FILE, w)).collect::<Vec<_>>(),
        vec![
            "scenes/moving.json: motion.tracks[0] (cart): 0 → 1000 ms moves (16, -3) in 1 step of (16, -3); on the grid each step is 1 px straight, 1:1 or 2:1 (scene_motion_path makes such keys)",
            "scenes/moving.json: motion.tracks[0] (cart): 1000 → 2000 ms moves (-16, 3) in 1 step of (-16, 3); on the grid each step is 1 px straight, 1:1 or 2:1 (scene_motion_path makes such keys)",
        ]
    );
    // A hidden item's move isn't seen; a scene with no motion has none (an object has none).
    let mut hidden = moving();
    hidden["motion"]["tracks"][0]["keys"] = json!([{ "t": 0, "show": false }, { "t": 1000, "move": [16, -3] }]);
    assert!(motion_ops::off_grid_moves(&hidden).is_empty()); // from Node
    assert!(motion_ops::off_grid_moves(&still()).is_empty());
    assert!(motion_ops::off_grid_moves(&json!({ "size": [4, 4] })).is_empty());
    // A one-step key list ending early, from Node: the loop back is cut short by the last key's own duration.
    let mut early = moving();
    early["motion"]["tracks"][0]["keys"].as_array_mut().unwrap().push(json!({ "t": 1500, "show": false, "move": [1, 1] }));
    assert_eq!(
        Value::Array(motion_ops::off_grid_moves(&early)),
        json!([
            { "track": 0, "item": "cart", "from": 0, "to": 1000, "move": [16, -3], "steps": 1, "step": [16, -3] },
            { "track": 0, "item": "cart", "from": 1000, "to": 1500, "move": [-15, 4], "steps": 1, "step": [-15, 4] },
        ])
    );
}

#[test]
fn r13_check_warns_about_each_off_grid_move_the_loop_back_included_without_failing_through_check() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = registry().call(&mut Ctx::new(&mut s), "check", json!({ "name": "moving" }), false).unwrap();
    assert_eq!(r["ok"], true);
    let texts: Vec<String> = r["warnings"].as_array().unwrap().iter().map(|w| w["text"].as_str().unwrap().to_string()).collect();
    assert_eq!(
        texts,
        vec![
            "scenes/moving.json: motion.tracks[0] (cart): 0 \u{2192} 1000 ms moves (16, -3) in 1 step of (16, -3); on the grid each step is 1 px straight, 1:1 or 2:1 (scene_motion_path makes such keys)",
            "scenes/moving.json: motion.tracks[0] (cart): 1000 \u{2192} 2000 ms moves (-16, 3) in 1 step of (-16, 3); on the grid each step is 1 px straight, 1:1 or 2:1 (scene_motion_path makes such keys)",
        ]
    );
    let all = registry().call(&mut Ctx::new(&mut s), "check", json!({}), false).unwrap();
    // (the sample's pebble stands partly outside its viewBox: scene-canvas spec R11, the third)
    let off_grid = all["warnings"].as_array().unwrap().iter().filter(|w| w["code"] == "off_grid").count();
    assert_eq!(off_grid, 2, "a check of everything lists them too");
    let obj = registry().call(&mut Ctx::new(&mut s), "check", json!({ "name": "pebble" }), false).unwrap();
    assert_eq!(obj["warnings"], json!([]), "an object has none");
    let (code, out, _) = cli(&fx, &["check", "moving"]);
    assert_eq!(code, 0, "warnings keep the exit code 0");
    assert!(out.lines().any(|l| l == "2 warnings"), "{out}");
}

#[test]
fn r14_a_segment_splits_into_on_grid_runs() {
    assert_eq!(motion_ops::grid_runs(16.0, -3.0), vec![[6.0, -3.0], [10.0, 0.0]]);
    assert_eq!(motion_ops::grid_runs(5.0, 4.0), vec![[2.0, 1.0], [3.0, 3.0]]);
    assert_eq!(motion_ops::grid_runs(-2.0, 7.0), vec![[-2.0, 2.0], [0.0, 5.0]]);
    assert_eq!(motion_ops::grid_runs(8.0, 4.0), vec![[8.0, 4.0]]);
    // From Node.
    assert!(motion_ops::grid_runs(0.0, 0.0).is_empty());
    assert_eq!(motion_ops::grid_runs(-7.0, 0.0), vec![[-7.0, 0.0]]);
    assert_eq!(motion_ops::grid_runs(0.0, -5.0), vec![[0.0, -5.0]]);
    assert_eq!(motion_ops::grid_runs(-9.0, -4.0), vec![[-8.0, -4.0], [-1.0, 0.0]]);
    assert_eq!(motion_ops::grid_runs(1.0, 10.0), vec![[1.0, 1.0], [0.0, 9.0]]);
}

#[test]
fn r14_a_polyline_becomes_on_grid_keys_at_a_steady_speed_keeping_the_show_and_pose_keys_on_the_way() {
    let mut s = moving();
    motion_ops::set_key(
        &mut s,
        "cart",
        1000.0,
        &vals(|v| {
            v.clear = vec!["move".into()];
            v.pose = Some("block".into());
        }),
    )
    .unwrap();
    let r = motion_ops::motion_path(
        &mut s,
        "cart",
        &[[0.0, 0.0], [16.0, -3.0], [16.0, 5.0]],
        PathTiming { start: 100.0, speed: Some(20.0), end: None },
    )
    .unwrap();
    // Runs: (6,-3) √45 ≈ 6.71, (10,0) 10, (0,8) 8; at 20 px/s from 100 ms.
    assert_eq!(
        r["keys"],
        json!([{ "t": 100, "move": [0, 0] }, { "t": 435, "move": [6, -3] }, { "t": 935, "move": [16, -3] }, { "t": 1335, "move": [16, 5] }])
    );
    assert_eq!(r["end"], 1335);
    assert_eq!(r["message"], "Keyed cart along 3 points: 3 on-grid runs, 100 ms to 1335 ms");
    let keys = keys_of(&s, "cart");
    assert_eq!(
        keys.as_array().unwrap().iter().map(|k| k["t"].clone()).collect::<Vec<_>>(),
        vec![json!(0), json!(100), json!(435), json!(935), json!(1000), json!(1335)]
    );
    // The pose key at 1000 ms sits on the path: 65 of 400 ms into the 8-step run down, so 1 step down.
    assert_eq!(keys[4], json!({ "t": 1000, "move": [16, -2], "pose": "block" }));
    let off: Vec<Value> = motion_ops::off_grid_moves(&s).into_iter().filter(|w| w["to"] != 2000).collect();
    assert!(off.is_empty(), "every move on the path is on the grid");
    // The step from the path's end to the next key (the loop back) stays off the grid.
    let left = motion_ops::off_grid_moves(&s);
    assert_eq!(
        motion_ops::off_grid_text(FILE, &left[0]),
        "scenes/moving.json: motion.tracks[0] (cart): 1335 → 2000 ms moves (-16, -5) in 1 step of (-16, -5); on the grid each step is 1 px straight, 1:1 or 2:1 (scene_motion_path makes such keys)"
    );
}

#[test]
fn r14_end_spreads_the_path_between_two_times_too_long_a_path_or_a_bad_one_is_refused() {
    let mut s = moving();
    motion_ops::add_track(&mut s, "bird", None).unwrap();
    let r = motion_ops::motion_path(
        &mut s,
        "bird",
        &[[0.0, 0.0], [8.0, 4.0], [8.0, 0.0]],
        PathTiming { start: 0.0, speed: None, end: Some(1000.0) },
    )
    .unwrap();
    let times: Vec<Value> = r["keys"].as_array().unwrap().iter().map(|k| k["t"].clone()).collect();
    assert_eq!(times, vec![json!(0), json!(691), json!(1000)]);
    // From Node.
    assert_eq!(keys_of(&s, "bird"), json!([{ "t": 0, "move": [0, 0] }, { "t": 691, "move": [8, 4] }, { "t": 1000, "move": [8, 0] }]));
    assert_eq!(r["message"], "Keyed bird along 3 points: 2 on-grid runs, 0 ms to 1000 ms");

    let timing = PathTiming { start: 0.0, speed: Some(10.0), end: None };
    let e = motion_ops::motion_path(&mut s, "bird", &[[0.0, 0.0], [100.0, 0.0]], timing).unwrap_err();
    assert_eq!(e.code, "too_long");
    assert_eq!(e.message, "at 10 px/s the path takes 10000 ms from 0 ms, ending at 10000 ms, past the 2000 ms loop");
    let e = motion_ops::motion_path(&mut s, "bird", &[[0.0, 0.0], [0.0, 0.0]], timing).unwrap_err();
    assert_eq!((e.code.as_str(), e.message.as_str()), ("usage", "the path goes nowhere: every point is the same"));
    let e = motion_ops::motion_path(&mut s, "bird", &[[0.0, 0.0], [4.0, 0.0]], PathTiming { start: 0.0, speed: None, end: Some(0.0) })
        .unwrap_err();
    assert_eq!((e.code.as_str(), e.message.as_str()), ("usage", "end (0 ms) must be after start (0 ms)"));
    let e = motion_ops::motion_path(&mut s, "bird", &[[0.0, 0.0]], timing).unwrap_err();
    assert_eq!(e.code, "usage");
    // The command's own refusals (neither speed nor end, an item with no id) belong to U23.
    assert_eq!(code(motion_ops::motion_path(&mut s, "nope", &[[0.0, 0.0], [4.0, 0.0]], timing)), "no_track");
}

#[test]
fn r14_end_spreads_the_path_between_two_times_too_long_a_path_or_a_bad_one_is_refused_through_the_command() {
    let fx = Fx::new();
    let mut s = fx.session();
    call(&mut s, "scene.track.add", json!({ "item": "bird" }), "moving").unwrap();
    let r =
        call(&mut s, "scene.motion.path", json!({ "item": "bird", "points": [[0, 0], [8, 4], [8, 0]], "start": 0, "end": 1000 }), "moving")
            .unwrap();
    let times: Vec<Value> = r["keys"].as_array().unwrap().iter().map(|k| k["t"].clone()).collect();
    assert_eq!(times, [json!(0), json!(691), json!(1000)]);
    let bad = |s: &mut Session, p: Value| rejects(call(s, "scene.motion.path", p, "moving"));
    assert_eq!(bad(&mut s, json!({ "item": "bird", "points": [[0, 0], [100, 0]], "speed": 10 })), "too_long");
    assert_eq!(bad(&mut s, json!({ "item": "bird", "points": [[0, 0], [0, 0]], "speed": 10 })), "usage");
    assert_eq!(bad(&mut s, json!({ "item": "bird", "points": [[0, 0], [4, 0]] })), "usage");
    assert_eq!(bad(&mut s, json!({ "item": "3", "points": [[0, 0], [4, 0]], "speed": 10 })), "no_id");
}

#[test]
fn r14_a_polyline_becomes_on_grid_keys_at_a_steady_speed_keeping_the_show_and_pose_keys_on_the_way_through_the_command() {
    let fx = Fx::new();
    let mut s = fx.session();
    call(&mut s, "scene.key.set", json!({ "item": "cart", "time": 1000, "clear": ["move"], "pose": "block" }), "moving").unwrap();
    let r = call(
        &mut s,
        "scene.motion.path",
        json!({ "item": "cart", "points": [[0, 0], [16, -3], [16, 5]], "start": 100, "speed": 20 }),
        "moving",
    )
    .unwrap();
    assert_eq!(
        r["keys"],
        json!([{ "t": 100, "move": [0, 0] }, { "t": 435, "move": [6, -3] }, { "t": 935, "move": [16, -3] }, { "t": 1335, "move": [16, 5] }])
    );
    let keys = keys_of_scene(&mut s, "cart");
    let times: Vec<Value> = keys.as_array().unwrap().iter().map(|k| k["t"].clone()).collect();
    assert_eq!(times, [json!(0), json!(100), json!(435), json!(935), json!(1000), json!(1335)]);
    assert_eq!(keys[4], json!({ "t": 1000, "move": [16, -2], "pose": "block" }));
    assert_eq!(
        r["warnings"],
        json!(["scenes/moving.json: motion.tracks[0] (cart): 1335 → 2000 ms moves (-16, -5) in 1 step of (-16, -5); on the grid each step is 1 px straight, 1:1 or 2:1 (scene_motion_path makes such keys)"])
    );
    assert_eq!(problems(&mut s, "moving"), Vec::<String>::new());
}

#[test]
fn motion_ops_messages_and_codes_match_node() {
    // From Node.
    let mut s = moving();
    motion_ops::add_track(&mut s, "bird", None).unwrap();
    let cases: Vec<(Result<Value, CommandError>, &str, &str)> = vec![
        (motion_ops::set_key(&mut s, "bird", -1.0, &KeyValues::default()), "usage", "a key's time is a whole number of ms from 0"),
        (
            motion_ops::set_key(&mut s, "bird", 2000.0, &KeyValues::default()),
            "out_of_range",
            "2000 ms is past the loop, which is 2000 ms long (0 to 1999)",
        ),
        (
            motion_ops::set_key(&mut s, "bird", 5.0, &vals(|v| v.clear = vec!["x".into()])),
            "usage",
            "clear takes move, show, pose or state, not x",
        ),
        (
            motion_ops::move_key(&mut s, "bird", 0.0, 100.0),
            "first_key",
            "the key at 0 is the rest frame and stays there; set its values instead",
        ),
        (motion_ops::move_key(&mut s, "bird", 77.0, 100.0), "not_found", "bird has no key at 77 ms; its keys are at 0, 5 ms"),
        (
            motion_ops::delete_key(&mut s, "bird", 0.0),
            "first_key",
            "the key at 0 is the rest frame; remove the track to drop it (scene.track.remove)",
        ),
        (motion_ops::delete_key(&mut s, "bird", 77.0), "not_found", "bird has no key at 77 ms; its keys are at 0, 5 ms"),
        (motion_ops::add_track(&mut s, "bird", None), "track_exists", "bird already has a track"),
        (motion_ops::add_track(&mut s, "zz", Some(999.0)), "usage", "the scene's motion is 2000 ms long; scene.motion.set changes that"),
        (
            motion_ops::set_key(&mut s, "nope", 5.0, &KeyValues::default()),
            "no_track",
            "nope has no motion track (scene.track.add makes one)",
        ),
    ];
    for (r, c, m) in cases {
        let e = r.unwrap_err();
        assert_eq!((e.code.as_str(), e.message.as_str()), (c, m));
    }
    motion_ops::set_key(&mut s, "bird", 1200.0, &vals(|v| v.show = Some(true))).unwrap();
    let e = motion_ops::set_duration(&mut s, 900.0).unwrap_err();
    assert_eq!(e.message, "a 900 ms loop would cut off keys: cart at 1000 ms, bird at 1200 ms; move or delete them first");
    let mut empty = json!({});
    assert_eq!(motion_ops::set_duration(&mut empty, 500.0).unwrap()["message"], "Made the scene's motion, 500 ms long");
}

// --- helpers for the command-level tests ---

/// A temp copy of the sample document with the moving and still scenes added, and a state folder.
struct Fx {
    doc: common::TempDocument,
    state: std::path::PathBuf,
}

impl Fx {
    fn new() -> Fx {
        let doc = common::make_document();
        let state = common::temp_dir("pixellab-state-");
        for (name, scene) in [("moving", moving()), ("still", still())] {
            doc.write(&format!("scenes/{name}.json"), &engine::format::serialize(&scene));
        }
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

fn cli(fx: &Fx, args: &[&str]) -> (i32, String, String) {
    let reg = registry();
    let mut env = fx.doc.env();
    env.insert("PIXELLAB_STATE".into(), fx.state.to_string_lossy().into_owned());
    let mut argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    argv.push("--document".into());
    argv.push(fx.doc.root.to_string_lossy().into_owned());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = {
        let mut io = pixellab::cli::Io { stdout: &mut out, stderr: &mut err, env, cwd: fx.doc.root.clone(), registry: &reg };
        pixellab::cli::main(&argv, &mut io)
    };
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn call(s: &mut Session, name: &str, params: Value, scene: &str) -> Result<Value, Error> {
    let mut p = json!({ "name": scene });
    for (k, v) in params.as_object().unwrap() {
        p[k] = v.clone();
    }
    let mut ctx = Ctx::new(s);
    registry().call(&mut ctx, name, p, false)
}

fn rejects(r: Result<Value, Error>) -> String {
    match r.expect_err("an error") {
        Error::Command(c) => c.code,
        Error::Internal(m) => m,
    }
}

fn tracks(s: &mut Session) -> Value {
    s.doc(DocKind::Scene, "moving").unwrap()["motion"]["tracks"].clone()
}

fn keys_of_scene(s: &mut Session, id: &str) -> Value {
    tracks(s).as_array().unwrap().iter().find(|t| t["item"] == json!(id)).expect("a track")["keys"].clone()
}

/// The engine's problems for a scene of the session, next to the sample document's palette and objects.
fn problems(s: &mut Session, name: &str) -> Vec<String> {
    let docs = read_docs(&common::fixture());
    let objects = BTreeMap::from_iter(docs.objects.clone());
    let scenes = BTreeMap::from([(name.to_string(), s.doc(DocKind::Scene, name).unwrap())]);
    let all = load_sources(docs.palette.clone().unwrap_or(Value::Null), objects, scenes);
    let prefix = format!("scenes/{name}.json: ");
    validate(&all).into_iter().filter_map(|p| p.strip_prefix(&prefix).map(str::to_string)).collect()
}
