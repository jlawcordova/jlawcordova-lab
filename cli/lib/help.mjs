// Help text, generated from the registry.

import { commandWords, GLOBAL_FLAGS, kebab } from './argv.mjs';

const MAPPING = `Arguments and flags (the same rules for every command):
  Positionals fill the command's arguments in the order its usage shows them.
  Every param is also a --flag, its name in kebab-case: --name hero-island.
  --flag value and --flag=value both work; -- ends the flags.
  Booleans: --flag, --no-flag, or --flag=true|false.
  Lists: comma-separated (--at 14,6,1) or the flag repeated.
  Objects: JSON (--data '{"a": 1}').
  Values are checked against the command's params schema (pixellab <command> --help).`;

const EXIT = `Exit codes: 0 ok, 1 command error (or check found problems), 2 usage error.`;

/** The placeholder for a param's value. */
function placeholder(/** @type {any} */ schema) {
  if (schema.enum) return `<${schema.enum.join('|')}>`;
  switch (schema.type) {
    case 'integer':
    case 'number':
      return '<n>';
    case 'boolean':
      return '';
    case 'array':
      return `<${placeholder(schema.items ?? {}).replace(/[<>]/g, '') || 'item'},…>`;
    case 'object':
      return '<json>';
    default:
      return '<text>';
  }
}

/** pixellab scene render <name> [--scale <n>] … */
export function usageLine(/** @type {import('./registry.mjs').Command} */ cmd) {
  const props = cmd.params.properties ?? {};
  const required = new Set(cmd.params.required ?? []);
  const args = cmd.args ?? [];
  const parts = [`pixellab ${commandWords(cmd.name)}`];
  for (const arg of args) {
    const many = props[arg].type === 'array' ? '…' : '';
    parts.push(required.has(arg) ? `<${arg}${many}>` : `[${arg}${many}]`);
  }
  for (const [key, schema] of Object.entries(props)) {
    if (args.includes(key)) continue;
    const flag = `--${kebab(key)}${placeholder(schema) ? ` ${placeholder(schema)}` : ''}`;
    parts.push(required.has(key) ? flag : `[${flag}]`);
  }
  if (cmd.writesPng) parts.push('[--out <file.png>]');
  return parts.join(' ');
}

const pad = (/** @type {string} */ s, /** @type {number} */ n) => s.padEnd(n);

/** The overview: every command, the global flags, the mapping rules. */
export function overviewHelp(/** @type {import('./registry.mjs').Registry} */ registry) {
  const rows = [...registry.commands.values()].map((cmd) => {
    const args = (cmd.args ?? []).map((a) => ((cmd.params.required ?? []).includes(a) ? `<${a}>` : `[${a}]`)).join(' ');
    return [`${commandWords(cmd.name)}${args ? ` ${args}` : ''}`, cmd.summary];
  });
  const w = Math.max(...rows.map(([a]) => a.length));
  return [
    'pixellab: read and edit the pixel art of jlawcordova.github.io',
    '',
    'Usage: pixellab <group> <verb> [args] [--flags] [--json] [--site <path>]',
    '',
    'Commands:',
    ...rows.map(([a, b]) => `  ${pad(a, w)}  ${b}`),
    '',
    globalFlagsHelp(),
    '',
    MAPPING,
    '',
    EXIT,
    '',
    'Run pixellab <group> --help or pixellab <group> <verb> --help for details.',
  ].join('\n');
}

function globalFlagsHelp() {
  const rows = Object.entries(GLOBAL_FLAGS).map(([name, spec]) => [
    name === 'help' ? '-h, --help' : `--${name}${spec.value ? (name === 'out' ? ' <file.png>' : ' <path>') : ''}`,
    spec.help,
  ]);
  const w = Math.max(...rows.map(([a]) => a.length));
  return ['Global flags:', ...rows.map(([a, b]) => `  ${pad(a, w)}  ${b}`)].join('\n');
}

