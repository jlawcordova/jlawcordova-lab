// `pixellab serve --stdio | --mcp [--direct] [--site <path>]`: starts one of
// the servers on stdin and stdout. Logs go to stderr.

import { UsageError } from './errors.mjs';
import { startMcpServer } from './server-mcp.mjs';
import { startStdioServer } from './server-stdio.mjs';
import { openSite } from './site.mjs';

export const SERVE_HELP = `Usage: pixellab serve --stdio [--site <path>]
       pixellab serve --mcp [--direct] [--site <path>]

Starts a server on stdin and stdout (newline-delimited JSON-RPC 2.0; logs on stderr).
  --stdio   the Mac app's server: methods are the command names, plus session.info and
            shutdown; edits stay in memory with undo until doc.save writes them
  --mcp     an MCP server for agents: tools are the commands (scene.place → scene_place);
            doc_save writes a proposal to the state dir for a person to approve
  --direct  with --mcp: doc_save writes the site instead of a proposal

Register with Claude Code:
  claude mcp add pixellab -- node <repo>/cli/bin/pixellab.mjs serve --mcp

The state dir is $PIXELLAB_STATE, else ~/Library/Application Support/Pixel Lab/. See docs/cli.md.`;

/**
 * @param {string[]} argv  after `serve`
 * @param {{ stdout?: { write: (s: string) => unknown }, env?: Record<string, string | undefined>, cwd?: string }} io
 */
export async function serve(argv, { stdout = process.stdout, env = process.env, cwd = process.cwd() } = {}) {
  /** @type {{ stdio?: boolean, mcp?: boolean, direct?: boolean, site?: string, help?: boolean }} */
  const flags = {};
  for (let i = 0; i < argv.length; i++) {
    const token = argv[i];
    const m = /^--(stdio|mcp|direct|help)$/.exec(token);
    if (m) flags[/** @type {'stdio' | 'mcp' | 'direct' | 'help'} */ (m[1])] = true;
    else if (token === '-h') flags.help = true;
    else if (token === '--site') {
      if (i + 1 >= argv.length) throw new UsageError('--site needs a value');
      flags.site = argv[++i];
    } else if (token.startsWith('--site=')) flags.site = token.slice('--site='.length);
    else throw new UsageError(`serve: unknown argument ${token}; run pixellab serve --help`);
  }
  if (flags.help) {
    stdout.write(`${SERVE_HELP}\n`);
    return 0;
  }
  if (Boolean(flags.stdio) === Boolean(flags.mcp)) throw new UsageError('serve: give --stdio or --mcp; run pixellab serve --help');
  if (flags.direct && !flags.mcp) throw new UsageError('serve: --direct goes with --mcp');
  const site = await openSite({ site: flags.site, env, cwd });
  if (flags.stdio) return startStdioServer({ site, env });
  return startMcpServer({ site, env, direct: Boolean(flags.direct) });
}
