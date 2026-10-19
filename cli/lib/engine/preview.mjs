// PNG previews of pixel art (the site's pixel-art engine spec R29), from the
// engine's own pixels, with Node built-ins only. Ported from the site's
// scripts/pixel-art-preview.mjs (jlawcordova.github.io at 3d2a9d1). An object
// stands on one 32×16 tile outline; a scene shows its viewBox, with only its
// first data-class variant. Both sit on the document's card color, at frame 0
// of every loop. `pixellab art preview <name>` writes them at 1×, 2×, 3× and
// 4×.

import { crc32, deflateSync } from 'node:zlib';

import { bounds, composite, renderObject, renderScene } from './engine.mjs';
import { TILE_H, TILE_W } from './iso.mjs';

const MARGIN = 2;

/**
 * The preview colors a document gets when its manifest names none: the
 * site's --color-card and --color-grid as they were when the engine moved
 * here.
 */
export const DEFAULT_PREVIEW = { card: '#E9DCC6', grid: 'rgba(90, 62, 43, 0.06)' };

/**
 * The card color and the grid line color composited over it, as opaque RGB,
 * from a manifest's `preview` ("#RRGGBB" and "rgba(r, g, b, a)" or
 * "#RRGGBB"), so a document can match the page it's published to.
 * @param {{ card?: string, grid?: string }} [preview]
 * @returns {{ card: number[], grid: number[] }}
 */
export function previewColors(preview = {}) {
  const cardText = preview.card ?? DEFAULT_PREVIEW.card;
  const gridText = preview.grid ?? DEFAULT_PREVIEW.grid;
  const card = /^#([0-9A-Fa-f]{6})$/.exec(cardText.trim());
  if (!card) throw new Error(`preview.card ${JSON.stringify(cardText)} isn't a #RRGGBB color`);
  const cardRgb = [0, 2, 4].map((i) => parseInt(card[1].slice(i, i + 2), 16));
  const rgba = /^rgba\((\d+),\s*(\d+),\s*(\d+),\s*([\d.]+)\)$/.exec(gridText.trim());
  const hex = /^#([0-9A-Fa-f]{6})$/.exec(gridText.trim());
  if (rgba) {
    const alpha = Number(rgba[4]);
    return { card: cardRgb, grid: cardRgb.map((c, i) => Math.round(Number(rgba[i + 1]) * alpha + c * (1 - alpha))) };
  }
  if (hex) return { card: cardRgb, grid: [0, 2, 4].map((i) => parseInt(hex[1].slice(i, i + 2), 16)) };
  throw new Error(`preview.grid ${JSON.stringify(gridText)} isn't a #RRGGBB or rgba(r, g, b, a) color`);
}

/**
 * The outline of the tile whose top-face center is (0, 0): rows of width 4,
 * 8, … 32, 32, … 8, 4, so every edge steps 2 across for 1 down.
 * @returns {string[]} "x,y" pixels
 */
function tileOutline() {
  const rows = TILE_H;
  const half = (/** @type {number} */ r) => (r < rows / 2 ? 2 * (r + 1) : 2 * (rows - r));
  const inside = (/** @type {number} */ x, /** @type {number} */ r) => r >= 0 && r < rows && x >= -half(r) && x < half(r);
  const out = [];
  for (let r = 0; r < rows; r++) {
    for (let x = -half(r); x < half(r); x++) {
      if (!inside(x, r - 1) || !inside(x, r + 1) || !inside(x - 1, r) || !inside(x + 1, r)) out.push(`${x},${r - rows / 2}`);
    }
  }
  if (out.some((p) => Math.abs(Number(p.split(',')[0])) > TILE_W / 2)) throw new Error('tile outline is wider than a tile');
  return out;
}

/**
 * Renders an object (on a tile) or a scene (at its viewBox) to RGBA.
 * @param {import('../src/lib/pixel-art/engine.mjs').Sources} sources
 * @param {{ object?: string, scene?: string }} target
 * @param {{ card: number[], grid: number[] }} colors
 * @returns {{ image: { width: number, height: number, rgba: Buffer }, origin: [number, number] }}
 */
