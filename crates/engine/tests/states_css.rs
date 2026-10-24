//! The CSS and the renders of objects with states (docs/intents/2026-10-object-states/spec.md, R3,
//! R9 to R16, D2), and R3's promise: a sprite without states renders and compiles as it always did.

mod common;

use std::path::PathBuf;

use engine::compile::{compile_scene, Budgets};
use engine::engine::{load_sources, render_scene, Order, Sources};
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
    let mut sources = fixture();
    // A third walker that plays what the first does.
    let scene = sources.scenes.get_mut("states").unwrap();
    scene["items"].as_array_mut().unwrap().push(json!({ "object": "walker", "at": { "tile": [3, 0, 0] } }));
    let (css, root) = compiled(&sources, "states");
    let mut seen = Vec::new();
    classes(&root, &mut seen);
    let walker_elements = seen.iter().filter(|c| c.starts_with("s") && !c.contains("-f") && !c.contains(' ')).count();
    assert_eq!(walker_elements, 5, "four items with states and the walker twice: {seen:?}");
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
