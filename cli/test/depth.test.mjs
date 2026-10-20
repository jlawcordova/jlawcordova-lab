// Depth order (docs/intents/2026-10-depth-order/spec.md): the geometry and
// the sort (R1, R2), the scene and object formats (R5, R7, R8), and the
// engine drawing in depth order (R3, R9). Small inline fixtures, Node
// built-ins only. Test names start with the requirement they cover.

import assert from 'node:assert/strict';
import { join } from 'node:path';
import { describe, it } from 'node:test';

import { createContext } from '../lib/context.mjs';
import { compileScene, readSources } from '../lib/engine/compile.mjs';
import { openDocument } from '../lib/document.mjs';
import { behind, blockBox, displaced, keyOf, overlaps, shiftBox, sortNodes, spriteBox, spriteHeight, unionBounds, unionBox } from '../lib/engine/depth.mjs';
import { composite, depthOrder, isInDepthOrder, loadSources, renderObject, renderScene, resolve, sceneInOrder, validate } from '../lib/engine/engine.mjs';
import { pxToTile, pxToTileExact, tileToPx } from '../lib/engine/iso.mjs';
import { previewColors, renderPreview } from '../lib/engine/preview.mjs';
import { serialize } from '../lib/engine/serialize.mjs';
import { flattenItems } from '../lib/render.mjs';
import { registry } from '../lib/registry.mjs';
import { decodePng, FIXTURE_DOC, makeDocument, REPO } from './helpers.mjs';

/** Deterministic pseudo-random numbers (mulberry32), so a failure can be replayed from its seed. */
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const SEEDS = [1, 7, 42, 2026, 31337];

const PALETTE = {
  world: { ink: '#2E2418', cream: '#F4EDE0', 'grass-2': '#8FA56E', 'grass-4': '#4E6B3A', 'wood-3': '#7A4E2D' },
  outfit: {},
};
const sprite = (map, extra = {}) => ({ kind: 'sprite', keys: { g: 'grass-2', i: 'ink' }, layers: [{ map }], ...extra });
const block = (extra = {}) => ({ kind: 'block', size: [1, 1, 1], faces: { top: 'grass-2', left: 'grass-4', right: 'wood-3' }, ...extra });
const scene = (items, extra = {}) => ({ viewBox: [0, 0, 8, 8], items, ...extra });
/** Sources from inline objects and scenes. */
const src = (objects = {}, scenes = {}) => loadSources({ palette: PALETTE, objects, scenes });
/** Imports source text as a module, the way `npm run art` loads a file. */
const load = async (text) => (await import(`data:text/javascript,${encodeURIComponent(text)}`)).default;

/** Pixel bounds big enough that every pair of nodes is tested. */
const EVERYWHERE = { minX: -1000, minY: -1000, maxX: 1000, maxY: 1000 };
const node = (box, extra = {}) => ({ box, bounds: EVERYWHERE, ...extra });
/** The names in the order the sort puts them. */
const names = (nodes, labels) => sortNodes(nodes).map((i) => labels[i]);

describe('boxes (R1, D2)', () => {
  it('R1: a block stands on its footprint from the first tile and hangs below its top face', () => {
    assert.deepEqual(blockBox(2, 3, 1, [1, 1, 1]), { c0: 1.5, c1: 2.5, r0: 2.5, r1: 3.5, l0: 0, l1: 1 });
    assert.deepEqual(blockBox(2, 5, 1, [2, 1, 1]), { c0: 1.5, c1: 3.5, r0: 4.5, r1: 5.5, l0: 0, l1: 1 }, 'the hall is two tiles wide');
    assert.deepEqual(blockBox(0, 0, 0, [1, 1, 0]), { c0: -0.5, c1: 0.5, r0: -0.5, r1: 0.5, l0: 0, l1: 0 }, 'a flat tile has no thickness');
  });

  it('R1: a sprite rises from its tile and covers its footprint', () => {
    assert.deepEqual(spriteBox(1, 2, 0, [1, 1], 17 / 16), { c0: 0.5, c1: 1.5, r0: 1.5, r1: 2.5, l0: 0, l1: 17 / 16 });
    assert.deepEqual(spriteBox(1, 2, 1, [3, 2], 2), { c0: 0.5, c1: 3.5, r0: 1.5, r1: 3.5, l0: 1, l1: 3 }, 'a footprint of 3×2 covers 3 columns and 2 rows');
    assert.equal(spriteHeight(17), 17 / 16, 'the anchor row over a level');
    assert.equal(spriteHeight(0), 1 / 8, 'a flat sprite is at least an eighth of a level');
    assert.equal(spriteHeight(1), 1 / 8);
  });

  it('R1, R7: a pixel point stands where a tile at the same point and level would, between tiles', () => {
    for (const origin of [[0, 0], [5, 32], [-17, 3]]) {
      for (const level of [0, 1, -2]) {
        for (const tile of [[0, 0], [2, 1], [-3, 4]]) {
          const px = tileToPx([...tile, level], origin);
          assert.deepEqual(pxToTileExact(px, level, origin), tile, `tile ${tile}, level ${level}, origin ${origin}`);
        }
      }
    }
    assert.deepEqual(pxToTileExact([16, 8], 0, [0, 0]), [1, 0]);
    assert.deepEqual(pxToTileExact([0, 8], 0, [0, 0]), [0.5, 0.5], 'half way between tiles');
    assert.deepEqual(pxToTileExact([0, 0], 1, [0, 0]), [1, 1], 'a level up is a tile along both, so [1, 1, 1] is drawn where [0, 0, 0] is');
    assert.deepEqual(pxToTileExact([1, 0], 0, [0, 0]), [1 / 32, -1 / 32], 'a pixel is 1/32 of a tile');
    assert.ok(Object.is(pxToTileExact([0, 0], 0, [0, 0])[0], 0), 'never -0');
    // The rounded conversion the editor uses agrees at tile centers.
    assert.deepEqual(pxToTile([16, 8], 0, [0, 0]), [1, 0, 0]);
  });

  it('R1: unions of boxes and of pixel bounds, ignoring nothing-boxes', () => {
    assert.deepEqual(unionBox([blockBox(0, 0, 0, [1, 1, 1]), null, blockBox(2, 0, 0, [1, 1, 2])]), { c0: -0.5, c1: 2.5, r0: -0.5, r1: 0.5, l0: -2, l1: 0 });
    assert.equal(unionBox([null]), null);
    assert.equal(unionBox([]), null);
    assert.deepEqual(unionBounds([{ minX: 0, minY: 0, maxX: 3, maxY: 3 }, null, { minX: -2, minY: 1, maxX: 1, maxY: 9 }]), { minX: -2, minY: 0, maxX: 3, maxY: 9 });
    assert.equal(unionBounds([null]), null);
    assert.deepEqual(shiftBox(blockBox(0, 0, 0, [1, 1, 1]), 1, -2, 3), { c0: 0.5, c1: 1.5, r0: -2.5, r1: -1.5, l0: 2, l1: 3 });
    assert.equal(overlaps({ minX: 0, minY: 0, maxX: 3, maxY: 3 }, { minX: 3, minY: 3, maxX: 5, maxY: 5 }), true, 'sharing one pixel is overlap');
    assert.equal(overlaps({ minX: 0, minY: 0, maxX: 3, maxY: 3 }, { minX: 4, minY: 0, maxX: 5, maxY: 5 }), false);
    assert.equal(overlaps(null, { minX: 0, minY: 0, maxX: 3, maxY: 3 }), false, 'nothing drawn meets nothing');
  });
});

