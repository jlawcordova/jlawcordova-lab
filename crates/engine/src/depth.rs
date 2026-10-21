//! Depth order (ports cli/lib/engine/depth.mjs; docs/intents/2026-10-depth-order/spec.md, D2):
//! which of a scene's items draws first. Each item stands in a box in tile
//! space, and of two items whose pixels could overlap, the one that is
//! behind draws first. Only geometry and ordering over plain nodes; the
//! engine builds the nodes.

use std::rc::Rc;

use crate::iso::LEVEL_H;

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
    let [w, d, h] = size;
    Box3 { c0: c - 0.5, c1: c - 0.5 + w, r0: r - 0.5, r1: r - 0.5 + d, l0: l - h, l1: l }
}

/// A sprite standing at (c, r) on level l (`spriteBox`).
pub fn sprite_box(c: f64, r: f64, l: f64, footprint: [f64; 2], height: f64) -> Box3 {
    let [w, d] = footprint;
    Box3 { c0: c - 0.5, c1: c - 0.5 + w, r0: r - 0.5, r1: r - 0.5 + d, l0: l, l1: l + height }
}

/// How high a sprite rises above its anchor, in levels (`spriteHeight`):
/// at least ⅛, so a flat sprite still has a thickness.
pub fn sprite_height(anchor_y: f64) -> f64 {
    (anchor_y / LEVEL_H as f64).max(1.0 / 8.0)
}

/// The smallest box holding all of these, or None for none (`unionBox`).
pub fn union_box(boxes: &[Option<Box3>]) -> Option<Box3> {
    let mut out: Option<Box3> = None;
    for b in boxes.iter().flatten() {
        out = Some(match out {
            None => *b,
            Some(o) => Box3 {
                c0: o.c0.min(b.c0),
                c1: o.c1.max(b.c1),
                r0: o.r0.min(b.r0),
                r1: o.r1.max(b.r1),
                l0: o.l0.min(b.l0),
                l1: o.l1.max(b.l1),
            },
        });
    }
    out
}

/// The smallest pixel box holding all of these (`unionBounds`).
pub fn union_bounds(list: &[Option<Bounds>]) -> Option<Bounds> {
    let mut out: Option<Bounds> = None;
    for b in list.iter().flatten() {
        out = Some(match out {
            None => *b,
            Some(o) => Bounds {
                min_x: o.min_x.min(b.min_x),
                min_y: o.min_y.min(b.min_y),
                max_x: o.max_x.max(b.max_x),
                max_y: o.max_y.max(b.max_y),
            },
        });
    }
    out
}

/// A box moved by (dc, dr, dl) tiles (`shiftBox`).
pub fn shift_box(b: Box3, dc: f64, dr: f64, dl: f64) -> Box3 {
    Box3 { c0: b.c0 + dc, c1: b.c1 + dc, r0: b.r0 + dr, r1: b.r1 + dr, l0: b.l0 + dl, l1: b.l1 + dl }
}

/// Whether two pixel boxes share a pixel (`overlaps`).
pub fn overlaps(a: Option<Bounds>, b: Option<Bounds>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.min_x <= b.max_x && b.min_x <= a.max_x && a.min_y <= b.max_y && b.min_y <= a.max_y,
        _ => false,
    }
}

/// The sort key (`keyOf`): twice the sum of the box centre's coordinates;
/// −∞ for no box.
pub fn key_of(b: Option<Box3>) -> f64 {
    match b {
        Some(b) => b.c0 + b.c1 + b.r0 + b.r1 + b.l0 + b.l1,
        None => f64::NEG_INFINITY,
    }
}

/// Whether `a` draws before `b` by their boxes alone (`behind`).
pub fn behind(a: Option<Box3>, b: Option<Box3>) -> bool {
    let (Some(x), Some(y)) = (a, b) else { return false };
    let before = x.c1 <= y.c0 || x.r1 <= y.r0 || x.l1 <= y.l0;
    let after = y.c1 <= x.c0 || y.r1 <= x.r0 || y.l1 <= x.l0;
    if before || after {
        return before && !after;
    }
    key_of(a) < key_of(b)
}

