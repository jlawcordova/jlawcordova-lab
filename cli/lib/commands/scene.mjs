// Read-only scene commands: scene.list, scene.show, scene.items, scene.render.

import { UsageError } from '../errors.mjs';
import { flattenItems, MAX_SCALE, parsePath, renderSceneImage } from '../render.mjs';

const NAME = { type: 'string', description: 'scene name (or scenes/<name>)' };

export const RENDER_PARAMS = {
  scale: { type: 'integer', minimum: 1, maximum: MAX_SCALE, default: 1, description: 'whole-pixel zoom of the PNG' },
  time: { type: 'integer', minimum: 0, description: 'ms into the animation: frame loops and motion tracks (default 0, the rest frame)' },
};

const countItems = (/** @type {any[]} */ items) =>
  (items ?? []).reduce((n, item) => n + (item && typeof item === 'object' && 'group' in item ? countItems(item.items) : 1), 0);

/** The scene's data, by name or scenes/<name>. */
async function sceneDoc(/** @type {any} */ ctx, /** @type {string} */ query) {
  const { name } = await ctx.find(query, 'scene');
  return { name, scene: await ctx.doc('scene', name) };
}

const at = (/** @type {any} */ a) => (a?.tile ? `tile ${a.tile.join(',')}` : a?.px ? `px ${a.px.join(',')}` : '?');

/** @type {import('../registry.mjs').Command} */
const list = {
  name: 'scene.list',
  summary: 'List every scene',
  params: { type: 'object', properties: {} },
  args: [],
  mutates: false,
  async run(ctx) {
    const { sources } = await ctx.sources();
    const scenes = [...sources.scenes].map(([name, scene]) => ({
      name,
      file: `scenes/${name}.mjs`,
      output: scene.output ?? null,
      viewBox: scene.viewBox,
      origin: scene.origin ?? [0, 0],
      items: countItems(scene.items),
      motion: Boolean(scene.motion),
    }));
    return { scenes };
  },
  format: (r) => {
    const w = Math.max(4, ...r.scenes.map((/** @type {any} */ s) => s.name.length));
    return r.scenes
      .map((/** @type {any} */ s) => {
        const [, , vw, vh] = s.viewBox ?? [0, 0, 0, 0];
        return `${s.name.padEnd(w)}  ${`${vw}×${vh}`.padEnd(9)} ${String(s.items).padStart(4)} items  ${s.output ?? '(preview only)'}${s.motion ? '  · motion' : ''}`;
      })
      .join('\n');
  },
};

/** @type {import('../registry.mjs').Command} */
const show = {
  name: 'scene.show',
  summary: "Show a scene's settings, size, objects and motion",
  params: {
    type: 'object',
    properties: {
      name: NAME,
      doc: { type: 'boolean', default: false, description: "also return the scene's source data as doc" },
    },
    required: ['name'],
  },
  args: ['name'],
  mutates: false,
  async run(ctx, { name: query, doc }) {
    const { name, scene } = await sceneDoc(ctx, query);
    const { items, groups } = flattenItems(ctx.site, scene);
    /** @type {Record<string, number>} */
    const objects = {};
    for (const item of items) objects[item.object] = (objects[item.object] ?? 0) + 1;
    return {
      name,
      file: `scenes/${name}.mjs`,
      output: scene.output ?? null,
      viewBox: scene.viewBox,
      origin: scene.origin ?? [0, 0],
      items: items.length,
      groups: groups.length,
      objects,
      motion: scene.motion
        ? { duration: scene.motion.duration, tracks: (scene.motion.tracks ?? []).map((/** @type {any} */ t) => ({ item: t.item, keys: t.keys?.length ?? 0 })) }
        : null,
      ...(doc ? { doc: scene } : {}),
    };
  },
  format: (r) => {
    const [vx, vy, vw, vh] = r.viewBox ?? [];
    const lines = [
      `${r.name}  (${r.file})`,
      `output    ${r.output ?? '(preview only)'}`,
      `viewBox   ${vx}, ${vy}, ${vw}×${vh}`,
      `origin    ${r.origin.join(', ')}`,
      `items     ${r.items} in ${r.groups} group${r.groups === 1 ? '' : 's'}`,
      `objects   ${Object.keys(r.objects).length}: ${Object.entries(r.objects).map(([o, n]) => (n > 1 ? `${o} ×${n}` : o)).join(', ')}`,
    ];
    if (r.motion) lines.push(`motion    ${r.motion.duration} ms, ${r.motion.tracks.length} track${r.motion.tracks.length === 1 ? '' : 's'}: ${r.motion.tracks.map((/** @type {any} */ t) => t.item).join(', ')}`);
    return lines.join('\n');
  },
};

