// document.info, document.new, document.copy and palette.get: the open
// document (docs/intents/2026-10-pixellab-document/spec.md, R3, R4), making a
// new one, saving a copy of it, and its palette.

import { existsSync, readFileSync, statSync } from 'node:fs';
import { cp, mkdir, writeFile } from 'node:fs/promises';
import { basename, dirname, join, resolve as resolvePath } from 'node:path';

import { EXTENSION, FORMAT, expandPath } from '../document.mjs';
import { CommandError } from '../errors.mjs';

/** The checked-out branch, read from .git/HEAD (no git process, nothing written). */
export function gitBranch(/** @type {string} */ root) {
  try {
    let dir = root;
    for (let i = 0; i < 8; i++) {
      let gitDir = join(dir, '.git');
      if (existsSync(gitDir)) {
        if (statSync(gitDir).isFile()) {
          const m = /^gitdir:\s*(.+)$/m.exec(readFileSync(gitDir, 'utf8'));
          if (!m) return null;
          gitDir = resolvePath(dir, m[1].trim());
        }
        const head = readFileSync(join(gitDir, 'HEAD'), 'utf8').trim();
        const ref = /^ref:\s*refs\/heads\/(.+)$/.exec(head);
        return ref ? ref[1] : `(detached at ${head.slice(0, 7)})`;
      }
      const up = dirname(dir);
      if (up === dir) break;
      dir = up;
    }
    return null;
  } catch {
    return null;
  }
}

/** The starter palette's source text, as a new document's palette.mjs. */
async function starterPaletteText() {
  const text = readFileSync(new URL('../engine/starter-palette.mjs', import.meta.url), 'utf8');
  const body = text.slice(text.indexOf('export default'));
  return `// The document's palette: objects name their colors from it. Colors are\n// added by hand. world: at most 32; outfit: at most 16, only for objects\n// that extend character. Names are unique across the tiers.\n${body}`;
}

/** @type {import('../registry.mjs').Command} */
const info = {
  name: 'document.info',
  summary: 'Show the document in use: where it came from, its publish folder and what it holds',
  description:
    'root is the .pixellab folder (or a legacy source/ folder in a site checkout, kind legacy-site); from says how it was named (--document, --site, PIXELLAB_DOC, ' +
    'PIXELLAB_SITE, the config file or the default); publishDir is where art build writes (null when the manifest names none); ' +
    'then the git branch, and how many scenes, objects and colors it has, with the count of validation problems.',
  params: { type: 'object', properties: {} },
  args: [],
  mutates: false,
  async run(ctx) {
    const doc = ctx.site;
    const { sources, loadErrors } = await ctx.sources();
    const problems = [...loadErrors, ...doc.engine.validate(sources)];
    /** @type {Record<string, number>} */
    const tiers = {};
    for (const { tier } of sources.colors.values()) tiers[tier] = (tiers[tier] ?? 0) + 1;
    return {
      root: doc.root,
      from: doc.from,
      via: doc.via,
      kind: doc.kind,
      name: doc.name,
      siteRoot: doc.siteRoot,
      publishDir: doc.publishDir,
      branch: gitBranch(doc.root),
      node: process.version,
      scenes: sources.scenes.size,
      objects: sources.objects.size,
      colors: tiers,
      problems: problems.length,
    };
  },
  format: (r) =>
    [
      `document  ${r.root} (${r.kind === 'legacy-site' ? 'a legacy source folder, ' : ''}from ${r.via === 'site' ? 'the site checkout given by ' : ''}${r.from === 'flag' ? (r.via === 'doc' ? '--document' : '--site') : r.from === 'env' ? (r.via === 'site' ? 'PIXELLAB_SITE' : 'PIXELLAB_DOC') : r.from})`,
      `name      ${r.name}`,
      `publish   ${r.publishDir ?? 'none (add publish.dir to manifest.json, or pass --dir to art build)'}`,
      `branch    ${r.branch ?? 'unknown'}`,
      `scenes    ${r.scenes}`,
      `objects   ${r.objects}`,
      `colors    ${Object.entries(r.colors).map(([t, n]) => `${n} ${t}`).join(', ')}`,
      `problems  ${r.problems}${r.problems ? ' (pixellab check)' : ''}`,
    ].join('\n'),
};

