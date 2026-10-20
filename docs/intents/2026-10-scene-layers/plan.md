# Plan: Scene layers (from intent.md 2026-10-20)

## Files that change

**PR 1, CLI (`feat/scene-layers-cli`)**
- `cli/lib/engine/engine.mjs`: validate `layers` and an item's `layer` (D1); rendering unchanged.
- `cli/lib/engine/serialize.mjs`: write `layers` (one per line, before `items`) and an item's `layer` (before `at`).
- `cli/lib/layer-ops.mjs` (new): find a layer, chains, common parent, `newLayer`, `renameLayer`, `ungroupLayer`, `assignLayer`, `itemsInLayers`, `layerOfTarget`, ids from names.
- `cli/lib/scene-ops.mjs`: `reorderKeys` keeps `layer`; `place` and `placeInGroup` take a layer.
- `cli/lib/commands/scene-layers.mjs` (new): `scene.layer.new|rename|ungroup|assign|move`.
- `cli/lib/commands/scene-edit.mjs`: `scene.place --layer`.
- `cli/lib/commands/scene.mjs`: `scene.items` layers, `scene.show` layers, `scene.render --hide --focus`.
- `cli/lib/render.mjs`: `flattenItems` adds `layer` and `layers`; `renderSceneImage` takes `hide` and `focus`.
- `cli/lib/registry.mjs`: the new command file.
- `cli/lib/server-mcp.mjs`: instructions name the layer tools.
- `cli/lib/merge.mjs`, `cli/lib/session.mjs` (as built, after the independent review): `foldScene()` folds both sides' first `layers` lists in a merge; `Session.merge` refuses a clash or a merge that adds problems.
- `cli/README.md`: the new module and command file.
- `cli/test/layers.test.mjs` (new): R1–R5. `cli/test/merge.test.mjs`: `foldScene`. `cli/test/help.test.mjs`, `cli/test/params.test.mjs`: the command counts and lists.
- `docs/cli.md` (generated).

**PR 2, the app (`feat/scene-layers-app`)**
- `app/Sources/PixelLab/Server/Protocol.swift`: `SceneItemInfo.layer`, `SceneLayerInfo`, `SceneItemsResult.layers`, `RenderResult.focusPng`, `EditResult.layer` decoding of a `{ id, name }` object (named `layerInfo`).
- `app/Sources/PixelLab/Model/LayerTree.swift` (new): the tree, chains, rows, picking rules, placement target, Moved by.
- `app/Sources/PixelLab/Model/LayersModel.swift` (new): inside, expanded, hidden, locked (and `layer-view.json`), hover, menu, drafts, baseline, the actions (group, ungroup, rename, hide, lock, go inside, move the selection).
- `app/Sources/PixelLab/Model/DocumentModel.swift`: `selection`, computed `selectedPath`, `layerTree`, `focusImage` on `StageImage`; `TreeRow` removed.
- `app/Sources/PixelLab/Model/SceneEditor.swift`: clicks, double-clicks, ⇧, drags and keys through `LayersModel`; `sidebarTab` `.layers` first; `collapsedGroups` removed.
- `app/Sources/PixelLab/Model/AppModel.swift`: `layers`; render params `hide` and `focus`; `focusPng`.
- `app/Sources/PixelLab/Views/LayersPanel.swift` (new; replaces `ItemsTree.swift`, deleted): rows, keyboard, VoiceOver, right-click catcher.
- `app/Sources/PixelLab/Views/StageMenu.swift` (new): the right-click menu.
- `app/Sources/PixelLab/Views/LayerInspector.swift` (new): the layer and several-things inspectors, Moved by, nudges, the item's layer line.
- `app/Sources/PixelLab/Views/SidebarView.swift`: tabs, no counts, the Library chip and note.
- `app/Sources/PixelLab/Views/StageView.swift`: the fade, the pills, the menu overlay.
- `app/Sources/PixelLab/Views/StageOverlay.swift`: layer and item outlines, hover, the inside box, the multi-item drag ghost and its tag.
- `app/Sources/PixelLab/Views/StageInput.swift`: right-click, ⇧ and double-click.
- `app/Sources/PixelLab/Views/SelectionInspector.swift`, `InspectorView.swift`: the item's layer line and wrapper line; which inspector shows.
- `app/Sources/PixelLab/PixelLabApp.swift`: Edit › Group ⌘G, Ungroup ⇧⌘G.
- `app/Sources/PixelLab/Views/MainWindow.swift`: the layer menu's overlay over the window, and `AccentButtonStyle(height:)` for the pill's Done.
- `app/Sources/PixelLab/Support/ScriptRunner.swift`: D14's actions and expects.
- `app/Tests/PixelLabTests/LayerTests.swift` (new): the tree, picking, targets, rows and keys, Moved by.
- `scripts/acceptance/layers.txt` (new), `scripts/acceptance.sh`: the layers pass and its byte-identity check.
- Docs: `README.md`, `app/README.md`, `cli/README.md`, `docs/architecture.md`, `docs/testing.md`, `CLAUDE.md`, `docs/screenshots/layers-*.png` (new), and this plan's "As built".

