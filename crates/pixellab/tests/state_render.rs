//! Renders of objects with states (docs/intents/2026-10-object-states/spec.md, R6, R15, R16, D2,
//! "Renders"): an object at a state and a time, and a scene placing each item in its state.

mod common;

use engine::engine::{load_sources, Sources};
use engine::format::read_docs;
use engine::png::{decode, Image};
use pixellab::render::*;
use serde_json::Value;

fn sources() -> Sources {
    let docs = read_docs(&common::repo().join("fixtures/states.pixellab"));
    assert!(docs.load_errors.is_empty(), "{:?}", docs.load_errors);
    load_sources(docs.palette.unwrap(), docs.objects, docs.scenes)
}

fn unbase64(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let mut out = Vec::new();
    for chunk in text.as_bytes().chunks(4) {
        let n: Vec<u32> = chunk.iter().filter(|&&c| c != b'=').map(|&c| value(c) as u32).collect();
        let bits = n.iter().fold(0u32, |a, &v| a << 6 | v) << (6 * (4 - n.len()));
        for i in 0..n.len() - 1 {
            out.push((bits >> (16 - 8 * i)) as u8);
        }
    }
    out
}

fn image(r: &Value) -> Image {
    decode(&unbase64(r["png"].as_str().unwrap())).unwrap()
}

/// What an object render at a state and time says about its one item.
fn at(sources: &Sources, object: &str, state: Option<&str>, time: Option<f64>) -> Value {
    render_object_image_in(sources, object, &ObjectImageOptions { time, ..ObjectImageOptions::default() }, state).unwrap()
}

fn item(r: &Value) -> (String, u64) {
    let i = &r["items"][0];
    (i["state"].as_str().unwrap().to_string(), i["frame"].as_u64().unwrap())
}

/// The top row of the box the item paints in, in scene pixels (the render's box grows to hold what moves).
fn top(r: &Value) -> i64 {
    r["viewBox"][1].as_i64().unwrap() + r["items"][0]["bounds"][1].as_i64().unwrap()
}

#[test]
fn r6_an_object_render_at_a_state_and_time_shows_the_frame_and_move_that_state_has() {
    let s = sources();
    // idle starts the walker: frame 0, and its move steps up a pixel at 500 ms.
    assert_eq!(item(&at(&s, "walker", None, Some(0.0))), ("idle".into(), 0));
    let (rest, bob) = (at(&s, "walker", None, Some(100.0)), at(&s, "walker", None, Some(600.0)));
    assert_eq!(top(&bob), top(&rest) - 1, "idle bobs a pixel up at 500 ms");
    // hop from its first frame: frame 1 (not idle's 0), up two pixels once it has stepped, then it hands over to idle.
    assert_eq!(item(&at(&s, "walker", Some("hop"), Some(0.0))), ("hop".into(), 1));
    let hop = at(&s, "walker", Some("hop"), Some(100.0));
    assert_eq!(item(&hop), ("hop".into(), 1));
    assert_eq!(top(&hop), top(&at(&s, "walker", Some("hop"), Some(0.0))) - 2);
    assert_eq!(item(&at(&s, "walker", Some("hop"), Some(480.0))).1, 0, "idle takes over, from its first frame");
    // The frames of walk change every 150 ms, and the sprite looks different.
    let walk: Vec<u64> = [0.0, 150.0, 300.0, 450.0, 600.0].iter().map(|t| item(&at(&s, "walker", Some("walk"), Some(*t))).1).collect();
    assert_eq!(walk, [0, 1, 2, 1, 0]);
    let (a, b) = (image(&at(&s, "walker", Some("walk"), Some(0.0))), image(&at(&s, "walker", Some("walk"), Some(150.0))));
    assert_ne!(a.rgba, b.rgba, "another frame is another picture");
    // dead plays two frames and holds the second.
    let dead: Vec<u64> = [0.0, 99.0, 100.0, 5000.0].iter().map(|t| item(&at(&s, "walker", Some("dead"), Some(*t))).1).collect();
    assert_eq!(dead, [1, 1, 2, 2]);
}

#[test]
fn r6_a_state_alone_renders_its_first_frame_and_a_sprite_without_states_reports_none() {
    let s = sources();
    let r = render_object_image_in(&s, "walker", &ObjectImageOptions::default(), Some("walk")).unwrap();
    assert_eq!(item(&r), ("walk".into(), 0));
    // Without a time, frame says which frame of the sprite shows (what painting edits).
    let r = render_object_image_in(&s, "walker", &ObjectImageOptions { frame: 2, ..ObjectImageOptions::default() }, None).unwrap();
    assert_eq!(item(&r), ("idle".into(), 2));
    let plain = render_object_image(&s, "pebble", &ObjectImageOptions { time: Some(0.0), ..ObjectImageOptions::default() }).unwrap();
    assert!(plain["items"][0].get("state").is_none() && plain["items"][0].get("frame").is_none());
}

/// A scene render at a time: its image, and each item by id (or object) with its state and frame.
fn scene_at(s: &Sources, time: f64) -> Value {
    render_scene_image(s, "states", "#1C1712", &SceneImageOptions { time, ..SceneImageOptions::default() }).unwrap()
}

