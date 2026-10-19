// What a command runs against. Commands never read the disk themselves: they
// ask the ctx for documents and engine sources, so a later session (slice 2:
// open documents held in memory, with undo and redo) can hand them its
// working copies by passing an `overlay`, without the commands changing.
//
//   ctx.site               the Site from site.mjs (root, engine modules, preview helpers)
//   ctx.sources()          { sources, loadErrors }: the engine's Sources for every doc,
//                          the overlay's working copies in place of what's on disk
//   ctx.doc(kind, name)    one document's data ('palette' | 'object' | 'scene'),
//                          from the overlay if it holds it, else from disk
//   ctx.names(kind)        the names of every object or scene
//   ctx.find(query, kind?) resolves a name, or objects/<name> or scenes/<name>,
//                          to { kind, name, file }
//
// Loading is memoized per ctx, so a door makes one ctx per call;
// ctx.invalidate() drops the merged sources after an edit. A session's ctx
// (session.mjs) adds ctx.session, ctx.state and ctx.edit for the mutating
// commands.

import { NotFoundError, CommandError } from './errors.mjs';

/**
 * @typedef {'palette' | 'object' | 'scene'} DocKind
 * @typedef {{
 *   get: (kind: DocKind, name: string) => any,
 *   names?: (kind: 'object' | 'scene') => string[],
 * }} Overlay  working copies; get returns undefined for a doc it doesn't hold, null for a deleted one
 */

const FILES = {
  palette: () => 'palette.mjs',
  object: (/** @type {string} */ name) => `objects/${name}.mjs`,
  scene: (/** @type {string} */ name) => `scenes/${name}.mjs`,
};

export const docFile = (/** @type {DocKind} */ kind, /** @type {string} */ name) => FILES[kind](name);

/**
 * @param {import('./site.mjs').Site} site
 * @param {{ overlay?: Overlay }} [options]
 */
export function createContext(site, { overlay } = {}) {
  /** @type {Promise<import('./site.mjs').Docs> | undefined} */
  let docsPending;
  /** @type {Promise<{ sources: any, loadErrors: string[] }> | undefined} */
  let sourcesPending;

  const disk = () => (docsPending ??= site.loadDocs());

  /** Disk docs with the overlay applied, as plain records. */
  const merged = async () => {
    const docs = await disk();
    const pick = (/** @type {'object' | 'scene'} */ kind, /** @type {Map<string, any>} */ map) => {
      /** @type {Record<string, any>} */
      const out = {};
      const names = new Set([...map.keys(), ...(overlay?.names?.(kind) ?? [])]);
      for (const name of names) {
        const over = overlay?.get(kind, name);
        if (over === null) continue;
        const doc = over ?? map.get(name);
        if (doc !== undefined) out[name] = doc;
      }
      return out;
    };
    const palette = overlay?.get('palette', 'palette') ?? docs.palette;
    return { palette, objects: pick('object', docs.objects), scenes: pick('scene', docs.scenes), loadErrors: docs.loadErrors, files: docs.files };
  };

  const ctx = {
    site,

    /** Forgets the merged sources, so the next read sees the overlay's latest working copies. */
    invalidate() {
      sourcesPending = undefined;
    },

    /** @returns {Promise<{ sources: any, loadErrors: string[] }>} */
    sources() {
      sourcesPending ??= merged().then(({ palette, objects, scenes, loadErrors }) => ({
        sources: site.engine.loadSources({ palette, objects, scenes }),
        loadErrors,
      }));
      return sourcesPending;
    },

    /**
     * One document's data. Throws NotFoundError if there's no such doc.
     * @param {DocKind} kind
     * @param {string} [name]
     */
    async doc(kind, name = 'palette') {
      const { sources } = await ctx.sources();
      const doc = kind === 'palette' ? sources.palette : (kind === 'object' ? sources.objects : sources.scenes).get(name);
      if (doc === undefined) {
        const loadError = await ctx.loadError(kind, name);
        if (loadError) throw new CommandError(loadError, { code: 'load_error' });
        throw new NotFoundError(kind === 'palette' ? 'palette.mjs is missing' : `no ${kind} named ${name}`);
      }
      return doc;
    },

    /** @param {'object' | 'scene'} kind @returns {Promise<string[]>} */
    async names(kind) {
      const { sources } = await ctx.sources();
      return [...(kind === 'object' ? sources.objects : sources.scenes).keys()];
    },

    /** The load error of a doc's file, if it exists but didn't import. */
    async loadError(/** @type {DocKind} */ kind, /** @type {string} */ name) {
      const { loadErrors } = await ctx.sources();
      const prefix = `${docFile(kind, name)}: cannot be loaded: `;
      return loadErrors.find((e) => e.startsWith(prefix));
    },

    /** Files on disk, with their mtimes (for site.info and, later, save conflicts). */
    async files() {
      return (await disk()).files;
    },

    /**
     * Finds an object or scene by name, or by objects/<name> or scenes/<name>
     * (a trailing .mjs is fine). A file that exists but didn't load is found
     * too, so its load error is what gets reported.
     * @param {string} query
     * @param {'object' | 'scene'} [kind]
     * @returns {Promise<{ kind: 'object' | 'scene', name: string, file: string }>}
     */
    async find(query, kind) {
      const m = /^(objects|scenes)\/(.+?)(?:\.mjs)?$/.exec(query);
      const name = m ? m[2] : query.replace(/\.mjs$/, '');
      const asked = m ? (m[1] === 'objects' ? 'object' : 'scene') : kind;
      if (m && kind && asked !== kind) throw new NotFoundError(`${query} is not a ${kind}`);
      const { sources } = await ctx.sources();
      const has = async (/** @type {'object' | 'scene'} */ k) =>
        (k === 'object' ? sources.objects : sources.scenes).has(name) || Boolean(await ctx.loadError(k, name));
      const isObject = (!asked || asked === 'object') && (await has('object'));
      const isScene = (!asked || asked === 'scene') && (await has('scene'));
      if (isObject && isScene) {
        throw new CommandError(`${name} is both an object and a scene; say objects/${name} or scenes/${name}`, { code: 'ambiguous' });
      }
      if (!isObject && !isScene) throw new NotFoundError(`no ${asked ?? 'object or scene'} named ${query}`);
      const found = isObject ? 'object' : 'scene';
      return { kind: found, name, file: docFile(found, name) };
    },
  };
  return ctx;
}

/** @typedef {ReturnType<typeof createContext>} Context */