## Order of work

```
main ── feat/scene-layers ─ (intent, spec, plan) ──────────────── PR 3 (owner merges) ──▶ main
          ├─ PR 1  CLI and engine  ◀─ feat/scene-layers-cli    (steps 2–4)
          └─ PR 2  the app         ◀─ feat/scene-layers-app    (steps 5–11, cut after PR 1 merges)
```

1. This intent, spec and plan, committed straight onto `feat/scene-layers` (the owner asked for no separate PR for them).

**PR 1: steps 2–4**

2. The engine: validate and serialize `layers` and `layer` (D1), with tests of every rule and a serialize round trip.
3. `layer-ops.mjs`, the five `scene.layer.*` commands, `scene.place --layer`, `scene.items` / `scene.show` layers, `scene.render --hide --focus`, MCP instructions; tests R1–R5, including the byte-identical `art build` (R2), `scene group set` on a layered scene (R3), a proposal of layer edits over MCP (R5).
4. `npm run docs:cli`; PR 1's checks; merge into `feat/scene-layers`.

**PR 2: steps 5–11**

5. Protocol, `LayerTree`, `LayersModel`, `DocumentModel.selection`, render params; unit tests of the tree and picking.
6. The sidebar: tabs, no counts, `LayersPanel` (rows, eye, lock, chevrons, keyboard, VoiceOver), the Library chip.
7. The stage: picking, ⇧-click, double-click, inside and the fade, the pills, outlines, hover, the multi-item drag, keys.
8. The right-click menu (stage and rows) and Edit › Group / Ungroup.
9. The inspector: layer, item, several, nothing; Moved by; Place into it.
10. The script hook (D14), `layers.txt` and the acceptance runner's checks; snapshots in dark and light.
11. Docs (D15); PR 2's checks; merge into `feat/scene-layers`.

**PR 3: step 12**

