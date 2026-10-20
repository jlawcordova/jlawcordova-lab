# The CLI and servers (`cli/`)

`pixellab` is a Node CLI (Node ≥ 22, ESM, Node built-ins only, no `node_modules`) and the two servers built on the same commands: `serve --stdio` for the Mac app and `serve --mcp` for agents. How it fits with the app and the site is in [Architecture](../docs/architecture.md); every command, flag, method and error code is in the generated [command reference](../docs/cli.md).

```sh
node cli/bin/pixellab.mjs help              # every command, from the registry
node cli/bin/pixellab.mjs scene --help      # one group
node cli/bin/pixellab.mjs document info --json  # a command, as JSON
npm test                                    # the suites in cli/test/, must show "# fail 0"
```

## Module map

| File | What it does |
| --- | --- |
| `bin/pixellab.mjs` | The entry point: calls `main()` from `lib/cli.mjs`. |
| `lib/cli.mjs` | The command-line door: argv → command → human text or `--json`, exit codes 0, 1, 2. One-shot edits: runs the command in a fresh session, then saves (or prints the diff with `--dry-run`). `main()` takes its streams and env as arguments, so tests call it directly. |
| `lib/argv.mjs` | Parses the command line against the registry: the longest run of words that names a command, positionals into the command's `args`, every param as a `--kebab-case` flag, the global flags (`GLOBAL_FLAGS`). |
| `lib/params.mjs` | `checkParams()`: the JSON Schema subset every door checks params with (type, enum, pattern, minimum, maximum, minItems, maxItems, items, required, default). Refuses unknown params; with `coerce`, turns command-line strings into the declared types. |
| `lib/registry.mjs` | The registry: `createRegistry()` checks each definition (`checkDefinition()`), sorts by name, and `call(ctx, name, params)` checks params and runs. `registry` is the built-in list. |
| `lib/commands/*.mjs` | The commands, one file per area: `document` (document.info, document.new, document.copy, palette.get), `scene` (read, and `scene.sort`), `scene-edit` (with groups, depth overrides and levels), `scene-layers` (making, renaming, ungrouping, assigning and moving layers), `scene-motion` (tracks, keys, paths), `object` (read), `object-edit` (with `object.map.set` and `object.footprint`), `check`, `art` (art.build, art.preview, art.sizes), `doc` (session commands), `proposal` (proposals, activity.list). `common.mjs` holds shared params and helpers. |
| `lib/context.mjs` | `createContext(site, { overlay })`: what a command runs against. `ctx.sources()`, `ctx.doc(kind, name)`, `ctx.names(kind)`, `ctx.find(query)`; memoized per call, `ctx.invalidate()` after an edit. Commands never read the disk themselves. |
| `lib/session.mjs` | `Session`: open docs as in-memory working copies with undo and redo, plugged into the ctx as its overlay, plus `ctx.edit()`. Saving: validate, serialize, conflict check by hash, atomic write; in `'propose'` mode (MCP) a save writes a proposal. `sync()` settles applied or rejected proposals and reloads clean docs whose files changed; `reload()` and `merge()` are `doc.reload` and `doc.merge`. |
| `lib/merge.mjs` | `merge3()`: the three-way line merge `doc.merge` and `proposal.apply` use. |
| `lib/open-docs.mjs` | `open/<pid>.json` in the state dir: the app's open docs, for `doc.status`'s `unsavedInApp`. |
| `lib/layer-ops.mjs` | Scene layers (`docs/intents/2026-10-scene-layers/`): a scene's `layers` list and each item's `layer`, finding layers by id or name, chains, the members' common parent, ids from names, and the edits behind `scene.layer.*`. Never reorders items for a layer's sake, so layers never change what's published. |
| `lib/motion-ops.mjs` | Motion edits: tracks, keys, the loop's length, `offGridMoves()` (check's warnings) and `motionPath()`. |
| `lib/document.mjs` | Finds the `.pixellab` document (or the one inside a site checkout, legacy `source/` folders included), reads and checks its manifest, carries the engine modules, and loads the art sources. |
| `lib/engine/` | The pixel-art engine: `engine`, `iso`, `depth` (the depth order: boxes, which item is behind, the sort, motion's places), `motion`, `serialize`, `edit`, `starter`, `svg` (from the site, verbatim), `compile` (rect SVG → paths, the lossless check, budgets, `compileScene`, `compileAll`, `sceneSizes`), `preview` (the review PNGs), `starter-palette`. Exported from the package as `pixellab/engine`, `pixellab/depth`, `pixellab/compile`, … `engine` also has the helpers the rest of the CLI orders with: `depthOrder`, `sceneInOrder`, `isInDepthOrder`, `sceneAt`, `motionCopies`. |
| `lib/scene-ops.mjs` | The scene edits, ported from the site's web lab (`src/components/lab/scene-mode.ts`); each function names the lab function it follows. `sortScene` puts a scene's lists in depth order and says which paths moved; every scene edit ends with it (raise, lower and `insertIndex` are gone: placing appends, then sorts). |
| `lib/render.mjs` | Scenes and objects to transparent PNGs at an integer scale, with each item's bounds for hit-testing; poses moving items at a time; leaves hidden layers out and renders one layer alone (`hide`, `focus`); the night theme's shadow color. |
| `lib/diff.mjs` | A unified line diff (Myers), for `--dry-run`, `doc.save` and proposals. |
| `lib/errors.mjs` | `CommandError` (a stable `code`, an exit code, optional `details`), `UsageError`, `NotFoundError`, `DocumentError`. |
| `lib/serve.mjs` | `pixellab serve --stdio \| --mcp [--direct]`. |
| `lib/jsonrpc.mjs` | Newline-delimited JSON-RPC 2.0 over two streams, shared by both servers; maps errors to JSON-RPC codes. |
| `lib/server-stdio.mjs` | The app's server: one session, the commands as methods plus `session.info` and `shutdown`, and the notifications. `VERSION` here (read from `package.json`) is the app's and the MCP server's version: `0.0.0-dev` in the repo, the release's in an app bundle. |
| `lib/server-mcp.mjs` | The MCP server: `initialize`, `ping`, `tools/list`, `tools/call`; tool names, `LEFT_OUT` (commands agents don't get), the annotations, the activity log, and `session.sync()` before each call. |
| `lib/state.mjs` | The state dir: proposals and `activity.jsonl`; `writeAtomic()`. |
| `lib/watch.mjs` | File watching for the stdio server: the art sources and the state dir, debounced. |
| `lib/activity-summary.mjs` | A few words per MCP tool call for the activity feed ("listed 15 scenes"). |
| `lib/help.mjs` | Help text and usage lines, generated from the registry (also used by `scripts/docs-cli.mjs`). |

## A command

```js
/** @type {import('../registry.mjs').Command} */
const info = {
  name: 'document.info',             // dotted lowercase words; the MCP tool is document_info
  summary: 'Show the document in use and what it holds',
  description: 'Longer help text…',  // optional
  params: { type: 'object', properties: {} },  // a JSON Schema object
  args: [],                          // params the CLI fills from positionals, in order
  mutates: false,                    // true: it edits docs through ctx.edit
  async run(ctx, params) {           // params are already checked and defaulted
    const { sources } = await ctx.sources();
    return { scenes: sources.scenes.size };    // plain JSON, the same through every door
  },
  format: (r) => `scenes ${r.scenes}`,          // optional: the CLI's human output
};
```

Optional fields: `examples` (command lines for the help and the reference), `writesPng` (the result has a base64 `png`, so the CLI takes `--out`), `session: true` (only makes sense in `pixellab serve`, like `doc.undo`), `writes: 'direct'` (writes files itself, like `art.build`, so `--dry-run` doesn't apply), `document: false` (needs no open document, like `document.new`, so the CLI doesn't look for one). The `Command` typedef in `lib/registry.mjs` is the source.

### Mutating commands

A command that edits a doc sets `mutates: true` and changes the doc only through `ctx.edit()`:

```js
async run(ctx, params) {
  return editOf(ctx)({ kind: 'scene', name }, (doc) => {
    // change `doc` in place; throw a CommandError to refuse (the doc is restored)
    return { message: 'pine placed at column 14, row 6, level 1', path: '198' };
  }, { key: null });   // a key coalesces rapid repeats into one undo step
}
```

`ctx.edit` opens the doc if it isn't open, snapshots it for undo, runs the function, and returns `{ doc, changed, summary, ...rest }`. An edit that changes nothing isn't an undo step. The door decides what happens next: the CLI saves at once (or diffs with `--dry-run`), the stdio server keeps the change in the session until `doc.save`, the MCP server turns `doc_save` into a proposal. So a command never writes the site itself (unless it's `writes: 'direct'`).

## Adding a command

1. Add the definition to the right file in `lib/commands/` and to its default export. A new area gets a new file, added to the list at the bottom of `lib/registry.mjs`.
2. Use the engine (`lib/engine/`) for anything the art's format decides (validate, serialize, edit, starters, compile, previews), from `ctx.site.engine` and friends (`ctx.document` is the same object): never reimplement it in a command. Scene edits go in `lib/scene-ops.mjs`.
3. Throw `CommandError` with a stable `code` for failures a person or agent can act on (`not_found`, `invalid`, `conflict`, …), `UsageError` for bad params beyond the schema.
4. Test it in `cli/test/`, named after the requirement it proves (`R3: …`), against the sample document (`makeDocument()` in `test/helpers.mjs`) or, for the owner's art, a copy of the site (`makeSite()`). See [Testing](../docs/testing.md).
5. Run `npm run docs:cli` to regenerate [docs/cli.md](../docs/cli.md), and `npm test`. A test fails if the reference is out of date.
6. It's now on the command line, in the stdio server and in MCP. If agents shouldn't have it, add it to `LEFT_OUT` in `lib/server-mcp.mjs` with the reason. If the app needs it, model its result in `app/Sources/PixelLab/Server/Protocol.swift` (or decode it where it's used) and call it through `ServerConnection` (see [the app's README](../app/README.md#adding-a-feature)).

## Sessions, undo and saving

- A **session** (`Session` in `lib/session.mjs`) holds open docs, each with an undo and a redo stack. Its scene edit sorts the scene into depth order before it returns, so an edit and its sort are one undo step; `doc.merge` and `doc.reload` sort what they adopt. A step is a snapshot of the doc before the edit; at most `UNDO_LIMIT` (200) are kept. Edits with the same `key` within `COALESCE_MS` (1 s) merge into one step: a held arrow key, one paint stroke sent as many `object.paint` calls.
- **Saving** validates with the engine and refuses a doc with problems (`invalid`), serializes with the engine, and refuses (`conflict`) if the file's hash on disk changed since the doc was opened; `force` overrides both. The write is atomic. In `'propose'` mode it writes `proposals/<id>.json` to the state dir instead, listing the edits since the doc last matched its file (each undo step has a sequence number; the entry's `baseSeq` marks the last one on disk).
- The stdio server watches the sources: a clean open doc reloads when its file changes; one with unsaved changes stays and is reported `stale`.

## Errors and exit codes

| | CLI exit | JSON-RPC |
| --- | --- | --- |
| ok | 0 | result |
| `CommandError` (ran and failed; `check` found problems) | 1 | -32000, `data.code` = the error's code |
| `UsageError` (bad params, unknown command or flag) | 2 | -32602 (`usage`), or -32601 for an unknown method |
| a bug | 1 | -32603 (`internal`) |

With `--json`, errors go to stderr as `{"error": {"code", "message", "details?"}}`. MCP reports a failed command as a tool result with `isError: true`.

## Logs and output

Both servers write nothing but JSON-RPC to stdout; logs go to stderr (`consoleToStderr()` makes sure a stray `console.log` can't corrupt the stream). The CLI writes results to stdout and errors to stderr, and stops quietly when stdout closes early (`| head`).