fn placed<'a>(r: &'a Value, id: &str) -> &'a Value {
    r["items"].as_array().unwrap().iter().find(|i| i["id"] == id).unwrap_or_else(|| panic!("no item {id}"))
}

fn state_of(i: &Value) -> (String, u64) {
    (i["state"].as_str().unwrap().to_string(), i["frame"].as_u64().unwrap())
}

#[test]
fn r15_a_scene_render_places_each_item_in_its_state_from_its_track_or_its_start() {
    let s = sources();
    // walker-a starts in idle. The second walker has its own state, walk, so it changes frame every 150 ms.
    for (t, frame) in [(0.0, 0), (150.0, 1), (300.0, 2), (450.0, 1), (600.0, 0)] {
        let r = scene_at(&s, t);
        let walkers: Vec<&Value> = r["items"].as_array().unwrap().iter().filter(|i| i["object"] == "walker").collect();
        assert_eq!(state_of(walkers[0]), ("idle".into(), 0), "{t} ms");
        assert_eq!(state_of(walkers[1]), ("walk".into(), frame), "{t} ms");
    }
    // lamp-b's track: idle until 1000 ms, flicker (frames 0, 1, 0, 1 every 80 ms) until 2000 ms, then idle.
    for (t, want) in [
        (0.0, ("idle", 0)),
        (999.0, ("idle", 0)),
        (1000.0, ("flicker", 0)),
        (1079.0, ("flicker", 0)),
        (1080.0, ("flicker", 1)),
        (1160.0, ("flicker", 0)),
        (1240.0, ("flicker", 1)),
        (1320.0, ("flicker", 0)),
        (1999.0, ("flicker", 0)),
        (2000.0, ("idle", 0)),
        (2999.0, ("idle", 0)),
        // The loop starts again: the same at every pass (R16).
        (3000.0 + 1080.0, ("flicker", 1)),
        (6000.0 + 2500.0, ("idle", 0)),
    ] {
        assert_eq!(state_of(placed(&scene_at(&s, t), "lamp-b")), (want.0.to_string(), want.1), "{t} ms");
    }
    // lamp-a has no track: it plays idle from time 0 and never flickers.
    assert_eq!(state_of(placed(&scene_at(&s, 1100.0), "lamp-a")), ("idle".into(), 0));
}

/// The pixels of the box an item of a scene render paints in.
fn crop(r: &Value, id: &str) -> Vec<u8> {
    let img = image(r);
    let [x, y, w, h] = [0, 1, 2, 3].map(|i| placed(r, id)["bounds"][i].as_u64().unwrap() as u32);
    let mut out = Vec::new();
    for row in y..y + h {
        let start = ((row * img.width + x) * 4) as usize;
        out.extend_from_slice(&img.rgba[start..start + (w * 4) as usize]);
    }
    out
}

#[test]
fn r15_the_picture_of_a_keyed_item_changes_with_its_frame() {
    let s = sources();
    let lamp = |t: f64| crop(&scene_at(&s, t), "lamp-b");
    assert_ne!(lamp(1000.0), lamp(1080.0), "flicker's second frame is another picture");
    assert_eq!(lamp(1000.0), lamp(1160.0), "and its third is its first again");
    assert_eq!(lamp(0.0), lamp(1000.0), "idle shows frame 0, as flicker starts on");
    assert_eq!(lamp(1080.0), lamp(1240.0));
    assert_eq!(lamp(1080.0), lamp(3000.0 + 1080.0), "the loop is the same on every pass");
}

/// The top row of an item's box in a scene render.
fn top_of(r: &Value, path: &str) -> i64 {
    r["items"].as_array().unwrap().iter().find(|i| i["path"] == path).unwrap_or_else(|| panic!("no item at {path}"))["bounds"][1]
        .as_i64()
        .unwrap()
}

#[test]
fn r15_a_scene_renders_boxes_follow_each_items_own_state_and_when_it_entered() {
    let mut s = sources();
    // The walker at 2 starts in walk, which doesn't move; walker-a (1) is in idle, which bobs up a pixel at 500 ms.
    let (early, late) = (scene_at(&s, 300.0), scene_at(&s, 600.0));
    assert_eq!(top_of(&late, "2"), top_of(&early, "2"), "walk doesn't move it");
    assert_eq!(top_of(&late, "1"), top_of(&early, "1") - 1, "idle does");
    // A pebble posed as a walker and keyed into idle at 1250 ms is at rest until idle's move, 500 ms after it entered.
    let scene = s.scenes.get_mut("states").unwrap();
    scene["items"][5]["id"] = serde_json::json!("peb");
    scene["motion"]["tracks"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({ "item": "peb", "keys": [{ "t": 0 }, { "t": 1250, "pose": "walker", "state": "idle" }] }));
    let s = load_sources(s.palette.clone(), s.objects.clone(), s.scenes.clone());
    let [rest, still, moved] = [1300.0, 1700.0, 1800.0].map(|t| top_of(&scene_at(&s, t), "5"));
    assert_eq!(still, rest, "at rest at 1700 ms");
    assert_eq!(moved, rest - 1, "up a pixel from 1750 ms");
}
