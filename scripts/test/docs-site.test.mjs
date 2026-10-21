// The developer docs site (docs/intents/2026-10-dev-docs/spec.md): the
// Markdown renderer, GitHub's heading ids, link rewriting and checking, the
// full build of the repo's docs, and the deploy workflow. No site checkout
// needed.

import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { after, before, describe, test } from 'node:test';
import { fileURLToPath } from 'node:url';

import { buildSite, findIntents, outPath, PAGES, REPO_URL } from '../docs-site/build.mjs';
import { createSlugger, renderInline, renderMarkdown } from '../docs-site/markdown.mjs';

const REPO = fileURLToPath(new URL('../..', import.meta.url));
const tempDir = (/** @type {string} */ prefix = 'pixellab-test-') => mkdtemp(join(tmpdir(), prefix));

const cleanup = (/** @type {string} */ dir) => rm(dir, { recursive: true, force: true });

const md = (/** @type {string} */ src) => renderMarkdown(src).html;

describe('docs site: Markdown', () => {
  test('R4: heading ids are GitHub\'s, with -1, -2 for repeats', () => {
    const slug = createSlugger();
    assert.equal(slug('Setup for Claude Code'), 'setup-for-claude-code');
    assert.equal(slug('scene.place'), 'sceneplace');
    assert.equal(slug('Board 03 look ("Field notebook")'), 'board-03-look-field-notebook');
    assert.equal(slug('What "ready to use" means'), 'what-ready-to-use-means');
    assert.equal(slug('build-app.sh'), 'build-appsh');
    assert.equal(slug('docs-site/'), 'docs-site');
    assert.equal(slug('An edit in the app, end to end'), 'an-edit-in-the-app-end-to-end');
    assert.deepEqual([slug('Usage'), slug('Usage'), slug('Usage')], ['usage', 'usage-1', 'usage-2']);
    const r = renderMarkdown('# Title\n\n## `scene.place`\n\n### **Bold** [link](x.md)\n');
    assert.equal(r.title, 'Title');
    assert.deepEqual(r.headings.map((h) => [h.level, h.text, h.id]), [
      [1, 'Title', 'title'],
      [2, 'scene.place', 'sceneplace'],
      [3, 'Bold link', 'bold-link'],
    ]);
    assert.match(r.html, /<h2 id="sceneplace"><code>scene\.place<\/code><a class="anchor" href="#sceneplace"/);
  });

  test('R3: inline: code, emphasis around code, links, images, autolinks, escapes', () => {
    assert.equal(renderInline('**`pixellab`**: a CLI'), '<strong><code>pixellab</code></strong>: a CLI');
    assert.equal(renderInline('*em* and _em_ but not snake_case_words'), '<em>em</em> and <em>em</em> but not snake_case_words');
    assert.equal(renderInline('`a <b> & c`'), '<code>a &lt;b&gt; &amp; c</code>');
    assert.equal(renderInline('`` a `tick` ``'), '<code>a `tick`</code>');
    assert.equal(renderInline('[the `cli`](cli/README.md#x "T")'), '<a href="cli/README.md#x" title="T">the <code>cli</code></a>');
    assert.equal(renderInline('![a pic](p.png)'), '<img src="p.png" alt="a pic" loading="lazy">');
    assert.equal(renderInline('see (https://example.com/a_b).'), 'see (<a href="https://example.com/a_b">https://example.com/a_b</a>).');
    assert.equal(renderInline('<https://example.com>'), '<a href="https://example.com">https://example.com</a>');
    assert.equal(renderInline('\\*not em\\* and a \\| pipe'), '*not em* and a | pipe');
    assert.equal(renderInline('raw <b>html</b> & "quotes"'), 'raw &lt;b&gt;html&lt;/b&gt; &amp; &quot;quotes&quot;');
    assert.equal(renderInline('a <!-- hidden --> b'), 'a  b');
    const seen = [];
    renderInline('[a](one.md) ![b](two.png)', { link: (h, kind) => (seen.push([h, kind]), h) });
    assert.deepEqual(seen, [['one.md', 'link'], ['two.png', 'image']]);
  });

  test('R3: blocks: lists, code in lists, tables, quotes, rules, comments', () => {
    assert.equal(md('- a\n- b\n  - c\n- d'), '<ul>\n<li>a</li>\n<li>b\n<ul>\n<li>c</li>\n</ul></li>\n<li>d</li>\n</ul>');
    assert.equal(md('3. three\n4. four'), '<ol start="3">\n<li>three</li>\n<li>four</li>\n</ol>');
    assert.equal(
      md('1. Bump it.\n2. Tag it:\n\n   ```sh\n   git tag v1\n   ```\n\nAfter.'),
      '<ol>\n<li><p>Bump it.</p></li>\n<li><p>Tag it:</p>\n<pre><code class="language-sh">git tag v1\n</code></pre></li>\n</ol>\n<p>After.</p>',
    );
    assert.equal(md('- [ ] todo\n- [x] done'), '<ul>\n<li class="task"><input type="checkbox" disabled> todo</li>\n<li class="task"><input type="checkbox" disabled checked> done</li>\n</ul>');
    assert.equal(
      md('| a | b |\n| --- | ---: |\n| `x \\| y` | 2 |\n| only |'),
      '<div class="table-wrap"><table><thead><tr><th>a</th><th style="text-align:right">b</th></tr></thead>\n' +
        '<tbody><tr><td><code>x | y</code></td><td style="text-align:right">2</td></tr>\n<tr><td>only</td><td style="text-align:right"></td></tr></tbody></table></div>',
    );
    assert.equal(md('> **Status:** done.\n>\n> - one\n> - two'), '<blockquote>\n<p><strong>Status:</strong> done.</p>\n<ul>\n<li>one</li>\n<li>two</li>\n</ul>\n</blockquote>');
    assert.equal(md('a\n\n---\n\nb'), '<p>a</p>\n<hr>\n<p>b</p>');
    assert.equal(md('<!-- Generated.\nDon\'t edit. -->\n\nText'), '<p>Text</p>');
    assert.equal(md('```\n# not a heading\n<b>\n```'), '<pre><code># not a heading\n&lt;b&gt;\n</code></pre>');
    assert.equal(md('one\ntwo\n- list'), '<p>one\ntwo</p>\n<ul>\n<li>list</li>\n</ul>');
    assert.equal(md('<div>raw</div>'), '<p>&lt;div&gt;raw&lt;/div&gt;</p>');
  });
});

