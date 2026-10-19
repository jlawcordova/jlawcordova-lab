// Read-only object commands: object.list, object.show, object.render.

import { CommandError } from '../errors.mjs';
import { MAX_SCALE, renderObjectImage } from '../render.mjs';

const NAME = { type: 'string', description: 'object name (or objects/<name>)' };

/**
 * A summary of one resolved object, or an error if it doesn't resolve.
 * @param {any} site
 * @param {any} sources
 * @param {string} name
 */
function summary(site, sources, name) {
  const { engine } = site;
  const doc = sources.objects.get(name);
  const kind = doc?.kind === 'block' ? 'block' : 'sprite';
  const base = { name, kind, ...(doc?.extends ? { extends: doc.extends } : {}) };
  let resolved;
  try {
    resolved = engine.resolve(sources, name);
  } catch (err) {
    return { ...base, error: /** @type {Error} */ (err).message };
  }
  try {
    const frames = Math.max(1, ...resolved.layers.map((/** @type {any} */ l) => engine.layerMaps(l).length));
    return {
      ...base,
      size: [resolved.width, resolved.height],
      ...(resolved.block ? { blockSize: [...resolved.block.size] } : {}),
      layers: resolved.layers.length,
      frames,
      colors: engine.usedColors(sources, resolved).length,
      description: engine.describeObject(sources, resolved),
      resolved,
    };
  } catch (err) {
    return { ...base, error: /** @type {Error} */ (err).message };
  }
}

/** @type {import('../registry.mjs').Command} */
const list = {
  name: 'object.list',
  summary: 'List every object with its kind, size, layers, frames and colors',
  params: {
    type: 'object',
    properties: { kind: { type: 'string', enum: ['sprite', 'block'], description: 'only objects of this kind' } },
  },
  args: [],
  mutates: false,
  async run(ctx, { kind }) {
    const { sources } = await ctx.sources();
    const objects = [];
    for (const name of sources.objects.keys()) {
      const { resolved, ...s } = summary(ctx.site, sources, name);
      if (!kind || s.kind === kind) objects.push(s);
    }
    return { objects };
  },
  format: (r) => {
    const w = Math.max(4, ...r.objects.map((/** @type {any} */ o) => o.name.length));
    return r.objects
      .map((/** @type {any} */ o) => `${o.name.padEnd(w)}  ${o.kind.padEnd(6)}  ${o.error ? `error: ${o.error}` : o.description}${o.extends ? `  (extends ${o.extends})` : ''}`)
      .join('\n');
  },
};