export function renderPreview(sources, target, { card, grid }) {
  /** @type {Map<string, number[]>} */
  const paint = new Map();
  const rgb = (/** @type {string} */ name) => {
    const hex = /** @type {string} */ (sources.colors.get(name)?.hex);
    return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  };
  let box;
  if (target.scene) {
    const { viewBox, root } = renderScene(sources, target.scene);
    for (const [p, name] of composite(root, { firstVariant: true })) paint.set(p, rgb(name));
    const [x, y, w, h] = viewBox;
    box = { minX: x, minY: y, maxX: x + w - 1, maxY: y + h - 1 };
  } else {
    for (const p of tileOutline()) paint.set(p, grid);
    for (const [p, name] of composite(renderObject(sources, /** @type {string} */ (target.object)))) paint.set(p, rgb(name));
    const b = /** @type {NonNullable<ReturnType<typeof bounds>>} */ (bounds(paint.keys()));
    box = { minX: b.minX - MARGIN, minY: b.minY - MARGIN, maxX: b.maxX + MARGIN, maxY: b.maxY + MARGIN };
  }
  const width = box.maxX - box.minX + 1;
  const height = box.maxY - box.minY + 1;
  const rgba = Buffer.alloc(width * height * 4);
  for (let i = 0; i < width * height; i++) rgba.set([...card, 255], i * 4);
  for (const [p, color] of paint) {
    const [x, y] = p.split(',').map(Number);
    if (x < box.minX || x > box.maxX || y < box.minY || y > box.maxY) continue;
    rgba.set([...color, 255], ((y - box.minY) * width + (x - box.minX)) * 4);
  }
  return { image: { width, height, rgba }, origin: [box.minX, box.minY] };
}

export const MATERIALS = ['grass', 'soil', 'wood', 'path', 'water', 'roof', 'gold'];
const SWATCH = 16;
const GAP = 2;

/**
 * The world palette's swatch sheet (spec R31): one row per material with its
 * four shades, highlight to shadow, then a row for ink, cream and the skin
 * tones. A name that's missing leaves its swatch as the card color.
 * @param {import('../src/lib/pixel-art/engine.mjs').Sources} sources
 * @param {{ card: number[], grid: number[] }} colors
 * @returns {{ image: { width: number, height: number, rgba: Buffer } }}
 */
export function renderPalette(sources, { card, grid }) {
  const line = grid.map((v) => Math.round(v * 0.75));
  const rows = [...MATERIALS.map((m) => [1, 2, 3, 4].map((n) => `${m}-${n}`)), ['ink', 'cream', 'skin-1', 'skin-2']];
  const width = GAP + 4 * (SWATCH + GAP);
  const height = GAP + rows.length * (SWATCH + GAP);
  const rgba = Buffer.alloc(width * height * 4);
  for (let i = 0; i < width * height; i++) rgba.set([...card, 255], i * 4);
  rows.forEach((names, r) => {
    names.forEach((name, c) => {
      const hex = sources.colors.get(name)?.hex;
      if (!hex) return;
      const rgb = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
      for (let y = 0; y < SWATCH; y++) {
        for (let x = 0; x < SWATCH; x++) {
          // A 1-pixel line in the grid's color keeps a swatch as light as the card visible.
          const edge = x === 0 || y === 0 || x === SWATCH - 1 || y === SWATCH - 1;
          rgba.set([...(edge ? line : rgb), 255], ((GAP + r * (SWATCH + GAP) + y) * width + GAP + c * (SWATCH + GAP) + x) * 4);
        }
      }
    });
  });
  return { image: { width, height, rgba } };
}

/**
 * An RGBA PNG of the image, scaled up by whole-pixel repetition.
 * @param {{ width: number, height: number, rgba: Buffer }} image
 * @param {number} scale
 */
export function encodePng({ width, height, rgba }, scale) {
  const w = width * scale;
  const h = height * scale;
  const stride = w * 4 + 1;
  const raw = Buffer.alloc(stride * h);
  for (let y = 0; y < h; y++) {
    const sy = Math.floor(y / scale);
    for (let x = 0; x < w; x++) {
      const from = (sy * width + Math.floor(x / scale)) * 4;
      rgba.copy(raw, y * stride + 1 + x * 4, from, from + 4);
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr.set([8, 6, 0, 0, 0], 8); // 8-bit RGBA, deflate, no filter set, no interlace
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw)),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

/**
 * @param {string} type
 * @param {Buffer} data
 */
function chunk(type, data) {
  const head = Buffer.alloc(8);
  head.writeUInt32BE(data.length, 0);
  head.write(type, 4, 'latin1');
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])) >>> 0, 0);
  return Buffer.concat([head, data, crc]);
}
