// Object edit commands: paint, erase, fill and pick pixels, layers, frames,
// loops, resize, new and whole maps (object.map.set,
// docs/intents/2026-10-mcp-gaps/spec.md D9). Every change goes through the engine's edit.mjs, as
// the web lab's Object mode does (src/components/lab/object-mode.ts,
// layers-panel.ts, frames-panel.ts, palette-panel.ts), so the source stays
// in the shape a person would write by hand.
//
// Pixel coordinates are those of object.render's PNG at scale 1: a sprite's
// map column and row; for a block, the PNG pixel of its render (whose
// viewBox starts at the block's top-left pixel), as the lab's stage shows it.

import { CommandError, UsageError } from '../errors.mjs';
import { editOf, formatChange } from './common.mjs';
import { checkNewName } from './scene-edit.mjs';

const NAME = { type: 'string', description: 'object name (or objects/<name>)' };
const LAYER = { type: 'integer', minimum: 0, default: 0, description: 'the layer, from 0 at the bottom' };
const FRAME = { type: 'integer', minimum: 0, default: 0, description: 'the frame of a frame loop (a block: of its surface loop)' };
const PIXELS = {
  type: 'array',
  items: { type: 'array', items: { type: 'integer' }, minItems: 2, maxItems: 2 },
  minItems: 1,
  description: 'pixels as [x, y] pairs; on the command line x,y pairs separated by spaces (or the flag repeated)',
};
const COLOR = { type: 'string', description: 'a palette color name (pixellab palette get)' };

/** The object's name, checked, its resolved form and the sources it's in. */
async function objectAt(/** @type {any} */ ctx, /** @type {string} */ query) {
  const { name } = await ctx.find(query, 'object');
  await ctx.doc('object', name);
  return name;
}

/** The engine's resolve, as a command error when the object doesn't resolve. */
function resolved(/** @type {any} */ ctx, /** @type {any} */ sources, /** @type {string} */ name) {
  try {
    return ctx.site.engine.resolve(sources, name);
  } catch (err) {
    throw new CommandError(`object ${name} doesn't resolve: ${/** @type {Error} */ (err).message}; fix the problems first (pixellab check ${name})`, { code: 'resolve_error' });
  }
}

/**
 * Whether the object may paint a color: its tier, then the colors cap.
 * Port of palette-panel.ts allowed and whyNot.
 * @param {any} ctx
 * @param {any} sources
 * @param {any} r  resolved
 * @param {string} color
 */
function checkColor(ctx, sources, r, color) {
  const { engine } = ctx.site;
  const entry = sources.colors.get(color);
  if (!entry) throw new UsageError(`no palette color named ${color} (pixellab palette get)`);
  if (entry.tier === 'outfit' && !r.character) throw new CommandError(`${color} is an outfit color; only objects that extend character may use it`, { code: 'color_not_allowed' });
  const used = new Set(engine.usedColors(sources, r));
  if (!used.has(entry.hex) && used.size >= engine.CAPS.colors) {
    throw new CommandError(`This object already uses ${engine.CAPS.colors} colors. Pick one it uses, or free one up first.`, { code: 'color_cap' });
  }
}

/**
 * Why painting is off for a layer, or null. Port of ObjectMode.paintBlocked.
 * @param {any} doc
 * @param {any} r
 * @param {number} layer
 */
function paintBlocked(doc, r, layer) {
  if (doc.kind === 'block') return null;
  const source = r.layers[layer];
  if (!source) return `there's no layer ${layer}; the object has ${r.layers.length} (0 to ${r.layers.length - 1})`;
  if (doc.extends && 'frames' in source) return `layer ${layer} is a frame loop of ${doc.extends}, which an outfit can't override`;
  return null;
}

/** A block's top-left pixel at a frame: where its render's PNG starts (ObjectMode.pixelsAt's blockOrigin). */
function blockCorner(/** @type {any} */ ctx, /** @type {any} */ sources, /** @type {string} */ name, /** @type {number} */ frame) {
  const { engine } = ctx.site;
  const pixels = engine.composite(engine.renderObject(sources, name), { frame });
  const box = engine.bounds(pixels.keys());
  return { pixels, minX: box?.minX ?? 0, minY: box?.minY ?? 0 };
}

