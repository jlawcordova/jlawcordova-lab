//! The engine tests of the site's `pixel-art-motion` suite (R20, D12),
//! rewritten against the committed fixture (`fixtures/sample.pixellab`,
//! format 2), names kept. The fixture has no animated layer and no motion
//! scene, so the scene (`demo`) and the two small sprites (`dot`, `box`)
//! are added in memory to the fixture's own sources, and the animated layer
//! is built on a document value with the engine's edit functions.
//!
//! Where a site test pinned the CSS class names (`mv-<id>`), these assert
//! the CSS's structure and timing through `class_of`, which reads the names
//! from `engine::motion`, so scoping the names later changes nothing here.
//! Those tests are "writes stepped keyframes…" and "the optimizer keeps the
//! <style>…".
//!
//! Not ported (the site's tests of the owner's own art, dropped by R20):
//! the rest of `pixel-art-island`, `pixel-art-sources`, `pixel-art-roundtrip`,
//! `range-art` and `achievement-icons`, and the checks of the art against
//! the site's own files. `pixel-art-motion` itself has no such test.

mod common;

use std::collections::BTreeMap;

use engine::compile::{compile_scene, Budgets};
use engine::edit::{animate_layer, set_loop, still_layer};
use engine::engine::{composite, load_sources, render_scene, validate, CompositeOptions, Order, Sources};
use engine::format::{read_docs, serialize};
use engine::motion::{loop_frame, motion_class, motion_css, pose_class, pose_items, shown_items, state_at, steps_for};
use engine::omap::OMap;
use serde_json::{json, Value};

/// The fixture's sources with the scene `demo` and the sprites `dot` and `box` added.
fn src(scene: Value) -> Sources {
    let docs = read_docs(&common::fixture());
    assert_eq!(docs.load_errors, Vec::<String>::new());
    let mut objects: BTreeMap<String, Value> = docs.objects;
    objects.insert("dot".into(), json!({ "kind": "sprite", "anchor": [0, 0], "keys": { "g": "grass-2" }, "layers": [{ "map": ["g"] }] }));
    objects.insert("box".into(), json!({ "kind": "sprite", "anchor": [0, 0], "keys": { "w": "wood-3" }, "layers": [{ "map": ["ww"] }] }));
    let mut scenes = docs.scenes;
    scenes.insert("demo".into(), scene);
    load_sources(docs.palette.unwrap(), objects, scenes)
}

fn scene(motion: Value, items: Value) -> Value {
    json!({ "viewBox": [0, 0, 40, 20], "items": items, "motion": motion })
}

fn mover() -> Value {
    json!([{ "object": "dot", "id": "mover", "at": { "px": [2, 2] } }])
}

fn problems(s: Value) -> Vec<String> {
    validate(&src(s)).into_iter().filter_map(|p| p.strip_prefix("scenes/demo.json: ").map(str::to_string)).collect()
}

fn track(keys: Value) -> Value {
    track_of("mover", keys)
}

fn track_of(item: &str, keys: Value) -> Value {
    json!({ "item": item, "keys": keys })
}

fn ids() -> OMap<String, Value> {
    let mut ids = OMap::new();
    ids.set("mover".to_string(), json!({ "object": "dot" }));
    ids
}

fn no_copies() -> OMap<String, Vec<engine::depth::Copy>> {
    OMap::new()
}

fn state(move_: [f64; 2], show: bool, pose: &str) -> engine::motion::State {
    engine::motion::State { moved: move_, show, pose: pose.to_string() }
}

#[test]
fn motion_validation_a_well_formed_motion_has_no_problems() {
    let motion =
        json!({ "duration": 1000, "tracks": [track(json!([{ "t": 0 }, { "t": 500, "move": [4, 2], "show": false, "pose": "box" }]))] });
    assert_eq!(problems(scene(motion, mover())), Vec::<String>::new());
}

#[test]
fn motion_validation_names_each_broken_rule() {
    let bad = scene(
        json!({
            "duration": 1000,
            "speed": 2,
            "tracks": [
                track(json!([{ "t": 5 }])),
                track_of("mover", json!([{ "t": 0 }, { "t": 0 }])),
                track_of("ghost", json!([{ "t": 0 }, { "t": 1000 }, { "t": 1200, "move": [1.5, 0], "show": "yes", "pose": "nope", "ease": "in" }])),
            ],
        }),
        json!([
            { "object": "dot", "id": "mover", "at": { "px": [0, 0] } },
            { "object": "dot", "id": "mover", "at": { "px": [1, 0] } },
            { "object": "dot", "id": "Bad Id", "at": { "px": [2, 0] } },
        ]),
    );
    assert_eq!(
        problems(bad),
        vec![
            "items[1]: id 'mover' is already used in this scene",
            "items[2]: id must be lowercase kebab-case",
            "motion: unknown property 'speed'",
            "motion.tracks[0].keys[0]: the first key must be at t 0, the rest frame",
            "motion.tracks[1]: item 'mover' already has a track",
            "motion.tracks[1].keys[1]: keys must be in time order, each later than the last",
            "motion.tracks[2]: item 'ghost' is no item's id",
            "motion.tracks[2].keys[1]: t must be less than the duration",
            "motion.tracks[2].keys[2]: unknown property 'ease'",
            "motion.tracks[2].keys[2]: t must be less than the duration",
            "motion.tracks[2].keys[2]: move must be two whole numbers",
            "motion.tracks[2].keys[2]: show must be true or false",
            "motion.tracks[2].keys[2]: pose 'nope' does not exist",
        ]
    );
}

