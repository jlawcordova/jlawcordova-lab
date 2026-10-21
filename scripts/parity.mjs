#!/usr/bin/env node
// The parity check (docs/intents/2026-10-rust-engine/spec.md, R3; plan.md
// step 8): the Node CLI and the Rust program over the corpus, the Node CLI
// on each document as it is (format 1), the Rust program on a copy its own
// `document migrate --to` made. Removed at cutover, with the Node CLI.
//
//   node scripts/parity.mjs [--program <pixellab>] [--docs sample,harbor,town,owner]
//                           [--site <checkout>] [--report <file.md>] [--keep <dir>]
//
// For each document it compares:
//   - every scene's SVG that art build writes, byte for byte;
//   - every preview PNG (art preview), decoded: dimensions and RGBA;
//   - every read command's JSON result (the invocations goldens.mjs uses),
//     ignoring ms, ids and times, the documents' paths and .mjs/.json, with
//     gzip sizes within 1% (spec D2: miniz_oxide isn't zlib);
//   - the validation problems (check), apart from the file extension;
// and, on the fixture:
//   - serve --mcp's initialize and tools/list, byte for byte;
//   - an edit replay: each mutating command once, through both CLIs, then
//     every scene's SVG and scene show / object show compared;
//   - the human output and exit code of every read command, and of failing calls.
// It writes a report and exits 1 on any difference.

