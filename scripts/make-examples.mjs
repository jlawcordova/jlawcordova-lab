#!/usr/bin/env node
// Writes the example documents in examples/ (docs/intents/2026-10-scene-layers/plan.md,
// "As built"): small .pixellab documents with scene layers, to try the app on and to
// test with. Everything is made here, from the starter palette, so the output is the
// same on every run; `npm test` checks the committed examples match it.
//
//   node scripts/make-examples.mjs [--out <dir>]     (default: examples/)
//
// harbor.pixellab   a small island: nested layers (Ground, Village › Houses › each
//                   house, Harbor › Dock and an animated Boat), the site's { group }
//                   wrappers around ground tiles and objects, a few items in no layer,
//                   and harbor-flat, the same scene without layers (it publishes the
//                   same SVG).
// town.pixellab     a bigger island (about 400 items) with four districts of streets
//                   of houses: about 60 layers, four levels deep, no wrappers.

import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import * as engine from '../cli/lib/engine/engine.mjs';
import * as iso from '../cli/lib/engine/iso.mjs';
import { serialize } from '../cli/lib/engine/serialize.mjs';

const REPO = join(dirname(fileURLToPath(import.meta.url)), '..');

// ------------------------------------------------------------------ objects

/** @param {string[]} map @param {Record<string, string>} keys @param {[number, number]} anchor */
const sprite = (map, keys, anchor) => ({ kind: 'sprite', anchor, keys, layers: [{ map }] });
/** @param {number[]} size @param {Record<string, string>} faces */
const block = (size, faces) => ({ kind: 'block', size, faces });

const LEAVES = { a: 'grass-1', b: 'grass-2', c: 'grass-3', d: 'grass-4', t: 'wood-3', u: 'wood-4' };

