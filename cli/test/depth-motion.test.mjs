// Depth order while items move (docs/intents/2026-10-depth-order/spec.md, R10,
// D6): the order at a time with the still items' chain, the places a moving
// item takes and the windows it spends in them, the copies the SVG draws, and
// the preview at a time matching them. Scenes are built in memory. Test
// names start with the requirement they cover.

import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { behind, blockBox, keyOf, movingOrder, overlaps, placesIn, placeWindows, sortNodes, spriteBox, spriteHeight } from '../lib/engine/depth.mjs';
import { createContext } from '../lib/context.mjs';
import { openDocument } from '../lib/document.mjs';
import { composite, loadSources, motionCopies, renderScene, sceneAt, validate } from '../lib/engine/engine.mjs';
import { isMoving, loopFrame, moveAtPercent, moveRuns, motionCss, pctOf, shownItems } from '../lib/engine/motion.mjs';
import { poseItems } from '../lib/engine/motion.mjs';
import { toRectSvg } from '../lib/engine/svg.mjs';
import { registry } from '../lib/registry.mjs';
import { renderSceneImage } from '../lib/render.mjs';
import { decodePng, makeDocument } from './helpers.mjs';

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


/** The pixels where two painted maps differ. */
const differences = (a, b) => [...new Set([...a.keys(), ...b.keys()])].filter((p) => a.get(p) !== b.get(p));

// ------------------------------------------------------- a small CSS evaluator

/**
 * Reads the CSS a scene's motion writes: its @keyframes (stops with their
 * declarations) and its class rules. Only the shapes motion.mjs writes.
 */
function parseCss(css) {
  const keyframes = new Map();
  const rules = new Map();
  let reduced = null;
  const decls = (text) => Object.fromEntries(text.split(';').filter(Boolean).map((d) => [d.slice(0, d.indexOf(':')), d.slice(d.indexOf(':') + 1)]));
  let at = 0;
  const block = () => {
    const open = css.indexOf('{', at);
    let depth = 1;
    let end = open + 1;
    while (depth) depth += css[end++] === '{' ? 1 : css[end - 1] === '}' ? -1 : 0;
    const out = { head: css.slice(at, open), body: css.slice(open + 1, end - 1) };
    at = end;
    return out;
  };
  while (at < css.length) {
    const { head, body } = block();
    if (head.startsWith('@keyframes ')) {
      const stops = [];
      for (const m of body.matchAll(/([\d.]+)%\{([^}]*)\}/g)) stops.push({ at: Number(m[1]), decl: decls(m[2]) });
      keyframes.set(head.slice('@keyframes '.length), stops);
    } else if (head.startsWith('@media')) reduced = body;
    else rules.set(head.slice(1), decls(body));
  }
  return { keyframes, rules, reduced };
}

/** The numbers in a `translate(x,y)` value, or null. */
const translation = (value) => {
  const m = /^translate\(([-\d.]+)(?:px)?,([-\d.]+)(?:px)?\)$/.exec(value);
  return m && [Number(m[1]), Number(m[2])];
};

/** A property's value in a keyframe animation at `p` percent of the loop, with step-end and steps(n,end) timing. */
function sampleKeyframes(stops, timing, p) {
  let i = -1;
  stops.forEach((stop, k) => {
    if (stop.at <= p) i = k;
  });
  if (i < 0) return {};
  const from = stops[i];
  const to = stops[i + 1];
  const fn = from.decl['animation-timing-function'] ?? timing;
  const out = {};
  for (const [prop, value] of Object.entries(from.decl)) {
    if (prop === 'animation-timing-function') continue;
    const steps = /^steps\((\d+),end\)$/.exec(fn);
    if (!steps || !to) out[prop] = value;
    else {
      const k = Math.floor((Number(steps[1]) * (p - from.at)) / (to.at - from.at));
      const [a, b] = [translation(value), translation(to.decl[prop])];
      out[prop] = `translate(${a[0] + ((b[0] - a[0]) * k) / Number(steps[1])},${a[1] + ((b[1] - a[1]) * k) / Number(steps[1])})`;
    }
  }
  return out;
}

