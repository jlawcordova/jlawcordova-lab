//! Depth order (ports cli/lib/engine/depth.mjs; docs/intents/2026-10-depth-order/spec.md, D2):
//! which of a scene's items draws first. Each item stands in a box in tile
//! space, and of two items whose pixels could overlap, the one that is
//! behind draws first. Only geometry and ordering over plain nodes; the
//! engine builds the nodes.

use std::rc::Rc;

/// A box in tile space: columns c0…c1, rows r0…r1, heights l0…l1. Every
/// coordinate is a multiple of 1/32, which doubles hold exactly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box3 {
    pub c0: f64,
    pub c1: f64,
    pub r0: f64,
    pub r1: f64,
    pub l0: f64,
    pub l1: f64,
}

/// A pixel box: the pixels an item can draw on, inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

/// An item's or a group's override: draw in front of, or behind, the rest of
/// its list (`depth: 'front' | 'back'`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Back,
    Front,
}

impl Mark {
    /// `'front'` or `'back'`, as a scene names it.
    pub fn parse(text: &str) -> Option<Mark> {
        match text {
            "front" => Some(Mark::Front),
            "back" => Some(Mark::Back),
            _ => None,
        }
    }
}

/// What the sort needs of an item, a group or a run of variants: where it
/// stands, where it can draw, its override, and (for `moving_order`)
/// whether it moves.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub boxed: Option<Box3>,
    pub bounds: Option<Bounds>,
    pub depth: Option<Mark>,
    pub moving: bool,
}

/// A block at tile [c, r, l] of `size` [wide, deep, high] (`blockBox`).
pub fn block_box(c: f64, r: f64, l: f64, size: [f64; 3]) -> Box3 {
    let _ = (c, r, l, size);
    todo!("U4: port depth.mjs blockBox")
}

/// A sprite standing at (c, r) on level l (`spriteBox`).
pub fn sprite_box(c: f64, r: f64, l: f64, footprint: [f64; 2], height: f64) -> Box3 {
    let _ = (c, r, l, footprint, height);
    todo!("U4: port depth.mjs spriteBox")
}

/// How high a sprite rises above its anchor, in levels (`spriteHeight`).
pub fn sprite_height(anchor_y: f64) -> f64 {
    let _ = anchor_y;
    todo!("U4: port depth.mjs spriteHeight")
}

/// The smallest box holding all of these, or None for none (`unionBox`).
pub fn union_box(boxes: &[Option<Box3>]) -> Option<Box3> {
    let _ = boxes;
    todo!("U4: port depth.mjs unionBox")
}

/// The smallest pixel box holding all of these (`unionBounds`).
pub fn union_bounds(list: &[Option<Bounds>]) -> Option<Bounds> {
    let _ = list;
    todo!("U4: port depth.mjs unionBounds")
}

/// A box moved by (dc, dr, dl) tiles (`shiftBox`).
pub fn shift_box(b: Box3, dc: f64, dr: f64, dl: f64) -> Box3 {
    let _ = (b, dc, dr, dl);
    todo!("U4: port depth.mjs shiftBox")
}

/// Whether two pixel boxes share a pixel (`overlaps`).
pub fn overlaps(a: Option<Bounds>, b: Option<Bounds>) -> bool {
    let _ = (a, b);
    todo!("U4: port depth.mjs overlaps")
}

/// The sort key (`keyOf`): −∞ for no box.
pub fn key_of(b: Option<Box3>) -> f64 {
    let _ = b;
    todo!("U4: port depth.mjs keyOf")
}

/// Whether `a` draws before `b` by their boxes alone (`behind`).
pub fn behind(a: Option<Box3>, b: Option<Box3>) -> bool {
    let _ = (a, b);
    todo!("U4: port depth.mjs behind")
}

/// How many items an order displaces (`displaced`).
pub fn displaced(order: &[usize]) -> usize {
    let _ = order;
    todo!("U4: port depth.mjs displaced")
}

/// The order the nodes draw in, as indices into `nodes` (`sortNodes`).
pub fn sort_nodes(nodes: &[Node]) -> Vec<usize> {
    let _ = nodes;
    todo!("U4: port depth.mjs sortNodes")
}

/// The order of one list while some of its items move (`movingOrder`):
/// made once, then asked for many instants. A moving node's relations with
/// the still ones are kept for as long as the same `Rc` is given for it
/// (the Node engine's `WeakMap` by node object).
pub struct MovingOrder {
    _private: (),
}

/// Prepares the order of one list while some of its items move.
pub fn moving_order(nodes: &[Node]) -> MovingOrder {
    let _ = nodes;
    todo!("U4: port depth.mjs movingOrder")
}

impl MovingOrder {
    /// The order at one instant, given the current node of each moving one
    /// by its index (`current`); nodes not given stand where they rest.
    pub fn order(&mut self, current: &[(usize, Rc<Node>)]) -> Vec<usize> {
        let _ = current;
        todo!("U4: port depth.mjs movingOrder's returned function")
    }
}

/// A moving node's place in an order: the still node it follows (−1 at the
/// start) and its rank among the moving nodes after that one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub follows: i64,
    pub rank: usize,
}

/// One place a moving node takes over a loop, with the windows ([from, to),
/// percentages) in which it is there.
#[derive(Clone, Debug, PartialEq)]
pub struct Copy {
    pub follows: i64,
    pub rank: usize,
    pub windows: Vec<(f64, f64)>,
}

/// The place of every moving node in an order, in the order they come
/// (`placesIn`, whose Map kept that order).
pub fn places_in(order: &[usize], nodes: &[Node]) -> Vec<(usize, Place)> {
    let _ = (order, nodes);
    todo!("U4: port depth.mjs placesIn")
}

/// What each moving node does over a loop (`placeWindows`), each node in the
/// order it first shows, its copies in the order they first show.
pub fn place_windows(cuts: &[f64], places_at: &mut dyn FnMut(usize) -> Vec<(usize, Place)>) -> Vec<(usize, Vec<Copy>)> {
    let _ = (cuts, places_at);
    todo!("U4: port depth.mjs placeWindows")
}
