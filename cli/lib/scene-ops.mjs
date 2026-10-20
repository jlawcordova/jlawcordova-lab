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
 * Sets a pixel-placed item's level, leaving the key out at 0 as the
 * serializer does (docs/intents/2026-10-depth-order/spec.md, D1).
 * @param {any} at
 * @param {number} level
 */
function setLevel(at, level) {
  if (level) at.level = level;
  else delete at.level;
}

/**
 * Moves an item by whole tiles and levels, however it's placed: a pixel-placed
 * item moves by the same screen offset, and its level by the same levels, so
 * it stands over the same tile a level higher (depth order, R7). Port of
 * scene-mode.ts moveByTiles.
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
    if (dl) setLevel(item.at, (item.at.level ?? 0) + dl);
  }
}

/** "column 3, row 4, level 0", "x 31, y 6" or, for a pixel item standing above level 0, "x 40, y -12, level 4". Port of scene-mode.ts spoken. */
export function spoken(/** @type {any} */ item) {
  if (item.at.tile) {
    const [c, r, l] = item.at.tile;
    return `column ${c}, row ${r}, level ${l}`;
  }
  return `x ${item.at.px[0]}, y ${item.at.px[1]}${item.at.level ? `, level ${item.at.level}` : ''}`;
}

/** Takes an item's track out of the scene's motion. Port of scene-mode.ts dropTrack. */
export function dropTrack(/** @type {any} */ doc, /** @type {string} */ id) {
  doc.motion.tracks = doc.motion.tracks.filter((/** @type {any} */ track) => track.item !== id);
}

/**
 * An `at` for a new or moved item: a tile, or a pixel point with the level it
 * stands on (left out at 0).
 * @param {{ tile?: number[], px?: number[], level?: number }} at
 */
function atOf(at) {
  if (at.tile) return { tile: [...at.tile] };
  const out = { px: [.../** @type {number[]} */ (at.px)] };
  return at.level ? { ...out, level: at.level } : out;
}

/**
 * Places an object on a tile or at a pixel point, as the newest item of the
 * scene's list; the session's sort (sortScene) then puts it where it stands.
 * Port of SceneMode.place, which slotted a tile item in by row and column.
 * @param {any} doc
 * @param {string} object
 * @param {{ tile?: number[], px?: number[], level?: number }} at
 * @returns {{ path: number[], item: any, message: string }}
 */
export function place(doc, object, at) {
  const item = { object, at: atOf(at) };
  doc.items.push(item);
  return { path: [doc.items.length - 1], item, message: `${object} placed at ${spoken(item)}` };
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
  // It keeps the level it stands on, so nudging it doesn't change what it stands behind.
  item.at = atOf({ px: [x + dx, y + dy], level: item.at.tile ? item.at.tile[2] : item.at.level });
  return { path, item, message: `${item.object} nudged to ${spoken(item)}`, keyed: false };
}

/**
 * Sets one coordinate of an item: col, row or level of an item on a tile,
 * x or y of a pixel-placed one, or the level a pixel-placed one stands on.
 * Port of SceneMode.setField; where the lab ignores a field that doesn't
 * match the placement, this refuses.
 * @param {any} doc
 * @param {number[]} path
 * @param {'col' | 'row' | 'level' | 'x' | 'y'} field
 * @param {number} value
 */
