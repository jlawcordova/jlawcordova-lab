// The command line → a command and its params, from the registry's schemas.
//
//   pixellab <group> <verb> [positionals] [--flags] [--json] [--site <path>] [--out <file>]
//
// The rules, the same for every command (the help prints them too):
// - The leading words name the command: `scene render` is scene.render. The
//   longest run of words that names a command wins, so `check hero-island` is
//   `check` with one positional.
// - Positionals fill the command's `args` in order (its usage line shows
//   them). If the last arg is a list, it takes the rest.
// - Every param is also a flag, its name in kebab-case: --name, --scale.
//   `--flag value` and `--flag=value` both work.
// - Booleans: `--flag` (true), `--no-flag` (false), or `--flag=true|false`.
// - Lists: comma-separated (`--at 14,6,1`) or the flag repeated.
// - Objects: JSON (`--data '{"a": 1}'`).
// - `--` ends the flags; everything after it is positional.
// - Global flags go anywhere: --json, --site <path>, --out <file>, -h/--help.
// Values stay strings here; checkParams() coerces them to the schema's types.

import { UsageError } from './errors.mjs';
import { suggest } from './params.mjs';

export const GLOBAL_FLAGS = {
  json: { value: false, help: "print the command's result as JSON" },
  site: { value: true, help: 'the site checkout (else $PIXELLAB_SITE, ~/.config/pixellab/config.json, ~/Projects/jlawcordova.github.io)' },
  out: { value: true, help: 'write the PNG of a render command to this file' },
  help: { value: false, help: 'help for the command or group' },
};

/** camelCase → kebab-case, for flag names. */
export const kebab = (/** @type {string} */ name) => name.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);

/** The words a command is typed as: scene.render → "scene render". */
export const commandWords = (/** @type {string} */ name) => name.split('.').join(' ');

/**
 * @typedef {{ json: boolean, help: boolean, site?: string, out?: string }} Globals
 * @typedef {
 *   | { kind: 'help', topic: string[], globals: Globals, usage?: boolean }
 *   | { kind: 'command', command: import('./registry.mjs').Command, params: Record<string, string | string[]>, globals: Globals }
 * } Parsed
 */

/**
 * Pulls the global flags out of argv, wherever they are.
 * @param {string[]} argv
 */
function takeGlobals(argv) {
  /** @type {Globals} */
  const globals = { json: false, help: false };
  /** @type {string[]} */
  const rest = [];
  for (let i = 0; i < argv.length; i++) {
    const token = argv[i];
    if (token === '--') {
      rest.push(...argv.slice(i));
      break;
    }
    if (token === '-h') {
      globals.help = true;
      continue;
    }
    const m = /^--([a-z][a-z0-9-]*)(?:=(.*))?$/s.exec(token);
    const name = m?.[1];
    if (!m || !name || !(name in GLOBAL_FLAGS)) {
      rest.push(token);
      continue;
    }
    const spec = GLOBAL_FLAGS[/** @type {keyof typeof GLOBAL_FLAGS} */ (name)];
    if (!spec.value) {
      if (m[2] !== undefined && !['true', 'false'].includes(m[2])) throw new UsageError(`--${name} takes no value`);
      /** @type {any} */ (globals)[name] = m[2] !== 'false';
      continue;
    }
    let value = m[2];
    if (value === undefined) {
      if (i + 1 >= argv.length) throw new UsageError(`--${name} needs a value`);
      value = argv[++i];
    }
    if (/** @type {any} */ (globals)[name] !== undefined) throw new UsageError(`--${name} is given twice`);
    /** @type {any} */ (globals)[name] = value;
  }
  return { globals, rest };
}

/**
 * Parses a command line against the registry.
 * @param {string[]} argv  the arguments after `pixellab`
 * @param {import('./registry.mjs').Registry} registry
 * @returns {Parsed}
 */
