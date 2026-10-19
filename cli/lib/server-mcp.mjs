// `pixellab serve --mcp`: the registry's commands as MCP tools, for agents
// (docs/cli.md, "MCP"). Hand-written MCP over stdio (protocol 2025-06-18):
// initialize, notifications/initialized, ping, tools/list and tools/call,
// as newline-delimited JSON-RPC 2.0.
//
// Agents propose, people approve (docs/intents/2026-10-mac-app/plan.md): edits stay in the server's
// session, and doc_save writes a proposal to the state dir instead of the
// site, unless the server was started with --direct. Every tools/call
// appends { time, tool, args, ok, summary } to activity.jsonl there.

import { activitySummary } from './activity-summary.mjs';
import { consoleToStderr, INVALID_PARAMS, METHOD_NOT_FOUND, RpcError, serveLines } from './jsonrpc.mjs';
import { CommandError } from './errors.mjs';
import { registry as defaultRegistry } from './registry.mjs';
import { VERSION } from './server-stdio.mjs';
import { Session } from './session.mjs';

export const PROTOCOL_VERSION = '2025-06-18';
const SUPPORTED = [PROTOCOL_VERSION, '2025-03-26', '2024-11-05'];

/**
 * Commands that aren't tools, and why: approving a proposal is a person's
 * job, and art.build writes the site's generated SVGs.
 */
export const LEFT_OUT = {
  'proposal.apply': 'approving a proposal is for a person, in Pixel Lab or with pixellab proposal apply',
  'proposal.reject': 'rejecting a proposal is for a person',
  'art.build': 'it publishes the SVGs a site inlines; a person publishes after approving',
  'document.copy': 'saving a copy of the document is for a person',
};

/** scene.place → scene_place. */
export const toolName = (/** @type {string} */ command) => command.replace(/\./g, '_');

const INSTRUCTIONS = `Pixel Lab edits the pixel art in a .pixellab document: scenes (isometric, 32×16 tiles) and objects (sprites and blocks drawn from a fixed palette). The site that inlines the art publishes from the document.
Look first: document_info, scene_list, scene_items, scene_render (returns the PNG), object_list, object_show, object_render, palette_get, check.
Edit: scene_place, scene_move, scene_set, object_paint, object_fill and the rest. Edits stay in this server's memory with undo (doc_undo, doc_redo, doc_status).
Save: doc_save validates the doc and, unless the server runs with --direct, writes a proposal for a person to approve in Pixel Lab instead of changing the document. Say what you changed when you're done.`;

/**
 * @param {{
 *   site: any,
 *   env?: Record<string, string | undefined>,
 *   input?: NodeJS.ReadableStream,
 *   output?: NodeJS.WritableStream,
 *   log?: (text: string) => void,
 *   registry?: import('./registry.mjs').Registry,
 *   direct?: boolean,
 * }} options
 */
export async function startMcpServer({ site, env = process.env, input = process.stdin, output = process.stdout, log = (t) => process.stderr.write(`pixellab: ${t}\n`), registry = defaultRegistry, direct = false }) {
  if (output === process.stdout) consoleToStderr();
  const session = new Session(site, { env, mode: direct ? 'direct' : 'propose' });
  const left = direct ? { 'proposal.apply': LEFT_OUT['proposal.apply'], 'proposal.reject': LEFT_OUT['proposal.reject'] } : LEFT_OUT;

  /** @type {Map<string, import('./registry.mjs').Command>} */
  const tools = new Map();
  for (const cmd of registry.commands.values()) if (!(cmd.name in left)) tools.set(toolName(cmd.name), cmd);

  const describe = (/** @type {import('./registry.mjs').Command} */ cmd) => {
    let text = `${cmd.summary}.${cmd.description ? ` ${cmd.description}` : ''}`;
    if (cmd.name === 'doc.save') {
      text += direct
        ? ' This server runs with --direct: doc_save writes the site.'
        : ' In this server doc_save does not change the site: it writes a proposal (with the list of edits) for a person to approve.';
    }
    if (cmd.mutates && !cmd.writes && cmd.name.split('.')[0] !== 'doc') text += ' The edit stays in memory until doc_save.';
    return text;
  };

  const listTools = () =>
    [...tools].map(([name, cmd]) => ({
      name,
      title: cmd.summary,
      description: describe(cmd),
      inputSchema: { type: 'object', properties: cmd.params.properties ?? {}, ...(cmd.params.required?.length ? { required: cmd.params.required } : {}), additionalProperties: false },
      annotations: { readOnlyHint: !cmd.mutates && !cmd.writes, destructiveHint: cmd.name === 'scene.remove' || cmd.name.endsWith('.delete'), openWorldHint: false },
    }));

  const callTool = async (/** @type {any} */ params) => {
    const name = params?.name;
    const args = params?.arguments ?? {};
    const cmd = typeof name === 'string' ? tools.get(name) : undefined;
    const time = new Date().toISOString();
    const append = (/** @type {boolean} */ ok, /** @type {string} */ summary) =>
      session.state.appendActivity({ time, tool: String(name), args, ok, summary }).catch((err) => log(`can't write the activity log: ${err.message}`));
    if (!cmd) {
      await append(false, `unknown tool ${name}`);
      const why = typeof name === 'string' && left[/** @type {keyof typeof left} */ (name.replace(/_/g, '.'))];
      throw new RpcError(INVALID_PARAMS, `unknown tool ${name}${why ? `: ${why}` : ''}`);
    }
    try {
      const result = await registry.call(session.context(), cmd.name, args);
      const summary = activitySummary(cmd, result, args);
      await append(true, summary);
      if (result && typeof result.png === 'string') {
        const { png, ...rest } = result;
        return {
          content: [
            { type: 'image', data: png, mimeType: 'image/png' },
            { type: 'text', text: JSON.stringify(rest) },
          ],
          structuredContent: rest,
        };
      }
      const structured = result && typeof result === 'object' && !Array.isArray(result) ? result : { result };
      return { content: [{ type: 'text', text: JSON.stringify(structured) }], structuredContent: structured };
    } catch (err) {
      const e = /** @type {any} */ (err);
      const message = e?.message ?? String(err);
      await append(false, message.split('\n')[0]);
      if (!(err instanceof CommandError)) log(`internal error in ${name}: ${e?.stack ?? e}`);
      const error = err instanceof CommandError ? err.toJSON() : { code: 'internal', message };
      return { content: [{ type: 'text', text: message }], structuredContent: { error }, isError: true };
    }
  };

  const handle = async (/** @type {string} */ method, /** @type {any} */ params) => {
    switch (method) {
      case 'initialize': {
        const client = params?.clientInfo;
        session.agent = client?.name ? `${client.name}${client.version ? ` ${client.version}` : ''}` : null;
        const asked = params?.protocolVersion;
        return {
          protocolVersion: SUPPORTED.includes(asked) ? asked : PROTOCOL_VERSION,
          capabilities: { tools: { listChanged: false } },
          serverInfo: { name: 'pixellab', title: 'Pixel Lab', version: VERSION },
          instructions: INSTRUCTIONS,
        };
      }
      case 'notifications/initialized':
      case 'notifications/cancelled':
        return null;
      case 'ping':
        return {};
      case 'tools/list':
        return { tools: listTools() };
      case 'tools/call':
        return callTool(params);
      default:
        if (method.startsWith('notifications/')) return null;
        throw new RpcError(METHOD_NOT_FOUND, `no method ${method}`);
    }
  };

  const rpc = serveLines({ input, output, handle, log });
  await rpc.done;
  return 0;
}
