// check: the engine's validation problems, for every source or for one doc
// and what it uses (as `npm run art -- --check <name>` scopes them), and
// warnings that don't fail it: motion that steps off the grid
// (docs/intents/2026-10-mcp-gaps/spec.md, D8).

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

/** @type {import('../registry.mjs').Command} */
const check = {
  name: 'check',
  summary: 'Validate the sources with the engine, all of them or one doc',
  description:
    'Problems are the engine\'s "<file>: <where>: <rule>" lines. With a name, only the problems in that doc and the files it uses ' +
    '(palette.mjs, the objects it places or extends) are listed. On the command line, the exit code is 1 when there are problems. ' +
    'Warnings don\'t fail the check: a scene\'s key-to-key moves (the loop back to the first key included) whose steps aren\'t 1 px straight, 1:1 or 2:1, ' +
    'so the item jumps instead of walking the grid; scene.motion.path makes on-grid keys.',
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
    const warningsOf = (/** @type {string[]} */ names) =>
      names.flatMap((n) => offGridMoves(ctx.site.motion, sources.scenes.get(n)).map((w) => entry(offGridText(`scenes/${n}.mjs`, w))));
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
