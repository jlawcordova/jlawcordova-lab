// node scripts/acceptance/mcp-agent.mjs <site> <state>: an agent over MCP (as Claude Code would)
// looks around, then saves two proposals: two trees on hero-island, three pixels on house.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
const [site, state] = process.argv.slice(2);
const cli = process.env.PIXELLAB_CLI_PATH ?? new URL('../../cli/bin/pixellab.mjs', import.meta.url).pathname;
const p = spawn(process.execPath, [cli, 'serve', '--mcp', '--site', site], { env: { ...process.env, PIXELLAB_STATE: state }, stdio: ['pipe', 'pipe', 'inherit'] });
const rl = createInterface({ input: p.stdout });
const waiting = new Map();
rl.on('line', (l) => { const m = JSON.parse(l); waiting.get(m.id)?.(m); });
let id = 0;
const rpc = (method, params) => new Promise((res) => { const i = ++id; waiting.set(i, res); p.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: i, method, params }) + '\n'); });
const call = async (name, args) => {
  const r = await rpc('tools/call', { name, arguments: args });
  const s = r.result?.structuredContent;
  console.log(`${name} → ${r.result?.isError ? 'ERROR ' + JSON.stringify(s) : (s?.summary ?? 'ok')}`);
  return s;
};
await rpc('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'claude-code', version: '2.1.0' } });
p.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n');
const tools = (await rpc('tools/list', {})).result.tools.map((t) => t.name);
console.log(`${tools.length} tools; art_build exposed: ${tools.includes('art_build')}; proposal_apply exposed: ${tools.includes('proposal_apply')}`);
await call('scene_list', {});
await call('object_list', {});
await call('scene_render', { name: 'hero-island', scale: 2 });
await call('object_show', { name: 'house' });
await call('check', {});
await call('scene_place', { name: 'hero-island', object: 'pine', px: [-48, 92] });
await call('scene_place', { name: 'hero-island', object: 'tree-small', px: [14, 62] });
await call('doc_save', { doc: 'scene:hero-island' });
await call('object_paint', { name: 'house', pixels: [[20, 30], [21, 30], [22, 30]], color: 'water-1' });
await call('doc_save', { doc: 'object:house' });
p.stdin.end();
