# Plan: Draw a scene's items by where they stand (from intent.md 2026-10-20)
Status: approved (the owner approved the plan ahead of review, with the intent and spec; the owner reviews once, at the end, on PR #21).

Before this plan, the spec got three clarifications in their own commit (no change of intent): the session's scene edit does the sorting, so motion edits that move a track's first key are covered too, and results say which paths moved (D3); `scene.sort`'s `changedPixels` compares the file's list order with depth order, because the published render no longer depends on the list's order (R3); its dry run is the CLI's global `--dry-run`.

## Design decisions the spec leaves to the plan

These are fixed here so no slice has to choose.

- **Where the order lives.** `cli/lib/engine/depth.mjs` is pure geometry and ordering: boxes, `behind(a, b)`, the banded sort over nodes, and the order at a time. It imports only `iso.mjs`. `engine.mjs` builds the nodes (it can resolve objects and knows their drawn bounds) and calls it, so there's no import cycle. Engine helpers the rest of the CLI uses: `depthOrder(sources, items, origin)` (the sorted order of one list, as indices), `sceneInOrder(sources, scene)` (the scene with every list sorted, a copy), `isInDepthOrder(sources, scene)`, and later (slice 2) `sceneAt(sources, name, time)` and `motionCopies(sources, name)`.
- **Exact numbers.** Every box coordinate and key is a multiple of 1/32 of a tile, so doubles hold them exactly; no rounding, no epsilon.
- **The sort.** Greedy topological sort: repeatedly take, of the items whose "behind" predecessors are all placed, the one that comes first in the list (changed during slice 1 from "the smallest key, then list index": see As built). With none available (a cycle), take the smallest key (c + r + l of the box centre) of all remaining items. Because availability depends only on the set already placed, sorting a sorted list returns it unchanged (R2); a test proves it on random scenes.
- **Drawn bounds.** An object's drawn bounds are the union of its pixels over every layer and every frame (blocks: their faces and sides), from `renderObject` and `composite`, cached per object in a `WeakMap` keyed by the sources. A tracked item's bounds also cover its poses.
- **The rest box.** A tracked item rests where the SVG draws it: its placement plus its first key's move (as `placeItems` does today).
- **Variant runs.** A run's band is its first member's mark; marks on the run's other members don't move it.
- **`renderScene(sources, name, { order })`.** `order: 'depth'` (the default) is this change; `order: 'list'` draws in the file's order with no motion copies, exactly as the engine before this change did. It's for three things: `scene.sort`'s `changedPixels`, the site-art test (below), and the preview, which draws a scene it has already put in order.
- **Order while moving (D6), made exact.** For a list with moving items, the still items (items without a moving track, groups, variant runs) keep one order for the whole loop: the rest sort with the moving items left out. The order at time t is the same greedy sort as above, over the boxes at t, plus a chain edge between each pair of consecutive still items; a cycle is broken by taking the smallest-key remaining item that no chain edge holds back. At t = 0 this is the rest sort (adding edges the rest sort already satisfies can't change a greedy choice). A moving item's place at t is (the still item it follows, its rank among the moving items in that gap), as the spec says. The times to check are every key time and every pixel step between keys of every moving item in the list (`stepsFor`). The SVG and the preview both call this one function, so they can't disagree.
- **Copy windows in CSS.** A copy's opacity keyframes switch at the same instants the transform steps: their percentages come from the same rounded key percentages `motionCss` already writes, interpolated per step, and are written with 4 decimals. The copy holding time 0 is `z0` and shows in the base rule; the others start at `opacity:0`. The existing reduced-motion rule (`[class^="mv-"]`) already covers the copy wrappers. A copy class that would equal another track's class (an id ending in `-z<n>`) is an engine problem, reported only when it actually happens.
- **The preview at a time.** `engine.sceneAt(sources, name, time)` returns a still scene: every list in its order at `time`, each tracked item posed (pixel point, its own level, its pose, hidden while it doesn't show). `renderSceneImage` filters that (hidden layers, shown variants, hidden items) and draws it with `order: 'list'`, so hiding a layer never reorders what's left. Its `items` come back in the order drawn at that time, so the app's hit test (the last item under the pointer) picks what's on top without any app change.
- **Sorting in the session.** `Session.edit` ends every edit of a scene doc with `sortScene` (scene-ops), inside the same undo step and coalescing. It rewrites the result's paths (`path`, `paths`, `group.path`, `groups[].path`) to where the sort put them, and adds `reordered: { "<old>": "<new>" }` when the sort moved anything, so the app can follow its selection. Edits keep calling their scene-ops functions as today; no scene-ops function needs the sources except `sortScene`.
- **Merges and reloads.** `Session.merge` sorts the merged doc inside the merge's undo step. A reload (`doc.reload`, and the automatic reload of a clean doc whose file changed) adopts the file as the clean base, then, only if it's out of order, sorts it as a separate undo step labelled "Sorted <scene> by depth", so the doc shows as unsaved and one undo gives back exactly the file. Files written by Pixel Lab are always sorted, so in practice this happens only after a hand edit. Opening a scene never sorts it.
- **Spoken text.** A pixel item's position reads "x 40, y −12, level 4"; at level 0 it reads as today ("x 40, y −12"), as the file leaves `level: 0` out.
- **Warnings.** `check`'s warnings gain a `code`: `off_grid` (today's), `out_of_order` (R11; with the scene and how many items would move) and `copies` (R12; per moving item with more than one place, its copy count). The app shows Sort only on `out_of_order`.
- **`scene.set --depth` on a group.** `scene.set` takes a group's path when `depth` is the only field given; any other field on a group path is a usage error naming `scene.group.set`.
- **The hook's additions beyond D8,** needed to drive the new inspector fields: `footprint <w>,<d>` (the object inspector's steppers), `expect footprint <w>,<d>`, `expect warning <code>` and `expect groupdepth <path> <v>`.

## Files that change

**Slice 1, the engine (`feat/depth-order-engine`)**
- `cli/lib/engine/depth.mjs` (new): tile-space boxes for blocks, sprites (with `footprint`) and pixel points with a level; `behind`; the banded greedy sort with its key and cycle rule; group and variant-run nodes.
- `cli/lib/engine/engine.mjs`: validate `depth` on items and groups, `level` with `at.px`, sprite `footprint` (blocks refuse it; `extends` inherits it in `resolve`); drawn-bounds cache; `renderScene` sorts every list (option `order`); `depthOrder`, `sceneInOrder`, `isInDepthOrder`; the "items paint in source order" doc comment.
- `cli/lib/engine/serialize.mjs`: item keys `object, id, class, layer, depth, at`; group keys `group, depth, items`; `level` after `px`, left out at 0; `footprint` after `anchor`.
- `cli/lib/engine/iso.mjs`: the unrounded pixel-to-tile conversion D2 uses.
- `cli/lib/engine/compile.mjs`: `compileScene` passes `order` through (for the site-art test).
- `cli/lib/document.mjs`: `depth` in the engine bundle a document carries (`site.depth`).
- `cli/lib/render.mjs`: `flattenItems` adds an item's or group's `depth` and a pixel item's `level` (what `scene.items` lists).
- `package.json`: export `./depth`.
- `cli/test/depth.test.mjs` (new); `cli/test/engine.test.mjs`, `cli/test/compile.test.mjs`, `cli/test/package.test.mjs`, and any other test whose expected pixels change only because the engine now draws in depth order.

**Slice 2, motion (`feat/depth-order-motion`), in parallel with slice 3**
- `cli/lib/engine/depth.mjs`: the order at a time with the still chain, places, windows.
- `cli/lib/engine/motion.mjs`: `motionCss` writes each copy's wrapper rule and opacity keyframes; `poseItems` keeps the item's level.
- `cli/lib/engine/engine.mjs`: `placeItems` draws one copy per place in `mv-<id>-z<k>` wrappers (inside a group's `<g>` for a member), `sceneAt`, `motionCopies`, the copy-class collision problem.
- `cli/lib/render.mjs`: `renderSceneImage` through `sceneAt` and `order: 'list'`; `items` in drawn order.
- `cli/lib/commands/check.mjs`: warning codes, the `out_of_order` and `copies` warnings, the description.
- `docs/cli.md` (generated).
- `cli/test/depth-motion.test.mjs` (new); `cli/test/render.test.mjs`; the `check` tests in `cli/test/commands.test.mjs`.

**Slice 3, editing and commands (`feat/depth-order-edit`), in parallel with slice 2**
- `cli/lib/scene-ops.mjs`: `sortScene(site, sources, doc)` returning `{ moved, remap }`; `insertIndex`, `insertIndexDeep`, `reorderItem` removed; `place` and `placeInGroup` append (no `index`); `setDepth` (item or group); a pixel item's level in `moveItem`, `setField`, `setAt`, `snapItem` (to the tile at its own level) and `spoken`; `reorderKeys` keeps `depth`.
- `cli/lib/layer-ops.mjs`: `setItemLayer` keeps the key order with `depth`.
- `cli/lib/session.mjs`: `edit` sorts scene docs, rewrites result paths, adds `reordered`; `merge` sorts; `reload` and the automatic reload sort as their own step when out of order.
- `cli/lib/merge.mjs`: only if a group line with `depth` needs it in `foldScene` (a test decides).
- `cli/lib/commands/scene-edit.mjs`: `scene.raise` and `scene.lower` removed; `scene.place` without `index`, with `level` for `--px`; `scene.set --depth` (items and groups) and `--level` for pixel items; `scene.move --by c,r,l` on a pixel item's level; `scene.snap`; `scene.duplicate`'s summary; `scene.sort` (new).
- `cli/lib/commands/scene-layers.mjs`: `scene.layer.move` returns the moved items' `paths` after the sort.
- `cli/lib/commands/scene.mjs`: `scene.items` description (depth order, `depth`, `level`).
- `cli/lib/commands/object-edit.mjs`: `object.new --footprint`, `object.footprint` (new).
- `cli/lib/commands/object.mjs`: `object.show` shows `footprint`.
- `cli/lib/commands/doc.mjs`: reload and merge summaries say when they sorted.
- `cli/lib/server-mcp.mjs`: the instructions (depth, the override, `scene_sort`, no raise/lower).
- `scripts/make-examples.mjs`, `examples/harbor.pixellab/`, `examples/town.pixellab/` (regenerated, sorted; a `footprint` on a sprite only where the before/after render shows it sorting wrong).
- `cli/test/fixtures/sample.pixellab/` (sorted, if its scenes aren't).
- `docs/cli.md` (generated).
- `cli/test/depth-edit.test.mjs` (new); `cli/test/scene-ops.test.mjs`, `edit-commands.test.mjs`, `groups.test.mjs`, `layers.test.mjs`, `params.test.mjs`, `help.test.mjs`, `merge.test.mjs`, `sync.test.mjs`, `servers.test.mjs`, `motion-commands.test.mjs`, `examples.test.mjs`, `cli/test/helpers.mjs` (a sorted site copy, if tests need one).

**Slice 4, the app and the acceptance pass (`feat/depth-order-app`), after slices 2 and 3**
- `app/Sources/PixelLab/Server/Protocol.swift`: `SceneItemInfo.depth`, a pixel `at`'s `level`, `SceneGroupInfo.depth`, `CheckResult.warnings` (with `code`), the object info's `footprint`, `EditResult.reordered`.
- `app/Sources/PixelLab/Model/DocumentModel.swift`: warnings; the selection follows `reordered`.
- `app/Sources/PixelLab/Model/SceneEditor.swift`: `raise()`, `lower()` and scene mode's `]`/`[` removed; `setDepth`, `setGroupDepth`, `sort`; Level for pixel items.
- `app/Sources/PixelLab/Model/LayersModel.swift`: follows `reordered` after its edits.
- `app/Sources/PixelLab/Model/ObjectEditor.swift`: footprint drafts and `object.footprint`.
- `app/Sources/PixelLab/Model/AppModel.swift`: only if loading check warnings needs it.
- `app/Sources/PixelLab/Views/SelectionInspector.swift`: the Depth control, Level for pixel items, the snap note.
- `app/Sources/PixelLab/Views/LayerInspector.swift`: `ItemLayerLines`' wrapper line shows the group's depth with its menu.
- `app/Sources/PixelLab/Views/ObjectPanels.swift`: FOOTPRINT for sprites.
- `app/Sources/PixelLab/Views/InspectorView.swift`: `ChecksList` shows warnings, the out-of-order one with Sort.
- `app/Sources/PixelLab/PixelLabApp.swift`: Edit › Raise and Lower removed.
- `app/Sources/PixelLab/Support/ScriptRunner.swift`: D8's actions and the additions above; `raise`, `lower` and scene mode's `]`/`[` key cases removed.
- `app/Tests/PixelLabTests/DepthTests.swift` (new); `StageTests.swift` if a test leaned on raise.
- `scripts/acceptance.sh`: sorts the temp site after copying it; runs the new script; its header.
- `scripts/acceptance/scene-object.txt` (raise/lower lines replaced by the override), the other `*.txt` pins that the new order changes, `scripts/acceptance/examples-depth.txt` (new).
- `docs/screenshots/depth-*.png` (new).

**Slice 5, the docs (`feat/depth-order-docs`), in parallel with slice 4**
- `README.md` (the Scene mode tour, the shortcuts table), `app/README.md`, `cli/README.md` (`depth.mjs`, the export), `docs/architecture.md`, `docs/testing.md`, `scripts/README.md`, `examples/README.md` (if it speaks of order), `CLAUDE.md` (the hook list), `docs/intents/2026-10-mac-app/plan.md` (a one-line pointer at its `insertIndex` note).
- `docs/intents/2026-10-depth-order/site-notes.md` (new): the site's migration from the spec's Areas of concern, with the `scene sort --all --dry-run` numbers for the site's scenes, taken on a temp copy.

**The main session:** this plan; merge commits; this plan's "As built"; PR #21's body.

## Order of work

```
main ── intent/depth-order (PR #21: intent, spec, plan, then every slice) ──── owner merges ──▶ main
          ├─ slice 1  engine   ◀─ feat/depth-order-engine                 (steps 2–4)
          ├─ slice 2  motion   ◀─ feat/depth-order-motion  ┐ in parallel, (steps 5–8)
          ├─ slice 3  editing  ◀─ feat/depth-order-edit    ┘ from slice 1 (steps 9–12)
          ├─ slice 4  app      ◀─ feat/depth-order-app     ┐ in parallel, (steps 13–16)
          └─ slice 5  docs     ◀─ feat/depth-order-docs    ┘ from 2 + 3   (step 17)
          then steps 18–19 on intent/depth-order; verify-change on PR #21
```

**A deliberate departure from `write-plan`'s branches:** the owner asked for one PR, so `intent/depth-order` (PR #21, already holding the intent and spec) plays the role of `feat/depth-order`. Slice branches are local, cut from it in their own worktrees, and merged into it by the main session with merge commits; there are no slice PRs on GitHub. The main session works in `.claude/worktrees/depth-order` on the local branch `spec/depth-order` and pushes with `git push origin HEAD:intent/depth-order`.

Five slices, not one: slice 1 is the foundation every other part calls; slices 2 and 3 own different files and can be built at the same time on it; the app (slice 4) needs both merged; the docs (slice 5) touch no code and can run beside the app.

1. Spec clarifications and this plan (main session; done).

**Slice 1: steps 2–4**

2. `depth.mjs` with `R1:` and `R2:` tests in `depth.test.mjs`: block, sprite, footprint and pixel boxes; `behind` on each axis, the crossed case (no order), interpenetration; the sort on hand-made lists (the pine at 0,1,0 behind the red house's walls at 1,1,1; the hall's 2×1 walls and roof against the tiles around them; a raised block); a three-box cycle; determinism (the same list twice, and on a shuffled copy, gives one answer per input); sorting a sorted list returns it (random scenes); a time bound on a 400-item list.
3. The format: validation (`depth`, `px` + `level`, `footprint`, refusals with today's message style), serialization (key orders, `level` left out at 0, a round trip of every new key, and existing files byte-identical), `extends` inheriting `footprint`, `site.depth`, the `./depth` export, `scene.items` fields. Tests `R5:`, `R7:`, `R8:` (format parts).
4. `renderScene` sorts each list: bands, groups as one node (union box) with their members sorted inside, variant runs as one node keeping their order, `order: 'list'`. Tests `R3:` (a file listed out of order publishes in depth order; the compiler, the preview and `scene.render` agree), `R9:` (group, variant run, `composite`'s `firstVariant` still right). Existing tests that pinned list-order pixels are updated to the new pixels with a comment saying why; none are deleted. `compile.test.mjs`'s site-art test (below) is reworked.

**Slice 2: steps 5–8**

5. The order at a time: still chain, places, windows; a naive reference (re-sort the whole list at every check time) and the real one agree on random scenes with moving items; `R10:` places and windows on hand-made tracks.
6. The SVG: copies in `placeItems`, wrapper rules and keyframes in `motionCss`, `z0` in the base rule, reduced motion; a group member's copies inside its group; `R10:` tests on the tree and the CSS; one-place items unchanged byte for byte.
7. The preview: `sceneAt`, `renderSceneImage` through it, `items` in drawn order; the parity test (Proof); `scene.render --time` and the app's timeline get it through `renderSceneImage`.
8. `check`: warning codes, `out_of_order` (R11) and `copies` (R12) with tests; `npm run docs:cli`.

**Slice 3: steps 9–12**

9. `sortScene` and the session: `edit` sorts, rewrites paths, adds `reordered`; `merge` and both reloads sort as described; undo and redo of a sorting edit; `R4:` tests for place, move (by command, by layer), set, snap, duplicate, remove, a motion edit to a first key, undo, redo, merge, reload, and a proposal approved into a dirty copy.
10. The commands: raise and lower removed, `place` without `index`, `set --depth` and `--level`, `move` on a pixel item's level, `snap` at its own level, `scene.sort` (`moved`, `changedPixels`, `--all`, nothing to do), `object.footprint`, `object.new --footprint`, `object.show`, `scene.layer.move` paths, the MCP instructions. Tests `R6:`, `R7:`, `R8:`, `R11:`, `R13:` (the three doors: CLI, `serve --stdio`, and MCP making a proposal). `npm run docs:cli`.
11. The existing tests: every test of raise, lower, `index`, `insertIndex` and `reorderItem` is rewritten to the behaviour that replaces it (the override, append-then-sort), keeping what it proved about groups and refusals; command lists and counts updated. `R15:` the pine moved −5,−5 in a temp copy of `examples/harbor.pixellab`'s `harbor-flat` draws behind the red house's wall (a wall pixel checked in `scene.render`), and moving it back draws it in front again.
12. The examples: `make-examples.mjs` writes sorted scenes (through `sceneInOrder`); `npm run examples`; before/after renders of each example scene looked at; `npm test`.

**Slice 4: steps 13–16**

13. Protocol, models and the selection following `reordered`; `DepthTests` (decoding, the Depth control's values and labels, remapping a multi-item selection).
14. The views: Depth control, Level, the wrapper line's depth menu, FOOTPRINT, Checks warnings with Sort, Raise and Lower gone from the inspector and the Edit menu, `]`/`[` gone in scene mode (kept in object mode).
15. The hook: D8's actions and the additions above, `raise`/`lower` gone, the header comment.
16. The acceptance pass: `acceptance.sh` sorts the temp site (the bundled CLI's `scene sort --all`) and commits that as its base; `scene-object.txt` checks the override (`depth front`, `expect depth front`, a scene pixel, `depth auto`) instead of raise and lower; pins that the new order changed are updated; `examples-depth.txt` on a temp copy of `examples/harbor.pixellab` (below). Snapshots in dark and light.

**Slice 5: step 17**

17. The docs of D9 and the site notes; `npm run docs:site`.

**On `intent/depth-order`: steps 18–19**

18. Integration: both merges, `npm run docs:cli` on the merged tree, the full Proof in the main worktree, `scripts/build-app.sh` there, this plan's "As built", PR #21's body.
19. `verify-change` on PR #21; fixes for its blocking findings; a re-check of them; hand over.

## Workflow and reviews

| Step | Who (model) | Where | Hands back | Then |
| --- | --- | --- | --- | --- |
| 1 | main session (Opus) | `spec/depth-order` → `intent/depth-order` | — | push |
| 2–4 | implementer subagent (Sonnet) | worktree `.claude/worktrees/depth-order-engine`, branch `feat/depth-order-engine` | one commit per step, the slice's checks' output, any departure from this plan | main session reviews the diff, merges, pushes; starts 2 and 3 |
| 5–8 | implementer subagent (Sonnet), in the background | `.claude/worktrees/depth-order-motion`, `feat/depth-order-motion` | same | review, merge |
| 9–12 | implementer subagent (Sonnet), in the background, at the same time as 5–8 | `.claude/worktrees/depth-order-edit`, `feat/depth-order-edit` | same, plus before/after PNGs of each example scene | review, merge; after both: `npm test` on the merge |
| 13–16 | implementer subagent (Sonnet), in the background | `.claude/worktrees/depth-order-app`, `feat/depth-order-app` | same, plus the snapshots and every acceptance pin changed (old → new, why) | review, merge |
| 17 | implementer subagent (Sonnet), in the background, at the same time as 13–16 | `.claude/worktrees/depth-order-docs`, `feat/depth-order-docs` | same, plus the site's dry-run numbers | review against what slice 4 built, merge |
| 18 | main session (Opus) | `.claude/worktrees/depth-order` | — | push, update PR #21 |
| 19 | `verify-change` subagent (Opus), fresh | read-only | one PR comment | main session fixes blocking findings (or hands each to a Sonnet subagent), re-check, hand over |

- **Starting a slice.** The main session cuts the branch and worktree from the current tip: `git worktree add .claude/worktrees/depth-order-<slice> -b feat/depth-order-<slice> spec/depth-order`. It gives the subagent (Agent tool, `model: "sonnet"`): this plan's path and its steps, the worktree path (work only there; never the main checkout, the site repo or `examples/` in place), its file ownership, the checks below, and "commit each step with a message ending `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; don't push; don't edit plan.md; report departures instead". A subagent reads CLAUDE.md, the spec and this plan first.
- **Each slice's checks** (pasted in its report): slices 1–3: `npm test` with `# fail 0` and `npm run docs:cli` leaving `git status` clean. Slice 4: also `swift build --package-path app`, `scripts/test-app.sh`, `scripts/build-app.sh`, `scripts/acceptance.sh` exit 0, the hook runs of its new actions, and the snapshots looked at. Slice 5: `npm run docs:site` with no broken links.
- **Review and merge.** The main session reads each slice's whole diff (`git diff spec/depth-order...feat/depth-order-<slice>`) against the spec and this plan, re-runs `npm test` in the slice's worktree, and either merges (`git merge --no-ff feat/depth-order-<slice>`, a message ending with the Co-Authored-By line; then `git worktree remove` and `git branch -d`) or sends the subagent its findings (SendMessage) and waits. Departures go into this plan's "As built" in the main session's next commit, since parallel slices can't share plan.md.
- **File ownership.** A slice edits only its files above. Slices 2 and 3 share `docs/cli.md` (generated: conflicts are resolved by re-running `npm run docs:cli` on the merge) and `cli/test/commands.test.mjs` (slice 2 only its `check` tests). Slice 3 owns the test fixtures and `cli/test/helpers.mjs`; slice 2 builds its scenes in memory. A slice that needs another's file, or finds a test it doesn't own failing because of its change, stops and reports. `ScriptRunner.swift`, `AppModel.swift` and `scripts/acceptance/` belong to slice 4 alone; slice 5 writes the hook's actions into `CLAUDE.md` and `docs/testing.md` from this plan's list, and the main session checks them against slice 4's `ScriptRunner` before merging slice 5.
- **Integration after slices 2 and 3.** The main session runs `npm test` on the merged tree before starting slice 4. A failure of a few lines (two slices' expectations meeting) it fixes itself in one commit; anything larger goes back to the slice's subagent.
- **When `main` moves,** the main session merges it into `intent/depth-order` before the next slice starts, and running slices merge `spec/depth-order` into their branch before handing back.
- **Owner reviews:** none until the end, as the owner asked. At step 19's hand-over PR #21 has, for the owner: the verification output, the snapshots (the Depth control, Level, the wrapper line's menu, FOOTPRINT, the Checks warning with Sort, the harbor pine behind the house, each in dark and light), the before/after renders of the examples, the changed acceptance pins with reasons, and the site notes. The owner tries `scripts/try-example.sh harbor` on the build in `.claude/worktrees/depth-order`. Feedback comes as PR comments or in the session; fixes go on `intent/depth-order` with this plan updated.
- **Independent review:** one `verify-change` run on PR #21 after step 18, because the change alters what the engine publishes, the scene and object formats the site will adopt, and the MCP surface (two tools removed, three added). A fresh Opus subagent is given only "Use the `verify-change` skill on PR #21." It comments on the PR and never pushes or commits. The main session fixes every blocking finding, records it in "As built", and has the same subagent re-check the fixes.

## Risks

- **The motion copies (steps 5–7) are the riskiest part:** a copy that switches a step early or late shows an item twice or not at all for a frame. Handled by one shared order-at-a-time function for the SVG and the preview, copy windows computed from the same rounded key percentages as the transform, a reference-versus-real equivalence test, and the parity test that reads the published CSS rather than the engine's own data. If parity can't be made exact at a window's edge, the slice stops and reports rather than loosening the test.
- **Size and speed.** Copies repeat pixels (hero-island is 518 of 768 KB); the order at every step of every track costs time. `check` reports copies (R12); slice 2 reports the site's hero-island copies, size and compile time on a temp copy, and its tests bound the time on a synthetic scene of hero-island's size. A blow-up stops the slice: the fix would be fewer check times, not a different rule.
- **Paths moving under callers.** After every scene edit an item can have a new path: the app's selection, a held arrow key, a drag, the Layers panel. Handled by results carrying paths after the sort plus `reordered`, and `DepthTests` and the hook covering a multi-item move and a nudge that reorders. A held arrow key whose item changes path starts a new undo step (the coalescing key holds the path); accepted, and noted in "As built" if it shows.
- **Merges of unsorted files.** Sorting moves lines, so a three-way merge between a sorted copy and an unsorted base can clash where it used to merge. Every file Pixel Lab writes is sorted, so after the first edit this is rare; `acceptance.sh` sorts its temp site first so `stale.txt` tests merges as they'll be, and slice 3 adds a test merging two edits of a sorted scene where one moved an item.
- **The site's art test.** `compile.test.mjs` checks every committed SVG in the site checkout is what this compiler makes from its sources. With this change the site's SVGs differ until the site migrates under its own workflow. The test becomes two (still read-only on the site, through `makeSite()`'s temp copy): in `order: 'list'` the compiler makes the committed bytes exactly (the compiler and optimizer are unchanged), and in depth order every site scene compiles lossless, with the scenes whose output changed and any over budget listed in the test's output, not failed. Nothing writes to the site.
- **Old engines refuse the new keys.** The site gets `depth`, `level` and `footprint` only after it bumps its `pixellab` pin; the site notes and PR #21 say so. Files without them serialize byte-for-byte as before (the round-trip tests).
- **Scene layers' R2 ("making layers never reorders items")** stays true for sorted scenes; on a hand-edited unsorted scene, a layer edit now sorts it, because every scene edit does (R4). `layers.test.mjs`'s R2 tests run on sorted scenes, and a new test shows a layer edit on an unsorted scene sorts it and publishes the same bytes.
- **Acceptance pins.** The new order changes some pinned pixels. Each changed pin is listed with its old and new value and a snapshot showing the new one is right; a pin is never changed to whatever the app says without that look.
- **The three doors.** Removing `scene_raise`/`scene_lower` breaks agents that call them; the MCP instructions name the override, and `servers.test.mjs` checks the tool list. Proposals are unchanged in format; their diffs now include reordered lines, which the Agent window's ghost shows as changes.
- **The real site checkout and `examples/`:** tests and scripts use temp copies only (`makeDocument()`, `makeSite()`, the acceptance temp site, temp copies of `examples/`). The owner's uncommitted edit in the main checkout is never touched.

## Proof

- `npm test` shows `# fail 0`, with `R1:`–`R15:` tests in `depth.test.mjs`, `depth-motion.test.mjs` and `depth-edit.test.mjs`, among them:
  - the pine moved behind the red house in `harbor-flat` draws behind the wall; the 2×1 hall; a pixel item with a level; a footprint; a group; a variant run; a box cycle; sorting a sorted list changes nothing;
  - the parity test: for a scene with moving items crossing still ones (and one inside a group), at every window boundary and the middle of every window, the published SVG's tree with its CSS evaluated at that time (a small keyframe evaluator in the test: `step-end` and `steps(n,end)`, translate and opacity) composites to the same pixels as `scene.render --time`; reduced motion (animations off) shows the time-0 frame;
  - every new command through the CLI, `serve --stdio` and MCP (a proposal).
- `npm run docs:cli`, then `git status` shows only this change's edits.
- `swift build --package-path app` with no errors; `scripts/test-app.sh` passes, with `DepthTests`.
- `scripts/build-app.sh` builds `build/Pixel Lab.app` in `.claude/worktrees/depth-order`; `scripts/try-example.sh harbor` opens it on a copy.
- `scripts/acceptance.sh` exits 0, including `examples-depth.txt`: on a temp copy of `examples/harbor.pixellab`, `open scene:harbor-flat`, select the front pine, move it −5,−5, `expect at 0,1,0`, `expect scenepixel` on a wall pixel it overlaps (the wall's color), `depth front` and `expect depth front` and the pine's color there, `depth auto`, `expect order`, `save`, `cli check harbor-flat` with no `out_of_order` warning, and snapshots in dark and light.
- `npm run docs:site` builds with no broken links.
- Snapshots in dark and light of the Depth control, Level on a pixel item, the wrapper line's depth menu, FOOTPRINT, the Checks warning with Sort, and the harbor with the pine behind the house, looked at, in `docs/screenshots/` and PR #21.
- `verify-change`'s comment on PR #21, with its blocking findings fixed and re-checked.

## As built

- **Tie-break (slice 1).** Among the items free to go next, the sort takes the one first in the list, not the one with the smallest depth key; the key only breaks cycles. Breaking ties by the key reordered items that never overlap, which split the compiler's colour runs: on a temp copy of the site's document, preview-only scenes grew (`closeup-water` 94 → 120 KB, `closeup-front-trees` 89 → 108 KB, both over budget). By list order they stay at the old engine's sizes and within budget, and existing art changes only where an overlap forces it. Spec R2 and D2 are updated to match.
