// @ts-check
// The pixel-art engine (pixel-art engine spec D2–D6). It takes the palette,
// objects and scenes as plain data, validates them, and renders scenes to
// layer trees. It uses no Node or DOM APIs, so `npm run art` and the editor
// run the same code.

import { blockFaces, blockSize, footprint, LEVEL_H, tilePixels, TILE_H, TILE_W, tileToPx } from './iso.mjs';
import { filledKeys, itemsById, loopFrame, motionClass, motionCss, poseClass, posesOf, validateMotion } from './motion.mjs';

/**
 * @typedef {{ class?: string, map: string[] }} MapLayer
 * @typedef {{ loop: string, prefix: string, frames: string[][] }} FrameLayer
 * @typedef {MapLayer | FrameLayer} SpriteLayer
 * @typedef {{ hex: string, tier: string }} PaletteColor
 * @typedef {{
 *   palette: any,
 *   colors: Map<string, PaletteColor>,
 *   objects: Map<string, any>,
 *   scenes: Map<string, any>,
 * }} Sources
 * @typedef {{ keys: Record<string, string>, left: string[], right: string[] }} Sides
 * @typedef {{ size: number[], faces: Record<string, string>, surface?: any, sides?: Sides }} Block
 * @typedef {{
 *   name: string,
 *   block?: Block,
 *   character: boolean,
 *   anchor: [number, number],
 *   keys: Record<string, string>,
 *   layers: SpriteLayer[],
 *   width: number,
 *   height: number,
 * }} Resolved
 * @typedef {{ pixels: Map<string, string>, colors: Set<string> }} Layer
 * @typedef {{ attrs: [string, string][], frame?: number, frames?: number, hidden?: boolean, children: (Group | Layer)[] }} Group
 */

export const TIERS = /** @type {const} */ (['world', 'outfit']);
export const CAPS = { world: 32, outfit: 16, size: 64, colors: 12, characterWidth: 24, characterHeight: 32 };

const NAME = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const CLASS_LIST = /^[a-z][a-z0-9-]*(?: [a-z][a-z0-9-]*)*$/;
const CLASS_TOKEN = /^[a-z][a-z0-9-]*$/;
const HEX = /^#[0-9A-F]{6}$/;
const OUTPUT = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*\.svg$/;
const DATA_ATTR = /^data-[a-z][a-z0-9-]*$/;

/** Code-point order, never locale order, so output never depends on the machine. */
export const byCodePoint = (/** @type {string} */ a, /** @type {string} */ b) => (a < b ? -1 : a > b ? 1 : 0);

const isRecord = (/** @type {unknown} */ v) => typeof v === 'object' && v !== null && !Array.isArray(v);
const isWhole = (/** @type {unknown} */ v, /** @type {number} */ n) =>
  Array.isArray(v) && v.length === n && v.every((x) => Number.isInteger(x));
/** A map key: one printable ASCII character other than space and '.'. */
export const isKey = (/** @type {string} */ k) => k.length === 1 && k > ' ' && k <= '~' && k !== '.';
export const isName = (/** @type {string} */ name) => NAME.test(name);
const q = (/** @type {unknown} */ v) => `'${String(v)}'`;
const isFrames = (/** @type {any} */ layer) => isRecord(layer) && 'frames' in layer;

/**
 * Wraps the three source kinds for the rest of the engine. Objects and scenes
 * are kept in code-point order by name.
 * @param {{ palette: any, objects?: Record<string, any>, scenes?: Record<string, any> }} input
 * @returns {Sources}
 */
export function loadSources({ palette, objects = {}, scenes = {} }) {
  /** @type {Map<string, PaletteColor>} */
  const colors = new Map();
  for (const tier of TIERS) {
    const entries = isRecord(palette?.[tier]) ? Object.entries(palette[tier]) : [];
    for (const [name, hex] of entries) if (!colors.has(name)) colors.set(name, { hex: String(hex), tier });
  }
  /** @param {Record<string, any>} record */
  const sorted = (record) => new Map(Object.keys(record).sort(byCodePoint).map((k) => [k, record[k]]));
  return { palette, colors, objects: sorted(objects), scenes: sorted(scenes) };
}

export const objectFile = (/** @type {string} */ name) => `objects/${name}.mjs`;
export const sceneFile = (/** @type {string} */ name) => `scenes/${name}.mjs`;

// ---------------------------------------------------------------- validation

/**
 * Every problem in the sources, as "<file>: <where>: <rule>" lines (R30).
 * It never throws, and it doesn't stop at the first problem.
 * @param {Sources} sources
 * @returns {string[]}
 */
export function validate(sources) {
  /** @type {string[]} */
  const problems = [];
  validatePalette(sources, problems);
  /** @type {Set<string>} */
  const broken = new Set();
  for (const name of sources.objects.keys()) {
    const before = problems.length;
    validateObjectShape(sources, name, problems);
    if (problems.length > before) broken.add(name);
  }
  for (const name of sources.objects.keys()) {
    if (broken.has(name)) continue;
    if (sources.objects.get(name).kind === 'block') validateBlockResolved(sources, name, problems);
    else validateResolved(sources, name, broken, problems);
  }
  /** @type {Map<string, string>} */
  const outputs = new Map();
  for (const name of sources.scenes.keys()) validateScene(sources, name, outputs, problems);
  return problems;
}

/**
 * @param {Sources} sources
 * @param {string[]} problems
 */