describe('behind (R1, D2)', () => {
  const unit = (c, r, l) => blockBox(c, r, l, [1, 1, 1]);

  it('R1: a box that ends before another starts on one axis is behind it, on each axis', () => {
    const middle = unit(5, 5, 5);
    assert.equal(behind(unit(4, 5, 5), middle), true, 'column');
    assert.equal(behind(unit(5, 4, 5), middle), true, 'row');
    assert.equal(behind(unit(5, 5, 4), middle), true, 'level');
    for (const flipped of [unit(6, 5, 5), unit(5, 6, 5), unit(5, 5, 6)]) assert.equal(behind(middle, flipped), true);
    for (const flipped of [unit(6, 5, 5), unit(5, 6, 5), unit(5, 5, 6)]) assert.equal(behind(flipped, middle), false);
    assert.equal(behind(unit(3, 5, 5), middle), true, 'any gap, not only a touching neighbour');
  });

  it('R1: boxes that touch are separated, boxes that share volume are not', () => {
    assert.equal(behind(unit(4, 5, 5), unit(5, 5, 5)), true, 'touching at c = 4.5');
    const reaching = blockBox(4, 5, 5, [1.5, 1, 1]); // reaches into the other's column
    assert.equal(behind(reaching, unit(5, 5, 5)), true, 'no axis separates them, so the smaller centre sum is behind');
    assert.equal(behind(unit(5, 5, 5), reaching), false);
    assert.equal(behind(reaching, shiftBox(reaching, 0.5, 0, 0)), true);
  });

  it('R1: behind on one axis and in front on another means the silhouettes cannot overlap, so no order', () => {
    const a = unit(0, 4, 5); // behind b by column, in front of b by row
    const b = unit(4, 0, 5);
    assert.equal(behind(a, b), false);
    assert.equal(behind(b, a), false);
    const low = unit(0, 0, 0); // behind by level, in front by column
    const high = unit(-3, 5, 3);
    assert.equal(behind(low, high), false);
    assert.equal(behind(high, low), false);
  });

  it('R1: interpenetrating boxes: the one whose centre sum is smaller is behind, and a tie is no order', () => {
    const a = blockBox(0, 0, 0, [2, 2, 2]);
    const b = shiftBox(a, 1, 0, 0);
    assert.equal(behind(a, b), true);
    assert.equal(behind(b, a), false);
    assert.equal(behind(a, a), false, 'a box is not behind itself');
    assert.equal(behind(a, shiftBox(a, 1, -1, 0)), false, 'equal sums are a tie');
    assert.equal(behind(shiftBox(a, 1, -1, 0), a), false);
  });

  it('R1: a missing box is behind nothing and has nothing behind it', () => {
    assert.equal(behind(null, unit(0, 0, 0)), false);
    assert.equal(behind(unit(0, 0, 0), null), false);
    assert.equal(keyOf(null), -Infinity);
    assert.equal(keyOf(unit(1, 2, 3)), 0.5 + 1.5 + 1.5 + 2.5 + 2 + 3, 'twice the centre sum: c0+c1+r0+r1+l0+l1');
  });
});

