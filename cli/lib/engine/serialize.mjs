// @ts-check
// Canonical source text for objects and scenes (pixel-art engine spec R9).
// One header comment, `export default {`, two-space indentation, keys in a
// fixed order, single-quoted strings, one map row per line, trailing commas
// and a final newline. Every committed source equals serialize() of itself,
// so hand edits and editor exports give small, comparable diffs.

export const OBJECT_HEADER = '// Pixel-art object. How to edit it: .claude/skills/pixel-art/SKILL.md';
export const SCENE_HEADER = '// Pixel-art scene. How to edit it: .claude/skills/pixel-art/SKILL.md';

const str = (/** @type {string} */ s) => `'${s.replace(/\\/g, '\\\\').replace(/'/g, "\\'")}'`;
const prop = (/** @type {string} */ k) => (/^[A-Za-z_$][\w$]*$/.test(k) || /^(?:0|[1-9]\d*)$/.test(k) ? k : str(k));
const nums = (/** @type {number[]} */ list) => `[${list.join(', ')}]`;
const pad = (/** @type {number} */ n) => '  '.repeat(n);

/**
 * A list of strings, one per line.
 * @param {string[]} rows
 * @param {number} depth  indentation of the opening line
 */
const stringList = (rows, depth) => `[\n${rows.map((r) => `${pad(depth + 1)}${str(r)},\n`).join('')}${pad(depth)}]`;

/**
 * An object of string values, one entry per line.
 * @param {Record<string, string>} record
 * @param {number} depth
 */
function stringRecord(record, depth) {
  const keys = Object.keys(record);
  if (keys.length === 0) return '{}';
  return `{\n${keys.map((k) => `${pad(depth + 1)}${prop(k)}: ${str(record[k])},\n`).join('')}${pad(depth)}}`;
}

/**
 * A layer's properties, one per entry, for an object whose lines sit at `depth` + 1.
 * @param {any} layer
 * @param {number} depth
 * @returns {string[]}
 */
function layerLines(layer, depth) {
  const lines = [];
  if ('frames' in layer) {
    lines.push(`loop: ${str(layer.loop)}`, `prefix: ${str(layer.prefix)}`);
    const frames = layer.frames.map((/** @type {string[]} */ map) => `${pad(depth + 2)}${stringList(map, depth + 2)},\n`).join('');
    lines.push(`frames: [\n${frames}${pad(depth + 1)}]`);
  } else {
    if (layer.class !== undefined) lines.push(`class: ${str(layer.class)}`);
    lines.push(`map: ${stringList(layer.map, depth + 1)}`);
  }
  return lines;
}

/**
 * @param {any} layer
 * @param {number} depth
 */
const layerText = (layer, depth) => `{\n${layerLines(layer, depth).map((l) => `${pad(depth + 1)}${l},\n`).join('')}${pad(depth)}}`;

/**
 * @param {any} obj  a sprite object
 * @returns {string}
 */
export function serializeObject(obj) {
  const lines = [`kind: ${str(obj.kind)}`];
  if (obj.kind === 'block') return serializeBlock(obj, lines);
  if (obj.extends !== undefined) lines.push(`extends: ${str(obj.extends)}`);
  if (obj.anchor !== undefined) lines.push(`anchor: ${nums(obj.anchor)}`);
  if (obj.keys !== undefined) lines.push(`keys: ${stringRecord(obj.keys, 1)}`);
  if (obj.rows !== undefined) {
    const layers = Object.keys(obj.rows);
    const body = layers.map((li) => `${pad(2)}${prop(li)}: ${stringRecord(obj.rows[li], 2)},\n`).join('');
    lines.push(layers.length ? `rows: {\n${body}${pad(1)}}` : 'rows: {}');
  }
  if (obj.layers !== undefined) {
    lines.push(`layers: [\n${obj.layers.map((/** @type {any} */ l) => `${pad(2)}${layerText(l, 2)},\n`).join('')}${pad(1)}]`);
  }
  return wrap(OBJECT_HEADER, lines);
}

/**
 * A block: its size, faces, optional surface (a layer with its own keys) and
 * optional sides (keys, then the left and right maps).
 * @param {any} obj
 * @param {string[]} lines  already holds the kind
 * @returns {string}
 */