function validatePalette({ palette }, problems) {
  const file = 'palette.mjs';
  if (!isRecord(palette)) {
    problems.push(`${file}: must export an object with world and outfit`);
    return;
  }
  for (const key of Object.keys(palette)) {
    if (!(/** @type {readonly string[]} */ (TIERS).includes(key))) problems.push(`${file}: unknown tier ${q(key)}`);
  }
  /** @type {Map<string, string>} */
  const seen = new Map();
  for (const tier of TIERS) {
    const colors = palette[tier];
    if (!isRecord(colors)) {
      problems.push(`${file}: ${tier} must be an object of name: '#RRGGBB'`);
      continue;
    }
    const names = Object.keys(colors);
    if (names.length > CAPS[tier]) problems.push(`${file}: ${tier} has ${names.length} colors, max ${CAPS[tier]}`);
    for (const name of names) {
      if (!NAME.test(name)) problems.push(`${file}: ${tier} ${q(name)}: names are lowercase kebab-case`);
      if (typeof colors[name] !== 'string' || !HEX.test(colors[name])) {
        problems.push(`${file}: ${tier} ${q(name)}: ${q(colors[name])} must be uppercase #RRGGBB, with no alpha`);
      }
      const other = seen.get(name);
      if (other) problems.push(`${file}: ${q(name)} is in both ${other} and ${tier}; names must be unique`);
      else seen.set(name, tier);
    }
  }
}

const OBJECT_PROPS = ['kind', 'extends', 'anchor', 'keys', 'rows', 'layers'];
const BLOCK_PROPS = ['kind', 'size', 'faces', 'surface', 'sides'];
const SIDES_PROPS = ['keys', 'left', 'right'];
/** A side map is one tile's face wide (its 16-pixel period along the face) and one level high. */
export const SIDE_W = TILE_W / 2;
export const SIDE_H = LEVEL_H;
const FACES = ['top', 'left', 'right', 'edge'];
const MAP_LAYER_PROPS = ['class', 'map'];
const FRAME_LAYER_PROPS = ['loop', 'prefix', 'frames'];

/**
 * Checks one object on its own: its fields, keys and maps.
 * @param {Sources} sources
 * @param {string} name
 * @param {string[]} problems
 */
function validateObjectShape(sources, name, problems) {
  const file = objectFile(name);
  const add = (/** @type {string} */ msg) => problems.push(`${file}: ${msg}`);
  const obj = sources.objects.get(name);
  if (!NAME.test(name)) add('names are lowercase kebab-case');
  if (!isRecord(obj)) return add('must export an object');
  if (obj.kind === 'block') {
    validateBlockShape(sources, obj, add);
    if ('sides' in obj) validateSides(sources, obj, add);
    return;
  }
  for (const key of Object.keys(obj)) if (!OBJECT_PROPS.includes(key)) add(`unknown property ${q(key)}`);
  if (obj.kind !== 'sprite') add(`kind must be 'sprite' or 'block', got ${q(obj.kind)}`);
  if ('anchor' in obj && !isWhole(obj.anchor, 2)) add('anchor must be two whole numbers');

  const keys = obj.keys ?? {};
  if ('keys' in obj && !isRecord(obj.keys)) add('keys must be an object of key: color name');
  else {
    for (const [key, color] of Object.entries(keys)) {
      if (!isKey(key)) add(`key ${q(key)}: keys are one printable character, not space or '.'`);
      if (typeof color !== 'string' || !sources.colors.has(color)) add(`key ${q(key)}: ${q(color)} is not in the palette`);
    }
  }

  if ('extends' in obj) {
    if (typeof obj.extends !== 'string' || !sources.objects.has(obj.extends)) add(`extends ${q(obj.extends)}, which does not exist`);
    else if (sources.objects.get(obj.extends)?.kind === 'block') add(`extends ${q(obj.extends)}, which is a block; only a sprite can be extended`);
    if ('layers' in obj) add('an object that extends another gives rows, not layers');
    const rowsShape = 'rows must be an object of layer: { row: string }';
    if (!('rows' in obj)) return;
    if (!isRecord(obj.rows)) return add(rowsShape);
    for (const [li, rows] of Object.entries(obj.rows)) {
      if (!isRecord(rows) || !Object.values(rows).every((row) => typeof row === 'string')) add(`layer ${li} (override): ${rowsShape}`);
    }
    return;
  }
  if ('rows' in obj) add('rows is only for an object that extends another');
  if (!Array.isArray(obj.layers) || obj.layers.length === 0) return add('layers must be a list of at least one layer');

  /** @type {{ at: string, map: string[] }[]} */
  const maps = [];
  obj.layers.forEach((/** @type {any} */ layer, /** @type {number} */ li) => {
    const where = `layer ${li}`;
    if (!isRecord(layer)) return add(`${where}: must be an object with a map, or a loop, prefix and frames`);
    const allowed = isFrames(layer) ? FRAME_LAYER_PROPS : MAP_LAYER_PROPS;
    for (const key of Object.keys(layer)) if (!allowed.includes(key)) add(`${where}: unknown property ${q(key)}`);
    /** @type {any[]} */
    let layerMaps;
    if (isFrames(layer)) {
      if (typeof layer.loop !== 'string' || !CLASS_TOKEN.test(layer.loop)) add(`${where}: loop must be one lowercase class name`);
      if (typeof layer.prefix !== 'string' || !CLASS_TOKEN.test(layer.prefix)) add(`${where}: prefix must be one lowercase class name`);
      if (!Array.isArray(layer.frames) || layer.frames.length === 0) return add(`${where}: frames must be a list of at least one map`);
      layerMaps = layer.frames;
    } else {
      if ('class' in layer && (typeof layer.class !== 'string' || !CLASS_LIST.test(layer.class))) {
        add(`${where}: class must be lowercase class names separated by single spaces`);
      }
      layerMaps = [layer.map];
    }
    layerMaps.forEach((map, fi) => {
      const at = isFrames(layer) ? `${where}, frame ${fi}` : where;
      if (!Array.isArray(map) || map.length === 0 || !map.every((row) => typeof row === 'string')) {
        return add(`${at}: map must be a list of at least one string`);
      }
      maps.push({ at, map });
    });
  });
  if (maps.length === 0) return;

  // Every map shares one size: the most common one, so the message points
  // at the row or map that's off, even when it's the first.
  const width = mostCommon(maps.flatMap(({ map }) => map.map((row) => row.length)));
  const height = mostCommon(maps.map(({ map }) => map.length));
  if (width === 0) add('maps must be at least 1 wide');
  for (const { at, map } of maps) {
    if (map.length !== height) add(`${at}: ${map.length} rows, expected ${height}`);
    map.forEach((row, ri) => {
      if (row.length !== width) add(`${at}, row ${ri}: ${row.length} wide, expected ${width}`);
      checkRow(row, keys, (col, msg) => add(`${at}, row ${ri}, column ${col}: ${msg}`));
    });
  }
}