describe('the sort (R1, R2, D2)', () => {
  it('R1: the pine at 0,1,0 draws before the red house\'s walls at 1,1,1, whatever the list says', () => {
    const pine = spriteBox(0, 1, 0, [1, 1], 17 / 16);
    const walls = blockBox(1, 1, 1, [1, 1, 1]);
    const roof = blockBox(1, 1, 2, [1, 1, 1]);
    const labels = ['walls', 'roof', 'pine'];
    assert.deepEqual(names([node(walls), node(roof), node(pine)], labels), ['pine', 'walls', 'roof']);
    const front = spriteBox(2, 1, 0, [1, 1], 17 / 16); // on the other side of the house
    assert.deepEqual(names([node(front), node(walls), node(roof)], ['front', 'walls', 'roof']), ['walls', 'roof', 'front']);
  });

  it('R1: the hall\'s 2×1 walls and roof sort against the tiles and trees around them, not by their first tile', () => {
    const walls = blockBox(2, 5, 1, [2, 1, 1]);
    const roof = blockBox(2, 5, 2, [2, 1, 1]);
    const behindRight = spriteBox(3, 4, 0, [1, 1], 15 / 16); // beside the hall's far half, one row back
    const inFrontRight = spriteBox(3, 6, 0, [1, 1], 15 / 16); // one row in front of its far half
    const under = blockBox(3, 5, 0, [1, 1, 1]); // the tile under its far half
    const labels = ['roof', 'inFront', 'walls', 'under', 'behind'];
    const list = [node(roof), node(inFrontRight), node(walls), node(under), node(behindRight)];
    const got = names(list, labels);
    assert.ok(got.indexOf('behind') < got.indexOf('walls'), `a tree one row behind draws before the hall: ${got}`);
    assert.ok(got.indexOf('under') < got.indexOf('walls'), `the tile under its far half draws first: ${got}`);
    assert.ok(got.indexOf('walls') < got.indexOf('roof'));
    assert.ok(got.indexOf('roof') < got.indexOf('inFront'), `a tree one row in front draws after the roof: ${got}`);
  });

  it('R1: a raised block is ordered against the tiles around it by its level too', () => {
    const pillar = blockBox(1, 1, 3, [1, 1, 3]); // levels 0 to 3
    const groundFront = blockBox(2, 1, 0, [1, 1, 1]);
    const groundBack = blockBox(0, 1, 0, [1, 1, 1]);
    const top = blockBox(1, 1, 4, [1, 1, 1]); // on top of the pillar
    const labels = ['top', 'groundFront', 'pillar', 'groundBack'];
    const got = names([node(top), node(groundFront), node(pillar), node(groundBack)], labels);
    assert.ok(got.indexOf('groundBack') < got.indexOf('pillar'), `${got}`);
    assert.ok(got.indexOf('pillar') < got.indexOf('top'), `the block on top draws after the pillar: ${got}`);
    // The tile in front of the pillar is in front of it by column but under it by level: their silhouettes
    // can't overlap, so nothing orders them.
    assert.equal(behind(groundFront, pillar) || behind(pillar, groundFront), false);
  });

  it('R1: items whose pixels cannot meet are not ordered by their boxes: they keep the order they are listed in', () => {
    // b is behind a by the box test (column), but they draw in different places.
    const a = { box: blockBox(5, 0, 0, [1, 1, 1]), bounds: { minX: 0, minY: 0, maxX: 10, maxY: 10 } };
    const b = { box: blockBox(0, 0, 0, [1, 1, 1]), bounds: { minX: 100, minY: 0, maxX: 110, maxY: 10 } };
    assert.deepEqual(sortNodes([a, b]), [0, 1], 'listed order, though b is behind a by their boxes');
    const c = { box: blockBox(0, 0, 0, [1, 1, 1]), bounds: null };
    const d = { box: null, bounds: null };
    assert.deepEqual(sortNodes([a, c, d]), [0, 1, 2], 'nodes with no box or no pixels are never ordered against anything');
  });

  it('R5, D2: back items draw before the others and front items after, each band sorted by position', () => {
    const near = blockBox(5, 5, 0, [1, 1, 1]);
    const far = blockBox(0, 0, 0, [1, 1, 1]);
    const mid = blockBox(3, 3, 0, [1, 1, 1]);
    const labels = ['near-back', 'far-front', 'mid', 'far-back', 'near-front', 'far'];
    const list = [
      node(near, { depth: 'back' }),
      node(far, { depth: 'front' }),
      node(mid),
      node(far, { depth: 'back' }),
      node(near, { depth: 'front' }),
      node(far),
    ];
    assert.deepEqual(names(list, labels), ['far-back', 'near-back', 'far', 'mid', 'far-front', 'near-front']);
  });

  it('R2: a cycle of three boxes that each cover the next is broken by the smallest key, deterministically', () => {
    // a is behind b by column, b behind c by row, and c behind a by level; no other axis settles any pair.
    const a = { c0: 0, c1: 2, r0: 0, r1: 6, l0: 4, l1: 6 };
    const b = { c0: 2, c1: 6, r0: 0, r1: 2, l0: 0, l1: 8 };
    const c = { c0: 0, c1: 4, r0: 2, r1: 6, l0: 0, l1: 3 };
    assert.equal(behind(a, b) && behind(b, c) && behind(c, a), true, 'a real cycle');
    assert.equal(keyOf(c) < keyOf(a) && keyOf(c) < keyOf(b), true);
    for (const list of [['a', 'b', 'c'], ['c', 'a', 'b'], ['b', 'c', 'a']]) {
      const by = { a, b, c };
      assert.deepEqual(names(list.map((n) => node(by[n])), list), ['c', 'a', 'b'], `listed as ${list}`);
    }
  });

  it('R1, R2: the order is a valid depth order: every overlapping pair that is ordered by its boxes is respected unless the pair sits in a cycle', () => {
    for (const seed of SEEDS) {
      const r = rng(seed);
      for (let round = 0; round < 10; round++) {
        const n = 10 + Math.floor(r() * 25);
        const nodes = randomBoxes(r, n, 6).map((box, i) => ({ box, bounds: r() < 0.7 ? EVERYWHERE : { minX: i * 3, minY: 0, maxX: i * 3 + 2 + Math.floor(r() * 8), maxY: 5 } }));
        const order = sortNodes(nodes);
        assert.deepEqual(sortNodes(nodes), order, 'the same input gives the same order');
        const position = new Map(order.map((index, at) => [index, at]));
        assert.equal(position.size, n);
        const edge = (i, j) => i !== j && overlaps(nodes[i].bounds, nodes[j].bounds) && behind(nodes[i].box, nodes[j].box);
        // reach[i][j]: following "is behind" edges leads from i to j.
        const reach = nodes.map((_, i) => nodes.map((__, j) => edge(i, j)));
        for (let k = 0; k < n; k++) for (let i = 0; i < n; i++) if (reach[i][k]) for (let j = 0; j < n; j++) if (reach[k][j]) reach[i][j] = true;
        for (let i = 0; i < n; i++) {
          for (let j = 0; j < n; j++) {
            if (edge(i, j) && position.get(j) < position.get(i)) assert.ok(reach[j][i], `seed ${seed}, round ${round}: ${i} is behind ${j} but draws after it, outside any cycle`);
          }
        }
      }
    }
  });

  it('R2: sorting a sorted list changes nothing, on random scenes with ties, cycles and bounds that miss', () => {
    for (const seed of SEEDS) {
      const r = rng(seed);
      for (let round = 0; round < 20; round++) {
        const boxes = randomBoxes(r, 5 + Math.floor(r() * 60), 4);
        const nodes = boxes.map((b, i) => ({
          box: b,
          bounds: r() < 0.7 ? EVERYWHERE : { minX: i * 3, minY: 0, maxX: i * 3 + 2 + Math.floor(r() * 8), maxY: 5 },
          ...(r() < 0.15 ? { depth: r() < 0.5 ? 'front' : 'back' } : {}),
        }));
        const once = sortNodes(nodes).map((i) => nodes[i]);
        const twice = sortNodes(once).map((i) => once[i]);
        assert.deepEqual(twice, once, `seed ${seed}, round ${round}`);
        assert.deepEqual(sortNodes(once), once.map((_, i) => i), `seed ${seed}, round ${round}: the identity order`);
      }
    }
  });

  it('R2: the same scene gives the same order on every run', () => {
    const r = rng(99);
    const nodes = randomBoxes(r, 80, 5).map((b) => node(b));
    const first = JSON.stringify(sortNodes(nodes));
    for (let i = 0; i < 3; i++) assert.equal(JSON.stringify(sortNodes(nodes)), first);
  });

  it('R2: a list of 400 items sorts in well under a second', () => {
    const r = rng(2026);
    const nodes = Array.from({ length: 400 }, () => {
      const x = Math.floor(r() * 600) - 300;
      const y = Math.floor(r() * 400) - 200;
      const c = Math.floor(r() * 20);
      const rw = Math.floor(r() * 20);
      return { box: blockBox(c, rw, Math.floor(r() * 3), [1 + Math.floor(r() * 2), 1, 1 + Math.floor(r() * 2)]), bounds: { minX: x, minY: y, maxX: x + 40, maxY: y + 40 } };
    });
    const start = performance.now();
    const order = sortNodes(nodes);
    const took = performance.now() - start;
    assert.equal(new Set(order).size, 400);
    assert.ok(took < 1000, `took ${took.toFixed(0)} ms`);
  });
});

/**
 * Random boxes, mostly unit blocks and sprites on a small grid so keys tie and boxes interpenetrate.
 * @param {() => number} r
 * @param {number} n
 * @param {number} [span]  how many tiles wide the grid is
 */
function randomBoxes(r, n, span = 8) {
  return Array.from({ length: n }, () => {
    const [c, row, l] = [Math.floor(r() * span), Math.floor(r() * span), Math.floor(r() * 3) - 1];
    if (r() < 0.5) return blockBox(c, row, l, [1 + Math.floor(r() * 2), 1 + Math.floor(r() * 2), Math.floor(r() * 3)]);
    return spriteBox(c + Math.floor(r() * 32) / 32, row + Math.floor(r() * 32) / 32, l, [1 + Math.floor(r() * 2), 1], spriteHeight(Math.floor(r() * 40)));
  });
}

