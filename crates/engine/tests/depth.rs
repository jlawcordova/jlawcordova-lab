//! Port of cli/test/depth.test.mjs (docs/intents/2026-10-depth-order/spec.md):
//! the geometry and the sort (R1, R2), the scene and object formats (R5, R7,
//! R8), and the engine drawing in depth order (R3, R9).
//!
//! Adapted: problems name `.json` files; the serializer tests check format 2's
//! text (expected text from Node's `canonical` in scripts/goldens.mjs); the
//! harbor tests run on a copy of examples/harbor.pixellab (format 2).
//! Not here (they are program tests): "scene.items lists an item's depth…",
//! "a document carries the depth module" (Node's module wiring, no Rust
//! counterpart) and the `scene.render` half of "the compiler, the preview and
//! scene.render all draw the tree under the house" (the engine and preview
//! halves are).

mod depthlib;

use std::collections::HashMap;

use depthlib::*;
use engine::compile::{compile_scene, Budgets};
use engine::depth::{behind, block_box, displaced, key_of, overlaps, shift_box, sort_nodes, sprite_box, sprite_height};
use engine::depth::{union_bounds, union_box, Bounds, Box3, Mark, Node};
use engine::engine::{
    composite, depth_order, is_in_depth_order, origin_of, render_scene, resolve, scene_in_order, tracks_of, validate, CompositeOptions,
    Order, Sources,
};
use engine::format::{parse, serialize, serialize_file};
use engine::iso::{px_to_tile, px_to_tile_exact, tile_to_px};
use engine::omap::OMap;
use engine::preview::{preview_colors, render_preview, Target};
use serde_json::{json, Value};

/// Pixel bounds big enough that every pair of nodes is tested.
const EVERYWHERE: Bounds = Bounds { min_x: -1000, min_y: -1000, max_x: 1000, max_y: 1000 };

fn node(b: Box3) -> Node {
    Node { boxed: Some(b), bounds: Some(EVERYWHERE), depth: None, moving: false }
}

fn node_d(b: Box3, depth: Mark) -> Node {
    Node { depth: Some(depth), ..node(b) }
}

fn node_b(b: Box3, bounds: Option<Bounds>) -> Node {
    Node { boxed: Some(b), bounds, depth: None, moving: false }
}

fn bounds(min_x: i32, min_y: i32, max_x: i32, max_y: i32) -> Option<Bounds> {
    Some(Bounds { min_x, min_y, max_x, max_y })
}

