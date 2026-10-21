// The example documents in examples/ (scripts/make-examples.mjs), checked through the
// Rust program: documents in format 2 with scene layers, to try the app on and to test
// with (docs/intents/2026-10-scene-layers/, docs/intents/2026-10-rust-engine/spec.md R10
// and R29). They must be what the generator writes, check clean and canonical, be in
// depth order, and show what layers promise: nesting, the site's wrappers beside them,
// and the same SVG without them.
//
// `npm test` builds the program first (cargo build --release).

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { after, describe, test } from 'node:test';
import { fileURLToPath } from 'node:url';

import { examples, writeExamples } from '../make-examples.mjs';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const PROGRAM = join(REPO, 'target/release/pixellab');
const EXAMPLES = join(REPO, 'examples');
const skip = existsSync(PROGRAM) ? false : `no program at ${PROGRAM}: run cargo build --release`;

const tmp = mkdtempSync(join(tmpdir(), 'pixellab-examples-'));
after(() => rmSync(tmp, { recursive: true, force: true }));

/** Runs the program on a document, with a temp HOME so no config file is read. */
function run(/** @type {string} */ doc, /** @type {string[]} */ args) {
  const r = spawnSync(PROGRAM, [...args, '--document', doc], { encoding: 'utf8', env: { ...process.env, HOME: tmp, PIXELLAB_STATE: join(tmp, 'state') } });
  return { code: r.status, stdout: r.stdout, stderr: r.stderr };
}

/** The JSON a command prints. */
function json(/** @type {string} */ doc, /** @type {string[]} */ args) {
  const r = run(doc, [...args, '--json']);
  assert.equal(r.code, 0, `${args.join(' ')}: ${r.stdout}${r.stderr}`);
  return JSON.parse(r.stdout);
}

/** Every file under a folder, relative to it. */
function filesIn(/** @type {string} */ dir) {
  return readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter((e) => e.isFile() && e.name !== '.DS_Store')
    .map((e) => join(e.parentPath ?? /** @type {any} */ (e).path, e.name).slice(dir.length + 1))
    .sort();
}

