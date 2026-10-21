//! The engine parts of cli/test/depth-motion.test.mjs (docs/intents/2026-10-depth-order/spec.md,
//! R10, D6): the order at a time with the still items' chain, the places a moving item takes and
//! the windows it spends in them, the copies the SVG draws, the SVG's text and CSS, and the
//! published tree with its CSS evaluated against the preview's draw. Scenes are built in memory.
//!
//! The program parts (`scene.render` at a time, the PNG compared with the SVG's tree, `scene.render`
//! with variants and layers) are in crates/pixellab/tests/depth_motion.rs. Not ported here, since
//! they go through `scene.render` and its PNG: "at every window boundary and the middle of every
//! window…", "reduced motion (animations off) shows the frame of time 0", "sceneAt lists the
//! items in the order drawn at that time…" (that one is in the program suite) and "through
//! scene.render, with variants and layers".
//!
//! Timing: the hero-island-sized check was already on a synthetic scene (220+ items, 39 moving
//! ones with 25 keys), so it is ported as it is.

mod depthlib;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use depthlib::css::{paint_css, parse_css, style_of};
use depthlib::*;
use engine::depth::{
    behind, block_box, key_of, moving_order, overlaps, place_windows, places_in, sort_nodes, Bounds, Box3, Copy, Mark, Node, Place,
};
use engine::engine::{composite, motion_copies, render_scene, scene_at, validate, CompositeOptions, Order, Sources};
use engine::motion::{is_moving, motion_css, move_at_percent, move_runs, pct_of, pose_items, shown_items};
use engine::omap::OMap;
use engine::svg::to_rect_svg;
use serde_json::{json, Value};

const EVERYWHERE: Bounds = Bounds { min_x: -1000, min_y: -1000, max_x: 1000, max_y: 1000 };

/// A 1×1×1 block (the house) and a small sprite (the walker) standing at its bottom middle.
fn objects() -> Vec<(&'static str, Value)> {
    vec![("house", block()), ("walker", sprite_x(&[".gg.", "gggg", "gggg", ".ii."], json!({ "anchor": [2, 4] })))]
}

fn tall() -> Value {
    sprite_x(&[".ii.", "gggg", "gggg", "gggg", "gggg", "gggg", ".ii."], json!({ "anchor": [2, 7] }))
}

fn walker_b() -> Value {
    sprite_x(&[".gg.", "gggg", "gggg", "ii.."], json!({ "anchor": [2, 4] }))
}

/// A scene whose motion is `tracks`, a loop of `duration` ms.
fn scn(items: Vec<Value>, tracks: Vec<Value>, duration: u32) -> Value {
    let mut s = json!({ "viewBox": [-80, -20, 160, 140], "items": items });
    if !tracks.is_empty() {
        s["motion"] = json!({ "duration": duration, "tracks": tracks });
    }
    s
}

fn scene10(items: Vec<Value>, tracks: Vec<Value>) -> Value {
    scn(items, tracks, 10000)
}

fn track(item: &str, keys: Vec<Value>) -> Value {
    json!({ "item": item, "keys": keys })
}

fn one(scene: Value) -> Sources {
    src(&objects(), &[("s", scene)])
}

fn hex_fn(sources: &Sources) -> impl Fn(&str) -> String + '_ {
    |c: &str| sources.colors.get(c).unwrap().hex.clone()
}

/// The pixels where two painted maps differ.
fn differences(a: &HashMap<(i32, i32), String>, b: &HashMap<(i32, i32), String>) -> Vec<(i32, i32)> {
    let keys: HashSet<_> = a.keys().chain(b.keys()).copied().collect();
    let mut out: Vec<_> = keys.into_iter().filter(|p| a.get(p) != b.get(p)).collect();
    out.sort();
    out
}

fn map_of(pixels: &OMap<(i32, i32), engine::engine::ColorName>) -> HashMap<(i32, i32), String> {
    pixels.iter().map(|(p, c)| (*p, c.to_string())).collect()
}

fn bounds(min_x: i32, min_y: i32, max_x: i32, max_y: i32) -> Option<Bounds> {
    Some(Bounds { min_x, min_y, max_x, max_y })
}

fn unit(c: f64, r: f64, l: f64) -> Box3 {
    block_box(c, r, l, [1.0, 1.0, 1.0])
}

fn nd(b: Box3) -> Node {
    Node { boxed: Some(b), bounds: Some(EVERYWHERE), depth: None, moving: false }
}

fn mover(b: Box3) -> Node {
    Node { moving: true, ..nd(b) }
}

// ------------------------------------------------------------ naive order

