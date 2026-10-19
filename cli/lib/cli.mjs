// The CLI door: argv → a command run → human text or exact JSON, and an exit
// code (0 ok, 1 command error or failed check, 2 usage error). main() takes
// its streams and environment as arguments, so tests call it directly.
//
// One-shot edits: each command runs in a fresh session (session.mjs). A
// mutating command's changes are saved right after it runs, validated and
// checked for conflicts first (--force writes anyway); --dry-run prints the
// diff instead and writes nothing. `serve` starts the stdio or MCP server.

import { writeFile } from 'node:fs/promises';
import { resolve as resolvePath } from 'node:path';

import { commandWords, GLOBAL_FLAGS, parseCommandLine } from './argv.mjs';
import { CommandError, UsageError } from './errors.mjs';
import { helpFor } from './help.mjs';
import { checkParams } from './params.mjs';
import { registry as defaultRegistry } from './registry.mjs';
import { serve } from './serve.mjs';
import { Session } from './session.mjs';
import { openDocument } from './document.mjs';

/**
 * @typedef {{ write: (text: string) => unknown }} Writable
 * @typedef {{
 *   stdout?: Writable,
 *   stderr?: Writable,
 *   env?: Record<string, string | undefined>,
 *   cwd?: string,
 *   registry?: import('./registry.mjs').Registry,
 * }} IO
 */

/**
 * Runs one command line. Never throws; returns the exit code.
 * @param {string[]} argv  the arguments after `pixellab`
 * @param {IO} [io]
 * @returns {Promise<number>}
 */
export async function main(argv, io = {}) {
  const { stdout = process.stdout, stderr = process.stderr, env = process.env, cwd = process.cwd(), registry = defaultRegistry } = io;
  const json = argv.includes('--json');
  try {
    for (const cmd of registry.commands.values()) {
      for (const key of Object.keys(cmd.params.properties ?? {})) {
        if (key in GLOBAL_FLAGS && key !== 'force') throw new Error(`command ${cmd.name}: param ${key} clashes with the global --${key}`);
      }
    }
    if (argv[0] === 'serve') return await serve(argv.slice(1), { stdout, env, cwd });
    const parsed = parseCommandLine(argv, registry);
    if (parsed.kind === 'help') {
      const text = helpFor(registry, parsed.topic);
      if (text === null) throw new UsageError(`no help for ${parsed.topic.join(' ')}; run pixellab help`);
      // A bare `pixellab` or `pixellab scene` is a usage error that shows help.
      (parsed.usage ? stderr : stdout).write(`${text}\n`);
      return parsed.usage ? 2 : 0;
    }
    const { command, params, globals } = parsed;
    const dryRun = Boolean(globals['dry-run']);
    if (globals.out !== undefined && !command.writesPng) {
      throw new UsageError(`--out goes with a render command, not ${commandWords(command.name)}`);
    }
    if (command.session) {
      throw new UsageError(`${commandWords(command.name)} works on a session's open docs: run it through pixellab serve --stdio or --mcp. On the command line, edits are saved as they're made.`);
    }
    if (dryRun && (!command.mutates || command.writes)) throw new UsageError(`--dry-run goes with an edit command, not ${commandWords(command.name)}`);
    if (globals.force && !command.mutates && !command.params.properties?.force) throw new UsageError(`--force goes with an edit command, not ${commandWords(command.name)}`);
    if (globals.force && command.params.properties?.force) params.force = 'true';
    // Params are checked before the site is read, so a usage error is always exit 2.
    const checked = checkParams(command.params, params, { coerce: true, command: command.name });
    if (command.document === false) {
      // It makes a document, so there's none to open first (a Mac with no site checkout has no default).
      const result = await registry.call({}, command.name, checked);
      if (globals.json) stdout.write(`${JSON.stringify(result, null, 2)}\n`);
      else stdout.write(`${command.format ? command.format(result, checked) : JSON.stringify(result, null, 2)}\n`);
      return 0;
    }
    const site = await openDocument({ doc: globals.document, site: globals.site, env, cwd });
    const session = new Session(site, { env });
    const ctx = session.context();
    let result = await registry.call(ctx, command.name, checked);
    /** @type {string[]} */
    const notes = [];
    if (command.mutates && !command.writes) {
      const touched = [...session.entries.values()].filter((e) => session.status(e).dirty);
      if (dryRun) {
        const plans = [];
        for (const entry of touched) {
          const plan = await session.plan(ctx, entry);
          plans.push({ doc: entry.ref, file: entry.file, diff: plan.diff, problems: plan.problems, conflict: plan.conflict });
          notes.push(plan.diff.trimEnd());
          if (plan.problems.length) notes.push(`${entry.file} has ${plan.problems.length} problem${plan.problems.length === 1 ? '' : 's'}; saving it would be refused (--force writes anyway):`, ...plan.problems);
          if (plan.conflict) notes.push(`${entry.file} changed on disk; saving would be refused as a conflict`);
        }
        notes.push(touched.length ? `dry run: nothing written (${touched.map((e) => e.file).join(', ')} would change)` : 'dry run: nothing would change');
        result = { ...result, dryRun: plans };
      } else {
        const saved = [];
        for (const entry of touched) {
          const r = await session.save(ctx, entry.ref, { force: Boolean(globals.force) });
          saved.push({ doc: r.doc, file: r.file, wrote: r.wrote, hash: r.hash, ...(r.forcedProblems ? { forcedProblems: r.forcedProblems } : {}) });
          notes.push(r.wrote ? `wrote ${r.file}` : `${r.file} unchanged`);
          if (r.forcedProblems) notes.push(`(forced: ${r.forcedProblems.length} problem${r.forcedProblems.length === 1 ? '' : 's'} written)`);
        }
        if (!touched.length) notes.push('nothing written');
        result = { ...result, saved };
      }
    }
    /** @type {string | undefined} */
    let wrote;
    if (globals.out !== undefined) {
      const file = resolvePath(cwd, globals.out);
      await writeFile(file, Buffer.from(result.png, 'base64'));
      wrote = `wrote ${file} (${result.width}×${result.height})`;
    }
    if (globals.json) stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    else {
      const text = command.format ? command.format(result, checked) : JSON.stringify(result, null, 2);
      const hint = command.writesPng && !wrote ? '(add --out <file.png> to write the PNG, or --json for it as base64)' : '';
      stdout.write([text, wrote, ...notes, hint].filter(Boolean).join('\n') + '\n');
    }
    return result && typeof result === 'object' && result.ok === false ? 1 : 0;
  } catch (err) {
    if (err instanceof CommandError) {
      if (json) stderr.write(`${JSON.stringify({ error: err.toJSON() })}\n`);
      else stderr.write(`pixellab: ${err.message}\n`);
      return err.exitCode;
    }
    const e = /** @type {Error} */ (err);
    if (json) stderr.write(`${JSON.stringify({ error: { code: 'internal', message: e.message } })}\n`);
    else stderr.write(`pixellab: internal error: ${e.stack ?? e.message}\n`);
    return 1;
  }
}
