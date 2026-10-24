//! The CSS and the renders of objects with states (docs/intents/2026-10-object-states/spec.md, R3,
//! R9 to R16, D2), and R3's promise: a sprite without states renders and compiles as it always did.

mod common;

use std::path::PathBuf;

use engine::compile::{compile_scene, Budgets};
use engine::engine::{load_sources, render_scene, validate, Order, Sources};
use engine::format::read_docs;
use engine::sha256;
use engine::svg::to_rect_svg;

fn data(name: &str) -> PathBuf {
    common::repo().join("crates/engine/tests/data/states-r3").join(name)
}

fn sources_of(dir: &std::path::Path) -> Sources {
    let docs = read_docs(dir);
    assert!(docs.load_errors.is_empty(), "{:?}", docs.load_errors);
    load_sources(docs.palette.expect("a palette"), docs.objects, docs.scenes)
}

/// Each scene of the three documents, with a digest of what it renders to (the unscoped rect SVG
/// the lab used to write) and compiles to (the scoped, optimized SVG a site inlines).
fn digests() -> Vec<String> {
    let mut lines = Vec::new();
    // frames.pixellab: frame loops (the site's and one of its own), a class layer, ambient loops, actions and a pose.
    for doc in ["sample.pixellab", "frames.pixellab", "harbor.pixellab", "town.pixellab"] {
        let sources = sources_of(&data(doc));
        let hex_of = |c: &str| sources.colors.get(c).map_or_else(|| "undefined".to_string(), |c| c.hex.clone());
        for name in sources.scenes.keys() {
            let rendered = render_scene(&sources, name, Order::Depth).expect("renders");
            let rect = to_rect_svg(&rendered, &hex_of);
            let compiled = compile_scene(&sources, name, &Budgets::new(), Order::Depth).expect("compiles");
            lines.push(format!(
                "{doc}/{name} render {} {} compile {} {}",
                rect.len(),
                sha256::hex(rect.as_bytes()),
                compiled.output.len(),
                sha256::hex(compiled.output.as_bytes())
            ));
        }
    }
    lines
}

#[test]
fn r3_a_sprite_without_states_renders_and_compiles_byte_identical() {
    let golden = data("goldens.txt");
    let now = digests().join("\n") + "\n";
    // PIXELLAB_WRITE_GOLDENS=1 rewrites the file: only for a change that is meant to change the output.
    if std::env::var_os("PIXELLAB_WRITE_GOLDENS").is_some() {
        std::fs::write(&golden, &now).expect("write the goldens");
    }
    let expected = std::fs::read_to_string(&golden).expect("the goldens");
    assert_eq!(now, expected, "R3: a document without states must render and compile as it did before states");
}

// ------------------------------------------------------------- the fixture's scene

use engine::engine::{
    composite, render_object, render_object_in, render_scene_with, resolve_frame, scene_at, Child, CompositeOptions, Group,
};
use serde_json::json;

fn fixture() -> Sources {
    let docs = read_docs(&common::repo().join("fixtures/states.pixellab"));
    assert!(docs.load_errors.is_empty(), "{:?}", docs.load_errors);
    load_sources(docs.palette.expect("a palette"), docs.objects, docs.scenes)
}

/// The compiled scene's style and tree: the CSS a site inlines, and the groups it plays it on.
fn compiled(sources: &Sources, scene: &str) -> (String, Group) {
    let r = render_scene_with(sources, scene, Order::Depth, Some(scene)).expect("renders");
    (r.style.expect("a style"), r.root)
}

fn classes(group: &Group, out: &mut Vec<String>) {
    for child in &group.children {
        if let Child::Group(g) = child {
            if let Some((_, class)) = g.attrs.iter().find(|(k, _)| k == "class") {
                out.push(class.clone());
            }
            classes(g, out);
        }
    }
}

