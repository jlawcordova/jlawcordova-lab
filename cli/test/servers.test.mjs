// The two servers as child processes: `pixellab serve --stdio` (the app's
// JSON-RPC session server, with its notifications) and `pixellab serve --mcp`
// (MCP for agents, whose saves become proposals). Every stdout line must be
// JSON: startServer() parses each one and fails on anything else.

import assert from 'node:assert/strict';
import { appendFile, readdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { after, before, describe, test } from 'node:test';

import { createState } from '../lib/state.mjs';
import { decodePng, makeDocument, makeSite, needsSite, readDoc, runCli, startServer, tempDir } from './helpers.mjs';

describe('serve --stdio', needsSite, () => {
  /** @type {Awaited<ReturnType<typeof makeSite>>} */
  let site;
  /** @type {string} */
  let state;
  /** @type {ReturnType<typeof startServer>} */
  let server;
  before(async () => {
    site = await makeSite();
    state = await tempDir('pixellab-state-');
    server = startServer(['--stdio', '--site', site.root], { env: site.env({ PIXELLAB_STATE: state }) });
  });
  after(async () => {
    server.child.kill();
    await site.cleanup();
  });

  test('session.info', async () => {
    const { result } = await server.request('session.info');
    assert.equal(result.name, 'pixellab');
    assert.equal(result.protocol, 1);
    assert.equal(result.site, join(site.root, 'src/assets/pixel-art/source'), 'site is the document root (the legacy source folder here)');
    assert.deepEqual(result.document, { root: result.site, name: 'site', kind: 'legacy-site', publishDir: join(site.root, 'src/assets/pixel-art'), siteRoot: site.root });
    assert.equal(result.stateDir, state);
    assert.equal(result.saves, 'direct');
    assert.deepEqual(result.docs, []);
  });

  test('a render: png, scale, viewBox, size and item bounds', async () => {
    const { result } = await server.request('scene.render', { name: 'footer-grass', scale: 2 });
    const png = decodePng(Buffer.from(result.png, 'base64'));
    assert.deepEqual([png.width, png.height], [result.width, result.height]);
    assert.equal(result.scale, 2);
    assert.equal(result.items.length, 1);
    assert.deepEqual(Object.keys(result.items[0]).sort(), ['at', 'bounds', 'object', 'path']);
  });

  test('place → undo → redo → save, with doc.changed after each', async () => {
    const file = join(site.sourceDir, 'scenes/footer-grass.mjs');
    const disk = await readFile(file, 'utf8');
    let since = server.notifications.length;
    const placed = await server.request('scene.place', { name: 'footer-grass', object: 'pine', at: [0, 0, 0] });
    assert.equal(placed.result.changed, true);
    const changed = (/** @type {any} */ n) => n.method === 'doc.changed';
    assert.deepEqual((await server.waitFor(changed, { since })).params, { doc: 'scene:footer-grass', dirty: true, canUndo: true, canRedo: false });
    assert.equal(await readFile(file, 'utf8'), disk, 'nothing written before doc.save');

    since = server.notifications.length;
    const undo = await server.request('doc.undo', { doc: 'scene:footer-grass' });
    assert.match(undo.result.summary, /^Undid: pine placed/);
    assert.deepEqual((await server.waitFor(changed, { since })).params, { doc: 'scene:footer-grass', dirty: false, canUndo: false, canRedo: true });

    since = server.notifications.length;
    await server.request('doc.redo', { doc: 'scene:footer-grass' });
    assert.deepEqual((await server.waitFor(changed, { since })).params, { doc: 'scene:footer-grass', dirty: true, canUndo: true, canRedo: false });

    const status = await server.request('doc.status', { doc: 'scene:footer-grass' });
    assert.deepEqual(
      { ...status.result, undo: status.result.undo.length },
      { doc: 'scene:footer-grass', dirty: true, canUndo: true, canRedo: false, file: 'scenes/footer-grass.mjs', exists: true, stale: false, undo: 1, redo: [], edits: ['pine placed at column 0, row 0, level 0'], unsavedInApp: false },
    );

    since = server.notifications.length;
    const saved = await server.request('doc.save', { doc: 'scene:footer-grass' });
    assert.equal(saved.result.wrote, true);
    assert.deepEqual((await server.waitFor(changed, { since })).params, { doc: 'scene:footer-grass', dirty: false, canUndo: true, canRedo: false });
    // The session sorted the scene as part of the edit, so the pine is where it stands, not last.
    assert.deepEqual((await readDoc(file)).items.find((/** @type {any} */ i) => i.object === 'pine'), { object: 'pine', at: { tile: [0, 0, 0] } });
    const info = await server.request('session.info');
    assert.deepEqual(info.result.docs, [{ doc: 'scene:footer-grass', dirty: false, canUndo: true, canRedo: false }]);
  });

  test('errors: unknown method, bad params, a failed command, a line that is not JSON', async () => {
    const unknown = await server.request('scene.nope');
    assert.equal(unknown.error.code, -32601);
    assert.equal(unknown.error.data.code, 'not_found');
    const usage = await server.request('scene.place', { name: 'footer-grass' });
    assert.equal(usage.error.code, -32602);
    assert.equal(usage.error.data.code, 'usage');
    assert.match(usage.error.message, /object is required/);
    const failed = await server.request('scene.show', { name: 'no-such-scene' });
    assert.equal(failed.error.code, -32000);
    assert.equal(failed.error.data.code, 'not_found');
    const count = server.lines.length;
    server.send('this is not json');
    const ping = await server.request('session.info');
    assert.ok(ping.result);
    const parseError = server.lines.slice(count).find((l) => l.error?.code === -32700);
    assert.ok(parseError, 'a parse error response');
    assert.equal(parseError.id, null);
  });

  test('files.changed when a source changes on disk, and a clean open doc reloads', async () => {
    await server.request('doc.open', { doc: 'object:pine' });
    const file = join(site.sourceDir, 'objects/pine.mjs');
    const since = server.notifications.length;
    const text = (await readFile(file, 'utf8')).replace("keys: {\n", "keys: {\n    z: 'ink',\n");
    await writeFile(file, text);
    const n = await server.waitFor((m) => m.method === 'files.changed' && m.params.files.includes('objects/pine.mjs'), { since });
    assert.ok(n);
    const reloaded = await server.waitFor((m) => m.method === 'doc.changed' && m.params.doc === 'object:pine', { since });
    assert.equal(reloaded.params.dirty, false);
    const shown = await server.request('object.show', { name: 'pine', doc: true });
    assert.equal(shown.result.doc.keys.z, 'ink');
  });

  test('proposals.changed and activity.appended when the state dir changes', async () => {
    const since = server.notifications.length;
    const store = createState(state);
    const disk = await readFile(join(site.sourceDir, 'scenes/footer-dirt.mjs'), 'utf8');
    await store.writeProposal({ id: '20261019-120000-abcdef', doc: 'scene:footer-dirt', baseHash: null, source: disk, summary: ['nothing'], createdAt: new Date().toISOString(), agent: 'test' });
    const p = await server.waitFor((m) => m.method === 'proposals.changed', { since });
    assert.deepEqual(p.params.proposals.map((/** @type {any} */ x) => x.id), ['20261019-120000-abcdef']);
    await appendFile(join(state, 'activity.jsonl'), `${JSON.stringify({ time: 't', tool: 'scene_list', args: {}, ok: true, summary: 's' })}\n`);
    const a = await server.waitFor((m) => m.method === 'activity.appended', { since });
    assert.deepEqual(a.params.entries, [{ time: 't', tool: 'scene_list', args: {}, ok: true, summary: 's' }]);
  });

  test('shutdown answers, then the server exits 0', async () => {
    const r = await server.request('shutdown');
    assert.deepEqual(r.result, { ok: true });
    assert.equal(await server.exited, 0);
  });
});

describe('serve --mcp', needsSite, () => {
  /** @type {Awaited<ReturnType<typeof makeSite>>} */
  let site;
  /** @type {string} */
  let state;
  /** @type {ReturnType<typeof startServer>} */
  let server;
  /** @type {string} */
  let sceneBefore;
  before(async () => {
    site = await makeSite();
    state = await tempDir('pixellab-state-');
    sceneBefore = await readFile(join(site.sourceDir, 'scenes/footer-grass.mjs'), 'utf8');
    server = startServer(['--mcp', '--site', site.root], { env: site.env({ PIXELLAB_STATE: state }) });
  });
  after(async () => {
    server.child.kill();
    await site.cleanup();
  });

  const call = async (/** @type {string} */ name, /** @type {any} */ args) => (await server.request('tools/call', { name, arguments: args })).result;
  const activity = async () => (await readFile(join(state, 'activity.jsonl'), 'utf8')).trim().split('\n').map((l) => JSON.parse(l));

  test('initialize, notifications/initialized, ping', async () => {
    const init = await server.request('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'test-agent', version: '9.9' } });
    assert.equal(init.result.protocolVersion, '2025-06-18');
    assert.deepEqual(init.result.capabilities, { tools: { listChanged: false } });
    assert.equal(init.result.serverInfo.name, 'pixellab');
    assert.equal(typeof init.result.instructions, 'string');
    const count = server.lines.length;
    server.notify('notifications/initialized');
    assert.deepEqual((await server.request('ping')).result, {});
    assert.equal(server.lines.length, count + 1, 'no response to a notification');
  });

  test('tools/list: the commands as tools, with input schemas', async () => {
    const { tools } = (await server.request('tools/list')).result;
    const names = tools.map((/** @type {any} */ t) => t.name);
    for (const name of ['scene_place', 'scene_render', 'object_paint', 'object_layer_add', 'doc_save', 'doc_undo', 'proposal_list', 'check']) assert.ok(names.includes(name), name);
    for (const name of ['proposal_apply', 'proposal_reject', 'art_build']) assert.ok(!names.includes(name), name);
    for (const t of tools) {
      assert.match(t.name, /^[a-z_]+$/);
      assert.equal(t.inputSchema.type, 'object');
      assert.equal(typeof t.description, 'string');
    }
    const place = tools.find((/** @type {any} */ t) => t.name === 'scene_place');
    assert.deepEqual(place.inputSchema.required, ['name', 'object']);
    assert.deepEqual(place.inputSchema.properties.at.items, { type: 'integer' });
    assert.match(tools.find((/** @type {any} */ t) => t.name === 'doc_save').description, /writes a proposal/);
  });

  test('tools/call: a render comes back as an image', async () => {
    const r = await call('scene_render', { name: 'footer-grass', scale: 2 });
    assert.equal(r.isError, undefined);
    assert.equal(r.content[0].type, 'image');
    assert.equal(r.content[0].mimeType, 'image/png');
    assert.ok(decodePng(Buffer.from(r.content[0].data, 'base64')).width > 0);
    assert.equal(r.content[1].type, 'text');
    assert.equal(r.structuredContent.png, undefined);
    assert.equal(r.structuredContent.scale, 2);
  });

  test('tools/call: edit, then doc_save writes a proposal and leaves the site alone', async () => {
    const placed = await call('scene_place', { name: 'footer-grass', object: 'pine', at: [0, 0, 0] });
    assert.equal(placed.isError, undefined);
    assert.equal(placed.structuredContent.changed, true);
    assert.deepEqual(JSON.parse(placed.content[0].text), placed.structuredContent);
    const saved = await call('doc_save', { doc: 'scene:footer-grass' });
    assert.equal(saved.isError, undefined, saved.content[0].text);
    const { proposal } = saved.structuredContent;
    const files = await readdir(join(state, 'proposals'));
    assert.deepEqual(files, [`${proposal}.json`]);
    const p = JSON.parse(await readFile(join(state, 'proposals', files[0]), 'utf8'));
    // A number (#n) too (docs/intents/2026-10-agent-chat/spec.md, R24); no chat from a terminal.
    assert.deepEqual(Object.keys(p), ['id', 'number', 'doc', 'baseHash', 'source', 'summary', 'createdAt', 'agent']);
    assert.equal(p.doc, 'scene:footer-grass');
    assert.equal(p.agent, 'test-agent 9.9');
    assert.deepEqual(p.summary, ['pine placed at column 0, row 0, level 0']);
    assert.match(p.source, /\{ object: 'pine', at: \{ tile: \[0, 0, 0\] \} \}/);
    assert.equal(await readFile(join(site.sourceDir, 'scenes/footer-grass.mjs'), 'utf8'), sceneBefore, 'the site is untouched');
  });

  test('tools/call errors: a failed command and bad arguments are isError results; an unknown tool is a protocol error', async () => {
    const failed = await call('scene_place', { name: 'footer-grass', object: 'no-such-object' });
    assert.equal(failed.isError, true);
    assert.match(failed.content[0].text, /no object named no-such-object/);
    assert.equal(failed.structuredContent.error.code, 'not_found');
    const bad = await call('scene_place', { name: 'footer-grass', object: 'pine', at: 'here' });
    assert.equal(bad.isError, true);
    assert.equal(bad.structuredContent.error.code, 'usage');
    const unknown = await server.request('tools/call', { name: 'proposal_apply', arguments: {} });
    assert.equal(unknown.error.code, -32602);
    assert.match(unknown.error.message, /for a person/);
  });

  test('every tools/call is in activity.jsonl', async () => {
    const entries = await activity();
    assert.deepEqual(
      entries.map((e) => [e.tool, e.ok]),
      [
        ['scene_render', true],
        ['scene_place', true],
        ['doc_save', true],
        ['scene_place', false],
        ['scene_place', false],
        ['proposal_apply', false],
      ],
    );
    // The client's name and version too (docs/intents/2026-10-agent-chat/spec.md, R24); no chat from a terminal.
    for (const e of entries) assert.deepEqual(Object.keys(e), ['time', 'tool', 'args', 'ok', 'summary', 'agent']);
    assert.equal(entries[1].summary, 'pine placed at column 0, row 0, level 0');
    assert.deepEqual(entries[1].args, { name: 'footer-grass', object: 'pine', at: [0, 0, 0] });
  });

  test('the server exits when its input ends', async () => {
    assert.equal(await server.close(), 0);
    assert.equal(server.stderr, '');
  });
});

describe('serve --mcp --direct', needsSite, () => {
  test('doc_save writes the site, and art_build is a tool', async () => {
    const site = await makeSite();
    const state = await tempDir('pixellab-state-');
    const server = startServer(['--mcp', '--direct', '--site', site.root], { env: site.env({ PIXELLAB_STATE: state }) });
    try {
      await server.request('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 't', version: '1' } });
      const names = (await server.request('tools/list')).result.tools.map((/** @type {any} */ t) => t.name);
      assert.ok(names.includes('art_build'));
      assert.ok(!names.includes('proposal_apply'));
      await server.request('tools/call', { name: 'scene_place', arguments: { name: 'footer-grass', object: 'pine', at: [1, 0, 0] } });
      const saved = (await server.request('tools/call', { name: 'doc_save', arguments: { doc: 'scene:footer-grass' } })).result;
      assert.equal(saved.structuredContent.wrote, true);
      assert.deepEqual((await readDoc(join(site.sourceDir, 'scenes/footer-grass.mjs'))).items.find((/** @type {any} */ i) => i.object === 'pine'), { object: 'pine', at: { tile: [1, 0, 0] } });
      assert.deepEqual(await readdir(join(state, 'proposals')).catch(() => []), []);
    } finally {
      server.child.kill();
      await site.cleanup();
    }
  });
});

describe('servers and disk (docs/intents/2026-10-mcp-gaps)', () => {
  test('R5, R7: the stdio server sends doc.stale for a dirty doc whose file changed, and keeps open/<pid>.json', async () => {
    const doc = await makeDocument();
    const state = await tempDir('pixellab-state-');
    const server = startServer(['--stdio', '--document', doc.root], { env: { ...doc.env, PIXELLAB_STATE: state } });
    try {
      const info = (await server.request('session.info')).result;
      const openFile = join(state, 'open', `${info.pid}.json`);
      await server.request('scene.place', { name: 'sample', object: 'pebble', at: [3, 3, 0] });
      const listed = JSON.parse(await readFile(openFile, 'utf8'));
      assert.deepEqual([listed.pid, listed.document, listed.docs], [info.pid, doc.root, [{ doc: 'scene:sample', dirty: true }]]);
      const since = server.notifications.length;
      const file = join(doc.root, 'scenes/sample.mjs');
      await writeFile(file, (await readFile(file, 'utf8')).replace('tile: [0, 0, 1]', 'tile: [0, 0, 2]'));
      const stale = await server.waitFor((m) => m.method === 'doc.stale', { since });
      assert.deepEqual(stale.params, { doc: 'scene:sample', file: 'scenes/sample.mjs' });
      const merged = await server.request('doc.merge', { doc: 'scene:sample' });
      assert.deepEqual([merged.result.merged, merged.result.dirty], [true, true]);
      await server.request('doc.close', { doc: 'scene:sample', force: true });
      assert.deepEqual(JSON.parse(await readFile(openFile, 'utf8')).docs, [], 'closing updates the list');
      await server.request('shutdown');
      assert.equal(await server.exited, 0);
      await assert.rejects(readFile(openFile, 'utf8'), { code: 'ENOENT' }, 'removed when the server stops');
    } finally {
      server.child.kill();
      await doc.cleanup();
    }
  });

  test('R1, R3: an MCP session settles its applied proposal before the next call; doc_reload and doc_revert annotations', async () => {
    const doc = await makeDocument();
    const state = await tempDir('pixellab-state-');
    const env = { ...doc.env, PIXELLAB_STATE: state };
    const server = startServer(['--mcp', '--document', doc.root], { env });
    const call = async (/** @type {string} */ name, /** @type {any} */ args) => (await server.request('tools/call', { name, arguments: args })).result.structuredContent;
    try {
      await server.request('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 't', version: '1' } });
      const tools = (await server.request('tools/list')).result.tools;
      const tool = (/** @type {string} */ n) => tools.find((/** @type {any} */ t) => t.name === n);
      assert.equal(tool('doc_revert').annotations.destructiveHint, true);
      assert.deepEqual([tool('doc_reload').annotations.destructiveHint, tool('doc_reload').annotations.readOnlyHint], [false, false]);
      assert.ok(tool('doc_merge'));
      await call('scene_place', { name: 'sample', object: 'pebble', at: [3, 3, 0] });
      const saved = await call('doc_save', { doc: 'scene:sample' });
      const applied = await runCli(['proposal', 'apply', saved.proposal, '--document', doc.root], { env });
      assert.equal(applied.code, 0, applied.stderr);
      const status = await call('doc_status', { doc: 'scene:sample' });
      assert.deepEqual([status.dirty, status.stale, status.synced.action, status.unsavedInApp], [false, false, 'clean', false]);
      assert.equal((await call('doc_reload', { doc: 'scene:sample' })).reloaded, false);
    } finally {
      server.child.kill();
      await doc.cleanup();
    }
  });
});

describe('serve usage', () => {
  test('serve needs --stdio or --mcp', async () => {
    const { spawnCli } = await import('./helpers.mjs');
    const r = await spawnCli(['serve']);
    assert.equal(r.code, 2);
    assert.match(r.stderr, /give --stdio or --mcp/);
    const help = await spawnCli(['serve', '--help']);
    assert.equal(help.code, 0);
    assert.match(help.stdout, /claude mcp add pixellab -- node/);
  });
});
