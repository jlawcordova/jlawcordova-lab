// art.build: runs the site's `npm run art`, which validates every source and
// compiles the scenes to the generated SVGs (src/assets/pixel-art/*.svg). It
// builds what's on disk: unsaved working copies aren't in it, so the result
// lists them.

import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';

export const BUILD_TIMEOUT_MS = 5 * 60 * 1000;

/** npm next to the running node (the app may start us without a login PATH), else npm on the PATH. */
export function npmPath(execPath = process.execPath) {
  const beside = join(dirname(execPath), 'npm');
  return existsSync(beside) ? beside : 'npm';
}

/**
 * Runs a command and collects its output.
 * @param {string} cmd
 * @param {string[]} args
 * @param {{ cwd: string, timeout: number, env?: Record<string, string | undefined> }} options
 * @returns {Promise<{ code: number | null, signal: string | null, stdout: string, stderr: string, timedOut: boolean, error?: string }>}
 */
export function run(cmd, args, { cwd, timeout, env = process.env }) {
  return new Promise((resolve) => {
    const path = [dirname(process.execPath), env.PATH ?? ''].filter(Boolean).join(':');
    const child = spawn(cmd, args, { cwd, env: { ...env, PATH: path }, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill('SIGTERM');
    }, timeout);
    child.stdout.setEncoding('utf8').on('data', (d) => (stdout += d));
    child.stderr.setEncoding('utf8').on('data', (d) => (stderr += d));
    child.on('error', (err) => {
      clearTimeout(timer);
      resolve({ code: null, signal: null, stdout, stderr, timedOut, error: err.message });
    });
    child.on('close', (code, signal) => {
      clearTimeout(timer);
      resolve({ code, signal, stdout, stderr, timedOut });
    });
  });
}

const lines = (/** @type {string} */ text) => text.split('\n').map((l) => l.trimEnd()).filter(Boolean);

/** @type {import('../registry.mjs').Command} */
const build = {
  name: 'art.build',
  summary: "Run the site's npm run art: validate every source and write the generated SVGs",
  description:
    'Runs `npm run art --silent` in the site checkout (timeout 5 minutes) and returns { ok, code, report: [stdout lines], errors: [stderr lines], ms, unsaved }. ' +
    'It builds the files on disk; unsaved lists the open docs with changes it leaves out.',
  params: { type: 'object', properties: {} },
  args: [],
  mutates: false,
  writes: 'direct',
  async run(ctx) {
    const started = Date.now();
    const r = await run(npmPath(), ['run', 'art', '--silent'], { cwd: ctx.site.root, timeout: BUILD_TIMEOUT_MS });
    const unsaved = ctx.session ? ctx.session.list().filter((/** @type {any} */ s) => s.dirty).map((/** @type {any} */ s) => s.doc) : [];
    return {
      ok: r.code === 0,
      code: r.code,
      report: lines(r.stdout),
      errors: [...lines(r.stderr), ...(r.timedOut ? [`npm run art took longer than ${BUILD_TIMEOUT_MS / 1000} s and was stopped`] : []), ...(r.error ? [`npm run art could not start: ${r.error}`] : [])],
      ms: Date.now() - started,
      unsaved,
    };
  },
  format: (r) =>
    [...r.report, ...r.errors, r.ok ? `art built in ${(r.ms / 1000).toFixed(1)} s` : `npm run art failed (exit ${r.code})`, ...(r.unsaved.length ? [`not built (unsaved): ${r.unsaved.join(', ')}`] : [])].join('\n'),
};

export default [build];
