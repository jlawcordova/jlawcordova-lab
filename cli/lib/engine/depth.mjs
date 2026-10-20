// @ts-check
// Depth order (docs/intents/2026-10-depth-order/spec.md, D2): which of a
// scene's items draws first. Each item stands in a box in tile space, and of
// two items whose pixels could overlap, the one that is behind draws first.
// This module is only geometry and ordering over plain nodes; the engine
// builds the nodes (it can resolve objects and knows their drawn bounds).
// It uses no Node or DOM APIs.
//
// Tile space: tile [c, r, l] covers c−½…c+½ and r−½…r+½, its top face at
// height l. The view looks along (1, 1, 1) in it (a level is as high as a
// tile is deep), so a box that ends before another starts on any one axis
// is wholly behind it. Every coordinate is a multiple of 1/32, so doubles
// hold them exactly and nothing here rounds or uses an epsilon.

import { LEVEL_H } from './iso.mjs';

/**
 * A box in tile space: columns c0…c1, rows r0…r1, heights l0…l1.
 * @typedef {{ c0: number, c1: number, r0: number, r1: number, l0: number, l1: number }} Box
 * A pixel box: the pixels an item can draw on, inclusive.
 * @typedef {{ minX: number, minY: number, maxX: number, maxY: number }} Bounds
 * What the sort needs of an item, a group or a run of variants: where it
 * stands (null when it has no box, as an empty group), where it can draw
 * (null when it draws nothing), and an optional override.
 * @typedef {{ box: Box | null, bounds: Bounds | null, depth?: 'front' | 'back' }} Node
 */

/**
 * A block at tile [c, r, l] of `size` [wide, deep, high]: the footprint from
 * the first tile's corner, and the body from l − high up to its top face l.
 * @param {number} c
 * @param {number} r
 * @param {number} l
 * @param {number[]} size
 * @returns {Box}
 */
export function blockBox(c, r, l, [w, d, h]) {
  return { c0: c - 0.5, c1: c - 0.5 + w, r0: r - 0.5, r1: r - 0.5 + d, l0: l - h, l1: l };
}

/**
 * A sprite standing at (c, r) on level l, covering `footprint` [wide, deep]
 * tiles from the first tile's corner and rising `height` levels.
 * @param {number} c
 * @param {number} r
 * @param {number} l
 * @param {number[]} footprint
 * @param {number} height
 * @returns {Box}
 */
export function spriteBox(c, r, l, [w, d], height) {
  return { c0: c - 0.5, c1: c - 0.5 + w, r0: r - 0.5, r1: r - 0.5 + d, l0: l, l1: l + height };
}

/**
 * How high a sprite rises above its anchor, in levels: the anchor's row
 * count over a level's height, and at least ⅛, so a flat sprite still has a
 * thickness to be above or below.
 * @param {number} anchorY
 */
export const spriteHeight = (anchorY) => Math.max(anchorY / LEVEL_H, 1 / 8);

/**
 * The smallest box holding all of these, or null for none.
 * @param {(Box | null)[]} boxes
 * @returns {Box | null}
 */
export function unionBox(boxes) {
  /** @type {Box | null} */
  let out = null;
  for (const b of boxes) {
    if (!b) continue;
    out = out
      ? { c0: Math.min(out.c0, b.c0), c1: Math.max(out.c1, b.c1), r0: Math.min(out.r0, b.r0), r1: Math.max(out.r1, b.r1), l0: Math.min(out.l0, b.l0), l1: Math.max(out.l1, b.l1) }
      : { ...b };
  }
  return out;
}

/**
 * The smallest pixel box holding all of these, or null for none.
 * @param {(Bounds | null)[]} list
 * @returns {Bounds | null}
 */
export function unionBounds(list) {
  /** @type {Bounds | null} */
  let out = null;
  for (const b of list) {
    if (!b) continue;
    out = out
      ? { minX: Math.min(out.minX, b.minX), minY: Math.min(out.minY, b.minY), maxX: Math.max(out.maxX, b.maxX), maxY: Math.max(out.maxY, b.maxY) }
      : { ...b };
  }
  return out;
}

/** An item's box moved by (dc, dr, dl) tiles. @param {Box} b @returns {Box} */
export const shiftBox = (b, dc, dr, dl) => ({ c0: b.c0 + dc, c1: b.c1 + dc, r0: b.r0 + dr, r1: b.r1 + dr, l0: b.l0 + dl, l1: b.l1 + dl });

/** Whether two pixel boxes share a pixel. @param {Bounds | null} a @param {Bounds | null} b */
export const overlaps = (a, b) => a !== null && b !== null && a.minX <= b.maxX && b.minX <= a.maxX && a.minY <= b.maxY && b.minY <= a.maxY;

