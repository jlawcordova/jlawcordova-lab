// The selection block a message in Pixel Lab's Agent chat carries
// (docs/intents/2026-10-agent-chat/spec.md, D5): what the owner selected, as
// the saved document has it. Claude works from the file on disk, so a target
// is named by its path in the saved file, and one that isn't in the saved
// file as it is in the app (placed, moved or changed since the save) is left
// out of the block.
//
// Working items are matched to saved ones by their content (object, at, id,
// class, layer, depth and their SVG group's attributes): the k-th working item
// with a given content is the k-th saved item with it. So an item that only
// moved down the list because another was placed before it keeps its saved
// path. A layer is saved when the file has a layer with its id holding the
// same items.

import { NotFoundError } from './errors.mjs';
import { closure, layersOf } from './layer-ops.mjs';

const isGroup = (/** @type {any} */ item) => item && typeof item === 'object' && 'group' in item;

/**
 * The Agent chat a server works for: Pixel Lab starts Claude Code with this
 * server's env holding PIXELLAB_CHAT (the chat's id) and PIXELLAB_CHAT_TITLE
 * (docs/intents/2026-10-agent-chat/spec.md, D7). Its proposals and activity
 * lines record it.
 * @param {Record<string, string | undefined>} env
 * @returns {{ id: string, title: string } | null}
 */
export function chatOf(env) {
  const id = env.PIXELLAB_CHAT?.trim();
  if (!id) return null;
  return { id, title: env.PIXELLAB_CHAT_TITLE?.trim() || 'Chat' };
}

/** A group lists at most this many item paths in the block. */
export const GROUP_PATHS = 60;

/**
 * Every placed item with its path and its wrapper's attributes, in paint order.
 * @param {any} scene
 * @returns {{ path: string, item: any, wrapper: any }[]}
 */
export function walkItems(scene) {
  /** @type {{ path: string, item: any, wrapper: any }[]} */
  const out = [];
  const walk = (/** @type {any[]} */ list, /** @type {number[]} */ prefix, /** @type {any} */ wrapper) =>
    (list ?? []).forEach((item, i) => {
      const path = [...prefix, i];
      if (isGroup(item)) walk(item.items, path, { ...item.group, ...(item.depth !== undefined ? { depth: item.depth } : {}) });
      // Paths as scene.items writes them: "12" at the top level, "5.2" inside a wrapper.
      else if (item) out.push({ path: path.join('.'), item, wrapper });
    });
  walk(scene?.items, [], null);
  return out;
}

/** What makes two items the same item, wherever they are in the list. */
export const contentKey = (/** @type {any} */ item, /** @type {any} */ wrapper) =>
  JSON.stringify([item.object ?? null, item.at ?? null, item.id ?? null, item.class ?? null, item.layer ?? null, item.depth ?? null, wrapper ?? null]);

/**
 * Maps each working item's path to its saved path, or null when the saved
 * file has no such item.
 * @param {any} working
 * @param {any} saved
 * @returns {Map<string, string | null>}
 */
export function matchSaved(working, saved) {
  /** @type {Map<string, string[]>} */
  const savedByKey = new Map();
  for (const { path, item, wrapper } of walkItems(saved)) {
    const key = contentKey(item, wrapper);
    savedByKey.set(key, [...(savedByKey.get(key) ?? []), path]);
  }
  /** @type {Map<string, number>} */
  const seen = new Map();
  /** @type {Map<string, string | null>} */
  const out = new Map();
  for (const { path, item, wrapper } of walkItems(working)) {
    const key = contentKey(item, wrapper);
    const k = seen.get(key) ?? 0;
    seen.set(key, k + 1);
    out.set(path, savedByKey.get(key)?.[k] ?? null);
  }
  return out;
}

/** The items in a layer or inside it, as their content keys (sorted). */
const layerKeys = (/** @type {any} */ scene, /** @type {string} */ id) => {
  const set = closure(scene, [id]);
  return walkItems(scene)
    .filter(({ item }) => item.layer !== undefined && set.has(item.layer))
    .map(({ item, wrapper }) => contentKey(item, wrapper))
    .sort();
};

/** "tile 3,-1,1", "px 31,6 level 0". */
export function atText(/** @type {any} */ at) {
  if (Array.isArray(at?.tile)) return `tile ${at.tile.join(',')}`;
  if (Array.isArray(at?.px)) return `px ${at.px.join(',')}${at.level !== undefined ? ` level ${at.level}` : ''}`;
  return JSON.stringify(at ?? null);
}

