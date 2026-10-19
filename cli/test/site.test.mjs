// Site resolution, checks and fresh source loading.

import assert from 'node:assert/strict';
import { mkdir, rm, utimes, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { after, before, describe, test } from 'node:test';

import { createContext } from '../lib/context.mjs';
import { SiteError } from '../lib/errors.mjs';
import { checkSite, openSite, resolveSiteRoot } from '../lib/site.mjs';
import { makeSite, needsSite, spawnCli } from './helpers.mjs';

describe('site resolution', needsSite, () => {
  /** @type {Awaited<ReturnType<typeof makeSite>>} */
  let site;
  /** @type {string | undefined} */
  let savedHome;
  before(async () => {
    site = await makeSite();
    savedHome = process.env.HOME;
    process.env.HOME = site.home;
  });
  after(async () => {
    process.env.HOME = savedHome;
    await site.cleanup();
  });

  const configDir = () => join(site.home, '.config', 'pixellab');
  const writeConfig = async (/** @type {string} */ text) => {
    await mkdir(configDir(), { recursive: true });
    await writeFile(join(configDir(), 'config.json'), text);
  };

  test('--site wins over PIXELLAB_SITE, the config file and the default', async () => {
    await writeConfig(JSON.stringify({ site: '/from/config' }));
    try {
      assert.deepEqual(resolveSiteRoot({ site: site.root, env: { PIXELLAB_SITE: '/from/env' } }), { root: site.root, from: 'flag' });
      assert.deepEqual(resolveSiteRoot({ env: { PIXELLAB_SITE: '/from/env' } }), { root: '/from/env', from: 'env' });
      assert.deepEqual(resolveSiteRoot({ env: {} }), { root: '/from/config', from: 'config' });
    } finally {
      await rm(configDir(), { recursive: true, force: true });
    }
    assert.deepEqual(resolveSiteRoot({ env: {} }), { root: join(site.home, 'Projects', 'jlawcordova.github.io'), from: 'default' });
  });

  test('~ and relative paths are expanded', () => {
    assert.equal(resolveSiteRoot({ site: '~/x', env: {} }).root, join(site.home, 'x'));
    assert.equal(resolveSiteRoot({ site: 'rel', env: {}, cwd: '/tmp/base' }).root, '/tmp/base/rel');
  });

  test('a config file that is not JSON is a clear error', async () => {
    await writeConfig('{ nope');
    try {
      assert.throws(() => resolveSiteRoot({ env: {} }), (err) => err instanceof SiteError && /config\.json is not valid JSON/.test(err.message));
    } finally {
      await rm(configDir(), { recursive: true, force: true });
    }
  });

  test('a missing folder names the path, where it came from and how to fix it', () => {
    assert.throws(
      () => checkSite(join(site.base, 'nowhere'), 'env'),
      (err) => err instanceof SiteError && /nowhere \(from PIXELLAB_SITE\) is not a folder/.test(err.message) && /--site <path>/.test(err.message),
    );
  });

  test('a folder that is not the site lists the missing files', () => {
    assert.throws(
      () => checkSite(site.home, 'flag'),
      (err) => err instanceof SiteError && /doesn't look like the jlawcordova\.github\.io checkout/.test(err.message) && /src\/lib\/pixel-art\/engine\.mjs/.test(err.message),
    );
  });

  test('the CLI reports a bad site with exit code 1', async () => {
    const r = await spawnCli(['scene', 'list', '--site', join(site.base, 'nowhere')], { env: site.env() });
    assert.equal(r.code, 1);
    assert.match(r.stderr, /^pixellab: site not found: .*nowhere \(from --site\)/);
    assert.equal(r.stdout, '');
    const json = await spawnCli(['scene', 'list', '--json'], { env: site.env({ PIXELLAB_SITE: site.home }) });
    assert.equal(json.code, 1);
    assert.equal(JSON.parse(json.stderr).error.code, 'site');
  });

  test('the default site under HOME is used when nothing else is set', async () => {
    const r = await spawnCli(['site', 'info'], { env: site.env() });
    assert.equal(r.code, 1);
    assert.match(r.stderr, /Projects\/jlawcordova\.github\.io \(from the default\)/);
  });

  test('openSite imports the engine and the preview helpers from the site', async () => {
    const opened = await openSite({ site: site.root });
    assert.equal(opened.root, site.root);
    assert.equal(typeof opened.engine.renderScene, 'function');
    assert.equal(typeof opened.iso.tileToPx, 'function');
    assert.equal(typeof opened.motion.poseItems, 'function');
    assert.equal(typeof opened.serialize.serialize, 'function');
    assert.equal(typeof opened.preview.encodePng, 'function');
  });
});

describe('source loading', needsSite, () => {
  /** @type {Awaited<ReturnType<typeof makeSite>>} */
  let site;
  before(async () => {
    site = await makeSite();
  });
  after(() => site.cleanup());

  test('loads the palette, objects and scenes', async () => {
    const opened = await openSite({ site: site.root });
    const docs = await opened.loadDocs();
    assert.ok(docs.palette.world['grass-1']);
    assert.ok(docs.objects.has('pine'));
    assert.ok(docs.scenes.has('hero-island'));
    assert.deepEqual(docs.loadErrors, []);
  });

  test('an edit on disk is picked up by the next load', async () => {
    const opened = await openSite({ site: site.root });
    const file = join(site.sourceDir, 'scenes', 'zz-fresh.mjs');
    const scene = (/** @type {number} */ w) => `export default { viewBox: [0, 0, ${w}, 10], items: [] };\n`;
    await writeFile(file, scene(10));
    assert.deepEqual((await opened.loadDocs()).scenes.get('zz-fresh').viewBox, [0, 0, 10, 10]);
    await writeFile(file, scene(20));
    const later = new Date(Date.now() + 5000);
    await utimes(file, later, later);
    assert.deepEqual((await opened.loadDocs()).scenes.get('zz-fresh').viewBox, [0, 0, 20, 10]);
    await rm(file);
  });

  test('docs are copies: changing one does not change the next load', async () => {
    const opened = await openSite({ site: site.root });
    const first = await opened.loadDocs();
    first.objects.get('pine').anchor[0] = 99;
    assert.notEqual((await opened.loadDocs()).objects.get('pine').anchor[0], 99);
  });

  test("a file that can't be imported is a load error, as npm run art reports it", async () => {
    await site.write('src/assets/pixel-art/source/objects/zz-broken.mjs', 'export default {');
    try {
      const opened = await openSite({ site: site.root });
      const ctx = createContext(opened);
      const { loadErrors } = await ctx.sources();
      assert.equal(loadErrors.length, 1);
      assert.match(loadErrors[0], /^objects\/zz-broken\.mjs: cannot be loaded: /);
      await assert.rejects(ctx.doc('object', 'zz-broken'), (err) => err.code === 'load_error');
      assert.deepEqual(await ctx.find('zz-broken'), { kind: 'object', name: 'zz-broken', file: 'objects/zz-broken.mjs' });
    } finally {
      await rm(join(site.sourceDir, 'objects', 'zz-broken.mjs'));
    }
  });

  test('an overlay replaces, adds and removes docs (for a later session)', async () => {
    const opened = await openSite({ site: site.root });
    const scene = { viewBox: [0, 0, 8, 8], items: [{ object: 'pine', at: { px: [4, 7] } }] };
    const ctx = createContext(opened, {
      overlay: {
        get: (kind, name) => (kind === 'scene' && name === 'draft' ? scene : kind === 'scene' && name === 'footer-dirt' ? null : undefined),
        names: (kind) => (kind === 'scene' ? ['draft'] : []),
      },
    });
    assert.equal(await ctx.doc('scene', 'draft'), scene);
    assert.ok((await ctx.names('scene')).includes('draft'));
    assert.ok(!(await ctx.names('scene')).includes('footer-dirt'));
    await assert.rejects(ctx.doc('scene', 'footer-dirt'), (err) => err.code === 'not_found');
  });
});
