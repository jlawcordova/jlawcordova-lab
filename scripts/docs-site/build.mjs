#!/usr/bin/env node
// Builds the developer docs site from the README files and the docs under
// docs/ (docs/intents/2026-10-dev-docs/spec.md): one HTML page per Markdown
// file, at its path with .md → .html (a README.md is its folder's
// index.html), every link relative so the site works under any base path.
// Node built-ins only. It reads the docs and never writes them.
//
//   npm run docs:site                      → build/docs-site/
//   node scripts/docs-site/build.mjs --out <dir>
//
// Links are written as they work on GitHub; the build turns .md links into
// page links, copies images, sends links to files that aren't pages to
// GitHub, and fails on a broken link, anchor or image.

import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, posix, relative, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { escapeHtml, renderMarkdown } from './markdown.mjs';

export const REPO_ROOT = fileURLToPath(new URL('../..', import.meta.url));
export const REPO_URL = 'https://github.com/jlawcordova/jlawcordova-lab';
export const SITE_TITLE = 'Pixel Lab developer docs';
const ASSETS = fileURLToPath(new URL('.', import.meta.url));

/** @typedef {{ file: string, section: string, title: string, group?: string }} Page */

/** The pages, in sidebar order. docs/intents/ is added by findIntents(). */
export const PAGES = /** @type {Page[]} */ ([
  { section: 'Start', file: 'README.md', title: 'Pixel Lab for Mac' },
  { section: 'Start', file: 'docs/README.md', title: 'Developer docs' },
  { section: 'Guides', file: 'docs/architecture.md', title: 'Architecture' },
  { section: 'Guides', file: 'docs/contributing.md', title: 'Contributing' },
  { section: 'Guides', file: 'docs/testing.md', title: 'Testing' },
  { section: 'Code', file: 'cli/README.md', title: 'CLI and servers' },
  { section: 'Code', file: 'app/README.md', title: 'Mac app' },
  { section: 'Code', file: 'scripts/README.md', title: 'Scripts' },
  { section: 'Code', file: 'examples/README.md', title: 'Example documents' },
  { section: 'Reference', file: 'docs/document-format.md', title: 'Document format' },
  { section: 'Reference', file: 'docs/site-contract.md', title: 'Site contract' },
  { section: 'Reference', file: 'docs/cli.md', title: 'Commands and protocols' },
  { section: 'Reference', file: 'CLAUDE.md', title: 'CLAUDE.md' },
]);

const STAGES = ['intent.md', 'spec.md', 'plan.md'];
const IMAGE = /\.(png|jpe?g|gif|svg|webp)$/i;

/** Every change under docs/intents/, newest first; intent, spec, plan, then any notes. */
export function findIntents(root = REPO_ROOT) {
  const dir = join(root, 'docs', 'intents');
  if (!existsSync(dir)) return [];
  /** @type {Page[]} */
  const pages = [];
  for (const change of readdirSync(dir).filter((d) => statSync(join(dir, d)).isDirectory()).sort().reverse()) {
    const files = readdirSync(join(dir, change)).filter((f) => f.endsWith('.md'));
    files.sort((a, b) => rank(a) - rank(b) || (a < b ? -1 : 1));
    for (const f of files) {
      const title = STAGES.includes(f) ? f[0].toUpperCase() + f.slice(1, -3) : f.slice(0, -3);
      pages.push({ section: 'Design record', group: change, file: `docs/intents/${change}/${f}`, title });
    }
  }
  return pages;
}
const rank = (/** @type {string} */ f) => (STAGES.includes(f) ? STAGES.indexOf(f) : STAGES.length);

/** Where a source file's page goes: docs/cli.md → docs/cli.html, cli/README.md → cli/index.html. */
export function outPath(/** @type {string} */ file) {
  return file.replace(/(^|\/)README\.md$/, '$1index.html').replace(/\.md$/, '.html');
}

/** A relative URL from one output file to another. */
function relativeUrl(/** @type {string} */ fromFile, /** @type {string} */ toFile) {
  const rel = posix.relative(posix.dirname(fromFile), toFile);
  return rel === '' ? posix.basename(toFile) : rel;
}

/**
 * Builds the site.
 * @param {{ root?: string, out?: string, pages?: Page[] }} [options]
 * @returns {{ out: string, pages: number, images: number }}
 */