/// The order at an instant, written plainly and checked against moving_order: every "behind" pair
/// among the nodes as they are now, the still chain, the first listed node that is free, the
/// smallest key among those the chain doesn't hold back when none is.
fn naive_order(nodes: &[Node], current: &HashMap<usize, Rc<Node>>) -> Vec<usize> {
    let at = |i: usize| -> Node { current.get(&i).map_or_else(|| nodes[i].clone(), |n| (**n).clone()) };
    let mut out = Vec::new();
    for band in [Some(Mark::Back), None, Some(Mark::Front)] {
        let members: Vec<usize> = (0..nodes.len()).filter(|&i| nodes[i].depth == band).collect();
        let before: HashMap<usize, Vec<usize>> = members
            .iter()
            .map(|&i| {
                (
                    i,
                    members
                        .iter()
                        .copied()
                        .filter(|&j| j != i && overlaps(at(j).bounds, at(i).bounds) && behind(at(j).boxed, at(i).boxed))
                        .collect(),
                )
            })
            .collect();
        let stills: Vec<usize> = members.iter().copied().filter(|&i| !nodes[i].moving).collect();
        let chain: HashMap<usize, usize> = stills.iter().skip(1).enumerate().map(|(k, &i)| (i, stills[k])).collect();
        let mut placed: HashSet<usize> = HashSet::new();
        while placed.len() < members.len() {
            let rest: Vec<usize> = members.iter().copied().filter(|i| !placed.contains(i)).collect();
            let held = |i: usize| chain.contains_key(&i) && !placed.contains(&chain[&i]);
            let free: Vec<usize> = rest.iter().copied().filter(|&i| !held(i) && before[&i].iter().all(|j| placed.contains(j))).collect();
            let next = if let Some(&f) = free.first() {
                f
            } else {
                rest.iter()
                    .copied()
                    .filter(|&i| !held(i))
                    .reduce(|a, b| if key_of(at(b).boxed) < key_of(at(a).boxed) { b } else { a })
                    .unwrap()
            };
            placed.insert(next);
            out.push(next);
        }
    }
    out
}

struct RandomList {
    listed: Vec<Node>,
    boxes: Vec<Box3>,
}

/// Random nodes in rest order, some of them moving, on a grid small enough for ties, cycles and interpenetration.
fn random_list(r: &mut Rng, n: usize) -> RandomList {
    let boxes = random_boxes(r, n, 6.0);
    let pixel_bounds = |r: &mut Rng| {
        if r.next() < 0.75 {
            Some(EVERYWHERE)
        } else {
            let (min_x, max_x) = (r.floor(30.0) as i32, 30 + r.floor(30.0) as i32);
            bounds(min_x, 0, max_x, 5)
        }
    };
    let nodes: Vec<Node> = boxes
        .into_iter()
        .map(|b| {
            let bo = pixel_bounds(r);
            let depth = if r.next() < 0.12 { Some(if r.next() < 0.5 { Mark::Front } else { Mark::Back }) } else { None };
            Node { boxed: Some(b), bounds: bo, depth, moving: false }
        })
        .collect();
    // The list is in the order the rest sort gives, which is what moving_order is given.
    let listed: Vec<Node> = sort_nodes(&nodes).into_iter().map(|i| Node { moving: r.next() < 0.3, ..nodes[i].clone() }).collect();
    let boxes = (0..n)
        .map(|_| {
            let (c, row, l) = (r.floor(6.0) as f64, r.floor(6.0) as f64, (r.floor(3.0) - 1) as f64);
            block_box(c, row, l, [1.0, 1.0, r.floor(3.0) as f64])
        })
        .collect();
    RandomList { listed, boxes }
}

/// A moving node somewhere new, with its own pixels (sometimes missing everything).
fn somewhere(r: &mut Rng, boxes: &[Box3], n: usize) -> Rc<Node> {
    let b = boxes[r.floor(n as f64) as usize];
    let bo = if r.next() < 0.8 { Some(EVERYWHERE) } else { bounds(500, 0, 510, 5) };
    Rc::new(Node { boxed: Some(b), bounds: bo, depth: None, moving: true })
}

fn current_map(current: &[(usize, Rc<Node>)]) -> HashMap<usize, Rc<Node>> {
    current.iter().cloned().collect()
}

// ------------------------------- the order while moving: chain, places, windows

#[test]
fn r10_a_moving_node_sits_behind_between_or_in_front_of_the_still_ones_as_its_box_says_and_the_still_ones_never_change_order() {
    // a stands behind b; m (moving) is placed at three instants.
    let nodes = [nd(unit(0.0, 0.0, 0.0)), nd(unit(4.0, 0.0, 0.0)), mover(unit(2.0, 0.0, 0.0))];
    let mut order_at = moving_order(&nodes);
    assert_eq!(order_at.order(&[]), [0, 2, 1], "at rest m is between a and b");
    assert_eq!(order_at.order(&[(2, Rc::new(mover(unit(-3.0, 0.0, 0.0))))]), [2, 0, 1], "behind both");
    assert_eq!(order_at.order(&[(2, Rc::new(mover(unit(7.0, 0.0, 0.0))))]), [0, 1, 2], "in front of both");
    let without_m: Vec<usize> = order_at.order(&[(2, Rc::new(mover(unit(-3.0, 0.0, 0.0))))]).into_iter().filter(|&i| i != 2).collect();
    assert_eq!(without_m, [0, 1]);
}

#[test]
fn r10_still_nodes_keep_one_order_even_where_their_own_boxes_would_put_them_the_other_way() {
    // a stands behind b by their boxes, but they never meet in pixels, so the rest sort keeps the order the file lists them in (b, a).
    // A mover that overlaps both ties them together at some instants; the chain still holds b before a.
    let a = Node { boxed: Some(unit(0.0, 0.0, 0.0)), bounds: bounds(0, 0, 10, 10), depth: None, moving: false };
    let b = Node { boxed: Some(unit(5.0, 0.0, 0.0)), bounds: bounds(20, 0, 30, 10), depth: None, moving: false };
    let m = Node { boxed: Some(unit(2.0, 0.0, 0.0)), bounds: bounds(0, 0, 30, 10), depth: None, moving: true };
    let raw = [b, a, m.clone()];
    let rest = sort_nodes(&raw);
    let listed: Vec<Node> = rest.iter().map(|&i| raw[i].clone()).collect();
    let names = ["b", "a", "m"];
    let stills: Vec<&str> = rest.iter().filter(|&&i| i != 2).map(|&i| names[i]).collect();
    assert_eq!(moving_order(&listed).order(&[]), (0..listed.len()).collect::<Vec<_>>(), "at rest it is the rest order");
    let m_at = rest.iter().position(|&i| i == 2).unwrap();
    for c in [-3.0, 2.0, 9.0] {
        let current = Rc::new(Node { boxed: Some(unit(c, 0.0, 0.0)), bounds: m.bounds, depth: None, moving: true });
        let order = moving_order(&listed).order(&[(m_at, current)]);
        let got: Vec<&str> = order.iter().map(|&i| names[rest[i]]).filter(|n| *n != "m").collect();
        assert_eq!(got, stills, "with m at column {c}: {order:?}");
    }
}