/// The names in the order the sort puts them.
fn names(nodes: &[Node], labels: &[&'static str]) -> Vec<&'static str> {
    sort_nodes(nodes).into_iter().map(|i| labels[i]).collect()
}

fn bx(c0: f64, c1: f64, r0: f64, r1: f64, l0: f64, l1: f64) -> Box3 {
    Box3 { c0, c1, r0, r1, l0, l1 }
}

fn unit(c: f64, r: f64, l: f64) -> Box3 {
    block_box(c, r, l, [1.0, 1.0, 1.0])
}

fn pos(list: &[&str], name: &str) -> usize {
    list.iter().position(|n| *n == name).unwrap()
}

fn boxed(b: Box3) -> Option<Box3> {
    Some(b)
}

// ------------------------------------------------------------------ boxes

#[test]
fn r1_a_block_stands_on_its_footprint_from_the_first_tile_and_hangs_below_its_top_face() {
    assert_eq!(block_box(2.0, 3.0, 1.0, [1.0, 1.0, 1.0]), bx(1.5, 2.5, 2.5, 3.5, 0.0, 1.0));
    assert_eq!(block_box(2.0, 5.0, 1.0, [2.0, 1.0, 1.0]), bx(1.5, 3.5, 4.5, 5.5, 0.0, 1.0), "the hall is two tiles wide");
    assert_eq!(block_box(0.0, 0.0, 0.0, [1.0, 1.0, 0.0]), bx(-0.5, 0.5, -0.5, 0.5, 0.0, 0.0), "a flat tile has no thickness");
}

#[test]
fn r1_a_sprite_rises_from_its_tile_and_covers_its_footprint() {
    assert_eq!(sprite_box(1.0, 2.0, 0.0, [1.0, 1.0], 17.0 / 16.0), bx(0.5, 1.5, 1.5, 2.5, 0.0, 17.0 / 16.0));
    assert_eq!(
        sprite_box(1.0, 2.0, 1.0, [3.0, 2.0], 2.0),
        bx(0.5, 3.5, 1.5, 3.5, 1.0, 3.0),
        "a footprint of 3×2 covers 3 columns and 2 rows"
    );
    assert_eq!(sprite_height(17.0), 17.0 / 16.0, "the anchor row over a level");
    assert_eq!(sprite_height(0.0), 1.0 / 8.0, "a flat sprite is at least an eighth of a level");
    assert_eq!(sprite_height(1.0), 1.0 / 8.0);
}

#[test]
fn r1_r7_a_pixel_point_stands_where_a_tile_at_the_same_point_and_level_would_between_tiles() {
    for origin in [[0.0, 0.0], [5.0, 32.0], [-17.0, 3.0]] {
        for level in [0.0, 1.0, -2.0] {
            for tile in [[0.0, 0.0], [2.0, 1.0], [-3.0, 4.0]] {
                let px = tile_to_px([tile[0], tile[1], level], origin);
                assert_eq!(px_to_tile_exact(px, level, origin), tile, "tile {tile:?}, level {level}, origin {origin:?}");
            }
        }
    }
    assert_eq!(px_to_tile_exact([16.0, 8.0], 0.0, [0.0, 0.0]), [1.0, 0.0]);
    assert_eq!(px_to_tile_exact([0.0, 8.0], 0.0, [0.0, 0.0]), [0.5, 0.5], "half way between tiles");
    assert_eq!(
        px_to_tile_exact([0.0, 0.0], 1.0, [0.0, 0.0]),
        [1.0, 1.0],
        "a level up is a tile along both, so [1, 1, 1] is drawn where [0, 0, 0] is"
    );
    assert_eq!(px_to_tile_exact([1.0, 0.0], 0.0, [0.0, 0.0]), [1.0 / 32.0, -1.0 / 32.0], "a pixel is 1/32 of a tile");
    assert!(px_to_tile_exact([0.0, 0.0], 0.0, [0.0, 0.0])[0].is_sign_positive(), "never -0");
    // The rounded conversion the editor uses agrees at tile centers.
    assert_eq!(px_to_tile([16.0, 8.0], 0.0, [0.0, 0.0]), [1.0, 0.0, 0.0]);
}

#[test]
fn r1_unions_of_boxes_and_of_pixel_bounds_ignoring_nothing_boxes() {
    assert_eq!(
        union_box(&[boxed(block_box(0.0, 0.0, 0.0, [1.0, 1.0, 1.0])), None, boxed(block_box(2.0, 0.0, 0.0, [1.0, 1.0, 2.0]))]),
        Some(bx(-0.5, 2.5, -0.5, 0.5, -2.0, 0.0))
    );
    assert_eq!(union_box(&[None]), None);
    assert_eq!(union_box(&[]), None);
    assert_eq!(union_bounds(&[bounds(0, 0, 3, 3), None, bounds(-2, 1, 1, 9)]), bounds(-2, 0, 3, 9));
    assert_eq!(union_bounds(&[None]), None);
    assert_eq!(shift_box(block_box(0.0, 0.0, 0.0, [1.0, 1.0, 1.0]), 1.0, -2.0, 3.0), bx(0.5, 1.5, -2.5, -1.5, 2.0, 3.0));
    assert!(overlaps(bounds(0, 0, 3, 3), bounds(3, 3, 5, 5)), "sharing one pixel is overlap");
    assert!(!overlaps(bounds(0, 0, 3, 3), bounds(4, 0, 5, 5)));
    assert!(!overlaps(None, bounds(0, 0, 3, 3)), "nothing drawn meets nothing");
}

// ----------------------------------------------------------------- behind

#[test]
fn r1_a_box_that_ends_before_another_starts_on_one_axis_is_behind_it_on_each_axis() {
    let middle = boxed(unit(5.0, 5.0, 5.0));
    assert!(behind(boxed(unit(4.0, 5.0, 5.0)), middle), "column");
    assert!(behind(boxed(unit(5.0, 4.0, 5.0)), middle), "row");
    assert!(behind(boxed(unit(5.0, 5.0, 4.0)), middle), "level");
    for flipped in [unit(6.0, 5.0, 5.0), unit(5.0, 6.0, 5.0), unit(5.0, 5.0, 6.0)] {
        assert!(behind(middle, boxed(flipped)));
    }
    for flipped in [unit(6.0, 5.0, 5.0), unit(5.0, 6.0, 5.0), unit(5.0, 5.0, 6.0)] {
        assert!(!behind(boxed(flipped), middle));
    }
    assert!(behind(boxed(unit(3.0, 5.0, 5.0)), middle), "any gap, not only a touching neighbour");
}

#[test]
fn r1_boxes_that_touch_are_separated_boxes_that_share_volume_are_not() {
    assert!(behind(boxed(unit(4.0, 5.0, 5.0)), boxed(unit(5.0, 5.0, 5.0))), "touching at c = 4.5");
    let reaching = block_box(4.0, 5.0, 5.0, [1.5, 1.0, 1.0]); // reaches into the other's column
    assert!(behind(boxed(reaching), boxed(unit(5.0, 5.0, 5.0))), "no axis separates them, so the smaller centre sum is behind");
    assert!(!behind(boxed(unit(5.0, 5.0, 5.0)), boxed(reaching)));
    assert!(behind(boxed(reaching), boxed(shift_box(reaching, 0.5, 0.0, 0.0))));
}

#[test]
fn r1_behind_on_one_axis_and_in_front_on_another_means_the_silhouettes_cannot_overlap_so_no_order() {
    let a = boxed(unit(0.0, 4.0, 5.0)); // behind b by column, in front of b by row
    let b = boxed(unit(4.0, 0.0, 5.0));
    assert!(!behind(a, b));
    assert!(!behind(b, a));
    let low = boxed(unit(0.0, 0.0, 0.0)); // behind by level, in front by column
    let high = boxed(unit(-3.0, 5.0, 3.0));
    assert!(!behind(low, high));
    assert!(!behind(high, low));
}

#[test]
fn r1_interpenetrating_boxes_the_one_whose_centre_sum_is_smaller_is_behind_and_a_tie_is_no_order() {
    let a = block_box(0.0, 0.0, 0.0, [2.0, 2.0, 2.0]);
    let b = shift_box(a, 1.0, 0.0, 0.0);
    assert!(behind(boxed(a), boxed(b)));
    assert!(!behind(boxed(b), boxed(a)));
    assert!(!behind(boxed(a), boxed(a)), "a box is not behind itself");
    assert!(!behind(boxed(a), boxed(shift_box(a, 1.0, -1.0, 0.0))), "equal sums are a tie");
    assert!(!behind(boxed(shift_box(a, 1.0, -1.0, 0.0)), boxed(a)));
}

#[test]
fn r1_a_missing_box_is_behind_nothing_and_has_nothing_behind_it() {
    assert!(!behind(None, boxed(unit(0.0, 0.0, 0.0))));
    assert!(!behind(boxed(unit(0.0, 0.0, 0.0)), None));
    assert_eq!(key_of(None), f64::NEG_INFINITY);
    assert_eq!(key_of(boxed(unit(1.0, 2.0, 3.0))), 0.5 + 1.5 + 1.5 + 2.5 + 2.0 + 3.0, "twice the centre sum: c0+c1+r0+r1+l0+l1");
}

// --------------------------------------------------------------- the sort

#[test]
fn r1_the_pine_at_0_1_0_draws_before_the_red_houses_walls_at_1_1_1_whatever_the_list_says() {
    let pine = sprite_box(0.0, 1.0, 0.0, [1.0, 1.0], 17.0 / 16.0);
    let walls = block_box(1.0, 1.0, 1.0, [1.0, 1.0, 1.0]);
    let roof = block_box(1.0, 1.0, 2.0, [1.0, 1.0, 1.0]);
    let labels = ["walls", "roof", "pine"];
    assert_eq!(names(&[node(walls), node(roof), node(pine)], &labels), ["pine", "walls", "roof"]);
    let front = sprite_box(2.0, 1.0, 0.0, [1.0, 1.0], 17.0 / 16.0); // on the other side of the house
    assert_eq!(names(&[node(front), node(walls), node(roof)], &["front", "walls", "roof"]), ["walls", "roof", "front"]);
}

#[test]
fn r1_the_halls_2_1_walls_and_roof_sort_against_the_tiles_and_trees_around_them_not_by_their_first_tile() {
    let walls = block_box(2.0, 5.0, 1.0, [2.0, 1.0, 1.0]);
    let roof = block_box(2.0, 5.0, 2.0, [2.0, 1.0, 1.0]);
    let behind_right = sprite_box(3.0, 4.0, 0.0, [1.0, 1.0], 15.0 / 16.0); // beside the hall's far half, one row back
    let in_front_right = sprite_box(3.0, 6.0, 0.0, [1.0, 1.0], 15.0 / 16.0); // one row in front of its far half
    let under = block_box(3.0, 5.0, 0.0, [1.0, 1.0, 1.0]); // the tile under its far half
    let labels = ["roof", "inFront", "walls", "under", "behind"];
    let list = [node(roof), node(in_front_right), node(walls), node(under), node(behind_right)];
    let got = names(&list, &labels);
    assert!(pos(&got, "behind") < pos(&got, "walls"), "a tree one row behind draws before the hall: {got:?}");
    assert!(pos(&got, "under") < pos(&got, "walls"), "the tile under its far half draws first: {got:?}");
    assert!(pos(&got, "walls") < pos(&got, "roof"));
    assert!(pos(&got, "roof") < pos(&got, "inFront"), "a tree one row in front draws after the roof: {got:?}");
}

#[test]
fn r1_a_raised_block_is_ordered_against_the_tiles_around_it_by_its_level_too() {
    let pillar = block_box(1.0, 1.0, 3.0, [1.0, 1.0, 3.0]); // levels 0 to 3
    let ground_front = block_box(2.0, 1.0, 0.0, [1.0, 1.0, 1.0]);
    let ground_back = block_box(0.0, 1.0, 0.0, [1.0, 1.0, 1.0]);
    let top = block_box(1.0, 1.0, 4.0, [1.0, 1.0, 1.0]); // on top of the pillar
    let labels = ["top", "groundFront", "pillar", "groundBack"];
    let got = names(&[node(top), node(ground_front), node(pillar), node(ground_back)], &labels);
    assert!(pos(&got, "groundBack") < pos(&got, "pillar"), "{got:?}");
    assert!(pos(&got, "pillar") < pos(&got, "top"), "the block on top draws after the pillar: {got:?}");
    // The tile in front of the pillar is in front of it by column but under it by level: their silhouettes
    // can't overlap, so nothing orders them.
    assert!(!(behind(boxed(ground_front), boxed(pillar)) || behind(boxed(pillar), boxed(ground_front))));
}

#[test]
fn r1_items_whose_pixels_cannot_meet_are_not_ordered_by_their_boxes_they_keep_the_order_they_are_listed_in() {
    // b is behind a by the box test (column), but they draw in different places.
    let a = node_b(block_box(5.0, 0.0, 0.0, [1.0, 1.0, 1.0]), bounds(0, 0, 10, 10));
    let b = node_b(block_box(0.0, 0.0, 0.0, [1.0, 1.0, 1.0]), bounds(100, 0, 110, 10));
    assert_eq!(sort_nodes(&[a.clone(), b]), [0, 1], "listed order, though b is behind a by their boxes");
    let c = node_b(block_box(0.0, 0.0, 0.0, [1.0, 1.0, 1.0]), None);
    let d = Node { boxed: None, bounds: None, depth: None, moving: false };
    assert_eq!(sort_nodes(&[a, c, d]), [0, 1, 2], "nodes with no box or no pixels are never ordered against anything");
}

#[test]
fn r5_d2_back_items_draw_before_the_others_and_front_items_after_each_band_sorted_by_position() {
    let near = block_box(5.0, 5.0, 0.0, [1.0, 1.0, 1.0]);
    let far = block_box(0.0, 0.0, 0.0, [1.0, 1.0, 1.0]);
    let mid = block_box(3.0, 3.0, 0.0, [1.0, 1.0, 1.0]);
    let labels = ["near-back", "far-front", "mid", "far-back", "near-front", "far"];
    let list =
        [node_d(near, Mark::Back), node_d(far, Mark::Front), node(mid), node_d(far, Mark::Back), node_d(near, Mark::Front), node(far)];
    assert_eq!(names(&list, &labels), ["far-back", "near-back", "far", "mid", "far-front", "near-front"]);
}

#[test]
fn r2_a_cycle_of_three_boxes_that_each_cover_the_next_is_broken_by_the_smallest_key_deterministically() {
    // a is behind b by column, b behind c by row, and c behind a by level; no other axis settles any pair.
    let a = bx(0.0, 2.0, 0.0, 6.0, 4.0, 6.0);
    let b = bx(2.0, 6.0, 0.0, 2.0, 0.0, 8.0);
    let c = bx(0.0, 4.0, 2.0, 6.0, 0.0, 3.0);
    assert!(behind(boxed(a), boxed(b)) && behind(boxed(b), boxed(c)) && behind(boxed(c), boxed(a)), "a real cycle");
    assert!(key_of(boxed(c)) < key_of(boxed(a)) && key_of(boxed(c)) < key_of(boxed(b)));
    for list in [["a", "b", "c"], ["c", "a", "b"], ["b", "c", "a"]] {
        let by = |n: &str| match n {
            "a" => a,
            "b" => b,
            _ => c,
        };
        let nodes: Vec<Node> = list.iter().map(|n| node(by(n))).collect();
        assert_eq!(names(&nodes, &list), ["c", "a", "b"], "listed as {list:?}");
    }
}

/// `r() < 0.7 ? EVERYWHERE : { minX: i * 3, minY: 0, maxX: i * 3 + 2 + floor(r() * 8), maxY: 5 }` for node `i`.
fn bounds_for(r: &mut Rng, i: usize, everywhere_share: f64) -> Option<Bounds> {
    if r.next() < everywhere_share {
        Some(EVERYWHERE)
    } else {
        let i = i as i32;
        bounds(i * 3, 0, i * 3 + 2 + r.floor(8.0) as i32, 5)
    }
}

#[test]
#[allow(clippy::needless_range_loop)]
fn r1_r2_the_order_is_a_valid_depth_order_every_overlapping_pair_that_is_ordered_by_its_boxes_is_respected_unless_the_pair_sits_in_a_cycle()
{
    for seed in SEEDS {
        let mut r = rng(seed);
        for round in 0..10 {
            let n = 10 + r.floor(25.0) as usize;
            let boxes = random_boxes(&mut r, n, 6.0);
            let nodes: Vec<Node> = boxes
                .into_iter()
                .enumerate()
                .map(|(i, b)| Node { boxed: Some(b), bounds: bounds_for(&mut r, i, 0.7), depth: None, moving: false })
                .collect();
            let order = sort_nodes(&nodes);
            assert_eq!(sort_nodes(&nodes), order, "the same input gives the same order");
            let mut position = HashMap::new();
            for (at, &index) in order.iter().enumerate() {
                position.insert(index, at);
            }
            assert_eq!(position.len(), n);
            let edge = |i: usize, j: usize| i != j && overlaps(nodes[i].bounds, nodes[j].bounds) && behind(nodes[i].boxed, nodes[j].boxed);
            // reach[i][j]: following "is behind" edges leads from i to j.
            let mut reach: Vec<Vec<bool>> = (0..n).map(|i| (0..n).map(|j| edge(i, j)).collect()).collect();
            for k in 0..n {
                for i in 0..n {
                    if reach[i][k] {
                        for j in 0..n {
                            if reach[k][j] {
                                reach[i][j] = true;
                            }
                        }
                    }
                }
            }
            for i in 0..n {
                for j in 0..n {
                    if edge(i, j) && position[&j] < position[&i] {
                        assert!(reach[j][i], "seed {seed}, round {round}: {i} is behind {j} but draws after it, outside any cycle");
                    }
                }
            }
        }
    }
}

#[test]
fn r2_sorting_a_sorted_list_changes_nothing_on_random_scenes_with_ties_cycles_and_bounds_that_miss() {
    for seed in SEEDS {
        let mut r = rng(seed);
        for round in 0..20 {
            let n = 5 + r.floor(60.0) as usize;
            let boxes = random_boxes(&mut r, n, 4.0);
            let nodes: Vec<Node> = boxes
                .into_iter()
                .enumerate()
                .map(|(i, b)| {
                    let bounds = bounds_for(&mut r, i, 0.7);
                    let depth = if r.next() < 0.15 { Some(if r.next() < 0.5 { Mark::Front } else { Mark::Back }) } else { None };
                    Node { boxed: Some(b), bounds, depth, moving: false }
                })
                .collect();
            let once: Vec<Node> = sort_nodes(&nodes).into_iter().map(|i| nodes[i].clone()).collect();
            let twice: Vec<Node> = sort_nodes(&once).into_iter().map(|i| once[i].clone()).collect();
            assert_eq!(twice, once, "seed {seed}, round {round}");
            assert_eq!(sort_nodes(&once), (0..once.len()).collect::<Vec<_>>(), "seed {seed}, round {round}: the identity order");
        }
    }
}

#[test]
fn r2_the_same_scene_gives_the_same_order_on_every_run() {
    let mut r = rng(99);
    let nodes: Vec<Node> = random_boxes(&mut r, 80, 5.0).into_iter().map(node).collect();
    let first = format!("{:?}", sort_nodes(&nodes));
    for _ in 0..3 {
        assert_eq!(format!("{:?}", sort_nodes(&nodes)), first);
    }
}

#[test]
fn r2_a_list_of_400_items_sorts_in_well_under_a_second() {
    let mut r = rng(2026);
    let nodes: Vec<Node> = (0..400)
        .map(|_| {
            let x = r.floor(600.0) as i32 - 300;
            let y = r.floor(400.0) as i32 - 200;
            let c = r.floor(20.0) as f64;
            let rw = r.floor(20.0) as f64;
            let l = r.floor(3.0) as f64;
            let size = [1.0 + r.floor(2.0) as f64, 1.0, 1.0 + r.floor(2.0) as f64];
            node_b(block_box(c, rw, l, size), bounds(x, y, x + 40, y + 40))
        })
        .collect();
    let start = std::time::Instant::now();
    let order = sort_nodes(&nodes);
    let took = start.elapsed().as_millis();
    let mut distinct = order.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), 400);
    assert!(took < 1000, "took {took} ms");
}

