// Every mutating command as a one-shot command line on a temp copy of the
// site: --dry-run prints a diff and writes nothing, a real run validates and
// writes. Then validation and conflict refusals, sessions (undo, redo,
// revert, coalescing, saves), proposals, art.build and the round trip of
// every real source through the engine's serializer.

import assert from 'node:assert/strict';
import { cp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { after, before, describe, test } from 'node:test';

import { registry } from '../lib/registry.mjs';
import { readSource, Session } from '../lib/session.mjs';
import { openSite } from '../lib/document.mjs';
import { makeSite, needsSite, readDoc, runCli, SITE_SOURCE, tempDir } from './helpers.mjs';

const SRC = 'src/assets/pixel-art/source';

const FIXTURES = {
  [`${SRC}/scenes/zz-test.mjs`]: `// Pixel-art scene. How to edit it: .claude/skills/pixel-art/SKILL.md
export default {
  viewBox: [-64, -64, 128, 128],
  origin: [0, 0],
  items: [
    { object: 'pine', at: { tile: [0, 0, 0] } },
    { object: 'pine', at: { tile: [1, 0, 0] } },
    { object: 'pine', at: { px: [10, 10] } },
    { object: 'house', id: 'home', at: { tile: [2, 2, 0] } },
  ],
  motion: {
    duration: 1000,
    tracks: [
      {
        item: 'home',
        keys: [
          { t: 0 },
          { t: 500, move: [2, 1] },
        ],
      },
    ],
  },
};
`,
  [`${SRC}/objects/zz-sprite.mjs`]: `// Pixel-art object. How to edit it: .claude/skills/pixel-art/SKILL.md
export default {
  kind: 'sprite',
  anchor: [2, 2],
  keys: {
    o: 'ink',
  },
  layers: [
    {
      map: [
        'oo..',
        'o...',
        '....',
      ],
    },
    {
      loop: 'ff',
      prefix: 'f',
      frames: [
        [
          '....',
          '.o..',
          '....',
        ],
        [
          '....',
          '..o.',
          '....',
        ],
        [
          '....',
          '...o',
          '....',
        ],
        [
          '....',
          '....',
          '...o',
        ],
      ],
    },
  ],
};
`,
};

describe('edit commands (one-shot CLI)', needsSite, () => {
  /** @type {Awaited<ReturnType<typeof makeSite>>} */
  let site;
  before(async () => {
    site = await makeSite(FIXTURES);
  });
  after(() => site.cleanup());

  const cli = (/** @type {string[]} */ args, /** @type {Record<string, string>} */ env = {}) =>
    runCli([...args, '--site', site.root], { env: site.env({ PIXELLAB_STATE: join(site.base, 'state'), ...env }) });
  const path = (/** @type {string} */ file) => join(site.sourceDir, file);
  const text = (/** @type {string} */ file) => readFile(path(file), 'utf8').catch(() => null);

  /**
   * Runs a command with --dry-run (nothing written, a diff shown), then for
   * real (written), and returns the doc after.
   * @param {string[]} args
   * @param {string} file
   */
  async function edit(args, file) {
    const before = await text(file);
    const dry = await cli([...args, '--dry-run']);
    assert.equal(dry.code, 0, `${args.join(' ')} --dry-run: ${dry.stderr}`);
    assert.match(dry.stdout, new RegExp(`--- a/${file}\\n\\+\\+\\+ b/${file}\\n@@ `), args.join(' '));
    assert.match(dry.stdout, /dry run: nothing written/);
    assert.equal(await text(file), before, `${args.join(' ')} --dry-run wrote ${file}`);
    const real = await cli(args);
    assert.equal(real.code, 0, `${args.join(' ')}: ${real.stderr}`);
    assert.match(real.stdout, new RegExp(`^wrote ${file}$`, 'm'), args.join(' '));
    assert.notEqual(await text(file), before, `${args.join(' ')} changed nothing`);
    return readDoc(path(file));
  }

  const SCENE = 'scenes/zz-test.mjs';
  const SPRITE = 'objects/zz-sprite.mjs';

  test('scene.place', async () => {
    const doc = await edit(['scene', 'place', 'zz-test', 'pine', '--at', '0,1,0', '--class', 'sway'], SCENE);
    // Back to front: after the tile items with row + col 1 (and the pixel item), before [2, 2, 0].
    assert.deepEqual(doc.items[3], { object: 'pine', class: 'sway', at: { tile: [0, 1, 0] } });
    const json = await cli(['scene', 'place', 'zz-test', 'tree', '--px', '3,4', '--id', 'tall', '--json']);
    const r = JSON.parse(json.stdout);
    assert.equal(r.path, '5');
    assert.deepEqual(r.saved, [{ doc: 'scene:zz-test', file: SCENE, wrote: true, hash: r.saved[0].hash }]);
    assert.deepEqual((await readDoc(path(SCENE))).items[5], { object: 'tree', id: 'tall', at: { px: [3, 4] } });
    // With no --at or --px: the level-0 tile at the middle of the view, where the lab's cursor starts.
    const middle = JSON.parse((await cli(['scene', 'place', 'zz-test', 'pine', '--dry-run', '--json'])).stdout);
    assert.deepEqual(middle.item.at, { tile: [0, 0, 0] });
    assert.equal(middle.dryRun[0].file, SCENE);
    const bad = await cli(['scene', 'place', 'zz-test', 'no-such-thing']);
    assert.equal(bad.code, 1);
    assert.match(bad.stderr, /no object named no-such-thing/);
  });

  test('scene.move by tiles, by pixels, and a key past time 0', async () => {
    let doc = await edit(['scene', 'move', 'zz-test', '0', '--by', '0,2,1'], SCENE);
    assert.deepEqual(doc.items[0].at, { tile: [0, 2, 1] });
    doc = await edit(['scene', 'move', 'zz-test', '2', '--nudge', '-1,2'], SCENE);
    assert.deepEqual(doc.items[2].at, { px: [9, 12] });
    doc = await edit(['scene', 'move', 'zz-test', 'home', '--by', '1,0', '--time', '500'], SCENE);
    assert.deepEqual(doc.motion.tracks[0].keys[1], { t: 500, move: [18, 9] });
  });

  test('scene.set: tile, px, a field, class and id', async () => {
    let doc = await edit(['scene', 'set', 'zz-test', '1', '--tile', '4,4,0'], SCENE);
    assert.deepEqual(doc.items[1].at, { tile: [4, 4, 0] });
    doc = await edit(['scene', 'set', 'zz-test', '1', '--level', '2'], SCENE);
    assert.deepEqual(doc.items[1].at, { tile: [4, 4, 2] });
    doc = await edit(['scene', 'set', 'zz-test', '1', '--px', '7,8'], SCENE);
    assert.deepEqual(doc.items[1].at, { px: [7, 8] });
    doc = await edit(['scene', 'set', 'zz-test', 'home', '--id', 'cabin', '--class', 'warm glow'], SCENE);
    const cabin = doc.items.find((/** @type {any} */ i) => i.id === 'cabin');
    assert.deepEqual(Object.keys(cabin), ['object', 'id', 'class', 'at']);
    assert.equal(doc.motion.tracks[0].item, 'cabin');
    const wrong = await cli(['scene', 'set', 'zz-test', '1', '--col', '1']);
    assert.equal(wrong.code, 1);
    assert.match(wrong.stderr, /placed by pixel/);
  });

  test('scene.raise, scene.lower, scene.duplicate, scene.snap, scene.remove', async () => {
    const before = (await readDoc(path(SCENE))).items.map((/** @type {any} */ i) => i.object);
    let doc = await edit(['scene', 'raise', 'zz-test', '0'], SCENE);
    assert.deepEqual(doc.items.slice(0, 2).map((/** @type {any} */ i) => i.object), [before[1], before[0]]);
    doc = await edit(['scene', 'lower', 'zz-test', '1'], SCENE);
    assert.deepEqual(doc.items.map((/** @type {any} */ i) => i.object), before);
    const front = await cli(['scene', 'raise', 'zz-test', String(before.length - 1)]);
    assert.equal(front.code, 1);
    assert.match(front.stderr, /already at the front/);
    doc = await edit(['scene', 'duplicate', 'zz-test', 'cabin'], SCENE);
    const i = doc.items.findIndex((/** @type {any} */ it) => it.id === 'cabin');
    assert.equal(doc.items[i + 1].id, undefined);
    assert.deepEqual(doc.items[i + 1].at.tile, [doc.items[i].at.tile[0] + 1, doc.items[i].at.tile[1], doc.items[i].at.tile[2]]);
    doc = await edit(['scene', 'snap', 'zz-test', '1'], SCENE);
    assert.ok(doc.items[1].at.tile);
    doc = await edit(['scene', 'remove', 'zz-test', 'cabin'], SCENE);
    assert.equal(doc.items.some((/** @type {any} */ it) => it.id === 'cabin'), false);
    assert.deepEqual(doc.motion.tracks, []);
  });

  test('scene.new and object.new', async () => {
    const dry = await cli(['scene', 'new', 'zz-made', '--dry-run']);
    assert.equal(dry.code, 0, dry.stderr);
    assert.match(dry.stdout, /\+\+\+ b\/scenes\/zz-made\.mjs\n@@ -0,0 \+1,6 @@/);
    assert.equal(await text('scenes/zz-made.mjs'), null);
    const made = await cli(['scene', 'new', 'zz-made']);
    assert.equal(made.code, 0, made.stderr);
    assert.deepEqual(await readDoc(path('scenes/zz-made.mjs')), { viewBox: [-64, -64, 128, 128], origin: [0, 0], items: [] });
    const again = await cli(['scene', 'new', 'zz-made']);
    assert.equal(again.code, 1);
    assert.match(again.stderr, /already an object or scene called zz-made/);
    assert.equal((await cli(['scene', 'new', 'Bad_Name'])).code, 2);

    assert.equal((await cli(['object', 'new', 'zz-rock', '--width', '5', '--height', '3'])).code, 0);
    const rock = await readDoc(path('objects/zz-rock.mjs'));
    assert.deepEqual(rock.anchor, [2, 2]);
    assert.deepEqual(rock.layers[0].map, ['.....', '.....', '.....']);
    assert.equal((await cli(['object', 'new', 'zz-cube', '--kind', 'block'])).code, 0);
    assert.equal((await readDoc(path('objects/zz-cube.mjs'))).kind, 'block');
    assert.equal((await cli(['object', 'new', 'zz-outfit', '--extends', 'character'])).code, 0);
    assert.deepEqual(await readDoc(path('objects/zz-outfit.mjs')), { kind: 'sprite', extends: 'character', keys: {}, rows: {} });
    assert.equal((await cli(['object', 'new', 'zz-x', '--kind', 'block', '--width', '3'])).code, 2);
  });

  test('object.paint, object.erase, object.fill, object.pick', async () => {
    let doc = await edit(['object', 'paint', 'zz-sprite', '2,0', '3,2', '--color', 'grass-2'], SPRITE);
    assert.deepEqual(doc.keys, { o: 'ink', g: 'grass-2' });
    assert.deepEqual(doc.layers[0].map, ['oog.', 'o...', '...g']);
    doc = await edit(['object', 'erase', 'zz-sprite', '--pixels', '0,0 1,0'], SPRITE);
    assert.deepEqual(doc.layers[0].map[0], '..g.');
    doc = await edit(['object', 'fill', 'zz-sprite', '--x', '1', '--y', '1', '--color', 'grass-2'], SPRITE);
    assert.deepEqual(doc.layers[0].map, ['gggg', 'oggg', 'gggg']);
    doc = await edit(['object', 'paint', 'zz-sprite', '0,0', '--color', 'ink', '--layer', '1', '--frame', '2'], SPRITE);
    assert.deepEqual(doc.layers[1].frames[2][0], 'o...');
    const pick = JSON.parse((await cli(['object', 'pick', 'zz-sprite', '--x', '0', '--y', '1', '--json'])).stdout);
    assert.deepEqual([pick.color, typeof pick.hex], ['ink', 'string']);
    const outfit = await cli(['object', 'paint', 'zz-sprite', '0,0', '--color', 'silver-1']);
    assert.equal(outfit.code, 1);
    const unknown = await cli(['object', 'paint', 'zz-sprite', '0,0', '--color', 'no-such-color']);
    assert.equal(unknown.code, 2);
  });

  test('object.layer.add, duplicate, move, delete', async () => {
    let doc = await edit(['object', 'layer', 'add', 'zz-sprite'], SPRITE);
    assert.equal(doc.layers.length, 3);
    assert.deepEqual(doc.layers[2].map, ['....', '....', '....']);
    doc = await edit(['object', 'layer', 'duplicate', 'zz-sprite', '0'], SPRITE);
    assert.equal(doc.layers.length, 4);
    assert.deepEqual(doc.layers[1], doc.layers[0]);
    doc = await edit(['object', 'layer', 'move', 'zz-sprite', '3', '--by', '-3'], SPRITE);
    assert.deepEqual(doc.layers[0].map, ['....', '....', '....']);
    doc = await edit(['object', 'layer', 'delete', 'zz-sprite', '0'], SPRITE);
    assert.equal(doc.layers.length, 3);
    assert.equal((await cli(['object', 'layer', 'add', 'outfit-front-end'])).code, 1, 'an outfit takes its layers from character');
  });

  test('object.frame.add, duplicate, move, delete', async () => {
    const loop = (await readDoc(path(SPRITE))).layers.findIndex((/** @type {any} */ l) => 'frames' in l);
    const L = String(loop);
    let doc = await edit(['object', 'frame', 'add', 'zz-sprite', '--layer', L, '--frame', '0'], SPRITE);
    assert.equal(doc.layers[loop].frames.length, 5);
    assert.deepEqual(doc.layers[loop].frames[1], ['....', '....', '....']);
    doc = await edit(['object', 'frame', 'duplicate', 'zz-sprite', '--layer', L, '--frame', '0'], SPRITE);
    assert.deepEqual(doc.layers[loop].frames[1], doc.layers[loop].frames[0]);
    doc = await edit(['object', 'frame', 'move', 'zz-sprite', '--layer', L, '--frame', '2', '--by', '-2'], SPRITE);
    assert.deepEqual(doc.layers[loop].frames[0], ['....', '....', '....']);
    doc = await edit(['object', 'frame', 'delete', 'zz-sprite', '--layer', L, '--frame', '0'], SPRITE);
    assert.equal(doc.layers[loop].frames.length, 5);
    const still = await cli(['object', 'frame', 'add', 'zz-sprite', '--layer', String(loop === 0 ? 1 : 0)]);
    assert.equal(still.code, 1);
    assert.match(still.stderr, /still map/);
  });

  test('object.animate, object.still, object.resize', async () => {
    const first = (await readDoc(path(SPRITE))).layers.findIndex((/** @type {any} */ l) => 'map' in l);
    let doc = await edit(['object', 'animate', 'zz-sprite', '--layer', String(first), '--loop', 'wf'], SPRITE);
    assert.equal(doc.layers[first].loop, 'wf');
    assert.equal(doc.layers[first].frames.length, 5);
    doc = await edit(['object', 'animate', 'zz-sprite', '--layer', String(first), '--loop', 'hf'], SPRITE);
    assert.deepEqual([doc.layers[first].loop, doc.layers[first].prefix, doc.layers[first].frames.length], ['hf', 'h', 6]);
    doc = await edit(['object', 'still', 'zz-sprite', '--layer', String(first), '--frame', '1'], SPRITE);
    assert.ok('map' in doc.layers[first]);
    doc = await edit(['object', 'resize', 'zz-sprite', '--width', '6'], SPRITE);
    for (const l of doc.layers) for (const map of 'frames' in l ? l.frames : [l.map]) assert.deepEqual(map.map((/** @type {string} */ r) => r.length), [6, 6, 6]);
  });

  test('an edit that changes nothing writes nothing', async () => {
    const before = await text(SPRITE);
    const r = await cli(['object', 'erase', 'zz-sprite', '99,99']);
    assert.equal(r.code, 0, r.stderr);
    assert.match(r.stdout, /nothing written/);
    assert.equal(await text(SPRITE), before);
  });

  test('validation refusal: a doc with problems is not written unless --force', async () => {
    const file = 'scenes/zz-broken.mjs';
    await site.write(`${SRC}/${file}`, `// Pixel-art scene. How to edit it: .claude/skills/pixel-art/SKILL.md
export default {
  viewBox: [0, 0, 64, 64],
  items: [
    { object: 'pine', at: { tile: [0, 0, 0] } },
    { object: 'no-such-object', at: { tile: [1, 0, 0] } },
  ],
};
`);
    const before = await text(file);
    const r = await cli(['scene', 'move', 'zz-broken', '0', '--by', '1,0', '--json']);
    assert.equal(r.code, 1);
    const err = JSON.parse(r.stderr).error;
    assert.equal(err.code, 'invalid');
    assert.match(err.details[0], /^scenes\/zz-broken\.mjs: items\[1\]/);
    assert.equal(await text(file), before);
    const dry = await cli(['scene', 'move', 'zz-broken', '0', '--by', '1,0', '--dry-run']);
    assert.match(dry.stdout, /saving it would be refused/);
    const forced = await cli(['scene', 'move', 'zz-broken', '0', '--by', '1,0', '--force']);
    assert.equal(forced.code, 0, forced.stderr);
    assert.match(forced.stdout, /forced: 1 problem written/);
    assert.deepEqual((await readDoc(path(file))).items[0].at, { tile: [1, 0, 0] });
  });

  test('--dry-run and --force only go with edit commands; doc.* needs a session', async () => {
    assert.equal((await cli(['scene', 'list', '--dry-run'])).code, 2);
    assert.equal((await cli(['scene', 'list', '--force'])).code, 2);
    const undo = await cli(['doc', 'undo', 'scene:zz-test']);
    assert.equal(undo.code, 2);
    assert.match(undo.stderr, /pixellab serve/);
  });
});

describe('sessions', needsSite, () => {
  /** @type {Awaited<ReturnType<typeof makeSite>>} */
  let site;
  /** @type {any} */
  let opened;
  /** @type {string} */
  let state;
  before(async () => {
    site = await makeSite(FIXTURES);
    opened = await openSite({ site: site.root });
    state = await tempDir('pixellab-state-');
  });
  after(() => site.cleanup());

  const make = (/** @type {any} */ options = {}) => new Session(opened, { env: { ...process.env, PIXELLAB_STATE: state }, ...options });
  const call = (/** @type {Session} */ s, /** @type {string} */ name, /** @type {any} */ params) => registry.call(s.context(), name, params);
  const file = (/** @type {string} */ f) => join(site.sourceDir, f);

  test('undo and redo, with doc.changed statuses, and nothing written until save', async () => {
    /** @type {any[]} */
    const changes = [];
    const s = make({ onChange: (/** @type {any} */ st) => changes.push(st) });
    const disk = await readFile(file('scenes/zz-test.mjs'), 'utf8');
    const placed = await call(s, 'scene.place', { name: 'zz-test', object: 'pine', at: [0, 1, 0] });
    assert.equal(placed.path, '3');
    await call(s, 'scene.move', { name: 'zz-test', item: '3', by: [1, 0] });
    assert.equal(await readFile(file('scenes/zz-test.mjs'), 'utf8'), disk, 'nothing written');
    // Session edits show in every command: items and render read the working copy.
    const items = await call(s, 'scene.items', { name: 'zz-test' });
    assert.deepEqual(items.items[3].at, { tile: [1, 1, 0] });
    let st = await call(s, 'doc.status', { doc: 'scene:zz-test' });
    assert.deepEqual([st.dirty, st.canUndo, st.canRedo, st.undo.length], [true, true, false, 2]);
    const undo = await call(s, 'doc.undo', { doc: 'zz-test' });
    assert.match(undo.summary, /^Undid: pine moved to column 1/);
    await call(s, 'doc.undo', { doc: 'zz-test' });
    st = await call(s, 'doc.status', { doc: 'scene:zz-test' });
    assert.deepEqual([st.dirty, st.canUndo, st.canRedo], [false, false, true]);
    assert.equal((await call(s, 'doc.undo', { doc: 'zz-test' })).summary, 'Nothing to undo');
    await call(s, 'doc.redo', { doc: 'zz-test' });
    await call(s, 'doc.redo', { doc: 'zz-test' });
    assert.deepEqual((await call(s, 'scene.items', { name: 'zz-test' })).items[3].at, { tile: [1, 1, 0] });
    assert.deepEqual(changes.at(-1), { doc: 'scene:zz-test', dirty: true, canUndo: true, canRedo: false });
    assert.equal(changes.length, 6);
    const saved = await call(s, 'doc.save', { doc: 'scene:zz-test' });
    assert.equal(saved.wrote, true);
    assert.equal(saved.dirty, false);
    assert.deepEqual((await readDoc(file('scenes/zz-test.mjs'))).items[3], { object: 'pine', at: { tile: [1, 1, 0] } });
    assert.equal((await readSource(opened, 'scene', 'zz-test')).hash, saved.hash);
  });

  test('rapid repeats coalesce into one undo step; other edits do not', async () => {
    let clock = 1000;
    const s = make({ now: () => clock });
    for (let i = 0; i < 3; i++) {
      await call(s, 'scene.move', { name: 'zz-test', item: '0', by: [1, 0] });
      clock += 100;
    }
    clock += 5000;
    await call(s, 'scene.move', { name: 'zz-test', item: '0', by: [1, 0] });
    const st = await call(s, 'doc.status', { doc: 'zz-test' });
    assert.equal(st.undo.length, 4, 'every edit is listed');
    assert.match((await call(s, 'doc.undo', { doc: 'zz-test' })).summary, /^Undid: pine moved/);
    const second = await call(s, 'doc.undo', { doc: 'zz-test' });
    assert.match(second.summary, /\(3 edits\)$/);
    assert.equal(second.canUndo, false);
    assert.equal(second.dirty, false);
  });

  test('conflict: a file changed on disk since it was opened is not overwritten unless forced', async () => {
    const s = make();
    await call(s, 'object.paint', { name: 'zz-sprite', pixels: [[3, 0]], color: 'ink' });
    const outside = (await readFile(file('objects/zz-sprite.mjs'), 'utf8')).replace("'o...',", "'o..o',");
    await writeFile(file('objects/zz-sprite.mjs'), outside);
    await assert.rejects(call(s, 'doc.save', { doc: 'object:zz-sprite' }), (/** @type {any} */ err) => err.code === 'conflict');
    assert.equal(await readFile(file('objects/zz-sprite.mjs'), 'utf8'), outside);
    const st = await call(s, 'doc.status', { doc: 'zz-sprite' });
    assert.equal(st.stale, true);
    const forced = await call(s, 'doc.save', { doc: 'object:zz-sprite', force: true });
    assert.equal(forced.wrote, true);
    assert.deepEqual((await readDoc(file('objects/zz-sprite.mjs'))).layers[0].map.slice(0, 2), ['oo.o', 'o...']);
  });

  test('revert loads the file again as an undo step; a new doc is discarded', async () => {
    const s = make();
    const disk = await readFile(file('scenes/zz-test.mjs'), 'utf8');
    await call(s, 'scene.remove', { name: 'zz-test', item: '0' });
    const r = await call(s, 'doc.revert', { doc: 'zz-test' });
    assert.equal(r.dirty, false);
    assert.equal(r.canUndo, true);
    assert.equal(s.serialize(s.entry('scene:zz-test').doc), disk);
    await call(s, 'scene.new', { name: 'zz-draft' });
    assert.equal((await call(s, 'doc.status', { doc: 'scene:zz-draft' })).dirty, true);
    assert.equal((await call(s, 'doc.revert', { doc: 'scene:zz-draft' })).closed, true);
    await assert.rejects(call(s, 'doc.status', { doc: 'scene:zz-draft' }), (/** @type {any} */ err) => err.code === 'not_found');
    await call(s, 'scene.remove', { name: 'zz-test', item: '0' });
    await assert.rejects(call(s, 'doc.close', { doc: 'zz-test' }), (/** @type {any} */ err) => err.code === 'dirty');
    assert.equal((await call(s, 'doc.close', { doc: 'zz-test', force: true })).closed, true);
  });

  test('a failed edit leaves the working copy as it was', async () => {
    const s = make();
    await call(s, 'doc.open', { doc: 'scene:zz-test' });
    const before = JSON.stringify(s.entry('scene:zz-test').doc);
    await assert.rejects(call(s, 'scene.set', { name: 'zz-test', item: '0', tile: [5, 5, 0], id: 'Bad Id' }), /kebab-case/);
    assert.equal(JSON.stringify(s.entry('scene:zz-test').doc), before);
    assert.equal((await call(s, 'doc.status', { doc: 'zz-test' })).canUndo, false);
  });

  test('propose mode: doc.save writes a proposal, not the site; apply and reject', async () => {
    const s = make({ mode: 'propose', agent: 'test-agent 1.0' });
    const disk = await readFile(file('scenes/zz-test.mjs'), 'utf8');
    await call(s, 'scene.place', { name: 'zz-test', object: 'pine', at: [3, 0, 0] });
    await call(s, 'scene.move', { name: 'zz-test', item: '0', by: [0, 1] });
    const saved = await call(s, 'doc.save', { doc: 'zz-test' });
    assert.equal(saved.wrote, false);
    assert.equal(await readFile(file('scenes/zz-test.mjs'), 'utf8'), disk, 'the site is untouched');
    const proposal = JSON.parse(await readFile(saved.file, 'utf8'));
    assert.deepEqual(Object.keys(proposal), ['id', 'doc', 'baseHash', 'source', 'summary', 'createdAt', 'agent']);
    assert.equal(proposal.doc, 'scene:zz-test');
    assert.equal(proposal.agent, 'test-agent 1.0');
    assert.equal(proposal.baseHash, (await readSource(opened, 'scene', 'zz-test')).hash);
    assert.equal(proposal.summary.length, 2);
    // A second save rewrites the same proposal.
    await call(s, 'scene.move', { name: 'zz-test', item: '0', by: [0, 1] });
    assert.equal((await call(s, 'doc.save', { doc: 'zz-test' })).proposal, proposal.id);

    const person = make();
    const list = await call(person, 'proposal.list', {});
    assert.deepEqual(list.proposals.map((/** @type {any} */ p) => [p.id, p.baseMatches, p.summary.length]), [[proposal.id, true, 3]]);
    const shown = await call(person, 'proposal.show', { id: proposal.id });
    assert.match(shown.diff, /^\+    \{ object: 'pine', at: \{ tile: \[3, 0, 0\] \} \},$/m);
    // A person's open working copy reloads when it's unchanged.
    await call(person, 'doc.open', { doc: 'zz-test' });
    const latest = JSON.parse(await readFile(saved.file, 'utf8'));
    const applied = await call(person, 'proposal.apply', { id: proposal.id });
    assert.equal(applied.reloaded, true);
    assert.equal(await readFile(file('scenes/zz-test.mjs'), 'utf8'), latest.source);
    await assert.rejects(stat(saved.file), 'the proposal is removed');
    assert.equal(person.serialize(person.entry('scene:zz-test').doc), (await readSource(opened, 'scene', 'zz-test')).text);
    assert.deepEqual((await call(person, 'proposal.list', {})).proposals, []);

    // A proposal on a base that changed is refused unless forced; reject deletes.
    const agent = make({ mode: 'propose' });
    await call(agent, 'scene.remove', { name: 'zz-test', item: '0' });
    const second = await call(agent, 'doc.save', { doc: 'zz-test' });
    await writeFile(file('scenes/zz-test.mjs'), `${await readFile(file('scenes/zz-test.mjs'), 'utf8')}\n`);
    assert.equal((await call(person, 'proposal.list', {})).proposals[0].baseMatches, false);
    await assert.rejects(call(person, 'proposal.apply', { id: second.proposal }), (/** @type {any} */ err) => err.code === 'conflict');
    const rejected = await call(person, 'proposal.reject', { id: second.proposal });
    assert.equal(rejected.rejected, true);
    await assert.rejects(stat(second.file));
    await assert.rejects(call(person, 'proposal.show', { id: second.proposal }), (/** @type {any} */ err) => err.code === 'not_found');
    await assert.rejects(call(person, 'proposal.show', { id: '../../etc/passwd' }), (/** @type {any} */ err) => err.code === 'not_found');
  });

  test('propose mode validates too: a proposal with problems is refused unless forced', async () => {
    const s = make({ mode: 'propose' });
    await call(s, 'scene.set', { name: 'zz-test', item: '0', class: 'ok' });
    // Break the working copy the way a hand edit could.
    s.entry('scene:zz-test').doc.items[0].object = 'no-such-object';
    await assert.rejects(call(s, 'doc.save', { doc: 'zz-test' }), (/** @type {any} */ err) => err.code === 'invalid');
  });

  test('proposal.render: the proposed doc over the files on disk, rendered without writing', async () => {
    const agent = make({ mode: 'propose', agent: 'render-agent' });
    const person = make();
    const sceneText = await readFile(file('scenes/zz-test.mjs'), 'utf8');
    const spriteText = await readFile(file('objects/zz-sprite.mjs'), 'utf8');
    const now = await call(person, 'scene.render', { name: 'zz-test' });
    await call(agent, 'scene.place', { name: 'zz-test', object: 'pine', at: [-2, 0, 0] });
    const scene = await call(agent, 'doc.save', { doc: 'zz-test' });
    const r = await call(person, 'proposal.render', { id: scene.proposal, scale: 2 });
    assert.equal(r.kind, 'scene');
    assert.equal(r.doc, 'scene:zz-test');
    assert.equal(r.baseMatches, true);
    assert.equal(r.scale, 2);
    assert.equal(r.width, now.width * 2);
    assert.equal(r.items.length, now.items.length + 1);
    assert.ok(r.items.some((/** @type {any} */ i) => i.object === 'pine' && JSON.stringify(i.at) === '{"tile":[-2,0,0]}' && i.bounds), 'the placed pine renders with bounds');
    assert.notEqual(r.png, (await call(person, 'scene.render', { name: 'zz-test', scale: 2 })).png);

    await call(agent, 'object.paint', { name: 'zz-sprite', pixels: [[3, 2]], color: 'ink' });
    const object = await call(agent, 'doc.save', { doc: 'zz-sprite' });
    const o = await call(person, 'proposal.render', { id: object.proposal, layers: [0] });
    assert.equal(o.kind, 'object');
    assert.deepEqual(o.viewBox, (await call(person, 'object.render', { name: 'zz-sprite', layers: [0] })).viewBox);
    assert.notEqual(o.png, (await call(person, 'object.render', { name: 'zz-sprite', layers: [0] })).png);

    assert.equal(await readFile(file('scenes/zz-test.mjs'), 'utf8'), sceneText, 'nothing written');
    assert.equal(await readFile(file('objects/zz-sprite.mjs'), 'utf8'), spriteText, 'nothing written');
    await assert.rejects(call(person, 'proposal.render', { id: '20990101-000000-abcdef' }), (/** @type {any} */ err) => err.code === 'not_found');
    await call(person, 'proposal.reject', { id: scene.proposal });
    await call(person, 'proposal.reject', { id: object.proposal });
  });
});

describe('round trip', needsSite, () => {
  test('every real scene and object serializes to its file byte for byte, and saves unchanged', async () => {
    const site = await makeSite();
    try {
      const opened = await openSite({ site: site.root });
      const s = new Session(opened, { env: { ...process.env, PIXELLAB_STATE: join(site.base, 'state') } });
      const ctx = s.context();
      /** @type {string[]} */
      const differ = [];
      let n = 0;
      for (const kind of /** @type {const} */ (['scene', 'object'])) {
        for (const name of await ctx.names(kind)) {
          const entry = await s.open(ctx, kind, name);
          n++;
          if (entry.clean !== entry.base.text) differ.push(entry.file);
          const before = await stat(join(opened.sourceDir, entry.file));
          const r = await s.save(ctx, entry.ref);
          assert.equal(r.wrote, false, `${entry.file} was rewritten`);
          assert.equal((await stat(join(opened.sourceDir, entry.file))).mtimeMs, before.mtimeMs);
        }
      }
      assert.ok(n > 150, `${n} docs`);
      assert.deepEqual(differ, [], 'the engine serializer is the authority: these files differ from serialize() of themselves');
    } finally {
      await site.cleanup();
    }
  });
});


describe('art.build (spec R7)', needsSite, () => {
  // The engine draws items in depth order now (docs/intents/2026-10-depth-order/spec.md, R3), so the site's
  // hero island and range sprite differ from the SVGs committed under the old order (the same elements, drawn
  // in another order), and everything stays within its budget.
  const tags = (/** @type {Buffer} */ svg) => svg.toString('utf8').split(/(?=<)/).sort();
  /** Whether a published file is the committed one, or the same elements drawn in another order. */
  const sameButReordered = (/** @type {Buffer} */ built, /** @type {Buffer} */ committed) => {
    const [a, b] = [tags(built), tags(committed)];
    return a.length === b.length && a.every((t, i) => t === b[i]);
  };

  test("R7: publishes the site's art into the legacy publish folder, the committed SVGs or the same elements in depth order", async () => {
    const site = await makeSite();
    try {
      const r = await runCli(['art', 'build', '--site', site.root, '--json'], { env: site.env() });
      assert.equal(r.code, 0, r.stderr);
      const out = JSON.parse(r.stdout);
      assert.equal(out.ok, true);
      assert.equal(out.publishDir, join(site.root, 'src/assets/pixel-art'));
      assert.deepEqual(out.problems, []);
      assert.deepEqual(out.unsaved, []);
      const files = out.outputs.map((/** @type {any} */ o) => o.file).sort();
      assert.ok(files.includes('hero-island.svg'), files.join(', '));
      assert.ok(out.outputs.every((/** @type {any} */ o) => /lossless$/.test(o.report) && o.over === false), out.outputs.map((/** @type {any} */ o) => o.report).join('\n'));
      assert.ok(out.previewOnly.length > 0, 'preview-only scenes are compiled and reported, not written');
      for (const file of files) {
        const built = await readFile(join(out.publishDir, file));
        const committed = await readFile(join(SITE_SOURCE, 'src/assets/pixel-art', file));
        assert.ok(built.equals(committed) || sameButReordered(built, committed), `${file} differs from the committed SVG by more than the order of its elements`);
      }
    } finally {
      await site.cleanup();
    }
  });

  test('R7: --dir publishes elsewhere, and a document without a publish folder refuses without it', async () => {
    const site = await makeSite();
    const out = await tempDir();
    try {
      const r = await runCli(['art', 'build', '--site', site.root, '--dir', out, '--json'], { env: site.env() });
      assert.equal(r.code, 0, r.stderr);
      assert.equal(JSON.parse(r.stdout).publishDir, out);
      assert.ok((await stat(join(out, 'footer-dirt.svg'))).size > 0);
      // The same sources as a document whose manifest names no publish folder.
      const doc = join(out, 'bare.pixellab');
      await cp(site.sourceDir, doc, { recursive: true });
      await writeFile(join(doc, 'manifest.json'), '{ "format": 1 }\n');
      const refused = await runCli(['art', 'build', '--document', doc, '--json'], { env: site.env() });
      assert.equal(refused.code, 1);
      assert.equal(JSON.parse(refused.stderr).error.code, 'no_publish_dir');
      // Without the manifest's budgets, the engine's 100 KB / 25 KB apply: the island is over, and still written.
      const over = await runCli(['art', 'build', '--document', doc, '--dir', join(out, 'bare-out'), '--json'], { env: site.env() });
      assert.equal(over.code, 1);
      const res = JSON.parse(over.stdout);
      assert.equal(res.ok, false);
      assert.equal(res.outputs.find((/** @type {any} */ o) => o.file === 'hero-island.svg').over, true);
      assert.ok((await stat(join(out, 'bare-out', 'hero-island.svg'))).size > 0);
      const budgets = { 'hero-island.svg': { raw: 768000, gzip: 153600 }, 'range-sprite.svg': { raw: 204800, gzip: 51200 } };
      await writeFile(join(doc, 'manifest.json'), `${JSON.stringify({ format: 1, publish: { budgets } })}\n`);
      const given = await runCli(['art', 'build', '--document', doc, '--dir', join(out, 'bare-out')], { env: site.env() });
      assert.equal(given.code, 0, given.stderr);
      assert.match(given.stdout, /published 6 files to .*bare-out/);
    } finally {
      await rm(out, { recursive: true, force: true });
      await site.cleanup();
    }
  });

  test('R7: nothing is written when a source has a problem', async () => {
    const site = await makeSite({ 'src/assets/pixel-art/source/objects/zz-bad.mjs': "export default { kind: 'sprite', keys: { a: 'no-such-color' }, layers: [{ map: ['a'] }] };\n" });
    const out = await tempDir();
    try {
      const r = await runCli(['art', 'build', '--site', site.root, '--dir', out, '--json'], { env: site.env() });
      assert.equal(r.code, 1);
      const res = JSON.parse(r.stdout);
      assert.equal(res.ok, false);
      assert.deepEqual(res.outputs, []);
      assert.ok(res.problems.some((/** @type {string} */ p) => p.startsWith('objects/zz-bad.mjs: ')), res.problems.join('\n'));
      await assert.rejects(stat(join(out, 'footer-dirt.svg')));
    } finally {
      await rm(out, { recursive: true, force: true });
      await site.cleanup();
    }
  });
});