#[test]
fn d2_a_sprite_with_states_draws_one_element_holding_a_group_for_each_frame() {
    let sources = fixture();
    let root = render_object(&sources, "walker").unwrap();
    let Child::Group(element) = &root.children[0] else { panic!("an element") };
    assert!(element.states.is_some() && element.generated);
    assert_eq!(element.attrs[0], ("class".to_string(), "s1".to_string()));
    let frames: Vec<(Option<usize>, &str)> = element
        .children
        .iter()
        .map(|c| match c {
            Child::Group(g) => (g.state_frame, g.attrs[0].1.as_str()),
            Child::Layer(_) => panic!("only frame groups"),
        })
        .collect();
    assert_eq!(frames, [(Some(0), "s1-f0"), (Some(1), "s1-f1"), (Some(2), "s1-f2")]);
    // Each frame group holds the sprite's map layers (the cap, in its own class group) and its column of the frame loop.
    for c in &element.children {
        let Child::Group(g) = c else { unreachable!() };
        assert!(g.children.iter().any(|c| matches!(c, Child::Group(cap) if cap.attrs[0].1 == "cap")), "the cap is in every frame");
        assert!(g.children.iter().any(|c| matches!(c, Child::Layer(_))), "and the frame's own pixels");
    }
}

#[test]
fn d2_a_sprite_without_states_draws_as_it_always_did() {
    let sources = fixture();
    let root = render_object(&sources, "pebble").unwrap();
    let mut seen = Vec::new();
    classes(&root, &mut seen);
    assert_eq!(seen, ["ff f0", "ff f1", "ff f2", "ff f3"]);
}

#[test]
fn d2_composite_shows_the_frame_a_sprite_has_at_a_time_and_moves_it() {
    let sources = fixture();
    let at = |state: Option<&str>, time: Option<f64>, frame: usize| {
        let root = render_object_in(&sources, "walker", state).unwrap();
        composite(&root, CompositeOptions { frame, time, ..CompositeOptions::default() })
    };
    // With no time, the frame asked for shows, whatever the state.
    let frame1 = at(None, None, 1);
    let frame2 = at(None, None, 2);
    assert_ne!(frame1.keys().collect::<Vec<_>>(), frame2.keys().collect::<Vec<_>>());
    // The hop starts on frame 1, and idle's first frame is frame 0.
    let same = |a: &engine::omap::OMap<engine::iso::Pixel, engine::engine::ColorName>,
                b: &engine::omap::OMap<engine::iso::Pixel, engine::engine::ColorName>| {
        a.iter().map(|(p, c)| (*p, c.to_string())).collect::<Vec<_>>() == b.iter().map(|(p, c)| (*p, c.to_string())).collect::<Vec<_>>()
    };
    assert!(same(&at(Some("hop"), Some(0.0), 0), &frame1));
    assert!(same(&at(Some("hop"), Some(480.0), 0), &at(None, None, 0)), "then idle, frame 0");
    // Two pixels up once the hop has stepped twice.
    let up: Vec<(i32, i32)> = at(Some("hop"), Some(100.0), 0).keys().map(|p| (p.0, p.1 + 2)).collect();
    let down: Vec<(i32, i32)> = frame1.keys().copied().collect();
    assert_eq!(up, down);
}

#[test]
fn d2_items_that_play_the_same_share_a_class_and_the_others_have_their_own() {
    // The fixture places the walker twice in idle (one in a group), once in walk, and two lamps.
    let sources = fixture();
    let (css, root) = compiled(&sources, "states");
    let mut seen = Vec::new();
    classes(&root, &mut seen);
    let walker_elements = seen.iter().filter(|c| c.starts_with("s") && !c.contains("-f") && !c.contains(' ')).count();
    assert_eq!(walker_elements, 5, "five items with states: {seen:?}");
    assert!(seen.iter().filter(|c| *c == "s1").count() == 2, "the two walkers that start in idle share s1: {seen:?}");
    assert!(
        css.contains("states--s1-f0") && css.contains("states--s2-f0") && css.contains("states--s3-f0") && css.contains("states--s4-f0")
    );
    assert!(!css.contains("states--s5"), "no fifth kind of play");
}

#[test]
fn d2_the_box_a_scene_fits_to_holds_how_far_a_state_moves_its_sprite() {
    let mut sources = fixture();
    let scene = sources.scenes.get_mut("states").unwrap();
    scene.as_object_mut().unwrap().shift_remove("viewBox");
    let before = resolve_frame(&sources, "states").unwrap().view_box;
    // Give the hop a long jump: the scene's box has to grow to hold it.
    sources.objects.get_mut("walker").unwrap()["states"]["hop"]["move"] =
        json!([{ "t": 0, "move": [0, 0] }, { "t": 44, "move": [0, -40] }]);
    sources.extents.borrow_mut().clear();
    let after = resolve_frame(&sources, "states").unwrap().view_box;
    assert!(after[1] <= before[1] - 38, "{before:?} → {after:?}");
}

