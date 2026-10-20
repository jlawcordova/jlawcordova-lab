// check: the engine's validation problems, for every source or for one doc
// and what it uses (as `npm run art -- --check <name>` scopes them), and
// warnings that don't fail it, each with a code: motion that steps off the
// grid (off_grid, docs/intents/2026-10-mcp-gaps/spec.md, D8), a scene whose
// file isn't in depth order (out_of_order) and the copies a moving item is
// drawn in (copies; docs/intents/2026-10-depth-order/spec.md, R11, R12).

import { offGridMoves, offGridText } from '../motion-ops.mjs';

/**
 * The files a check of one doc covers: palette.mjs, the doc, and every
 * object it uses (by extends, items and motion poses).
 * @param {any} sources
 * @param {{ kind: 'object' | 'scene', name: string }} target
 */
export function dependencies(sources, { kind, name }) {
  const files = new Set(['palette.mjs']);
  const addObject = (/** @type {unknown} */ object) => {
    for (let n = object; typeof n === 'string' && !files.has(`objects/${n}.mjs`); n = sources.objects.get(n)?.extends) files.add(`objects/${n}.mjs`);
  };
  const addItems = (/** @type {unknown} */ items) => {
    for (const item of Array.isArray(items) ? items : []) {
      if (item?.group) addItems(item.items);
      else if (typeof item?.object === 'string') addObject(item.object);
    }
  };
  if (kind === 'object') addObject(name);
  else {
    files.add(`scenes/${name}.mjs`);
    const scene = sources.scenes.get(name);
    addItems(scene?.items);
    for (const track of Array.isArray(scene?.motion?.tracks) ? scene.motion.tracks : []) {
      for (const key of Array.isArray(track?.keys) ? track.keys : []) addObject(key?.pose);
    }
  }
  return files;
}

const fileOf = (/** @type {string} */ problem) => problem.slice(0, problem.indexOf(': '));

/**
 * How many items a sort would move in a scene, as scene.sort counts them: in
 * each list (groups' members apart), those not in the longest run that keeps
 * its order, so one item moved far counts once.
 * @param {any} engine
 * @param {any} depth  the engine's depth module
 * @param {any} sources
 * @param {any} scene
 */
function misplaced(engine, depth, sources, scene) {
  const tracks = new Map((scene.motion?.tracks ?? []).map((/** @type {any} */ track) => [track.item, track]));
  const walk = (/** @type {any[]} */ items) =>
    depth.displaced(engine.depthOrder(sources, items, scene.origin ?? [0, 0], tracks)) + items.reduce((n, item) => n + (item.group ? walk(item.items ?? []) : 0), 0);
  return walk(scene.items);
}

