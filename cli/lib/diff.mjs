// A unified line diff (Myers' O(ND) algorithm), for dry runs and proposals.
// Sources are canonical (serialize.mjs: one map row or item per line), so a
// line diff is the diff a person reads in a pull request.

/**
 * The edit script from a to b: a list of [op, line] with op ' ', '-' or '+'.
 * @param {string[]} a
 * @param {string[]} b
 * @returns {[string, string][]}
 */
export function diffLines(a, b) {
  // Common prefix and suffix first: most edits touch a few lines.
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let endA = a.length;
  let endB = b.length;
  while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) {
    endA--;
    endB--;
  }
  const mid = myers(a.slice(start, endA), b.slice(start, endB));
  return [
    ...a.slice(0, start).map((l) => /** @type {[string, string]} */ ([' ', l])),
    ...mid,
    ...a.slice(endA).map((l) => /** @type {[string, string]} */ ([' ', l])),
  ];
}

/**
 * @param {string[]} a
 * @param {string[]} b
 * @returns {[string, string][]}
 */
function myers(a, b) {
  const n = a.length;
  const m = b.length;
  if (n === 0) return b.map((l) => ['+', l]);
  if (m === 0) return a.map((l) => ['-', l]);
  const max = n + m;
  const offset = max;
  let v = new Int32Array(2 * max + 2);
  /** @type {Int32Array[]} */
  const trace = [];
  let found = false;
  for (let d = 0; d <= max && !found; d++) {
    trace.push(v.slice());
    for (let k = -d; k <= d; k += 2) {
      let x = k === -d || (k !== d && v[offset + k - 1] < v[offset + k + 1]) ? v[offset + k + 1] : v[offset + k - 1] + 1;
      let y = x - k;
      while (x < n && y < m && a[x] === b[y]) {
        x++;
        y++;
      }
      v[offset + k] = x;
      if (x >= n && y >= m) {
        found = true;
        break;
      }
    }
  }
  // Walk back through the trace.
  /** @type {[string, string][]} */
  const out = [];
  let x = n;
  let y = m;
  for (let d = trace.length - 1; d >= 0; d--) {
    const vd = trace[d];
    const k = x - y;
    const prevK = k === -d || (k !== d && vd[offset + k - 1] < vd[offset + k + 1]) ? k + 1 : k - 1;
    const prevX = vd[offset + prevK];
    const prevY = prevX - prevK;
    while (x > prevX && y > prevY) {
      x--;
      y--;
      out.push([' ', a[x]]);
    }
    if (d > 0) {
      if (x === prevX) out.push(['+', b[--y]]);
      else out.push(['-', a[--x]]);
    }
  }
  return out.reverse();
}

/**
 * A unified diff of two texts, '' when they're equal.
 * @param {string} before
 * @param {string} after
 * @param {{ from?: string, to?: string, context?: number }} [options]
 */
export function unifiedDiff(before, after, { from = 'a', to = 'b', context = 3 } = {}) {
  if (before === after) return '';
  const split = (/** @type {string} */ t) => (t === '' ? [] : t.replace(/\n$/, '').split('\n'));
  const ops = diffLines(split(before), split(after));
  /** @type {string[]} */
  const lines = [`--- ${from}`, `+++ ${to}`];
  // Group changes into hunks with `context` lines around them.
  let i = 0;
  let lineA = 1;
  let lineB = 1;
  /** Line numbers at each op. */
  const at = ops.map(([op]) => {
    const here = [lineA, lineB];
    if (op !== '+') lineA++;
    if (op !== '-') lineB++;
    return here;
  });
  while (i < ops.length) {
    while (i < ops.length && ops[i][0] === ' ') i++;
    if (i >= ops.length) break;
    let start = Math.max(0, i - context);
    let end = i;
    // Extend while the next change is within 2 × context of this one.
    for (;;) {
      while (end < ops.length && ops[end][0] !== ' ') end++;
      let next = end;
      while (next < ops.length && ops[next][0] === ' ') next++;
      if (next < ops.length && next - end <= 2 * context) end = next;
      else {
        end = Math.min(ops.length, end + context);
        break;
      }
    }
    const hunk = ops.slice(start, end);
    const countA = hunk.filter(([op]) => op !== '+').length;
    const countB = hunk.filter(([op]) => op !== '-').length;
    const [a0, b0] = at[start];
    lines.push(`@@ -${countA ? a0 : a0 - 1},${countA} +${countB ? b0 : b0 - 1},${countB} @@`);
    for (const [op, line] of hunk) lines.push(`${op}${line}`);
    i = end;
  }
  return `${lines.join('\n')}\n`;
}