/// How many items an order displaces (`displaced`): those not in the
/// longest run that keeps its relative order.
pub fn displaced(order: &[usize]) -> usize {
    let mut tails: Vec<usize> = Vec::new();
    for &old in order {
        let (mut lo, mut hi) = (0, tails.len());
        while lo < hi {
            let mid = (lo + hi) >> 1;
            if tails[mid] < old {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == tails.len() {
            tails.push(old);
        } else {
            tails[lo] = old;
        }
    }
    order.len() - tails.len()
}

const BANDS: [Option<Mark>; 3] = [Some(Mark::Back), None, Some(Mark::Front)];

/// The order the nodes draw in, as indices into `nodes` (`sortNodes`):
/// back-marked first, then unmarked, then front-marked, each band sorted on
/// its own.
pub fn sort_nodes(nodes: &[Node]) -> Vec<usize> {
    let mut order = Vec::with_capacity(nodes.len());
    for band in BANDS {
        let members: Vec<usize> = (0..nodes.len()).filter(|&i| nodes[i].depth == band).collect();
        order.extend(sort_band(nodes, &members));
    }
    order
}

/// Sorts one band by its "behind" pairs, two ways, keeping the one that
/// displaces fewer nodes (`sortBand`).
fn sort_band(nodes: &[Node], members: &[usize]) -> Vec<usize> {
    let n = members.len();
    let keys: Vec<f64> = members.iter().map(|&i| key_of(nodes[i].boxed)).collect();
    let mut behind_count = vec![0i64; n];
    let mut after: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut before: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (&nodes[members[i]], &nodes[members[j]]);
            if !overlaps(a.bounds, b.bounds) {
                continue;
            }
            let (first, second) = if behind(a.boxed, b.boxed) {
                (i, j)
            } else if behind(b.boxed, a.boxed) {
                (j, i)
            } else {
                continue;
            };
            after[first].push(second);
            before[second].push(first);
            behind_count[second] += 1;
        }
    }
    // (a)
    let mut waiting = behind_count.clone();
    let mut placed = vec![false; n];
    let mut first = Vec::with_capacity(n);
    let mut cycle = false;
    for _ in 0..n {
        let (mut free, mut any): (Option<usize>, Option<usize>) = (None, None);
        for k in 0..n {
            if placed[k] {
                continue;
            }
            if any.is_none_or(|a| keys[k] < keys[a]) {
                any = Some(k);
            }
            if waiting[k] == 0 && free.is_none() {
                free = Some(k);
            }
        }
        if free.is_none() {
            cycle = true;
        }
        let next = free.or(any).expect("a node is left");
        placed[next] = true;
        first.push(next);
        for &k in &after[next] {
            waiting[k] -= 1;
        }
    }
    if cycle || displaced(&first) == 0 {
        return first.iter().map(|&k| members[k]).collect();
    }
    // (b)
    let mut done = vec![false; n];
    let mut second = Vec::with_capacity(n);
    fn place(k: usize, done: &mut [bool], before: &[Vec<usize>], out: &mut Vec<usize>) {
        if done[k] {
            return;
        }
        done[k] = true;
        for &b in &before[k] {
            place(b, done, before, out);
        }
        out.push(k);
    }
    for k in 0..n {
        place(k, &mut done, &before, &mut second);
    }
    let pick = if displaced(&second) < displaced(&first) { second } else { first };
    pick.iter().map(|&k| members[k]).collect()
}

// --------------------------------------------------------------- while moving

/// What a moving node standing as one node object has to do with the
/// still ones (`relate`): the still nodes that wait for it, how many of the
/// chain must be placed before it, and its key.
#[derive(Clone)]
struct Relation {
    holds: Vec<usize>,
    needs: usize,
    key: f64,
}

struct Band {
    stills: Vec<usize>,
    movers: Vec<usize>,
    waiting: Vec<i64>,
    after: Vec<Vec<usize>>,
    key_of_still: std::collections::HashMap<usize, f64>,
    position: Vec<i64>,
    known: std::collections::HashMap<*const Node, (Rc<Node>, Relation)>,
}

