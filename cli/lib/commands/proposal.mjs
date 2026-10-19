// Proposals and the activity log (docs/plan.md, "Agents propose, people
// approve"): an MCP agent's doc.save writes a proposal to the state dir;
// a person lists, shows (with a diff), applies or rejects it. activity.list
// tails the log of MCP tool calls.

import { join } from 'node:path';

import { createContext, docFile } from '../context.mjs';
import { unifiedDiff } from '../diff.mjs';
import { MAX_SCALE, renderObjectImage, renderSceneImage } from '../render.mjs';
import { CommandError } from '../errors.mjs';
import { docRef, importSource, problemsIn, readSource } from '../session.mjs';
import { writeAtomic } from '../state.mjs';

const ID = { type: 'string', description: 'the proposal id (proposal.list)' };

/** The state of a ctx: a session's, or a clear error. */
function stateOf(/** @type {any} */ ctx) {
  if (!ctx.state) throw new CommandError('no state dir in this ctx', { code: 'no_state' });
  return ctx.state;
}

/**
 * A proposal's doc, checked: "scene:<name>" or "object:<name>" with a
 * kebab-case name, so a hand-made file can't point outside the sources.
 * @param {any} ctx
 * @param {any} proposal
 */
function targetOf(ctx, proposal) {
  const m = /^(scene|object):(.+)$/.exec(String(proposal?.doc));
  if (!m || !ctx.site.engine.isName(m[2]) || typeof proposal.source !== 'string') {
    throw new CommandError(`proposal ${proposal?.id} is malformed: doc must be scene:<name> or object:<name> and source a string`, { code: 'bad_proposal' });
  }
  return { kind: /** @type {'scene' | 'object'} */ (m[1]), name: m[2] };
}

/** Whether the file still is what the proposal was made from. */
const baseMatches = (/** @type {any} */ proposal, /** @type {any} */ disk) => (proposal.baseHash === null ? !disk.exists : disk.exists && disk.hash === proposal.baseHash);

/** @param {any} p @param {boolean} matches */
const listed = (p, matches) => ({ id: p.id, doc: p.doc, summary: p.summary ?? [], createdAt: p.createdAt, agent: p.agent ?? null, baseMatches: matches });

