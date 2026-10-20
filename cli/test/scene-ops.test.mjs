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

  /** The scene of real sprites that sortScene needs (the fake objects above don't resolve). */
  const real = () => ({
    viewBox: [-100, -100, 200, 200],
    origin: [0, 0],
    items: [
      { object: 'pine', id: 'front', at: { tile: [1, 1, 0] } },
      { object: 'pine', id: 'back', at: { tile: [0, 0, 0] } },
      { object: 'pine', id: 'middle', at: { tile: [1, 0, 0] } },
      { object: 'pine', id: 'far', at: { tile: [9, 9, 0] } },
    ],
  });
  const ids = (/** @type {any} */ doc) => doc.items.map((/** @type {any} */ i) => i.id);
  const sources = async () => {
    const { palette, objects, scenes } = await site.loadDocs();
    return site.engine.loadSources({ palette, objects: Object.fromEntries(objects), scenes: Object.fromEntries(scenes) });
  };

  test('sortScene: back to front by column and row, then level, where items overlap (what insertIndex did for tiles)', async () => {
    const doc = real();
    const r = ops.sortScene(site, await sources(), doc);
    // "front" was listed first but stands in front of "back" and "middle"; "far" overlaps nothing and keeps its place.
    assert.deepEqual(ids(doc), ['back', 'middle', 'front', 'far']);
    assert.deepEqual(r.moved, [{ path: '0', to: '2', object: 'pine' }], 'only the item the sort took out of its place is listed');
    assert.deepEqual(Object.fromEntries(r.remap), { '0': '2', '1': '0', '2': '1' }, 'remap holds every path that changed');
    // Sorting a sorted scene moves nothing.
    const again = ops.sortScene(site, await sources(), doc);
    assert.deepEqual([again.moved, again.remap.size], [[], 0]);
  });

  test('sortScene sorts a group as one object and its members inside it; a scene that does not draw is left alone', async () => {
    const doc = real();
    doc.items = [
      { group: { class: 'g' }, items: [{ object: 'pine', id: 'g-front', at: { tile: [1, 1, 0] } }, { object: 'pine', id: 'g-back', at: { tile: [0, 0, 0] } }] },
      { object: 'pine', id: 'behind-all', at: { tile: [-1, -1, 0] } },
    ];
    const r = ops.sortScene(site, await sources(), doc);
    assert.equal(doc.items[0].id, 'behind-all', 'the group stands in front of it');
    assert.deepEqual(doc.items[1].items.map((/** @type {any} */ i) => i.id), ['g-back', 'g-front']);
    assert.deepEqual(Object.fromEntries(r.remap), { '0': '1', '1': '0', '0.0': '1.1', '0.1': '1.0' });
    assert.ok(r.moved.some((m) => m.group === true || m.object === 'pine'));
    const broken = { ...real(), items: [{ object: 'no-such-object', at: { tile: [1, 1, 0] } }, { object: 'pine', at: { tile: [0, 0, 0] } }] };
    const left = ops.sortScene(site, await sources(), broken);
    assert.deepEqual([left.moved, left.remap.size, broken.items[0].object], [[], 0, 'no-such-object']);
  });

  test('place: added last, on a tile or by pixel (the session then sorts it, lab place)', () => {
    const doc = scene();
    const r = ops.place(doc, 'pine', { tile: [0, 1, 0] });
    assert.deepEqual(r.path, [doc.items.length - 1]);
    assert.deepEqual(doc.items.at(-1), { object: 'pine', at: { tile: [0, 1, 0] } });
    assert.equal(r.message, 'pine placed at column 0, row 1, level 0');
    const p = ops.place(doc, 'rock', { px: [1, 2] });
    assert.deepEqual(p.path, [doc.items.length - 1]);
    assert.equal(p.message, 'rock placed at x 1, y 2');
    // A pixel item can stand on a level; level 0 is left out and not spoken.
    const high = ops.place(doc, 'rock', { px: [1, 2], level: 4 });
    assert.deepEqual(doc.items.at(-1).at, { px: [1, 2], level: 4 });
    assert.equal(high.message, 'rock placed at x 1, y 2, level 4');
    assert.deepEqual(ops.place(doc, 'rock', { px: [1, 2], level: 0 }).item.at, { px: [1, 2] });
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
    assert.deepEqual(px.at, { px: [10, 10], level: 1 }, 'a pixel item stands a level higher too, over the same tile');
    ops.moveByTiles(px, 0, 0, -1);
    assert.deepEqual(px.at, { px: [10, 26] }, 'back to level 0, which is left out');
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
    // A nudge keeps the level the item stands on: a tile item's, or a pixel item's own.
    const high = { object: 'x', at: { tile: [1, 0, 2] } };
    doc.items.push(high);
    ops.nudgeItem(site, doc, [doc.items.length - 1], 1, 0);
    assert.equal(high.at.level, 2);
    ops.nudgeItem(site, doc, [doc.items.length - 1], 1, 0);
    assert.equal(high.at.level, 2);
    assert.deepEqual(high.at.px, [site.iso.tileToPx([1, 0, 2], doc.origin)[0] + 2, site.iso.tileToPx([1, 0, 2], doc.origin)[1]]);
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
    // A pixel item has a level too, left out at 0.
    ops.setField(doc, [2], 'level', 4);
    assert.deepEqual(doc.items[2].at, { px: [10, 40], level: 4 });
    assert.equal(ops.setField(doc, [2], 'level', 0).message, 'c moved to x 10, y 40');
    assert.deepEqual(doc.items[2].at, { px: [10, 40] });
    assert.throws(() => ops.setField(doc, [2], 'col', 1), (err) => err.code === 'wrong_placement');
  });

  test('snapItem: to the tile under the placement point, at the item\'s level (lab snapSelected)', () => {
    const doc = scene();
    const point = site.iso.tileToPx([2, 1, 0], doc.origin);
    doc.items[2].at = { px: [point[0] + 3, point[1] - 2] };
    assert.equal(ops.snapItem(site, doc, [2]).message, 'c snapped to column 2, row 1, level 0');
    assert.deepEqual(doc.items[2].at, { tile: [2, 1, 0] });
    assert.throws(() => ops.snapItem(site, doc, [2]), /already on a tile/);
    // A pixel item snaps to the tile at the level it stands on.
    const high = site.iso.tileToPx([2, 1, 3], doc.origin);
    doc.items[2].at = { px: [high[0] + 3, high[1] - 2], level: 3 };
    assert.equal(ops.snapItem(site, doc, [2]).message, 'c snapped to column 2, row 1, level 3');
    assert.deepEqual(doc.items[2].at, { tile: [2, 1, 3] });
  });

  test('setDepth: front, back or auto on an item or a group, keeping the key order (replaces raise and lower)', () => {
    const doc = scene();
    doc.items[0].layer = 'l';
    doc.items[0].id = 'first';
    const r = ops.setDepth(doc, [0], 'front');
    assert.equal(r.message, 'a draws in front of everything else in its list');
    assert.deepEqual(Object.keys(doc.items[0]), ['object', 'id', 'layer', 'depth', 'at']);
    assert.equal(ops.setDepth(doc, [0], 'back').item.depth, 'back');
    assert.equal(ops.setDepth(doc, [0], 'auto').message, 'a draws by where it stands');
    assert.deepEqual(Object.keys(doc.items[0]), ['object', 'id', 'layer', 'at']);
    // A group takes it too, and writes it after the group's attributes.
    assert.equal(ops.setDepth(doc, [5], 'back').message, 'Group 5 draws behind everything else in its list');
    assert.deepEqual(Object.keys(doc.items[5]), ['group', 'depth', 'items']);
    ops.setDepth(doc, [5], 'auto');
    assert.deepEqual(Object.keys(doc.items[5]), ['group', 'items']);
    // And the rest of an item's edits keep the override in its place.
    ops.setDepth(doc, [5, 0], 'front');
    ops.setClass(doc, [5, 0], 'sway');
    assert.deepEqual(Object.keys(doc.items[5].items[0]), ['object', 'id', 'class', 'depth', 'at']);
    assert.throws(() => ops.setDepth(doc, [0], /** @type {any} */ ('forward')), (err) => err.code === 'usage');
    assert.throws(() => ops.setDepth(doc, [9], 'front'), (err) => err.code === 'not_found');
  });

  test('duplicateItem: one column over, listed right after it (the session sorts), without its id (lab duplicateSelected)', () => {
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