#[test]
fn r10_with_nothing_moved_from_rest_the_order_is_the_rest_order_on_random_lists_with_ties_and_cycles() {
    for seed in SEEDS {
        let mut r = rng(seed);
        for round in 0..40 {
            let n = 4 + r.floor(30.0) as usize;
            let RandomList { listed, .. } = random_list(&mut r, n);
            assert_eq!(moving_order(&listed).order(&[]), (0..listed.len()).collect::<Vec<_>>(), "seed {seed}, round {round}");
        }
    }
}

#[test]
fn r10_the_order_while_moving_agrees_with_a_plain_re_sort_of_the_whole_list_on_random_scenes() {
    for seed in SEEDS {
        let mut r = rng(seed * 3 + 1);
        for round in 0..60 {
            let n = 4 + r.floor(30.0) as usize;
            let RandomList { listed, boxes } = random_list(&mut r, n);
            let mut order_at = moving_order(&listed);
            for instant in 0..5 {
                // Every moving node is somewhere new, with its own pixels (sometimes missing everything).
                let mut current = Vec::new();
                for (i, node) in listed.iter().enumerate() {
                    if node.moving {
                        current.push((i, somewhere(&mut r, &boxes, n)));
                    }
                }
                let real = order_at.order(&current);
                assert_eq!(real, naive_order(&listed, &current_map(&current)), "seed {seed}, round {round}, instant {instant}");
                let stills: Vec<usize> = real.iter().copied().filter(|&i| !listed[i].moving).collect();
                let wanted: Vec<usize> = (0..listed.len()).filter(|&i| !listed[i].moving).collect();
                // Bands put front and back nodes apart; within the whole order the stills only move by band, which rest order already has.
                assert_eq!(stills, wanted, "seed {seed}, round {round}, instant {instant}: the stills keep their order");
            }
        }
    }
}

#[test]
fn r10_the_same_node_objects_handed_over_again_at_many_instants_as_the_svg_does_still_agree_with_a_plain_re_sort_on_lists_up_to_120_nodes()
{
    // moving_order remembers a moving node's pairs with the still ones for as long as it is given the same object, so reuse them across instants.
    for seed in SEEDS {
        let mut r = rng(seed * 5 + 2);
        for round in 0..6 {
            let n = 20 + r.floor(100.0) as usize;
            let RandomList { listed, boxes } = random_list(&mut r, n);
            let places: Vec<Vec<Rc<Node>>> =
                listed.iter().map(|nd| if nd.moving { (0..4).map(|_| somewhere(&mut r, &boxes, n)).collect() } else { vec![] }).collect();
            let mut order_at = moving_order(&listed);
            for instant in 0..12 {
                // Each moving node is in one of its places, or at rest (not in the map) now and then.
                let mut current = Vec::new();
                for (i, node) in listed.iter().enumerate() {
                    if node.moving && r.next() < 0.9 {
                        current.push((i, places[i][r.floor(places[i].len() as f64) as usize].clone()));
                    }
                }
                let real = order_at.order(&current);
                assert_eq!(real, naive_order(&listed, &current_map(&current)), "seed {seed}, round {round}, instant {instant}");
                assert_eq!(
                    order_at.order(&current),
                    real,
                    "seed {seed}, round {round}, instant {instant}: asking again gives the same order"
                );
            }
        }
    }
}

fn plain(moving: bool) -> Node {
    Node { boxed: None, bounds: None, depth: None, moving }
}

#[test]
fn r10_a_place_is_the_still_node_a_moving_one_follows_and_its_rank_among_the_moving_ones_after_it() {
    let nodes = [plain(true), plain(false), plain(true), plain(true), plain(false), plain(true)];
    let places = places_in(&[0, 1, 3, 2, 4, 5], &nodes);
    assert_eq!(
        places,
        [
            (0, Place { follows: -1, rank: 0 }),
            (3, Place { follows: 1, rank: 0 }),
            (2, Place { follows: 1, rank: 1 }),
            (5, Place { follows: 4, rank: 0 }),
        ]
    );
}

