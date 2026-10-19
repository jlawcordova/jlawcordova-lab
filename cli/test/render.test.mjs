// Rendering: valid PNGs at the right size, transparent backgrounds, and item
// bounds that match the pixels each item paints, in output pixels.

import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { after, before, describe, test } from 'node:test';

import { createContext } from '../lib/context.mjs';
import { registry } from '../lib/registry.mjs';
import { openSite } from '../lib/site.mjs';
import { decodePng, makeSite, needsSite, spawnCli } from './helpers.mjs';

const scene = (/** @type {string} */ items, viewBox = '[-40, -40, 80, 80]', extra = '') =>
  `export default {\n  viewBox: ${viewBox},\n  origin: [0, 0],\n  items: [\n${items}\n  ],\n${extra}};\n`;

/** Single-item scenes, so each item's bounds must equal the PNG's painted box. */
const FIXTURES = {
  'src/assets/pixel-art/source/scenes/zz-pine.mjs': scene("    { object: 'pine', at: { tile: [0, 0, 0] } },"),
  'src/assets/pixel-art/source/scenes/zz-tile.mjs': scene("    { object: 'tile', at: { tile: [1, 0, 0] } },"),
  'src/assets/pixel-art/source/scenes/zz-px.mjs': scene("    { object: 'pine', at: { px: [-13, 21] } },"),
  'src/assets/pixel-art/source/scenes/zz-two.mjs': scene(
    "    { object: 'pine', at: { px: [-20, 0] } },\n    { object: 'pine', id: 'mover', at: { px: [20, 0] } },",
    '[-40, -40, 80, 80]',
    "  motion: {\n    duration: 1000,\n    tracks: [\n      { item: 'mover', keys: [{ t: 0 }, { t: 500, move: [0, 10], show: true }] },\n    ],\n  },\n",
  ),
  'src/assets/pixel-art/source/scenes/zz-hide.mjs': scene(
    "    { object: 'pine', id: 'blink', at: { px: [0, 0] } },",
    '[-40, -40, 80, 80]',
    "  motion: {\n    duration: 1000,\n    tracks: [\n      { item: 'blink', keys: [{ t: 0 }, { t: 500, show: false }] },\n    ],\n  },\n",
  ),
};

