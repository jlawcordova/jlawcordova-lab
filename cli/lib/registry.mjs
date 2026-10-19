// The command registry (docs/intents/2026-10-mac-app/plan.md, "One command registry, three doors").
// Every action is a command: a dotted name, a summary, its params as a JSON
// Schema object, whether it mutates, and run(ctx, params) → plain JSON. The
// CLI, the app server and the MCP server all list and call commands from
// here, and all check params with the same checkParams().
//
// Door-specific fields, all optional:
//   description  longer help text
//   args         param names the CLI fills from positionals, in order
//   format       (result, params) → the CLI's human-readable output
//   examples     command lines for the help
//   writesPng    the result has a base64 `png` the CLI can write with --out
//   session      runs only in a session (pixellab serve): doc.open, doc.undo…
//   writes       'direct': writes files itself (art.build, proposal.apply), so
//                the CLI's --dry-run doesn't apply to it
//
// A mutating command (mutates: true) edits docs through ctx.edit (session.mjs).
// The CLI saves what it changed right after it runs (or shows the diff with
// --dry-run); the servers keep the edits in the session until doc.save.

import artCommands from './commands/art.mjs';
import checkCommands from './commands/check.mjs';
import docCommands from './commands/doc.mjs';
import objectEditCommands from './commands/object-edit.mjs';
import objectCommands from './commands/object.mjs';
import proposalCommands from './commands/proposal.mjs';
import sceneEditCommands from './commands/scene-edit.mjs';
import sceneMotionCommands from './commands/scene-motion.mjs';
import sceneCommands from './commands/scene.mjs';
import documentCommands from './commands/document.mjs';
import { NotFoundError } from './errors.mjs';
import { checkParams, suggest } from './params.mjs';

/**
 * @typedef {{
 *   name: string,
 *   summary: string,
 *   description?: string,
 *   params: import('./params.mjs').Schema,
 *   args?: string[],
 *   mutates: boolean,
 *   run: (ctx: any, params: Record<string, any>) => Promise<any> | any,
 *   format?: (result: any, params: Record<string, any>) => string,
 *   examples?: string[],
 *   writesPng?: boolean,
 *   session?: boolean,
 *   writes?: 'direct',
 * }} Command
 */

const NAME = /^[a-z][a-z0-9]*(?:\.[a-z][a-z0-9]*)*$/;

/** Throws if a command definition is malformed: caught by the tests, never at a user. */
export function checkDefinition(/** @type {Command} */ cmd) {
  const fail = (/** @type {string} */ msg) => {
    throw new Error(`command ${cmd?.name ?? '?'}: ${msg}`);
  };
  if (!cmd || typeof cmd !== 'object') fail('must be an object');
  if (typeof cmd.name !== 'string' || !NAME.test(cmd.name)) fail('name must be dotted lowercase words, like scene.place');
  if (typeof cmd.summary !== 'string' || !cmd.summary) fail('summary is required');
  if (!cmd.params || cmd.params.type !== 'object' || typeof cmd.params.properties !== 'object') fail('params must be a JSON Schema object with properties');
  if (typeof cmd.mutates !== 'boolean') fail('mutates must be true or false');
  if (typeof cmd.run !== 'function') fail('run must be a function');
  if (cmd.writes !== undefined && cmd.writes !== 'direct') fail("writes must be 'direct' when given");
  for (const arg of cmd.args ?? []) if (!(arg in cmd.params.properties)) fail(`args names ${arg}, which isn't a param`);
  for (const req of cmd.params.required ?? []) if (!(req in cmd.params.properties)) fail(`required names ${req}, which isn't a param`);
  return cmd;
}

/**
 * Builds a registry from command definitions.
 * @param {Command[]} list
 */
export function createRegistry(list) {
  /** @type {Map<string, Command>} */
  const commands = new Map();
  for (const cmd of list) {
    checkDefinition(cmd);
    if (commands.has(cmd.name)) throw new Error(`command ${cmd.name} is defined twice`);
    commands.set(cmd.name, cmd);
  }
  const sorted = new Map([...commands].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));

  return {
    /** Every command, by name, in name order. */
    commands: sorted,

    /** @param {string} name */
    get: (name) => sorted.get(name),

    /** @param {string} name */
    has: (name) => sorted.has(name),

    /** Commands grouped by their first word ("scene", "object", "check"). */
    groups() {
      /** @type {Map<string, Command[]>} */
      const out = new Map();
      for (const cmd of sorted.values()) {
        const group = cmd.name.split('.')[0];
        if (!out.has(group)) out.set(group, []);
        /** @type {Command[]} */ (out.get(group)).push(cmd);
      }
      return out;
    },

    /**
     * Checks params against the command's schema, then runs it.
     * @param {any} ctx
     * @param {string} name
     * @param {Record<string, unknown>} [params]
     * @param {{ coerce?: boolean }} [options]  coerce: params are strings from a command line
     */
    async call(ctx, name, params = {}, { coerce = false } = {}) {
      const cmd = sorted.get(name);
      if (!cmd) throw new NotFoundError(`no command ${name}${suggest(name, [...sorted.keys()])}`);
      const checked = checkParams(cmd.params, params, { coerce, command: name });
      return cmd.run(ctx, checked);
    },
  };
}

/** @typedef {ReturnType<typeof createRegistry>} Registry */

/** The built-in commands. */
export const registry = createRegistry([
  ...documentCommands,
  ...sceneCommands,
  ...sceneEditCommands,
  ...sceneMotionCommands,
  ...objectCommands,
  ...objectEditCommands,
  ...checkCommands,
  ...artCommands,
  ...docCommands,
  ...proposalCommands,
]);
