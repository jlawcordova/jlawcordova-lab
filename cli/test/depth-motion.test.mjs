// Depth order while items move (docs/intents/2026-10-depth-order/spec.md, R10,
// D6): the order at a time with the still items' chain, the places a moving
// item takes and the windows it spends in them, the copies the SVG draws, and
// the preview at a time matching them. Scenes are built in memory. Test
// names start with the requirement they cover.

import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { behind, blockBox, keyOf, movingOrder, overlaps, placesIn, placeWindows, sortNodes, spriteBox, spriteHeight } from '../lib/engine/depth.mjs';
import { loadSources, motionCopies } from '../lib/engine/engine.mjs';
import { isMoving, moveAtPercent, moveRuns, pctOf } from '../lib/engine/motion.mjs';

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
/** A 1×1×1 block (the house) and a small sprite (the walker) standing at its bottom middle. */
const OBJECTS = { house: block(), walker: sprite(['.gg.', 'gggg', 'gggg', '.ii.'], { anchor: [2, 4] }) };
/** A scene whose motion is `tracks`, a loop of `duration` ms. */
const scene = (items, tracks = [], duration = 10000) => ({ viewBox: [-80, -20, 160, 140], items, ...(tracks.length ? { motion: { duration, tracks } } : {}) });
const src = (scenes, objects = OBJECTS) => loadSources({ palette: PALETTE, objects, scenes });
const track = (item, ...keys) => ({ item, keys });

/** Pixel bounds big enough that every pair of nodes is tested. */
const EVERYWHERE = { minX: -1000, minY: -1000, maxX: 1000, maxY: 1000 };

/**
 * The order at an instant, written plainly and checked against movingOrder:
 * every "behind" pair among the nodes as they are now, the still chain, the
 * first listed node that is free, the smallest key among those the chain
 * doesn't hold back when none is.
 */
function naiveOrder(nodes, current) {
  const at = (i) => current.get(i) ?? nodes[i];
  const out = [];
  for (const band of ['back', undefined, 'front']) {
    const members = nodes.map((_, i) => i).filter((i) => nodes[i].depth === band);
    const before = new Map(members.map((i) => [i, members.filter((j) => j !== i && overlaps(at(j).bounds, at(i).bounds) && behind(at(j).box, at(i).box))]));
    const stills = members.filter((i) => !nodes[i].moving);
    const chain = new Map(stills.slice(1).map((i, k) => [i, stills[k]]));
    const placed = new Set();
    while (placed.size < members.length) {
      const rest = members.filter((i) => !placed.has(i));
      const held = (i) => chain.has(i) && !placed.has(chain.get(i));
      const free = rest.filter((i) => !held(i) && before.get(i).every((j) => placed.has(j)));
      const next = free.length ? free[0] : rest.filter((i) => !held(i)).reduce((a, b) => (keyOf(at(b).box) < keyOf(at(a).box) ? b : a));
      placed.add(next);
      out.push(next);
    }
  }
  return out;
}

/** Random nodes in rest order, some of them moving, on a grid small enough for ties, cycles and interpenetration. */
function randomList(r, n) {
  const boxes = Array.from({ length: n }, () => {
    const [c, row, l] = [Math.floor(r() * 6), Math.floor(r() * 6), Math.floor(r() * 3) - 1];
    if (r() < 0.5) return blockBox(c, row, l, [1 + Math.floor(r() * 2), 1 + Math.floor(r() * 2), Math.floor(r() * 3)]);
    return spriteBox(c + Math.floor(r() * 32) / 32, row + Math.floor(r() * 32) / 32, l, [1 + Math.floor(r() * 2), 1], spriteHeight(Math.floor(r() * 40)));
  });
  const bounds = () => (r() < 0.75 ? EVERYWHERE : { minX: Math.floor(r() * 30), minY: 0, maxX: 30 + Math.floor(r() * 30), maxY: 5 });
  const nodes = boxes.map((box) => ({ box, bounds: bounds(), ...(r() < 0.12 ? { depth: r() < 0.5 ? 'front' : 'back' } : {}) }));
  // The list is in the order the rest sort gives, which is what movingOrder is given.
  const listed = sortNodes(nodes).map((i) => ({ ...nodes[i], moving: r() < 0.3 }));
  return { listed, boxes: randomBoxes(r, n) };
}
const randomBoxes = (r, n) => Array.from({ length: n }, () => blockBox(Math.floor(r() * 6), Math.floor(r() * 6), Math.floor(r() * 3) - 1, [1, 1, Math.floor(r() * 3)]));