// ----------------------------------------- the scene format: depth and level

fn dot() -> Value {
    sprite(&["g"])
}

fn problems(items: Value) -> Vec<String> {
    validate(&src(&[("dot", dot())], &[("demo", scene(items))]))
}

fn strs(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn r5_an_item_or_a_group_may_carry_depth_front_or_back_and_nothing_else() {
    let none: Vec<String> = vec![];
    assert_eq!(
        problems(json!([
            { "object": "dot", "depth": "front", "at": { "px": [0, 0] } },
            { "group": {}, "depth": "back", "items": [{ "object": "dot", "depth": "back", "at": { "tile": [0, 0, 0] } }] }
        ])),
        none
    );
    let depth_rule = strs(&["scenes/demo.json: items[0]: depth must be 'front' or 'back'"]);
    assert_eq!(problems(json!([{ "object": "dot", "depth": "above", "at": { "px": [0, 0] } }])), depth_rule);
    assert_eq!(
        problems(json!([{ "object": "dot", "depth": "auto", "at": { "px": [0, 0] } }])),
        depth_rule,
        "auto is the absence of the key"
    );
    assert_eq!(problems(json!([{ "group": {}, "depth": 1, "items": [] }])), depth_rule);
    assert_eq!(
        problems(json!([{ "group": {}, "order": 1, "items": [] }])),
        strs(&["scenes/demo.json: items[0]: unknown property 'order'"])
    );
    assert_eq!(
        problems(json!([{ "object": "dot", "zIndex": 1, "at": { "px": [0, 0] } }])),
        strs(&["scenes/demo.json: items[0]: unknown property 'zIndex'"])
    );
}

#[test]
fn r7_at_px_takes_a_whole_number_level_and_only_at_px_does() {
    let none: Vec<String> = vec![];
    assert_eq!(
        problems(json!([
            { "object": "dot", "at": { "px": [40, -12], "level": 4 } },
            { "object": "dot", "at": { "px": [0, 0], "level": -2 } },
            { "object": "dot", "at": { "px": [0, 0], "level": 0 } }
        ])),
        none
    );
    let whole = strs(&["scenes/demo.json: items[0]: at.level must be a whole number"]);
    assert_eq!(problems(json!([{ "object": "dot", "at": { "px": [0, 0], "level": 1.5 } }])), whole);
    assert_eq!(problems(json!([{ "object": "dot", "at": { "px": [0, 0], "level": "2" } }])), whole);
    assert_eq!(
        problems(json!([{ "object": "dot", "at": { "tile": [0, 0, 0], "level": 1 } }])),
        strs(&["scenes/demo.json: items[0]: at.level is only for at.px; a tile's level is its third number"])
    );
    let shape = strs(&["scenes/demo.json: items[0]: at must be { px: [x, y], level? } or { tile: [col, row, level] }"]);
    assert_eq!(problems(json!([{ "object": "dot", "at": { "level": 1 } }])), shape);
    assert_eq!(problems(json!([{ "object": "dot", "at": { "px": [0, 0], "tile": [0, 0, 0] } }])), shape);
    assert_eq!(problems(json!([{ "object": "dot", "at": { "px": [0, 0], "height": 2 } }])), shape);
}

#[test]
fn r5_r7_the_serializer_writes_depth_after_layer_and_level_after_px_and_leaves_level_out_at_0() {
    // Adapted: format 2's text, as Node's `canonical` writes it.
    let s = json!({
        "viewBox": [0, 0, 8, 8],
        "items": [
            { "object": "dot", "id": "a", "class": "x", "layer": "yard", "depth": "front", "at": { "px": [40, -12], "level": 4 } },
            { "object": "dot", "depth": "back", "at": { "tile": [1, 2, 3] } },
            { "object": "dot", "at": { "px": [1, 2], "level": 0 } },
            { "object": "dot", "at": { "px": [1, 2], "level": -1 } },
            { "group": { "class": "enter" }, "depth": "back", "items": [{ "object": "dot", "at": { "px": [0, 0] } }] },
            { "group": {}, "items": [] },
        ],
    });
    assert_eq!(
        serialize(&s),
        r#"{
  "viewBox": [0, 0, 8, 8],
  "items": [
    { "object": "dot", "id": "a", "class": "x", "layer": "yard", "depth": "front", "at": { "px": [40, -12], "level": 4 } },
    { "object": "dot", "depth": "back", "at": { "tile": [1, 2, 3] } },
    { "object": "dot", "at": { "px": [1, 2] } },
    { "object": "dot", "at": { "px": [1, 2], "level": -1 } },
    {
      "group": { "class": "enter" },
      "depth": "back",
      "items": [
        { "object": "dot", "at": { "px": [0, 0] } }
      ]
    },
    {
      "group": {},
      "items": []
    }
  ]
}
"#
    );
}

