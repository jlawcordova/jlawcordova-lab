// Session commands: doc.open, doc.status, doc.undo, doc.redo, doc.save,
// doc.revert, doc.reload, doc.merge, doc.close. They work on the session's
// working copies (session.mjs), so they run in `pixellab serve` (stdio or
// MCP), not as one-shot command lines (`session: true`).

import { unsavedInApp } from '../open-docs.mjs';
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
      'With a doc: { doc, dirty, canUndo, canRedo, file, exists, stale, undo: [edits], redo: [edits], edits, proposal?, synced?, unsavedInApp }. ' +
      'stale: the file changed on disk since the doc was opened or last matched it. edits: the edits since the doc last matched its file (what a proposal lists). ' +
      'synced: how the doc was brought in line with disk after its proposal was applied or rejected ({ proposal, outcome, action }). ' +
      'unsavedInApp: Pixel Lab has this doc open with unsaved edits, so a proposal for it will clash with them. ' +
      'Without a doc: { docs: [{ doc, dirty, canUndo, canRedo }], unsavedInApp: [docs Pixel Lab has open with unsaved edits], settled: [proposals settled lately] }.',
    params: { type: 'object', properties: { doc: DOC } },
    args: ['doc'],
    mutates: false,
    session: true,
    async run(ctx, { doc }) {
      const session = sessionOf(ctx);
      const inApp = await unsavedInApp(session.state.dir, ctx.site.root);
      if (doc === undefined) return { docs: session.list(), unsavedInApp: inApp, settled: session.settled };
      const { entry } = await entryOf(ctx, doc);
      return { ...(await session.details(entry)), unsavedInApp: inApp.includes(entry.ref) };
    },
    format: (r) =>
      r.docs
        ? [r.docs.map(statusLine).join('\n') || 'no docs open', r.unsavedInApp.length ? `unsaved in Pixel Lab: ${r.unsavedInApp.join(', ')}` : ''].filter(Boolean).join('\n')
        : `${statusLine(r)}${r.stale ? ', changed on disk' : ''}${r.unsavedInApp ? ', unsaved in Pixel Lab' : ''}`,
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
      'Returns { doc, file, wrote, hash, diff, dirty, canUndo, canRedo }, or { doc, proposal, file, summary, edits, diff, warnings?, … } for a proposal: ' +
      'edits lists the edits since the doc last matched its file, and warnings says when Pixel Lab has the doc open with unsaved edits.',
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
    description: 'A doc that was never saved (made by scene.new or object.new) is discarded. To reload without dropping anything, use doc.reload.',
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
    name: 'doc.reload',
    summary: 'Load an open doc from disk again, only if no unsaved work is lost',
    description:
      'Reloads when the working copy has no edits its file lacks: it is clean, or it matches the file (as after its proposal was applied). ' +
      "Refuses with code dirty (details: the edits that would be lost) when there are unsaved edits, and with code proposed while its proposal waits for the owner. " +
      'Returns { doc, reloaded, changed, summary, dirty, canUndo, canRedo }; reloaded is false when the doc already matched the file. ' +
      'A scene whose file is not in depth order (a hand edit) is sorted as a step of its own: the result has sorted (how many items moved), the doc is then unsaved, ' +
      'and one undo gives back exactly the file. doc.revert is the one that drops edits.',
    params: docParams(),
    args: ['doc'],
    mutates: true,
    session: true,
    async run(ctx, { doc }) {
      const { session, ref } = await entryOf(ctx, doc);
      return session.reload(ctx, ref);
    },
    format: (r) => r.summary,
  },
  {
    name: 'doc.merge',
    summary: "Merge the changes to a doc's file on disk into its working copy, keeping the unsaved edits",
    description:
      'A three-way line merge: the file as the doc was opened or last saved, the file now, and the working copy. One undo step; the doc stays dirty with its own edits. ' +
      'A doc with no unsaved edits just reloads. When both sides changed the same lines it refuses with code merge_conflict ' +
      '(details: [{ base: [from, to], ours: [lines], theirs: [lines] }]) and changes nothing. The merged scene is put in depth order, in the same step ' +
      '(a line merge cannot judge overlaps): sorted says how many items moved. Returns { doc, merged, changed, summary, dirty, canUndo, canRedo, sorted? }.',
    params: docParams(),
    args: ['doc'],
    mutates: true,
    session: true,
    async run(ctx, { doc }) {
      const { session, ref } = await entryOf(ctx, doc);
      return session.merge(ctx, ref);
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
