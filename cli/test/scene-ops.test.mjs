// scene-ops.mjs against the web lab's behavior (src/components/lab/scene-mode.ts
// in the site): each test states what the lab does and checks the port does
// the same, on plain scene documents.

import assert from 'node:assert/strict';
import { after, before, describe, test } from 'node:test';

import * as ops from '../lib/scene-ops.mjs';
import { openSite } from '../lib/document.mjs';
import { makeSite, needsSite } from './helpers.mjs';

const scene = () => ({
  viewBox: [0, 0, 200, 200],
  origin: [100, 50],
  items: [
    { object: 'a', at: { tile: [0, 0, 0] } },
    { object: 'b', at: { tile: [1, 0, 0] } },
    { object: 'c', at: { px: [10, 10] } },
    { object: 'd', at: { tile: [1, 1, 0] } },
    { object: 'e', at: { tile: [1, 1, 1] } },
    { group: { class: 'g' }, items: [{ object: 'f', id: 'cart', at: { tile: [3, 3, 0] } }, { object: 'h', at: { px: [5, 6] } }] },
  ],
  motion: { duration: 1000, tracks: [{ item: 'cart', keys: [{ t: 0 }, { t: 500, move: [32, 16] }] }] },
});

describe('scene ops', needsSite, () => {
  /** @type {Awaited<ReturnType<typeof makeSite>>} */
  let copy;
  /** @type {any} */
  let site;
  before(async () => {
    copy = await makeSite();
    site = await openSite({ site: copy.root });
  });
  after(() => copy.cleanup());

  test('insertIndex: back to front by row + col, then level, among tile items (lab insertIndex)', () => {
    const { items } = scene();
    // The first tile item with a greater row + col, or the same and a higher level.
    assert.equal(ops.insertIndex(items, [0, 0, 0]), 1); // before b (sum 1)
    assert.equal(ops.insertIndex(items, [0, 1, 0]), 3); // sum 1: after b, the px item is skipped, before d (sum 2)
    assert.equal(ops.insertIndex(items, [2, 0, 0]), 4); // sum 2, level 0: after d (level 0, not higher), before e (level 1)
    assert.equal(ops.insertIndex(items, [1, 1, 1]), 6); // e has the same level, not higher, and a group has no tile: at the end
    assert.equal(ops.insertIndex(items, [9, 9, 0]), items.length); // nothing in front: at the end (groups have no tile)
  });

  test('place: on a tile in back-to-front order, by pixel at the end (lab place)', () => {
    const doc = scene();
    const r = ops.place(doc, 'pine', { tile: [0, 1, 0] });
    assert.deepEqual(r.path, [3]);
    assert.deepEqual(doc.items[3], { object: 'pine', at: { tile: [0, 1, 0] } });
    assert.equal(r.message, 'pine placed at column 0, row 1, level 0');
    const p = ops.place(doc, 'rock', { px: [1, 2] });
    assert.deepEqual(p.path, [doc.items.length - 1]);
    assert.equal(p.message, 'rock placed at x 1, y 2');
  });

  test('moveByTiles: a tile item by tiles, a pixel item by the same screen offset (lab moveByTiles)', () => {
    const tile = { at: { tile: [1, 2, 0] } };
    ops.moveByTiles(tile, 1, -1, 2);
    assert.deepEqual(tile.at.tile, [2, 1, 2]);
    const px = { at: { px: [10, 10] } };
    ops.moveByTiles(px, 1, 0, 0);
    assert.deepEqual(px.at.px, [26, 18]); // (dc - dr) * 16, (dc + dr) * 8
    ops.moveByTiles(px, 0, 1, 1);
    assert.deepEqual(px.at.px, [10, 10]); // -16, +8 - 16
  });

  test('moveItem and nudgeItem (lab moveSelected, nudgeSelected)', () => {
    const doc = scene();
    assert.equal(ops.moveItem(site, doc, [0], 2, 1, 0).message, 'a moved to column 2, row 1, level 0');
    assert.deepEqual(doc.items[0].at, { tile: [2, 1, 0] });
    // A nudge places a tile item by pixel, from its placement point (iso tileToPx).
    const [x, y] = site.iso.tileToPx([1, 0, 0], doc.origin);
    const r = ops.nudgeItem(site, doc, [1], 1, -2);
    assert.deepEqual(doc.items[1].at, { px: [x + 1, y - 2] });
    assert.equal(r.message, `b nudged to x ${x + 1}, y ${y - 2}`);
    assert.throws(() => ops.moveItem(site, doc, [5], 1, 0, 0), (err) => err.code === 'is_group');
    assert.throws(() => ops.moveItem(site, doc, [9], 1, 0, 0), (err) => err.code === 'not_found');
  });

  test('keyedMove: past time 0 an animated item moves its key, made if needed (lab keyedMove)', () => {
    const doc = scene();
    // At t 250 the cart is half-way to its t 500 key: the new key takes that point plus the move.
    const start = site.motion.stateAt(doc.motion.tracks[0], 'f', 1000, 250).move;
    const r = ops.moveItem(site, doc, [5, 0], 1, 0, 0, 250);
    assert.equal(r.keyed, true);
    assert.equal(r.message, 'f moved at 0.3 s');
    assert.deepEqual(doc.motion.tracks[0].keys, [{ t: 0 }, { t: 250, move: [start[0] + 16, start[1] + 8] }, { t: 500, move: [32, 16] }]);
    assert.deepEqual(doc.items[5].items[0].at, { tile: [3, 3, 0] }, 'the placement stays');
    // An existing key moves; time 0 moves the placement.
    ops.nudgeItem(site, doc, [5, 0], 2, 0, 500);
    assert.deepEqual(doc.motion.tracks[0].keys[2], { t: 500, move: [34, 16] });
    ops.moveItem(site, doc, [5, 0], 1, 0, 0, 0);
    assert.deepEqual(doc.items[5].items[0].at, { tile: [4, 3, 0] });
  });

  test('setField sets one coordinate of the matching placement (lab setField)', () => {
    const doc = scene();
    ops.setField(doc, [0], 'level', 3);
    assert.deepEqual(doc.items[0].at.tile, [0, 0, 3]);
    ops.setField(doc, [2], 'y', 40);
    assert.deepEqual(doc.items[2].at.px, [10, 40]);
    // The lab ignores x on a tile item; the port refuses.
    assert.throws(() => ops.setField(doc, [0], 'x', 1), (err) => err.code === 'wrong_placement');
  });

  test('snapItem: to the level-0 tile under the placement point (lab snapSelected)', () => {
    const doc = scene();
    const point = site.iso.tileToPx([2, 1, 0], doc.origin);
    doc.items[2].at = { px: [point[0] + 3, point[1] - 2] };
    assert.equal(ops.snapItem(site, doc, [2]).message, 'c snapped to column 2, row 1, level 0');
    assert.deepEqual(doc.items[2].at, { tile: [2, 1, 0] });
    assert.throws(() => ops.snapItem(site, doc, [2]), /already on a tile/);
  });

  test('reorderItem swaps within its own list, refusing past either end (lab reorderSelected)', () => {
    const doc = scene();
    const r = ops.reorderItem(doc, [0], 1);
    assert.deepEqual(r.path, [1]);
    assert.deepEqual(doc.items.slice(0, 2).map((i) => i.object), ['b', 'a']);
    assert.equal(r.message, 'a raised to 2 of 6 in paint order');
    assert.equal(ops.reorderItem(doc, [1], -1).message, 'a lowered to 1 of 6 in paint order');
    assert.throws(() => ops.reorderItem(doc, [0], -1), /already at the back/);
    assert.throws(() => ops.reorderItem(doc, [5, 1], 1), /already at the front/);
    assert.deepEqual(ops.reorderItem(doc, [5, 1], -1).path, [5, 0]);
  });

  test('duplicateItem: one column over, right after it, without its id (lab duplicateSelected)', () => {
    const doc = scene();
    const r = ops.duplicateItem(doc, [5, 0]);
    assert.deepEqual(r.path, [5, 1]);
    assert.deepEqual(doc.items[5].items[1], { object: 'f', at: { tile: [4, 3, 0] } });
    assert.equal(doc.items[5].items[0].id, 'cart');
    assert.equal(doc.motion.tracks.length, 1);
    const px = ops.duplicateItem(doc, [2]);
    assert.deepEqual(doc.items[3], { object: 'c', at: { px: [26, 18] } });
    assert.equal(px.message, 'c duplicated at x 26, y 18');
  });

  test('removeItem drops the track of an item with an id (lab removeSelected)', () => {
    const doc = scene();
    ops.removeItem(doc, [5, 0]);
    assert.deepEqual(doc.items[5].items.map((i) => i.object), ['h']);
    assert.deepEqual(doc.motion.tracks, []);
  });

  test('setClass keeps the key order object, id, class, at (lab class field)', () => {
    const doc = scene();
    ops.setClass(doc, [5, 0], 'sway slow');
    assert.deepEqual(Object.keys(doc.items[5].items[0]), ['object', 'id', 'class', 'at']);
    assert.equal(ops.setClass(doc, [5, 0], '').message, 'Class removed');
    assert.deepEqual(Object.keys(doc.items[5].items[0]), ['object', 'id', 'at']);
    assert.throws(() => ops.setClass(doc, [0], 'Bad Class'), /lowercase class names/);
  });

  test('setId renames or drops the track with the id, and refuses a taken id', () => {
    const doc = scene();
    ops.setClass(doc, [5, 0], 'x');
    ops.setId(site, doc, [5, 0], 'wagon');
    assert.deepEqual(Object.keys(doc.items[5].items[0]), ['object', 'id', 'class', 'at']);
    assert.equal(doc.motion.tracks[0].item, 'wagon');
    ops.setId(site, doc, [0], 'first');
    assert.throws(() => ops.setId(site, doc, [1], 'wagon'), (err) => err.code === 'id_taken');
    assert.throws(() => ops.setId(site, doc, [1], 'Not An Id'), /kebab-case/);
    ops.setId(site, doc, [5, 0], '');
    assert.equal(doc.items[5].items[0].id, undefined);
    assert.deepEqual(doc.motion.tracks, []);
  });

  test('findItem: a path, or an id anywhere in the groups', () => {
    const doc = scene();
    assert.deepEqual(ops.findItem(site, doc, '5.1'), [5, 1]);
    assert.deepEqual(ops.findItem(site, doc, 'cart'), [5, 0]);
    assert.deepEqual(ops.findItem(site, doc, '#cart'), [5, 0]);
    assert.throws(() => ops.findItem(site, doc, 'nope'), (err) => err.code === 'not_found');
  });
});
