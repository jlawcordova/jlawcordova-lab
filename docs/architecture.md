# Architecture

How Pixel Lab fits together today. The design record, [`2026-10-mac-app/plan.md`](intents/2026-10-mac-app/plan.md), says why it's built this way and holds the look (board 03, "Field notebook"); this page is the map you need before changing the code. The engine and the program are Rust (`crates/`), since the change in [`2026-10-rust-engine`](intents/2026-10-rust-engine/spec.md); Node is left only as repo tooling. Each folder's README goes one level deeper: [the program](../crates/pixellab/README.md), [the app](../app/README.md), [the scripts](../scripts/README.md).

## Three doors onto one registry

Two Rust crates make the whole program. **`crates/engine`** (the `pixellab-engine` library) is the pixel-art engine: it validates a document, renders, orders and plays it, compiles scenes to SVGs and makes PNG previews. **`crates/pixellab`** is the `pixellab` executable, built on it. Every action Pixel Lab can take is a **command** in one registry (`crates/pixellab/src/registry.rs`, the commands in `crates/pixellab/src/commands/`). A command is a dotted name, a summary, its params as a JSON Schema object, whether it mutates, and a `run` function that takes the context and the params and returns plain JSON. Three doors call it:

```
a terminal ───── argv ─────────────▶ cli.rs ───────────┐
Pixel Lab.app ── JSON-RPC (stdio) ─▶ server_stdio.rs ──┼─▶ registry ─▶ command run(ctx, params)
an agent ─────── MCP (stdio) ──────▶ server_mcp.rs ────┘                    │
                                                        session.rs (working copies, undo,
                                                        save) and the engine (crates/engine)
```

- **The command line** (`pixellab scene place hero-island pine --at 14,6,1`): `argv.rs` turns words and flags into a command and params from the command's schema; `cli.rs` runs it in a fresh session and saves right away (one-shot edits), or prints a diff with `--dry-run`. While the app owns the document, it sends the command to the app instead (below).
- **The app's server** (`pixellab serve --stdio`): newline-delimited JSON-RPC 2.0, method names are the command names. It holds one long-lived session, so edits are instant, and it saves them itself a second after the last one (autosave). It pushes notifications: `doc.changed`, `files.changed`, `history.changed`, `activity.appended`. It owns its document, and the CLI and MCP servers send it their commands over a socket.
- **MCP** (`pixellab serve --mcp`): the same commands as MCP tools (`scene.place` → `scene_place`), hand-written on `serde_json`, with no MCP library. An agent's edits apply: through the app while it owns the document, else saved by the MCP server after each call.

All three check params with the same `check_params` (`crates/pixellab/src/params.rs`) and return the same JSON, so a command added once works everywhere. The full list of commands, the wire formats and the error codes are in the generated reference, [docs/cli.md](cli.md).

## The document is the source of truth, and the engine lives here

The art lives in a **`.pixellab` document**, a folder in **format 2**: `manifest.json`, `palette.json`, `objects/*.json`, `scenes/*.json` (the files are described in [Document format](document-format.md)). Documents written by the old Node CLI are format 1 (JavaScript sources); the engine reads those only to migrate them, with `pixellab document migrate` (or **Migrate…** in the app), which checks the migrated document builds byte-identical SVGs before it writes. Any other command on a format-1 document says so and how to migrate. The engine lives in this repo, in `crates/engine/src/`:

