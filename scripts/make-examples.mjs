#!/usr/bin/env node
// Writes the example documents in examples/ (docs/intents/2026-10-scene-layers/plan.md,
// "As built"): small .pixellab documents in format 2 with scene layers, to try the app
// on and to test with. They are made by the Rust program (target/release/pixellab,
// built with `cargo build --release` when missing): `document new`, `scene new`-style
// skeletons, then `scene place`, `scene layer new`, `scene track add`, `scene key set`,
// `scene ambient` and `scene action`. The output is the same on every run;
// scripts/test/examples.test.mjs checks the committed examples match it.
//
//   node scripts/make-examples.mjs [--out <dir>]     (default: examples/)
//
// What no command can say is written as canonical JSON first: the objects (blocks
// have no command that sets their size or faces, and a sprite's anchor is not always
// its bottom center), each scene's output and viewBox, and the manifest's budgets.
//
// harbor.pixellab   a small island: nested layers (Ground, Village > Houses > each
//                   house, Harbor > Dock and an animated Boat), the site's { group }
//                   wrappers around ground tiles and objects, a few items in no layer,
//                   a barrel that bobs and a boat that hops, and harbor-flat, the same
//                   scene without layers (it publishes the same SVG).
// town.pixellab     a bigger island (about 400 items) with four districts of streets
//                   of houses: about 60 layers, four levels deep, no wrappers.

import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = join(dirname(fileURLToPath(import.meta.url)), '..');
const BIN = join(REPO, 'target', 'release', 'pixellab');

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
 * @typedef {{ id: string, name: string, parent?: string }} Layer
 */

/**
 * How far back an item sits: a first guess at the back-to-front order (row +
 * col of its last tile, then level). Placements go in this order; the program
 * puts every scene in its depth order as each item lands.
 */
function depth(/** @type {Place} */ p) {
  const obj = /** @type {any} */ (OBJECTS)[p.object];
  const [w, d] = obj.kind === 'block' ? obj.size : [1, 1];
  const [c, r, l] = p.tile;
  return [c + w - 1 + r + d - 1, l, obj.kind === 'block' ? 0 : 1];
}

/** @param {Place[]} places */
const inFirstOrder = (places) =>
  places
    .map((p, i) => ({ p, i, k: depth(p) }))
    .sort((a, b) => a.k[0] - b.k[0] || a.k[1] - b.k[1] || a.k[2] - b.k[2] || a.i - b.i)
    .map(({ p }) => p);

/** The attributes of the site's entrance wrapper: data-x/data-y are the tile's corner in pixels, a level (16 px) up. */
function wrapper(/** @type {Place} */ p) {
  const [c, r, l] = p.tile;
  const [x, y] = [String((c - r) * 16), String((c + r) * 8 - l * 16)];
  return p.wrap === 'ground'
    ? { class: 'enter', 'data-enter': 'ground', 'data-col': String(c), 'data-row': String(r), 'data-x': x, 'data-y': y }
    : { class: 'enter', 'data-enter': 'object', 'data-x': x, 'data-y': y };
}

/**
 * harbor: an 8×8 island. G grass, S sand, P path, D dock (over the shore), W water.
 */
const HARBOR_MAP = ['GGGGGGSW', 'GGGGGGSW', 'GGPPPGSW', 'GGPGPGSW', 'GGPPPPDD', 'GGGGGGSW', 'GGGGGSSW', 'SSSSSSWW'];