/** @type {import('../registry.mjs').Command} */
const items = {
  name: 'scene.items',
  summary: "List a scene's items in paint order, with stable paths",
  description:
    'Paths index into the scene\'s items: "3" is the fourth top-level item, "5.2" the third item inside the group at "5". ' +
    'Each item has its object, at ({ tile: [col, row, level] } or { px: [x, y] }), class, id, enclosing group and placement point in scene pixels.',
  params: { type: 'object', properties: { name: NAME }, required: ['name'] },
  args: ['name'],
  mutates: false,
  async run(ctx, { name: query }) {
    const { name, scene } = await sceneDoc(ctx, query);
    return { scene: name, ...flattenItems(ctx.site, scene) };
  },
  format: (r) => {
    const w = Math.max(4, ...r.items.map((/** @type {any} */ i) => i.path.length));
    const o = Math.max(6, ...r.items.map((/** @type {any} */ i) => String(i.object).length));
    return r.items
      .map((/** @type {any} */ i) => {
        const extra = [i.id ? `#${i.id}` : '', i.class ? `.${i.class.replace(/ /g, '.')}` : ''].filter(Boolean).join(' ');
        return `${i.path.padEnd(w)}  ${String(i.object).padEnd(o)}  ${at(i.at).padEnd(16)}${extra ? ` ${extra}` : ''}`.trimEnd();
      })
      .join('\n');
  },
};

/** @type {import('../registry.mjs').Command} */
const render = {
  name: 'scene.render',
  summary: 'Render a scene to a transparent PNG, with item bounds',
  description:
    'Returns { png (base64), scale, viewBox, width, height, time, items: [{ path, object, at, id, bounds: [x, y, w, h] }] }. ' +
    'Bounds are in output pixels (scale included), for hit-testing. Of each run of data-class variant groups only the first shows, ' +
    'or the one holding --show. On the command line, --out <file.png> writes the PNG.',
  params: {
    type: 'object',
    properties: {
      name: NAME,
      ...RENDER_PARAMS,
      show: { type: 'string', pattern: '^\\d+(\\.\\d+)*$', description: 'an item path whose variant group to show, like 12.0' },
    },
    required: ['name'],
  },
  args: ['name'],
  mutates: false,
  writesPng: true,
  examples: ['pixellab scene render hero-island --scale 2 --out /tmp/hero.png', 'pixellab scene render range-sprite --time 400 --json'],
  async run(ctx, { name: query, scale, time = 0, show }) {
    const { name } = await ctx.find(query, 'scene');
    await ctx.doc('scene', name);
    const path = show === undefined ? null : parsePath(show);
    if (show !== undefined && !path) throw new UsageError(`--show ${show}: give an item path like 12 or 5.2`);
    const { sources } = await ctx.sources();
    return { scene: name, ...renderSceneImage(ctx.site, sources, name, { scale, time, show: path }) };
  },
  format: (r) => `${r.scene}: ${r.width}×${r.height} PNG at ${r.scale}×, ${r.items.length} items shown, time ${r.time} ms`,
};

export default [list, show, items, render];