- `engine.rs`, `iso.rs`, `depth.rs`, `motion.rs`, `ambient.rs`, `serialize.rs`, `edit.rs`, `starter.rs`, `svg.rs`: load, validate, resolve, render, serialize, edit, the isometric grid, the depth order (`depth.rs`, below), the frame loops and build-ups, ambient loops and actions, the starters, layers → rect SVG.
- `compile.rs`: rect SVG → one `<path>` per color, the lossless check, the budgets, and compiling a scene or a whole document (what a site's `npm run art` did). Each SVG is **self-contained**: it carries the animation it needs in its own `<style>`, so a page needs no stylesheet. [The site contract](site-contract.md) is what a page may rely on.
- `preview.rs`, `png.rs`, `deflate.rs`: the review PNGs (an object on a tile, a scene at its viewBox, the palette's swatch sheet) on the document's card color; the PNG encoder, on `miniz_oxide`.
- `format.rs`, `jsnum.rs`, `migrate.rs`: format 2's JSON and its canonical text (what a source file looks like on disk, so a merge and a diff are a line per item), JavaScript's number printing (so outputs match the Node engine's byte for byte), and the format-1 reader.

The only crates are `serde`, `serde_json` (with `preserve_order`) and `miniz_oxide`.

`crates/pixellab/src/document.rs` resolves the document (`--document`, else `PIXELLAB_DOC`, else `~/.config/pixellab/config.json`, else `--site` / `PIXELLAB_SITE`: a site checkout, whose `*.pixellab` under `src/assets/pixel-art/` is the document), reads and checks its manifest (the publish folder, per-output budgets, preview and theme colors), and hands every command the engine's view of it. Sources are read fresh from disk on each load, so an edit on disk is picked up by the next one. Every write validates with the engine, serializes with the engine and writes atomically (a temp file, then a rename). The SVGs a site inlines are written only by `art.build` (Publish), into the manifest's `publish.dir` or `--dir`.

The one piece of editing logic that didn't come from the engine, the scene edits of the site's old web lab (place, move, duplicate, remove, snap; raise and lower are gone, see depth below), is in `crates/pixellab/src/scene_ops.rs`. A scene can also name its parts as **layers** (`2026-10-scene-layers`): a `layers` list of `{ id, name, parent }` and a `layer` on each placed item, checked and written by the engine, edited by `layer_ops.rs` (`scene.layer.*`). They're bookkeeping only: no layer edit reorders the items for the sake of the layers, the engine draws a scene the same with or without them, and they're apart from the `{ group, items }` wrappers that become the SVG's `<g>`s. Which layers the app hides or locks is the app's view state (`layer-view.json` in the state dir), never part of the document. Motion edits (tracks, keys, paths) are in `motion_ops.rs`. Objects can carry a **category** (`2026-10-object-libraries`), a list of names the engine checks and writes after `notes` and never reads when it renders or compiles; `object.category` sets it, and the app's Library popover groups by it.

Documents share objects and colors by copying, through two more file kinds (`crates/pixellab/src/library.rs`, [Libraries and palette files](libraries.md)): a **`.pixellib`** package (`library.json` with a record of the colors its objects used, and `objects/*.json`) and a **`.pixelpal`** palette file. `library.export` and `palette.export` write them; `library.import` and `palette.import` plan first (renames, identical objects, the `character` swap, differing and missing colors, the tiers' caps), refuse with that plan until every difference is answered, and then copy. A library import never touches the palette, the existing objects or the scenes, and is written as one version, the way `history.restore` writes, so `history.undo` takes it back whole; a palette import is one edit on the session's `palette:` doc. A library has no session or history: `library.category` edits it in place under a file lock.

The engine is no longer a package a site installs: a site publishes with the `pixellab` program (`art build`) and reads the SVGs' [contract](site-contract.md). A change to the engine is a change here, released by tag.

## Depth: where each item stands decides what draws first

A scene's items draw back to front by where they stand, and nobody lists them in order by hand (`2026-10-depth-order`). The rules live in `crates/engine/src/depth.rs`, pure geometry that the engine calls:

- **A box in tile space.** Each item stands in a box worked out from its placement (a tile `col, row, level`, or a pixel point plus a `level`, read in sub-tile steps) and its object: a block's `size`, a sprite's `footprint: [wide, deep]` (default `[1, 1]`) and height. A `{ group, items }` wrapper's box is the union of its members', and a run of consecutive variant groups (`data-class`) sorts as one unit.
- **Behind.** Of two items whose drawn pixels could overlap, the one whose box ends before the other's starts on any axis draws first. Items that don't overlap keep the list's order, a cycle is broken by a fixed key, and sorting a sorted list changes nothing.
- **Overrides.** An item or group may carry `depth: 'front'` or `depth: 'back'`, which puts it after or before everything else in its own list. It is the one manual control; `scene.raise`, `scene.lower` and `scene.place --index` are gone.
- **Two places keep it right.** The engine sorts every list when it draws (`render_scene`, the compiler, the previews, `scene.render`), so what's published is right whatever order a file lists. And every scene edit sorts the whole scene in the same undo step (`sort_scene` in `sorting.rs`, run by the session's edit), so the file, `scene.items` and the Layers panel read back to front. A result names the paths the sort moved (`reordered`) so a caller holding other paths, such as the app's selection, can follow them. `scene.sort` and `check`'s `out_of_order` warning cover a file that isn't in order.
- **Motion.** A moving item has, at each moment of the loop, the depth of where it is then, and the published SVG draws it once for each place it takes in its list, each copy shown only while it's in that place. The list and the Layers panel keep it where it rests. `check` reports the copies, since they add to the SVG's size.

## An edit in the app, end to end

```
StageInputView (a click)                   pixellab serve --stdio
 └▶ SceneEditor.place(…)
     └▶ connection.call("scene.place") ───▶ scene.place run(ctx)
                                             └▶ ctx.edit: snapshot for undo, then
                                                scene_ops place on the working copy
AppModel.handle(.docChanged) ◀────────────── doc.changed { dirty, canUndo, saveState, undoLabel, author, … }
 └▶ requestRender, requestCheck ──────────▶ scene.render (PNG + item bounds), check
StageView draws the PNG, nearest-neighbor, at a whole zoom

a second later, the server's tick ─────────▶ autosave: validate, serialize, compare the
                                             file's hash on disk, write atomically, and
                                             record the version (history.rs)
⌘S → AppModel.save ───────────────────────▶ doc.save: the same, now, and close the version
```

The app never reads or writes the art itself. It renders what the server sends (a transparent PNG at scale 1, with each item's bounds for hit-testing) and keeps view state (selection, zoom, tool) in its model.

## An agent's edit, the CLI's, and one writer

`2026-10-autosave`. While the app has a document open, its server is the only process that writes it:

```
serve --stdio ── File::try_lock owners/<key>.lock ─▶ held for its life; the OS lets go when it ends
             └── binds owners/<key>.sock (0600)   ─▶ a listener thread feeds its event loop

agent ── tools/call scene_place ──▶ serve --mcp ── finds the lock held (owner.rs, client.rs)
                                         └── client.hello { kind: mcp, name, chat } ─▶ the app's server
                                         └── scene.place ────────────────────────────▶ runs in the app's session,
                                                                                         author: the agent
                                    doc.changed { author: agent, undoLabel: "… (Claude)" } ─▶ the stage redraws
                                    the app's autosave ─▶ the file, and a version by the agent
a terminal ── pixellab scene place ─▶ the same, kind: cli; the app saves before it answers

no app owns it ─▶ serve --mcp and the CLI open it themselves and save after each command
```

A client undoes only its own latest step (`not-yours`), can't revert the app's copy, and a call cut off by the app quitting fails with `owner-gone` and isn't retried. A second `serve --stdio` on an owned document answers `owned` with the owner's app pid, and the app brings that copy forward. Every write is recorded in the document's **version history** in the state dir (`history.rs`: a version per stretch of one author's writes, blobs by hash, thinned with age), which `history.*` lists, renders, restores and undoes, and which the app's Versions panel shows.

## The Agent chat: Claude Code in the app

The Agent tab (`2026-10-agent-chat`) runs the owner's own `claude` CLI, one headless process per message, with the same MCP server an agent would use:

```
app ── claude -p --input-format stream-json --output-format stream-json --verbose
          --mcp-config {pixellab: <program> serve --mcp --document <doc>, env PIXELLAB_STATE, PIXELLAB_CHAT, PIXELLAB_CHAT_TITLE}
          --strict-mcp-config --tools "" --allowedTools mcp__pixellab__* --disallowedTools mcp__pixellab__document_new
          --permission-prompts none --setting-sources "" --append-system-prompt <ChatPrompt> [--resume <session>]
    stdin:  <selection …> block (chat.context) + the message
    stdout: system/init, assistant text and tool_use, user tool_result, result ─▶ the transcript
```

The child runs in `<state dir>/chats/` (so `--resume` finds the session) with no `CLAUDE*` or `ANTHROPIC_*` variables, so it uses the CLI's own login. Its `serve --mcp` sends its edits through the app, as any agent's are, and they're versions by the agent with `chat: { id, title }`; its activity lines carry `agent` and `chat`. A turn that changed something ends with a change card in the chat: the app closes the version (`history.mark`) and lists the chat's versions since the turn began. Questions are a tool, `chat_ask`, that returns at once; the app shows its `tool_use` as buttons and sends the answer as the next message. Chats are `<state dir>/chats/<id>.json`. The tests run a fake `claude` (`scripts/fake-claude/`) that does its tool calls through the real MCP server.

Agents can't publish or save a copy: those commands are left out of the MCP tools (`LEFT_OUT` in `crates/pixellab/src/server_mcp.rs`). `serve --mcp --direct` is accepted and ignored.

## Keeping working copies in step with disk

With one writer per document there's one working copy of an open document, the app's. What remains is changes from outside Pixel Lab (git, an editor) and two MCP servers with no app (`2026-10-mcp-gaps/spec.md`, `2026-10-autosave/spec.md`, R7):

- **Edits since disk.** Every undo step has a sequence number, and each open doc remembers the number when it last matched its file (`base_seq` in `session.rs`). A version lists the steps after it.
- **The MCP session follows the file.** With no owner, before each `tools/call` the MCP server runs `session.sync()`: a clean doc whose file changed reloads. `doc.reload` does the same on request, refusing when it would drop unsaved edits.
- **The app's server watches** (`watch.rs`, by polling the document's files and the state dir every 250 ms from the server's event loop, no extra thread). A clean open doc reloads when its file changes (recorded as a version from outside Pixel Lab); one with a save pending gets the change merged in (`doc.merge`: a three-way line merge, `merge.rs`, of the file as the doc was opened, the file now and the working copy) and saves. When both changed the same lines, the doc is blocked and a `doc.stale` notification brings up Reload and Keep Mine.

The **state dir** is `$PIXELLAB_STATE`, else `~/Library/Application Support/Pixel Lab/` (`state.rs`). Tests and the script hook always point it at a temp folder.

## The Mac app

A SwiftPM executable (`app/`), SwiftUI with AppKit where a Mac editor needs it (the stage's pointer and keys, window close), macOS 15+. It bundles the program, built from `crates/`, at `Contents/Helpers/pixellab` (not in `Contents/MacOS`, where on a case-insensitive volume `pixellab` would be the app's own `PixelLab`) and starts `pixellab serve --stdio --document <path>` (or `--site <checkout>`) through one `ServerConnection` actor; File › New, Open and Save As… switch the document by restarting it. `AppModel` holds the state, with an editor per mode (`SceneEditor`, `ObjectEditor`), the `TimelineModel` and the `AgentModel`; views read it from the environment. Colors come from the board 03 tokens in `Theme.swift`. More in [the app's README](../app/README.md).

## Where things are

| Folder | What | README |
| --- | --- | --- |
| `crates/engine/` | The engine library: validate, render, depth, motion, compile, previews, format 2 | [the engine](#the-document-is-the-source-of-truth-and-the-engine-lives-here) |
| `crates/pixellab/` | The `pixellab` program: the registry and commands, the CLI, both servers, and their tests | [crates/pixellab/README.md](../crates/pixellab/README.md) |
| `app/` | The SwiftUI app and its unit tests | [app/README.md](../app/README.md) |
| `scripts/` | Build, install, snapshot, acceptance, docs, the repo's Node tests | [scripts/README.md](../scripts/README.md) |
| `docs/` | These guides, the command reference, each change's intent, spec and plan, screenshots | [docs/README.md](README.md) |
| `.github/workflows/` | The release and the docs site | [contributing](contributing.md#ci) |
| `.claude/` | Agent skills for the intent → spec → plan workflow, and the hook that keeps agents from merging into `main` | [contributing](contributing.md#how-changes-flow) |
