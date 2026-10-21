// The Agent chat's CLI side (docs/intents/2026-10-agent-chat/spec.md): the
// selection block (chat.context, D5), questions (chat.ask, D8), and proposals
// and activity that know where they came from (D7). Everything runs on temp
// copies of the format-1 harbor example (helpers.mjs, FORMAT1_EXAMPLES).

import assert from 'node:assert/strict';
import { cp, readFile, readdir, writeFile, mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { after, before, describe, test } from 'node:test';

import { GROUP_PATHS, matchSaved, selectionContext } from '../lib/chat-context.mjs';
import { openDocument } from '../lib/document.mjs';
import { registry } from '../lib/registry.mjs';
import { Session } from '../lib/session.mjs';
import { createState } from '../lib/state.mjs';
import { FORMAT1_EXAMPLES, runCli, startServer, tempDir } from './helpers.mjs';

const HARBOR = join(FORMAT1_EXAMPLES, 'harbor.pixellab');

/** @type {string} */
let tmp;
before(async () => {
  tmp = await tempDir('pixellab-chat-');
});
after(async () => {
  const { rm } = await import('node:fs/promises');
  await rm(tmp, { recursive: true, force: true });
});

let copies = 0;
/** A temp copy of the harbor, a session on it (the app's), and a call into it. */
async function harbor(/** @type {Record<string, any>} */ options = {}) {
  const root = join(tmp, `copy-${++copies}`, 'harbor.pixellab');
  await cp(HARBOR, root, { recursive: true });
  const state = join(tmp, `state-${copies}`);
  const env = { ...process.env, HOME: tmp, PIXELLAB_STATE: state };
  const doc = await openDocument({ doc: root, env });
  const s = new Session(doc, { env, ...options });
  return { root, state, env, s, call: (/** @type {string} */ cmd, /** @type {any} */ params) => registry.call(s.context(), cmd, params) };
}

describe('chat.context: the selection block (R10)', () => {
  test('R10: a group with its saved paths, items with object, position and group, pins marked', async () => {
    const h = await harbor();
    const r = await h.call('chat.context', { name: 'harbor', layers: ['trees'], items: ['14.0', '51'], pinned: ['layer:trees'] });
    assert.equal(r.document, 'harbor');
    assert.equal(r.scene, 'harbor');
    assert.match(r.version, /^[0-9a-f]{8}$/);
    const lines = r.text.split('\n');
    assert.equal(lines[0], `<selection document="harbor" scene="harbor" version="${r.version}">`);
    assert.equal(lines[1], '  group trees "Trees" items=1.0,14.0,21.0,28.0,39.0,49.0,50.0,80.0 pinned');
    assert.equal(lines[2], '  item 14.0 object=pine at=tile 3,0,0 group=trees');
    assert.equal(lines[3], '  item 51 object=rock at=tile 0,7,0');
    assert.equal(lines.at(-1), '</selection>');
    assert.deepEqual(r.unsaved, []);
    assert.deepEqual(r.items.map((/** @type {any} */ i) => i.savedPath), ['14.0', '51']);
    assert.equal(r.layers[0].saved, true);
  });

  test('R10: nothing selected says the whole scene is in view', async () => {
    const h = await harbor();
    const r = await h.call('chat.context', { name: 'harbor' });
    assert.equal(r.text.split('\n')[1], '  (nothing selected: the whole scene is in view)');
  });

  test('R10: a layer nested in the selected one counts, and a long group lists 60 paths then says how many more', async () => {
    const h = await harbor();
    const ground = await h.call('chat.context', { name: 'harbor', layers: ['ground'] });
    const line = ground.text.split('\n')[1];
    assert.match(line, /^ {2}group ground "Ground" items=/);
    assert.equal(line.split('items=')[1].split(' ')[0].split(',').length, GROUP_PATHS);
    assert.match(line, /…and 2 more \(scene_items lists them\)$/, 'the harbor\'s ground holds 62 items, in its four layers');
  });

  test('R10: an object names the whole object', async () => {
    const h = await harbor();
    const r = await h.call('chat.context', { object: 'pine' });
    assert.match(r.text, /^<selection document="harbor" object="pine" version="[0-9a-f]{8}">\n {2}\(the whole object is in view\)\n<\/selection>$/);
  });

  test('R10: an unknown item or layer is not_found, not "not saved"', async () => {
    const h = await harbor();
    await assert.rejects(h.call('chat.context', { name: 'harbor', items: ['3'] }), (e) => /** @type {any} */ (e).code === 'not_found');
    await assert.rejects(h.call('chat.context', { name: 'harbor', layers: ['nope'] }), (e) => /** @type {any} */ (e).code === 'not_found');
  });

  test('R10: the CLI prints the block', async () => {
    const h = await harbor();
    const r = await runCli(['chat', 'context', 'harbor', '--items', '14.0', '--document', h.root], { env: h.env });
    assert.equal(r.code, 0, r.stderr);
    assert.match(r.stdout, /^<selection document="harbor" scene="harbor" version="[0-9a-f]{8}">\n {2}item 14\.0 object=pine at=tile 3,0,0 group=trees\n<\/selection>\n$/);
  });
});

describe('chat.context: what is not saved is left out (R11)', () => {
  test('R11: an item placed since the save is left out; items it pushed down the list keep their saved paths', async () => {
    const h = await harbor();
    const placed = await h.call('scene.place', { name: 'harbor', object: 'rock', at: [0, 0, 1] });
    const newPath = placed.path ?? placed.item?.path;
    assert.ok(newPath, JSON.stringify(placed));
    const items = await h.call('scene.items', { name: 'harbor' });
    const pine = items.items.find((/** @type {any} */ i) => i.object === 'pine' && i.at.tile?.join() === '5,6,0');
    assert.notEqual(pine.path, '80.0', 'the placed rock sorted in before the pine, so its working path moved');
    const r = await h.call('chat.context', { name: 'harbor', items: [newPath, pine.path] });
    assert.deepEqual(r.unsaved, [newPath]);
    assert.equal(r.items[1].savedPath, '80.0', 'the pine is named by its path in the saved file');
    assert.match(r.text, /\n {2}item 80\.0 object=pine at=tile 5,6,0 group=trees\n/);
    assert.match(r.text, /\n {2}\(1 selected item is left out: it isn't saved yet\)\n/);
    assert.ok(!r.text.includes(`item ${newPath} `));
  });

  test('R11: a moved item, a new layer and a layer whose items changed are left out; saving brings them back', async () => {
    const h = await harbor();
    await h.call('scene.move', { name: 'harbor', item: '14.0', by: [1, 0] });
    await h.call('scene.layer.new', { name: 'harbor', layerName: 'Rocks', items: ['51'] });
    await h.call('scene.layer.assign', { name: 'harbor', to: 'trees', items: ['85.0'] });
    const items = (await h.call('scene.items', { name: 'harbor' })).items;
    const moved = items.find((/** @type {any} */ i) => i.object === 'pine' && i.at.tile?.join() === '4,0,0').path;
    const r = await h.call('chat.context', { name: 'harbor', layers: ['rocks', 'trees', 'boat'], items: [moved] });
    assert.deepEqual(r.unsaved, ['layer:rocks', 'layer:trees', moved]);
    assert.match(r.text, /\n {2}group boat "Boat" items=41\n/);
    assert.match(r.text, /\(2 selected groups and 1 selected item are left out: they aren't saved yet\)/);
    await h.call('doc.save', { doc: 'scene:harbor' });
    const after = await h.call('chat.context', { name: 'harbor', layers: ['rocks', 'trees'], items: [moved] });
    assert.deepEqual(after.unsaved, []);
    assert.notEqual(after.version, r.version, 'the version is the saved file\'s');
  });

  test('R11: identical items match in order', () => {
    const saved = { items: [{ object: 'a', at: { tile: [0, 0, 0] } }, { object: 'a', at: { tile: [0, 0, 0] } }] };
    const working = { items: [{ object: 'b', at: { tile: [9, 9, 0] } }, ...saved.items, { object: 'a', at: { tile: [0, 0, 0] } }] };
    assert.deepEqual([...matchSaved(working, saved)], [['0', null], ['1', '0'], ['2', '1'], ['3', null]]);
  });

  test('R11: an item whose SVG group changed attributes is not the saved one', () => {
    const saved = { items: [{ group: { class: 'enter' }, items: [{ object: 'a', at: { tile: [0, 0, 0] } }] }] };
    const working = { items: [{ group: { class: 'enter', 'data-x': '3' }, items: [{ object: 'a', at: { tile: [0, 0, 0] } }] }] };
    const r = selectionContext({ document: 'd', scene: 's', version: 'v', working, saved, items: ['0.0'] });
    assert.deepEqual(r.unsaved, ['0.0']);
  });
});

describe('chat.ask: a question with answer buttons (R15, R23)', () => {
  test('R15: from a chat it is asked; from a terminal it says to ask in the conversation', async () => {
    const chat = await harbor({ chat: { id: 'c1', title: 'Pines' } });
    const asked = await chat.call('chat.ask', { question: 'Every pine, or only these 2?', options: ['Only these 2', 'Every pine (5)'] });
    assert.equal(asked.asked, true);
    assert.deepEqual(asked.options, ['Only these 2', 'Every pine (5)']);
    assert.match(asked.note, /their next message/);
    const terminal = await harbor();
    const not = await terminal.call('chat.ask', { question: 'Which?', options: ['A', 'B'] });
    assert.equal(not.asked, false);
    assert.match(not.note, /ask in your own conversation/);
  });

  test('R15: two to four answers, each a short label', async () => {
    const h = await harbor();
    for (const options of [['A'], ['A', 'B', 'C', 'D', 'E'], ['A', 'x'.repeat(61)]]) {
      await assert.rejects(h.call('chat.ask', { question: 'Which?', options }), (e) => /** @type {any} */ (e).code === 'usage', JSON.stringify(options));
    }
  });

  test('R23: over MCP with PIXELLAB_CHAT, chat_ask and chat_context are tools, the question is asked, and the activity line says so', async () => {
    const h = await harbor();
    const server = startServer(['--mcp', '--document', h.root], { env: { ...h.env, PIXELLAB_CHAT: 'chat-7', PIXELLAB_CHAT_TITLE: 'Forest by the house' } });
    try {
      const init = await server.request('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'claude-code', version: '2.1.290' } });
      assert.match(init.result.instructions, /chat_ask/);
      const names = (await server.request('tools/list')).result.tools.map((/** @type {any} */ t) => t.name);
      for (const name of ['chat_ask', 'chat_context']) assert.ok(names.includes(name), name);
      const r = (await server.request('tools/call', { name: 'chat_ask', arguments: { question: 'Every pine?', options: ['Yes', 'No'] } })).result;
      assert.equal(r.structuredContent.asked, true);
      assert.deepEqual(r.structuredContent.chat, { id: 'chat-7', title: 'Forest by the house' });
    } finally {
      server.child.kill();
    }
    const lines = (await readFile(join(h.state, 'activity.jsonl'), 'utf8')).trim().split('\n').map((l) => JSON.parse(l));
    assert.equal(lines.at(-1).summary, 'asked: Every pine?');
    assert.equal(lines.at(-1).agent, 'claude-code 2.1.290');
    assert.deepEqual(lines.at(-1).chat, { id: 'chat-7', title: 'Forest by the house' });
  });
});

describe('proposals and activity know where they came from (R24)', () => {
  test('R24: a chat\'s proposal records the chat and a number; the activity lines record the agent and the chat', async () => {
    const h = await harbor();
    const server = startServer(['--mcp', '--document', h.root], { env: { ...h.env, PIXELLAB_CHAT: 'chat-1', PIXELLAB_CHAT_TITLE: 'Move the pines' } });
    /** @type {any} */
    let saved;
    try {
      await server.request('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'claude-code', version: '2.1.290' } });
      const tool = async (/** @type {string} */ name, /** @type {any} */ args) => (await server.request('tools/call', { name, arguments: args })).result;
      const moved = await tool('scene_move', { name: 'harbor', item: '14.0', by: [1, 0] });
      assert.equal(moved.isError, undefined, moved.content[0].text);
      saved = (await tool('doc_save', { doc: 'scene:harbor' })).structuredContent;
    } finally {
      server.child.kill();
    }
    assert.equal(saved.number, 1);
    assert.match(saved.summary, /^Proposed scene:harbor as #1 \(\d{8}-\d{6}-[0-9a-f]{6}\) \(1 edit\)$/);
    const p = JSON.parse(await readFile(join(h.state, 'proposals', `${saved.proposal}.json`), 'utf8'));
    assert.equal(p.number, 1);
    assert.deepEqual(p.chat, { id: 'chat-1', title: 'Move the pines' });
    assert.equal(p.agent, 'claude-code 2.1.290');
    const listed = (await h.call('proposal.list', {})).proposals[0];
    assert.equal(listed.number, 1);
    assert.deepEqual(listed.chat, { id: 'chat-1', title: 'Move the pines' });
    const shown = await h.call('proposal.show', { id: saved.proposal });
    assert.equal(shown.number, 1);
    assert.deepEqual(shown.chat, listed.chat);
    const lines = (await readFile(join(h.state, 'activity.jsonl'), 'utf8')).trim().split('\n').map((l) => JSON.parse(l));
    for (const line of lines) {
      assert.equal(line.agent, 'claude-code 2.1.290');
      assert.deepEqual(line.chat, { id: 'chat-1', title: 'Move the pines' });
    }
    const out = await runCli(['proposal', 'list', '--document', h.root], { env: h.env });
    assert.match(out.stdout, /^#1 {2}\d{8}-\d{6}-[0-9a-f]{6} {2}scene:harbor {2}chat "Move the pines" {2}1 edit$/m);
  });

  test('R24: numbers count up, a rewrite keeps its number, and servers proposing at once get their own', async () => {
    const dir = join(tmp, 'numbers');
    const state = createState(dir);
    const base = { doc: 'scene:a', baseHash: null, source: '', summary: [], createdAt: '2026-10-21T00:00:00.000Z', agent: null };
    const ids = Array.from({ length: 8 }, (_, i) => `20261021-000000-00000${i}`);
    const written = await Promise.all(ids.map((id) => state.writeProposal({ ...base, id })));
    assert.deepEqual(written.map((p) => p.number).sort((a, b) => a - b), [1, 2, 3, 4, 5, 6, 7, 8], 'eight at once, eight numbers');
    const again = await state.writeProposal({ ...base, id: ids[3], summary: ['rewritten'] });
    assert.equal(again.number, written[3].number);
    const next = await state.writeProposal({ ...base, id: '20261021-000001-aaaaaa' });
    assert.equal(next.number, 9);
    assert.equal((await readFile(join(dir, 'proposal-numbers.log'), 'utf8')).trim().split('\n').length, 9);
    assert.deepEqual(await readdir(join(dir, 'proposals')).then((f) => f.length), 9, 'numbers.log is beside the proposals, not among them');
  });

  test('R24: an older proposal without a number or a chat still lists, shows and is rejected', async () => {
    const h = await harbor();
    await mkdir(join(h.state, 'proposals'), { recursive: true });
    const id = '20261019-120000-abcdef';
    const source = await readFile(join(h.root, 'scenes', 'harbor.mjs'), 'utf8');
    await writeFile(join(h.state, 'proposals', `${id}.json`), JSON.stringify({ id, doc: 'scene:harbor', baseHash: null, source, summary: ['old'], createdAt: '2026-10-19T12:00:00.000Z', agent: 'old-agent' }));
    const listed = (await h.call('proposal.list', {})).proposals;
    assert.equal(listed.length, 1);
    assert.equal(listed[0].number, undefined);
    assert.equal(listed[0].chat, undefined);
    assert.equal((await h.call('proposal.show', { id })).agent, 'old-agent');
    await h.call('proposal.reject', { id });
    assert.deepEqual((await h.call('proposal.list', {})).proposals, []);
  });
});
