// A three-way line merge (docs/intents/2026-10-mcp-gaps/spec.md, D4), for a
// working copy whose file changed on disk. Sources are canonical (one map row
// or item per line), so lines are the grain a person reads in a diff: edits
// to different items or rows merge, the same row edited on both sides clashes.
// With `union`, two insertions at the same point both go in (ours first): a
// scene's items are one per line, so both sides' new items are kept, where
// an object's new keys could collide and must clash.

import { diffLines } from './diff.mjs';

/**
 * @typedef {{ from: number, to: number, lines: string[] }} Hunk  base lines [from, to) become `lines`
 * @typedef {{ base: [number, number], ours: string[], theirs: string[] }} Conflict  base lines are 1-based, inclusive (an empty range is [n + 1, n])
 */

/**
 * The changes from `base` to `other`, as hunks over the base's lines.
 * @param {string[]} base
 * @param {string[]} other
 * @returns {Hunk[]}
 */
export function hunks(base, other) {
  /** @type {Hunk[]} */
  const out = [];
  let at = 0;
  /** @type {Hunk | null} */
  let open = null;
  for (const [op, line] of diffLines(base, other)) {
    if (op === ' ') {
      if (open) out.push(open);
      open = null;
      at++;
      continue;
    }
    open ??= { from: at, to: at, lines: [] };
    if (op === '-') {
      open.to++;
      at++;
    } else open.lines.push(line);
  }
  if (open) out.push(open);
  return out;
}

/** Whether two hunks touch the same base lines (two insertions at one point count). */
const overlaps = (/** @type {Hunk} */ a, /** @type {Hunk} */ b) => {
  if (a.from === a.to && b.from === b.to) return a.from === b.from;
  if (a.from === a.to) return a.from > b.from && a.from < b.to;
  if (b.from === b.to) return b.from > a.from && b.from < a.to;
  return a.from < b.to && b.from < a.to;
};

const same = (/** @type {Hunk} */ a, /** @type {Hunk} */ b) => a.from === b.from && a.to === b.to && a.lines.join('\n') === b.lines.join('\n');

/**
 * Merges two texts that both started from `base`. Returns the merged text,
 * and the places both sides changed differently (empty when it's clean).
 * @param {string} base
 * @param {string} ours
 * @param {string} theirs
 * @param {{ union?: boolean }} [options]  union: keep both sides' insertions at one point
 * @returns {{ text: string, conflicts: Conflict[] }}
 */
export function merge3(base, ours, theirs, { union = false } = {}) {
  if (ours === theirs) return { text: ours, conflicts: [] };
  if (ours === base) return { text: theirs, conflicts: [] };
  if (theirs === base) return { text: ours, conflicts: [] };
  const b = base.split('\n');
  const mine = hunks(b, ours.split('\n'));
  const yours = hunks(b, theirs.split('\n'));
  /** @type {Conflict[]} */
  const conflicts = [];
  /** @type {Hunk[]} */
  const apply = [];
  // Group hunks that overlap, transitively, across the two sides.
  const all = [...mine.map((h) => ({ h, side: 0 })), ...yours.map((h) => ({ h, side: 1 }))].sort((x, y) => x.h.from - y.h.from || x.h.to - y.h.to || x.side - y.side);
  let i = 0;
  while (i < all.length) {
    const group = [all[i]];
    let j = i + 1;
    while (j < all.length && group.some((g) => g.side !== all[j].side && overlaps(g.h, all[j].h))) group.push(all[j++]);
    const sides = new Set(group.map((g) => g.side));
    if (sides.size === 1) for (const g of group) apply.push(g.h);
    else {
      const a = group.filter((g) => g.side === 0).map((g) => g.h);
      const c = group.filter((g) => g.side === 1).map((g) => g.h);
      const inserts = (/** @type {Hunk[]} */ list) => list.every((h) => h.from === h.to);
      if (a.length === 1 && c.length === 1 && same(a[0], c[0])) apply.push(a[0]);
      else if (union && inserts(a) && inserts(c) && new Set(group.map((g) => g.h.from)).size === 1) {
        apply.push({ from: a[0].from, to: a[0].from, lines: [...a.flatMap((h) => h.lines), ...c.flatMap((h) => h.lines)] });
      } else {
        const from = Math.min(...group.map((g) => g.h.from));
        const to = Math.max(...group.map((g) => g.h.to));
        conflicts.push({ base: [from + 1, to], ours: side(b, a, from, to), theirs: side(b, c, from, to) });
      }
    }
    i = j;
  }
  if (conflicts.length) return { text: '', conflicts };
  apply.sort((x, y) => x.from - y.from || x.to - y.to);
  /** @type {string[]} */
  const out = [];
  let at = 0;
  for (const h of apply) {
    out.push(...b.slice(at, h.from), ...h.lines);
    at = h.to;
  }
  out.push(...b.slice(at));
  return { text: out.join('\n'), conflicts };
}