describe('the scene format: depth and level (R5, R7, D1)', () => {
  const dot = sprite(['g']);
  const problems = (items) => validate(src({ dot }, { demo: scene(items) }));

  it('R5: an item or a group may carry depth front or back, and nothing else', () => {
    assert.deepEqual(problems([{ object: 'dot', depth: 'front', at: { px: [0, 0] } }, { group: {}, depth: 'back', items: [{ object: 'dot', depth: 'back', at: { tile: [0, 0, 0] } }] }]), []);
    assert.deepEqual(problems([{ object: 'dot', depth: 'above', at: { px: [0, 0] } }]), ["scenes/demo.mjs: items[0]: depth must be 'front' or 'back'"]);
    assert.deepEqual(problems([{ object: 'dot', depth: 'auto', at: { px: [0, 0] } }]), ["scenes/demo.mjs: items[0]: depth must be 'front' or 'back'"], 'auto is the absence of the key');
    assert.deepEqual(problems([{ group: {}, depth: 1, items: [] }]), ["scenes/demo.mjs: items[0]: depth must be 'front' or 'back'"]);
    assert.deepEqual(problems([{ group: {}, order: 1, items: [] }]), ["scenes/demo.mjs: items[0]: unknown property 'order'"]);
    assert.deepEqual(problems([{ object: 'dot', zIndex: 1, at: { px: [0, 0] } }]), ["scenes/demo.mjs: items[0]: unknown property 'zIndex'"]);
  });

  it('R7: at.px takes a whole-number level, and only at.px does', () => {
    assert.deepEqual(problems([{ object: 'dot', at: { px: [40, -12], level: 4 } }, { object: 'dot', at: { px: [0, 0], level: -2 } }, { object: 'dot', at: { px: [0, 0], level: 0 } }]), []);
    assert.deepEqual(problems([{ object: 'dot', at: { px: [0, 0], level: 1.5 } }]), ['scenes/demo.mjs: items[0]: at.level must be a whole number']);
    assert.deepEqual(problems([{ object: 'dot', at: { px: [0, 0], level: '2' } }]), ['scenes/demo.mjs: items[0]: at.level must be a whole number']);
    assert.deepEqual(problems([{ object: 'dot', at: { tile: [0, 0, 0], level: 1 } }]), ["scenes/demo.mjs: items[0]: at.level is only for at.px; a tile's level is its third number"]);
    const shape = 'scenes/demo.mjs: items[0]: at must be { px: [x, y], level? } or { tile: [col, row, level] }';
    assert.deepEqual(problems([{ object: 'dot', at: { level: 1 } }]), [shape]);
    assert.deepEqual(problems([{ object: 'dot', at: { px: [0, 0], tile: [0, 0, 0] } }]), [shape]);
    assert.deepEqual(problems([{ object: 'dot', at: { px: [0, 0], height: 2 } }]), [shape]);
  });

  it('R5, R7: the serializer writes depth after layer and level after px, and leaves level out at 0', () => {
    const s = {
      viewBox: [0, 0, 8, 8],
      items: [
        { object: 'dot', id: 'a', class: 'x', layer: 'yard', depth: 'front', at: { px: [40, -12], level: 4 } },
        { object: 'dot', depth: 'back', at: { tile: [1, 2, 3] } },
        { object: 'dot', at: { px: [1, 2], level: 0 } },
        { object: 'dot', at: { px: [1, 2], level: -1 } },
        { group: { class: 'enter' }, depth: 'back', items: [{ object: 'dot', at: { px: [0, 0] } }] },
        { group: {}, items: [] },
      ],
    };
    assert.equal(
      serialize(s),
      `// Pixel-art scene. How to edit it: .claude/skills/pixel-art/SKILL.md
export default {
  viewBox: [0, 0, 8, 8],
  items: [
    { object: 'dot', id: 'a', class: 'x', layer: 'yard', depth: 'front', at: { px: [40, -12], level: 4 } },
    { object: 'dot', depth: 'back', at: { tile: [1, 2, 3] } },
    { object: 'dot', at: { px: [1, 2] } },
    { object: 'dot', at: { px: [1, 2], level: -1 } },
    {
      group: { class: 'enter' },
      depth: 'back',
      items: [
        { object: 'dot', at: { px: [0, 0] } },
      ],
    },
    {
      group: {},
      items: [],
    },
  ],
};
`,
    );
  });

  it('R5, R7: every new key survives save and reload byte for byte, and files without them are unchanged', async () => {
    const full = {
      viewBox: [0, 0, 8, 8],
      items: [
        { object: 'dot', id: 'jib', layer: 'crane', depth: 'front', at: { px: [40, -12], level: 4 } },
        { group: { 'data-class': '1' }, depth: 'back', items: [{ object: 'dot', depth: 'front', at: { tile: [0, 0, 0] } }] },
        { object: 'dot', at: { px: [3, 4], level: -2 } },
      ],
    };
    const text = serialize(full);
    assert.deepEqual(await load(text), full);
    assert.equal(serialize(await load(text)), text);
    // A file with none of the new keys writes the same text it always did.
    const plain = scene([{ object: 'dot', at: { px: [1, 2] } }, { group: { class: 'a' }, items: [{ object: 'dot', class: 'b c', at: { tile: [0, 1, 2] } }] }]);
    assert.equal(
      serialize(plain),
      `// Pixel-art scene. How to edit it: .claude/skills/pixel-art/SKILL.md
export default {
  viewBox: [0, 0, 8, 8],
  items: [
    { object: 'dot', at: { px: [1, 2] } },
    {
      group: { class: 'a' },
      items: [
        { object: 'dot', class: 'b c', at: { tile: [0, 1, 2] } },
      ],
    },
  ],
};
`,
    );
  });

  it('R5, R7: scene.items lists an item\'s depth, a pixel item\'s level, and a group\'s depth', async () => {
    const site = await openDocument({ doc: FIXTURE_DOC });
    const flat = flattenItems(
      site,
      scene([
        { object: 'dot', depth: 'front', at: { px: [40, -12], level: 4 } },
        { object: 'dot', at: { px: [1, 2] } },
        { object: 'dot', at: { tile: [1, 2, 3] } },
        { group: { class: 'enter' }, depth: 'back', items: [{ object: 'dot', at: { tile: [0, 0, 0] } }] },
        { group: {}, items: [] },
      ]),
    );
    assert.deepEqual(flat.items.map((i) => [i.path, i.depth, i.level]), [['0', 'front', 4], ['1', undefined, 0], ['2', undefined, undefined], ['3.0', undefined, undefined]]);
    assert.deepEqual(flat.items[0].at, { px: [40, -12], level: 4 });
    assert.deepEqual(flat.groups.map((g) => [g.path, g.depth]), [['3', 'back'], ['4', undefined]]);
  });

  it('D2: a document carries the depth module with the rest of the engine', async () => {
    const site = await openDocument({ doc: FIXTURE_DOC });
    assert.equal(site.depth.sortNodes, sortNodes);
    assert.equal(typeof site.depth.behind, 'function');
  });
});

describe('sprite footprints (R8, D1)', () => {
  const problems = (objects) => validate(src(objects));

  it('R8: a sprite may say how many tiles it covers, whole numbers from 1 to 16', () => {
    assert.deepEqual(problems({ house: sprite(['g'], { footprint: [3, 2] }), tall: sprite(['g'], { footprint: [16, 1] }) }), []);
    const rule = 'footprint must be two whole numbers, tiles wide and deep, from 1 to 16';
    for (const bad of [[0, 1], [1, 17], [1.5, 1], [1], [1, 1, 1], '2,2', { w: 1 }, [-1, 2]]) {
      assert.deepEqual(problems({ house: sprite(['g'], { footprint: bad }) }), [`objects/house.mjs: ${rule}`], JSON.stringify(bad));
    }
  });

  it('R8: a block refuses a footprint, since its size is its footprint', () => {
    assert.deepEqual(problems({ b: block({ footprint: [1, 1] }) }), ["objects/b.mjs: footprint is for sprites; a block's size is its footprint"]);
    assert.deepEqual(problems({ b: block() }), []);
  });

  it('R8: the footprint defaults to 1×1, a block\'s is its size, and extends inherits it', () => {
    const sources = src({
      plain: sprite(['g']),
      house: sprite(['g'], { footprint: [3, 2] }),
      cottage: { kind: 'sprite', extends: 'house', rows: {} },
      shed: { kind: 'sprite', extends: 'house', footprint: [1, 2], rows: {} },
      wall: block({ size: [2, 1, 1] }),
    });
    assert.deepEqual(validate(sources), []);
    assert.deepEqual(resolve(sources, 'plain').footprint, [1, 1]);
    assert.deepEqual(resolve(sources, 'house').footprint, [3, 2]);
    assert.deepEqual(resolve(sources, 'cottage').footprint, [3, 2], 'inherited');
    assert.deepEqual(resolve(sources, 'shed').footprint, [1, 2], 'its own wins');
    assert.deepEqual(resolve(sources, 'wall').footprint, [2, 1]);
    // resolve hands out copies: changing one leaves the object alone.
    resolve(sources, 'house').footprint[0] = 9;
    assert.deepEqual(resolve(sources, 'house').footprint, [3, 2]);
  });

  it('R8: the serializer writes footprint after anchor, and files without it are unchanged', async () => {
    const obj = { kind: 'sprite', extends: 'house', anchor: [5, 13], footprint: [3, 2], keys: { x: 'ink' }, rows: {} };
    assert.equal(
      serialize(obj),
      `// Pixel-art object. How to edit it: .claude/skills/pixel-art/SKILL.md
export default {
  kind: 'sprite',
  extends: 'house',
  anchor: [5, 13],
  footprint: [3, 2],
  keys: {
    x: 'ink',
  },
  rows: {},
};
`,
    );
    assert.deepEqual(await load(serialize(obj)), obj);
    const plain = { kind: 'sprite', anchor: [1, 1], keys: { g: 'ink' }, layers: [{ map: ['g'] }] };
    assert.doesNotMatch(serialize(plain), /footprint/);
    assert.equal(serialize(await load(serialize(plain))), serialize(plain));
  });
});