/** One group's commands with their usage lines. */
export function groupHelp(/** @type {import('./registry.mjs').Registry} */ registry, /** @type {string} */ group) {
  const cmds = registry.groups().get(group) ?? [];
  return [
    `pixellab ${group}: ${cmds.length} command${cmds.length === 1 ? '' : 's'}`,
    '',
    ...cmds.flatMap((cmd) => [`  ${usageLine(cmd)}`, `      ${cmd.summary}`]),
    '',
    `Run pixellab ${group} <verb> --help for a command's params. Global flags: --json, --site <path>, -h.`,
  ].join('\n');
}

/** One command: usage, description, arguments, flags, examples. */
export function commandHelp(/** @type {import('./registry.mjs').Command} */ cmd) {
  const props = cmd.params.properties ?? {};
  const required = new Set(cmd.params.required ?? []);
  const args = cmd.args ?? [];
  /** @type {string[][]} */
  const rows = [];
  for (const [key, schema] of Object.entries(props)) {
    const s = /** @type {any} */ (schema);
    const name = args.includes(key) ? `${required.has(key) ? `<${key}>` : `[${key}]`}, --${kebab(key)}` : `--${kebab(key)}${placeholder(s) ? ` ${placeholder(s)}` : ''}`;
    const facts = [
      s.type === 'array' ? `list of ${s.items?.type ?? 'values'}` : s.type,
      required.has(key) ? 'required' : '',
      s.default !== undefined ? `default ${JSON.stringify(s.default)}` : '',
      s.minimum !== undefined && s.maximum !== undefined
        ? `${s.minimum} to ${s.maximum}`
        : s.minimum !== undefined
          ? `at least ${s.minimum}`
          : s.maximum !== undefined
            ? `at most ${s.maximum}`
            : '',
    ].filter(Boolean);
    rows.push([name, `${s.description ?? ''}${s.description ? ' ' : ''}(${facts.join(', ')})`]);
  }
  if (cmd.writesPng) rows.push(['--out <file.png>', 'write the PNG to this file (CLI only)']);
  const w = Math.max(0, ...rows.map(([a]) => a.length));
  const lines = [`Usage: ${usageLine(cmd)}`, '', cmd.summary + '.'];
  if (cmd.description) lines.push('', ...wrap(cmd.description, 78));
  lines.push('', `Command: ${cmd.name}${cmd.mutates ? ' (mutates)' : ' (read-only)'}`);
  if (rows.length) lines.push('', 'Params:', ...rows.map(([a, b]) => `  ${pad(a, w)}  ${b}`));
  if (cmd.examples?.length) lines.push('', 'Examples:', ...cmd.examples.map((e) => `  ${e}`));
  lines.push('', 'Global flags: --json (the result as JSON), --site <path>, -h. Run pixellab help for how args and flags map to params.');
  return lines.join('\n');
}

function wrap(/** @type {string} */ text, /** @type {number} */ width) {
  const lines = [];
  let line = '';
  for (const word of text.split(/\s+/)) {
    if (line && line.length + 1 + word.length > width) {
      lines.push(line);
      line = word;
    } else line = line ? `${line} ${word}` : word;
  }
  if (line) lines.push(line);
  return lines;
}

/**
 * Help for a topic: [] (overview), [group], or a command's words.
 * @param {import('./registry.mjs').Registry} registry
 * @param {string[]} topic
 * @returns {string | null} null if the topic names nothing
 */
export function helpFor(registry, topic) {
  if (topic.length === 0) return overviewHelp(registry);
  const cmd = registry.get(topic.join('.'));
  // A group of one command with the group's own name (check) is that command.
  if (cmd && (topic.length > 1 || (registry.groups().get(topic[0]) ?? []).length === 1)) return commandHelp(cmd);
  if (topic.length === 1 && registry.groups().has(topic[0])) return groupHelp(registry, topic[0]);
  return null;
}