/**
 * Checks a block on its own: its size, faces and surface (spec D3).
 * @param {Sources} sources
 * @param {any} obj
 * @param {(msg: string) => void} add
 */
function validateBlockShape(sources, obj, add) {
  for (const key of Object.keys(obj)) if (!BLOCK_PROPS.includes(key)) add(`unknown property ${q(key)}`);
  const sized = isWhole(obj.size, 3) && obj.size[0] >= 1 && obj.size[1] >= 1 && obj.size[2] >= 0;
  if (!sized) add('size must be three whole numbers: tiles wide, tiles deep (both at least 1) and levels high (0 is a flat tile)');
  if (!isRecord(obj.faces)) add('faces must be an object of face: color name');
  else {
    for (const [face, color] of Object.entries(obj.faces)) {
      if (!FACES.includes(face)) add(`faces: unknown face ${q(face)}; use top, left, right or edge`);
      else if (typeof color !== 'string' || !sources.colors.has(color)) add(`faces.${face}: ${q(color)} is not in the palette`);
    }
    if (!('top' in obj.faces)) add('faces.top is missing');
    if (sized) {
      const flat = obj.size[2] === 0;
      for (const face of ['left', 'right']) {
        if (!flat && !(face in obj.faces)) add(`faces.${face} is missing; a block with levels needs a left and a right face`);
        if (flat && face in obj.faces) add(`faces.${face}: a flat block (0 levels) has no ${face} face`);
      }
    }
  }
  if (!('surface' in obj)) return;
  const surface = obj.surface;
  const where = 'surface';
  if (!isRecord(surface)) return add(`${where}: must be { keys, map } or { keys, loop, prefix, frames }`);
  const looped = 'frames' in surface;
  for (const key of Object.keys(surface)) {
    if (!['keys', ...(looped ? FRAME_LAYER_PROPS : ['map'])].includes(key)) add(`${where}: unknown property ${q(key)}`);
  }
  const keys = surface.keys;
  if (!isRecord(keys)) add(`${where}: keys must be an object of key: color name`);
  else {
    for (const [key, color] of Object.entries(keys)) {
      if (!isKey(key)) add(`${where}, key ${q(key)}: keys are one printable character, not space or '.'`);
      if (typeof color !== 'string' || !sources.colors.has(color)) add(`${where}, key ${q(key)}: ${q(color)} is not in the palette`);
    }
  }
  /** @type {any[]} */
  let maps;
  if (looped) {
    if (typeof surface.loop !== 'string' || !CLASS_TOKEN.test(surface.loop)) add(`${where}: loop must be one lowercase class name`);
    if (typeof surface.prefix !== 'string' || !CLASS_TOKEN.test(surface.prefix)) add(`${where}: prefix must be one lowercase class name`);
    if (!Array.isArray(surface.frames) || surface.frames.length === 0) return add(`${where}: frames must be a list of at least one map`);
    maps = surface.frames;
  } else maps = [surface.map];
  maps.forEach((map, fi) => {
    const at = looped ? `${where}, frame ${fi}` : where;
    if (!Array.isArray(map) || !map.every((row) => typeof row === 'string')) return add(`${at}: map must be a list of strings`);
    if (map.length !== TILE_H) add(`${at}: ${map.length} rows, expected ${TILE_H}, one tile`);
    map.forEach((row, ri) => {
      if (row.length !== TILE_W) add(`${at}, row ${ri}: ${row.length} wide, expected ${TILE_W}, one tile`);
      checkRow(row, isRecord(keys) ? keys : {}, (col, msg) => add(`${at}, row ${ri}, column ${col}: ${msg}`));
    });
  });
}

/**
 * Checks a block's side textures (hero island detail spec R6): { keys, left,
 * right }, each map 16 columns by 16 rows, on a block with levels.
 * @param {Sources} sources
 * @param {any} obj
 * @param {(msg: string) => void} add
 */
function validateSides(sources, obj, add) {
  const { sides } = obj;
  if (!isRecord(sides)) return add('sides: must be { keys, left, right }');
  if (isWhole(obj.size, 3) && obj.size[2] === 0) add('sides: a flat block (0 levels) has no sides');
  for (const key of Object.keys(sides)) if (!SIDES_PROPS.includes(key)) add(`sides: unknown property ${q(key)}`);
  const keys = sides.keys;
  if (!isRecord(keys)) add('sides: keys must be an object of key: color name');
  else {
    for (const [key, color] of Object.entries(keys)) {
      if (!isKey(key)) add(`sides, key ${q(key)}: keys are one printable character, not space or '.'`);
      if (typeof color !== 'string' || !sources.colors.has(color)) add(`sides, key ${q(key)}: ${q(color)} is not in the palette`);
    }
  }
  for (const face of ['left', 'right']) {
    const at = `sides.${face}`;
    if (!(face in sides)) {
      add(`${at} is missing`);
      continue;
    }
    const map = sides[face];
    if (!Array.isArray(map) || !map.every((row) => typeof row === 'string')) {
      add(`${at}: map must be a list of strings`);
      continue;
    }
    if (map.length !== SIDE_H) add(`${at}: ${map.length} rows, expected ${SIDE_H}, one level`);
    map.forEach((row, ri) => {
      if (row.length !== SIDE_W) add(`${at}, row ${ri}: ${row.length} wide, expected ${SIDE_W}, one tile's face`);
      checkRow(row, isRecord(keys) ? keys : {}, (col, msg) => add(`${at}, row ${ri}, column ${col}: ${msg}`));
    });
  }
}