describe('the engine draws in depth order (R3, R5, R9, D2)', () => {
  const tree = { kind: 'sprite', anchor: [2, 11], keys: { g: 'grass-4' }, layers: [{ map: Array.from({ length: 12 }, () => 'gggg') }] };
  const house = block({ faces: { top: 'cream', left: 'wood-3', right: 'ink' } });
  const OBJECTS = { tree, house };

  /** The pixels an object paints standing at a point, as "x,y" → color name. */
  const paintedAt = (sources, object, [x, y]) => {
    const out = new Map();
    for (const [p, color] of composite(renderObject(sources, object))) {
      const [px, py] = p.split(',').map(Number);
      out.set(`${px + x},${py + y}`, color);
    }
    return out;
  };
  /** A pixel both the tree and the house paint when they stand where the items say, with each one's color. */
  const overlap = (sources, a, b) => {
    const [pa, pb] = [a, b].map(({ object, at }) => paintedAt(sources, object, at.tile ? tileToPx(at.tile, [0, 0]) : at.px));
    const shared = [...pa.keys()].filter((p) => pb.has(p));
    assert.ok(shared.length > 0, 'the fixtures overlap on screen');
    return { p: shared[0], a: pa.get(shared[0]), b: pb.get(shared[0]), count: shared.length };
  };
  const colorAt = (sources, p, options) => composite(renderScene(sources, 's', options).root).get(p);
  const treeBehind = { object: 'tree', at: { tile: [0, 1, 0] } };
  const treeInFront = { object: 'tree', at: { tile: [2, 1, 0] } };
  const redHouse = { object: 'house', at: { tile: [1, 1, 1] } };

  it('R3: a file listed out of order draws in depth order: the tree behind the house is under it, the tree in front is over it', () => {
    for (const [items, label] of [[[treeBehind, redHouse], 'behind, listed first'], [[redHouse, treeBehind], 'behind, listed last']]) {
      const sources = src(OBJECTS, { s: scene(items, { viewBox: [-40, -30, 80, 70] }) });
      const { p, b } = overlap(sources, treeBehind, redHouse);
      assert.equal(colorAt(sources, p), b, label);
    }
    for (const items of [[treeInFront, redHouse], [redHouse, treeInFront]]) {
      const sources = src(OBJECTS, { s: scene(items, { viewBox: [-40, -30, 80, 70] }) });
      const { p, a } = overlap(sources, treeInFront, redHouse);
      assert.equal(colorAt(sources, p), a, 'in front');
    }
  });

  it('R8: a wide sprite with a footprint sorts by the ground it covers, in the boxes and in the pixels', () => {
    // A 56×16 slab that reaches right from its tile, drawn in grass-2, beside a 4×12 tree in grass-4 two tiles right and a row back.
    const slab = (extra = {}) => ({ kind: 'sprite', anchor: [4, 15], keys: { g: 'grass-2' }, layers: [{ map: Array.from({ length: 16 }, () => 'g'.repeat(56)) }], ...extra });
    const items = [{ object: 'slab', at: { tile: [0, 2, 0] } }, { object: 'tree', at: { tile: [2, 1, 0] } }];
    const sourcesOf = (extra) => src({ slab: slab(extra), tree }, { s: scene(items, { viewBox: [-60, -30, 120, 70] }) });
    const [plain, wide] = [sourcesOf(), sourcesOf({ footprint: [3, 1] })];
    // The boxes: on one tile the two cannot be told apart, and over three columns the tree stands behind the slab.
    const standing = spriteBox(2, 1, 0, [1, 1], 12 / 16);
    assert.equal(behind(standing, spriteBox(0, 2, 0, [1, 1], 16 / 16)), false);
    assert.equal(behind(spriteBox(0, 2, 0, [1, 1], 16 / 16), standing), false, 'no order on one tile');
    assert.equal(behind(standing, spriteBox(0, 2, 0, [3, 1], 16 / 16)), true, 'the tree is behind the three-tile slab');
    // The engine: the same scene lists the slab first. With no footprint it stays first; with one the tree moves under it.
    assert.deepEqual(depthOrder(plain, plain.scenes.get('s').items), [0, 1]);
    assert.deepEqual(depthOrder(wide, wide.scenes.get('s').items), [1, 0]);
    // The pixels: where both paint, the tree is over the slab without a footprint and under it with one.
    const both = [...paintedAt(plain, 'slab', tileToPx([0, 2, 0], [0, 0])).keys()].filter((p) => paintedAt(plain, 'tree', tileToPx([2, 1, 0], [0, 0])).has(p));
    assert.ok(both.length > 0, 'the fixtures overlap on screen');
    assert.equal(colorAt(plain, both[0]), 'grass-4', 'tree over slab');
    assert.equal(colorAt(wide, both[0]), 'grass-2', 'slab over tree');
  });

  it('R3: order list draws the items as the file lists them, as engines before the depth order did', () => {
    const sources = src(OBJECTS, { s: scene([redHouse, treeBehind], { viewBox: [-40, -30, 80, 70] }) });
    const { p, a, b } = overlap(sources, treeBehind, redHouse);
    assert.notEqual(a, b);
    assert.equal(colorAt(sources, p, { order: 'list' }), a, 'listed last, the tree paints over the house');
    assert.equal(colorAt(sources, p), b, 'depth order is the default');
    assert.equal(colorAt(sources, p, { order: 'depth' }), b);
  });

  it('R3: the draw order is the same tree whichever way the file lists a sorted scene', () => {
    const sources = src(OBJECTS, { s: scene([treeBehind, redHouse, treeInFront], { viewBox: [-40, -30, 80, 70] }) });
    const sorted = sceneInOrder(sources, sources.scenes.get('s'));
    const both = src(OBJECTS, { s: sources.scenes.get('s'), t: sorted });
    assert.deepEqual(renderScene(both, 's').root, renderScene(both, 't', { order: 'list' }).root);
    assert.deepEqual(renderScene(both, 's').root, renderScene(both, 't').root);
  });

  it('R5: a front mark draws an item over what stands in front of it, and a back mark under what stands behind', () => {
    const view = { viewBox: [-40, -30, 80, 70] };
    let sources = src(OBJECTS, { s: scene([treeInFront, { ...redHouse, depth: 'front' }], view) });
    let o = overlap(sources, treeInFront, redHouse);
    assert.equal(colorAt(sources, o.p), o.b, 'the house is marked front, so it paints over the tree in front of it');
    sources = src(OBJECTS, { s: scene([{ ...treeBehind, depth: 'front' }, redHouse], view) });
    o = overlap(sources, treeBehind, redHouse);
    assert.equal(colorAt(sources, o.p), o.a, 'the tree behind it is marked front, so it paints over the house');
    sources = src(OBJECTS, { s: scene([{ ...redHouse, depth: 'back' }, treeBehind, treeInFront], view) });
    assert.deepEqual(depthOrder(sources, sources.scenes.get('s').items), [0, 1, 2], 'a back-marked house draws first');
    sources = src(OBJECTS, { s: scene([treeBehind, treeInFront, { ...redHouse, depth: 'back' }], view) });
    assert.deepEqual(depthOrder(sources, sources.scenes.get('s').items), [2, 0, 1]);
  });

  it('R3: depthOrder, sceneInOrder and isInDepthOrder agree, and sceneInOrder returns a copy', () => {
    const sources = src(OBJECTS, { s: scene([redHouse, treeInFront, treeBehind]) });
    const original = sources.scenes.get('s');
    assert.deepEqual(depthOrder(sources, original.items), [2, 0, 1]);
    assert.equal(isInDepthOrder(sources, original), false);
    const sorted = sceneInOrder(sources, original);
    assert.deepEqual(sorted.items.map((i) => i.object + i.at.tile.join()), ['tree0,1,0', 'house1,1,1', 'tree2,1,0']);
    assert.equal(isInDepthOrder(sources, sorted), true);
    assert.deepEqual(original.items.map((i) => i.object), ['house', 'tree', 'tree'], 'the original is untouched');
    assert.deepEqual(sceneInOrder(sources, sorted), sorted, 'sorting a sorted scene returns it');
    sorted.items[0].at.tile[0] = 99;
    assert.deepEqual(original.items[2].at.tile, [0, 1, 0], 'and shares nothing with the copy');
  });

  it('R3: a tracked item rests where its first key puts it', () => {
    const items = [{ object: 'tree', id: 'walker', at: { tile: [0, 1, 0] } }, redHouse];
    const motion = (move) => ({ duration: 1000, tracks: [{ item: 'walker', keys: [{ t: 0, move }, { t: 500, move: [0, 0] }] }] });
    const behindHouse = src(OBJECTS, { s: scene(items, { motion: motion([0, 0]) }) }).scenes.get('s');
    const sources = src(OBJECTS, { a: behindHouse, b: scene(items, { motion: motion([32, 16]) }) });
    const tracks = (s) => new Map(s.motion.tracks.map((t) => [t.item, t]));
    assert.deepEqual(depthOrder(sources, sources.scenes.get('a').items, [0, 0], tracks(sources.scenes.get('a'))), [0, 1], 'at rest behind the house');
    assert.deepEqual(depthOrder(sources, sources.scenes.get('b').items, [0, 0], tracks(sources.scenes.get('b'))), [1, 0], 'its first key moves it in front');
    assert.ok(isInDepthOrder(sources, sources.scenes.get('a')));
    assert.ok(!isInDepthOrder(sources, sources.scenes.get('b')));
  });

  it('R9: a group sorts as one object, by the union of its members, and its members sort inside it', () => {
    const wrap = (items) => ({ group: { class: 'enter' }, items });
    const view = { viewBox: [-40, -30, 80, 70] };
    // The house and the tree in front are one group; the tree behind them is listed after the group.
    const sources = src(OBJECTS, { s: scene([wrap([treeInFront, redHouse]), treeBehind], view) });
    const items = sources.scenes.get('s').items;
    assert.deepEqual(depthOrder(sources, items), [1, 0], 'the group draws after the tree behind it');
    const sorted = sceneInOrder(sources, sources.scenes.get('s'));
    assert.deepEqual(sorted.items[1].items.map((i) => i.object + i.at.tile.join()), ['house1,1,1', 'tree2,1,0'], 'the members were sorted inside it');
    assert.deepEqual(sorted.items[1].group, { class: 'enter' }, 'the wrapper is the same');
    assert.equal(isInDepthOrder(sources, sorted), true);
    assert.equal(isInDepthOrder(sources, sources.scenes.get('s')), false);
    // Each member is still drawn inside its group's <g>.
    const tree = renderScene(sources, 's').root;
    assert.equal(tree.children.length, 2);
    assert.ok('pixels' in tree.children[0], 'the tree behind paints first, at the top level');
    assert.deepEqual(tree.children[1].attrs, [['class', 'enter']], 'then the group, with the house and the tree in front inside it');
    const o = overlap(sources, treeInFront, redHouse);
    assert.equal(colorAt(sources, o.p), o.a, 'inside the group the tree in front paints over the house');
  });

  it('R9, R5: a group carries its own depth mark, and the marks of its members act inside it only', () => {
    const wrap = (items, extra = {}) => ({ group: { class: 'enter' }, ...extra, items });
    const sources = src(OBJECTS, { s: scene([wrap([redHouse], { depth: 'back' }), treeBehind, wrap([{ ...treeInFront, depth: 'back' }, redHouse])]) });
    const items = sources.scenes.get('s').items;
    // The back-marked group goes first although the tree is behind it; the member marked back stays inside its own group.
    assert.deepEqual(depthOrder(sources, items), [0, 1, 2]);
    const sorted = sceneInOrder(sources, sources.scenes.get('s'));
    assert.deepEqual(sorted.items[2].items.map((i) => i.object), ['tree', 'house'], 'a back mark inside a group puts the member first in the group');
  });

  it('R9: a run of consecutive variant groups sorts as one unit and keeps its own order', () => {
    const variant = (n, item) => ({ group: { 'data-class': String(n) }, items: [item] });
    const near = { object: 'house', at: { tile: [3, 3, 1] } };
    const far = { object: 'house', at: { tile: [1, 1, 1] } };
    // The second variant is behind the first, yet they stay together and in this order; the tree behind both goes first.
    const sources = src(OBJECTS, { s: scene([variant(0, near), variant(1, far), treeBehind], { viewBox: [-60, -50, 120, 100] }) });
    const items = sources.scenes.get('s').items;
    assert.deepEqual(depthOrder(sources, items), [2, 0, 1]);
    const sorted = sceneInOrder(sources, sources.scenes.get('s'));
    assert.deepEqual(sorted.items.map((i) => i.group?.['data-class'] ?? i.object), ['tree', '0', '1']);
    // composite's firstVariant still shows only the run's first.
    const root = renderScene(sources, 's').root;
    const first = composite(root, { firstVariant: true });
    const all = composite(root);
    const only = (sourcesOf, item) => paintedAt(sourcesOf, item.object, tileToPx(item.at.tile, [0, 0]));
    const [pNear, pFar] = [only(sources, near), only(sources, far)];
    const farOnly = [...pFar.keys()].find((p) => !pNear.has(p));
    assert.ok(all.has(farOnly), 'the second variant draws when all show');
    assert.ok(!first.has(farOnly), 'and not when only the first of the run shows');
    assert.ok(first.has([...pNear.keys()][0]));
  });

  it('R9, R2: variant groups that sorting brings side by side form a run, and then stay put', () => {
    const variant = (n, item) => ({ group: { 'data-class': String(n) }, items: [item] });
    // The tree behind the first variant is listed between the two, so it goes first and they end up together.
    const sources = src(OBJECTS, {
      s: scene([variant(0, { object: 'house', at: { tile: [3, 3, 1] } }), { object: 'tree', at: { tile: [2, 3, 0] } }, variant(1, { object: 'house', at: { tile: [4, 3, 1] } })]),
    });
    const once = sceneInOrder(sources, sources.scenes.get('s'));
    assert.deepEqual(once.items.map((i) => i.group?.['data-class'] ?? i.object), ['tree', '0', '1']);
    assert.equal(isInDepthOrder(sources, once), true);
    assert.deepEqual(sceneInOrder(sources, once), once);
  });

  it('R2: on random scenes with groups and variants, sorting twice is sorting once, and the render matches the sorted copy', () => {
    for (const seed of SEEDS) {
      const r = rng(seed);
      const pick = (list) => list[Math.floor(r() * list.length)];
      const leaf = () => ({
        object: pick(['tree', 'house']),
        ...(r() < 0.15 ? { depth: pick(['front', 'back']) } : {}),
        at: r() < 0.5 ? { tile: [Math.floor(r() * 6), Math.floor(r() * 6), Math.floor(r() * 3)] } : { px: [Math.floor(r() * 100) - 50, Math.floor(r() * 60) - 20], ...(r() < 0.5 ? { level: Math.floor(r() * 3) } : {}) },
      });
      const list = (n, depthLeft) =>
        Array.from({ length: n }, () => {
          if (depthLeft > 0 && r() < 0.3) {
            const group = r() < 0.5 ? { 'data-class': String(Math.floor(r() * 3)) } : { class: 'enter' };
            return { group, ...(r() < 0.15 ? { depth: pick(['front', 'back']) } : {}), items: list(1 + Math.floor(r() * 3), depthLeft - 1) };
          }
          return leaf();
        });
      for (let round = 0; round < 12; round++) {
        const original = scene(list(4 + Math.floor(r() * 25), 2), { viewBox: [-80, -60, 160, 140] });
        const sources = src(OBJECTS, { s: original });
        const once = sceneInOrder(sources, original);
        const check = src(OBJECTS, { s: original, once, again: sceneInOrder(sources, once) });
        assert.deepEqual(check.scenes.get('again'), once, `seed ${seed}, round ${round}: sorted twice is sorted once`);
        assert.equal(isInDepthOrder(check, once), true, `seed ${seed}, round ${round}`);
        const sortedPixels = (name, options) => [...composite(renderScene(check, name, options).root, { firstVariant: false })].sort();
        assert.deepEqual(sortedPixels('s'), sortedPixels('once', { order: 'list' }), `seed ${seed}, round ${round}: depth order draws what the sorted copy draws`);
      }
    }
  });

  it('R3: the compiler, the preview and scene.render all draw the tree under the house, whatever order the file lists them', async () => {
    const files = {
      'objects/tree.mjs': serialize(tree),
      'objects/house.mjs': serialize(house),
      'scenes/depth.mjs': serialize(scene([redHouse, treeBehind], { output: 'depth.svg', viewBox: [-40, -30, 80, 70] })),
    };
    const doc = await makeDocument({ files });
    try {
      const { sources } = await readSources(doc.root);
      const { p, a, b } = overlap(sources, treeBehind, redHouse);
      const hex = (name) => sources.colors.get(name).hex;
      const [x, y] = p.split(',').map(Number);
      assert.notEqual(a, b);

      const compiled = compileScene(sources, 'depth').output;
      let painted;
      for (const m of compiled.matchAll(/<path fill="(#[0-9A-F]{6})" d="([^"]*)"\/>/g)) {
        for (const run of m[2].matchAll(/M(-?\d+) (-?\d+)h(\d+)v(\d+)h-\d+z/g)) {
          const [rx, ry, w, h] = run.slice(1).map(Number);
          if (x >= rx && x < rx + w && y >= ry && y < ry + h) painted = m[1];
        }
      }
      assert.equal(painted, hex(b), 'the compiled SVG paints the house last at the shared pixel');

      const { image, origin } = renderPreview(sources, { scene: 'depth' }, previewColors());
      const at = ((y - origin[1]) * image.width + (x - origin[0])) * 4;
      assert.equal(`#${[...image.rgba.subarray(at, at + 3)].map((v) => v.toString(16).padStart(2, '0')).join('').toUpperCase()}`, hex(b), 'the preview PNG');

      const ctx = createContext(await openDocument({ doc: doc.root }));
      const r = await registry.call(ctx, 'scene.render', { name: 'depth' });
      const png = decodePng(Buffer.from(r.png, 'base64'));
      assert.deepEqual(png.rgba(x - r.viewBox[0], y - r.viewBox[1]).slice(0, 3), [1, 3, 5].map((i) => parseInt(hex(b).slice(i, i + 2), 16)), 'scene.render');
    } finally {
      await doc.cleanup();
    }
  });
});