import { execFileSync, spawn, spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import { inflateSync } from 'node:zlib';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const NODE_CLI = join(REPO, 'cli/bin/pixellab.mjs');

const args = process.argv.slice(2);
const flag = (/** @type {string} */ name, /** @type {string | null} */ fallback = null) => {
  const i = args.indexOf(name);
  return i < 0 ? fallback : (args[i + 1] ?? fallback);
};
const PROGRAM = resolve(flag('--program', join(REPO, 'target/release/pixellab')));
const SITE = flag('--site', join(homedir(), 'Projects', 'jlawcordova.github.io'));
const WANT = (flag('--docs', 'sample,harbor,town,owner') ?? '').split(',').filter(Boolean);
const REPORT = flag('--report');
const KEEP = flag('--keep');

const SOURCES = {
  sample: join(REPO, 'cli/test/fixtures/sample.pixellab'),
  harbor: join(REPO, 'examples/harbor.pixellab'),
  town: join(REPO, 'examples/town.pixellab'),
};

/** @type {{ doc: string, what: string, detail: string }[]} */
const differences = [];
/** @type {string[]} */
const lines = [];
const differ = (/** @type {string} */ doc, /** @type {string} */ what, /** @type {string} */ detail) => differences.push({ doc, what, detail });
const note = (/** @type {string} */ text) => {
  lines.push(text);
  process.stderr.write(`${text}\n`);
};

// ------------------------------------------------------------------ the doors

/** A clean env: a temp HOME and state dir, no document from the environment. */
const envFor = (/** @type {string} */ base) => {
  const env = { ...process.env, HOME: join(base, 'home'), PIXELLAB_STATE: join(base, 'state') };
  delete env.PIXELLAB_DOC;
  delete env.PIXELLAB_SITE;
  mkdirSync(env.HOME, { recursive: true });
  return env;
};

/** Runs a CLI: { status, stdout, stderr }. */
function run(/** @type {string[]} */ cmd, /** @type {Record<string, string | undefined>} */ env) {
  const r = spawnSync(cmd[0], cmd.slice(1), { env, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  return { status: r.status ?? -1, stdout: r.stdout ?? '', stderr: r.stderr ?? '' };
}
const nodeCli = (/** @type {string[]} */ argv, /** @type {any} */ env) => run(['node', NODE_CLI, ...argv], env);
const rustCli = (/** @type {string[]} */ argv, /** @type {any} */ env) => run([PROGRAM, ...argv], env);

/** A JSON-RPC server: call(method, params) → result, or { error }. */
function server(/** @type {string[]} */ cmd, /** @type {any} */ env) {
  const child = spawn(cmd[0], cmd.slice(1), { env, stdio: ['pipe', 'pipe', 'inherit'] });
  const lines = createInterface({ input: /** @type {any} */ (child.stdout) });
  /** @type {Map<number, (v: any) => void>} */
  const waiting = new Map();
  /** @type {string[]} */
  const raw = [];
  lines.on('line', (line) => {
    raw.push(line);
    const msg = JSON.parse(line);
    if (msg.id !== undefined && waiting.has(msg.id)) {
      /** @type {(v: any) => void} */ (waiting.get(msg.id))(msg);
      waiting.delete(msg.id);
    }
  });
  let id = 0;
  return {
    raw,
    /** @param {string} method @param {any} [params] */
    call(method, params = {}) {
      const n = ++id;
      return new Promise((done) => {
        waiting.set(n, done);
        /** @type {any} */ (child.stdin).write(`${JSON.stringify({ jsonrpc: '2.0', id: n, method, params })}\n`);
      });
    },
    /** @param {string} method @param {any} [params] */
    notify(method, params) {
      /** @type {any} */ (child.stdin).write(`${JSON.stringify({ jsonrpc: '2.0', method, ...(params ? { params } : {}) })}\n`);
    },
    close() {
      /** @type {any} */ (child.stdin).end();
      return new Promise((done) => child.on('close', done));
    },
  };
}

// -------------------------------------------------------------------- compare

/** RGBA of a PNG (8-bit RGBA or RGB, any filter), as hex digest material. */
function decodePng(/** @type {Buffer} */ png) {
  let pos = 8;
  let width = 0;
  let height = 0;
  let channels = 4;
  /** @type {Buffer[]} */
  const idat = [];
  while (pos < png.length) {
    const len = png.readUInt32BE(pos);
    const type = png.toString('latin1', pos + 4, pos + 8);
    const data = png.subarray(pos + 8, pos + 8 + len);
    if (type === 'IHDR') {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      channels = data[9] === 6 ? 4 : 3;
    } else if (type === 'IDAT') idat.push(data);
    pos += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const row = width * channels;
  const out = Buffer.alloc(row * height);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (row + 1)];
    for (let i = 0; i < row; i++) {
      const x = raw[y * (row + 1) + 1 + i];
      const a = i >= channels ? out[y * row + i - channels] : 0;
      const b = y > 0 ? out[(y - 1) * row + i] : 0;
      const c = y > 0 && i >= channels ? out[(y - 1) * row + i - channels] : 0;
      const p = a + b - c;
      const pa = Math.abs(p - a);
      const pb = Math.abs(p - b);
      const pc = Math.abs(p - c);
      const paeth = pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      out[y * row + i] = (x + [0, a, b, (a + b) >> 1, paeth][filter]) & 255;
    }
  }
  return `<png ${width}×${height} ${channels === 4 ? out.toString('base64').length : 'rgb'}:${hash(out)}>`;
}

/** A short hash (FNV-1a over the bytes, as hex). */
function hash(/** @type {Buffer} */ bytes) {
  let h = 0x811c9dc5;
  for (const b of bytes) h = Math.imul(h ^ b, 0x01000193) >>> 0;
  return h.toString(16);
}

/** Paths and extensions out, PNGs decoded, ms, node, ids and times dropped, gzip sizes kept apart. */
function normalize(/** @type {any} */ v, /** @type {[string, string][]} */ roots, /** @type {boolean} */ node, /** @type {{ path: string, node: number }[]} */ gz, path = '') {
  if (typeof v === 'string') {
    let s = v;
    for (const [from, to] of roots) s = s.split(from).join(to);
    if (node) s = s.split('.mjs').join('.json');
    // Reports print gzip sizes in KB: those are compared within 1% with the numbers.
    return s.replace(/\d+\.\d KB gzip/g, '<gz> KB gzip').replace(/\d+\.\d KB \/ \d+\.\d KB gzip/g, '<sizes>');
  }
  if (Array.isArray(v)) return v.map((e, i) => normalize(e, roots, node, gz, `${path}[${i}]`));
  if (v && typeof v === 'object') {
    /** @type {Record<string, any>} */
    const out = {};
    for (const [k, e] of Object.entries(v)) {
      if (['ms', 'node', 'createdAt', 'time', 'version'].includes(k)) continue;
      if ((k === 'png' || k === 'focusPng') && typeof e === 'string') out[k] = decodePng(Buffer.from(e, 'base64'));
      else if (k === 'gzip' && typeof e === 'number') {
        gz.push({ path: `${path}.${k}`, node: e });
        out[k] = '<gzip>';
      } else out[k] = normalize(e, roots, node, gz, `${path}.${k}`);
    }
    return out;
  }
  return v;
}

/** Compares two results; returns a difference's text or null. */
function compare(/** @type {any} */ nodeResult, /** @type {any} */ rustResult, /** @type {[string, string][]} */ nodeRoots, /** @type {[string, string][]} */ rustRoots) {
  /** @type {{ path: string, node: number }[]} */
  const gzNode = [];
  /** @type {{ path: string, node: number }[]} */
  const gzRust = [];
  const a = JSON.stringify(normalize(nodeResult, nodeRoots, true, gzNode));
  const b = JSON.stringify(normalize(rustResult, rustRoots, false, gzRust));
  if (a !== b) {
    const at = [...a].findIndex((ch, i) => ch !== b[i]);
    return `node …${a.slice(Math.max(0, at - 100), at + 150)}…\n  rust …${b.slice(Math.max(0, at - 100), at + 150)}…`;
  }
  // gzip within 1% of the larger of the value and the total it's part of.
  const total = Math.max(0, ...gzNode.map((g) => g.node));
  for (let i = 0; i < gzNode.length; i++) {
    const [n, r] = [gzNode[i].node, gzRust[i].node];
    if (Math.abs(n - r) > 0.01 * Math.max(Math.abs(n), total)) return `${gzNode[i].path}: gzip ${n} (node) vs ${r} (rust), more than 1% apart`;
  }
  return null;
}

// --------------------------------------------------------------------- corpus

/** The read invocations for one document (as goldens.mjs). @param {any} sources */
function invocations(sources) {
  /** @type {{ command: string, params: Record<string, unknown> }[]} */
  const list = [];
  const add = (/** @type {string} */ command, /** @type {Record<string, unknown>} */ params = {}) => list.push({ command, params });
  for (const c of ['document.info', 'palette.get', 'scene.list', 'object.list', 'check', 'activity.list', 'proposal.list']) add(c);
  add('object.list', { kind: 'block' });
  add('art.preview', { name: 'palette' });
  for (const name of sources.objects.keys()) {
    add('check', { name, kind: 'object' });
    add('object.show', { name, doc: true });
    add('object.render', { name });
    add('object.render', { name, scale: 2, time: 450 });
    add('object.pick', { name, x: 1, y: 1 });
    add('art.preview', { name, kind: 'object' });
    add('chat.context', { object: name });
  }
  for (const [name, scene] of sources.scenes) {
    add('check', { name, kind: 'scene' });
    add('scene.show', { name, doc: true });
    add('scene.items', { name });
    add('scene.render', { name });
    add('scene.render', { name, theme: 'dark' });
    if (scene.motion) for (const time of [0, 250, Math.floor(scene.motion.duration / 3), Math.floor((scene.motion.duration * 2) / 3)]) add('scene.render', { name, time });
    add('art.preview', { name, kind: 'scene' });
    add('art.sizes', { name });
    add('chat.context', { name });
  }
  return list;
}

/** Every file under a folder, relative. */
const filesIn = (/** @type {string} */ dir) => (existsSync(dir) ? readdirSync(dir, { recursive: true }).map(String).filter((f) => !f.endsWith('/')).sort() : []);

/** The edit replay on the fixture: each mutating command once, in an order that keeps the document valid. */
const REPLAY = [
  ['scene', 'place', 'sample', 'pebble', '--at', '1,1,0'],
  ['scene', 'place', 'sample', 'pebble', '--px', '4,-6', '--level', '1', '--id', 'p2'],
  ['scene', 'move', 'sample', '0', '--by', '1,0,0'],
  ['scene', 'set', 'sample', '1', '--class', 'shine'],
  ['scene', 'duplicate', 'sample', '1'],
  ['scene', 'snap', 'sample', '2'],
  ['scene', 'group', 'set', 'sample', '0', '--attrs', '{"data-x":"1"}'],
  ['scene', 'remove', 'sample', '3'],
  ['scene', 'sort', 'sample'],
  ['scene', 'new', 'meadow'],
  ['scene', 'place', 'meadow', 'block', '--at', '0,0,0', '--id', 'b1'],
  ['scene', 'track', 'add', 'meadow', 'b1', '--duration', '1200'],
  ['scene', 'key', 'set', 'meadow', 'b1', '--time', '600', '--move', '4,2'],
  ['scene', 'key', 'move', 'meadow', 'b1', '--time', '600', '--to', '500'],
  ['scene', 'motion', 'set', 'meadow', '--duration', '1600'],
  ['scene', 'motion', 'path', 'meadow', 'b1', '--points', '0,0 8,4', '--speed', '20'],
  ['scene', 'key', 'delete', 'meadow', 'b1', '--time', '0', '--force'],
  ['scene', 'track', 'remove', 'meadow', 'b1'],
  ['scene', 'layer', 'new', 'sample', '--layer-name', 'Rocks', '--items', '0,1'],
  ['scene', 'layer', 'rename', 'sample', 'rocks', 'Stones'],
  ['scene', 'layer', 'move', 'sample', 'rocks', '--by', '0,1,0'],
  ['scene', 'layer', 'assign', 'sample', '--to', 'top', '--items', '0'],
  ['scene', 'layer', 'ungroup', 'sample', 'rocks'],
  ['object', 'new', 'stone', '--width', '6', '--height', '5'],
  ['object', 'paint', 'stone', '1,1', '2,2', '--color', 'ink'],
  ['object', 'fill', 'stone', '--x', '0', '--y', '0', '--color', 'grass-2'],
  ['object', 'erase', 'stone', '2,2'],
  ['object', 'layer', 'add', 'stone'],
  ['object', 'layer', 'duplicate', 'stone', '1'],
  ['object', 'layer', 'move', 'stone', '2', '--by', '-1'],
  ['object', 'layer', 'delete', 'stone', '1'],
  ['object', 'animate', 'stone', '--layer', '1', '--loop', 'wf'],
  ['object', 'frame', 'add', 'stone', '--layer', '1', '--frame', '0'],
  ['object', 'frame', 'duplicate', 'stone', '--layer', '1', '--frame', '1'],
  ['object', 'frame', 'move', 'stone', '--layer', '1', '--frame', '0', '--by', '1'],
  ['object', 'frame', 'delete', 'stone', '--layer', '1', '--frame', '2'],
  ['object', 'still', 'stone', '--layer', '1', '--frame', '0'],
  ['object', 'resize', 'stone', '--width', '8', '--height', '6'],
  ['object', 'footprint', 'stone', '--to', '2,1'],
  ['object', 'map', 'set', 'pebble', '.bb.', 'baab', 'caac', '.cc.', '--keys', '{"a":"grass-4","b":"grass-2","c":"ink"}'],
  ['object', 'new', 'tower', '--kind', 'block'],
];

/** The failing calls whose messages and exit codes must match. */
const FAILING = [['no-such-command'], ['scene', 'render'], ['scene', 'render', 'nope'], ['scene', 'render', 'sample', '--scale', '99'], ['object', 'show', 'nope'], ['check', 'nope']];

async function main() {
  if (!existsSync(PROGRAM)) throw new Error(`no program at ${PROGRAM}: run cargo build --release first`);
  const base = KEEP ? resolve(KEEP) : mkdtempSync(join(tmpdir(), 'pixellab-parity-'));
  mkdirSync(base, { recursive: true });
  const env = envFor(base);
  /** @type {[string, string][]} */
  const docs = [];
  for (const name of WANT) {
    const format1 = join(base, 'format1', `${name}.pixellab`);
    rmSync(format1, { recursive: true, force: true });
    mkdirSync(dirname(format1), { recursive: true });
    if (name === 'owner') {
      if (!SITE || !existsSync(SITE)) {
        note(`owner: skipped (no site checkout at ${SITE})`);
        continue;
      }
      // document copy only reads the checkout.
      const r = nodeCli(['document', 'copy', '--to', format1, '--site', SITE], env);
      if (r.status !== 0) throw new Error(`copying the owner's document failed: ${r.stderr}`);
    } else cpSync(/** @type {string} */ (SOURCES[/** @type {'sample'} */ (name)]), format1, { recursive: true });
    docs.push([name, format1]);
  }

  for (const [name, format1] of docs) {
    const format2 = join(base, 'format2', `${name}.pixellab`);
    rmSync(format2, { recursive: true, force: true });
    const migrated = rustCli(['document', 'migrate', '--to', format2, '--document', format1, '--json'], env);
    if (migrated.status !== 0) {
      differ(name, 'document migrate', migrated.stderr || migrated.stdout);
      continue;
    }
    const nodeRoots = /** @type {[string, string][]} */ ([[format1, '<root>'], [dirname(format1), '<base>']]);
    const rustRoots = /** @type {[string, string][]} */ ([[format2, '<root>'], [dirname(format2), '<base>']]);
    let compared = 0;

    // SVGs: art build into a folder each.
    const [out1, out2] = [join(base, 'svg1', name), join(base, 'svg2', name)];
    rmSync(out1, { recursive: true, force: true });
    rmSync(out2, { recursive: true, force: true });
    const b1 = nodeCli(['art', 'build', '--dir', out1, '--json', '--document', format1], env);
    const b2 = rustCli(['art', 'build', '--dir', out2, '--json', '--document', format2], env);
    if (b1.status !== b2.status) differ(name, 'art build exit code', `node ${b1.status}, rust ${b2.status}`);
    const built = compare(JSON.parse(b1.stdout || '{}'), JSON.parse(b2.stdout || '{}'), [[out1, '<out>'], ...nodeRoots], [[out2, '<out>'], ...rustRoots]);
    if (built) differ(name, 'art build result', built);
    const svgs = filesIn(out1);
    if (JSON.stringify(svgs) !== JSON.stringify(filesIn(out2))) differ(name, 'art build files', `node ${svgs.join(', ')}; rust ${filesIn(out2).join(', ')}`);
    for (const f of svgs) {
      compared++;
      if (!existsSync(join(out2, f)) || !readFileSync(join(out1, f)).equals(readFileSync(join(out2, f)))) differ(name, `SVG ${f}`, 'not byte-identical');
    }

    // Read commands: Node in-process, Rust through serve --stdio.
    const { openDocument } = await import(join(REPO, 'cli/lib/document.mjs'));
    const { registry } = await import(join(REPO, 'cli/lib/registry.mjs'));
    const { Session } = await import(join(REPO, 'cli/lib/session.mjs'));
    const nodeDoc = await openDocument({ doc: format1, env });
    const nodeSession = new Session(nodeDoc, { env });
    const { sources } = await nodeSession.context().sources();
    const rust = server([PROGRAM, 'serve', '--stdio', '--document', format2], env);
    for (const { command, params } of invocations(sources)) {
      let nodeResult;
      try {
        nodeResult = await registry.call(nodeSession.context(), command, structuredClone(params));
      } catch (err) {
        const e = /** @type {any} */ (err);
        nodeResult = { error: { code: e.code ?? 'internal', message: e.message } };
      }
      const reply = /** @type {any} */ (await rust.call(command, params));
      const rustResult = reply.error ? { error: { code: reply.error.data?.code ?? 'internal', message: reply.error.message } } : reply.result;
      compared++;
      const d = compare(nodeResult, rustResult, nodeRoots, rustRoots);
      if (d) differ(name, `${command} ${JSON.stringify(params)}`, d);
    }
    await rust.close();

    // Validation problems.
    const c1 = nodeCli(['check', '--json', '--document', format1], env);
    const c2 = rustCli(['check', '--json', '--document', format2], env);
    const problems = (/** @type {string} */ text) => (JSON.parse(text || '{}').problems ?? []).map((/** @type {string} */ p) => p.split('.mjs').join('.json'));
    if (JSON.stringify(problems(c1.stdout)) !== JSON.stringify(problems(c2.stdout))) differ(name, 'check problems', `node ${c1.stdout.slice(0, 300)}\n  rust ${c2.stdout.slice(0, 300)}`);
    note(`${name}: ${compared} outputs compared, ${svgs.length} SVGs`);
  }

  const sample = docs.find(([n]) => n === 'sample');
  if (sample) await fixtureChecks(base, env, sample[1]);

  const report = [
    '# Parity report (R3)',
    '',
    `Node CLI (format 1) vs ${PROGRAM} (format 2, migrated with its own document migrate --to).`,
    '',
    ...lines.map((l) => `- ${l}`),
    '',
    differences.length ? `**${differences.length} differences**` : '**No differences.**',
    '',
    ...differences.map((d) => `- ${d.doc}: ${d.what}\n\n  \`\`\`\n  ${d.detail.split('\n').join('\n  ')}\n  \`\`\``),
  ].join('\n');
  if (REPORT) writeFileSync(REPORT, `${report}\n`);
  process.stdout.write(`${report}\n`);
  if (!KEEP) rmSync(base, { recursive: true, force: true });
  process.exitCode = differences.length ? 1 : 0;
}

/** The MCP surface, the edit replay and the human output, on the fixture. */
async function fixtureChecks(/** @type {string} */ base, /** @type {any} */ env, /** @type {string} */ format1) {
  const format2 = join(base, 'format2', 'sample.pixellab');
  // serve --mcp: initialize and tools/list, byte for byte.
  const mcp = async (/** @type {string[]} */ cmd) => {
    const s = server(cmd, env);
    await s.call('initialize', { protocolVersion: '2025-06-18', clientInfo: { name: 'parity', version: '1' } });
    s.notify('notifications/initialized');
    await s.call('tools/list');
    await s.close();
    return s.raw;
  };
  const n = await mcp(['node', NODE_CLI, 'serve', '--mcp', '--document', format1]);
  const r = await mcp([PROGRAM, 'serve', '--mcp', '--document', format2]);
  for (let i = 0; i < Math.max(n.length, r.length); i++) {
    if (n[i] !== r[i]) {
      const at = [...(n[i] ?? '')].findIndex((ch, k) => ch !== (r[i] ?? '')[k]);
      differ('sample', `serve --mcp message ${i + 1}`, `node …${(n[i] ?? '').slice(Math.max(0, at - 120), at + 200)}…\n  rust …${(r[i] ?? '').slice(Math.max(0, at - 120), at + 200)}…`);
    }
  }
  note(`sample: serve --mcp initialize and tools/list compared (${n.length} messages)`);

  // The edit replay, each through both CLIs on its own copy.
  const [e1, e2] = [join(base, 'edit1.pixellab'), join(base, 'edit2.pixellab')];
  rmSync(e1, { recursive: true, force: true });
  rmSync(e2, { recursive: true, force: true });
  cpSync(format1, e1, { recursive: true });
  cpSync(format2, e2, { recursive: true });
  const editRoots1 = /** @type {[string, string][]} */ ([[e1, '<root>']]);
  const editRoots2 = /** @type {[string, string][]} */ ([[e2, '<root>']]);
  for (const argv of REPLAY) {
    const a = nodeCli([...argv, '--json', '--document', e1], env);
    const b = rustCli([...argv, '--json', '--document', e2], env);
    if (a.status !== b.status) differ('sample', `edit ${argv.join(' ')}`, `exit node ${a.status}, rust ${b.status}\n  node ${a.stderr.trim()}\n  rust ${b.stderr.trim()}`);
    else if (a.status === 0) {
      const strip = (/** @type {any} */ v) => {
        // A save's hash and diff are of the .mjs and .json texts, which differ by design.
        for (const s of v.saved ?? []) {
          delete s.hash;
        }
        delete v.diff;
        return v;
      };
      const d = compare(strip(JSON.parse(a.stdout)), strip(JSON.parse(b.stdout)), editRoots1, editRoots2);
      if (d) differ('sample', `edit ${argv.join(' ')}`, d);
    } else if (a.stderr.split('.mjs').join('.json').split(e1).join('<root>') !== b.stderr.split(e2).join('<root>')) {
      differ('sample', `edit ${argv.join(' ')} (error)`, `node ${a.stderr.trim()}\n  rust ${b.stderr.trim()}`);
    }
  }
  const [o1, o2] = [join(base, 'edit-svg1'), join(base, 'edit-svg2')];
  rmSync(o1, { recursive: true, force: true });
  rmSync(o2, { recursive: true, force: true });
  nodeCli(['art', 'build', '--dir', o1, '--document', e1], env);
  rustCli(['art', 'build', '--dir', o2, '--document', e2], env);
  for (const f of filesIn(o1)) if (!existsSync(join(o2, f)) || !readFileSync(join(o1, f)).equals(readFileSync(join(o2, f)))) differ('sample', `edit replay: SVG ${f}`, 'not byte-identical');
  for (const kind of ['scene', 'object']) {
    const listed = JSON.parse(nodeCli([kind, 'list', '--json', '--document', e1], env).stdout)[`${kind}s`].map((/** @type {any} */ x) => x.name);
    for (const name of listed) {
      const a = JSON.parse(nodeCli([kind, 'show', name, '--doc', '--json', '--document', e1], env).stdout || '{}');
      const b = JSON.parse(rustCli([kind, 'show', name, '--doc', '--json', '--document', e2], env).stdout || '{}');
      const d = compare(a, b, editRoots1, editRoots2);
      if (d) differ('sample', `edit replay: ${kind} show ${name}`, d);
    }
  }
  note(`sample: edit replay of ${REPLAY.length} commands compared`);

  // Human output and exit codes: every read command once, and failing calls.
  const { registry } = await import(join(REPO, 'cli/lib/registry.mjs'));
  /** @type {string[][]} */
  const reads = [];
  for (const cmd of registry.commands.values()) {
    if (cmd.mutates || cmd.session || cmd.writes || cmd.document === false) continue;
    const words = cmd.name.split('.');
    const first = cmd.args?.[0];
    const arg = first === 'name' ? (cmd.name.startsWith('scene') ? ['sample'] : cmd.name.startsWith('object') ? ['pebble'] : ['sample']) : [];
    const extra = cmd.name === 'object.pick' ? ['--x', '1', '--y', '1'] : cmd.name === 'chat.ask' ? ['Which?', '--options', 'a,b'] : [];
    reads.push([...words, ...arg, ...extra]);
  }
  for (const argv of [...reads, ...FAILING]) {
    const a = nodeCli([...argv, '--document', e1], env);
    const b = rustCli([...argv, '--document', e2], env);
    const norm = (/** @type {string} */ s, /** @type {string} */ root) => s.split(root).join('<root>').split('.mjs').join('.json').replace(/\d+\.\d KB gzip/g, '<gz> KB gzip').replace(/\d+ ms\b/g, '<ms>').replace(/v\d+\.\d+\.\d+/g, '<node>');
    if (a.status !== b.status) differ('sample', `${argv.join(' ')} exit code`, `node ${a.status}, rust ${b.status}`);
    if (norm(a.stdout, e1) !== norm(b.stdout, e2)) differ('sample', `${argv.join(' ')} stdout`, `node ${norm(a.stdout, e1).slice(0, 400)}\n  rust ${norm(b.stdout, e2).slice(0, 400)}`);
    if (norm(a.stderr, e1) !== norm(b.stderr, e2)) differ('sample', `${argv.join(' ')} stderr`, `node ${norm(a.stderr, e1).slice(0, 400)}\n  rust ${norm(b.stderr, e2).slice(0, 400)}`);
  }
  note(`sample: human output and exit codes of ${reads.length} read commands and ${FAILING.length} failing calls compared`);
  void execFileSync;
}

await main();
