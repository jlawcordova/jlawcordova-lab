// art.build, art.preview and art.sizes: publishing a document's scenes to
// the SVGs a site inlines, the review PNGs an agent looks at, and where a
// compiled scene's bytes go (docs/intents/2026-10-pixellab-document/spec.md,
// R7, R8). They do in-process what the site's `npm run art`, `--preview` and
// `--sizes` did, with the engine in cli/lib/engine/.

import { resolve as resolvePath } from 'node:path';

import { createContext } from '../context.mjs';
import { CommandError } from '../errors.mjs';
import { MAX_SCALE } from '../render.mjs';
import { dependencies } from './check.mjs';

const fileOf = (/** @type {string} */ problem) => problem.slice(0, problem.indexOf(': '));

/** Problems in the files a target uses, as the site's `--check` scoped them. */
const problemsFor = (/** @type {any} */ sources, /** @type {string[]} */ problems, /** @type {{ kind: 'object' | 'scene', name: string }} */ target) => {
  const files = dependencies(sources, target);
  return problems.filter((p) => files.has(fileOf(p)));
};

const invalid = (/** @type {string} */ what, /** @type {string[]} */ problems) =>
  new CommandError(`${what} has ${problems.length} problem${problems.length === 1 ? '' : 's'}:\n${problems.join('\n')}`, { code: 'invalid', details: problems });

/** @type {import('../registry.mjs').Command} */
const build = {
  name: 'art.build',
  summary: 'Publish: validate every source, compile each scene with an output and write the SVGs',
  description:
    "Validates the document's files on disk (unsaved working copies aren't in it; unsaved lists them), renders each scene with an `output` to a rect SVG, " +
    'merges each color group into one <path>, checks the result lossless and within its budget (100 KB raw / 25 KB gzip, or the manifest\'s), and writes <publish dir>/<output>. ' +
    "The publish folder is the manifest's publish.dir (relative to the document), or --dir; with neither it refuses. Nothing is written when any source has a problem. " +
    'Returns { ok, publishDir, outputs: [{ file, scene, raw, gzip, over, report }], previewOnly: [{ scene, raw, gzip, over, report }], problems, ms, unsaved }.',
  params: { type: 'object', properties: { dir: { type: 'string', description: "the folder to write the SVGs to (else the manifest's publish.dir)" } } },
  args: [],
  mutates: false,
  writes: 'direct',
  examples: ['pixellab art build', 'pixellab art build --dir ~/Desktop/art'],
  async run(ctx, { dir }) {
    const started = Date.now();
    const doc = ctx.site;
    const out = dir ? resolvePath(dir) : doc.publishDir;
    if (!out) {
      throw new CommandError('this document has no publish folder: add "publish": { "dir": "<folder>" } to its manifest.json, or pass --dir <folder>', { code: 'no_publish_dir' });
    }
    // The files on disk, not anyone's unsaved working copies.
    const disk = createContext(doc);
    const { sources, loadErrors } = await disk.sources();
    const problems = [...loadErrors, ...doc.engine.validate(sources)];
    const r = await doc.compile.compileAll(sources, problems, out, doc.budgets);
    const unsaved = ctx.session ? ctx.session.list().filter((/** @type {any} */ s) => s.dirty).map((/** @type {any} */ s) => s.doc) : [];
    return { ok: r.ok, publishDir: out, outputs: r.outputs, previewOnly: r.previewOnly, problems: r.problems, ms: Date.now() - started, unsaved };
  },
  format: (r) => {
    const lines = [
      ...r.outputs.map((/** @type {any} */ o) => `${o.file}: ${o.report}`),
      ...r.previewOnly.map((/** @type {any} */ p) => `${p.scene} (preview only, not written): ${p.report}`),
      ...r.problems,
    ];
    if (r.problems.length) lines.push(`${r.problems.length} problem${r.problems.length === 1 ? '' : 's'}; nothing written.`);
    else if (r.ok) lines.push(`published ${r.outputs.length} file${r.outputs.length === 1 ? '' : 's'} to ${r.publishDir} in ${(r.ms / 1000).toFixed(1)} s`);
    else lines.push(`published ${r.outputs.length} file${r.outputs.length === 1 ? '' : 's'} to ${r.publishDir}, but some are OVER BUDGET`);
    if (r.unsaved.length) lines.push(`not built (unsaved): ${r.unsaved.join(', ')}`);
    return lines.join('\n');
  },
};

