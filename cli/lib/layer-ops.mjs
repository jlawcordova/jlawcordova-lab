// Scene layers (docs/intents/2026-10-scene-layers/spec.md): named parts of a
// scene that can nest (the Mine, the Railway, the Train inside it). A scene's
// `layers` list holds { id, name, parent? }, and a placed item's `layer` names
// the one it belongs to. They're bookkeeping only: nothing here reorders the
// scene's items, and the engine draws a scene the same with or without them
// (R2). They're also apart from the { group, items } wrappers, which these
// functions never read or write (R3).
//
// Like scene-ops.mjs, every function changes a scene document in place and
// throws a SceneOpError with a stable code when nothing can be done.

import { SceneOpError } from './scene-ops.mjs';

/** A layer's name: 1–64 characters on one line, no spaces at either end (engine.mjs LAYER_NAME). */
export const LAYER_NAME = /^\S(?:[^\n\r\t]{0,62}\S)?$/;
/** A layer's id: lowercase kebab-case (engine.mjs NAME). */
export const LAYER_ID = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;

const isGroup = (/** @type {any} */ item) => item && typeof item === 'object' && 'group' in item;

/** The scene's layers (an empty list when it has none). */
export const layersOf = (/** @type {any} */ doc) => (Array.isArray(doc.layers) ? doc.layers : []);

/**
 * Every placed item of the scene with its path, in paint order, wrappers
 * looked through.
 * @param {any} doc
 * @returns {{ path: number[], item: any }[]}
 */
export function placedItems(doc) {
  /** @type {{ path: number[], item: any }[]} */
  const out = [];
  const walk = (/** @type {any[]} */ list, /** @type {number[]} */ prefix) =>
    (list ?? []).forEach((item, i) => {
      if (isGroup(item)) walk(item.items, [...prefix, i]);
      else if (item) out.push({ path: [...prefix, i], item });
    });
  walk(doc.items, []);
  return out;
}

/**
 * A layer by its id, or by its name when only one layer has it.
 * @param {any} doc
 * @param {string} ref
 */
export function findLayer(doc, ref) {
  const layers = layersOf(doc);
  const byId = layers.find((l) => l.id === ref);
  if (byId) return byId;
  const named = layers.filter((l) => l.name === ref);
  if (named.length === 1) return named[0];
  if (named.length > 1) {
    throw new SceneOpError(`${named.length} layers are called ${JSON.stringify(ref)}; give one's id (${named.map((l) => l.id).join(', ')})`, 'ambiguous');
  }
  const known = layers.map((l) => l.id);
  throw new SceneOpError(`no layer ${JSON.stringify(ref)} in this scene${known.length ? ` (it has ${known.join(', ')})` : '; it has no layers yet'}`, 'not_found');
}

/**
 * The layers that hold a layer, outermost first, ending with the layer itself.
 * @param {any} doc
 * @param {string | null | undefined} id
 * @returns {string[]}
 */
export function chainOf(doc, id) {
  const byId = new Map(layersOf(doc).map((l) => [l.id, l]));
  /** @type {string[]} */
  const chain = [];
  let next = id ?? undefined;
  while (next !== undefined && byId.has(next) && !chain.includes(next)) {
    chain.unshift(next);
    next = byId.get(next).parent;
  }
  return chain;
}

/**
 * A layer's id and the ids of every layer inside it.
 * @param {any} doc
 * @param {string} id
 */
export function withInner(doc, id) {
  const out = new Set([id]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const l of layersOf(doc)) {
      if (l.parent !== undefined && out.has(l.parent) && !out.has(l.id)) {
        out.add(l.id);
        grew = true;
      }
    }
  }
  return out;
}

/**
 * Every layer id in `ids` and inside them.
 * @param {any} doc
 * @param {Iterable<string>} ids
 */
export function closure(doc, ids) {
  /** @type {Set<string>} */
  const out = new Set();
  for (const id of ids) for (const inner of withInner(doc, id)) out.add(inner);
  return out;
}

/**
 * The placed items in the given layers, or inside them, in paint order.
 * @param {any} doc
 * @param {Iterable<string>} ids
 */
export function itemsInLayers(doc, ids) {
  const set = closure(doc, ids);
  return placedItems(doc).filter(({ item }) => item.layer !== undefined && set.has(item.layer));
}

/**
 * Sets (or with null takes off) an item's layer, keeping the canonical key
 * order object, id, class, layer, at.
 * @param {any} item
 * @param {string | null} layer
 */
