//! Ambient loops and actions (spec R12, R13, D6, D7): what they express,
//! how they're checked, where renders at a time put them, and the CSS a
//! compiled SVG plays them with.

use std::collections::BTreeMap;

use engine::ambient::{action_at, ambient_at, move_at, moves_of, Action, Ambient, Key};
use engine::compile::{compile_scene, Budgets};
use engine::engine::{composite, load_sources, render_scene, validate, CompositeOptions, Order, Sources};
use engine::format::{parse, serialize_file};
use serde_json::{json, Value};

fn ambient(duration: f64, offset: f64, keys: &[(f64, [f64; 2])]) -> Ambient {
    Ambient { duration, offset, keys: keys.iter().map(|&(t, moved)| Key { t, moved }).collect() }
}

/// `pcloud` in pixel-art.css: 0%,100% translateX(0), 50% translateX(12px),
/// steps(12,end) per interval, over `d` ms, started `delay` ms in.
fn pcloud_css(time: f64, d: f64, delay: f64) -> f64 {
    let f = (((time + delay) % d) + d) % d / d;
    if f < 0.5 {
        (12.0 * (f / 0.5)).floor()
    } else {
        12.0 - (12.0 * ((f - 0.5) / 0.5)).floor()
    }
}

#[test]
fn r12_d6_pclouds_three_drifts_are_expressed_exactly() {
    // .pcloud 16 s; .pc1 20 s, delay -7 s; .pc2 13 s, delay -3 s.
    for (d, delay) in [(16000.0, 0.0), (20000.0, 7000.0), (13000.0, 3000.0)] {
        let a = ambient(d, delay, &[(0.0, [0.0, 0.0]), (d / 2.0, [12.0, 0.0])]);
        let mut t = 0.5;
        while t < 2.0 * d {
            assert_eq!(ambient_at(&a, t), [pcloud_css(t, d, delay), 0.0], "{d} ms loop at {t} ms");
            t += 37.0;
        }
    }
}

#[test]
fn r12_d6_cbobs_bob_is_expressed_exactly() {
    // .cbob: 1 s step-end, translateY(-1px) at 50%.
    let a = ambient(1000.0, 0.0, &[(0.0, [0.0, 0.0]), (500.0, [0.0, -1.0])]);
    for t in [0.0, 1.0, 250.0, 499.0, 500.0, 501.0, 750.0, 999.0, 1000.0, 1499.0, 1500.0] {
        let want = if (t % 1000.0) < 500.0 { 0.0 } else { -1.0 };
        assert_eq!(ambient_at(&a, t), [0.0, want], "at {t} ms");
    }
}

#[test]
fn r13_d6_an_action_holds_its_last_key_until_its_duration_then_is_at_rest() {
    // The hop: up to -6 and back, then -1 on landing, a whole pixel at a time; hold keys make each jump.
    let hop = Action {
        duration: 480.0,
        keys: [(0.0, 0.0), (43.0, 0.0), (44.0, -2.0), (436.0, -2.0), (437.0, -1.0)]
            .iter()
            .map(|&(t, y)| Key { t, moved: [0.0, y] })
            .collect(),
    };
    assert_eq!(action_at(&hop, 0.0), Some([0.0, 0.0]));
    assert_eq!(action_at(&hop, 40.0), Some([0.0, 0.0]));
    assert_eq!(action_at(&hop, 100.0), Some([0.0, -2.0]));
    assert_eq!(action_at(&hop, 470.0), Some([0.0, -1.0]), "the last key holds");
    assert_eq!(action_at(&hop, 480.0), None, "ended");
    assert_eq!(action_at(&hop, -1.0), None);
}

#[test]
fn d6_an_element_plays_its_action_over_its_ambient_loop_and_goes_back_to_it_when_the_action_ends() {
    let m = moves_of(&json!({
        "ambient": { "duration": 1000, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 500, "move": [0, -1] }] },
        "actions": { "hop": { "duration": 200, "keys": [{ "t": 0, "move": [0, -4] }] } }
    }))
    .unwrap();
    assert_eq!(move_at(&m, 600.0, None), [0.0, -1.0]);
    assert_eq!(move_at(&m, 600.0, Some(("hop", 50.0))), [0.0, -4.0]);
    assert_eq!(move_at(&m, 600.0, Some(("hop", 250.0))), [0.0, -1.0], "after the action, the ambient loop");
    assert_eq!(move_at(&m, 600.0, Some(("wave", 50.0))), [0.0, -1.0], "an action it doesn't have");
}