/**
 * Paints (or clears, color null) pixels as one undo step. Port of
 * ObjectMode.paintPixels: edit.paintBlock for a block, edit.paint for a sprite.
 * @param {any} ctx
 * @param {string} name
 * @param {{ layer: number, frame: number }} where
 * @param {(r: any, sources: any) => [number, number][]} pixelsOf  the pixels, given the resolved object
 * @param {string | null} color
 * @param {string} verb
 * @param {string | null} key
 */
async function paintEdit(ctx, name, { layer, frame }, pixelsOf, color, verb, key) {
  return editOf(ctx)(
    { kind: 'object', name },
    async (/** @type {any} */ doc) => {
      const { sources } = await ctx.sources();
      const r = resolved(ctx, sources, name);
      const blocked = paintBlocked(doc, r, layer);
      if (blocked) throw new CommandError(blocked, { code: 'paint_blocked' });
      if (color !== null) checkColor(ctx, sources, r, color);
      const pixels = pixelsOf(r, sources);
      const { edit } = ctx.site;
      let count;
      if (doc.kind === 'block') {
        const { minX, minY } = blockCorner(ctx, sources, name, frame);
        count = edit.paintBlock(doc, frame, pixels.map(([x, y]) => [x + minX, y + minY]), color);
      } else count = edit.paint(sources, name, doc, { layer, frame }, pixels, color);
      const n = `${count} pixel${count === 1 ? '' : 's'}`;
      const message = count === 0 ? `Nothing changed` : color === null ? `Erased ${n}` : `${verb} ${n} ${color}`;
      return { message, pixels: count };
    },
    { key },
  );
}