#[test]
fn r5_r7_every_new_key_survives_save_and_reload_byte_for_byte_and_files_without_them_are_unchanged() {
    let full = json!({
        "viewBox": [0, 0, 8, 8],
        "items": [
            { "object": "dot", "id": "jib", "layer": "crane", "depth": "front", "at": { "px": [40, -12], "level": 4 } },
            { "group": { "data-class": "1" }, "depth": "back", "items": [{ "object": "dot", "depth": "front", "at": { "tile": [0, 0, 0] } }] },
            { "object": "dot", "at": { "px": [3, 4], "level": -2 } },
        ],
    });
    let text = serialize(&full);
    assert_eq!(parse(&text).unwrap(), full);
    assert_eq!(serialize(&parse(&text).unwrap()), text);
    // A file with none of the new keys writes the same text it always did.
    let plain = scene(json!([
        { "object": "dot", "at": { "px": [1, 2] } },
        { "group": { "class": "a" }, "items": [{ "object": "dot", "class": "b c", "at": { "tile": [0, 1, 2] } }] },
    ]));
    assert_eq!(
        serialize(&plain),
        r#"{
  "viewBox": [0, 0, 8, 8],
  "items": [
    { "object": "dot", "at": { "px": [1, 2] } },
    {
      "group": { "class": "a" },
      "items": [
        { "object": "dot", "class": "b c", "at": { "tile": [0, 1, 2] } }
      ]
    }
  ]
}
"#
    );
}

// ------------------------------------------------- sprite footprints (R8)

fn problems_of(objects: &[(&str, Value)]) -> Vec<String> {
    validate(&src(objects, &[]))
}

#[test]
fn r8_a_sprite_may_say_how_many_tiles_it_covers_whole_numbers_from_1_to_16() {
    let none: Vec<String> = vec![];
    assert_eq!(
        problems_of(&[
            ("house", sprite_x(&["g"], json!({ "footprint": [3, 2] }))),
            ("tall", sprite_x(&["g"], json!({ "footprint": [16, 1] })))
        ]),
        none
    );
    let rule = "footprint must be two whole numbers, tiles wide and deep, from 1 to 16";
    let bad =
        [json!([0, 1]), json!([1, 17]), json!([1.5, 1]), json!([1]), json!([1, 1, 1]), json!("2,2"), json!({ "w": 1 }), json!([-1, 2])];
    for bad in bad {
        assert_eq!(
            problems_of(&[("house", sprite_x(&["g"], json!({ "footprint": bad })))]),
            vec![format!("objects/house.json: {rule}")],
            "{bad}"
        );
    }
}

#[test]
fn r8_a_block_refuses_a_footprint_since_its_size_is_its_footprint() {
    assert_eq!(
        problems_of(&[("b", block_x(json!({ "footprint": [1, 1] })))]),
        strs(&["objects/b.json: footprint is for sprites; a block's size is its footprint"])
    );
    assert_eq!(problems_of(&[("b", block())]), Vec::<String>::new());
}

#[test]
fn r8_the_footprint_defaults_to_1_1_a_blocks_is_its_size_and_extends_inherits_it() {
    let sources = src(
        &[
            ("plain", sprite(&["g"])),
            ("house", sprite_x(&["g"], json!({ "footprint": [3, 2] }))),
            ("cottage", json!({ "kind": "sprite", "extends": "house", "rows": {} })),
            ("shed", json!({ "kind": "sprite", "extends": "house", "footprint": [1, 2], "rows": {} })),
            ("wall", block_x(json!({ "size": [2, 1, 1] }))),
        ],
        &[],
    );
    assert_eq!(validate(&sources), Vec::<String>::new());
    let fp = |name: &str| resolve(&sources, name).unwrap().footprint;
    assert_eq!(fp("plain"), [1, 1]);
    assert_eq!(fp("house"), [3, 2]);
    assert_eq!(fp("cottage"), [3, 2], "inherited");
    assert_eq!(fp("shed"), [1, 2], "its own wins");
    assert_eq!(fp("wall"), [2, 1]);
    // resolve hands out copies: changing one leaves the object alone.
    let mut copy = resolve(&sources, "house").unwrap();
    copy.footprint[0] = 9;
    assert_eq!(fp("house"), [3, 2]);
}

#[test]
fn r8_the_serializer_writes_footprint_after_anchor_and_files_without_it_are_unchanged() {
    let obj = json!({ "kind": "sprite", "extends": "house", "anchor": [5, 13], "footprint": [3, 2], "keys": { "x": "ink" }, "rows": {} });
    assert_eq!(
        serialize(&obj),
        r#"{
  "kind": "sprite",
  "extends": "house",
  "anchor": [5, 13],
  "footprint": [3, 2],
  "keys": {
    "x": "ink"
  },
  "rows": {}
}
"#
    );
    assert_eq!(parse(&serialize(&obj)).unwrap(), obj);
    let plain = json!({ "kind": "sprite", "anchor": [1, 1], "keys": { "g": "ink" }, "layers": [{ "map": ["g"] }] });
    assert!(!serialize(&plain).contains("footprint"));
    assert_eq!(serialize(&parse(&serialize(&plain)).unwrap()), serialize(&plain));
}

// ------------------------------- the engine draws in depth order (R3, R5, R9)

fn tree() -> Value {
    json!({ "kind": "sprite", "anchor": [2, 11], "keys": { "g": "grass-4" }, "layers": [{ "map": vec!["gggg"; 12] }] })
}

fn house() -> Value {
    block_x(json!({ "faces": { "top": "cream", "left": "wood-3", "right": "ink" } }))
}

fn objects() -> Vec<(&'static str, Value)> {
    vec![("tree", tree()), ("house", house())]
}

fn s_scene(items: Vec<Value>) -> Value {
    scene_x(json!(items), json!({ "viewBox": [-40, -30, 80, 70] }))
}

fn tree_behind() -> Value {
    json!({ "object": "tree", "at": { "tile": [0, 1, 0] } })
}

fn tree_in_front() -> Value {
    json!({ "object": "tree", "at": { "tile": [2, 1, 0] } })
}

fn red_house() -> Value {
    json!({ "object": "house", "at": { "tile": [1, 1, 1] } })
}

fn with_depth(item: &Value, depth: &str) -> Value {
    with(item.clone(), json!({ "depth": depth }))
}

