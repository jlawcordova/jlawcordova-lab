// Test helpers. makeDocument() copies the sample document under
// cli/test/fixtures/ into a temp folder, so the core suites run without a
// site checkout. Tests of the owner's art never touch the real site
// checkout: makeSite() copies the parts the CLI needs (the art sources,
// variables.css and package.json) into a temp folder laid out like a site,
// and points --site at that copy. Both set HOME to a temp folder so no real
// config file is read.

import { spawn } from 'node:child_process';
import { existsSync, readdirSync } from 'node:fs';
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { homedir, tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { crc32, inflateSync } from 'node:zlib';

import { main } from '../lib/cli.mjs';

export const REPO = fileURLToPath(new URL('../..', import.meta.url));
export const BIN = join(REPO, 'cli', 'bin', 'pixellab.mjs');

/** The sample document the core suites run on: two objects, two scenes, a manifest. */
export const FIXTURE_DOC = join(REPO, 'cli', 'test', 'fixtures', 'sample.pixellab');

/** The site checkout the copies are made from (read only). */
export const SITE_SOURCE = process.env.PIXELLAB_TEST_SITE ?? join(homedir(), 'Projects', 'jlawcordova.github.io');
/** The site's art: its document (a *.pixellab under src/assets/pixel-art/) or its legacy source/ folder. */
export const SITE_ART = siteArt(SITE_SOURCE);
export const HAVE_SITE = SITE_ART !== null;
/** node:test's skip option: the suites need a site checkout to copy. */
export const needsSite = HAVE_SITE ? {} : { skip: `no site checkout with art at ${SITE_SOURCE} (set PIXELLAB_TEST_SITE)` };

/** @returns {string | null} the path of the site's art folder, or null */
function siteArt(/** @type {string} */ root) {
  const artDir = join(root, 'src', 'assets', 'pixel-art');
  try {
    const bundle = readdirSync(artDir).filter((f) => f.endsWith('.pixellab')).sort()[0];
    if (bundle && existsSync(join(artDir, bundle, 'palette.mjs'))) return join(artDir, bundle);
  } catch {
    return null;
  }
  return existsSync(join(artDir, 'source', 'palette.mjs')) ? join(artDir, 'source') : null;
}

/** What makeSite() copies besides the art: the theme tokens a legacy site's colors come from, and package.json. */
export const SITE_PARTS = ['src/styles/variables.css', 'package.json'];

/** A temp folder, removed by cleanup(). */
export async function tempDir(prefix = 'pixellab-test-') {
  return mkdtemp(join(tmpdir(), prefix));
}

/** An env for the CLI: HOME is a temp home; PIXELLAB_SITE and PIXELLAB_DOC unset unless given. */
const envFor = (/** @type {string} */ home) => (/** @type {Record<string, string>} */ extra = {}) => {
  const env = { ...process.env, HOME: home, ...extra };
  if (!('PIXELLAB_SITE' in extra)) delete env.PIXELLAB_SITE;
  if (!('PIXELLAB_DOC' in extra)) delete env.PIXELLAB_DOC;
  return env;
};

/**
 * A temp copy of the site, laid out as a site checkout with its art in the
 * legacy src/assets/pixel-art/source/ folder (whatever layout the real
 * checkout has), plus variables.css and package.json, with an empty HOME
 * next to it. Tests point --site at it.
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
  const sourceDir = join(root, 'src/assets/pixel-art/source');
  await cp(/** @type {string} */ (SITE_ART), sourceDir, { recursive: true });
  // A legacy folder has no manifest: the site's budgets and colors come from the checkout.
  await rm(join(sourceDir, 'manifest.json'), { force: true });
  const write = async (/** @type {string} */ file, /** @type {string} */ text) => {
    await mkdir(dirname(join(root, file)), { recursive: true });
    await writeFile(join(root, file), text);
  };
  for (const [file, text] of Object.entries(files)) await write(file, text);
  return {
    base,
    root,
    home,
    sourceDir,
    write,
    env: envFor(home),
    cleanup: () => rm(base, { recursive: true, force: true }),
  };
}

/**
 * A temp copy of a document (the sample fixture by default, or the site's
 * art as a .pixellab with the site's manifest), with an empty HOME next to
 * it. Tests point --document at `root`.
 * @param {{ art?: 'sample' | 'site', files?: Record<string, string>, manifest?: Record<string, unknown> | null }} [options]
 *   files: extra or replaced files, by path under the document; manifest: replaces manifest.json (null removes it)
 */
