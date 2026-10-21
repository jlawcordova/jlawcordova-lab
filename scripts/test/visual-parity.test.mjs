// The visual gate's own logic (scripts/visual-parity.mjs, spec R16, D9):
// the site swap it rehearses, and how it picks sample times. The gate itself
// needs Chrome and the site checkout, and runs in the main session.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { BOB, CLOUDS, boundaries, cleanTime, cloudKeys, hopAction, margin } from '../visual-parity.mjs';

/** chop in pixel-art.css: translateY at time t of 480 ms, step-end. */
function chopAt(/** @type {number} */ t) {
  const frames = [[0, 0], [9.09, -2], [18.18, -4], [27.27, -5], [36.36, -6], [45.45, -6], [54.55, -5], [63.64, -4], [72.73, -2], [81.82, 0], [90.91, -1]];
  let y = 0;
  for (const [pct, to] of frames) if (t >= (pct / 100) * 480) y = to;
  return y;
}

/** An action's place at t, stepped a whole pixel at a time between keys (as ambient.rs steps it). */
function actionAt(/** @type {{ duration: number, keys: { t: number, move: number[] }[] }} */ a, /** @type {number} */ t) {
  const keys = [...a.keys, { t: a.duration, move: a.keys.at(-1)?.move ?? [0, 0] }];
  for (let i = 0; i + 1 < keys.length; i++) {
    const [k, next] = [keys[i], keys[i + 1]];
    if (t < k.t || t >= next.t) continue;
    const dy = next.move[1] - k.move[1];
    const n = Math.abs(dy);
    return n === 0 ? k.move[1] : k.move[1] + (dy / n) * Math.floor(((t - k.t) / (next.t - k.t)) * n);
  }
  return 0;
}

test('R16: the hop as an action matches chop everywhere more than 4 ms from one of its jumps', () => {
  const hop = hopAction();
  assert.equal(hop.duration, 480);
  assert.deepEqual(hop.keys[0], { t: 0, move: [0, 0] });
  const jumps = [9.09, 18.18, 27.27, 36.36, 54.55, 63.64, 72.73, 81.82, 90.91].map((p) => (p / 100) * 480);
  for (let t = 0; t < 480; t += 0.5) {
    if (jumps.some((j) => Math.abs(t - j) <= 4)) continue;
    assert.equal(actionAt(hop, t), chopAt(t), `at ${t} ms`);
  }
});

test('R12: the cloud loops and the bob carry pixel-art.css values', () => {
  assert.deepEqual(CLOUDS, { pc0: { duration: 16000, offset: 0 }, pc1: { duration: 20000, offset: 7000 }, pc2: { duration: 13000, offset: 3000 } });
  assert.deepEqual(cloudKeys(20000), [{ t: 0, move: [0, 0] }, { t: 10000, move: [12, 0] }]);
  assert.deepEqual(BOB.keys, [{ t: 0, move: [0, 0] }, { t: 500, move: [0, -1] }]);
});

test('D9: step boundaries come from each keyframe interval and its steps', () => {
  const pcloud = { duration: 16000, frames: [{ offset: 0, easing: 'steps(12, end)' }, { offset: 0.5, easing: 'steps(12, end)' }, { offset: 1, easing: 'linear' }] };
  const b = boundaries(pcloud).sort((x, y) => x - y);
  assert.equal(b.length, 24);
  assert.equal(b[1], (0.5 / 12) * 16000);
  const wf = { duration: 1000, frames: [{ offset: 0, easing: 'step-end' }, { offset: 0.2, easing: 'step-end' }, { offset: 1, easing: 'step-end' }] };
  assert.deepEqual(boundaries(wf).sort((x, y) => x - y), [0, 200]);
});

test('D9: a sample time sits at least 5 ms from every boundary, a delay taken into account', () => {
  const wf = { duration: 1000, delay: -800, frames: [{ offset: 0, easing: 'step-end' }, { offset: 0.2, easing: 'step-end' }, { offset: 1, easing: 'step-end' }] };
  // Its boundaries in its own cycle are 0 and 200; started 800 ms in, they fall at 200 and 400 in page time.
  assert.equal(margin(200, [wf]), 0);
  assert.equal(margin(0, [wf]), 200);
  assert.equal(margin(402, [wf]), 2);
  const picked = cleanTime(401, [wf]);
  assert.ok(picked.margin >= 5, JSON.stringify(picked));
  assert.ok(Math.abs(picked.t - 401) <= 5);
});