// ------------------------------------------------------------- in documents

fn palette() -> Value {
    json!({ "world": { "ink": "#2E2418", "cream": "#F4EDE0", "grass-2": "#8FA56E" }, "outfit": {} })
}

fn sources(objects: Vec<(&str, Value)>, scenes: Vec<(&str, Value)>) -> Sources {
    let map = |v: Vec<(&str, Value)>| v.into_iter().map(|(k, v)| (k.to_string(), v)).collect::<BTreeMap<_, _>>();
    load_sources(palette(), map(objects), map(scenes))
}

fn dot() -> Value {
    json!({ "kind": "sprite", "anchor": [0, 0], "keys": { "g": "grass-2" }, "layers": [{ "map": ["g"] }] })
}

/// A character whose second layer bobs and hops (the Range's character, in small).
fn character() -> Value {
    json!({
        "kind": "sprite", "anchor": [0, 0], "keys": { "g": "grass-2", "c": "cream" },
        "layers": [
            { "map": ["g"] },
            { "map": [".c"],
              "ambient": { "duration": 1000, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 500, "move": [0, -1] }] },
              "actions": { "hop": { "duration": 480, "keys": [{ "t": 0, "move": [0, -2] }] } } }
        ]
    })
}

fn scene(items: Value) -> Value {
    json!({ "output": "s.svg", "viewBox": [-8, -8, 16, 16], "items": items })
}

fn problems(s: &Sources) -> Vec<String> {
    validate(s).into_iter().map(|p| p.to_string()).collect()
}

#[test]
fn d6_validation_names_every_broken_rule_with_its_item_or_layer() {
    let bad = json!({ "object": "dot", "at": { "px": [0, 0] },
        "ambient": { "duration": 0, "offset": 5, "keys": [{ "t": 10, "move": [1] }, { "t": 5, "move": [0, 0], "show": false }], "speed": 1 },
        "actions": { "Hop": { "duration": 700000, "keys": [] } } });
    let s = sources(vec![("dot", dot())], vec![("s", scene(json!([bad])))]);
    let p = problems(&s);
    for want in [
        "scenes/s.json: items[0]: ambient: unknown property 'speed'",
        "scenes/s.json: items[0]: ambient.duration must be a whole number of ms from 1 to 600000",
        "scenes/s.json: items[0]: ambient.keys[0]: the first key must be at t: 0",
        "scenes/s.json: items[0]: ambient.keys[0]: move must be two whole numbers",
        "scenes/s.json: items[0]: ambient.keys[1]: unknown property 'show' (a key has only t and move)",
        "scenes/s.json: items[0]: ambient.keys[1]: t must come after the key before it",
        "scenes/s.json: items[0]: action 'Hop': names are lowercase words joined by hyphens",
        "scenes/s.json: items[0]: actions.Hop.duration must be a whole number of ms from 1 to 600000",
        "scenes/s.json: items[0]: actions.Hop.keys must be a list of at least one key",
    ] {
        assert!(p.iter().any(|x| x == want), "missing {want:?} in {p:#?}");
    }
}

#[test]
fn d6_an_offset_and_a_key_must_fall_inside_the_duration_and_an_element_has_at_most_16_actions() {
    let actions: serde_json::Map<String, Value> = (0..17)
        .map(|i| (format!("a{}", ["x"; 1][0].repeat(i + 1)), json!({ "duration": 10, "keys": [{ "t": 0, "move": [0, 0] }] })))
        .collect();
    let mut layer = json!({ "map": ["g"], "ambient": { "duration": 100, "offset": 100, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 100, "move": [1, 0] }] } });
    layer["actions"] = Value::Object(actions);
    let obj = json!({ "kind": "sprite", "anchor": [0, 0], "keys": { "g": "grass-2" }, "layers": [layer] });
    let p = problems(&sources(vec![("dot", obj)], vec![]));
    for want in [
        "objects/dot.json: layer 0: ambient.offset must be a whole number of ms from 0 to below the duration",
        "objects/dot.json: layer 0: ambient.keys[1]: t must be from 0 to below the duration",
        "objects/dot.json: layer 0: at most 16 actions",
    ] {
        assert!(p.iter().any(|x| x == want), "missing {want:?} in {p:#?}");
    }
}

