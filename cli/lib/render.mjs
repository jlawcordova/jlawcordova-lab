// Rendering for the doors: a scene or object as a transparent PNG at an
// integer scale, with each item's bounds in output pixels so the app can
// hit-test clicks. It does what the web lab's stage does
// (src/components/lab/stage.ts in the site): pose the scene's moving items at
// `time`, show one variant of each run of sibling data-class groups, render
// with the engine's renderScene and composite, and take each item's pixels
// from the engine's renderObject at its placement point.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { CommandError } from './errors.mjs';

export const MAX_SCALE = 16;

/** A path into a scene's items: [5, 2] ↔ "5.2". */
export const pathString = (/** @type {number[]} */ path) => path.join('.');

/** Parses "5.2" to [5, 2]; null if it isn't a path. */
export function parsePath(/** @type {string} */ text) {
  if (typeof text !== 'string' || !/^\d+(\.\d+)*$/.test(text)) return null;
  return text.split('.').map(Number);
}

/**
 * Where an item's placement point is, in scene pixels.
 * @param {any} site
 * @param {any} item
 * @param {number[]} origin
 * @returns {[number, number]}
 */
export function placementPoint(site, item, origin) {
  return item.at?.tile ? site.iso.tileToPx(item.at.tile, origin) : [item.at?.px?.[0] ?? 0, item.at?.px?.[1] ?? 0];
}

const isVariant = (/** @type {any} */ item) => item && typeof item === 'object' && 'group' in item && 'data-class' in (item.group ?? {});

/**
 * The scene as the stage shows it (a port of the lab's visibleItems): of each
 * run of sibling data-class groups only one shows, the one holding `selected`,
 * else the first. Returns the trimmed items, and every item that shows, in
 * paint order, with its path into the untrimmed items.
 * @param {any[]} items
 * @param {number[] | null} selected
 * @param {number[]} [prefix]
 * @returns {{ kept: any[], leaves: { path: number[], item: any }[] }}
 */
export function visibleItems(items, selected, prefix = []) {
  /** @type {any[]} */
  const kept = [];
  /** @type {{ path: number[], item: any }[]} */
  const leaves = [];
  /** @type {number[]} */
  let run = [];
  const flush = () => {
    if (run.length === 0) return;
    const holder = run.find((i) => selected && selected.length > prefix.length && selected[prefix.length] === i);
    const shown = holder ?? run[0];
    const sub = visibleItems(items[shown].items ?? [], selected, [...prefix, shown]);
    kept.push({ ...items[shown], items: sub.kept });
    leaves.push(...sub.leaves);
    run = [];
  };
  items.forEach((item, i) => {
    if (isVariant(item)) return void run.push(i);
    flush();
    if (item && typeof item === 'object' && 'group' in item) {
      const sub = visibleItems(item.items ?? [], selected, [...prefix, i]);
      kept.push({ ...item, items: sub.kept });
      leaves.push(...sub.leaves);
    } else {
      kept.push(item);
      leaves.push({ path: [...prefix, i], item });
    }
  });
  flush();
  return { kept, leaves };
}

/**
 * Every item of a scene, flattened in paint order, with groups listed apart.
 * @param {any} site
 * @param {any} scene
 */
export function flattenItems(site, scene) {
  const origin = scene.origin ?? [0, 0];
  /** @type {any[]} */
  const items = [];
  /** @type {any[]} */
  const groups = [];
  const walk = (/** @type {any[]} */ list, /** @type {number[]} */ prefix, /** @type {string | undefined} */ group) => {
    (list ?? []).forEach((item, i) => {
      const path = [...prefix, i];
      if (item && typeof item === 'object' && 'group' in item) {
        groups.push({
          path: pathString(path),
          attrs: { ...item.group },
          items: (item.items ?? []).length,
          ...(item.depth !== undefined ? { depth: item.depth } : {}),
          ...(group ? { group } : {}),
        });
        walk(item.items, path, pathString(path));
        return;
      }
      items.push({
        path: pathString(path),
        object: item?.object,
        at: item?.at,
        // A pixel item stands on a level of its own (default 0); a tile's level is its third number.
        ...(item?.at?.px ? { level: item.at.level ?? 0 } : {}),
        ...(item?.depth !== undefined ? { depth: item.depth } : {}),
        ...(item?.class !== undefined ? { class: item.class } : {}),
        ...(item?.id !== undefined ? { id: item.id } : {}),
        ...(item?.layer !== undefined ? { layer: item.layer } : {}),
        ...(group ? { group } : {}),
        point: placementPoint(site, item ?? {}, origin),
      });
    });
  };
  walk(scene.items, [], undefined);
  return { items, groups, layers: layerSummary(scene, items) };
}