12. Merge `main` in if the animation pane (#17) has landed; the full Proof; the final PR into `main`; `verify-change` on it; fix its blocking findings.

**As built.**
- PR 1 (#18) merged with a merge commit; the repository doesn't allow squash merges.
- "Group n" counts `group-n` ids as well as names, so after Group 1 is renamed the next group is Group 2 (`group-2`), not a second Group 1 (`group-1-2`); the spec says so (D4).
- The hook's expect for the layer count is `expect scenelayers <n>`: `expect layers` already counts an object's layers. It also gained `pointer <x> <y>` (the hover outline), `menus` now logs the Edit menu too, and `expect scenepixel x,y empty` now matches (it compared against an upper-cased `EMPTY` and never could).
- `StageTests`' test of the old Items tree's rows (`TreeRow`, which is gone) is replaced by `LayerTests`' rows, wrapper and paint-order tests.
- Found while building: a timeline frame cached before a scene's hidden layers were known kept showing them, so `LayersModel.itemsChanged` clears the cache when the render's `hide`/`focus` change; inspector notes use line limits and layout priority, not `fixedSize`, which inflated the window's minimum height; the sidebar takes a scroll request so the hook can bring a row into view before its real right-click; `layers.txt` closes the Publish sheet (`done`) after each publish, since a sheet keeps real events from the window.
- The Place inspector also says where the item goes ("into Railway"), as D13 asks.
- `scene.place --layer` sets the layer in `scene-edit.mjs` (with `layer-ops.mjs`'s `setItemLayer`) after `scene-ops.mjs` places the item, rather than inside `place` and `placeInGroup`.
- After the `verify-change` run on #20: a three-way merge where both sides made a scene's first layers kept only one list, so `foldScene` folds them and `Session.merge` refuses a clash or a merge that adds problems (D1, with tests); the sidebar now gives the scene list, and the Layers tree or the Library grid, scroll areas of their own (D11), with LAYERS, LIBRARY and the chip pinned; `layers.txt` checks what a locked layer's click picks (`expect inspector item`), that another scene has no hidden or locked layers, and a real mouse drag; `expect menu` says `closed` when no menu is open; the tree's Esc clears the selection before leaving, as the stage does (R9; the spec's D11 said otherwise).

## Workflow and reviews

| Step | Who | Where | Then |
| --- | --- | --- | --- |
| 1 | main session | `feat/scene-layers` | push, go on |
| 2–4 | main session | `feat/scene-layers-cli` → PR 1 | merge into `feat/scene-layers` when its checks pass |
| 5–11 | main session | `feat/scene-layers-app` → PR 2 | merge into `feat/scene-layers` when its checks pass |
| 12 | main session | `feat/scene-layers` → PR 3 into `main` | `verify-change` subagent, then fixes, then hand over |

- **One runner.** The main session runs every slice in order. The app slice is built on the CLI's commands and the same few files (`ScriptRunner`, `AppModel`, `SceneEditor`), so subagents in parallel would only collide. Two slices instead of one because the format change (what's written to scenes) is merged and checked before anything builds on it.
- **Merging.** The main session merges PR 1 and PR 2 into `feat/scene-layers` (`gh pr merge --merge --delete-branch`: the repository allows merge commits, not squashes) once each one's checks, pasted in its PR, pass. If `main` moves, it's merged into `feat/scene-layers` before the next slice. Only the owner merges PR 3.
- **Owner reviews:** none until PR 3 (the owner asked for that). The snapshots in PR 3 are for the owner's look at the panel, the stage inside a layer, the menu and the layer inspector in dark and light.
- **Independent review:** a `verify-change` run on PR 3, because the change alters what's written to scene files and the MCP surface. A fresh subagent (Opus) is given only "Use the `verify-change` skill on PR #<n>." Its blocking findings are fixed on `feat/scene-layers` before handing over; a re-check covers them.

## Risks

- **A format change the site's engine refuses** (spec, Areas of concern): handled by the site bumping its pin before layers are made in its document; the PR says so. Documents without layers serialize byte-for-byte as before, which the existing round-trip and compile tests prove.
- **A layer command that reorders items** would change output. Every command's test checks the items' order and the published bytes (R2).
- **Wrappers.** `scene.layer.move` reuses `followGroup` per item; `groups.test.mjs` runs unchanged, and a test moves a layer whose item's wrapper has `data-x`/`data-y`.
- **The selection refactor** (`selectedPath` computed over `selection`) touches every item feature. The existing acceptance scripts (`scene-object.txt`, `agent.txt`, `stale.txt`, `document.txt`) must pass unchanged, apart from `sidebar items`, which stays an alias.
- **The right-click catcher over SwiftUI rows** must not swallow left clicks. The hook's `rightclick row` sends a real right-mouse event through the window, and the acceptance script clicks rows afterwards.
- **The riskiest step is 7** (the stage): picking with inside and locks, and the drag. `LayerTests` cover the rules; `layers.txt` drives clicks, double-clicks, drags and keys with `expect` lines.
- **The real site checkout:** every test and script runs on temp copies (`makeDocument()`, the acceptance temp site).

## Proof

- `npm test` shows `# fail 0`, with `R1:`–`R5:` tests in `layers.test.mjs`.
- `npm run docs:cli`, then `git status` shows only this change's edits.
- `swift build --package-path app` with no errors; `scripts/test-app.sh` passes, with `LayerTests`.
- `scripts/build-app.sh`, then `scripts/acceptance.sh` exits 0: the existing passes, and `layers.txt` (tabs, rows, picking, ⇧-click, Group, rename, inside and Done, hide, lock, drag, keys, the menu on the stage and on a row, Ungroup, Place into it, save, publish), with the runner checking that the hero island's SVG is byte-identical before and after its layers were made and saved, and that the saved scene has `layers` but no hidden or locked state.
- `npm run docs:site` builds with no broken links.
- Snapshots in dark and light of the Layers panel, the stage inside a layer, the right-click menu and the layer inspector, looked at, in `docs/screenshots/` and the PR.