/** @type {import('../registry.mjs').Command} */
const check = {
  name: 'check',
  summary: 'Validate the sources with the engine, all of them or one doc',
  description:
    'Problems are the engine\'s "<file>: <where>: <rule>" lines. With a name, only the problems in that doc and the files it uses ' +
    '(palette.mjs, the objects it places or extends) are listed. On the command line, the exit code is 1 when there are problems. ' +
    'Warnings don\'t fail the check, and each has a code. off_grid: a scene\'s key-to-key moves (the loop back to the first key included) whose steps ' +
    'aren\'t 1 px straight, 1:1 or 2:1, so the item jumps instead of walking the grid; scene.motion.path makes on-grid keys. ' +
    'out_of_order: a scene whose file lists its items out of depth order (it still publishes in depth order; scene.sort writes the file in order), with how many items would move. ' +
    'copies: a moving item that takes several places among the others as it moves is drawn once for each place in the SVG, which adds to its size; the warning says how many.',
  params: {
    type: 'object',
    properties: {
      name: { type: 'string', description: 'an object or scene (or objects/<name>, scenes/<name>); every source when left out' },
      kind: { type: 'string', enum: ['object', 'scene'], description: 'which kind name is, when both exist' },
    },
  },
  args: ['name'],
  mutates: false,
  examples: ['pixellab check', 'pixellab check hero-island', 'pixellab check objects/pine --json'],
  async run(ctx, { name: query, kind }) {
    const { sources, loadErrors } = await ctx.sources();
    const all = [...loadErrors, ...ctx.site.engine.validate(sources)];
    const entry = (/** @type {string} */ text) => ({ file: fileOf(text), message: text.slice(fileOf(text).length + 2), text });
    const { engine } = ctx.site;
    /** The depth warnings of a scene whose sources are sound: out of order, and the copies its moving items are drawn in. */
    const depthWarnings = (/** @type {string} */ n) => {
      const file = `scenes/${n}.mjs`;
      const scene = sources.scenes.get(n);
      if ([...dependencies(sources, { kind: 'scene', name: n })].some((f) => all.some((p) => fileOf(p) === f))) return [];
      try {
        /** @type {any[]} */
        const out = [];
        if (!engine.isInDepthOrder(sources, scene)) {
          const moves = misplaced(engine, ctx.site.depth, sources, scene);
          out.push({ ...entry(`${file}: items are not in depth order: ${moves} would move (pixellab scene sort ${n} puts them right)`), code: 'out_of_order', scene: n, moves });
        }
        for (const { item, copies } of engine.motionCopies(sources, n)) {
          out.push({ ...entry(`${file}: ${item} is drawn ${copies} times as it moves among other items (once for each place it takes in the depth order)`), code: 'copies', scene: n, item, copies });
        }
        return out;
      } catch {
        return []; // The sources have problems of their own, which are listed.
      }
    };
    const warningsOf = (/** @type {string[]} */ names) =>
      names.flatMap((n) => [
        ...offGridMoves(ctx.site.motion, sources.scenes.get(n)).map((w) => ({ ...entry(offGridText(`scenes/${n}.mjs`, w)), code: 'off_grid', scene: n })),
        ...depthWarnings(n),
      ]);
    if (query === undefined) {
      return {
        ok: all.length === 0,
        target: null,
        checked: 1 + sources.objects.size + sources.scenes.size,
        problems: all.map(entry),
        warnings: warningsOf([...sources.scenes.keys()].sort()),
      };
    }
    const target = await ctx.find(query, kind);
    const files = dependencies(sources, target);
    const mine = all.filter((p) => files.has(fileOf(p)));
    /** @type {string | null} */
    let summary = null;
    if (!mine.length) {
      const { engine } = ctx.site;
      if (target.kind === 'object') summary = engine.describeObject(sources, engine.resolve(sources, target.name));
      else {
        const scene = sources.scenes.get(target.name);
        const count = (/** @type {any[]} */ items) => items.reduce((n, item) => n + (item.group ? count(item.items) : 1), 0);
        const objects = [...files].filter((f) => f.startsWith('objects/')).length;
        const [, , w, h] = scene.viewBox;
        summary = `${w}×${h} · ${count(scene.items)} items · ${objects} objects · ${scene.output ? `output ${scene.output}` : 'preview only'}`;
      }
    }
    return {
      ok: mine.length === 0,
      target,
      checked: files.size,
      problems: mine.map(entry),
      warnings: target.kind === 'scene' ? warningsOf([target.name]) : [],
      ...(summary ? { summary } : {}),
    };
  },
  format: (r) => {
    const w = r.warnings.length;
    const warnings = [...r.warnings.map((/** @type {any} */ p) => `warning: ${p.text}`), ...(w ? [`${w} warning${w === 1 ? '' : 's'}`] : [])];
    if (r.ok) return [r.target ? `${r.target.file}: ok · ${r.summary}` : `ok · ${r.checked} files, no problems`, ...warnings].join('\n');
    const n = r.problems.length;
    return [...r.problems.map((/** @type {any} */ p) => p.text), `${n} problem${n === 1 ? '' : 's'}`, ...warnings].join('\n');
  },
};

export default [check];
