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
//
// Edits since disk (docs/intents/2026-10-mcp-gaps/spec.md, D1): every undo
// step has a sequence number, and an entry's baseSeq is the number when the
// doc last matched its file (open, save, reload, a settled proposal); a
// proposal lists only the steps after it. sync() (D2) settles an MCP
// session's proposals once the owner applies or rejects them, and reloads
// clean docs whose files changed, before each tool call. merge() (D4) brings
// a change on disk into a dirty working copy, three ways.
//
// Depth order (docs/intents/2026-10-depth-order/spec.md, R4): an edit of a
// scene ends by putting every list of its items in depth order, inside the
// same undo step, so the file always reads back to front. The result's paths
// are rewritten to where the sort put the items, and `reordered` says which
// paths moved. A merge sorts what it makes; a reload that finds a file out of
// order (a hand edit) sorts it as a step of its own, so one undo gives the
// file back.

import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

import { createContext, docFile } from './context.mjs';
import { unifiedDiff } from './diff.mjs';
import { CommandError, NotFoundError } from './errors.mjs';
import { layersOf, placedItems } from './layer-ops.mjs';
import { foldScene, merge3 } from './merge.mjs';
import { unsavedInApp } from './open-docs.mjs';
import { pathChanges, sortScene } from './scene-ops.mjs';
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
 * @typedef {{ before: string, labels: string[], seq: number }} Step
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
 *   baseSeq: number,
 *   undone: string[],
 *   created?: string,
 *   proposal?: string,
 *   proposed?: { id: string, text: string, seq: number },
 *   synced?: Settled,
 * }} Entry
 * @typedef {{
 *   doc: string,
 *   proposal: string,
 *   outcome: 'applied' | 'rejected',
 *   action: 'clean' | 'reloaded' | 'rebased' | 'kept' | 'discarded',
 * }} Settled
 */

/** The paths a result names: path, paths, group.path and groups[].path. */
const PATH = /^\d+(\.\d+)*$/;

/**
 * A result with its item and group paths moved to where a sort put them
 * (`remap`: old path → new path). Only the fields that hold paths change.
 * @param {any} result
 * @param {Map<string, string>} remap
 */