/** A layer name in double quotes, inner quotes escaped. */
const quoted = (/** @type {string} */ s) => JSON.stringify(s);

/**
 * The block and what went into it.
 * @param {{
 *   document: string,
 *   scene: string,
 *   version: string,
 *   working: any,
 *   saved: any,
 *   layers?: string[],
 *   items?: string[],
 *   pinned?: string[],
 * }} input  layers and items as the app has them (working copy); pinned holds `layer:<id>` or paths
 */
export function selectionContext({ document, scene, version, working, saved, layers = [], items = [], pinned = [] }) {
  const map = matchSaved(working, saved);
  const workingItems = new Map(walkItems(working).map((e) => [e.path, e]));
  const savedLayers = new Map(layersOf(saved).map((/** @type {any} */ l) => [l.id, l]));
  const workingLayers = new Map(layersOf(working).map((/** @type {any} */ l) => [l.id, l]));
  const pins = new Set(pinned);
  for (const id of layers) if (!workingLayers.has(id)) throw new NotFoundError(`no layer ${JSON.stringify(id)} in ${scene} (scene items lists them)`);
  for (const path of items) if (!workingItems.has(path)) throw new NotFoundError(`no item ${path} in ${scene} (scene items lists the paths)`);
  /** @type {string[]} */
  const unsaved = [];
  /** @type {string[]} */
  const lines = [];

  const outLayers = [];
  for (const id of [...new Set(layers)]) {
    const layer = workingLayers.get(id);
    const onDisk = savedLayers.get(id);
    const ok = !!layer && !!onDisk && JSON.stringify(layerKeys(working, id)) === JSON.stringify(layerKeys(saved, id));
    const paths = onDisk ? walkItems(saved).filter(({ item }) => item.layer !== undefined && closure(saved, [id]).has(item.layer)).map((e) => e.path) : [];
    outLayers.push({ id, name: layer?.name ?? onDisk?.name ?? id, items: ok ? paths : [], saved: ok });
    if (!ok) {
      unsaved.push(`layer:${id}`);
      continue;
    }
    const shown = paths.slice(0, GROUP_PATHS).join(',');
    const more = paths.length > GROUP_PATHS ? ` …and ${paths.length - GROUP_PATHS} more (scene_items lists them)` : '';
    lines.push(`  group ${id} ${quoted(onDisk.name)} items=${shown}${more}${pins.has(`layer:${id}`) ? ' pinned' : ''}`);
  }

  const outItems = [];
  for (const path of [...new Set(items)]) {
    const entry = workingItems.get(path);
    const savedPath = entry ? (map.get(path) ?? null) : null;
    const item = entry?.item;
    outItems.push({ path, savedPath, object: item?.object ?? null, at: item?.at ?? null, ...(item?.layer ? { layer: item.layer } : {}) });
    if (!savedPath) {
      unsaved.push(path);
      continue;
    }
    lines.push(`  item ${savedPath} object=${item.object} at=${atText(item.at)}${item.layer ? ` group=${item.layer}` : ''}${pins.has(path) ? ' pinned' : ''}`);
  }

  if (!layers.length && !items.length) lines.push('  (nothing selected: the whole scene is in view)');
  if (unsaved.length) {
    const what = (/** @type {number} */ n, /** @type {string} */ word) => `${n} selected ${word}${n === 1 ? '' : 's'}`;
    const nl = unsaved.filter((t) => t.startsWith('layer:')).length;
    const ni = unsaved.length - nl;
    const parts = [nl ? what(nl, 'group') : '', ni ? what(ni, 'item') : ''].filter(Boolean).join(' and ');
    lines.push(`  (${parts} ${unsaved.length === 1 ? 'is' : 'are'} left out: ${unsaved.length === 1 ? "it isn't" : "they aren't"} saved yet)`);
  }
  const head = `<selection document=${quoted(document)} scene=${quoted(scene)} version=${quoted(version)}>`;
  return { document, scene, version, text: [head, ...lines, '</selection>'].join('\n'), layers: outLayers, items: outItems, unsaved };
}

/** The block for an object (object-mode context is the whole object; spec, D4). */
export function objectContext({ document, object, version }) {
  return {
    document,
    object,
    version,
    text: `<selection document=${quoted(document)} object=${quoted(object)} version=${quoted(version)}>\n  (the whole object is in view)\n</selection>`,
    layers: [],
    items: [],
    unsaved: [],
  };
}