/**
 * Checks a block against the design language: world colors only, at most 12
 * colors, at most 64×64 pixels (R26, R27).
 * @param {Sources} sources
 * @param {string} name
 * @param {string[]} problems
 */
function validateBlockResolved(sources, name, problems) {
  const add = (/** @type {string} */ msg) => problems.push(`${objectFile(name)}: ${msg}`);
  const resolved = resolve(sources, name);
  const block = /** @type {Block} */ (resolved.block);
  /** @type {[string, string][]} */
  const named = Object.entries(block.faces).map(([face, color]) => [`faces.${face}`, color]);
  const surfaceKeys = usedKeys(resolved);
  for (const key of surfaceKeys) named.push([`surface, key ${q(key)}`, resolved.keys[key]]);
  for (const key of usedSideKeys(block)) named.push([`sides, key ${q(key)}`, /** @type {Sides} */ (block.sides).keys[key]]);
  for (const [where, color] of named) {
    const tier = sources.colors.get(color)?.tier;
    if (tier === 'outfit') add(`${where}: ${q(color)} is an outfit color; only objects that extend character may use it`);
  }
  if (resolved.width > CAPS.size || resolved.height > CAPS.size) add(`${resolved.width}×${resolved.height}, max ${CAPS.size}×${CAPS.size}`);
  const colors = usedColors(sources, resolved).length;
  if (colors > CAPS.colors) add(`uses ${colors} colors, max ${CAPS.colors}`);
}

/**
 * The most common value, and the earliest one on a tie.
 * @param {number[]} values
 */
function mostCommon(values) {
  /** @type {Map<number, number>} */
  const counts = new Map();
  for (const v of values) counts.set(v, (counts.get(v) ?? 0) + 1);
  let best = values[0];
  for (const [v, n] of counts) if (n > /** @type {number} */ (counts.get(best))) best = v;
  return best;
}

/**
 * @param {string} row
 * @param {Record<string, string>} keys
 * @param {(col: number, msg: string) => void} report
 */
function checkRow(row, keys, report) {
  for (let x = 0; x < row.length; x++) {
    const ch = row[x];
    if (ch !== '.' && !Object.hasOwn(keys, ch)) report(x, `key ${q(ch)} is not in keys`);
  }
}

/**
 * Checks what needs the `extends` chain: row overrides, the colors used, and
 * the design-language caps (R26, R27).
 * @param {Sources} sources
 * @param {string} name
 * @param {Set<string>} broken
 * @param {string[]} problems
 */
function validateResolved(sources, name, broken, problems) {
  const file = objectFile(name);
  const add = (/** @type {string} */ msg) => problems.push(`${file}: ${msg}`);
  const chain = extendsChain(sources, name);
  if (chain.cycle) return add(`extends cycle: ${chain.names.join(' → ')}`);
  if (chain.names.slice(1).some((n) => broken.has(n))) return;

  const obj = sources.objects.get(name);
  let resolved;
  try {
    resolved = resolve(sources, name);
  } catch (err) {
    return add(/** @type {Error} */ (err).message);
  }
  if (obj.extends) {
    for (const [li, rows] of Object.entries(obj.rows ?? {})) {
      for (const [ri, row] of Object.entries(/** @type {any} */ (rows))) {
        if (row.length !== resolved.width) add(`layer ${li}, row ${ri} (override): ${row.length} wide, expected ${resolved.width}`);
        checkRow(row, resolved.keys, (col, msg) => add(`layer ${li}, row ${ri}, column ${col} (override): ${msg}`));
      }
    }
  }

  for (const key of usedKeys(resolved)) {
    const color = resolved.keys[key];
    if (sources.colors.get(color)?.tier === 'outfit' && !resolved.character) {
      add(`key ${q(key)}: ${q(color)} is an outfit color; only objects that extend character may use it`);
    }
  }
  const size = `${resolved.width}×${resolved.height}`;
  if (resolved.width > CAPS.size || resolved.height > CAPS.size) add(`${size}, max ${CAPS.size}×${CAPS.size}`);
  // A character's cap is on the figure it paints, since the shared canvas
  // is wider than the figure, so a held prop fits.
  const figure = paintedSize(resolved);
  if (resolved.character && (figure[0] > CAPS.characterWidth || figure[1] > CAPS.characterHeight)) {
    add(`the figure is ${figure[0]}×${figure[1]}; a character is at most ${CAPS.characterWidth}×${CAPS.characterHeight}`);
  }
  const colors = usedColors(sources, resolved).length;
  if (colors > CAPS.colors) add(`uses ${colors} colors, max ${CAPS.colors}`);
}

/**
 * @param {Sources} sources
 * @param {string} name
 * @returns {{ names: string[], cycle: boolean }}
 */
function extendsChain(sources, name) {
  const names = [name];
  let next = sources.objects.get(name)?.extends;
  while (typeof next === 'string' && sources.objects.has(next)) {
    names.push(next);
    if (names.indexOf(next) !== names.length - 1) return { names, cycle: true };
    next = sources.objects.get(next)?.extends;
  }
  return { names, cycle: false };
}

