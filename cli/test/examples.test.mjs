// The example documents in examples/ (scripts/make-examples.mjs): documents with
// scene layers to try the app on and to test with (docs/intents/2026-10-scene-layers/).
// They must be what the generator writes, check clean, and show what layers
// promise: nesting, the site's wrappers beside them, and the same SVG without them.

import assert from 'node:assert/strict';
import { cp, readFile, readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { after, before, describe, test } from 'node:test';

import { openDocument } from '../lib/document.mjs';
import { registry } from '../lib/registry.mjs';
import { Session } from '../lib/session.mjs';
import { examples, writeExamples } from '../../scripts/make-examples.mjs';
import { REPO, runCli, tempDir } from './helpers.mjs';

const EXAMPLES = join(REPO, 'examples');

/** Every file under a folder, relative to it. */
async function filesIn(/** @type {string} */ dir) {
  return (await readdir(dir, { recursive: true, withFileTypes: true }))
    .filter((e) => e.isFile())
    .map((e) => join(e.parentPath ?? /** @type {any} */ (e).path, e.name).slice(dir.length + 1))
    .sort();
}

/** @type {string} */
let tmp;
before(async () => {
  tmp = await tempDir('pixellab-examples-');
});
after(async () => {
  const { rm } = await import('node:fs/promises');
  await rm(tmp, { recursive: true, force: true });
});

/** A session on a temp copy of an example, and a call into it. */
async function open(/** @type {string} */ name) {
  const root = join(tmp, 'copies', `${name}.pixellab`);
  await cp(join(EXAMPLES, `${name}.pixellab`), root, { recursive: true, force: true });
  const doc = await openDocument({ doc: root, env: { ...process.env, HOME: tmp } });
  const s = new Session(doc, { env: { ...process.env, PIXELLAB_STATE: join(tmp, 'state') } });
  return { root, call: (/** @type {string} */ cmd, /** @type {any} */ params) => registry.call(s.context(), cmd, params) };
}

describe('the example documents (R18)', () => {
  test('R18: examples/ is what scripts/make-examples.mjs writes (npm run examples)', async () => {
    const out = join(tmp, 'made');
    await writeExamples(out);
    const made = await filesIn(out);
    assert.deepEqual(await filesIn(EXAMPLES).then((f) => f.filter((p) => p.includes('.pixellab/'))), made);
    for (const file of made) assert.equal(await readFile(join(EXAMPLES, file), 'utf8'), await readFile(join(out, file), 'utf8'), `${file} is out of date: run npm run examples`);
  });

  test('R18: every example checks clean', async () => {
    for (const name of Object.keys(examples())) {
      const r = await runCli(['check', '--document', join(EXAMPLES, `${name}.pixellab`)], { env: { ...process.env, HOME: tmp } });
      assert.equal(r.code, 0, `${name}: ${r.stdout}${r.stderr}`);
      assert.match(r.stdout, /no problems/);
    }
  });

  test('depth order R4, R3: every example scene is in depth order already, and draws the same in the order it lists as in depth order', async () => {
    for (const name of Object.keys(examples())) {
      const doc = await openDocument({ doc: join(EXAMPLES, `${name}.pixellab`), env: { ...process.env, HOME: tmp } });
      const { palette, objects, scenes } = await doc.loadDocs();
      const sources = doc.engine.loadSources({ palette, objects: Object.fromEntries(objects), scenes: Object.fromEntries(scenes) });
      for (const scene of scenes.keys()) {
        assert.equal(doc.engine.isInDepthOrder(sources, scenes.get(scene)), true, `${name}/${scene} is out of depth order: run npm run examples`);
        const draw = (/** @type {'list' | 'depth'} */ order) => [...doc.engine.composite(doc.engine.renderScene(sources, scene, { order }).root, { time: 0 })].join(';');
        assert.equal(draw('depth'), draw('list'), `${name}/${scene}: depth order changes pixels of the file's own order`);
      }
    }
  });

  test('R1, R3: the harbor nests layers three deep beside the site\'s { group } wrappers, with items in no layer', async () => {
    const { call } = await open('harbor');
    const r = await call('scene.items', { name: 'harbor' });
    const byId = new Map(r.layers.map((/** @type {any} */ l) => [l.id, l]));
    assert.equal(r.layers.length, 16);
    assert.deepEqual(['red-house', 'houses', 'village'].map((id) => byId.get(id).parent ?? null), ['houses', 'village', null]);
    assert.ok(r.groups.length > 50, 'most items sit in entrance wrappers');
    assert.ok(r.groups.every((/** @type {any} */ g) => g.attrs.class === 'enter'));
    assert.deepEqual(r.items.filter((/** @type {any} */ i) => !i.layer).map((/** @type {any} */ i) => i.object), ['rock', 'rock']);
    const flat = await call('scene.items', { name: 'harbor-flat' });
    assert.deepEqual([flat.layers, flat.items.length], [[], r.items.length]);
  });

  test('R2: the harbor and harbor-flat (the same scene without layers) publish the same SVG', async () => {
    const { root } = await open('harbor');
    const out = join(tmp, 'harbor-out');
    const r = await runCli(['art', 'build', '--dir', out, '--document', root], { env: { ...process.env, HOME: tmp } });
    assert.equal(r.code, 0, r.stderr);
    assert.ok((await readFile(join(out, 'harbor.svg'))).equals(await readFile(join(out, 'harbor-flat.svg'))));
  });

  test('R1, R5: the town has 59 layers four deep, and moving a district moves its houses', async () => {
    const { call } = await open('town');
    const r = await call('scene.items', { name: 'town' });
    assert.equal(r.layers.length, 59);
    const parentOf = new Map(r.layers.map((/** @type {any} */ l) => [l.id, l.parent ?? null]));
    assert.deepEqual([parentOf.get('nw-street-1-house-1'), parentOf.get('nw-street-1'), parentOf.get('nw')], ['nw-street-1', 'nw', null]);
    assert.equal(r.layers.find((/** @type {any} */ l) => l.id === 'nw').items > 18, true);
    const moved = await call('scene.layer.move', { name: 'town', layer: 'nw', by: [0, -1] });
    assert.equal(moved.moved, r.layers.find((/** @type {any} */ l) => l.id === 'nw').items);
  });
});