#[test]
fn r10_windows_are_the_spans_a_node_spends_in_one_place_merged_each_place_in_the_order_it_first_shows() {
    let cuts = [0.0, 10.0, 20.0, 35.0, 60.0, 100.0];
    let p = |follows: i64, rank: usize| Place { follows, rank };
    let spans: Vec<Vec<(usize, Place)>> = vec![
        vec![(1, p(0, 0)), (2, p(0, 1))],
        vec![(1, p(0, 0)), (2, p(0, 1))],
        vec![(1, p(-1, 0)), (2, p(0, 0))],
        vec![(1, p(-1, 0)), (2, p(0, 0))],
        vec![(1, p(0, 0)), (2, p(0, 1))],
    ];
    let got = place_windows(&cuts, &mut |span| spans[span].clone());
    let find = |node: usize| got.iter().find(|(n, _)| *n == node).map(|(_, c)| c.clone()).unwrap();
    assert_eq!(
        find(1),
        [
            Copy { follows: 0, rank: 0, windows: vec![(0.0, 20.0), (60.0, 100.0)] },
            Copy { follows: -1, rank: 0, windows: vec![(20.0, 60.0)] },
        ]
    );
    assert_eq!(
        find(2),
        [
            Copy { follows: 0, rank: 1, windows: vec![(0.0, 20.0), (60.0, 100.0)] },
            Copy { follows: 0, rank: 0, windows: vec![(20.0, 60.0)] },
        ]
    );
    assert_eq!(got.iter().map(|(n, _)| *n).collect::<Vec<_>>(), [1, 2], "each node in the order it first shows");
}

// ----------------------------- where a moving item is over the loop, as the CSS steps it

#[test]
fn r10_key_percentages_are_rounded_to_3_decimals_as_motion_css_writes_them() {
    assert_eq!(pct_of(8529.0, 28000.0), 30.461);
    assert_eq!(pct_of(5000.0, 10000.0), 50.0);
    assert_eq!(pct_of(28000.0, 28000.0), 100.0);
}

#[test]
fn r10_a_move_steps_at_from_to_from_k_n_between_two_keys_and_loops_back_at_the_end() {
    let t = track("w", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, 4] })]);
    assert!(is_moving(&t, "walker"));
    assert!(
        !is_moving(&track("w", vec![json!({ "t": 0 }), json!({ "t": 5000, "show": false })]), "walker"),
        "a track that only shows and hides stays put"
    );
    let runs = move_runs(&t, "walker", 10000.0);
    assert_eq!(runs.iter().map(|r| r.at).collect::<Vec<_>>(), [0.0, 12.5, 25.0, 37.5, 50.0, 62.5, 75.0, 87.5]);
    assert_eq!(runs.iter().map(|r| r.moved[1]).collect::<Vec<_>>(), [0.0, 1.0, 2.0, 3.0, 4.0, 3.0, 2.0, 1.0]);
    assert_eq!(move_at_percent(&runs, 0.0), [0.0, 0.0]);
    assert_eq!(move_at_percent(&runs, 12.49), [0.0, 0.0]);
    assert_eq!(move_at_percent(&runs, 12.5), [0.0, 1.0], "a step takes effect at its instant");
    assert_eq!(move_at_percent(&runs, 99.99), [0.0, 1.0]);
}

#[test]
fn r10_a_run_starts_where_the_move_changes_and_holds_across_keys_that_do_not_move_it() {
    let t = track(
        "w",
        vec![
            json!({ "t": 0 }),
            json!({ "t": 2000, "move": [2, 1] }),
            json!({ "t": 4000 }),
            json!({ "t": 6000, "move": [2, 1], "show": false }),
        ],
    );
    let runs = move_runs(&t, "walker", 8000.0);
    // One (2, 1) step arrives at the key at 25%, the move holds through the keys at 50% and 75%, and the one step back is the loop's end.
    assert_eq!(runs.iter().map(|r| (r.at, r.moved)).collect::<Vec<_>>(), [(0.0, [0.0, 0.0]), (25.0, [2.0, 1.0])]);
}

// ------------------------------------------------------ the copies an item draws

fn copies_of(sources: &Sources) -> Vec<(String, usize)> {
    motion_copies(sources, "s").unwrap()
}

fn walker(id: &str, px: [i32; 2]) -> Value {
    json!({ "object": "walker", "id": id, "at": { "px": px } })
}

fn house_at(tile: [i32; 3]) -> Value {
    json!({ "object": "house", "at": { "tile": tile } })
}

fn pairs(list: &[(&str, usize)]) -> Vec<(String, usize)> {
    list.iter().map(|(a, b)| (a.to_string(), *b)).collect()
}

#[test]
fn r10_a_walker_that_passes_behind_a_house_and_then_in_front_of_it_takes_two_places() {
    // The house stands at tile 2,2 on level 1; the walker goes down the column the house is in.
    let sources = one(scene10(
        vec![house_at([2, 2, 1]), walker("w", [0, 0])],
        vec![track("w", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, 64] })])],
    ));
    assert_eq!(copies_of(&sources), pairs(&[("w", 2)]));
}

#[test]
fn r10_an_item_that_never_meets_another_in_pixels_or_never_changes_order_with_it_has_one_place_and_no_copies() {
    let away = one(scene10(
        vec![house_at([6, 0, 0]), walker("w", [-60, 0])],
        vec![track("w", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, 40] })])],
    ));
    assert_eq!(copies_of(&away), pairs(&[]));
    // It walks up to the house from the front and stops short of it.
    let front = one(scene10(
        vec![house_at([0, 0, 1]), walker("w", [0, 60])],
        vec![track("w", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, 10] })])],
    ));
    assert_eq!(copies_of(&front), pairs(&[]));
    assert_eq!(copies_of(&one(scene10(vec![walker("w", [0, 0])], vec![]))), pairs(&[]), "no motion at all");
}