describe('the order while moving: still chain, places, windows (R10, D6)', () => {
  const unit = (c, r, l) => blockBox(c, r, l, [1, 1, 1]);
  const node = (box, extra = {}) => ({ box, bounds: EVERYWHERE, ...extra });

  it('R10: a moving node sits behind, between or in front of the still ones as its box says, and the still ones never change order', () => {
    // a stands behind b; m (moving) is placed at three instants.
    const nodes = [node(unit(0, 0, 0)), node(unit(4, 0, 0)), node(unit(2, 0, 0), { moving: true })];
    const orderAt = movingOrder(nodes);
    assert.deepEqual(orderAt(new Map()), [0, 2, 1], 'at rest m is between a and b');
    assert.deepEqual(orderAt(new Map([[2, node(unit(-3, 0, 0), { moving: true })]])), [2, 0, 1], 'behind both');
    assert.deepEqual(orderAt(new Map([[2, node(unit(7, 0, 0), { moving: true })]])), [0, 1, 2], 'in front of both');
    assert.deepEqual(orderAt(new Map([[2, node(unit(-3, 0, 0), { moving: true })]])).filter((i) => i !== 2), [0, 1]);
  });

  it('R10: still nodes keep one order even where their own boxes would put them the other way', () => {
    // a stands behind b by their boxes, but they never meet in pixels, so the rest sort keeps the order the file lists them in (b, a).
    // A mover that overlaps both ties them together at some instants; the chain still holds b before a.
    const a = { box: unit(0, 0, 0), bounds: { minX: 0, minY: 0, maxX: 10, maxY: 10 } };
    const b = { box: unit(5, 0, 0), bounds: { minX: 20, minY: 0, maxX: 30, maxY: 10 } };
    const m = { box: unit(2, 0, 0), bounds: { minX: 0, minY: 0, maxX: 30, maxY: 10 }, moving: true };
    const raw = [b, a, m];
    const rest = sortNodes(raw);
    const listed = rest.map((i) => raw[i]);
    const names = ['b', 'a', 'm'];
    const stills = rest.filter((i) => i !== 2).map((i) => names[i]);
    assert.deepEqual(movingOrder(listed)(new Map()), listed.map((_, i) => i), 'at rest it is the rest order');
    for (const c of [-3, 2, 9]) {
      const order = movingOrder(listed)(new Map([[rest.indexOf(2), { box: unit(c, 0, 0), bounds: m.bounds }]]));
      assert.deepEqual(order.map((i) => names[rest[i]]).filter((n) => n !== 'm'), stills, `with m at column ${c}: ${order}`);
    }
  });

  it('R10: with nothing moved from rest the order is the rest order, on random lists with ties and cycles', () => {
    for (const seed of SEEDS) {
      const r = rng(seed);
      for (let round = 0; round < 40; round++) {
        const { listed } = randomList(r, 4 + Math.floor(r() * 30));
        assert.deepEqual(movingOrder(listed)(new Map()), listed.map((_, i) => i), `seed ${seed}, round ${round}`);
      }
    }
  });

  it('R10: the order while moving agrees with a plain re-sort of the whole list, on random scenes', () => {
    for (const seed of SEEDS) {
      const r = rng(seed * 3 + 1);
      for (let round = 0; round < 60; round++) {
        const n = 4 + Math.floor(r() * 30);
        const { listed, boxes } = randomList(r, n);
        const orderAt = movingOrder(listed);
        for (let instant = 0; instant < 5; instant++) {
          // Every moving node is somewhere new, with its own pixels (sometimes missing everything).
          const current = new Map(
            listed.flatMap((nd, i) => (nd.moving ? [[i, { box: boxes[Math.floor(r() * n)], bounds: r() < 0.8 ? EVERYWHERE : { minX: 500, minY: 0, maxX: 510, maxY: 5 } }]] : [])),
          );
          const real = orderAt(current);
          assert.deepEqual(real, naiveOrder(listed, current), `seed ${seed}, round ${round}, instant ${instant}`);
          const stills = real.filter((i) => !listed[i].moving);
          const wanted = listed.flatMap((nd, i) => (nd.moving ? [] : [i]));
          // Bands put front and back nodes apart; within the whole order the stills only move by band, which rest order already has.
          assert.deepEqual(stills, wanted, `seed ${seed}, round ${round}, instant ${instant}: the stills keep their order`);
        }
      }
    }
  });

  it('R10: a place is the still node a moving one follows and its rank among the moving ones after it', () => {
    const nodes = [{ moving: true }, { moving: false }, { moving: true }, { moving: true }, { moving: false }, { moving: true }];
    const places = placesIn([0, 1, 3, 2, 4, 5], nodes);
    assert.deepEqual([...places], [
      [0, { follows: -1, rank: 0 }],
      [3, { follows: 1, rank: 0 }],
      [2, { follows: 1, rank: 1 }],
      [5, { follows: 4, rank: 0 }],
    ]);
  });

  it('R10: windows are the spans a node spends in one place, merged, each place in the order it first shows', () => {
    const cuts = [0, 10, 20, 35, 60, 100];
    const spans = [
      new Map([[1, { follows: 0, rank: 0 }], [2, { follows: 0, rank: 1 }]]),
      new Map([[1, { follows: 0, rank: 0 }], [2, { follows: 0, rank: 1 }]]),
      new Map([[1, { follows: -1, rank: 0 }], [2, { follows: 0, rank: 0 }]]),
      new Map([[1, { follows: -1, rank: 0 }], [2, { follows: 0, rank: 0 }]]),
      new Map([[1, { follows: 0, rank: 0 }], [2, { follows: 0, rank: 1 }]]),
    ];
    const got = placeWindows(cuts, (span) => spans[span]);
    assert.deepEqual(got.get(1), [
      { follows: 0, rank: 0, windows: [[0, 20], [60, 100]] },
      { follows: -1, rank: 0, windows: [[20, 60]] },
    ]);
    assert.deepEqual(got.get(2), [
      { follows: 0, rank: 1, windows: [[0, 20], [60, 100]] },
      { follows: 0, rank: 0, windows: [[20, 60]] },
    ]);
  });
});