describe('docs site: build', () => {
  /** @type {string} */
  let base;
  /** @type {string} */
  let out;
  /** @type {ReturnType<typeof buildSite>} */
  let result;
  const pages = [...PAGES, ...findIntents()];
  /** @type {Map<string, string>} */
  const before_ = new Map();

  before(async () => {
    base = await tempDir('pixellab-docs-');
    out = join(base, 'site');
    for (const p of pages) before_.set(p.file, createHash('sha256').update(readFileSync(join(REPO, p.file))).digest('hex'));
    result = buildSite({ out });
  });
  after(() => cleanup(base));
  const read = (/** @type {string} */ file) => readFileSync(join(out, file), 'utf8');

  test('R1, R3: every README and doc is a page, with the intents found on their own', () => {
    for (const file of ['README.md', 'docs/README.md', 'docs/architecture.md', 'docs/contributing.md', 'docs/testing.md', 'crates/pixellab/README.md', 'app/README.md', 'scripts/README.md', 'docs/cli.md', 'CLAUDE.md']) {
      assert.ok(pages.some((p) => p.file === file), file);
    }
    for (const change of readdirSync(join(REPO, 'docs', 'intents'))) {
      for (const f of readdirSync(join(REPO, 'docs', 'intents', change)).filter((x) => x.endsWith('.md'))) {
        assert.ok(pages.some((p) => p.file === `docs/intents/${change}/${f}`), `${change}/${f}`);
      }
    }
    assert.equal(outPath('README.md'), 'index.html');
    assert.equal(outPath('crates/pixellab/README.md'), 'crates/pixellab/index.html');
    assert.equal(outPath('docs/cli.md'), 'docs/cli.html');
    assert.equal(result.pages, pages.length);
    for (const p of pages) assert.ok(existsSync(join(out, outPath(p.file))), outPath(p.file));
    for (const asset of ['style.css', 'site.js', 'search-index.js', 'icon.png']) assert.ok(existsSync(join(out, 'assets', asset)), asset);
  });

  test('R3: the build never writes the docs it reads', () => {
    for (const p of pages) assert.equal(createHash('sha256').update(readFileSync(join(REPO, p.file))).digest('hex'), before_.get(p.file), p.file);
  });

  test('R4: links become page links, GitHub links or copied images', () => {
    const home = read('index.html');
    assert.ok(home.includes('href="docs/cli.html"'), 'README → docs/cli.md');
    assert.ok(home.includes('href="crates/pixellab/index.html"'), 'README → crates/pixellab/README.md');
    assert.ok(home.includes(`href="${REPO_URL}/blob/main/.github/workflows/release.yml"`), 'a source file goes to GitHub');
    assert.ok(home.includes('src="docs/screenshots/final-scene-dark.png"'), 'an image');
    assert.ok(statSync(join(out, 'docs/screenshots/final-scene-dark.png')).size > 0, 'the image is copied');
    const arch = read('docs/architecture.html');
    assert.ok(arch.includes('href="../crates/pixellab/index.html"'), 'docs/ → ../crates/pixellab/README.md');
    assert.ok(arch.includes('href="cli.html"'), 'a sibling page');
    assert.ok(arch.includes('href="contributing.html#ci"'), 'a page with an anchor');
    const cli = read('docs/cli.html');
    assert.ok(cli.includes('href="#sceneplace"') && cli.includes('id="sceneplace"'), 'the reference\'s own anchors');
    assert.ok(read('crates/pixellab/index.html').includes('href="../../app/index.html#adding-a-feature"'));
  });

  test('R6: each page has the frame: nav with the current page, contents, source link, search', () => {
    const page = read('docs/testing.html');
    assert.match(page, /<title>Testing · Pixel Lab developer docs<\/title>/);
    for (const p of pages) assert.ok(page.includes(`>${p.title.replace(/&/g, '&amp;')}</a>`), `nav: ${p.title}`);
    assert.match(page, /<a href="testing.html" aria-current="page">Testing<\/a>/);
    assert.equal((page.match(/aria-current="page"/g) ?? []).length, 1);
    assert.match(page, /<aside class="toc"[^>]*>.*<a href="#the-script-hook">The script hook<\/a>/s);
    assert.ok(page.includes(`${REPO_URL}/blob/main/docs/testing.md`), 'source link');
    assert.ok(page.includes('href="../assets/style.css"') && page.includes('src="../assets/site.js"'), 'assets, relative');
    assert.ok(page.includes('id="search-input"') && page.includes('class="theme-button"'));
    const index = read('assets/search-index.js');
    assert.match(index, /^window\.DOCS_INDEX = \[/);
    const entries = JSON.parse(index.slice('window.DOCS_INDEX = '.length).replace(/;\n$/, ''));
    assert.ok(entries.some((e) => e.u === 'docs/testing.html#the-script-hook' && e.t === 'The script hook'));
    assert.ok(entries.some((e) => e.u === 'docs/architecture.html' && e.t === 'Architecture'));
  });

  test('R7: board 03 tokens, night and day, with the toggle and no animation', () => {
    const css = readFileSync(join(REPO, 'scripts/docs-site/style.css'), 'utf8').toLowerCase();
    for (const hex of ['#f4ede0', '#eadfc8', '#efe6d6', '#e9dcc6', '#2e2418', '#5c4b39', '#3f6b45', '#1c1712', '#262019', '#211b15', '#2d251c', '#f1e8d8', '#c8b9a1', '#8cbc86', '#d8b66a']) {
      assert.ok(css.includes(hex), hex);
    }
    assert.ok(css.includes('@media (prefers-color-scheme: dark)') && css.includes(':root:not([data-theme="light"])') && css.includes(':root[data-theme="dark"]'));
    assert.ok(!/transition|animation/.test(css), 'nothing animates');
    assert.ok(css.includes(':focus-visible'), 'visible focus');
    assert.match(read('index.html'), /localStorage\.getItem\('docs-theme'\)/);
  });
});

describe('docs site: link check', () => {
  /** @type {string} */
  let base;
  before(async () => {
    base = await tempDir('pixellab-docs-fixture-');
    const files = {
      'README.md': '# Home\n\n## Part\n\n[guide](docs/guide.md#usage) [src](lib/x.mjs) [folder](lib/) [top](#part)\n',
      'docs/guide.md': '# Guide\n\n## Usage\n\n![shot](shot.png) [home](../README.md)\n',
      'docs/shot.png': 'png',
      'lib/x.mjs': '',
    };
    for (const [file, text] of Object.entries(files)) {
      await mkdir(join(base, 'repo', file, '..'), { recursive: true });
      await writeFile(join(base, 'repo', file), text);
    }
  });
  after(() => cleanup(base));
  const pages = [
    { section: 'Start', file: 'README.md', title: 'Home' },
    { section: 'Guides', file: 'docs/guide.md', title: 'Guide' },
  ];
  const build = (/** @type {Record<string, string>} */ change = {}) => {
    for (const [file, text] of Object.entries(change)) writeFileSync(join(base, 'repo', file), text);
    return buildSite({ root: join(base, 'repo'), out: join(base, 'out'), pages });
  };

  test('R4: a good fixture builds: page links, anchors, GitHub links for files and folders, images', () => {
    const r = build();
    assert.equal(r.pages, 2);
    assert.equal(r.images, 1);
    const home = readFileSync(join(base, 'out', 'index.html'), 'utf8');
    assert.ok(home.includes('href="docs/guide.html#usage"'));
    assert.ok(home.includes(`href="${REPO_URL}/blob/main/lib/x.mjs"`));
    assert.ok(home.includes(`href="${REPO_URL}/tree/main/lib"`));
    assert.ok(readFileSync(join(base, 'out', 'docs', 'guide.html'), 'utf8').includes('href="../index.html"'));
    assert.ok(existsSync(join(base, 'out', 'docs', 'shot.png')));
  });

  test('R5: a broken link, anchor or image fails the build, naming the file and the link', () => {
    const good = readFileSync(join(base, 'repo', 'docs', 'guide.md'), 'utf8');
    assert.throws(() => build({ 'docs/guide.md': `${good}\n[gone](missing.md)\n` }), /docs\/guide\.md: missing\.md is a broken link/);
    assert.throws(() => build({ 'docs/guide.md': `${good}\n[gone](../README.md#nope)\n` }), /docs\/guide\.md: \.\.\/README\.md#nope is a broken anchor/);
    assert.throws(() => build({ 'docs/guide.md': `${good}\n[here](#nope)\n` }), /broken anchor \(no #nope in docs\/guide\.md\)/);
    assert.throws(() => build({ 'docs/guide.md': `${good}\n![gone](gone.png)\n` }), /gone\.png is a broken image/);
    assert.throws(() => build({ 'docs/guide.md': `${good}\n[out](../../elsewhere.md)\n` }), /points outside the repo/);
    build({ 'docs/guide.md': good });
  });

  test('R5: it won\'t replace a folder that isn\'t an earlier build', async () => {
    const other = join(base, 'other');
    await mkdir(other, { recursive: true });
    await writeFile(join(other, 'keep.txt'), 'keep');
    assert.throws(() => buildSite({ root: join(base, 'repo'), out: other, pages }), /isn't empty and isn't an earlier build/);
    assert.throws(() => buildSite({ root: join(base, 'repo'), out: join(base, 'repo'), pages }), /holds the repo/);
    assert.equal(readFileSync(join(other, 'keep.txt'), 'utf8'), 'keep');
  });
});

describe('docs site: workflow', () => {
  const yml = readFileSync(join(REPO, '.github/workflows/docs.yml'), 'utf8');

  test('R8: builds every PR, deploys main, least privilege, reference checked first', () => {
    assert.match(yml, /^ {2}pull_request:$/m);
    assert.match(yml, /^ {2}push:\n {4}branches: \[main\]$/m);
    assert.match(yml, /^ {2}workflow_dispatch:$/m);
    assert.match(yml, /^permissions:\n {2}contents: read$/m);
    assert.match(yml, /node scripts\/docs-cli\.mjs --check[\s\S]*node --test scripts\/test\/docs-site\.test\.mjs[\s\S]*npm run docs:site[\s\S]*actions\/upload-pages-artifact@/);
    const deploy = yml.slice(yml.indexOf('\n  deploy:'));
    assert.match(deploy, /if: github\.event_name != 'pull_request' && github\.ref == 'refs\/heads\/main'/);
    assert.match(deploy, /permissions:\n {6}pages: write\n {6}id-token: write/);
    assert.match(deploy, /actions\/deploy-pages@/);
    assert.match(deploy, /cancel-in-progress: false/);
  });

  test('R9: the docs say how to build, add a page and deploy', () => {
    const contributing = readFileSync(join(REPO, 'docs/contributing.md'), 'utf8');
    for (const part of ['npm run docs:site', 'To add a page', 'Source: GitHub Actions', 'fails on a broken link']) assert.ok(contributing.includes(part), part);
    assert.ok(readFileSync(join(REPO, 'README.md'), 'utf8').includes('npm run docs:site'));
    assert.ok(readFileSync(join(REPO, 'scripts/README.md'), 'utf8').includes('npm run docs:site'));
  });
});
