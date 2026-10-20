// The document (docs/intents/2026-10-pixellab-document/spec.md, D2): a
// `.pixellab` folder holding manifest.json, palette.mjs, objects/*.mjs and
// scenes/*.mjs. Where it comes from, whether it looks like one, what its
// manifest says, and its sources loaded fresh. The engine is this package's
// own (cli/lib/engine/), imported once.
//
// A site checkout is also accepted (--site, PIXELLAB_SITE, the config's
// "site"): the first *.pixellab under src/assets/pixel-art/, or that
// folder's legacy source/ (a document without a manifest, built as
// `npm run art` built it). Sources are imported with `?v=<mtime>-<size>`, so
// an edit on disk is picked up by the next load in the same process.

import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { readdir, stat } from 'node:fs/promises';
import { homedir } from 'node:os';
import { basename, dirname, isAbsolute, join, resolve as resolvePath } from 'node:path';
import { pathToFileURL } from 'node:url';

import * as compile from './engine/compile.mjs';
import * as depth from './engine/depth.mjs';
import * as edit from './engine/edit.mjs';
import * as engine from './engine/engine.mjs';
import * as iso from './engine/iso.mjs';
import * as motion from './engine/motion.mjs';
import * as preview from './engine/preview.mjs';
import * as serialize from './engine/serialize.mjs';
import * as starter from './engine/starter.mjs';
import { DocumentError } from './errors.mjs';

export const FORMAT = 1;
export const EXTENSION = '.pixellab';
export const DEFAULT_SITE = join('~', 'Projects', 'jlawcordova.github.io');
/** Where a site keeps its document, and the legacy sources it had before documents. */
export const SITE_ART_DIR = 'src/assets/pixel-art';
export const LEGACY_SOURCE_DIR = `${SITE_ART_DIR}/source`;
/** The budgets the site's compiler gave two outputs; a legacy source folder keeps them. */
export const SITE_BUDGETS = {
  'hero-island.svg': { raw: 750 * 1024, gzip: 150 * 1024 },
  'range-sprite.svg': { raw: 200 * 1024, gzip: 50 * 1024 },
};
/** The night theme's page color when a manifest names none (the site's dark --color-page). */
export const DEFAULT_THEME = { darkPage: '#1C1712' };

export const ENGINE_MODULES = /** @type {const} */ (['engine', 'iso', 'depth', 'motion', 'serialize', 'edit', 'starter']);
const MODULES = { engine, iso, depth, motion, serialize, edit, starter, preview, compile };

/** The config file: `{"document": "<path>"}` or `{"site": "<checkout>"}`. */
export const configFile = () => join(homedir(), '.config', 'pixellab', 'config.json');

const FROM = {
  flag: { doc: "--document", site: "--site" },
  env: { doc: 'PIXELLAB_DOC', site: 'PIXELLAB_SITE' },
  config: { doc: '~/.config/pixellab/config.json ("document")', site: '~/.config/pixellab/config.json ("site")' },
  default: { doc: 'the default', site: 'the default site' },
};

const HOW = 'Pass --document <path.pixellab> (or --site <checkout>), set PIXELLAB_DOC, or put {"document": "<path>"} in ~/.config/pixellab/config.json.';

/** Expands a leading `~` and makes the path absolute. */
export function expandPath(/** @type {string} */ path, cwd = process.cwd()) {
  const home = path === '~' || path.startsWith('~/') ? join(homedir(), path.slice(1)) : path;
  return resolvePath(cwd, home);
}

function readConfig() {
  const file = configFile();
  if (!existsSync(file)) return {};
  let config;
  try {
    config = JSON.parse(readFileSync(file, 'utf8'));
  } catch (err) {
    throw new DocumentError(`${file} is not valid JSON (${/** @type {Error} */ (err).message}); it should be {"document": "<path to a .pixellab>"}`);
  }
  for (const key of ['document', 'site']) {
    if (config && key in config && !(typeof config[key] === 'string' && config[key])) {
      throw new DocumentError(`${file}: "${key}" must be a path`);
    }
  }
  return config ?? {};
}

/**
 * Where the document comes from: --document, else PIXELLAB_DOC, else the config's
 * "document", else --site, PIXELLAB_SITE, the config's "site", else the
 * default site. Doesn't check the folder; a site path is resolved to its
 * document by checkDocument().
 * @param {{ doc?: string, site?: string, env?: Record<string, string | undefined>, cwd?: string }} [options]
 * @returns {{ root: string, from: 'flag' | 'env' | 'config' | 'default', via: 'doc' | 'site' }}
 */
