// @ts-check
// Edits to object sources, for the editor's Object mode (pixel-art engine
// spec D9.5, R17). Each function changes a source document in place and keeps
// it in the canonical shape, so what the editor exports is what a person would
// write by hand. No DOM, so `npm test` covers it.
//
// A sprite either has its own `layers`, or `extends` another object and
// overrides whole rows of its map layers. Painting an object that extends
// another writes the painted row as an override, and drops the override again
// when the row matches the base. Painting a block writes its top `surface`
// and its `sides` (hero island detail spec R6).

import { isKey, layerMaps, resolve, SIDE_H, SIDE_W, sideCells } from './engine.mjs';
import { blockFaces, footprint, tilePixels, TILE_H, TILE_W } from './iso.mjs';
import { SITE_LOOPS } from './motion.mjs';

/**
 * @typedef {{ layer: number, frame: number }} Where
 * @typedef {import('./engine.mjs').Sources} Sources
 */

/** Keys tried for a new color after its own initial: lowercase, uppercase, digits, then punctuation. */
const KEY_ORDER = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!#$%&*+-=?@^_~:;<>/|()[]{}"`,\\\'';

/**
 * The map a layer shows at a frame: a frame loop shows frame `frame`, or its
 * last frame if it has fewer.
 * @param {any} layer
 * @param {number} frame
 * @returns {string[]}
 */
export function mapAt(layer, frame) {
  const maps = layerMaps(layer);
  return maps[Math.min(frame, maps.length - 1)];
}

/**
 * The key for a color: the one already mapped to it, or a new one, added to
 * the document's own `keys` (after the inherited ones, so it paints last).
 * @param {any} doc
 * @param {Record<string, string>} keys  the resolved keys
 * @param {string} color
 */
export function keyFor(doc, keys, color) {
  const found = Object.keys(keys).find((k) => keys[k] === color);
  if (found) return found;
  const initial = color[0] ?? 'a';
  const candidates = [initial, initial.toUpperCase(), ...KEY_ORDER];
  const key = /** @type {string} */ (candidates.find((k) => isKey(k) && !(k in keys)));
  doc.keys = { ...(doc.keys ?? {}), [key]: color };
  return key;
}

/**
 * Sets pixels on one layer and frame to a color, or clears them (`null`).
 * Pixels outside the map are ignored. Returns how many changed.
 * @param {Sources} sources  sources that hold `doc` as `name`
 * @param {string} name
 * @param {any} doc
 * @param {Where} where
 * @param {[number, number][]} pixels
 * @param {string | null} color
 */
export function paint(sources, name, doc, { layer, frame }, pixels, color) {
  const resolved = resolve(sources, name);
  const source = resolved.layers[layer];
  if (!source) return 0;
  const map = mapAt(source, frame);
  const ch = color === null ? '.' : keyFor(doc, resolved.keys, color);
  /** @type {Map<number, string[]>} */
  const rows = new Map();
  let changed = 0;
  for (const [x, y] of pixels) {
    if (y < 0 || y >= map.length || x < 0 || x >= map[y].length) continue;
    const row = rows.get(y) ?? [...map[y]];
    if (row[x] !== ch) changed++;
    row[x] = ch;
    rows.set(y, row);
  }
  if (changed === 0) return 0;
  if (doc.extends) {
    if ('frames' in source) throw new Error(`layer ${layer} is a frame loop; an object that extends another can only override map layers`);
    const base = resolve(sources, doc.extends).layers[layer];
    const baseMap = /** @type {string[]} */ (layerMaps(base)[0]);
    /** @type {Record<string, Record<string, string>>} */
    const overrides = { ...(doc.rows ?? {}) };
    const mine = { ...(overrides[layer] ?? {}) };
    for (const [y, row] of rows) {
      const text = row.join('');
      if (text === baseMap[y]) delete mine[y];
      else mine[y] = text;
    }
    // Rows in order, so the canonical source lists them top to bottom.
    const sorted = Object.fromEntries(Object.keys(mine).map(Number).sort((a, b) => a - b).map((y) => [y, mine[y]]));
    if (Object.keys(sorted).length) overrides[layer] = sorted;
    else delete overrides[layer];
    doc.rows = Object.fromEntries(Object.keys(overrides).map(Number).sort((a, b) => a - b).map((l) => [l, overrides[l]]));
  } else {
    const target = 'frames' in doc.layers[layer] ? doc.layers[layer].frames[Math.min(frame, doc.layers[layer].frames.length - 1)] : doc.layers[layer].map;
    for (const [y, row] of rows) target[y] = row.join('');
  }
  return changed;
}

/**
 * @typedef {{ part: 'surface' | 'left' | 'right', col: number, row: number }} BlockCell
 */

/**
 * Where each pixel of a block lands in its source, by "x,y" relative to its
 * first tile's top-face center: a top pixel is a cell of the surface map
 * (the cell its own tile's diamond shows), and a side pixel a cell of its
 * side map, the way the engine shears the map onto the face.
 * @param {number[]} size
 * @returns {Map<string, BlockCell>}
 */