const SCENE_PROPS = ['output', 'viewBox', 'origin', 'items', 'motion'];

/**
 * @param {Sources} sources
 * @param {string} name
 * @param {Map<string, string>} outputs
 * @param {string[]} problems
 */
function validateScene(sources, name, outputs, problems) {
  const file = sceneFile(name);
  const add = (/** @type {string} */ msg) => problems.push(`${file}: ${msg}`);
  const scene = sources.scenes.get(name);
  if (!NAME.test(name)) add('names are lowercase kebab-case');
  if (!isRecord(scene)) return add('must export an object');
  for (const key of Object.keys(scene)) if (!SCENE_PROPS.includes(key)) add(`unknown property ${q(key)}`);
  if ('output' in scene) {
    if (typeof scene.output !== 'string' || !OUTPUT.test(scene.output)) add('output must be a kebab-case file name ending in .svg');
    else if (outputs.has(scene.output)) add(`output ${q(scene.output)} is also written by ${sceneFile(String(outputs.get(scene.output)))}`);
    else outputs.set(scene.output, name);
  }
  if (!isWhole(scene.viewBox, 4) || scene.viewBox[2] <= 0 || scene.viewBox[3] <= 0) {
    add('viewBox must be four whole numbers: x, y, width, height (width and height above 0)');
  }
  if ('origin' in scene && !isWhole(scene.origin, 2)) add('origin must be two whole numbers');
  if (!Array.isArray(scene.items)) return add('items must be a list');
  validateItems(sources, scene.items, 'items', add, new Set());
  if ('motion' in scene) validateMotion(sources, scene, add);
}

/**
 * @param {Sources} sources
 * @param {any[]} items
 * @param {string} path
 * @param {(msg: string) => void} add
 * @param {Set<string>} ids  the item ids seen so far in the scene
 */
