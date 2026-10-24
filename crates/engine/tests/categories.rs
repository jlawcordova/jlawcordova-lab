//! Categories (docs/intents/2026-10-object-libraries/spec.md, R1–R3): an
//! object or a block can carry a nested `category`, checked by `validate`, and
//! never read by a render or a compile.

mod depthlib;

use depthlib::*;
use engine::compile::{compile_scene, Budgets};
use engine::engine::{composite, load_sources, render_object, render_scene, validate, CompositeOptions, Order, Sources};
use engine::format::serialize;
use serde_json::{json, Value};

fn sprite_with(extra: Value) -> Value {
    with(json!({ "kind": "sprite", "keys": { "g": "grass-2" }, "layers": [{ "map": ["g"] }] }), extra)
}

fn block_with(extra: Value) -> Value {
    with(json!({ "kind": "block", "size": [1, 1, 1], "faces": { "top": "grass-2", "left": "grass-4", "right": "wood-3" } }), extra)
}

fn problems_of(name: &str, object: Value) -> Vec<String> {
    validate(&load_sources(palette(), [(name.to_string(), object)].into_iter().collect(), Default::default()))
}

#[test]
fn r1_a_sprite_and_a_block_take_a_category_of_any_depth_and_it_is_written_after_notes() {
    for category in [json!(["Nature"]), json!(["Nature", "Trees"]), json!(["A", "B", "C", "D", "E"])] {
        assert_eq!(problems_of("pine", sprite_with(json!({ "category": category }))), Vec::<String>::new());
        assert_eq!(problems_of("slab", block_with(json!({ "category": category }))), Vec::<String>::new());
    }
    let text = serialize(&sprite_with(json!({ "category": ["Nature", "Trees"], "notes": ["A pine."] })));
    let lines: Vec<&str> = text.lines().collect();
    let at = |needle: &str| lines.iter().position(|l| l.starts_with(needle)).unwrap();
    assert!(at("  \"notes\"") < at("  \"category\"") && at("  \"category\"") < at("  \"kind\""), "{text}");
    let text = serialize(&block_with(json!({ "category": ["Terrain"], "notes": ["A slab."] })));
    let lines: Vec<&str> = text.lines().collect();
    let at = |needle: &str| lines.iter().position(|l| l.starts_with(needle)).unwrap();
    assert!(at("  \"notes\"") < at("  \"category\"") && at("  \"category\"") < at("  \"kind\""), "{text}");
}

#[test]
fn r2_a_category_is_a_list_of_readable_names_and_all_and_others_cannot_be_outermost() {
    let bad = |category: Value, expect: &str| {
        for object in [sprite_with(json!({ "category": category })), block_with(json!({ "category": category }))] {
            let problems = problems_of("pine", object);
            assert!(problems.iter().any(|p| p.starts_with("objects/pine.json: ") && p.contains(expect)), "{category}: {problems:?}");
        }
    };
    bad(json!("Nature/Trees"), "category must be a list of names");
    bad(json!(null), "category must be a list of names");
    bad(json!([]), "at least one name");
    bad(json!([""]), "0 characters");
    bad(json!(["x".repeat(33)]), "33 characters");
    bad(json!([7]), "is not a name");
    bad(json!(["Nature", "Trees!"]), "use letters, digits, spaces");
    bad(json!(["Nature/Trees"]), "use letters, digits, spaces");
    bad(json!([" Nature"]), "leading, trailing or doubled");
    bad(json!(["Nature "]), "leading, trailing or doubled");
    bad(json!(["Big  Trees"]), "leading, trailing or doubled");
    bad(json!(["All"]), "can't be the outermost");
    bad(json!(["others"]), "can't be the outermost");
    bad(json!(["ALL", "Trees"]), "can't be the outermost");
    // The allowed characters, 32 long, and All or Others deeper down.
    for ok in
        [json!(["Fish & Chips"]), json!(["Cook's-Pot 2"]), json!(["x".repeat(32)]), json!(["Nature", "All"]), json!(["Nature", "Others"])]
    {
        assert_eq!(problems_of("pine", sprite_with(json!({ "category": ok }))), Vec::<String>::new(), "{ok}");
    }
}

/// Every object of `sources` with `category` added (or removed), as new sources.
fn categorized(sources: &Sources, category: Option<Value>) -> Sources {
    let objects = sources
        .objects
        .iter()
        .map(|(name, object)| {
            let mut object = object.clone();
            match &category {
                Some(c) => {
                    object["category"] = c.clone();
                }
                None => {
                    object.as_object_mut().unwrap().remove("category");
                }
            }
            (name.clone(), object)
        })
        .collect();
    load_sources(sources.palette.clone(), objects, sources.scenes.clone())
}

#[test]
fn r3_categories_never_change_a_render_or_a_compiled_svg() {
    let dir = Temp::new();
    let mut scenes = 0;
    for name in ["harbor", "town"] {
        let (plain, load_errors) = read_sources(&example(&dir.0, name));
        assert_eq!(load_errors, Vec::<String>::new());
        // The examples may be categorized already: compare against none, and against two spellings.
        let none = categorized(&plain, None);
        for with_category in [categorized(&plain, Some(json!(["Nature", "Trees"]))), categorized(&plain, Some(json!(["Props"])))] {
            assert_eq!(validate(&with_category), Vec::<String>::new());
            for object in none.objects.keys() {
                let a = composite(&render_object(&none, object).unwrap(), CompositeOptions::default());
                let b = composite(&render_object(&with_category, object).unwrap(), CompositeOptions::default());
                assert_eq!(
                    format!("{:?}", a.iter().collect::<Vec<_>>()),
                    format!("{:?}", b.iter().collect::<Vec<_>>()),
                    "{name}/{object} renders the same"
                );
            }
            for scene in none.scenes.keys() {
                let a = render_scene(&none, scene, Order::Depth).unwrap();
                let b = render_scene(&with_category, scene, Order::Depth).unwrap();
                let (pa, pb) = (composite(&a.root, CompositeOptions::default()), composite(&b.root, CompositeOptions::default()));
                assert_eq!(
                    format!("{:?}", pa.iter().collect::<Vec<_>>()),
                    format!("{:?}", pb.iter().collect::<Vec<_>>()),
                    "{name}/{scene} renders the same"
                );
                let a = compile_scene(&none, scene, &Budgets::new(), Order::Depth).unwrap();
                let b = compile_scene(&with_category, scene, &Budgets::new(), Order::Depth).unwrap();
                assert_eq!(a.output, b.output, "{name}/{scene} compiles to the same bytes");
                scenes += 1;
            }
        }
    }
    assert_eq!(scenes, 6, "both examples, two categorizations each");
}