/** @type {import('../registry.mjs').Command} */
const paint = {
  name: 'object.paint',
  summary: 'Paint pixels of an object a palette color, on a layer and frame',
  description:
    'Pixels are object.render coordinates at scale 1 (a sprite\'s map column and row). A new color gets a key, as the lab gives one. ' +
    'An object that extends another paints row overrides; a block paints its top surface and side maps. ' +
    'Outfit colors are only for objects that extend character, and an object paints at most 12 colors. Rapid repeats coalesce into one undo step.',
  params: { type: 'object', properties: { name: NAME, pixels: PIXELS, color: COLOR, layer: LAYER, frame: FRAME }, required: ['name', 'pixels', 'color'] },
  args: ['name', 'pixels'],
  mutates: true,
  examples: ['pixellab object paint pine 3,4 4,4 --color grass-2', 'pixellab object paint pine --pixels 3,4 --color ink --layer 1'],
  async run(ctx, { name: query, pixels, color, layer, frame }) {
    const name = await objectAt(ctx, query);
    return paintEdit(ctx, name, { layer, frame }, () => pixels, color, 'Painted', `object.paint:${layer}:${frame}:${color}`);
  },
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const erase = {
  name: 'object.erase',
  summary: 'Erase pixels of an object (make them transparent), on a layer and frame',
  params: { type: 'object', properties: { name: NAME, pixels: PIXELS, layer: LAYER, frame: FRAME }, required: ['name', 'pixels'] },
  args: ['name', 'pixels'],
  mutates: true,
  async run(ctx, { name: query, pixels, layer, frame }) {
    const name = await objectAt(ctx, query);
    return paintEdit(ctx, name, { layer, frame }, () => pixels, null, 'Erased', `object.erase:${layer}:${frame}`);
  },
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const fill = {
  name: 'object.fill',
  summary: 'Flood-fill from a pixel with a palette color, on a layer and frame',
  description:
    "As the lab's Fill: the 4-connected pixels that share the start pixel's key (edit.floodFill); on a block, those of the start pixel's color " +
    'on the same face (edit.blockFill).',
  params: {
    type: 'object',
    properties: { name: NAME, x: { type: 'integer' }, y: { type: 'integer' }, color: COLOR, layer: LAYER, frame: FRAME },
    required: ['name', 'x', 'y', 'color'],
  },
  args: ['name'],
  mutates: true,
  examples: ['pixellab object fill pine --x 3 --y 4 --color grass-1'],
  async run(ctx, { name: query, x, y, color, layer, frame }) {
    const name = await objectAt(ctx, query);
    const { edit } = ctx.site;
    return paintEdit(
      ctx,
      name,
      { layer, frame },
      (r, sources) => {
        if (!r.block) return edit.floodFill(edit.mapAt(r.layers[layer], frame), x, y);
        // ObjectMode.fillFrom: in block coordinates, then back to the render's.
        const { pixels, minX, minY } = blockCorner(ctx, sources, name, frame);
        return edit.blockFill(edit.blockCells(r.block.size), pixels, x + minX, y + minY).map(([px, py]) => [px - minX, py - minY]);
      },
      color,
      'Filled',
      null,
    );
  },
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const pick = {
  name: 'object.pick',
  summary: 'The color at a pixel of an object, on a layer and frame (the Picker)',
  description: "A sprite's color on that layer; a block's as it's drawn. Returns { color, hex }, both null for a transparent pixel.",
  params: { type: 'object', properties: { name: NAME, x: { type: 'integer' }, y: { type: 'integer' }, layer: LAYER, frame: FRAME }, required: ['name', 'x', 'y'] },
  args: ['name'],
  mutates: false,
  async run(ctx, { name: query, x, y, layer, frame }) {
    const name = await objectAt(ctx, query);
    const { sources } = await ctx.sources();
    const r = resolved(ctx, sources, name);
    let color;
    if (r.block) {
      const { pixels, minX, minY } = blockCorner(ctx, sources, name, frame);
      color = pixels.get(`${x + minX},${y + minY}`) ?? null;
    } else {
      if (!r.layers[layer]) throw new UsageError(`there's no layer ${layer}; the object has ${r.layers.length}`);
      color = ctx.site.edit.colorAt(r, { layer, frame }, x, y);
    }
    return { object: name, x, y, layer, frame, color, hex: color ? (sources.colors.get(color)?.hex ?? null) : null };
  },
  format: (r) => `x ${r.x}, y ${r.y}: ${r.color ? `${r.color} ${r.hex}` : 'empty'}`,
};

/**
 * An edit to an object with its own layers (layers-panel.ts and
 * frames-panel.ts `editable`: a sprite that doesn't extend another).
 * @param {any} ctx
 * @param {string} query
 * @param {(doc: any) => { message: string, [k: string]: unknown }} fn
 */
async function layerEdit(ctx, query, fn) {
  const name = await objectAt(ctx, query);
  return editOf(ctx)({ kind: 'object', name }, (/** @type {any} */ doc) => {
    if (doc.kind !== 'sprite') throw new CommandError(`${name} is a block; a block has no layers or frames to change`, { code: 'not_editable' });
    if (doc.extends) throw new CommandError(`${name} extends ${doc.extends} and takes its layers from it; paint row overrides instead`, { code: 'not_editable' });
    return fn(doc);
  });
}

const layerOf = (/** @type {any} */ doc, /** @type {number} */ layer) => {
  if (!doc.layers[layer]) throw new UsageError(`there's no layer ${layer}; the object has ${doc.layers.length} (0 to ${doc.layers.length - 1})`);
  return doc.layers[layer];
};

const loopOf = (/** @type {any} */ doc, /** @type {number} */ layer) => {
  const l = layerOf(doc, layer);
  if (!('frames' in l)) throw new CommandError(`layer ${layer} is a still map, not a frame loop (object.animate makes it one)`, { code: 'not_a_loop' });
  return l;
};

/** Moves a list item by `by` steps (edit.move one step at a time); refuses past either end. */
function moveBy(/** @type {any} */ edit, /** @type {any[]} */ list, /** @type {number} */ index, /** @type {number} */ by, /** @type {string} */ what) {
  const target = index + by;
  if (by === 0) throw new UsageError('--by must not be 0');
  if (target < 0 || target >= list.length) throw new CommandError(`${what} ${index} can't move ${by > 0 ? 'up' : 'down'} ${Math.abs(by)}: there are ${list.length} (0 to ${list.length - 1})`, { code: 'out_of_range' });
  let i = index;
  while (i !== target) i = edit.move(list, i, by > 0 ? 1 : -1);
  return i;
}

/** @type {import('../registry.mjs').Command[]} */
const layerCommands = [
  {
    name: 'object.layer.add',
    summary: 'Add a blank layer on top of an object',
    params: { type: 'object', properties: { name: NAME }, required: ['name'] },
    args: ['name'],
    mutates: true,
    run: (ctx, { name }) =>
      layerEdit(ctx, name, (doc) => {
        const layer = ctx.site.edit.addLayer(doc);
        return { message: `Added layer ${layer}`, layer };
      }),
    format: formatChange,
  },
  {
    name: 'object.layer.duplicate',
    summary: 'Duplicate a layer, just above it',
    params: { type: 'object', properties: { name: NAME, layer: LAYER }, required: ['name'] },
    args: ['name', 'layer'],
    mutates: true,
    run: (ctx, { name, layer }) =>
      layerEdit(ctx, name, (doc) => {
        layerOf(doc, layer);
        const next = ctx.site.edit.duplicateLayer(doc, layer);
        return { message: `Layer ${layer} duplicated as layer ${next}`, layer: next };
      }),
    format: formatChange,
  },
  {
    name: 'object.layer.delete',
    summary: 'Delete a layer (not the only one)',
    params: { type: 'object', properties: { name: NAME, layer: LAYER }, required: ['name', 'layer'] },
    args: ['name', 'layer'],
    mutates: true,
    run: (ctx, { name, layer }) =>
      layerEdit(ctx, name, (doc) => {
        layerOf(doc, layer);
        if (doc.layers.length === 1) throw new CommandError(`layer ${layer} is the only layer, so it stays`, { code: 'last_layer' });
        const next = ctx.site.edit.deleteLayer(doc, layer);
        return { message: `Layer ${layer} deleted`, layer: next };
      }),
    format: formatChange,
  },
  {
    name: 'object.layer.move',
    summary: 'Move a layer up (--by 1, raise) or down (--by -1, lower)',
    params: { type: 'object', properties: { name: NAME, layer: LAYER, by: { type: 'integer', description: 'steps up (positive) or down (negative)' } }, required: ['name', 'layer', 'by'] },
    args: ['name', 'layer'],
    mutates: true,
    run: (ctx, { name, layer, by }) =>
      layerEdit(ctx, name, (doc) => {
        layerOf(doc, layer);
        const next = moveBy(ctx.site.edit, doc.layers, layer, by, 'layer');
        return { message: `Layer ${layer} ${by > 0 ? 'raised' : 'lowered'} to layer ${next}`, layer: next };
      }),
    format: formatChange,
  },
];

const frameParams = (/** @type {Record<string, any>} */ more = {}) => ({
  type: 'object',
  properties: { name: NAME, layer: LAYER, frame: FRAME, ...more },
  required: ['name'],
});

/** @type {import('../registry.mjs').Command[]} */
const frameCommands = [
  {
    name: 'object.frame.add',
    summary: "Add a blank frame after a frame of a layer's loop",
    params: frameParams(),
    args: ['name'],
    mutates: true,
    run: (ctx, { name, layer, frame }) =>
      layerEdit(ctx, name, (doc) => {
        const l = loopOf(doc, layer);
        if (!l.frames[frame]) throw new UsageError(`there's no frame ${frame}; the loop has ${l.frames.length}`);
        const next = ctx.site.edit.addFrame(doc, layer, frame);
        return { message: `Added a blank frame, ${l.prefix}${next}`, frame: next };
      }),
    format: formatChange,
  },
  {
    name: 'object.frame.duplicate',
    summary: 'Duplicate a frame of a loop, just after it',
    params: frameParams(),
    args: ['name'],
    mutates: true,
    run: (ctx, { name, layer, frame }) =>
      layerEdit(ctx, name, (doc) => {
        const l = loopOf(doc, layer);
        if (!l.frames[frame]) throw new UsageError(`there's no frame ${frame}; the loop has ${l.frames.length}`);
        const next = ctx.site.edit.duplicateFrame(doc, layer, frame);
        return { message: `Frame ${l.prefix}${frame} duplicated as ${l.prefix}${next}`, frame: next };
      }),
    format: formatChange,
  },
  {
    name: 'object.frame.delete',
    summary: 'Delete a frame of a loop (not the only one)',
    params: frameParams(),
    args: ['name'],
    mutates: true,
    run: (ctx, { name, layer, frame }) =>
      layerEdit(ctx, name, (doc) => {
        const l = loopOf(doc, layer);
        if (!l.frames[frame]) throw new UsageError(`there's no frame ${frame}; the loop has ${l.frames.length}`);
        if (l.frames.length === 1) throw new CommandError('A loop keeps at least one frame', { code: 'last_frame' });
        const next = ctx.site.edit.deleteFrame(doc, layer, frame);
        return { message: `Frame ${l.prefix}${frame} deleted`, frame: next };
      }),
    format: formatChange,
  },
  {
    name: 'object.frame.move',
    summary: 'Move a frame of a loop later (--by 1) or earlier (--by -1)',
    params: { ...frameParams({ by: { type: 'integer', description: 'steps later (positive) or earlier (negative)' } }), required: ['name', 'by'] },
    args: ['name'],
    mutates: true,
    run: (ctx, { name, layer, frame, by }) => {
      return layerEdit(ctx, name, (doc) => {
        const l = loopOf(doc, layer);
        if (!l.frames[frame]) throw new UsageError(`there's no frame ${frame}; the loop has ${l.frames.length}`);
        const next = moveBy(ctx.site.edit, l.frames, frame, by, 'frame');
        return { message: `Frame ${l.prefix}${frame} moved ${by > 0 ? 'later' : 'earlier'}, to ${l.prefix}${next}`, frame: next };
      });
    },
    format: formatChange,
  },
];

/** @type {import('../registry.mjs').Command} */
const animate = {
  name: 'object.animate',
  summary: "Turn a still layer into one of the site's frame loops, or switch a loop's kind",
  description:
    "As the lab's Animate this layer: the map becomes frame 0 and is copied into every other frame of the loop (edit.animateLayer). " +
    "On a layer that's already a loop, it plays as the other loop instead, gaining copies of its last frame if it needs more (edit.setLoop).",
  params: {
    type: 'object',
    properties: { name: NAME, layer: LAYER, loop: { type: 'string', enum: ['ff', 'wf', 'hf'], default: 'ff', description: 'ff: 4 frames, 600 ms; wf: 5, 1 s; hf: 6, 3 s' } },
    required: ['name'],
  },
  args: ['name'],
  mutates: true,
  run: (ctx, { name, layer, loop }) =>
    layerEdit(ctx, name, (doc) => {
      const l = layerOf(doc, layer);
      const { SITE_LOOPS } = ctx.site.motion;
      if ('frames' in l) ctx.site.edit.setLoop(doc, layer, loop);
      else ctx.site.edit.animateLayer(doc, layer, loop);
      return { message: `Layer ${layer} plays as ${loop}, ${SITE_LOOPS[loop].frames} frames`, frames: doc.layers[layer].frames.length };
    }),
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const still = {
  name: 'object.still',
  summary: 'Turn a frame loop back into a still layer, keeping one frame',
  params: frameParams(),
  args: ['name'],
  mutates: true,
  run: (ctx, { name, layer, frame }) =>
    layerEdit(ctx, name, (doc) => {
      const l = loopOf(doc, layer);
      if (!l.frames[frame]) throw new UsageError(`there's no frame ${frame}; the loop has ${l.frames.length}`);
      ctx.site.edit.stillLayer(doc, layer, frame);
      return { message: `Still, keeping frame ${frame}` };
    }),
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const resize = {
  name: 'object.resize',
  summary: "Pad or crop a sprite's maps to a width and height (at the right and bottom)",
  params: {
    type: 'object',
    properties: {
      name: NAME,
      width: { type: 'integer', minimum: 1, maximum: 64, description: 'the new width (default: as it is)' },
      height: { type: 'integer', minimum: 1, maximum: 64, description: 'the new height (default: as it is)' },
    },
    required: ['name'],
  },
  args: ['name'],
  mutates: true,
  async run(ctx, params) {
    if (params.width === undefined && params.height === undefined) throw new UsageError('object.resize: give --width, --height or both');
    const name = await objectAt(ctx, params.name);
    const { sources } = await ctx.sources();
    const r = resolved(ctx, sources, name);
    return layerEdit(ctx, name, (doc) => {
      const width = params.width ?? r.width;
      const height = params.height ?? r.height;
      ctx.site.edit.resize(doc, width, height);
      return { message: `Resized to ${width}×${height}`, size: [width, height] };
    });
  },
  format: formatChange,
};

/** @type {import('../registry.mjs').Command} */
const create = {
  name: 'object.new',
  summary: 'Make a new object from a starter: a blank sprite, a block, or one that extends another',
  description:
    "The same starters as the lab's New dialog and `npm run art -- --new` (starter.mjs): a sprite of --width × --height (16×16 by default) " +
    'anchored at its bottom center, a one-level grass block, or an object that --extends another with no rows overridden yet.',
  params: {
    type: 'object',
    properties: {
      name: { type: 'string', description: "the new object's name (lowercase kebab-case)" },
      kind: { type: 'string', enum: ['sprite', 'block'], default: 'sprite' },
      extends: { type: 'string', description: 'the object it extends (a sprite, such as character)' },
      width: { type: 'integer', minimum: 1, maximum: 64, description: 'a new sprite\'s width (default 16)' },
      height: { type: 'integer', minimum: 1, maximum: 64, description: 'a new sprite\'s height (default 16)' },
    },
    required: ['name'],
  },
  args: ['name'],
  mutates: true,
  examples: ['pixellab object new small-rock --width 12 --height 8', 'pixellab object new tile-sand --kind block', 'pixellab object new range-hat --extends character'],
  async run(ctx, params) {
    const { name, kind } = params;
    await checkNewName(ctx, name, 'object.new');
    const { starter } = ctx.site;
    let doc;
    if (kind === 'block') {
      if (params.extends || params.width || params.height) throw new UsageError("object.new --kind block doesn't take --width, --height or --extends; a block's size is in its file, in tiles and levels");
      doc = starter.starterBlock();
    } else if (params.extends) {
      if (params.width || params.height) throw new UsageError("object.new: --width/--height and --extends don't mix: an object that extends another has its size");
      if (!(await ctx.names('object')).includes(params.extends)) throw new UsageError(`object.new --extends ${params.extends}: no such object`);
      doc = starter.starterExtends(params.extends);
    } else doc = starter.starterSprite(params.width ?? 16, params.height ?? 16);
    const label = `Created object ${name}`;
    ctx.session.create(ctx, 'object', name, doc, label);
    return { doc: `object:${name}`, changed: true, summary: label, file: `objects/${name}.mjs` };
  },
  format: formatChange,
};

/**
 * Rows → the object's own keys: legend characters become the object's key
 * for their color (made as painting makes one), the rest must be its keys.
 * @param {any} ctx
 * @param {any} sources
 * @param {any} r  resolved
 * @param {any} doc
 * @param {string[]} rows
 * @param {Record<string, string>} legend
 */
function rekey(ctx, sources, r, doc, rows, legend) {
  const { engine, edit } = ctx.site;
  /** @type {Record<string, string>} */
  const to = {};
  /** @type {string[]} */
  const added = [];
  for (const [ch, color] of Object.entries(legend)) {
    if (!engine.isKey(ch)) throw new UsageError(`object.map.set: legend key ${JSON.stringify(ch)} must be one printable character, not space or '.'`);
    if (typeof color !== 'string') throw new UsageError(`object.map.set: legend key ${ch} must name a palette color`);
    const entry = sources.colors.get(color);
    if (!entry) throw new UsageError(`object.map.set: no palette color named ${color} (pixellab palette get)`);
    if (entry.tier === 'outfit' && !r.character) throw new CommandError(`${color} is an outfit color; only objects that extend character may use it`, { code: 'color_not_allowed' });
    const known = { ...r.keys, ...(doc.keys ?? {}) };
    const key = edit.keyFor(doc, known, color);
    if (!(key in known)) added.push(`${key}: ${color}`);
    to[ch] = key;
  }
  const own = { ...r.keys, ...(doc.keys ?? {}) };
  const unknown = new Set();
  const out = rows.map((row) =>
    [...row]
      .map((ch) => {
        if (ch === '.') return ch;
        if (ch in to) return to[ch];
        if (ch in own) return ch;
        unknown.add(ch);
        return ch;
      })
      .join(''),
  );
  if (unknown.size) {
    throw new UsageError(`object.map.set: ${[...unknown].map((c) => JSON.stringify(c)).join(', ')} ${unknown.size === 1 ? "isn't a key" : "aren't keys"} of ${'this object'} or the legend; give them in --keys (char → color)`);
  }
  return { rows: out, added };
}

/** @type {import('../registry.mjs').Command} */
const setMap = {
  name: 'object.map.set',
  summary: "Replace a whole map of a sprite's layer (or frame of a loop) from rows, in one undo step",
  description:
    "Rows are strings of one character per pixel, top to bottom, '.' transparent. With --keys (a legend, { char: color }), each legend character " +
    "becomes the object's key for that color, made if it has none, so a generated map can use any characters; without, the characters are the object's own keys. " +
    'Checks the palette, outfit colors (only for objects that extend character), the 12-color cap and the 64×64 size cap. A size other than the object\'s ' +
    'is refused unless --resize, which pads or crops every map first (as object.resize). On an object that extends another, a still layer\'s rows become ' +
    'row overrides. Blocks are refused: paint their surface and sides.',
  params: {
    type: 'object',
    properties: {
      name: NAME,
      rows: { type: 'array', items: { type: 'string' }, minItems: 1, description: "the map's rows, top to bottom; on the command line each row a positional or the flag repeated" },
      keys: { type: 'object', description: 'a legend: character → palette color, as JSON on the command line' },
      layer: LAYER,
      frame: FRAME,
      resize: { type: 'boolean', default: false, description: "resize the object to the rows' size first" },
    },
    required: ['name', 'rows'],
  },
  args: ['name', 'rows'],
  mutates: true,
  examples: ["pixellab object map set pebble .gg. gwwg gwwg .gg. --keys '{\"g\": \"grass-2\", \"w\": \"ink\"}'"],
  async run(ctx, params) {
    const name = await objectAt(ctx, params.name);
    const { rows, layer, frame } = params;
    const width = rows[0].length;
    const bad = rows.findIndex((/** @type {string} */ row) => row.length !== width);
    if (width === 0) throw new UsageError('object.map.set: rows are empty');
    if (bad >= 0) throw new UsageError(`object.map.set: row ${bad} is ${rows[bad].length} wide; every row must be ${width}, like row 0`);
    const { engine, edit } = ctx.site;
    if (width > engine.CAPS.size || rows.length > engine.CAPS.size) {
      throw new CommandError(`${width}×${rows.length} is over the size cap, ${engine.CAPS.size}×${engine.CAPS.size}`, { code: 'size_cap' });
    }
    return editOf(ctx)({ kind: 'object', name }, async (/** @type {any} */ doc) => {
      if (doc.kind === 'block') throw new CommandError(`${name} is a block; its maps are its surface and sides, which object.paint paints`, { code: 'not_editable' });
      let { sources } = await ctx.sources();
      let r = resolved(ctx, sources, name);
      const blocked = paintBlocked(doc, r, layer);
      if (blocked) throw new CommandError(blocked, { code: 'paint_blocked' });
      const source = r.layers[layer];
      const loop = 'frames' in source;
      if (loop && !source.frames[frame]) throw new UsageError(`there's no frame ${frame}; the loop has ${source.frames.length}`);
      if (!loop && frame > 0) throw new UsageError(`layer ${layer} is a still map, so it has only frame 0`);
      if (width !== r.width || rows.length !== r.height) {
        if (doc.extends) throw new CommandError(`${name} extends ${doc.extends} and takes its size (${r.width}×${r.height}) from it`, { code: 'size_mismatch' });
        if (!params.resize) {
          throw new CommandError(`${name} is ${r.width}×${r.height}; these rows are ${width}×${rows.length}. Give --resize to resize every map first.`, { code: 'size_mismatch' });
        }
        edit.resize(doc, width, rows.length);
        ctx.invalidate();
        ({ sources } = await ctx.sources());
        r = resolved(ctx, sources, name);
      }
      const { rows: keyed, added } = rekey(ctx, sources, r, doc, rows, params.keys ?? {});
      if (doc.extends) {
        const baseMap = edit.mapAt(engine.resolve(sources, doc.extends).layers[layer], 0);
        const mine = Object.fromEntries(keyed.map((row, y) => [y, row]).filter(([y, row]) => row !== baseMap[/** @type {number} */ (y)]));
        const overrides = { ...(doc.rows ?? {}) };
        if (Object.keys(mine).length) overrides[layer] = mine;
        else delete overrides[layer];
        doc.rows = Object.fromEntries(Object.keys(overrides).map(Number).sort((a, b) => a - b).map((l) => [l, overrides[l]]));
      } else if (loop) doc.layers[layer].frames[frame] = keyed;
      else doc.layers[layer].map = keyed;
      ctx.invalidate();
      ({ sources } = await ctx.sources());
      const colors = engine.usedColors(sources, resolved(ctx, sources, name)).length;
      if (colors > engine.CAPS.colors) throw new CommandError(`That map gives ${name} ${colors} colors; an object paints at most ${engine.CAPS.colors}`, { code: 'color_cap' });
      const where = `layer ${layer}${loop ? `, frame ${frame}` : ''}`;
      return { message: `Set the map of ${where}: ${width}×${rows.length}, ${colors} color${colors === 1 ? '' : 's'} in the object`, size: [width, rows.length], colors, keysAdded: added };
    });
  },
  format: formatChange,
};

export default [paint, erase, fill, pick, ...layerCommands, ...frameCommands, animate, still, resize, create, setMap];
