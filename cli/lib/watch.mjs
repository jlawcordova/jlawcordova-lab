// File watching for the stdio server: the art sources (another editor, git,
// an approved proposal) and the state dir (proposals, the activity log),
// debounced, so the app hears about changes it didn't make.

import { watch } from 'node:fs';
import { mkdir, open, stat } from 'node:fs/promises';

import { parseActivity } from './state.mjs';

export const DEBOUNCE_MS = 150;

/**
 * Calls `onChange(files)` with the source files (palette.mjs, objects/x.mjs,
 * scenes/x.mjs) that changed, at most once per DEBOUNCE_MS.
 * @param {string} dir
 * @param {(files: string[]) => void} onChange
 * @param {(text: string) => void} [log]
 */
export function watchSources(dir, onChange, log = () => {}) {
  /** @type {Set<string>} */
  const pending = new Set();
  /** @type {NodeJS.Timeout | undefined} */
  let timer;
  let watcher;
  try {
    watcher = watch(dir, { recursive: true }, (_event, filename) => {
      const file = String(filename ?? '').split('\\').join('/');
      if (!/^(palette|objects\/[^/]+|scenes\/[^/]+)\.mjs$/.test(file)) return;
      pending.add(file);
      clearTimeout(timer);
      timer = setTimeout(() => {
        const files = [...pending].sort();
        pending.clear();
        onChange(files);
      }, DEBOUNCE_MS);
    });
    watcher.on('error', (err) => log(`watching ${dir} failed: ${err.message}`));
  } catch (err) {
    log(`can't watch ${dir}: ${/** @type {Error} */ (err).message}`);
  }
  return () => {
    clearTimeout(timer);
    watcher?.close();
  };
}

/**
 * Watches the state dir: `onProposals()` when a proposal file comes, changes
 * or goes; `onActivity(entries)` with the entries appended to activity.jsonl.
 * @param {import('./state.mjs').State} state
 * @param {{ onProposals: () => void, onActivity: (entries: any[]) => void, log?: (text: string) => void }} handlers
 */
export async function watchState(state, { onProposals, onActivity, log = () => {} }) {
  await mkdir(state.proposalsDir, { recursive: true });
  let offset = 0;
  try {
    offset = (await stat(state.activityFile)).size;
  } catch {
    // no log yet
  }
  /** @type {NodeJS.Timeout | undefined} */
  let proposalsTimer;
  /** @type {NodeJS.Timeout | undefined} */
  let activityTimer;
  let reading = Promise.resolve();
  const readActivity = async () => {
    let size = 0;
    try {
      size = (await stat(state.activityFile)).size;
    } catch {
      return;
    }
    if (size < offset) offset = 0; // truncated or replaced
    if (size === offset) return;
    const fh = await open(state.activityFile, 'r');
    try {
      const buf = Buffer.alloc(size - offset);
      await fh.read(buf, 0, buf.length, offset);
      const text = buf.toString('utf8');
      // Only whole lines; a partial last line waits for the next event.
      const end = text.lastIndexOf('\n') + 1;
      if (end === 0) return;
      offset += Buffer.byteLength(text.slice(0, end));
      const entries = parseActivity(text.slice(0, end));
      if (entries.length) onActivity(entries);
    } finally {
      await fh.close();
    }
  };
  let watcher;
  try {
    watcher = watch(state.dir, { recursive: true }, (_event, filename) => {
      const file = String(filename ?? '').split('\\').join('/');
      if (file.startsWith('proposals')) {
        if (file !== 'proposals' && !file.endsWith('.json')) return;
        clearTimeout(proposalsTimer);
        proposalsTimer = setTimeout(onProposals, DEBOUNCE_MS);
      } else if (file === 'activity.jsonl') {
        clearTimeout(activityTimer);
        activityTimer = setTimeout(() => {
          reading = reading.then(readActivity).catch((err) => log(`reading ${state.activityFile} failed: ${err.message}`));
        }, DEBOUNCE_MS);
      }
    });
    watcher.on('error', (err) => log(`watching ${state.dir} failed: ${err.message}`));
  } catch (err) {
    log(`can't watch ${state.dir}: ${/** @type {Error} */ (err).message}`);
  }
  return () => {
    clearTimeout(proposalsTimer);
    clearTimeout(activityTimer);
    watcher?.close();
  };
}