describe('where a moving item is over the loop, as the CSS steps it (R10, D6)', () => {
  it('R10: key percentages are rounded to 3 decimals, as motionCss writes them', () => {
    assert.equal(pctOf(8529, 28000), 30.461);
    assert.equal(pctOf(5000, 10000), 50);
    assert.equal(pctOf(28000, 28000), 100);
  });

  it('R10: a move steps at from + (to - from) × k / n between two keys and loops back at the end', () => {
    const t = track('w', { t: 0 }, { t: 5000, move: [0, 4] });
    assert.equal(isMoving(t, 'walker'), true);
    assert.equal(isMoving(track('w', { t: 0 }, { t: 5000, show: false }), 'walker'), false, 'a track that only shows and hides stays put');
    const runs = moveRuns(t, 'walker', 10000);
    assert.deepEqual(runs.map((r) => r.at), [0, 12.5, 25, 37.5, 50, 62.5, 75, 87.5]);
    assert.deepEqual(runs.map((r) => r.move[1]), [0, 1, 2, 3, 4, 3, 2, 1]);
    assert.deepEqual(moveAtPercent(runs, 0), [0, 0]);
    assert.deepEqual(moveAtPercent(runs, 12.49), [0, 0]);
    assert.deepEqual(moveAtPercent(runs, 12.5), [0, 1], 'a step takes effect at its instant');
    assert.deepEqual(moveAtPercent(runs, 99.99), [0, 1]);
  });

  it('R10: a run starts where the move changes, and holds across keys that do not move it', () => {
    const runs = moveRuns(track('w', { t: 0 }, { t: 2000, move: [2, 1] }, { t: 4000 }, { t: 6000, move: [2, 1], show: false }), 'walker', 8000);
    // One (2, 1) step arrives at the key at 25%, the move holds through the keys at 50% and 75%, and the one step back is the loop's end.
    assert.deepEqual(runs, [
      { at: 0, move: [0, 0] },
      { at: 25, move: [2, 1] },
    ]);
  });
});