function harbor() {
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
  // The first barrel bobs on the dock (an ambient loop); the boat sails and hops.
  places.push({ object: 'barrel', tile: [6, 3, 0], id: 'barrel', layer: 'dock' });
  places.push({ object: 'barrel', tile: [7, 4, 0], layer: 'dock' });
  places.push({ object: 'boat', tile: [7, 0, -1], id: 'boat', layer: 'boat' });
  // In no layer: they show at the top level of the tree.
  places.push({ object: 'rock', tile: [6, 6, 0], wrap: 'object' });
  places.push({ object: 'rock', tile: [0, 7, 0] });
  /** @type {Layer[]} */
  const layers = [
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
  ];
  /** Run on a scene after its items are placed (and the layers made): its motion, ambient loop and action. */
  const finish = (/** @type {(...a: string[]) => void} */ run, /** @type {string} */ scene) => {
    // The boat sails three rows along the shore and back: 2:1 steps, on the grid.
    run('scene', 'track', 'add', scene, 'boat', '--duration', '8000');
    run('scene', 'key', 'set', scene, 'boat', '--time', '3500', '--move', '-48,24');
    run('scene', 'key', 'set', scene, 'boat', '--time', '4500', '--move', '-48,24');
    // The barrel bobs a pixel, and the boat hops when the page says "hop".
    run('scene', 'ambient', scene, 'barrel', '--duration', '3000', '--keys', JSON.stringify([{ t: 0, move: [0, 0] }, { t: 1500, move: [0, -1] }]));
    run('scene', 'action', scene, 'boat', 'hop', '--duration', '480', '--keys', JSON.stringify([{ t: 0, move: [0, 0] }, { t: 120, move: [0, -4] }, { t: 240, move: [0, -6] }, { t: 360, move: [0, -4] }]));
  };
  return { scenes: [{ name: 'harbor', output: 'harbor.svg', viewBox: [-136, -56, 272, 200], places, layers, finish }, { name: 'harbor-flat', output: 'harbor-flat.svg', viewBox: [-136, -56, 272, 200], places, layers: [], finish }] };
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
  const rand = random(20261020);
  /** @type {Place[]} */
  const places = [];
  const isPath = (/** @type {number} */ c, /** @type {number} */ r) => c === 7 || c === 8 || r === 7 || r === 8;
  for (let r = 0; r < 16; r++) {
    for (let c = 0; c < 16; c++) places.push({ object: isPath(c, r) ? 'path' : 'grass', tile: [c, r, 0], layer: isPath(c, r) ? 'town-paths' : 'town-grass' });
  }
  /** @type {Layer[]} */
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
  return { scenes: [{ name: 'town', output: 'town.svg', viewBox: [-264, -56, 528, 336], places, layers, finish: () => {} }] };
}

// ---------------------------------------------------------------- documents

/** The example documents: their manifests, the objects they use and their scenes. */
export function examples() {
  return {
    harbor: {
      manifest: { format: 2, name: 'harbor' },
      objects: ['grass', 'sand', 'path', 'water', 'dock', 'walls', 'roof-red', 'roof-wood', 'hall-walls', 'hall-roof', 'tree', 'pine', 'rock', 'barrel', 'lamp', 'flowers', 'boat'],
      ...harbor(),
    },
    town: {
      manifest: { format: 2, name: 'town', publish: { budgets: { 'town.svg': { raw: 409600, gzip: 81920 } } } },
      objects: ['grass', 'path', 'walls', 'roof-red', 'roof-wood', 'tree', 'pine', 'lamp'],
      ...town(),
    },
  };
}

/** An object as canonical JSON text: the form the program writes (rows one per line, size and anchor inline). */
function objectText(/** @type {any} */ o) {
  const lines = (/** @type {Record<string, string>} */ m, /** @type {string} */ pad) =>
    Object.entries(m)
      .map(([k, v]) => `${pad}${JSON.stringify(k)}: ${JSON.stringify(v)}`)
      .join(',\n');
  if (o.kind === 'block') {
    return `{\n  "kind": "block",\n  "size": [${o.size.join(', ')}],\n  "faces": {\n${lines(o.faces, '    ')}\n  }\n}\n`;
  }
  const rows = o.layers[0].map.map((/** @type {string} */ r) => `        ${JSON.stringify(r)}`).join(',\n');
  return `{\n  "kind": "sprite",\n  "anchor": [${o.anchor.join(', ')}],\n  "keys": {\n${lines(o.keys, '    ')}\n  },\n  "layers": [\n    {\n      "map": [\n${rows}\n      ]\n    }\n  ]\n}\n`;
}