export function setItemLayer(item, layer) {
  const { object, id, class: cls, at, layer: _old, ...rest } = item;
  for (const key of Object.keys(item)) delete item[key];
  Object.assign(item, {
    object,
    ...(id === undefined ? {} : { id }),
    ...(cls === undefined ? {} : { class: cls }),
    ...(layer === null ? {} : { layer }),
    at,
    ...rest,
  });
}

/**
 * Checks a layer name (spec D1) and returns it trimmed.
 * @param {unknown} name
 */
export function checkName(name) {
  const trimmed = typeof name === 'string' ? name.trim() : '';
  if (!LAYER_NAME.test(trimmed)) throw new SceneOpError('A layer name is 1 to 64 characters on one line, like Railway', 'usage');
  return trimmed;
}

/**
 * An id for a new layer, made from its name: lowercase, words joined by
 * dashes, `layer-…` when it doesn't start with a letter, and -2, -3… when
 * it's taken.
 * @param {any} doc
 * @param {string} name
 */
export function idFor(doc, name) {
  const words = name.toLowerCase().normalize('NFKD').replace(/[^a-z0-9]+/g, ' ').trim().split(' ').filter(Boolean);
  let base = words.join('-').slice(0, 40).replace(/-+$/, '');
  if (!/^[a-z]/.test(base)) base = base ? `layer-${base}` : 'layer';
  const taken = new Set(layersOf(doc).map((l) => l.id));
  if (!taken.has(base)) return base;
  for (let n = 2; ; n++) if (!taken.has(`${base}-${n}`)) return `${base}-${n}`;
}

/**
 * "Group n": one more than the highest n of the scene's "Group n" names and
 * group-n ids, so the count goes on after a group is renamed (spec D10).
 */
export function nextGroupName(/** @type {any} */ doc) {
  let high = 0;
  for (const l of layersOf(doc)) {
    for (const m of [/^Group (\d+)$/.exec(l.name), /^group-(\d+)$/.exec(l.id)]) if (m) high = Math.max(high, Number(m[1]));
  }
  return `Group ${high + 1}`;
}

/**
 * The deepest layer that holds every one of `parents` (each a layer id, or
 * null for the top level), or null.
 * @param {any} doc
 * @param {(string | null)[]} parents
 * @returns {string | null}
 */
export function commonParent(doc, parents) {
  if (parents.length === 0) return null;
  const chains = parents.map((p) => (p === null ? [] : chainOf(doc, p)));
  /** @type {string | null} */
  let common = null;
  for (let i = 0; ; i++) {
    const id = chains[0][i];
    if (id === undefined || !chains.every((c) => c[i] === id)) return common;
    common = id;
  }
}

/** The layer an item is in (null for the top level), if that layer exists. */
const layerOfItem = (/** @type {any} */ doc, /** @type {any} */ item) => (item.layer !== undefined && layersOf(doc).some((l) => l.id === item.layer) ? item.layer : null);

/**
 * Drops members that are inside another member: a layer inside a member
 * layer, and an item in (or inside) a member layer stay where they are.
 * @param {any} doc
 * @param {{ path: number[], item: any }[]} items
 * @param {string[]} layers
 */
function topMembers(doc, items, layers) {
  const unique = [...new Set(layers)];
  const keptLayers = unique.filter((id) => !unique.some((other) => other !== id && chainOf(doc, id).slice(0, -1).includes(other)));
  const inner = closure(doc, keptLayers);
  /** @type {Set<string>} */
  const seen = new Set();
  const keptItems = items.filter(({ path, item }) => {
    const key = path.join('.');
    if (seen.has(key)) return false;
    seen.add(key);
    const layer = layerOfItem(doc, item);
    return layer === null || !inner.has(layer);
  });
  return { items: keptItems, layers: keptLayers };
}

/**
 * Makes a layer and moves items and layers into it (spec D4, D10). Its
 * parent is `parent` (null for the top level), else the deepest layer that
 * holds every member, else the top level.
 * @param {any} doc
 * @param {{ name?: string, id?: string, items?: { path: number[], item: any }[], layers?: string[], parent?: string | null }} options
 */