/** Every object the examples use, by name. */
export const OBJECTS = {
  grass: block([1, 1, 1], { top: 'grass-2', left: 'soil-2', right: 'soil-3', edge: 'soil-4' }),
  sand: block([1, 1, 1], { top: 'path-1', left: 'path-2', right: 'path-3', edge: 'path-4' }),
  path: block([1, 1, 1], { top: 'path-2', left: 'path-3', right: 'path-4', edge: 'soil-4' }),
  water: block([1, 1, 0], { top: 'water-2', edge: 'water-3' }),
  dock: block([1, 1, 0], { top: 'wood-2', edge: 'wood-4' }),
  walls: block([1, 1, 1], { top: 'path-1', left: 'cream', right: 'path-1', edge: 'wood-4' }),
  'roof-red': block([1, 1, 1], { top: 'roof-1', left: 'roof-2', right: 'roof-3', edge: 'roof-4' }),
  'roof-wood': block([1, 1, 1], { top: 'wood-1', left: 'wood-2', right: 'wood-3', edge: 'wood-4' }),
  'hall-walls': block([2, 1, 1], { top: 'path-1', left: 'cream', right: 'path-1', edge: 'wood-4' }),
  'hall-roof': block([2, 1, 1], { top: 'gold-1', left: 'gold-2', right: 'gold-3', edge: 'gold-4' }),
  tree: sprite(
    [
      '....bbbbb....',
      '..bbaaabbcc..',
      '.baaabbbbccc.',
      '.babbbbbcccc.',
      'bbbbbbbbccccd',
      'bbbbbbbcccccd',
      'bbbbbbccccccd',
      '.bbbbcccccdd.',
      '.ccbcccccccd.',
      '..ccccccccd..',
      '...ddcccddd..',
      '.....dtud....',
      '......tu.....',
      '......tu.....',
      '......tu.....',
      '.....ttuu....',
    ],
    LEAVES,
    [6, 15],
  ),
  pine: sprite(
    [
      '.....b.....',
      '....bbc....',
      '....abc....',
      '...abbcc...',
      '....bbc....',
      '...abbcc...',
      '..abbbbcc..',
      '...abbcc...',
      '..abbbbcc..',
      '.abbbbbccc.',
      '..abbbccc..',
      '.abbbbbbcc.',
      'abbbbbbbccc',
      '.ccccccccc.',
      '.....tu....',
      '.....tu....',
      '.....tu....',
      '....ttuu...',
    ],
    { a: 'grass-2', b: 'grass-3', c: 'grass-4', t: 'wood-3', u: 'wood-4' },
    [5, 17],
  ),
  rock: sprite(
    ['...aaa...', '..aaabb..', '.aaabbbc.', 'aabbbbbcc', '.bbbbccc.', '..ccccc..'],
    { a: 'path-2', b: 'path-3', c: 'path-4' },
    [4, 5],
  ),
  barrel: sprite(
    ['.ccccc.', 'cbbbbbc', 'daabbbd', 'daabbcd', 'eeeeeee', 'daabbcd', 'daabbcd', 'daabbcd', '.ddddd.'],
    { a: 'wood-1', b: 'wood-2', c: 'wood-3', d: 'wood-4', e: 'ink' },
    [3, 8],
  ),
  lamp: sprite(
    ['.iii.', 'igggi', 'ighgi', 'iiiii', '..i..', '..i..', '..i..', '..i..', '..i..', '..i..', '..i..', '..i..', '..i..', '..i..', '.iii.', 'iiiii'],
    { g: 'gold-1', h: 'gold-2', i: 'ink' },
    [2, 15],
  ),
  flowers: sprite(
    ['.r...y.c.', 'rsr.yyycc', '.r.g.y.c.', '.g..g..g.', '..g.g.g..'],
    { r: 'roof-1', s: 'roof-2', y: 'gold-1', c: 'cream', g: 'grass-3' },
    [4, 4],
  ),
  boat: sprite(
    [
      '..........d.........',
      '..........ds........',
      '..........dss.......',
      '..........dsss......',
      '..........dssss.....',
      '..........dsssss....',
      '..........d.........',
      '....aaaaaaaaaaaa....',
      '..aabbbbbbbbbbbbaa..',
      'aabbbbbbbbbbbbbbbbaa',
      '.ccbbbbbbbbbbbbbbcc.',
      '...cccccccccccccc...',
      '.....dddddddddd.....',
    ],
    { a: 'wood-1', b: 'wood-2', c: 'wood-3', d: 'wood-4', s: 'cream' },
    [10, 11],
  ),
};

// ------------------------------------------------------------------- scenes

/**
 * A placement before it's put in depth order: an object on a tile, with its
 * layer, an id, and the site's wrapper attributes when it has one.
 * @typedef {{ object: string, tile: number[], layer?: string, id?: string, wrap?: 'ground' | 'object' }} Place
 */

/**
 * How far back an item sits: a first guess at the back-to-front order (row +
 * col of its last tile, then level). It only gives the list a good start, which
 * keeps the compiler's runs of one color together; writeExamples then puts
 * every scene in the engine's depth order, which is what the files hold.
 */
function depth(/** @type {Place} */ p) {
  const obj = /** @type {any} */ (OBJECTS)[p.object];
  const [w, d] = obj.kind === 'block' ? obj.size : [1, 1];
  const [c, r, l] = p.tile;
  return [c + w - 1 + r + d - 1, l, obj.kind === 'block' ? 0 : 1];
}

/**
 * Turns placements into a scene's items, in a first back-to-front order, each in the
 * site's entrance wrapper when it has one (its data-x/data-y the item's foot).
 * @param {Place[]} places
 * @param {number[]} origin
 */
function itemsOf(places, origin) {
  const sorted = places
    .map((p, i) => ({ p, i, k: depth(p) }))
    .sort((a, b) => a.k[0] - b.k[0] || a.k[1] - b.k[1] || a.k[2] - b.k[2] || a.i - b.i)
    .map(({ p }) => p);
  return sorted.map((p) => {
    const item = { object: p.object, ...(p.id ? { id: p.id } : {}), ...(p.layer ? { layer: p.layer } : {}), at: { tile: [...p.tile] } };
    if (!p.wrap) return item;
    const [x, y] = iso.tileToPx(p.tile, origin);
    const group =
      p.wrap === 'ground'
        ? { class: 'enter', 'data-enter': 'ground', 'data-col': String(p.tile[0]), 'data-row': String(p.tile[1]), 'data-x': String(x), 'data-y': String(y) }
        : { class: 'enter', 'data-enter': 'object', 'data-x': String(x), 'data-y': String(y) };
    return { group, items: [item] };
  });
}

