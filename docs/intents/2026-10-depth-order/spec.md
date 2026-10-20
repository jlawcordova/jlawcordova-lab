# Spec: Draw a scene's items by where they stand (from intent.md 2026-10-20)
Status: approved (the owner answered the intent's open questions in the session, approved the spec and plan ahead of review, and asked for the code in the same PR).

Today a scene paints its items in the order they're listed, and only placing slots an item in by its tile. This change makes depth come from where each item stands, always: the engine works the order out when it draws, every edit keeps the list in that order, and an item or group can be marked to draw in front of or behind everything else.

## Requirements

**Depth**

- **R1. Depth comes from where an item stands.** Each item has a box in tile space, worked out from its placement (col, row and level, or its pixel point and level) and its object (a block's `size`, a sprite's `footprint` and height). Of two items whose drawn pixels could overlap, the one behind is drawn first. "Behind" is decided by the boxes (D2), not by the anchor tile alone, so a 2×1 block, a raised block and the tiles around them come out right.
- **R2. The order is deterministic.** The same scene always gives the same order on any machine. Where the boxes don't decide (nothing overlaps, or two boxes interpenetrate), the order falls back to a fixed key, then to the list order. Sorting a sorted list changes nothing.
- **R3. The engine draws in depth order.** `renderScene`, the compiler, the previews and `scene.render` draw every list (the scene's items and each group's items) in depth order, whatever order the file lists them in. What's published is always right, even for a file edited by hand or a merge.
- **R4. Every edit keeps the list sorted.** Placing, moving (by command, inspector, drag, arrow keys or a moved layer), setting a field, snapping, duplicating, removing, setting an override, undo and redo, applying a proposal and merging all leave each list in depth order, so the file, `scene.items` and the Layers panel read back to front. An edit to a scene sorts the whole scene. Sorting is part of the edit, in the same undo step.
- **R5. Front and back overrides.** An item or a group can carry `depth: 'front'` or `depth: 'back'`. Within its own list, back items draw before all the others and front items after all the others; items with the same mark sort by where they stand among themselves. No mark means by position. To draw over the whole scene, mark the top-level item or group.
- **R6. Raise and Lower go away.** `scene.raise`, `scene.lower`, `scene.place --index`, their MCP tools, the inspector's buttons, the Edit menu's items, the `]` and `[` shortcuts in scene mode and the script hook's `raise` and `lower` are removed. The override (R5) is the one manual control. Object mode keeps `]` and `[` for its layers.

**Placements and objects**

- **R7. Pixel-placed items have a level.** `at: { px: [x, y], level: n }` (default 0). The pixel point is where the item's foot is drawn, as today; the level says how high it stands, so its depth is read in sub-tile steps: (x, y) at level n stands where a tile placement at the same point and level would, between whole tiles. The inspector shows and edits Level for a pixel-placed item; `scene.set` and `scene.move` take it.
- **R8. Sprites can have a footprint.** A sprite object may say how many tiles it covers, `footprint: [wide, deep]` (whole numbers, 1 to 16; default `[1, 1]`), so large sprites (a house, a locomotive, the crane mast) sort against their neighbours by the ground they cover. Blocks use their `size`. `object.show` shows it, `object.new --footprint` and the new `object.footprint` set it, and the object inspector edits it.

**Groups and motion**

- **R9. Groups sort as one object.** A `{ group, items }` wrapper sorts in its list as one object, its box the union of its members' boxes at rest. Its members sort among themselves inside it. Wrappers are never split, so their SVG `<g>`s and enter animations stay as they are. A run of consecutive variant groups (`data-class`) sorts as one unit and keeps its own order, so the variants stay together.
- **R10. Moving items reorder while they move.** An item with a motion track has, at each moment of the loop, the depth of where it is then (its rest box moved by its track, at its own level), and draws in that place among its list's other items. In the SVG it's drawn once for each place it takes in the order, and each copy shows only while it's in that place (D6). `scene.render --time` and the app's timeline draw the same. The list, `scene.items` and the Layers panel keep the item where it rests. A moving item inside a group reorders only among the group's members; the group keeps its rest depth, so a crane in a group stays one object while its hook moves.

**Migration and checks**

- **R11. Sorting existing scenes.** `scene.sort` sorts one scene's lists, or every scene with `--all`, as one undo step, and reports what moved and how many published pixels change (`--dry-run` writes nothing). `check` warns about a scene whose file isn't in depth order. Existing art is expected to change; the examples are regenerated and the acceptance pass's pins updated in this change. The site migrates under its own workflow (Areas of concern).
- **R12. `check` reports copies.** For a scene with motion, `check` reports how many copies each moving item draws, so a size jump is visible before Publish. An output over its budget is written and flagged as today.

**The three doors, the app and proof**

- **R13. Every new action is a registry command.** `scene.sort`, `scene.set --depth`, `scene.set` and `scene.move` on a pixel item's level, and `object.footprint` are in the registry, so the CLI, the app and MCP agents all have them; MCP edits become proposals. `docs/cli.md` is regenerated and the MCP server's instructions are updated.
- **R14. The app.** The item inspector replaces Raise and Lower with a Depth control (Auto, Front, Back). A group's depth is set from its wrapper line. Pixel-placed items show Level. The object inspector shows Footprint for sprites. A scene opened out of order shows the warning in the Checks panel with a Sort button. Everything has VoiceOver labels, visible focus and comfortable hit targets, uses the board 03 tokens and no `@State`.
- **R15. Tests, the hook and the acceptance pass.** Tests named after these requirements prove each one, including the pine moved behind the red house in `harbor-flat` (it draws behind the wall), the 2×1 hall, a pixel item with a level, a footprint, a group, a variant run, a box cycle, and a moving item's copies matching `scene.render --time` at every change of place. The script hook gains `depth front|back|auto`, `sort`, and `expect depth <v>` and `expect order <paths>`; `raise` and `lower` go. `scripts/acceptance.sh` passes with its pins updated.
- **R16. Docs.** README (the tour, the shortcuts table without `]`/`[` in scene mode), `docs/architecture.md`, the scene-layers docs that say "each item still draws back to front by where it stands" (true again), `cli/README.md`, `CLAUDE.md`'s hook list, the MCP instructions and the generated `docs/cli.md` describe depth.

## Design

### D1. The scene and object formats (R5, R7, R8)

```js
items: [
  { group: { class: 'enter', 'data-enter': 'shadow' }, depth: 'back', items: [ … ] },
  { object: 'pine', at: { tile: [0, 1, 0] } },
  { object: 'crane-jib', id: 'jib', at: { px: [40, -12], level: 4 } },
  { object: 'cloud-a', depth: 'front', at: { px: [-120, -70], level: 6 } },
]
```

- An item's keys go `object, id, class, layer, depth, at`; a group's go `group, depth, items`. `depth` is `'front'` or `'back'`; absent means by position. The engine validates it (unknown values and keys are problems, as today).
- `at.px` takes an optional `level` (an integer, may be negative, like a tile's). The serializer writes it after `px` and leaves it out at 0, so existing files read the same.
- A sprite object takes `footprint: [w, d]`. The serializer writes it after `anchor`. Blocks refuse it (their `size` is their footprint). `extends` inherits it.
- Every engine before this change refuses these keys, so a document that uses them needs the new engine (Areas of concern).

### D2. Boxes and the order (R1, R2, R3)

New module `cli/lib/engine/depth.mjs`, pure, no Node or DOM APIs, exported from the package (`pixellab/depth`).

- **Tile space.** Tile `[c, r, l]` covers c−½…c+½ and r−½…r+½, its top face at height l. A pixel point (x, y) at level n stands at c + r = (y − oy + n·LEVEL_H) / (TILE_H/2) and c − r = (x − ox) / (TILE_W/2), as fractions (`pxToTile` without the rounding). Because LEVEL_H equals TILE_H, the view looks along (1, 1, 1) in tile space; that's what makes the box test below exact.
- **A block's box** at `[c, r, l]` with `size [w, d, h]`: c−½…c−½+w, r−½…r−½+d, l−h…l.
- **A sprite's box** standing at (c, r, n) with `footprint [w, d]`: c−½…c−½+w, r−½…r−½+d, n…n+H, where H is the sprite's height above its anchor in levels (anchor y / LEVEL_H, at least ⅛ so it's never flat).
- **A group's box** is the union of its members' boxes (at rest, D6), and its drawn area the union of theirs.
- **Behind.** For two items whose drawn pixel bounds overlap: A is behind B if A's box ends before B's starts on any axis (A.c₁ ≤ B.c₀, or A.r₁ ≤ B.r₀, or A.l₁ ≤ B.l₀). Seen along (1, 1, 1), any such separation means every line of sight meets A first. If A is behind B on one axis and B behind A on another, their silhouettes can't overlap, and no order is set. If the boxes interpenetrate, the one whose centre has the smaller c + r + l is behind.
- **The order.** Each list is sorted on its own (a group's members inside it). The overrides split it into back, auto and front bands. In each band, a topological sort over the "behind" pairs, which always takes next the available item with the smallest key: (c + r + l of the box centre, then its list index). A cycle (three boxes that each cover the next) is broken by taking the remaining item with the smallest key. Taking the list index last makes a second sort a no-op (R2).
- Pairs are only tested when their pixel bounds overlap, using each item's drawn bounds (`objectPixels` bounds, cached per object). A scene of 300 items is well under a millisecond's worth of pair tests per list in practice; a test bounds it on the hero-island-sized example.
- `renderScene` calls `sortItems` on each list before drawing (R3). Variant runs (R9) are one node in the sort.

### D3. Keeping the list sorted (R4, R11)

- `cli/lib/scene-ops.mjs` gains `sortScene(doc, sources)`, which reorders `doc.items` and each group's `items` in place with D2's order, and returns the paths that moved. `insertIndex` and `insertIndexDeep` are removed (placing appends, then sorts).
- Every scene edit in `scene-ops.mjs` and `layer-ops.mjs` runs `sortScene` before it returns, inside the edit, so the session's undo step holds both. Returned paths are the item's path after the sort (callers already take the returned path).
- Merges (`doc.merge`, approving into an open copy) and `doc.reload` sort the result when they adopt it into an open session; a saved file is always written sorted. Opening a scene doesn't rewrite it; `check` says when it's out of order.
- An edit to an object's `size` or `footprint` changes the depth of the scenes using it. Their output stays right (R3); their files are re-sorted at their next edit, and `check` warns until then.
- `scene.sort` (`name` or `--all`, `dryRun?`): sorts and returns `moved` (paths, before → after) and `changedPixels` (the published render before and after, at time 0). It's an edit like any other, so over MCP it's a proposal.

### D4. Commands (R6, R7, R8, R13)

| command | change |
| --- | --- |
| `scene.raise`, `scene.lower` | removed (and MCP `scene_raise`, `scene_lower`) |
| `scene.place` | `index` removed; the item is appended and sorted. `at` may give `level` with `px` (`--px 40,-12 --level 4`). |
| `scene.set` | adds `depth` (`front`, `back`, `auto` to clear) and accepts a group's path for it; adds `level` for a pixel-placed item |
| `scene.move` | `--by c,r,l` moves a pixel item's level by l, as it moves a tile's |
| `scene.snap` | snaps a pixel item to the tile at its own level |
| `scene.duplicate` | the copy goes where it sorts (summary updated) |
| `scene.items` | lists in depth order; adds `depth` and a pixel item's `level` |
| `scene.sort` | new (D3) |
| `object.footprint` | new: `name`, `to w,d` (or `--clear`); sprites only (`usage` for a block) |
| `object.new`, `object.show` | `footprint` |
| `check` | the out-of-order warning (R11) and the copies line (R12) |

Errors use the existing codes. Pixel and tile coordinates in errors and spoken text gain "level n" for pixel items ("x 40, y −12, level 4").

### D5. Groups (R9)

- A group with `depth` sorts in its band in its parent list; its members' own `depth` marks act inside the group only.
- A variant run: two or more consecutive siblings that are groups with `data-class`. The sort treats the run as one node (union box, the run's first member's index), and the run keeps its internal order, so `composite`'s `firstVariant` and the site's variant CSS keep working.

### D6. Motion (R10, R12)

- **Where a moving item is.** At time t its box is its rest box moved by `stateAt(track).move` converted to tile space at its own level (D2's pixel rule, with the level fixed). Moves are pixel steps, so the position, and so the order, changes only at whole steps; the times to check are the keys and each step between them (`stepsFor`).
- **Its place in the list.** At each of those times the item's list is sorted with every moving item at its current box and every other item at rest. A moving item's place is (the static item it follows, its rank among the moving items after that same static item). The loop is split into windows where that place doesn't change.
- **The SVG.** A moving item with one place over the whole loop is drawn as today. One with several places is drawn once per place, each copy in its place in the list, inside a wrapper `mv-<id>-z<k>` whose opacity animation shows it only in its windows (step-end, as the pose groups do). Inside each copy the item's own motion group and poses are as today. The copy holding time 0 is the visible one in the base rule, so reduced motion shows the rest frame. Copies of moving items in the same gap are ordered by rank, so whichever two are visible at a time are in the right order.
- **The preview.** `renderSceneImage` with `time` sorts the posed items (D2, with each tracked item at its moved point and level) and draws that. A test checks, at every window boundary and the middle of each window, that the composite of the published SVG tree at that time equals `scene.render --time`.
- **Groups.** A group's box comes from its members at rest; a moving member reorders among its group's members (copies inside the group's `<g>`).
- **Size.** Each copy repeats the item's pixels. `check` lists, per scene, each moving item with more than one place and its copy count (R12).

### D7. The app (R14)

- `SelectionInspector`: the Raise and Lower buttons are replaced by a **Depth** segmented control: Auto, Front, Back (VoiceOver "Depth, Auto"; help "Front draws it over everything in its list; Back under everything"). The POSITION section shows Level for a pixel-placed item. The wrapper line (scene-layers D3) gains the group's depth with a small menu (Auto, Front, Back) that sends `scene.set --depth` on the group's path.
- `PixelLabApp`: the Edit menu loses Raise and Lower. `SceneEditor` loses `raise()`/`lower()` and the `]`/`[` keys in scene mode (object mode keeps them).
- The object inspector (sprites only): FOOTPRINT, two steppers (W, D) from 1 to 16.
- The Checks panel shows the out-of-order warning with a **Sort** button (`scene.sort`), and the copies line.
- The Layers panel's item rows keep list order (R4), which is now depth order; the timeline doesn't reorder rows.
- Colors from the board 03 tokens; no `@State`; no new dependencies.

### D8. The script hook (R15)

`ScriptRunner.swift` and `CLAUDE.md`'s list: `depth front|back|auto` (the selection), `sort`, `expect depth <front|back|auto>`, `expect order <path,path,…>` (the scene's top-level object names in list order, or the paths), `expect level <n>`; `raise` and `lower` are removed. `scripts/acceptance/scene-object.txt` drops its raise/lower lines and checks the override instead; a new acceptance script moves the pine behind the red house on a temp copy of `examples/harbor.pixellab` and checks the wall's pixel.

### D9. Docs (R16)

README (Scene mode tour: depth, the Depth control, Level for pixel items, Footprint; the shortcuts table), `app/README.md`, `docs/architecture.md` (depth in the scene format), `docs/testing.md` (the hook's new actions), `cli/README.md`, `CLAUDE.md` (hook list), the MCP instructions, `npm run docs:cli`, and the scene-layers wording that's now true. The previous paint-order rule in the record (the site's pixel-art engine spec, "Items paint in source order"; the mac-app plan's `insertIndex` note) is superseded by this spec; the mac-app plan gets a one-line pointer here, not a rewrite.

## Areas of concern

- **The order is worked out at draw time as well as kept in the list.** The owner chose "the list can be sorted after every change". The spec keeps that (R4) and also has the engine sort when it draws (R3), because motion has to be ordered at draw time anyway (R10), and because an object's size changing, a hand edit or a merge would otherwise publish a stale order. The list stays the readable record; the output never depends on it being sorted. If the owner wants the file to be the only source of order, R3 can drop to "draw in list order" for still items, at the cost of those three cases.
- **The site (external dependency).** The site's document gets `depth`, `level` and `footprint` only after it bumps its `pixellab` pin to the release with this change; older engines refuse those keys. Its art changes when the new engine draws it, even before any key is added, because the order is now computed. The site's migration, under its own workflow: bump the pin, run `pixellab scene sort --all --dry-run` and review each scene's before/after, add `depth: 'back'` on the `island-shadow` group, levels on raised pixel items (the crane jib and hook, clouds), footprints on large sprites, group the crane's parts, re-publish, and raise `hero-island.svg`'s budget in its manifest if the copies need it (the owner has agreed). Nothing here writes to the site.
- **The site's "random" entrance order** seeds each piece's delay with its index in the SVG, so re-sorting reshuffles which piece enters when. The other orders use `data-col`/`data-row`/`data-x`/`data-y` and are unchanged. A note for the site's migration.
- **SVG size from motion copies** (D6). hero-island is 518 KB of a 768 KB raw budget with 39 tracks; trains and containers crossing many tiles could add a lot. `check` shows it (R12); the budget can be raised in the site.
- **Two records said "items paint in source order"**: the site's pixel-art engine spec and the mac-app plan. The intent supersedes them; the mac-app plan gets a pointer (D9), and the site's spec is the site's to update.
- **Pixel items assume level 0 until told otherwise.** In the site's document every pixel item is read as standing on level 0 until it gets a `level`, so raised ones (the jib, the hook, clouds) may sort too far back until the site migration adds levels or overrides.
- **Interpenetrating boxes and cycles** have no true answer; the fixed key keeps them deterministic, and the override fixes any that look wrong.
- **Line merges.** Sorting moves lines, so an item moved on one side and edited on the other can show as a clash in a three-way merge where it used to merge. The merged result is sorted again (D3).

## Open questions

- From the intent, sorted at draw time or kept sorted: both; the list is kept sorted after every change, as the owner chose, and the engine also sorts when it draws (Areas of concern).
- From the intent, Raise and Lower: removed (R6); the override is the manual control.
- From the intent, pixel-placed items: ordered like tiles in sub-tile steps, with a level of their own (R7).
- From the intent, the `{ group }` wrappers: they sort as one object outside and sort their members inside (R9). Grouping by kind is what layers are for; layers still never change paint order.
- From the intent, motion: moving items reorder while they move (R10), without changing the list or the Layers panel. A group keeps its rest depth while its members move.
- From the intent, existing art: it changes and is migrated (R11); the site does its part under its own workflow.
- None carried forward.
