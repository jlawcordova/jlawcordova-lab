//! Ports of the iso.mjs tests in cli/test/engine.test.mjs (the grid and
//! block faces).

use std::collections::{HashMap, HashSet};

use engine::iso::{block_faces, block_size, footprint, px_to_tile, px_to_tile_exact, tile_pixels, tile_to_px};

const SEEDS: [u32; 5] = [1, 7, 42, 2026, 31337];

/// Deterministic pseudo-random numbers (mulberry32), so a failure can be
/// replayed from its seed.
fn rng(seed: u32) -> impl FnMut() -> f64 {
    let mut a = seed;
    move || {
        a = a.wrapping_add(0x6d2b79f5);
        let mut t = a;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4294967296.0
    }
}

type Picture = (&'static str, [i32; 2], &'static [&'static str]);

/// Faces from `block_faces`, drawn as text: T, L and R for the top, left and
/// right faces, and the same letter in lowercase where the edge covers it.
/// Each picture starts at its origin, relative to the first tile's top-face
/// center.
const PICTURES: &[Picture] = &[
    (
        "1,1,0",
        [-15, -8],
        &[
            "..............tt..............",
            "............ttTTtt............",
            "..........ttTTTTTTtt..........",
            "........ttTTTTTTTTTTtt........",
            "......ttTTTTTTTTTTTTTTtt......",
            "....ttTTTTTTTTTTTTTTTTTTtt....",
            "..ttTTTTTTTTTTTTTTTTTTTTTTtt..",
            "ttTTTTTTTTTTTTTTTTTTTTTTTTTTtt",
            "ttTTTTTTTTTTTTTTTTTTTTTTTTTTtt",
            "..ttTTTTTTTTTTTTTTTTTTTTTTtt..",
            "....ttTTTTTTTTTTTTTTTTTTtt....",
            "......ttTTTTTTTTTTTTTTtt......",
            "........ttTTTTTTTTTTtt........",
            "..........ttTTTTTTtt..........",
            "............ttTTtt............",
            "..............tt..............",
        ],
    ),
    (
        "1,1,1",
        [-15, -8],
        &[
            "..............tt..............",
            "............ttTTtt............",
            "..........ttTTTTTTtt..........",
            "........ttTTTTTTTTTTtt........",
            "......ttTTTTTTTTTTTTTTtt......",
            "....ttTTTTTTTTTTTTTTTTTTtt....",
            "..ttTTTTTTTTTTTTTTTTTTTTTTtt..",
            "ttTTTTTTTTTTTTTTTTTTTTTTTTTTtt",
            "tTTTTTTTTTTTTTTTTTTTTTTTTTTTTt",
            "lLTTTTTTTTTTTTTTTTTTTTTTTTTTRr",
            "lLLLTTTTTTTTTTTTTTTTTTTTTTRRRr",
            "lLLLLLTTTTTTTTTTTTTTTTTTRRRRRr",
            "lLLLLLLLTTTTTTTTTTTTTTRRRRRRRr",
            "lLLLLLLLLLTTTTTTTTTTRRRRRRRRRr",
            "lLLLLLLLLLLLTTTTTTRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLTTRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "llLLLLLLLLLLLLLRRRRRRRRRRRRRrr",
            "..llLLLLLLLLLLLRRRRRRRRRRRrr..",
            "....llLLLLLLLLLRRRRRRRRRrr....",
            "......llLLLLLLLRRRRRRRrr......",
            "........llLLLLLRRRRRrr........",
            "..........llLLLRRRrr..........",
            "............llLRrr............",
            "..............lr..............",
        ],
    ),
    (
        "2,1,2",
        [-15, -8],
        &[
            "..............tt..............................",
            "............ttTTtt............................",
            "..........ttTTTTTTtt..........................",
            "........ttTTTTTTTTTTtt........................",
            "......ttTTTTTTTTTTTTTTtt......................",
            "....ttTTTTTTTTTTTTTTTTTTtt....................",
            "..ttTTTTTTTTTTTTTTTTTTTTTTtt..................",
            "ttTTTTTTTTTTTTTTTTTTTTTTTTTTtt................",
            "tTTTTTTTTTTTTTTTTTTTTTTTTTTTTTtt..............",
            "lLTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTtt............",
            "lLLLTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTtt..........",
            "lLLLLLTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTtt........",
            "lLLLLLLLTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTtt......",
            "lLLLLLLLLLTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTtt....",
            "lLLLLLLLLLLLTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTtt..",
            "lLLLLLLLLLLLLLTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTtt",
            "lLLLLLLLLLLLLLLLTTTTTTTTTTTTTTTTTTTTTTTTTTTTTt",
            "lLLLLLLLLLLLLLLLLLTTTTTTTTTTTTTTTTTTTTTTTTTTRr",
            "lLLLLLLLLLLLLLLLLLLLTTTTTTTTTTTTTTTTTTTTTTRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLTTTTTTTTTTTTTTTTTTRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLTTTTTTTTTTTTTTRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLTTTTTTTTTTRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLTTTTTTRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLTTRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "lLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "llLLLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "..llLLLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "....llLLLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "......llLLLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "........llLLLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "..........llLLLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "............llLLLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "..............llLLLLLLLLLLLLLLLRRRRRRRRRRRRRRr",
            "................llLLLLLLLLLLLLLRRRRRRRRRRRRRrr",
            "..................llLLLLLLLLLLLRRRRRRRRRRRrr..",
            "....................llLLLLLLLLLRRRRRRRRRrr....",
            "......................llLLLLLLLRRRRRRRrr......",
            "........................llLLLLLRRRRRrr........",
            "..........................llLLLRRRrr..........",
            "............................llLRrr............",
            "..............................lr..............",
        ],
    ),
];

fn picture(size: [i64; 3]) -> (Vec<String>, usize) {
    let key = format!("{},{},{}", size[0], size[1], size[2]);
    let (_, origin, rows) = PICTURES.iter().find(|p| p.0 == key).unwrap();
    let faces = block_faces(size, [0, 0]);
    let mut at: HashMap<(i32, i32), char> = HashMap::new();
    for (face, letter) in [(&faces.top, 'T'), (&faces.left, 'L'), (&faces.right, 'R')] {
        for p in face {
            at.insert(*p, letter);
        }
    }
    for p in &faces.edge {
        let c = at[p].to_ascii_lowercase();
        at.insert(*p, c);
    }
    let drawn = (0..rows.len())
        .map(|r| (0..rows[r].len()).map(|c| at.get(&(origin[0] + c as i32, origin[1] + r as i32)).copied().unwrap_or('.')).collect())
        .collect();
    (drawn, at.len())
}

#[test]
fn r3_tile_placement_uses_the_grid_and_the_scene_origin_tile_to_px() {
    // The tileToPx assertions of 'R3: tile placement uses the grid and the
    // scene origin; px placement is exact'. Its scene half needs U3 (below).
    assert_eq!(tile_to_px([0.0, 0.0, 0.0], [0.0, 0.0]), [0.0, 0.0]);
    assert_eq!(tile_to_px([1.0, 0.0, 0.0], [0.0, 0.0]), [16.0, 8.0]);
    assert_eq!(tile_to_px([0.0, 1.0, 0.0], [0.0, 0.0]), [-16.0, 8.0]);
    assert_eq!(tile_to_px([2.0, 1.0, 1.0], [5.0, 32.0]), [21.0, 40.0]);
}

#[test]
#[ignore = "needs U3"]
fn r3_tile_placement_uses_the_grid_and_the_scene_origin_px_placement_is_exact() {
    // renderScene + composite: tile [1,0,1] at origin [2,30] paints 'grass-2'
    // at 18,22, and px [3,4] paints 'ink' at 3,4.
    todo!("needs render_scene and composite (U3)");
}

#[test]
fn r16_px_to_tile_finds_the_tile_under_every_pixel_at_any_level_and_origin() {
    for origin in [[0.0, 0.0], [5.0, 32.0], [-17.0, 3.0]] {
        for level in [0.0, 1.0, -2.0] {
            for col in -3..=3 {
                for row in -3..=3 {
                    let [cx, cy] = tile_to_px([col as f64, row as f64, level], origin);
                    for (x, y) in tile_pixels(cx as i32, cy as i32) {
                        assert_eq!(
                            px_to_tile([x as f64, y as f64], level, origin),
                            [col as f64, row as f64, level],
                            "pixel {x},{y}, origin {origin:?}, level {level}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn px_to_tile_exact_inverts_tile_to_px_and_never_gives_negative_zero() {
    let origin = [5.0, 32.0];
    let [px, py] = tile_to_px([2.0, 1.0, 1.0], origin);
    assert_eq!(px_to_tile_exact([px, py], 1.0, origin), [2.0, 1.0]);
    let zero = px_to_tile_exact([0.0, 0.0], 0.0, [0.0, 0.0]);
    assert!(zero.iter().all(|v| v.is_sign_positive()));
    let tile = px_to_tile([-1.0, -1.0], 0.0, [0.0, 0.0]);
    assert!(tile.iter().all(|v| v.is_sign_positive()), "{tile:?}");
}

#[test]
fn r28_pixel_exact_faces_for_1_1_0_1_1_1_and_2_1_2() {
    for (key, _, rows) in PICTURES {
        let size: Vec<i64> = key.split(',').map(|n| n.parse().unwrap()).collect();
        let (got, count) = picture([size[0], size[1], size[2]]);
        assert_eq!(got, rows.iter().map(|r| r.to_string()).collect::<Vec<_>>(), "size {key}");
        // Nothing was drawn outside the picture.
        let expected: usize = rows.iter().map(|r| r.chars().filter(|&c| c != '.').count()).sum();
        assert_eq!(count, expected, "size {key}: no pixel outside the picture");
    }
}

#[test]
fn d4_a_tile_is_16_rows_2_pixels_wider_on_each_side_per_row_and_its_top_face_is_a_diamond() {
    let pixels = tile_pixels(0, 0);
    let widths: Vec<usize> = (-8..8).map(|y| pixels.iter().filter(|p| p.1 == y).count()).collect();
    assert_eq!(widths, [2, 6, 10, 14, 18, 22, 26, 30, 30, 26, 22, 18, 14, 10, 6, 2]);
}

#[test]
fn d4_the_edge_is_one_pixel_wide_and_every_step_across_the_top_steps_2_across_for_1_down() {
    let faces = block_faces([1, 1, 0], [0, 0]);
    let (edge, top) = (&faces.edge, &faces.top);
    let outline: HashSet<_> = edge.iter().collect();
    // Each row of the diamond's outline holds its leftmost and rightmost pixel pair, and they move 2 per row.
    let lefts: Vec<i32> = (-8..0).map(|y| top.iter().filter(|p| p.1 == y).map(|p| p.0).min().unwrap()).collect();
    let steps: Vec<i32> = lefts.windows(2).map(|w| w[1] - w[0]).collect();
    assert_eq!(steps, vec![-2; 7]);
    for &(x, y) in top {
        let exposed = [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)].iter().any(|n| !top.contains(n));
        assert_eq!(outline.contains(&(x, y)), exposed, "edge at {x},{y}");
    }
}

fn sorted(mut v: Vec<(i32, i32)>) -> Vec<(i32, i32)> {
    v.sort_by_key(|p| format!("{},{}", p.0, p.1));
    v
}

#[test]
fn d8_two_adjacent_blocks_share_an_edge_with_no_gap_and_no_overlap() {
    let a = block_faces([1, 1, 0], [0, 0]).top;
    let b = block_faces([1, 1, 0], [16, 8]).top;
    let bset: HashSet<_> = b.iter().collect();
    assert!(a.iter().all(|p| !bset.contains(p)));
    let union: Vec<_> = a.iter().chain(&b).copied().collect::<HashSet<_>>().into_iter().collect();
    assert_eq!(sorted(union), sorted(block_faces([2, 1, 0], [0, 0]).top));
    let c = block_faces([1, 1, 0], [-16, 8]).top;
    let d = block_faces([1, 1, 0], [0, 16]).top;
    let four: Vec<_> = [a, b, c, d].concat();
    assert_eq!(four.iter().collect::<HashSet<_>>().len(), four.len(), "four tiles around a corner share no pixel");
    assert_eq!(sorted(four), sorted(block_faces([2, 2, 0], [0, 0]).top), "and together they are the 2×2 block's top");
}

/// Cuts a cols×rows rectangle at [c, d] into blocks, guillotine style.
fn cut(r: &mut impl FnMut() -> f64, c: i64, d: i64, w: i64, h: i64) -> Vec<[i64; 4]> {
    if w > 1 && r() < 0.5 {
        let k = 1 + (r() * (w - 1) as f64).floor() as i64;
        let mut out = cut(r, c, d, k, h);
        out.extend(cut(r, c + k, d, w - k, h));
        return out;
    }
    if h > 1 && r() < 0.5 {
        let k = 1 + (r() * (h - 1) as f64).floor() as i64;
        let mut out = cut(r, c, d, w, k);
        out.extend(cut(r, c, d + k, w, h - k));
        return out;
    }
    vec![[c, d, w, h]]
}

#[test]
fn d8_l4_a_seeded_random_grid_cut_into_blocks_of_random_sizes_tiles_with_no_gaps_or_overlaps() {
    for seed in SEEDS {
        let mut r = rng(seed);
        let cols = 1 + (r() * 7.0).floor() as i64;
        let rows = 1 + (r() * 7.0).floor() as i64;
        let blocks = cut(&mut r, 0, 0, cols, rows);
        let mut painted: HashMap<(i32, i32), i32> = HashMap::new();
        for [c, d, w, h] in &blocks {
            for p in block_faces([*w, *h, 0], [((c - d) * 16) as i32, ((c + d) * 8) as i32]).top {
                *painted.entry(p).or_insert(0) += 1;
            }
        }
        assert!(painted.values().all(|&n| n == 1), "seed {seed}: no pixel is painted twice");
        let mut whole = HashSet::new();
        for c in 0..cols {
            for d in 0..rows {
                for p in tile_pixels(((c - d) * 16) as i32, ((c + d) * 8) as i32) {
                    whole.insert(p);
                }
            }
        }
        assert_eq!(painted.len(), whole.len(), "seed {seed}: no gaps ({} blocks in {cols}×{rows})", blocks.len());
        assert!(painted.keys().all(|p| whole.contains(p)), "seed {seed}: nothing outside the grid");
        assert_eq!(painted.len() as i64, cols * rows * 256, "seed {seed}: every tile is 256 pixels");
    }
}

#[test]
fn r3_a_flat_block_has_a_top_and_nothing_below_it_faces() {
    // The faces half of 'R3: a flat block has a top and nothing below it'.
    let faces = block_faces([1, 1, 0], [0, 0]);
    assert!(faces.left.is_empty() && faces.right.is_empty());
    assert_eq!(faces.top.len(), 256);
}

#[test]
fn r27_l4_a_blocks_computed_size_is_its_drawn_size_block_size() {
    // block_size against the drawn faces (the resolve half is below).
    for seed in SEEDS {
        let mut r = rng(seed);
        for _ in 0..6 {
            let size = [1 + (r() * 3.0).floor() as i64, 1 + (r() * 3.0).floor() as i64, (r() * 3.0).floor() as i64];
            let faces = block_faces(size, [0, 0]);
            let all: Vec<_> = faces.top.iter().chain(&faces.left).chain(&faces.right).collect();
            let (min_x, max_x) = (all.iter().map(|p| p.0).min().unwrap(), all.iter().map(|p| p.0).max().unwrap());
            let (min_y, max_y) = (all.iter().map(|p| p.1).min().unwrap(), all.iter().map(|p| p.1).max().unwrap());
            assert_eq!(block_size(size), [(max_x - min_x + 1) as i64, (max_y - min_y + 1) as i64], "size {size:?}");
        }
    }
}

#[test]
#[ignore = "needs U3"]
fn r27_l4_a_blocks_computed_size_is_its_drawn_size() {
    // resolve(sources, 'blk') width and height equal the drawn faces' extent.
    todo!("needs resolve (U3)");
}

#[test]
fn footprint_runs_columns_right_and_down_and_rows_left_and_down() {
    assert_eq!(footprint([2, 2], [0, 0]), [[0, 0], [16, 8], [-16, 8], [0, 16]]);
}

#[test]
fn rng_matches_the_js_mulberry32() {
    let mut r = rng(2026);
    // The first two numbers Node gives for seed 2026.
    assert_eq!(r(), 0.45540769933722913);
    assert_eq!(r(), 0.30849614599719644);
}
