// Motion over the MCP (docs/intents/2026-10-mcp-gaps/spec.md, R12–R14):
// tracks and keys as commands, check's off-grid warnings, and the path
// helper. On a temp copy of the sample document with a scene that moves.

import assert from 'node:assert/strict';
import { after, before, describe, test } from 'node:test';

import { openDocument } from '../lib/document.mjs';
import * as motionLib from '../lib/engine/motion.mjs';
import { gridRuns, offGridMoves, onGrid } from '../lib/motion-ops.mjs';
import { registry } from '../lib/registry.mjs';
import { Session } from '../lib/session.mjs';
import { makeDocument, runCli, tempDir } from './helpers.mjs';

const MOVING = `export default {
  viewBox: [-32, -16, 96, 64],
  items: [
    { object: 'block', at: { tile: [0, 0, 0] } },
    { object: 'pebble', id: 'cart', at: { px: [0, 0] } },
    { object: 'pebble', id: 'bird', at: { px: [20, 0] } },
    { object: 'pebble', at: { px: [40, 0] } },
  ],
  motion: {
    duration: 2000,
    tracks: [
      { item: 'cart', keys: [{ t: 0 }, { t: 1000, move: [16, -3] }] },
    ],
  },
};
`;
const STILL = `export default {
  viewBox: [0, 0, 16, 16],
  items: [
    { object: 'pebble', id: 'lone', at: { px: [8, 12] } },
  ],
};
`;

/** @type {Awaited<ReturnType<typeof makeDocument>>} */
let doc;
/** @type {any} */
let opened;
/** @type {string} */
let state;
before(async () => {
  doc = await makeDocument({ files: { 'scenes/moving.mjs': MOVING, 'scenes/still.mjs': STILL } });
  opened = await openDocument({ doc: doc.root, env: doc.env });
  state = await tempDir('pixellab-state-');
});
after(() => doc.cleanup());

const session = () => new Session(opened, { env: { ...process.env, PIXELLAB_STATE: state } });
const call = (/** @type {Session} */ s, /** @type {string} */ name, /** @type {any} */ params) => registry.call(s.context(), name, { name: 'moving', ...params });
const tracks = async (/** @type {Session} */ s) => (await s.context().doc('scene', 'moving')).motion.tracks;
const keysOf = async (/** @type {Session} */ s, /** @type {string} */ id) => (await tracks(s)).find((/** @type {any} */ t) => t.item === id).keys;
const problems = async (/** @type {Session} */ s) => (await registry.call(s.context(), 'check', { name: 'moving' })).problems;