describe("the harbor example: a pine moved behind the red house (R3, intent)", () => {
  it('R3: harbor-flat\'s front pine moved by -5,-5 draws behind the house\'s wall, where the old order drew it over', async () => {
    const { sources, loadErrors } = await readSources(join(REPO, 'examples', 'harbor.pixellab'));
    assert.deepEqual(loadErrors, []);
    const flat = structuredClone(sources.scenes.get('harbor-flat'));
    const pine = flat.items.find((item) => item.group && item.items[0].object === 'pine' && item.items[0].at.tile.join() === '5,6,0').items[0];
    pine.at.tile = [0, 1, 0];
    const walls = flat.items.find((item) => item.group && item.items[0].object === 'walls' && item.items[0].at.tile.join() === '1,1,1').items[0];
    const moved = { ...sources, scenes: new Map([['moved', flat]]) };
    const paintedAt = (object, [x, y]) => new Map([...composite(renderObject(sources, object))].map(([p, color]) => { const [px, py] = p.split(',').map(Number); return [`${px + x},${py + y}`, color]; }));
    const roof = flat.items.find((item) => item.group && item.items[0].object === 'roof-red').items[0];
    const [pinePixels, wallPixels, roofPixels] = [pine, walls, roof].map((item) => paintedAt(item.object, tileToPx(item.at.tile, [0, 0])));
    // A pixel of the moved pine that the wall paints and the roof above it does not.
    const p = [...pinePixels.keys()].find((q) => wallPixels.has(q) && !roofPixels.has(q));
    assert.ok(p, 'the moved pine overlaps the wall on screen');
    assert.notEqual(pinePixels.get(p), wallPixels.get(p));
    assert.equal(composite(renderScene(moved, 'moved').root).get(p), wallPixels.get(p), 'depth order: the wall is over the pine');
    assert.equal(composite(renderScene(moved, 'moved', { order: 'list' }).root).get(p), pinePixels.get(p), 'the old order drew the pine over the wall');
    const order = depthOrder(moved, flat.items, [0, 0], new Map());
    const at = (object, tile) => flat.items.findIndex((item) => (item.group ? item.items[0] : item).object === object && (item.group ? item.items[0] : item).at.tile.join() === tile);
    assert.ok(order.indexOf(at('pine', '0,1,0')) < order.indexOf(at('walls', '1,1,1')), 'the pine comes before the walls in the depth order');
    assert.ok(order.indexOf(at('walls', '1,1,1')) < order.indexOf(at('roof-red', '1,1,2')));
  });

  it('R3: the hall\'s 2×1 walls and roof draw over a tree one row behind them and under a tree one row in front', async () => {
    const { sources } = await readSources(join(REPO, 'examples', 'harbor.pixellab'));
    const base = structuredClone(sources.scenes.get('harbor-flat'));
    const hall = [{ object: 'hall-walls', at: { tile: [2, 5, 1] } }, { object: 'hall-roof', at: { tile: [2, 5, 2] } }];
    const trees = [{ object: 'tree', at: { tile: [3, 6, 0] } }, { object: 'tree', at: { tile: [3, 4, 0] } }];
    const lone = { ...sources, scenes: new Map([['hall', { ...base, items: [...trees, ...hall] }]]) };
    const order = depthOrder(lone, lone.scenes.get('hall').items);
    const where = (i) => order.indexOf(i);
    // trees[1] (3,4,0) is one row behind the hall's far half: it draws before the walls and the roof.
    assert.ok(where(1) < where(2) && where(1) < where(3), `${order}`);
    // trees[0] (3,6,0) is one row in front of it: it draws after the walls (it stands under the roof's level, so the roof is not ordered against it).
    assert.ok(where(0) > where(2), `${order}`);
  });
});

