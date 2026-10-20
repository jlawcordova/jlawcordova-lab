// Depth order (docs/intents/2026-10-depth-order/spec.md): the geometry and
// the sort (R1, R2), the scene and object formats (R5, R7, R8), and the
// engine drawing in depth order (R3, R9). Small inline fixtures, Node
// built-ins only. Test names start with the requirement they cover.

import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { behind, blockBox, keyOf, overlaps, shiftBox, sortNodes, spriteBox, spriteHeight, unionBounds, unionBox } from '../lib/engine/depth.mjs';
import { pxToTile, pxToTileExact, tileToPx } from '../lib/engine/iso.mjs';

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

  it('R1: items whose pixels cannot meet are not ordered by their boxes, only by the fixed key', () => {
    // b is behind a by the box test (column), but they draw in different places.
    const a = { box: blockBox(5, 0, 0, [1, 1, 1]), bounds: { minX: 0, minY: 0, maxX: 10, maxY: 10 } };
    const b = { box: blockBox(0, 0, 0, [1, 1, 1]), bounds: { minX: 100, minY: 0, maxX: 110, maxY: 10 } };
    assert.deepEqual(sortNodes([a, b]), [1, 0], 'smaller key first');
    const c = { box: blockBox(0, 0, 0, [1, 1, 1]), bounds: null };
    const d = { box: null, bounds: null };
    assert.deepEqual(sortNodes([a, c, d]), [2, 1, 0], 'a node with no box first, then by key');
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

  it('R2: with distinct keys the answer does not depend on the list order', () => {
    for (const seed of SEEDS) {
      const r = rng(seed);
      const boxes = randomBoxes(r, 40);
      const keys = new Set(boxes.map(keyOf));
      if (keys.size !== boxes.length) continue;
      const labels = boxes.map((_, i) => i);
      const want = sortNodes(boxes.map((b) => node(b))).map((i) => labels[i]);
      for (let round = 0; round < 3; round++) {
        const shuffled = labels.slice().sort(() => r() - 0.5);
        const got = sortNodes(shuffled.map((i) => node(boxes[i]))).map((i) => shuffled[i]);
        assert.deepEqual(got, want, `seed ${seed}, round ${round}`);
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
