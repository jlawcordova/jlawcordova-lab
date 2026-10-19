// The state dir (docs/plan.md, "Agents propose, people approve"): where MCP
// saves become proposals and every MCP tool call is logged, for the app's
// Agent window to show. PIXELLAB_STATE overrides the default,
// ~/Library/Application Support/Pixel Lab/.
//
//   proposals/<id>.json   { id, doc: "scene:<name>" | "object:<name>", baseHash, source, summary, createdAt, agent }
//   activity.jsonl        one { time, tool, args, ok, summary } per line

import { randomBytes } from 'node:crypto';
import { appendFile, mkdir, readdir, readFile, rename, rm, stat, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';

import { CommandError, NotFoundError } from './errors.mjs';

/** The state dir for an environment. */
export function stateDir(env = process.env) {
  if (env.PIXELLAB_STATE) return resolve(env.PIXELLAB_STATE);
  return join(env.HOME ?? homedir(), 'Library', 'Application Support', 'Pixel Lab');
}

/** Writes a file atomically: a temp file next to it, then a rename. */
export async function writeAtomic(/** @type {string} */ path, /** @type {string} */ text) {
  await mkdir(dirname(path), { recursive: true });
  const tmp = join(dirname(path), `.${process.pid}-${randomBytes(4).toString('hex')}.tmp`);
  try {
    await writeFile(tmp, text);
    await rename(tmp, path);
  } catch (err) {
    await rm(tmp, { force: true });
    throw err;
  }
}

const ID = /^[0-9]{8}-[0-9]{6}-[0-9a-f]{6}$/;

/** A sortable, file-safe proposal id: 20261019-142301-a1b2c3 (UTC). */
export function newProposalId(now = new Date()) {
  const stamp = now.toISOString().replace(/[-:]/g, '').replace('T', '-').slice(0, 15);
  return `${stamp}-${randomBytes(3).toString('hex')}`;
}

/**
 * @typedef {{
 *   id: string,
 *   doc: string,
 *   baseHash: string | null,
 *   source: string,
 *   summary: string[],
 *   createdAt: string,
 *   agent: string | null,
 * }} Proposal
 */

/** Proposals and activity in one state dir. */
export function createState(/** @type {string} */ dir) {
  const proposalsDir = join(dir, 'proposals');
  const activityFile = join(dir, 'activity.jsonl');
  const proposalFile = (/** @type {string} */ id) => {
    if (!ID.test(id)) throw new NotFoundError(`no proposal ${id}; ids look like 20261019-142301-a1b2c3 (pixellab proposal list)`);
    return join(proposalsDir, `${id}.json`);
  };

  return {
    dir,
    proposalsDir,
    activityFile,

    /** @param {Proposal} proposal */
    async writeProposal(proposal) {
      await writeAtomic(proposalFile(proposal.id), `${JSON.stringify(proposal, null, 2)}\n`);
      return proposal;
    },

    /** @returns {Promise<Proposal>} */
    async readProposal(/** @type {string} */ id) {
      let text;
      try {
        text = await readFile(proposalFile(id), 'utf8');
      } catch {
        throw new NotFoundError(`no proposal ${id} (pixellab proposal list)`);
      }
      try {
        return JSON.parse(text);
      } catch (err) {
        throw new CommandError(`proposal ${id} is not valid JSON: ${/** @type {Error} */ (err).message}`, { code: 'bad_proposal' });
      }
    },

    async hasProposal(/** @type {string} */ id) {
      try {
        await stat(proposalFile(id));
        return true;
      } catch {
        return false;
      }
    },

    /** Every proposal, oldest first. Unreadable files are skipped. */
    async listProposals() {
      let names = [];
      try {
        names = (await readdir(proposalsDir)).filter((f) => f.endsWith('.json') && ID.test(f.slice(0, -5))).sort();
      } catch {
        return [];
      }
      /** @type {Proposal[]} */
      const out = [];
      for (const f of names) {
        try {
          out.push(JSON.parse(await readFile(join(proposalsDir, f), 'utf8')));
        } catch {
          // a half-written or hand-edited file: not listed
        }
      }
      // Ids sort by second only (two saves in one second get random suffixes): order by createdAt (ms), then id.
      const at = (/** @type {Proposal} */ p) => Date.parse(p.createdAt) || 0;
      return out.sort((a, b) => at(a) - at(b) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
    },

    async removeProposal(/** @type {string} */ id) {
      const file = proposalFile(id);
      try {
        await stat(file);
      } catch {
        throw new NotFoundError(`no proposal ${id} (pixellab proposal list)`);
      }
      await rm(file, { force: true });
    },

    /** Appends one activity entry. */
    async appendActivity(/** @type {Record<string, unknown>} */ entry) {
      await mkdir(dir, { recursive: true });
      await appendFile(activityFile, `${JSON.stringify(entry)}\n`);
    },

    /** The last `limit` activity entries, oldest first. */
    async tailActivity(limit = 50) {
      let text = '';
      try {
        text = await readFile(activityFile, 'utf8');
      } catch {
        return [];
      }
      return parseActivity(text).slice(-limit);
    },
  };
}

/** Activity lines → entries; a broken line is skipped. */
export function parseActivity(/** @type {string} */ text) {
  /** @type {any[]} */
  const out = [];
  for (const line of text.split('\n')) {
    if (!line.trim()) continue;
    try {
      out.push(JSON.parse(line));
    } catch {
      // skipped
    }
  }
  return out;
}

/** @typedef {ReturnType<typeof createState>} State */
