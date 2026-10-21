# Architecture

How Pixel Lab fits together today. The design record, [`2026-10-mac-app/plan.md`](intents/2026-10-mac-app/plan.md), says why it's built this way and holds the look (board 03, "Field notebook"); this page is the map you need before changing the code. Each folder's README goes one level deeper: [the CLI](../cli/README.md), [the app](../app/README.md), [the scripts](../scripts/README.md).

## Three doors onto one registry

Every action Pixel Lab can take is a **command** in one registry (`cli/lib/registry.mjs`, the commands in `cli/lib/commands/`). A command is a dotted name, a summary, its params as a JSON Schema object, whether it mutates, and `run(ctx, params)`, which returns plain JSON. Three doors call it:

```
a terminal ───── argv ─────────────▶ cli.mjs ──────────┐
Pixel Lab.app ── JSON-RPC (stdio) ─▶ server-stdio.mjs ─┼─▶ registry ─▶ command.run(ctx)
an agent ─────── MCP (stdio) ──────▶ server-mcp.mjs ───┘                    │
                                                        session.mjs (working copies, undo,
                                                        save) and the engine (cli/lib/engine/)
```

- **The command line** (`pixellab scene place hero-island pine --at 14,6,1`): `argv.mjs` turns words and flags into a command and params from the command's schema; `cli.mjs` runs it in a fresh session and saves right away (one-shot edits), or prints a diff with `--dry-run`.
- **The app's server** (`pixellab serve --stdio`): newline-delimited JSON-RPC 2.0, method names are the command names. It holds one long-lived session, so edits are instant and stay in memory until `doc.save`. It pushes notifications: `doc.changed`, `files.changed`, `proposals.changed`, `activity.appended`.
- **MCP** (`pixellab serve --mcp`): the same commands as MCP tools (`scene.place` → `scene_place`), hand-written on Node built-ins. Its session saves to **proposals**, not the document.

All three check params with the same `checkParams()` (`cli/lib/params.mjs`) and return the same JSON, so a command added once works everywhere. The full list of commands, the wire formats and the error codes are in the generated reference, [docs/cli.md](cli.md).

## The document is the source of truth, and the engine lives here

The art lives in a **`.pixellab` document**, a folder: `manifest.json`, `palette.mjs`, `objects/*.mjs`, `scenes/*.mjs`. The engine lives in this repo, `cli/lib/engine/` (the design record `2026-10-pixellab-document` moved it in from the site):

