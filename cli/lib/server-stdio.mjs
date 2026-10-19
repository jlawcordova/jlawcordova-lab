// `pixellab serve --stdio`: the Mac app's server (docs/cli.md, "The stdio
// protocol"). Newline-delimited JSON-RPC 2.0 on stdin and stdout. Methods
// are the registry's command names, plus session.info and shutdown; a
// command's result is the response's result. The server holds one session,
// so edits are instant and stay in memory until doc.save writes them.
//
// Notifications it sends:
//   doc.changed        { doc, dirty, canUndo, canRedo }  after each edit, undo, redo, save, revert or reload
//   doc.stale          { doc, file }                     an open doc with unsaved edits whose file changed on disk
//   files.changed      { files: ['scenes/x.mjs', …] }    when sources change on disk (debounced)
//
// It also keeps open/<pid>.json in the state dir current (open-docs.mjs), so
// agents can see which docs the app has open with unsaved edits.
//   proposals.changed  { proposals: [...] }              when the state dir's proposals change (as proposal.list)
//   activity.appended  { entries: [...] }                new lines of activity.jsonl

import { consoleToStderr, METHOD_NOT_FOUND, RpcError, serveLines } from './jsonrpc.mjs';
import { openDocsWriter } from './open-docs.mjs';
import { registry as defaultRegistry } from './registry.mjs';
import { readSource, Session } from './session.mjs';
import { watchSources, watchState } from './watch.mjs';

export const PROTOCOL_VERSION = 1;
export const VERSION = '0.4.0';

/**
 * @param {{
 *   site: any,
 *   env?: Record<string, string | undefined>,
 *   input?: NodeJS.ReadableStream,
 *   output?: NodeJS.WritableStream,
 *   log?: (text: string) => void,
 *   registry?: import('./registry.mjs').Registry,
 *   watch?: boolean,
 * }} options
 */
export async function startStdioServer({ site, env = process.env, input = process.stdin, output = process.stdout, log = (t) => process.stderr.write(`pixellab: ${t}\n`), registry = defaultRegistry, watch = true }) {
  if (output === process.stdout) consoleToStderr();
  /** @type {ReturnType<typeof serveLines>} */
  let rpc;
  /** @type {ReturnType<typeof openDocsWriter> | undefined} */
  let openDocs;
  const session = new Session(site, {
    env,
    mode: 'direct',
    onChange: (status) => {
      rpc?.notify('doc.changed', status);
      openDocs?.update(session.list());
    },
  });
  openDocs = openDocsWriter(session.state.dir, site.root, { log });
  // The app's server notices stale docs itself (the watcher), so proposal.apply can tell it.
  session.onStale = (/** @type {{ doc: string, file: string }} */ stale) => rpc?.notify('doc.stale', stale);
  /** @type {(() => void)[]} */
  const stops = [];
  let shuttingDown = false;

  const call = (/** @type {string} */ name, /** @type {any} */ params = {}) => registry.call(session.context(), name, params);

  const handle = async (/** @type {string} */ method, /** @type {any} */ params) => {
    if (method === 'session.info') {
      return {
        name: 'pixellab',
        version: VERSION,
        protocol: PROTOCOL_VERSION,
        pid: process.pid,
        document: { root: site.root, name: site.name, kind: site.kind, publishDir: site.publishDir, siteRoot: site.siteRoot },
        site: site.root,
        sourceDir: site.sourceDir,
        stateDir: session.state.dir,
        saves: 'direct',
        docs: session.list(),
      };
    }
    if (method === 'shutdown') {
      shuttingDown = true;
      setImmediate(() => {
        for (const stop of stops) stop();
        rpc.stop();
        rpc.close();
      });
      return { ok: true };
    }
    if (!registry.has(method)) throw new RpcError(METHOD_NOT_FOUND, `no method ${method}`, { code: 'not_found' });
    try {
      return await call(method, params);
    } finally {
      // Opening and closing docs changes the list without a doc.changed; written before the response.
      await openDocs.update(session.list());
    }
  };

  rpc = serveLines({ input, output, handle, log });

  if (watch) {
    stops.push(
      watchSources(
        site.sourceDir,
        async (files) => {
          if (shuttingDown) return;
          for (const file of files) {
            const m = /^(objects|scenes)\/(.+)\.mjs$/.exec(file);
            if (!m) continue;
            const ref = `${m[1] === 'objects' ? 'object' : 'scene'}:${m[2]}`;
            if (await session.reloadIfClean(ref).catch(() => false)) continue;
            const entry = session.entries.get(ref);
            if (!entry || !session.status(entry).dirty) continue;
            const disk = await readSource(site, entry.kind, entry.name).catch(() => null);
            if (disk && session.isStale(entry, disk)) rpc.notify('doc.stale', { doc: ref, file });
          }
          rpc.notify('files.changed', { files });
        },
        log,
      ),
    );
    stops.push(
      await watchState(session.state, {
        onProposals: async () => {
          if (shuttingDown) return;
          try {
            const { proposals } = await call('proposal.list');
            rpc.notify('proposals.changed', { proposals });
          } catch (err) {
            log(`listing proposals failed: ${/** @type {Error} */ (err).message}`);
          }
        },
        onActivity: (entries) => {
          if (!shuttingDown) rpc.notify('activity.appended', { entries });
        },
        log,
      }),
    );
  }

  await rpc.done;
  for (const stop of stops) stop();
  await openDocs.remove().catch(() => {});
  return 0;
}