/**
 * The scene's layers (docs/intents/2026-10-scene-layers/spec.md, D4), each
 * with how many items are in it or in the layers inside it.
 * @param {any} scene
 * @param {{ layer?: string }[]} items
 */
function layerSummary(scene, items) {
  const layers = Array.isArray(scene.layers) ? scene.layers : [];
  /** @type {Map<string, string | undefined>} */
  const parents = new Map(layers.map((/** @type {any} */ l) => [l.id, l.parent]));
  /** @type {Map<string, number>} */
  const counts = new Map();
  for (const item of items) {
    /** @type {Set<string>} */
    const seen = new Set();
    for (let l = item.layer; l !== undefined && parents.has(l) && !seen.has(l); l = parents.get(l)) {
      seen.add(l);
      counts.set(l, (counts.get(l) ?? 0) + 1);
    }
  }
  return layers.map((/** @type {any} */ l) => ({ id: l.id, name: l.name, ...(l.parent !== undefined ? { parent: l.parent } : {}), items: counts.get(l.id) ?? 0 }));
}

/**
 * The scene's items with every placed item for which `hidden(item)` holds
 * marked hidden (as a moving item away from the stage is), leaving paths alone.
 * @param {any[]} items
 * @param {(item: any) => boolean} hidden
 * @returns {any[]}
 */
function hideWhere(items, hidden) {
  return items.map((item) => {
    if (item && typeof item === 'object' && 'group' in item) return { ...item, items: hideWhere(item.items ?? [], hidden) };
    return item && !item.hidden && hidden(item) ? { ...item, hidden: true } : item;
  });
}

/** The color name the dark theme's ground pixels get (not a palette name). */
const PAGE = '--color-page';
const DARK_PAGE = '#1C1712';

/**
 * The site's dark-theme page color: --color-page in the [data-theme="dark"]
 * block of src/styles/variables.css, else #1C1712.
 * @param {string} root
 */
