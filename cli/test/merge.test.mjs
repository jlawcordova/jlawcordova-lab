// merge3 (docs/intents/2026-10-mcp-gaps/spec.md, D4): a three-way line merge
// of canonical sources.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { hunks, merge3 } from '../lib/merge.mjs';

const lines = (/** @type {string[]} */ ...l) => l.join('\n');
const BASE = lines('a', 'b', 'c', 'd', 'e');

test('R8: hunks are the base ranges each side replaced', () => {
  assert.deepEqual(hunks(['a', 'b', 'c'], ['a', 'x', 'c']), [{ from: 1, to: 2, lines: ['x'] }]);
  assert.deepEqual(hunks(['a', 'b'], ['a', 'n', 'b']), [{ from: 1, to: 1, lines: ['n'] }]);
  assert.deepEqual(hunks(['a', 'b'], ['b']), [{ from: 0, to: 1, lines: [] }]);
});

test('R8: one side changed: take it', () => {
  assert.deepEqual(merge3(BASE, BASE, lines('a', 'x', 'c', 'd', 'e')), { text: lines('a', 'x', 'c', 'd', 'e'), conflicts: [] });
  assert.deepEqual(merge3(BASE, lines('a', 'x', 'c', 'd', 'e'), BASE), { text: lines('a', 'x', 'c', 'd', 'e'), conflicts: [] });
});

test('R8: changes to different lines both apply, adjacent ones too', () => {
  assert.equal(merge3(BASE, lines('a', 'B', 'c', 'd', 'e'), lines('a', 'b', 'c', 'D', 'e')).text, lines('a', 'B', 'c', 'D', 'e'));
  assert.equal(merge3(BASE, lines('a', 'B', 'c', 'd', 'e'), lines('a', 'b', 'C', 'd', 'e')).text, lines('a', 'B', 'C', 'd', 'e'));
  assert.equal(merge3(BASE, lines('a', 'b', 'c', 'd', 'e', 'f'), lines('z', 'a', 'b', 'c', 'd', 'e')).text, lines('z', 'a', 'b', 'c', 'd', 'e', 'f'));
  assert.equal(merge3(BASE, lines('a', 'c', 'd', 'e'), lines('a', 'b', 'c', 'e')).text, lines('a', 'c', 'e'));
});

test('R8: the same change on both sides applies once', () => {
  assert.deepEqual(merge3(BASE, lines('a', 'x', 'c', 'd', 'e'), lines('a', 'x', 'c', 'd', 'e')).text, lines('a', 'x', 'c', 'd', 'e'));
  assert.deepEqual(merge3(BASE, lines('a', 'x', 'c', 'D', 'e'), lines('a', 'x', 'c', 'd', 'e')), { text: lines('a', 'x', 'c', 'D', 'e'), conflicts: [] });
});

test('R8: different changes to the same line clash, with both sides shown', () => {
  const r = merge3(BASE, lines('a', 'x', 'c', 'd', 'e'), lines('a', 'y', 'c', 'd', 'e'));
  assert.deepEqual(r, { text: '', conflicts: [{ base: [2, 2], ours: ['x'], theirs: ['y'] }] });
  // One side deletes a line the other changes.
  assert.equal(merge3(BASE, lines('a', 'c', 'd', 'e'), lines('a', 'y', 'c', 'd', 'e')).conflicts.length, 1);
});

test('R8: insertions at the same point clash, unless union keeps both (ours first)', () => {
  const ours = lines('a', 'b', 'c', 'd', 'e', 'mine');
  const theirs = lines('a', 'b', 'c', 'd', 'e', 'yours');
  assert.equal(merge3(BASE, ours, theirs).conflicts.length, 1);
  assert.deepEqual(merge3(BASE, ours, theirs, { union: true }), { text: lines('a', 'b', 'c', 'd', 'e', 'mine', 'yours'), conflicts: [] });
  // union never joins a replacement with an insertion.
  assert.equal(merge3(BASE, lines('a', 'B', 'c', 'd', 'e'), lines('a', 'n', 'b', 'c', 'd', 'e'), { union: true }).text, lines('a', 'n', 'B', 'c', 'd', 'e'));
  assert.equal(merge3(BASE, lines('a', 'B', 'c', 'd', 'e'), lines('a', 'y', 'c', 'd', 'e'), { union: true }).conflicts.length, 1);
});
