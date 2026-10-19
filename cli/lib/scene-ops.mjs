// Scene edits, ported from the site's web lab so the CLI, the app and agents
// edit scenes exactly as /lab/pixel-art/ does. The lab keeps these in its
// client code, not the engine: src/components/lab/scene-mode.ts (the scene
// edits at the top of the file and the SceneMode operations), with
// placementPoint from stage.ts. Each function here names the lab function it
// ports; keep them in step when the lab changes.
//
// Every function changes a scene document in place and returns what the
// caller needs (a new path, a message). Paths are index lists into the
// scene's items, as in the lab: [5, 2] is the third item of the group at 5.
// Functions throw a SceneOpError (with a stable code) where the lab would
// announce that nothing could be done.

import { CommandError } from './errors.mjs';

/** A class list: lowercase class names separated by single spaces (scene-mode.ts CLASS_LIST). */
export const CLASS_LIST = /^[a-z][a-z0-9-]*(?: [a-z][a-z0-9-]*)*$/;
/** An item id: lowercase kebab-case (engine.mjs NAME, which validates ids). */
export const ID = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;

/** Tile steps in scene pixels (iso.mjs: 32×16 tiles, 16-pixel levels), as moveByTiles uses them. */
const HALF_W = 16;
const HALF_H = 8;
const LEVEL_H = 16;

export class SceneOpError extends CommandError {
  /** @param {string} message @param {string} [code] */
  constructor(message, code = 'scene_op') {
    super(message, { code });
    this.name = 'SceneOpError';
  }
}

const isGroup = (/** @type {any} */ item) => item && typeof item === 'object' && 'group' in item;
const show = (/** @type {number[]} */ path) => path.join('.');

/**
 * The list that holds the item at `path`, and its index there.
 * Port of scene-mode.ts parentOf.
 * @param {any} doc
 * @param {number[]} path
 */
export function parentOf(doc, path) {
  /** @type {any[]} */
  let list = doc.items;
  for (const i of path.slice(0, -1)) list = list?.[i]?.items;
  return { list, index: path[path.length - 1] };
}

/**
 * The item at `path`, or null. Port of scene-mode.ts itemAt.
 * @param {any} doc
 * @param {number[] | null} path
 */
export function itemAt(doc, path) {
  if (!path || path.length === 0) return null;
  const { list, index } = parentOf(doc, path);
  return list?.[index] ?? null;
}

/**
 * The placed item at `path` (not a group), or a not_found / is_group error.
 * The lab only ever selects placed items, never groups.
 * @param {any} doc
 * @param {number[]} path
 */
export function placedAt(doc, path) {
  const item = itemAt(doc, path);
  if (!item) throw new SceneOpError(`no item at ${show(path)}`, 'not_found');
  if (isGroup(item)) throw new SceneOpError(`${show(path)} is a group, not a placed item; give the path of an item inside it`, 'is_group');
  return item;
}

/**
 * Where an item's placement point is, in scene pixels. Port of stage.ts
 * placementPoint (iso tileToPx for an item on a tile).
 * @param {any} iso  the engine's iso module
 * @param {any} item
 * @param {number[]} origin
 * @returns {[number, number]}
 */
export function placementPoint(iso, item, origin) {
  return item.at?.tile ? iso.tileToPx(item.at.tile, origin) : [item.at?.px?.[0] ?? 0, item.at?.px?.[1] ?? 0];
}

/**
 * Moves an item by whole tiles and levels, however it's placed: a pixel-placed
 * item moves by the same screen offset. Port of scene-mode.ts moveByTiles.
 * @param {any} item
 * @param {number} dc
 * @param {number} dr
 * @param {number} dl
 */
export function moveByTiles(item, dc, dr, dl) {
  if (item.at.tile) {
    const [c, r, l] = item.at.tile;
    item.at.tile = [c + dc, r + dr, l + dl];
  } else {
    const [x, y] = item.at.px;
    item.at.px = [x + (dc - dr) * HALF_W, y + (dc + dr) * HALF_H - dl * LEVEL_H];
  }
}

/**
 * Where a new item goes in the scene's top-level list: back to front by
 * row + col, then level, among the items placed on tiles (lab spec D9.4).
 * Port of scene-mode.ts insertIndex.
 * @param {any[]} items
 * @param {number[]} tile
 */
