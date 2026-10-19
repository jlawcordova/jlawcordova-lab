// Docs in step with disk (docs/intents/2026-10-mcp-gaps/spec.md, R1–R8): an
// MCP session settles its proposals once the owner applies or rejects them,
// doc.reload reloads only when nothing is lost, proposals list the edits
// since disk, the app's open docs are visible to agents, and doc.merge and
// proposal.apply merge a change on disk into a dirty working copy.
// Sessions run in-process on a temp copy of the sample document.

import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { after, before, beforeEach, describe, test } from 'node:test';

import { openDocument } from '../lib/document.mjs';
import { openDocsWriter } from '../lib/open-docs.mjs';
import { registry } from '../lib/registry.mjs';
import { hashText, Session } from '../lib/session.mjs';
import { makeDocument, tempDir } from './helpers.mjs';

/** @type {Awaited<ReturnType<typeof makeDocument>>} */
let doc;
/** @type {any} */
let opened;
/** @type {string} */
let state;
/** @type {string} */
let original;

const SCENE = 'scenes/sample.mjs';

before(async () => {
  doc = await makeDocument();
  opened = await openDocument({ doc: doc.root, env: doc.env });
  original = await readFile(join(doc.root, SCENE), 'utf8');
});
after(() => doc.cleanup());
beforeEach(async () => {
  await writeFile(join(doc.root, SCENE), original);
  state = await tempDir('pixellab-state-');
});

/** An agent's session (proposals) and the owner's (direct), sharing the state dir. */
const agent = () => new Session(opened, { env: { ...process.env, PIXELLAB_STATE: state }, mode: 'propose', agent: 'test' });
const owner = () => new Session(opened, { env: { ...process.env, PIXELLAB_STATE: state } });
const call = (/** @type {Session} */ s, /** @type {string} */ name, /** @type {any} */ params) => registry.call(s.context(), name, params);
/** One MCP tool call: the server syncs first. */
const mcp = async (/** @type {Session} */ s, /** @type {string} */ name, /** @type {any} */ params) => {
  await s.sync();
  return call(s, name, params);
};
const place = (/** @type {Session} */ s, /** @type {number} */ col) => mcp(s, 'scene.place', { name: 'sample', object: 'pebble', at: [col, 2, 0] });
const status = (/** @type {Session} */ s) => mcp(s, 'doc.status', { doc: 'scene:sample' });
const items = async () => (await import(`data:text/javascript;base64,${Buffer.from(await readFile(join(doc.root, SCENE), 'utf8')).toString('base64')}`)).default.items;

