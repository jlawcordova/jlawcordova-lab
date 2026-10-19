// Scene edit commands: place, move, set, raise, lower, duplicate, remove,
// snap and new. The edits themselves are scene-ops.mjs, ported from the web
// lab; these commands find the scene and the item and run one edit as one
// undo step through ctx.edit.

import { CommandError, NotFoundError, UsageError } from '../errors.mjs';
import { pathString } from '../render.mjs';
import * as ops from '../scene-ops.mjs';
import { atMostOne, editOf, formatChange, needOne } from './common.mjs';

const NAME = { type: 'string', description: 'scene name (or scenes/<name>)' };
const ITEM = { type: 'string', description: 'the item: its path from scene items (12, 5.2) or its id' };
const TILE = { type: 'array', items: { type: 'integer' }, minItems: 3, maxItems: 3, description: 'a tile: col,row,level' };
const PX = { type: 'array', items: { type: 'integer' }, minItems: 2, maxItems: 2, description: 'a point in scene pixels: x,y' };

/** The scene's name, checked. */
async function sceneName(/** @type {any} */ ctx, /** @type {string} */ query) {
  const { name } = await ctx.find(query, 'scene');
  await ctx.doc('scene', name);
  return name;
}

/**
 * Runs one item edit on a scene: finds the item by path or id, calls `op`
 * with the working copy and the item's path, and reports the item's new path.
 * @param {any} ctx
 * @param {{ name: string, item: string }} params
 * @param {(doc: any, path: number[]) => { path: number[], item: any, message: string }} op
 * @param {string | null} [key]  coalesces rapid repeats
 */
async function itemEdit(ctx, { name: query, item: ref }, op, key = null) {
  const name = await sceneName(ctx, query);
  return editOf(ctx)(
    { kind: 'scene', name },
    (/** @type {any} */ doc) => {
      const path = ops.findItem(ctx.site, doc, ref);
      const r = op(doc, path);
      return { message: r.message, path: pathString(r.path), item: structuredClone(r.item) };
    },
    { key },
  );
}

const itemParams = (/** @type {Record<string, any>} */ more = {}) => ({
  type: 'object',
  properties: { name: NAME, item: ITEM, ...more },
  required: ['name', 'item'],
});

