// Scene edit commands: place, move, set, duplicate, remove, snap, sort, new
// and group.set. The edits themselves are scene-ops.mjs, ported from the web
// lab; these commands find the scene and the item and run one edit as one
// undo step through ctx.edit. Moving an item at rest keeps its group's
// data-x/data-y in step (docs/intents/2026-10-mcp-gaps/spec.md, D7). Every
// scene edit ends with the session putting the items in depth order
// (docs/intents/2026-10-depth-order/spec.md), so a command reports the
// paths it names after that sort, and `reordered` when it moved any.

import { CommandError, NotFoundError, UsageError } from '../errors.mjs';
import { pathString } from '../render.mjs';
import * as layerOps from '../layer-ops.mjs';
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

const KEEP_GROUP = {
  type: 'boolean',
  default: true,
  description: "keep the item's group in step: data-x/data-y that were its placement point or foot, and data-col/data-row that were its tile, follow it",
};

/**
 * An item edit that moves the item at rest: like itemEdit, then its
 * innermost group's attributes follow it (scene-ops followGroup) unless
 * keepGroup is false or the move was keyed past time 0.
 * @param {any} ctx
 * @param {{ name: string, item: string, keepGroup?: boolean }} params
 * @param {(doc: any, path: number[]) => { path: number[], item: any, message: string, keyed?: boolean }} op
 * @param {string | null} [key]
 */