function followSort(result, remap) {
  const to = (/** @type {any} */ path) => (typeof path === 'string' ? remap.get(path) ?? path : path);
  const out = { ...result };
  if ('path' in out) out.path = to(out.path);
  if (Array.isArray(out.paths)) out.paths = out.paths.map(to);
  if (out.group && typeof out.group === 'object' && 'path' in out.group) out.group = { ...out.group, path: to(out.group.path) };
  if (Array.isArray(out.groups)) out.groups = out.groups.map((/** @type {any} */ g) => (g && typeof g === 'object' && 'path' in g ? { ...g, path: to(g.path) } : g));
  // scene.layer.ungroup lists the items its layer held by path.
  if (Array.isArray(out.items) && out.items.every((/** @type {any} */ p) => typeof p === 'string' && PATH.test(p))) out.items = out.items.map(to);
  return out;
}

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
    /** Undo steps take their sequence numbers from here. */
    this.seq = 0;
    /** @type {Settled[]} the proposals sync() settled lately, newest last */
    this.settled = [];
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
      baseSeq: this.seq,
      undone: [],
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
      baseSeq: this.seq,
      undone: [],
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
   * Puts a scene's lists of items in depth order, in place (scene-ops
   * sortScene), judging objects as `ctx` has them.
   * @param {any} ctx
   * @param {any} doc
   */
  async sortDoc(ctx, doc) {
    const { sources } = await ctx.sources();
    return sortScene(this.site, sources, doc);
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
    // Sorting is part of the edit: the same undo step holds both.
    if (kind === 'scene') {
      const { remap } = await this.sortDoc(ctx, entry.doc);
      if (remap.size) {
        ctx.invalidate();
        result = { ...followSort(result, remap), reordered: Object.fromEntries(remap) };
      }
    }
    const after = JSON.stringify(entry.doc);
    const changed = after !== before;
    // A message may be a function of the result, so it can name what the sort moved by its path after the sort.
    const message = (typeof result.message === 'function' ? result.message(result) : result.message) ?? 'changed';
    if (changed) {
      const time = this.now();
      const top = entry.undo[entry.undo.length - 1];
      if (key !== null && entry.last?.key === key && time - entry.last.time < COALESCE_MS && top) {
        top.labels.push(message);
      } else {
        entry.undo.push({ before, labels: [message], seq: ++this.seq });
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
    return {
      ...this.status(entry),
      file: entry.file,
      exists: entry.base.exists,
      stale: this.isStale(entry, disk),
      undo: entry.undo.flatMap((s) => s.labels),
      redo: entry.redo.flatMap((s) => s.labels).reverse(),
      edits: this.edits(entry),
      ...(entry.proposal ? { proposal: entry.proposal } : {}),
      ...(entry.synced ? { synced: entry.synced } : {}),
    };
  }

  /** Whether the file changed on disk since the doc's base. */
  isStale(/** @type {Entry} */ entry, /** @type {{ exists: boolean, hash: string | null }} */ disk) {
    return entry.base.exists ? disk.hash !== entry.base.hash : disk.exists;
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
    to.push({ before: JSON.stringify(entry.doc), labels: snap.labels, seq: snap.seq });
    // A step from before the doc last matched its file: undoing it is itself a change from disk.
    if (snap.seq <= entry.baseSeq) {
      if (which === 'undo') entry.undone.push(...snap.labels.map((l) => `Undid: ${l}`));
      else entry.undone.splice(Math.max(0, entry.undone.length - snap.labels.length));
    }
    const left = entry.doc;
    entry.doc = JSON.parse(snap.before);
    entry.last = null;
    ctx.invalidate();
    this.changed(entry);
    // The step swaps the whole doc, so the paths of its items can change (a sorting edit undone): say where they went.
    const reordered = entry.kind === 'scene' ? pathChanges(left, entry.doc) : new Map();
    const what = snap.labels.length === 1 ? snap.labels[0] : `${snap.labels[snap.labels.length - 1]} (${snap.labels.length} edits)`;
    return { doc: ref, changed: true, summary: `${which === 'undo' ? 'Undid' : 'Redid'}: ${what}`, ...this.status(entry), ...(reordered.size ? { reordered: Object.fromEntries(reordered) } : {}) };
  }

  /**
   * The edits since the doc last matched its file, in order: what a
   * proposal lists (spec D1).
   * @param {Entry} entry
   */
  edits(entry) {
    return [...(entry.created ? [entry.created] : []), ...entry.undone, ...entry.undo.filter((s) => s.seq > entry.baseSeq).flatMap((s) => s.labels)];
  }

  /**
   * Marks the doc as matching its file: the next proposal lists only later
   * edits, and no edit coalesces across the line.
   * @param {Entry} entry
   * @param {{ exists: boolean, hash: string | null, text: string }} disk
   * @param {string} clean  the doc's canonical text as of the file
   * @param {number} [seq]  the last step the file holds (default: every step)
   */
  matchDisk(entry, disk, clean, seq = this.seq) {
    entry.base = { exists: disk.exists, hash: disk.hash, text: disk.text };
    entry.clean = clean;
    entry.baseSeq = seq;
    entry.undone = [];
    entry.last = null;
    delete entry.created;
  }

  /** A source text as the engine would write it, or null if it doesn't load. */
  async canonical(/** @type {string} */ text) {
    try {
      return this.serialize(await importSource(text));
    } catch {
      return null;
    }
  }

  /**
   * Replaces the working copy with the file, clearing its history, then
   * sorts it if the file is out of depth order (sortLoaded).
   * @param {Entry} entry
   * @param {{ exists: boolean, hash: string | null, text: string }} disk
   */
  async loadFromDisk(entry, disk) {
    const data = await importSource(disk.text);
    entry.doc = data;
    entry.undo = [];
    entry.redo = [];
    this.matchDisk(entry, disk, this.serialize(data) ?? '');
    return this.sortLoaded(entry);
  }

  /**
   * A scene just adopted from its file is put in depth order if it isn't (a
   * hand edit, a merge by git), as a step of its own: the doc shows as unsaved
   * and one undo gives back exactly the file. Files Pixel Lab writes are
   * sorted, so this rarely finds anything. Returns what moved, or null.
   * @param {Entry} entry
   */
  async sortLoaded(entry) {
    if (entry.kind !== 'scene') return null;
    const before = JSON.stringify(entry.doc);
    const sorted = await this.sortDoc(this.context(), entry.doc);
    if (!sorted.moved.length) return null;
    entry.undo.push({ before, labels: [`Sorted ${entry.name} by depth`], seq: ++this.seq });
    this.changed(entry);
    return sorted;
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
    this.matchDisk(entry, { exists: true, hash: hashText(plan.text), text: plan.text }, plan.text);
    delete entry.proposal;
    delete entry.proposed;
    delete entry.synced;
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
    const inApp = (await unsavedInApp(this.state.dir, this.site.root).catch(() => [])).includes(entry.ref);
    entry.proposal = id;
    entry.proposed = { id, text: plan.text, seq: this.seq };
    delete entry.synced;
    this.changed(entry);
    return {
      doc: entry.ref,
      proposal: id,
      file: join(this.state.proposalsDir, `${id}.json`),
      wrote: false,
      summary: `Proposed ${entry.ref} as ${id} (${proposal.summary.length} edit${proposal.summary.length === 1 ? '' : 's'})`,
      edits: proposal.summary,
      diff: plan.diff,
      ...(inApp ? { warnings: [`${entry.ref} has unsaved edits in Pixel Lab; approving this proposal will merge it into them, and lines both of you changed will clash. Tell the owner.`] } : {}),
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
    if (JSON.stringify(data) !== before) {
      entry.undo.push({ before, labels: [`Reverted ${ref} to the file on disk`], seq: ++this.seq });
      if (entry.undo.length > UNDO_LIMIT) entry.undo.shift();
      entry.redo = [];
    }
    this.matchDisk(entry, disk, this.serialize(data) ?? '');
    delete entry.proposal;
    delete entry.proposed;
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
    try {
      await this.loadFromDisk(entry, disk);
    } catch {
      return false;
    }
    this.changed(entry);
    return true;
  }

  /**
   * Brings every open doc in line with disk where nothing is lost (spec D2):
   * settles the proposals this session saved once they're applied or
   * rejected, reloads clean docs whose file changed, and marks clean a dirty
   * doc that now matches its file. The MCP server runs it before each call.
   */
  async sync() {
    for (const entry of [...this.entries.values()]) {
      try {
        await this.settle(entry);
      } catch {
        // an unreadable file or a broken source: left for doc.status and save to report
      }
    }
  }

  /** @param {Entry} entry */
  async settle(entry) {
    const disk = await readSource(this.site, entry.kind, entry.name);
    const text = this.serialize(entry.doc);
    if (entry.proposal) {
      if (await this.state.hasProposal(entry.proposal)) return;
      const proposed = entry.proposed;
      const id = entry.proposal;
      const outcome = disk.exists && proposed !== undefined && disk.text === proposed.text ? 'applied' : 'rejected';
      delete entry.proposal;
      delete entry.proposed;
      const canon = disk.exists ? await this.canonical(disk.text) : null;
      /** @type {Settled['action']} */
      let action;
      if (canon !== null && text === canon) {
        this.matchDisk(entry, disk, canon);
        action = 'clean';
      } else if (proposed && text === proposed.text) {
        // Nothing since the proposal: the file is all there is to keep.
        if (disk.exists && canon !== null) {
          await this.loadFromDisk(entry, disk);
          action = 'reloaded';
        } else {
          this.entries.delete(entry.ref);
          action = 'discarded';
        }
      } else if (outcome === 'applied' && canon !== null && proposed) {
        // Later edits on top of the applied proposal: they stay, now over the file.
        this.matchDisk(entry, disk, canon, proposed.seq);
        action = 'rebased';
      } else action = 'kept';
      const settled = { doc: entry.ref, proposal: id, outcome, action };
      if (action !== 'discarded') entry.synced = settled;
      this.settled = [...this.settled, settled].slice(-20);
      if (action !== 'discarded') this.changed(entry);
      return;
    }
    if (!disk.exists || !this.isStale(entry, disk)) return;
    if (text === entry.clean) {
      await this.reloadIfClean(entry.ref);
      return;
    }
    const canon = await this.canonical(disk.text);
    if (canon !== null && canon === text) {
      this.matchDisk(entry, disk, canon);
      this.changed(entry);
    }
  }

  /**
   * Loads a doc from disk, but only if no unsaved work is lost (spec D3):
   * it's clean, or it matches the file. Refused while its proposal waits.
   * @param {any} ctx
   * @param {string} ref
   */
  async reload(ctx, ref) {
    const entry = this.entry(ref);
    if (entry.proposal && (await this.state.hasProposal(entry.proposal))) {
      throw new CommandError(
        `${ref}'s edits are in proposal ${entry.proposal}, which is waiting for the owner; reloading would start over without them. Keep editing on top of it (doc.save updates the proposal), or wait until it's approved or rejected.`,
        { code: 'proposed', details: { proposal: entry.proposal } },
      );
    }
    const disk = await readSource(this.site, entry.kind, entry.name);
    const text = this.serialize(entry.doc);
    const dirty = text !== entry.clean;
    const canon = disk.exists ? await this.canonical(disk.text) : null;
    if (dirty && (canon === null || canon !== text)) {
      const edits = this.edits(entry);
      const n = edits.length;
      throw new CommandError(
        `${ref} has ${n || 'some'} unsaved edit${n === 1 ? '' : 's'} the file doesn't have; reloading would drop ${n === 1 ? 'it' : 'them'}. Save ${n === 1 ? 'it' : 'them'} (doc.save), or revert to drop ${n === 1 ? 'it' : 'them'} (doc.revert).`,
        { code: 'dirty', details: edits },
      );
    }
    if (!disk.exists) {
      this.entries.delete(ref);
      ctx.invalidate();
      return { doc: ref, reloaded: false, closed: true, changed: true, summary: `Closed ${ref}: its file isn't on disk any more`, dirty: false, canUndo: false, canRedo: false };
    }
    if (canon === null) throw new CommandError(`${entry.file} on disk doesn't load; fix it first (pixellab check ${entry.name})`, { code: 'load_error' });
    if (!this.isStale(entry, disk) && !dirty) {
      return { doc: ref, reloaded: false, changed: false, summary: `${ref} already matches ${entry.file}`, ...this.status(entry) };
    }
    const before = JSON.stringify(entry.doc);
    const sorted = await this.loadFromDisk(entry, disk);
    delete entry.proposal;
    delete entry.proposed;
    ctx.invalidate();
    this.changed(entry);
    return {
      doc: ref,
      reloaded: true,
      changed: JSON.stringify(entry.doc) !== before,
      summary: `Reloaded ${ref} from ${entry.file}${sorted ? `; it wasn't in depth order, so ${entry.name} was sorted (one undo gives back the file)` : ''}`,
      ...(sorted ? { sorted: sorted.moved.length } : {}),
      ...this.status(entry),
    };
  }

  /**
   * The engine's problems for `data` as this doc (next to the other docs as
   * the session has them) that none of `others` (other versions of it) has.
   * @param {any} ctx
   * @param {Entry} entry
   * @param {any} data
   * @param {any[]} others
   */
  async newProblems(ctx, entry, data, others) {
    const { sources } = await ctx.sources();
    const engine = this.site.engine;
    const problemsOf = (/** @type {any} */ doc) => {
      const objects = Object.fromEntries(sources.objects);
      const scenes = entry.kind === 'scene' ? { [entry.name]: doc } : {};
      if (entry.kind === 'object') objects[entry.name] = doc;
      return engine.validate(engine.loadSources({ palette: sources.palette, objects, scenes })).filter((/** @type {string} */ p) => p.startsWith(`${entry.file}: `));
    };
    const had = new Set(others.flatMap(problemsOf));
    return problemsOf(data).filter((/** @type {string} */ p) => !had.has(p));
  }

  /**
   * Merges the file's changes since the doc's base into its working copy,
   * three ways (spec D4), as one undo step. A clean doc just reloads; a clash
   * changes nothing and throws merge_conflict.
   * @param {any} ctx
   * @param {string} ref
   */
  async merge(ctx, ref) {
    const entry = this.entry(ref);
    const disk = await readSource(this.site, entry.kind, entry.name);
    if (!disk.exists) throw new CommandError(`${entry.file} isn't on disk; there's nothing to merge`, { code: 'not_found' });
    if (!this.isStale(entry, disk)) return { doc: ref, merged: false, changed: false, summary: `${entry.file} hasn't changed since ${ref} was opened`, ...this.status(entry) };
    const theirs = await this.canonical(disk.text);
    if (theirs === null) throw new CommandError(`${entry.file} on disk doesn't load, so it can't be merged; fix it first (pixellab check ${entry.name})`, { code: 'load_error' });
    const ours = this.serialize(entry.doc);
    if (ours === null) throw new CommandError(`${ref} can't be serialized; fix its shape first (pixellab check ${entry.name})`, { code: 'invalid' });
    if (ours === entry.clean) {
      const sorted = await this.loadFromDisk(entry, disk);
      ctx.invalidate();
      this.changed(entry);
      return {
        doc: ref,
        merged: true,
        changed: true,
        summary: `Reloaded ${ref}: it had no unsaved edits${sorted ? `; it wasn't in depth order, so ${entry.name} was sorted (one undo gives back the file)` : ''}`,
        ...(sorted ? { sorted: sorted.moved.length } : {}),
        ...this.status(entry),
      };
    }
    const base = entry.base.exists ? (await this.canonical(entry.base.text)) ?? entry.base.text : '';
    const merged = merge3(base, ours, theirs, { union: entry.kind === 'scene' });
    const { conflicts } = merged;
    if (conflicts.length) {
      const n = conflicts.length;
      throw new CommandError(
        `Both the open copy and ${entry.file} changed the same line${n === 1 ? '' : 's'} (${n} place${n === 1 ? '' : 's'}); nothing merged. Reload the file (dropping the unsaved edits), or keep the open copy and save over the file.`,
        { code: 'merge_conflict', details: conflicts },
      );
    }
    // Both sides' first layers fold into one list (docs/intents/2026-10-scene-layers/spec.md, D1);
    // what can't be folded (the same layer id, or both adding another key) clashes.
    const folded = entry.kind === 'scene' ? foldScene(merged.text) : { text: merged.text, clashes: [] };
    if (folded.clashes.length) {
      throw new CommandError(`Both the open copy and ${entry.file} added ${folded.clashes.join(', ')}; nothing merged. Reload the file (dropping the unsaved edits), or keep the open copy and save over the file.`, {
        code: 'merge_conflict',
        details: folded.clashes,
      });
    }
    const text = folded.text;
    // A layer both sides made under the same id (two ⌘Gs both named Group 1) writes the same
    // line on both sides, so the line merge takes it once: unless both put the same items in it,
    // that would make two layers one. It clashes; the same change made twice still merges.
    const theirsData = await importSource(theirs);
    if (entry.kind === 'scene') {
      const baseIds = new Set(base ? layersOf(await importSource(base)).map((/** @type {any} */ l) => l.id) : []);
      const theirIds = new Set(layersOf(theirsData).map((/** @type {any} */ l) => l.id));
      const members = (/** @type {any} */ doc, /** @type {string} */ id) =>
        placedItems(doc)
          .filter(({ item }) => item.layer === id)
          .map(({ item }) => JSON.stringify(item))
          .sort()
          .join('\n');
      const both = layersOf(entry.doc).map((/** @type {any} */ l) => l.id).filter((/** @type {string} */ id) => theirIds.has(id) && !baseIds.has(id));
      const differ = both.filter((/** @type {string} */ id) => members(entry.doc, id) !== members(theirsData, id));
      if (differ.length) {
        throw new CommandError(
          `Both the open copy and ${entry.file} made layer ${differ.join(', ')}, holding different items; nothing merged. Reload the file (dropping the unsaved edits), or keep the open copy and save over the file.`,
          { code: 'merge_conflict', details: differ.map((id) => `layer ${id}`) },
        );
      }
    }
    let data;
    try {
      data = await importSource(text);
    } catch (err) {
      throw new CommandError(`the merge of ${entry.file} doesn't load (${/** @type {Error} */ (err).message.split('\n')[0]}); nothing merged`, { code: 'merge_invalid' });
    }
    // A merge that loads but breaks the doc (an item naming a layer neither side kept, two layers
    // with one id) is refused: only problems the open copy or the file already had may stay.
    const added = await this.newProblems(ctx, entry, data, [entry.doc, theirsData]);
    if (added.length) {
      throw new CommandError(`the merge of ${entry.file} would add ${added.length} problem${added.length === 1 ? '' : 's'} (${added[0]}); nothing merged`, { code: 'merge_invalid', details: added });
    }
    const before = JSON.stringify(entry.doc);
    // The merge keeps the list in depth order: both sides' lines come together, and a line merge can't judge overlaps.
    const sorted = entry.kind === 'scene' ? await this.sortDoc(ctx, data) : null;
    entry.doc = data;
    // The base is the file now. The edits since the old base stay unsaved;
    // the merge itself isn't one (its step sits at the base, so undoing it is).
    const keep = entry.baseSeq;
    const undone = entry.undone;
    entry.undo.push({ before, labels: [`Merged the changes to ${entry.file} from disk`], seq: keep });
    if (entry.undo.length > UNDO_LIMIT) entry.undo.shift();
    entry.redo = [];
    this.matchDisk(entry, disk, theirs, keep);
    entry.undone = undone;
    ctx.invalidate();
    this.changed(entry);
    return {
      doc: ref,
      merged: true,
      changed: JSON.stringify(data) !== before,
      summary: `Merged the changes to ${entry.file} into ${ref}${sorted?.moved.length ? '; the result was sorted by depth' : ''}`,
      ...(sorted?.moved.length ? { sorted: sorted.moved.length } : {}),
      ...this.status(entry),
    };
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
