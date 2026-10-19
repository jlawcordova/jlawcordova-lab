// Test helpers. Tests never touch the real site checkout: makeSite() copies
// the parts the CLI needs (the engine, the preview helpers, variables.css,
// the art sources and package.json) into a temp folder, and every test points
// --site at that copy, with HOME set to a temp folder so no real config file
// is read.

import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { cp, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { homedir, tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { crc32, inflateSync } from 'node:zlib';

import { main } from '../lib/cli.mjs';

export const REPO = fileURLToPath(new URL('../..', import.meta.url));
export const BIN = join(REPO, 'cli', 'bin', 'pixellab.mjs');

/** The site checkout the copies are made from (read only). */
export const SITE_SOURCE = process.env.PIXELLAB_TEST_SITE ?? join(homedir(), 'Projects', 'jlawcordova.github.io');
export const HAVE_SITE = existsSync(join(SITE_SOURCE, 'src', 'lib', 'pixel-art', 'engine.mjs'));
/** node:test's skip option: the suites need a site checkout to copy. */
export const needsSite = HAVE_SITE ? {} : { skip: `no site checkout at ${SITE_SOURCE} (set PIXELLAB_TEST_SITE)` };

export const SITE_PARTS = ['src/lib/pixel-art', 'scripts/pixel-art-preview.mjs', 'src/styles/variables.css', 'src/assets/pixel-art/source', 'package.json'];

/** A temp folder, removed by cleanup(). */
export async function tempDir(prefix = 'pixellab-test-') {
  return mkdtemp(join(tmpdir(), prefix));
}

/**
 * A copy of the site's needed parts in a temp folder, with an empty HOME
 * next to it.
 * @param {Record<string, string>} [files]  extra or replaced files, by path under the site root
 */
export async function makeSite(files = {}) {
  const base = await tempDir();
  const root = join(base, 'site');
  const home = join(base, 'home');
  await mkdir(home, { recursive: true });
  for (const part of SITE_PARTS) {
    await cp(join(SITE_SOURCE, part), join(root, part), { recursive: true });
  }
  const write = async (/** @type {string} */ file, /** @type {string} */ text) => {
    await mkdir(dirname(join(root, file)), { recursive: true });
    await writeFile(join(root, file), text);
  };
  for (const [file, text] of Object.entries(files)) await write(file, text);
  return {
    base,
    root,
    home,
    sourceDir: join(root, 'src/assets/pixel-art/source'),
    write,
    /** An env for the CLI: HOME is the temp home; PIXELLAB_SITE unset. */
    env: (/** @type {Record<string, string>} */ extra = {}) => {
      const env = { ...process.env, HOME: home, ...extra };
      if (!('PIXELLAB_SITE' in extra)) delete env.PIXELLAB_SITE;
      return env;
    },
    cleanup: () => rm(base, { recursive: true, force: true }),
  };
}

/**
 * Runs the real CLI in a child process.
 * @param {string[]} args
 * @param {{ env?: Record<string, string | undefined>, cwd?: string }} [options]
 * @returns {Promise<{ code: number | null, stdout: string, stderr: string }>}
 */
export function spawnCli(args, { env = process.env, cwd = REPO } = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [BIN, ...args], { env, cwd });
    let stdout = '';
    let stderr = '';
    child.stdout.setEncoding('utf8').on('data', (d) => (stdout += d));
    child.stderr.setEncoding('utf8').on('data', (d) => (stderr += d));
    child.on('error', reject);
    child.on('close', (code) => resolve({ code, stdout, stderr }));
  });
}

/**
 * Runs the CLI's main() in this process, capturing its output.
 * @param {string[]} args
 * @param {{ env?: Record<string, string | undefined>, cwd?: string }} [options]
 */
export async function runCli(args, { env = process.env, cwd = REPO } = {}) {
  let stdout = '';
  let stderr = '';
  const code = await main(args, {
    stdout: { write: (s) => (stdout += s) },
    stderr: { write: (s) => (stderr += s) },
    env,
    cwd,
  });
  return { code, stdout, stderr };
}

/**
 * Decodes the PNGs encodePng writes: 8-bit RGBA, no interlace, filter 0.
 * Checks the signature and every chunk's CRC.
 * @param {Buffer} png
 */
export function decodePng(png) {
  const SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  if (!png.subarray(0, 8).equals(SIGNATURE)) throw new Error('bad PNG signature');
  let off = 8;
  /** @type {Buffer[]} */
  const idat = [];
  let header;
  const types = [];
  while (off < png.length) {
    const len = png.readUInt32BE(off);
    const type = png.toString('latin1', off + 4, off + 8);
    types.push(type);
    const data = png.subarray(off + 8, off + 8 + len);
    if (png.readUInt32BE(off + 8 + len) !== crc32(png.subarray(off + 4, off + 8 + len)) >>> 0) throw new Error(`bad CRC in ${type}`);
    if (type === 'IHDR') {
      header = { width: data.readUInt32BE(0), height: data.readUInt32BE(4), bitDepth: data[8], colorType: data[9], interlace: data[12] };
    }
    if (type === 'IDAT') idat.push(data);
    off += 12 + len;
  }
  if (!header) throw new Error('no IHDR');
  const raw = inflateSync(Buffer.concat(idat));
  const stride = header.width * 4 + 1;
  if (raw.length !== stride * header.height) throw new Error(`IDAT has ${raw.length} bytes, expected ${stride * header.height}`);
  const alpha = (/** @type {number} */ x, /** @type {number} */ y) => raw[y * stride + 1 + x * 4 + 3];
  const rgba = (/** @type {number} */ x, /** @type {number} */ y) => [...raw.subarray(y * stride + 1 + x * 4, y * stride + 1 + x * 4 + 4)];
  /** The box around every non-transparent pixel, as [x, y, w, h], or null. */
  const opaqueBox = () => {
    let [minX, minY, maxX, maxY] = [Infinity, Infinity, -Infinity, -Infinity];
    for (let y = 0; y < header.height; y++) {
      for (let x = 0; x < header.width; x++) {
        if (!alpha(x, y)) continue;
        [minX, minY, maxX, maxY] = [Math.min(minX, x), Math.min(minY, y), Math.max(maxX, x), Math.max(maxY, y)];
      }
    }
    return minX === Infinity ? null : [minX, minY, maxX - minX + 1, maxY - minY + 1];
  };
  return { ...header, types, alpha, rgba, opaqueBox };
}
