// Compiling scenes to the compact SVGs a site inlines (the site's pixel-art
// engine spec D6, R7, R8). Ported from the site's scripts/optimize-pixel-art.mjs
// (jlawcordova.github.io at 3d2a9d1), the command-line parts left to
// cli/lib/commands/art.mjs; the pixels this writes are byte for byte what
// `npm run art` wrote.
//
// Each scene renders with the engine to a rect SVG, one <rect> per run, which
// optimizeSvg() below turns into paths: each <g fill> group made only of rects
// becomes one <path> of merged runs. Every other element, attribute and the
// order are kept, so the CSS classes, data-class variants and z-order still
// work. verifyLossless() then checks that every fill group covers exactly the
// same pixels as before and that nothing else changed, and the budget is
// checked: 100 KB raw and 25 KB gzip per file, unless the document's manifest
// gives an output its own (the site's hero island has 750 KB and 150 KB).
// Node built-ins only.

import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { gzipSync } from 'node:zlib';

import { loadSources, objectFile, renderScene, sceneFile, validate } from './engine.mjs';
import { itemsById } from './motion.mjs';
import { toRectSvg } from './svg.mjs';

/** The engine's budget for an output, in bytes, unless the manifest gives it one. */
export const BUDGET = { raw: 100 * 1024, gzip: 25 * 1024 };

/**
 * @typedef {{ raw: number, gzip: number }} Budget
 * @typedef {Record<string, Budget>} Budgets  by output file name
 */

// Just enough XML for the sources: elements with double-quoted attributes,
// and text. Comments, CDATA and processing instructions are refused.
const TOKEN = /<(\/?)([a-zA-Z][\w:-]*)((?:\s+[\w:-]+="[^"<]*")*)\s*(\/?)>|([^<]+)/y;
const ATTR = /([\w:-]+)="([^"]*)"/g;

/**
 * Parses the sources' markup into { name, attrs, children, selfClosing } and { text } nodes.
 * @param {string} source
 * @returns {any[]}
 */
export function parse(source) {
  /** @type {any} */
  const root = { children: [] };
  const stack = [root];
  TOKEN.lastIndex = 0;
  while (TOKEN.lastIndex < source.length) {
    const at = TOKEN.lastIndex;
    const m = TOKEN.exec(source);
    if (!m) throw new Error(`Unsupported markup at offset ${at}: ${source.slice(at, at + 40)}`);
    const [, closing, name, attrText, selfClosing, text] = m;
    const parent = stack[stack.length - 1];
    if (text !== undefined) {
      parent.children.push({ text });
    } else if (closing) {
      if (stack.length === 1 || parent.name !== name) throw new Error(`Unsupported markup: unexpected </${name}> at offset ${at}`);
      stack.pop();
    } else {
      const attrs = [...attrText.matchAll(ATTR)].map(([, k, v]) => [k, v]);
      const el = { name, attrs, children: [], selfClosing: Boolean(selfClosing) };
      parent.children.push(el);
      if (!selfClosing) stack.push(el);
    }
  }
  if (stack.length !== 1) throw new Error(`Unsupported markup: <${stack[stack.length - 1].name}> is never closed`);
  return root.children;
}

export const attr = (/** @type {any} */ el, /** @type {string} */ key) => el.attrs.find((/** @type {string[]} */ [k]) => k === key)?.[1];
const attrsText = (/** @type {string[][]} */ attrs) => attrs.map(([k, v]) => ` ${k}="${v}"`).join('');

function serialize(/** @type {any[]} */ nodes) {
  return nodes
    .map((n) => {
      if (n.text !== undefined) return n.text;
      if (n.selfClosing) return `<${n.name}${attrsText(n.attrs)}/>`;
      return `<${n.name}${attrsText(n.attrs)}>${serialize(n.children)}</${n.name}>`;
    })
    .join('');
}

/** A <g fill> whose children are all <rect>s (at least one). */
export function isRectGroup(/** @type {any} */ el) {
  return el.name === 'g' && attr(el, 'fill') !== undefined && el.children.length > 0 && el.children.every((/** @type {any} */ c) => c.name === 'rect');
}

function wholeNumber(/** @type {any} */ rect, /** @type {string} */ key) {
  const raw = attr(rect, key) ?? (key === 'x' || key === 'y' ? '0' : undefined);
  if (raw === undefined || !/^-?\d+$/.test(raw)) {
    throw new Error(`Only whole-number rects are supported; got ${key}="${raw}"`);
  }
  return Number(raw);
}