/** @type {import('../registry.mjs').Command[]} */
export default [
  {
    name: 'proposal.list',
    summary: 'List the proposals agents made, oldest first',
    description: 'Each is { id, doc, summary: [edits], createdAt, agent, baseMatches }; baseMatches is false when the file changed since the agent opened it.',
    params: { type: 'object', properties: {} },
    args: [],
    mutates: false,
    async run(ctx) {
      const out = [];
      for (const p of await stateOf(ctx).listProposals()) {
        let matches = false;
        try {
          const { kind, name } = targetOf(ctx, p);
          matches = baseMatches(p, await readSource(ctx.site, kind, name));
        } catch {
          // malformed: listed, never matching
        }
        out.push(listed(p, matches));
      }
      return { proposals: out };
    },
    format: (r) =>
      r.proposals.map((/** @type {any} */ p) => `${p.id}  ${p.doc}  ${p.agent ?? '?'}  ${p.summary.length} edit${p.summary.length === 1 ? '' : 's'}${p.baseMatches ? '' : '  (file changed since)'}`).join('\n') ||
      'no proposals',
  },
  {
    name: 'proposal.show',
    summary: 'Show a proposal: its edits, the source, and a diff from the file as it is now',
    params: { type: 'object', properties: { id: ID }, required: ['id'] },
    args: ['id'],
    mutates: false,
    async run(ctx, { id }) {
      const p = await stateOf(ctx).readProposal(id);
      const { kind, name } = targetOf(ctx, p);
      const disk = await readSource(ctx.site, kind, name);
      const file = docFile(kind, name);
      return {
        ...listed(p, baseMatches(p, disk)),
        baseHash: p.baseHash,
        file,
        exists: disk.exists,
        currentHash: disk.hash,
        source: p.source,
        diff: unifiedDiff(disk.text, p.source, { from: `a/${file}`, to: `b/${file}` }),
      };
    },
    format: (r) =>
      [
        `${r.id}: ${r.doc} by ${r.agent ?? 'an agent'}, ${r.createdAt}`,
        ...r.summary.map((/** @type {string} */ s) => `  - ${s}`),
        r.baseMatches ? '' : `warning: ${r.file} changed since the proposal was made; apply needs --force`,
        r.diff || '(no change from the file as it is)',
      ]
        .filter(Boolean)
        .join('\n'),
  },
  {
    name: 'proposal.render',
    summary: "Render a proposal's doc as it would be once applied, without writing anything",
    description:
      'The proposed source over the files on disk, rendered as scene.render or object.render renders the doc (same params, same result shape), ' +
      'plus { id, doc, kind, baseMatches }. The Agent window draws it over the stage as a ghost.',
    params: {
      type: 'object',
      properties: {
        id: ID,
        scale: { type: 'integer', minimum: 1, maximum: MAX_SCALE, default: 1, description: 'whole-pixel zoom of the PNG' },
        time: { type: 'integer', minimum: 0, description: 'ms into the motion and loops (a scene: default 0; an object: overrides frame)' },
        frame: { type: 'integer', minimum: 0, default: 0, description: "an object's frame" },
        layers: { type: 'array', items: { type: 'integer', minimum: 0 }, description: "only these layers of a sprite, as object.render's --layers" },
      },
      required: ['id'],
    },
    args: ['id'],
    mutates: false,
    writesPng: true,
    examples: ['pixellab proposal render 20261019-142301-a1b2c3 --scale 2 --out /tmp/proposal.png'],
    async run(ctx, { id, scale, time, frame, layers }) {
      const p = await stateOf(ctx).readProposal(id);
      const { kind, name } = targetOf(ctx, p);
      let data;
      try {
        data = await importSource(p.source);
      } catch (err) {
        throw new CommandError(`proposal ${id}: its source can't be loaded: ${/** @type {Error} */ (err).message.split('\n')[0]}`, { code: 'invalid' });
      }
      // As apply would leave the site: the proposed doc over the files on disk.
      const view = createContext(ctx.site, { overlay: { get: (k, n) => (k === kind && n === name ? data : undefined), names: (k) => (k === kind ? [name] : []) } });
      const { sources } = await view.sources();
      const disk = await readSource(ctx.site, kind, name);
      const image =
        kind === 'scene'
          ? renderSceneImage(ctx.site, sources, name, { scale, time: time ?? 0 })
          : renderObjectImage(ctx.site, sources, name, { scale, frame, time, layers });
      return { id, doc: p.doc, kind, baseMatches: baseMatches(p, disk), ...image };
    },
    format: (r) => `${r.id}: ${r.doc} as proposed, ${r.width}×${r.height} PNG at ${r.scale}×, ${r.items.length} item${r.items.length === 1 ? '' : 's'}`,
  },
  {
    name: 'proposal.apply',
    summary: "Apply a proposal: check its base, validate it, write the doc's source and remove the proposal",
    description:
      'Refuses (code conflict) if the file changed since the proposal was made, and (code invalid) if the proposed doc has validation problems, unless --force. ' +
      'An open, unchanged working copy of the doc reloads from the new file.',
    params: { type: 'object', properties: { id: ID, force: { type: 'boolean', default: false, description: 'apply despite a changed base or problems' } }, required: ['id'] },
    args: ['id'],
    mutates: true,
    writes: 'direct',
    async run(ctx, { id, force }) {
      const state = stateOf(ctx);
      const p = await state.readProposal(id);
      const { kind, name } = targetOf(ctx, p);
      const file = docFile(kind, name);
      const disk = await readSource(ctx.site, kind, name);
      if (!baseMatches(p, disk) && !force) {
        throw new CommandError(`${file} changed since proposal ${id} was made; nothing written. Show it to compare, then reject it or force the apply.`, { code: 'conflict' });
      }
      let data;
      try {
        data = await importSource(p.source);
      } catch (err) {
        throw new CommandError(`proposal ${id}: its source can't be loaded: ${/** @type {Error} */ (err).message.split('\n')[0]}`, { code: 'invalid' });
      }
      // Validated against the files on disk, not anyone's unsaved working copies.
      const check = createContext(ctx.site, { overlay: { get: (k, n) => (k === kind && n === name ? data : undefined), names: (k) => (k === kind ? [name] : []) } });
      const problems = await problemsIn(check, file);
      if (problems.length && !force) {
        throw new CommandError(`proposal ${id} has ${problems.length} problem${problems.length === 1 ? '' : 's'}; nothing written:\n${problems.join('\n')}`, { code: 'invalid', details: problems });
      }
      await writeAtomic(join(ctx.site.sourceDir, file), p.source);
      await state.removeProposal(id);
      const reloaded = ctx.session ? await ctx.session.reloadIfClean(docRef(kind, name)) : false;
      ctx.invalidate?.();
      return {
        id,
        doc: p.doc,
        file,
        wrote: true,
        reloaded,
        diff: unifiedDiff(disk.text, p.source, { from: `a/${file}`, to: `b/${file}` }),
        ...(problems.length ? { forcedProblems: problems } : {}),
        summary: `Applied ${id}: wrote ${file}`,
      };
    },
    format: (r) => r.summary,
  },
  {
    name: 'proposal.reject',
    summary: 'Reject a proposal: delete it, leaving the site as it is',
    params: { type: 'object', properties: { id: ID }, required: ['id'] },
    args: ['id'],
    mutates: true,
    writes: 'direct',
    async run(ctx, { id }) {
      const state = stateOf(ctx);
      const p = await state.readProposal(id);
      await state.removeProposal(id);
      return { id, doc: p.doc ?? null, rejected: true, summary: `Rejected ${id}` };
    },
    format: (r) => r.summary,
  },
  {
    name: 'activity.list',
    summary: 'The last MCP tool calls agents made, oldest first',
    description: 'Each entry is { time, tool, args, ok, summary }, from activity.jsonl in the state dir.',
    params: { type: 'object', properties: { limit: { type: 'integer', minimum: 1, maximum: 1000, default: 50, description: 'how many entries' } } },
    args: [],
    mutates: false,
    async run(ctx, { limit }) {
      return { entries: await stateOf(ctx).tailActivity(limit) };
    },
    format: (r) => r.entries.map((/** @type {any} */ e) => `${e.time}  ${e.ok ? 'ok ' : 'err'}  ${e.tool}  ${e.summary ?? ''}`).join('\n') || 'no activity',
  },
];