export function blockCells(size) {
  /** @type {Map<string, BlockCell>} */
  const cells = new Map();
  for (const [tx, ty] of footprint(size, [0, 0])) {
    for (const p of tilePixels(tx, ty)) {
      const [x, y] = p.split(',').map(Number);
      cells.set(p, { part: 'surface', col: x - tx + TILE_W / 2, row: y - ty + TILE_H / 2 });
    }
  }
  const faces = blockFaces(size, [0, 0]);
  for (const part of /** @type {const} */ (['left', 'right'])) {
    for (const { p, col, row } of sideCells(faces[part])) cells.set(p, { part, col, row });
  }
  return cells;
}

/** @param {number} width @param {number} height */
const blankMap = (width, height) => Array(height).fill('.'.repeat(width));

/**
 * Sets block pixels to a color, or clears them (`null`): each pixel paints
 * its cell of the surface (on frame `frame` of a loop) or of a side map.
 * Painting adds a surface or sides, and keys for new colors, as painting a
 * sprite adds keys. Returns how many cells changed.
 * @param {any} doc  a block
 * @param {number} frame
 * @param {[number, number][]} pixels  relative to the first tile's top-face center
 * @param {string | null} color
 */
export function paintBlock(doc, frame, pixels, color) {
  const cells = blockCells(doc.size);
  let changed = 0;
  for (const [x, y] of pixels) {
    const cell = cells.get(`${x},${y}`);
    if (!cell) continue;
    const surface = cell.part === 'surface';
    if (color === null && !(surface ? doc.surface : doc.sides)) continue;
    // Created in canonical order: keys first.
    if (surface) doc.surface ??= { keys: {}, map: blankMap(TILE_W, TILE_H) };
    else doc.sides ??= { keys: {}, left: blankMap(SIDE_W, SIDE_H), right: blankMap(SIDE_W, SIDE_H) };
    const owner = surface ? doc.surface : doc.sides;
    const map = surface ? mapAt(owner, frame) : owner[cell.part];
    const ch = color === null ? '.' : keyFor(owner, owner.keys, color);
    const row = map[cell.row];
    if (row[cell.col] === ch) continue;
    map[cell.row] = row.slice(0, cell.col) + ch + row.slice(cell.col + 1);
    changed++;
  }
  return changed;
}

/**
 * Fill on a block: the 4-connected pixels of the start pixel's color on the
 * same part (the top, the left face or the right face).
 * @param {Map<string, BlockCell>} cells  from blockCells
 * @param {Map<string, string>} colors  what the block shows, "x,y" → color
 * @param {number} x
 * @param {number} y
 * @returns {[number, number][]}
 */
export function blockFill(cells, colors, x, y) {
  const start = cells.get(`${x},${y}`);
  if (!start) return [];
  const color = colors.get(`${x},${y}`);
  const seen = new Set([`${x},${y}`]);
  /** @type {[number, number][]} */
  const out = [];
  const stack = [[x, y]];
  while (stack.length) {
    const [px, py] = /** @type {number[]} */ (stack.pop());
    out.push([px, py]);
    for (const [nx, ny] of [[px + 1, py], [px - 1, py], [px, py + 1], [px, py - 1]]) {
      const p = `${nx},${ny}`;
      if (seen.has(p) || cells.get(p)?.part !== start.part || colors.get(p) !== color) continue;
      seen.add(p);
      stack.push([nx, ny]);
    }
  }
  return out;
}

/**
 * The 4-connected pixels that share the start pixel's key, for Fill.
 * @param {string[]} map
 * @param {number} x
 * @param {number} y
 * @returns {[number, number][]}
 */
export function floodFill(map, x, y) {
  if (y < 0 || y >= map.length || x < 0 || x >= map[y].length) return [];
  const target = map[y][x];
  const seen = new Set([`${x},${y}`]);
  /** @type {[number, number][]} */
  const out = [];
  const stack = [[x, y]];
  while (stack.length) {
    const [px, py] = /** @type {number[]} */ (stack.pop());
    out.push([px, py]);
    for (const [nx, ny] of [[px + 1, py], [px - 1, py], [px, py + 1], [px, py - 1]]) {
      if (ny < 0 || ny >= map.length || nx < 0 || nx >= map[ny].length || map[ny][nx] !== target || seen.has(`${nx},${ny}`)) continue;
      seen.add(`${nx},${ny}`);
      stack.push([nx, ny]);
    }
  }
  return out;
}

/**
 * The color at a pixel, for the Picker, or null if it's transparent.
 * @param {import('./engine.mjs').Resolved} resolved
 * @param {Where} where
 * @param {number} x
 * @param {number} y
 */