export function insertIndex(items, [col, row, level]) {
  const index = items.findIndex((other) => {
    const tile = other?.at?.tile;
    if (!tile) return false;
    const [c, r, l] = tile;
    return c + r > col + row || (c + r === col + row && l > level);
  });
  return index === -1 ? items.length : index;
}

/** "column 3, row 4, level 0" or "x 31, y 6". Port of scene-mode.ts spoken. */
export function spoken(/** @type {any} */ item) {
  if (item.at.tile) {
    const [c, r, l] = item.at.tile;
    return `column ${c}, row ${r}, level ${l}`;
  }
  return `x ${item.at.px[0]}, y ${item.at.px[1]}`;
}

/** Takes an item's track out of the scene's motion. Port of scene-mode.ts dropTrack. */
export function dropTrack(/** @type {any} */ doc, /** @type {string} */ id) {
  doc.motion.tracks = doc.motion.tracks.filter((/** @type {any} */ track) => track.item !== id);
}

/**
 * Places an object on a tile, as the newest item in back-to-front order.
 * Port of SceneMode.place. The lab only places on tiles; a pixel placement
 * (which the lab makes by nudging) goes at the end of the top-level list,
 * in front of everything.
 * @param {any} doc
 * @param {string} object
 * @param {{ tile?: number[], px?: number[] }} at
 * @returns {{ path: number[], item: any, message: string }}
 */
export function place(doc, object, at) {
  const item = { object, at: at.tile ? { tile: [...at.tile] } : { px: [.../** @type {number[]} */ (at.px)] } };
  const index = at.tile ? insertIndex(doc.items, at.tile) : doc.items.length;
  doc.items.splice(index, 0, item);
  return { path: [index], item, message: `${object} placed at ${spoken(item)}` };
}

/**
 * Past time 0, moving an animated item moves it at that time: the key
 * there, made if there isn't one, takes the move. Returns false when the
 * item isn't animated (or time is 0), for the caller to move its placement.
 * Port of SceneMode.keyedMove (the lab's motion helper).
 * @param {any} motionLib  the engine's motion module
 * @param {any} doc
 * @param {any} item
 * @param {number} dx
 * @param {number} dy
 * @param {number} time  ms; rounded as the lab rounds its pane time
 */
export function keyedMove(motionLib, doc, item, dx, dy, time) {
  const motion = doc.motion;
  const t = Math.round(time);
  if (!item || item.id === undefined || !motion || t <= 0) return false;
  const ti = motion.tracks.findIndex((/** @type {any} */ track) => track.item === item.id);
  if (ti < 0) return false;
  const start = motionLib.stateAt(motion.tracks[ti], item.object, motion.duration, t).move;
  const keys = motion.tracks[ti].keys;
  let key = keys.find((/** @type {any} */ k) => k.t === t);
  if (!key) keys.splice(keys.filter((/** @type {any} */ k) => k.t < t).length, 0, (key = { t }));
  key.move = [start[0] + dx, start[1] + dy];
  return `${item.object} moved at ${(t / 1000).toFixed(1)} s`;
}

/**
 * Moves an item by tiles and levels. Port of SceneMode.moveSelected: past
 * time 0 an animated item's key moves instead (keyedMove).
 * @param {any} site  for the engine's motion module
 * @param {any} doc
 * @param {number[]} path
 * @param {number} dc
 * @param {number} dr
 * @param {number} dl
 * @param {number} [time]
 */
export function moveItem(site, doc, path, dc, dr, dl, time = 0) {
  const item = placedAt(doc, path);
  const keyed = keyedMove(site.motion, doc, item, (dc - dr) * HALF_W, (dc + dr) * HALF_H - dl * LEVEL_H, time);
  if (keyed) return { path, item, message: keyed, keyed: true };
  moveByTiles(item, dc, dr, dl);
  return { path, item, message: `${item.object} moved to ${spoken(item)}`, keyed: false };
}

/**
 * Nudges an item by pixels; an item on a tile is then placed by pixel.
 * Port of SceneMode.nudgeSelected (keyedMove past time 0, as there).
 * @param {any} site  for iso and motion
 * @param {any} doc
 * @param {number[]} path
 * @param {number} dx
 * @param {number} dy
 * @param {number} [time]
 */
