// Every read-only command, called directly through the registry and through
// the CLI door (a spawned process), against a temp copy of the site.

import assert from 'node:assert/strict';
import { join } from 'node:path';
import { after, before, describe, test } from 'node:test';

import { createContext } from '../lib/context.mjs';
import { registry } from '../lib/registry.mjs';
import { openDocument, openSite } from '../lib/document.mjs';
import { makeDocument, makeSite, needsSite, spawnCli } from './helpers.mjs';

/** A scene that breaks two rules, and an object that breaks one. */
const BROKEN = {
  'src/assets/pixel-art/source/scenes/zz-broken.mjs': `export default {
  viewBox: [0, 0, 10, 10],
  items: [
    { object: 'no-such-object', at: { tile: [0, 0, 0] } },
    { object: 'pine', at: { px: [1, 2, 3] } },
  ],
};
`,
  'src/assets/pixel-art/source/objects/zz-bad.mjs': `export default {
  kind: 'sprite',
  keys: { a: 'not-a-color' },
  layers: [{ map: ['a'] }],
};
`,
};

describe('commands', needsSite, () => {
  /** @type {Awaited<ReturnType<typeof makeSite>>} */
  let site;
  /** @type {Awaited<ReturnType<typeof makeSite>>} */
  let broken;
  /** @type {import('../lib/context.mjs').Context} */
  let ctx;
  before(async () => {
    site = await makeSite();
    broken = await makeSite(BROKEN);
    ctx = createContext(await openSite({ site: site.root }));
  });
  after(async () => {
    await site.cleanup();
    await broken.cleanup();
  });

  const call = (/** @type {string} */ name, /** @type {Record<string, unknown>} */ params = {}) => registry.call(ctx, name, params);
  const cli = (/** @type {string[]} */ args, /** @type {string} */ root = site.root) => spawnCli([...args, '--site', root], { env: site.env() });
  const cliJson = async (/** @type {string[]} */ args) => {
    const r = await cli([...args, '--json']);
    assert.equal(r.code, 0, r.stderr);
    return JSON.parse(r.stdout);
  };

  test('document.info (spec R3, R4): a site checkout resolves to its legacy source folder', async () => {
    const r = await call('document.info');
    assert.equal(r.root, join(site.root, 'src/assets/pixel-art/source'));
    assert.equal(r.siteRoot, site.root);
    assert.equal(r.kind, 'legacy-site');
    assert.equal(r.from, 'flag');
    assert.equal(r.via, 'site');
    assert.equal(r.publishDir, join(site.root, 'src/assets/pixel-art'));
    assert.ok(r.scenes >= 10);
    assert.ok(r.objects >= 100);
    assert.equal(r.colors.world, 32);
    assert.equal(r.problems, 0);
    assert.equal(r.branch, null, 'the temp copy is not a git checkout');
    assert.deepEqual(await cliJson(['document', 'info']), r);
    const human = await cli(['document', 'info']);
    assert.match(human.stdout, /^document {2}.*source \(a legacy source folder, from the site checkout given by --site\)\n/);
    assert.match(human.stdout, /problems  0\n$/);
  });

  test('palette.get', async () => {
    const r = await call('palette.get');
    assert.equal(r.world['grass-1'], r.colors.find((/** @type {any} */ c) => c.name === 'grass-1').hex);
    assert.equal(r.colors.filter((/** @type {any} */ c) => c.tier === 'world').length, 32);
    for (const c of r.colors) assert.match(c.hex, /^#[0-9A-F]{6}$/);
    assert.deepEqual(await cliJson(['palette', 'get']), r);
    assert.match((await cli(['palette', 'get'])).stdout, /^world {3}#[0-9A-F]{6} {2}grass-1$/m);
  });

  test('scene.list', async () => {
    const { scenes } = await call('scene.list');
    const hero = scenes.find((/** @type {any} */ s) => s.name === 'hero-island');
    assert.equal(hero.output, 'hero-island.svg');
    assert.equal(hero.file, 'scenes/hero-island.mjs');
    assert.equal(hero.viewBox.length, 4);
    assert.ok(hero.items > 100);
    assert.equal(hero.motion, true);
    assert.deepEqual(await cliJson(['scene', 'list']), { scenes });
    assert.match((await cli(['scene', 'list'])).stdout, /^hero-island +\d+×\d+ +\d+ items {2}hero-island\.svg {2}· motion$/m);
  });

  test('scene.show', async () => {
    const r = await call('scene.show', { name: 'hero-island' });
    assert.equal(r.name, 'hero-island');
    assert.ok(r.objects.pine > 0);
    assert.ok(r.motion.duration > 0);
    assert.equal(r.doc, undefined);
    const withDoc = await call('scene.show', { name: 'scenes/hero-island', doc: true });
    assert.ok(Array.isArray(withDoc.doc.items));
    assert.deepEqual(await cliJson(['scene', 'show', 'hero-island']), r);
    const human = await cli(['scene', 'show', 'hero-island']);
    assert.match(human.stdout, /^hero-island {2}\(scenes\/hero-island\.mjs\)\noutput {4}hero-island\.svg\n/);
    await assert.rejects(call('scene.show', { name: 'pine' }), (err) => err.code === 'not_found' && /no scene named pine/.test(err.message));
    const missing = await cli(['scene', 'show', 'nope']);
    assert.equal(missing.code, 1);
    assert.equal(missing.stderr, 'pixellab: no scene named nope\n');
  });

  test('scene.items: flattened, with stable paths into groups', async () => {
    const r = await call('scene.items', { name: 'library-demo' });
    assert.equal(r.scene, 'library-demo');
    const scene = await ctx.doc('scene', 'library-demo');
    for (const item of r.items) {
      const path = item.path.split('.').map(Number);
      const source = path.reduce((/** @type {any} */ list, /** @type {number} */ i, /** @type {number} */ k) => (k === path.length - 1 ? list[i] : list[i].items), scene.items);
      assert.equal(item.object, source.object);
      assert.deepEqual(item.at, source.at);
      assert.equal(item.point.length, 2);
    }
    // A scene with groups has dotted paths and lists its groups.
    const range = await call('scene.items', { name: 'range-sprite' });
    const grouped = range.items.find((/** @type {any} */ i) => i.path.includes('.'));
    assert.ok(grouped, 'range-sprite has grouped items');
    assert.equal(grouped.group, grouped.path.split('.').slice(0, -1).join('.'));
    assert.ok(range.groups.some((/** @type {any} */ g) => 'data-class' in g.attrs));
    // A tile item's point is tileToPx of its tile.
    const tileItem = r.items.find((/** @type {any} */ i) => i.at.tile);
    assert.deepEqual(tileItem.point, ctx.site.iso.tileToPx(tileItem.at.tile, scene.origin ?? [0, 0]));
    assert.deepEqual(await cliJson(['scene', 'items', 'library-demo']), r);
    const human = await cli(['scene', 'items', 'library-demo']);
    assert.equal(human.stdout.trim().split('\n').length, r.items.length);
    assert.match(human.stdout, /^0 +\S+ +tile -?\d+,-?\d+,-?\d+/);
  });

  test('scene.render (shape; pixels and bounds are in render.test.mjs)', async () => {
    const r = await call('scene.render', { name: 'library-demo', scale: 2 });
    for (const key of ['png', 'scale', 'viewBox', 'width', 'height', 'items']) assert.ok(key in r, key);
    assert.equal(r.width, r.viewBox[2] * 2);
    const viaCli = await cliJson(['scene', 'render', 'library-demo', '--scale', '2']);
    assert.deepEqual(viaCli, r);
    const human = await cli(['scene', 'render', 'library-demo']);
    assert.match(human.stdout, /^library-demo: \d+×\d+ PNG at 1×, \d+ items shown, time 0 ms\n\(add --out/);
  });

  test('object.list', async () => {
    const { objects } = await call('object.list');
    const pine = objects.find((/** @type {any} */ o) => o.name === 'pine');
    assert.equal(pine.kind, 'sprite');
    assert.deepEqual(pine.size.length, 2);
    assert.equal(pine.layers, 1);
    assert.ok(pine.colors > 0);
    assert.equal(pine.resolved, undefined);
    const blocks = (await call('object.list', { kind: 'block' })).objects;
    assert.ok(blocks.length > 0 && blocks.every((/** @type {any} */ o) => o.kind === 'block' && o.blockSize.length === 3));
    const outfit = objects.find((/** @type {any} */ o) => o.extends === 'character');
    assert.ok(outfit, 'an outfit extends character');
    assert.deepEqual(await cliJson(['object', 'list']), { objects });
    assert.match((await cli(['object', 'list', '--kind', 'block'])).stdout, /^block +block +block \d+×\d+×\d+ · /m);
  });

  test('object.show: kind, size, layers, frames, colors, extends', async () => {
    const pine = await call('object.show', { name: 'pine' });
    assert.equal(pine.kind, 'sprite');
    assert.equal(pine.file, 'objects/pine.mjs');
    assert.deepEqual(pine.anchor, (await ctx.doc('object', 'pine')).anchor);
    assert.equal(pine.layers.length, 1);
    assert.equal(pine.frames, 1);
    assert.ok(pine.keys.every((/** @type {any} */ k) => /^#[0-9A-F]{6}$/.test(k.hex) && k.tier === 'world'));
    const water = await call('object.show', { name: 'water' });
    assert.equal(water.kind, 'block');
    assert.ok(water.frames > 1);
    assert.equal(water.layers[0].loop, 'wf');
    assert.deepEqual(water.block.size, [1, 1, 0]);
    const { objects } = await call('object.list');
    const outfitName = objects.find((/** @type {any} */ o) => o.extends === 'character').name;
    const outfit = await call('object.show', { name: outfitName, doc: true });
    assert.equal(outfit.extends, 'character');
    assert.equal(outfit.character, true);
    assert.ok(outfit.doc.rows);
    assert.deepEqual(await cliJson(['object', 'show', 'pine']), pine);
    assert.match((await cli(['object', 'show', 'water'])).stdout, /^kind {6}block$/m);
  });

  test('object.render (shape)', async () => {
    const r = await call('object.render', { name: 'pine', scale: 3 });
    const pine = await call('object.show', { name: 'pine' });
    assert.deepEqual(r.viewBox, [-pine.anchor[0], -pine.anchor[1], ...pine.size]);
    assert.equal(r.width, pine.size[0] * 3);
    assert.equal(r.items.length, 1);
    assert.deepEqual(await cliJson(['object', 'render', 'pine', '--scale', '3']), r);
  });

  test('check: everything, one doc, and problems with exit code 1', async () => {
    const all = await call('check');
    assert.equal(all.ok, true);
    assert.deepEqual(all.problems, []);
    const one = await call('check', { name: 'hero-island' });
    assert.equal(one.ok, true);
    assert.deepEqual(one.target, { kind: 'scene', name: 'hero-island', file: 'scenes/hero-island.mjs' });
    assert.match(one.summary, /^\d+×\d+ · \d+ items · \d+ objects · output hero-island\.svg$/);
    const ok = await cli(['check', 'pine']);
    assert.equal(ok.code, 0);
    assert.match(ok.stdout, /^objects\/pine\.mjs: ok · \d+×\d+ · /);

    const bad = createContext(await openSite({ site: broken.root }));
    const everything = await registry.call(bad, 'check', {});
    assert.equal(everything.ok, false);
    assert.ok(everything.problems.some((/** @type {any} */ p) => p.file === 'scenes/zz-broken.mjs' && /no-such-object/.test(p.message)));
    assert.ok(everything.problems.some((/** @type {any} */ p) => p.file === 'objects/zz-bad.mjs'));
    const scoped = await registry.call(bad, 'check', { name: 'zz-broken' });
    assert.ok(scoped.problems.length >= 2);
    assert.ok(scoped.problems.every((/** @type {any} */ p) => p.file === 'scenes/zz-broken.mjs'));
    // Another doc that's fine is fine, even though the sources have problems.
    assert.equal((await registry.call(bad, 'check', { name: 'pine' })).ok, true);

    const viaCli = await cli(['check', 'zz-broken'], broken.root);
    assert.equal(viaCli.code, 1);
    assert.match(viaCli.stdout, /^scenes\/zz-broken\.mjs: items\[0\]: object 'no-such-object' does not exist$/m);
    assert.match(viaCli.stdout, /^\d+ problems\n$/m);
    const json = await cli(['check', '--json'], broken.root);
    assert.equal(json.code, 1);
    assert.equal(JSON.parse(json.stdout).ok, false);
  });

  test('R11, R12: check warns with a code: off the grid, a scene out of depth order, and the copies a moving item is drawn in', async () => {
    const house = { kind: 'block', size: [1, 1, 1], faces: { top: 'grass-2', left: 'grass-4', right: 'wood-3' } };
    const walker = { kind: 'sprite', keys: { g: 'grass-2', i: 'ink' }, anchor: [2, 4], layers: [{ map: ['.gg.', 'gggg', 'gggg', '.ii.'] }] };
    const scene = (items, tracks = [], extra = '') => `export default ${JSON.stringify({ viewBox: [-80, -20, 160, 140], items, ...(tracks.length ? { motion: { duration: 10000, tracks } } : {}) })};\n${extra}`;
    const doc = await makeDocument({
      files: {
        'objects/house.mjs': `export default ${JSON.stringify(house)};\n`,
        'objects/walker.mjs': `export default ${JSON.stringify(walker)};\n`,
        // The pine at tile 0,1,0 is behind the house's walls at 1,1,1, but is listed after them.
        'scenes/out-of-order.mjs': scene([{ object: 'house', at: { tile: [1, 1, 1] } }, { object: 'walker', at: { tile: [0, 1, 0] } }]),
        'scenes/in-order.mjs': scene([{ object: 'walker', at: { tile: [0, 1, 0] } }, { object: 'house', at: { tile: [1, 1, 1] } }]),
        // The walker passes behind the house and then in front of it, so it is drawn in two places; its first move is off the grid.
        'scenes/crossing.mjs': scene(
          [{ object: 'house', at: { tile: [2, 2, 1] } }, { object: 'walker', id: 'w', at: { px: [0, 0] } }],
          [{ item: 'w', keys: [{ t: 0 }, { t: 5000, move: [0, 64] }] }],
        ),
        'scenes/off-grid.mjs': scene([{ object: 'walker', id: 'w', at: { px: [0, 0] } }], [{ item: 'w', keys: [{ t: 0 }, { t: 5000, move: [3, 5] }] }]),
      },
    });
    try {
      const own = createContext(await openDocument({ doc: doc.root }));
      const check = (/** @type {string} */ name) => registry.call(own, 'check', name === undefined ? {} : { name });
      const codes = (/** @type {any} */ r) => r.warnings.map((/** @type {any} */ w) => `${w.code} ${w.scene}`);
      const order = await check('out-of-order');
      assert.equal(order.ok, true);
      assert.deepEqual(codes(order), ['out_of_order out-of-order']);
      assert.equal(order.warnings[0].moves, 1, 'the walker moves up in front of the house; the house is not counted');
      assert.equal(order.warnings[0].file, 'scenes/out-of-order.mjs');
      assert.match(order.warnings[0].text, /^scenes\/out-of-order\.mjs: items are not in depth order: 1 would move \(pixellab scene sort out-of-order puts them right\)$/);
      assert.deepEqual((await check('in-order')).warnings, [], 'a scene in order has none');
      const crossing = await check('crossing');
      assert.deepEqual(codes(crossing), ['copies crossing']);
      assert.deepEqual({ item: crossing.warnings[0].item, copies: crossing.warnings[0].copies }, { item: 'w', copies: 2 });
      assert.match(crossing.warnings[0].text, /^scenes\/crossing\.mjs: w is drawn 2 times as it moves among other items/);
      assert.deepEqual(codes(await check('off-grid')), ['off_grid off-grid', 'off_grid off-grid']);
      assert.deepEqual((await check('walker')).warnings, [], 'an object has none');
      const everything = await check(undefined);
      assert.deepEqual(codes(everything), ['copies crossing', 'off_grid off-grid', 'off_grid off-grid', 'out_of_order out-of-order']);
      const cli = await spawnCli(['check', 'crossing', '--document', doc.root], { env: doc.env() });
      assert.equal(cli.code, 0, 'warnings keep the exit code 0');
      assert.match(cli.stdout, /^warning: scenes\/crossing\.mjs: w is drawn 2 times/m);
      assert.match(cli.stdout, /^1 warning$/m);
    } finally {
      await doc.cleanup();
    }
  });

  test('R11: a scene with a problem gets no depth warnings, only its problems', async () => {
    const bad = createContext(await openSite({ site: broken.root }));
    const r = await registry.call(bad, 'check', { name: 'zz-broken' });
    assert.ok(r.problems.length >= 2);
    assert.deepEqual(r.warnings, []);
  });

  test('rendering a broken scene is a clear command error', async () => {
    const bad = createContext(await openSite({ site: broken.root }));
    await assert.rejects(registry.call(bad, 'scene.render', { name: 'zz-broken' }), (err) => err.code === 'render_error' && /run pixellab check zz-broken/.test(err.message));
  });

  test('an ambiguous name asks for objects/ or scenes/', async () => {
    const both = await makeSite({ 'src/assets/pixel-art/source/scenes/pine.mjs': 'export default { viewBox: [0, 0, 4, 4], items: [] };\n' });
    try {
      const r = await spawnCli(['check', 'pine', '--site', both.root], { env: both.env() });
      assert.equal(r.code, 1);
      assert.match(r.stderr, /pine is both an object and a scene; say objects\/pine or scenes\/pine/);
      const scoped = await spawnCli(['check', 'scenes/pine', '--site', both.root, '--json'], { env: both.env() });
      assert.equal(JSON.parse(scoped.stdout).target.kind, 'scene');
      const kind = await spawnCli(['check', 'pine', '--kind', 'object', '--site', both.root, '--json'], { env: both.env() });
      assert.equal(JSON.parse(kind.stdout).target.kind, 'object');
    } finally {
      await both.cleanup();
    }
  });
});