#[test]
fn d6_an_item_with_scene_motion_cannot_also_have_an_ambient_loop() {
    let mut s = scene(json!([{ "object": "dot", "id": "cart", "at": { "px": [0, 0] },
        "ambient": { "duration": 1000, "keys": [{ "t": 0, "move": [0, 0] }] } }]));
    s["motion"] = json!({ "duration": 1000, "tracks": [{ "item": "cart", "keys": [{ "t": 0 }, { "t": 500, "move": [2, 0] }] }] });
    let p = problems(&sources(vec![("dot", dot())], vec![("s", s)]));
    assert!(
        p.contains(
            &"scenes/s.json: items[0]: has scene motion and an ambient loop; their moves would fight, so give it one of them".to_string()
        ),
        "{p:#?}"
    );
}

#[test]
fn d6_d7_one_action_name_is_one_move_in_a_scene() {
    let hop = |y: i64| json!({ "hop": { "duration": 480, "keys": [{ "t": 0, "move": [0, y] }] } });
    let items = json!([
        { "object": "dot", "at": { "px": [0, 0] }, "actions": hop(-2) },
        { "object": "dot", "at": { "px": [2, 0] }, "actions": hop(-2) },
        { "object": "dot", "at": { "px": [4, 0] }, "actions": hop(-3) }
    ]);
    let p = problems(&sources(vec![("dot", dot())], vec![("s", scene(items))]));
    assert_eq!(p, ["scenes/s.json: action 'hop' plays one move on items[0] and another on items[2]; one name is one move in a scene"]);
}

fn pixels_at(s: &Sources, time: f64, play: Option<(&str, f64)>) -> Vec<((i32, i32), String)> {
    let r = render_scene(s, "s", Order::Depth).unwrap();
    let opts = CompositeOptions { time: Some(time), play: play.map(|(a, at)| (a.to_string(), at)), ..CompositeOptions::default() };
    let mut out: Vec<((i32, i32), String)> = composite(&r.root, opts).iter().map(|(p, c)| (*p, c.to_string())).collect();
    out.sort();
    out
}

#[test]
fn d6_renders_at_a_time_place_ambient_loops_and_play_an_action_into_its_move() {
    let s = sources(vec![("character", character())], vec![("s", scene(json!([{ "object": "character", "at": { "px": [0, 0] } }])))]);
    let cream = |p: &[((i32, i32), String)]| p.iter().filter(|(_, c)| c == "cream").map(|(p, _)| *p).collect::<Vec<_>>();
    assert_eq!(cream(&pixels_at(&s, 0.0, None)), [(1, 0)], "at rest");
    assert_eq!(cream(&pixels_at(&s, 600.0, None)), [(1, -1)], "the bob's second half");
    assert_eq!(cream(&pixels_at(&s, 600.0, Some(("hop", 100.0)))), [(1, -2)], "the hop over the bob");
    assert_eq!(cream(&pixels_at(&s, 600.0, Some(("hop", 480.0)))), [(1, -1)], "the hop ended");
    // The still layer never moves.
    assert!(pixels_at(&s, 600.0, Some(("hop", 100.0))).contains(&((0, 0), "grass-2".to_string())));
    // Without a time, a render shows the rest frame.
    let r = render_scene(&s, "s", Order::Depth).unwrap();
    assert!(composite(&r.root, CompositeOptions::default()).iter().any(|(p, c)| *p == (1, 0) && &**c == "cream"));
}

