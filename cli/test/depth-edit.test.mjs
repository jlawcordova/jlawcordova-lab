// Depth order in editing (docs/intents/2026-10-depth-order/spec.md, R4 and
// R6–R8, R11, R13): every scene edit leaves each list of items in depth
// order inside the same undo step, results name the paths after the sort,
// merges and reloads sort what they adopt, and the commands that came with
// depth (the override, a pixel item's level, scene.sort, footprints) work
// through the CLI, serve --stdio and MCP. On temp copies of the sample
// document with scenes of blocks that overlap on purpose.

import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { after, before, beforeEach, describe, test } from 'node:test';

import { openDocument } from '../lib/document.mjs';
import { registry } from '../lib/registry.mjs';
import { Session } from '../lib/session.mjs';
import { makeDocument, readDoc, tempDir } from './helpers.mjs';

// Four blocks side by side, listed back to front, and a pebble nothing overlaps.
const STAND = `export default {
  viewBox: [-48, -32, 128, 96],
  origin: [0, 0],
  items: [
    { object: 'block', id: 'a', at: { tile: [0, 0, 0] } },
    { object: 'block', id: 'b', at: { tile: [1, 0, 0] } },
    { object: 'block', id: 'c', at: { tile: [0, 1, 0] } },
    { object: 'block', id: 'd', at: { tile: [1, 1, 0] } },
    { object: 'pebble', id: 'far', at: { tile: [9, 9, 0] } },
  ],
};
`;