export function nudgeItem(site, doc, path, dx, dy, time = 0) {
  const item = placedAt(doc, path);
  const keyed = keyedMove(site.motion, doc, item, dx, dy, time);
  if (keyed) return { path, item, message: keyed, keyed: true };
  const [x, y] = placementPoint(site.iso, item, doc.origin ?? [0, 0]);
  item.at = { px: [x + dx, y + dy] };
  return { path, item, message: `${item.object} nudged to ${spoken(item)}`, keyed: false };
}

/**
 * Sets one coordinate of an item: col, row or level of an item on a tile,
 * x or y of a pixel-placed one. Port of SceneMode.setField; where the lab
 * ignores a field that doesn't match the placement, this refuses.
 * @param {any} doc
 * @param {number[]} path
 * @param {'col' | 'row' | 'level' | 'x' | 'y'} field
 * @param {number} value
 */
export function setField(doc, path, field, value) {
  const item = placedAt(doc, path);
  if (!Number.isInteger(value)) throw new SceneOpError(`${field} must be a whole number`, 'usage');
  const at = item.at;
  /** @type {Record<string, [number[] | undefined, number]>} */
  const fields = { col: [at.tile, 0], row: [at.tile, 1], level: [at.tile, 2], x: [at.px, 0], y: [at.px, 1] };
  const [list, i] = fields[field] ?? [undefined, 0];
  if (!list) {
    const placed = at.tile ? 'on a tile (col, row, level)' : 'by pixel (x, y)';
    throw new SceneOpError(`${item.object} at ${show(path)} is placed ${placed}, so it has no ${field}; set tile or px to change how it's placed`, 'wrong_placement');
  }
  list[i] = value;
  return { path, item, message: `${item.object} moved to ${spoken(item)}` };
}

/**
 * Places an item at an absolute tile or pixel point, replacing how it was
 * placed. The lab does this by dragging (setUpStage's pointermove sets
 * `at`); the CLI takes the target directly.
 * @param {any} doc
 * @param {number[]} path
 * @param {{ tile?: number[], px?: number[] }} at
 */
export function setAt(doc, path, at) {
  const item = placedAt(doc, path);
  item.at = at.tile ? { tile: [...at.tile] } : { px: [.../** @type {number[]} */ (at.px)] };
  return { path, item, message: `${item.object} moved to ${spoken(item)}` };
}

/**
 * Snaps a pixel-placed item to the level-0 tile under its placement point.
 * Port of SceneMode.snapSelected.
 * @param {any} site  for iso
 * @param {any} doc
 * @param {number[]} path
 */
export function snapItem(site, doc, path) {
  const item = placedAt(doc, path);
  if (item.at.tile) throw new SceneOpError(`${item.object} is already on a tile`, 'already_on_tile');
  item.at = { tile: site.iso.pxToTile(item.at.px, 0, doc.origin ?? [0, 0]) };
  return { path, item, message: `${item.object} snapped to ${spoken(item)}` };
}

/**
 * Moves an item later (forward, delta 1) or earlier (back, delta -1) in
 * paint order, within its list. Port of SceneMode.reorderSelected.
 * @param {any} doc
 * @param {number[]} path
 * @param {1 | -1} delta
 */
export function reorderItem(doc, path, delta) {
  const item = placedAt(doc, path);
  const { list, index } = parentOf(doc, path);
  const target = index + delta;
  if (target < 0 || target >= list.length) {
    throw new SceneOpError(`${item.object} is already ${delta > 0 ? 'at the front' : 'at the back'}`, delta > 0 ? 'at_front' : 'at_back');
  }
  [list[index], list[target]] = [list[target], list[index]];
  const next = [...path.slice(0, -1), target];
  return { path: next, item, message: `${item.object} ${delta > 0 ? 'raised' : 'lowered'} to ${target + 1} of ${list.length} in paint order` };
}

/**
 * Duplicates an item one column over (moveByTiles(1, 0, 0)), right after
 * it in paint order. The copy takes no id, so no track either. Port of
 * SceneMode.duplicateSelected.
 * @param {any} doc
 * @param {number[]} path
 */