fn point_of(item: &Value) -> [f64; 2] {
    match item["at"].get("tile") {
        Some(t) => tile_to_px([t[0].as_f64().unwrap(), t[1].as_f64().unwrap(), t[2].as_f64().unwrap()], [0.0, 0.0]),
        None => [item["at"]["px"][0].as_f64().unwrap(), item["at"]["px"][1].as_f64().unwrap()],
    }
}

struct Overlap {
    p: (i32, i32),
    a: String,
    b: String,
}

/// A pixel both items paint when they stand where the items say, with each one's color.
fn overlap(sources: &Sources, a: &Value, b: &Value) -> Overlap {
    let pa = painted_at(sources, a["object"].as_str().unwrap(), point_of(a));
    let pb = painted_at(sources, b["object"].as_str().unwrap(), point_of(b));
    let shared: Vec<_> = pa.order.iter().filter(|p| pb.map.contains_key(p)).collect();
    assert!(!shared.is_empty(), "the fixtures overlap on screen");
    let p = *shared[0];
    Overlap { p, a: pa.map[&p].clone(), b: pb.map[&p].clone() }
}

fn color_at(sources: &Sources, name: &str, p: (i32, i32), order: Order) -> Option<String> {
    composite(&render_scene(sources, name, order).unwrap().root, CompositeOptions::default()).get(&p).map(|c| c.to_string())
}

fn depth_of(sources: &Sources, name: &str) -> Vec<usize> {
    let scene = &sources.scenes[name];
    depth_order(sources, &items(sources, name), origin_of(scene), &OMap::new()).unwrap()
}

fn tiles(list: &[Value]) -> Vec<String> {
    list.iter().map(|i| format!("{}{}", i["object"].as_str().unwrap(), tile_str(i))).collect()
}

#[test]
fn r3_a_file_listed_out_of_order_draws_in_depth_order_the_tree_behind_the_house_is_under_it_the_tree_in_front_is_over_it() {
    for (items, label) in
        [(vec![tree_behind(), red_house()], "behind, listed first"), (vec![red_house(), tree_behind()], "behind, listed last")]
    {
        let sources = src(&objects(), &[("s", s_scene(items))]);
        let o = overlap(&sources, &tree_behind(), &red_house());
        assert_eq!(color_at(&sources, "s", o.p, Order::Depth).as_deref(), Some(o.b.as_str()), "{label}");
    }
    for items in [vec![tree_in_front(), red_house()], vec![red_house(), tree_in_front()]] {
        let sources = src(&objects(), &[("s", s_scene(items))]);
        let o = overlap(&sources, &tree_in_front(), &red_house());
        assert_eq!(color_at(&sources, "s", o.p, Order::Depth).as_deref(), Some(o.a.as_str()), "in front");
    }
}

#[test]
fn r8_a_wide_sprite_with_a_footprint_sorts_by_the_ground_it_covers_in_the_boxes_and_in_the_pixels() {
    // A 56×16 slab that reaches right from its tile, drawn in grass-2, beside a 4×12 tree in grass-4 two tiles right and a row back.
    let rows: Vec<String> = vec!["g".repeat(56); 16];
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let slab =
        |extra: Value| with(json!({ "kind": "sprite", "anchor": [4, 15], "keys": { "g": "grass-2" }, "layers": [{ "map": rows }] }), extra);
    let items = vec![json!({ "object": "slab", "at": { "tile": [0, 2, 0] } }), json!({ "object": "tree", "at": { "tile": [2, 1, 0] } })];
    let sources_of = |extra: Value| {
        src(&[("slab", slab(extra)), ("tree", tree())], &[("s", scene_x(json!(items), json!({ "viewBox": [-60, -30, 120, 70] })))])
    };
    let (plain, wide) = (sources_of(json!({})), sources_of(json!({ "footprint": [3, 1] })));
    // The boxes: on one tile the two cannot be told apart, and over three columns the tree stands behind the slab.
    let standing = sprite_box(2.0, 1.0, 0.0, [1.0, 1.0], 12.0 / 16.0);
    assert!(!behind(boxed(standing), boxed(sprite_box(0.0, 2.0, 0.0, [1.0, 1.0], 16.0 / 16.0))));
    assert!(!behind(boxed(sprite_box(0.0, 2.0, 0.0, [1.0, 1.0], 16.0 / 16.0)), boxed(standing)), "no order on one tile");
    assert!(behind(boxed(standing), boxed(sprite_box(0.0, 2.0, 0.0, [3.0, 1.0], 16.0 / 16.0))), "the tree is behind the three-tile slab");
    // The engine: the same scene lists the slab first. With no footprint it stays first; with one the tree moves under it.
    assert_eq!(depth_of(&plain, "s"), [0, 1]);
    assert_eq!(depth_of(&wide, "s"), [1, 0]);
    // The pixels: where both paint, the tree is over the slab without a footprint and under it with one.
    let slab_at = painted_at(&plain, "slab", tile_to_px([0.0, 2.0, 0.0], [0.0, 0.0]));
    let tree_at = painted_at(&plain, "tree", tile_to_px([2.0, 1.0, 0.0], [0.0, 0.0]));
    let both: Vec<_> = slab_at.order.iter().filter(|p| tree_at.map.contains_key(p)).collect();
    assert!(!both.is_empty(), "the fixtures overlap on screen");
    assert_eq!(color_at(&plain, "s", *both[0], Order::Depth).as_deref(), Some("grass-4"), "tree over slab");
    assert_eq!(color_at(&wide, "s", *both[0], Order::Depth).as_deref(), Some("grass-2"), "slab over tree");
}

#[test]
fn r3_order_list_draws_the_items_as_the_file_lists_them_as_engines_before_the_depth_order_did() {
    let sources = src(&objects(), &[("s", s_scene(vec![red_house(), tree_behind()]))]);
    let o = overlap(&sources, &tree_behind(), &red_house());
    assert_ne!(o.a, o.b);
    assert_eq!(color_at(&sources, "s", o.p, Order::List).as_deref(), Some(o.a.as_str()), "listed last, the tree paints over the house");
    assert_eq!(color_at(&sources, "s", o.p, Order::default()).as_deref(), Some(o.b.as_str()), "depth order is the default");
    assert_eq!(color_at(&sources, "s", o.p, Order::Depth).as_deref(), Some(o.b.as_str()));
}

#[test]
fn r3_the_draw_order_is_the_same_tree_whichever_way_the_file_lists_a_sorted_scene() {
    let sources = src(&objects(), &[("s", s_scene(vec![tree_behind(), red_house(), tree_in_front()]))]);
    let sorted = scene_in_order(&sources, &sources.scenes["s"]).unwrap();
    let both = src(&objects(), &[("s", sources.scenes["s"].clone()), ("t", sorted)]);
    let text = |name: &str, order: Order| tree_text(&render_scene(&both, name, order).unwrap().root);
    assert_eq!(text("s", Order::Depth), text("t", Order::List));
    assert_eq!(text("s", Order::Depth), text("t", Order::Depth));
}

#[test]
fn r5_a_front_mark_draws_an_item_over_what_stands_in_front_of_it_and_a_back_mark_under_what_stands_behind() {
    let mut sources = src(&objects(), &[("s", s_scene(vec![tree_in_front(), with_depth(&red_house(), "front")]))]);
    let mut o = overlap(&sources, &tree_in_front(), &red_house());
    assert_eq!(
        color_at(&sources, "s", o.p, Order::Depth).as_deref(),
        Some(o.b.as_str()),
        "the house is marked front, so it paints over the tree in front of it"
    );
    sources = src(&objects(), &[("s", s_scene(vec![with_depth(&tree_behind(), "front"), red_house()]))]);
    o = overlap(&sources, &tree_behind(), &red_house());
    assert_eq!(
        color_at(&sources, "s", o.p, Order::Depth).as_deref(),
        Some(o.a.as_str()),
        "the tree behind it is marked front, so it paints over the house"
    );
    sources = src(&objects(), &[("s", s_scene(vec![with_depth(&red_house(), "back"), tree_behind(), tree_in_front()]))]);
    assert_eq!(depth_of(&sources, "s"), [0, 1, 2], "a back-marked house draws first");
    sources = src(&objects(), &[("s", s_scene(vec![tree_behind(), tree_in_front(), with_depth(&red_house(), "back")]))]);
    assert_eq!(depth_of(&sources, "s"), [2, 0, 1]);
}