#[test]
fn d2_a_scene_at_a_time_marks_the_state_a_keyed_item_is_in_and_when_it_entered() {
    let sources = fixture();
    let at = |t: f64| {
        let v = scene_at(&sources, "states", t).unwrap();
        let lamp = v["items"].as_array().unwrap().iter().find(|i| i["id"] == "lamp-b").unwrap().clone();
        (lamp.get("state").cloned(), lamp.get("since").cloned())
    };
    assert_eq!(at(500.0), (Some(json!("idle")), Some(json!(0))));
    assert_eq!(at(1100.0), (Some(json!("flicker")), Some(json!(1000))));
    assert_eq!(at(3000.0 + 1100.0), (Some(json!("flicker")), Some(json!(4000))), "the loop before it counts");
    assert_eq!(at(2500.0), (Some(json!("idle")), Some(json!(2000))));
}

// ------------------------------------------------- the CSS, played, against the renders

/// A little CSS player for the rules a compiled scene writes for its states (no browser): the
/// keyframes, `animation` lists (a name, a length, a delay, `infinite` or a count, `forwards`) and the
/// base rules, for elements with no attribute on them. It plays them as CSS Animations does: an
/// animation in its delay does nothing, one that has ended holds its last stop only with `forwards`,
/// and the one later in the list wins.
struct Player {
    keyframes: std::collections::HashMap<String, Vec<(f64, String)>>,
    /// (selectors, declarations), in the order written; the no-preference block's rules included.
    rules: Vec<(Vec<String>, Vec<(String, String)>)>,
}

impl Player {
    fn new(css: &str) -> Player {
        let mut keyframes = std::collections::HashMap::new();
        let mut rules = Vec::new();
        let mut rest = css;
        // The reduced-motion block is for the reduced-motion test; here motion is welcome.
        let css_owned: String;
        if let Some(at) = css.find("@media (prefers-reduced-motion:reduce)") {
            css_owned = css[..at].to_string();
            rest = &css_owned;
        }
        // Unwrap the no-preference block: its rules count as written there.
        let open = "@media (prefers-reduced-motion:no-preference){";
        let mut unwrapped = rest.to_string();
        if let Some(at) = unwrapped.find(open) {
            let mut depth = 1;
            let mut end = at + open.len();
            for (i, c) in unwrapped[at + open.len()..].char_indices() {
                depth += match c {
                    '{' => 1,
                    '}' => -1,
                    _ => 0,
                };
                if depth == 0 {
                    end = at + open.len() + i;
                    break;
                }
            }
            unwrapped = format!("{}{}{}", &unwrapped[..at], &unwrapped[at + open.len()..end], &unwrapped[end + 1..]);
        }
        let mut s = unwrapped.as_str();
        while !s.is_empty() {
            if let Some(body) = s.strip_prefix("@keyframes ") {
                let open = body.find('{').unwrap();
                let name = body[..open].to_string();
                // Stops: p%{prop:value} ... up to the closing brace of the whole.
                let mut stops = Vec::new();
                let mut t = &body[open + 1..];
                while !t.starts_with('}') && !t.is_empty() {
                    let brace = t.find('{').unwrap();
                    let close = t.find('}').unwrap();
                    let at = t[..brace].trim_end_matches('%');
                    let at = match at {
                        "from" => 0.0,
                        "to" => 100.0,
                        n => n.parse::<f64>().unwrap(),
                    };
                    let decl = &t[brace + 1..close];
                    stops.push((at, decl.to_string()));
                    t = &t[close + 1..];
                }
                keyframes.insert(name, stops);
                s = &t[1..];
            } else {
                let brace = s.find('{').unwrap();
                let close = s.find('}').unwrap();
                let selectors = s[..brace].split(',').map(str::to_string).collect();
                let decls = s[brace + 1..close]
                    .split(';')
                    .filter(|d| !d.is_empty())
                    .map(|d| {
                        let (k, v) = d.split_once(':').unwrap();
                        (k.to_string(), v.to_string())
                    })
                    .collect();
                rules.push((selectors, decls));
                s = &s[close + 1..];
            }
        }
        Player { keyframes, rules }
    }