export function setField(doc, path, field, value) {
  const item = placedAt(doc, path);
  if (!Number.isInteger(value)) throw new SceneOpError(`${field} must be a whole number`, 'usage');
  const at = item.at;
  if (field === 'level' && at.px) {
    setLevel(at, value);
    return { path, item, message: `${item.object} moved to ${spoken(item)}` };
  }
  /** @type {Record<string, [number[] | undefined, number]>} */
  const fields = { col: [at.tile, 0], row: [at.tile, 1], level: [at.tile, 2], x: [at.px, 0], y: [at.px, 1] };
  const [list, i] = fields[field] ?? [undefined, 0];
  if (!list) {
    const placed = at.tile ? 'on a tile (col, row, level)' : 'by pixel (x, y, level)';
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
 * @param {{ tile?: number[], px?: number[], level?: number }} at
 */
export function setAt(doc, path, at) {
  const item = placedAt(doc, path);
  // A pixel point keeps the level the item already stands on, unless the caller gives one.
  item.at = atOf(at.tile ? at : { px: at.px, level: at.level ?? item.at.level });
  return { path, item, message: `${item.object} moved to ${spoken(item)}` };
}

/**
 * Snaps a pixel-placed item to the tile under its placement point at the
 * level it stands on (level 0 unless it has one). Port of SceneMode.snapSelected,
 * which snapped to level 0.
 * @param {any} site  for iso
 * @param {any} doc
 * @param {number[]} path
 */
export function snapItem(site, doc, path) {
  const item = placedAt(doc, path);
  if (item.at.tile) throw new SceneOpError(`${item.object} is already on a tile`, 'already_on_tile');
  item.at = { tile: site.iso.pxToTile(item.at.px, item.at.level ?? 0, doc.origin ?? [0, 0]) };
  return { path, item, message: `${item.object} snapped to ${spoken(item)}` };
}

/**
 * Duplicates an item one column over (moveByTiles(1, 0, 0)), listed right
 * after it until the session's sort puts it where it stands. The copy takes
 * no id, so no track either. Port of SceneMode.duplicateSelected.
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
 * Rebuilds an item's keys in the canonical order: object, id, class, layer,
 * depth, at (a layer stays as it was: docs/intents/2026-10-scene-layers/spec.md,
 * D1; depth: docs/intents/2026-10-depth-order/spec.md, D1).
 * @param {any} item
 * @param {{ id?: string, class?: string }} parts
 */
function reorderKeys(item, { id, class: cls }) {
  const { object, at, layer, depth } = item;
  for (const key of Object.keys(item)) delete item[key];
  Object.assign(item, { object, ...(id === undefined ? {} : { id }), ...(cls === undefined ? {} : { class: cls }), ...(layer === undefined ? {} : { layer }), ...(depth === undefined ? {} : { depth }), at });
}

/**
 * Sets an item's or a group's depth override: 'front' draws it over everything
 * else in its list, 'back' under everything, and 'auto' takes the override
 * off, so it draws by where it stands. Keeps the canonical key order.
 * @param {any} doc
 * @param {number[]} path  an item's or a group's
 * @param {'front' | 'back' | 'auto'} depth
 */
export function setDepth(doc, path, depth) {
  const item = itemAt(doc, path);
  if (!item) throw new SceneOpError(`no item at ${show(path)}`, 'not_found');
  if (depth !== 'front' && depth !== 'back' && depth !== 'auto') throw new SceneOpError("depth is 'front', 'back' or 'auto'", 'usage');
  const value = depth === 'auto' ? undefined : depth;
  if (isGroup(item)) {
    const { group, items, depth: _old, ...rest } = item;
    for (const key of Object.keys(item)) delete item[key];
    Object.assign(item, { group, ...(value === undefined ? {} : { depth: value }), items, ...rest });
  } else {
    if (value === undefined) delete item.depth;
    else item.depth = value;
    reorderKeys(item, { id: item.id, class: item.class });
  }
  const what = isGroup(item) ? `Group ${show(path)}` : item.object;
  const message = value === undefined ? `${what} draws by where it stands` : `${what} draws ${value === 'front' ? 'in front of' : 'behind'} everything else in its list`;
  return { path, item, message };
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

// Groups (docs/intents/2026-10-mcp-gaps/spec.md, D7). The lab never edited
// groups; these follow the site's conventions for them: a group's data-x and
// data-y are its first item's foot, data-col and data-row its tile.

/** A group attribute's name: class or data-* (engine.mjs DATA_ATTR). */
export const GROUP_ATTR = /^(class|data-[a-z][a-z0-9-]*)$/;

/**
 * The path of a group: `ref` names the group itself (its path), or an item
 * inside it (a path or an id), whose innermost group it is.
 * @param {any} site
 * @param {any} doc
 * @param {string} ref
 * @returns {number[]}
 */
export function groupPathOf(site, doc, ref) {
  /** @type {number[]} */
  let path;
  if (/^\d+(\.\d+)*$/.test(ref)) {
    path = ref.split('.').map(Number);
    const at = itemAt(doc, path);
    if (!at) throw new SceneOpError(`no item or group at ${show(path)}`, 'not_found');
    if (isGroup(at)) return path;
  } else path = findItem(site, doc, ref);
  if (path.length < 2) throw new SceneOpError(`${ref} is at the top level, not inside a group`, 'not_in_group');
  return path.slice(0, -1);
}

/**
 * An item's foot in scene pixels, as the site's entrance groups record it:
 * an item on a tile stands on its tile's top-face center (its placement
 * point); a sprite placed by pixel on the center of its bottom row.
 * @param {any} site
 * @param {any} sources  the engine's, to size the item's object
 * @param {any} item
 * @param {number[]} origin
 * @returns {[number, number]}
 */
export function footOf(site, sources, item, origin) {
  const [x, y] = placementPoint(site.iso, item, origin);
  if (item.at?.tile) return [x, y];
  let r;
  try {
    r = site.engine.resolve(sources, item.object);
  } catch {
    return [x, y];
  }
  if (r.block || !r.anchor) return [x, y];
  return [Math.round(x - r.anchor[0] + r.width / 2), y - r.anchor[1] + r.height - 1];
}

/** Where an item stands, for followGroup: its point, foot and tile. */
export function standing(/** @type {any} */ site, /** @type {any} */ sources, /** @type {any} */ doc, /** @type {any} */ item) {
  const origin = doc.origin ?? [0, 0];
  return { point: placementPoint(site.iso, item, origin), foot: footOf(site, sources, item, origin), tile: item.at?.tile ? [...item.at.tile] : null };
}

const int = (/** @type {unknown} */ v) => (typeof v === 'string' && /^-?\d+$/.test(v) ? Number(v) : null);

/**
 * Keeps the item's innermost group in step with a move: data-x and data-y
 * that were its placement point (or its foot) take the new one, and
 * data-col, data-row that were its tile take the new tile. Returns the
 * group's path and attributes when anything changed, else null.
 * @param {any} doc
 * @param {number[]} path  the item's path
 * @param {ReturnType<typeof standing>} before
 * @param {ReturnType<typeof standing>} after
 */
export function followGroup(doc, path, before, after) {
  if (path.length < 2) return null;
  const group = itemAt(doc, path.slice(0, -1));
  if (!isGroup(group) || !group.group) return null;
  const attrs = group.group;
  let changed = false;
  const [gx, gy] = [int(attrs['data-x']), int(attrs['data-y'])];
  if (gx !== null && gy !== null) {
    const pick = gx === before.point[0] && gy === before.point[1] ? after.point : gx === before.foot[0] && gy === before.foot[1] ? after.foot : null;
    if (pick && (pick[0] !== gx || pick[1] !== gy)) {
      attrs['data-x'] = String(pick[0]);
      attrs['data-y'] = String(pick[1]);
      changed = true;
    }
  }
  const [gc, gr] = [int(attrs['data-col']), int(attrs['data-row'])];
  if (gc !== null && gr !== null && before.tile && after.tile && gc === before.tile[0] && gr === before.tile[1] && (gc !== after.tile[0] || gr !== after.tile[1])) {
    attrs['data-col'] = String(after.tile[0]);
    attrs['data-row'] = String(after.tile[1]);
    changed = true;
  }
  return changed ? { path: show(path.slice(0, -1)), attrs: { ...attrs } } : null;
}

/**
 * Sets (or with '' removes) a group's attributes. New ones go after the
 * existing ones. Refuses names and values the engine would.
 * @param {any} doc
 * @param {number[]} path  the group's path
 * @param {Record<string, string>} attrs
 */
export function setGroupAttrs(doc, path, attrs) {
  const group = itemAt(doc, path);
  if (!isGroup(group)) throw new SceneOpError(`${show(path)} is not a group`, 'not_a_group');
  for (const [name, value] of Object.entries(attrs)) {
    if (!GROUP_ATTR.test(name)) throw new SceneOpError(`A group attribute is class or data-*, like data-enter; not ${JSON.stringify(name)}`, 'usage');
    if (typeof value !== 'string' || /["<>&]/.test(value)) throw new SceneOpError(`group attribute ${name} must be a string without " < > &`, 'usage');
  }
  group.group = { ...(group.group ?? {}) };
  /** @type {string[]} */
  const said = [];
  for (const [name, value] of Object.entries(attrs)) {
    if (value === '') {
      if (name in group.group) said.push(`${name} removed`);
      delete group.group[name];
    } else {
      group.group[name] = value;
      said.push(`${name} ${value}`);
    }
  }
  return { path, group, message: `Group ${show(path)}: ${said.join(', ') || 'unchanged'}` };
}

/** The first placed item in a list, depth first, with its path below `prefix`. */
export function firstPlaced(/** @type {any[]} */ list, /** @type {number[]} */ prefix = []) {
  for (let i = 0; i < (list ?? []).length; i++) {
    const item = list[i];
    if (isGroup(item)) {
      const found = firstPlaced(item.items ?? [], [...prefix, i]);
      if (found) return found;
    } else if (item) return { item, path: [...prefix, i] };
  }
  return null;
}

/**
 * Places an object inside a group: an existing one at `group`, and/or a new
 * one with `newGroup` attributes. The item (or the new group) goes last in
 * the list it joins; the session's sort then puts it where it stands.
 * @param {any} doc
 * @param {string} object
 * @param {{ tile?: number[], px?: number[], level?: number }} at
 * @param {{ group?: number[] | null, newGroup?: Record<string, string> | null }} where
 * @returns {{ path: number[], group: number[], item: any, message: string }}
 */
export function placeInGroup(doc, object, at, { group = null, newGroup = null }) {
  const item = { object, at: atOf(at) };
  /** @type {any[]} */
  let list = doc.items;
  if (group) {
    const g = itemAt(doc, group);
    if (!isGroup(g)) throw new SceneOpError(`${show(group)} is not a group`, 'not_a_group');
    g.items ??= [];
    list = g.items;
  }
  if (newGroup) {
    list.push({ group: { ...newGroup }, items: [item] });
    const gpath = [...(group ?? []), list.length - 1];
    return { path: [...gpath, 0], group: gpath, item, message: `${object} placed at ${spoken(item)} in a new group` };
  }
  list.push(item);
  return { path: [.../** @type {number[]} */ (group), list.length - 1], group: /** @type {number[]} */ (group), item, message: `${object} placed at ${spoken(item)} in group ${show(/** @type {number[]} */ (group))}` };
}

// Depth order (docs/intents/2026-10-depth-order/spec.md, R4): every scene
// edit ends by putting each list of items in depth order, so the file, scene
// items and the Layers panel read back to front.

/**
 * The longest run of positions that already hold their relative order: the
 * items of a sorted list that stayed put, so the others are the ones the
 * sort moved. `order[k]` is the old index of the item now at k.
 * @param {number[]} order
 * @returns {Set<number>}  the old indices that stayed
 */
function stayed(order) {
  /** @type {number[]} */
  const length = [];
  /** @type {number[]} */
  const from = [];
  let best = -1;
  order.forEach((old, k) => {
    length[k] = 1;
    from[k] = -1;
    for (let j = 0; j < k; j++) if (order[j] < old && length[j] + 1 > length[k]) [length[k], from[k]] = [length[j] + 1, j];
    if (best < 0 || length[k] > length[best]) best = k;
  });
  const out = new Set();
  for (let k = best; k >= 0; k = from[k]) out.add(order[k]);
  return out;
}

/**
 * Puts every list of the scene's items (the scene's, and each group's) in
 * depth order, in place. A group sorts as one object in its list and its
 * members sort inside it. Returns what moved (the items the sort took out of
 * their place, by path before and after) and `remap`, every path that
 * changed ("12" → "4", a moved group's members too), for a caller holding
 * other paths. Nothing is touched if the order can't be worked out (an item
 * names an object that doesn't resolve): then `moved` and `remap` are empty.
 * @param {any} site  for the engine's depth order
 * @param {any} sources  the engine's, to size the items' objects
 * @param {any} doc
 * @returns {{ moved: { path: string, to: string, object?: string, group?: true }[], remap: Map<string, string> }}
 */
export function sortScene(site, sources, doc) {
  const origin = doc.origin ?? [0, 0];
  /** @type {Map<string, any>} */
  const tracks = new Map((doc.motion?.tracks ?? []).map((/** @type {any} */ track) => [track.item, track]));
  /** Every item and group of the scene by its path now. */
  const pathsOf = () => {
    /** @type {Map<any, string>} */
    const out = new Map();
    const walk = (/** @type {any[]} */ list, /** @type {number[]} */ prefix) =>
      (list ?? []).forEach((item, i) => {
        out.set(item, show([...prefix, i]));
        if (isGroup(item)) walk(item.items, [...prefix, i]);
      });
    walk(doc.items, []);
    return out;
  };
  /** @type {{ list: any[], order: number[] }[]} */
  const plans = [];
  try {
    const plan = (/** @type {any[]} */ list) => {
      for (const item of list) if (isGroup(item)) plan(item.items ?? []);
      plans.push({ list, order: site.engine.depthOrder(sources, list, origin, tracks) });
    };
    plan(doc.items ?? []);
  } catch {
    return { moved: [], remap: new Map() };
  }
  if (plans.every(({ order }) => order.every((old, k) => old === k))) return { moved: [], remap: new Map() };
  const before = pathsOf();
  /** @type {any[]} */
  const jumped = [];
  for (const { list, order } of plans) {
    const keep = stayed(order);
    for (const old of order) if (!keep.has(old)) jumped.push(list[old]);
    const items = list.slice();
    order.forEach((old, k) => (list[k] = items[old]));
  }
  const after = pathsOf();
  /** @type {Map<string, string>} */
  const remap = new Map();
  for (const [item, path] of before) if (after.get(item) !== path) remap.set(path, /** @type {string} */ (after.get(item)));
  const moved = jumped
    .map((item) => ({ path: /** @type {string} */ (before.get(item)), to: /** @type {string} */ (after.get(item)), ...(isGroup(item) ? { group: /** @type {true} */ (true) } : { object: item.object }) }))
    .sort((a, b) => a.path.localeCompare(b.path, 'en', { numeric: true }));
  return { moved, remap };
}

/**
 * Where a scene's items and groups went between two states of the same scene
 * (undo and redo swap the whole doc): old path → new path, for the paths that
 * changed, so a caller holding paths (the app's selection) can follow. Items
 * are matched by content, in the order they come (so identical items pair
 * off in order); what's left is matched by object, in order, so an item whose
 * placement the step changed still pairs with itself. A group goes where its
 * first item went. Items that exist on one side only aren't in the map.
 * @param {any} before
 * @param {any} after
 * @returns {Map<string, string>}
 */
export function pathChanges(before, after) {
  /** @param {any} doc */
  const items = (doc) => {
    /** @type {{ path: string, item: any }[]} */
    const out = [];
    const walk = (/** @type {any[]} */ list, /** @type {number[]} */ prefix) =>
      (list ?? []).forEach((item, i) => {
        if (isGroup(item)) walk(item.items, [...prefix, i]);
        else out.push({ path: show([...prefix, i]), item });
      });
    walk(doc.items, []);
    return out;
  };
  const was = items(before);
  const now = items(after);
  /** @type {Map<string, string>} */
  const to = new Map();
  /** @type {Set<number>} */
  const taken = new Set();
  /** @type {Map<string, number[]>} */
  const byContent = new Map();
  now.forEach(({ item }, i) => {
    const key = JSON.stringify(item);
    byContent.set(key, [...(byContent.get(key) ?? []), i]);
  });
  const left = [];
  for (const entry of was) {
    const i = byContent.get(JSON.stringify(entry.item))?.shift();
    if (i === undefined) left.push(entry);
    else {
      taken.add(i);
      to.set(entry.path, now[i].path);
    }
  }
  /** @type {Map<string, number[]>} */
  const byObject = new Map();
  now.forEach(({ item }, i) => {
    if (!taken.has(i)) byObject.set(item?.object, [...(byObject.get(item?.object) ?? []), i]);
  });
  for (const entry of left) {
    const i = byObject.get(entry.item?.object)?.shift();
    if (i !== undefined) to.set(entry.path, now[i].path);
  }
  // A group goes where its first item went (the same depth of nesting).
  /** @type {Set<string>} */
  const groupsNow = new Set();
  const collect = (/** @type {any[]} */ list, /** @type {number[]} */ prefix) =>
    (list ?? []).forEach((item, i) => {
      if (!isGroup(item)) return;
      groupsNow.add(show([...prefix, i]));
      collect(item.items, [...prefix, i]);
    });
  collect(after.items, []);
  const groups = (/** @type {any[]} */ list, /** @type {number[]} */ prefix) =>
    (list ?? []).forEach((item, i) => {
      if (!isGroup(item)) return;
      const path = [...prefix, i];
      const first = (/** @type {any} */ g, /** @type {number[]} */ p) => {
        for (const [k, m] of (g.items ?? []).entries()) return isGroup(m) ? first(m, [...p, k]) : show([...p, k]);
        return undefined;
      };
      const moved = first(item, path);
      const there = moved === undefined ? undefined : to.get(moved);
      const parent = there?.split('.').slice(0, path.length).join('.');
      if (parent !== undefined && groupsNow.has(parent)) to.set(show(path), parent);
      groups(item.items, path);
    });
  groups(before.items, []);
  for (const [path, next] of [...to]) if (path === next) to.delete(path);
  return to;
}