export function resolveDocumentRoot({ doc, site, env = process.env, cwd = process.cwd() } = {}) {
  if (doc) return { root: expandPath(doc, cwd), from: 'flag', via: 'doc' };
  if (env.PIXELLAB_DOC) return { root: expandPath(env.PIXELLAB_DOC, cwd), from: 'env', via: 'doc' };
  const config = readConfig();
  if (config.document) return { root: expandPath(config.document, cwd), from: 'config', via: 'doc' };
  if (site) return { root: expandPath(site, cwd), from: 'flag', via: 'site' };
  if (env.PIXELLAB_SITE) return { root: expandPath(env.PIXELLAB_SITE, cwd), from: 'env', via: 'site' };
  if (config.site) return { root: expandPath(config.site, cwd), from: 'config', via: 'site' };
  return { root: expandPath(DEFAULT_SITE, cwd), from: 'default', via: 'site' };
}

/** The old name, for callers that still say site. */
export const resolveSiteRoot = resolveDocumentRoot;

const isDir = (/** @type {string} */ path) => {
  try {
    return statSync(path).isDirectory();
  } catch {
    return false;
  }
};

/**
 * The document inside a site checkout: the first `*.pixellab` folder under
 * src/assets/pixel-art/ (by name), else the legacy source/ folder.
 * @param {string} siteRoot
 * @returns {{ root: string, legacy: boolean } | null}
 */
export function findDocumentInSite(siteRoot) {
  const artDir = join(siteRoot, SITE_ART_DIR);
  if (isDir(artDir)) {
    const bundles = readdirSync(artDir).filter((f) => f.endsWith(EXTENSION) && isDir(join(artDir, f))).sort();
    if (bundles.length) return { root: join(artDir, bundles[0]), legacy: false };
  }
  const legacy = join(siteRoot, LEGACY_SOURCE_DIR);
  if (isDir(legacy)) return { root: legacy, legacy: true };
  return null;
}

/**
 * @typedef {{
 *   format: number,
 *   name?: string,
 *   publish?: { dir?: string, budgets?: Record<string, { raw: number, gzip: number }> },
 *   preview?: { card?: string, grid?: string },
 *   theme?: { darkPage?: string },
 * }} Manifest
 */

const KNOWN = { top: ['format', 'name', 'publish', 'preview', 'theme'], publish: ['dir', 'budgets'], preview: ['card', 'grid'], theme: ['darkPage'] };
const HEX = /^#[0-9A-Fa-f]{6}$/;

/**
 * Checks a manifest's shape; throws a DocumentError naming the first problem.
 * @param {any} m
 * @param {string} file
 * @returns {Manifest}
 */
export function checkManifest(m, file = 'manifest.json') {
  const bad = (/** @type {string} */ what) => new DocumentError(`${file}: ${what}`);
  if (!m || typeof m !== 'object' || Array.isArray(m)) throw bad('must be a JSON object');
  if (m.format !== FORMAT) throw bad(`"format" must be ${FORMAT}${m.format === undefined ? ' (missing)' : ` (got ${JSON.stringify(m.format)})`}`);
  const unknown = (/** @type {Record<string, any>} */ obj, /** @type {string[]} */ keys, /** @type {string} */ at) => {
    const extra = Object.keys(obj).filter((k) => !keys.includes(k));
    if (extra.length) throw bad(`unknown key ${at}${JSON.stringify(extra[0])}`);
  };
  unknown(m, KNOWN.top, '');
  if (m.name !== undefined && (typeof m.name !== 'string' || !m.name)) throw bad('"name" must be a non-empty string');
  if (m.publish !== undefined) {
    if (!m.publish || typeof m.publish !== 'object') throw bad('"publish" must be an object');
    unknown(m.publish, KNOWN.publish, 'publish.');
    if (m.publish.dir !== undefined && (typeof m.publish.dir !== 'string' || !m.publish.dir)) throw bad('"publish.dir" must be a path');
    if (m.publish.budgets !== undefined) {
      if (!m.publish.budgets || typeof m.publish.budgets !== 'object') throw bad('"publish.budgets" must be an object of { "<output>.svg": { raw, gzip } }');
      for (const [out, b] of Object.entries(m.publish.budgets)) {
        const ok = b && typeof b === 'object' && Number.isInteger(b.raw) && b.raw > 0 && Number.isInteger(b.gzip) && b.gzip > 0 && Object.keys(b).every((k) => k === 'raw' || k === 'gzip');
        if (!ok) throw bad(`"publish.budgets.${out}" must be { raw: <bytes>, gzip: <bytes> }`);
      }
    }
  }
  if (m.preview !== undefined) {
    if (!m.preview || typeof m.preview !== 'object') throw bad('"preview" must be an object');
    unknown(m.preview, KNOWN.preview, 'preview.');
    try {
      preview.previewColors(m.preview);
    } catch (err) {
      throw bad(/** @type {Error} */ (err).message);
    }
  }
  if (m.theme !== undefined) {
    if (!m.theme || typeof m.theme !== 'object') throw bad('"theme" must be an object');
    unknown(m.theme, KNOWN.theme, 'theme.');
    if (m.theme.darkPage !== undefined && !(typeof m.theme.darkPage === 'string' && HEX.test(m.theme.darkPage))) throw bad('"theme.darkPage" must be a #RRGGBB color');
  }
  return m;
}