    /// A property of the element with this class, `t` ms in, with no attribute on any group above it.
    fn value(&self, scope: &str, class: &str, property: &str, t: f64) -> Option<String> {
        let selector = format!("[data-pixellab=\"{scope}\"] .{class}");
        let mut value: Option<String> = None;
        for (selectors, decls) in &self.rules {
            if !selectors.contains(&selector) {
                continue;
            }
            for (k, v) in decls {
                if k == property {
                    value = Some(v.clone());
                }
                if k == "animation" {
                    for part in split_top(v) {
                        if let Some(played) = self.play(&part, property, t) {
                            value = Some(played);
                        }
                    }
                }
            }
        }
        value
    }

    fn play(&self, part: &str, property: &str, t: f64) -> Option<String> {
        let words: Vec<&str> = part.split_whitespace().collect();
        let stops = self.keyframes.get(words[0])?;
        if !stops.first()?.1.starts_with(property) {
            return None;
        }
        let ms = |w: &str| w.strip_suffix("ms").map(|n| n.parse::<f64>().unwrap());
        let times: Vec<f64> = words.iter().filter_map(|w| ms(w)).collect();
        let (dur, delay) = (times[0], times.get(1).copied().unwrap_or(0.0));
        let elapsed = t - delay;
        if elapsed < 0.0 {
            return None;
        }
        let infinite = words.contains(&"infinite");
        let local = if infinite {
            elapsed % dur
        } else if elapsed >= dur {
            if words.contains(&"forwards") {
                return Some(stops.last()?.1.split_once(':').unwrap().1.to_string());
            }
            return None;
        } else {
            elapsed
        };
        let p = local / dur * 100.0;
        let stop = stops.iter().rev().find(|(at, _)| *at <= p + 1e-9).or(stops.first())?;
        Some(stop.1.split_once(':').unwrap().1.to_string())
    }
}

/// Splits an `animation` list at its top-level commas.
fn split_top(v: &str) -> Vec<String> {
    v.split(',').map(str::to_string).collect()
}

use engine::iso::Pixel;

