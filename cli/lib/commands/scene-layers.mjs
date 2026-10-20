// Scene layer commands (docs/intents/2026-10-scene-layers/spec.md, D4):
// make, rename, ungroup, assign and move layers. The logic is layer-ops.mjs;
// each command is one undo step through ctx.edit, so the app and agents get
// the same edits (an agent's become a proposal at doc_save). Layers never
// reorder the scene's items, so nothing here changes what's published (moving
// items is the exception: they stand somewhere else, so the session's depth
// sort may reorder them).

import { UsageError } from '../errors.mjs';
import * as L from '../layer-ops.mjs';
import * as ops from '../scene-ops.mjs';
import { editOf, formatChange } from './common.mjs';

const NAME = { type: 'string', description: 'scene name (or scenes/<name>)' };
const LAYER = { type: 'string', description: 'a layer: its id, or its name when only one layer has it (scene items lists them)' };
const ITEMS = { type: 'array', items: { type: 'string' }, description: 'items: their paths from scene items (12, 5.2) or ids, comma-separated on the command line' };
const LAYERS = { type: 'array', items: { type: 'string' }, description: 'layers: their ids (or names), comma-separated on the command line' };

/** The scene's name, checked. */
async function sceneName(/** @type {any} */ ctx, /** @type {string} */ query) {
  const { name } = await ctx.find(query, 'scene');
  await ctx.doc('scene', name);
  return name;
}

/** Items named by path or id, as { path, item } (each must be a placed item, not a group). */
const itemsOf = (/** @type {any} */ ctx, /** @type {any} */ doc, /** @type {string[] | undefined} */ refs) =>
  (refs ?? []).map((ref) => {
    const path = ops.findItem(ctx.site, doc, ref);
    return { path, item: ops.placedAt(doc, path) };
  });

/** Layers named by id or name, as ids. */
const layersOf = (/** @type {any} */ doc, /** @type {string[] | undefined} */ refs) => (refs ?? []).map((ref) => L.findLayer(doc, ref).id);

/** '' is the top level; anything else a layer. */
const levelOf = (/** @type {any} */ doc, /** @type {string} */ ref) => (ref === '' ? null : L.findLayer(doc, ref).id);