export function parseCommandLine(argv, registry) {
  const { globals, rest } = takeGlobals(argv);
  let words = [];
  let i = 0;
  while (i < rest.length && !rest[i].startsWith('-')) words.push(rest[i++]);
  if (words[0] === 'help') return { kind: 'help', topic: words.slice(1), globals };
  if (words.length === 0) {
    if (i < rest.length && rest[i] !== '--') throw new UsageError(`a command comes before ${rest[i]}; run pixellab help`);
    return { kind: 'help', topic: [], globals, usage: !globals.help };
  }
  let found;
  for (let n = words.length; n > 0 && !found; n--) {
    const name = words.slice(0, n).join('.');
    if (registry.has(name)) found = { command: /** @type {import('./registry.mjs').Command} */ (registry.get(name)), used: n };
  }
  if (!found) {
    const group = words[0];
    const isGroup = registry.groups().has(group);
    if (isGroup && (globals.help || words.length === 1)) return { kind: 'help', topic: [group], globals, usage: !globals.help };
    const typed = words.slice(0, isGroup ? 2 : 1).join(' ');
    const names = isGroup ? [...registry.commands.keys()].map(commandWords) : [...registry.groups().keys()];
    throw new UsageError(`unknown command ${typed}${suggest(typed, names)}; run pixellab help`);
  }
  const { command, used } = found;
  if (globals.help) return { kind: 'help', topic: command.name.split('.'), globals };
  const params = parseParams(command, [...words.slice(used), ...rest.slice(i)]);
  return { kind: 'command', command, params, globals };
}

/**
 * Positionals and flags → params (as strings), for one command.
 * @param {import('./registry.mjs').Command} command
 * @param {string[]} tokens
 */
export function parseParams(command, tokens) {
  const props = command.params.properties ?? {};
  /** @type {Map<string, string>} flag name → param */
  const flags = new Map();
  for (const key of Object.keys(props)) {
    flags.set(kebab(key), key);
    flags.set(key, key);
  }
  /** @type {Record<string, string | string[]>} */
  const params = {};
  const words = commandWords(command.name);
  const set = (/** @type {string} */ key, /** @type {string} */ value, /** @type {string} */ how) => {
    const isList = props[key]?.type === 'array';
    if (key in params) {
      if (!isList) throw new UsageError(`${words}: ${how} is given twice`);
      params[key] = [.../** @type {string[]} */ ([params[key]].flat()), value];
    } else params[key] = isList ? [value] : value;
  };
  /** @type {string[]} */
  const positionals = [];
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    if (token === '--') {
      positionals.push(...tokens.slice(i + 1));
      break;
    }
    if (!token.startsWith('--') || token === '--') {
      if (/^-[^-\d]/.test(token)) throw new UsageError(`${words}: unknown flag ${token}; run pixellab ${words} --help`);
      positionals.push(token);
      continue;
    }
    const m = /^--([^=]+)(?:=(.*))?$/s.exec(token);
    const raw = /** @type {RegExpExecArray} */ (m)[1];
    let value = /** @type {RegExpExecArray} */ (m)[2];
    let key = flags.get(raw);
    if (!key && raw.startsWith('no-') && props[flags.get(raw.slice(3)) ?? '']?.type === 'boolean') {
      if (value !== undefined) throw new UsageError(`${words}: --${raw} takes no value`);
      set(/** @type {string} */ (flags.get(raw.slice(3))), 'false', `--${raw.slice(3)}`);
      continue;
    }
    if (!key) {
      const known = [...new Set([...flags.keys()].map((f) => `--${kebab(f)}`))];
      throw new UsageError(`${words}: unknown flag --${raw}${suggest(`--${raw}`, known)}; run pixellab ${words} --help`);
    }
    if (props[key].type === 'boolean') {
      set(key, value ?? 'true', `--${kebab(key)}`);
      continue;
    }
    if (value === undefined) {
      if (i + 1 >= tokens.length) throw new UsageError(`${words}: --${kebab(key)} needs a value`);
      value = tokens[++i];
    }
    set(key, value, `--${kebab(key)}`);
  }
  const args = command.args ?? [];
  positionals.forEach((value, n) => {
    const last = args.length - 1;
    const key = n < args.length ? args[n] : props[args[last]]?.type === 'array' ? args[last] : undefined;
    if (!key) {
      const extra = positionals.slice(args.length).join(' ');
      throw new UsageError(`${words}: unexpected ${extra}; run pixellab ${words} --help`);
    }
    if (key in params && !(props[key].type === 'array' && n >= args.length)) throw new UsageError(`${words}: ${key} is given twice`);
    set(key, value, key);
  });
  return params;
}