describe('tracks and keys (R12)', () => {
  test('R12: add a track (an item needs an id), set move, show and pose keys, move and delete them, remove the track', async () => {
    const s = session();
    await assert.rejects(call(s, 'scene.track.add', { item: '3' }), { code: 'no_id' });
    await assert.rejects(call(s, 'scene.track.add', { item: 'cart' }), { code: 'track_exists' });
    const added = await call(s, 'scene.track.add', { item: 'bird' });
    assert.deepEqual(added.track, { item: 'bird', keys: [{ t: 0 }] });

    const moved = await call(s, 'scene.key.set', { item: 'bird', time: 500, move: [4, -2] });
    assert.deepEqual(moved.key, { t: 500, move: [4, -2] });
    assert.deepEqual(moved.warnings, []);
    await call(s, 'scene.key.set', { item: 'bird', time: 500, show: false, pose: 'block' });
    await call(s, 'scene.key.set', { item: 'bird', time: 1500, show: true });
    assert.deepEqual(await keysOf(s, 'bird'), [{ t: 0 }, { t: 500, move: [4, -2], show: false, pose: 'block' }, { t: 1500, show: true }]);
    await call(s, 'scene.key.set', { item: 'bird', time: 500, clear: ['pose'] });
    assert.deepEqual((await keysOf(s, 'bird'))[1], { t: 500, move: [4, -2], show: false });

    await call(s, 'scene.key.move', { item: 'bird', time: 1500, to: 1200 });
    assert.deepEqual((await keysOf(s, 'bird')).map((/** @type {any} */ k) => k.t), [0, 500, 1200]);
    await assert.rejects(call(s, 'scene.key.move', { item: 'bird', time: 1200, to: 500 }), { code: 'key_exists' });
    await assert.rejects(call(s, 'scene.key.move', { item: 'bird', time: 0, to: 100 }), { code: 'first_key' });
    await assert.rejects(call(s, 'scene.key.set', { item: 'bird', time: 2000, show: true }), { code: 'out_of_range' });
    await call(s, 'scene.key.delete', { item: 'bird', time: 1200 });
    await assert.rejects(call(s, 'scene.key.delete', { item: 'bird', time: 0 }), { code: 'first_key' });
    await assert.rejects(call(s, 'scene.key.set', { item: 'bird', time: 100, pose: 'no-such-object' }), { code: 'usage' });
    assert.deepEqual(await problems(s), [], 'what the commands wrote validates');

    await call(s, 'scene.track.remove', { item: 'bird' });
    assert.deepEqual((await tracks(s)).map((/** @type {any} */ t) => t.item), ['cart']);
    await assert.rejects(call(s, 'scene.track.remove', { item: 'bird' }), { code: 'no_track' });
  });

  test('R12 (animation pane): scene.key.set with no values pins the item where it is at that time; a key already there is left alone', async () => {
    const s = session();
    await call(s, 'scene.track.add', { item: 'bird' });
    await call(s, 'scene.key.set', { item: 'bird', time: 1000, move: [8, 4] });
    // 8,4 is a 2:1 run of 4 steps; halfway through it the bird has taken 2.
    const pinned = await call(s, 'scene.key.set', { item: 'bird', time: 500 });
    assert.equal(pinned.changed, true);
    assert.equal(pinned.pinned, true);
    assert.deepEqual(pinned.key, { t: 500, move: [4, 2] });
    assert.match(pinned.summary, /Made bird's key at 500 ms, pinned at 4, 2/);
    const again = await call(s, 'scene.key.set', { item: 'bird', time: 500 });
    assert.equal(again.changed, false, 'a key already there is left as it is');
    assert.equal(again.pinned, false);
    assert.deepEqual(again.key, { t: 500, move: [4, 2] });
    const rest = await call(s, 'scene.key.set', { item: 'bird', time: 1000 });
    assert.equal(rest.changed, false);
    assert.deepEqual(await keysOf(s, 'bird'), [{ t: 0 }, { t: 500, move: [4, 2] }, { t: 1000, move: [8, 4] }]);
    // The second pin changed nothing, so one undo takes the pinned key away.
    await registry.call(s.context(), 'doc.undo', { doc: 'scene:moving' });
    assert.deepEqual((await keysOf(s, 'bird')).map((/** @type {any} */ k) => k.t), [0, 1000]);
    await assert.rejects(call(s, 'scene.key.set', { item: '3', time: 500 }), { code: 'no_id' });
    const cli = await runCli(['scene', 'key', 'set', 'moving', 'cart', '--time', '500', '--document', doc.root, '--dry-run'], { env: doc.env });
    assert.equal(cli.code, 0, cli.stderr);
    assert.match(cli.stdout, /pinned at 0, 0/);
  });

  test("R12: a scene with no motion gets one from scene.track.add --duration or scene.motion.set; the loop's length can't cut off keys", async () => {
    const s = session();
    const still = (/** @type {string} */ name, /** @type {any} */ params) => registry.call(s.context(), name, { name: 'still', ...params });
    await assert.rejects(still('scene.track.add', { item: 'lone' }), { code: 'no_motion' });
    await still('scene.track.add', { item: 'lone', duration: 3000 });
    assert.deepEqual((await s.context().doc('scene', 'still')).motion, { duration: 3000, tracks: [{ item: 'lone', keys: [{ t: 0 }] }] });
    await assert.rejects(call(s, 'scene.motion.set', { duration: 900 }), { code: 'out_of_range' });
    await call(s, 'scene.motion.set', { duration: 4000 });
    assert.equal((await s.context().doc('scene', 'moving')).motion.duration, 4000);
  });

  test('R12: the commands are one-shot edits on the command line too', async () => {
    const copy = await makeDocument({ files: { 'scenes/moving.mjs': MOVING } });
    try {
      const r = await runCli(['scene', 'key', 'set', 'moving', 'cart', '--time', '1500', '--no-show', '--document', copy.root], { env: copy.env });
      assert.equal(r.code, 0, r.stderr);
      assert.match(r.stdout, /Made cart's key at 1500 ms: show false/);
      assert.match(r.stdout, /warning: scenes\/moving\.mjs: motion\.tracks\[0\] \(cart\): 0 → 1000 ms moves \(16, -3\) in 1 step/);
      assert.match(r.stdout, /wrote scenes\/moving\.mjs/);
    } finally {
      await copy.cleanup();
    }
  });
});

describe('off-grid moves (R13)', () => {
  test('R13: on the grid means each step is 1 px straight, 1:1 or 2:1', () => {
    for (const [dx, dy] of [[5, 0], [0, -3], [4, 4], [-6, 3], [8, -4], [0, 0]]) assert.equal(onGrid(motionLib, dx, dy), true, `${dx},${dy}`);
    for (const [dx, dy] of [[16, -3], [6, 2], [2, 4], [3, 2]]) assert.equal(onGrid(motionLib, dx, dy), false, `${dx},${dy}`);
  });

  test('R13: check warns about each off-grid move, the loop back included, without failing', async () => {
    const s = session();
    const r = await registry.call(s.context(), 'check', { name: 'moving' });
    assert.equal(r.ok, true);
    assert.deepEqual(
      r.warnings.map((/** @type {any} */ w) => w.text),
      [
        'scenes/moving.mjs: motion.tracks[0] (cart): 0 → 1000 ms moves (16, -3) in 1 step of (16, -3); on the grid each step is 1 px straight, 1:1 or 2:1 (scene_motion_path makes such keys)',
        'scenes/moving.mjs: motion.tracks[0] (cart): 1000 → 2000 ms moves (-16, 3) in 1 step of (-16, 3); on the grid each step is 1 px straight, 1:1 or 2:1 (scene_motion_path makes such keys)',
      ],
    );
    assert.equal((await registry.call(s.context(), 'check', {})).warnings.length, 2, 'a check of everything lists them too');
    assert.deepEqual((await registry.call(s.context(), 'check', { name: 'pebble' })).warnings, [], 'an object has none');
    const cli = await runCli(['check', 'moving', '--document', doc.root], { env: doc.env });
    assert.equal(cli.code, 0, 'warnings keep the exit code 0');
    assert.match(cli.stdout, /^2 warnings$/m);
  });
});

describe('scene.motion.path (R14)', () => {
  test('R14: a segment splits into on-grid runs', () => {
    assert.deepEqual(gridRuns(16, -3), [[6, -3], [10, 0]]);
    assert.deepEqual(gridRuns(5, 4), [[2, 1], [3, 3]]);
    assert.deepEqual(gridRuns(-2, 7), [[-2, 2], [0, 5]]);
    assert.deepEqual(gridRuns(8, 4), [[8, 4]]);
  });

  test('R14: a polyline becomes on-grid keys at a steady speed, keeping the show and pose keys on the way', async () => {
    const s = session();
    await call(s, 'scene.key.set', { item: 'cart', time: 1000, clear: ['move'], pose: 'block' });
    const r = await call(s, 'scene.motion.path', { item: 'cart', points: [[0, 0], [16, -3], [16, 5]], start: 100, speed: 20 });
    // Runs: (6,-3) √45 ≈ 6.71, (10,0) 10, (0,8) 8; at 20 px/s from 100 ms.
    assert.deepEqual(r.keys, [
      { t: 100, move: [0, 0] },
      { t: 435, move: [6, -3] },
      { t: 935, move: [16, -3] },
      { t: 1335, move: [16, 5] },
    ]);
    const keys = await keysOf(s, 'cart');
    assert.deepEqual(keys.map((/** @type {any} */ k) => k.t), [0, 100, 435, 935, 1000, 1335]);
    // The pose key at 1000 ms sits on the path: 65 of 400 ms into the 8-step run down, so 1 step down.
    assert.deepEqual(keys[4], { t: 1000, move: [16, -2], pose: 'block' });
    assert.deepEqual(offGridMoves(motionLib, await s.context().doc('scene', 'moving')).filter((w) => w.to !== 2000), [], 'every move on the path is on the grid');
    assert.deepEqual(r.warnings, ['scenes/moving.mjs: motion.tracks[0] (cart): 1335 → 2000 ms moves (-16, -5) in 1 step of (-16, -5); on the grid each step is 1 px straight, 1:1 or 2:1 (scene_motion_path makes such keys)']);
    assert.deepEqual(await problems(s), []);
  });

  test('R14: --end spreads the path between two times; too long a path, or a bad one, is refused', async () => {
    const s = session();
    await call(s, 'scene.track.add', { item: 'bird' });
    const r = await call(s, 'scene.motion.path', { item: 'bird', points: [[0, 0], [8, 4], [8, 0]], start: 0, end: 1000 });
    assert.deepEqual(r.keys.map((/** @type {any} */ k) => k.t), [0, 691, 1000]);
    await assert.rejects(call(s, 'scene.motion.path', { item: 'bird', points: [[0, 0], [100, 0]], speed: 10 }), { code: 'too_long' });
    await assert.rejects(call(s, 'scene.motion.path', { item: 'bird', points: [[0, 0], [0, 0]], speed: 10 }), { code: 'usage' });
    await assert.rejects(call(s, 'scene.motion.path', { item: 'bird', points: [[0, 0], [4, 0]] }), { code: 'usage' });
    await assert.rejects(call(s, 'scene.motion.path', { item: '3', points: [[0, 0], [4, 0]], speed: 10 }), { code: 'no_id' });
  });
});