export function darkPageColor(root) {
  try {
    const css = readFileSync(join(root, 'src/styles/variables.css'), 'utf8');
    const at = css.indexOf('[data-theme="dark"]');
    const m = at < 0 ? null : /--color-page:\s*(#[0-9a-fA-F]{6})\b/.exec(css.slice(at));
    return m ? m[1].toUpperCase() : DARK_PAGE;
  } catch {
    return DARK_PAGE;
  }
}

/**
 * The site's dark theme in a rendered tree: every pixel under a group with
 * data-enter="shadow" (the hero island's dithered drop shadow) takes the page
 * color, as src/styles/hero.css does with
 * `html[data-theme="dark"] .hero-island [data-enter="shadow"] path { fill: var(--color-page) }`.
 * The sources are untouched; only this render changes.
 * @param {any} group  the engine's render tree
 * @param {boolean} [inside]
 */
function darkTheme(group, inside = false) {
  for (const child of group.children ?? []) {
    if ('pixels' in child) {
      if (inside) child.pixels = new Map([...child.pixels.keys()].map((p) => [p, PAGE]));
      continue;
    }
    const shadow = inside || (child.attrs ?? []).some((/** @type {string[]} */ [k, v]) => k === 'data-enter' && v === 'shadow');
    darkTheme(child, shadow);
  }
}

/**
 * An RGBA buffer of the pixels inside a box; everything else transparent.
 * @param {any} sources
 * @param {Map<string, string>} pixels  "x,y" → color name
 * @param {number[]} box  [x, y, width, height]
 * @param {Record<string, string>} [extra]  hex of color names outside the palette
 */
function toImage(sources, pixels, [bx, by, width, height], extra = {}) {
  const rgba = Buffer.alloc(width * height * 4);
  /** @type {Map<string, number[]>} */
  const rgb = new Map();
  for (const [p, name] of pixels) {
    const comma = p.indexOf(',');
    const x = Number(p.slice(0, comma)) - bx;
    const y = Number(p.slice(comma + 1)) - by;
    if (x < 0 || y < 0 || x >= width || y >= height) continue;
    let color = rgb.get(name);
    if (!color) {
      const hex = sources.colors.get(name)?.hex ?? extra[name] ?? '#000000';
      color = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
      rgb.set(name, color);
    }
    rgba.set([...color, 255], (y * width + x) * 4);
  }
  return { width, height, rgba };
}

/**
 * The engine's pixels for one object drawn alone with its anchor at (0, 0),
 * and their bounding box.
 * @param {any} site
 * @param {any} sources
 * @param {string} name
 * @param {{ time?: number, frame?: number }} at
 */
function objectPixels(site, sources, name, { time, frame }) {
  const pixels = site.engine.composite(site.engine.renderObject(sources, name), time === undefined ? { frame: frame ?? 0 } : { time });
  return { pixels, box: site.engine.bounds(pixels.keys()) };
}

/** Scene-pixel box → [x, y, w, h] in output pixels. */
const toOutput = (/** @type {any} */ box, /** @type {number[]} */ [vx, vy], /** @type {number} */ scale) =>
  box ? [(box.minX - vx) * scale, (box.minY - vy) * scale, (box.maxX - box.minX + 1) * scale, (box.maxY - box.minY + 1) * scale] : null;

/**
 * Renders a scene at `time` ms to a PNG, with its items' bounds.
 * @param {any} site
 * @param {any} sources
 * @param {string} name
 * With `theme: 'dark'` it draws as the site's dark theme does (darkTheme).
 * `hide` is a set of layer ids whose items aren't drawn or listed; with
 * `focus` (a set of layer ids) the result adds `focusPng`, the same render
 * with only those layers' items (docs/intents/2026-10-scene-layers/spec.md, D4).
 * @param {{ scale?: number, time?: number, show?: number[] | null, theme?: 'light' | 'dark', hide?: Set<string> | null, focus?: Set<string> | null }} [options]
 */
export function renderSceneImage(site, sources, name, { scale = 1, time = 0, show = null, theme = 'light', hide = null, focus = null } = {}) {
  const { engine, motion: motionLib, preview } = site;
  const scene = sources.scenes.get(name);
  const origin = scene.origin ?? [0, 0];
  const viewBox = scene.viewBox;
  try {
    const { motion, ...still } = scene;
    let posed = motionLib.poseItems(scene.items ?? [], motion, time, (/** @type {any} */ item) => placementPoint(site, item, origin));
    if (hide?.size) posed = hideWhere(posed, (item) => item.layer !== undefined && hide.has(item.layer));
    const { kept, leaves } = visibleItems(posed, show);
    /** The PNG of these items (trimmed to the shown variants), as the scene draws them. */
    const paint = (/** @type {any[]} */ shown) => {
      const view = { ...still, items: motionLib.shownItems(shown) };
      const local = { ...sources, scenes: new Map([[name, view]]) };
      const { root } = engine.renderScene(local, name);
      if (theme === 'dark') darkTheme(root);
      return toImage(sources, engine.composite(root, { time }), viewBox, theme === 'dark' ? { [PAGE]: site.theme.darkPage } : {});
    };
    const image = paint(kept);
    const focusImage = focus ? paint(hideWhere(kept, (item) => item.layer === undefined || !focus.has(item.layer))) : null;
    /** @type {Map<string, ReturnType<typeof objectPixels>>} */
    const arts = new Map();
    const art = (/** @type {string} */ object) => {
      let found = arts.get(object);
      if (!found) arts.set(object, (found = objectPixels(site, sources, object, { time })));
      return found;
    };
    const original = (/** @type {number[]} */ path) => path.reduce((list, i, k) => (k === path.length - 1 ? list[i] : list[i].items), scene.items);
    const items = leaves
      .filter(({ item }) => !item.hidden)
      .map(({ path, item }) => {
        const source = original(path);
        const [x, y] = placementPoint(site, item, origin);
        const { box } = art(item.object);
        const shifted = box && { minX: box.minX + x, minY: box.minY + y, maxX: box.maxX + x, maxY: box.maxY + y };
        return {
          path: pathString(path),
          object: source.object,
          ...(item.object !== source.object ? { pose: item.object } : {}),
          at: source.at,
          ...(source.id !== undefined ? { id: source.id } : {}),
          bounds: toOutput(shifted, viewBox, scale),
        };
      });
    return {
      png: preview.encodePng(image, scale).toString('base64'),
      scale,
      viewBox: [...viewBox],
      width: image.width * scale,
      height: image.height * scale,
      time,
      ...(theme === 'dark' ? { theme } : {}),
      ...(focusImage ? { focusPng: preview.encodePng(focusImage, scale).toString('base64') } : {}),
      items,
    };
  } catch (err) {
    if (err instanceof CommandError) throw err;
    throw new CommandError(`scene ${name} can't render: ${/** @type {Error} */ (err).message}; run pixellab check ${name}`, { code: 'render_error' });
  }
}

/**
 * Some of a sprite's layers at a frame (each clamped to its last frame), with
 * its anchor at (0, 0), and their box. Port of the lab's ObjectMode.pixelsAt.
 * @param {any} site
 * @param {any} resolved
 * @param {number[]} layers
 * @param {number} frame
 */
function layerPixels(site, resolved, layers, frame) {
  /** @type {Map<string, string>} */
  const pixels = new Map();
  const [ax, ay] = resolved.anchor;
  resolved.layers.forEach((/** @type {any} */ layer, /** @type {number} */ i) => {
    if (!layers.includes(i)) return;
    site.edit.mapAt(layer, frame).forEach((/** @type {string} */ row, /** @type {number} */ y) => {
      for (let x = 0; x < row.length; x++) {
        const color = row[x] !== '.' && resolved.keys[row[x]];
        if (color) pixels.set(`${x - ax},${y - ay}`, color);
      }
    });
  });
  return { pixels, box: site.engine.bounds(pixels.keys()) };
}

/**
 * Renders one object to a PNG with its anchor at (0, 0). A sprite's viewBox
 * is its whole map, so output pixel (x, y) at 1× is map column x, row y; a
 * block's is the box around its painted pixels.
 * With `layers` (a sprite's layer indexes) only those layers show, each at
 * `frame` clamped to its own last frame, as the lab's object stage draws them
 * (ObjectMode.pixelsAt): hidden layers and the onion skin. A block ignores it.
 * @param {any} site
 * @param {any} sources
 * @param {string} name
 * @param {{ scale?: number, time?: number, frame?: number, layers?: number[] }} [options]
 */
export function renderObjectImage(site, sources, name, { scale = 1, time, frame = 0, layers } = {}) {
  const { engine, preview } = site;
  try {
    const resolved = engine.resolve(sources, name);
    const { pixels, box } = layers && !resolved.block ? layerPixels(site, resolved, layers, frame) : objectPixels(site, sources, name, { time, frame });
    let viewBox;
    if (!resolved.block) viewBox = [-resolved.anchor[0], -resolved.anchor[1], resolved.width, resolved.height];
    else if (box) viewBox = [box.minX, box.minY, box.maxX - box.minX + 1, box.maxY - box.minY + 1];
    else viewBox = [0, 0, 1, 1];
    const image = toImage(sources, pixels, viewBox);
    return {
      png: preview.encodePng(image, scale).toString('base64'),
      scale,
      viewBox,
      width: image.width * scale,
      height: image.height * scale,
      ...(time !== undefined ? { time } : { frame }),
      anchor: resolved.block ? [0, 0] : [...resolved.anchor],
      items: [{ path: null, object: name, at: { px: [0, 0] }, bounds: toOutput(box, viewBox, scale) }],
    };
  } catch (err) {
    if (err instanceof CommandError) throw err;
    throw new CommandError(`object ${name} can't render: ${/** @type {Error} */ (err).message}; run pixellab check ${name}`, { code: 'render_error' });
  }
}