#[test]
fn r10_two_items_that_cross_each_other_each_take_two_places_told_apart_by_rank() {
    // They walk toward each other down one column and pass for a few pixels: for those steps the one that has gone past is in front.
    let sources = one(scene10(
        vec![walker("a", [0, 0]), walker("b", [0, 64])],
        vec![
            track("a", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, 64] })]),
            track("b", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, -64] })]),
        ],
    ));
    assert_eq!(copies_of(&sources), pairs(&[("a", 2), ("b", 2)]));
}

#[test]
fn r10_a_moving_item_inside_a_group_reorders_among_the_groups_members_only_the_group_keeps_its_rest_depth() {
    let in_group = one(scene10(
        vec![house_at([4, 4, 1]), json!({ "group": { "class": "enter" }, "items": [house_at([2, 2, 1]), walker("w", [0, 0])] })],
        vec![track("w", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, 64] })])],
    ));
    // The walker passes behind and in front of its own group's house.
    assert_eq!(copies_of(&in_group), pairs(&[("w", 2)]));
    // Alone in its group it has nothing to reorder with, though it crosses a house outside the group.
    let alone = one(scene10(
        vec![house_at([2, 2, 1]), json!({ "group": { "class": "enter" }, "items": [walker("w", [0, 0])] })],
        vec![track("w", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, 64] })])],
    ));
    assert_eq!(copies_of(&alone), pairs(&[]));
}

#[test]
fn r10_the_number_of_places_does_not_depend_on_the_order_the_file_lists_the_items_in() {
    let items = vec![house_at([2, 2, 1]), walker("w", [0, 0])];
    let tracks = vec![track("w", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, 64] })])];
    let reversed: Vec<Value> = items.iter().rev().cloned().collect();
    assert_eq!(copies_of(&one(scene10(items, tracks.clone()))), copies_of(&one(scene10(reversed, tracks))));
}

// --------------------------------------------- the SVG draws a copy for each place

fn walker_scene() -> Sources {
    one(scene10(
        vec![house_at([2, 2, 1]), walker("w", [0, 0])],
        vec![track("w", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, 64] })])],
    ))
}

fn svg(sources: &Sources, name: &str) -> String {
    to_rect_svg(&render_scene(sources, name, Order::Depth).unwrap(), &hex_fn(sources))
}

fn style_of_scene(sources: &Sources, name: &str, order: Order) -> String {
    render_scene(sources, name, order).unwrap().style.unwrap_or_default()
}

fn has_copy_class(style: &str) -> bool {
    style.match_indices("-z").any(|(i, _)| style[i + 2..].starts_with(|c: char| c.is_ascii_digit()))
}

#[test]
fn r10_each_copy_is_a_wrapper_holding_the_whole_moving_group_in_its_place_in_the_list_and_only_the_copy_holding_time_0_shows_in_the_base_rule(
) {
    let sources = walker_scene();
    let out = render_scene(&sources, "s", Order::Depth).unwrap();
    let style = out.style.clone().unwrap();
    // The walker is behind the house for part of the loop (its copy z1 sits before the house) and in front of it the rest.
    assert_eq!(mv_classes(&out.root), ["mv-w-z1", "mv-w", "mv-w-z0", "mv-w"]);
    assert!(style.contains(".mv-w-z0{animation:mv-w-z0 10000ms step-end infinite}"));
    assert!(style.contains(".mv-w-z1{opacity:0;animation:mv-w-z1 10000ms step-end infinite}"));
    assert!(style.contains("@keyframes mv-w-z0{0%{opacity:1}7.0313%{opacity:0}27.3438%{opacity:1}73.4375%{opacity:0}93.75%{opacity:1}}"));
    assert!(style.contains("@keyframes mv-w-z1{0%{opacity:0}7.0313%{opacity:1}27.3438%{opacity:0}73.4375%{opacity:1}93.75%{opacity:0}}"));
    // Each copy holds the item's own motion group, whose rules are the item's as before.
    assert!(style.contains("@keyframes mv-w{0%{transform:translate(0,0)"));
    let mixed = svg(&sources, "s");
    assert_eq!(mixed.matches("<g class=\"mv-w\">").count(), 2);
}

#[test]
fn r10_exactly_one_copy_shows_at_every_moment_of_the_loop_and_the_one_holding_time_0_shows_at_rest() {
    let sources = walker_scene();
    let out = render_scene(&sources, "s", Order::Depth).unwrap();
    let style = out.style.unwrap();
    let css = parse_css(&style);
    let copies = ["mv-w-z0", "mv-w-z1"];
    let shown =
        |p: f64, animate: bool| -> Vec<&str> { copies.iter().copied().filter(|c| style_of(&css, c, p, animate).opacity == 1.0).collect() };
    let mut p = 0.0;
    while p < 100.0 {
        assert_eq!(shown(p, true).len(), 1, "at {p}%: {:?}", shown(p, true));
        p += 0.0625;
    }
    for boundary in [7.0313, 27.3438, 73.4375, 93.75] {
        let before = shown(boundary - 0.0001, true);
        let at = shown(boundary, true);
        assert_ne!(before, at, "a copy switches at {boundary}%");
        assert_eq!(at.len(), 1);
    }
    assert_eq!(shown(0.0, false), ["mv-w-z0"], "reduced motion shows the copy of time 0 only");
    assert!(
        style.ends_with("@media (prefers-reduced-motion:reduce){[class^=\"mv-\"],[class*=\" mv-\"]{animation:none}}"),
        "the reduced-motion rule covers the copies"
    );
    assert!(copies.iter().all(|c| c.starts_with("mv-")));
}