/** The same scene with every trace of layers taken out. */
function withoutLayers(/** @type {any} */ scene) {
  const strip = (/** @type {any[]} */ items) =>
    items.map((it) => ('group' in it ? { ...it, items: strip(it.items) } : (({ layer: _l, ...rest }) => rest)(it)));
  const { layers: _layers, ...rest } = scene;
  return { ...rest, items: strip(scene.items) };
}

/**
 * harbor: an 8×8 island. G grass, S sand, P path, D dock (over the shore), W water.
 */
const HARBOR_MAP = ['GGGGGGSW', 'GGGGGGSW', 'GGPPPGSW', 'GGPGPGSW', 'GGPPPPDD', 'GGGGGGSW', 'GGGGGSSW', 'SSSSSSWW'];

function harbor() {
  const origin = [0, 0];
  /** @type {Place[]} */
  const places = [];
  const ground = { G: ['grass', 'grass'], S: ['sand', 'beach'], P: ['path', 'paths'], D: ['dock', 'dock'], W: ['water', 'water'] };
  HARBOR_MAP.forEach((line, r) =>
    [...line].forEach((ch, c) => {
      const [object, layer] = /** @type {any} */ (ground)[ch];
      // Water lies a level down; the dock floats at the ground's height.
      const level = ch === 'W' ? -1 : 0;
      places.push({ object, tile: [c, r, level], layer, wrap: ch === 'W' || ch === 'D' ? undefined : 'ground' });
    }),
  );
  const house = (/** @type {number[]} */ [c, r], /** @type {string} */ roof, /** @type {string} */ layer) => {
    places.push({ object: 'walls', tile: [c, r, 1], layer, wrap: 'object' });
    places.push({ object: roof, tile: [c, r, 2], layer, wrap: 'object' });
  };
  house([1, 1], 'roof-red', 'red-house');
  house([4, 1], 'roof-wood', 'wood-house');
  places.push({ object: 'hall-walls', tile: [2, 5, 1], layer: 'town-hall', wrap: 'object' });
  places.push({ object: 'hall-roof', tile: [2, 5, 2], layer: 'town-hall', wrap: 'object' });
  for (const t of [[3, 3], [1, 3], [4, 6]]) places.push({ object: 'flowers', tile: [...t, 0], layer: 'garden', wrap: 'object' });
  places.push({ object: 'tree', tile: [0, 2, 0], layer: 'garden', wrap: 'object' });
  for (const t of [[0, 0], [5, 0], [0, 4], [1, 6], [5, 2]]) places.push({ object: 'tree', tile: [...t, 0], layer: 'trees', wrap: 'object' });
  for (const t of [[3, 0], [0, 6], [5, 6]]) places.push({ object: 'pine', tile: [...t, 0], layer: 'trees', wrap: 'object' });
  for (const t of [[5, 3], [5, 5]]) places.push({ object: 'lamp', tile: [...t, 0], layer: 'lamps', wrap: 'object' });
  for (const t of [[6, 3], [7, 4]]) places.push({ object: 'barrel', tile: [...t, 0], layer: 'dock' });
  places.push({ object: 'boat', tile: [7, 0, -1], id: 'boat', layer: 'boat' });
  // In no layer: they show at the top level of the tree.
  places.push({ object: 'rock', tile: [6, 6, 0], wrap: 'object' });
  places.push({ object: 'rock', tile: [0, 7, 0] });
  const scene = {
    output: 'harbor.svg',
    viewBox: [-136, -56, 272, 200],
    origin,
    layers: [
      { id: 'ground', name: 'Ground' },
      { id: 'grass', name: 'Grass', parent: 'ground' },
      { id: 'paths', name: 'Paths', parent: 'ground' },
      { id: 'beach', name: 'Beach', parent: 'ground' },
      { id: 'water', name: 'Water', parent: 'ground' },
      { id: 'village', name: 'Village' },
      { id: 'houses', name: 'Houses', parent: 'village' },
      { id: 'red-house', name: 'Red house', parent: 'houses' },
      { id: 'wood-house', name: 'Wood house', parent: 'houses' },
      { id: 'town-hall', name: 'Town hall', parent: 'houses' },
      { id: 'garden', name: 'Garden', parent: 'village' },
      { id: 'harbor', name: 'Harbor' },
      { id: 'dock', name: 'Dock', parent: 'harbor' },
      { id: 'boat', name: 'Boat', parent: 'harbor' },
      { id: 'lamps', name: 'Lamps', parent: 'harbor' },
      { id: 'trees', name: 'Trees' },
    ],
    items: itemsOf(places, origin),
    // The boat sails three rows along the shore and back: 2:1 steps, on the grid.
    motion: {
      duration: 8000,
      tracks: [{ item: 'boat', keys: [{ t: 0 }, { t: 3500, move: [-48, 24] }, { t: 4500, move: [-48, 24] }] }],
    },
  };
  return { harbor: scene, 'harbor-flat': { ...withoutLayers(scene), output: 'harbor-flat.svg' } };
}

