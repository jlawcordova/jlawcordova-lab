// The document (docs/intents/2026-10-pixellab-document/spec.md): resolution
// (R3), the manifest (R2), making and copying one (R4), legacy site folders,
// and fresh source loading. Replaces the old site.test.mjs.

import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { mkdir, readFile, rm, utimes, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { after, before, describe, test } from 'node:test';

import { createContext } from '../lib/context.mjs';
import { checkDocument, checkManifest, DEFAULT_THEME, findDocumentInSite, openDocument, resolveDocumentRoot, SITE_BUDGETS } from '../lib/document.mjs';
import { DocumentError } from '../lib/errors.mjs';
import { makeDocument, makeSite, needsSite, runCli, spawnCli, tempDir } from './helpers.mjs';

describe('document resolution (R3)', () => {
  /** @type {Awaited<ReturnType<typeof makeDocument>>} */
  let doc;
  /** @type {string | undefined} */
  let savedHome;
  before(async () => {
    doc = await makeDocument();
    savedHome = process.env.HOME;
    process.env.HOME = doc.home;
  });
  after(async () => {
    process.env.HOME = savedHome;
    await doc.cleanup();
  });

  const configDir = () => join(doc.home, '.config', 'pixellab');
  const writeConfig = async (/** @type {string} */ text) => {
    await mkdir(configDir(), { recursive: true });
    await writeFile(join(configDir(), 'config.json'), text);
  };

  test('R3: --document wins over PIXELLAB_DOC, the config file, --site, PIXELLAB_SITE and the default', async () => {
    await writeConfig(JSON.stringify({ document: '/from/config', site: '/site/from/config' }));
    try {
      assert.deepEqual(resolveDocumentRoot({ doc: doc.root, site: '/s', env: { PIXELLAB_DOC: '/from/env', PIXELLAB_SITE: '/s2' } }), { root: doc.root, from: 'flag', via: 'doc' });
      assert.deepEqual(resolveDocumentRoot({ site: '/s', env: { PIXELLAB_DOC: '/from/env' } }), { root: '/from/env', from: 'env', via: 'doc' });
      assert.deepEqual(resolveDocumentRoot({ site: '/s', env: {} }), { root: '/from/config', from: 'config', via: 'doc' });
      await writeConfig(JSON.stringify({ site: '/site/from/config' }));
      assert.deepEqual(resolveDocumentRoot({ site: '/s', env: { PIXELLAB_SITE: '/s2' } }), { root: '/s', from: 'flag', via: 'site' });
      assert.deepEqual(resolveDocumentRoot({ env: { PIXELLAB_SITE: '/s2' } }), { root: '/s2', from: 'env', via: 'site' });
      assert.deepEqual(resolveDocumentRoot({ env: {} }), { root: '/site/from/config', from: 'config', via: 'site' });
    } finally {
      await rm(configDir(), { recursive: true, force: true });
    }
    assert.deepEqual(resolveDocumentRoot({ env: {} }), { root: join(doc.home, 'Projects', 'jlawcordova.github.io'), from: 'default', via: 'site' });
  });

  test('~ and relative paths are expanded', () => {
    assert.equal(resolveDocumentRoot({ doc: '~/x.pixellab', env: {} }).root, join(doc.home, 'x.pixellab'));
    assert.equal(resolveDocumentRoot({ doc: 'rel', env: {}, cwd: '/tmp/base' }).root, '/tmp/base/rel');
  });

  test('a config file that is not JSON, or with a non-path value, is a clear error', async () => {
    await writeConfig('{ nope');
    try {
      assert.throws(() => resolveDocumentRoot({ env: {} }), (err) => err instanceof DocumentError && /config\.json is not valid JSON/.test(err.message));
      await writeConfig(JSON.stringify({ document: 7 }));
      assert.throws(() => resolveDocumentRoot({ env: {} }), (err) => err instanceof DocumentError && /"document" must be a path/.test(err.message));
    } finally {
      await rm(configDir(), { recursive: true, force: true });
    }
  });

  test('R3: a missing folder names the path, where it came from and how to fix it', () => {
    assert.throws(
      () => checkDocument(join(doc.base, 'nowhere'), 'env', 'doc'),
      (err) => err instanceof DocumentError && /document not found: .*nowhere \(from PIXELLAB_DOC\) is not a folder/.test(err.message) && /--document <path\.pixellab>/.test(err.message),
    );
    assert.throws(
      () => checkDocument(join(doc.base, 'nowhere'), 'flag', 'site'),
      (err) => err instanceof DocumentError && /site not found: .*nowhere \(from --site\) is not a folder/.test(err.message),
    );
  });

  test('R3: a folder without palette.mjs is not a document; a checkout without art is not a site', () => {
    assert.throws(() => checkDocument(doc.home, 'flag', 'doc'), (err) => err instanceof DocumentError && /isn't a Pixel Lab document: it has no palette\.mjs/.test(err.message));
    assert.throws(() => checkDocument(doc.home, 'flag', 'site'), (err) => err instanceof DocumentError && /doesn't look like the site checkout: no \*\.pixellab or source\//.test(err.message));
  });

  test('R3: the CLI reports a bad document with exit code 1 and code document', async () => {
    const r = await spawnCli(['scene', 'list', '--document', join(doc.base, 'nowhere')], { env: doc.env() });
    assert.equal(r.code, 1);
    assert.match(r.stderr, /^pixellab: document not found: .*nowhere \(from --document\)/);
    assert.equal(r.stdout, '');
    const json = await spawnCli(['scene', 'list', '--json'], { env: doc.env({ PIXELLAB_SITE: doc.home }) });
    assert.equal(json.code, 1);
    assert.equal(JSON.parse(json.stderr).error.code, 'document');
  });

  test('R3: the default site under HOME is tried when nothing else is set', async () => {
    const r = await spawnCli(['document', 'info'], { env: doc.env() });
    assert.equal(r.code, 1);
    assert.match(r.stderr, /Projects\/jlawcordova\.github\.io \(from the default site\)/);
  });

  test("R1, R6: openDocument reads the manifest and carries this package's engine", async () => {
    const opened = await openDocument({ doc: doc.root, env: {} });
    assert.equal(opened.root, doc.root);
    assert.equal(opened.kind, 'document');
    assert.equal(opened.name, 'sample');
    assert.equal(opened.publishDir, join(doc.base, 'sample-out'));
    assert.deepEqual(opened.budgets, {});
    assert.deepEqual(opened.previewColors, { card: '#E9DCC6', grid: 'rgba(90, 62, 43, 0.06)' });
    assert.deepEqual(opened.theme, DEFAULT_THEME);
    assert.equal(typeof opened.engine.renderScene, 'function');
    assert.equal(typeof opened.iso.tileToPx, 'function');
    assert.equal(typeof opened.motion.poseItems, 'function');
    assert.equal(typeof opened.serialize.serialize, 'function');
    assert.equal(typeof opened.preview.encodePng, 'function');
    assert.equal(typeof opened.compile.compileScene, 'function');
  });

  test('R2: a document without a manifest opens with defaults and no publish folder', async () => {
    const bare = await makeDocument({ manifest: null });
    try {
      const opened = await openDocument({ doc: bare.root, env: {} });
      assert.equal(opened.publishDir, null);
      assert.equal(opened.name, 'sample');
      assert.deepEqual(opened.previewColors, {});
    } finally {
      await bare.cleanup();
    }
  });
});

describe('the manifest (R2)', () => {
  const ok = (/** @type {any} */ m) => assert.equal(checkManifest(m), m);
  const bad = (/** @type {any} */ m, /** @type {RegExp} */ re) => assert.throws(() => checkManifest(m), (err) => err instanceof DocumentError && re.test(err.message));

  test('R2: format is required and must be 1; everything else is optional', () => {
    ok({ format: 1 });
    ok({ format: 1, name: 'x', publish: { dir: '..', budgets: { 'a.svg': { raw: 1, gzip: 1 } } }, preview: { card: '#000000', grid: '#111111' }, theme: { darkPage: '#1C1712' } });
    bad({}, /"format" must be 1 \(missing\)/);
    bad({ format: 2 }, /"format" must be 1 \(got 2\)/);
    bad([], /must be a JSON object/);
  });

  test('R2: unknown keys and bad values are named', () => {
    bad({ format: 1, colour: 1 }, /unknown key "colour"/);
    bad({ format: 1, publish: { folder: '..' } }, /unknown key publish\."folder"/);
    bad({ format: 1, publish: { dir: '' } }, /"publish\.dir" must be a path/);
    bad({ format: 1, publish: { budgets: { 'a.svg': { raw: -1, gzip: 1 } } } }, /"publish\.budgets\.a\.svg" must be \{ raw: <bytes>, gzip: <bytes> \}/);
    bad({ format: 1, preview: { card: 'red' } }, /preview\.card "red" isn't a #RRGGBB color/);
    bad({ format: 1, preview: { grid: 'rgb(1, 2, 3)' } }, /preview\.grid .* isn't a #RRGGBB or rgba/);
    bad({ format: 1, theme: { darkPage: '#12' } }, /"theme\.darkPage" must be a #RRGGBB color/);
  });

  test('R2: a manifest that is not JSON, or that fails the checks, stops the document from opening', async () => {
    const doc = await makeDocument();
    try {
      await doc.write('manifest.json', '{ nope');
      await assert.rejects(openDocument({ doc: doc.root, env: {} }), (err) => err instanceof DocumentError && /manifest\.json is not valid JSON/.test(err.message));
      await doc.write('manifest.json', '{ "format": 3 }');
      await assert.rejects(openDocument({ doc: doc.root, env: {} }), /manifest\.json: "format" must be 1/);
    } finally {
      await doc.cleanup();
    }
  });

  test("R2: the manifest's budgets, colors and theme reach the document", async () => {
    const doc = await makeDocument({ manifest: { format: 1, publish: { dir: '/abs/out', budgets: { 'sample.svg': { raw: 10, gzip: 5 } } }, theme: { darkPage: '#000000' } } });
    try {
      const opened = await openDocument({ doc: doc.root, env: {} });
      assert.equal(opened.publishDir, '/abs/out');
      assert.deepEqual(opened.budgets, { 'sample.svg': { raw: 10, gzip: 5 } });
      assert.deepEqual(opened.theme, { darkPage: '#000000' });
      assert.equal(opened.name, 'sample', 'the folder name when the manifest has none');
    } finally {
      await doc.cleanup();
    }
  });
});

describe('document.new and document.copy (R4)', () => {
  test('R4: document new needs no open document, so it works on a Mac with no site checkout', async () => {
    // A child process with an empty home: no config, no default site to fall back on (as in CI).
    const base = await tempDir();
    try {
      const env = { ...process.env, HOME: base };
      delete env.PIXELLAB_DOC;
      delete env.PIXELLAB_SITE;
      const r = await spawnCli(['document', 'new', join(base, 'fresh'), '--json'], { env });
      assert.equal(r.code, 0, r.stderr);
      assert.equal(JSON.parse(r.stdout).root, join(base, 'fresh.pixellab'));
      const other = await spawnCli(['scene', 'list'], { env });
      assert.equal(other.code, 1, 'a command that needs a document still says how to name one');
      assert.match(other.stderr, /Pass --document <path\.pixellab>/);
    } finally {
      await rm(base, { recursive: true, force: true });
    }
  });

  test('R4: document new makes an empty document with the starter palette and a manifest, and refuses an existing path', async () => {
    const base = await tempDir();
    try {
      const path = join(base, 'meadow');
      const r = await runCli(['document', 'new', path, '--json'], { env: { ...process.env, HOME: base } });
      assert.equal(r.code, 0, r.stderr);
      const out = JSON.parse(r.stdout);
      assert.equal(out.root, `${path}.pixellab`);
      assert.equal(out.name, 'meadow');
      assert.deepEqual(JSON.parse(await readFile(join(out.root, 'manifest.json'), 'utf8')), { format: 1, name: 'meadow' });
      assert.ok(existsSync(join(out.root, 'objects')) && existsSync(join(out.root, 'scenes')));
      const opened = await openDocument({ doc: out.root, env: {} });
      const { sources, loadErrors } = await createContext(opened).sources();
      assert.deepEqual(loadErrors, []);
      assert.equal(sources.colors.size, 48, 'the starter palette: 32 world and 16 outfit colors');
      assert.deepEqual(opened.engine.validate(sources), []);
      // It can be worked on at once.
      const made = await runCli(['scene', 'new', 'first', '--document', out.root], { env: { ...process.env, HOME: base } });
      assert.equal(made.code, 0, made.stderr);
      const again = await runCli(['document', 'new', path, '--json'], { env: { ...process.env, HOME: base } });
      assert.equal(again.code, 1);
      assert.equal(JSON.parse(again.stderr).error.code, 'exists');
      const named = await runCli(['document', 'new', join(base, 'other.pixellab'), '--name', 'Other art', '--json'], { env: { ...process.env, HOME: base } });
      assert.equal(JSON.parse(named.stdout).name, 'Other art');
    } finally {
      await rm(base, { recursive: true, force: true });
    }
  });

  test('R4: document copy saves the whole document elsewhere (Save As) and refuses an existing path', async () => {
    const doc = await makeDocument();
    try {
      const to = join(doc.base, 'copy');
      const r = await runCli(['document', 'copy', '--to', to, '--document', doc.root, '--json'], { env: doc.env() });
      assert.equal(r.code, 0, r.stderr);
      assert.deepEqual(JSON.parse(r.stdout), { from: doc.root, to: `${to}.pixellab`, wroteManifest: false, summary: `Copied ${doc.root} to ${to}.pixellab` });
      assert.equal(await readFile(join(`${to}.pixellab`, 'objects', 'pebble.mjs'), 'utf8'), await readFile(join(doc.root, 'objects', 'pebble.mjs'), 'utf8'));
      const opened = await openDocument({ doc: `${to}.pixellab`, env: {} });
      assert.equal(opened.name, 'sample', 'the manifest comes along');
      assert.equal(opened.publishDir, join(doc.base, 'sample-out'), 'a relative publish.dir now points next to the copy');
      const again = await runCli(['document', 'copy', '--to', to, '--document', doc.root], { env: doc.env() });
      assert.equal(again.code, 1);
      assert.match(again.stderr, /already exists/);
      const inside = await runCli(['document', 'copy', '--to', join(doc.root, 'inner'), '--document', doc.root], { env: doc.env() });
      assert.equal(inside.code, 2);
    } finally {
      await doc.cleanup();
    }
  });
});

describe('a site checkout (R3, legacy)', needsSite, () => {
  /** @type {Awaited<ReturnType<typeof makeSite>>} */
  let site;
  before(async () => {
    site = await makeSite();
  });
  after(() => site.cleanup());

  test('R3: a legacy source/ folder is the document, with the site\'s budgets, colors and publish folder', async () => {
    assert.deepEqual(findDocumentInSite(site.root), { root: site.sourceDir, legacy: true });
    const opened = await openDocument({ site: site.root, env: {} });
    assert.equal(opened.kind, 'legacy-site');
    assert.equal(opened.root, site.sourceDir);
    assert.equal(opened.siteRoot, site.root);
    assert.equal(opened.publishDir, join(site.root, 'src/assets/pixel-art'));
    assert.deepEqual(opened.budgets, SITE_BUDGETS);
    assert.deepEqual(opened.previewColors, { card: '#E9DCC6', grid: 'rgba(90, 62, 43, 0.06)' }, "from the checkout's variables.css");
    assert.deepEqual(opened.theme, { darkPage: '#1C1712' });
  });

  test("R4: a copy of a legacy source folder gets a manifest with the site's budgets and publish folder", async () => {
    const to = join(site.base, 'from-legacy.pixellab');
    const r = await runCli(['document', 'copy', '--to', to, '--site', site.root, '--json'], { env: site.env() });
    assert.equal(r.code, 0, r.stderr);
    assert.equal(JSON.parse(r.stdout).wroteManifest, true);
    const manifest = JSON.parse(await readFile(join(to, 'manifest.json'), 'utf8'));
    assert.equal(manifest.format, 1);
    assert.deepEqual(manifest.publish, { dir: '..', budgets: SITE_BUDGETS });
    assert.deepEqual(manifest.preview, { card: '#E9DCC6', grid: 'rgba(90, 62, 43, 0.06)' });
    const opened = await openDocument({ doc: to, env: {} });
    assert.equal(opened.kind, 'document');
    assert.equal(opened.publishDir, site.base, 'a relative publish.dir points next to the copy');
    assert.deepEqual(opened.budgets, SITE_BUDGETS);
  });

  test('R3: a *.pixellab under src/assets/pixel-art/ wins over source/', async () => {
    const bundle = join(site.root, 'src/assets/pixel-art/art.pixellab');
    await mkdir(bundle, { recursive: true });
    await writeFile(join(bundle, 'palette.mjs'), 'export default { world: {}, outfit: {} };\n');
    await writeFile(join(bundle, 'manifest.json'), '{ "format": 1, "name": "bundled", "publish": { "dir": ".." } }\n');
    try {
      assert.deepEqual(findDocumentInSite(site.root), { root: bundle, legacy: false });
      const opened = await openDocument({ site: site.root, env: {} });
      assert.equal(opened.kind, 'document');
      assert.equal(opened.name, 'bundled');
      assert.equal(opened.publishDir, join(site.root, 'src/assets/pixel-art'));
    } finally {
      await rm(bundle, { recursive: true, force: true });
    }
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
    const opened = await openDocument({ site: site.root, env: {} });
    const docs = await opened.loadDocs();
    assert.ok(docs.palette.world['grass-1']);
    assert.ok(docs.objects.has('pine'));
    assert.ok(docs.scenes.has('hero-island'));
    assert.deepEqual(docs.loadErrors, []);
  });

  test('an edit on disk is picked up by the next load', async () => {
    const opened = await openDocument({ site: site.root, env: {} });
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
    const opened = await openDocument({ site: site.root, env: {} });
    const first = await opened.loadDocs();
    first.objects.get('pine').anchor[0] = 99;
    assert.notEqual((await opened.loadDocs()).objects.get('pine').anchor[0], 99);
  });

  test("a file that can't be imported is a load error, as the compiler reports it", async () => {
    await site.write('src/assets/pixel-art/source/objects/zz-broken.mjs', 'export default {');
    try {
      const opened = await openDocument({ site: site.root, env: {} });
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

  test('an overlay replaces, adds and removes docs (for a session)', async () => {
    const opened = await openDocument({ site: site.root, env: {} });
    const scene = { viewBox: [0, 0, 8, 8], items: [{ object: 'pine', at: { px: [4, 7] } }] };
    const ctx = createContext(opened, {
      overlay: {
        get: (kind, name) => (kind === 'scene' && name === 'draft' ? scene : kind === 'scene' && name === 'footer-dirt' ? null : undefined),
        names: (kind) => (kind === 'scene' ? ['draft'] : []),
      },
    });
    assert.equal(await ctx.doc('scene', 'draft'), scene);
    assert.equal(ctx.document, opened, 'ctx.document is the open document');
    assert.ok((await ctx.names('scene')).includes('draft'));
    assert.ok(!(await ctx.names('scene')).includes('footer-dirt'));
    await assert.rejects(ctx.doc('scene', 'footer-dirt'), (err) => err.code === 'not_found');
  });
});
