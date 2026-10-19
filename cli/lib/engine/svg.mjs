// @ts-check
// Layer tree → rect SVG (pixel-art engine spec D6 step 3), the input that
// scripts/optimize-pixel-art.mjs turns into paths. In each layer there's one
// <g fill> per color, in order of first paint, holding one <rect> per
// horizontal run, sorted by y and then x.

/** @typedef {import('./engine.mjs').Group} Group */
/** @typedef {import('./engine.mjs').Layer} Layer */

const escape = (/** @type {string} */ v) => v.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');

/**
 * A scene with motion carries its CSS in a <style> first (motion.mjs).
 * @param {{ viewBox: number[], root: Group, style?: string }} scene
 * @param {(name: string) => string} hexOf  color name → "#RRGGBB"
 */
export function toRectSvg({ viewBox, root, style }, hexOf) {
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" aria-hidden="true" viewBox="${viewBox.join(' ')}" shape-rendering="crispEdges">` +
    (style ? `<style>${style}</style>` : '') +
    children(root, hexOf) +
    '</svg>'
  );
}

/**
 * @param {Group} group
 * @param {(name: string) => string} hexOf
 * @returns {string}
 */
function children(group, hexOf) {
  return group.children
    .map((child) => {
      if ('pixels' in child) return layer(child, hexOf);
      const attrs = child.attrs.map(([k, v]) => ` ${k}="${escape(v)}"`).join('');
      return `<g${attrs}>${children(child, hexOf)}</g>`;
    })
    .join('');
}

/**
 * @param {Layer} layer
 * @param {(name: string) => string} hexOf
 */
function layer({ pixels, colors }, hexOf) {
  // Colors are grouped by value, so two names for one hex share a group.
  /** @type {Map<string, number[][]>} */
  const byHex = new Map();
  for (const name of colors) if (!byHex.has(hexOf(name))) byHex.set(hexOf(name), []);
  for (const [p, name] of pixels) {
    const [x, y] = p.split(',').map(Number);
    /** @type {number[][]} */ (byHex.get(hexOf(name))).push([x, y]);
  }
  let out = '';
  for (const [hex, points] of byHex) {
    if (points.length === 0) continue;
    points.sort((a, b) => a[1] - b[1] || a[0] - b[0]);
    out += `<g fill="${hex}">`;
    for (let i = 0; i < points.length; ) {
      let j = i;
      while (j + 1 < points.length && points[j + 1][1] === points[i][1] && points[j + 1][0] === points[j][0] + 1) j++;
      out += `<rect x="${points[i][0]}" y="${points[i][1]}" width="${j - i + 1}" height="1"/>`;
      i = j + 1;
    }
    out += '</g>';
  }
  return out;
}