/// The order of one list while some of its items move (`movingOrder`):
/// made once, then asked for many instants. A moving node's relations with
/// the still ones are kept for as long as the same `Rc` is given for it
/// (the Node engine's `WeakMap` by node object).
pub struct MovingOrder {
    nodes: Vec<Node>,
    /// Each node at rest, made once, so a moving node given no current node
    /// keeps its relations (as `nodes[i]` kept its identity in Node).
    rest: Vec<Rc<Node>>,
    bands: Vec<Band>,
}

/// Prepares the order of one list while some of its items move.
pub fn moving_order(nodes: &[Node]) -> MovingOrder {
    let bands = BANDS
        .iter()
        .map(|band| {
            let members: Vec<usize> = (0..nodes.len()).filter(|&i| nodes[i].depth == *band).collect();
            let stills: Vec<usize> = members.iter().copied().filter(|&i| !nodes[i].moving).collect();
            let movers: Vec<usize> = members.iter().copied().filter(|&i| nodes[i].moving).collect();
            let mut position = vec![-1i64; nodes.len()];
            for (at, &i) in stills.iter().enumerate() {
                position[i] = at as i64;
            }
            let mut waiting = vec![0i64; nodes.len()];
            let mut after: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
            for a in 0..stills.len() {
                for b in a + 1..stills.len() {
                    let (i, j) = (stills[a], stills[b]);
                    if !overlaps(nodes[i].bounds, nodes[j].bounds) {
                        continue;
                    }
                    if behind(nodes[i].boxed, nodes[j].boxed) {
                        after[i].push(j);
                        waiting[j] += 1;
                    } else if behind(nodes[j].boxed, nodes[i].boxed) {
                        after[j].push(i);
                        waiting[i] += 1;
                    }
                }
            }
            // The chain: each still node comes after the one before it.
            for a in 1..stills.len() {
                after[stills[a - 1]].push(stills[a]);
                waiting[stills[a]] += 1;
            }
            let key_of_still = stills.iter().map(|&i| (i, key_of(nodes[i].boxed))).collect();
            Band { stills, movers, waiting, after, key_of_still, position, known: Default::default() }
        })
        .collect();
    MovingOrder { nodes: nodes.to_vec(), rest: nodes.iter().cloned().map(Rc::new).collect(), bands }
}