/** @type {import('../registry.mjs').Command} */
const preview = {
  name: 'art.preview',
  summary: 'Render the review PNG of an object (standing on a tile) or a scene, on the card color; `palette` for the swatch sheet',
  description:
    "The picture an agent or a person checks art by: an object on one 32×16 tile outline, a scene at its viewBox (first variant, frame 0), both on the document's card color (manifest preview.card). " +
    'Refuses (code invalid) when the doc or what it uses has problems. Returns { name, kind, scale, width, height, png } with the PNG as base64; --out writes it.',
  params: {
    type: 'object',
    properties: {
      name: { type: 'string', description: 'an object or scene (or objects/<name>, scenes/<name>), or palette' },
      scale: { type: 'integer', minimum: 1, maximum: MAX_SCALE, default: 1, description: 'whole-pixel zoom of the PNG' },
      kind: { type: 'string', enum: ['object', 'scene'], description: 'which kind name is, when both exist' },
    },
    required: ['name'],
  },
  args: ['name'],
  mutates: false,
  writesPng: true,
  examples: ['pixellab art preview pine --scale 4 --out /tmp/pine@4x.png', 'pixellab art preview palette --out /tmp/palette.png'],
  async run(ctx, { name, scale, kind }) {
    const doc = ctx.site;
    const { sources, loadErrors } = await ctx.sources();
    const problems = [...loadErrors, ...doc.engine.validate(sources)];
    const colors = doc.preview.previewColors(doc.previewColors);
    let image;
    let found = 'palette';
    if (name === 'palette') {
      const mine = problems.filter((p) => p.startsWith('palette.mjs: '));
      if (mine.length) throw invalid('palette.mjs', mine);
      image = doc.preview.renderPalette(sources, colors).image;
    } else {
      const target = await ctx.find(name, kind);
      const mine = problemsFor(sources, problems, target);
      if (mine.length) throw invalid(target.file, mine);
      found = target.kind;
      image = doc.preview.renderPreview(sources, target.kind === 'object' ? { object: target.name } : { scene: target.name }, colors).image;
    }
    return { name, kind: found, scale, width: image.width * scale, height: image.height * scale, png: doc.preview.encodePng(image, scale).toString('base64') };
  },
  format: (r) => `${r.name}: ${r.width}×${r.height} preview PNG at ${r.scale}×`,
};

const bytes = (/** @type {number | undefined} */ n) => (n === undefined ? '—' : n.toLocaleString('en-US'));
const delta = (/** @type {number | undefined} */ a, /** @type {number | undefined} */ b) => (a === undefined || b === undefined ? '—' : `${a - b >= 0 ? '+' : ''}${(a - b).toLocaleString('en-US')}`);