- `engine.mjs`, `iso.mjs`, `depth.mjs`, `motion.mjs`, `serialize.mjs`, `edit.mjs`, `starter.mjs`, `svg.mjs`: load, validate, render, serialize, edit, the isometric grid, the depth order (`depth.mjs`, below), the frame loops and build-ups, the starters, layers → rect SVG.
- `compile.mjs`: rect SVG → one `<path>` per color, the lossless check, the budgets, `compileScene` and `compileAll` (what the site's `npm run art` did).
- `preview.mjs`: the review PNGs (an object on a tile, a scene at its viewBox, the palette's swatch sheet) on the document's card color; `encodePng` with `node:zlib`.
- `starter-palette.mjs`: what a new document's palette starts as.

`cli/lib/document.mjs` resolves the document (`--document`, else `PIXELLAB_DOC`, else `~/.config/pixellab/config.json`, else `--site` / `PIXELLAB_SITE`: a site checkout, whose `*.pixellab` under `src/assets/pixel-art/`, or older `source/` folder, is the document), reads and checks its manifest (the publish folder, per-output budgets, preview and theme colors; a legacy `source/` folder gets the site's old values), and hands every command the engine as `ctx.site` (also `ctx.document`). Sources load with `?v=<mtime>-<size>`, so an edit on disk is picked up by the next load. Every write validates with the engine, serializes with the engine and writes atomically (a temp file, then a rename). The SVGs a site inlines are written only by `art.build` (Publish), into the manifest's `publish.dir` or `--dir`.

The one piece of editing logic that didn't come from the engine, the scene edits of the site's old web lab (place, move, duplicate, remove, snap; raise and lower are gone, see depth below), is in `cli/lib/scene-ops.mjs`. A scene can also name its parts as **layers** (`2026-10-scene-layers`): a `layers` list of `{ id, name, parent }` and a `layer` on each placed item, checked and written by the engine, edited by `cli/lib/layer-ops.mjs` (`scene.layer.*`). They're bookkeeping only: no layer edit reorders the items for the sake of the layers, the engine draws a scene the same with or without them, and they're apart from the `{ group, items }` wrappers that become the SVG's `<g>`s. Which layers the app hides or locks is the app's view state (`layer-view.json` in the state dir), never part of the document. The package exports the engine modules (`pixellab/engine`, `pixellab/compile`, `pixellab/document`, …), so the site installs it from git to publish and to test its art; a change to the engine is a change here, released by tag, and the site bumps its pin.

## Depth: where each item stands decides what draws first

A scene's items draw back to front by where they stand, and nobody lists them in order by hand (`2026-10-depth-order`). The rules live in `cli/lib/engine/depth.mjs`, pure geometry that the engine calls:

- **A box in tile space.** Each item stands in a box worked out from its placement (a tile `col, row, level`, or a pixel point plus a `level`, read in sub-tile steps) and its object: a block's `size`, a sprite's `footprint: [wide, deep]` (default `[1, 1]`) and height. A `{ group, items }` wrapper's box is the union of its members', and a run of consecutive variant groups (`data-class`) sorts as one unit.
- **Behind.** Of two items whose drawn pixels could overlap, the one whose box ends before the other's starts on any axis draws first. Items that don't overlap keep the list's order, a cycle is broken by a fixed key, and sorting a sorted list changes nothing.
- **Overrides.** An item or group may carry `depth: 'front'` or `depth: 'back'`, which puts it after or before everything else in its own list. It is the one manual control; `scene.raise`, `scene.lower` and `scene.place --index` are gone.
- **Two places keep it right.** The engine sorts every list when it draws (`renderScene`, the compiler, the previews, `scene.render`), so what's published is right whatever order a file lists. And every scene edit sorts the whole scene in the same undo step (`sortScene` in `scene-ops.mjs`, run by the session's edit), so the file, `scene.items` and the Layers panel read back to front. A result names the paths the sort moved (`reordered`) so a caller holding other paths, such as the app's selection, can follow them. `scene.sort` and `check`'s `out_of_order` warning cover a file that isn't in order.
- **Motion.** A moving item has, at each moment of the loop, the depth of where it is then, and the published SVG draws it once for each place it takes in its list, each copy shown only while it's in that place. The list and the Layers panel keep it where it rests. `check` reports the copies, since they add to the SVG's size.

## An edit in the app, end to end

```
StageInputView (a click)                   pixellab serve --stdio
 └▶ SceneEditor.place(…)
     └▶ connection.call("scene.place") ───▶ scene.place.run(ctx)
                                             └▶ ctx.edit: snapshot for undo, then
                                                scene-ops place on the working copy
AppModel.handle(.docChanged) ◀────────────── doc.changed { dirty, canUndo, … }
 └▶ requestRender, requestCheck ──────────▶ scene.render (PNG + item bounds), check
StageView draws the PNG, nearest-neighbor, at a whole zoom

⌘S → AppModel.save ───────────────────────▶ doc.save: validate, serialize, compare the
                                             file's hash on disk, write atomically
```

The app never reads or writes the art itself. It renders what the server sends (a transparent PNG at scale 1, with each item's bounds for hit-testing) and keeps view state (selection, zoom, tool) in its model.

## An agent's edit: proposals

```
agent ── tools/call scene_place ──▶ serve --mcp (a session in 'propose' mode)
agent ── tools/call doc_save ────▶ validate, then write proposals/<id>.json
                                   { id, number, doc, baseHash, source, summary, createdAt, agent, chat? }
                                   (nothing in the document changes)
       every tools/call ─────────▶ a line in activity.jsonl

Pixel Lab's serve --stdio watches the state dir ─▶ proposals.changed, activity.appended
  Agent window: the list, a diff (proposal.show), a ghost on the stage (proposal.render)
  Approve → proposal.apply: the file must still hash to baseHash (else it asks),
            validate, write; an open copy with unsaved edits gets it merged in
  Reject  → proposal.reject: delete the proposal

agent's next tools/call ─▶ session.sync(): its proposal is gone, so the doc is
                           settled against the file (clean, reloaded or rebased)
```

## The Agent chat: Claude Code in the app

The Agent tab (`2026-10-agent-chat`) runs the owner's own `claude` CLI, one headless process per message, with the same MCP server an agent would use:

```
app ── claude -p --input-format stream-json --output-format stream-json --verbose
          --mcp-config {pixellab: node <cli> serve --mcp --document <doc>, env PIXELLAB_STATE, PIXELLAB_CHAT, PIXELLAB_CHAT_TITLE}
          --strict-mcp-config --tools "" --allowedTools mcp__pixellab__* --disallowedTools mcp__pixellab__document_new
          --permission-prompts none --setting-sources "" --append-system-prompt <ChatPrompt> [--resume <session>]
    stdin:  <selection …> block (chat.context) + the message
    stdout: system/init, assistant text and tool_use, user tool_result, result ─▶ the transcript
```

The child runs in `<state dir>/chats/` (so `--resume` finds the session) with no `CLAUDE*` or `ANTHROPIC_*` variables, so it uses the CLI's own login. Its `serve --mcp` is in propose mode, so its saves are proposals like any agent's, with `chat: { id, title }` and a number (`proposal-numbers.log`); its activity lines carry `agent` and `chat`. Questions are a tool, `chat_ask`, that returns at once; the app shows its `tool_use` as buttons and sends the answer as the next message. Chats are `<state dir>/chats/<id>.json`. The tests run a fake `claude` (`scripts/fake-claude/`) that does its tool calls through the real MCP server.

Agents can't approve, reject, publish or save a copy: those commands are left out of the MCP tools (`LEFT_OUT` in `cli/lib/server-mcp.mjs`). `serve --mcp --direct` lets an agent write the document directly; it's off by default.

## Keeping working copies in step with disk

Two processes hold working copies of the same files: the app's `serve --stdio` and each agent's `serve --mcp`. The design is in [`2026-10-mcp-gaps/spec.md`](intents/2026-10-mcp-gaps/spec.md):

- **Edits since disk.** Every undo step has a sequence number, and each open doc remembers the number when it last matched its file (`baseSeq` in `session.mjs`). A proposal lists only the steps after it, so a second proposal doesn't repeat the first one's edits.
- **The MCP session settles.** The MCP server has no file watcher; before each `tools/call` it runs `session.sync()`: a doc whose proposal is gone is marked clean, reloaded or rebased onto the file, and a clean doc whose file changed reloads. `doc.reload` does the same on request, refusing when it would drop unsaved edits.
- **The app's server watches.** A clean open doc reloads when its file changes; a dirty one gets a `doc.stale` notification, and the app shows Merge, Reload and Keep Mine. Merge is `doc.merge`: a three-way line merge (`cli/lib/merge.mjs`) of the file as the doc was opened, the file now and the working copy, refused when both changed the same lines. `proposal.apply` merges into a dirty open copy the same way.
- **What the app has open.** The app's server keeps `open/<pid>.json` in the state dir (`cli/lib/open-docs.mjs`); an MCP session's `doc.status` lists the docs there with unsaved edits as `unsavedInApp`.

The **state dir** is `$PIXELLAB_STATE`, else `~/Library/Application Support/Pixel Lab/` (`cli/lib/state.mjs`). Tests and the script hook always point it at a temp folder.

## The Mac app

A SwiftPM executable (`app/`), SwiftUI with AppKit where a Mac editor needs it (the stage's pointer and keys, window close), macOS 15+. It bundles the CLI in `Contents/Resources/cli` and starts `node … serve --stdio --document <path>` (or `--site <checkout>`) through one `ServerConnection` actor; File › New, Open and Save As… switch the document by restarting it. `AppModel` holds the state, with an editor per mode (`SceneEditor`, `ObjectEditor`), the `TimelineModel` and the `AgentModel`; views read it from the environment. Colors come from the board 03 tokens in `Theme.swift`. More in [the app's README](../app/README.md).

## Where things are

| Folder | What | README |
| --- | --- | --- |
| `cli/` | The `pixellab` CLI, both servers and their tests | [cli/README.md](../cli/README.md) |
| `app/` | The SwiftUI app and its unit tests | [app/README.md](../app/README.md) |
| `scripts/` | Build, install, snapshot, acceptance, docs | [scripts/README.md](../scripts/README.md) |
| `docs/` | These guides, the command reference, each change's intent, spec and plan, screenshots | [docs/README.md](README.md) |
| `.github/workflows/` | The release and the docs site | [contributing](contributing.md#ci) |
| `.claude/` | Agent skills for the intent → spec → plan workflow, and the hook that keeps agents from merging into `main` | [contributing](contributing.md#how-changes-flow) |