export function buildSite({ root = REPO_ROOT, out = join(root, 'build', 'docs-site'), pages } = {}) {
  const list = pages ?? [...PAGES, ...findIntents(root)];
  const byFile = new Map(list.map((p) => [p.file, p]));
  /** @type {string[]} */
  const problems = [];
  /** @type {{ from: string, target: string, hash: string, href: string }[]} */
  const anchorChecks = [];
  /** @type {Set<string>} */
  const images = new Set();

  for (const p of list) if (!existsSync(join(root, p.file))) problems.push(`${p.file}: listed as a page but missing`);
  if (problems.length) throw new Error(`docs site: ${problems.length} problem(s)\n  ${problems.join('\n  ')}`);

  const rendered = list.map((page) => {
    const src = readFileSync(join(root, page.file), 'utf8');
    const here = outPath(page.file);
    const link = (/** @type {string} */ href, /** @type {'link' | 'image'} */ kind) => {
      if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith('//')) return href;
      const [pathPart, ...rest] = href.split('#');
      const hash = rest.length ? rest.join('#') : '';
      if (pathPart === '') {
        anchorChecks.push({ from: page.file, target: page.file, hash, href });
        return href;
      }
      let target = posix.normalize(posix.join(posix.dirname(page.file), decodeURI(pathPart)));
      if (target.startsWith('../') || target === '..') {
        problems.push(`${page.file}: ${href} points outside the repo`);
        return href;
      }
      target = target.replace(/\/$/, '');
      const abs = join(root, target);
      const isDir = existsSync(abs) && statSync(abs).isDirectory();
      if (isDir && byFile.has(posix.join(target, 'README.md'))) target = posix.join(target, 'README.md');
      const withHash = (/** @type {string} */ url) => (rest.length ? `${url}#${hash}` : url);
      if (byFile.has(target)) {
        if (rest.length) anchorChecks.push({ from: page.file, target, hash, href });
        return withHash(relativeUrl(here, outPath(target)));
      }
      if (!existsSync(abs)) {
        problems.push(`${page.file}: ${href} is a broken ${kind === 'image' ? 'image' : 'link'} (no ${target})`);
        return href;
      }
      if (kind === 'image' || IMAGE.test(target)) {
        images.add(target);
        return withHash(relativeUrl(here, target));
      }
      return withHash(`${REPO_URL}/${isDir ? 'tree' : 'blob'}/main/${target}`);
    };
    const r = renderMarkdown(src, { link });
    return { page, here, ...r };
  });

  const ids = new Map(rendered.map((r) => [r.page.file, new Set(r.headings.map((h) => h.id))]));
  for (const { from, target, hash, href } of anchorChecks) {
    if (hash === '' || hash === 'top') continue;
    if (!ids.get(target)?.has(decodeURIComponent(hash))) problems.push(`${from}: ${href} is a broken anchor (no #${hash} in ${target})`);
  }
  if (problems.length) throw new Error(`docs site: ${problems.length} problem(s)\n  ${problems.join('\n  ')}`);

  out = resolve(out);
  // It replaces the folder: refuse anything that isn't empty or an earlier build.
  const up = relative(out, resolve(root));
  if (up === '' || !up.startsWith('..')) throw new Error(`docs site: won't write into ${out}, which holds the repo`);
  if (existsSync(out) && readdirSync(out).length && !existsSync(join(out, 'assets', 'site.js'))) {
    throw new Error(`docs site: ${out} isn't empty and isn't an earlier build; pick another --out`);
  }
  rmSync(out, { recursive: true, force: true });
  mkdirSync(join(out, 'assets'), { recursive: true });
  for (const r of rendered) {
    const file = join(out, r.here);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, frame(r, list));
  }
  for (const img of images) {
    mkdirSync(dirname(join(out, img)), { recursive: true });
    cpSync(join(root, img), join(out, img));
  }
  for (const asset of ['style.css', 'site.js']) cpSync(join(ASSETS, asset), join(out, 'assets', asset));
  const icon = join(root, 'app/Sources/PixelLab/Resources/AppIcon.png');
  if (existsSync(icon)) cpSync(icon, join(out, 'assets', 'icon.png'));
  const index = rendered.flatMap((r) => [
    { t: r.page.title, u: r.here, p: r.page.section === 'Design record' ? `${r.page.group}` : r.page.section },
    ...r.headings.filter((h) => h.level >= 2 && h.level <= 4).map((h) => ({ t: h.text, u: `${r.here}#${h.id}`, p: r.page.title })),
  ]);
  writeFileSync(join(out, 'assets', 'search-index.js'), `window.DOCS_INDEX = ${JSON.stringify(index)};\n`);
  return { out, pages: rendered.length, images: images.size };
}

/**
 * One page in the site's frame: top bar, sidebar, the doc, its contents, footer.
 * @param {{ page: Page, here: string, html: string, title: string | null, headings: import('./markdown.mjs').Heading[] }} r
 * @param {Page[]} list
 */
