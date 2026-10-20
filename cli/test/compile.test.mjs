// The compiler (docs/intents/2026-10-pixellab-document/spec.md, R7): the
// optimizer and lossless check, ported with their tests from the site's
// scripts/optimize-pixel-art.test.mjs, plus compileScene and compileAll on
// the sample document and on the site's committed art.

import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { readFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { after, before, describe, it } from 'node:test';

import { BUDGET, compileAll, compileScene, optimizeSvg, readSources, verifyLossless } from '../lib/engine/compile.mjs';
import { SITE_BUDGETS } from '../lib/document.mjs';
import { makeDocument, makeSite, needsSite, SITE_SOURCE, tempDir } from './helpers.mjs';

const svg = (body) => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8">${body}</svg>`;
const rect = (x, y, w = 1, h = 1) => `<rect x="${x}" y="${y}" width="${w}" height="${h}"></rect>`;

/** Deterministic pseudo-random generator, so the fuzz fixture never changes. */
function lcg(seed) {
  let s = seed;
  return () => (s = (s * 1103515245 + 12345) % 2 ** 31) / 2 ** 31;
}

describe('optimize-pixel-art', () => {
  it('merges a horizontal run of pixels into one path', () => {
    const out = optimizeSvg(svg(`<g fill="#111111">${rect(0, 0)}${rect(1, 0)}${rect(2, 0)}</g>`));
    assert.equal(out, svg('<path fill="#111111" d="M0 0h3v1h-3z"/>'));
  });

  it('merges identical runs in consecutive rows into a taller rectangle', () => {
    const out = optimizeSvg(svg(`<g fill="#111111">${rect(0, 0, 2)}${rect(0, 1)}${rect(1, 1)}</g>`));
    assert.equal(out, svg('<path fill="#111111" d="M0 0h2v2h-2z"/>'));
  });

  it('handles negative coordinates and overlapping rects', () => {
    const out = optimizeSvg(svg(`<g fill="#111111">${rect(-3, -2, 2)}${rect(-2, -2, 3)}</g>`));
    assert.equal(out, svg('<path fill="#111111" d="M-3 -2h4v1h-4z"/>'));
  });

  it('keeps class, data-class and element order', () => {
    const out = optimizeSvg(
      svg(
        `<g fill="#222222">${rect(0, 0)}</g>` +
          `<g class="pcloud pc0"><g fill="#333333">${rect(1, 1)}</g></g>` +
          `<g data-class="2"><g fill="#444444">${rect(2, 2)}</g><g fill="#222222">${rect(3, 3)}</g></g>`,
      ),
    );
    assert.equal(
      out,
      svg(
        '<path fill="#222222" d="M0 0h1v1h-1z"/>' +
          '<g class="pcloud pc0"><path fill="#333333" d="M1 1h1v1h-1z"/></g>' +
          '<g data-class="2"><path fill="#444444" d="M2 2h1v1h-1z"/><path fill="#222222" d="M3 3h1v1h-1z"/></g>',
      ),
    );
  });

  it('handles a class group wrapping several fill groups, nested two deep', () => {
    const out = optimizeSvg(svg(`<g class="wf w0"><g class="cbob"><g fill="#555555">${rect(0, 0)}${rect(0, 1)}</g></g></g>`));
    assert.equal(out, svg('<g class="wf w0"><g class="cbob"><path fill="#555555" d="M0 0h1v2h-1z"/></g></g>'));
  });

  it('leaves non-rect content untouched', () => {
    const body =
      '<polygon points="0,0 1,1 0,1" fill="#666666"></polygon>' +
      `<g fill="#777777">${rect(0, 0)}<circle cx="1" cy="1" r="1"></circle></g>` +
      '<g fill="#888888"></g>';
    assert.equal(optimizeSvg(svg(body)), svg(body));
  });

  it('keeps extra attributes of a fill group on the path', () => {
    const out = optimizeSvg(svg(`<g fill="#999999" opacity="0.5">${rect(0, 0)}</g>`));
    assert.equal(out, svg('<path fill="#999999" opacity="0.5" d="M0 0h1v1h-1z"/>'));
  });

  it('is lossless on a large random fixture', () => {
    const rand = lcg(42);
    let body = '';
    for (let g = 0; g < 6; g++) {
      body += `<g fill="#00000${g}">`;
      for (let i = 0; i < 400; i++) body += rect(Math.floor(rand() * 40) - 20, Math.floor(rand() * 30) - 10, 1 + Math.floor(rand() * 3), 1 + Math.floor(rand() * 2));
      body += '</g>';
    }
    const source = svg(`<g class="hf h0">${body}</g>`);
    assert.doesNotThrow(() => verifyLossless(source, optimizeSvg(source)));
  });

  it('fails when a group gains or loses a pixel', () => {
    const source = svg(`<g fill="#111111">${rect(0, 0, 3)}</g><g fill="#222222">${rect(0, 1)}</g>`);
    const output = optimizeSvg(source);
    assert.throws(() => verifyLossless(source, output.replace('h3v1h-3z', 'h2v1h-2z')), /#111111/);
    assert.throws(() => verifyLossless(source, output.replace('M0 1h1', 'M1 1h1')), /#222222/);
  });

  it('fails when the structure or z-order changes', () => {
    const source = svg(`<g class="a"><g fill="#111111">${rect(0, 0)}</g></g><g fill="#222222">${rect(1, 1)}</g>`);
    const output = optimizeSvg(source);
    assert.throws(() => verifyLossless(source, output.replace('class="a"', 'class="b"')));
    const swapped = svg('<g class="a"><path fill="#222222" d="M1 1h1v1h-1z"/></g><path fill="#111111" d="M0 0h1v1h-1z"/>');
    assert.throws(() => verifyLossless(source, swapped));
  });

  it('rejects markup it does not understand', () => {
    assert.throws(() => optimizeSvg(svg('<!-- note --><g fill="#111111">' + rect(0, 0) + '</g>')), /unsupported/i);
    assert.throws(() => optimizeSvg(svg(`<g fill="#111111"><rect x="0.5" y="0" width="1" height="1"></rect></g>`)), /whole-number/i);
  });
});

describe('compileScene and compileAll (spec R7)', () => {
  /** @type {Awaited<ReturnType<typeof makeDocument>>} */
  let doc;
  before(async () => {
    doc = await makeDocument();
  });
  after(() => doc.cleanup());

  it('R7: compiles a scene to one path per color, lossless, with the engine budget unless the manifest gives one', async () => {
    const { sources, loadErrors } = await readSources(doc.root);
    assert.deepEqual(loadErrors, []);
    const r = compileScene(sources, 'sample');
    assert.match(r.output, /^<svg xmlns="http:\/\/www\.w3\.org\/2000\/svg" aria-hidden="true" viewBox="-16 -16 64 48" shape-rendering="crispEdges"><path fill="#/);
    assert.equal((r.output.match(/<path /g) ?? []).length, 5, 'one path per color used');
    assert.equal(r.over, false);
    assert.deepEqual(r.budget, BUDGET);
    assert.match(r.report, /^[\d.]+ KB \/ [\d.]+ KB gzip → [\d.]+ KB \/ [\d.]+ KB gzip, lossless$/);
    const tight = compileScene(sources, 'sample', { 'sample.svg': { raw: 10, gzip: 10 } });
    assert.equal(tight.over, true);
    assert.match(tight.report, / — OVER BUDGET$/);
    assert.equal(compileScene(sources, 'sample-preview', { 'sample.svg': { raw: 10, gzip: 10 } }).over, false, 'a preview-only scene keeps the engine budget');
  });

  it('R7: compileAll writes each output once, reports preview-only scenes, and writes nothing on problems', async () => {
    const out = await tempDir();
    try {
      const { sources } = await readSources(doc.root);
      const r = await compileAll(sources, [], out, {});
      assert.equal(r.ok, true);
      assert.deepEqual(r.outputs.map((o) => [o.file, o.scene, o.over]), [['sample.svg', 'sample', false]]);
      assert.deepEqual(r.previewOnly.map((p) => p.scene), ['sample-preview']);
      assert.equal(await readFile(join(out, 'sample.svg'), 'utf8'), compileScene(sources, 'sample').output);
      const refused = await compileAll(sources, ['objects/x.mjs: broken'], join(out, 'none'), {});
      assert.deepEqual(refused, { ok: false, outputs: [], previewOnly: [], problems: ['objects/x.mjs: broken'] });
      assert.ok(!existsSync(join(out, 'none')));
    } finally {
      await rm(out, { recursive: true, force: true });
    }
  });

  it('R8: compiling twice gives identical bytes', async () => {
    const a = await readSources(doc.root);
    const b = await readSources(doc.root);
    assert.equal(compileScene(a.sources, 'sample').output, compileScene(b.sources, 'sample').output);
  });
});

describe("the site's art (spec R7, depth order spec R3)", needsSite, () => {
  it("R7: in list order, every committed SVG in the site checkout is what the ported compiler produces from its sources", async () => {
    const site = await makeSite();
    try {
      const { sources, loadErrors } = await readSources(site.sourceDir);
      assert.deepEqual(loadErrors, []);
      const outputs = [...sources.scenes].filter(([, s]) => s.output);
      assert.ok(outputs.length >= 6);
      for (const [name, scene] of outputs) {
        const committed = await readFile(join(SITE_SOURCE, 'src/assets/pixel-art', scene.output), 'utf8');
        // The committed SVGs were made by engines that drew items in the order the file lists them.
        const { output, over } = compileScene(sources, name, SITE_BUDGETS, { order: 'list' });
        assert.ok(committed === output, `${scene.output} differs from scenes/${name}.mjs compiled here`);
        assert.equal(over, false, `${scene.output} is within its budget`);
      }
    } finally {
      await site.cleanup();
    }
  });

  it("R3: in depth order, every scene of the site's art compiles lossless; the scenes whose output changed and any over budget are listed, not failed", async (t) => {
    const site = await makeSite();
    try {
      const { sources, loadErrors } = await readSources(site.sourceDir);
      assert.deepEqual(loadErrors, []);
      const changed = [];
      const over = [];
      for (const [name, scene] of sources.scenes) {
        const before = compileScene(sources, name, SITE_BUDGETS, { order: 'list' });
        const after = compileScene(sources, name, SITE_BUDGETS); // throws unless lossless
        if (after.output !== before.output) changed.push(`${name} ${before.raw} → ${after.raw} bytes`);
        if (after.over) over.push(`${scene.output ?? `${name} (preview only)`} ${after.report}`);
        assert.equal(compileScene(sources, name, SITE_BUDGETS).output, after.output, `${name} compiles to the same bytes twice`);
      }
      // The site adopts the new order under its own workflow (docs/intents/2026-10-depth-order/spec.md, Areas of concern).
      t.diagnostic(`scenes whose output changes in depth order: ${changed.join('; ') || 'none'}`);
      t.diagnostic(`scenes over budget in depth order: ${over.join('; ') || 'none'}`);
    } finally {
      await site.cleanup();
    }
  });
});
