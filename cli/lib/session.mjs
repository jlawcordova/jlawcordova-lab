// A session: open documents held in memory as working copies, each with its
// own undo and redo stacks, plugged into every command's ctx as the overlay
// (context.mjs). Mutating commands edit through ctx.edit(), which opens the
// doc if it isn't open yet, so every command works the same in a one-shot
// CLI run (a fresh session, saved right after the command), in the app's
// stdio server and in the MCP server. Nothing is written until save().
//
// Undo works as the web lab's does (src/components/lab/lab.ts Lab.edit,
// undo, redo, step): a step is a snapshot of the doc before the edit, at
// most 200 are kept, and an edit that changes nothing is no step. Rapid
// repeats of the same edit (a held arrow key, one paint stroke sent pixel
// by pixel) within COALESCE_MS merge into one step, as the lab merges a
// drag or a stroke by its `group`.
//
// Saving (docs/intents/2026-10-mac-app/plan.md, "The site is the source of truth"): validate with
// the engine and refuse a doc with problems unless forced; serialize with
// the engine; refuse with a conflict if the file changed on disk since the
// doc was opened (by hash) unless forced; write atomically. In 'propose'
// mode (MCP) a save writes a proposal to the state dir instead.

import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

import { createContext, docFile } from './context.mjs';
import { unifiedDiff } from './diff.mjs';
import { CommandError, NotFoundError } from './errors.mjs';
import { createState, newProposalId, stateDir, writeAtomic } from './state.mjs';

export const UNDO_LIMIT = 200;
export const COALESCE_MS = 1000;

/** sha256 of a source's text, hex. */
export const hashText = (/** @type {string} */ text) => createHash('sha256').update(text).digest('hex');

/** "scene:hero-island" ↔ { kind, name }. */
export const docRef = (/** @type {string} */ kind, /** @type {string} */ name) => `${kind}:${name}`;

/**
 * The data a source text exports, imported as a module (so it's read exactly
 * as `npm run art` reads it).
 * @param {string} text
 */
export async function importSource(text) {
  const mod = await import(`data:text/javascript;base64,${Buffer.from(text).toString('base64')}`);
  return structuredClone(mod.default);
}

/**
 * The file on disk now.
 * @param {any} site
 * @param {'object' | 'scene'} kind
 * @param {string} name
 */
export async function readSource(site, kind, name) {
  const file = docFile(kind, name);
  const path = join(site.sourceDir, file);
  try {
    const text = await readFile(path, 'utf8');
    return { file, path, exists: true, text, hash: hashText(text) };
  } catch {
    return { file, path, exists: false, text: '', hash: null };
  }
}

/**
 * The engine's problems for one doc's own file, load errors included.
 * @param {any} ctx
 * @param {string} file
 */
export async function problemsIn(ctx, file) {
  const { sources, loadErrors } = await ctx.sources();
  return [...loadErrors, ...ctx.site.engine.validate(sources)].filter((p) => p.startsWith(`${file}: `));
}

/**
 * @typedef {{ before: string, labels: string[] }} Step
 * @typedef {{
 *   kind: 'object' | 'scene',
 *   name: string,
 *   ref: string,
 *   file: string,
 *   doc: any,
 *   clean: string,
 *   base: { exists: boolean, hash: string | null, text: string },
 *   undo: Step[],
 *   redo: Step[],
 *   last: { key: string, time: number } | null,
 *   proposal?: string,
 * }} Entry
 */

export class Session {
  /**
   * @param {any} site
   * @param {{
   *   mode?: 'direct' | 'propose',
   *   env?: Record<string, string | undefined>,
   *   agent?: string | null,
   *   onChange?: (status: any) => void,
   *   now?: () => number,
   * }} [options]
   */
  constructor(site, { mode = 'direct', env = process.env, agent = null, onChange, now = Date.now } = {}) {
    this.site = site;
    this.mode = mode;
    this.env = env;
    this.agent = agent;
    this.onChange = onChange;
    this.now = now;
    this.state = createState(stateDir(env));
    /** @type {Map<string, Entry>} */
    this.entries = new Map();
    const entries = this.entries;
    this.overlay = {
      get: (/** @type {string} */ kind, /** @type {string} */ name) => entries.get(docRef(kind, name))?.doc,
      names: (/** @type {string} */ kind) => [...entries.values()].filter((e) => e.kind === kind).map((e) => e.name),
    };
  }

