// site.info and palette.get.

import { readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

/** The checked-out branch, read from .git/HEAD (no git process, nothing written). */
function gitBranch(/** @type {string} */ root) {
  try {
    let gitDir = join(root, '.git');
    if (statSync(gitDir).isFile()) {
      const m = /^gitdir:\s*(.+)$/m.exec(readFileSync(gitDir, 'utf8'));
      if (!m) return null;
      gitDir = join(root, m[1].trim());
    }
    const head = readFileSync(join(gitDir, 'HEAD'), 'utf8').trim();
    const ref = /^ref:\s*refs\/heads\/(.+)$/.exec(head);
    return ref ? ref[1] : `(detached at ${head.slice(0, 7)})`;
  } catch {
    return null;
  }
}

/** @type {import('../registry.mjs').Command} */
const info = {
  name: 'site.info',
  summary: 'Show the site checkout in use and what it holds',
  description:
    'Where the site root came from (--site, PIXELLAB_SITE, the config file or the default), its branch, ' +
    'and how many scenes, objects and colors it has, with the count of validation problems.',
  params: { type: 'object', properties: {} },
  args: [],
  mutates: false,
  async run(ctx) {
    const { site } = ctx;
    const { sources, loadErrors } = await ctx.sources();
    const problems = [...loadErrors, ...site.engine.validate(sources)];
    /** @type {Record<string, number>} */
    const tiers = {};
    for (const { tier } of sources.colors.values()) tiers[tier] = (tiers[tier] ?? 0) + 1;
    return {
      root: site.root,
      from: site.from,
      sourceDir: site.sourceDir,
      branch: gitBranch(site.root),
      node: process.version,
      scenes: sources.scenes.size,
      objects: sources.objects.size,
      colors: tiers,
      problems: problems.length,
    };
  },
  format: (r) =>
    [
      `site      ${r.root} (from ${r.from})`,
      `branch    ${r.branch ?? 'unknown'}`,
      `sources   ${r.sourceDir}`,
      `scenes    ${r.scenes}`,
      `objects   ${r.objects}`,
      `colors    ${Object.entries(r.colors).map(([t, n]) => `${n} ${t}`).join(', ')}`,
      `problems  ${r.problems}${r.problems ? ' (pixellab check)' : ''}`,
    ].join('\n'),
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

export default [info, palette];