/**
 * The sort key: twice the sum of the box centre's column, row and height, so
 * it stays a multiple of 1/32 and a smaller key is further back. It decides
 * interpenetrating boxes and breaks cycles; free items keep their list order. A box with
 * none (an empty group) sorts before every other.
 * @param {Box | null} b
 */
export const keyOf = (b) => (b ? b.c0 + b.c1 + b.r0 + b.r1 + b.l0 + b.l1 : -Infinity);

/**
 * Whether `a` draws before `b` by their boxes alone. If a's box ends before
 * b's starts on any axis, every line of sight meets a first. If that holds
 * both ways (on different axes), the silhouettes can't overlap and neither is
 * behind. If the boxes interpenetrate, the one whose centre has the smaller
 * sum is behind, and a tie is no order. Never true both ways.
 * @param {Box | null} a
 * @param {Box | null} b
 */
export function behind(a, b) {
  if (!a || !b) return false;
  const before = a.c1 <= b.c0 || a.r1 <= b.r0 || a.l1 <= b.l0;
  const after = b.c1 <= a.c0 || b.r1 <= a.r0 || b.l1 <= a.l0;
  if (before || after) return before && !after;
  return keyOf(a) < keyOf(b);
}

/**
 * How many items an order displaces: those that are not part of the longest
 * run of items that keep their relative order. `order[k]` is the old index of
 * the item now at k, so an order that changes nothing displaces none, and one
 * item taken out and put elsewhere displaces one however many it jumps.
 * @param {number[]} order
 */
export function displaced(order) {
  /** The smallest last value of an increasing run of each length. @type {number[]} */
  const tails = [];
  for (const old of order) {
    let [lo, hi] = [0, tails.length];
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (tails[mid] < old) lo = mid + 1;
      else hi = mid;
    }
    tails[lo] = old;
  }
  return order.length - tails.length;
}

/**
 * The order the nodes draw in, as indices into `nodes`. Back-marked nodes
 * draw first, then unmarked ones, then front-marked ones; each band is sorted
 * on its own, keeping the order the file lists them in unless an overlap
 * forces a change, and moving as few items as it can (sortBand). Whether the
 * result is right depends only on the "behind" pairs, so sorting a sorted
 * list returns it.
 * @param {Node[]} nodes
 * @returns {number[]}
 */
export function sortNodes(nodes) {
  /** @type {number[]} */
  const order = [];
  for (const band of /** @type {const} */ (['back', undefined, 'front'])) {
    const members = nodes.flatMap((node, i) => (node.depth === band ? [i] : []));
    order.push(...sortBand(nodes, members));
  }
  return order;
}

/**
 * Sorts one band by its "behind" pairs (where the nodes' pixels can meet), two
 * ways, and keeps the one that displaces fewer nodes (a tie keeps the first):
 * (a) repeatedly take the first listed node that nothing unplaced is behind,
 * which holds a blocked node back while the nodes after it go ahead; (b) walk
 * the list and, before a node, place the nodes behind it that aren't placed
 * yet, in list order, which pulls a blocking node forward instead. A list
 * that is in order comes back unchanged either way. If every remaining node
 * has something behind it (a cycle), only (a) is used, and it takes the node
 * with the smallest key, then the smallest index; (a) leaves what it made
 * where it is when sorted again, which (b) can't promise with a cycle.
 * @param {Node[]} nodes
 * @param {number[]} members  indices into nodes, ascending
 * @returns {number[]}
 */
function sortBand(nodes, members) {
  const n = members.length;
  const keys = members.map((i) => keyOf(nodes[i].box));
  /** How many nodes are behind each, the nodes each is behind of, and the nodes behind each. */
  const behindCount = new Array(n).fill(0);
  /** @type {number[][]} */
  const after = Array.from({ length: n }, () => []);
  /** @type {number[][]} */
  const before = Array.from({ length: n }, () => []);
  for (let i = 0; i < n; i++) {
    for (let j = i + 1; j < n; j++) {
      const [a, b] = [nodes[members[i]], nodes[members[j]]];
      if (!overlaps(a.bounds, b.bounds)) continue;
      const [first, second] = behind(a.box, b.box) ? [i, j] : behind(b.box, a.box) ? [j, i] : [-1, -1];
      if (first < 0) continue;
      after[first].push(second);
      before[second].push(first);
      behindCount[second]++;
    }
  }
  // (a)
  const waiting = behindCount.slice();
  const placed = new Array(n).fill(false);
  /** @type {number[]} */
  const first = [];
  let cycle = false;
  for (let step = 0; step < n; step++) {
    // The first listed node that is free; failing that (a cycle), the smallest key.
    let free = -1;
    let any = -1;
    for (let k = 0; k < n; k++) {
      if (placed[k]) continue;
      if (any < 0 || keys[k] < keys[any]) any = k;
      if (waiting[k] === 0 && free < 0) free = k;
    }
    if (free < 0) cycle = true;
    const next = free >= 0 ? free : any;
    placed[next] = true;
    first.push(next);
    for (const k of after[next]) waiting[k]--;
  }
  if (cycle || displaced(first) === 0) return first.map((k) => members[k]);
  // (b)
  const done = new Array(n).fill(false);
  /** @type {number[]} */
  const second = [];
  const place = (/** @type {number} */ k) => {
    if (done[k]) return;
    done[k] = true;
    for (const behindK of before[k]) place(behindK);
    second.push(k);
  };
  for (let k = 0; k < n; k++) place(k);
  return (displaced(second) < displaced(first) ? second : first).map((k) => members[k]);
}

