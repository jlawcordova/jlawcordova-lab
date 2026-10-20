// A stand-in for the `claude` CLI (docs/intents/2026-10-agent-chat/spec.md, D16), so the app's
// Agent chat is tested without the owner's plan or the network. It answers what Pixel Lab runs:
//
//   claude --version                         2.1.290 (Claude Code)
//   claude auth status --json                { loggedIn, authMethod, ... } from its state
//   claude auth login --claudeai             prints a sign-in URL, waits FAKE_CLAUDE_LOGIN_MS, signs in
//   claude auth logout                       signs out
//   claude -p --input-format stream-json --output-format stream-json ... --mcp-config <json> [--resume <id>]
//       reads one user message, then plays the first turn in turns.json whose `match` fits the
//       message: its tool calls go through the real pixellab MCP server named in --mcp-config, and
//       every line it writes has the shape the real CLI writes (system/init, assistant text and
//       tool_use, user tool_result, result). Signed out, it writes the lines the real CLI wrote for
//       an expired login on 2026-10-21.
//
// It refuses a turn without the flags that keep the chat to Pixel Lab's tools (--tools "",
// --strict-mcp-config, --permission-prompts none), so the acceptance pass catches the app dropping one.
//
// State (auth.json, sessions/) is kept in FAKE_CLAUDE_STATE, else beside this script's real path.
// FAKE_CLAUDE_STEP_MS paces the steps (default 60). FAKE_CLAUDE_SIGNED_IN=1 starts signed in.

import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { createInterface } from 'node:readline';

const VERSION = '2.1.290';
const here = dirname(realpathSync(process.argv[1]));
const stateDir = process.env.FAKE_CLAUDE_STATE || join(here, 'fake-claude-state');
mkdirSync(join(stateDir, 'sessions'), { recursive: true });
const authFile = join(stateDir, 'auth.json');
const sleep = (/** @type {number} */ ms) => new Promise((r) => setTimeout(r, ms));
const out = (/** @type {any} */ line) => process.stdout.write(`${JSON.stringify(line)}\n`);

function signedIn() {
  if (existsSync(authFile)) return JSON.parse(readFileSync(authFile, 'utf8')).loggedIn === true;
  return process.env.FAKE_CLAUDE_SIGNED_IN === '1';
}
const setSignedIn = (/** @type {boolean} */ on) => writeFileSync(authFile, `${JSON.stringify({ loggedIn: on })}\n`);

const args = process.argv.slice(2);
const flag = (/** @type {string} */ name) => {
  const i = args.indexOf(name);
  return i < 0 ? undefined : args[i + 1];
};

async function auth(/** @type {string | undefined} */ sub) {
  if (sub === 'status') {
    const on = signedIn();
    console.log(JSON.stringify(on ? { loggedIn: true, authMethod: 'claude.ai', apiProvider: 'firstParty', subscriptionType: 'max' } : { loggedIn: false, authMethod: 'none', apiProvider: 'firstParty' }, null, 2));
  } else if (sub === 'login') {
    console.log('Opening your browser to sign in…');
    console.log('If it doesn\'t open, visit: https://claude.ai/oauth/authorize?code=true&fake=1');
    await sleep(Number(process.env.FAKE_CLAUDE_LOGIN_MS ?? 800));
    setSignedIn(true);
    console.log('Login successful.');
  } else if (sub === 'logout') {
    setSignedIn(false);
    console.log('Successfully logged out from your Anthropic account.');
  } else {
    console.error(`fake claude: no auth ${sub}`);
    process.exitCode = 2;
  }
}