#[test]
fn motion_validation_a_duration_under_100_ms_or_tracks_that_are_not_a_list_are_refused() {
    assert_eq!(
        problems(scene(json!({ "duration": 50, "tracks": {} }), mover())),
        vec!["motion: duration must be a whole number of ms, at least 100", "motion: tracks must be a list"]
    );
}

#[test]
fn motion_stepping_a_move_takes_one_step_per_pixel_along_a_straight_or_2_1_line() {
    assert_eq!(steps_for(0.0, 0.0), 0.0);
    assert_eq!(steps_for(-12.0, 0.0), 12.0);
    assert_eq!(steps_for(32.0, -16.0), 16.0);
    assert_eq!(steps_for(6.0, 9.0), 3.0);
}

#[test]
fn motion_stepping_steps_toward_the_next_key_changes_show_and_pose_at_their_key_and_loops_back_to_the_first_key() {
    let t = track(json!([{ "t": 0, "show": false }, { "t": 100, "show": true }, { "t": 900, "move": [16, -8], "pose": "box" }]));
    let at = |time: f64| state_at(&t, "dot", 1000.0, time);
    assert_eq!(at(0.0), state([0.0, 0.0], false, "dot"));
    assert_eq!(at(100.0), state([0.0, 0.0], true, "dot"));
    assert_eq!(at(500.0), state([8.0, -4.0], true, "dot"));
    assert_eq!(at(899.0), state([14.0, -7.0], true, "dot"));
    assert_eq!(at(900.0), state([16.0, -8.0], true, "box"));
    assert_eq!(at(950.0), state([8.0, -4.0], true, "box"), "the last key steps back to the first over the rest of the loop");
    assert_eq!(at(1500.0), at(500.0), "time wraps around the loop");
}

#[test]
fn motion_stepping_pose_items_moves_re_poses_and_hides_tracked_items_and_leaves_the_rest_alone() {
    let items = vec![
        json!({ "object": "box", "at": { "px": [0, 0] } }),
        json!({ "group": { "class": "g" }, "items": [{ "object": "dot", "id": "mover", "at": { "px": [2, 2] } }] }),
    ];
    let motion =
        json!({ "duration": 1000, "tracks": [track(json!([{ "t": 0 }, { "t": 500, "move": [4, 0], "pose": "box", "show": false }]))] });
    let posed = pose_items(&items, Some(&motion), 500.0, &|item: &Value| {
        let p = &item["at"]["px"];
        [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]
    });
    assert_eq!(posed[0], items[0]);
    assert_eq!(posed[1]["items"][0], json!({ "object": "box", "id": "mover", "at": { "px": [6, 2] }, "hidden": true }));
    assert_eq!(shown_items(&posed)[1]["items"], json!([]));
}

#[test]
fn motion_stepping_loop_frame_follows_each_loops_css_cycle() {
    assert_eq!(loop_frame("wf", 5, 0.0), 0);
    assert_eq!(loop_frame("wf", 5, 450.0), 2);
    assert_eq!(loop_frame("ff", 4, 600.0), 0);
    assert_eq!(loop_frame("other", 2, 250.0), 1);
}

fn motion() -> Value {
    json!({
        "duration": 2000,
        "tracks": [track(json!([{ "t": 0, "show": false }, { "t": 500, "show": true }, { "t": 1000, "move": [4, -2], "pose": "box" }, { "t": 1500, "show": false }]))],
    })
}

#[test]
fn motion_compiled_draws_the_rest_frame_hidden_items_and_unused_poses_never_composite() {
    let rendered = render_scene(&src(scene(motion(), mover())), "demo", Order::Depth).unwrap();
    assert_eq!(composite(&rendered.root, CompositeOptions::default()).len(), 0, "the mover is hidden at rest");
    assert!(rendered.style.is_some());
    let shown = json!({ "duration": 2000, "tracks": [track(json!([{ "t": 0, "move": [1, 0] }, { "t": 1000, "pose": "box" }]))] });
    let rest = render_scene(&src(scene(shown, mover())), "demo", Order::Depth).unwrap();
    let pixels = composite(&rest.root, CompositeOptions::default());
    assert_eq!(pixels.len(), 1, "the rest key's move is drawn in, and only the first pose shows");
    assert_eq!(pixels.keys().copied().collect::<Vec<_>>(), vec![(3, 2)]);
}