function validateItems(sources, items, path, add, ids) {
  items.forEach((item, i) => {
    const where = `${path}[${i}]`;
    if (!isRecord(item)) return add(`${where}: must be { object, at } or { group, items }`);
    if ('group' in item) {
      for (const key of Object.keys(item)) if (key !== 'group' && key !== 'items') add(`${where}: unknown property ${q(key)}`);
      if (!isRecord(item.group)) add(`${where}: group must be an object of attributes`);
      else {
        for (const [attr, value] of Object.entries(item.group)) {
          if (attr !== 'class' && !DATA_ATTR.test(attr)) add(`${where}: group attribute ${q(attr)} must be class or data-*`);
          if (typeof value !== 'string' || /["<>&]/.test(value)) add(`${where}: group attribute ${q(attr)} must be a string without " < > &`);
          if (attr === 'class' && typeof value === 'string' && !CLASS_LIST.test(value)) {
            add(`${where}: class must be lowercase class names separated by single spaces`);
          }
        }
      }
      if (!Array.isArray(item.items)) return add(`${where}: items must be a list`);
      return validateItems(sources, item.items, `${where}.items`, add, ids);
    }
    for (const key of Object.keys(item)) if (!['object', 'id', 'class', 'at'].includes(key)) add(`${where}: unknown property ${q(key)}`);
    if ('id' in item) {
      if (typeof item.id !== 'string' || !NAME.test(item.id)) add(`${where}: id must be lowercase kebab-case`);
      else if (ids.has(item.id)) add(`${where}: id ${q(item.id)} is already used in this scene`);
      else ids.add(item.id);
    }
    if (typeof item.object !== 'string' || !sources.objects.has(item.object)) add(`${where}: object ${q(item.object)} does not exist`);
    if ('class' in item && (typeof item.class !== 'string' || !CLASS_LIST.test(item.class))) {
      add(`${where}: class must be lowercase class names separated by single spaces`);
    }
    const at = item.at;
    if (!isRecord(at) || Object.keys(at).length !== 1 || !('px' in at || 'tile' in at)) {
      add(`${where}: at must be { px: [x, y] } or { tile: [col, row, level] }`);
    } else if ('px' in at && !isWhole(at.px, 2)) add(`${where}: at.px must be two whole numbers`);
    else if ('tile' in at && !isWhole(at.tile, 3)) add(`${where}: at.tile must be three whole numbers`);
  });
}

// ------------------------------------------------------------------- resolve

/**
 * Applies `extends`: the base's layers with whole rows replaced, then the
 * keys merged (the base's order first, new keys after). Throws on a missing
 * object or a cycle; run validate() first for friendly messages.
 * @param {Sources} sources
 * @param {string} name
 * @param {string[]} [seen]
 * @returns {Resolved}
 */
export function resolve(sources, name, seen = []) {
  const obj = sources.objects.get(name);
  if (!obj) throw new Error(`object ${q(name)} does not exist`);
  if (seen.includes(name)) throw new Error(`extends cycle: ${[...seen, name].join(' → ')}`);
  if (obj.kind === 'block') return resolveBlock(name, obj);
  /** @type {[number, number] | undefined} */
  const anchor = obj.anchor ? [obj.anchor[0], obj.anchor[1]] : undefined;
  if (!obj.extends) {
    /** @type {SpriteLayer[]} */
    const layers = obj.layers.map(copyLayer);
    const first = firstMap(layers[0]);
    return {
      name,
      character: name === 'character',
      anchor: anchor ?? [0, 0],
      keys: { ...obj.keys },
      layers,
      width: first[0].length,
      height: first.length,
    };
  }
  const base = resolve(sources, obj.extends, [...seen, name]);
  const layers = base.layers.map(copyLayer);
  for (const [li, rows] of Object.entries(obj.rows ?? {})) {
    const layer = layers[Number(li)];
    if (!layer || isFrames(layer)) throw new Error(`layer ${li} (override): ${obj.extends} has no map layer ${li}`);
    for (const [ri, row] of Object.entries(/** @type {Record<string, string>} */ (rows))) {
      const map = /** @type {MapLayer} */ (layer).map;
      if (!Number.isInteger(Number(ri)) || Number(ri) < 0 || Number(ri) >= map.length) {
        throw new Error(`layer ${li}, row ${ri} (override): ${obj.extends} has rows 0 to ${map.length - 1}`);
      }
      map[Number(ri)] = row;
    }
  }
  return {
    name,
    character: name === 'character' || base.character,
    anchor: anchor ?? base.anchor,
    keys: { ...base.keys, ...obj.keys },
    layers,
    width: base.width,
    height: base.height,
  };
}

/**
 * A block as a Resolved: its surface is the one layer, and its size is the
 * silhouette's (worked out from the numbers, so a huge `size` is an error
 * and not a crash), so the caps and `--check` treat it like any object.
 * @param {string} name
 * @param {any} obj
 * @returns {Resolved}
 */
function resolveBlock(name, obj) {
  const { surface, sides } = obj;
  /** @type {SpriteLayer[]} */
  const layers = surface ? [isFrames(surface) ? { loop: surface.loop, prefix: surface.prefix, frames: surface.frames } : { map: surface.map }] : [];
  const [width, height] = blockSize(obj.size);
  return {
    name,
    block: { size: obj.size, faces: { ...obj.faces }, surface, ...(sides ? { sides } : {}) },
    character: false,
    anchor: [0, 0],
    keys: { ...surface?.keys },
    layers,
    width,
    height,
  };
}

/** @param {SpriteLayer} layer @returns {SpriteLayer} */
const copyLayer = (layer) =>
  isFrames(layer)
    ? { .../** @type {FrameLayer} */ (layer), frames: /** @type {FrameLayer} */ (layer).frames.map((m) => [...m]) }
    : { .../** @type {MapLayer} */ (layer), map: [.../** @type {MapLayer} */ (layer).map] };

/** @param {SpriteLayer} layer */
const firstMap = (layer) => (isFrames(layer) ? /** @type {FrameLayer} */ (layer).frames[0] : /** @type {MapLayer} */ (layer).map);

/** @param {SpriteLayer} layer */
export const layerMaps = (layer) => (isFrames(layer) ? /** @type {FrameLayer} */ (layer).frames : [/** @type {MapLayer} */ (layer).map]);

/**
 * The keys that some map actually uses, in key order.
 * @param {Resolved} resolved
 */
export function usedKeys(resolved) {
  const used = new Set();
  for (const layer of resolved.layers) for (const map of layerMaps(layer)) for (const row of map) for (const ch of row) used.add(ch);
  return Object.keys(resolved.keys).filter((k) => used.has(k));
}

/**
 * The keys that a block's side maps actually use, in key order.
 * @param {Block} block
 */
export function usedSideKeys({ sides }) {
  if (!sides) return [];
  const used = new Set([...sides.left, ...sides.right].flatMap((row) => [...row]));
  return Object.keys(sides.keys).filter((k) => used.has(k));
}

/**
 * The width and height of the pixels an object paints, across every layer
 * and frame.
 * @param {Resolved} resolved
 * @returns {[number, number]}
 */
export function paintedSize(resolved) {
  let [minX, minY, maxX, maxY] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const layer of resolved.layers) {
    for (const map of layerMaps(layer)) {
      map.forEach((row, y) => {
        for (let x = 0; x < row.length; x++) {
          if (row[x] === '.') continue;
          [minX, minY, maxX, maxY] = [Math.min(minX, x), Math.min(minY, y), Math.max(maxX, x), Math.max(maxY, y)];
        }
      });
    }
  }
  return minX === Infinity ? [0, 0] : [maxX - minX + 1, maxY - minY + 1];
}

/**
 * The distinct palette colors (by value) that a resolved object paints.
 * @param {Sources} sources
 * @param {Resolved} resolved
 */
export function usedColors(sources, resolved) {
  const { block } = resolved;
  const sideColors = block?.sides ? usedSideKeys(block).map((k) => /** @type {Sides} */ (block.sides).keys[k]) : [];
  const names = [...Object.values(block?.faces ?? {}), ...sideColors, ...usedKeys(resolved).map((k) => resolved.keys[k])];
  return [...new Set(names.map((n) => sources.colors.get(n)?.hex ?? n))];
}

/**
 * The one-line summary `--check` prints for an object.
 * @param {Sources} sources
 * @param {Resolved} resolved
 */
export function describeObject(sources, resolved) {
  const colors = usedColors(sources, resolved).length;
  const frames = Math.max(1, ...resolved.layers.map((l) => layerMaps(l).length));
  const plural = (/** @type {number} */ n, /** @type {string} */ word) => `${n} ${word}${n === 1 ? '' : 's'}`;
  const cap = `of ${CAPS.colors}`;
  if (resolved.block) {
    const [w, d, h] = resolved.block.size;
    return [`block ${w}×${d}×${h}`, `${resolved.width}×${resolved.height}`, `${plural(colors, 'color')} (${cap})`, plural(frames, 'frame')].join(' · ');
  }
  return [
    `${resolved.width}×${resolved.height}`,
    `${plural(colors, 'color')} (${cap})`,
    plural(resolved.layers.length, 'layer'),
    plural(frames, 'frame'),
  ].join(' · ');
}

// -------------------------------------------------------------------- render

/** @returns {Layer} */
const newLayer = () => ({ pixels: new Map(), colors: new Set() });

/**
 * The layer that unclassed paint goes into: the group's last child if it's
 * a layer, so consecutive paint shares one layer and later paint wins.
 * @param {Group} group
 */
function openLayer(group) {
  const last = group.children[group.children.length - 1];
  if (last && 'pixels' in last) return last;
  const layer = newLayer();
  group.children.push(layer);
  return layer;
}

/**
 * Paints a map key by key, in key order, so a layer's colors are first
 * painted in the order the object lists its keys.
 * @param {Layer} layer
 * @param {string[]} map
 * @param {Record<string, string>} keys
 * @param {number} ox
 * @param {number} oy
 * @param {Set<string>} [clip]  paint only these pixels
 */
function paint(layer, map, keys, ox, oy, clip) {
  /** @type {Map<string, string[]>} */
  const byKey = new Map();
  map.forEach((row, y) => {
    for (let x = 0; x < row.length; x++) {
      const ch = row[x];
      if (ch === '.' || (clip && !clip.has(`${ox + x},${oy + y}`))) continue;
      let list = byKey.get(ch);
      if (!list) byKey.set(ch, (list = []));
      list.push(`${ox + x},${oy + y}`);
    }
  });
  layByKey(layer, byKey, keys);
}

/**
 * Lays pixels grouped by key onto a layer, in key order.
 * @param {Layer} layer
 * @param {Map<string, string[]>} byKey
 * @param {Record<string, string>} keys
 */
function layByKey(layer, byKey, keys) {
  for (const key of Object.keys(keys)) {
    const pixels = byKey.get(key);
    if (!pixels) continue;
    layer.colors.add(keys[key]);
    for (const p of pixels) layer.pixels.set(p, keys[key]);
  }
}

/**
 * Where each pixel of a side face falls in its side map. Column c of the face
 * (from its leftmost x) reads map column c mod 16. Row r below that column's
 * first face pixel, which is one pixel below the top's edge, reads map row
 * r mod 16. So the map is sheared onto the face, follows the top edge, and
 * repeats along the face and up every level.
 * @param {string[]} face  the face's pixels, from blockFaces
 * @returns {{ p: string, col: number, row: number }[]}
 */
export function sideCells(face) {
  const points = face.map((p) => p.split(',').map(Number));
  const x0 = Math.min(...points.map(([x]) => x));
  /** @type {Map<number, number>} */
  const first = new Map();
  for (const [x, y] of points) first.set(x, Math.min(first.get(x) ?? Infinity, y));
  return points.map(([x, y], i) => ({ p: face[i], col: (x - x0) % SIDE_W, row: (y - /** @type {number} */ (first.get(x))) % SIDE_H }));
}

/**
 * Draws a block with its first tile's top-face center at (cx, cy): the three
 * faces, then the side textures, then the surface on every tile's top, then
 * the edge over everything.
 * @param {Block} block
 * @param {Record<string, string>} keys  the surface's keys
 * @param {number} cx
 * @param {number} cy
 * @param {Group} group
 */
function drawBlock({ size, faces, surface, sides }, keys, cx, cy, group) {
  const shapes = blockFaces(size, [cx, cy]);
  const base = openLayer(group);
  for (const face of /** @type {const} */ (['top', 'left', 'right'])) {
    if (!faces[face]) continue;
    base.colors.add(faces[face]);
    for (const p of shapes[face]) base.pixels.set(p, faces[face]);
  }
  if (sides) {
    /** @type {Map<string, string[]>} */
    const byKey = new Map();
    for (const face of /** @type {const} */ (['left', 'right'])) {
      for (const { p, col, row } of sideCells(shapes[face])) {
        const ch = sides[face][row][col];
        if (ch === '.') continue;
        let list = byKey.get(ch);
        if (!list) byKey.set(ch, (list = []));
        list.push(p);
      }
    }
    layByKey(base, byKey, sides.keys);
  }
  if (surface) {
    /** @param {Layer} layer @param {string[]} map */
    const lay = (layer, map) => {
      // Each tile's map is clipped to that tile's own diamond.
      for (const [tx, ty] of footprint(size, [cx, cy])) {
        paint(layer, map, keys, tx - TILE_W / 2, ty - TILE_H / 2, new Set(tilePixels(tx, ty)));
      }
    };
    if (isFrames(surface)) {
      surface.frames.forEach((/** @type {string[]} */ map, /** @type {number} */ i) => {
        const layerOut = newLayer();
        group.children.push({ attrs: [['class', `${surface.loop} ${surface.prefix}${i}`]], frame: i, frames: surface.frames.length, children: [layerOut] });
        lay(layerOut, map);
      });
    } else lay(openLayer(group), surface.map);
  }
  if (faces.edge) {
    const edge = openLayer(group);
    edge.colors.add(faces.edge);
    for (const p of shapes.edge) edge.pixels.set(p, faces.edge);
  }
}

/**
 * Draws a resolved object with its map's top-left pixel at (ox, oy).
 * @param {Resolved} obj
 * @param {number} ox
 * @param {number} oy
 * @param {Group} group
 */
function draw(obj, ox, oy, group) {
  if (obj.block) return drawBlock(obj.block, obj.keys, ox, oy, group);
  for (const layer of obj.layers) {
    if (isFrames(layer)) {
      const { loop, prefix, frames } = /** @type {FrameLayer} */ (layer);
      frames.forEach((map, i) => {
        const layerOut = newLayer();
        group.children.push({ attrs: [['class', `${loop} ${prefix}${i}`]], frame: i, frames: frames.length, children: [layerOut] });
        paint(layerOut, map, obj.keys, ox, oy);
      });
    } else if (/** @type {MapLayer} */ (layer).class) {
      const layerOut = newLayer();
      group.children.push({ attrs: [['class', String(/** @type {MapLayer} */ (layer).class)]], children: [layerOut] });
      paint(layerOut, /** @type {MapLayer} */ (layer).map, obj.keys, ox, oy);
    } else {
      paint(openLayer(group), /** @type {MapLayer} */ (layer).map, obj.keys, ox, oy);
    }
  }
}

/**
 * Renders a scene to a tree that mirrors its groups, placement classes and
 * frame loops. Items paint in source order.
 * @param {Sources} sources
 * @param {string} name
 * A scene with motion also returns the CSS that plays it, and each tracked
 * item is drawn at its rest frame in its own group (motion.mjs).
 * @returns {{ viewBox: number[], root: Group, style?: string }}
 */
export function renderScene(sources, name) {
  const scene = sources.scenes.get(name);
  if (!scene) throw new Error(`scene ${q(name)} does not exist`);
  /** @type {Group} */
  const root = { attrs: [], children: [] };
  const tracks = new Map((scene.motion?.tracks ?? []).map((/** @type {any} */ track) => [track.item, track]));
  placeItems(sources, scene.items, root, scene.origin ?? [0, 0], tracks);
  if (!tracks.size) return { viewBox: scene.viewBox, root };
  return { viewBox: scene.viewBox, root, style: motionCss(scene.motion, itemsById(scene.items)) };
}

/**
 * @param {Sources} sources
 * @param {any[]} items
 * @param {Group} group
 * @param {number[]} origin
 * @param {Map<string, any>} tracks  motion tracks by item id
 */
function placeItems(sources, items, group, origin, tracks) {
  for (const item of items) {
    if ('group' in item) {
      /** @type {Group} */
      const child = { attrs: Object.entries(item.group), children: [] };
      group.children.push(child);
      placeItems(sources, item.items, child, origin, tracks);
      continue;
    }
    let [x, y] = item.at.tile ? tileToPx(item.at.tile, origin) : item.at.px;
    let target = group;
    const track = item.id !== undefined ? tracks.get(item.id) : undefined;
    /** @type {[string, Group][]} */
    let poses = [[item.object, group]];
    if (track) {
      const rest = filledKeys(track, item.object)[0];
      [x, y] = [x + rest.move[0], y + rest.move[1]];
      /** @type {Group} */
      const moving = { attrs: [['class', motionClass(item.id)]], children: [], ...(rest.show ? {} : { hidden: true }) };
      group.children.push(moving);
      const names = posesOf(track, item.object);
      poses =
        names.length === 1
          ? [[item.object, moving]]
          : names.map((pose, k) => {
              /** @type {Group} */
              const posed = { attrs: [['class', poseClass(item.id, k)]], children: [], ...(pose === rest.pose ? {} : { hidden: true }) };
              moving.children.push(posed);
              return [pose, posed];
            });
    }
    for (const [name, parent] of poses) {
      const obj = resolve(sources, name);
      target = parent;
      if (item.class) {
        target = { attrs: [['class', item.class]], children: [] };
        parent.children.push(target);
      }
      draw(obj, x - obj.anchor[0], y - obj.anchor[1], target);
    }
  }
}

/**
 * Renders one object on its own, with its anchor at (0, 0).
 * @param {Sources} sources
 * @param {string} name
 * @returns {Group}
 */
export function renderObject(sources, name) {
  const obj = resolve(sources, name);
  /** @type {Group} */
  const root = { attrs: [], children: [] };
  draw(obj, -obj.anchor[0], -obj.anchor[1], root);
  return root;
}

/**
 * Flattens a tree to what shows with every group visible and each loop on
 * frame 0 (what reduced motion shows), or on `frame` (the editor's frame,
 * for a loop that long). Later paint wins. With `firstVariant`, only the
 * first of each run of sibling data-class groups draws, as the Range shows
 * one variant at a time.
 * @param {Group} root
 * With `time` (ms), each loop shows the frame it shows then instead, as the
 * editor plays a scene. Hidden groups (a moving item while it's away, a pose
 * not in use) never draw.
 * @param {{ firstVariant?: boolean, frame?: number, time?: number }} [options]
 * @returns {Map<string, string>} "x,y" → color name
 */
export function composite(root, { firstVariant = false, frame = 0, time } = {}) {
  /** @type {Map<string, string>} */
  const out = new Map();
  const isVariant = (/** @type {Group | Layer} */ c) => 'attrs' in c && c.attrs.some(([k]) => k === 'data-class');
  /** @param {Group} group */
  const walk = (group) => {
    group.children.forEach((child, i) => {
      if ('pixels' in child) for (const [p, color] of child.pixels) out.set(p, color);
      else if (child.hidden) return;
      else if (child.frame !== undefined && child.frame !== (time === undefined ? frame : loopFrame(loopOf(child), child.frames ?? 1, time))) return;
      else if (firstVariant && isVariant(child) && i > 0 && isVariant(group.children[i - 1])) return;
      else walk(child);
    });
  };
  walk(root);
  return out;
}

/** A frame group's loop name, the first of its classes. */
const loopOf = (/** @type {Group} */ group) => (group.attrs.find(([k]) => k === 'class')?.[1] ?? '').split(' ')[0];

/**
 * The bounding box of a set of "x,y" pixels, or null if it's empty.
 * @param {Iterable<string>} pixels
 */
export function bounds(pixels) {
  let [minX, minY, maxX, maxY] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const p of pixels) {
    const [x, y] = p.split(',').map(Number);
    minX = Math.min(minX, x);
    minY = Math.min(minY, y);
    maxX = Math.max(maxX, x);
    maxY = Math.max(maxY, y);
  }
  return minX === Infinity ? null : { minX, minY, maxX, maxY };
}