/**
 * After a scene's union merge: when both sides made a scene's first layers,
 * both `layers: [ … ]` blocks went in, and importing would keep only the
 * last. Folds them into one list, ours first (docs/intents/2026-10-scene-layers/spec.md,
 * D1). Returns what can't be folded: a layer id both sides made, and any other
 * top-level key both sides added (two `motion:` blocks), for the caller to refuse.
 * Works on canonical source text: top-level keys are the lines indented two spaces.
 * @param {string} text
 * @returns {{ text: string, clashes: string[] }}
 */
export function foldScene(text) {
  const lines = text.split('\n');
  const isKey = (/** @type {string} */ l) => /^ {2}[A-Za-z_$][\w$]*: /.test(l);
  /** @type {{ start: number, end: number }[]} */
  const blocks = [];
  for (let i = 0; i < lines.length; i++) {
    if (lines[i] !== '  layers: [') continue;
    let end = i + 1;
    while (end < lines.length && lines[end] !== '  ],') end++;
    blocks.push({ start: i, end });
    i = end;
  }
  /** @type {string[]} */
  const clashes = [];
  let out = lines;
  if (blocks.length > 1) {
    const entries = blocks.map((b) => lines.slice(b.start + 1, b.end));
    const idOf = (/** @type {string} */ l) => /\bid: '([^']*)'/.exec(l)?.[1];
    /** @type {Set<string>} */
    const seen = new Set();
    /** @type {string[]} */
    const kept = [];
    for (const list of entries) {
      for (const l of list) {
        const id = idOf(l);
        if (id !== undefined && seen.has(id)) clashes.push(`layer ${id}`);
        if (id !== undefined) seen.add(id);
        kept.push(l);
      }
    }
    const drop = new Set(blocks.slice(1).flatMap((b) => Array.from({ length: b.end - b.start + 1 }, (_, k) => b.start + k)));
    out = lines.flatMap((l, i) => {
      if (i === blocks[0].start) return [l, ...kept];
      if (i > blocks[0].start && i < blocks[0].end) return [];
      return drop.has(i) ? [] : [l];
    });
  }
  /** @type {Map<string, number>} */
  const counts = new Map();
  for (const l of out) if (isKey(l)) counts.set(l.slice(2, l.indexOf(':')), (counts.get(l.slice(2, l.indexOf(':'))) ?? 0) + 1);
  for (const [key, n] of counts) if (n > 1) clashes.push(key);
  return { text: out.join('\n'), clashes };
}

/** One side's text for base lines [from, to): its hunks there over the base. */
function side(/** @type {string[]} */ base, /** @type {Hunk[]} */ list, /** @type {number} */ from, /** @type {number} */ to) {
  /** @type {string[]} */
  const out = [];
  let at = from;
  for (const h of list) {
    out.push(...base.slice(at, h.from), ...h.lines);
    at = h.to;
  }
  out.push(...base.slice(at, to));
  return out;
}