export async function makeDocument({ art = 'sample', files = {}, manifest } = {}) {
  const base = await tempDir();
  const root = join(base, `${art}.pixellab`);
  const home = join(base, 'home');
  await mkdir(home, { recursive: true });
  await cp(art === 'sample' ? FIXTURE_DOC : /** @type {string} */ (SITE_ART), root, { recursive: true });
  if (art === 'site' && !existsSync(join(root, 'manifest.json'))) {
    const budgets = { 'hero-island.svg': { raw: 768000, gzip: 153600 }, 'range-sprite.svg': { raw: 204800, gzip: 51200 } };
    await writeFile(join(root, 'manifest.json'), `${JSON.stringify({ format: 1, name: 'site', publish: { dir: '../out', budgets } }, null, 2)}\n`);
  }
  if (manifest === null) await rm(join(root, 'manifest.json'), { force: true });
  else if (manifest) await writeFile(join(root, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
  const write = async (/** @type {string} */ file, /** @type {string} */ text) => {
    await mkdir(dirname(join(root, file)), { recursive: true });
    await writeFile(join(root, file), text);
  };
  for (const [file, text] of Object.entries(files)) await write(file, text);
  return { base, root, home, sourceDir: root, write, env: envFor(home), cleanup: () => rm(base, { recursive: true, force: true }) };
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

/**
 * A source file's data, read fresh from disk (as a module, the way the
 * engine reads it).
 * @param {string} path
 */
export async function readDoc(path) {
  const text = await readFile(path, 'utf8');
  const mod = await import(`data:text/javascript;base64,${Buffer.from(text).toString('base64')}`);
  return structuredClone(mod.default);
}

/**
 * Starts `pixellab serve …` as a child process and speaks newline-delimited
 * JSON-RPC to it: request() resolves with the response, notifications are
 * collected, and waitFor() waits for one.
 * @param {string[]} args
 * @param {{ env?: Record<string, string | undefined> }} [options]
 */
export function startServer(args, { env = process.env } = {}) {
  const child = spawn(process.execPath, [BIN, 'serve', ...args], { env, cwd: REPO, stdio: ['pipe', 'pipe', 'pipe'] });
  let buffer = '';
  let stderr = '';
  let nextId = 1;
  /** @type {Map<number, (msg: any) => void>} */
  const pending = new Map();
  /** @type {any[]} */
  const notifications = [];
  /** @type {any[]} */
  const lines = [];
  /** @type {{ test: (n: any) => boolean, resolve: (n: any) => void }[]} */
  const waiters = [];
  child.stdout.setEncoding('utf8').on('data', (chunk) => {
    buffer += chunk;
    let nl;
    while ((nl = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, nl);
      buffer = buffer.slice(nl + 1);
      const msg = JSON.parse(line); // anything but JSON on stdout fails the test
      lines.push(msg);
      if ('id' in msg && pending.has(msg.id)) {
        /** @type {(msg: any) => void} */ (pending.get(msg.id))(msg);
        pending.delete(msg.id);
      } else if (msg.method) {
        notifications.push(msg);
        for (const w of [...waiters]) {
          if (w.test(msg)) {
            waiters.splice(waiters.indexOf(w), 1);
            w.resolve(msg);
          }
        }
      }
    }
  });
  child.stderr.setEncoding('utf8').on('data', (d) => (stderr += d));
  const exited = new Promise((resolve) => child.on('close', (code) => resolve(code)));
  return {
    child,
    lines,
    notifications,
    get stderr() {
      return stderr;
    },
    exited,
    /** Sends a raw line. */
    send(/** @type {string} */ line) {
      child.stdin.write(`${line}\n`);
    },
    /** @returns {Promise<any>} the whole response message */
    request(/** @type {string} */ method, /** @type {any} */ params = {}) {
      const id = nextId++;
      return new Promise((resolve) => {
        pending.set(id, resolve);
        child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
      });
    },
    notify(/** @type {string} */ method, /** @type {any} */ params = {}) {
      child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', method, params })}\n`);
    },
    /** Waits for a notification (one already received counts if `since` covers it). */
    waitFor(/** @type {(n: any) => boolean} */ test, { since = 0, timeout = 5000 } = {}) {
      const seen = notifications.slice(since).find(test);
      if (seen) return Promise.resolve(seen);
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error(`timed out waiting for a notification; got ${JSON.stringify(notifications.slice(since).map((n) => n.method))}`)), timeout);
        waiters.push({ test, resolve: (n) => (clearTimeout(timer), resolve(n)) });
      });
    },
    async close() {
      child.stdin.end();
      return exited;
    },
  };
}