/** A class's opacity and translation at `p` percent of the loop; with `animate: false`, with animations off (reduced motion). */
function styleOf(css, cls, p, animate) {
  const rule = css.rules.get(cls);
  if (!rule) return { opacity: 1, translate: [0, 0] };
  const style = { opacity: 'opacity' in rule ? Number(rule.opacity) : 1, translate: [0, 0] };
  if (!animate || !rule.animation) return style;
  for (const part of rule.animation.split(/,(?![^(]*\))/)) {
    const [name, , timing] = part.split(' ');
    const got = sampleKeyframes(css.keyframes.get(name), timing, p);
    if ('opacity' in got) style.opacity = Number(got.opacity);
    if ('transform' in got) style.translate = translation(got.transform);
  }
  return style;
}

/**
 * The pixels the published tree shows at `p` percent of a loop of `duration`
 * ms, reading only its CSS: mv- groups take the opacity and translation the
 * rules and keyframes give, frame loops show the frame of that time.
 */
function paintCss(scene, p, duration, animate = true) {
  const css = parseCss(scene.style ?? '');
  const time = animate ? (p / 100) * duration : 0;
  const out = new Map();
  const walk = (group, dx, dy) => {
    for (const child of group.children) {
      if ('pixels' in child) {
        for (const [key, color] of child.pixels) {
          const [x, y] = key.split(',').map(Number);
          out.set(`${x + dx},${y + dy}`, color);
        }
        continue;
      }
      const cls = child.attrs.find(([k]) => k === 'class')?.[1] ?? '';
      if (child.frame !== undefined && child.frame !== loopFrame(cls.split(' ')[0], child.frames ?? 1, time)) continue;
      let [ox, oy] = [dx, dy];
      if (cls.startsWith('mv-') && !cls.includes(' ')) {
        const style = styleOf(css, cls, p, animate);
        if (style.opacity === 0) continue;
        [ox, oy] = [dx + style.translate[0], dy + style.translate[1]];
      }
      walk(child, ox, oy);
    }
  };
  walk(scene.root, 0, 0);
  return out;
}

/** The classes of `mv-` groups in a tree, in tree order. */
const mvClasses = (group) =>
  group.children.flatMap((child) => {
    if ('pixels' in child) return [];
    const cls = child.attrs.find(([k]) => k === 'class')?.[1] ?? '';
    return [...(cls.startsWith('mv-') ? [cls] : []), ...mvClasses(child)];
  });

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