/** A small, fixed pseudo-random sequence, so the town is the same on every run. */
function random(/** @type {number} */ seed) {
  let s = seed >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 2 ** 32;
  };
}

/**
 * town: a 16×16 island with paths along rows and columns 7 and 8, and four
 * districts of three streets of three houses, with trees between.
 */
function town() {
  const origin = [0, 0];
  const rand = random(20261020);
  /** @type {Place[]} */
  const places = [];
  const isPath = (/** @type {number} */ c, /** @type {number} */ r) => c === 7 || c === 8 || r === 7 || r === 8;
  for (let r = 0; r < 16; r++) {
    for (let c = 0; c < 16; c++) places.push({ object: isPath(c, r) ? 'path' : 'grass', tile: [c, r, 0], layer: isPath(c, r) ? 'town-paths' : 'town-grass' });
  }
  /** @type {{ id: string, name: string, parent?: string }[]} */
  const layers = [
    { id: 'land', name: 'Land' },
    { id: 'town-grass', name: 'Grass', parent: 'land' },
    { id: 'town-paths', name: 'Paths', parent: 'land' },
  ];
  const districts = [
    ['nw', 'North-west', 0, 0],
    ['ne', 'North-east', 9, 0],
    ['sw', 'South-west', 0, 9],
    ['se', 'South-east', 9, 9],
  ];
  /** @type {Set<string>} */
  const built = new Set();
  for (const [id, name, c0, r0] of /** @type {[string, string, number, number][]} */ (districts)) {
    layers.push({ id, name });
    for (let s = 0; s < 3; s++) {
      const street = `${id}-street-${s + 1}`;
      layers.push({ id: street, name: `Street ${s + 1}`, parent: id });
      for (let h = 0; h < 3; h++) {
        const houseId = `${street}-house-${h + 1}`;
        layers.push({ id: houseId, name: `House ${h + 1}`, parent: street });
        const c = c0 + h * 2 + 1;
        const r = r0 + s * 2 + 1;
        built.add(`${c},${r}`);
        places.push({ object: 'walls', tile: [c, r, 1], layer: houseId });
        places.push({ object: rand() < 0.5 ? 'roof-red' : 'roof-wood', tile: [c, r, 2], layer: houseId });
      }
    }
    layers.push({ id: `${id}-trees`, name: 'Trees', parent: id });
    for (let r = r0; r < r0 + 7; r++) {
      for (let c = c0; c < c0 + 7; c++) {
        if (built.has(`${c},${r}`) || rand() > 0.32) continue;
        places.push({ object: rand() < 0.6 ? 'tree' : 'pine', tile: [c, r, 0], layer: `${id}-trees` });
      }
    }
  }
  // Lamps along the paths, in no layer.
  for (const k of [1, 4, 11, 14]) {
    places.push({ object: 'lamp', tile: [k, 7, 0] });
    places.push({ object: 'lamp', tile: [8, k, 0] });
  }
  return {
    town: {
      output: 'town.svg',
      viewBox: [-264, -56, 528, 336],
      origin,
      layers,
      items: itemsOf(places, origin),
    },
  };
}