/** @type {import('../registry.mjs').Command} */
const create = {
  name: 'scene.layer.new',
  summary: 'Make a layer (a named, nestable part of a scene) and move items and layers into it',
  description:
    'A layer names part of a scene (the Mine, the Railway, the Train inside it) so it can be selected, moved and placed into as one. ' +
    'It never changes paint order or what is published: items keep their place in the scene, and their { group } wrappers stay as they are. ' +
    'Without --layer-name it is "Group n". Its parent is --parent (\'\' for the top level), else the deepest layer that holds every member, ' +
    'else the top level. A member already inside another member stays where it is. Its id is made from the name unless --id gives one. ' +
    'Returns the layer, how many items joined it and which layers.',
  params: {
    type: 'object',
    properties: {
      name: NAME,
      layerName: { type: 'string', description: 'the layer\'s name, 1 to 64 characters (default "Group n")' },
      id: { type: 'string', description: 'its id, lowercase kebab-case (default: made from the name)' },
      items: ITEMS,
      layers: LAYERS,
      parent: { type: 'string', description: "the layer to make it in ('' for the top level); default: the members' common parent" },
    },
    required: ['name'],
  },
  args: ['name'],
  mutates: true,
  examples: [
    'pixellab scene layer new hero-island --layer-name Railway --items 140.0,141.0,142.0',
    'pixellab scene layer new hero-island --layer-name Train --items cart --parent railway',
  ],
  async run(ctx, params) {
    const name = await sceneName(ctx, params.name);
    return editOf(ctx)({ kind: 'scene', name }, (/** @type {any} */ doc) => {
      const r = L.newLayer(doc, {
        name: params.layerName,
        id: params.id,
        items: itemsOf(ctx, doc, params.items),
        layers: layersOf(doc, params.layers),
        parent: params.parent === undefined ? undefined : levelOf(doc, params.parent),
      });
      return { message: r.message, layer: r.layer, items: r.items, layers: r.layers };
    });
  },
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const rename = {
  name: 'scene.layer.rename',
  summary: 'Rename a layer',
  description: 'Its id stays the same, so the scene file changes on one line.',
  params: {
    type: 'object',
    properties: { name: NAME, layer: LAYER, to: { type: 'string', description: 'the new name, 1 to 64 characters' } },
    required: ['name', 'layer', 'to'],
  },
  args: ['name', 'layer', 'to'],
  mutates: true,
  examples: ['pixellab scene layer rename hero-island group-1 "Island trees"'],
  async run(ctx, params) {
    const name = await sceneName(ctx, params.name);
    return editOf(ctx)({ kind: 'scene', name }, (/** @type {any} */ doc) => {
      const r = L.renameLayer(doc, params.layer, params.to);
      return { message: r.message, layer: r.layer };
    });
  },
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const ungroup = {
  name: 'scene.layer.ungroup',
  summary: 'Ungroup a layer: what it held moves up one level and stays where it is',
  description:
    "Takes the layer away; its layers and items now belong to its parent (or the top level). Nothing moves on the stage. Returns the parent, and the layers and items (paths) it held.",
  params: { type: 'object', properties: { name: NAME, layer: LAYER }, required: ['name', 'layer'] },
  args: ['name', 'layer'],
  mutates: true,
  examples: ['pixellab scene layer ungroup hero-island railway'],
  async run(ctx, params) {
    const name = await sceneName(ctx, params.name);
    return editOf(ctx)({ kind: 'scene', name }, (/** @type {any} */ doc) => {
      const r = L.ungroupLayer(doc, params.layer);
      return { message: r.message, parent: r.parent, layers: r.layers, items: r.items };
    });
  },
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const assign = {
  name: 'scene.layer.assign',
  summary: 'Move items and layers into a layer, or to the top level',
  description:
    "--to is the layer ('' for the top level). Items keep their place in the scene (paint order doesn't change); a layer can't go inside itself or a layer within it.",
  params: {
    type: 'object',
    properties: { name: NAME, to: { type: 'string', description: "the layer to move them into: its id or name, or '' for the top level" }, items: ITEMS, layers: LAYERS },
    required: ['name', 'to'],
  },
  args: ['name'],
  mutates: true,
  examples: ['pixellab scene layer assign hero-island --to railway --items 150.0,151.0', "pixellab scene layer assign hero-island --to '' --layers train"],
  async run(ctx, params) {
    if (!params.items?.length && !params.layers?.length) throw new UsageError('scene.layer.assign: give --items, --layers or both');
    const name = await sceneName(ctx, params.name);
    return editOf(ctx)({ kind: 'scene', name }, (/** @type {any} */ doc) => {
      const r = L.assignLayer(doc, levelOf(doc, params.to), { items: itemsOf(ctx, doc, params.items), layers: layersOf(doc, params.layers) });
      return { message: r.message, to: r.to, items: r.items, layers: r.layers };
    });
  },
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const move = {
  name: 'scene.layer.move',
  summary: 'Move layers (every item in them, and in the layers inside them) and items by tiles, together',
  description:
    'As dragging a selection on the stage: every item moves by --by col,row[,level] tiles (a pixel-placed item by the same screen offset), ' +
    'each once, in one undo step; rapid repeats coalesce. Placements move at rest: an animated item\'s keys are offsets from its placement, ' +
    "so its motion moves with it. Each item's group keeps its data-x/data-y and data-col/data-row in step, as scene.move does (--no-keep-group leaves them). " +
    'Each item then stands at a new depth, so the scene is put in depth order: paths lists the moved items\' paths after that, and reordered says which other paths moved. ' +
    'Layers themselves never change what is published.',
  params: {
    type: 'object',
    properties: {
      name: NAME,
      layer: LAYER,
      layers: LAYERS,
      items: ITEMS,
      by: { type: 'array', items: { type: 'integer' }, minItems: 2, maxItems: 3, description: 'tiles to move: col,row[,level]' },
      keepGroup: { type: 'boolean', default: true, description: "keep each item's group attributes in step, as scene.move does" },
    },
    required: ['name', 'by'],
  },
  args: ['name', 'layer'],
  mutates: true,
  examples: ['pixellab scene layer move hero-island railway --by 1,0', 'pixellab scene layer move hero-island --layers railway,mine --items 12.0 --by 0,-1'],
  async run(ctx, params) {
    if (params.layer === undefined && !params.layers?.length && !params.items?.length) throw new UsageError('scene.layer.move: give a layer, --layers or --items');
    const name = await sceneName(ctx, params.name);
    const [dc, dr, dl = 0] = params.by;
    const key = `scene.layer.move:${[params.layer ?? '', ...(params.layers ?? []), '|', ...(params.items ?? [])].join(',')}`;
    return editOf(ctx)(
      { kind: 'scene', name },
      async (/** @type {any} */ doc) => {
        const { sources } = await ctx.sources();
        const ids = [...(params.layer === undefined ? [] : [L.findLayer(doc, params.layer).id]), ...layersOf(doc, params.layers)];
        /** @type {Map<string, number[]>} */
        const paths = new Map();
        for (const { path } of L.itemsInLayers(doc, ids)) paths.set(path.join('.'), path);
        for (const { path } of itemsOf(ctx, doc, params.items)) paths.set(path.join('.'), path);
        /** @type {{ path: string, attrs: Record<string, string> }[]} */
        const followed = [];
        for (const path of paths.values()) {
          const item = ops.placedAt(doc, path);
          const before = ops.standing(ctx.site, sources, doc, item);
          ops.moveByTiles(item, dc, dr, dl);
          if (params.keepGroup === false) continue;
          const group = ops.followGroup(doc, path, before, ops.standing(ctx.site, sources, doc, item));
          if (group) followed.push(group);
        }
        const names = ids.map((id) => L.findLayer(doc, id).name);
        const what = names.length === 1 && !params.items?.length ? names[0] : `${paths.size} item${paths.size === 1 ? '' : 's'}`;
        const sign = (/** @type {number} */ n) => (n > 0 ? `+${n}` : String(n));
        const message = paths.size
          ? `Moved ${what} ${sign(dc)} col, ${sign(dr)} row${dl ? `, ${sign(dl)} level` : ''}${followed.length ? `; ${followed.length} group${followed.length === 1 ? '' : 's'} followed` : ''}`
          : `${what} has no items to move`;
        return { message, moved: paths.size, paths: [...paths.keys()], by: [dc, dr, dl], ...(followed.length ? { groups: followed } : {}) };
      },
      { key },
    );
  },
  format: formatChange,
};

export default [create, rename, ungroup, assign, move];