/** @type {import('../registry.mjs').Command} */
const sizes = {
  name: 'art.sizes',
  summary: "Bytes each object and each color adds to a compiled scene, against the document's git HEAD",
  description:
    'The bytes a compiled scene loses, raw and gzip, when an object\'s items are left out or a color\'s paths are dropped (costs overlap a little), now and as committed at HEAD of the repository the document is in. ' +
    'Returns { scene, output, now: { raw, gzip }, head: { raw, gzip } | { reason }, objects: [{ object, placed, raw, gzip, headRaw, headGzip }], colors: [{ color, raw, gzip, headRaw, headGzip }] }.',
  params: {
    type: 'object',
    properties: { name: { type: 'string', description: 'a scene (or scenes/<name>)' } },
    required: ['name'],
  },
  args: ['name'],
  mutates: false,
  examples: ['pixellab art sizes hero-island'],
  async run(ctx, { name }) {
    const doc = ctx.site;
    const { sources, loadErrors } = await ctx.sources();
    const target = await ctx.find(name, 'scene');
    const problems = problemsFor(sources, [...loadErrors, ...doc.engine.validate(sources)], target);
    if (problems.length) throw invalid(target.file, problems);
    const now = doc.compile.sceneSizes(sources, target.name);
    const head = await doc.compile.headSources(doc.root);
    /** @type {any} */
    let before;
    try {
      if ('reason' in head) before = { reason: head.reason };
      else if (!head.sources.scenes.has(target.name)) before = { reason: `${target.file} isn't at HEAD` };
      else if (problemsFor(head.sources, head.problems, target).length) before = { reason: 'its sources at HEAD have problems' };
      else before = doc.compile.sceneSizes(head.sources, target.name);
    } finally {
      await doc.compile.dropHead(head);
    }
    const rows = (/** @type {Map<string, any>} */ nowRows, /** @type {Map<string, any> | undefined} */ headRows, /** @type {string} */ key) => {
      const keys = [...new Set([...nowRows.keys(), ...(headRows?.keys() ?? [])])];
      const raw = (/** @type {string} */ k) => nowRows.get(k)?.raw ?? -1;
      keys.sort((a, b) => raw(b) - raw(a) || (headRows?.get(b)?.raw ?? 0) - (headRows?.get(a)?.raw ?? 0) || (a < b ? -1 : 1));
      return keys.map((k) => {
        const [n, h] = [nowRows.get(k), headRows?.get(k)];
        return { [key]: k, ...(n?.placed !== undefined ? { placed: n.placed } : {}), raw: n?.raw, gzip: n?.gzip, headRaw: h?.raw, headGzip: h?.gzip };
      });
    };
    const scene = sources.scenes.get(target.name);
    return {
      scene: target.name,
      output: scene.output ?? null,
      now: { raw: now.raw, gzip: now.gzip },
      head: before.reason ? { reason: before.reason } : { raw: before.raw, gzip: before.gzip },
      objects: rows(now.objects, before.objects, 'object'),
      colors: rows(now.colors, before.colors, 'color'),
    };
  },
  format: (r) => {
    const lines = [`${r.output ?? `${r.scene} (preview only)`}: ${bytes(r.now.raw)} B raw, ${bytes(r.now.gzip)} B gzip`];
    lines.push(r.head.reason ? `HEAD: not available (${r.head.reason})` : `HEAD: ${bytes(r.head.raw)} B raw, ${bytes(r.head.gzip)} B gzip (${delta(r.now.raw, r.head.raw)} raw, ${delta(r.now.gzip, r.head.gzip)} gzip)`);
    const table = (/** @type {string} */ title, /** @type {string} */ caption, /** @type {any[]} */ list, /** @type {boolean} */ placed) => {
      const header = [title, ...(placed ? ['placed'] : []), 'raw', 'Δ raw', 'gzip', 'Δ gzip'];
      const body = list.map((row) => [row[title], ...(placed ? [bytes(row.placed)] : []), bytes(row.raw), delta(row.raw, row.headRaw), bytes(row.gzip), delta(row.gzip, row.headGzip)]);
      const widths = header.map((_, i) => Math.max(...[header, ...body].map((l) => l[i].length)));
      lines.push('', caption);
      for (const l of [header, ...body]) lines.push(l.map((c, i) => (i === 0 ? c.padEnd(widths[i]) : c.padStart(widths[i]))).join('  '));
    };
    table('object', 'Bytes each object adds (the scene compiled without it), against HEAD:', r.objects, true);
    table('color', "Bytes each color's path takes, against HEAD:", r.colors, false);
    return lines.join('\n');
  },
};

export default [build, preview, sizes];
