// Session commands: doc.open, doc.status, doc.undo, doc.redo, doc.save,
// doc.revert, doc.close. They work on the session's working copies
// (session.mjs), so they run in `pixellab serve` (stdio or MCP), not as
// one-shot command lines (`session: true`).

import { DOC, resolveDoc, sessionOf } from './common.mjs';

const docParams = (/** @type {Record<string, any>} */ more = {}) => ({ type: 'object', properties: { doc: DOC, ...more }, required: ['doc'] });

/**
 * The doc's session entry, opening it first if `open` (and it isn't).
 * @param {any} ctx
 * @param {string} query
 * @param {boolean} [open]
 */
async function entryOf(ctx, query, open = false) {
  const session = sessionOf(ctx);
  const { kind, name, ref } = await resolveDoc(ctx, query);
  if (open) await session.open(ctx, kind, name);
  return { session, ref, entry: session.entry(ref) };
}

const statusLine = (/** @type {any} */ s) => `${s.doc}: ${s.dirty ? 'unsaved changes' : 'saved'}${s.canUndo ? ', can undo' : ''}${s.canRedo ? ', can redo' : ''}`;

/** @type {import('../registry.mjs').Command[]} */
export default [
  {
    name: 'doc.open',
    summary: 'Open a scene or object as a working copy, with undo and redo',
    description: 'Edits to an open doc stay in memory until doc.save. Editing a doc that isn\'t open opens it, so this is only needed to start from a clean status.',
    params: docParams(),
    args: ['doc'],
    mutates: false,
    session: true,
    async run(ctx, { doc }) {
      const { session, entry } = await entryOf(ctx, doc, true);
      return session.details(entry);
    },
    format: statusLine,
  },
  {
    name: 'doc.status',
    summary: 'Whether open docs have unsaved changes, and can undo or redo',
    description:
      'With a doc: { doc, dirty, canUndo, canRedo, file, exists, stale, undo: [edits], redo: [edits], proposal? }; stale means the file changed on disk ' +
      'since it was opened. Without: { docs: [{ doc, dirty, canUndo, canRedo }] } for every open doc.',
    params: { type: 'object', properties: { doc: DOC } },
    args: ['doc'],
    mutates: false,
    session: true,
    async run(ctx, { doc }) {
      const session = sessionOf(ctx);
      if (doc === undefined) return { docs: session.list() };
      const { entry } = await entryOf(ctx, doc);
      return session.details(entry);
    },
    format: (r) => (r.docs ? r.docs.map(statusLine).join('\n') || 'no docs open' : statusLine(r)),
  },
  {
    name: 'doc.undo',
    summary: 'Undo the last edit to an open doc',
    params: docParams(),
    args: ['doc'],
    mutates: true,
    session: true,
    async run(ctx, { doc }) {
      const { session, ref } = await entryOf(ctx, doc);
      return session.step(ctx, ref, 'undo');
    },
    format: (r) => r.summary,
  },
  {
    name: 'doc.redo',
    summary: 'Redo the last undone edit to an open doc',
    params: docParams(),
    args: ['doc'],
    mutates: true,
    session: true,
    async run(ctx, { doc }) {
      const { session, ref } = await entryOf(ctx, doc);
      return session.step(ctx, ref, 'redo');
    },
    format: (r) => r.summary,
  },
  {
    name: 'doc.save',
    summary: "Save an open doc: validate, serialize with the engine and write its source (MCP: write a proposal)",
    description:
      'Refuses a doc with validation problems (code invalid, details: the problems) and a file that changed on disk since the doc was opened ' +
      '(code conflict), unless --force. In the MCP server it writes a proposal to the state dir for a person to approve instead (unless started with --direct). ' +
      'Returns { doc, file, wrote, hash, diff, dirty, canUndo, canRedo }, or { doc, proposal, file, summary, diff, … } for a proposal.',
    params: docParams({ force: { type: 'boolean', default: false, description: 'save despite problems or a conflict' } }),
    args: ['doc'],
    mutates: true,
    session: true,
    async run(ctx, { doc, force }) {
      const { session, ref } = await entryOf(ctx, doc);
      return session.save(ctx, ref, { force });
    },
    format: (r) => (r.proposal ? `${r.doc}: proposed as ${r.proposal}` : `${r.doc}: ${r.wrote ? `wrote ${r.file}` : `${r.file} unchanged`}`),
  },
  {
    name: 'doc.revert',
    summary: 'Load an open doc from disk again, dropping its changes (as one undo step)',
    description: 'A doc that was never saved (made by scene.new or object.new) is discarded.',
    params: docParams(),
    args: ['doc'],
    mutates: true,
    session: true,
    async run(ctx, { doc }) {
      const { session, ref } = await entryOf(ctx, doc);
      return session.revert(ctx, ref);
    },
    format: (r) => r.summary,
  },
  {
    name: 'doc.close',
    summary: 'Close an open doc (refused with unsaved changes unless --force)',
    params: docParams({ force: { type: 'boolean', default: false, description: 'drop unsaved changes' } }),
    args: ['doc'],
    mutates: false,
    session: true,
    async run(ctx, { doc, force }) {
      const { session, ref } = await entryOf(ctx, doc);
      const r = session.close(ref, { force });
      ctx.invalidate();
      return r;
    },
    format: (r) => `${r.doc}: closed`,
  },
];
