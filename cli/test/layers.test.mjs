// Scene layers (docs/intents/2026-10-scene-layers/spec.md, R1–R5): named,
// nestable parts of a scene, stored in the scene file as a `layers` list and
// a `layer` on each item. They never change paint order or what's published,
// they're apart from the { group } wrappers, and every action is a command.
// On temp copies of the sample document.

import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { after, before, describe, test } from 'node:test';

import { openDocument } from '../lib/document.mjs';
import { registry } from '../lib/registry.mjs';
import { Session } from '../lib/session.mjs';
import { decodePng, makeDocument, readDoc, runCli, startServer, tempDir } from './helpers.mjs';

// Two ground cubes in entrance wrappers whose data-* point at them, two
// pebbles on top, an animated pebble and one placed by pixel.
const YARD = `// Pixel-art scene. How to edit it: .claude/skills/pixel-art/SKILL.md
export default {
  output: 'yard.svg',
  viewBox: [-48, -32, 128, 96],
  items: [
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '0', 'data-row': '0', 'data-x': '0', 'data-y': '0' },
      items: [
        { object: 'block', at: { tile: [0, 0, 0] } },
      ],
    },
    {
      group: { class: 'enter', 'data-enter': 'ground', 'data-col': '1', 'data-row': '0', 'data-x': '16', 'data-y': '8' },
      items: [
        { object: 'block', at: { tile: [1, 0, 0] } },
      ],
    },
    { object: 'pebble', at: { tile: [0, 0, 1] } },
    { object: 'pebble', id: 'rock', at: { tile: [1, 0, 1] } },
    { object: 'pebble', at: { px: [8, 20] } },
  ],
  motion: {
    duration: 1000,
    tracks: [
      {
        item: 'rock',
        keys: [
          { t: 0 },
          { t: 500, move: [2, 0] },
        ],
      },
    ],
  },
};
`;

/** YARD with layers: Ground (the cubes), Stones (the pebbles) and Rock inside Stones. */
const LAYERED = YARD.replace(
  "  items: [\n",
  "  layers: [\n    { id: 'ground', name: 'Ground' },\n    { id: 'stones', name: 'Stones' },\n    { id: 'rock', name: 'Rock', parent: 'stones' },\n  ],\n  items: [\n",
)
  .replace("{ object: 'block', at: { tile: [0, 0, 0] } }", "{ object: 'block', layer: 'ground', at: { tile: [0, 0, 0] } }")
  .replace("{ object: 'block', at: { tile: [1, 0, 0] } }", "{ object: 'block', layer: 'ground', at: { tile: [1, 0, 0] } }")
  .replace("{ object: 'pebble', at: { tile: [0, 0, 1] } }", "{ object: 'pebble', layer: 'stones', at: { tile: [0, 0, 1] } }")
  .replace("{ object: 'pebble', id: 'rock', at: { tile: [1, 0, 1] } }", "{ object: 'pebble', id: 'rock', layer: 'rock', at: { tile: [1, 0, 1] } }");

/** @type {Awaited<ReturnType<typeof makeDocument>>} */
let doc;
/** @type {any} */
let opened;
/** @type {string} */
let state;
before(async () => {
  doc = await makeDocument({ files: { 'scenes/yard.mjs': YARD, 'scenes/layered.mjs': LAYERED } });
  opened = await openDocument({ doc: doc.root, env: doc.env });
  state = await tempDir('pixellab-state-');
});
after(() => doc.cleanup());

const session = () => new Session(opened, { env: { ...process.env, PIXELLAB_STATE: state } });
const call = (/** @type {Session} */ s, /** @type {string} */ name, /** @type {any} */ params, scene = 'yard') =>
  registry.call(s.context(), name, { name: scene, ...params });