describe('the copies an item draws (R10, D6)', () => {
  it('R10: a walker that passes behind a house and then in front of it takes two places', () => {
    // The house stands at tile 2,2 on level 1; the walker goes down the column the house is in.
    const sources = src({ s: scene([{ object: 'house', at: { tile: [2, 2, 1] } }, { object: 'walker', id: 'w', at: { px: [0, 0] } }], [track('w', { t: 0 }, { t: 5000, move: [0, 64] })]) });
    assert.deepEqual(motionCopies(sources, 's'), [{ item: 'w', copies: 2 }]);
  });

  it('R10: an item that never meets another in pixels, or never changes order with it, has one place and no copies', () => {
    const away = src({ s: scene([{ object: 'house', at: { tile: [6, 0, 0] } }, { object: 'walker', id: 'w', at: { px: [-60, 0] } }], [track('w', { t: 0 }, { t: 5000, move: [0, 40] })]) });
    assert.deepEqual(motionCopies(away, 's'), []);
    // It walks up to the house from the front and stops short of it.
    const front = src({ s: scene([{ object: 'house', at: { tile: [0, 0, 1] } }, { object: 'walker', id: 'w', at: { px: [0, 60] } }], [track('w', { t: 0 }, { t: 5000, move: [0, 10] })]) });
    assert.deepEqual(motionCopies(front, 's'), []);
    assert.deepEqual(motionCopies(src({ s: scene([{ object: 'walker', id: 'w', at: { px: [0, 0] } }]) }), 's'), [], 'no motion at all');
  });

  it('R10: two items that cross each other each take two places, told apart by rank', () => {
    // They walk toward each other down one column and pass for a few pixels: for those steps the one that has gone past is in front.
    const sources = src({
      s: scene(
        [{ object: 'walker', id: 'a', at: { px: [0, 0] } }, { object: 'walker', id: 'b', at: { px: [0, 64] } }],
        [track('a', { t: 0 }, { t: 5000, move: [0, 64] }), track('b', { t: 0 }, { t: 5000, move: [0, -64] })],
      ),
    });
    assert.deepEqual(motionCopies(sources, 's'), [{ item: 'a', copies: 2 }, { item: 'b', copies: 2 }]);
  });

  it('R10: a moving item inside a group reorders among the group\'s members only: the group keeps its rest depth', () => {
    const inGroup = src({
      s: scene(
        [
          { object: 'house', at: { tile: [4, 4, 1] } },
          { group: { class: 'enter' }, items: [{ object: 'house', at: { tile: [2, 2, 1] } }, { object: 'walker', id: 'w', at: { px: [0, 0] } }] },
        ],
        [track('w', { t: 0 }, { t: 5000, move: [0, 64] })],
      ),
    });
    // The walker passes behind and in front of its own group's house.
    assert.deepEqual(motionCopies(inGroup, 's'), [{ item: 'w', copies: 2 }]);
    // Alone in its group it has nothing to reorder with, though it crosses a house outside the group.
    const alone = src({
      s: scene(
        [{ object: 'house', at: { tile: [2, 2, 1] } }, { group: { class: 'enter' }, items: [{ object: 'walker', id: 'w', at: { px: [0, 0] } }] }],
        [track('w', { t: 0 }, { t: 5000, move: [0, 64] })],
      ),
    });
    assert.deepEqual(motionCopies(alone, 's'), []);
  });

  it('R10: the number of places does not depend on the order the file lists the items in', () => {
    const items = [{ object: 'house', at: { tile: [2, 2, 1] } }, { object: 'walker', id: 'w', at: { px: [0, 0] } }];
    const tracks = [track('w', { t: 0 }, { t: 5000, move: [0, 64] })];
    assert.deepEqual(motionCopies(src({ s: scene(items, tracks) }), 's'), motionCopies(src({ s: scene([...items].reverse(), tracks) }), 's'));
  });
});
