// Groups over the MCP (docs/intents/2026-10-mcp-gaps/spec.md, R9–R11): a
// group's data-x/data-y and data-col/data-row follow the item they point at,
// scene.group.set edits a group's attributes, and scene.place places inside
// an existing or a new group. On a temp copy of the sample document, with a
// scene of entrance groups like the hero island's.

import assert from 'node:assert/strict';
import { after, before, describe, test } from 'node:test';

import { openDocument } from '../lib/document.mjs';
import { registry } from '../lib/registry.mjs';
import { Session } from '../lib/session.mjs';
import { makeDocument, tempDir } from './helpers.mjs';

// A 4×4 sprite anchored at its top-left corner, so its foot isn't its placement point.
const STONE = `export default {
  kind: 'sprite',
  anchor: [0, 0],
  keys: { a: 'ink' },
  layers: [{ map: ['aaaa', 'aaaa', 'aaaa', 'aaaa'] }],
};
`;

const GROUPS = `export default {
  viewBox: [-32, -16, 96, 64],
  items: [
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '1', 'data-row': '0', 'data-x': '16', 'data-y': '8' },
      items: [
        { object: 'block', at: { tile: [1, 0, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '42', 'data-y': '13' },
      items: [
        { object: 'stone', id: 'rock', at: { px: [40, 10] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'object', 'data-x': '99', 'data-y': '99' },
      items: [
        { object: 'pebble', at: { px: [8, 30] } },
      ],
    },
    { object: 'pebble', at: { tile: [3, 3, 0] } },
  ],
  motion: {
    duration: 1000,
    tracks: [
      { item: 'rock', keys: [{ t: 0 }] },
    ],
  },
};
`;

/** @type {Awaited<ReturnType<typeof makeDocument>>} */
let doc;
/** @type {any} */
let opened;
/** @type {string} */
let state;
before(async () => {
  doc = await makeDocument({ files: { 'objects/stone.mjs': STONE, 'scenes/groups.mjs': GROUPS } });
  opened = await openDocument({ doc: doc.root, env: doc.env });
  state = await tempDir('pixellab-state-');
});
after(() => doc.cleanup());

const session = () => new Session(opened, { env: { ...process.env, PIXELLAB_STATE: state } });
const call = (/** @type {Session} */ s, /** @type {string} */ name, /** @type {any} */ params) => registry.call(s.context(), name, { name: 'groups', ...params });
const groups = async (/** @type {Session} */ s) => (await call(s, 'scene.items', {})).groups.map((/** @type {any} */ g) => g.attrs);