#[test]
fn r10_the_css_steps_a_track_exactly_as_move_runs_says_wherever_it_is_asked() {
    // Keys on whole percentages so the steps fall on exact floating-point values.
    let tracks = vec![
        track(
            "w",
            vec![
                json!({ "t": 0 }),
                json!({ "t": 2500, "move": [0, 8] }),
                json!({ "t": 5000, "move": [8, 12] }),
                json!({ "t": 7500, "move": [2, 1] }),
            ],
        ),
        track("x", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [-4, 4], "show": false }), json!({ "t": 7500, "show": true })]),
    ];
    let sources = one(scene10(vec![walker("w", [0, 0]), walker("x", [30, 0])], tracks.clone()));
    let out = render_scene(&sources, "s", Order::Depth).unwrap();
    let css = parse_css(&out.style.unwrap());
    for t in &tracks {
        let runs = move_runs(t, "walker", 10000.0);
        let rest = runs[0].moved;
        let mut cuts: Vec<f64> = runs.iter().map(|r| r.at).collect();
        cuts.push(100.0);
        for i in 0..runs.len() {
            for p in [cuts[i], (cuts[i] + cuts[i + 1]) / 2.0, cuts[i + 1] - 0.001] {
                let got = style_of(&css, &format!("mv-{}", t["item"].as_str().unwrap()), p, true).translate;
                assert_eq!(got, [runs[i].moved[0] - rest[0], runs[i].moved[1] - rest[1]], "{} at {p}%", t["item"]);
            }
        }
    }
}

#[test]
fn r10_an_item_whose_place_never_changes_is_drawn_once_byte_for_byte_as_the_list_order_engine_draws_it() {
    let items = vec![house_at([6, 0, 0]), walker("w", [-60, 0])];
    let sources = one(scene10(
        items.clone(),
        vec![track(
            "w",
            vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, 40] }), json!({ "t": 7000, "move": [0, 40], "pose": "walker" })],
        )],
    ));
    assert_eq!(copies_of(&sources), pairs(&[]));
    let depth = render_scene(&sources, "s", Order::Depth).unwrap();
    let list = render_scene(&sources, "s", Order::List).unwrap();
    let hex = hex_fn(&sources);
    assert_eq!(to_rect_svg(&depth, &hex), to_rect_svg(&list, &hex));
    let mut ids = OMap::new();
    ids.set("w".to_string(), items[1].clone());
    let expected = motion_css(&sources.scenes["s"]["motion"], &ids, &OMap::new());
    assert_eq!(depth.style.clone().unwrap(), expected);
    assert!(!has_copy_class(&depth.style.unwrap()));
}

#[test]
fn r10_order_list_draws_no_copies_the_files_order_as_engines_before_the_depth_order_drew_it() {
    let sources = walker_scene();
    let list = render_scene(&sources, "s", Order::List).unwrap();
    assert_eq!(mv_classes(&list.root), ["mv-w"]);
    assert!(!has_copy_class(&style_of_scene(&sources, "s", Order::List)));
}

#[test]
fn r10_a_copy_holds_everything_the_item_draws_as_before_its_class_wrapper_its_poses_and_its_show_rule() {
    let items = vec![house_at([2, 2, 1]), json!({ "object": "walker", "id": "w", "class": "cargo", "at": { "px": [0, 0] } })];
    let mut objs = objects();
    objs.push(("walker-b", walker_b()));
    let sources = src(
        &objs,
        &[(
            "s",
            scene10(
                items,
                vec![track(
                    "w",
                    vec![json!({ "t": 0 }), json!({ "t": 2500, "pose": "walker-b" }), json!({ "t": 5000, "move": [0, 64], "show": false })],
                )],
            ),
        )],
    );
    let out = render_scene(&sources, "s", Order::Depth).unwrap();
    assert_eq!(mv_classes(&out.root), ["mv-w-z1", "mv-w", "mv-w-0", "mv-w-1", "mv-w-z0", "mv-w", "mv-w-0", "mv-w-1"]);
    let copy = out.root.children.iter().filter_map(as_group).find(|g| g.attrs.first().is_some_and(|a| a.1 == "mv-w-z0")).unwrap();
    assert_eq!(copy.children.len(), 1, "one moving group in the copy");
    let pose = as_group(&copy.children[0]).unwrap();
    let wrapper = as_group(&as_group(&pose.children[0]).unwrap().children[0]).unwrap();
    assert_eq!(wrapper.attrs[0].1, "cargo", "the item class wraps its pixels in each pose");
    let style = out.style.unwrap();
    for rule in ["@keyframes mv-w-s", "@keyframes mv-w-0", "@keyframes mv-w-1", ".mv-w-1{opacity:0;"] {
        assert!(style.contains(rule), "{rule}");
    }
}

#[test]
fn r10_a_group_members_copies_are_inside_its_group_and_the_group_draws_where_it_rests() {
    let sources = one(scene10(
        vec![house_at([4, 4, 1]), json!({ "group": { "class": "enter" }, "items": [house_at([2, 2, 1]), walker("w", [0, 0])] })],
        vec![track("w", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, 64] })])],
    ));
    let out = render_scene(&sources, "s", Order::Depth).unwrap();
    let group = out
        .root
        .children
        .iter()
        .filter_map(as_group)
        .find(|g| g.attrs.iter().any(|(k, v)| k == "class" && v == "enter"))
        .expect("the group is a <g> of its own");
    assert_eq!(mv_classes(group), ["mv-w-z1", "mv-w", "mv-w-z0", "mv-w"]);
    assert_eq!(mv_classes(&out.root), mv_classes(group), "no copy outside the group");
    assert_eq!(out.root.children.len(), 2, "the outside house and the group");
}