/** Pixels as a Set of "x,y" keys. */
export function rectPixels(/** @type {any[]} */ rects) {
  const pixels = new Set();
  for (const r of rects) {
    const [x, y, w, h] = ['x', 'y', 'width', 'height'].map((k) => wholeNumber(r, k));
    for (let dy = 0; dy < h; dy++) for (let dx = 0; dx < w; dx++) pixels.add(`${x + dx},${y + dy}`);
  }
  return pixels;
}

/**
 * Path data for a pixel set: horizontal runs per row, then runs with the same
 * x and width in consecutive rows merged into one taller rectangle.
 * @param {Set<string>} pixels
 */
function pathData(pixels) {
  /** @type {Map<number, number[]>} */
  const rows = new Map();
  for (const key of pixels) {
    const [x, y] = key.split(',').map(Number);
    if (!rows.has(y)) rows.set(y, []);
    /** @type {number[]} */ (rows.get(y)).push(x);
  }
  // Runs per row, keyed "x,w" so the row below can be checked in O(1).
  /** @type {Map<number, Map<string, { x: number, w: number }>>} */
  const runs = new Map();
  for (const [y, xs] of rows) {
    xs.sort((a, b) => a - b);
    const rowRuns = new Map();
    for (let i = 0; i < xs.length; ) {
      let j = i;
      while (j + 1 < xs.length && xs[j + 1] === xs[j] + 1) j++;
      rowRuns.set(`${xs[i]},${j - i + 1}`, { x: xs[i], w: j - i + 1 });
      i = j + 1;
    }
    runs.set(y, rowRuns);
  }
  let d = '';
  for (const y of [...runs.keys()].sort((a, b) => a - b)) {
    for (const [key, { x, w }] of [.../** @type {Map<string, any>} */ (runs.get(y))].sort(([, a], [, b]) => a.x - b.x)) {
      let h = 1;
      while (runs.get(y + h)?.has(key)) {
        /** @type {Map<string, any>} */ (runs.get(y + h)).delete(key);
        h++;
      }
      d += `M${x} ${y}h${w}v${h}h-${w}z`;
    }
  }
  return d;
}

function optimizeNodes(/** @type {any[]} */ nodes) {
  return nodes.map((n) => {
    if (n.text !== undefined) return n;
    if (isRectGroup(n)) {
      return { name: 'path', attrs: [...n.attrs, ['d', pathData(rectPixels(n.children))]], children: [], selfClosing: true };
    }
    return { ...n, children: optimizeNodes(n.children) };
  });
}

/** Rect form → path form. Throws on markup it doesn't understand. */
export function optimizeSvg(/** @type {string} */ source) {
  return serialize(optimizeNodes(parse(source)));
}

const PATH_RUN = /M(-?\d+) (-?\d+)h(\d+)v(\d+)h-(\d+)z/y;

function pathPixels(/** @type {string} */ d) {
  const pixels = new Set();
  PATH_RUN.lastIndex = 0;
  while (PATH_RUN.lastIndex < d.length) {
    const m = PATH_RUN.exec(d);
    if (!m || m[3] !== m[5]) throw new Error(`Unexpected path data: ${d.slice(PATH_RUN.lastIndex, PATH_RUN.lastIndex + 40)}`);
    const [x, y, w, h] = m.slice(1, 5).map(Number);
    for (let dy = 0; dy < h; dy++) for (let dx = 0; dx < w; dx++) pixels.add(`${x + dx},${y + dy}`);
  }
  return pixels;
}

/**
 * Flattens a tree into a sequence of steps, with each fill group (source) or
 * generated path (output) reduced to its fill, other attributes and pixels.
 * @param {any[]} nodes
 * @param {any[]} [out]
 */
function steps(nodes, out = []) {
  for (const n of nodes) {
    if (n.text !== undefined) out.push({ text: n.text });
    else if (isRectGroup(n)) out.push({ pixels: rectPixels(n.children), attrs: n.attrs });
    else if (n.name === 'path' && attr(n, 'd') !== undefined) {
      out.push({ pixels: pathPixels(attr(n, 'd')), attrs: n.attrs.filter((/** @type {string[]} */ [k]) => k !== 'd') });
    } else {
      out.push({ open: `<${n.name}${attrsText(n.attrs)}>` });
      steps(n.children, out);
      out.push({ close: n.name });
    }
  }
  return out;
}

const sameSet = (/** @type {Set<string>} */ a, /** @type {Set<string>} */ b) => a.size === b.size && [...a].every((p) => b.has(p));

