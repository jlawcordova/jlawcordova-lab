# Pixel Lab for Mac: architecture and build plan

> This plan predates the intent → spec → plan workflow (`CLAUDE.md`, "How changes flow"), so it has no `intent.md` or `spec.md` and doesn't follow the `write-plan` template. It stays as the architecture and design record that later specs build on. Its "How we build it" section (`slice/<name>` branches off `feat/mac-app`) is superseded by the `write-plan` skill's "Branches".
>
> **Superseded in part (2026-10-19):** "The site is the source of truth" and the runtime import of the engine from the site checkout are replaced by [`../2026-10-pixellab-document/spec.md`](../2026-10-pixellab-document/spec.md): the engine lives in this repo (`cli/lib/engine/`), the art is a `.pixellab` document, and **Build art** became **Publish** (`art build`). The rest of this record still holds.

> **Status (2026-10-19): all seven slices are done** (PRs #1–#7 into `feat/mac-app`); the final PR `feat/mac-app` → `main` is open for review. Every item of "What ready to use means" below was verified with scripted runs on a temp copy of the site and read-only snapshots of the real one (the checklist is in the final PR). Install with `scripts/install.sh`; the README has the tour, shortcuts, MCP setup and troubleshooting. This file stays as the design record.
>
> Follow-ups, not blocking day-to-day use:
> - Notarization / a Developer ID signature (the app is ad-hoc signed, so the first launch needs Control-click › Open).
> - Edits to one item within a second merge into one undo step (meant for held keys; two quick, different moves undo together).
> - Place level is manual (the stepper or the selected item's level); it doesn't infer the top of the block under the pointer.
> - Hit-testing uses each object's frame-0 art, so a pixel that only shows in another loop frame can miss.
> - Library drag-and-drop, Space-panning and the Save/Don't Save prompt on quit are wired but only exercised by hand, not by the script hook.
> - The Agent window's activity list opens scrolled so its first visible row is partly clipped; live updates while the app runs were verified in slice 6, not re-run here.
> - The dark-theme shadow recolor follows the site's `[data-enter="shadow"]` rule in `hero.css`; if the site adds other dark-theme pixel rules, `render.mjs` needs the same.

A native macOS editor for the pixel art in J. Law's site (`jlawcordova.github.io`), built CLI first so agents can do everything a person can. It replaces the web lab at `/lab/pixel-art/` for day-to-day editing.

The look was chosen on the mood-board canvas (https://claude.ai/artifact/QW1yk9qUyV4SYKabHQ6Gtu): **board 03, Field notebook**. Agent tooling lives in a **separate Agent window**, opened on demand from the View and Window menus.

## What "ready to use" means

J. Law can, without a terminal:

1. Launch `Pixel Lab.app`, pick the site checkout once (default `~/Projects/jlawcordova.github.io`), and see every scene and object.
2. **Scene mode:** open a scene, see it rendered crisply at 1×–4× and Fit on the isometric grid, pick from the library, place on a tile, select by clicking, move by column, row, level (or x, y for pixel-placed items), raise, lower, duplicate, remove and snap. Undo and redo everything. Play the scene's animation and scrub its timeline.
3. **Object mode:** open an object, paint with the palette (pencil, eraser, fill, picker), switch layers and frames, see a live preview.
4. See problems (validation) live in a Checks panel, then **Save** (⌘S) to write the `.mjs` source, and **Build art** to run the site's `npm run art` and show its report.
5. Open **Window › Agent** (⌥⌘A) to watch what an agent did over MCP and approve or reject its proposed edits, which show as ghosts on the stage first.

And an agent can do the same through `pixellab` on the command line or through `pixellab serve --mcp`.

## Repository layout

```
cli/                 Node ≥ 22, ESM, zero dependencies (Node built-ins only)
  bin/pixellab.mjs   entry point
  lib/               site loading, command registry, scene and object ops, servers
  test/              node:test suites (`npm test` at the repo root)
app/                 SwiftPM package for the Mac app (SwiftUI + AppKit), macOS 15+
  Package.swift
  Sources/PixelLab/
scripts/
  build-app.sh       swift build → "build/Pixel Lab.app" (Info.plist, icon, CLI copied into Resources, ad-hoc signed)
docs/plan.md         this file (now docs/intents/2026-10-mac-app/plan.md)
```

## The site is the source of truth

- The art lives in the site: `src/assets/pixel-art/source/{palette.mjs,objects/*.mjs,scenes/*.mjs}`.
- The engine lives in the site too: `src/lib/pixel-art/*.mjs` (engine, edit, iso, motion, serialize, starter, svg) and `scripts/pixel-art-preview.mjs` (PNG encoding, Node built-ins only). It is plain zero-dependency ESM.
- **The CLI imports the engine from the site checkout at runtime** (dynamic `import()` of file URLs under the site root). We never copy or port it, so the editor always validates and serializes with exactly the engine that `npm run art` uses.
- The site root comes from `--site <path>`, else `PIXELLAB_SITE`, else `~/.config/pixellab/config.json` (`{"site": "…"}`), else `~/Projects/jlawcordova.github.io`. A missing or wrong root is a clear error naming what was expected.
- Sources are loaded with `import(url + '?v=' + mtimeMs)` so edits on disk are picked up.
- Writes: validate with the engine first, serialize with the engine's `serialize`, write atomically (temp file + rename). Never touch generated SVGs; `art build` runs `npm run art` in the site for that.
- **Never write to the real site checkout in tests.** Tests copy the site's `src/lib/pixel-art/`, `scripts/pixel-art-preview.mjs`, `src/styles/variables.css` and `src/assets/pixel-art/source/` into a temp dir and point `--site` at it.
- Scene editing logic (place, move, raise, lower, duplicate, remove, snap, insert order) currently lives in the web lab's client code, `src/components/lab/scene-mode.ts`, not the engine. Port it to `cli/lib/scene-ops.mjs` and keep the behavior the same (for example `insertIndex` keeps tile items sorted by row + col, then level). Object painting uses the engine's `edit.mjs`.

## One command registry, three doors

Every action is a command in `cli/lib/commands/`:

```js
export default {
  name: 'scene.place',              // dotted, stable; also the MCP tool name with '.' → '_'
  summary: 'Place an object on a tile in a scene',
  params: { /* JSON Schema object */ },
  mutates: true,
  run: async (ctx, params) => result // plain JSON
};
```

- **CLI:** `pixellab <group> <verb> [args] [--json] [--site …]`, e.g. `pixellab scene place hero-island pine --at 14,6,1`. Human output by default, the command's JSON with `--json`. Exit code 0 ok, 1 command error, 2 usage error. `pixellab help` and `pixellab <group> --help` list commands from the registry.
- **App server:** `pixellab serve --stdio` speaks newline-delimited JSON-RPC 2.0. Method names are the command names. It holds a **session**: open documents in memory with undo and redo stacks, so the app's edits are instant; `doc.save` writes. It sends notifications (`doc.changed`, `proposals.changed`, `activity.appended`).
- **MCP:** `pixellab serve --mcp` speaks MCP over stdio (JSON-RPC 2.0: `initialize`, `notifications/initialized`, `ping`, `tools/list`, `tools/call`; protocol version `2025-06-18`), hand-written on Node built-ins. Tools are the registry's commands. Register with Claude Code using `claude mcp add pixellab -- node <repo>/cli/bin/pixellab.mjs serve --mcp`.

Core commands (names may be refined, but keep the shape):

- `site.info`, `palette.get`
- `scene.list`, `scene.show`, `scene.items`, `scene.render`, `scene.place`, `scene.move`, `scene.raise`, `scene.lower`, `scene.duplicate`, `scene.remove`, `scene.snap`, `scene.set` (class, id), `scene.new`
- `object.list`, `object.show`, `object.render`, `object.paint` (pixels + color, layer, frame), `object.fill`, `object.layer.*`, `object.frame.*`, `object.new`
- `check` (engine validation problems for one doc or all), `art.build` (runs `npm run art`, returns its report)
- `doc.open`, `doc.undo`, `doc.redo`, `doc.save`, `doc.revert` (session modes)
- `proposal.list`, `proposal.show`, `proposal.apply`, `proposal.reject`; `activity.list`

Rendering for the app: `scene.render` and `object.render` return `{ png: base64, scale, viewBox, width, height, items: [{ path, object, at, bounds: [x, y, w, h] }] }` at an integer scale, using the engine's `renderScene`/`renderObject` + `composite` (with `time` for animation frames) and the PNG encoder from `scripts/pixel-art-preview.mjs`. Transparent background; the app draws the grid. Item bounds let the app hit-test clicks.

## Agents propose, people approve

- In MCP mode, mutating tools edit an in-memory working copy, and each edit (or a `proposal.submit`) writes a **proposal** to the state dir: `~/Library/Application Support/Pixel Lab/proposals/<id>.json` with `{ id, doc: "scene:hero-island", baseHash, source, summary, createdAt, agent }`. Nothing touches the site until a person approves. `--direct` lets MCP write straight to the site (off by default).
- Every MCP tool call appends a line to `~/Library/Application Support/Pixel Lab/activity.jsonl`: `{ time, tool, args, ok, summary }`.
- `PIXELLAB_STATE` overrides the state dir (tests use a temp dir).
- The app watches the state dir. The Agent window lists activity and proposals; selecting a proposal shows it as ghosts on the stage (rendered proposed doc over the current one) with a source diff; Approve = `proposal.apply` (checks `baseHash`, writes the source), Reject = delete.

## The Mac app

- SwiftPM executable target `PixelLab`, SwiftUI app lifecycle with AppKit where needed, macOS 15+. Build with `swift build -c release` (works with Command Line Tools; `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer` is available if a tool needs full Xcode, never `xcode-select -s`). `scripts/build-app.sh` makes `build/Pixel Lab.app`: Info.plist (bundle id `com.jlawcordova.pixellab`), a pixel-art app icon (render one with the engine, e.g. the house or the island, into an `.icns` via `iconutil`), the `cli/` folder in `Contents/Resources/cli`, ad-hoc `codesign -s -`.
- The app starts `node Contents/Resources/cli/bin/pixellab.mjs serve --stdio --site <path>` and talks JSON-RPC to it through one `ServerConnection` actor. Find `node` from Settings, else `zsh -lc 'command -v node'`, else `/opt/homebrew/bin/node` and `/usr/local/bin/node`. Show a friendly setup screen when node or the site can't be found.
- Windows: the document window (one per open scene or object, or one window with a document picker; follow Mac conventions), **Agent** (Window and View menus, ⌥⌘A), Settings (site path, node path, appearance: Auto, Light, Dark).
- Pixel art is always drawn with nearest-neighbor (`.interpolation(.none)`), at integer zoom, on the isometric grid (32×16 diamond lattice).
- Keyboard: V select, A place, H pan, B pencil, E eraser, G fill, I picker, 1–4 zoom, 0 fit, ⌘Z / ⇧⌘Z, ⌘S save, ⌘B build art, arrows nudge the selection by a tile (⌥ for a pixel), ⌫ remove.
- VoiceOver labels on controls, visible focus, 44 pt-ish hit targets on custom controls, honors Reduce Motion.

### Board 03 look ("Field notebook")

A real Mac window with system controls, rounded corners and SF Pro for controls, and **SF Symbols** for icons (the owner asked for Mac-native icons, not the site's pixel glyphs). Inside it, the site's warm palette:

| Token | Night (dark) | Day (light) |
| --- | --- | --- |
| page (window ground) | `#1C1712` | `#F4EDE0` |
| hero (toolbar, stage) | `#262019` | `#EADFC8` |
| sidebar / inspector | `#211B15` | `#EFE6D6` |
| card (thumbnails, fields) | `#2D251C` | `#E9DCC6` |
| ink (text) | `#F1E8D8` | `#2E2418` |
| ink-muted | `#C8B9A1` | `#5C4B39` |
| border | ink at 16% | `rgba(90,62,43,.16)` |
| accent (selection, primary button, app tint) | `#8CBC86` | `#3F6B45` |
| gold (playhead, warnings) | `#D8B66A` | `#D8B66A` (fills only, never text on light) |
| grid lines | ink at 6% | `rgba(90,62,43,.06–.10)` |

- Type: **Sora** for document and item names and headings, **Silkscreen** only for short uppercase section labels (SCENES, LIBRARY, SELECTED, CHECKS, TIMELINE), SF Pro for every control, SF Mono for coordinates and code. Sora and Silkscreen aren't installed on this Mac: register them from `app/Sources/PixelLab/Resources/Fonts/` if the files are there, else fall back to SF Pro (names) and SF Mono uppercase (labels). Don't download fonts.
- Layout: unified toolbar (title + draft state, Scene/Object segmented control, tool group, zoom, Export/Save as the one accent button), left sidebar (Scenes list, Library grid of object thumbnails), center square-cornered stage with the iso grid and a timeline strip under it, right inspector (Selected, Position, Checks).
- Only the pixel art is colorful. One accent, used sparingly.

## How we build it

- `main` holds only this plan and the scaffolding. All work lands on the integration branch **`feat/mac-app`**.
- Each slice is one subagent on its own branch `slice/<name>` off the latest `feat/mac-app`. It commits as it goes, opens a PR **into `feat/mac-app`**, pastes its verification output in the PR body, and merges it (`gh pr merge --squash --delete-branch`) once its checks pass.
- The final PR is `feat/mac-app` → `main`, opened for J. Law to review and merge. Agents never merge into `main`.
- Commit messages: imperative, with a body when useful, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. PR bodies end with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- No new dependencies (npm or Swift packages) without asking.
- Verification each slice runs and pastes: `npm test` (`# fail 0`), `swift build` (no errors) once the app exists, and for app slices `scripts/build-app.sh` plus a screenshot of the running app.

### Slices

1. **CLI core:** site resolution, engine loading, command registry, CLI parser and help, read-only commands (`site.info`, `palette.get`, `scene.list/show/items/render`, `object.list/show/render`, `check`), PNG rendering, test harness with a temp copy of the site.
2. **Editing and servers:** `scene-ops.mjs` port and scene edit commands, object paint commands, writes, `art.build`, session with undo and redo, `serve --stdio`, `serve --mcp`, proposals and activity log, docs for registering the MCP server.
3. **App shell:** SwiftPM app, `build-app.sh`, app icon, server connection, setup screen, Settings, board 03 theme (day and night), document window with sidebar, scene list and rendered stage with zoom and grid.
4. **Scene editing in the app:** library, place, select by click, inspector fields, nudge keys, raise, lower, duplicate, remove, snap, undo and redo, save, checks panel, build art, timeline playback and scrubbing.
5. **Object mode in the app:** palette, pencil, eraser, fill, picker, layers, frames, onion skin if cheap, live preview, new scene and new object.
6. **Agent window:** Window and View menu items (⌥⌘A), activity list, proposals with ghost preview and diff, approve and reject, live updates.
7. **Finish:** end-to-end pass with the real site (read-only unless saving to a scratch copy), README with install and MCP setup, polish, final PR.
