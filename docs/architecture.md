# Architecture

How Pixel Lab fits together today. The design record, [`2026-10-mac-app/plan.md`](intents/2026-10-mac-app/plan.md), says why it's built this way and holds the look (board 03, "Field notebook"); this page is the map you need before changing the code. Each folder's README goes one level deeper: [the CLI](../cli/README.md), [the app](../app/README.md), [the scripts](../scripts/README.md).

## Three doors onto one registry

Every action Pixel Lab can take is a **command** in one registry (`cli/lib/registry.mjs`, the commands in `cli/lib/commands/`). A command is a dotted name, a summary, its params as a JSON Schema object, whether it mutates, and `run(ctx, params)`, which returns plain JSON. Three doors call it:

```
a terminal ───── argv ─────────────▶ cli.mjs ──────────┐
Pixel Lab.app ── JSON-RPC (stdio) ─▶ server-stdio.mjs ─┼─▶ registry ─▶ command.run(ctx)
an agent ─────── MCP (stdio) ──────▶ server-mcp.mjs ───┘                    │
                                                        session.mjs (working copies, undo,
                                                        save) and the site's engine
```

- **The command line** (`pixellab scene place hero-island pine --at 14,6,1`): `argv.mjs` turns words and flags into a command and params from the command's schema; `cli.mjs` runs it in a fresh session and saves right away (one-shot edits), or prints a diff with `--dry-run`.
- **The app's server** (`pixellab serve --stdio`): newline-delimited JSON-RPC 2.0, method names are the command names. It holds one long-lived session, so edits are instant and stay in memory until `doc.save`. It pushes notifications: `doc.changed`, `files.changed`, `proposals.changed`, `activity.appended`.
- **MCP** (`pixellab serve --mcp`): the same commands as MCP tools (`scene.place` → `scene_place`), hand-written on Node built-ins. Its session saves to **proposals**, not the site.

All three check params with the same `checkParams()` (`cli/lib/params.mjs`) and return the same JSON, so a command added once works everywhere. The full list of commands, the wire formats and the error codes are in the generated reference, [docs/cli.md](cli.md).

## The site is the source of truth

The art and the engine live in the site checkout (`~/Projects/jlawcordova.github.io` by default), never here:

- **Art:** `src/assets/pixel-art/source/` (`palette.mjs`, `objects/*.mjs`, `scenes/*.mjs`).
- **Engine:** `src/lib/pixel-art/*.mjs` (validate, render, serialize, edit, iso, motion, starters) and `scripts/pixel-art-preview.mjs` (PNG encoding).

`cli/lib/site.mjs` resolves the checkout (`--site`, else `PIXELLAB_SITE`, else `~/.config/pixellab/config.json`, else the default) and `import()`s the engine from it at runtime. Sources load with `?v=<mtime>-<size>`, so an edit on disk is picked up by the next load. Every write validates with the engine, serializes with the engine and writes atomically (a temp file, then a rename). Generated SVGs are only ever written by the site's own `npm run art` (`art.build`).

The one piece of editing logic that isn't in the engine, the scene edits of the web lab (place, move, raise, lower, duplicate, remove, snap), is ported into `cli/lib/scene-ops.mjs`, each function naming the lab function it follows. A change that needs the engine or the art to change belongs in the site repo.

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
                                   { id, doc, baseHash, source, summary, createdAt, agent }
                                   (nothing in the site changes)
       every tools/call ─────────▶ a line in activity.jsonl

Pixel Lab's serve --stdio watches the state dir ─▶ proposals.changed, activity.appended
  Agent window: the list, a diff (proposal.show), a ghost on the stage (proposal.render)
  Approve → proposal.apply: the file must still hash to baseHash (else it asks),
            validate, write
  Reject  → proposal.reject: delete the proposal
```

Agents can't approve, reject or build: those commands are left out of the MCP tools (`LEFT_OUT` in `cli/lib/server-mcp.mjs`). `serve --mcp --direct` lets an agent write the site directly; it's off by default.

The **state dir** is `$PIXELLAB_STATE`, else `~/Library/Application Support/Pixel Lab/` (`cli/lib/state.mjs`). Tests and the script hook always point it at a temp folder.

## The Mac app

A SwiftPM executable (`app/`), SwiftUI with AppKit where a Mac editor needs it (the stage's pointer and keys, window close), macOS 15+. It bundles the CLI in `Contents/Resources/cli` and starts `node … serve --stdio --site <path>` through one `ServerConnection` actor. `AppModel` holds the state, with an editor per mode (`SceneEditor`, `ObjectEditor`), the `TimelineModel` and the `AgentModel`; views read it from the environment. Colors come from the board 03 tokens in `Theme.swift`. More in [the app's README](../app/README.md).

## Where things are

| Folder | What | README |
| --- | --- | --- |
| `cli/` | The `pixellab` CLI, both servers and their tests | [cli/README.md](../cli/README.md) |
| `app/` | The SwiftUI app and its unit tests | [app/README.md](../app/README.md) |
| `scripts/` | Build, install, snapshot, acceptance, docs | [scripts/README.md](../scripts/README.md) |
| `docs/` | These guides, the command reference, each change's intent, spec and plan, screenshots | [docs/README.md](README.md) |
| `.github/workflows/` | The release and the docs site | [contributing](contributing.md#ci) |
| `.claude/` | Agent skills for the intent → spec → plan workflow, and the hook that keeps agents from merging into `main` | [contributing](contributing.md#how-changes-flow) |