// --------------------------------------------------------------- while moving

/**
 * Prepares the order of one list while some of its items move (spec D6).
 * `nodes` are the list's nodes in the order sortNodes put them at rest; the
 * moving ones are flagged `moving`, and every other one is still. The still
 * nodes keep that one order for the whole loop: each must come after the
 * still node before it, besides the usual "behind" pairs. The result gives,
 * for the current node of each moving one (by index, with the box and pixel
 * bounds it has at that instant), the order at that instant: the same greedy
 * sort as sortNodes, which takes the first listed node that nothing unplaced
 * is behind. A cycle is broken by the smallest key among the nodes that the
 * still chain doesn't hold back. With nothing moved from rest, the order is
 * the rest order, so time 0 draws what the rest sort draws.
 *
 * It is made to be called for many instants: the pairs of still nodes are
 * worked out once, and a moving node's pairs with the still ones are worked
 * out once for each node object it is given (so give the same object while
 * its box and bounds stay the same, and don't change a node after giving
 * it). Only the pairs among moving nodes are tested again at each instant.
 * @param {(Node & { moving?: boolean })[]} nodes
 * @returns {(current: Map<number, Node>) => number[]}
 */
export function movingOrder(nodes) {
  const bands = /** @type {const} */ (['back', undefined, 'front']).map((band) => {
    const members = nodes.flatMap((node, i) => (node.depth === band ? [i] : []));
    const stills = members.filter((i) => !nodes[i].moving);
    const movers = members.filter((i) => nodes[i].moving);
    /** Where each still node stands in the chain, by node index. */
    const position = new Int32Array(nodes.length).fill(-1);
    stills.forEach((i, at) => (position[i] = at));
    /** The unplaced "behind" pairs among still nodes never change, so they're counted once. */
    const waiting = new Int32Array(nodes.length);
    /** The still nodes that come after each still one, by node index. @type {number[][]} */
    const after = [];
    for (const i of stills) after[i] = [];
    for (let a = 0; a < stills.length; a++) {
      for (let b = a + 1; b < stills.length; b++) {
        const [i, j] = [stills[a], stills[b]];
        if (!overlaps(nodes[i].bounds, nodes[j].bounds)) continue;
        if (behind(nodes[i].box, nodes[j].box)) {
          after[i].push(j);
          waiting[j]++;
        } else if (behind(nodes[j].box, nodes[i].box)) {
          after[j].push(i);
          waiting[i]++;
        }
      }
    }
    // The chain: each still node comes after the one before it. The loop below
    // only ever offers the next still node (stills[head]), so these edges
    // block nothing today. They are kept because they state the invariant the
    // still nodes keep (their own order) and guard it if the offering changes.
    for (let a = 1; a < stills.length; a++) {
      after[stills[a - 1]].push(stills[a]);
      waiting[stills[a]]++;
    }
    const keyOfStill = new Map(stills.map((i) => [i, keyOf(nodes[i].box)]));
    /**
     * What a moving node standing as `node` has to do with the still ones, kept
     * for as long as the node object is. `holds`: the still nodes that wait for
     * it (it is behind them). `needs`: how many of the chain must be placed
     * before it is (the position after the last still node behind it).
     * @type {WeakMap<Node, { holds: number[], needs: number, key: number }>}
     */
    const known = new WeakMap();
    const relate = (/** @type {Node} */ node) => {
      let found = known.get(node);
      if (!found) {
        const holds = [];
        let needs = 0;
        for (const o of stills) {
          const other = nodes[o];
          if (!overlaps(node.bounds, other.bounds)) continue;
          if (behind(node.box, other.box)) holds.push(o);
          else if (behind(other.box, node.box)) needs = position[o] + 1;
        }
        known.set(node, (found = { holds, needs, key: keyOf(node.box) }));
      }
      return found;
    };
    return { stills, movers, waiting, after, keyOfStill, relate };
  });
  return (current) => {
    /** @type {number[]} */
    const order = [];
    for (const { stills, movers, waiting: base, after: stillAfter, keyOfStill, relate } of bands) {
      const waiting = base.slice();
      const at = (/** @type {number} */ i) => current.get(i) ?? nodes[i];
      const now = movers.map((m) => at(m));
      const relations = now.map(relate);
      /** For each moving node (by its place in `movers`): the nodes that wait for it, the count of moving nodes it waits for, and how many of the chain it needs placed. @type {number[][]} */
      const holdsMovers = movers.map(() => []);
      const waits = new Int32Array(movers.length);
      for (let a = 0; a < movers.length; a++) {
        for (const o of relations[a].holds) waiting[o]++;
        for (let b = a + 1; b < movers.length; b++) {
          if (!overlaps(now[a].bounds, now[b].bounds)) continue;
          if (behind(now[a].box, now[b].box)) {
            holdsMovers[a].push(b);
            waits[b]++;
          } else if (behind(now[b].box, now[a].box)) {
            holdsMovers[b].push(a);
            waits[a]++;
          }
        }
      }
      /** The moving nodes not yet placed, by place in `movers`, ascending. */
      const unplaced = movers.map((_, k) => k);
      let head = 0;
      for (let step = stills.length + movers.length; step > 0; step--) {
        // The candidates: every unplaced moving node, and the next still one.
        const still = head < stills.length ? stills[head] : -1;
        // The first listed one that is free: a moving node waits for the moving ones behind it and for the chain up to the still ones behind it.
        let free = -1;
        let freeAt = -1;
        for (let u = 0; u < unplaced.length; u++) {
          const k = unplaced[u];
          if (waits[k] === 0 && head >= relations[k].needs) {
            [free, freeAt] = [movers[k], u];
            break;
          }
        }
        if (still >= 0 && waiting[still] === 0 && (free < 0 || still < free)) [free, freeAt] = [still, -1];
        let next = free;
        if (free < 0) {
          // A cycle: the smallest key, then the first listed.
          let best = Infinity;
          for (let u = 0; u < unplaced.length; u++) {
            const k = unplaced[u];
            if (next < 0 || relations[k].key < best || (relations[k].key === best && movers[k] < next)) [next, best, freeAt] = [movers[k], relations[k].key, u];
          }
          if (still >= 0) {
            const key = /** @type {number} */ (keyOfStill.get(still));
            if (next < 0 || key < best || (key === best && still < next)) [next, freeAt] = [still, -1];
          }
        }
        order.push(next);
        if (freeAt >= 0) {
          const k = unplaced[freeAt];
          unplaced.splice(freeAt, 1);
          for (const o of relations[k].holds) waiting[o]--;
          for (const b of holdsMovers[k]) waits[b]--;
        } else {
          head++;
          for (const k of stillAfter[next]) waiting[k]--;
        }
      }
    }
    return order;
  };
}