describe('the SVG draws a copy for each place (R10, D6)', () => {
  const walkerScene = () => src({ s: scene([{ object: 'house', at: { tile: [2, 2, 1] } }, { object: 'walker', id: 'w', at: { px: [0, 0] } }], [track('w', { t: 0 }, { t: 5000, move: [0, 64] })]) });
  const svg = (sources, name = 's') => toRectSvg(renderScene(sources, name), (color) => sources.colors.get(color).hex);

  it('R10: each copy is a wrapper holding the whole moving group, in its place in the list, and only the copy holding time 0 shows in the base rule', () => {
    const sources = walkerScene();
    const out = renderScene(sources, 's');
    // The walker is behind the house for part of the loop (its copy z1 sits before the house) and in front of it the rest.
    assert.deepEqual(mvClasses(out.root), ['mv-w-z1', 'mv-w', 'mv-w-z0', 'mv-w']);
    assert.match(out.style, /\.mv-w-z0\{animation:mv-w-z0 10000ms step-end infinite\}/);
    assert.match(out.style, /\.mv-w-z1\{opacity:0;animation:mv-w-z1 10000ms step-end infinite\}/);
    assert.match(out.style, /@keyframes mv-w-z0\{0%\{opacity:1\}7\.0313%\{opacity:0\}27\.3438%\{opacity:1\}73\.4375%\{opacity:0\}93\.75%\{opacity:1\}\}/);
    assert.match(out.style, /@keyframes mv-w-z1\{0%\{opacity:0\}7\.0313%\{opacity:1\}27\.3438%\{opacity:0\}73\.4375%\{opacity:1\}93\.75%\{opacity:0\}\}/);
    // Each copy holds the item's own motion group, whose rules are the item's as before.
    assert.match(out.style, /@keyframes mv-w\{0%\{transform:translate\(0,0\)/);
    const mixed = svg(sources);
    assert.equal(mixed.match(/<g class="mv-w">/g).length, 2);
  });

  it('R10: exactly one copy shows at every moment of the loop, and the one holding time 0 shows at rest', () => {
    const sources = walkerScene();
    const out = renderScene(sources, 's');
    const css = parseCss(out.style);
    const copies = ['mv-w-z0', 'mv-w-z1'];
    for (let p = 0; p < 100; p += 0.0625) {
      const shown = copies.filter((c) => styleOf(css, c, p, true).opacity === 1);
      assert.equal(shown.length, 1, `at ${p}%: ${shown}`);
    }
    for (const boundary of [7.0313, 27.3438, 73.4375, 93.75]) {
      const before = copies.filter((c) => styleOf(css, c, boundary - 0.0001, true).opacity === 1);
      const at = copies.filter((c) => styleOf(css, c, boundary, true).opacity === 1);
      assert.notDeepEqual(before, at, `a copy switches at ${boundary}%`);
      assert.equal(at.length, 1);
    }
    assert.deepEqual(copies.filter((c) => styleOf(css, c, 0, false).opacity === 1), ['mv-w-z0'], 'reduced motion shows the copy of time 0 only');
    assert.match(out.style, /@media \(prefers-reduced-motion:reduce\)\{\[class\^="mv-"\],\[class\*=" mv-"\]\{animation:none\}\}$/, 'the reduced-motion rule covers the copies');
    assert.ok(copies.every((c) => c.startsWith('mv-')));
  });

  it('R10: the CSS steps a track exactly as moveRuns says, wherever it is asked', () => {
    // Keys on whole percentages so the steps fall on exact floating-point values.
    const tracks = [
      track('w', { t: 0 }, { t: 2500, move: [0, 8] }, { t: 5000, move: [8, 12] }, { t: 7500, move: [2, 1] }),
      track('x', { t: 0 }, { t: 5000, move: [-4, 4], show: false }, { t: 7500, show: true }),
    ];
    const sources = src({ s: scene([{ object: 'walker', id: 'w', at: { px: [0, 0] } }, { object: 'walker', id: 'x', at: { px: [30, 0] } }], tracks) });
    const out = renderScene(sources, 's');
    const css = parseCss(out.style);
    for (const t of tracks) {
      const runs = moveRuns(t, 'walker', 10000);
      const rest = runs[0].move;
      const cuts = [...runs.map((r) => r.at), 100];
      for (let i = 0; i < runs.length; i++) {
        for (const p of [cuts[i], (cuts[i] + cuts[i + 1]) / 2, cuts[i + 1] - 0.001]) {
          const got = styleOf(css, `mv-${t.item}`, p, true).translate;
          assert.deepEqual(got, [runs[i].move[0] - rest[0], runs[i].move[1] - rest[1]], `${t.item} at ${p}%`);
        }
      }
    }
  });

  it('R10: an item whose place never changes is drawn once, byte for byte as the list-order engine draws it', () => {
    const items = [{ object: 'house', at: { tile: [6, 0, 0] } }, { object: 'walker', id: 'w', at: { px: [-60, 0] } }];
    const sources = src({ s: scene(items, [track('w', { t: 0 }, { t: 5000, move: [0, 40] }, { t: 7000, move: [0, 40], pose: 'walker' })]) });
    assert.deepEqual(motionCopies(sources, 's'), []);
    const depth = renderScene(sources, 's');
    const list = renderScene(sources, 's', { order: 'list' });
    assert.equal(toRectSvg(depth, (c) => sources.colors.get(c).hex), toRectSvg(list, (c) => sources.colors.get(c).hex));
    assert.equal(depth.style, motionCss(sources.scenes.get('s').motion, new Map([['w', items[1]]])));
    assert.ok(!/-z\d/.test(depth.style));
  });

  it('R10: order list draws no copies: the file\'s order, as engines before the depth order drew it', () => {
    const sources = walkerScene();
    const list = renderScene(sources, 's', { order: 'list' });
    assert.deepEqual(mvClasses(list.root), ['mv-w']);
    assert.ok(!/-z\d/.test(list.style));
  });

  it('R10: a copy holds everything the item draws as before: its class wrapper, its poses and its show rule', () => {
    const objects = { ...OBJECTS, 'walker-b': sprite(['.gg.', 'gggg', 'gggg', 'ii..'], { anchor: [2, 4] }) };
    const items = [{ object: 'house', at: { tile: [2, 2, 1] } }, { object: 'walker', id: 'w', class: 'cargo', at: { px: [0, 0] } }];
    const sources = src({ s: scene(items, [track('w', { t: 0 }, { t: 2500, pose: 'walker-b' }, { t: 5000, move: [0, 64], show: false })]) }, objects);
    const out = renderScene(sources, 's');
    assert.deepEqual(mvClasses(out.root), ['mv-w-z1', 'mv-w', 'mv-w-0', 'mv-w-1', 'mv-w-z0', 'mv-w', 'mv-w-0', 'mv-w-1']);
    const copy = out.root.children.find((c) => 'attrs' in c && c.attrs[0][1] === 'mv-w-z0');
    assert.equal(copy.children.length, 1, 'one moving group in the copy');
    assert.equal(copy.children[0].children[0].children[0].attrs[0][1], 'cargo', 'the item class wraps its pixels in each pose');
    for (const rule of ['@keyframes mv-w-s', '@keyframes mv-w-0', '@keyframes mv-w-1', '.mv-w-1{opacity:0;']) assert.ok(out.style.includes(rule), rule);
  });

  it('R10: a group member\'s copies are inside its group, and the group draws where it rests', () => {
    const sources = src({
      s: scene(
        [
          { object: 'house', at: { tile: [4, 4, 1] } },
          { group: { class: 'enter' }, items: [{ object: 'house', at: { tile: [2, 2, 1] } }, { object: 'walker', id: 'w', at: { px: [0, 0] } }] },
        ],
        [track('w', { t: 0 }, { t: 5000, move: [0, 64] })],
      ),
    });
    const out = renderScene(sources, 's');
    const group = out.root.children.find((c) => 'attrs' in c && c.attrs.some(([k, v]) => k === 'class' && v === 'enter'));
    assert.ok(group, 'the group is a <g> of its own');
    assert.deepEqual(mvClasses(group), ['mv-w-z1', 'mv-w', 'mv-w-z0', 'mv-w']);
    assert.deepEqual(mvClasses(out.root), mvClasses(group), 'no copy outside the group');
    assert.equal(out.root.children.length, 2, 'the outside house and the group');
  });

  it('R10: windows are written to 4 decimals, and stops that round to one point collapse into the later', () => {
    const motion = { duration: 10000, tracks: [{ item: 'w', keys: [{ t: 0 }, { t: 5000, move: [0, 4] }] }] };
    const ids = new Map([['w', { object: 'walker' }]]);
    const at = (windows) => motionCss(motion, ids, new Map([['w', windows.map((w) => ({ windows: w }))]])).match(/@keyframes mv-w-z\d\{(?:[^{}]*\{[^{}]*\})*\}/g);
    assert.deepEqual(at([[[0, 25], [75, 100]], [[25, 75]]]).map((k) => k.replace(/^@keyframes /, '')), ['mv-w-z0{0%{opacity:1}25%{opacity:0}75%{opacity:1}}', 'mv-w-z1{0%{opacity:0}25%{opacity:1}75%{opacity:0}}']);
    assert.deepEqual(at([[[0, 25.000012], [25.000012, 100]], [[25.00004, 25.00004]]]).map((k) => k.replace(/^@keyframes /, '')), ['mv-w-z0{0%{opacity:1}}', 'mv-w-z1{0%{opacity:0}}'], 'a window shorter than 0.0001% is never shown');
    assert.deepEqual(at([[[0, 12.34567]], [[12.34567, 100]]]).map((k) => k.replace(/^@keyframes /, '')), ['mv-w-z0{0%{opacity:1}12.3457%{opacity:0}}', 'mv-w-z1{0%{opacity:0}12.3457%{opacity:1}}']);
  });

  it('R10: an id that looks like another track\'s copy is a problem only once that copy exists', () => {
    const items = (extra) => [{ object: 'house', at: { tile: [2, 2, 1] } }, { object: 'walker', id: 'w', at: { px: [0, 0] } }, { object: 'walker', id: 'w-z1', at: { px: [40, 0] } }, ...extra];
    const tracks = [track('w', { t: 0 }, { t: 5000, move: [0, 64] }), track('w-z1', { t: 0 }, { t: 5000, show: false })];
    const problems = validate(src({ s: scene(items([]), tracks) }));
    assert.equal(problems.filter((p) => /w-z1/.test(p) && /copy 1 of 'w'/.test(p)).length, 1, problems.join('\n'));
    // With the walker nowhere near the house there are no copies, so no clash.
    const apart = scene([{ object: 'house', at: { tile: [6, 0, 0] } }, { object: 'walker', id: 'w', at: { px: [-60, 0] } }, { object: 'walker', id: 'w-z1', at: { px: [40, 0] } }], [track('w', { t: 0 }, { t: 5000, move: [0, 40] }), track('w-z1', { t: 0 }, { t: 5000, show: false })]);
    assert.deepEqual(validate(src({ s: apart })), []);
  });

  it('R10, R12: a scene of hero-island\'s size (220 items, 39 moving ones with 25 keys) draws in a few seconds', () => {
    const r = rng(2026);
    const items = [];
    for (let c = 0; c < 15; c++) for (let row = 0; row < 15; row++) items.push({ object: r() < 0.5 ? 'house' : 'walker', at: { tile: [c, row, r() < 0.2 ? 1 : 0] } });
    const tracks = [];
    for (let i = 0; i < 39; i++) {
      const id = `m${i}`;
      items.push({ object: 'walker', id, at: { px: [Math.floor(r() * 200) - 100, Math.floor(r() * 120)] } });
      const keys = [{ t: 0 }];
      let [x, y] = [0, 0];
      for (let k = 1; k < 25; k++) {
        [x, y] = [x + 2 * (Math.floor(r() * 11) - 5), y + (Math.floor(r() * 11) - 5)];
        keys.push({ t: k * 1000 + Math.floor(r() * 500), move: [x, y] });
      }
      tracks.push(track(id, ...keys));
    }
    const sources = src({ s: scene(items, tracks, 28000) });
    const started = performance.now();
    const out = renderScene(sources, 's');
    const took = performance.now() - started;
    const copies = motionCopies(sources, 's');
    assert.ok(copies.length > 0, 'items cross others in the order');
    assert.ok(out.style.includes('-z0'), 'the CSS has copies');
    assert.ok(took < 20000, `took ${took.toFixed(0)} ms`);
  });
});

describe('the preview draws what the published SVG shows (R10, R15, D6)', () => {
  /**
   * A scene whose keys fall on whole percentages and whose steps on exact ones,
   * with items crossing each other and still ones: a walker down a column past
   * two houses, one up the same column the other way, a diagonal one, a group
   * with its own house and walker, and one that hides, changes pose and shows
   * again.
   */
  const NICE = (() => {
    const objects = {
      ...OBJECTS,
      'walker-b': sprite(['.gg.', 'gggg', 'gggg', 'ii..'], { anchor: [2, 4] }),
      tall: sprite(['.ii.', 'gggg', 'gggg', 'gggg', 'gggg', 'gggg', '.ii.'], { anchor: [2, 7] }),
    };
    const items = [
      { object: 'house', at: { tile: [2, 2, 1] } },
      { object: 'house', at: { tile: [4, 1, 1] } },
      { object: 'tall', at: { tile: [-1, 3, 0] } },
      { object: 'walker', id: 'down', at: { px: [0, -4] } },
      { object: 'walker', id: 'up', at: { px: [0, 60] } },
      { object: 'tall', id: 'diag', at: { px: [-40, 12] } },
      { object: 'walker', id: 'blink', class: 'cargo', at: { px: [30, 18] } },
      {
        group: { class: 'enter' },
        items: [{ object: 'house', at: { tile: [-1, 2, 1] } }, { object: 'walker', id: 'inner', at: { px: [-60, 4] } }, { object: 'walker', at: { px: [-60, 40] } }],
      },
    ];
    const tracks = [
      track('down', { t: 0 }, { t: 2500, move: [0, 20] }, { t: 5000, move: [0, 40] }, { t: 7500, move: [0, 40] }),
      track('up', { t: 0 }, { t: 5000, move: [0, -40] }, { t: 7500, move: [0, -40] }),
      track('diag', { t: 0 }, { t: 5000, move: [40, 20] }),
      track('blink', { t: 0 }, { t: 2500, move: [0, 10], pose: 'walker-b' }, { t: 5000, move: [0, 20], show: false }, { t: 7500, show: true }),
      track('inner', { t: 0 }, { t: 5000, move: [40, 20] }),
    ];
    return { objects, scene: scene(items, tracks) };
  })();

  /** The scene as a document in a temp folder, with its sources as the engine loads them. */
  async function open(nice = NICE) {
    const files = Object.fromEntries([
      ...Object.entries(nice.objects).map(([name, obj]) => [`objects/${name}.mjs`, `export default ${JSON.stringify(obj)};\n`]),
      ['scenes/nice.mjs', `export default ${JSON.stringify(nice.scene)};\n`],
    ]);
    const doc = await makeDocument({ files });
    const ctx = createContext(await openDocument({ doc: doc.root }));
    const { sources } = await ctx.sources();
    return { ctx, sources, doc };
  }

  /** The PNG that scene.render makes at `time`, as a map of "x,y" → hex of the pixels it paints. */
  async function previewAt(ctx, time) {
    // scene.render takes whole milliseconds; a window can end between two, which the same function renders.
    const r = Number.isInteger(time) ? await registry.call(ctx, 'scene.render', { name: 'nice', time }) : renderSceneImage(ctx.site, (await ctx.sources()).sources, 'nice', { time });
    const png = decodePng(Buffer.from(r.png, 'base64'));
    const [vx, vy] = r.viewBox;
    const out = new Map();
    for (let y = 0; y < png.height; y++) {
      for (let x = 0; x < png.width; x++) {
        if (!png.alpha(x, y)) continue;
        out.set(`${x + vx},${y + vy}`, png.rgba(x, y).slice(0, 3).map((v) => v.toString(16).padStart(2, '0')).join(''));
      }
    }
    return out;
  }

  /** A painted map with colors as lower-case hex digits, cut to the scene's view box as the PNG is. */
  const inHex = (sources, pixels) => {
    const [vx, vy, w, h] = NICE.scene.viewBox;
    const inside = ([x, y]) => x >= vx && y >= vy && x < vx + w && y < vy + h;
    return new Map([...pixels].filter(([p]) => inside(p.split(',').map(Number))).map(([p, color]) => [p, sources.colors.get(color).hex.slice(1).toLowerCase()]));
  };

  /** Every time to check, in ms: each copy window's boundary, each step of every track, and the middle of every stretch between them. */
  function checkTimes(scn, out) {
    const { duration, tracks } = scn.motion;
    const css = parseCss(out.style);
    const cuts = new Set([0]);
    for (const [name, stops] of css.keyframes) if (/-z\d+$/.test(name)) for (const stop of stops) cuts.add(stop.at);
    for (const t of tracks) for (const run of moveRuns(t, 'walker', duration)) cuts.add(run.at);
    const sorted = [...cuts].sort((a, b) => a - b);
    const times = new Set();
    sorted.forEach((at, i) => {
      times.add((at / 100) * duration);
      const next = sorted[i + 1] ?? 100;
      times.add(Math.round((((at + next) / 2) / 100) * duration));
    });
    return { times: [...times].sort((a, b) => a - b), boundaries: sorted };
  }

  it('R10, R15: at every window boundary and the middle of every window, the SVG\'s tree with its CSS evaluated at that time paints the pixels scene.render paints', async () => {
    const { ctx, sources, doc } = await open();
    try {
      const out = renderScene(sources, 'nice');
      const copies = motionCopies(sources, 'nice');
      assert.ok(copies.length >= 3, `items cross others and take several places: ${JSON.stringify(copies)}`);
      assert.ok(copies.some((c) => c.item === 'inner'), 'a group member reorders among its group');
      const { times, boundaries } = checkTimes(NICE.scene, out);
      assert.ok(boundaries.length > 40 && times.length > 80, `${boundaries.length} cuts, ${times.length} times`);
      let sawDifference = false;
      for (const time of times) {
        const p = time / (NICE.scene.motion.duration / 100);
        const expected = previewAt(ctx, time);
        const published = inHex(sources, paintCss(out, p, NICE.scene.motion.duration));
        const got = await expected;
        const bad = differences(got, published);
        assert.equal(bad.length, 0, `at ${time} ms (${p}%): ${bad.length} pixels differ, e.g. ${bad.slice(0, 3).map((q) => `${q}: preview ${got.get(q)}, svg ${published.get(q)}`)}`);
        // What the engine drew before the depth order (the list's order, nothing reordered) must differ somewhere, or this proves nothing.
        const scn = sources.scenes.get('nice');
        const listed = renderScene({ ...sources, scenes: new Map([['nice', { ...scn, motion: undefined, items: poseItems(scn.items, scn.motion, time, (item) => (item.at.tile ? [(item.at.tile[0] - item.at.tile[1]) * 16, (item.at.tile[0] + item.at.tile[1]) * 8 - item.at.tile[2] * 16] : item.at.px)) }]]) }, 'nice', { order: 'list' });
        const old = inHex(sources, composite(listed.root, { time }));
        if (differences(old, published).length) sawDifference = true;
      }
      assert.ok(sawDifference, 'drawing in list order is wrong somewhere, so the copies matter');
    } finally {
      await doc.cleanup();
    }
  });

  it('R10, R15: reduced motion (animations off) shows the frame of time 0', async () => {
    const { ctx, sources, doc } = await open();
    try {
      const out = renderScene(sources, 'nice');
      const rest = await previewAt(ctx, 0);
      assert.equal(differences(rest, inHex(sources, paintCss(out, 0, NICE.scene.motion.duration, false))).length, 0);
      assert.equal(differences(rest, inHex(sources, paintCss(out, 0, NICE.scene.motion.duration, true))).length, 0, 'and so does the animation at 0%');
      // Every copy but the one holding time 0 is hidden by its base rule.
      const css = parseCss(out.style);
      const hidden = [...css.rules].filter(([cls, rule]) => /-z\d+$/.test(cls) && rule.opacity === '0').map(([cls]) => cls);
      assert.ok(hidden.length > 0 && hidden.every((cls) => !cls.endsWith('-z0')));
    } finally {
      await doc.cleanup();
    }
  });

  it('R10: sceneAt lists the items in the order drawn at that time, and an item hides and shows while keeping its place', async () => {
    const { sources, doc } = await open();
    try {
      const at = (time) => sceneAt(sources, 'nice', time).items.map((i) => ('group' in i ? 'group' : (i.id ?? i.object)));
      const rest = at(0);
      assert.equal(rest.length, 8, 'every item and the group');
      assert.deepEqual(sceneAt(sources, 'nice', 0).items.map((i) => i.path.join('.')).sort(), ['0', '1', '2', '3', '4', '5', '6', '7']);
      const moved = sceneAt(sources, 'nice', 3500).items;
      assert.notDeepEqual(moved.map((i) => i.path.join('.')), sceneAt(sources, 'nice', 0).items.map((i) => i.path.join('.')), 'the order changed');
      const blink = (time) => sceneAt(sources, 'nice', time).items.find((i) => i.id === 'blink');
      assert.equal(blink(3000).hidden, undefined);
      assert.equal(blink(6000).hidden, true, 'hidden while it does not show, still in the list');
      assert.equal(blink(3000).object, 'walker-b', 'drawing its pose');
      assert.equal(sceneAt(sources, 'nice', 0).motion, undefined);
    } finally {
      await doc.cleanup();
    }
  });

  it('R10, R15: on random scenes with keys at arbitrary milliseconds, the SVG and the preview agree in the middle of every stretch', () => {
    for (const seed of SEEDS.slice(0, 3)) {
      const r = rng(seed + 100);
      const items = [];
      for (let i = 0; i < 14; i++) items.push({ object: ['house', 'walker', 'tall'][Math.floor(r() * 3)], at: { tile: [Math.floor(r() * 7) - 1, Math.floor(r() * 7) - 1, Math.floor(r() * 2)] } });
      const tracks = [];
      for (let i = 0; i < 5; i++) {
        const id = `m${i}`;
        items.splice(Math.floor(r() * items.length), 0, { object: ['walker', 'tall'][Math.floor(r() * 2)], id, at: { px: [Math.floor(r() * 80) - 40, Math.floor(r() * 50)] } });
        const keys = [{ t: 0 }];
        let [x, y] = [0, 0];
        for (let k = 1; k <= 5; k++) {
          const along = r() < 0.5 ? [2, 1] : [0, 1];
          const steps = 4 + Math.floor(r() * 24);
          [x, y] = [x + along[0] * steps * (r() < 0.5 ? -1 : 1), y + along[1] * steps * (r() < 0.5 ? -1 : 1)];
          keys.push({ t: k * 1900 + Math.floor(r() * 1500), move: [x, y], ...(r() < 0.2 ? { show: r() < 0.5 } : {}) });
        }
        tracks.push(track(id, ...keys));
      }
      const scn = scene(items, tracks, 12000);
      const sources = src({ s: scn }, { ...OBJECTS, tall: sprite(['.ii.', 'gggg', 'gggg', 'gggg', 'gggg', 'gggg', '.ii.'], { anchor: [2, 7] }) });
      assert.deepEqual(validate(sources), [], `seed ${seed}`);
      const out = renderScene(sources, 's');
      const css = parseCss(out.style);
      const cuts = new Set([0]);
      for (const [name, stops] of css.keyframes) if (/-z\d+$/.test(name)) for (const stop of stops) cuts.add(stop.at);
      for (const t of tracks) for (const run of moveRuns(t, 'walker', 12000)) cuts.add(run.at);
      const sorted = [...cuts].sort((a, b) => a - b);
      let checked = 0;
      const stride = Math.ceil(sorted.length / 150);
      sorted.forEach((at, i) => {
        const next = sorted[i + 1] ?? 100;
        if (i % stride || (next - at) * 120 < 6) return; // a stretch of under 6 ms: the CSS and the preview round it differently
        const time = ((at + next) / 2) * 120;
        const at0 = sceneAt(sources, 's', time);
        const preview = composite(renderScene({ ...sources, scenes: new Map([['s', { ...at0, items: shownItems(at0.items) }]]) }, 's', { order: 'list' }).root, { time });
        const published = paintCss(out, time / 120, 12000);
        const bad = differences(preview, published);
        assert.equal(bad.length, 0, `seed ${seed} at ${time.toFixed(1)} ms: ${bad.length} pixels differ, e.g. ${bad.slice(0, 3).map((q) => `${q}: ${preview.get(q)} / ${published.get(q)}`)}`);
        checked++;
      });
      assert.ok(checked > 40, `seed ${seed}: ${checked} stretches checked`);
      assert.ok(motionCopies(sources, 's').length > 0, `seed ${seed}: items change places`);
    }
  });

  it('R10: poseItems keeps a tracked item\'s level, whether it was placed on a tile or by pixel point', () => {
    const motion = { duration: 1000, tracks: [track('a', { t: 0 }, { t: 500, move: [0, 4] }), track('b', { t: 0 }, { t: 500, move: [0, 4] }), track('c', { t: 0 }, { t: 500, move: [0, 4] })] };
    const items = [
      { object: 'walker', id: 'a', at: { tile: [1, 2, 3] } },
      { object: 'walker', id: 'b', at: { px: [5, 6], level: 2 } },
      { object: 'walker', id: 'c', at: { px: [1, 1] } },
    ];
    const pointOf = (item) => (item.at.tile ? [(item.at.tile[0] - item.at.tile[1]) * 16, (item.at.tile[0] + item.at.tile[1]) * 8 - item.at.tile[2] * 16] : item.at.px);
    const [a, b, c] = poseItems(items, motion, 500, pointOf);
    assert.deepEqual(a.at, { px: [-16, -20], level: 3 }, 'tile 1,2,3 is at -16,-24, moved by 0,4');
    assert.deepEqual(b.at, { px: [5, 10], level: 2 });
    assert.deepEqual(c.at, { px: [1, 5] }, 'level 0 is left out, as in the file');
  });

  describe('through scene.render, with variants and layers', () => {
    const LAYERED = {
      objects: OBJECTS,
      scene: {
        ...scene(
          [
            { object: 'house', at: { tile: [2, 2, 1] } },
            { group: { 'data-class': 'a' }, items: [{ object: 'house', at: { tile: [6, 0, 0] } }] },
            { group: { 'data-class': 'b' }, items: [{ object: 'house', at: { tile: [7, 0, 0] } }] },
            { object: 'walker', id: 'down', layer: 'cast', at: { px: [0, -4] } },
          ],
          [track('down', { t: 0 }, { t: 5000, move: [0, 40] }, { t: 7500, move: [0, 40] })],
        ),
        layers: [{ id: 'cast', name: 'Cast' }],
      },
    };

    it('R10: the items come back in the order drawn at that time, with paths into the file, and show and hide leave the rest where it is', async () => {
      const { ctx, sources, doc } = await open(LAYERED);
      try {
        // The walker is drawn before the house (behind it) for part of the loop.
        const before = [...Array(40).keys()].map((k) => k * 250).find((time) => sceneAt(sources, 'nice', time).items[0].id === 'down');
        assert.ok(before !== undefined, 'the walker is behind the house at some time');
        const at = async (params = {}) => registry.call(ctx, 'scene.render', { name: 'nice', time: before, ...params });
        const plain = await at();
        assert.deepEqual(plain.items.map((i) => i.path), ['3', '0', '1.0'], 'drawn order: the walker, the house, the first variant');
        assert.deepEqual(plain.items.map((i) => i.id ?? i.object), ['down', 'house', 'house']);
        // At rest it is drawn after the house; the list never changed.
        assert.deepEqual((await registry.call(ctx, 'scene.render', { name: 'nice', time: 0 })).items.map((i) => i.path), ['0', '1.0', '3']);
        // The second variant is chosen by its path in the file, though the walker's move changed what sits at each place in the drawn list.
        assert.deepEqual((await at({ show: '2' })).items.map((i) => i.path), ['3', '0', '2.0']);
        // Hiding the walker's layer leaves the others in the order they have.
        assert.deepEqual((await at({ hide: ['cast'] })).items.map((i) => i.path), ['0', '1.0']);
        assert.notEqual(plain.png, (await at({ hide: ['cast'] })).png);
      } finally {
        await doc.cleanup();
      }
    });
  });
});