// A block in front of another that moves: its first key puts it somewhere else.
const MOVING = `export default {
  viewBox: [-48, -32, 128, 96],
  origin: [0, 0],
  items: [
    { object: 'block', id: 'a', at: { tile: [0, 0, 0] } },
    { object: 'block', id: 'cart', at: { tile: [0, 2, 0] } },
  ],
  motion: {
    duration: 1000,
    tracks: [
      { item: 'cart', keys: [{ t: 0 }, { t: 500, move: [16, 8] }] },
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
  doc = await makeDocument({ files: { 'scenes/stand.mjs': STAND, 'scenes/moving.mjs': MOVING } });
  opened = await openDocument({ doc: doc.root, env: doc.env });
});
after(() => doc.cleanup());
beforeEach(async () => {
  for (const [name, text] of [['stand', STAND], ['moving', MOVING]]) await writeFile(join(doc.root, `scenes/${name}.mjs`), text);
  state = await tempDir('pixellab-state-');
});

const owner = () => new Session(opened, { env: { ...process.env, PIXELLAB_STATE: state } });
const agent = () => new Session(opened, { env: { ...process.env, PIXELLAB_STATE: state }, mode: 'propose', agent: 'test' });
const call = (/** @type {Session} */ s, /** @type {string} */ name, /** @type {any} */ params) => registry.call(s.context(), name, { name: 'stand', ...params });
/** The scene's items back to front, by id (or object), as scene.items lists them. */
const order = async (/** @type {Session} */ s, name = 'stand') => (await call(s, 'scene.items', { name })).items.map((/** @type {any} */ i) => i.id ?? i.object);
/** Whether the working copy has every list in depth order, by the engine's own test. */
const inOrder = async (/** @type {Session} */ s, name = 'stand') => {
  const { sources } = await s.context().sources();
  return opened.engine.isInDepthOrder(sources, sources.scenes.get(name));
};
const file = (/** @type {string} */ name) => join(doc.root, `scenes/${name}.mjs`);

describe('R4: every edit keeps the list sorted', () => {
  test('R4: placing puts the item where it stands, and the path it returns is after the sort', async () => {
    const s = owner();
    const r = await call(s, 'scene.place', { object: 'block', at: [-1, -1, 0], id: 'e' });
    // e is behind a, b and c, which must wait for it; "far" overlaps nothing, so it is the first free item listed and the sort takes it first.
    assert.deepEqual(await order(s), ['far', 'e', 'a', 'b', 'c', 'd']);
    assert.equal(r.path, '1');
    assert.deepEqual(r.reordered, { '0': '2', '1': '3', '2': '4', '3': '5', '4': '0', '5': '1' }, 'every path the sort changed, old → new; e was added last');
    assert.equal(r.item.id, 'e');
    assert.equal(await inOrder(s), true);
    // Nothing it overlaps: it stays last and nothing is reordered.
    const near = await call(s, 'scene.place', { object: 'pebble', at: [8, 9, 0] });
    assert.deepEqual([near.path, near.reordered], ['6', undefined]);
  });

  test('R4: moving by command re-sorts; the result names the new path and what moved', async () => {
    const s = owner();
    const r = await call(s, 'scene.move', { item: 'd', by: [-2, -2] });
    assert.equal(r.path, '0');
    assert.deepEqual(await order(s), ['d', 'a', 'b', 'c', 'far']);
    assert.deepEqual(r.reordered, { '0': '1', '1': '2', '2': '3', '3': '0' });
    assert.match(r.summary, /^block moved to column -1, row -1, level 0$/);
    // Moving it back puts it back in its place.
    const back = await call(s, 'scene.move', { item: 'd', by: [2, 2] });
    assert.equal(back.path, '3');
    assert.deepEqual(await order(s), ['a', 'b', 'c', 'd', 'far']);
  });

  test('R4: moving by a layer (scene.layer.move) re-sorts and lists the moved items\' paths after the sort', async () => {
    const s = owner();
    await call(s, 'scene.layer.new', { layerName: 'Corner', items: ['d'] });
    const r = await call(s, 'scene.layer.move', { layer: 'corner', by: [-2, -2] });
    assert.deepEqual(r.paths, ['0']);
    assert.deepEqual(r.reordered, { '0': '1', '1': '2', '2': '3', '3': '0' });
    assert.deepEqual(await order(s), ['d', 'a', 'b', 'c', 'far']);
    assert.equal((await call(s, 'scene.items', {})).items[0].layer, 'corner');
  });

  test('R4: setting a field, a tile or a pixel point, snapping, duplicating and removing keep the order', async () => {
    const s = owner();
    assert.equal((await call(s, 'scene.set', { item: 'd', col: -1, row: -1 })).path, '0');
    assert.deepEqual(await order(s), ['d', 'a', 'b', 'c', 'far']);
    assert.equal((await call(s, 'scene.set', { item: 'd', tile: [1, 1, 0] })).path, '3');
    const px = await call(s, 'scene.set', { item: 'd', px: [0, -16] });
    assert.deepEqual(await order(s), ['d', 'a', 'b', 'c', 'far'], 'a block placed by pixel at the point of tile -1,-1');
    assert.equal(px.path, '0');
    const snapped = await call(s, 'scene.snap', { item: 'd' });
    assert.deepEqual([snapped.path, snapped.item.at], ['0', { tile: [-1, -1, 0] }]);
    const twin = await call(s, 'scene.duplicate', { item: 'd' });
    assert.equal((await call(s, 'scene.items', {})).items[Number(twin.path)].at.tile.join(), '0,-1,0');
    assert.equal(await inOrder(s), true);
    await call(s, 'scene.remove', { item: 'd' });
    assert.deepEqual(await order(s), ['block', 'a', 'b', 'c', 'far']);
    assert.equal(await inOrder(s), true);
  });

  test('R4: setting an override (scene.set --depth) re-sorts, and so does clearing it', async () => {
    const s = owner();
    const front = await call(s, 'scene.set', { item: 'a', depth: 'front' });
    assert.equal(front.path, '4');
    assert.deepEqual(await order(s), ['b', 'c', 'd', 'far', 'a']);
    assert.equal((await call(s, 'scene.items', {})).items.at(-1).depth, 'front');
    // Back where it stands, it is behind b and c again (and "far", free, comes first).
    const auto = await call(s, 'scene.set', { item: 'a', depth: 'auto' });
    assert.deepEqual(await order(s), ['far', 'a', 'b', 'c', 'd']);
    assert.equal(auto.path, '1');
    assert.equal((await call(s, 'scene.items', {})).items[1].depth, undefined);
  });

  test('R4: undo and redo give the edit and its sort back together, in one step', async () => {
    const s = owner();
    const ref = 'scene:stand';
    await call(s, 'scene.move', { item: 'd', by: [-2, -2] });
    assert.deepEqual(await order(s), ['d', 'a', 'b', 'c', 'far']);
    const undone = await registry.call(s.context(), 'doc.undo', { doc: ref });
    assert.equal(undone.dirty, false, 'one undo is the whole edit: back to the file');
    assert.deepEqual(await order(s), ['a', 'b', 'c', 'd', 'far']);
    await registry.call(s.context(), 'doc.redo', { doc: ref });
    assert.deepEqual(await order(s), ['d', 'a', 'b', 'c', 'far']);
    assert.equal(await inOrder(s), true);
  });

  test('R4: a motion edit that moves a track\'s first key re-sorts (the item rests somewhere else)', async () => {
    const s = owner();
    const call2 = (/** @type {string} */ name, /** @type {any} */ params) => call(s, name, { name: 'moving', ...params });
    assert.deepEqual(await order(s, 'moving'), ['a', 'cart']);
    // The cart rests at its first key: moved up the screen to tile -1,-1 it is behind block a.
    const r = await call2('scene.key.set', { item: 'cart', time: 0, move: [32, -32] });
    assert.deepEqual(await order(s, 'moving'), ['cart', 'a']);
    assert.equal(await inOrder(s, 'moving'), true);
    assert.deepEqual(r.reordered, { '0': '1', '1': '0' });
    // Another key doesn't change where it rests.
    await call2('scene.key.set', { item: 'cart', time: 500, move: [0, 0] });
    assert.deepEqual(await order(s, 'moving'), ['cart', 'a']);
  });

  test('R4: merging a change from disk into a dirty copy leaves it sorted', async () => {
    const s = owner();
    // The owner moves d behind everything (unsaved); meanwhile the file gets a new block at the front, on disk.
    await call(s, 'scene.move', { item: 'd', by: [-2, -2] });
    const other = STAND.replace("    { object: 'pebble'", "    { object: 'block', id: 'e', at: { tile: [2, 2, 0] } },\n    { object: 'pebble'");
    await writeFile(file('stand'), other);
    const merged = await registry.call(s.context(), 'doc.merge', { doc: 'scene:stand' });
    assert.equal(merged.merged, true);
    assert.deepEqual(await order(s), ['d', 'a', 'b', 'c', 'e', 'far']);
    assert.equal(await inOrder(s), true);
    assert.equal(merged.dirty, true, 'the owner\'s edit is still unsaved');
  });

  test('R4: a merge that brings an out-of-order list together comes out sorted, and says so', async () => {
    const s = owner();
    await call(s, 'scene.place', { object: 'pebble', at: [8, 9, 0], id: 'near' });
    // The file, edited by hand and merged by git, now lists d before the blocks it stands in front of.
    await writeFile(file('stand'), STAND.replace("    { object: 'block', id: 'd', at: { tile: [1, 1, 0] } },\n", '').replace("  items: [\n", "  items: [\n    { object: 'block', id: 'd', at: { tile: [1, 1, 0] } },\n"));
    const merged = await registry.call(s.context(), 'doc.merge', { doc: 'scene:stand' });
    assert.equal(merged.merged, true);
    assert.equal(merged.sorted, 1);
    assert.match(merged.summary, /the result was sorted by depth/);
    assert.deepEqual(await order(s), ['a', 'b', 'c', 'd', 'far', 'near']);
  });

  test('R4: a file found out of order on reload is sorted as a step of its own: unsaved, and one undo gives the file back', async () => {
    const s = owner();
    await call(s, 'doc.open', { doc: 'scene:stand' }).catch(() => registry.call(s.context(), 'doc.open', { doc: 'scene:stand' }));
    const unsorted = STAND.replace("    { object: 'block', id: 'd', at: { tile: [1, 1, 0] } },\n", '').replace("  items: [\n", "  items: [\n    { object: 'block', id: 'd', at: { tile: [1, 1, 0] } },\n");
    await writeFile(file('stand'), unsorted);
    const reloaded = await registry.call(s.context(), 'doc.reload', { doc: 'scene:stand' });
    assert.deepEqual([reloaded.reloaded, reloaded.sorted, reloaded.dirty, reloaded.canUndo], [true, 1, true, true]);
    assert.match(reloaded.summary, /wasn't in depth order, so stand was sorted/);
    assert.deepEqual(await order(s), ['a', 'b', 'c', 'd', 'far']);
    const status = await registry.call(s.context(), 'doc.status', { doc: 'scene:stand' });
    assert.deepEqual(status.edits, ['Sorted stand by depth']);
    const undone = await registry.call(s.context(), 'doc.undo', { doc: 'scene:stand' });
    assert.equal(undone.dirty, false);
    assert.deepEqual(await order(s), ['d', 'a', 'b', 'c', 'far'], 'exactly the file');
  });

  test('R4: a sorted file reloads as it is, with no extra step', async () => {
    const s = owner();
    await registry.call(s.context(), 'doc.open', { doc: 'scene:stand' });
    await writeFile(file('stand'), STAND.replace("tile: [9, 9, 0]", "tile: [8, 9, 0]"));
    const reloaded = await registry.call(s.context(), 'doc.reload', { doc: 'scene:stand' });
    assert.deepEqual([reloaded.reloaded, reloaded.sorted, reloaded.dirty, reloaded.canUndo], [true, undefined, false, false]);
  });

  test('R4: a proposal approved into a copy with unsaved edits merges and leaves it sorted', async () => {
    const a = agent();
    const o = owner();
    // The owner has an unsaved edit; the agent proposes a different one, and the owner approves it.
    await call(o, 'scene.move', { item: 'c', by: [0, -2] });
    await a.sync();
    await call(a, 'scene.place', { object: 'block', at: [-1, -1, 0], id: 'e' });
    const proposed = await registry.call(a.context(), 'doc.save', { doc: 'scene:stand' });
    const applied = await registry.call(o.context(), 'proposal.apply', { id: proposed.proposal });
    assert.equal(applied.merged, true);
    assert.equal(await inOrder(o), true);
    const ids = await order(o);
    assert.ok(ids.indexOf('e') < ids.indexOf('a'), 'the proposed block stands behind a');
    const text = await readFile(file('stand'), 'utf8');
    assert.ok(text.indexOf("id: 'e'") < text.indexOf("id: 'a'"), 'and the file the proposal wrote lists it so');
    assert.equal((await readDoc(file('stand'))).items.length, 6);
  });
});