impl MovingOrder {
    /// The order at one instant, given the current node of each moving one
    /// by its index (`current`); nodes not given stand where they rest.
    pub fn order(&mut self, current: &[(usize, Rc<Node>)]) -> Vec<usize> {
        let nodes = &self.nodes;
        let rest = &self.rest;
        let mut order = Vec::with_capacity(nodes.len());
        for band in &mut self.bands {
            let mut waiting = band.waiting.clone();
            let now: Vec<Rc<Node>> = band
                .movers
                .iter()
                .map(|&m| current.iter().find(|(i, _)| *i == m).map(|(_, n)| n.clone()).unwrap_or_else(|| rest[m].clone()))
                .collect();
            let relations: Vec<Relation> = now
                .iter()
                .map(|node| {
                    let ptr = Rc::as_ptr(node);
                    if let Some((_, r)) = band.known.get(&ptr) {
                        return r.clone();
                    }
                    let mut holds = Vec::new();
                    let mut needs = 0usize;
                    for &o in &band.stills {
                        let other = &nodes[o];
                        if !overlaps(node.bounds, other.bounds) {
                            continue;
                        }
                        if behind(node.boxed, other.boxed) {
                            holds.push(o);
                        } else if behind(other.boxed, node.boxed) {
                            needs = (band.position[o] + 1) as usize;
                        }
                    }
                    let r = Relation { holds, needs, key: key_of(node.boxed) };
                    band.known.insert(ptr, (node.clone(), r.clone()));
                    r
                })
                .collect();
            let m = band.movers.len();
            let mut holds_movers: Vec<Vec<usize>> = vec![Vec::new(); m];
            let mut waits = vec![0i64; m];
            for a in 0..m {
                for &o in &relations[a].holds {
                    waiting[o] += 1;
                }
                for b in a + 1..m {
                    if !overlaps(now[a].bounds, now[b].bounds) {
                        continue;
                    }
                    if behind(now[a].boxed, now[b].boxed) {
                        holds_movers[a].push(b);
                        waits[b] += 1;
                    } else if behind(now[b].boxed, now[a].boxed) {
                        holds_movers[b].push(a);
                        waits[a] += 1;
                    }
                }
            }
            let mut unplaced: Vec<usize> = (0..m).collect();
            let mut head = 0usize;
            for _ in 0..band.stills.len() + m {
                let still: Option<usize> = band.stills.get(head).copied();
                let (mut free, mut free_at): (Option<usize>, Option<usize>) = (None, None);
                for (u, &k) in unplaced.iter().enumerate() {
                    if waits[k] == 0 && head >= relations[k].needs {
                        free = Some(band.movers[k]);
                        free_at = Some(u);
                        break;
                    }
                }
                if let Some(s) = still {
                    if waiting[s] == 0 && free.is_none_or(|f| s < f) {
                        free = Some(s);
                        free_at = None;
                    }
                }
                let mut next = free;
                if free.is_none() {
                    // A cycle: the smallest key, then the first listed.
                    let mut best = f64::INFINITY;
                    for (u, &k) in unplaced.iter().enumerate() {
                        let mk = band.movers[k];
                        if next.is_none() || relations[k].key < best || (relations[k].key == best && Some(mk) < next) {
                            next = Some(mk);
                            best = relations[k].key;
                            free_at = Some(u);
                        }
                    }
                    if let Some(s) = still {
                        let key = band.key_of_still[&s];
                        if next.is_none() || key < best || (key == best && Some(s) < next) {
                            next = Some(s);
                            free_at = None;
                        }
                    }
                }
                let next = next.expect("a node is left");
                order.push(next);
                match free_at {
                    Some(u) => {
                        let k = unplaced.remove(u);
                        for &o in &relations[k].holds {
                            waiting[o] -= 1;
                        }
                        for &b in &holds_movers[k] {
                            waits[b] -= 1;
                        }
                    }
                    None => {
                        head += 1;
                        for &k in &band.after[next] {
                            waiting[k] -= 1;
                        }
                    }
                }
            }
        }
        order
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
    let mut places = Vec::new();
    let mut follows = -1i64;
    let mut rank = 0usize;
    for &i in order {
        if !nodes[i].moving {
            follows = i as i64;
            rank = 0;
        } else {
            places.push((i, Place { follows, rank }));
            rank += 1;
        }
    }
    places
}

/// What each moving node does over a loop (`placeWindows`), each node in the
/// order it first shows, its copies in the order they first show.
pub fn place_windows(cuts: &[f64], places_at: &mut dyn FnMut(usize) -> Vec<(usize, Place)>) -> Vec<(usize, Vec<Copy>)> {
    let mut out: Vec<(usize, Vec<Copy>)> = Vec::new();
    for span in 0..cuts.len().saturating_sub(1) {
        for (node, place) in places_at(span) {
            let at = match out.iter().position(|(n, _)| *n == node) {
                Some(at) => at,
                None => {
                    out.push((node, Vec::new()));
                    out.len() - 1
                }
            };
            let copies = &mut out[at].1;
            let k = match copies.iter().position(|c| c.follows == place.follows && c.rank == place.rank) {
                Some(k) => k,
                None => {
                    copies.push(Copy { follows: place.follows, rank: place.rank, windows: Vec::new() });
                    copies.len() - 1
                }
            };
            let windows = &mut copies[k].windows;
            match windows.last_mut() {
                Some(last) if last.1 == cuts[span] => last.1 = cuts[span + 1],
                _ => windows.push((cuts[span], cuts[span + 1])),
            }
        }
    }
    out
}
