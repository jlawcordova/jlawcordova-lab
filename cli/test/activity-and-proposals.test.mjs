import assert from 'node:assert/strict';
import { test } from 'node:test';

import { activitySummary } from '../lib/activity-summary.mjs';

const cmd = (/** @type {string} */ name, format = () => 'formatted\nsecond line') => ({ name, format });

test('read tools get a short summary of what they did', () => {
  assert.equal(activitySummary(cmd('scene.list'), { scenes: new Array(15).fill({}) }, {}), 'listed 15 scenes');
  assert.equal(activitySummary(cmd('object.list'), { objects: [{}] }, {}), 'listed 1 object');
  assert.equal(activitySummary(cmd('scene.render'), { scene: 'hero-island', scale: 2, time: 0, items: [] }, {}), 'rendered hero-island at 2×');
  assert.equal(activitySummary(cmd('scene.render'), { scene: 'hero-island', scale: 1, time: 400 }, {}), 'rendered hero-island at 1×, 400 ms');
  assert.equal(activitySummary(cmd('object.render'), { object: 'house', scale: 4, frame: 0 }, {}), 'rendered house at 4×');
  assert.equal(activitySummary(cmd('scene.items'), { scene: 'meadow', items: [{}, {}] }, {}), 'listed 2 items of meadow');
  assert.equal(
    activitySummary(cmd('object.show'), { name: 'house', description: '46×42 · 10 colors' }, {}),
    'showed object house (46×42 · 10 colors)',
  );
  assert.equal(activitySummary(cmd('check'), { target: null, checked: 177, problems: [] }, {}), 'checked every source (177 files): no problems');
  assert.equal(
    activitySummary(cmd('check'), { target: { name: 'pine' }, checked: 2, problems: [{}], summary: 'ignored' }, {}),
    'checked pine (2 files): 1 problem',
  );
});

test('edits keep their own summary; other commands fall back to the format', () => {
  assert.equal(activitySummary(cmd('scene.place'), { summary: 'pine placed at column 1, row 2, level 0' }, {}), 'pine placed at column 1, row 2, level 0');
  assert.equal(activitySummary(cmd('doc.close', () => 'scene:x:   closed'), {}, {}), 'scene:x: closed');
  assert.equal(activitySummary(cmd('scene.list'), null, {}), 'listed 0 scenes');
});

test('proposals list oldest first by createdAt, even within one second', async () => {
  const { mkdtemp, rm } = await import('node:fs/promises');
  const { tmpdir } = await import('node:os');
  const { join } = await import('node:path');
  const { createState } = await import('../lib/state.mjs');
  const dir = await mkdtemp(join(tmpdir(), 'pixellab-state-'));
  try {
    const state = createState(dir);
    const base = { baseHash: null, source: '', summary: [], agent: null };
    await state.writeProposal({ ...base, id: '20261019-112830-ffffff', doc: 'scene:a', createdAt: '2026-10-19T11:28:30.100Z' });
    await state.writeProposal({ ...base, id: '20261019-112830-000000', doc: 'object:b', createdAt: '2026-10-19T11:28:30.900Z' });
    assert.deepEqual((await state.listProposals()).map((p) => p.doc), ['scene:a', 'object:b']);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