/** One headless turn. Returns the exit code. */
async function turn() {
  for (const [name, want] of [['--input-format', 'stream-json'], ['--output-format', 'stream-json'], ['--tools', ''], ['--permission-prompts', 'none']]) {
    if (flag(name) !== want) return refuse(`${name} ${JSON.stringify(want)} is missing`);
  }
  for (const name of ['--strict-mcp-config', '--verbose']) if (!args.includes(name)) return refuse(`${name} is missing`);
  if (!(flag('--allowedTools') ?? '').startsWith('mcp__pixellab__')) return refuse('--allowedTools mcp__pixellab__* is missing');

  const message = await readMessage();
  const resume = flag('--resume');
  if (resume && !existsSync(join(stateDir, 'sessions', `${resume}.json`))) {
    console.error(`No conversation found with session ID: ${resume}`);
    return 1;
  }
  const session = resume ?? randomUUID();
  const base = { session_id: session };
  if (!signedIn()) {
    // As the real CLI wrote it for an expired login (2026-10-21), trimmed.
    out({ type: 'system', subtype: 'init', cwd: process.cwd(), session_id: session, tools: [], mcp_servers: [{ name: 'pixellab', status: 'connected' }], model: 'claude-fable-5-1', permissionMode: 'default', apiKeySource: 'none', claude_code_version: VERSION });
    out({ type: 'assistant', message: { id: randomUUID(), model: '<synthetic>', role: 'assistant', stop_reason: 'stop_sequence', type: 'message', content: [{ type: 'text', text: 'Failed to authenticate: OAuth session expired and could not be refreshed' }] }, parent_tool_use_id: null, ...base, uuid: randomUUID(), error: 'authentication_failed', is_api_error_message: true });
    out({ type: 'result', subtype: 'success', is_error: true, num_turns: 1, result: 'Failed to authenticate: OAuth session expired and could not be refreshed', ...base, total_cost_usd: 0, terminal_reason: 'api_error', uuid: randomUUID() });
    return 1;
  }

  const mcp = await startMcp(flag('--mcp-config'));
  out({ type: 'system', subtype: 'init', cwd: process.cwd(), session_id: session, tools: mcp.tools.map((t) => `mcp__pixellab__${t}`), mcp_servers: [{ name: 'pixellab', status: mcp.status }], model: 'claude-fable-5-1', permissionMode: 'default', apiKeySource: 'none', claude_code_version: VERSION });

  const vars = contextOf(message);
  const turns = JSON.parse(readFileSync(join(here, 'turns.json'), 'utf8'));
  const play = turns.find((/** @type {any} */ t) => new RegExp(t.match, 'i').test(vars.text) && (!t.needsSelection || vars.items.length || vars.groups.length)) ?? turns.at(-1);
  const pace = Number(process.env.FAKE_CLAUDE_STEP_MS ?? 60);
  /** @type {Record<string, any>} */
  const results = {};
  let last = '';
  let turnsUsed = 1;
  for (const step of play.steps) {
    await sleep(pace);
    if (step.wait) await sleep(step.wait);
    if (step.say) {
      last = fill(step.say, vars, results);
      out({ type: 'assistant', message: msg([{ type: 'text', text: last }]), parent_tool_use_id: null, ...base, uuid: randomUUID() });
    }
    const calls = step.each === 'items' ? vars.items.map((/** @type {any} */ it) => ({ ...vars, item: it.path, object: it.object })) : step.tool || step.ask ? [vars] : [];
    for (const v of calls) {
      const name = step.ask ? 'chat_ask' : step.tool;
      const input = step.ask ? { question: fill(step.ask, v, results), options: step.options.map((/** @type {string} */ o) => fill(o, v, results)) } : fillAll(step.input ?? {}, v, results);
      const id = `toolu_${randomUUID().replace(/-/g, '').slice(0, 24)}`;
      out({ type: 'assistant', message: msg([{ type: 'tool_use', id, name: `mcp__pixellab__${name}`, input }]), parent_tool_use_id: null, ...base, uuid: randomUUID() });
      const r = await mcp.call(name, input);
      results[step.as ?? name] = r.structuredContent;
      out({ type: 'user', message: { role: 'user', content: [{ tool_use_id: id, type: 'tool_result', content: r.content, ...(r.isError ? { is_error: true } : {}) }] }, parent_tool_use_id: null, ...base, uuid: randomUUID() });
      turnsUsed++;
    }
  }
  mcp.close();
  writeFileSync(join(stateDir, 'sessions', `${session}.json`), JSON.stringify({ session, last: message.slice(0, 200) }));
  out({ type: 'result', subtype: 'success', is_error: false, duration_ms: 0, num_turns: turnsUsed, result: last, ...base, total_cost_usd: 0, uuid: randomUUID() });
  return 0;
}

function refuse(/** @type {string} */ why) {
  console.error(`fake claude: refusing the turn: ${why} (spec D1)`);
  return 2;
}

const msg = (/** @type {any[]} */ content) => ({ id: `msg_${randomUUID().slice(0, 8)}`, type: 'message', role: 'assistant', model: 'claude-fable-5-1', content, stop_reason: null, usage: { input_tokens: 0, output_tokens: 0 } });

/** The one stream-json user message on stdin, as text. */
async function readMessage() {
  let text = '';
  for await (const chunk of process.stdin) text += chunk;
  const line = text.split('\n').find((l) => l.trim());
  if (!line) return '';
  const m = JSON.parse(line);
  const c = m?.message?.content;
  return typeof c === 'string' ? c : Array.isArray(c) ? c.filter((b) => b.type === 'text').map((b) => b.text).join('\n') : '';
}

