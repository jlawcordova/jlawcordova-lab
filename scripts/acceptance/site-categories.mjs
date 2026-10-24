// node scripts/acceptance/site-categories.mjs <site> <state> <out> <real-site>: the owner's art in categories
// (docs/intents/2026-10-object-libraries/spec.md, R24–R26), on temp copies only.
//
// 1. The temp site's document gets Appendix A's categories: no problems, nothing left in Others, and every
//    published SVG byte-identical.
// 2. A copy of the explainer document (git archive of the real site, read-only) gets Appendix B's: no problems,
//    nothing left in Others.
// 3. The explainer copy is exported as a library.
// 4. It's imported into a new document, then into the site copy: the import reports its renames and the
//    character swap, and the site's published SVGs stay byte-identical.
//
// The categories are read from the spec's appendices, so the spec and this check never drift apart.
import { execFileSync } from 'node:child_process';
import { cpSync, mkdirSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';

const [site, state, out, real] = process.argv.slice(2);
const program = process.env.PIXELLAB_PROGRAM ?? new URL('../../target/release/pixellab', import.meta.url).pathname;
const spec = readFileSync(new URL('../../docs/intents/2026-10-object-libraries/spec.md', import.meta.url), 'utf8');
const work = join(out, 'site-categories');
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

let failed = 0;
const fail = (/** @type {string} */ m) => { console.log(`FAIL ${m}`); failed++; };
const ok = (/** @type {string} */ m) => console.log(`ok   ${m}`);

/** Runs the program on a document; its JSON result, or the refusal's JSON when it refuses and `refusal` is set. */
function px(/** @type {string} */ doc, /** @type {string[]} */ args, { refusal = false } = {}) {
  try {
    const stdout = execFileSync(program, [...args, '--document', doc, '--json'], { env: { ...process.env, PIXELLAB_STATE: state }, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
    return JSON.parse(stdout);
  } catch (e) {
    const err = /** @type {any} */ (e);
    if (refusal) return JSON.parse(err.stdout || err.stderr);
    throw new Error(`pixellab ${args.join(' ')}: ${err.stderr || err.stdout}`);
  }
}

/** An appendix's rows: [category path, object names]. */
function appendix(/** @type {string} */ letter) {
  const part = spec.split(`## Appendix ${letter}`)[1].split('## Appendix')[0];
  return [...part.matchAll(/^\| ([^|]+?) \| (.+?) \|$/gm)]
    .filter(([, c]) => c !== 'Category' && !c.startsWith('---'))
    .map(([, c, names]) => /** @type {[string, string[]]} */ ([c, names.split(',').map((n) => n.trim())]));
}

/** Sets every row's category, one call per row, and checks nothing is left without one. */
function categorize(/** @type {string} */ doc, /** @type {[string, string[]][]} */ rows, /** @type {string} */ what) {
  for (const [category, names] of rows) px(doc, ['object', 'category', ...names, '--to', category]);
  const problems = px(doc, ['check']).problems ?? [];
  problems.length ? fail(`${what}: ${problems.length} problems after categorizing: ${problems.slice(0, 3).join('; ')}`) : ok(`${what}: no problems`);
  const left = px(doc, ['object', 'list']).objects.filter((/** @type {any} */ o) => !o.category);
  left.length ? fail(`${what}: ${left.length} objects left in Others: ${left.map((/** @type {any} */ o) => o.name).join(', ')}`) : ok(`${what}: every object has a category`);
}

/** The published SVGs' bytes, by file name. */
function published(/** @type {string} */ doc, /** @type {string} */ dir) {
  rmSync(dir, { recursive: true, force: true });
  px(doc, ['art', 'build', '--dir', dir]);
  return Object.fromEntries(readdirSync(dir).filter((f) => f.endsWith('.svg')).map((f) => [f, readFileSync(join(dir, f), 'utf8')]));
}

function same(/** @type {Record<string,string>} */ a, /** @type {Record<string,string>} */ b, /** @type {string} */ what) {
  const files = Object.keys(a);
  const differ = files.filter((f) => a[f] !== b[f]);
  if (!files.length) fail(`${what}: no SVGs were published`);
  else if (differ.length || Object.keys(b).length !== files.length) fail(`${what}: ${differ.join(', ') || 'the set of SVGs'} changed`);
  else ok(`${what}: ${files.length} SVGs byte-identical`);
}

// 1. The site's document (R24, R26.1).
const siteDoc = join(work, 'jlawcordova.pixellab');
cpSync(join(site, 'src/assets/pixel-art/jlawcordova.pixellab'), siteDoc, { recursive: true });
const before = published(siteDoc, join(work, 'svg-before'));
categorize(siteDoc, appendix('A'), 'the site document');
const categorized = published(siteDoc, join(work, 'svg-categorized'));
same(before, categorized, 'the site document, categorized');

// 2. The explainer document, from the real site's last commit, read-only (R24, R26.2).
const explainer = join(work, 'explainer-library.pixellab');
execFileSync('sh', ['-c', `git -C "$1" archive HEAD art/explainer-library.pixellab | tar -x -C "$2"`, 'sh', real, work]);
cpSync(join(work, 'art/explainer-library.pixellab'), explainer, { recursive: true });
categorize(explainer, appendix('B'), 'the explainer document');

// 3. The explainer as a library (R25, R26.3).
const lib = join(work, 'explainer.pixellib');
const exported = px(explainer, ['library', 'export', lib]);
const count = exported.objects?.length ?? 0;
count === px(explainer, ['object', 'list']).objects.length ? ok(`exported ${count} objects to explainer.pixellib`) : fail(`exported ${count} objects, not every one`);

// 4a. Into a new document: its starter palette may lack or differ from some colors; convert each to the nearest.
const fresh = join(work, 'fresh.pixellab');
execFileSync(program, ['document', 'new', fresh], { env: { ...process.env, PIXELLAB_STATE: state } });
importInto(fresh, 'a new document');

// 4b. Into the site's document copy: renames and the character swap reported, the published art unchanged (R16).
const plan = importInto(siteDoc, 'the site document');
(plan.renames?.length ?? 0) > 0 ? ok(`the site import renames ${plan.renames.map((/** @type {any} */ r) => `${r.from} → ${r.to}`).join(", ")}`) : fail('the site import reported no renames');
plan.character?.swapped ? ok('the site import swaps in the document\'s character, as a difference') : fail('the site import did not report the character swap');
(plan.differing?.length || plan.missing?.length) ? fail(`the site import has ${plan.differing?.length ?? 0} differing and ${plan.missing?.length ?? 0} missing colors`) : ok('the site import\'s colors match the document\'s');
same(before, published(siteDoc, join(work, 'svg-imported')), 'the site document, after the import');

/** Imports the library: a dry run, then the import with every missing color converted to its nearest and the differences accepted. */
function importInto(/** @type {string} */ doc, /** @type {string} */ what) {
  const dry = px(doc, ['library', 'import', lib, '--dry-run']);
  const convert = Object.fromEntries((dry.missing ?? []).map((/** @type {any} */ m) => [m.name, m.nearest?.name]));
  const args = ['library', 'import', lib, '--accept-differences'];
  if (Object.keys(convert).length) args.push('--convert', JSON.stringify(convert));
  const done = px(doc, args);
  const problems = px(doc, ['check']).problems ?? [];
  problems.length ? fail(`${what}: ${problems.length} problems after the import: ${problems.slice(0, 3).join('; ')}`) : ok(`${what}: imported ${done.imported?.length ?? '?'} objects, no problems`);
  return dry;
}

console.log(failed ? `site-categories: ${failed} failed` : 'site-categories: all passed');
process.exitCode = failed ? 1 : 0;
