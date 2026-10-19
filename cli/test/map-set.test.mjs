// object.map.set (docs/intents/2026-10-mcp-gaps/spec.md, R15): a whole map
// of a layer or frame in one call and one undo step, checked against the
// palette, the color tiers and the caps. On a temp copy of the sample
// document, with a palette big enough to hit the 12-color cap.

import assert from 'node:assert/strict';
import { after, before, describe, test } from 'node:test';

import { openDocument } from '../lib/document.mjs';
import { registry } from '../lib/registry.mjs';
import { Session } from '../lib/session.mjs';
import { makeDocument, tempDir } from './helpers.mjs';

const MANY = Array.from({ length: 14 }, (_, i) => `    'c-${i}': '#${(0x101010 * (i + 1)).toString(16).padStart(6, '0').slice(-6).toUpperCase()}',`).join('\n');
const PALETTE = `export default {
  world: {
    ink: '#2E2418',
    'grass-2': '#8FA56E',
    'grass-4': '#4E6B3A',
    'soil-2': '#8A5A34',
    'soil-3': '#5A3E2B',
${MANY}
  },
  outfit: {
    'teal-1': '#7FA89B'
  }
};
`;
const FLAME = `export default {
  kind: 'sprite',
  anchor: [1, 1],
  keys: { a: 'ink' },
  layers: [{ class: 'ff', frames: [['a.', '.a'], ['.a', 'a.'], ['aa', '..'], ['..', 'aa']] }],
};
`;
const PEBBLE_TOO = `export default {
  kind: 'sprite',
  extends: 'pebble',
  rows: {},
};
`;

/** @type {Awaited<ReturnType<typeof makeDocument>>} */
let doc;
/** @type {any} */
let opened;
/** @type {string} */
let state;
before(async () => {
  doc = await makeDocument({ files: { 'palette.mjs': PALETTE, 'objects/flame.mjs': FLAME, 'objects/pebble-too.mjs': PEBBLE_TOO } });
  opened = await openDocument({ doc: doc.root, env: doc.env });
  state = await tempDir('pixellab-state-');
});
after(() => doc.cleanup());

const session = () => new Session(opened, { env: { ...process.env, PIXELLAB_STATE: state } });
const call = (/** @type {Session} */ s, /** @type {string} */ name, /** @type {any} */ params) => registry.call(s.context(), name, params);
const source = (/** @type {Session} */ s, /** @type {string} */ name) => s.context().doc('object', name);

describe('object.map.set (R15)', () => {
  test("R15: with a legend, any characters map to the object's keys, new colors get keys, in one undo step", async () => {
    const s = session();
    const r = await call(s, 'object.map.set', { name: 'pebble', rows: ['xyyx', 'y..y', 'y..y', 'xyyx'], keys: { x: 'ink', y: 'soil-2' } });
    assert.deepEqual([r.changed, r.size, r.colors, r.keysAdded], [true, [4, 4], 2, ['s: soil-2']]);
    const pebble = await source(s, 'pebble');
    assert.deepEqual(pebble.layers[0].map, ['cssc', 's..s', 's..s', 'cssc']);
    assert.equal(pebble.keys.s, 'soil-2');
    assert.equal((await call(s, 'doc.status', { doc: 'object:pebble' })).undo.length, 1);
    await call(s, 'doc.undo', { doc: 'object:pebble' });
    assert.deepEqual((await source(s, 'pebble')).layers[0].map, ['.bb.', 'baab', 'caac', '.cc.']);
  });

  test("R15: without a legend, rows use the object's own keys; a frame of a loop is set alone", async () => {
    const s = session();
    await call(s, 'object.map.set', { name: 'pebble', rows: ['aaaa', 'bbbb', 'cccc', '....'] });
    assert.deepEqual((await source(s, 'pebble')).layers[0].map, ['aaaa', 'bbbb', 'cccc', '....']);
    await call(s, 'object.map.set', { name: 'flame', rows: ['..', '..'], frame: 2 });
    assert.deepEqual((await source(s, 'flame')).layers[0].frames, [['a.', '.a'], ['.a', 'a.'], ['..', '..'], ['..', 'aa']]);
    await assert.rejects(call(s, 'object.map.set', { name: 'flame', rows: ['..', '..'], frame: 7 }), { code: 'usage' });
    await assert.rejects(call(s, 'object.map.set', { name: 'pebble', rows: ['aaaa', 'bbbb', 'cccc', '....'], frame: 1 }), { code: 'usage' });
  });

  test('R15: a size change needs --resize; the size cap and ragged rows are refused', async () => {
    const s = session();
    const rows = ['aaaaaa', 'a....a', 'aaaaaa'];
    await assert.rejects(call(s, 'object.map.set', { name: 'pebble', rows }), { code: 'size_mismatch' });
    const r = await call(s, 'object.map.set', { name: 'pebble', rows, resize: true });
    assert.deepEqual(r.size, [6, 3]);
    assert.deepEqual((await source(s, 'pebble')).layers[0].map, rows);
    await assert.rejects(call(s, 'object.map.set', { name: 'pebble', rows: ['a'.repeat(65)], resize: true }), { code: 'size_cap' });
    await assert.rejects(call(s, 'object.map.set', { name: 'pebble', rows: ['aaa', 'aa'], resize: true }), /row 1 is 2 wide; every row must be 3/);
  });

  test('R15: the palette, outfit colors, unknown characters and the 12-color cap are checked', async () => {
    const s = session();
    const rows = ['abcd', 'efgh', 'ijkl', 'mnop'];
    await assert.rejects(call(s, 'object.map.set', { name: 'pebble', rows: ['zzzz', 'zzzz', 'zzzz', 'zzzz'], keys: { z: 'no-such-color' } }), /no palette color named no-such-color/);
    await assert.rejects(call(s, 'object.map.set', { name: 'pebble', rows: ['zzzz', 'zzzz', 'zzzz', 'zzzz'], keys: { z: 'teal-1' } }), { code: 'color_not_allowed' });
    await assert.rejects(call(s, 'object.map.set', { name: 'pebble', rows: ['qqqq', 'qqqq', 'qqqq', 'qqqq'] }), /"q" isn't a key/);
    const legend = Object.fromEntries([...'abcdefghijklm'].map((ch, i) => [ch, `c-${i}`]));
    await assert.rejects(call(s, 'object.map.set', { name: 'pebble', rows: ['abcdefghijklm'], keys: legend, resize: true }), { code: 'color_cap' });
    assert.deepEqual((await source(s, 'pebble')).layers[0].map, ['.bb.', 'baab', 'caac', '.cc.'], 'a refused map changes nothing');
    assert.equal((await call(s, 'object.map.set', { name: 'pebble', rows, keys: Object.fromEntries([...'abcdefghijklmnop'].map((ch, i) => [ch, `c-${i % 12}`])) })).colors, 12);
  });

  test('R15: an object that extends another gets row overrides; blocks are refused', async () => {
    const s = session();
    await call(s, 'object.map.set', { name: 'pebble-too', rows: ['.bb.', 'cccc', 'caac', '.cc.'] });
    assert.deepEqual((await source(s, 'pebble-too')).rows, { 0: { 1: 'cccc' } });
    await assert.rejects(call(s, 'object.map.set', { name: 'pebble-too', rows: ['..', '..'], resize: true }), { code: 'size_mismatch' });
    await assert.rejects(call(s, 'object.map.set', { name: 'block', rows: ['..'] }), { code: 'not_editable' });
    assert.deepEqual((await call(s, 'check', { name: 'pebble-too' })).problems, []);
  });
});