#[test]
fn r10_windows_are_written_to_4_decimals_and_stops_that_round_to_one_point_collapse_into_the_later() {
    let motion = json!({ "duration": 10000, "tracks": [{ "item": "w", "keys": [{ "t": 0 }, { "t": 5000, "move": [0, 4] }] }] });
    let mut ids = OMap::new();
    ids.set("w".to_string(), json!({ "object": "walker" }));
    // The `@keyframes mv-w-z<n>{…}` blocks, without the `@keyframes `.
    let at = |windows: Vec<Vec<(f64, f64)>>| -> Vec<String> {
        let mut copies = OMap::new();
        copies.set("w".to_string(), windows.into_iter().map(|w| Copy { follows: 0, rank: 0, windows: w }).collect::<Vec<_>>());
        let css = motion_css(&motion, &ids, &copies);
        let mut out = Vec::new();
        let mut from = 0;
        while let Some(i) = css[from..].find("@keyframes mv-w-z") {
            let start = from + i + "@keyframes ".len();
            let mut depth = 0;
            let mut end = start;
            for (k, ch) in css[start..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = start + k + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            out.push(css[start..end].to_string());
            from = end;
        }
        out
    };
    assert_eq!(
        at(vec![vec![(0.0, 25.0), (75.0, 100.0)], vec![(25.0, 75.0)]]),
        ["mv-w-z0{0%{opacity:1}25%{opacity:0}75%{opacity:1}}", "mv-w-z1{0%{opacity:0}25%{opacity:1}75%{opacity:0}}"]
    );
    assert_eq!(
        at(vec![vec![(0.0, 25.000012), (25.000012, 100.0)], vec![(25.00004, 25.00004)]]),
        ["mv-w-z0{0%{opacity:1}}", "mv-w-z1{0%{opacity:0}}"],
        "a window shorter than 0.0001% is never shown"
    );
    assert_eq!(
        at(vec![vec![(0.0, 12.34567)], vec![(12.34567, 100.0)]]),
        ["mv-w-z0{0%{opacity:1}12.3457%{opacity:0}}", "mv-w-z1{0%{opacity:0}12.3457%{opacity:1}}"]
    );
}

#[test]
fn r10_an_id_that_looks_like_another_tracks_copy_is_a_problem_only_once_that_copy_exists() {
    let items = vec![house_at([2, 2, 1]), walker("w", [0, 0]), walker("w-z1", [40, 0])];
    let tracks = vec![
        track("w", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, 64] })]),
        track("w-z1", vec![json!({ "t": 0 }), json!({ "t": 5000, "show": false })]),
    ];
    let problems = validate(&one(scene10(items, tracks)));
    let clash = problems.iter().filter(|p| p.contains("w-z1") && p.contains("copy 1 of 'w'")).count();
    assert_eq!(clash, 1, "{}", problems.join("\n"));
    // With the walker nowhere near the house there are no copies, so no clash.
    let apart = scene10(
        vec![house_at([6, 0, 0]), walker("w", [-60, 0]), walker("w-z1", [40, 0])],
        vec![
            track("w", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, 40] })]),
            track("w-z1", vec![json!({ "t": 0 }), json!({ "t": 5000, "show": false })]),
        ],
    );
    assert_eq!(validate(&one(apart)), Vec::<String>::new());
}

#[test]
fn r10_r12_a_scene_of_hero_islands_size_220_items_39_moving_ones_with_25_keys_draws_in_a_few_seconds() {
    let mut r = rng(2026);
    let mut items = Vec::new();
    for c in 0..15 {
        for row in 0..15 {
            let object = if r.next() < 0.5 { "house" } else { "walker" };
            let level = if r.next() < 0.2 { 1 } else { 0 };
            items.push(json!({ "object": object, "at": { "tile": [c, row, level] } }));
        }
    }
    let mut tracks = Vec::new();
    for i in 0..39 {
        let id = format!("m{i}");
        let px = [r.floor(200.0) - 100, r.floor(120.0)];
        items.push(json!({ "object": "walker", "id": id, "at": { "px": px } }));
        let mut keys = vec![json!({ "t": 0 })];
        let (mut x, mut y) = (0, 0);
        for k in 1..25 {
            x += 2 * (r.floor(11.0) - 5);
            y += r.floor(11.0) - 5;
            keys.push(json!({ "t": k * 1000 + r.floor(500.0), "move": [x, y] }));
        }
        tracks.push(track(&id, keys));
    }
    let sources = one(scn(items, tracks, 28000));
    let started = std::time::Instant::now();
    let out = render_scene(&sources, "s", Order::Depth).unwrap();
    let took = started.elapsed().as_millis();
    let copies = copies_of(&sources);
    assert!(!copies.is_empty(), "items cross others in the order");
    assert!(out.style.unwrap().contains("-z0"), "the CSS has copies");
    assert!(took < 5000, "took {took} ms");
}

// ---------------- the preview draws what the published SVG shows (engine parts)