#[test]
fn r3_depth_order_scene_in_order_and_is_in_depth_order_agree_and_scene_in_order_returns_a_copy() {
    let sources = src(&objects(), &[("s", scene(json!([red_house(), tree_in_front(), tree_behind()])))]);
    let original = sources.scenes["s"].clone();
    assert_eq!(depth_of(&sources, "s"), [2, 0, 1]);
    assert!(!is_in_depth_order(&sources, &original).unwrap());
    let mut sorted = scene_in_order(&sources, &original).unwrap();
    assert_eq!(tiles(sorted["items"].as_array().unwrap()), ["tree0,1,0", "house1,1,1", "tree2,1,0"]);
    assert!(is_in_depth_order(&sources, &sorted).unwrap());
    let originals: Vec<&str> = original["items"].as_array().unwrap().iter().map(|i| i["object"].as_str().unwrap()).collect();
    assert_eq!(originals, ["house", "tree", "tree"], "the original is untouched");
    assert_eq!(scene_in_order(&sources, &sorted).unwrap(), sorted, "sorting a sorted scene returns it");
    sorted["items"][0]["at"]["tile"][0] = json!(99);
    assert_eq!(original["items"][2]["at"]["tile"], json!([0, 1, 0]), "and shares nothing with the copy");
}

#[test]
fn r3_a_tracked_item_rests_where_its_first_key_puts_it() {
    let items = json!([{ "object": "tree", "id": "walker", "at": { "tile": [0, 1, 0] } }, red_house()]);
    let motion = |mv: [i64; 2]| json!({ "duration": 1000, "tracks": [{ "item": "walker", "keys": [{ "t": 0, "move": mv }, { "t": 500, "move": [0, 0] }] }] });
    let sources = src(
        &objects(),
        &[("a", scene_x(items.clone(), json!({ "motion": motion([0, 0]) }))), ("b", scene_x(items, json!({ "motion": motion([32, 16]) })))],
    );
    let order_of = |name: &str| {
        let scene = &sources.scenes[name];
        depth_order(&sources, &items_of(scene), [0.0, 0.0], &tracks_of(scene)).unwrap()
    };
    assert_eq!(order_of("a"), [0, 1], "at rest behind the house");
    assert_eq!(order_of("b"), [1, 0], "its first key moves it in front");
    assert!(is_in_depth_order(&sources, &sources.scenes["a"]).unwrap());
    assert!(!is_in_depth_order(&sources, &sources.scenes["b"]).unwrap());
}

fn items_of(scene: &Value) -> Vec<Value> {
    scene["items"].as_array().unwrap().clone()
}

fn wrap(items: Vec<Value>) -> Value {
    wrap_x(items, json!({}))
}

fn wrap_x(items: Vec<Value>, extra: Value) -> Value {
    with(json!({ "group": { "class": "enter" }, "items": items }), extra)
}

#[test]
fn r9_a_group_sorts_as_one_object_by_the_union_of_its_members_and_its_members_sort_inside_it() {
    // The house and the tree in front are one group; the tree behind them is listed after the group.
    let sources = src(&objects(), &[("s", s_scene(vec![wrap(vec![tree_in_front(), red_house()]), tree_behind()]))]);
    assert_eq!(depth_of(&sources, "s"), [1, 0], "the group draws after the tree behind it");
    let sorted = scene_in_order(&sources, &sources.scenes["s"]).unwrap();
    assert_eq!(tiles(sorted["items"][1]["items"].as_array().unwrap()), ["house1,1,1", "tree2,1,0"], "the members were sorted inside it");
    assert_eq!(sorted["items"][1]["group"], json!({ "class": "enter" }), "the wrapper is the same");
    assert!(is_in_depth_order(&sources, &sorted).unwrap());
    assert!(!is_in_depth_order(&sources, &sources.scenes["s"]).unwrap());
    // Each member is still drawn inside its group's <g>.
    let root = render_scene(&sources, "s", Order::Depth).unwrap().root;
    assert_eq!(root.children.len(), 2);
    assert!(matches!(root.children[0], engine::engine::Child::Layer(_)), "the tree behind paints first, at the top level");
    assert_eq!(
        as_group(&root.children[1]).unwrap().attrs,
        [("class".to_string(), "enter".to_string())],
        "then the group, with the house and the tree in front inside it"
    );
    let o = overlap(&sources, &tree_in_front(), &red_house());
    assert_eq!(
        color_at(&sources, "s", o.p, Order::Depth).as_deref(),
        Some(o.a.as_str()),
        "inside the group the tree in front paints over the house"
    );
}

#[test]
fn r9_r5_a_group_carries_its_own_depth_mark_and_the_marks_of_its_members_act_inside_it_only() {
    let sources = src(
        &objects(),
        &[(
            "s",
            scene(json!([
                wrap_x(vec![red_house()], json!({ "depth": "back" })),
                tree_behind(),
                wrap(vec![with_depth(&tree_in_front(), "back"), red_house()]),
            ])),
        )],
    );
    // The back-marked group goes first although the tree is behind it; the member marked back stays inside its own group.
    assert_eq!(depth_of(&sources, "s"), [0, 1, 2]);
    let sorted = scene_in_order(&sources, &sources.scenes["s"]).unwrap();
    let objs: Vec<&str> = sorted["items"][2]["items"].as_array().unwrap().iter().map(|i| i["object"].as_str().unwrap()).collect();
    assert_eq!(objs, ["tree", "house"], "a back mark inside a group puts the member first in the group");
}

fn variant(n: i32, item: Value) -> Value {
    json!({ "group": { "data-class": n.to_string() }, "items": [item] })
}

/// `i.group?.['data-class'] ?? i.object`
fn variant_labels(scene: &Value) -> Vec<String> {
    scene["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| match i.get("group").and_then(|g| g.get("data-class")) {
            Some(d) => d.as_str().unwrap().to_string(),
            None => i["object"].as_str().unwrap().to_string(),
        })
        .collect()
}

#[test]
fn r9_a_run_of_consecutive_variant_groups_sorts_as_one_unit_and_keeps_its_own_order() {
    let near = json!({ "object": "house", "at": { "tile": [3, 3, 1] } });
    let far = json!({ "object": "house", "at": { "tile": [1, 1, 1] } });
    // The second variant is behind the first, yet they stay together and in this order; the tree behind both goes first.
    let sources = src(
        &objects(),
        &[(
            "s",
            scene_x(json!([variant(0, near.clone()), variant(1, far.clone()), tree_behind()]), json!({ "viewBox": [-60, -50, 120, 100] })),
        )],
    );
    assert_eq!(depth_of(&sources, "s"), [2, 0, 1]);
    let sorted = scene_in_order(&sources, &sources.scenes["s"]).unwrap();
    assert_eq!(variant_labels(&sorted), ["tree", "0", "1"]);
    // composite's firstVariant still shows only the run's first.
    let root = render_scene(&sources, "s", Order::Depth).unwrap().root;
    let first = composite(&root, CompositeOptions { first_variant: true, ..Default::default() });
    let all = composite(&root, CompositeOptions::default());
    let only = |item: &Value| painted_at(&sources, item["object"].as_str().unwrap(), point_of(item));
    let (p_near, p_far) = (only(&near), only(&far));
    let far_only = p_far.order.iter().find(|p| !p_near.map.contains_key(p)).unwrap();
    assert!(all.has(far_only), "the second variant draws when all show");
    assert!(!first.has(far_only), "and not when only the first of the run shows");
    assert!(first.has(&p_near.order[0]));
}

#[test]
fn r9_r2_variant_groups_that_sorting_brings_side_by_side_form_a_run_and_then_stay_put() {
    // The tree behind the first variant is listed between the two, so it goes first and they end up together.
    let sources = src(
        &objects(),
        &[(
            "s",
            scene(json!([
                variant(0, json!({ "object": "house", "at": { "tile": [3, 3, 1] } })),
                { "object": "tree", "at": { "tile": [2, 3, 0] } },
                variant(1, json!({ "object": "house", "at": { "tile": [4, 3, 1] } })),
            ])),
        )],
    );
    let once = scene_in_order(&sources, &sources.scenes["s"]).unwrap();
    assert_eq!(variant_labels(&once), ["tree", "0", "1"]);
    assert!(is_in_depth_order(&sources, &once).unwrap());
    assert_eq!(scene_in_order(&sources, &once).unwrap(), once);
}

fn random_leaf(r: &mut Rng) -> Value {
    let object = *r.pick(&["tree", "house"]);
    let depth = if r.next() < 0.15 { Some(*r.pick(&["front", "back"])) } else { None };
    let at = if r.next() < 0.5 {
        json!({ "tile": [r.floor(6.0), r.floor(6.0), r.floor(3.0)] })
    } else {
        let mut at = json!({ "px": [r.floor(100.0) - 50, r.floor(60.0) - 20] });
        if r.next() < 0.5 {
            at["level"] = json!(r.floor(3.0));
        }
        at
    };
    let mut item = json!({ "object": object });
    if let Some(d) = depth {
        item["depth"] = json!(d);
    }
    item["at"] = at;
    item
}

