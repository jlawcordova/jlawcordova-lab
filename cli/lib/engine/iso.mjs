// @ts-check
// The isometric grid (pixel-art engine spec D4): 2:1 dimetric, 32×16 tiles,
// 16-pixel levels. Grid positions only; block faces come with the block kind.

export const TILE_W = 32;
export const TILE_H = 16;
export const LEVEL_H = 16;

/**
 * The top-face center of the tile at [col, row, level], in scene pixels.
 * `origin` is where tile [0, 0, 0] sits.
 * @param {number[]} tile
 * @param {number[]} origin
 * @returns {[number, number]}
 */
export function tileToPx([col, row, level], [ox, oy]) {
  return [(col - row) * (TILE_W / 2) + ox, (col + row) * (TILE_H / 2) - level * LEVEL_H + oy];
}

/**
 * The tile at `level` whose top face holds the pixel (x, y): the inverse of
 * tileToPx, used by the editor to find the tile under the pointer. Inside a
 * tile, |dx|/32 + |dy|/16 < ½, so rounding finds the column and row. `+ 0`
 * turns a -0 into 0.
 * @param {number[]} pixel
 * @param {number} level
 * @param {number[]} origin
 * @returns {[number, number, number]}
 */
export function pxToTile([x, y], level, [ox, oy]) {
  const u = (x + 0.5 - ox) / TILE_W;
  const v = (y + 0.5 - oy + level * LEVEL_H) / TILE_H;
  return [Math.round(v + u) + 0, Math.round(v - u) + 0, level];
}

/**
 * The pixels of the tile whose top-face center is (cx, cy), as "x,y". A pixel
 * belongs to the top face when its center, (x + ½, y + ½), satisfies
 * |dx|/16 + |dy|/8 < 1 from the tile's center. Pixel centers are half-way
 * between whole numbers, so the sum is never exactly 1: no pixel sits on an
 * edge, and neighbouring tiles share no pixel and leave no gap. Every row is
 * 2 pixels wider on each side than the one above it (2, 6, … 30, 30, … 6, 2
 * wide), which keeps every edge at 2:1.
 * @param {number} cx
 * @param {number} cy
 * @returns {string[]}
 */
export function tilePixels(cx, cy) {
  /** @type {string[]} */
  const out = [];
  for (let y = cy - TILE_H / 2; y < cy + TILE_H / 2; y++) {
    const half = TILE_W / 2 - 2 * Math.abs(y + 0.5 - cy);
    for (let x = cx - TILE_W / 2; x < cx + TILE_W / 2; x++) if (Math.abs(x + 0.5 - cx) < half) out.push(`${x},${y}`);
  }
  return out;
}

/**
 * The top-face centers of a block's footprint, in scene pixels: `size` is
 * [tiles wide, tiles deep, levels high], and (cx, cy) is the first tile's
 * center. Columns run to the right and down, rows to the left and down.
 * @param {number[]} size
 * @param {number[]} center
 * @returns {[number, number][]}
 */
export function footprint([width, depth], [cx, cy]) {
  /** @type {[number, number][]} */
  const out = [];
  for (let j = 0; j < depth; j++) for (let i = 0; i < width; i++) out.push([cx + (i - j) * (TILE_W / 2), cy + (i + j) * (TILE_H / 2)]);
  return out;
}

/**
 * The width and height of a block's silhouette, from its size alone. It
 * agrees with the drawn faces (a test checks it), and costs nothing however
 * big `size` is.
 * @param {number[]} size
 * @returns {[number, number]}
 */
export function blockSize([width, depth, levels]) {
  return [(width + depth) * (TILE_W / 2) - 2, (width + depth) * (TILE_H / 2) + levels * LEVEL_H];
}

/**
 * The faces of a block whose first tile's top-face center is (cx, cy), as
 * "x,y" pixels (spec D3, D4). The top is the union of the footprint's tiles.
 * The sides hang `levels × 16` pixels below the top, split at the vertical
 * line through the top's lowest corner: the left face (x before it) and the
 * right face (x from it on). The edge is the 1-pixel outline of the whole
 * silhouette, and is drawn over the faces.
 * @param {number[]} size
 * @param {number[]} center
 * @returns {{ top: string[], left: string[], right: string[], edge: string[] }}
 */
export function blockFaces(size, center) {
  const [width, depth, levels] = size;
  const [cx] = center;
  /** @type {Map<number, [number, number]>} */
  const spans = new Map();
  /** @type {string[]} */
  const top = [];
  for (const [tx, ty] of footprint(size, center)) {
    for (const p of tilePixels(tx, ty)) {
      top.push(p);
      const [x, y] = p.split(',').map(Number);
      const span = spans.get(x);
      if (span) {
        span[0] = Math.min(span[0], y);
        span[1] = Math.max(span[1], y);
      } else spans.set(x, [y, y]);
    }
  }
  const split = cx + (width - depth) * (TILE_W / 2);
  /** @type {string[]} */
  const left = [];
  /** @type {string[]} */
  const right = [];
  for (const [x, [, bottom]] of spans) {
    for (let y = bottom + 1; y <= bottom + levels * LEVEL_H; y++) (x < split ? left : right).push(`${x},${y}`);
  }
  const solid = new Set([...top, ...left, ...right]);
  const edge = [...solid].filter((p) => {
    const [x, y] = p.split(',').map(Number);
    return [`${x - 1},${y}`, `${x + 1},${y}`, `${x},${y - 1}`, `${x},${y + 1}`].some((n) => !solid.has(n));
  });
  return { top, left, right, edge };
}
