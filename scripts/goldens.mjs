#!/usr/bin/env node
// The Node side of the Rust port's checks (docs/intents/2026-10-rust-engine/plan.md,
// steps 3, 5 and 6). Removed at cutover, with the Node CLI.
//
//   node scripts/goldens.mjs --bootstrap <from.pixellab> <to.pixellab>
//       Imports each .mjs source of a format-1 document with Node and writes
//       a format-2 copy: manifest.json with "format": 2, palette.json,
//       objects/*.json and scenes/*.json in the canonical text of
//       crates/engine/src/format.rs. Comments aren't carried (document
//       migrate does that, R7); fixtures/sample.pixellab is made this way.
//
//   node scripts/goldens.mjs --out <dir> [--site <checkout>]
//       The corpus and what the Node CLI makes of it, for the Rust units to
//       compare against (PIXELLAB_GOLDENS=<dir> cargo test --test corpus):
//         <dir>/corpus.json                 the documents and their invocations
//         <dir>/format1/<doc>.pixellab      each document as it is
//         <dir>/format2/<doc>.pixellab      its bootstrap copy
//         <dir>/goldens/<doc>/svg/<scene>.svg       compileScene's output
//         <dir>/goldens/<doc>/png/<kind>-<name>.png renderPreview at 1×
//         <dir>/goldens/<doc>/commands.json  every read command's result
//       The documents are the test fixture, both examples and, when a site
//       checkout is there (--site, else ~/Projects/jlawcordova.github.io),
//       a copy of the owner's document made with `document copy`, which
//       only reads the checkout.
//
//   node scripts/goldens.mjs --jsnum <file.json>
//       The number cases for crates/engine/src/jsnum.rs, from Node:
//       Math.round, toFixed and String(x) of every number the corpus's
//       motion and sizes print, and the edge cases (−0, halves, 1e21, 1e-7).

import { execFileSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const CLI = join(REPO, 'cli/bin/pixellab.mjs');

// ------------------------------------------------------------ canonical text
// The same layout as crates/engine/src/format.rs; a Rust test checks that
// serialize(parse(text)) == text for every file this writes.

const ORDER = {
  palette: ['notes', 'world', 'outfit', 'colorNotes'],
  sprite: ['notes', 'kind', 'extends', 'anchor', 'footprint', 'keys', 'rows', 'layers'],
  block: ['notes', 'kind', 'size', 'faces', 'surface', 'sides'],
  layer: ['keys', 'class', 'loop', 'prefix', 'frames', 'map', 'ambient', 'actions'],
  sides: ['keys', 'left', 'right'],
  scene: ['notes', 'output', 'image', 'viewBox', 'origin', 'layers', 'items', 'motion'],
  sceneLayer: ['id', 'name', 'parent'],
  item: ['object', 'id', 'class', 'layer', 'depth', 'at', 'ambient', 'actions'],
  group: ['group', 'depth', 'items'],
  at: ['tile', 'px', 'level'],
  motion: ['duration', 'tracks'],
  track: ['item', 'keys'],
  key: ['t', 'move', 'show', 'pose'],
  loop: ['duration', 'offset', 'keys'],
  image: ['file', 'scale'],
};

const isRecord = (/** @type {unknown} */ v) => typeof v === 'object' && v !== null && !Array.isArray(v);
const pad = (/** @type {number} */ n) => '  '.repeat(n);
const scalar = (/** @type {unknown} */ v) => JSON.stringify(v);

/** A record's keys: the table's, then the rest in JavaScript's own order. */
const tableKeys = (/** @type {Record<string, unknown>} */ map, /** @type {string[]} */ table) => [
  ...table.filter((k) => k in map),
  ...Object.keys(map).filter((k) => !table.includes(k)),
];

/** One line. */
function inline(/** @type {any} */ v, /** @type {string[] | null} */ table = null) {
  if (Array.isArray(v)) return `[${v.map((e) => inline(e)).join(', ')}]`;
  if (!isRecord(v)) return scalar(v);
  const names = table ? tableKeys(v, table) : Object.keys(v);
  if (!names.length) return '{}';
  const parts = [];
  for (const k of names) {
    if (table === ORDER.at && k === 'level' && v[k] === 0) continue;
    parts.push(`${JSON.stringify(k)}: ${inline(v[k], table === ORDER.item && k === 'at' ? ORDER.at : null)}`);
  }
  return parts.length ? `{ ${parts.join(', ')} }` : '{}';
}

/**
 * @typedef {{ t: 'pretty' } | { t: 'inline' } | { t: 'inlineRecord', table: string[] } | { t: 'record', table: string[], byKey: (k: string) => Shape } | { t: 'list', each: (v: any) => Shape }} Shape
 */
const PRETTY = /** @type {Shape} */ ({ t: 'pretty' });
const pretty = () => PRETTY;

/** @param {any} v @param {Shape} shape @param {number} depth @returns {string} */
function write(v, shape, depth) {
  if (shape.t === 'inline') return inline(v);
  if (shape.t === 'inlineRecord') return inline(v, shape.table);
  if (shape.t === 'record' && isRecord(v)) return record(v, shape.table, shape.byKey, depth);
  if (shape.t === 'list' && Array.isArray(v)) {
    if (!v.length) return '[]';
    return `[\n${v.map((e) => pad(depth + 1) + write(e, shape.each(e), depth + 1)).join(',\n')}\n${pad(depth)}]`;
  }
  if (Array.isArray(v)) {
    if (!v.length) return '[]';
    if (v.every((e) => typeof e === 'number')) return inline(v);
    return `[\n${v.map((e) => pad(depth + 1) + write(e, PRETTY, depth + 1)).join(',\n')}\n${pad(depth)}]`;
  }
  if (isRecord(v)) return record(v, [], pretty, depth);
  return scalar(v);
}

/** @param {Record<string, any>} map @param {string[]} table @param {(k: string) => Shape} byKey @param {number} depth */
function record(map, table, byKey, depth) {
  const names = tableKeys(map, table);
  if (!names.length) return '{}';
  return `{\n${names.map((k) => `${pad(depth + 1)}${JSON.stringify(k)}: ${write(map[k], byKey(k), depth + 1)}`).join(',\n')}\n${pad(depth)}}`;
}

/** @param {any} v @returns {Shape} */
const itemShape = (v) => (isRecord(v) && 'group' in v ? { t: 'record', table: ORDER.group, byKey: groupKey } : { t: 'inlineRecord', table: ORDER.item });
/** @param {string} k @returns {Shape} */
const groupKey = (k) => (k === 'group' ? { t: 'inline' } : k === 'items' ? { t: 'list', each: itemShape } : PRETTY);
/** @param {string} k @returns {Shape} */
const sceneKey = (k) =>
  k === 'layers'
    ? { t: 'list', each: () => ({ t: 'inlineRecord', table: ORDER.sceneLayer }) }
    : k === 'items'
      ? { t: 'list', each: itemShape }
      : k === 'motion'
        ? { t: 'record', table: ORDER.motion, byKey: motionKey }
        : k === 'image'
          ? { t: 'inlineRecord', table: ORDER.image }
          : PRETTY;
/** @param {string} k @returns {Shape} */
const motionKey = (k) => (k === 'tracks' ? { t: 'list', each: () => ({ t: 'record', table: ORDER.track, byKey: keysKey }) } : PRETTY);
/** @param {string} k @returns {Shape} */
const keysKey = (k) => (k === 'keys' ? { t: 'list', each: () => ({ t: 'inlineRecord', table: ORDER.key }) } : PRETTY);
/** @param {string} k @returns {Shape} */
const layerKey = (k) => (k === 'ambient' ? { t: 'record', table: ORDER.loop, byKey: keysKey } : PRETTY);
/** @param {string} k @returns {Shape} */
const objectKey = (k) =>
  k === 'layers'
    ? { t: 'list', each: () => ({ t: 'record', table: ORDER.layer, byKey: layerKey }) }
    : k === 'surface'
      ? { t: 'record', table: ORDER.layer, byKey: layerKey }
      : k === 'sides'
        ? { t: 'record', table: ORDER.sides, byKey: pretty }
        : PRETTY;

/** The canonical text of a source, by its file. @param {string} file @param {any} doc */
export function canonical(file, doc) {
  let shape;
  if (file === 'palette.json') shape = { t: 'record', table: ORDER.palette, byKey: pretty };
  else if (file.startsWith('objects/')) shape = { t: 'record', table: doc?.kind === 'block' ? ORDER.block : ORDER.sprite, byKey: objectKey };
  else shape = { t: 'record', table: ORDER.scene, byKey: sceneKey };
  return `${write(doc, /** @type {Shape} */ (shape), 0)}\n`;
}

// ------------------------------------------------------------------ bootstrap

/** The default export of a source file, imported fresh. @param {string} path */
async function load(path) {
  const { mtimeMs, size } = statSync(path);
  return structuredClone((await import(`${pathToFileURL(path).href}?v=${mtimeMs}-${size}`)).default);
}

/**
 * Writes a format-2 copy of a format-1 document.
 * @param {string} from
 * @param {string} to
 */
export async function bootstrap(from, to) {
  rmSync(to, { recursive: true, force: true });
  mkdirSync(join(to, 'objects'), { recursive: true });
  mkdirSync(join(to, 'scenes'), { recursive: true });
  const manifestFile = join(from, 'manifest.json');
  const manifest = existsSync(manifestFile) ? JSON.parse(readFileSync(manifestFile, 'utf8')) : { format: 1 };
  manifest.format = 2;
  writeFileSync(join(to, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
  writeFileSync(join(to, 'palette.json'), canonical('palette.json', await load(join(from, 'palette.mjs'))));
  for (const sub of ['objects', 'scenes']) {
    if (!existsSync(join(from, sub))) continue;
    for (const f of readdirSync(join(from, sub)).filter((f) => f.endsWith('.mjs')).sort()) {
      const file = `${sub}/${f.slice(0, -'.mjs'.length)}.json`;
      writeFileSync(join(to, file), canonical(file, await load(join(from, sub, f))));
    }
  }
}

// -------------------------------------------------------------------- goldens

/** The documents of the corpus, as [name, format-1 folder]. @param {string} dir @param {string | null} site */
function corpusDocs(dir, site) {
  // examples/ is in format 2 now; the corpus is format 1, so the examples are the ones at
  // PR 2's merge commit (the parity gate's), by `git archive`, until cutover.
  const examples = join(dir, 'examples-format1');
  mkdirSync(examples, { recursive: true });
  execFileSync('tar', ['-x', '-C', examples], { input: execFileSync('git', ['-C', REPO, 'archive', '00ef555', 'examples/']) });
  const docs = [
    ['sample', join(REPO, 'cli/test/fixtures/sample.pixellab')],
    ['harbor', join(examples, 'examples/harbor.pixellab')],
    ['town', join(examples, 'examples/town.pixellab')],
  ];
  const format1 = join(dir, 'format1');
  mkdirSync(format1, { recursive: true });
  const out = [];
  for (const [name, from] of docs) {
    const copy = join(format1, `${name}.pixellab`);
    rmSync(copy, { recursive: true, force: true });
    cpSync(from, copy, { recursive: true });
    out.push([name, copy]);
  }
  if (site && existsSync(site)) {
    const copy = join(format1, 'owner.pixellab');
    rmSync(copy, { recursive: true, force: true });
    // document copy only reads the checkout; the copy is ours.
    execFileSync('node', [CLI, 'document', 'copy', '--to', copy, '--site', site], { stdio: ['ignore', 'ignore', 'inherit'], env: { ...process.env, PIXELLAB_DOC: '' } });
    out.push(['owner', copy]);
  }
  return out;
}

/** The read commands' invocations for one document, from what it holds. @param {any} sources */
function invocations(sources) {
  /** @type {{ command: string, params: Record<string, unknown> }[]} */
  const list = [];
  const add = (/** @type {string} */ command, /** @type {Record<string, unknown>} */ params = {}) => list.push({ command, params });
  add('document.info');
  add('palette.get');
  add('scene.list');
  add('object.list');
  add('object.list', { kind: 'block' });
  add('check');
  add('activity.list');
  add('proposal.list');
  add('art.preview', { name: 'palette' });
  for (const name of sources.objects.keys()) {
    add('check', { name, kind: 'object' });
    add('object.show', { name, doc: true });
    add('object.render', { name });
    add('object.render', { name, scale: 2, time: 450 });
    add('object.pick', { name, x: 1, y: 1 });
    add('art.preview', { name, kind: 'object' });
    add('chat.context', { object: name });
  }
  for (const [name, scene] of sources.scenes) {
    add('check', { name, kind: 'scene' });
    add('scene.show', { name, doc: true });
    add('scene.items', { name });
    add('scene.render', { name });
    add('scene.render', { name, theme: 'dark' });
    if (scene.motion) for (const time of [0, 250, Math.floor(scene.motion.duration / 3), Math.floor((scene.motion.duration * 2) / 3)]) add('scene.render', { name, time });
    add('art.preview', { name, kind: 'scene' });
    add('art.sizes', { name });
    add('chat.context', { name });
  }
  return list;
}

/** @param {string} dir @param {string | null} site */
async function goldens(dir, site) {
  const { openDocument } = await import(join(REPO, 'cli/lib/document.mjs'));
  const { registry } = await import(join(REPO, 'cli/lib/registry.mjs'));
  const { Session } = await import(join(REPO, 'cli/lib/session.mjs'));
  const { compileScene } = await import(join(REPO, 'cli/lib/engine/compile.mjs'));
  const { renderPreview, encodePng, previewColors } = await import(join(REPO, 'cli/lib/engine/preview.mjs'));
  const state = mkdtempSync(join(tmpdir(), 'pixellab-goldens-state-'));
  const env = { ...process.env, PIXELLAB_STATE: state, PIXELLAB_DOC: '' };
  const corpus = [];
  for (const [name, root] of corpusDocs(dir, site)) {
    const format2 = join(dir, 'format2', `${name}.pixellab`);
    await bootstrap(root, format2);
    const out = join(dir, 'goldens', name);
    rmSync(out, { recursive: true, force: true });
    mkdirSync(join(out, 'svg'), { recursive: true });
    mkdirSync(join(out, 'png'), { recursive: true });
    const doc = await openDocument({ doc: root, env });
    const session = new Session(doc, { env });
    const ctx = session.context();
    const { sources } = await ctx.sources();
    const budgets = doc.budgets;
    for (const scene of sources.scenes.keys()) writeFileSync(join(out, 'svg', `${scene}.svg`), compileScene(sources, scene, budgets).output);
    const colors = previewColors(doc.previewColors);
    for (const object of sources.objects.keys()) writeFileSync(join(out, 'png', `object-${object}.png`), encodePng(renderPreview(sources, { object }, colors).image, 1));
    for (const scene of sources.scenes.keys()) writeFileSync(join(out, 'png', `scene-${scene}.png`), encodePng(renderPreview(sources, { scene }, colors).image, 1));
    const results = [];
    for (const { command, params } of invocations(sources)) {
      try {
        results.push({ command, params, ok: true, result: await registry.call(session.context(), command, structuredClone(params)) });
      } catch (err) {
        const e = /** @type {any} */ (err);
        results.push({ command, params, ok: false, error: { name: e.name, code: e.code ?? 'internal', message: e.message } });
      }
    }
    writeFileSync(join(out, 'commands.json'), `${JSON.stringify(results, null, 2)}\n`);
    corpus.push({ name, format1: root, format2, root });
    process.stderr.write(`${name}: ${sources.scenes.size} scenes, ${sources.objects.size} objects, ${results.length} commands\n`);
  }
  rmSync(state, { recursive: true, force: true });
  writeFileSync(join(dir, 'corpus.json'), `${JSON.stringify({ docs: corpus }, null, 2)}\n`);
}

// ---------------------------------------------------------------------- jsnum

/** @param {string} file */
async function jsnum(file) {
  const { pctOf } = await import(join(REPO, 'cli/lib/engine/motion.mjs'));
  /** @type {Set<number>} */
  const numbers = new Set([0, -0, 0.5, -0.5, 1.5, -1.5, 2.5, -2.5, 1e21, -1e21, 1e-7, 1.5e-7, 1e-6, 123e-20, 0.1, 0.2, 0.30000000000000004, 1 / 3, 2 / 3, 100 / 7, 1.005, 1.45, 0.125, 99.9995, 4.35, 8.345, 1e20, 123456789.125, Number.MAX_SAFE_INTEGER, 2 ** 53 + 2, 5e-324, 1.7976931348623157e308]);
  // The percentages motion prints: every key time over every duration a scene can have, in steps.
  for (const duration of [100, 300, 600, 700, 1000, 1200, 1500, 2400, 3000, 7000, 9400, 12000, 16000, 20000, 60000]) {
    for (let t = 0; t <= duration; t += Math.max(1, Math.floor(duration / 97))) {
      numbers.add((t / duration) * 100);
      numbers.add(pctOf(t, duration));
    }
  }
  // The sizes kb() prints, and the steps between keys.
  for (let bytes = 0; bytes < 300 * 1024; bytes += 997) numbers.add(bytes / 1024);
  for (let n = 1; n < 40; n++) for (let k = 0; k <= n; k++) numbers.add((100 * k) / n);
  const cases = [...numbers].map((x) => ({
    x: Object.is(x, -0) ? '-0' : String(x),
    bits: Buffer.from(new Float64Array([x]).buffer).toString('hex'),
    string: String(x),
    round: Object.is(Math.round(x), -0) ? '-0' : String(Math.round(x)),
    fixed: [0, 1, 2, 3, 4].map((d) => x.toFixed(d)),
  }));
  writeFileSync(file, `${JSON.stringify({ cases }, null, 1)}\n`);
  process.stderr.write(`${cases.length} number cases → ${file}\n`);
}

// ----------------------------------------------------------------------- main

const args = process.argv.slice(2);
const flag = (/** @type {string} */ name) => {
  const i = args.indexOf(name);
  return i < 0 ? null : (args[i + 1] ?? null);
};
if (args[0] === '--bootstrap' && args.length === 3) {
  await bootstrap(resolve(args[1]), resolve(args[2]));
} else if (args[0] === '--out' && args[1]) {
  const site = flag('--site') ?? join(homedir(), 'Projects', 'jlawcordova.github.io');
  await goldens(resolve(args[1]), existsSync(site) ? resolve(site) : null);
} else if (args[0] === '--jsnum' && args[1]) {
  await jsnum(resolve(args[1]));
} else {
  process.stderr.write('usage: node scripts/goldens.mjs --bootstrap <from.pixellab> <to.pixellab> | --out <dir> [--site <checkout>] | --jsnum <file.json>\n');
  process.exitCode = 2;
}
void basename;