fn random_list(r: &mut Rng, n: usize, depth_left: u32) -> Vec<Value> {
    (0..n)
        .map(|_| {
            if depth_left > 0 && r.next() < 0.3 {
                let group = if r.next() < 0.5 { json!({ "data-class": r.floor(3.0).to_string() }) } else { json!({ "class": "enter" }) };
                let mut g = json!({ "group": group });
                if r.next() < 0.15 {
                    g["depth"] = json!(*r.pick(&["front", "back"]));
                }
                let count = 1 + r.floor(3.0) as usize;
                g["items"] = json!(random_list(r, count, depth_left - 1));
                g
            } else {
                random_leaf(r)
            }
        })
        .collect()
}

#[test]
fn r2_on_random_scenes_with_groups_and_variants_sorting_twice_is_sorting_once_and_the_render_matches_the_sorted_copy() {
    for seed in SEEDS {
        let mut r = rng(seed);
        for round in 0..12 {
            let n = 4 + r.floor(25.0) as usize;
            let original = scene_x(json!(random_list(&mut r, n, 2)), json!({ "viewBox": [-80, -60, 160, 140] }));
            let sources = src(&objects(), &[("s", original.clone())]);
            let once = scene_in_order(&sources, &original).unwrap();
            let again = scene_in_order(&sources, &once).unwrap();
            let check = src(&objects(), &[("s", original), ("once", once.clone()), ("again", again)]);
            assert_eq!(check.scenes["again"], once, "seed {seed}, round {round}: sorted twice is sorted once");
            assert!(is_in_depth_order(&check, &once).unwrap(), "seed {seed}, round {round}");
            let sorted_pixels = |name: &str, order: Order| {
                sorted(&composite(
                    &render_scene(&check, name, order).unwrap().root,
                    CompositeOptions { first_variant: false, ..Default::default() },
                ))
            };
            assert_eq!(
                sorted_pixels("s", Order::Depth),
                sorted_pixels("once", Order::List),
                "seed {seed}, round {round}: depth order draws what the sorted copy draws"
            );
        }
    }
}

// ------------------- the compiler and the preview draw in depth order (R3)

/// The pixel's color in a compiled SVG: the last path run that covers it.
fn painted_in_svg(svg: &str, x: i32, y: i32) -> Option<String> {
    let mut painted = None;
    let mut rest = svg;
    while let Some(at) = rest.find("<path fill=\"") {
        let body = &rest[at + 12..];
        let (fill, tail) = body.split_at(7);
        let d_start = tail.find("d=\"").unwrap() + 3;
        let d_end = tail[d_start..].find('"').unwrap() + d_start;
        let d = &tail[d_start..d_end];
        for run in d.split('M').skip(1) {
            // "-3 -2h4v1h-4z"
            let (xy, after) = run.split_once('h').unwrap();
            let (rx, ry) = xy.split_once(' ').unwrap();
            let (w, after) = after.split_once('v').unwrap();
            let (h, _) = after.split_once('h').unwrap();
            let (rx, ry, w, h): (i32, i32, i32, i32) = (rx.parse().unwrap(), ry.parse().unwrap(), w.parse().unwrap(), h.parse().unwrap());
            if x >= rx && x < rx + w && y >= ry && y < ry + h {
                painted = Some(fill.to_string());
            }
        }
        rest = &tail[d_end..];
    }
    painted
}

#[test]
fn r3_the_compiler_and_the_preview_both_draw_the_tree_under_the_house_whatever_order_the_file_lists_them() {
    // The document is written to a folder in format 2 and read back, as `readSources` does.
    let dir = Temp::new();
    let write = |file: &str, doc: &Value| {
        let path = dir.0.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serialize_file(file, doc)).unwrap();
    };
    write("palette.json", &palette());
    write("objects/tree.json", &tree());
    write("objects/house.json", &house());
    write(
        "scenes/depth.json",
        &scene_x(json!([red_house(), tree_behind()]), json!({ "output": "depth.svg", "viewBox": [-40, -30, 80, 70] })),
    );
    let (sources, load_errors) = read_sources(&dir.0);
    assert_eq!(load_errors, Vec::<String>::new());
    let o = overlap(&sources, &tree_behind(), &red_house());
    let hex = |name: &str| sources.colors.get(name).unwrap().hex.clone();
    assert_ne!(o.a, o.b);

    let compiled = compile_scene(&sources, "depth", &Budgets::new(), Order::Depth).unwrap().output;
    assert_eq!(painted_in_svg(&compiled, o.p.0, o.p.1), Some(hex(&o.b)), "the compiled SVG paints the house last at the shared pixel");

    let (image, origin) = render_preview(&sources, Target::Scene("depth"), preview_colors(None, None).unwrap()).unwrap();
    let at = (((o.p.1 - origin[1]) as usize) * image.width as usize + (o.p.0 - origin[0]) as usize) * 4;
    let got: String = image.rgba[at..at + 3].iter().map(|v| format!("{v:02X}")).collect();
    assert_eq!(format!("#{got}"), hex(&o.b), "the preview PNG");
}

// ----------------------- the harbor example: a pine moved behind the red house

fn harbor() -> (Temp, Sources) {
    let dir = Temp::new();
    let path = example(&dir.0, "harbor");
    let (sources, load_errors) = read_sources(&path);
    assert_eq!(load_errors, Vec::<String>::new());
    (dir, sources)
}

/// The first item of a top-level group that places `object` at `tile`.
fn group_member<'a>(list: &'a [Value], object: &str, tile: Option<&str>) -> &'a Value {
    list.iter()
        .find(|item| {
            item.get("group").is_some() && item["items"][0]["object"] == object && tile.is_none_or(|t| tile_str(&item["items"][0]) == t)
        })
        .map(|item| &item["items"][0])
        .unwrap()
}

#[test]
fn r3_harbor_flats_front_pine_moved_by_minus_5_minus_5_draws_behind_the_houses_wall_where_the_old_order_drew_it_over() {
    let (_dir, sources) = harbor();
    let mut flat = sources.scenes["harbor-flat"].clone();
    let at = flat["items"]
        .as_array()
        .unwrap()
        .iter()
        .position(|i| i.get("group").is_some() && i["items"][0]["object"] == "pine" && tile_str(&i["items"][0]) == "5,6,0")
        .unwrap();
    flat["items"][at]["items"][0]["at"]["tile"] = json!([0, 1, 0]);
    let list = items_of(&flat);
    let pine = group_member(&list, "pine", Some("0,1,0")).clone();
    let walls = group_member(&list, "walls", Some("1,1,1")).clone();
    let roof = group_member(&list, "roof-red", None).clone();
    let mut moved = sources.clone();
    moved.scenes = [("moved".to_string(), flat.clone())].into_iter().collect();
    let pixels = |item: &Value| painted_at(&sources, item["object"].as_str().unwrap(), point_of(item));
    let (pine_pixels, wall_pixels, roof_pixels) = (pixels(&pine), pixels(&walls), pixels(&roof));
    // A pixel of the moved pine that the wall paints and the roof above it does not.
    let p = *pine_pixels
        .order
        .iter()
        .find(|q| wall_pixels.map.contains_key(q) && !roof_pixels.map.contains_key(q))
        .expect("the moved pine overlaps the wall on screen");
    assert_ne!(pine_pixels.map[&p], wall_pixels.map[&p]);
    assert_eq!(color_at(&moved, "moved", p, Order::Depth), Some(wall_pixels.map[&p].clone()), "depth order: the wall is over the pine");
    assert_eq!(color_at(&moved, "moved", p, Order::List), Some(pine_pixels.map[&p].clone()), "the old order drew the pine over the wall");
    let order = depth_order(&moved, &list, [0.0, 0.0], &OMap::new()).unwrap();
    let find = |object: &str, tile: &str| {
        list.iter()
            .position(|item| {
                let i = if item.get("group").is_some() { &item["items"][0] } else { item };
                i["object"] == object && tile_str(i) == tile
            })
            .unwrap()
    };
    let idx = |i: usize| order.iter().position(|&n| n == i).unwrap();
    assert!(idx(find("pine", "0,1,0")) < idx(find("walls", "1,1,1")), "the pine comes before the walls in the depth order");
    assert!(idx(find("walls", "1,1,1")) < idx(find("roof-red", "1,1,2")));
}