  /** A ctx for one command call, with this session's working copies. */
  context() {
    const ctx = /** @type {any} */ (createContext(this.site, { overlay: this.overlay }));
    ctx.session = this;
    ctx.state = this.state;
    /**
     * Edits a doc as one undo step (opening it first if needed).
     * @param {{ kind: 'object' | 'scene', name: string }} target
     * @param {(doc: any) => any} fn  changes doc in place; returns { message, ... }
     * @param {{ key?: string | null }} [options]  key: coalesce rapid repeats with the same key
     */
    ctx.edit = (target, fn, options) => this.edit(ctx, target, fn, options);
    return ctx;
  }

  /** @param {string} ref */
  entry(ref) {
    const entry = this.entries.get(ref);
    if (!entry) throw new NotFoundError(`${ref} is not open in this session (doc.open opens it)`);
    return entry;
  }

  /**
   * Opens a doc as a working copy, or returns it if it's open.
   * @param {any} ctx
   * @param {'object' | 'scene'} kind
   * @param {string} name
   */
  async open(ctx, kind, name) {
    const ref = docRef(kind, name);
    const open = this.entries.get(ref);
    if (open) return open;
    const data = structuredClone(await ctx.doc(kind, name));
    const disk = await readSource(this.site, kind, name);
    /** @type {Entry} */
    const entry = {
      kind,
      name,
      ref,
      file: docFile(kind, name),
      doc: data,
      clean: this.serialize(data),
      base: { exists: disk.exists, hash: disk.hash, text: disk.text },
      undo: [],
      redo: [],
      last: null,
    };
    this.entries.set(ref, entry);
    ctx.invalidate();
    return entry;
  }

  /**
   * Adds a new doc (not on disk yet) as a working copy. It's dirty until saved.
   * @param {any} ctx
   * @param {'object' | 'scene'} kind
   * @param {string} name
   * @param {any} doc
   * @param {string} label
   */
  create(ctx, kind, name, doc, label) {
    const ref = docRef(kind, name);
    /** @type {Entry} */
    const entry = {
      kind,
      name,
      ref,
      file: docFile(kind, name),
      doc: structuredClone(doc),
      clean: '',
      base: { exists: false, hash: null, text: '' },
      undo: [],
      redo: [],
      last: null,
    };
    entry.created = label;
    this.entries.set(ref, entry);
    ctx.invalidate();
    this.changed(entry);
    return entry;
  }

  /** The engine's canonical source text for a doc, or null if it can't be serialized. */
  serialize(/** @type {any} */ doc) {
    try {
      return this.site.serialize.serialize(doc);
    } catch {
      return null;
    }
  }

  /**
   * @param {any} ctx
   * @param {{ kind: 'object' | 'scene', name: string }} target
   * @param {(doc: any) => any} fn
   * @param {{ key?: string | null }} [options]
   */
  async edit(ctx, { kind, name }, fn, { key = null } = {}) {
    const entry = await this.open(ctx, kind, name);
    const before = JSON.stringify(entry.doc);
    let result;
    try {
      result = (await fn(entry.doc)) ?? {};
    } catch (err) {
      entry.doc = JSON.parse(before);
      ctx.invalidate();
      throw err;
    }
    ctx.invalidate();
    const after = JSON.stringify(entry.doc);
    const changed = after !== before;
    const message = result.message ?? 'changed';
    if (changed) {
      const time = this.now();
      const top = entry.undo[entry.undo.length - 1];
      if (key !== null && entry.last?.key === key && time - entry.last.time < COALESCE_MS && top) {
        top.labels.push(message);
      } else {
        entry.undo.push({ before, labels: [message] });
        if (entry.undo.length > UNDO_LIMIT) entry.undo.shift();
      }
      entry.last = key === null ? null : { key, time };
      entry.redo = [];
      this.changed(entry);
    }
    const { message: _m, ...rest } = result;
    return { doc: entry.ref, changed, summary: changed ? message : `nothing changed (${message})`, ...rest };
  }

  /** @param {Entry} entry */
  status(entry) {
    const text = this.serialize(entry.doc);
    return { doc: entry.ref, dirty: text !== entry.clean, canUndo: entry.undo.length > 0, canRedo: entry.redo.length > 0 };
  }