/** What the message's selection block names, and the text after it. */
function contextOf(/** @type {string} */ message) {
  const end = message.indexOf('</selection>');
  const block = end < 0 ? '' : message.slice(0, end);
  const text = (end < 0 ? message : message.slice(end + '</selection>'.length)).trim();
  const scene = /scene="([^"]+)"/.exec(block)?.[1] ?? '';
  const object = /object="([^"]+)"/.exec(block)?.[1] ?? '';
  const items = [...block.matchAll(/^ {2}item (\S+) object=(\S+)/gm)].map((m) => ({ path: m[1], object: m[2] }));
  const groups = [...block.matchAll(/^ {2}group (\S+) "((?:[^"\\]|\\.)*)" items=([^\s]*)/gm)].map((m) => ({ id: m[1], name: m[2], items: m[3].split(',').filter(Boolean) }));
  const first = items[0]?.object ?? object;
  const names = [...groups.map((g) => g.name), ...items.map((i) => `${i.object} ${i.path}`)];
  return { text, scene, object, items, groups, first, count: items.length + groups.reduce((n, g) => n + g.items.length, 0), names: names.length ? listed(names) : '' };
}

const listed = (/** @type {string[]} */ names) => (names.length === 1 ? names[0] : `${names.slice(0, -1).join(', ')} and ${names.at(-1)}`);

/** {{scene}}, {{item}}, {{first}}, {{count}}, {{names}}, {{object_show.size.1+4}}, {{uses}}, {{proposal}}. */
function fill(/** @type {string} */ s, /** @type {any} */ v, /** @type {Record<string, any>} */ results) {
  const whole = /^\{\{([^}]+)\}\}$/.exec(s);
  const value = (/** @type {string} */ key) => {
    const [path, add] = key.split('+');
    if (path === 'uses') return (results.scene_items?.items ?? []).filter((/** @type {any} */ i) => i.object === v.first).length;
    if (path === 'proposal') return results.doc_save?.number ? `#${results.doc_save.number}` : 'a proposal';
    let x = path in v ? v[path] : path.split('.').reduce((o, k) => o?.[k], results);
    if (add) x = Number(x) + Number(add);
    return x ?? '';
  };
  if (whole) return value(whole[1]);
  return s.replace(/\{\{([^}]+)\}\}/g, (_, key) => String(value(key)));
}

function fillAll(/** @type {any} */ input, /** @type {any} */ v, /** @type {Record<string, any>} */ results) {
  if (typeof input === 'string') return fill(input, v, results);
  if (Array.isArray(input)) return input.map((x) => fillAll(x, v, results));
  if (input && typeof input === 'object') return Object.fromEntries(Object.entries(input).map(([k, x]) => [k, fillAll(x, v, results)]));
  return input;
}

/** The pixellab MCP server from --mcp-config (a JSON string or a file), initialized. */
async function startMcp(/** @type {string | undefined} */ config) {
  const text = config && existsSync(config) ? readFileSync(config, 'utf8') : config;
  const server = JSON.parse(text ?? '{}').mcpServers?.pixellab;
  if (!server) return { status: 'failed', tools: [], call: async () => ({ isError: true, content: [{ type: 'text', text: 'no pixellab server' }] }), close() {} };
  const child = spawn(server.command, server.args ?? [], { env: { ...process.env, ...(server.env ?? {}) }, stdio: ['pipe', 'pipe', 'inherit'] });
  const waiting = new Map();
  createInterface({ input: child.stdout }).on('line', (l) => {
    const m = JSON.parse(l);
    waiting.get(m.id)?.(m);
  });
  let id = 0;
  const rpc = (/** @type {string} */ method, /** @type {any} */ params) =>
    new Promise((res) => {
      const i = ++id;
      waiting.set(i, res);
      child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id: i, method, params })}\n`);
    });
  await rpc('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'claude-code', version: VERSION } });
  child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' })}\n`);
  const tools = (await rpc('tools/list', {})).result.tools.map((/** @type {any} */ t) => t.name);
  return {
    status: 'connected',
    tools,
    async call(/** @type {string} */ name, /** @type {any} */ input) {
      const r = await rpc('tools/call', { name, arguments: input });
      return r.result ?? { isError: true, content: [{ type: 'text', text: r.error?.message ?? 'error' }] };
    },
    close() {
      child.stdin.end();
    },
  };
}

// Last, so every helper above is defined before a turn runs.
if (args[0] === '--version' || args[0] === '-v') {
  console.log(`${VERSION} (Claude Code)`);
} else if (args[0] === 'auth') {
  await auth(args[1]);
} else if (args.includes('-p') || args.includes('--print')) {
  process.exitCode = await turn();
} else {
  console.error('fake claude: only --version, auth and -p turns are played');
  process.exitCode = 2;
}