/** Throws unless `output` draws exactly the same pixels, groups and structure as `source`. */
export function verifyLossless(/** @type {string} */ source, /** @type {string} */ output) {
  const a = steps(parse(source));
  const b = steps(parse(output));
  if (a.length !== b.length) throw new Error(`Structure changed: ${a.length} steps before, ${b.length} after`);
  for (let i = 0; i < a.length; i++) {
    const [x, y] = [a[i], b[i]];
    if (x.pixels && y.pixels) {
      const label = `fill group ${attr(x, 'fill') ?? attrsText(x.attrs)} (step ${i})`;
      if (attrsText(x.attrs) !== attrsText(y.attrs)) throw new Error(`Attributes changed on ${label}`);
      if (!sameSet(x.pixels, y.pixels)) throw new Error(`Pixels changed in ${label}: ${x.pixels.size} before, ${y.pixels.size} after`);
    } else if (JSON.stringify(x) !== JSON.stringify(y)) {
      throw new Error(`Structure changed at step ${i}: ${JSON.stringify(x)} → ${JSON.stringify(y)}`);
    }
  }
}

export const kb = (/** @type {number} */ bytes) => `${(bytes / 1024).toFixed(1)} KB`;

/**
 * Loads palette.mjs, objects/*.mjs and scenes/*.mjs from a document folder,
 * as `npm run art` read them (plain imports, no cache busting). A file that
 * can't be imported is reported like any other problem.
 * @param {string} dir
 * @returns {Promise<{ sources: any, loadErrors: string[] }>}
 */
export async function readSources(dir) {
  /** @type {string[]} */
  const loadErrors = [];
  const load = async (/** @type {string} */ file) => {
    try {
      return (await import(pathToFileURL(join(dir, file)).href)).default;
    } catch (err) {
      loadErrors.push(`${file}: cannot be loaded: ${/** @type {Error} */ (err).message.split('\n')[0]}`);
      return undefined;
    }
  };
  const folder = async (/** @type {string} */ sub) => {
    const files = existsSync(join(dir, sub)) ? (await readdir(join(dir, sub))).filter((f) => f.endsWith('.mjs')).sort() : [];
    /** @type {Record<string, any>} */
    const out = {};
    for (const f of files) {
      const data = await load(`${sub}/${f}`);
      if (data !== undefined) out[f.slice(0, -'.mjs'.length)] = data;
    }
    return out;
  };
  const palette = await load('palette.mjs');
  const sources = loadSources({ palette, objects: await folder('objects'), scenes: await folder('scenes') });
  return { sources, loadErrors };
}

/**
 * Renders a scene and runs it through the optimizer, lossless check and
 * budget. A preview-only scene, or one without its own budget, gets the
 * engine's.
 * @param {any} sources
 * @param {string} name
 * @param {Budgets} [budgets]  per output file, from the document's manifest
 */
export function compileScene(sources, name, budgets = {}) {
  const rects = toRectSvg(renderScene(sources, name), (/** @type {string} */ color) => sources.colors.get(color).hex);
  const output = optimizeSvg(rects);
  verifyLossless(rects, output);
  const [raw, gz] = [Buffer.byteLength(output), gzipSync(output).length];
  const [srcRaw, srcGz] = [Buffer.byteLength(rects), gzipSync(rects).length];
  const file = sources.scenes.get(name).output;
  const budget = (file && budgets[file]) || BUDGET;
  const over = raw > budget.raw || gz > budget.gzip;
  return { output, raw, gzip: gz, over, budget, report: `${kb(srcRaw)} / ${kb(srcGz)} gzip → ${kb(raw)} / ${kb(gz)} gzip, lossless${over ? ' — OVER BUDGET' : ''}` };
}

/**
 * Compiles every scene and writes those with an `output` into `out`, as
 * `npm run art` did: nothing is written when any source has a problem.
 * @param {any} sources
 * @param {string[]} problems  load errors and validation problems
 * @param {string} out  the publish folder
 * @param {Budgets} [budgets]
 * @returns {Promise<{ ok: boolean, outputs: { file: string, scene: string, raw: number, gzip: number, over: boolean, report: string }[], previewOnly: { scene: string, raw: number, gzip: number, over: boolean, report: string }[], problems: string[] }>}
 */
export async function compileAll(sources, problems, out, budgets = {}) {
  if (problems.length) return { ok: false, outputs: [], previewOnly: [], problems };
  /** @type {any[]} */
  const outputs = [];
  /** @type {any[]} */
  const previewOnly = [];
  let over = false;
  for (const name of sources.scenes.keys()) {
    const scene = sources.scenes.get(name);
    const { output, raw, gzip, over: isOver, report } = compileScene(sources, name, budgets);
    over ||= isOver;
    if (scene.output) {
      await mkdir(out, { recursive: true });
      await writeFile(join(out, scene.output), output);
      outputs.push({ file: scene.output, scene: name, raw, gzip, over: isOver, report });
    } else previewOnly.push({ scene: name, raw, gzip, over: isOver, report });
  }
  return { ok: !over, outputs, previewOnly, problems: [] };
}

