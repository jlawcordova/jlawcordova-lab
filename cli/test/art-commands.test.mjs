// art.preview and art.sizes (docs/intents/2026-10-pixellab-document/spec.md,
// R8): the headless toolkit the site's `npm run art -- --preview | --sizes`
// gave agents, as pixellab commands. The flows come from the site's
// scripts/pixel-art-cli.test.mjs; art.build is covered in
// edit-commands.test.mjs (R7), check, object.new and scene.new in
// commands.test.mjs and edit-commands.test.mjs.

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFile, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { after, before, describe, test } from 'node:test';

import { decodePng, makeDocument, makeSite, needsSite, runCli, tempDir } from './helpers.mjs';

describe('art.preview (R8)', () => {
  /** @type {Awaited<ReturnType<typeof makeDocument>>} */
  let doc;
  before(async () => {
    doc = await makeDocument();
  });
  after(() => doc.cleanup());

  test('R8: an object previews on one tile outline, on the card color, with a 2-pixel margin', async () => {
    const r = await runCli(['art', 'preview', 'pebble', '--document', doc.root, '--json'], { env: doc.env() });
    assert.equal(r.code, 0, r.stderr);
    const out = JSON.parse(r.stdout);
    assert.equal(out.kind, 'object');
    assert.equal(out.scale, 1);
    const png = decodePng(Buffer.from(out.png, 'base64'));
    assert.deepEqual([png.width, png.height], [out.width, out.height]);
    // The tile outline is 32 wide; the pebble sits on it; 2 pixels of margin all round.
    assert.equal(png.width, 36);
    assert.deepEqual(png.rgba(0, 0), [0xe9, 0xdc, 0xc6, 255], 'the corner is the card color from the manifest');
  });

  test('R8: --scale multiplies the size, and --out writes the PNG', async () => {
    const out = await tempDir();
    try {
      const file = join(out, 'pebble@4x.png');
      const r = await runCli(['art', 'preview', 'pebble', '--scale', '4', '--document', doc.root, '--out', file], { env: doc.env() });
      assert.equal(r.code, 0, r.stderr);
      assert.match(r.stdout, /^pebble: 144×\d+ preview PNG at 4×\nwrote .*pebble@4x\.png \(144×\d+\)\n$/);
      const png = decodePng(await readFile(file));
      assert.equal(png.width, 144);
    } finally {
      await rm(out, { recursive: true, force: true });
    }
  });

  test('R8: a scene previews at its viewBox, first variant', async () => {
    const r = await runCli(['art', 'preview', 'scenes/sample', '--document', doc.root, '--json'], { env: doc.env() });
    assert.equal(r.code, 0, r.stderr);
    const out = JSON.parse(r.stdout);
    assert.equal(out.kind, 'scene');
    assert.deepEqual([out.width, out.height], [64, 48]);
  });

  test("R8: palette renders the swatch sheet: four across, a row per material and one for ink, cream and skin", async () => {
    const r = await runCli(['art', 'preview', 'palette', '--document', doc.root, '--json'], { env: doc.env() });
    assert.equal(r.code, 0, r.stderr);
    const out = JSON.parse(r.stdout);
    assert.equal(out.kind, 'palette');
    assert.deepEqual([out.width, out.height], [2 + 4 * 18, 2 + 8 * 18]);
  });

  test('R8: a doc with problems, or one that uses a broken object, gets no preview (code invalid, exit 1)', async () => {
    await doc.write('objects/zz-bad.mjs', "export default { kind: 'sprite', keys: { a: 'nope' }, layers: [{ map: ['a'] }] };\n");
    await doc.write('scenes/zz-uses-bad.mjs', "export default { viewBox: [0, 0, 8, 8], items: [{ object: 'zz-bad', at: { px: [0, 0] } }] };\n");
    try {
      for (const name of ['zz-bad', 'zz-uses-bad']) {
        const r = await runCli(['art', 'preview', name, '--document', doc.root, '--json'], { env: doc.env() });
        assert.equal(r.code, 1, name);
        const err = JSON.parse(r.stderr).error;
        assert.equal(err.code, 'invalid');
        assert.ok(err.details.some((/** @type {string} */ p) => p.startsWith('objects/zz-bad.mjs: ')), err.details.join('\n'));
      }
      const fine = await runCli(['art', 'preview', 'pebble', '--document', doc.root], { env: doc.env() });
      assert.equal(fine.code, 0, 'an object that does not use it still previews');
    } finally {
      await rm(join(doc.root, 'objects', 'zz-bad.mjs'));
      await rm(join(doc.root, 'scenes', 'zz-uses-bad.mjs'));
    }
  });

  test('R8: an unknown name exits 1', async () => {
    const r = await runCli(['art', 'preview', 'nope', '--document', doc.root], { env: doc.env() });
    assert.equal(r.code, 1);
    assert.match(r.stderr, /no object or scene named nope/);
  });
});