describe('group attributes follow the item (R9)', () => {
  test('R9: a cube moved by tiles takes its ground group\'s data-col, data-row, data-x and data-y with it', async () => {
    const s = session();
    const r = await call(s, 'scene.move', { item: '0.0', by: [1, 0] });
    assert.deepEqual(r.group, { path: '0', attrs: { class: 'enter', 'data-enter': 'ground', 'data-col': '2', 'data-row': '0', 'data-x': '32', 'data-y': '16' } });
    assert.match(r.summary, /its group's data-col, data-row, data-x, data-y followed$/);
  });

  test("R9: a sprite placed by pixel takes its group's foot with it; scene.set and scene.snap follow too", async () => {
    const s = session();
    const nudged = await call(s, 'scene.move', { item: 'rock', nudge: [1, -1] });
    assert.deepEqual([nudged.group.attrs['data-x'], nudged.group.attrs['data-y']], ['43', '12'], 'the foot: x − 0 + 4/2, y − 0 + 4 − 1');
    const set = await call(s, 'scene.set', { item: 'rock', px: [10, 20] });
    assert.deepEqual([set.group.attrs['data-x'], set.group.attrs['data-y']], ['12', '23']);
    const snapped = await call(s, 'scene.snap', { item: 'rock' });
    assert.ok(snapped.group, 'snapping moves the foot to the tile center, and the group follows');
    const tile = snapped.item.at.tile;
    assert.deepEqual([snapped.group.attrs['data-x'], snapped.group.attrs['data-y']], [String((tile[0] - tile[1]) * 16), String((tile[0] + tile[1]) * 8)]);
  });

  test('R9: a group that points elsewhere, --no-keep-group and a keyed move leave the attributes alone', async () => {
    const s = session();
    const other = await call(s, 'scene.move', { item: '2.0', nudge: [1, 0] });
    assert.equal(other.group, undefined);
    const off = await call(s, 'scene.move', { item: '0.0', by: [0, 1], keepGroup: false });
    assert.equal(off.group, undefined);
    assert.equal((await groups(s))[0]['data-row'], '0');
    const keyed = await call(s, 'scene.move', { item: 'rock', nudge: [2, 0], time: 500 });
    assert.equal(keyed.group, undefined);
    assert.equal((await groups(s))[1]['data-x'], '42');
  });
});

describe('scene.group.set (R10)', () => {
  test('R10: sets and removes attributes, by group path or by an item inside it', async () => {
    const s = session();
    const r = await call(s, 'scene.group.set', { group: '2', attrs: { 'data-enter': 'sky', 'data-x': '' } });
    assert.deepEqual(r.group, { path: '2', attrs: { class: 'enter', 'data-enter': 'sky', 'data-y': '99' } });
    const byItem = await call(s, 'scene.group.set', { group: 'rock', attrs: { 'data-wave': '2' } });
    assert.equal(byItem.group.path, '1');
    assert.equal(byItem.group.attrs['data-wave'], '2');
  });

  test("R10: --foot sets data-x and data-y to the first item's foot", async () => {
    const s = session();
    const r = await call(s, 'scene.group.set', { group: '2', foot: true });
    assert.deepEqual([r.group.attrs['data-x'], r.group.attrs['data-y']], ['8', '30'], "the pebble's anchor is its foot");
  });

  test('R10: refuses attributes the engine would, a top-level item and a call with nothing to set', async () => {
    const s = session();
    await assert.rejects(call(s, 'scene.group.set', { group: '0', attrs: { style: 'x' } }), /class or data-\*/);
    await assert.rejects(call(s, 'scene.group.set', { group: '0', attrs: { 'data-x': 'a"b' } }), /without " < > &/);
    await assert.rejects(call(s, 'scene.group.set', { group: '3', foot: true }), { code: 'not_in_group' });
    await assert.rejects(call(s, 'scene.group.set', { group: '0' }), { code: 'usage' });
  });
});

describe('scene.place into groups (R11)', () => {
  test('R11: into an existing group, where it stands among that group\'s items', async () => {
    const s = session();
    const r = await call(s, 'scene.place', { object: 'pebble', at: [1, 0, 1], group: '0' });
    assert.deepEqual([r.path, r.group.path], ['0.1', '0'], 'nothing it overlaps: it joins the end');
    // Below the cube it is behind it: it is listed first, and the cube moves to second.
    const behind = await call(session(), 'scene.place', { object: 'pebble', at: [1, 0, -1], group: '0' });
    assert.deepEqual([behind.path, behind.group.path], ['0.0', '0']);
    assert.equal(behind.reordered['0.0'], '0.1', 'the cube moved down one');
    // Into the group around the rock, with nothing it overlaps: it joins last, and the rock keeps its place.
    const fresh = session();
    const far = await call(fresh, 'scene.place', { object: 'pebble', px: [5, 5], group: 'rock' });
    assert.equal(far.path, '1.1');
    assert.equal((await call(fresh, 'scene.items', {})).items.find((/** @type {any} */ i) => i.id === 'rock').path, '1.0');
  });

  test('R11: into a new group with its foot, the group goes where its item stands among the groups', async () => {
    const s = session();
    const r = await call(s, 'scene.place', { object: 'stone', px: [0, 0], newGroup: { class: 'enter', 'data-enter': 'object' }, foot: true });
    assert.deepEqual(r.group.attrs, { class: 'enter', 'data-enter': 'object', 'data-x': '2', 'data-y': '3' });
    assert.equal(r.path, `${r.group.path}.0`);
    const stone = (await call(s, 'scene.items', {})).items.find((/** @type {any} */ i) => i.group === r.group.path);
    assert.equal(stone.path, r.path, 'the path it returns is where it is after the sort');
    // On a tile, a new group goes where its item stands: the cube at 0,0,0 is behind the cube at 1,0,0, so its group is listed first.
    const t = await call(s, 'scene.place', { object: 'block', at: [0, 0, 0], newGroup: { 'data-enter': 'ground' } });
    const items = (await call(s, 'scene.items', {})).items;
    const [new0, cube1] = [items.find((/** @type {any} */ i) => i.path === t.path), items.find((/** @type {any} */ i) => i.object === 'block' && i.at.tile.join() === '1,0,0')];
    assert.ok(Number(new0.path.split('.')[0]) < Number(cube1.path.split('.')[0]), 'before the group whose cube is at 1,0,0');
    const nested = await call(s, 'scene.place', { object: 'pebble', at: [1, 0, 1], group: t.group.path, newGroup: { 'data-part': 'top' } });
    assert.equal(nested.path.split('.').length, 3);
  });

  test('R6, R11: a top-level item goes where it stands (--index is gone), and the refusals', async () => {
    const s = session();
    const r = await call(s, 'scene.place', { object: 'pebble', at: [9, 9, 0] });
    assert.equal(r.path, '4', 'nothing overlaps it, so it joins the end of the list');
    await assert.rejects(call(s, 'scene.place', { object: 'pebble', at: [9, 9, 0], index: 0 }), /unknown param index/);
    await assert.rejects(call(s, 'scene.place', { object: 'pebble', at: [9, 9, 0], group: '4' }), { code: 'not_in_group' });
    await assert.rejects(call(s, 'scene.place', { object: 'pebble', at: [9, 9, 0], newGroup: { onclick: 'x' } }), /class or data-\*/);
    await assert.rejects(call(s, 'scene.place', { object: 'pebble', at: [9, 9, 0], foot: true }), { code: 'usage' });
  });
});
