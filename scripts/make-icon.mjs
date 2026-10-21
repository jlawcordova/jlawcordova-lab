#!/usr/bin/env node
// Renders Pixel Lab's app icon: the house, at a whole pixel scale, on a
// 1024×1024 warm squircle with a faint iso grid. The art comes from the
// program (target/release/pixellab object render, a 1× PNG), which this reads
// back; everything else is Node built-ins. Reads the document, never writes it.
//
//   node scripts/make-icon.mjs [--document <path> | --site <path>] [--object house] [--out app/Sources/PixelLab/Resources/AppIcon.png]
//
// scripts/build-app.sh turns the PNG into AppIcon.icns with sips and iconutil.

import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { crc32, deflateSync, inflateSync } from 'node:zlib';

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

const PROGRAM = join(repo, 'target/release/pixellab');

/** The object's frame 0 at 1×, from the program: its pixels (as "x,y" → [r, g, b]) with x and y in map coordinates. */
function objectPixels() {
  const env = { ...process.env, PATH: `${join(homedir(), '.cargo/bin')}:${process.env.PATH ?? ''}` };
  const built = spawnSync('cargo', ['build', '--release', '--quiet', '--bin', 'pixellab'], { cwd: repo, env, stdio: ['ignore', 'inherit', 'inherit'] });
  if (built.status !== 0) throw new Error('cargo build --release failed');
  const doc = flag('document') ? ['--document', flag('document')] : flag('site') ? ['--site', flag('site')] : [];
  const r = spawnSync(PROGRAM, ['object', 'render', objectName, '--json', ...doc], { cwd: repo, env, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  const result = JSON.parse(r.stdout || '{}');
  if (r.status !== 0 || result.error) throw new Error(`pixellab object render ${objectName}: ${result.error?.message ?? r.stderr}`);
  const png = decodePng(Buffer.from(result.png, 'base64'));
  const [vx, vy] = result.viewBox;
  const map = new Map();
  for (let y = 0; y < png.height; y++) {
    for (let x = 0; x < png.width; x++) {
      const i = (y * png.width + x) * 4;
      if (png.rgba[i + 3] === 0) continue;
      map.set(`${vx + x},${vy + y}`, [png.rgba[i], png.rgba[i + 1], png.rgba[i + 2]]);
    }
  }
  return map;
}

/** Decodes an 8-bit RGBA, non-interlaced PNG. @param {Buffer} data */
function decodePng(data) {
  let at = 8;
  let width = 0;
  let height = 0;
  const idat = [];
  while (at < data.length) {
    const len = data.readUInt32BE(at);
    const type = data.toString('latin1', at + 4, at + 8);
    const body = data.subarray(at + 8, at + 8 + len);
    if (type === 'IHDR') {
      width = body.readUInt32BE(0);
      height = body.readUInt32BE(4);
      if (body[8] !== 8 || body[9] !== 6 || body[12] !== 0) throw new Error('expected an 8-bit RGBA PNG without interlace');
    } else if (type === 'IDAT') idat.push(body);
    at += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const rgba = Buffer.alloc(width * height * 4);
  const stride = width * 4;
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    for (let x = 0; x < stride; x++) {
      const v = raw[y * (stride + 1) + 1 + x];
      const a = x >= 4 ? rgba[y * stride + x - 4] : 0;
      const b = y > 0 ? rgba[(y - 1) * stride + x] : 0;
      const c = x >= 4 && y > 0 ? rgba[(y - 1) * stride + x - 4] : 0;
      let add = 0;
      if (filter === 1) add = a;
      else if (filter === 2) add = b;
      else if (filter === 3) add = (a + b) >> 1;
      else if (filter === 4) {
        const p = a + b - c;
        const pa = Math.abs(p - a);
        const pb = Math.abs(p - b);
        const pc = Math.abs(p - c);
        add = pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      rgba[y * stride + x] = (v + add) & 255;
    }
  }
  return { width, height, rgba };
}

/** Encodes 8-bit RGBA as a PNG. @param {number} width @param {number} height @param {Buffer} rgba */
function encodePng(width, height, rgba) {
  const stride = width * 4 + 1;
  const raw = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) rgba.copy(raw, y * stride + 1, y * width * 4, (y + 1) * width * 4);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr.set([8, 6, 0, 0, 0], 8);
  const chunk = (/** @type {string} */ type, /** @type {Buffer} */ body) => {
    const head = Buffer.alloc(8);
    head.writeUInt32BE(body.length, 0);
    head.write(type, 4, 'latin1');
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), body])) >>> 0, 0);
    return Buffer.concat([head, body, crc]);
  };
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}

const pixels = objectPixels();
if (pixels.size === 0) throw new Error(`object ${objectName} has no pixels`);
let [minX, minY, maxX, maxY] = [Infinity, Infinity, -Infinity, -Infinity];
for (const key of pixels.keys()) {
  const [px, py] = key.split(',').map(Number);
  minX = Math.min(minX, px);
  maxX = Math.max(maxX, px);
  minY = Math.min(minY, py);
  maxY = Math.max(maxY, py);
}
const box = { minX, minY, maxX, maxY };
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

for (const [key, rgb] of pixels) {
  const [px, py] = key.split(',').map(Number);
  const x0 = ox + (px - box.minX) * scale;
  const y0 = oy + (py - box.minY) * scale;
  for (let y = y0; y < y0 + scale; y++) for (let x = x0; x < x0 + scale; x++) blend((y * SIZE + x) * 4, rgb, 1);
}

mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, encodePng(SIZE, SIZE, rgba));
console.log(`wrote ${out} (${objectName} at ${scale}×, 1024×1024)`);