export function duplicateItem(doc, path) {
  const item = placedAt(doc, path);
  const twin = JSON.parse(JSON.stringify(item));
  // An id names one item, so the copy doesn't take it, or its track.
  delete twin.id;
  moveByTiles(twin, 1, 0, 0);
  const index = path[path.length - 1] + 1;
  parentOf(doc, path).list.splice(index, 0, twin);
  return { path: [...path.slice(0, -1), index], item: twin, message: `${item.object} duplicated at ${spoken(twin)}` };
}

/**
 * Removes an item, and its motion track if it has an id. Port of
 * SceneMode.removeSelected.
 * @param {any} doc
 * @param {number[]} path
 */
export function removeItem(doc, path) {
  const item = placedAt(doc, path);
  parentOf(doc, path).list.splice(path[path.length - 1], 1);
  if (item.id !== undefined && doc.motion) dropTrack(doc, item.id);
  return { path, item, message: `${item.object} removed` };
}

/**
 * Rebuilds an item's keys in the canonical order: object, id, class, at.
 * @param {any} item
 * @param {{ id?: string, class?: string }} parts
 */
function reorderKeys(item, { id, class: cls }) {
  const { object, at } = item;
  for (const key of Object.keys(item)) delete item[key];
  Object.assign(item, { object, ...(id === undefined ? {} : { id }), ...(cls === undefined ? {} : { class: cls }), at });
}

/**
 * Sets an item's class list, or removes it with ''. Port of the lab's class
 * field (SceneMode.setUpInspector, the lab-class change handler), which
 * keeps the canonical key order object, id, class, at.
 * @param {any} doc
 * @param {number[]} path
 * @param {string} value
 */
export function setClass(doc, path, value) {
  const item = placedAt(doc, path);
  const cls = value.trim();
  if (cls && !CLASS_LIST.test(cls)) throw new SceneOpError('A class is lowercase class names separated by single spaces', 'usage');
  reorderKeys(item, { id: item.id, class: cls || undefined });
  return { path, item, message: cls ? `Class set to ${cls}` : 'Class removed' };
}

/**
 * Sets an item's id, or removes it with ''. The lab has no id field, so
 * this follows the engine's rules (lowercase kebab-case, unique in the
 * scene) and the lab's habits: the canonical key order, and an item's
 * track goes with its id (renamed with it; dropped with it, as removing the
 * item drops it).
 * @param {any} site  for the motion module
 * @param {any} doc
 * @param {number[]} path
 * @param {string} value
 */
export function setId(site, doc, path, value) {
  const item = placedAt(doc, path);
  const id = value.trim();
  if (id && !ID.test(id)) throw new SceneOpError('An id is lowercase kebab-case, like crane-arm', 'usage');
  if (id && id !== item.id && site.motion.itemsById(doc.items).has(id)) throw new SceneOpError(`id '${id}' is already used in this scene`, 'id_taken');
  const old = item.id;
  if (doc.motion && old !== undefined && old !== id) {
    if (id) {
      for (const track of doc.motion.tracks) if (track.item === old) track.item = id;
    } else dropTrack(doc, old);
  }
  reorderKeys(item, { id: id || undefined, class: item.class });
  return { path, item, message: id ? `Id set to ${id}` : 'Id removed' };
}

/**
 * Resolves an item reference: a path ("12", "5.2") or an item id.
 * @param {any} site  for the motion module
 * @param {any} doc
 * @param {string} ref
 * @returns {number[]}
 */
export function findItem(site, doc, ref) {
  if (/^\d+(\.\d+)*$/.test(ref)) return ref.split('.').map(Number);
  const id = ref.replace(/^#/, '');
  /** @type {number[] | null} */
  let found = null;
  const walk = (/** @type {any[]} */ list, /** @type {number[]} */ prefix) =>
    list.forEach((item, i) => {
      if (found) return;
      if (isGroup(item)) walk(item.items ?? [], [...prefix, i]);
      else if (item?.id === id) found = [...prefix, i];
    });
  walk(doc.items ?? [], []);
  if (!found) throw new SceneOpError(`no item with id ${id}; give a path like 12 or 5.2, or an id`, 'not_found');
  return found;
}