  /** The full status the doc.status command shows. */
  async details(/** @type {Entry} */ entry) {
    const disk = await readSource(this.site, entry.kind, entry.name);
    const stale = entry.base.exists ? disk.hash !== entry.base.hash : disk.exists;
    return {
      ...this.status(entry),
      file: entry.file,
      exists: entry.base.exists,
      stale,
      undo: entry.undo.flatMap((s) => s.labels),
      redo: entry.redo.flatMap((s) => s.labels).reverse(),
      ...(entry.proposal ? { proposal: entry.proposal } : {}),
    };
  }

  /** @param {Entry} entry */
  changed(entry) {
    this.onChange?.(this.status(entry));
  }

  /** @param {any} ctx @param {string} ref @param {'undo' | 'redo'} which */
  step(ctx, ref, which) {
    const entry = this.entry(ref);
    const [from, to] = which === 'undo' ? [entry.undo, entry.redo] : [entry.redo, entry.undo];
    const snap = from.pop();
    if (!snap) return { doc: ref, changed: false, summary: which === 'undo' ? 'Nothing to undo' : 'Nothing to redo', ...this.status(entry) };
    to.push({ before: JSON.stringify(entry.doc), labels: snap.labels });
    entry.doc = JSON.parse(snap.before);
    entry.last = null;
    ctx.invalidate();
    this.changed(entry);
    const what = snap.labels.length === 1 ? snap.labels[0] : `${snap.labels[snap.labels.length - 1]} (${snap.labels.length} edits)`;
    return { doc: ref, changed: true, summary: `${which === 'undo' ? 'Undid' : 'Redid'}: ${what}`, ...this.status(entry) };
  }

  /**
   * Every edit since the doc was opened (or created), in order.
   * @param {Entry} entry
   */
  edits(entry) {
    return [...(entry.created ? [entry.created] : []), ...entry.undo.flatMap((s) => s.labels)];
  }

  /**
   * What saving would write, and whether it may: the new text, the diff
   * from the file as it was opened, the doc's problems and any conflict.
   * @param {any} ctx
   * @param {Entry} entry
   */
  async plan(ctx, entry) {
    const text = this.serialize(entry.doc);
    if (text === null) throw new CommandError(`${entry.ref} can't be serialized; fix its shape first (pixellab check ${entry.name})`, { code: 'invalid' });
    const problems = await problemsIn(ctx, entry.file);
    const disk = await readSource(this.site, entry.kind, entry.name);
    const conflict = entry.base.exists ? (!disk.exists ? 'deleted' : disk.hash !== entry.base.hash ? 'changed' : null) : disk.exists ? 'created' : null;
    return {
      text,
      disk,
      problems,
      conflict,
      diff: unifiedDiff(disk.text, text, { from: `a/${entry.file}`, to: `b/${entry.file}` }),
    };
  }

  /**
   * Saves a doc: writes its file ('direct') or a proposal ('propose').
   * @param {any} ctx
   * @param {string} ref
   * @param {{ force?: boolean }} [options]
   */
  async save(ctx, ref, { force = false } = {}) {
    const entry = this.entry(ref);
    const plan = await this.plan(ctx, entry);
    if (plan.problems.length && !force) {
      const n = plan.problems.length;
      throw new CommandError(`${entry.file} has ${n} problem${n === 1 ? '' : 's'}; nothing written. Fix ${n === 1 ? 'it' : 'them'}, or force the save:\n${plan.problems.join('\n')}`, {
        code: 'invalid',
        details: plan.problems,
      });
    }
    if (this.mode === 'propose') return this.propose(entry, plan);
    if (plan.conflict && !force) {
      const how = { changed: 'changed on disk', deleted: 'was deleted', created: 'was created on disk' }[plan.conflict];
      throw new CommandError(`${entry.file} ${how} since it was opened; nothing written. Revert to load it again, or force the save to overwrite it.`, {
        code: 'conflict',
        details: { conflict: plan.conflict },
      });
    }
    const wrote = plan.text !== plan.disk.text || !plan.disk.exists;
    if (wrote) await writeAtomic(plan.disk.path, plan.text);
    entry.base = { exists: true, hash: hashText(plan.text), text: plan.text };
    entry.clean = plan.text;
    delete entry.created;
    delete entry.proposal;
    this.changed(entry);
    return {
      doc: ref,
      file: entry.file,
      wrote,
      hash: entry.base.hash,
      bytes: Buffer.byteLength(plan.text),
      diff: plan.diff,
      ...(plan.problems.length ? { forcedProblems: plan.problems } : {}),
      ...this.status(entry),
    };
  }

