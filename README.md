# Pixel Lab for Mac

A native macOS editor for the pixel art in [jlawcordova.github.io](https://github.com/jlawcordova/jlawcordova.github.io), with a `pixellab` CLI and MCP server so agents can do everything a person can.

Work in progress. The architecture and build plan are in [docs/plan.md](docs/plan.md).

## The CLI

Node 22 or later, no dependencies. It reads and writes the site checkout (`--site`, `PIXELLAB_SITE`, or `~/Projects/jlawcordova.github.io`).

```sh
node cli/bin/pixellab.mjs help
node cli/bin/pixellab.mjs scene render hero-island --scale 2 --out /tmp/hero.png
node cli/bin/pixellab.mjs scene place hero-island pine --at 14,6,1 --dry-run   # show the diff
node cli/bin/pixellab.mjs object paint pine 3,4 4,4 --color grass-2           # validate and write
```

Edits are saved as they run, after the engine validates them; `--dry-run` shows the diff instead. Every command, the stdio protocol the Mac app speaks (`pixellab serve --stdio`) and the MCP server are in [docs/cli.md](docs/cli.md), generated from the command registry with `npm run docs:cli`.

## MCP for agents

```sh
claude mcp add pixellab -- node /Users/jlawcordova/Projects/jlawcordova-lab/cli/bin/pixellab.mjs serve --mcp
```

Agents edit in memory; their `doc_save` writes a proposal to `~/Library/Application Support/Pixel Lab/proposals/` for a person to approve (`pixellab proposal list`, `proposal show`, `proposal apply`), and every tool call is logged to `activity.jsonl` there.

## Tests

`npm test` (node:test) copies the site's engine and sources into a temp folder; it never writes the real checkout.