function serializeBlock(obj, lines) {
  lines.push(`size: ${nums(obj.size)}`, `faces: ${stringRecord(obj.faces, 1)}`);
  if (obj.surface !== undefined) {
    const { keys, ...layer } = obj.surface;
    const inner = [`keys: ${stringRecord(keys, 2)}`, ...layerLines(layer, 1)];
    lines.push(`surface: {\n${inner.map((l) => `${pad(2)}${l},\n`).join('')}${pad(1)}}`);
  }
  if (obj.sides !== undefined) {
    const { keys, left, right } = obj.sides;
    const inner = [`keys: ${stringRecord(keys, 2)}`, `left: ${stringList(left, 2)}`, `right: ${stringList(right, 2)}`];
    lines.push(`sides: {\n${inner.map((l) => `${pad(2)}${l},\n`).join('')}${pad(1)}}`);
  }
  return wrap(OBJECT_HEADER, lines);
}

/**
 * @param {any} item
 * @param {number} depth
 * @returns {string}
 */
function itemText(item, depth) {
  if ('group' in item) {
    const attrs = Object.keys(item.group);
    const group = attrs.length ? `{ ${attrs.map((k) => `${prop(k)}: ${str(item.group[k])}`).join(', ')} }` : '{}';
    return `{\n${pad(depth + 1)}group: ${group},\n${pad(depth + 1)}items: ${itemList(item.items, depth + 1)},\n${pad(depth)}}`;
  }
  const parts = [`object: ${str(item.object)}`];
  if (item.id !== undefined) parts.push(`id: ${str(item.id)}`);
  if (item.class !== undefined) parts.push(`class: ${str(item.class)}`);
  const at = 'tile' in item.at ? `{ tile: ${nums(item.at.tile)} }` : `{ px: ${nums(item.at.px)} }`;
  parts.push(`at: ${at}`);
  return `{ ${parts.join(', ')} }`;
}

/**
 * @param {any[]} items
 * @param {number} depth
 */
const itemList = (items, depth) =>
  items.length ? `[\n${items.map((i) => `${pad(depth + 1)}${itemText(i, depth + 1)},\n`).join('')}${pad(depth)}]` : '[]';

/**
 * @param {any} scene
 * @returns {string}
 */
export function serializeScene(scene) {
  const lines = [];
  if (scene.output !== undefined) lines.push(`output: ${str(scene.output)}`);
  lines.push(`viewBox: ${nums(scene.viewBox)}`);
  if (scene.origin !== undefined) lines.push(`origin: ${nums(scene.origin)}`);
  lines.push(`items: ${itemList(scene.items, 1)}`);
  if (scene.motion !== undefined) lines.push(`motion: ${motionText(scene.motion)}`);
  return wrap(SCENE_HEADER, lines);
}

/**
 * A key on one line: t, then move, show and pose when it sets them.
 * @param {any} key
 */
function keyText(key) {
  const parts = [`t: ${key.t}`];
  if (key.move !== undefined) parts.push(`move: ${nums(key.move)}`);
  if (key.show !== undefined) parts.push(`show: ${key.show}`);
  if (key.pose !== undefined) parts.push(`pose: ${str(key.pose)}`);
  return `{ ${parts.join(', ')} }`;
}

/**
 * A scene's motion (motion.mjs): the duration, then a block per track with
 * one key per line.
 * @param {any} motion
 */
function motionText(motion) {
  const tracks = motion.tracks.map(
    (/** @type {any} */ track) =>
      `${pad(3)}{\n${pad(4)}item: ${str(track.item)},\n${pad(4)}keys: [\n${track.keys.map((/** @type {any} */ k) => `${pad(5)}${keyText(k)},\n`).join('')}${pad(4)}],\n${pad(3)}},\n`,
  );
  return `{\n${pad(2)}duration: ${motion.duration},\n${pad(2)}tracks: ${tracks.length ? `[\n${tracks.join('')}${pad(2)}]` : '[]'},\n${pad(1)}}`;
}

/**
 * An object (it has a `kind`) or a scene.
 * @param {any} doc
 */
export const serialize = (doc) => ('kind' in doc ? serializeObject(doc) : serializeScene(doc));

/**
 * @param {string} header
 * @param {string[]} lines
 */
const wrap = (header, lines) => `${header}\nexport default {\n${lines.map((l) => `  ${l},\n`).join('')}};\n`;
