// The site checkout: where it is, whether it looks like the site, and the
// engine and art sources loaded from it at runtime (docs/plan.md, "The site is
// the source of truth"). The engine is never copied: it's imported from the
// checkout, so the CLI validates and renders with exactly what `npm run art`
// uses. Sources are imported with `?v=<mtime>-<size>`, so an edit on disk is
// picked up by the next load in the same process.

import { existsSync, readFileSync, statSync } from 'node:fs';
import { readdir, stat } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join, resolve as resolvePath } from 'node:path';
import { pathToFileURL } from 'node:url';

import { SiteError } from './errors.mjs';

export const DEFAULT_SITE = join('~', 'Projects', 'jlawcordova.github.io');
export const SOURCE_DIR = 'src/assets/pixel-art/source';
export const ENGINE_DIR = 'src/lib/pixel-art';
export const PREVIEW_SCRIPT = 'scripts/pixel-art-preview.mjs';
export const ENGINE_MODULES = /** @type {const} */ (['engine', 'iso', 'motion', 'serialize', 'edit', 'starter']);

/** Files that must exist for a folder to count as the site. */
export const REQUIRED_FILES = [
  ...ENGINE_MODULES.map((m) => `${ENGINE_DIR}/${m}.mjs`),
  PREVIEW_SCRIPT,
  `${SOURCE_DIR}/palette.mjs`,
];

/** The config file: `{"site": "<path>"}`. */
export const configFile = () => join(homedir(), '.config', 'pixellab', 'config.json');

const FROM = {
  flag: '--site',
  env: 'PIXELLAB_SITE',
  config: '~/.config/pixellab/config.json',
  default: 'the default',
};

/** Expands a leading `~` and makes the path absolute. */
export function expandPath(/** @type {string} */ path, cwd = process.cwd()) {
  const home = path === '~' || path.startsWith('~/') ? join(homedir(), path.slice(1)) : path;
  return resolvePath(cwd, home);
}

/**
 * Where the site root comes from: --site, else PIXELLAB_SITE, else the config
 * file, else ~/Projects/jlawcordova.github.io. Doesn't check the folder.
 * @param {{ site?: string, env?: Record<string, string | undefined>, cwd?: string }} [options]
 * @returns {{ root: string, from: 'flag' | 'env' | 'config' | 'default' }}
 */
export function resolveSiteRoot({ site, env = process.env, cwd = process.cwd() } = {}) {
  if (site) return { root: expandPath(site, cwd), from: 'flag' };
  if (env.PIXELLAB_SITE) return { root: expandPath(env.PIXELLAB_SITE, cwd), from: 'env' };
  const file = configFile();
  if (existsSync(file)) {
    let config;
    try {
      config = JSON.parse(readFileSync(file, 'utf8'));
    } catch (err) {
      throw new SiteError(`${file} is not valid JSON (${/** @type {Error} */ (err).message}); it should be {"site": "<path to the site checkout>"}`);
    }
    if (config && typeof config.site === 'string' && config.site) return { root: expandPath(config.site, cwd), from: 'config' };
    if (config && 'site' in config) throw new SiteError(`${file}: "site" must be a path to the site checkout`);
  }
  return { root: expandPath(DEFAULT_SITE, cwd), from: 'default' };
}

const HOW = 'Pass --site <path>, set PIXELLAB_SITE, or put {"site": "<path>"} in ~/.config/pixellab/config.json.';

/**
 * Throws a SiteError unless `root` is a folder with the engine, the preview
 * helpers and the art sources.
 * @param {string} root
 * @param {keyof typeof FROM} [from]
 */
export function checkSite(root, from = 'flag') {
  const via = `(from ${FROM[from]})`;
  let isDir = false;
  try {
    isDir = statSync(root).isDirectory();
  } catch {
    // reported below
  }
  if (!isDir) {
    throw new SiteError(`site not found: ${root} ${via} is not a folder. Expected a checkout of jlawcordova.github.io. ${HOW}`);
  }
  const missing = REQUIRED_FILES.filter((f) => !existsSync(join(root, f)));
  if (missing.length) {
    throw new SiteError(
      `${root} ${via} doesn't look like the jlawcordova.github.io checkout: missing ${missing.join(', ')}. ${HOW}`,
    );
  }
}

/** @type {Map<string, Promise<any>>} */
const engines = new Map();

/**
 * The engine modules and the preview helpers, imported once per site root.
 * They're imported without a cache-busting query, so the preview script's own
 * relative import of the engine shares the same module instance.
 * @param {string} root
 */
function importEngine(root) {
  let pending = engines.get(root);
  if (!pending) {
    const load = async () => {
      /** @type {Record<string, any>} */
      const modules = {};
      for (const name of ENGINE_MODULES) modules[name] = await importFile(join(root, ENGINE_DIR, `${name}.mjs`));
      modules.preview = await importFile(join(root, PREVIEW_SCRIPT));
      return modules;
    };
    pending = load();
    engines.set(root, pending);
    pending.catch(() => engines.delete(root));
  }
  return pending;
}

async function importFile(/** @type {string} */ path) {
  try {
    return await import(pathToFileURL(path).href);
  } catch (err) {
    throw new SiteError(`${path} cannot be loaded: ${String(/** @type {Error} */ (err).message).split('\n')[0]}`);
  }
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
 *   sourceDir: string,
 *   engine: any, iso: any, motion: any, serialize: any, edit: any, starter: any, preview: any,
 *   loadDocs: () => Promise<Docs>,
 * }} Site
 */

/**
 * Finds, checks and loads the site: its engine modules now, its sources on
 * each loadDocs() call.
 * @param {{ site?: string, env?: Record<string, string | undefined>, cwd?: string }} [options]
 * @returns {Promise<Site>}
 */
export async function openSite(options = {}) {
  const { root, from } = resolveSiteRoot(options);
  checkSite(root, from);
  const modules = await importEngine(root);
  const sourceDir = join(root, SOURCE_DIR);
  return {
    root,
    from,
    sourceDir,
    engine: modules.engine,
    iso: modules.iso,
    motion: modules.motion,
    serialize: modules.serialize,
    edit: modules.edit,
    starter: modules.starter,
    preview: modules.preview,
    loadDocs: () => loadDocs(sourceDir),
  };
}

/**
 * Reads palette.mjs, objects/*.mjs and scenes/*.mjs, freshly: each file is
 * imported with its mtime and size in the URL, and its default export is
 * deep-copied, so callers can change what they get without touching the
 * module cache. A file that can't be imported becomes a load error in the
 * engine's "<file>: <message>" form, as `npm run art` reports it.
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
