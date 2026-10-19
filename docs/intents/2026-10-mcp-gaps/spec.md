# Spec: Keep agents' docs in step with disk, and fill the MCP's gaps (from intent.md 2026-10-20)
Status: draft (reviewed with its intent, plan and code in one PR, at the owner's request).

## Requirements

**Docs in step with disk**

- **R1. An MCP session settles its own proposals.** Before each tool call, the MCP session looks at every open doc it saved as a proposal. When that proposal is gone (applied or rejected), the doc is brought in line with disk: if its content matches the file, it's marked clean; if it has no edits since the proposal was written, it's reloaded from the file; if it has later edits and the file now holds the proposal (applied), it keeps those later edits on top of the file. Only a rejected proposal with later edits on top is left as it is, and `doc_status` says so. In every case the next edit and the next `doc_save` start from the file, and no revert is needed.
- **R2. Clean docs follow the file in MCP sessions too.** Before each tool call, a doc with no unsaved edits whose file changed on disk reloads, as the app's server already does when it sees the change. A dirty doc whose content now equals the file is marked clean.
- **R3. `doc.reload` reloads only when nothing is lost.** It loads the doc from disk when the working copy has no edits the file lacks: it's clean, or its content equals the file, or the proposal holding its edits was applied. It refuses (code `dirty`, the edits that would be lost in `details`) when there are unsaved edits, and (code `proposed`) while its proposal is waiting for the owner. It isn't destructive, and its MCP annotations say so; `doc_revert`'s now say it is.
- **R4. A proposal lists only the edits since the doc last matched disk.** Its `summary` (and `doc_save`'s `edits`) leaves out edits that are already on disk, whether they got there by a save, an applied proposal or a reload.
- **R5. The app says when a doc with unsaved edits changed on disk.** The open doc shows a banner naming the file, and offers **Merge** (a three-way merge of the file into the open copy, against the version it was opened or last saved as), **Reload** (the file, dropping the unsaved edits as one undoable step) and **Keep Mine** (hide the banner; a later save still asks before it overwrites the file). A merge that both sides changed the same lines of is refused, and the banner stays with the other two choices.
- **R6. Approving a proposal merges it into a dirty open copy.** When the doc is open in the app with unsaved edits, approving writes the file and merges the proposal into the open copy, keeping the owner's edits. If the merge clashes, the file is still written and the banner of R5 appears. The approve warning says this.
- **R7. Agents can see what the app has open with unsaved edits.** The app's server records its open docs in the state dir. In an MCP session, `doc_status` lists them as `unsavedInApp`, and `doc_save` adds a warning to a proposal for a doc that is unsaved in the app.
- **R8. `doc.merge` is a command,** so the app, the CLI's session and agents merge the same way.

**Scene edits the MCP couldn't express**

- **R9. A group's attributes follow the item they point at.** When `scene_move`, `scene_set` or `scene_snap` moves an item at rest (not a keyed move past time 0), and its enclosing group's `data-x` and `data-y` equal the item's placement point or its foot before the move, they move with it; `data-col` and `data-row` follow the item's tile the same way. `keepGroup: false` turns it off. The result says what followed.
- **R10. `scene.group.set` edits a group's attributes.** Set or remove any `class` or `data-*` attribute, or set `data-x` and `data-y` to the foot of the group's first item. The group is named by its path or by an item inside it.
- **R11. `scene.place` places inside groups.** Into an existing group (by its path or an item in it), or into a new group with given attributes (optionally with its foot), at a given index or where back-to-front order puts it.
- **R12. Motion tracks and keys are commands.** `scene.track.add` and `scene.track.remove`; `scene.key.set` (a key's `move`, `show` and `pose`, or clearing any of them), `scene.key.move` and `scene.key.delete`; `scene.motion.set` for the loop's length.
- **R13. Off-grid moves are warned about.** `check` lists a warning (not a problem) for every key-to-key move the item makes while it shows, including the loop back to the first key, whose steps aren't 1 px straight, 1:1 or 2:1, with the jump it makes. A jump while it's hidden (a reset) isn't seen, so it isn't listed. The key commands return the warnings of the track they changed.
- **R14. `scene.motion.path` turns a polyline into on-grid keys** at a steady speed (px per second) or between two times, keeping any show and pose keys on the way.

**Object edits**

- **R15. `object.map.set` replaces a whole map** of one layer or frame from rows, with an optional legend of characters to palette colors, as one undo step. It checks the palette, the color tiers, the 12-color cap and the 64×64 size cap, and refuses a size change unless asked to resize.

**Cross-cutting**

- **R16. Every new command reaches all three doors,** with tests named after these requirements, `docs/cli.md` regenerated, and the MCP server's instructions and tool descriptions updated.
- **R17. Notes for the site's pixel-art skill** describe each change, so its "What the MCP can't express" list can shrink. The site repo isn't changed.

## Design

### D1. Edits since disk (R4)

Each undo step gets a sequence number from a session-wide counter. An entry remembers `baseSeq`, the counter when the doc last matched disk (open, save, reload, a settled proposal). The edits a proposal lists are the labels of the undo steps after `baseSeq`, plus `Undid: <label>` for a step at or before it that was undone (it's in the redo stack). A doc not on disk keeps its `Created …` label first. Saving or settling also stops coalescing across the boundary (`last = null`), so a repeat edit after a save is a new step. The undo stack itself is kept, as today.

### D2. Settling (R1, R2)

`Session.sync()` runs at the start of every MCP `tools/call`. An entry that proposed records `proposed: { id, text, seq }` (the source it wrote and the step it was written at). For each entry:

| proposal | working copy | file | result |
| --- | --- | --- | --- |
| gone | equals the file | any | clean: base = file, `baseSeq` = now |
| gone | equals the proposal (no later edits) | anything else (rejected) | reloaded from the file, history cleared |
| gone | later edits | equals the proposal (applied) | rebased: base = file, `baseSeq` = the proposal's step; the later edits stay unsaved |
| gone | later edits | differs (rejected) | left as it is (still based on the old file); `synced.outcome: rejected, action: kept` |
| none | clean | changed | reloaded (as `reloadIfClean`) |
| none | dirty, equals the file | — | clean |

`doc.status` with a doc adds `synced: { proposal, outcome: applied | rejected, action: clean | reloaded | rebased | kept }` for the last settle, until the next save. The stdio server doesn't call `sync()`: its watcher already reloads clean docs, and its session doesn't propose.

### D3. `doc.reload` (R3)

`doc.reload --doc <ref>` (session only). After `sync()`, it reloads when the entry is clean or its serialized text equals the file, clearing history; it reports `reloaded: false` when there was nothing to load. It refuses with `proposed` while `entry.proposal` points at a proposal that still exists, and with `dirty` otherwise, listing the edits since disk. A clean doc whose file was deleted is closed. MCP annotations: `doc_reload` is not destructive; `doc_revert` gets `destructiveHint: true` (it drops edits).

### D4. Three-way merge (R5, R6, R8)

`cli/lib/merge.mjs`: `merge3(base, ours, theirs)` on lines, from `diffLines` in `diff.mjs`. Each side's diff against the base becomes hunks (a base range and its replacement). Hunks from both sides are walked in base order; one that overlaps no hunk of the other side applies; two that overlap (or are insertions at the same point) apply once if they're identical and are a conflict otherwise. Adjacent hunks that don't overlap both apply. In a scene, two insertions at the same point both go in, ours first (each side added items in the same place); in an object they clash, since two new keys could take the same character. The sources are canonical (one item or map row per line), so lines are the right grain: two people moving different items, painting different rows or adding items in different places merge; the same row painted by both clashes.

`doc.merge --doc <ref>` (session only, mutating): base is the file as the doc was opened or last saved (`entry.base.text`), theirs is the file now, ours is the working copy serialized. If the file hasn't changed, nothing happens. If the working copy is clean, it reloads. Otherwise a clean merge is imported (a merge that doesn't load is refused, code `merge_invalid`), becomes the working copy as one undo step ("Merged the changes to <file> from disk"), and the base becomes the file; the doc stays dirty with the owner's edits. A clash is refused with code `merge_conflict` and `details: [{ base: [from, to], ours: [lines], theirs: [lines] }]`, changing nothing.

`proposal.apply` in a session: after writing, a clean open copy reloads (as today); a dirty one is merged as above. The result adds `merged: true`, or `conflict: true` when it clashed (the file is written either way).

### D5. Stale notifications and the banner (R5, R6)

The stdio server sends a new notification, `doc.stale { doc, file }`, when a watched file changes for an open doc with unsaved edits whose file no longer matches its base, and after an apply whose merge clashed.

The app keeps `staleDocs` (doc refs), set by `doc.stale` and cleared when the doc merges, reverts, saves, or reports itself clean. Keep Mine adds the ref to `keptStale`, which hides the banner until the next `doc.stale` for that doc. The banner sits at the top of the stage, under the toolbar: a `Theme` warning tint, an `exclamationmark.triangle` symbol, "<file> changed on disk. You have unsaved edits here.", and buttons **Merge** (default), **Reload** and **Keep Mine**. A refused merge shows an alert ("Both you and the file changed the same lines"), and the banner stays. The save-conflict alert offers Merge too, beside Revert and Save Anyway. VoiceOver reads the bar as one element with its three buttons; the buttons have visible focus and standard hit targets; nothing animates.

The approve warning becomes: "Approving writes <file> with the agent's version and merges it into your open copy, keeping your unsaved changes. If you both changed the same lines, Pixel Lab asks what to keep." The Agent window's result line says "merged into your open copy" or "couldn't merge into your open copy; choose in the editor".

Script hook: `cli <args…>` runs the bundled CLI against the script's document (an outside editor writing the file), `stale merge|reload|keep`, and `expect stale true|false` (the banner is showing).

### D6. Open docs in the app (R7)

The stdio server writes `<state>/open/<pid>.json`, `{ pid, document: <root>, updatedAt, docs: [{ doc, dirty }] }`, when its open docs or their dirty flags change, and removes it when it stops. An MCP session reads every such file whose process is alive and whose document is its own. `doc.status` without a doc adds `unsavedInApp: [refs]`; with a doc, `unsavedInApp: true | false`. A proposal saved for a doc that's unsaved in the app returns `warnings: ["<ref> has unsaved edits in Pixel Lab; …"]`.

### D7. Groups (R9, R10, R11)

An item's **foot** is the site's rule: for an item on a tile, its tile's top-face center (`tileToPx`, which is also its placement point); for a sprite placed by pixel, the center of its bottom row, `[round(x − anchor[0] + width / 2), y − anchor[1] + height − 1]`; for anything else, its placement point.

**Following (R9).** For the item's innermost group: if `data-x` and `data-y` parse as integers equal to the item's placement point (or foot) before the edit, they're set to the new point (or foot). `data-col` and `data-row` equal to its tile's col and row before follow its new tile, while it stays on one. `scene.move`, `scene.set` and `scene.snap` take `keepGroup` (default true) and return `group: { path, attrs }` when something followed.

**`scene.group.set --name <scene> --group <path | item>`** with `attrs` (an object of strings; `''` removes an attribute) and/or `foot: true`. Names must be `class` or `data-*`, values strings without `" < > &`, as the engine checks. New attributes go after the existing ones.

**`scene.place`** gains `group` (an existing group: its path, or an item path or id inside it), `newGroup` (an object of attributes for a new group around the item), `foot` (with `newGroup`: its `data-x`, `data-y`), and `index` (the position in the list it goes into). By default an item on a tile goes in back-to-front order in that list; a new group goes where its item would, judging each group by its first item on a tile; a pixel placement goes last. The result adds `group` (the group's path).

### D8. Motion (R12, R13, R14)

New file `cli/lib/commands/scene-motion.mjs`, with the logic in `cli/lib/motion-ops.mjs`. Items are named by path or id; the item must have an id to have a track.

| command | params | does |
| --- | --- | --- |
| `scene.track.add` | `name, item, duration?` | a track `{ item, keys: [{ t: 0 }] }`; makes the scene's motion with `duration` if it has none |
| `scene.track.remove` | `name, item` | removes the item's track |
| `scene.key.set` | `name, item, time, move?, show?, pose?, clear?` | the key at `time` (made in order if missing) gets the values; `clear` lists props to remove |
| `scene.key.move` | `name, item, time, to` | moves a key to another time (not the first key; not onto another key) |
| `scene.key.delete` | `name, item, time` | deletes a key (not the first: remove the track) |
| `scene.motion.set` | `name, duration` | sets (or makes) the loop's length; refuses one that cuts off keys |
| `scene.motion.path` | `name, item, points, start?, speed? \| end?` | on-grid keys along the polyline, made in the track (made if missing) |

**On the grid.** A move between two keys steps `stepsFor(dx, dy)` times. It's on the grid when each step is 1 px straight, 1:1 or 2:1: `|dx|/n, |dy|/n` is one of (1, 0), (0, 1), (1, 1), (2, 1). `cli/lib/motion-ops.mjs offGridMoves(scene)` lists the rest, and `check` returns them as `warnings: [{ file, message, text }]`, after the problems, without changing `ok` or the exit code: `scenes/x.mjs: motion.tracks[2] (truck): 4200 → 5000 ms moves (16, -3) in 1 step of (16, -3); on the grid each step is 1 px straight, 1:1 or 2:1 (scene_motion_path makes such keys)`.

**The path helper.** Each polyline segment is split into on-grid runs: with `a = |dx|, b = |dy|`, when `a ≥ 2b` a 2:1 run of `b` steps then a straight run of `a − 2b`; when `b ≤ a < 2b` a 2:1 run of `a − b` and a 1:1 run of `2b − a`; when `a < b` a 1:1 run of `a` and a straight vertical run of `b − a`. Each run ends in a key; times follow the cumulative Euclidean length at `speed` px/s from `start` (or spread from `start` to `end`), rounded to whole ms and kept strictly increasing. Existing keys in `(start, end]` lose their `move`; those left with nothing are deleted; those with `show` or `pose` get the on-path point at their time (a whole number of steps into their run), so the path stays on the grid through them. The key at `start` gets the first point. A path that ends at or past the loop's length is refused with how long it takes. The result lists the keys written and the track's remaining warnings (such as the step from the path's end to the next key).

### D9. `object.map.set` (R15)

`object.map.set --name <object> --rows <row>… [--keys <legend>] [--layer n] [--frame n] [--resize]`. Rows are strings of one character per pixel, `.` transparent. With `keys` (`{ char: color }`), each legend character is mapped to the object's key for that color, adding keys as painting does (`edit.keyFor`); other characters must be the object's own keys. It refuses: rows of unequal width; a size over 64×64; a size other than the object's without `resize` (which pads or crops every map first, as `object.resize`); a color not in the palette or outfit colors on an object that doesn't extend `character`; a result over 12 colors (counted by the engine after the change); blocks. On an object that extends another, a still layer's rows become row overrides (rows equal to the base are left out), as painting does; a size change is refused. One undo step; the result has `size`, `colors` and any `keysAdded`.

### D10. Doors and docs (R16, R17)

All new commands are in the registry, so the CLI (one-shot, except the session-only `doc.reload` and `doc.merge`) and both servers have them. MCP: tool descriptions come from the commands; the server's instructions mention settling, `doc_reload`, `unsavedInApp`, groups, motion and `object_map_set`. `scripts/docs-cli.mjs` documents `doc.stale`, `open/<pid>.json` and the proposal summary rule. The README, `docs/architecture.md`, `docs/testing.md`, `cli/README.md`, `app/README.md` and `CLAUDE.md`'s script-hook list are updated. The site notes are `site-notes.md` in this folder.

## Areas of concern

- **Merging by line.** A line merge of canonical sources is predictable and matches the diffs the owner already reads, but it's coarser than a merge of items: two edits to the same map row clash even when they touch different pixels. Accepted for now; a clash never loses anything, since the merge is refused and the owner picks Reload or Keep Mine.
- **Following group attributes guesses intent from equality.** A group whose `data-x`/`data-y` happen to equal an item's point would follow it. It only happens to the item's own enclosing group, the result says so, and `keepGroup: false` turns it off.
- **The foot rule is the site's convention,** not the engine's. It's used only to recognise and set `data-x`/`data-y`, which only the site reads.
- **The state dir gains `open/`.** Old servers ignore it. A crashed app leaves a file behind; it's ignored once its process is gone.
- **Command name.** The intent says `object_set_map`; command names are `noun.verb` (`object.layer.add` → `object_layer_add`), so it's `object.map.set` → `object_map_set`.
- **Site dependency:** none. The site's skill can use the new tools once it pins a release with them; the notes say what to change there.

## Open questions

- None carried forward.