export function colorAt(resolved, { layer, frame }, x, y) {
  const source = resolved.layers[layer];
  const ch = source ? mapAt(source, frame)[y]?.[x] : undefined;
  return ch && ch !== '.' ? (resolved.keys[ch] ?? null) : null;
}

/**
 * The rows an object overrides, per layer, top to bottom.
 * @param {any} doc
 * @returns {Record<number, number[]>}
 */
export function overriddenRows(doc) {
  /** @type {Record<number, number[]>} */
  const out = {};
  for (const [layer, rows] of Object.entries(doc.rows ?? {})) out[Number(layer)] = Object.keys(rows).map(Number).sort((a, b) => a - b);
  return out;
}

/**
 * Pads or crops every map of a sprite with its own layers to `width` ×
 * `height`, at the right and the bottom.
 * @param {any} doc
 * @param {number} width
 * @param {number} height
 */
export function resize(doc, width, height) {
  const fit = (/** @type {string[]} */ map) =>
    Array.from({ length: height }, (_, y) => (map[y] ?? '').slice(0, width).padEnd(width, '.'));
  for (const layer of doc.layers) {
    if ('frames' in layer) layer.frames = layer.frames.map(fit);
    else layer.map = fit(layer.map);
  }
}

/**
 * A blank map the size of the object's others.
 * @param {any} doc
 */
const blank = (doc) => {
  const first = layerMaps(doc.layers[0])[0];
  return Array(first.length).fill('.'.repeat(first[0].length));
};

/** @param {any} doc */
export function addLayer(doc) {
  doc.layers.push({ map: blank(doc) });
  return doc.layers.length - 1;
}

/**
 * @param {any} doc
 * @param {number} index
 */
export function duplicateLayer(doc, index) {
  doc.layers.splice(index + 1, 0, JSON.parse(JSON.stringify(doc.layers[index])));
  return index + 1;
}

/**
 * Deletes a layer, unless it's the last one. Returns the layer to select.
 * @param {any} doc
 * @param {number} index
 */
export function deleteLayer(doc, index) {
  if (doc.layers.length <= 1) return index;
  doc.layers.splice(index, 1);
  return Math.min(index, doc.layers.length - 1);
}

/**
 * Moves an item in a list by `delta`; returns its new index.
 * @param {any[]} list
 * @param {number} index
 * @param {number} delta
 */
export function move(list, index, delta) {
  const target = index + delta;
  if (target < 0 || target >= list.length) return index;
  [list[index], list[target]] = [list[target], list[index]];
  return target;
}

/**
 * Adds a blank frame after `index` to a frame loop. Returns its index.
 * @param {any} doc
 * @param {number} layer
 * @param {number} index
 */
export function addFrame(doc, layer, index) {
  const frames = doc.layers[layer].frames;
  frames.splice(index + 1, 0, frames[index].map((/** @type {string} */ row) => '.'.repeat(row.length)));
  return index + 1;
}

/**
 * @param {any} doc
 * @param {number} layer
 * @param {number} index
 */
export function duplicateFrame(doc, layer, index) {
  const frames = doc.layers[layer].frames;
  frames.splice(index + 1, 0, [...frames[index]]);
  return index + 1;
}

/**
 * Deletes a frame, unless it's the last one. Returns the frame to show.
 * @param {any} doc
 * @param {number} layer
 * @param {number} index
 */
export function deleteFrame(doc, layer, index) {
  const frames = doc.layers[layer].frames;
  if (frames.length <= 1) return index;
  frames.splice(index, 1);
  return Math.min(index, frames.length - 1);
}

/**
 * Turns a still map layer into one of the site's frame loops (motion.mjs
 * SITE_LOOPS): its map becomes frame 0, the rest frame, and is copied into
 * every other frame the loop shows, ready to paint. A class on the layer
 * gives way to the loop's.
 * @param {any} doc
 * @param {number} layer
 * @param {string} loop
 */
export function animateLayer(doc, layer, loop) {
  const { prefix, frames } = SITE_LOOPS[loop];
  const { map } = doc.layers[layer];
  doc.layers[layer] = { loop, prefix, frames: Array.from({ length: frames }, () => [...map]) };
}

/**
 * Switches a frame loop to another of the site's loops: its prefix follows,
 * and it gains copies of its last frame until it has as many as the loop
 * shows. Extra frames stay, for the person to delete.
 * @param {any} doc
 * @param {number} layer
 * @param {string} loop
 */
export function setLoop(doc, layer, loop) {
  const target = doc.layers[layer];
  const { prefix, frames } = SITE_LOOPS[loop];
  const list = target.frames;
  while (list.length < frames) list.push([...list[list.length - 1]]);
  doc.layers[layer] = { loop, prefix, frames: list };
}

/**
 * Turns a frame loop back into a still map layer, keeping one frame.
 * @param {any} doc
 * @param {number} layer
 * @param {number} frame
 */
export function stillLayer(doc, layer, frame) {
  doc.layers[layer] = { map: [...doc.layers[layer].frames[frame]] };
}