describe('render', needsSite, () => {
  /** @type {Awaited<ReturnType<typeof makeSite>>} */
  let site;
  /** @type {import('../lib/context.mjs').Context} */
  let ctx;
  before(async () => {
    site = await makeSite(FIXTURES);
    ctx = createContext(await openSite({ site: site.root }));
  });
  after(() => site.cleanup());

  const render = async (/** @type {string} */ name, params = {}) => {
    const r = await registry.call(ctx, 'scene.render', { name, ...params });
    return { r, png: decodePng(Buffer.from(r.png, 'base64')) };
  };

  test('the fixtures are valid sources', async () => {
    for (const name of ['zz-pine', 'zz-tile', 'zz-px', 'zz-two', 'zz-hide']) {
      const r = await registry.call(ctx, 'check', { name });
      assert.deepEqual(r.problems, [], name);
    }
  });

  test('a PNG: signature, IHDR size at the scale, RGBA, transparent background', async () => {
    for (const scale of [1, 2, 3]) {
      const { r, png } = await render('library-demo', { scale });
      const scn = await ctx.doc('scene', 'library-demo');
      assert.deepEqual(png.types, ['IHDR', 'IDAT', 'IEND']);
      assert.equal(png.width, scn.viewBox[2] * scale);
      assert.equal(png.height, scn.viewBox[3] * scale);
      assert.equal(r.width, png.width);
      assert.equal(r.height, png.height);
      assert.deepEqual(r.viewBox, scn.viewBox);
      assert.equal(r.scale, scale);
      assert.equal(png.colorType, 6);
      assert.equal(png.bitDepth, 8);
      assert.equal(png.alpha(0, 0), 0, 'the corner is transparent');
      assert.ok(png.opaqueBox(), 'something is painted');
    }
  });

  test("a tile-placed sprite's bounds: tileToPx minus its anchor, plus its painted box", async () => {
    const pine = await ctx.doc('object', 'pine');
    const map = pine.layers[0].map;
    const cols = map.flatMap((/** @type {string} */ row) => [...row].flatMap((c, x) => (c === '.' ? [] : [x])));
    const rows = map.flatMap((/** @type {string} */ row, /** @type {number} */ y) => (/[^.]/.test(row) ? [y] : []));
    const [minC, maxC, minR, maxR] = [Math.min(...cols), Math.max(...cols), Math.min(...rows), Math.max(...rows)];
    for (const scale of [1, 2, 4]) {
      const { r, png } = await render('zz-pine', { scale });
      // tile [0, 0, 0] at origin [0, 0] is (0, 0); the viewBox starts at (-40, -40).
      const expected = [(0 - pine.anchor[0] + minC + 40) * scale, (0 - pine.anchor[1] + minR + 40) * scale, (maxC - minC + 1) * scale, (maxR - minR + 1) * scale];
      assert.equal(r.items.length, 1);
      assert.deepEqual(r.items[0], { path: '0', object: 'pine', at: { tile: [0, 0, 0] }, bounds: expected });
      assert.deepEqual(png.opaqueBox(), expected);
      // Every painted pixel is one of the pine's colors, fully opaque.
      const { sources } = await ctx.sources();
      const rgbs = new Set(Object.values(pine.keys).map((name) => {
        const hex = sources.colors.get(name).hex;
        return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)).concat(255).join(',');
      }));
      for (let y = expected[1]; y < expected[1] + expected[3]; y++) {
        for (let x = expected[0]; x < expected[0] + expected[2]; x++) {
          if (png.alpha(x, y)) assert.ok(rgbs.has(png.rgba(x, y).join(',')), `pixel ${x},${y}`);
        }
      }
    }
  });

  test('a flat tile block: 30×16 around its top-face center', async () => {
    const { r, png } = await render('zz-tile', { scale: 2 });
    const [cx, cy] = ctx.site.iso.tileToPx([1, 0, 0], [0, 0]);
    const expected = [(cx - 15 + 40) * 2, (cy - 8 + 40) * 2, 30 * 2, 16 * 2];
    assert.deepEqual(r.items[0].bounds, expected);
    assert.deepEqual(png.opaqueBox(), expected);
  });

  test('a px-placed item: at.px minus its anchor', async () => {
    const { r: atTile } = await render('zz-pine');
    const { r, png } = await render('zz-px');
    const [dx, dy] = [-13, 21];
    const b = atTile.items[0].bounds;
    // Clip to the 80×80 view, as the PNG is.
    const x0 = Math.max(0, b[0] + dx);
    const y0 = Math.max(0, b[1] + dy);
    const x1 = Math.min(80, b[0] + dx + b[2]);
    const y1 = Math.min(80, b[1] + dy + b[3]);
    assert.deepEqual(r.items[0].bounds, [b[0] + dx, b[1] + dy, b[2], b[3]]);
    assert.deepEqual(png.opaqueBox(), [x0, y0, x1 - x0, y1 - y0]);
  });

  test('every painted pixel of a real scene is inside some item bounds', async () => {
    for (const [name, params] of /** @type {[string, any][]} */ ([
      ['library-demo', { scale: 2 }],
      ['range-sprite', {}],
      ['hero-island', {}],
      ['hero-island', { time: 5000 }],
    ])) {
      const { r, png } = await render(name, params);
      let uncovered = 0;
      for (let y = 0; y < png.height; y++) {
        for (let x = 0; x < png.width; x++) {
          if (!png.alpha(x, y)) continue;
          const inside = r.items.some(({ bounds: b }) => b && x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3]);
          if (!inside) uncovered++;
        }
      }
      assert.equal(uncovered, 0, `${name} ${JSON.stringify(params)}`);
      // Every bounds box is inside the image or overlaps it, and is whole pixels at the scale.
      for (const { bounds: b } of r.items) {
        if (!b) continue;
        assert.ok(b.every(Number.isInteger));
        assert.equal(b[2] % r.scale, 0);
      }
    }
  });

  test('--time moves a tracked item, its bounds follow, and hidden items drop out', async () => {
    const rest = await render('zz-two', { time: 0 });
    const moved = await render('zz-two', { time: 500 });
    const [a, b] = [rest.r.items[1], moved.r.items[1]];
    assert.equal(a.id, 'mover');
    assert.deepEqual(b.bounds, [a.bounds[0], a.bounds[1] + 10, a.bounds[2], a.bounds[3]]);
    assert.deepEqual(moved.r.items[0].bounds, rest.r.items[0].bounds, 'the untracked item stays');
    assert.notDeepEqual(moved.png.opaqueBox(), rest.png.opaqueBox());
    assert.equal(moved.r.time, 500);

    const shown = await render('zz-hide', { time: 0 });
    const hidden = await render('zz-hide', { time: 700 });
    assert.equal(shown.r.items.length, 1);
    assert.equal(hidden.r.items.length, 0);
    assert.equal(hidden.png.opaqueBox(), null);
  });

  test('--time changes frame loops', async () => {
    const water = await ctx.doc('object', 'water');
    assert.ok(water.surface.frames.length > 1);
    const frames = new Set();
    for (const time of [0, 200, 400, 600, 800]) frames.add((await registry.call(ctx, 'object.render', { name: 'water', time })).png);
    assert.ok(frames.size > 1, 'water looks different over time');
    const f0 = await registry.call(ctx, 'object.render', { name: 'water', frame: 0 });
    const t0 = await registry.call(ctx, 'object.render', { name: 'water', time: 0 });
    assert.equal(f0.png, t0.png);
  });

  test('variants: the first shows, or the one holding --show', async () => {
    const { groups, items } = await registry.call(ctx, 'scene.items', { name: 'range-sprite' });
    const variants = groups.filter((/** @type {any} */ g) => 'data-class' in g.attrs);
    assert.ok(variants.length > 1);
    const second = variants[1].path;
    const first = await render('range-sprite');
    const shown = new Set(first.r.items.map((/** @type {any} */ i) => i.path));
    assert.ok(![...shown].some((p) => p.startsWith(`${second}.`)), 'the second variant is hidden by default');
    const inSecond = items.find((/** @type {any} */ i) => i.path.startsWith(`${second}.`));
    const picked = await render('range-sprite', { show: inSecond.path });
    assert.ok(picked.r.items.some((/** @type {any} */ i) => i.path === inSecond.path));
    assert.notEqual(picked.r.png, first.r.png);
  });

  test("object.render: a sprite's viewBox is its map, the anchor at (0, 0)", async () => {
    const r = await registry.call(ctx, 'object.render', { name: 'pine', scale: 2 });
    const png = decodePng(Buffer.from(r.png, 'base64'));
    const pine = await ctx.doc('object', 'pine');
    assert.deepEqual(r.viewBox, [-pine.anchor[0], -pine.anchor[1], pine.layers[0].map[0].length, pine.layers[0].map.length]);
    assert.equal(png.width, r.viewBox[2] * 2);
    // Map pixel (x, y) is PNG pixel (x, y) at 1×: check every cell's alpha.
    const one = decodePng(Buffer.from((await registry.call(ctx, 'object.render', { name: 'pine' })).png, 'base64'));
    pine.layers[0].map.forEach((/** @type {string} */ row, /** @type {number} */ y) => {
      [...row].forEach((c, x) => assert.equal(one.alpha(x, y) > 0, c !== '.', `pixel ${x},${y}`));
    });
    assert.deepEqual(r.items[0].bounds, png.opaqueBox());
    const block = await registry.call(ctx, 'object.render', { name: 'block', scale: 1 });
    assert.deepEqual(block.items[0].bounds, [0, 0, block.width, block.height]);
  });

  test('the CLI writes the PNG with --out', async () => {
    const out = join(site.base, 'out.png');
    const r = await spawnCli(['scene', 'render', 'zz-pine', '--scale', '3', '--out', out, '--site', site.root], { env: site.env() });
    assert.equal(r.code, 0, r.stderr);
    assert.match(r.stdout, /^zz-pine: 240×240 PNG at 3×, 1 items shown, time 0 ms\nwrote .*out\.png \(240×240\)\n$/);
    const png = decodePng(await readFile(out));
    assert.equal(png.width, 240);
    const json = await spawnCli(['object', 'render', 'pine', '--json', '--out', join(site.base, 'pine.png'), '--site', site.root], { env: site.env() });
    assert.equal(json.code, 0, json.stderr);
    assert.equal(JSON.parse(json.stdout).png, (await readFile(join(site.base, 'pine.png'))).toString('base64'));
  });

  test('render params are checked: scale range, time, show', async () => {
    for (const args of [['--scale', '0'], ['--scale', '17'], ['--scale', '1.5'], ['--time', '-1'], ['--show', 'x']]) {
      const r = await spawnCli(['scene', 'render', 'zz-pine', ...args, '--site', site.root], { env: site.env() });
      assert.equal(r.code, 2, args.join(' '));
      assert.match(r.stderr, /^pixellab: scene\.render: /);
    }
    const out = await spawnCli(['scene', 'list', '--out', 'x.png', '--site', site.root], { env: site.env() });
    assert.equal(out.code, 2);
    assert.match(out.stderr, /--out goes with a render command/);
  });
});
