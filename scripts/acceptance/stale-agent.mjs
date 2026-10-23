// node scripts/acceptance/stale-agent.mjs <site> <state>: an agent over MCP places a tree on
// footer-grass with no app open: it applies and saves (docs/intents/2026-10-autosave/spec.md, R23),
// and stale.txt finds it there as the agent's version.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
const [site, state] = process.argv.slice(2);
const program = process.env.PIXELLAB_PROGRAM ?? new URL('../../target/release/pixellab', import.meta.url).pathname;
const p = spawn(program, ['serve', '--mcp', '--site', site], { env: { ...process.env, PIXELLAB_STATE: state }, stdio: ['pipe', 'pipe', 'inherit'] });
const rl = createInterface({ input: p.stdout });
const waiting = new Map();
rl.on('line', (l) => { const m = JSON.parse(l); waiting.get(m.id)?.(m); });
let id = 0;
const rpc = (method, params) => new Promise((res) => { const i = ++id; waiting.set(i, res); p.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: i, method, params }) + '\n'); });
const call = async (name, args) => {
  const r = await rpc('tools/call', { name, arguments: args });
  const s = r.result?.structuredContent;
  console.log(`${name} → ${r.result?.isError ? 'ERROR ' + JSON.stringify(s) : (s?.summary ?? 'ok')}`);
  if (r.result?.isError) process.exitCode = 1;
  return s;
};
await rpc('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'claude-code', version: '2.1.0' } });
await call('scene_place', { name: 'footer-grass', object: 'pine', px: [50, 20] });
await call('doc_save', { doc: 'scene:footer-grass' });
p.stdin.end();