describe('settling proposals (R1, R4)', () => {
  test('R1: applied with no later edits: the doc is clean, not stale, and the next proposal starts from the file', async () => {
    const a = agent();
    await place(a, 1);
    const first = await mcp(a, 'doc.save', { doc: 'scene:sample' });
    await call(owner(), 'proposal.apply', { id: first.proposal });
    const s = await status(a);
    assert.equal(s.dirty, false);
    assert.equal(s.stale, false);
    assert.deepEqual(s.synced, { doc: 'scene:sample', proposal: first.proposal, outcome: 'applied', action: 'clean' });
    assert.equal(s.proposal, undefined);
    // R4: the next proposal lists only the new edit, has a new id and the file's hash as its base.
    await mcp(a, 'scene.move', { name: 'sample', item: '0', by: [1, 0] });
    const second = await mcp(a, 'doc.save', { doc: 'scene:sample' });
    assert.notEqual(second.proposal, first.proposal);
    assert.deepEqual(second.edits, ['block moved to column 1, row 0, level 0']);
    const p = JSON.parse(await readFile(join(state, 'proposals', `${second.proposal}.json`), 'utf8'));
    assert.deepEqual(p.summary, ['block moved to column 1, row 0, level 0']);
    assert.equal(p.baseHash, hashText(await readFile(join(doc.root, SCENE), 'utf8')));
    await call(owner(), 'proposal.apply', { id: second.proposal });
    assert.equal((await items()).length, 3, 'both proposals landed');
  });

  test('R1: rejected with no later edits: the doc reloads from the file', async () => {
    const a = agent();
    await place(a, 1);
    const saved = await mcp(a, 'doc.save', { doc: 'scene:sample' });
    await call(owner(), 'proposal.reject', { id: saved.proposal });
    const s = await status(a);
    assert.deepEqual([s.dirty, s.stale, s.canUndo, s.synced.outcome, s.synced.action], [false, false, false, 'rejected', 'reloaded']);
    assert.equal((await mcp(a, 'scene.items', { name: 'sample' })).items.length, 2, 'the rejected pebble is gone');
  });

  test('R1, R4: applied with later edits: rebased, keeping only the later edits as unsaved', async () => {
    const a = agent();
    await place(a, 1);
    const saved = await mcp(a, 'doc.save', { doc: 'scene:sample' });
    await mcp(a, 'scene.move', { name: 'sample', item: '0', by: [0, 1] });
    await call(owner(), 'proposal.apply', { id: saved.proposal });
    const s = await status(a);
    assert.deepEqual([s.dirty, s.stale, s.synced.action], [true, false, 'rebased']);
    assert.deepEqual(s.edits, ['block moved to column 0, row 1, level 0']);
    const again = await mcp(a, 'doc.save', { doc: 'scene:sample' });
    assert.deepEqual(again.edits, ['block moved to column 0, row 1, level 0']);
    assert.match(again.diff, /^-    \{ object: 'block', at: \{ tile: \[0, 0, 0\] \} \},$/m);
    assert.doesNotMatch(again.diff, /^\+.*pebble.*\[1, 2, 0\]/m, 'the applied pebble is not in the new diff');
  });

  test('R1: rejected with later edits: kept as it is, and doc.status says so', async () => {
    const a = agent();
    await place(a, 1);
    const saved = await mcp(a, 'doc.save', { doc: 'scene:sample' });
    await place(a, 3);
    await call(owner(), 'proposal.reject', { id: saved.proposal });
    const s = await status(a);
    assert.deepEqual([s.dirty, s.stale, s.synced.outcome, s.synced.action], [true, false, 'rejected', 'kept']);
    assert.equal(s.edits.length, 2);
  });

  test('R1: a new doc whose proposal was rejected is discarded', async () => {
    const a = agent();
    await mcp(a, 'scene.new', { name: 'meadow' });
    const saved = await mcp(a, 'doc.save', { doc: 'scene:meadow' });
    await call(owner(), 'proposal.reject', { id: saved.proposal });
    const all = await mcp(a, 'doc.status', {});
    assert.deepEqual(all.docs, []);
    assert.deepEqual(all.settled.at(-1), { doc: 'scene:meadow', proposal: saved.proposal, outcome: 'rejected', action: 'discarded' });
  });

  test('R1: a proposal still waiting leaves the doc alone', async () => {
    const a = agent();
    await place(a, 1);
    const saved = await mcp(a, 'doc.save', { doc: 'scene:sample' });
    const s = await status(a);
    assert.deepEqual([s.dirty, s.proposal, s.synced], [true, saved.proposal, undefined]);
  });
});

describe('clean docs follow the file (R2)', () => {
  test('R2: a clean doc reloads when its file changes; a dirty one that now matches the file is clean', async () => {
    const a = agent();
    await mcp(a, 'doc.open', { doc: 'scene:sample' });
    await writeFile(join(doc.root, SCENE), original.replace('tile: [0, 0, 1]', 'tile: [0, 0, 2]'));
    assert.deepEqual((await mcp(a, 'scene.items', { name: 'sample' })).items[1].at, { tile: [0, 0, 2] });
    assert.equal((await status(a)).stale, false);
    // The owner saves the very edit the agent made.
    await place(a, 4);
    const o = owner();
    await call(o, 'scene.place', { name: 'sample', object: 'pebble', at: [4, 2, 0] });
    await call(o, 'doc.save', { doc: 'scene:sample' });
    const s = await status(a);
    assert.deepEqual([s.dirty, s.stale, s.edits], [false, false, []]);
  });
});