/** @type {import('../registry.mjs').Command} */
const place = {
  name: 'scene.place',
  summary: 'Place an object on a tile (or at a pixel point) in a scene',
  description:
    'Like the lab\'s Place tool: an item on a tile goes in back-to-front order (by row + col, then level, among the items on tiles), ' +
    'so it paints in front of what\'s behind it. A pixel placement goes last, in front of everything. With neither --at nor --px, ' +
    'it goes on the level-0 tile at the middle of the view, where the lab\'s cursor starts. Returns the new item\'s path.',
  params: {
    type: 'object',
    properties: {
      name: NAME,
      object: { type: 'string', description: 'the object to place' },
      at: TILE,
      px: PX,
      class: { type: 'string', description: 'a class list for the item' },
      id: { type: 'string', description: 'an id for the item (lowercase kebab-case, unique in the scene)' },
    },
    required: ['name', 'object'],
  },
  args: ['name', 'object'],
  mutates: true,
  examples: ['pixellab scene place hero-island pine --at 14,6,1', 'pixellab scene place range-sprite crate --px 40,12 --dry-run'],
  async run(ctx, params) {
    atMostOne(params, ['at', 'px'], 'scene.place');
    const name = await sceneName(ctx, params.name);
    if (!(await ctx.names('object')).includes(params.object)) throw new NotFoundError(`no object named ${params.object} (pixellab object list)`);
    return editOf(ctx)({ kind: 'scene', name }, (/** @type {any} */ doc) => {
      let at = params.px ? { px: params.px } : params.at ? { tile: params.at } : null;
      if (!at) {
        // SceneMode/Lab.open: the cursor starts on the tile at the middle of the view.
        const [x, y, w, h] = doc.viewBox;
        at = { tile: ctx.site.iso.pxToTile([Math.floor(x + w / 2), Math.floor(y + h / 2)], 0, doc.origin ?? [0, 0]) };
      }
      const r = ops.place(doc, params.object, at);
      if (params.class) ops.setClass(doc, r.path, params.class);
      if (params.id) ops.setId(ctx.site, doc, r.path, params.id);
      return { message: r.message, path: pathString(r.path), item: structuredClone(ops.itemAt(doc, r.path)) };
    });
  },
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const move = {
  name: 'scene.move',
  summary: 'Move an item by tiles and levels (--by) or by pixels (--nudge)',
  description:
    'As the lab\'s arrow keys: --by col,row[,level] moves an item on a tile by whole tiles (a pixel-placed item moves by the same screen offset); ' +
    '--nudge x,y moves it by pixels, and an item on a tile is then placed by pixel. With --time past 0, an animated item (one with an id ' +
    'and a motion track) moves at that time instead: the key there, made if needed, takes the move. Rapid repeats coalesce into one undo step.',
  params: itemParams({
    by: { type: 'array', items: { type: 'integer' }, minItems: 2, maxItems: 3, description: 'tiles to move: col,row[,level]' },
    nudge: { type: 'array', items: { type: 'integer' }, minItems: 2, maxItems: 2, description: 'pixels to move: x,y' },
    time: { type: 'integer', minimum: 0, default: 0, description: 'ms into the scene\'s motion; past 0 an animated item\'s key moves' },
  }),
  args: ['name', 'item'],
  mutates: true,
  examples: ['pixellab scene move hero-island 12 --by 1,0', 'pixellab scene move hero-island crane --nudge 0,-1'],
  async run(ctx, params) {
    needOne(params, ['by', 'nudge'], 'scene.move');
    atMostOne(params, ['by', 'nudge'], 'scene.move');
    const { by, nudge, time } = params;
    return itemEdit(
      ctx,
      /** @type {any} */ (params),
      (doc, path) => (by ? ops.moveItem(ctx.site, doc, path, by[0], by[1], by[2] ?? 0, time) : ops.nudgeItem(ctx.site, doc, path, nudge[0], nudge[1], time)),
      `scene.move:${params.item}:${by ? 'by' : 'nudge'}:${time}`,
    );
  },
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const set = {
  name: 'scene.set',
  summary: "Set an item's tile or pixel point, one coordinate, its class or its id",
  description:
    '--tile col,row,level or --px x,y place the item there (changing how it\'s placed if need be); --col, --row, --level (an item on a tile) ' +
    'and --x, --y (a pixel-placed item) set one coordinate, like the lab\'s Selected fields. --class sets the class list and --id the id; ' +
    'an empty value removes it. An id\'s motion track follows it when it\'s renamed and goes with it when it\'s removed. All in one undo step.',
  params: itemParams({
    tile: TILE,
    px: PX,
    col: { type: 'integer' },
    row: { type: 'integer' },
    level: { type: 'integer' },
    x: { type: 'integer' },
    y: { type: 'integer' },
    class: { type: 'string', description: "a class list, or '' to remove it" },
    id: { type: 'string', description: "an id, or '' to remove it" },
  }),
  args: ['name', 'item'],
  mutates: true,
  examples: ['pixellab scene set hero-island 12 --tile 3,4,0', "pixellab scene set hero-island 12 --class 'sway slow'"],
  async run(ctx, params) {
    const fields = /** @type {const} */ (['col', 'row', 'level', 'x', 'y']);
    needOne(params, ['tile', 'px', ...fields, 'class', 'id'], 'scene.set');
    atMostOne(params, ['tile', 'px'], 'scene.set');
    return itemEdit(ctx, /** @type {any} */ (params), (doc, path) => {
      /** @type {string[]} */
      const said = [];
      let r;
      if (params.tile || params.px) said.push((r = ops.setAt(doc, path, params.tile ? { tile: params.tile } : { px: params.px })).message);
      for (const f of fields) if (params[f] !== undefined) r = ops.setField(doc, path, f, params[f]);
      if (r && !params.tile && !params.px) said.push(r.message);
      if (params.class !== undefined) said.push((r = ops.setClass(doc, path, params.class)).message);
      if (params.id !== undefined) said.push((r = ops.setId(ctx.site, doc, path, params.id)).message);
      return { path, item: ops.itemAt(doc, path), message: said.join('; ') };
    });
  },
  format: formatChange,
};

/** @param {'raise' | 'lower'} which @returns {import('../registry.mjs').Command} */
const reorder = (which) => ({
  name: `scene.${which}`,
  summary: which === 'raise' ? 'Raise an item: paint it one later, in front of the next item' : 'Lower an item: paint it one earlier, behind the item before it',
  description: 'Swaps the item with its neighbour in its own list (the scene\'s items, or its group\'s), as the lab\'s Raise and Lower do. Returns its new path.',
  params: itemParams(),
  args: ['name', 'item'],
  mutates: true,
  async run(ctx, params) {
    return itemEdit(ctx, /** @type {any} */ (params), (doc, path) => ops.reorderItem(doc, path, which === 'raise' ? 1 : -1));
  },
  format: formatChange,
});

/** @type {import('../registry.mjs').Command} */
const duplicate = {
  name: 'scene.duplicate',
  summary: 'Duplicate an item one column over, right after it in paint order',
  description: "As the lab's Duplicate: the copy is moved one column (a tile, or the same screen offset by pixel) and takes no id, so no motion track.",
  params: itemParams(),
  args: ['name', 'item'],
  mutates: true,
  async run(ctx, params) {
    return itemEdit(ctx, /** @type {any} */ (params), (doc, path) => ops.duplicateItem(doc, path));
  },
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const remove = {
  name: 'scene.remove',
  summary: 'Remove an item (and its motion track, if it has an id)',
  params: itemParams(),
  args: ['name', 'item'],
  mutates: true,
  async run(ctx, params) {
    return itemEdit(ctx, /** @type {any} */ (params), (doc, path) => ops.removeItem(doc, path));
  },
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const snap = {
  name: 'scene.snap',
  summary: 'Snap a pixel-placed item to the level-0 tile under its placement point',
  params: itemParams(),
  args: ['name', 'item'],
  mutates: true,
  async run(ctx, params) {
    return itemEdit(ctx, /** @type {any} */ (params), (doc, path) => ops.snapItem(ctx.site, doc, path));
  },
  format: formatChange,
};

/**
 * Refuses a name that isn't kebab-case, is too long, or is taken by any
 * object or scene (dialogs.ts nameProblem, as `npm run art -- --new` checks).
 * @param {any} ctx
 * @param {string} name
 * @param {string} command
 */
export async function checkNewName(ctx, name, command) {
  const { MAX_NAME } = ctx.site.starter;
  if (name.length > MAX_NAME) throw new UsageError(`${command}: name is ${name.length} characters long, max ${MAX_NAME}`);
  if (!ctx.site.engine.isName(name)) throw new UsageError(`${command}: name ${JSON.stringify(name)} must be lowercase kebab-case, like small-rock`);
  const taken = (await ctx.names('object')).includes(name) || (await ctx.names('scene')).includes(name) || (await ctx.loadError('object', name)) || (await ctx.loadError('scene', name));
  if (taken) throw new CommandError(`there's already an object or scene called ${name}`, { code: 'exists' });
}

/** @type {import('../registry.mjs').Command} */
const create = {
  name: 'scene.new',
  summary: 'Make a new, empty scene (the starter: a 128×128 view around tile 0,0,0)',
  params: { type: 'object', properties: { name: { type: 'string', description: 'the new scene\'s name (lowercase kebab-case)' } }, required: ['name'] },
  args: ['name'],
  mutates: true,
  async run(ctx, { name }) {
    await checkNewName(ctx, name, 'scene.new');
    const label = `Created scene ${name}`;
    ctx.session.create(ctx, 'scene', name, ctx.site.starter.starterScene(), label);
    return { doc: `scene:${name}`, changed: true, summary: label, file: `scenes/${name}.mjs` };
  },
  format: formatChange,
};

export default [place, move, set, reorder('raise'), reorder('lower'), duplicate, remove, snap, create];