// ---------------------------------------------------------------- documents

/** The starter palette as a new document's palette.mjs (as document.new writes it). */
async function paletteText() {
  const text = await readFile(join(REPO, 'cli/lib/engine/starter-palette.mjs'), 'utf8');
  const body = text.slice(text.indexOf('export default'));
  return `// The document's palette: objects name their colors from it. Colors are\n// added by hand. world: at most 32; outfit: at most 16, only for objects\n// that extend character. Names are unique across the tiers.\n${body}`;
}

/** The example documents: their manifests, the objects they use and their scenes. */
export function examples() {
  return {
    harbor: {
      manifest: { format: 1, name: 'harbor' },
      objects: ['grass', 'sand', 'path', 'water', 'dock', 'walls', 'roof-red', 'roof-wood', 'hall-walls', 'hall-roof', 'tree', 'pine', 'rock', 'barrel', 'lamp', 'flowers', 'boat'],
      scenes: harbor(),
    },
    town: {
      manifest: { format: 1, name: 'town', publish: { budgets: { 'town.svg': { raw: 409600, gzip: 81920 } } } },
      objects: ['grass', 'path', 'walls', 'roof-red', 'roof-wood', 'tree', 'pine', 'lamp'],
      scenes: town(),
    },
  };
}

/**
 * Writes every example document under `out`, after checking each with the
 * engine. Returns the files written, relative to `out`.
 * @param {string} out
 */
export async function writeExamples(out) {
  const palette = await paletteText();
  const paletteData = (await import('../cli/lib/engine/starter-palette.mjs')).default;
  /** @type {string[]} */
  const files = [];
  for (const [name, ex] of Object.entries(examples())) {
    const objects = Object.fromEntries(ex.objects.map((o) => [o, /** @type {any} */ (OBJECTS)[o]]));
    const problems = engine.validate(engine.loadSources({ palette: paletteData, objects, scenes: ex.scenes }));
    if (problems.length) throw new Error(`examples/${name}.pixellab has problems:\n${problems.join('\n')}`);
    // The files hold each scene in depth order, as every edit in Pixel Lab leaves it
    // (docs/intents/2026-10-depth-order/spec.md, R4).
    const sources = engine.loadSources({ palette: paletteData, objects, scenes: ex.scenes });
    ex.scenes = Object.fromEntries(Object.entries(ex.scenes).map(([s, data]) => [s, engine.sceneInOrder(sources, data)]));
    const root = join(out, `${name}.pixellab`);
    await rm(root, { recursive: true, force: true });
    await mkdir(join(root, 'objects'), { recursive: true });
    await mkdir(join(root, 'scenes'), { recursive: true });
    /** @type {[string, string][]} */
    const writes = [
      ['manifest.json', `${JSON.stringify(ex.manifest, null, 2)}\n`],
      ['palette.mjs', palette],
      ...Object.entries(objects).map(([o, data]) => /** @type {[string, string]} */ ([`objects/${o}.mjs`, serialize(data)])),
      ...Object.entries(ex.scenes).map(([s, data]) => /** @type {[string, string]} */ ([`scenes/${s}.mjs`, serialize(data)])),
    ];
    for (const [file, text] of writes) {
      await writeFile(join(root, file), text);
      files.push(`${name}.pixellab/${file}`);
    }
  }
  return files;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const at = process.argv.indexOf('--out');
  const out = at > 0 ? process.argv[at + 1] : join(REPO, 'examples');
  const files = await writeExamples(out);
  console.log(`wrote ${files.length} files in ${out}`);
}