/**
 * The place of every moving node in an order: the still node it follows (its
 * index, or -1 at the start) and its rank among the moving nodes after that
 * same still node. Two moving nodes in one gap are told apart by rank, so
 * whichever are shown at one instant sit in the order they have there.
 * @param {number[]} order  indices into nodes
 * @param {{ moving?: boolean }[]} nodes
 * @returns {Map<number, { follows: number, rank: number }>}
 */
export function placesIn(order, nodes) {
  /** @type {Map<number, { follows: number, rank: number }>} */
  const places = new Map();
  let follows = -1;
  let rank = 0;
  for (const i of order) {
    if (!nodes[i].moving) {
      follows = i;
      rank = 0;
    } else places.set(i, { follows, rank: rank++ });
  }
  return places;
}

/**
 * What each moving node does over a loop, given the instants between which
 * nothing moves: the loop's cut points (from 0 to its end) and, for the span
 * between each two, the places the moving nodes take there. Each node's
 * distinct places come back as copies, in the order they first show, with
 * the spans (merged) in which the node is in that place, as [from, to).
 * @param {number[]} cuts  ascending, from 0 to the end
 * @param {(span: number) => Map<number, { follows: number, rank: number }>} placesAt
 * @returns {Map<number, { follows: number, rank: number, windows: [number, number][] }[]>}
 */
export function placeWindows(cuts, placesAt) {
  /** @type {Map<number, { follows: number, rank: number, windows: [number, number][] }[]>} */
  const out = new Map();
  for (let span = 0; span < cuts.length - 1; span++) {
    for (const [node, place] of placesAt(span)) {
      if (!out.has(node)) out.set(node, []);
      const copies = /** @type {{ follows: number, rank: number, windows: [number, number][] }[]} */ (out.get(node));
      let copy = copies.find((c) => c.follows === place.follows && c.rank === place.rank);
      if (!copy) copies.push((copy = { ...place, windows: [] }));
      const last = copy.windows[copy.windows.length - 1];
      if (last && last[1] === cuts[span]) last[1] = cuts[span + 1];
      else copy.windows.push([cuts[span], cuts[span + 1]]);
    }
  }
  return out;
}