/** How many items a scene places, groups flattened. */
export const countItems = (/** @type {any[]} */ items) => items.reduce((n, item) => n + (item.group ? countItems(item.items) : 1), 0);

/** A scene's compiled size in bytes, raw and gzip, without the lossless check. */
function measure(/** @type {any} */ sources, /** @type {string} */ name) {
  const output = optimizeSvg(toRectSvg(renderScene(sources, name), (/** @type {string} */ color) => sources.colors.get(color).hex));
  return { output, raw: Buffer.byteLength(output), gzip: gzipSync(output).length };
}

/** The scene's items without any that place `object`. */
const withoutObject = (/** @type {any[]} */ items, /** @type {string} */ object) =>
  items.filter((item) => item.object !== object).map((item) => (item.group ? { ...item, items: withoutObject(item.items, object) } : item));

/** How many times each object is placed. */
function placements(/** @type {any[]} */ items, counts = new Map()) {
  for (const item of items) {
    if (item.group) placements(item.items, counts);
    else counts.set(item.object, (counts.get(item.object) ?? 0) + 1);
  }
  return counts;
}

/**
 * What each object and each color costs in a compiled scene: the bytes the
 * scene loses, raw and gzip, when the object's items are left out or the
 * color's paths are dropped. Costs overlap a little, so they don't add up to
 * the total exactly.
 * @param {any} sources
 * @param {string} name
 * @returns {{ raw: number, gzip: number, objects: Map<string, { placed: number, raw: number, gzip: number }>, colors: Map<string, { raw: number, gzip: number }> }}
 */
export function sceneSizes(sources, name) {
  const scene = sources.scenes.get(name);
  const total = measure(sources, name);
  const objects = new Map();
  for (const [object, placed] of placements(scene.items)) {
    const items = withoutObject(scene.items, object);
    // A motion track moves an item by id, so the tracks of the items just
    // taken out go with them (island level up spec).
    const left = itemsById(items);
    const motion = scene.motion && { ...scene.motion, tracks: scene.motion.tracks.filter((/** @type {any} */ t) => left.has(t.item)) };
    sources.scenes.set('sizes-probe', { ...scene, output: undefined, items, ...(motion ? { motion } : {}) });
    const less = measure(sources, 'sizes-probe');
    objects.set(object, { placed, raw: total.raw - less.raw, gzip: total.gzip - less.gzip });
  }
  sources.scenes.delete('sizes-probe');
  // A hex can have several names; the first tier's (world before outfit) wins.
  const names = new Map();
  for (const [color, { hex }] of sources.colors) if (!names.has(hex)) names.set(hex, color);
  const colors = new Map();
  for (const hex of new Set([...total.output.matchAll(/<path fill="(#[0-9A-F]{6})"/g)].map((m) => m[1]))) {
    const less = total.output.replace(new RegExp(`<path fill="${hex}"[^>]*/>`, 'g'), '');
    colors.set(`${names.get(hex) ?? '?'} ${hex}`, { raw: total.raw - Buffer.byteLength(less), gzip: total.gzip - gzipSync(less).length });
  }
  return { raw: total.raw, gzip: total.gzip, objects, colors };
}

/**
 * The sources as committed at HEAD of the git repository holding `dir`, or
 * the reason they can't be read. They're copied from git into a temp folder,
 * which the caller removes.
 * @param {string} dir
 * @returns {Promise<{ reason: string } | { temp: string, sources: any, problems: string[] }>}
 */
export async function headSources(dir) {
  const git = (/** @type {string[]} */ ...args) => execFileSync('git', ['-C', dir, ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'], maxBuffer: 64 * 1024 * 1024 });
  let files;
  try {
    files = git('ls-tree', '-r', '--name-only', 'HEAD', '--', '.').split('\n').filter((f) => f.endsWith('.mjs'));
  } catch {
    return { reason: 'not in a git repository with a HEAD commit' };
  }
  if (!files.length) return { reason: 'no sources at HEAD' };
  const temp = await mkdtemp(join(tmpdir(), 'pixel-art-head-'));
  for (const file of files) {
    await mkdir(dirname(join(temp, file)), { recursive: true });
    await writeFile(join(temp, file), git('show', `HEAD:./${file}`));
  }
  const { sources, loadErrors } = await readSources(temp);
  return { temp, sources, problems: [...loadErrors, ...validate(sources)] };
}

/**
 * Removes a headSources() temp folder.
 * @param {{ temp?: string }} head
 */
export const dropHead = (head) => (head.temp ? rm(head.temp, { recursive: true, force: true }) : Promise.resolve());

export { objectFile, sceneFile };