#[test]
fn motion_compiled_writes_stepped_keyframes_show_and_pose_animations_and_turns_them_off_under_reduced_motion() {
    let css = motion_css(&motion(), &ids(), &no_copies());
    // The class names come from the engine, so scoping them later changes nothing here.
    let (cls, pose1) = (motion_class("mover"), pose_class("mover", 1));
    let has = |part: String| assert!(css.contains(&part), "{part} in {css}");
    has(format!("@keyframes {cls}{{0%{{transform:translate(0,0)}}25%{{transform:translate(0,0);animation-timing-function:steps(2,end)}}50%{{transform:translate(4px,-2px)}}"));
    has(format!("@keyframes {cls}-s{{0%{{opacity:0}}25%{{opacity:1}}50%{{opacity:1}}75%{{opacity:0}}}}"));
    has(format!(".{cls}{{opacity:0;animation:{cls} 2000ms step-end infinite,{cls}-s 2000ms step-end infinite}}"));
    has(format!(".{pose1}{{opacity:0;animation:{pose1} 2000ms step-end infinite}}"));
    let tail = css.rsplit_once("@media ").expect("a reduced-motion rule").1;
    assert!(tail.starts_with("(prefers-reduced-motion:reduce){[class^=\""), "{tail}");
    assert!(tail.ends_with("{animation:none}}"), "{tail}");
}

#[test]
fn motion_compiled_a_still_track_writes_no_css() {
    let still = json!({ "duration": 1000, "tracks": [track(json!([{ "t": 0 }]))] });
    assert_eq!(motion_css(&still, &ids(), &no_copies()), "");
}

#[test]
fn motion_compiled_the_optimizer_keeps_the_style_and_stays_lossless() {
    let mut s = scene(motion(), mover());
    s["output"] = json!("demo.svg");
    let compiled = compile_scene(&src(s), "demo", &Budgets::new(), Order::Depth).unwrap();
    // A compiled SVG scopes the engine's names to its scene (spec R15).
    let (cls, pose0) = (format!("demo--{}", motion_class("mover")), format!("demo--{}", pose_class("mover", 0)));
    assert!(
        compiled.output.starts_with("<svg ") && compiled.output.contains(&format!("><style>@keyframes {cls}{{")),
        "{}",
        compiled.output
    );
    assert!(compiled.output.contains(&format!("<g class=\"{cls}\"><g class=\"{pose0}\">")), "{}", compiled.output);
}

#[test]
fn motion_compiled_serializes_canonically_ids_after_the_object_then_motion_with_one_key_per_line() {
    let motion =
        json!({ "duration": 1000, "tracks": [track(json!([{ "t": 0 }, { "t": 500, "move": [4, -2], "show": false, "pose": "box" }]))] });
    let text = serialize(&scene(motion, mover()));
    assert!(text.contains(r#"{ "object": "dot", "id": "mover", "at": { "px": [2, 2] } }"#), "{text}");
    let expected = concat!(
        "  \"motion\": {\n    \"duration\": 1000,\n    \"tracks\": [\n      {\n        \"item\": \"mover\",\n        \"keys\": [\n",
        "          { \"t\": 0 },\n          { \"t\": 500, \"move\": [4, -2], \"show\": false, \"pose\": \"box\" }\n",
        "        ]\n      }\n    ]\n  }"
    );
    assert!(text.contains(expected), "{text}");
}

fn layer_doc() -> Value {
    json!({ "kind": "sprite", "keys": { "g": "grass-2" }, "layers": [{ "class": "cbob", "map": ["g.", ".g"] }] })
}

#[test]
fn frame_loops_animating_a_layer_a_still_layer_becomes_one_of_the_sites_loops_its_map_copied_into_every_frame() {
    let mut d = layer_doc();
    animate_layer(&mut d, 0, "ff");
    assert_eq!(d["layers"][0], json!({ "loop": "ff", "prefix": "f", "frames": [["g.", ".g"], ["g.", ".g"], ["g.", ".g"], ["g.", ".g"]] }));
    d["layers"][0]["frames"][3] = json!(["gg", ".."]);
    let text = serialize(&d);
    assert!(text.contains("\"loop\": \"ff\",\n      \"prefix\": \"f\",\n      \"frames\": ["), "{text}");
}

#[test]
fn frame_loops_animating_a_layer_switching_loops_follows_the_prefix_and_pads_with_the_last_frame_keeping_extras() {
    let mut d = layer_doc();
    animate_layer(&mut d, 0, "ff");
    d["layers"][0]["frames"][3] = json!(["gg", ".."]);
    set_loop(&mut d, 0, "hf");
    assert_eq!(d["layers"][0]["prefix"], "h");
    assert_eq!(d["layers"][0]["frames"].as_array().unwrap().len(), 6);
    assert_eq!(d["layers"][0]["frames"][5], json!(["gg", ".."]));
    set_loop(&mut d, 0, "wf");
    let l = &d["layers"][0];
    assert_eq!((l["loop"].clone(), l["prefix"].clone(), l["frames"].as_array().unwrap().len()), (json!("wf"), json!("w"), 6));
}

#[test]
fn frame_loops_animating_a_layer_stopping_keeps_the_chosen_frame_as_a_still_map() {
    let mut d = layer_doc();
    animate_layer(&mut d, 0, "wf");
    d["layers"][0]["frames"][2] = json!(["..", "gg"]);
    still_layer(&mut d, 0, 2);
    assert_eq!(d["layers"][0], json!({ "map": ["..", "gg"] }));
}