/** The scene's working copy in a session. */
const working = async (/** @type {Session} */ s, scene = 'yard') => s.context().doc('scene', scene);
/** The objects and placements in paint order, and the wrappers' attributes: what layers must never change. */
const paintOrder = (/** @type {any} */ scene) => JSON.stringify(scene.items, (key, value) => (key === 'layer' ? undefined : value));
/** A scene's source with every trace of layers taken out. */
const stripLayers = (/** @type {string} */ text) => text.replace(/ {2}layers: \[\n(?: {4}.*\n)*? {2}\],\n/, '').replace(/, layer: '[a-z0-9-]+'/g, '');
/** The engine's problems for a scene given as data, next to the sample document's palette and objects. */
const problems = async (/** @type {any} */ data) => {
  const { sources } = await session().context().sources();
  const all = opened.engine.loadSources({ palette: sources.palette, objects: Object.fromEntries(sources.objects), scenes: { s: data } });
  return opened.engine.validate(all).filter((/** @type {string} */ p) => p.startsWith('scenes/s.mjs: ')).map((/** @type {string} */ p) => p.slice('scenes/s.mjs: '.length));
};
const scene = (/** @type {any} */ extra) => ({ viewBox: [0, 0, 16, 16], items: [{ object: 'pebble', at: { px: [8, 8] } }], ...extra });