function frame(r, list) {
  const root = relativeUrl(r.here, 'index.html').replace(/index\.html$/, '');
  const url = (/** @type {string} */ to) => relativeUrl(r.here, to);
  const title = r.title ?? r.page.title;

  /** @type {Map<string, Page[]>} */
  const sections = new Map();
  for (const p of list) {
    if (!sections.has(p.section)) sections.set(p.section, []);
    /** @type {Page[]} */ (sections.get(p.section)).push(p);
  }
  const navLink = (/** @type {Page} */ p) => {
    const current = p.file === r.page.file;
    return `<li><a href="${escapeHtml(url(outPath(p.file)))}"${current ? ' aria-current="page"' : ''}>${escapeHtml(p.title)}</a></li>`;
  };
  const nav = [...sections]
    .map(([section, pages]) => {
      if (section !== 'Design record') return `<section><h2 class="label">${escapeHtml(section)}</h2><ul>${pages.map(navLink).join('')}</ul></section>`;
      /** @type {Map<string, Page[]>} */
      const groups = new Map();
      for (const p of pages) {
        const g = p.group ?? '';
        if (!groups.has(g)) groups.set(g, []);
        /** @type {Page[]} */ (groups.get(g)).push(p);
      }
      const inner = [...groups].map(([g, ps]) => `<li class="group"><span class="group-name">${escapeHtml(g)}</span><ul>${ps.map(navLink).join('')}</ul></li>`).join('');
      return `<section><h2 class="label">${escapeHtml(section)}</h2><ul>${inner}</ul></section>`;
    })
    .join('\n');

  const toc = r.headings.filter((h) => h.level === 2 || h.level === 3);
  const tocHtml = toc.length
    ? `<aside class="toc" aria-labelledby="toc-label"><h2 class="label" id="toc-label">On this page</h2><ul>${toc
        .map((h) => `<li class="l${h.level}"><a href="#${escapeHtml(h.id)}">${escapeHtml(h.text)}</a></li>`)
        .join('')}</ul></aside>`
    : '';
  const crumb = r.page.section === 'Design record' ? `${r.page.section} · ${r.page.group}` : r.page.section;

  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(title)} · ${SITE_TITLE}</title>
<meta name="description" content="Developer docs for Pixel Lab, the pixellab CLI and MCP server and the Mac app: ${escapeHtml(r.page.title)}.">
<link rel="icon" href="${root}assets/icon.png">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Silkscreen&amp;family=Sora:wght@400;600;700&amp;display=swap">
<link rel="stylesheet" href="${root}assets/style.css">
<script>try{var t=localStorage.getItem('docs-theme');if(t==='light'||t==='dark')document.documentElement.dataset.theme=t}catch(e){}</script>
</head>
<body data-root="${root}">
<a class="skip" href="#content">Skip to content</a>
<header class="topbar">
  <button class="menu-button" type="button" aria-expanded="false" aria-controls="sidebar">Menu</button>
  <a class="brand" href="${root}index.html" aria-label="Pixel Lab developer docs: home"><img src="${root}assets/icon.png" alt="" width="28" height="28"><span>Pixel Lab</span><span class="brand-sub">developer docs</span></a>
  <div class="search" role="search">
    <label class="visually-hidden" for="search-input">Search the docs</label>
    <input id="search-input" type="search" placeholder="Search  /" autocomplete="off" spellcheck="false" aria-controls="search-results" aria-expanded="false" aria-autocomplete="list" role="combobox">
    <ul id="search-results" role="listbox" aria-label="Results" hidden></ul>
  </div>
  <button class="theme-button" type="button" aria-label="Theme: auto">Auto</button>
</header>
<div class="layout">
<nav class="sidebar" id="sidebar" aria-label="Pages">
${nav}
</nav>
<main id="content" class="content" tabindex="-1">
<p class="crumb label">${escapeHtml(crumb)}</p>
<article class="doc">
${r.html}
</article>
<footer class="page-foot">
<p>Source: <a href="${REPO_URL}/blob/main/${escapeHtml(r.page.file)}"><code>${escapeHtml(r.page.file)}</code></a> · built by <code>npm run docs:site</code></p>
</footer>
</main>
${tocHtml}
</div>
<script src="${root}assets/search-index.js"></script>
<script src="${root}assets/site.js"></script>
</body>
</html>
`;
}

function main(/** @type {string[]} */ argv) {
  const i = argv.indexOf('--out');
  const out = i >= 0 ? argv[i + 1] : undefined;
  if (i >= 0 && !out) throw new Error('--out needs a folder');
  const r = buildSite({ out });
  process.stdout.write(`wrote ${r.out} (${r.pages} pages, ${r.images} images)\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main(process.argv.slice(2));
  } catch (err) {
    process.stderr.write(`${/** @type {Error} */ (err).message}\n`);
    process.exitCode = 1;
  }
}