describe('the example documents (R10, R29)', { skip }, () => {
  test('R10: examples/ is what scripts/make-examples.mjs writes (npm run examples)', async () => {
    const out = join(tmp, 'made');
    await writeExamples(out);
    const made = filesIn(out);
    assert.deepEqual(filesIn(EXAMPLES).filter((p) => p.includes('.pixellab/')), made);
    for (const file of made) assert.equal(readFileSync(join(EXAMPLES, file), 'utf8'), readFileSync(join(out, file), 'utf8'), `${file} is out of date: run npm run examples`);
  });

  test('R10: running the generator twice changes nothing', async () => {
    const a = join(tmp, 'twice-a');
    const b = join(tmp, 'twice-b');
    await writeExamples(a);
    await writeExamples(b);
    for (const file of filesIn(a)) assert.equal(readFileSync(join(a, file), 'utf8'), readFileSync(join(b, file), 'utf8'), file);
  });

  test('R10, R29: every example is in format 2, checks clean and is canonical', () => {
    for (const name of Object.keys(examples())) {
      const doc = join(EXAMPLES, `${name}.pixellab`);
      assert.equal(JSON.parse(readFileSync(join(doc, 'manifest.json'), 'utf8')).format, 2, `${name} is format 2`);
      assert.deepEqual(readdirSync(doc).filter((f) => f.endsWith('.mjs')), [], `${name} has no .mjs sources`);
      const r = json(doc, ['check']);
      assert.equal(r.ok, true);
      assert.deepEqual(r.problems, []);
      // Only the budget and moving-copies notes are expected: nothing out of order or off the canonical form.
      assert.deepEqual([...new Set(r.warnings.map((/** @type {any} */ w) => w.code))].filter((c) => c !== 'copies' && c !== 'over_budget'), [], `${name}: ${JSON.stringify(r.warnings)}`);
    }
  });

  test('depth order R4: every example scene is in depth order already', () => {
    for (const [name, ex] of Object.entries(examples())) {
      const doc = join(EXAMPLES, `${name}.pixellab`);
      for (const { name: scene } of ex.scenes) {
        const r = json(doc, ['scene', 'sort', scene, '--dry-run']);
        assert.equal(r.changed, false, `${name}/${scene} is out of depth order: run npm run examples`);
        assert.equal(r.changedPixels, 0);
      }
      assert.ok(!json(doc, ['check']).warnings.some((/** @type {any} */ w) => w.code === 'out_of_order'));
    }
  });

  test('R1, R3: the harbor nests layers three deep beside the site\'s { group } wrappers, with items in no layer', () => {
    const doc = join(EXAMPLES, 'harbor.pixellab');
    const r = json(doc, ['scene', 'items', 'harbor']);
    const byId = new Map(r.layers.map((/** @type {any} */ l) => [l.id, l]));
    assert.equal(r.layers.length, 16);
    assert.deepEqual(['red-house', 'houses', 'village'].map((id) => byId.get(id).parent ?? null), ['houses', 'village', null]);
    assert.ok(r.groups.length > 50, 'most items sit in entrance wrappers');
    assert.ok(r.groups.every((/** @type {any} */ g) => g.attrs.class === 'enter'));
    assert.deepEqual(r.items.filter((/** @type {any} */ i) => !i.layer).map((/** @type {any} */ i) => i.object), ['rock', 'rock']);
    const flat = json(doc, ['scene', 'items', 'harbor-flat']);
    assert.deepEqual([flat.layers, flat.items.length], [[], r.items.length]);
  });

  test('R10: the harbor has an ambient loop on the barrel and a hop action on the boat, in both scenes', () => {
    const doc = join(EXAMPLES, 'harbor.pixellab');
    for (const scene of ['harbor', 'harbor-flat']) {
      const source = JSON.parse(readFileSync(join(doc, 'scenes', `${scene}.json`), 'utf8'));
      const all = (/** @type {any[]} */ items) => items.flatMap((i) => ('group' in i ? all(i.items) : [i]));
      const items = all(source.items);
      assert.equal(items.find((i) => i.id === 'barrel').ambient.duration, 3000);
      assert.deepEqual(Object.keys(items.find((i) => i.id === 'boat').actions), ['hop']);
    }
  });

  test('R2: the harbor and harbor-flat (the same scene without layers) publish the same SVG (apart from the scene name each is scoped by)', () => {
    const doc = join(EXAMPLES, 'harbor.pixellab');
    const out = join(tmp, 'harbor-out');
    const r = run(doc, ['art', 'build', '--dir', out]);
    assert.equal(r.code, 0, r.stderr);
    // Every generated name and selector starts with the scene's name, so only that differs.
    const flat = readFileSync(join(out, 'harbor-flat.svg'), 'utf8').replaceAll('harbor-flat', 'harbor');
    assert.ok(flat === readFileSync(join(out, 'harbor.svg'), 'utf8'));
  });

  test('R1, R5: the town has 59 layers four deep, and moving a district moves its houses', () => {
    const copy = join(tmp, 'town-copy.pixellab');
    cpSync(join(EXAMPLES, 'town.pixellab'), copy, { recursive: true });
    const r = json(copy, ['scene', 'items', 'town']);
    assert.equal(r.layers.length, 59);
    const parentOf = new Map(r.layers.map((/** @type {any} */ l) => [l.id, l.parent ?? null]));
    assert.deepEqual([parentOf.get('nw-street-1-house-1'), parentOf.get('nw-street-1'), parentOf.get('nw')], ['nw-street-1', 'nw', null]);
    const nw = r.layers.find((/** @type {any} */ l) => l.id === 'nw').items;
    assert.equal(nw > 18, true);
    const moved = json(copy, ['scene', 'layer', 'move', 'town', 'nw', '--by', '0,-1']);
    assert.equal(moved.moved, nw);
  });
});