/** @type {import('../registry.mjs').Command} */
const make = {
  name: 'document.new',
  summary: 'Make a new, empty .pixellab document with the starter palette',
  description: 'Writes manifest.json, palette.mjs and empty objects/ and scenes/ folders at path (.pixellab is added when missing). Refuses an existing path. The new document is not opened: name it with --document.',
  params: {
    type: 'object',
    properties: {
      path: { type: 'string', description: 'where to make it, e.g. ~/Art/meadow.pixellab' },
      name: { type: 'string', description: "the document's name (the folder's, by default)" },
    },
    required: ['path'],
  },
  args: ['path'],
  mutates: true,
  writes: 'direct',
  examples: ['pixellab document new ~/Art/meadow.pixellab'],
  async run(_ctx, { path, name }) {
    let root = expandPath(path);
    if (!root.endsWith(EXTENSION)) root += EXTENSION;
    if (existsSync(root)) throw new CommandError(`${root} already exists; pick another path, or open it with --document`, { code: 'exists' });
    const docName = name ?? basename(root, EXTENSION);
    if (!docName) throw new CommandError('the document needs a name', { code: 'usage', exitCode: 2 });
    await mkdir(join(root, 'objects'), { recursive: true });
    await mkdir(join(root, 'scenes'), { recursive: true });
    await writeFile(join(root, 'manifest.json'), `${JSON.stringify({ format: FORMAT, name: docName }, null, 2)}\n`);
    await writeFile(join(root, 'palette.mjs'), await starterPaletteText());
    return { root, name: docName, summary: `Made ${root}` };
  },
  format: (r) => `${r.summary}\nNext: pixellab --document ${r.root} scene new <name>, or open it in Pixel Lab.`,
};

/** @type {import('../registry.mjs').Command} */
const copy = {
  name: 'document.copy',
  summary: 'Save a copy of the open document at another path (Save As)',
  description:
    'Copies the whole .pixellab folder (manifest, palette, objects, scenes) to --to. Refuses an existing path. The copy keeps its publish.dir, so a relative one now points next to the copy. ' +
    "A legacy source/ folder has no manifest: the copy gets one with the site's budgets, publish folder (..) and colors, so it publishes as the site did.",
  params: { type: 'object', properties: { to: { type: 'string', description: 'the new path (.pixellab is added when missing)' } }, required: ['to'] },
  args: [],
  mutates: true,
  writes: 'direct',
  examples: ['pixellab document copy --to ~/Art/meadow-v2.pixellab'],
  async run(ctx, { to }) {
    let dest = expandPath(to);
    if (!dest.endsWith(EXTENSION)) dest += EXTENSION;
    if (existsSync(dest)) throw new CommandError(`${dest} already exists; pick another path`, { code: 'exists' });
    const from = ctx.site.root;
    if (dest === from || dest.startsWith(`${from}/`)) throw new CommandError(`${dest} is inside the document itself`, { code: 'usage', exitCode: 2 });
    await mkdir(dirname(dest), { recursive: true });
    await cp(from, dest, { recursive: true });
    let wroteManifest = false;
    if (!existsSync(join(dest, 'manifest.json'))) {
      await writeFile(join(dest, 'manifest.json'), `${JSON.stringify({ ...ctx.site.manifest, name: basename(dest, EXTENSION) }, null, 2)}\n`);
      wroteManifest = true;
    }
    return { from, to: dest, wroteManifest, summary: `Copied ${from} to ${dest}${wroteManifest ? ' (with a manifest.json for it)' : ''}` };
  },
  format: (r) => r.summary,
};

/** @type {import('../registry.mjs').Command} */
const palette = {
  name: 'palette.get',
  summary: 'List the palette: every color name, hex and tier',
  description: 'World colors are for every object; outfit colors only for objects that extend character.',
  params: { type: 'object', properties: {} },
  args: [],
  mutates: false,
  async run(ctx) {
    const { sources } = await ctx.sources();
    const doc = await ctx.doc('palette');
    const colors = [...sources.colors].map(([name, { hex, tier }]) => ({ name, hex, tier }));
    return { world: { ...doc.world }, outfit: { ...doc.outfit }, colors };
  },
  format: (r) => r.colors.map((/** @type {any} */ c) => `${c.tier.padEnd(7)} ${c.hex}  ${c.name}`).join('\n'),
};

export default [info, make, copy, palette];