export function newLayer(doc, { name, id, items = [], layers = [], parent }) {
  const members = topMembers(doc, items, layers);
  const layerName = name === undefined ? nextGroupName(doc) : checkName(name);
  if (id !== undefined) {
    if (!LAYER_ID.test(id)) throw new SceneOpError('A layer id is lowercase kebab-case, like railway', 'usage');
    if (layersOf(doc).some((l) => l.id === id)) throw new SceneOpError(`layer id ${id} is already used in this scene`, 'exists');
  }
  const newId = id ?? idFor(doc, layerName);
  const byId = new Map(layersOf(doc).map((l) => [l.id, l]));
  /** @type {string | null} */
  let under;
  if (parent === undefined) {
    under = commonParent(doc, [...members.layers.map((l) => byId.get(l)?.parent ?? null), ...members.items.map(({ item }) => layerOfItem(doc, item))]);
  } else {
    under = parent;
    if (under !== null) {
      const inMember = members.layers.find((m) => chainOf(doc, under).includes(m));
      if (inMember) throw new SceneOpError(`the new layer can't go inside ${inMember}, which it would hold`, 'layer_cycle');
    }
  }
  const list = layersOf(doc).slice();
  let at = list.findIndex((l) => members.layers.includes(l.id));
  if (at < 0) at = list.findIndex((l) => (l.parent ?? null) === under);
  const layer = { id: newId, name: layerName, ...(under === null ? {} : { parent: under }) };
  list.splice(at < 0 ? list.length : at, 0, layer);
  for (const l of list) if (members.layers.includes(l.id)) l.parent = newId;
  doc.layers = list;
  for (const { item } of members.items) setItemLayer(item, newId);
  const inside = itemsInLayers(doc, [newId]).length;
  const what = members.layers.length + members.items.length === 0 ? `Made an empty layer, ${layerName}` : `Grouped ${inside} item${inside === 1 ? '' : 's'} as ${layerName}`;
  return { layer: { ...layer }, items: members.items.length, layers: members.layers, inside, message: what };
}

/**
 * @param {any} doc
 * @param {string} ref
 * @param {string} to
 */
export function renameLayer(doc, ref, to) {
  const layer = findLayer(doc, ref);
  const name = checkName(to);
  const old = layer.name;
  layer.name = name;
  return { layer: { ...layer }, message: old === name ? `${name} is already called that` : `Renamed ${old} to ${name}` };
}

/**
 * Takes a layer away and moves what it held up one level: its layers and
 * items now belong to its parent, and stay where they are (spec D10).
 * @param {any} doc
 * @param {string} ref
 */
export function ungroupLayer(doc, ref) {
  const layer = findLayer(doc, ref);
  const parent = layer.parent ?? null;
  const list = layersOf(doc).filter((l) => l !== layer);
  /** @type {string[]} */
  const layers = [];
  for (const l of list) {
    if (l.parent !== layer.id) continue;
    if (parent === null) delete l.parent;
    else l.parent = parent;
    layers.push(l.id);
  }
  /** @type {string[]} */
  const items = [];
  for (const { path, item } of placedItems(doc)) {
    if (item.layer !== layer.id) continue;
    setItemLayer(item, parent);
    items.push(path.join('.'));
  }
  if (list.length) doc.layers = list;
  else delete doc.layers;
  const parentName = parent === null ? 'the top level' : findLayer(doc, parent).name;
  const n = layers.length + items.length;
  return { parent, layers, items, message: `Ungrouped ${layer.name}: its ${n} part${n === 1 ? '' : 's'} moved up to ${parentName} and stay where they are` };
}

/**
 * Moves items and layers into a layer (null for the top level). Refuses to
 * put a layer inside itself, or inside a layer within it.
 * @param {any} doc
 * @param {string | null} to
 * @param {{ items?: { path: number[], item: any }[], layers?: string[] }} members
 */
export function assignLayer(doc, to, { items = [], layers = [] }) {
  const chain = to === null ? [] : chainOf(doc, to);
  for (const id of layers) {
    if (chain.includes(id)) throw new SceneOpError(id === to ? `layer ${id} can't go inside itself` : `layer ${id} can't go inside ${to}, which is inside it`, 'layer_cycle');
  }
  const byId = new Map(layersOf(doc).map((l) => [l.id, l]));
  for (const id of new Set(layers)) {
    const l = byId.get(id);
    if (to === null) delete l.parent;
    else l.parent = to;
  }
  /** @type {Set<string>} */
  const seen = new Set();
  for (const { path, item } of items) {
    if (seen.has(path.join('.'))) continue;
    seen.add(path.join('.'));
    setItemLayer(item, to);
  }
  const where = to === null ? 'the top level' : byId.get(to).name;
  const said = [seen.size ? `${seen.size} item${seen.size === 1 ? '' : 's'}` : '', new Set(layers).size ? `${new Set(layers).size} layer${new Set(layers).size === 1 ? '' : 's'}` : '']
    .filter(Boolean)
    .join(' and ');
  return { to, items: seen.size, layers: [...new Set(layers)], message: said ? `Moved ${said} into ${where}` : `Nothing to move into ${where}` };
}