/**
 * The colors a legacy site's variables.css gives previews and the night
 * theme, when it has them: what the compiler read before documents.
 * @param {string} siteRoot
 */
function siteColors(siteRoot) {
  try {
    const css = readFileSync(join(siteRoot, 'src/styles/variables.css'), 'utf8');
    const card = /--color-card:\s*(#[0-9A-Fa-f]{6});/.exec(css)?.[1];
    const grid = /--color-grid:\s*(rgba\([^)]*\));/.exec(css)?.[1];
    const at = css.indexOf('[data-theme="dark"]');
    const darkPage = at < 0 ? undefined : /--color-page:\s*(#[0-9a-fA-F]{6})\b/.exec(css.slice(at))?.[1].toUpperCase();
    return { preview: { ...(card ? { card } : {}), ...(grid ? { grid } : {}) }, theme: darkPage ? { darkPage } : {} };
  } catch {
    return { preview: {}, theme: {} };
  }
}

/**
 * Checks that `root` is a document (or, via a site, finds the one in it) and
 * reads its manifest. Throws a DocumentError naming what's wrong and how to
 * name a document.
 * @param {string} root
 * @param {'flag' | 'env' | 'config' | 'default'} [from]
 * @param {'doc' | 'site'} [via]
 * @returns {{ root: string, siteRoot: string | null, legacy: boolean, manifest: Manifest }}
 */
export function checkDocument(root, from = 'flag', via = 'doc') {
  const where = `(from ${FROM[from][via]})`;
  if (!isDir(root)) {
    const expected = via === 'site' ? 'a checkout of the site' : 'a .pixellab document';
    throw new DocumentError(`${via === 'site' ? 'site' : 'document'} not found: ${root} ${where} is not a folder. Expected ${expected}. ${HOW}`);
  }
  let docRoot = root;
  let siteRoot = null;
  let legacy = false;
  if (via === 'site') {
    const found = findDocumentInSite(root);
    if (!found) {
      throw new DocumentError(`${root} ${where} doesn't look like the site checkout: no *.pixellab or source/ under ${SITE_ART_DIR}. ${HOW}`);
    }
    siteRoot = root;
    docRoot = found.root;
    legacy = found.legacy;
  }
  if (!existsSync(join(docRoot, 'palette.mjs'))) {
    throw new DocumentError(`${docRoot} ${where} isn't a Pixel Lab document: it has no palette.mjs. ${HOW}`);
  }
  const manifestFile = join(docRoot, 'manifest.json');
  /** @type {Manifest} */
  let manifest;
  if (existsSync(manifestFile)) {
    let parsed;
    try {
      parsed = JSON.parse(readFileSync(manifestFile, 'utf8'));
    } catch (err) {
      throw new DocumentError(`${manifestFile} is not valid JSON (${/** @type {Error} */ (err).message})`);
    }
    manifest = checkManifest(parsed, manifestFile);
  } else if (legacy && siteRoot) {
    // A site's source/ folder before documents: built as npm run art built it.
    const colors = siteColors(siteRoot);
    manifest = { format: FORMAT, name: basename(siteRoot), publish: { dir: '..', budgets: SITE_BUDGETS }, preview: colors.preview, theme: colors.theme };
  } else {
    manifest = { format: FORMAT };
  }
  return { root: docRoot, siteRoot, legacy, manifest };
}

/**
 * @typedef {{
 *   palette: any,
 *   objects: Map<string, any>,
 *   scenes: Map<string, any>,
 *   loadErrors: string[],
 *   files: Map<string, { path: string, mtimeMs: number, size: number }>,
 * }} Docs
 *
 * @typedef {{
 *   root: string,
 *   from: 'flag' | 'env' | 'config' | 'default',
 *   via: 'doc' | 'site',
 *   kind: 'document' | 'legacy-site',
 *   siteRoot: string | null,
 *   name: string,
 *   sourceDir: string,
 *   manifest: Manifest,
 *   publishDir: string | null,
 *   budgets: Record<string, { raw: number, gzip: number }>,
 *   preview: { card?: string, grid?: string },
 *   theme: { darkPage: string },
 *   engine: typeof engine, iso: typeof iso, depth: typeof depth, motion: typeof motion, serialize: typeof serialize,
 *   edit: typeof edit, starter: typeof starter, preview_: typeof preview, compile: typeof compile,
 *   loadDocs: () => Promise<Docs>,
 * }} Document
 */

/**
 * Finds, checks and opens the document: its manifest and the engine now,
 * its sources on each loadDocs() call.
 * @param {{ doc?: string, site?: string, env?: Record<string, string | undefined>, cwd?: string }} [options]
 * @returns {Promise<any>}  a Document (see the typedef; `preview` is the manifest's colors, the preview module is `png`)
 */
export async function openDocument(options = {}) {
  const { root, from, via } = resolveDocumentRoot(options);
  const checked = checkDocument(root, from, via);
  return documentAt(checked, from, via);
}

/** The old name, for callers that still say site. */
export const openSite = openDocument;

/**
 * A Document from a checked root.
 * @param {ReturnType<typeof checkDocument>} checked
 * @param {'flag' | 'env' | 'config' | 'default'} from
 * @param {'doc' | 'site'} via
 */
function documentAt({ root, siteRoot, legacy, manifest }, from, via) {
  const dir = manifest.publish?.dir;
  const publishDir = dir === undefined ? null : isAbsolute(dir) ? dir : resolvePath(root, dir);
  const name = manifest.name ?? (root.endsWith(EXTENSION) ? basename(root, EXTENSION) : basename(siteRoot ?? dirname(root)));
  return {
    root,
    from,
    via,
    kind: legacy ? 'legacy-site' : 'document',
    siteRoot,
    name,
    sourceDir: root,
    manifest,
    publishDir,
    budgets: manifest.publish?.budgets ?? {},
    /** The manifest's preview colors (strings); `preview` is the preview module. */
    previewColors: manifest.preview ?? {},
    theme: { ...DEFAULT_THEME, ...(manifest.theme ?? {}) },
    ...MODULES,
    loadDocs: () => loadDocs(root),
  };
}

/**
 * Reads palette.mjs, objects/*.mjs and scenes/*.mjs, freshly: each file is
 * imported with its mtime and size in the URL, and its default export is
 * deep-copied, so callers can change what they get without touching the
 * module cache. A file that can't be imported becomes a load error in the
 * engine's "<file>: <message>" form.
 * @param {string} dir
 * @returns {Promise<Docs>}
 */
export async function loadDocs(dir) {
  /** @type {string[]} */
  const loadErrors = [];
  /** @type {Docs['files']} */
  const files = new Map();
  const load = async (/** @type {string} */ file) => {
    const path = join(dir, file);
    try {
      const { mtimeMs, size } = await stat(path);
      files.set(file, { path, mtimeMs, size });
      const mod = await import(`${pathToFileURL(path).href}?v=${mtimeMs}-${size}`);
      return structuredClone(mod.default);
    } catch (err) {
      loadErrors.push(`${file}: cannot be loaded: ${String(/** @type {Error} */ (err).message).split('\n')[0]}`);
      return undefined;
    }
  };
  const folder = async (/** @type {string} */ sub) => {
    /** @type {Map<string, any>} */
    const out = new Map();
    let names = [];
    try {
      names = (await readdir(join(dir, sub))).filter((f) => f.endsWith('.mjs')).sort();
    } catch {
      return out;
    }
    const loaded = await Promise.all(names.map((f) => load(`${sub}/${f}`)));
    names.forEach((f, i) => {
      if (loaded[i] !== undefined) out.set(f.slice(0, -'.mjs'.length), loaded[i]);
    });
    return out;
  };
  const [palette, objects, scenes] = await Promise.all([load('palette.mjs'), folder('objects'), folder('scenes')]);
  loadErrors.sort();
  return { palette, objects, scenes, loadErrors, files };
}