describe('art.sizes (R8)', () => {
  /** @type {Awaited<ReturnType<typeof makeDocument>>} */
  let doc;
  before(async () => {
    doc = await makeDocument();
  });
  after(() => doc.cleanup());

  test("R8: prints the scene's bytes and each object's and color's, and says when there is no HEAD", async () => {
    const r = await runCli(['art', 'sizes', 'sample', '--document', doc.root], { env: doc.env() });
    assert.equal(r.code, 0, r.stderr);
    assert.match(r.stdout, /^sample\.svg: [\d,]+ B raw, [\d,]+ B gzip\nHEAD: not available \(not in a git repository with a HEAD commit\)\n/);
    assert.match(r.stdout, /\nobject +placed +raw +Δ raw +gzip +Δ gzip\n/);
    assert.match(r.stdout, /\nblock +1 +\d+ +— +\d+ +—\n/);
    assert.match(r.stdout, /\ncolor +raw +Δ raw +gzip +Δ gzip\n/);
    assert.match(r.stdout, /grass-2 #8FA56E/);
    const json = await runCli(['art', 'sizes', 'sample', '--document', doc.root, '--json'], { env: doc.env() });
    const out = JSON.parse(json.stdout);
    assert.equal(out.output, 'sample.svg');
    assert.deepEqual(out.head, { reason: 'not in a git repository with a HEAD commit' });
    assert.deepEqual(out.objects.map((/** @type {any} */ o) => o.object).sort(), ['block', 'pebble']);
  });

  test('R8: compares with the sources at HEAD of the repository the document is in', async () => {
    const git = (/** @type {string[]} */ ...args) => execFileSync('git', ['-C', doc.root, ...args], { stdio: ['ignore', 'pipe', 'ignore'] });
    git('init', '-q');
    git('add', '-A');
    git('-c', 'user.email=t@t', '-c', 'user.name=t', 'commit', '-qm', 'base');
    try {
      // Place the pebble a second time: more bytes than HEAD.
      const scene = await readFile(join(doc.root, 'scenes', 'sample.mjs'), 'utf8');
      await writeFile(join(doc.root, 'scenes', 'sample.mjs'), scene.replace("{ object: 'pebble', at: { tile: [0, 0, 1] } },", "{ object: 'pebble', at: { tile: [0, 0, 1] } },\n    { object: 'pebble', at: { tile: [1, 0, 0] } },"));
      const r = await runCli(['art', 'sizes', 'scenes/sample', '--document', doc.root, '--json'], { env: doc.env() });
      assert.equal(r.code, 0, r.stderr);
      const out = JSON.parse(r.stdout);
      assert.ok(out.head.raw > 0 && out.now.raw > out.head.raw, `${out.now.raw} > ${out.head.raw}`);
      const pebble = out.objects.find((/** @type {any} */ o) => o.object === 'pebble');
      assert.equal(pebble.placed, 2);
      assert.ok(pebble.raw > pebble.headRaw);
      const human = await runCli(['art', 'sizes', 'sample', '--document', doc.root], { env: doc.env() });
      assert.match(human.stdout, /\nHEAD: [\d,]+ B raw, [\d,]+ B gzip \(\+[\d,]+ raw, [+-][\d,]+ gzip\)\n/);
    } finally {
      await rm(join(doc.root, '.git'), { recursive: true, force: true });
    }
  });

  test('R8: takes a scene, not an object', async () => {
    const r = await runCli(['art', 'sizes', 'pebble', '--document', doc.root], { env: doc.env() });
    assert.equal(r.code, 1);
    assert.match(r.stderr, /no scene named pebble/);
  });
});

describe("art.preview on the site's art (R8)", needsSite, () => {
  test('R8: the island previews at its viewBox and an outfit on its tile', async () => {
    const site = await makeSite();
    try {
      const island = await runCli(['art', 'preview', 'hero-island', '--site', site.root, '--json'], { env: site.env() });
      assert.equal(island.code, 0, island.stderr);
      assert.deepEqual([JSON.parse(island.stdout).width, JSON.parse(island.stdout).height], [446, 264]);
      const outfit = await runCli(['art', 'preview', 'outfit-front-end', '--site', site.root, '--scale', '2', '--json'], { env: site.env() });
      assert.equal(outfit.code, 0, outfit.stderr);
      assert.ok(JSON.parse(outfit.stdout).width >= 2 * 36);
    } finally {
      await site.cleanup();
    }
  });
});