/// What the compiled scene shows at `t`, by playing its CSS on its tree: pixel → color.
fn played(root: &Group, player: &Player, scope: &str, t: f64) -> std::collections::BTreeMap<Pixel, String> {
    fn walk(g: &Group, player: &Player, scope: &str, t: f64, shift: (i32, i32), out: &mut std::collections::BTreeMap<Pixel, String>) {
        for child in &g.children {
            match child {
                Child::Layer(l) => {
                    for ((x, y), c) in l.pixels.iter() {
                        out.insert((x + shift.0, y + shift.1), c.to_string());
                    }
                }
                Child::Group(g) => {
                    // (A group that is hidden at rest says so in the CSS too: the player reads that.)
                    // A frame loop of a sprite without states plays as it always has (R3).
                    if let (Some(frame), Some(count)) = (g.frame, g.frames) {
                        let name = g.attrs[0].1.split(' ').next().unwrap();
                        if engine::motion::loop_frame(name, count, t) != frame {
                            continue;
                        }
                    }
                    let mut shift = shift;
                    if let Some((_, class)) = g.attrs.iter().find(|(k, _)| k == "class").filter(|_| g.generated) {
                        let full = format!("{scope}--{class}");
                        // A group that fades out (a frame of a state, or a pose the track isn't showing) is not painted.
                        if player.value(scope, &full, "opacity", t).as_deref() == Some("0") {
                            continue;
                        }
                        if class.starts_with('s')
                            && !class.contains("-f")
                            && class.starts_with('s')
                            && class[1..].chars().all(|c| c.is_ascii_digit())
                        {
                            if let Some(tf) = player.value(scope, &full, "transform", t) {
                                // translate(0,-1px)
                                let inner = tf.trim_start_matches("translate(").trim_end_matches(')');
                                let n = |v: &str| v.trim_end_matches("px").parse::<i32>().unwrap();
                                let (x, y) = inner.split_once(',').unwrap();
                                shift = (shift.0 + n(x), shift.1 + n(y));
                            }
                        }
                    }
                    walk(g, player, scope, t, shift, out);
                }
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(root, player, scope, t, (0, 0), &mut out);
    out
}

/// What `scene render --time` shows at `t`: the scene at that time, placed and composited, as the app does.
fn rendered(sources: &Sources, scene: &str, t: f64) -> std::collections::BTreeMap<Pixel, String> {
    let mut still = scene_at(sources, scene, t).unwrap();
    let items = still.as_object_mut().unwrap().shift_remove("items").unwrap();
    let mut local = sources.clone();
    let mut view = still.clone();
    view["items"] = items;
    local.scenes.clear();
    local.scenes.insert(scene.to_string(), view);
    let r = engine::engine::render_scene(&local, scene, Order::List).unwrap();
    let pixels = composite(&r.root, CompositeOptions { time: Some(t), ..CompositeOptions::default() });
    pixels.iter().map(|(p, c)| (*p, c.to_string())).collect()
}

/// The pixels two renderings differ in, for a failure to say.
fn same_pixels(a: &std::collections::BTreeMap<Pixel, String>, b: &std::collections::BTreeMap<Pixel, String>, what: &str) {
    let only_css: Vec<_> = a.iter().filter(|(p, c)| b.get(*p) != Some(*c)).take(8).collect();
    let only_render: Vec<_> = b.iter().filter(|(p, c)| a.get(*p) != Some(*c)).take(8).collect();
    assert!(only_css.is_empty() && only_render.is_empty(), "{what}: the CSS shows {only_css:?}, the render {only_render:?}");
}

#[test]
fn r16_the_compiled_css_plays_the_scene_as_scene_render_shows_it_at_every_100_ms() {
    let sources = fixture();
    let (css, root) = compiled(&sources, "states");
    let player = Player::new(&css);
    for t in (0..3000).step_by(100).chain([1, 1001, 1081, 1161, 2001]) {
        let t = t as f64;
        same_pixels(&played(&root, &player, "states", t), &rendered(&sources, "states", t), &format!("at {t} ms"));
    }
}

#[test]
fn r16_the_baked_track_moves_the_item_too_and_its_states_keep_playing_from_where_they_were_entered() {
    // Give the keyed lamp's flicker a move, and add a second pose with its own states: the baked CSS has to move it as the render does.
    let mut sources = fixture();
    sources.objects.get_mut("lamp").unwrap()["states"]["flicker"]["move"] =
        json!([{ "t": 0, "move": [0, 0] }, { "t": 80, "move": [2, -1] }, { "t": 160, "move": [0, -3] }]);
    sources.extents.borrow_mut().clear();
    let (css, root) = compiled(&sources, "states");
    assert!(css.contains("@keyframes states--s4-m{"), "the track's moves are baked");
    let player = Player::new(&css);
    for t in (0..3000).step_by(50).chain([1001, 1041, 1081, 1121, 1161, 1241, 1301]) {
        let t = t as f64;
        same_pixels(&played(&root, &player, "states", t), &rendered(&sources, "states", t), &format!("at {t} ms"));
    }
}

#[test]
fn r16_a_pose_with_its_own_states_follows_the_track_across_a_hand_over_between_objects() {
    // lamp-b is posed as the walker from 500 ms on, with no state key: the walker keeps its start (idle), entered then; a state key later
    // puts it in hop; posing back as the lamp enters the lamp's start.
    let mut sources = fixture();
    // Away from the others, so the order things draw in doesn't differ between a render at a time and the compiled SVG, which is not what this is about.
    sources.scenes.get_mut("states").unwrap()["items"][4]["at"] = json!({ "tile": [6, 6, 0] });
    let track = &mut sources.scenes.get_mut("states").unwrap()["motion"]["tracks"][0]["keys"];
    *track = json!([
        { "t": 0 },
        { "t": 500, "pose": "walker" },
        { "t": 1000, "state": "hop" },
        { "t": 1700, "pose": "lamp", "state": "flicker" },
        { "t": 2200 }
    ]);
    assert_eq!(validate(&sources), Vec::<String>::new());
    let (css, root) = compiled(&sources, "states");
    let player = Player::new(&css);
    // A hair after each whole 50 ms: a pose key's own stop is rounded to 3 decimals of a percent, as the poses always were.
    for t in (0..3000).step_by(50).chain([501, 1001, 1045, 1481, 1701, 1781, 2201]) {
        let t = t as f64 + 0.5;
        same_pixels(&played(&root, &player, "states", t), &rendered(&sources, "states", t), &format!("at {t} ms"));
    }
}
