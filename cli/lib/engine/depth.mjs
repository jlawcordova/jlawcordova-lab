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
 * it stays a multiple of 1/32 and a smaller key is further back. A box with
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
 * The order the nodes draw in, as indices into `nodes`. Back-marked nodes
 * draw first, then unmarked ones, then front-marked ones; each band is sorted
 * on its own. In a band, repeatedly take, of the nodes that nothing unplaced
 * is behind (and whose pixels can meet), the one with the smallest key, then
 * the smallest index. If every remaining node has something behind it (a
 * cycle), take the smallest key of them all. Whether a node is available
 * depends only on the set already placed, so sorting a sorted list returns it.
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
 * @param {Node[]} nodes
 * @param {number[]} members  indices into nodes, ascending
 * @returns {number[]}
 */
function sortBand(nodes, members) {
  const n = members.length;
  const keys = members.map((i) => keyOf(nodes[i].box));
  /** How many unplaced nodes are behind each, and the nodes each is behind of. */
  const waiting = new Array(n).fill(0);
  /** @type {number[][]} */
  const after = Array.from({ length: n }, () => []);
  for (let i = 0; i < n; i++) {
    for (let j = i + 1; j < n; j++) {
      const [a, b] = [nodes[members[i]], nodes[members[j]]];
      if (!overlaps(a.bounds, b.bounds)) continue;
      if (behind(a.box, b.box)) {
        after[i].push(j);
        waiting[j]++;
      } else if (behind(b.box, a.box)) {
        after[j].push(i);
        waiting[i]++;
      }
    }
  }
  const placed = new Array(n).fill(false);
  /** @type {number[]} */
  const order = [];
  for (let step = 0; step < n; step++) {
    let free = -1;
    let any = -1;
    for (let k = 0; k < n; k++) {
      if (placed[k]) continue;
      if (any < 0 || keys[k] < keys[any]) any = k;
      if (waiting[k] === 0 && (free < 0 || keys[k] < keys[free])) free = k;
    }
    const next = free >= 0 ? free : any;
    placed[next] = true;
    order.push(members[next]);
    for (const k of after[next]) waiting[k]--;
  }
  return order;
}