async function moveEdit(ctx, params, op, key = null) {
  const name = await sceneName(ctx, params.name);
  return editOf(ctx)(
    { kind: 'scene', name },
    async (/** @type {any} */ doc) => {
      const { sources } = await ctx.sources();
      const path = ops.findItem(ctx.site, doc, params.item);
      const before = ops.standing(ctx.site, sources, doc, ops.placedAt(doc, path));
      const r = op(doc, path);
      const group = params.keepGroup !== false && !r.keyed ? ops.followGroup(doc, r.path, before, ops.standing(ctx.site, sources, doc, r.item)) : null;
      return {
        message: group ? `${r.message}; its group's ${Object.keys(group.attrs).filter((k) => /^data-(x|y|col|row)$/.test(k)).join(', ')} followed` : r.message,
        path: pathString(r.path),
        item: structuredClone(r.item),
        ...(group ? { group } : {}),
      };
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
    'Like the lab\'s Place tool, but the item goes where it stands: it is added to its list and the scene\'s items are put in depth order, ' +
    'so it draws behind what is in front of it and over what is behind (scene.items shows the order). With neither --at nor --px, ' +
    'it goes on the level-0 tile at the middle of the view, where the lab\'s cursor starts. --px takes --level, how high the item stands ' +
    '(default 0), which is only for depth: its pixel point is where its foot is drawn. ' +
    '--group puts it inside an existing group (the group\'s path, or an item inside it); --new-group wraps it in a new group with those attributes ' +
    '(class and data-*, like {"class": "enter", "data-enter": "object"}), and --foot sets that group\'s data-x and data-y to the item\'s foot. ' +
    '--layer puts it in a scene layer (its id or name; layers are apart from groups and never change paint order). ' +
    'Returns the new item\'s path after the sort, and its group\'s; reordered says which other paths the sort moved.',
  params: {
    type: 'object',
    properties: {
      name: NAME,
      object: { type: 'string', description: 'the object to place' },
      at: TILE,
      px: PX,
      level: { type: 'integer', description: 'with --px: how high the item stands, for depth order (default 0)' },
      class: { type: 'string', description: 'a class list for the item' },
      id: { type: 'string', description: 'an id for the item (lowercase kebab-case, unique in the scene)' },
      group: { type: 'string', description: 'an existing group to place it in: its path, or an item (path or id) inside it' },
      newGroup: { type: 'object', description: 'attributes of a new group around the item (class, data-*), as JSON on the command line' },
      foot: { type: 'boolean', default: false, description: "with --new-group: set its data-x and data-y to the item's foot" },
      layer: { type: 'string', description: 'a layer of the scene for the new item to join: its id, or its name (scene items lists them)' },
    },
    required: ['name', 'object'],
  },
  args: ['name', 'object'],
  mutates: true,
  examples: [
    'pixellab scene place hero-island pine --at 14,6,1',
    'pixellab scene place range-sprite boxes --px 40,12 --dry-run',
    'pixellab scene place hero-island crane-jib --px 40,-12 --level 4',
    'pixellab scene place hero-island crate --at 3,1,1 --new-group \'{"class": "enter", "data-enter": "object"}\' --foot',
    'pixellab scene place hero-island pine --at 3,1,1 --layer railway',
  ],
  async run(ctx, params) {
    atMostOne(params, ['at', 'px'], 'scene.place');
    if (params.level !== undefined && !params.px) throw new UsageError('scene.place: --level goes with --px; an item on a tile has its level in --at');
    if (params.foot && !params.newGroup) throw new UsageError('scene.place: --foot goes with --new-group');
    const name = await sceneName(ctx, params.name);
    if (!(await ctx.names('object')).includes(params.object)) throw new NotFoundError(`no object named ${params.object} (pixellab object list)`);
    return editOf(ctx)({ kind: 'scene', name }, async (/** @type {any} */ doc) => {
      const layer = params.layer === undefined ? null : layerOps.findLayer(doc, params.layer);
      let at = params.px ? { px: params.px, level: params.level } : params.at ? { tile: params.at } : null;
      if (!at) {
        // SceneMode/Lab.open: the cursor starts on the tile at the middle of the view.
        const [x, y, w, h] = doc.viewBox;
        at = { tile: ctx.site.iso.pxToTile([Math.floor(x + w / 2), Math.floor(y + h / 2)], 0, doc.origin ?? [0, 0]) };
      }
      /** @type {{ path: number[], message: string, group?: number[] }} */
      let r;
      if (params.group !== undefined || params.newGroup) {
        const group = params.group !== undefined ? ops.groupPathOf(ctx.site, doc, params.group) : null;
        if (params.newGroup) ops.setGroupAttrs({ items: [{ group: {}, items: [] }] }, [0], params.newGroup);
        r = ops.placeInGroup(doc, params.object, at, { group, newGroup: params.newGroup ?? null });
      } else r = ops.place(doc, params.object, at);
      if (params.class) ops.setClass(doc, r.path, params.class);
      if (params.id) ops.setId(ctx.site, doc, r.path, params.id);
      if (layer) layerOps.setItemLayer(ops.itemAt(doc, r.path), layer.id);
      if (params.foot && r.group) {
        const { sources } = await ctx.sources();
        const [fx, fy] = ops.footOf(ctx.site, sources, ops.itemAt(doc, r.path), doc.origin ?? [0, 0]);
        ops.setGroupAttrs(doc, r.group, { 'data-x': String(fx), 'data-y': String(fy) });
      }
      return {
        message: layer ? `${r.message} in ${layer.name}` : r.message,
        path: pathString(r.path),
        item: structuredClone(ops.itemAt(doc, r.path)),
        ...(r.group ? { group: { path: pathString(r.group), attrs: { ...ops.itemAt(doc, r.group).group } } } : {}),
      };
    });
  },
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const move = {
  name: 'scene.move',
  summary: 'Move an item by tiles and levels (--by) or by pixels (--nudge)',
  description:
    'As the lab\'s arrow keys: --by col,row[,level] moves an item on a tile by whole tiles (a pixel-placed item moves by the same screen offset, ' +
    'and its level by the same levels, so it stands over the same tile a level higher); ' +
    '--nudge x,y moves it by pixels, and an item on a tile is then placed by pixel at the level it stood on. With --time past 0, an animated item (one with an id ' +
    'and a motion track) moves at that time instead: the key there, made if needed, takes the move. Rapid repeats coalesce into one undo step. ' +
    "A move at rest keeps the item's group in step: data-x/data-y that were its placement point or foot, and data-col/data-row that were its tile, " +
    'follow it (--no-keep-group leaves them); the result has the group when they did. The scene is then put in depth order: the result has the item\'s path after that, ' +
    'and reordered says which other paths moved.',
  params: itemParams({
    keepGroup: KEEP_GROUP,
    by: { type: 'array', items: { type: 'integer' }, minItems: 2, maxItems: 3, description: 'tiles to move: col,row[,level]' },
    nudge: { type: 'array', items: { type: 'integer' }, minItems: 2, maxItems: 2, description: 'pixels to move: x,y' },
    time: { type: 'integer', minimum: 0, default: 0, description: 'ms into the scene\'s motion; past 0 an animated item\'s key moves' },
  }),
  args: ['name', 'item'],
  mutates: true,
  examples: ['pixellab scene move hero-island 0.0 --by 1,0', 'pixellab scene move hero-island jib --nudge 0,-1 --dry-run'],
  async run(ctx, params) {
    needOne(params, ['by', 'nudge'], 'scene.move');
    atMostOne(params, ['by', 'nudge'], 'scene.move');
    const { by, nudge, time } = params;
    return moveEdit(
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
  summary: "Set an item's tile or pixel point, one coordinate, its level, depth override, class or id",
  description:
    '--tile col,row,level or --px x,y place the item there (changing how it\'s placed if need be; a pixel point keeps the level the item stands on); ' +
    '--col, --row, --level (an item on a tile) and --x, --y (a pixel-placed item) set one coordinate, like the lab\'s Selected fields. ' +
    '--level also sets how high a pixel-placed item stands, which only matters for depth order. ' +
    '--depth is the override: front draws the item over everything else in its list, back under everything, auto takes the override off ' +
    "so it draws by where it stands. It also takes a group's path, as the only field (a group's attributes are scene.group.set). " +
    '--class sets the class list and --id the id; an empty value removes it. An id\'s motion track follows it when it\'s renamed and goes with it ' +
    "when it's removed. All in one undo step, after which the scene is in depth order (the result has the item's path after that; reordered says what moved). " +
    "Moving it keeps its group's data-x/data-y and data-col/data-row in step, as scene.move does (--no-keep-group leaves them).",
  params: itemParams({
    keepGroup: KEEP_GROUP,
    tile: TILE,
    px: PX,
    col: { type: 'integer' },
    row: { type: 'integer' },
    level: { type: 'integer', description: 'the level of an item on a tile, or how high a pixel-placed item stands' },
    x: { type: 'integer' },
    y: { type: 'integer' },
    depth: { type: 'string', enum: ['front', 'back', 'auto'], description: 'front or back draws it over or under everything else in its list; auto clears the override' },
    class: { type: 'string', description: "a class list, or '' to remove it" },
    id: { type: 'string', description: "an id, or '' to remove it" },
  }),
  args: ['name', 'item'],
  mutates: true,
  examples: ['pixellab scene set hero-island 0.0 --tile 3,4,0', "pixellab scene set hero-island 0.0 --class 'sway slow'", 'pixellab scene set hero-island 140 --depth back'],
  async run(ctx, params) {
    const fields = /** @type {const} */ (['col', 'row', 'level', 'x', 'y']);
    needOne(params, ['tile', 'px', ...fields, 'depth', 'class', 'id'], 'scene.set');
    atMostOne(params, ['tile', 'px'], 'scene.set');
    const others = [...fields, 'tile', 'px', 'class', 'id'].filter((k) => params[k] !== undefined);
    if (params.depth !== undefined && !others.length) {
      // The override is the one field a group takes.
      return itemEdit(ctx, /** @type {any} */ (params), (doc, path) => ops.setDepth(doc, path, params.depth));
    }
    try {
      return await setEdit(ctx, params, fields);
    } catch (err) {
      // Only the override applies to a group, and the rest is scene.group.set's.
      if (err instanceof ops.SceneOpError && err.code === 'is_group') {
        throw new UsageError(`scene.set: ${params.item} is a group; only --depth applies to one (its attributes are scene.group.set)`);
      }
      throw err;
    }
  },
  format: formatChange,
};

/**
 * scene.set's edit of a placed item.
 * @param {any} ctx
 * @param {Record<string, any>} params
 * @param {readonly ('col' | 'row' | 'level' | 'x' | 'y')[]} fields
 */
function setEdit(ctx, params, fields) {
  return moveEdit(ctx, /** @type {any} */ (params), (doc, path) => {
    /** @type {string[]} */
    const said = [];
    let r;
    if (params.tile || params.px) said.push((r = ops.setAt(doc, path, params.tile ? { tile: params.tile } : { px: params.px })).message);
    for (const f of fields) if (params[f] !== undefined) r = ops.setField(doc, path, f, params[f]);
    if (r && !params.tile && !params.px) said.push(r.message);
    if (params.depth !== undefined) said.push((r = ops.setDepth(doc, path, params.depth)).message);
    if (params.class !== undefined) said.push((r = ops.setClass(doc, path, params.class)).message);
    if (params.id !== undefined) said.push((r = ops.setId(ctx.site, doc, path, params.id)).message);
    return { path, item: ops.itemAt(doc, path), message: said.join('; ') };
  });
}

/** @type {import('../registry.mjs').Command} */
const duplicate = {
  name: 'scene.duplicate',
  summary: 'Duplicate an item one column over, where the copy stands in depth order',
  description:
    "As the lab's Duplicate: the copy is moved one column (a tile, or the same screen offset by pixel) and takes no id, so no motion track. " +
    "It is listed where it sorts, which is usually next to the original; the result has its path after the sort.",
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
  summary: 'Snap a pixel-placed item to the tile under its placement point, at the level it stands on',
  description: "A pixel item with no level snaps to a level-0 tile. Its group's data-x/data-y and data-col/data-row follow it, as scene.move keeps them (--no-keep-group leaves them).",
  params: itemParams({ keepGroup: KEEP_GROUP }),
  args: ['name', 'item'],
  mutates: true,
  async run(ctx, params) {
    return moveEdit(ctx, /** @type {any} */ (params), (doc, path) => ops.snapItem(ctx.site, doc, path));
  },
  format: formatChange,
};

/**
 * How many pixels differ, at time 0, between a scene drawn in the order its
 * file lists its items and in depth order (what the site sees when it adopts
 * this engine: the published render no longer depends on the list's order).
 * @param {any} ctx
 * @param {any} sources
 * @param {string} name
 */
function changedPixels(ctx, sources, name) {
  const { engine } = ctx.site;
  const draw = (/** @type {'list' | 'depth'} */ order) => engine.composite(engine.renderScene(sources, name, { order }).root, { firstVariant: true, time: 0 });
  const [was, now] = [draw('list'), draw('depth')];
  let n = 0;
  for (const [p, color] of now) if (was.get(p) !== color) n++;
  for (const p of was.keys()) if (!now.has(p)) n++;
  return n;
}

/** @type {import('../registry.mjs').Command} */
const sort = {
  name: 'scene.sort',
  summary: "Put a scene's items (or every scene's) in depth order, and say what moved",
  description:
    'Every edit to a scene already keeps its items in depth order (back to front, by where each stands); this is for a scene whose file isn\'t, ' +
    'such as one written by hand or made before depth order. A group sorts as one object and its members sort inside it. One undo step per scene. ' +
    'Returns { moved: [{ scene, path, to, object | group }], changedPixels, scenes: [{ scene, moved, changedPixels }] }: moved lists the items the sort took out ' +
    'of their place (paths before and after), and changedPixels counts the pixels that differ at time 0 between the scene drawn in its file\'s list order ' +
    '(as engines before depth order publish it) and in depth order, which is what the site sees when it adopts this engine. ' +
    'Give a scene or --all. For one scene, reordered maps every path the sort changed to its new path. ' +
    'With --dry-run on the command line nothing is written and the diff shows the new order.',
  params: {
    type: 'object',
    properties: {
      name: { ...NAME, description: 'scene name (or scenes/<name>); leave out with --all' },
      all: { type: 'boolean', default: false, description: 'sort every scene' },
    },
  },
  args: ['name'],
  mutates: true,
  examples: ['pixellab scene sort hero-island --dry-run', 'pixellab scene sort --all'],
  async run(ctx, params) {
    if (params.all && params.name !== undefined) throw new UsageError('scene.sort: give a scene or --all, not both');
    if (!params.all && params.name === undefined) throw new UsageError('scene.sort: give a scene, or --all for every scene');
    const names = params.all ? await ctx.names('scene') : [await sceneName(ctx, params.name)];
    /** @type {{ scene: string, path: string, to: string, object?: string, group?: true }[]} */
    const moved = [];
    /** @type {{ scene: string, moved: number, changedPixels: number }[]} */
    const scenes = [];
    /** @type {string[]} */
    const docs = [];
    /** @type {Record<string, string>} */
    let reordered = {};
    for (const name of names) {
      const r = await editOf(ctx)({ kind: 'scene', name }, async (/** @type {any} */ doc) => {
        const { sources } = await ctx.sources();
        let pixels = 0;
        try {
          pixels = changedPixels(ctx, sources, name);
        } catch {
          // a scene that doesn't draw (a missing object) can't be compared; the sort leaves it alone
        }
        const sorted = ops.sortScene(ctx.site, sources, doc);
        return { message: `Sorted ${name} by depth: ${sorted.moved.length} item${sorted.moved.length === 1 ? '' : 's'} moved`, scene: name, sorted, pixels };
      });
      const { sorted, pixels } = /** @type {any} */ (r);
      if (r.changed) docs.push(r.doc);
      reordered = Object.fromEntries(sorted.remap);
      moved.push(...sorted.moved.map((/** @type {any} */ m) => ({ scene: name, ...m })));
      scenes.push({ scene: name, moved: sorted.moved.length, changedPixels: pixels });
    }
    const total = moved.length;
    const pixels = scenes.reduce((n, s) => n + s.changedPixels, 0);
    const sortedScenes = scenes.filter((s) => s.moved);
    return {
      changed: docs.length > 0,
      summary: total
        ? `Sorted ${sortedScenes.length === 1 ? sortedScenes[0].scene : `${sortedScenes.length} scenes`} by depth: ${total} item${total === 1 ? '' : 's'} moved, ${pixels} pixel${pixels === 1 ? '' : 's'} change${pixels === 1 ? 's' : ''}`
        : `Already in depth order${params.all ? '' : `: ${names[0]}`}`,
      moved,
      changedPixels: pixels,
      scenes,
      // Every path the sort changed, for a caller holding paths into the one scene sorted.
      ...(params.all ? {} : { reordered }),
    };
  },
  format: (r) => {
    const lines = r.scenes.filter((/** @type {any} */ s) => s.moved).map((/** @type {any} */ s) => `${s.scene}: ${s.moved} moved, ${s.changedPixels} pixels change`);
    return [r.summary, ...lines].join('\n');
  },
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

/** @type {import('../registry.mjs').Command} */
const groupSet = {
  name: 'scene.group.set',
  summary: "Set or remove a group's class and data attributes, or set its data-x and data-y to its first item's foot",
  description:
    "--group is the group's path (scene items lists groups), or an item (path or id) inside it. --attrs is an object of attribute values " +
    "({\"data-enter\": \"object\", \"data-x\": \"96\"}); '' removes one. --foot sets data-x and data-y to the foot of the group's first item: " +
    "an item on a tile stands on its tile's center, a sprite placed by pixel on the center of its bottom row. One undo step.",
  params: {
    type: 'object',
    properties: {
      name: NAME,
      group: { type: 'string', description: "the group: its path, or an item (path or id) inside it" },
      attrs: { type: 'object', description: "attribute values; '' removes one (JSON on the command line)" },
      foot: { type: 'boolean', default: false, description: "set data-x and data-y to the first item's foot" },
    },
    required: ['name', 'group'],
  },
  args: ['name', 'group'],
  mutates: true,
  examples: ['pixellab scene group set hero-island 42 --foot', 'pixellab scene group set hero-island truck-a --attrs \'{"data-enter": "object"}\''],
  async run(ctx, params) {
    if (!params.attrs && !params.foot) throw new UsageError('scene.group.set: give --attrs, --foot or both');
    const name = await sceneName(ctx, params.name);
    return editOf(ctx)({ kind: 'scene', name }, async (/** @type {any} */ doc) => {
      const path = ops.groupPathOf(ctx.site, doc, params.group);
      const attrs = { ...(params.attrs ?? {}) };
      if (params.foot) {
        const first = ops.firstPlaced(ops.itemAt(doc, path).items ?? []);
        if (!first) throw new ops.SceneOpError(`group ${pathString(path)} has no items to take a foot from`, 'empty_group');
        const { sources } = await ctx.sources();
        const [fx, fy] = ops.footOf(ctx.site, sources, first.item, doc.origin ?? [0, 0]);
        Object.assign(attrs, { 'data-x': String(fx), 'data-y': String(fy) });
      }
      const r = ops.setGroupAttrs(doc, path, attrs);
      return { message: r.message, group: { path: pathString(path), attrs: { ...r.group.group } } };
    });
  },
  format: formatChange,
};

export default [place, move, set, duplicate, remove, snap, sort, create, groupSet];
