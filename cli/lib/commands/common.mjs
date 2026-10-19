// Pieces the editing commands share: doc references, the output of a change.

import { CommandError, UsageError } from '../errors.mjs';

/** A doc param: scene:<name>, object:<name>, or a name find() resolves. */
export const DOC = { type: 'string', description: 'the doc: scene:<name>, object:<name>, or a name (scenes/<name>, objects/<name>)' };

/**
 * Resolves a doc reference to { kind, name, ref }.
 * @param {any} ctx
 * @param {string} query
 */
export async function resolveDoc(ctx, query) {
  const m = /^(scene|object):(.+)$/.exec(query);
  const { kind, name } = m ? await ctx.find(m[2], /** @type {'scene' | 'object'} */ (m[1])) : await ctx.find(query);
  return { kind, name, ref: `${kind}:${name}` };
}

/** The session a command needs; the one-shot CLI refuses session-only commands before this. */
export function sessionOf(/** @type {any} */ ctx) {
  if (!ctx.session) throw new CommandError('this command needs a session (pixellab serve --stdio or --mcp)', { code: 'no_session' });
  return ctx.session;
}

/** ctx.edit, or a clear error if a ctx without a session gets a mutating command. */
export function editOf(/** @type {any} */ ctx) {
  if (typeof ctx.edit !== 'function') throw new CommandError('this ctx has no session to edit in', { code: 'no_session' });
  return ctx.edit;
}

/** Human output for a change: its summary, and where the item is now. */
export const formatChange = (/** @type {any} */ r) => r.summary;

/** Refuses when none of `keys` is given. */
export function needOne(/** @type {Record<string, unknown>} */ params, /** @type {string[]} */ keys, /** @type {string} */ command) {
  if (!keys.some((k) => params[k] !== undefined)) throw new UsageError(`${command}: give ${keys.slice(0, -1).map((k) => `--${k}`).join(', ')} or --${keys[keys.length - 1]}`);
}

/** Refuses when more than one of `keys` is given. */
export function atMostOne(/** @type {Record<string, unknown>} */ params, /** @type {string[]} */ keys, /** @type {string} */ command) {
  const given = keys.filter((k) => params[k] !== undefined);
  if (given.length > 1) throw new UsageError(`${command}: give only one of ${given.map((k) => `--${k}`).join(', ')}`);
}