describe('the scene format (R1)', () => {
  test('R1: a scene with nested layers and items in them is valid', async () => {
    assert.deepEqual(
      await problems(scene({ layers: [{ id: 'yard', name: 'Yard' }, { id: 'crates', name: 'Crates and boxes', parent: 'yard' }], items: [{ object: 'pebble', layer: 'crates', at: { px: [8, 8] } }] })),
      [],
    );
  });

  test('R1: the engine refuses malformed layers and items in missing layers', async () => {
    assert.deepEqual(await problems(scene({ layers: 'yard' })), ['layers must be a list of { id, name, parent }']);
    assert.deepEqual(await problems(scene({ layers: [{ id: 'yard', name: 'Yard', color: 'red' }] })), ["layers[0]: unknown property 'color'"]);
    assert.deepEqual(await problems(scene({ layers: [{ id: 'Yard', name: 'Yard' }] })), ['layers[0]: id must be lowercase kebab-case']);
    assert.deepEqual(await problems(scene({ layers: [{ id: 'yard', name: 'Yard' }, { id: 'yard', name: 'Two' }] })), ["layers[1]: id 'yard' is already used by another layer"]);
    for (const name of ['', ' Yard', 'Yard ', 'Two\nlines', 'x'.repeat(65), 7]) {
      assert.deepEqual(await problems(scene({ layers: [{ id: 'yard', name }] })), ['layers[0]: name must be 1 to 64 characters on one line, without spaces at either end'], JSON.stringify(name));
    }
    assert.deepEqual(await problems(scene({ layers: [{ id: 'yard', name: 'x'.repeat(64) }] })), []);
    assert.deepEqual(await problems(scene({ layers: [{ id: 'yard', name: 'Yard', parent: 'barn' }] })), ["layers[0]: parent 'barn' is no layer's id"]);
    assert.deepEqual(await problems(scene({ layers: [{ id: 'a', name: 'A', parent: 'b' }, { id: 'b', name: 'B', parent: 'a' }] })), ["layers: layer 'a' is inside itself (a layer's parents can't come back to it)"]);
    assert.deepEqual(await problems(scene({ layers: [{ id: 'a', name: 'A', parent: 'a' }] })), ["layers: layer 'a' is inside itself (a layer's parents can't come back to it)"]);
    assert.deepEqual(await problems(scene({ items: [{ object: 'pebble', layer: 'yard', at: { px: [8, 8] } }] })), ["items[0]: layer 'yard' is no layer of this scene"]);
    assert.deepEqual(await problems(scene({ layers: [], items: [{ object: 'pebble', layer: 3, at: { px: [8, 8] } }] })), ["items[0]: layer '3' is no layer of this scene"]);
  });

  test('R1: layers serialize before items, one per line, and an item\'s layer before at; the source round-trips', async () => {
    const data = await readDoc(join(doc.root, 'scenes/layered.mjs'));
    assert.equal(opened.serialize.serialize(data), LAYERED);
    assert.match(LAYERED, /\n {2}layers: \[\n {4}\{ id: 'ground', name: 'Ground' \},\n {4}\{ id: 'stones', name: 'Stones' \},\n {4}\{ id: 'rock', name: 'Rock', parent: 'stones' \},\n {2}\],\n {2}items: \[/);
    assert.match(LAYERED, /\{ object: 'pebble', id: 'rock', layer: 'rock', at: \{ tile: \[1, 0, 1\] \} \}/);
  });

  test('R1: scene.items lists the layers, what each holds (layers inside included) and each item\'s layer', async () => {
    const r = await call(session(), 'scene.items', {}, 'layered');
    assert.deepEqual(r.layers, [
      { id: 'ground', name: 'Ground', items: 2 },
      { id: 'stones', name: 'Stones', items: 2 },
      { id: 'rock', name: 'Rock', parent: 'stones', items: 1 },
    ]);
    assert.deepEqual(r.items.map((/** @type {any} */ i) => [i.path, i.layer ?? null]), [['0.0', 'ground'], ['1.0', 'ground'], ['2', 'stones'], ['3', 'rock'], ['4', null]]);
    const shown = await call(session(), 'scene.show', {}, 'layered');
    assert.deepEqual([shown.layers, shown.layerNames], [3, ['Ground', 'Stones', 'Rock']]);
  });
});

describe('paint order and output (R2)', () => {
  test('R2: art build of a document with layers is byte-identical to the same document without them', async () => {
    const plain = await makeDocument({ files: { 'scenes/layered.mjs': stripLayers(LAYERED) } });
    const layered = await makeDocument({ files: { 'scenes/layered.mjs': LAYERED } });
    try {
      assert.equal(stripLayers(LAYERED), YARD, 'the stripped scene is the scene without layers');
      const outs = [];
      for (const d of [plain, layered]) {
        const out = join(d.base, 'out');
        const r = await runCli(['art', 'build', '--dir', out, '--document', d.root], { env: d.env });
        assert.equal(r.code, 0, r.stderr);
        const files = (await readdir(out)).sort();
        outs.push(Object.fromEntries(await Promise.all(files.map(async (f) => [f, await readFile(join(out, f))]))));
      }
      assert.deepEqual(Object.keys(outs[1]), ['sample.svg', 'yard.svg']);
      for (const file of Object.keys(outs[0])) assert.ok(outs[0][file].equals(outs[1][file]), `${file} differs`);
    } finally {
      await plain.cleanup();
      await layered.cleanup();
    }
  });

  test('R2: scene.render draws the same PNG with and without layers', async () => {
    const s = session();
    const [a, b] = [await call(s, 'scene.render', { scale: 2 }, 'yard'), await call(s, 'scene.render', { scale: 2 }, 'layered')];
    assert.equal(a.png, b.png);
    assert.deepEqual(a.items, b.items);
  });

  test('R2: making, renaming, assigning and ungrouping layers never reorder or move items', async () => {
    const s = session();
    const before = paintOrder(await working(s));
    await call(s, 'scene.layer.new', { layerName: 'Ground', items: ['0.0', '1.0'] });
    await call(s, 'scene.layer.new', { layerName: 'Stones', items: ['2', 'rock', '4'] });
    await call(s, 'scene.layer.new', { items: ['rock'] });
    await call(s, 'scene.layer.rename', { layer: 'Group 1', to: 'Rock' });
    await call(s, 'scene.layer.assign', { to: 'ground', items: ['4'] });
    await call(s, 'scene.layer.assign', { to: '', layers: ['group-1'] });
    assert.equal(paintOrder(await working(s)), before);
    for (const id of ['group-1', 'stones', 'ground']) await call(s, 'scene.layer.ungroup', { layer: id });
    assert.equal(paintOrder(await working(s)), before);
  });

  test('R2: ungrouping the last layer takes the layers list away, so the file is what it was', async () => {
    const s = session();
    await call(s, 'scene.layer.new', { layerName: 'Ground', items: ['0.0', '1.0'] });
    await call(s, 'scene.layer.ungroup', { layer: 'ground' });
    const scene = await working(s);
    assert.equal('layers' in scene, false);
    assert.equal(opened.serialize.serialize(scene), YARD);
  });
});

describe('wrappers (R3)', () => {
  test('R3: layer commands leave the { group } wrappers alone, and scene group set works on a layered scene', async () => {
    const s = session();
    const groups = async () => (await call(s, 'scene.items', {}, 'layered')).groups;
    const before = await groups();
    await call(s, 'scene.layer.new', { layers: ['ground', 'stones'] }, 'layered');
    await call(s, 'scene.layer.ungroup', { layer: 'stones' }, 'layered');
    assert.deepEqual(await groups(), before);
    const r = await call(s, 'scene.group.set', { group: '0.0', attrs: { 'data-enter': 'object' } }, 'layered');
    assert.equal(r.group.attrs['data-enter'], 'object');
    assert.equal((await working(s, 'layered')).items[0].items[0].layer, 'ground');
  });

  test('R3: moving a layer keeps each item\'s wrapper data-x/data-y and data-col/data-row in step; --no-keep-group leaves them', async () => {
    const s = session();
    const r = await call(s, 'scene.layer.move', { layer: 'ground', by: [1, 0] }, 'layered');
    assert.equal(r.moved, 2);
    const items = (await working(s, 'layered')).items;
    assert.deepEqual(items[0].group, { class: 'enter', 'data-enter': 'ground', 'data-col': '1', 'data-row': '0', 'data-x': '16', 'data-y': '8' });
    assert.deepEqual(items[1].group, { class: 'enter', 'data-enter': 'ground', 'data-col': '2', 'data-row': '0', 'data-x': '32', 'data-y': '16' });
    await call(s, 'scene.layer.move', { layer: 'ground', by: [0, 1], keepGroup: false }, 'layered');
    assert.equal((await working(s, 'layered')).items[0].group['data-row'], '0');
    assert.deepEqual((await working(s, 'layered')).items[0].items[0].at, { tile: [1, 1, 0] });
  });
});

describe('existing documents (R4)', () => {
  test('R4: a scene without layers lists none, and every item is at the top level', async () => {
    const r = await call(session(), 'scene.items', {}, 'sample');
    assert.deepEqual(r.layers, []);
    assert.ok(r.items.every((/** @type {any} */ i) => !('layer' in i)));
  });

  test('R4: editing a scene without layers writes nothing about layers', async () => {
    const s = session();
    await call(s, 'scene.place', { object: 'pebble', at: [2, 2, 1] }, 'sample');
    await call(s, 'scene.move', { item: '0', by: [1, 0] }, 'sample');
    await call(s, 'scene.set', { item: '1', class: 'sway' }, 'sample');
    assert.doesNotMatch(opened.serialize.serialize(await working(s, 'sample')), /layer/);
  });
});

describe('commands (R5)', () => {
  test('R5: scene.layer.new names layers Group 1, Group 2, … and makes ids from names', async () => {
    const s = session();
    const a = await call(s, 'scene.layer.new', { items: ['0.0'] });
    const b = await call(s, 'scene.layer.new', { items: ['1.0'] });
    assert.deepEqual([a.layer, b.layer], [{ id: 'group-1', name: 'Group 1' }, { id: 'group-2', name: 'Group 2' }]);
    assert.equal(a.summary, 'Grouped 1 item as Group 1');
    await call(s, 'scene.layer.rename', { layer: 'group-1', to: 'Ground' });
    assert.equal((await call(s, 'scene.layer.new', {})).layer.name, 'Group 3', 'one more than the highest Group n');
    assert.equal((await call(s, 'scene.layer.new', { layerName: 'Ground' })).layer.id, 'ground');
    assert.equal((await call(s, 'scene.layer.new', { layerName: 'Ground' })).layer.id, 'ground-2');
    assert.equal((await call(s, 'scene.layer.new', { layerName: '3 Trees' })).layer.id, 'layer-3-trees');
    await assert.rejects(call(s, 'scene.layer.new', { id: 'ground' }), { code: 'exists' });
    await assert.rejects(call(s, 'scene.layer.new', { layerName: ' ' }), { code: 'usage' });
  });

  test('R5: a new layer goes under the members\' common parent, a given parent, or the top level', async () => {
    const s = session();
    const inStones = await call(s, 'scene.layer.new', { items: ['2'], layers: ['rock'] }, 'layered');
    assert.equal(inStones.layer.parent, 'stones', 'an item in Stones and a layer in Stones');
    const top = await call(s, 'scene.layer.new', { items: ['0.0', '4'] }, 'layered');
    assert.equal(top.layer.parent, undefined, 'Ground and the top level share only the top level');
    const given = await call(s, 'scene.layer.new', { layerName: 'Inner', parent: 'ground' }, 'layered');
    assert.equal(given.layer.parent, 'ground');
    await assert.rejects(call(s, 'scene.layer.new', { layers: ['ground'], parent: 'inner' }, 'layered'), { code: 'layer_cycle' });
    const layers = (await working(s, 'layered')).layers;
    assert.deepEqual(
      layers.map((/** @type {any} */ l) => l.id),
      ['group-2', 'ground', 'stones', 'group-1', 'rock', 'inner'],
      'each goes before its first member layer, else before the first layer beside it, else last',
    );
  });

  test('R5: a member inside another member stays where it is', async () => {
    const s = session();
    const r = await call(s, 'scene.layer.new', { layers: ['stones', 'rock'], items: ['rock', '2'] }, 'layered');
    assert.deepEqual([r.layers, r.items], [['stones'], 0]);
    const scene = await working(s, 'layered');
    assert.equal(scene.layers.find((/** @type {any} */ l) => l.id === 'rock').parent, 'stones');
    assert.equal(scene.items[3].layer, 'rock');
  });

  test('R5: scene.layer.ungroup moves what a layer held up one level', async () => {
    const s = session();
    const r = await call(s, 'scene.layer.ungroup', { layer: 'stones' }, 'layered');
    assert.deepEqual([r.parent, r.layers, r.items], [null, ['rock'], ['2']]);
    const scene = await working(s, 'layered');
    assert.deepEqual(scene.layers.map((/** @type {any} */ l) => [l.id, l.parent ?? null]), [['ground', null], ['rock', null]]);
    assert.equal('layer' in scene.items[2], false);
    await call(s, 'scene.layer.new', { layerName: 'Outer', layers: ['rock'] }, 'layered');
    await call(s, 'scene.layer.new', { layerName: 'Middle', layers: ['rock'] }, 'layered');
    const inner = await call(s, 'scene.layer.ungroup', { layer: 'middle' }, 'layered');
    assert.equal(inner.parent, 'outer');
    assert.equal((await working(s, 'layered')).layers.find((/** @type {any} */ l) => l.id === 'rock').parent, 'outer');
  });

  test('R5: scene.layer.assign moves items and layers into a layer or the top level, and refuses cycles', async () => {
    const s = session();
    const r = await call(s, 'scene.layer.assign', { to: 'Rock', items: ['4', '0.0'] }, 'layered');
    assert.equal(r.summary, 'Moved 2 items into Rock');
    await call(s, 'scene.layer.assign', { to: '', layers: ['rock'] }, 'layered');
    const scene = await working(s, 'layered');
    assert.deepEqual([scene.items[4].layer, scene.items[0].items[0].layer], ['rock', 'rock']);
    assert.equal(scene.layers.find((/** @type {any} */ l) => l.id === 'rock').parent, undefined);
    await call(s, 'scene.layer.assign', { to: 'stones', layers: ['rock'] }, 'layered');
    await assert.rejects(call(s, 'scene.layer.assign', { to: 'rock', layers: ['stones'] }, 'layered'), { code: 'layer_cycle' });
    await assert.rejects(call(s, 'scene.layer.assign', { to: 'stones', layers: ['stones'] }, 'layered'), { code: 'layer_cycle' });
    await assert.rejects(call(s, 'scene.layer.assign', { to: 'barn', items: ['4'] }, 'layered'), { code: 'not_found' });
  });

  test('R5: scene.layer.move moves every item in the layers (and inside them) and the items once, in one undo step', async () => {
    const s = session();
    const r = await call(s, 'scene.layer.move', { layer: 'stones', items: ['2', '4'], by: [1, -1] }, 'layered');
    assert.deepEqual([r.moved, r.by, r.summary], [3, [1, -1, 0], 'Moved 3 items +1 col, -1 row']);
    const scene = await working(s, 'layered');
    assert.deepEqual(scene.items.slice(2).map((/** @type {any} */ i) => i.at), [{ tile: [1, -1, 1] }, { tile: [2, -1, 1] }, { px: [8 + 32, 20] }]);
    assert.deepEqual(scene.motion.tracks[0].keys[1].move, [2, 0], 'an animated item\'s keys are offsets: its motion moves with it');
    await registry.call(s.context(), 'doc.undo', { doc: 'scene:layered' });
    assert.deepEqual((await working(s, 'layered')).items[2].at, { tile: [0, 0, 1] });
    const level = await call(s, 'scene.layer.move', { layer: 'Ground', by: [0, 0, 1] }, 'layered');
    assert.equal(level.summary, 'Moved Ground 0 col, 0 row, +1 level; 2 groups followed');
  });

  test('R5: scene.place --layer puts the new item in a layer; an unknown layer places nothing', async () => {
    const s = session();
    const r = await call(s, 'scene.place', { object: 'pebble', at: [2, 0, 1], layer: 'Rock' }, 'layered');
    assert.equal(r.item.layer, 'rock');
    assert.match(r.summary, / in Rock$/);
    const count = (await working(s, 'layered')).items.length;
    await assert.rejects(call(s, 'scene.place', { object: 'pebble', at: [2, 2, 1], layer: 'barn' }, 'layered'), { code: 'not_found' });
    assert.equal((await working(s, 'layered')).items.length, count);
    const dup = await call(s, 'scene.duplicate', { item: r.path }, 'layered');
    assert.equal(dup.item.layer, 'rock', 'a duplicate stays in its layer');
    await call(s, 'scene.set', { item: r.path, class: 'sway', id: 'new-stone' }, 'layered');
    const set = (await working(s, 'layered')).items[Number(r.path)];
    assert.deepEqual(Object.keys(set), ['object', 'id', 'class', 'layer', 'at'], 'class and id edits keep the layer, in order');
  });

  test('R5: scene.render --hide leaves hidden layers out; --focus adds the layer\'s own render', async () => {
    const s = session();
    const all = await call(s, 'scene.render', {}, 'layered');
    const hidden = await call(s, 'scene.render', { hide: ['stones'] }, 'layered');
    assert.deepEqual(hidden.items.map((/** @type {any} */ i) => i.path), ['0.0', '1.0', '4'], 'Stones and Rock inside it');
    assert.notEqual(hidden.png, all.png);
    const focused = await call(s, 'scene.render', { focus: 'rock' }, 'layered');
    assert.equal(focused.png, all.png);
    const only = decodePng(Buffer.from(focused.focusPng, 'base64'));
    const rock = all.items.find((/** @type {any} */ i) => i.path === '3').bounds;
    assert.deepEqual(only.opaqueBox(), rock, 'only the rock is drawn');
    assert.equal(all.focusPng, undefined);
    await assert.rejects(call(s, 'scene.render', { focus: 'barn' }, 'layered'), { code: 'not_found' });
  });

  test('R5: the CLI runs the layer commands and writes the scene', async () => {
    const d = await makeDocument({ files: { 'scenes/yard.mjs': YARD } });
    try {
      const made = await runCli(['scene', 'layer', 'new', 'yard', '--layer-name', 'Ground', '--items', '0.0,1.0', '--document', d.root], { env: d.env });
      assert.equal(made.code, 0, made.stderr);
      assert.match(made.stdout, /Grouped 2 items as Ground/);
      const listed = await runCli(['scene', 'items', 'yard', '--document', d.root], { env: d.env });
      assert.match(listed.stdout, /^0\.0 +block +tile 0,0,0 +\^ground$/m);
      const text = await readFile(join(d.root, 'scenes/yard.mjs'), 'utf8');
      assert.match(text, /layers: \[\n {4}\{ id: 'ground', name: 'Ground' \},\n {2}\],/);
    } finally {
      await d.cleanup();
    }
  });

  test('R5: over MCP the layer commands are tools, and their edits become a proposal', async () => {
    const d = await makeDocument({ files: { 'scenes/yard.mjs': YARD } });
    const st = await tempDir('pixellab-state-');
    const server = startServer(['--mcp', '--document', d.root], { env: { ...d.env, PIXELLAB_STATE: st } });
    const tool = async (/** @type {string} */ name, /** @type {any} */ args) => (await server.request('tools/call', { name, arguments: args })).result;
    try {
      await server.request('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 't', version: '1' } });
      const names = (await server.request('tools/list')).result.tools.map((/** @type {any} */ t) => t.name);
      for (const n of ['scene_layer_new', 'scene_layer_rename', 'scene_layer_ungroup', 'scene_layer_assign', 'scene_layer_move']) assert.ok(names.includes(n), n);
      const before = await readFile(join(d.root, 'scenes/yard.mjs'), 'utf8');
      const made = await tool('scene_layer_new', { name: 'yard', layerName: 'Ground', items: ['0.0', '1.0'] });
      assert.equal(made.isError, undefined, JSON.stringify(made));
      await tool('scene_layer_move', { name: 'yard', layer: 'ground', by: [1, 0] });
      const saved = (await tool('doc_save', { doc: 'scene:yard' })).structuredContent;
      assert.ok(saved.proposal, 'a proposal');
      assert.deepEqual(saved.edits, ['Grouped 2 items as Ground', 'Moved Ground +1 col, 0 row; 2 groups followed']);
      assert.equal(await readFile(join(d.root, 'scenes/yard.mjs'), 'utf8'), before, 'the document is untouched until the owner approves');
    } finally {
      server.child.kill();
      await d.cleanup();
    }
  });
});