/** @type {import('../registry.mjs').Command} */
const show = {
  name: 'object.show',
  summary: "Show an object's kind, size, anchor, layers, frames, colors and extends",
  params: {
    type: 'object',
    properties: {
      name: NAME,
      doc: { type: 'boolean', default: false, description: "also return the object's source data as doc" },
    },
    required: ['name'],
  },
  args: ['name'],
  mutates: false,
  async run(ctx, { name: query, doc: withDoc }) {
    const { name } = await ctx.find(query, 'object');
    const doc = await ctx.doc('object', name);
    const { sources } = await ctx.sources();
    const { engine } = ctx.site;
    const s = summary(ctx.site, sources, name);
    if (s.error || !s.resolved) throw new CommandError(`object ${name} doesn't resolve: ${s.error}; run pixellab check ${name}`, { code: 'resolve_error' });
    const { resolved, ...rest } = s;
    const used = new Set(engine.usedKeys(resolved));
    const color = (/** @type {string} */ colorName) => {
      const c = sources.colors.get(colorName);
      return { name: colorName, hex: c?.hex ?? null, tier: c?.tier ?? null };
    };
    const keys = Object.entries(resolved.keys).map(([key, colorName]) => ({ key, ...color(/** @type {string} */ (colorName)), used: used.has(key) }));
    const layers = resolved.block
      ? resolved.layers.map((/** @type {any} */ l, /** @type {number} */ index) => ({
          index,
          surface: true,
          frames: engine.layerMaps(l).length,
          ...(l.loop ? { loop: l.loop, prefix: l.prefix } : {}),
        }))
      : resolved.layers.map((/** @type {any} */ l, /** @type {number} */ index) => ({
          index,
          frames: engine.layerMaps(l).length,
          ...(l.class ? { class: l.class } : {}),
          ...(l.loop ? { loop: l.loop, prefix: l.prefix } : {}),
        }));
    return {
      ...rest,
      file: `objects/${name}.mjs`,
      anchor: [...resolved.anchor],
      character: resolved.character,
      ...(resolved.block
        ? {
            block: {
              size: [...resolved.block.size],
              faces: Object.fromEntries(Object.entries(resolved.block.faces).map(([face, c]) => [face, color(/** @type {string} */ (c))])),
              sides: resolved.block.sides
                ? Object.entries(resolved.block.sides.keys).map(([key, c]) => ({ key, ...color(/** @type {string} */ (c)) }))
                : null,
            },
          }
        : {}),
      layers,
      keys,
      usedColors: engine.usedColors(sources, resolved),
      ...(withDoc ? { doc } : {}),
    };
  },
  format: (r) => {
    const lines = [
      `${r.name}  (${r.file})`,
      `kind      ${r.kind}${r.extends ? `, extends ${r.extends}` : ''}${r.character ? ', character' : ''}`,
      `size      ${r.size.join('×')}${r.block ? ` (block ${r.block.size.join('×')})` : ''}`,
      `anchor    ${r.anchor.join(', ')}`,
      `summary   ${r.description}`,
    ];
    if (r.block) lines.push(`faces     ${Object.entries(r.block.faces).map(([f, c]) => `${f} ${/** @type {any} */ (c).name}`).join(', ')}`);
    for (const l of r.layers) {
      const what = [l.surface ? 'surface' : '', l.class ? `class ${l.class}` : '', l.loop ? `loop ${l.loop} (${l.prefix}0…)` : ''].filter(Boolean).join(', ');
      lines.push(`layer ${l.index}   ${l.frames} frame${l.frames === 1 ? '' : 's'}${what ? `, ${what}` : ''}`);
    }
    if (r.keys.length) lines.push(`keys      ${r.keys.map((/** @type {any} */ k) => `${k.key} ${k.name}${k.used ? '' : ' (unused)'}`).join(', ')}`);
    return lines.join('\n');
  },
};

/** @type {import('../registry.mjs').Command} */
const render = {
  name: 'object.render',
  summary: 'Render one object to a transparent PNG',
  description:
    "The anchor is at (0, 0). A sprite's viewBox is its whole map, so at scale 1 PNG pixel (x, y) is map column x, row y; " +
    "a block's is the box around its pixels. Returns the same shape as scene.render, with one item. " +
    'On the command line, --out <file.png> writes the PNG.',
  params: {
    type: 'object',
    properties: {
      name: NAME,
      scale: { type: 'integer', minimum: 1, maximum: MAX_SCALE, default: 1, description: 'whole-pixel zoom of the PNG' },
      frame: { type: 'integer', minimum: 0, default: 0, description: 'the frame every loop shows (loops shorter than this show nothing)' },
      time: { type: 'integer', minimum: 0, description: 'ms into the loops; overrides frame' },
      layers: {
        type: 'array',
        items: { type: 'integer', minimum: 0 },
        description: "only these layers of a sprite, each at --frame or its last frame, as the lab's stage shows them (hidden layers, onion skin)",
      },
    },
    required: ['name'],
  },
  args: ['name'],
  mutates: false,
  writesPng: true,
  examples: ['pixellab object render pine --scale 4 --out /tmp/pine.png'],
  async run(ctx, { name: query, scale, frame, time, layers }) {
    const { name } = await ctx.find(query, 'object');
    await ctx.doc('object', name);
    const { sources } = await ctx.sources();
    return { object: name, ...renderObjectImage(ctx.site, sources, name, { scale, frame, time, layers }) };
  },
  format: (r) => `${r.object}: ${r.width}×${r.height} PNG at ${r.scale}×, viewBox ${r.viewBox.join(', ')}`,
};

export default [list, show, render];