describe('doc.reload (R3)', () => {
  test('R3: reloads a clean, stale doc, and says when there was nothing to load', async () => {
    const a = agent();
    await call(a, 'doc.open', { doc: 'scene:sample' });
    assert.equal((await call(a, 'doc.reload', { doc: 'scene:sample' })).reloaded, false);
    await writeFile(join(doc.root, SCENE), original.replace('tile: [0, 0, 1]', 'tile: [0, 0, 3]'));
    const r = await call(a, 'doc.reload', { doc: 'scene:sample' });
    assert.deepEqual([r.reloaded, r.changed, r.dirty], [true, true, false]);
  });

  test('R3: refuses unsaved edits (code dirty, the edits in details) and a waiting proposal (code proposed)', async () => {
    const a = agent();
    await call(a, 'scene.place', { name: 'sample', object: 'pebble', at: [1, 2, 0] });
    await assert.rejects(call(a, 'doc.reload', { doc: 'scene:sample' }), (err) => {
      assert.equal(/** @type {any} */ (err).code, 'dirty');
      assert.deepEqual(/** @type {any} */ (err).details, ['pebble placed at column 1, row 2, level 0']);
      return true;
    });
    const saved = await call(a, 'doc.save', { doc: 'scene:sample' });
    await assert.rejects(call(a, 'doc.reload', { doc: 'scene:sample' }), { code: 'proposed' });
    // Once applied, the edits are on disk, so reloading drops nothing.
    await call(owner(), 'proposal.apply', { id: saved.proposal });
    const r = await call(a, 'doc.reload', { doc: 'scene:sample' });
    assert.deepEqual([r.reloaded, r.changed, r.dirty], [true, false, false]);
  });
});

describe('edits since disk (R4)', () => {
  test('R4: after a save, only later edits are listed; undoing a saved edit is listed as an undo', async () => {
    const o = owner();
    await call(o, 'scene.place', { name: 'sample', object: 'pebble', at: [1, 2, 0] });
    await call(o, 'doc.save', { doc: 'scene:sample' });
    assert.deepEqual((await call(o, 'doc.status', { doc: 'scene:sample' })).edits, []);
    await call(o, 'doc.undo', { doc: 'scene:sample' });
    assert.deepEqual((await call(o, 'doc.status', { doc: 'scene:sample' })).edits, ['Undid: pebble placed at column 1, row 2, level 0']);
    await call(o, 'doc.redo', { doc: 'scene:sample' });
    const s = await call(o, 'doc.status', { doc: 'scene:sample' });
    assert.deepEqual([s.edits, s.dirty], [[], false]);
    // A repeat right after a save is its own step, not merged into the saved one.
    await call(o, 'scene.move', { name: 'sample', item: '0', by: [1, 0] });
    await call(o, 'doc.save', { doc: 'scene:sample' });
    await call(o, 'scene.move', { name: 'sample', item: '0', by: [1, 0] });
    assert.deepEqual((await call(o, 'doc.status', { doc: 'scene:sample' })).edits, ['block moved to column 2, row 0, level 0']);
  });
});

describe('the app\'s open docs (R7)', () => {
  test('R7: doc.status lists what the app has unsaved, and a proposal for one of them warns', async () => {
    // A live process that isn't this one stands in for the app's server.
    const writer = openDocsWriter(state, doc.root, { pid: process.ppid });
    await writer.update([
      { doc: 'scene:sample', dirty: true },
      { doc: 'object:pebble', dirty: false },
    ]);
    const a = agent();
    assert.deepEqual((await call(a, 'doc.status', {})).unsavedInApp, ['scene:sample']);
    await call(a, 'scene.place', { name: 'sample', object: 'pebble', at: [1, 2, 0] });
    assert.equal((await call(a, 'doc.status', { doc: 'scene:sample' })).unsavedInApp, true);
    const saved = await call(a, 'doc.save', { doc: 'scene:sample' });
    assert.match(saved.warnings[0], /^scene:sample has unsaved edits in Pixel Lab/);
    // Another document's app, or a dead one, doesn't count.
    const other = openDocsWriter(state, join(doc.base, 'other.pixellab'), { pid: process.pid + 100000 });
    await other.update([{ doc: 'object:pebble', dirty: true }]);
    assert.deepEqual((await call(a, 'doc.status', {})).unsavedInApp, ['scene:sample']);
    await writer.remove();
    assert.deepEqual((await call(a, 'doc.status', {})).unsavedInApp, []);
  });
});