#[test]
fn r3_the_halls_2_1_walls_and_roof_draw_over_a_tree_one_row_behind_them_and_under_a_tree_one_row_in_front() {
    let (_dir, sources) = harbor();
    let base = sources.scenes["harbor-flat"].clone();
    let hall =
        [json!({ "object": "hall-walls", "at": { "tile": [2, 5, 1] } }), json!({ "object": "hall-roof", "at": { "tile": [2, 5, 2] } })];
    let trees = [json!({ "object": "tree", "at": { "tile": [3, 6, 0] } }), json!({ "object": "tree", "at": { "tile": [3, 4, 0] } })];
    let mut lone = sources.clone();
    let mut scene = base;
    scene["items"] = json!([trees[0], trees[1], hall[0], hall[1]]);
    lone.scenes = [("hall".to_string(), scene)].into_iter().collect();
    let order = depth_of(&lone, "hall");
    let at = |i: usize| order.iter().position(|&n| n == i).unwrap();
    // trees[1] (3,4,0) is one row behind the hall's far half: it draws before the walls and the roof.
    assert!(at(1) < at(2) && at(1) < at(3), "{order:?}");
    // trees[0] (3,6,0) is one row in front of it: it draws after the walls (it stands under the roof's level, so the roof is not ordered against it).
    assert!(at(0) > at(2), "{order:?}");
}

// ------------------------------------ the sort moves as few items as it can

/// The first listed free node, always (what the sort did before it also tried pulling blockers forward).
fn first_free(nodes: &[Node]) -> Vec<usize> {
    let n = nodes.len();
    let mut waiting: Vec<i64> = (0..n)
        .map(|j| {
            (0..n).filter(|&i| i != j && overlaps(nodes[i].bounds, nodes[j].bounds) && behind(nodes[i].boxed, nodes[j].boxed)).count()
                as i64
        })
        .collect();
    let mut placed = vec![false; n];
    let mut out = Vec::new();
    while out.len() < n {
        let rest: Vec<usize> = (0..n).filter(|&i| !placed[i]).collect();
        let free = rest.iter().copied().find(|&i| waiting[i] == 0);
        let next = free.unwrap_or_else(|| {
            rest.iter().copied().reduce(|a, b| if key_of(nodes[b].boxed) < key_of(nodes[a].boxed) { b } else { a }).unwrap()
        });
        placed[next] = true;
        out.push(next);
        for j in 0..n {
            if !placed[j] && overlaps(nodes[next].bounds, nodes[j].bounds) && behind(nodes[next].boxed, nodes[j].boxed) {
                waiting[j] -= 1;
            }
        }
    }
    out
}

fn free(b: Box3) -> Node {
    node_b(b, None)
}

#[test]
fn r2_displaced_counts_the_items_out_of_the_longest_run_that_keeps_its_order() {
    assert_eq!(displaced(&[]), 0);
    assert_eq!(displaced(&[0, 1, 2, 3]), 0);
    assert_eq!(displaced(&[3, 0, 1, 2]), 1, "one item jumped far");
    assert_eq!(displaced(&[1, 2, 3, 0]), 1);
    assert_eq!(displaced(&[2, 3, 4, 5, 0, 1]), 2);
    assert_eq!(displaced(&[3, 2, 1, 0]), 3);
}

#[test]
fn r2_an_item_that_must_come_before_two_others_listed_far_ahead_of_it_is_pulled_forward_not_left_for_them_to_wait_on() {
    // W1 and W2 are in front of P but listed first; f1…f3 meet nothing. Holding W1 and W2 back would displace both; pulling P forward displaces one.
    let nodes = [
        node_b(unit(3.0, 0.0, 0.0), bounds(0, 0, 9, 9)),
        node_b(unit(4.0, 0.0, 0.0), bounds(0, 0, 9, 9)),
        free(unit(9.0, 9.0, 0.0)),
        free(unit(8.0, 9.0, 0.0)),
        free(unit(7.0, 9.0, 0.0)),
        node_b(unit(0.0, 0.0, 0.0), bounds(0, 0, 9, 9)),
    ];
    assert_eq!(first_free(&nodes), [2, 3, 4, 5, 0, 1]);
    assert_eq!(sort_nodes(&nodes), [5, 0, 1, 2, 3, 4], "only P moves, up in front of the walls");
}

#[test]
fn r2_an_item_listed_early_that_must_come_after_two_later_ones_is_the_one_that_moves_down() {
    let nodes = [
        node_b(unit(3.0, 0.0, 0.0), bounds(0, 0, 9, 9)),
        free(unit(9.0, 9.0, 0.0)),
        free(unit(8.0, 9.0, 0.0)),
        node_b(unit(0.0, 0.0, 0.0), bounds(0, 0, 9, 9)),
        node_b(unit(1.0, 0.0, 0.0), bounds(0, 0, 9, 9)),
    ];
    assert_eq!(sort_nodes(&nodes), [1, 2, 3, 4, 0], "pulling the two forward would displace two; holding the one back displaces one");
}

#[test]
fn r2_where_both_ways_displace_as_many_items_the_one_that_holds_the_blocked_item_back_is_kept() {
    let nodes = [
        node_b(unit(3.0, 0.0, 0.0), bounds(0, 0, 9, 9)),
        free(unit(9.0, 9.0, 0.0)),
        free(unit(8.0, 9.0, 0.0)),
        node_b(unit(0.0, 0.0, 0.0), bounds(0, 0, 9, 9)),
    ];
    assert_eq!(sort_nodes(&nodes), [1, 2, 3, 0]);
}

#[test]
fn r2_the_harbor_pine_moved_behind_the_red_house_moves_one_item_and_back_again_moves_one_item() {
    let (_dir, sources) = harbor();
    let flat = sources.scenes["harbor-flat"].clone();
    let mut lone = sources.clone();
    let pine_at = |list: &[Value], tile: &str| {
        list.iter().position(|i| i.get("group").is_some() && i["items"][0]["object"] == "pine" && tile_str(&i["items"][0]) == tile).unwrap()
    };
    let set = |lone: &mut Sources, list: &[Value]| {
        let mut scene = flat.clone();
        scene["items"] = json!(list);
        lone.scenes = [("flat".to_string(), scene)].into_iter().collect();
    };
    let mut list = items_of(&flat);
    set(&mut lone, &list);
    let order_of = |lone: &Sources, list: &[Value]| depth_order(lone, list, [0.0, 0.0], &OMap::new()).unwrap();
    assert_eq!(order_of(&lone, &list), (0..list.len()).collect::<Vec<_>>(), "the example is in order");
    let before = pine_at(&list, "5,6,0");
    list[before]["items"][0]["at"]["tile"] = json!([0, 1, 0]);
    set(&mut lone, &list);
    let up = order_of(&lone, &list);
    assert_eq!(displaced(&up), 1, "only the pine moves");
    assert!(up.iter().position(|&i| i == before).unwrap() < before, "up in front of the walls it stands behind");
    // Sort it, then put it back where it stood.
    list = up.iter().map(|&i| list[i].clone()).collect();
    set(&mut lone, &list);
    assert_eq!(order_of(&lone, &list), (0..list.len()).collect::<Vec<_>>());
    let at = pine_at(&list, "0,1,0");
    list[at]["items"][0]["at"]["tile"] = json!([5, 6, 0]);
    set(&mut lone, &list);
    let down = order_of(&lone, &list);
    assert_eq!(displaced(&down), 1, "only the pine moves");
    assert!(down.iter().position(|&i| i == at).unwrap() > at, "down behind everything it stands in front of");
}

#[test]
fn r2_never_displaces_more_than_holding_blocked_items_back_would_and_a_second_sort_changes_nothing_on_random_scenes_with_cycles() {
    for seed in SEEDS {
        let mut r = rng(seed + 7);
        for round in 0..40 {
            let n = 5 + r.floor(50.0) as usize;
            let boxes = random_boxes(&mut r, n, 5.0);
            let nodes: Vec<Node> = boxes
                .into_iter()
                .enumerate()
                .map(|(i, b)| Node { boxed: Some(b), bounds: bounds_for(&mut r, i, 0.5), depth: None, moving: false })
                .collect();
            let order = sort_nodes(&nodes);
            assert!(displaced(&order) <= displaced(&first_free(&nodes)), "seed {seed}, round {round}");
            let once: Vec<Node> = order.iter().map(|&i| nodes[i].clone()).collect();
            assert_eq!(sort_nodes(&once), (0..once.len()).collect::<Vec<_>>(), "seed {seed}, round {round}: sorted lists stay");
        }
    }
}