#[test]
fn r10_r15_on_random_scenes_with_keys_at_arbitrary_milliseconds_the_svg_and_the_preview_agree_in_the_middle_of_every_stretch() {
    for seed in &SEEDS[..3] {
        let seed = *seed;
        let mut r = rng(seed + 100);
        let mut items = Vec::new();
        for _ in 0..14 {
            let object = *r.pick(&["house", "walker", "tall"]);
            let tile = [r.floor(7.0) - 1, r.floor(7.0) - 1, r.floor(2.0)];
            items.push(json!({ "object": object, "at": { "tile": tile } }));
        }
        let mut tracks = Vec::new();
        for i in 0..5 {
            let id = format!("m{i}");
            let at = r.floor(items.len() as f64) as usize;
            let object = *r.pick(&["walker", "tall"]);
            let px = [r.floor(80.0) - 40, r.floor(50.0)];
            items.insert(at, json!({ "object": object, "id": id, "at": { "px": px } }));
            let mut keys = vec![json!({ "t": 0 })];
            let (mut x, mut y) = (0i64, 0i64);
            for k in 1..=5 {
                let along = if r.next() < 0.5 { [2, 1] } else { [0, 1] };
                let steps = 4 + r.floor(24.0);
                x += along[0] * steps * if r.next() < 0.5 { -1 } else { 1 };
                y += along[1] * steps * if r.next() < 0.5 { -1 } else { 1 };
                let mut key = json!({ "t": k * 1900 + r.floor(1500.0), "move": [x, y] });
                if r.next() < 0.2 {
                    key["show"] = json!(r.next() < 0.5);
                }
                keys.push(key);
            }
            tracks.push(track(&id, keys));
        }
        let s = scn(items, tracks.clone(), 12000);
        let mut objs = objects();
        objs.push(("tall", tall()));
        let sources = src(&objs, &[("s", s)]);
        assert_eq!(validate(&sources), Vec::<String>::new(), "seed {seed}");
        let out = render_scene(&sources, "s", Order::Depth).unwrap();
        let css = parse_css(out.style.as_deref().unwrap());
        let mut cuts: Vec<f64> = vec![0.0];
        for (name, stops) in &css.keyframes {
            let is_copy = name.rsplit_once("-z").is_some_and(|(_, d)| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()));
            if is_copy {
                cuts.extend(stops.iter().map(|s| s.at));
            }
        }
        for t in &tracks {
            cuts.extend(move_runs(t, "walker", 12000.0).iter().map(|run| run.at));
        }
        cuts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        cuts.dedup();
        let mut checked = 0;
        let stride = (cuts.len() as f64 / 150.0).ceil() as usize;
        for (i, &at) in cuts.iter().enumerate() {
            let next = cuts.get(i + 1).copied().unwrap_or(100.0);
            if i % stride != 0 || (next - at) * 120.0 < 6.0 {
                continue; // a stretch of under 6 ms: the CSS and the preview round it differently
            }
            let time = ((at + next) / 2.0) * 120.0;
            let at0 = scene_at(&sources, "s", time).unwrap();
            let mut shown = at0.clone();
            shown["items"] = Value::Array(shown_items(at0["items"].as_array().unwrap()));
            let mut listed = sources.clone();
            listed.scenes = [("s".to_string(), shown)].into_iter().collect();
            let preview = map_of(&composite(
                &render_scene(&listed, "s", Order::List).unwrap().root,
                CompositeOptions { time: Some(time), ..Default::default() },
            ));
            let published = paint_css(&out, time / 120.0, 12000.0, true);
            let bad = differences(&preview, &published);
            assert!(
                bad.is_empty(),
                "seed {seed} at {time:.1} ms: {} pixels differ, e.g. {:?}",
                bad.len(),
                bad.iter().take(3).map(|q| format!("{q:?}: {:?} / {:?}", preview.get(q), published.get(q))).collect::<Vec<_>>()
            );
            checked += 1;
        }
        assert!(checked > 40, "seed {seed}: {checked} stretches checked");
        assert!(!copies_of(&sources).is_empty(), "seed {seed}: items change places");
    }
}

#[test]
fn r10_pose_items_keeps_a_tracked_items_level_whether_it_was_placed_on_a_tile_or_by_pixel_point() {
    let motion = json!({
        "duration": 1000,
        "tracks": [
            track("a", vec![json!({ "t": 0 }), json!({ "t": 500, "move": [0, 4] })]),
            track("b", vec![json!({ "t": 0 }), json!({ "t": 500, "move": [0, 4] })]),
            track("c", vec![json!({ "t": 0 }), json!({ "t": 500, "move": [0, 4] })]),
        ],
    });
    let items = vec![
        json!({ "object": "walker", "id": "a", "at": { "tile": [1, 2, 3] } }),
        json!({ "object": "walker", "id": "b", "at": { "px": [5, 6], "level": 2 } }),
        json!({ "object": "walker", "id": "c", "at": { "px": [1, 1] } }),
    ];
    let point_of = |item: &Value| -> [f64; 2] {
        match item["at"].get("tile") {
            Some(t) => {
                let n = |i: usize| t[i].as_f64().unwrap();
                [(n(0) - n(1)) * 16.0, (n(0) + n(1)) * 8.0 - n(2) * 16.0]
            }
            None => [item["at"]["px"][0].as_f64().unwrap(), item["at"]["px"][1].as_f64().unwrap()],
        }
    };
    let posed = pose_items(&items, Some(&motion), 500.0, &point_of);
    assert_eq!(posed[0]["at"], json!({ "px": [-16, -20], "level": 3 }), "tile 1,2,3 is at -16,-24, moved by 0,4");
    assert_eq!(posed[1]["at"], json!({ "px": [5, 10], "level": 2 }));
    assert_eq!(posed[2]["at"], json!({ "px": [1, 5] }), "level 0 is left out, as in the file");
}
