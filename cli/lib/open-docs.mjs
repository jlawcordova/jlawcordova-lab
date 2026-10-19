// The app's open docs, in the state dir (docs/intents/2026-10-mcp-gaps/spec.md,
// D6), so an agent can see what the owner has open with unsaved edits before
// it proposes against it. The app's server (serve --stdio) writes
//
//   open/<pid>.json   { pid, document: <root>, updatedAt, docs: [{ doc, dirty }] }
//
// whenever its open docs change, and removes it when it stops. Readers skip
// files whose process is gone (a crashed app) and other documents'.

import { readdir, readFile, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';

import { writeAtomic } from './state.mjs';

/** The folder of open-docs files in a state dir. */
export const openDir = (/** @type {string} */ stateDir) => join(stateDir, 'open');

/** Whether a process is running (one we may not signal still counts). */
export function alive(/** @type {number} */ pid) {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (err) {
    return /** @type {NodeJS.ErrnoException} */ (err).code === 'EPERM';
  }
}

/**
 * A writer for this process's file: `update(docs)` writes it when the list
 * changed since the last write; `remove()` deletes it.
 * @param {string} stateDir
 * @param {string} document  the document's root
 * @param {{ pid?: number, log?: (text: string) => void }} [options]
 */
export function openDocsWriter(stateDir, document, { pid = process.pid, log = () => {} } = {}) {
  const file = join(openDir(stateDir), `${pid}.json`);
  let last = '';
  /** @type {Promise<void>} */
  let writing = Promise.resolve();
  return {
    file,
    /** @param {{ doc: string, dirty: boolean }[]} docs */
    update(docs) {
      const list = docs.map(({ doc, dirty }) => ({ doc, dirty })).sort((a, b) => (a.doc < b.doc ? -1 : a.doc > b.doc ? 1 : 0));
      const key = JSON.stringify(list);
      if (key === last) return writing;
      last = key;
      const text = `${JSON.stringify({ pid, document: resolve(document), updatedAt: new Date().toISOString(), docs: list }, null, 2)}\n`;
      writing = writing.then(() => writeAtomic(file, text)).catch((err) => log(`can't write ${file}: ${err.message}`));
      return writing;
    },
    async remove() {
      await writing;
      await rm(file, { force: true });
    },
  };
}

/**
 * The docs open in a running app on this document, with whether each has
 * unsaved edits.
 * @param {string} stateDir
 * @param {string} document
 * @returns {Promise<{ doc: string, dirty: boolean, pid: number }[]>}
 */
export async function appOpenDocs(stateDir, document) {
  let names = [];
  try {
    names = (await readdir(openDir(stateDir))).filter((f) => /^\d+\.json$/.test(f));
  } catch {
    return [];
  }
  const root = resolve(document);
  /** @type {{ doc: string, dirty: boolean, pid: number }[]} */
  const out = [];
  for (const f of names) {
    try {
      const data = JSON.parse(await readFile(join(openDir(stateDir), f), 'utf8'));
      if (data.pid === process.pid || !alive(data.pid) || resolve(String(data.document)) !== root || !Array.isArray(data.docs)) continue;
      for (const d of data.docs) if (typeof d?.doc === 'string') out.push({ doc: d.doc, dirty: d.dirty === true, pid: data.pid });
    } catch {
      // half-written or hand-made: skipped
    }
  }
  return out;
}

/** The refs the app has open with unsaved edits, sorted. */
export async function unsavedInApp(/** @type {string} */ stateDir, /** @type {string} */ document) {
  return [...new Set((await appOpenDocs(stateDir, document)).filter((d) => d.dirty).map((d) => d.doc))].sort();
}
