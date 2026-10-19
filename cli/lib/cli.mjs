// The CLI door: argv → a command run → human text or exact JSON, and an exit
// code (0 ok, 1 command error or failed check, 2 usage error). main() takes
// its streams and environment as arguments, so tests call it directly.

import { writeFile } from 'node:fs/promises';
import { resolve as resolvePath } from 'node:path';

import { commandWords, GLOBAL_FLAGS, parseCommandLine } from './argv.mjs';
import { createContext } from './context.mjs';
import { CommandError, UsageError } from './errors.mjs';
import { helpFor } from './help.mjs';
import { checkParams } from './params.mjs';
import { registry as defaultRegistry } from './registry.mjs';
import { openSite } from './site.mjs';

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
        if (key in GLOBAL_FLAGS) throw new Error(`command ${cmd.name}: param ${key} clashes with the global --${key}`);
      }
    }
    const parsed = parseCommandLine(argv, registry);
    if (parsed.kind === 'help') {
      const text = helpFor(registry, parsed.topic);
      if (text === null) throw new UsageError(`no help for ${parsed.topic.join(' ')}; run pixellab help`);
      // A bare `pixellab` or `pixellab scene` is a usage error that shows help.
      (parsed.usage ? stderr : stdout).write(`${text}\n`);
      return parsed.usage ? 2 : 0;
    }
    const { command, params, globals } = parsed;
    if (globals.out !== undefined && !command.writesPng) {
      throw new UsageError(`--out goes with a render command, not ${commandWords(command.name)}`);
    }
    // Params are checked before the site is read, so a usage error is always exit 2.
    const checked = checkParams(command.params, params, { coerce: true, command: command.name });
    const site = await openSite({ site: globals.site, env, cwd });
    const ctx = createContext(site);
    const result = await registry.call(ctx, command.name, checked);
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
      stdout.write([text, wrote, hint].filter(Boolean).join('\n') + '\n');
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
