// Depth order in editing (docs/intents/2026-10-depth-order/spec.md, R4 and
// R6–R8, R11, R13): every scene edit leaves each list of items in depth
// order inside the same undo step, results name the paths after the sort,
// merges and reloads sort what they adopt, and the commands that came with
// depth (the override, a pixel item's level, scene.sort, footprints) work
// through the CLI, serve --stdio and MCP. On temp copies of the sample
// document with scenes of blocks that overlap on purpose.

import assert from 'node:assert/strict';
import { cp, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { after, before, beforeEach, describe, test } from 'node:test';

import { openDocument } from '../lib/document.mjs';
import { registry } from '../lib/registry.mjs';
import { Session } from '../lib/session.mjs';
import { decodePng, makeDocument, readDoc, REPO, runCli, startServer, tempDir } from './helpers.mjs';

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

// A group around a block that stands in front of the block beside it.
const GROUPED = `export default {
  viewBox: [-48, -32, 128, 96],
  items: [
    { group: { class: 'enter' }, items: [{ object: 'block', id: 'x', at: { tile: [1, 1, 0] } }] },
    { object: 'block', id: 'y', at: { tile: [0, 0, 0] } },
  ],
};
`;

// STAND written out of order: d is listed first, so drawn first, under the blocks it stands in front of.
const TANGLED = STAND.replace("    { object: 'block', id: 'd', at: { tile: [1, 1, 0] } },\n", '').replace('  items: [\n', "  items: [\n    { object: 'block', id: 'd', at: { tile: [1, 1, 0] } },\n");

/** @type {Awaited<ReturnType<typeof makeDocument>>} */
let doc;
/** @type {any} */
let opened;
/** @type {string} */
let state;

before(async () => {
  doc = await makeDocument({ files: { 'scenes/stand.mjs': STAND, 'scenes/moving.mjs': MOVING, 'scenes/tangled.mjs': TANGLED, 'scenes/grouped.mjs': GROUPED } });
  opened = await openDocument({ doc: doc.root, env: doc.env });
});
after(() => doc.cleanup());
beforeEach(async () => {
  for (const [name, text] of [['stand', STAND], ['moving', MOVING], ['tangled', TANGLED], ['grouped', GROUPED]]) await writeFile(join(doc.root, `scenes/${name}.mjs`), text);
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
    // e is behind a, b and c, which would wait for it; the sort moves e (one item) up in front of them and leaves "far" where it was.
    assert.deepEqual(await order(s), ['e', 'a', 'b', 'c', 'd', 'far']);
    assert.equal(r.path, '0');
    assert.deepEqual(r.reordered, { '0': '1', '1': '2', '2': '3', '3': '4', '4': '5', '5': '0' }, 'every path the sort changed, old → new; e was added last');
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
    // Back where it stands, it is behind b and c again: it alone moves back up, past them.
    const auto = await call(s, 'scene.set', { item: 'a', depth: 'auto' });
    assert.deepEqual(await order(s), ['a', 'b', 'c', 'd', 'far']);
    assert.equal(auto.path, '0');
    assert.equal((await call(s, 'scene.items', {})).items[0].depth, undefined);
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

  test('R4: undo of a sorting edit says where items went (and redo says it back)', async () => {
    const s = owner();
    const ref = 'scene:stand';
    await call(s, 'scene.move', { item: 'd', by: [-2, -2] });
    // d went to the front of the list, so a, b and c are one later; undo puts d back at 3 and the rest a place earlier.
    const undone = await registry.call(s.context(), 'doc.undo', { doc: ref });
    assert.deepEqual(undone.reordered, { '0': '3', '1': '0', '2': '1', '3': '2' }, 'old path → new path, d included though its placement changed');
    const redone = await registry.call(s.context(), 'doc.redo', { doc: ref });
    assert.deepEqual(redone.reordered, { '3': '0', '0': '1', '1': '2', '2': '3' });
    // A step that moves no paths says nothing.
    await call(s, 'scene.set', { item: 'a', class: 'x' });
    assert.equal((await registry.call(s.context(), 'doc.undo', { doc: ref })).reordered, undefined);
  });

  test('R4: undo of a sorting edit moves a group with its first item', async () => {
    const s = owner();
    // The group (x at 1,1) is listed before y (0,0), which it stands in front of: the first edit sorts the scene.
    const r = await call(s, 'scene.set', { name: 'grouped', item: '1', class: 'z' });
    assert.deepEqual(r.reordered, { '0': '1', '0.0': '1.0', '1': '0' });
    const undone = await registry.call(s.context(), 'doc.undo', { doc: 'scene:grouped' });
    assert.deepEqual(undone.reordered, { '1': '0', '1.0': '0.0', '0': '1' }, 'the group and its member, old → new');
  });

  test('R4: a motion edit that moves a track\'s first key re-sorts (the item rests somewhere else)', async () => {
    const s = owner();
    const call2 = (/** @type {string} */ name, /** @type {any} */ params) => call(s, name, { name: 'moving', ...params });
    assert.deepEqual(await order(s, 'moving'), ['a', 'cart']);
    // The cart rests at its first key: moved up the screen to tile -1,-1 it is behind block a.
    const r = await call2('scene.key.set', { item: 'cart', time: 0, move: [32, -32] });
    assert.deepEqual(await order(s, 'moving'), ['cart', 'a']);
    assert.equal(await inOrder(s, 'moving'), true);
    assert.equal(r.reordered['0'], '1');
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

  test('R4: two edits of a sorted scene, one of which moved an item, still merge; the same item changed both ways clashes', async () => {
    const s = owner();
    await call(s, 'scene.move', { item: 'd', by: [-2, -2] });
    // On disk someone else gave block a a class: a different line from the ones the move touched.
    await writeFile(file('stand'), STAND.replace("id: 'a'", "id: 'a', class: 'sway'"));
    const merged = await registry.call(s.context(), 'doc.merge', { doc: 'scene:stand' });
    assert.deepEqual([merged.merged, merged.sorted], [true, undefined], 'nothing needed sorting again');
    assert.deepEqual(await order(s), ['d', 'a', 'b', 'c', 'far']);
    assert.equal((await call(s, 'scene.items', {})).items.find((/** @type {any} */ i) => i.id === 'a').class, 'sway');
    // Both sides changed d's own line: that is a clash, as before.
    const t = owner();
    await call(t, 'scene.move', { item: 'd', by: [-2, -2] });
    await writeFile(file('stand'), STAND.replace("id: 'd', at: { tile: [1, 1, 0] }", "id: 'd', at: { tile: [3, 3, 0] }"));
    await assert.rejects(registry.call(t.context(), 'doc.merge', { doc: 'scene:stand' }), { code: 'merge_conflict' });
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

describe('R6: Raise and Lower are gone; the override is the one manual control', () => {
  test('R6: scene.place has no --index, and the override puts an item over or under everything in its list', async () => {
    const s = owner();
    await assert.rejects(call(s, 'scene.place', { object: 'pebble', at: [8, 9, 0], index: 0 }), /unknown param index/);
    // Back: d stands in front of a, b and c, but draws before all of them.
    await call(s, 'scene.set', { item: 'd', depth: 'back' });
    assert.equal((await order(s))[0], 'd');
    // Front on a, which stands behind b, c and d: it draws over them.
    const front = await call(s, 'scene.set', { item: 'a', depth: 'front' });
    assert.equal((await order(s)).at(-1), 'a');
    assert.equal(front.item.depth, 'front');
    const items = (await call(s, 'scene.items', {})).items;
    assert.deepEqual([items[0].depth, items.at(-1).depth], ['back', 'front']);
  });

  test('R6: the override takes a group by its path, and no other field; a group\'s attributes are scene.group.set', async () => {
    const s = owner();
    const g = (/** @type {any} */ params) => call(s, 'scene.set', { name: 'grouped', ...params });
    const r = await g({ item: '0', depth: 'back' });
    assert.equal(r.item.depth, 'back');
    assert.deepEqual(Object.keys(r.item), ['group', 'depth', 'items']);
    assert.equal(r.path, '0', 'the group stands in front of y, but it draws first');
    assert.equal(r.item.items[0].id, 'x');
    const items = (await call(s, 'scene.items', { name: 'grouped' }));
    assert.deepEqual(items.groups.map((/** @type {any} */ x) => [x.path, x.depth]), [['0', 'back']]);
    await assert.rejects(g({ item: '0', depth: 'front', class: 'x' }), { code: 'usage', message: /is a group; only --depth applies to one \(its attributes are scene.group.set\)/ });
    await assert.rejects(g({ item: '0', col: 2 }), { code: 'usage', message: /scene.group.set/ });
    await assert.rejects(g({ item: '0', id: 'q' }), { code: 'usage' });
    await assert.rejects(g({ item: '0', depth: 'sideways' }), /depth/);
  });

  test('R6: a group\'s summary names it by its path after the sort, the one the result returns', async () => {
    const s = owner();
    const g = (/** @type {any} */ params) => call(s, 'scene.set', { name: 'grouped', ...params });
    await g({ item: '0', depth: 'back' });
    // Clearing the override lets the group stand in front of y again, so the sort moves it from path 0 to path 1.
    const r = await g({ item: '0', depth: 'auto' });
    assert.equal(r.path, '1');
    assert.equal(r.reordered['0'], '1');
    assert.equal(r.summary, 'Group 1 draws by where it stands');
    assert.equal((await registry.call(s.context(), 'doc.undo', { doc: 'scene:grouped' })).summary, 'Undid: Group 1 draws by where it stands');
  });
});

describe('R7: a pixel item has a level', () => {
  test('R7: scene.place --px --level, scene.set --level and scene.items show it; level 0 is not written', async () => {
    const s = owner();
    const placed = await call(s, 'scene.place', { object: 'pebble', px: [40, -12], level: 4, id: 'jib' });
    assert.deepEqual(placed.item.at, { px: [40, -12], level: 4 });
    assert.equal(placed.summary, 'pebble placed at x 40, y -12, level 4');
    const listed = (await call(s, 'scene.items', {})).items.find((/** @type {any} */ i) => i.id === 'jib');
    assert.deepEqual([listed.level, listed.at], [4, { px: [40, -12], level: 4 }]);
    const set = await call(s, 'scene.set', { item: 'jib', level: 2 });
    assert.deepEqual(set.item.at, { px: [40, -12], level: 2 });
    assert.equal(set.summary, 'pebble moved to x 40, y -12, level 2');
    const flat = await call(s, 'scene.set', { item: 'jib', level: 0 });
    assert.deepEqual(flat.item.at, { px: [40, -12] });
    const tile = await call(s, 'scene.place', { object: 'pebble', at: [3, 3, 1] });
    assert.deepEqual((await call(s, 'scene.items', {})).items.find((/** @type {any} */ i) => i.path === tile.path).level, undefined, 'a tile item\'s level is its tile\'s third number');
    await assert.rejects(call(s, 'scene.place', { object: 'pebble', at: [3, 3, 1], level: 2 }), { code: 'usage', message: /--level goes with --px/ });
  });

  test('R7: scene.move --by moves a pixel item\'s level with it; --nudge and --px keep it; scene.snap snaps at it', async () => {
    const s = owner();
    await call(s, 'scene.place', { object: 'pebble', px: [40, -12], level: 4, id: 'jib' });
    const up = await call(s, 'scene.move', { item: 'jib', by: [0, 0, 1] });
    assert.deepEqual(up.item.at, { px: [40, -28], level: 5 }, 'a level higher over the same tile: 16 pixels up');
    const down = await call(s, 'scene.move', { item: 'jib', by: [1, 0, -1] });
    assert.deepEqual(down.item.at, { px: [40 + 16, -28 + 8 + 16], level: 4 });
    const nudged = await call(s, 'scene.move', { item: 'jib', nudge: [1, 1] });
    assert.deepEqual(nudged.item.at, { px: [57, -3], level: 4 });
    const placed = await call(s, 'scene.set', { item: 'jib', px: [10, 10] });
    assert.deepEqual(placed.item.at, { px: [10, 10], level: 4 }, 'it still stands on level 4');
    const snapped = await call(s, 'scene.snap', { item: 'jib' });
    assert.equal(snapped.item.at.tile[2], 4, 'snapped to the tile at its own level');
    assert.equal(snapped.summary.endsWith('level 4'), true);
    // A tile item nudged becomes a pixel item that keeps the level of its tile.
    const lifted = await call(s, 'scene.place', { object: 'pebble', at: [5, 5, 2], id: 'lifted' });
    const nudge = await call(s, 'scene.move', { item: 'lifted', nudge: [1, 0] });
    assert.equal(nudge.item.at.level, 2);
    assert.equal(lifted.item.at.tile[2], 2);
  });
});

describe('R8: sprites can have a footprint', () => {
  test('R8: object.footprint sets and clears it, after the anchor in the file; blocks refuse', async () => {
    const s = owner();
    const oc = (/** @type {string} */ name, /** @type {any} */ params) => registry.call(s.context(), name, params);
    const r = await oc('object.footprint', { name: 'pebble', to: [2, 1] });
    assert.deepEqual(r.footprint, [2, 1]);
    assert.equal(r.summary, 'Footprint of pebble set to 2×1');
    await oc('doc.save', { doc: 'object:pebble' });
    const text = await readFile(join(doc.root, 'objects/pebble.mjs'), 'utf8');
    assert.ok(text.indexOf('anchor') < text.indexOf('footprint: [2, 1]') && text.indexOf('footprint') < text.indexOf('keys'), 'after the anchor, before the keys');
    const shown = await oc('object.show', { name: 'pebble' });
    assert.deepEqual(shown.footprint, [2, 1]);
    assert.match((await registry.get('object.show')?.format?.(shown, {})) ?? '', /^footprint {1}2×1 tiles$/m);
    const cleared = await oc('object.footprint', { name: 'pebble', clear: true });
    assert.equal(cleared.footprint, null);
    assert.deepEqual((await oc('object.show', { name: 'pebble' })).footprint, [1, 1]);
    await assert.rejects(oc('object.footprint', { name: 'block', to: [2, 2] }), { code: 'usage', message: /its size is its footprint/ });
    await assert.rejects(oc('object.footprint', { name: 'pebble' }), /give --to or --clear/);
    await assert.rejects(oc('object.footprint', { name: 'pebble', to: [0, 1] }), { code: 'usage' });
    await assert.rejects(oc('object.footprint', { name: 'pebble', to: [17, 1] }), { code: 'usage' });
    await assert.rejects(oc('object.footprint', { name: 'pebble', to: [2, 1], clear: true }), /only one of/);
    assert.equal((await oc('object.show', { name: 'block' })).footprint, undefined, 'a block\'s size is its footprint');
  });

  test('R8: object.new --footprint gives a new sprite its footprint; a block refuses it', async () => {
    const s = owner();
    const oc = (/** @type {string} */ name, /** @type {any} */ params) => registry.call(s.context(), name, params);
    const made = await oc('object.new', { name: 'shed', width: 12, height: 8, footprint: [2, 1] });
    assert.equal(made.changed, true);
    assert.deepEqual((await oc('object.show', { name: 'shed' })).footprint, [2, 1]);
    await assert.rejects(oc('object.new', { name: 'slab', kind: 'block', footprint: [2, 1] }), { code: 'usage', message: /size is its footprint/ });
    const extended = await oc('object.new', { name: 'big-shed', extends: 'shed', footprint: [3, 2] });
    assert.equal(extended.changed, true);
    assert.deepEqual((await oc('object.show', { name: 'big-shed' })).footprint, [3, 2]);
  });

  test('R8: an object that extends a sprite has its footprint, and a footprint changes where its scene sorts', async () => {
    const s = owner();
    const oc = (/** @type {string} */ name, /** @type {any} */ params) => registry.call(s.context(), name, params);
    await oc('object.new', { name: 'shed', width: 12, height: 8 });
    await oc('object.footprint', { name: 'shed', to: [4, 4] });
    await oc('object.new', { name: 'shed-two', extends: 'shed' });
    assert.deepEqual((await oc('object.show', { name: 'shed-two' })).footprint, [4, 4]);
  });
});

describe('R11: scene.sort', () => {
  /** How many pixels differ at time 0 between a scene drawn in its listed order and in depth order, counted here from the engine. */
  async function pixelsThatChange(/** @type {Session} */ s, /** @type {string} */ name) {
    const { sources } = await s.context().sources();
    const draw = (/** @type {'list' | 'depth'} */ o) => opened.engine.composite(opened.engine.renderScene(sources, name, { order: o }).root, { firstVariant: true, time: 0 });
    const [was, now] = [draw('list'), draw('depth')];
    return [...now].filter(([p, c]) => was.get(p) !== c).length + [...was.keys()].filter((p) => !now.has(p)).length;
  }

  test('R11: sorts one scene as one undo step and says what moved and how many pixels change', async () => {
    const s = owner();
    const expected = await pixelsThatChange(s, 'tangled');
    assert.ok(expected > 0, 'drawn in list order, d paints under the blocks it stands in front of');
    const r = await call(s, 'scene.sort', { name: 'tangled' });
    assert.deepEqual(r.moved, [{ scene: 'tangled', path: '0', to: '3', object: 'block' }]);
    assert.equal(r.changedPixels, expected);
    assert.deepEqual(r.scenes, [{ scene: 'tangled', moved: 1, changedPixels: expected }]);
    assert.deepEqual(r.reordered, { '0': '3', '1': '0', '2': '1', '3': '2' });
    assert.equal(r.changed, true);
    assert.match(r.summary, /^Sorted tangled by depth: 1 item moved, \d+ pixels? changes?$/);
    assert.deepEqual(await order(s, 'tangled'), ['a', 'b', 'c', 'd', 'far']);
    assert.equal(await inOrder(s, 'tangled'), true);
    const undone = await registry.call(s.context(), 'doc.undo', { doc: 'scene:tangled' });
    assert.equal(undone.dirty, false, 'one undo gives the file back');
    assert.deepEqual(await order(s, 'tangled'), ['d', 'a', 'b', 'c', 'far']);
  });

  test('R11: --all sorts every scene; a sorted scene is nothing to do', async () => {
    const s = owner();
    const all = await registry.call(s.context(), 'scene.sort', { all: true });
    assert.equal(all.changed, true);
    // tangled is listed out of order; grouped is (its group stands in front of y but is listed first).
    assert.deepEqual(all.scenes.filter((/** @type {any} */ x) => x.moved).map((/** @type {any} */ x) => x.scene), ['grouped', 'tangled']);
    assert.equal(all.scenes.length, (await registry.call(s.context(), 'scene.list', {})).scenes.length);
    assert.deepEqual([...new Set(all.moved.map((/** @type {any} */ m) => m.scene))], ['grouped', 'tangled']);
    assert.equal(all.reordered, undefined, 'paths are for one scene');
    const again = await registry.call(s.context(), 'scene.sort', { all: true });
    assert.deepEqual([again.changed, again.moved, again.changedPixels], [false, [], 0]);
    assert.equal(again.summary, 'Already in depth order');
    const one = await call(s, 'scene.sort', { name: 'stand' });
    assert.deepEqual([one.changed, one.moved, one.summary], [false, [], 'Already in depth order: stand']);
    await assert.rejects(call(s, 'scene.sort', { name: undefined }), { code: 'usage', message: /give a scene, or --all/ });
    await assert.rejects(call(s, 'scene.sort', { all: true }), { code: 'usage', message: /not both/ });
  });

  test('R11: the CLI\'s --dry-run shows the new order and writes nothing', async () => {
    const before = await readFile(file('tangled'), 'utf8');
    const r = await runCli(['scene', 'sort', 'tangled', '--dry-run', '--document', doc.root], { env: doc.env({ PIXELLAB_STATE: state }) });
    assert.equal(r.code, 0, r.stderr);
    assert.match(r.stdout, /^-    \{ object: 'block', id: 'd', at: \{ tile: \[1, 1, 0\] \} \},$/m);
    assert.match(r.stdout, /dry run: nothing written \(scenes\/tangled\.mjs would change\)/);
    assert.match(r.stdout, /^Sorted tangled by depth: 1 item moved, \d+ pixels? changes?$/m);
    assert.equal(await readFile(file('tangled'), 'utf8'), before);
    const real = await runCli(['scene', 'sort', 'tangled', '--document', doc.root], { env: doc.env({ PIXELLAB_STATE: state }) });
    assert.equal(real.code, 0, real.stderr);
    assert.deepEqual((await readDoc(file('tangled'))).items.map((/** @type {any} */ i) => i.id), ['a', 'b', 'c', 'd', 'far']);
    assert.equal((await runCli(['scene', 'sort', 'tangled', '--document', doc.root], { env: doc.env({ PIXELLAB_STATE: state }) })).stdout.startsWith('Already in depth order: tangled'), true);
  });
});

describe('R13: every new action is a command on all three doors', () => {
  test('R13: through serve --stdio (what the app uses)', async () => {
    const server = startServer(['--stdio', '--document', doc.root], { env: doc.env({ PIXELLAB_STATE: state }) });
    try {
      const sorted = (await server.request('scene.sort', { name: 'tangled' })).result;
      assert.deepEqual([sorted.moved.length, sorted.reordered['0']], [1, '3']);
      const moved = (await server.request('scene.move', { name: 'stand', item: 'd', by: [-2, -2] })).result;
      assert.deepEqual([moved.path, moved.reordered['3']], ['0', '0']);
      const depth = (await server.request('scene.set', { name: 'stand', item: 'a', depth: 'front' })).result;
      assert.equal(depth.path, '4');
      const level = (await server.request('scene.place', { name: 'stand', object: 'pebble', px: [5, 5], level: 3 })).result;
      assert.deepEqual(level.item.at, { px: [5, 5], level: 3 });
      const fp = (await server.request('object.footprint', { name: 'pebble', to: [2, 2] })).result;
      assert.deepEqual(fp.footprint, [2, 2]);
      const gone = await server.request('scene.raise', { name: 'stand', item: 'a' });
      assert.equal(gone.error.data.code, 'not_found');
    } finally {
      server.child.kill();
    }
  });

  test('R13: through MCP: the tools exist, raise and lower do not, and an edit becomes a proposal', async () => {
    const server = startServer(['--mcp', '--document', doc.root], { env: doc.env({ PIXELLAB_STATE: state }) });
    try {
      const init = await server.request('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'test-agent', version: '1' } });
      assert.match(init.result.instructions, /There is no raise or lower: scene_set depth/);
      assert.match(init.result.instructions, /scene_sort/);
      const names = (await server.request('tools/list')).result.tools.map((/** @type {any} */ t) => t.name);
      for (const name of ['scene_sort', 'object_footprint', 'scene_set']) assert.ok(names.includes(name), name);
      for (const name of ['scene_raise', 'scene_lower']) assert.ok(!names.includes(name), name);
      const tool = async (/** @type {string} */ name, /** @type {any} */ args) => (await server.request('tools/call', { name, arguments: args })).result;
      const sorted = await tool('scene_sort', { name: 'tangled' });
      assert.equal(sorted.isError, undefined, sorted.content[0].text);
      assert.equal(sorted.structuredContent.moved.length, 1);
      const before = await readFile(file('tangled'), 'utf8');
      const saved = await tool('doc_save', { doc: 'scene:tangled' });
      assert.equal(saved.isError, undefined, saved.content[0].text);
      assert.equal(await readFile(file('tangled'), 'utf8'), before, 'the document is untouched until the owner approves');
      const p = JSON.parse(await readFile(join(state, 'proposals', `${saved.structuredContent.proposal}.json`), 'utf8'));
      assert.deepEqual(p.summary, ['Sorted tangled by depth: 1 item moved']);
      assert.match(p.source, /id: 'd'[^\n]*\n {4}\{ object: 'pebble'/, 'the proposal lists d after c, before the pebble');
      const footprint = await tool('object_footprint', { name: 'pebble', to: [2, 1] });
      assert.equal(footprint.structuredContent.footprint.join(), '2,1');
      const depth = await tool('scene_set', { name: 'stand', item: 'a', depth: 'back' });
      assert.equal(depth.structuredContent.item.depth, 'back');
    } finally {
      server.child.kill();
    }
  });
});

describe('R15: the harbor\'s pine moved behind the red house', () => {
  /** @type {string} */
  let tmp;
  /** @type {Session} */
  let s;
  /** @type {any} */
  let sources;
  before(async () => {
    tmp = await tempDir('pixellab-harbor-');
    const root = join(tmp, 'harbor.pixellab');
    await cp(join(REPO, 'examples', 'harbor.pixellab'), root, { recursive: true });
    const harbor = await openDocument({ doc: root, env: { ...process.env, HOME: tmp } });
    s = new Session(harbor, { env: { ...process.env, PIXELLAB_STATE: join(tmp, 'state') } });
    sources = (await s.context().sources()).sources;
  });
  after(async () => {
    const { rm } = await import('node:fs/promises');
    await rm(tmp, { recursive: true, force: true });
  });

  const h = (/** @type {string} */ name, /** @type {any} */ params) => registry.call(s.context(), name, { name: 'harbor-flat', ...params });
  /** The color name at a scene pixel of the PNG scene.render makes, by palette lookup. */
  const colorAt = async (/** @type {number} */ x, /** @type {number} */ y) => {
    const view = (await h('scene.show', {})).viewBox;
    const png = decodePng(Buffer.from((await h('scene.render', {})).png, 'base64'));
    const [r, g, b] = png.rgba(x - view[0], y - view[1]);
    const hex = `#${[r, g, b].map((n) => n.toString(16).padStart(2, '0')).join('')}`.toUpperCase();
    return [...sources.colors].find(([, c]) => c.hex.toUpperCase() === hex)?.[0];
  };

  test('R15: moved −5,−5 to tile 0,1,0 it draws behind the red house; moved back, it draws in front again, as it did', async () => {
    await h('scene.sort', {});
    const tile = (/** @type {string} */ t) => (/** @type {any} */ i) => i.object === 'pine' && i.at.tile.join() === t;
    const start = (await h('scene.items', {})).items.find(tile('5,6,0'));
    // Standing alone in front of the island, its trunk is the front-most thing at its foot (x −16, y 88).
    assert.equal(await colorAt(-16, 88), 'wood-3');
    const picture = async () => (await h('scene.render', {})).png;
    const startPicture = await picture();
    const out = await h('scene.move', { item: start.path, by: [-5, -5] });
    assert.deepEqual(out.item.at, { tile: [0, 1, 0] });
    // The pine's crown at x −14, y −6 is behind the red house's roof, so that pixel shows the roof.
    assert.equal(await colorAt(-14, -6), 'roof-2', 'the red house is in front of the pine');
    assert.equal(await colorAt(-15, -8), 'roof-4');
    // Marking the pine's wrapper to draw in front puts the pine over the house at the same pixel, so that pixel was the house hiding it.
    const wrapper = out.path.split('.')[0];
    const fronted = await h('scene.set', { item: wrapper, depth: 'front' });
    assert.equal(await colorAt(-14, -6), 'grass-4', 'the pine\'s own color');
    // The wrapper now draws last, so its path is the one the result gives, not the old one.
    await h('scene.set', { item: fronted.path, depth: 'auto' });
    assert.equal(await colorAt(-14, -6), 'roof-2');
    // Moving it back: the scene draws as it did, and the pine is in front again. (The list needn't match: items the first move displaced stay where the sort put them.)
    const back = await h('scene.move', { item: (await h('scene.items', {})).items.find(tile('0,1,0')).path, by: [5, 5] });
    assert.deepEqual(back.item.at, { tile: [5, 6, 0] });
    assert.equal(await picture(), startPicture, 'out and back draws the scene as it was, pixel for pixel');
    assert.equal(await colorAt(-16, 88), 'wood-3');
  });
});

describe('R8: a footprint on the harbor\'s tree puts its scenes out of order', () => {
  /** @type {string} */
  let tmp;
  /** @type {Session} */
  let s;
  before(async () => {
    tmp = await tempDir('pixellab-harbor-footprint-');
    const root = join(tmp, 'harbor.pixellab');
    await cp(join(REPO, 'examples', 'harbor.pixellab'), root, { recursive: true });
    const harbor = await openDocument({ doc: root, env: { ...process.env, HOME: tmp } });
    s = new Session(harbor, { env: { ...process.env, PIXELLAB_STATE: join(tmp, 'state') } });
  });
  after(async () => {
    const { rm } = await import('node:fs/promises');
    await rm(tmp, { recursive: true, force: true });
  });

  test('R8: object.footprint tree --to 3,3 makes check report out_of_order, and scene.sort then moves the items', async () => {
    const oc = (/** @type {string} */ name, /** @type {any} */ params) => registry.call(s.context(), name, params);
    const outOfOrder = async () => (await oc('check', {})).warnings.filter((/** @type {any} */ w) => w.code === 'out_of_order');
    assert.deepEqual(await outOfOrder(), [], 'the example is in depth order as it stands');
    await oc('object.footprint', { name: 'tree', to: [3, 3] });
    const warned = await outOfOrder();
    assert.deepEqual(warned.map((/** @type {any} */ w) => w.scene).sort(), ['harbor', 'harbor-flat']);
    assert.ok(warned.every((/** @type {any} */ w) => w.moves > 0), 'each scene says how many items would move');
    const sorted = await oc('scene.sort', { all: true });
    assert.equal(sorted.changed, true);
    assert.deepEqual(sorted.scenes.map((/** @type {any} */ x) => x.scene).sort(), ['harbor', 'harbor-flat']);
    assert.ok(sorted.scenes.every((/** @type {any} */ x) => x.moved > 0), 'the sort moves items in both scenes');
    assert.deepEqual(await outOfOrder(), [], 'sorted again, check is quiet');
  });
});