describe('the sort moves as few items as it can (R2)', () => {
  const unit = (c, r, l) => blockBox(c, r, l, [1, 1, 1]);
  const FREE = { bounds: null };

  /** The first listed free node, always (what the sort did before it also tried pulling blockers forward). */
  function firstFree(nodes) {
    const n = nodes.length;
    const waiting = nodes.map((b, j) => nodes.filter((a, i) => i !== j && overlaps(a.bounds, b.bounds) && behind(a.box, b.box)).length);
    const placed = new Set();
    const out = [];
    while (out.length < n) {
      const rest = nodes.map((_, i) => i).filter((i) => !placed.has(i));
      const free = rest.find((i) => waiting[i] === 0);
      const next = free ?? rest.reduce((a, b) => (keyOf(nodes[b].box) < keyOf(nodes[a].box) ? b : a));
      placed.add(next);
      out.push(next);
      nodes.forEach((b, j) => {
        if (!placed.has(j) && overlaps(nodes[next].bounds, b.bounds) && behind(nodes[next].box, b.box)) waiting[j]--;
      });
    }
    return out;
  }

  it('R2: displaced counts the items out of the longest run that keeps its order', () => {
    assert.equal(displaced([]), 0);
    assert.equal(displaced([0, 1, 2, 3]), 0);
    assert.equal(displaced([3, 0, 1, 2]), 1, 'one item jumped far');
    assert.equal(displaced([1, 2, 3, 0]), 1);
    assert.equal(displaced([2, 3, 4, 5, 0, 1]), 2);
    assert.equal(displaced([3, 2, 1, 0]), 3);
  });

  it('R2: an item that must come before two others listed far ahead of it is pulled forward, not left for them to wait on', () => {
    // W1 and W2 are in front of P but listed first; f1…f3 meet nothing. Holding W1 and W2 back would displace both; pulling P forward displaces one.
    const nodes = [
      { box: unit(3, 0, 0), bounds: { minX: 0, minY: 0, maxX: 9, maxY: 9 } },
      { box: unit(4, 0, 0), bounds: { minX: 0, minY: 0, maxX: 9, maxY: 9 } },
      { box: unit(9, 9, 0), ...FREE },
      { box: unit(8, 9, 0), ...FREE },
      { box: unit(7, 9, 0), ...FREE },
      { box: unit(0, 0, 0), bounds: { minX: 0, minY: 0, maxX: 9, maxY: 9 } },
    ];
    assert.deepEqual(firstFree(nodes), [2, 3, 4, 5, 0, 1]);
    assert.deepEqual(sortNodes(nodes), [5, 0, 1, 2, 3, 4], 'only P moves, up in front of the walls');
  });

  it('R2: an item listed early that must come after two later ones is the one that moves down', () => {
    const nodes = [
      { box: unit(3, 0, 0), bounds: { minX: 0, minY: 0, maxX: 9, maxY: 9 } },
      { box: unit(9, 9, 0), ...FREE },
      { box: unit(8, 9, 0), ...FREE },
      { box: unit(0, 0, 0), bounds: { minX: 0, minY: 0, maxX: 9, maxY: 9 } },
      { box: unit(1, 0, 0), bounds: { minX: 0, minY: 0, maxX: 9, maxY: 9 } },
    ];
    assert.deepEqual(sortNodes(nodes), [1, 2, 3, 4, 0], 'pulling the two forward would displace two; holding the one back displaces one');
  });

  it('R2: where both ways displace as many items, the one that holds the blocked item back is kept', () => {
    const nodes = [
      { box: unit(3, 0, 0), bounds: { minX: 0, minY: 0, maxX: 9, maxY: 9 } },
      { box: unit(9, 9, 0), ...FREE },
      { box: unit(8, 9, 0), ...FREE },
      { box: unit(0, 0, 0), bounds: { minX: 0, minY: 0, maxX: 9, maxY: 9 } },
    ];
    assert.deepEqual(sortNodes(nodes), [1, 2, 3, 0]);
  });

  it('R2: the harbor pine moved behind the red house moves one item, and back again moves one item', async () => {
    const { sources } = await readSources(join(REPO, 'examples', 'harbor.pixellab'));
    const flat = structuredClone(sources.scenes.get('harbor-flat'));
    const lone = { ...sources, scenes: new Map([['flat', flat]]) };
    const pineAt = (list, tile) => list.findIndex((item) => item.group && item.items[0].object === 'pine' && item.items[0].at.tile.join() === tile);
    assert.deepEqual(depthOrder(lone, flat.items), flat.items.map((_, i) => i), 'the example is in order');
    const before = pineAt(flat.items, '5,6,0');
    flat.items[before].items[0].at.tile = [0, 1, 0];
    const up = depthOrder(lone, flat.items);
    assert.equal(displaced(up), 1, 'only the pine moves');
    assert.ok(up.indexOf(before) < before, 'up in front of the walls it stands behind');
    // Sort it, then put it back where it stood.
    const sorted = up.map((i) => flat.items[i]);
    flat.items.splice(0, flat.items.length, ...sorted);
    assert.deepEqual(depthOrder(lone, flat.items), flat.items.map((_, i) => i));
    const at = pineAt(flat.items, '0,1,0');
    flat.items[at].items[0].at.tile = [5, 6, 0];
    const down = depthOrder(lone, flat.items);
    assert.equal(displaced(down), 1, 'only the pine moves');
    assert.ok(down.indexOf(at) > at, 'down behind everything it stands in front of');
  });

  it('R2: never displaces more than holding blocked items back would, and a second sort changes nothing, on random scenes with cycles', () => {
    for (const seed of SEEDS) {
      const r = rng(seed + 7);
      for (let round = 0; round < 40; round++) {
        const boxes = randomBoxes(r, 5 + Math.floor(r() * 50), 5);
        const nodes = boxes.map((box, i) => ({ box, bounds: r() < 0.5 ? EVERYWHERE : { minX: i * 3, minY: 0, maxX: i * 3 + 2 + Math.floor(r() * 8), maxY: 5 } }));
        const order = sortNodes(nodes);
        assert.ok(displaced(order) <= displaced(firstFree(nodes)), `seed ${seed}, round ${round}`);
        const once = order.map((i) => nodes[i]);
        assert.deepEqual(sortNodes(once), once.map((_, i) => i), `seed ${seed}, round ${round}: sorted lists stay`);
      }
    }
  });
});