#[test]
fn d6_an_items_ambient_loop_moves_the_whole_item_and_an_offset_starts_it_partway() {
    let item = json!({ "object": "dot", "at": { "px": [0, 0] },
        "ambient": { "duration": 1000, "offset": 500, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 500, "move": [3, 0] }] } });
    let s = sources(vec![("dot", dot())], vec![("s", scene(json!([item])))]);
    assert_eq!(pixels_at(&s, 0.0, None), [((3, 0), "grass-2".to_string())], "time 0 is 500 ms into the loop");
}

fn style(svg: &str) -> String {
    let start = svg.find("<style>").unwrap() + "<style>".len();
    svg[start..svg.find("</style>").unwrap()].to_string()
}

#[test]
fn d7_a_compiled_svg_plays_ambient_loops_and_actions_by_scoped_names_and_data_play() {
    let s = sources(vec![("character", character())], vec![("range", scene(json!([{ "object": "character", "at": { "px": [0, 0] } }])))]);
    let svg = compile_scene(&s, "range", &Budgets::new(), Order::Depth).unwrap().output;
    assert!(svg.contains("<g class=\"range--a1\">"), "{svg}");
    assert_eq!(
        style(&svg),
        concat!(
            "@keyframes range--am-1{0%{transform:translate(0,0);animation-timing-function:steps(1,end)}50%{transform:translate(0,-1px);animation-timing-function:steps(1,end)}100%{transform:translate(0,0)}}",
            "[data-pixellab=\"range\"] .range--a1{animation:range--am-1 1000ms step-end infinite}",
            "@keyframes range--hop{0%{transform:translate(0,-2px)}100%{transform:translate(0,-2px)}}",
            "@media (prefers-reduced-motion:no-preference){[data-pixellab=\"range\"] [data-play~=\"hop\"] .range--a1,[data-pixellab=\"range\"][data-play~=\"hop\"] .range--a1{animation:range--hop 480ms step-end 1}}",
            "@media (prefers-reduced-motion:reduce){[data-pixellab=\"range\"] [class*=\"range--\"]{animation:none}}",
        )
    );
}

#[test]
fn d7_an_offset_is_a_negative_delay_and_elements_that_play_the_same_share_a_class() {
    let item = |x: i64| {
        json!({ "object": "dot", "at": { "px": [x, 0] },
        "ambient": { "duration": 20000, "offset": 7000, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 10000, "move": [12, 0] }] } })
    };
    let s = sources(vec![("dot", dot())], vec![("sky", scene(json!([item(0), item(4)])))]);
    let svg = compile_scene(&s, "sky", &Budgets::new(), Order::Depth).unwrap().output;
    assert_eq!(svg.matches("<g class=\"sky--a1\">").count(), 2, "{svg}");
    assert!(style(&svg).contains("[data-pixellab=\"sky\"] .sky--a1{animation:sky--am-1 20000ms step-end -7000ms infinite}"));
    assert!(style(&svg).contains("steps(12,end)"));
}

#[test]
fn d4_d6_ambient_loops_and_actions_serialize_canonically_and_round_trip() {
    let s = scene(
        json!([{ "object": "dot", "at": { "px": [0, 0] }, "actions": { "hop": { "duration": 480, "keys": [{ "t": 0, "move": [0, -2] }] } },
        "ambient": { "keys": [{ "move": [0, 0], "t": 0 }], "offset": 0, "duration": 1000 } }]),
    );
    let text = serialize_file("scenes/s.json", &s);
    assert!(
        text.contains("{ \"object\": \"dot\", \"at\": { \"px\": [0, 0] }, \"ambient\": { \"duration\": 1000, \"offset\": 0, \"keys\": [{ \"t\": 0, \"move\": [0, 0] }] }, \"actions\": { \"hop\": { \"duration\": 480, \"keys\": [{ \"t\": 0, \"move\": [0, -2] }] } } }"),
        "{text}"
    );
    assert_eq!(serialize_file("scenes/s.json", &parse(&text).unwrap()), text);
    let text = serialize_file("objects/character.json", &character());
    assert_eq!(serialize_file("objects/character.json", &parse(&text).unwrap()), text);
    assert!(text.contains("\"ambient\": {\n        \"duration\": 1000,"), "{text}");
}