/** A scene's canonical start: its settings and no items. */
const sceneText = (/** @type {any} */ s) =>
  `{\n  "output": ${JSON.stringify(s.output)},\n  "viewBox": [${s.viewBox.join(', ')}],\n  "origin": [0, 0],\n  "items": []\n}\n`;

/**
 * Writes every example document under `out` with the program, and returns the
 * files written, relative to `out`.
 * @param {string} out
 */
export async function writeExamples(out) {
  if (!existsSync(BIN)) {
    console.error('building target/release/pixellab (cargo build --release)');
    execFileSync('cargo', ['build', '--release'], { cwd: REPO, stdio: 'inherit', env: { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH}` } });
  }
  /** @type {string[]} */
  const files = [];
  for (const [name, ex] of Object.entries(examples())) {
    const root = join(out, `${name}.pixellab`);
    await rm(root, { recursive: true, force: true });
    await mkdir(out, { recursive: true });
    const run = (/** @type {string[]} */ ...args) => {
      try {
        return execFileSync(BIN, ['--document', root, ...args], { encoding: 'utf8', env: { ...process.env, HOME: out }, stdio: ['ignore', 'pipe', 'pipe'] });
      } catch (e) {
        throw new Error(`pixellab ${args.join(' ')} failed:\n${/** @type {any} */ (e).stderr}${/** @type {any} */ (e).stdout}`);
      }
    };
    execFileSync(BIN, ['document', 'new', root, '--name', name], { stdio: 'ignore', env: { ...process.env, HOME: out } });
    await writeFile(join(root, 'manifest.json'), `${JSON.stringify(ex.manifest, null, 2)}\n`);
    for (const o of ex.objects) await writeFile(join(root, 'objects', `${o}.json`), objectText(/** @type {any} */ (OBJECTS)[o]));
    for (const scene of ex.scenes) {
      await writeFile(join(root, 'scenes', `${scene.name}.json`), sceneText(scene));
      for (const p of inFirstOrder(scene.places)) {
        run(
          'scene', 'place', scene.name, p.object, '--at', p.tile.join(','),
          ...(p.id ? ['--id', p.id] : []),
          ...(p.wrap ? ['--new-group', JSON.stringify(wrapper(p))] : []),
        );
      }
      makeLayers(run, scene);
      scene.finish((...a) => run(...a), scene.name);
    }
    run('check');
  }
  const { readdir } = await import('node:fs/promises');
  for (const e of await readdir(out, { recursive: true, withFileTypes: true })) {
    if (e.isFile()) files.push(join(e.parentPath ?? /** @type {any} */ (e).path, e.name).slice(out.length + 1));
  }
  return files.sort();
}

/**
 * Makes a scene's layers.
 * @param {(...a: string[]) => string} run
 * @param {{ name: string, places: Place[], layers: Layer[] }} scene
 */
function makeLayers(run, scene) {
  if (!scene.layers.length) return;
  const listed = JSON.parse(run('scene', 'items', scene.name, '--json'));
  const pathOf = new Map(listed.items.map((/** @type {any} */ i) => [`${i.object}@${i.at.tile.join(',')}`, i.path]));
  const made = (/** @type {Layer} */ l) => ['scene', 'layer', 'new', scene.name, '--layer-name', l.name, '--id', l.id];
  // A new layer goes to the front of the list, so making them last to first leaves the
  // list as it is written (and every layer after the ones it holds).
  for (const l of [...scene.layers].reverse()) {
    const items = scene.places.filter((p) => p.layer === l.id).map((p) => pathOf.get(`${p.object}@${p.tile.join(',')}`));
    const kids = scene.layers.filter((k) => k.parent === l.id);
    run(...made(l), ...(items.length ? ['--items', items.join(',')] : ['--layers', kids.map((k) => k.id).join(',')]));
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const at = process.argv.indexOf('--out');
  const out = at > 0 ? process.argv[at + 1] : join(REPO, 'examples');
  const files = await writeExamples(out);
  console.log(`wrote ${files.length} files in ${out}`);
}