  /**
   * Writes (or rewrites) this doc's proposal in the state dir.
   * @param {Entry} entry
   * @param {Awaited<ReturnType<Session['plan']>>} plan
   */
  async propose(entry, plan) {
    const reuse = entry.proposal && (await this.state.hasProposal(entry.proposal));
    const id = reuse ? /** @type {string} */ (entry.proposal) : newProposalId(new Date(this.now()));
    const proposal = {
      id,
      doc: entry.ref,
      baseHash: entry.base.exists ? entry.base.hash : null,
      source: plan.text,
      summary: this.edits(entry),
      createdAt: new Date(this.now()).toISOString(),
      agent: this.agent,
    };
    await this.state.writeProposal(proposal);
    entry.proposal = id;
    this.changed(entry);
    return {
      doc: entry.ref,
      proposal: id,
      file: join(this.state.proposalsDir, `${id}.json`),
      wrote: false,
      summary: `Proposed ${entry.ref} as ${id} (${proposal.summary.length} edit${proposal.summary.length === 1 ? '' : 's'})`,
      edits: proposal.summary,
      diff: plan.diff,
      ...this.status(entry),
    };
  }

  /**
   * Loads the doc from disk again, as one undo step; a new doc is dropped.
   * @param {any} ctx
   * @param {string} ref
   */
  async revert(ctx, ref) {
    const entry = this.entry(ref);
    const disk = await readSource(this.site, entry.kind, entry.name);
    if (!disk.exists) {
      this.entries.delete(ref);
      ctx.invalidate();
      return { doc: ref, changed: true, summary: `Discarded ${ref}, which isn't on disk`, closed: true, dirty: false, canUndo: false, canRedo: false };
    }
    const data = await importSource(disk.text);
    const before = JSON.stringify(entry.doc);
    entry.doc = data;
    entry.base = { exists: true, hash: disk.hash, text: disk.text };
    entry.clean = this.serialize(data) ?? '';
    if (JSON.stringify(data) !== before) {
      entry.undo.push({ before, labels: [`Reverted ${ref} to the file on disk`] });
      if (entry.undo.length > UNDO_LIMIT) entry.undo.shift();
      entry.redo = [];
    }
    entry.last = null;
    ctx.invalidate();
    this.changed(entry);
    return { doc: ref, changed: JSON.stringify(data) !== before, summary: `Reverted ${ref} to the file on disk`, ...this.status(entry) };
  }

  /**
   * Re-reads a clean doc whose file changed on disk (another editor, git, an
   * approved proposal). A dirty doc is left alone: its save will conflict.
   * Returns true if it reloaded.
   * @param {string} ref
   */
  async reloadIfClean(ref) {
    const entry = this.entries.get(ref);
    if (!entry) return false;
    const disk = await readSource(this.site, entry.kind, entry.name);
    if (!disk.exists || disk.hash === entry.base.hash) return false;
    if (this.status(entry).dirty) return false;
    let data;
    try {
      data = await importSource(disk.text);
    } catch {
      return false;
    }
    entry.doc = data;
    entry.base = { exists: true, hash: disk.hash, text: disk.text };
    entry.clean = this.serialize(data) ?? '';
    entry.undo = [];
    entry.redo = [];
    entry.last = null;
    this.changed(entry);
    return true;
  }

  /** @param {string} ref @param {{ force?: boolean }} [options] */
  close(ref, { force = false } = {}) {
    const entry = this.entry(ref);
    if (this.status(entry).dirty && !force) {
      throw new CommandError(`${ref} has unsaved changes; save or revert it, or force the close to drop them`, { code: 'dirty' });
    }
    this.entries.delete(ref);
    return { doc: ref, closed: true };
  }

  /** Every open doc's status. */
  list() {
    return [...this.entries.values()].map((e) => this.status(e));
  }
}