describe('merging a change on disk (R6, R8)', () => {
  test('R8: doc.merge brings the file\'s change into a dirty copy as one undo step, keeping its edits', async () => {
    const o = owner();
    await call(o, 'scene.place', { name: 'sample', object: 'pebble', at: [5, 5, 0] });
    // Someone else moves the block on disk.
    await writeFile(join(doc.root, SCENE), original.replace("{ object: 'block', at: { tile: [0, 0, 0] } }", "{ object: 'block', at: { tile: [0, -1, 0] } }"));
    assert.equal((await call(o, 'doc.status', { doc: 'scene:sample' })).stale, true);
    const r = await call(o, 'doc.merge', { doc: 'scene:sample' });
    assert.deepEqual([r.merged, r.dirty], [true, true]);
    const s = await call(o, 'doc.status', { doc: 'scene:sample' });
    assert.deepEqual([s.stale, s.edits], [false, ['pebble placed at column 5, row 5, level 0']]);
    const listed = (await call(o, 'scene.items', { name: 'sample' })).items.map((/** @type {any} */ i) => [i.object, i.at.tile]);
    assert.deepEqual(listed, [['block', [0, -1, 0]], ['pebble', [0, 0, 1]], ['pebble', [5, 5, 0]]]);
    await call(o, 'doc.undo', { doc: 'scene:sample' });
    assert.deepEqual((await call(o, 'scene.items', { name: 'sample' })).items[0].at, { tile: [0, 0, 0] }, 'undo takes the merge back out');
    await call(o, 'doc.redo', { doc: 'scene:sample' });
    const saved = await call(o, 'doc.save', { doc: 'scene:sample' });
    assert.equal(saved.wrote, true, 'no conflict: the base is the merged file');
  });

  test('R8: a clash changes nothing and says where', async () => {
    const o = owner();
    await call(o, 'scene.move', { name: 'sample', item: '0', by: [1, 0] });
    await writeFile(join(doc.root, SCENE), original.replace("{ object: 'block', at: { tile: [0, 0, 0] } }", "{ object: 'block', at: { tile: [0, -1, 0] } }"));
    await assert.rejects(call(o, 'doc.merge', { doc: 'scene:sample' }), (err) => {
      const e = /** @type {any} */ (err);
      assert.equal(e.code, 'merge_conflict');
      assert.equal(e.details.length, 1);
      assert.match(e.details[0].ours[0], /tile: \[1, 0, 0\]/);
      assert.match(e.details[0].theirs[0], /tile: \[0, -1, 0\]/);
      return true;
    });
    const s = await call(o, 'doc.status', { doc: 'scene:sample' });
    assert.deepEqual([s.dirty, s.stale], [true, true]);
  });

  test('R6: approving a proposal merges it into the owner\'s dirty copy, or reports the clash', async () => {
    const a = agent();
    await call(a, 'scene.place', { name: 'sample', object: 'pebble', at: [1, 2, 0] });
    const p1 = await call(a, 'doc.save', { doc: 'scene:sample' });
    const o = owner();
    await call(o, 'scene.place', { name: 'sample', object: 'pebble', at: [6, 6, 0] });
    /** @type {any[]} */
    const stale = [];
    o.onStale = (/** @type {any} */ x) => stale.push(x);
    const applied = await call(o, 'proposal.apply', { id: p1.proposal });
    assert.deepEqual([applied.wrote, applied.reloaded, applied.merged, applied.conflict], [true, false, true, undefined]);
    assert.match(applied.summary, /merged into the open copy/);
    assert.equal((await call(o, 'scene.items', { name: 'sample' })).items.length, 4, "the agent's pebble and the owner's");
    assert.equal((await call(o, 'doc.status', { doc: 'scene:sample' })).stale, false);

    // A second proposal that clashes with the owner's edit: written, but the open copy stays and is stale.
    const b = agent();
    await call(b, 'scene.move', { name: 'sample', item: '0', by: [0, 1] });
    const p2 = await call(b, 'doc.save', { doc: 'scene:sample' });
    await call(o, 'scene.move', { name: 'sample', item: '0', by: [1, 0] });
    const clash = await call(o, 'proposal.apply', { id: p2.proposal });
    assert.deepEqual([clash.wrote, clash.merged, clash.conflict], [true, undefined, true]);
    assert.deepEqual(stale, [{ doc: 'scene:sample', file: SCENE }]);
    assert.equal((await call(o, 'doc.status', { doc: 'scene:sample' })).stale, true);
  });
});
