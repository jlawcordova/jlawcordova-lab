#!/usr/bin/env node
// Renders Pixel Lab's app icon with the engine (cli/lib/engine/): the house, at a whole
// pixel scale, on a 1024×1024 warm squircle with a faint iso grid. Node
// built-ins only; PNG via the engine's encodePng. Reads the document, never writes it.
//
//   node scripts/make-icon.mjs [--document <path> | --site <path>] [--object house] [--out app/Sources/PixelLab/Resources/AppIcon.png]
//
// scripts/build-app.sh turns the PNG into AppIcon.icns with sips and iconutil.

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createContext } from '../cli/lib/context.mjs';
import { openDocument } from '../cli/lib/document.mjs';

const repo = join(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const flag = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
};
const objectName = flag('object', 'house');
const out = flag('out', join(repo, 'app/Sources/PixelLab/Resources/AppIcon.png'));

const SIZE = 1024;
// macOS icon grid: an 824-point body with a 100-point margin.
const BODY = 824;
const GROUND_TOP = [0x35, 0x2c, 0x21];
const GROUND_BOTTOM = [0x2d, 0x25, 0x1c];
const GRID = [0xf1, 0xe8, 0xd8, 0.07];
const RIM = [0xf1, 0xe8, 0xd8, 0.1];

const site = await openDocument({ doc: flag('document'), site: flag('site') });
const { sources } = await createContext(site).sources();
if (!sources.objects.has(objectName)) throw new Error(`no object ${objectName} in ${site.root}`);
const pixels = site.engine.composite(site.engine.renderObject(sources, objectName), { frame: 0 });
const box = site.engine.bounds(pixels.keys());
const artW = box.maxX - box.minX + 1;
const artH = box.maxY - box.minY + 1;

const rgba = Buffer.alloc(SIZE * SIZE * 4);
const blend = (i, [r, g, b], a) => {
  const ia = rgba[i + 3] / 255;
  const oa = a + ia * (1 - a);
  if (oa === 0) return;
  rgba[i] = Math.round((r * a + rgba[i] * ia * (1 - a)) / oa);
  rgba[i + 1] = Math.round((g * a + rgba[i + 1] * ia * (1 - a)) / oa);
  rgba[i + 2] = Math.round((b * a + rgba[i + 2] * ia * (1 - a)) / oa);
  rgba[i + 3] = Math.round(oa * 255);
};

// Squircle coverage (superellipse, n = 5), 4×4 supersampled.
const half = BODY / 2;
const inside = (x, y) => Math.abs((x - SIZE / 2) / half) ** 5 + Math.abs((y - SIZE / 2) / half) ** 5 <= 1;
const coverage = (px, py) => {
  let n = 0;
  for (let sy = 0; sy < 4; sy++) for (let sx = 0; sx < 4; sx++) if (inside(px + (sx + 0.5) / 4, py + (sy + 0.5) / 4)) n++;
  return n / 16;
};

// The art's scale and placement: as big as fits 62% of the body, whole pixels.
const scale = Math.floor((BODY * 0.62) / Math.max(artW, artH));
const ox = Math.round(SIZE / 2 - (artW * scale) / 2);
const oy = Math.round(SIZE / 2 - (artH * scale) / 2 + scale * 2);
// The grid's origin: the art's anchor (0, 0), i.e. its footprint center.
const gx = ox + (0 - box.minX) * scale;
const gy = oy + (0 - box.minY) * scale;
const tileW = 32 * scale;
const tileH = 16 * scale;

for (let y = 0; y < SIZE; y++) {
  for (let x = 0; x < SIZE; x++) {
    const c = coverage(x, y);
    if (c === 0) continue;
    const i = (y * SIZE + x) * 4;
    const t = (y - (SIZE - BODY) / 2) / BODY;
    const ground = GROUND_TOP.map((v, k) => Math.round(v + (GROUND_BOTTOM[k] - v) * t));
    blend(i, ground, c);
    // Iso lattice: lines where (x/tileW ± y/tileH) is a half-integer.
    const u = (x + 0.5 - gx) / tileW;
    const v = (y + 0.5 - gy) / tileH;
    const d1 = Math.abs(((v + u + 0.5) % 1 + 1) % 1 - 0.5);
    const d2 = Math.abs(((v - u + 0.5) % 1 + 1) % 1 - 0.5);
    const near = Math.min(d1, d2) * tileH; // ≈ distance in pixels (vertically)
    if (near < 1.2) blend(i, GRID.slice(0, 3), GRID[3] * c * (1.2 - near));
    // A faint rim just inside the edge.
    if (c < 1 || !inside(x - 3, y) || !inside(x + 3, y) || !inside(x, y - 3) || !inside(x, y + 3)) blend(i, RIM.slice(0, 3), RIM[3] * c);
  }
}

for (const [key, name] of pixels) {
  const [px, py] = key.split(',').map(Number);
  const hex = sources.colors.get(name)?.hex ?? '#000000';
  const rgb = [1, 3, 5].map((k) => parseInt(hex.slice(k, k + 2), 16));
  const x0 = ox + (px - box.minX) * scale;
  const y0 = oy + (py - box.minY) * scale;
  for (let y = y0; y < y0 + scale; y++) for (let x = x0; x < x0 + scale; x++) blend((y * SIZE + x) * 4, rgb, 1);
}

mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, site.preview.encodePng({ width: SIZE, height: SIZE, rgba }, 1));
console.log(`wrote ${out} (${objectName} at ${scale}×, 1024×1024)`);
