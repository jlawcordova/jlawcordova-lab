# Notes for the site's pixel-art skill

What changed in Pixel Lab for agents editing the art over the MCP, written for `jlawcordova.github.io`'s `.claude/skills/pixel-art/SKILL.md` (and `pixel-art-motion`). They apply once the site pins a Pixel Lab release with this change. Nothing here changes the site; its own workflow updates the skill.

## The "What the MCP can't express" list

All three items can go. Replace the section with a short pointer to the tools below, and drop the "write the source with the owner's OK" steps for them.

- **A new item inside an `enter` group** → `scene_place` takes `group` (an existing group's path, or an item path or id inside it), `newGroup` (attributes for a new group around the item, such as `{ "class": "enter", "data-enter": "object" }`), `foot: true` (with `newGroup`: sets its `data-x`/`data-y` to the item's foot) and `index` (its position in that list, 0 at the back). A new group with no index goes where its item would in back-to-front order, judging groups by their first item on a tile; for the hero island's paint-order rules, give `index`.
- **A group's attributes** → `scene_group_set` sets or removes (`''`) any `class` or `data-*` attribute, by the group's path or an item inside it, and `foot: true` sets `data-x`/`data-y` to the foot of its first item (a tile's center for an item on a tile; for a pixel-placed sprite, `[round(x − anchor[0] + width / 2), y − anchor[1] + height − 1]`, the skill's rule). And `scene_move`, `scene_set` and `scene_snap` now keep the item's own group in step: `data-x`/`data-y` that equalled its placement point or foot, and `data-col`/`data-row` that equalled its tile, follow it, and the result says so (`group`). `keepGroup: false` leaves them. A keyed move past time 0 never touches them. Moving the first item of a multi-item `object` group (the crane's mast) moves the group's foot but not the other items: move them too, or the island test catches it.
- **Show and pose keys, whole tracks** → `scene_track_add` (an item with an id; `duration` makes the scene's motion if it has none), `scene_track_remove`, `scene_key_set` (`time`, `move`, `show`, `pose`, `clear: ["move" | "show" | "pose"]`), `scene_key_move`, `scene_key_delete` (not the key at 0), `scene_motion_set` (the loop's length; refuses to cut off keys). "Its `show` and `pose` keys are written in the source" in the Pixel Lab section can go.

## Motion: off-grid moves

- `check` now lists **warnings** after its problems (they don't fail it): a key-to-key move, the loop back included, whose steps aren't 1 px straight, 1:1 or 2:1, made while the item shows. Moves while it's hidden (a reset) aren't listed. The key tools return the warnings of the track they changed.
- `scene_motion_path` keys an item along a polyline (moves from its placement, the first at `start`), at `speed` px per second or spread to `end`, splitting each segment into on-grid runs; show and pose keys on the way keep a point on the path. Suggest it in `pixel-art-motion` wherever a move has to go diagonally at an angle that isn't 1:1 or 2:1.
- The site's hero island has 15 such warnings today (2026-10-20), all small jumps the owner may not mean: the four ore and stone belts' `(4, −1)` moves (`ore-belt-1` 2400 → 2600 ms and its repeats), the ore carts' `(−6, 8)` and `(−2, 8)`, and the hoppers' `(9, 3)`, `(13, 1)` and `(5, 1)`. Worth a look with the owner; `scene_motion_path` can redo them on the grid.

## Maps

- `object_map_set` replaces a whole map of one layer (or one frame of a loop) in one call and one undo step: `rows` top to bottom (`.` transparent) and an optional `keys` legend `{ char: color }`, whose characters become the object's keys for those colors (made as painting makes them, so the legend can use any characters). It checks the palette, outfit colors, the 12-color and 64×64 caps; a different size needs `resize: true`; an object that extends another gets row overrides.
- In "Draw maps in bulk", replace "one `object_paint` call per color … one `object_erase`" with one `object_map_set` call. Check the keys it chose with `object_show`, as for painting.

## Proposals and the owner's open docs

- **The session follows disk.** After the owner applies or rejects a proposal, the MCP server brings the doc back in line with the file before the agent's next call (`doc_status` shows `synced: { proposal, outcome, action }`): clean if it matches, reloaded if nothing came after the proposal, rebased (later edits kept on top) if it was applied. The next proposal starts from the file and lists only the edits since then. No `doc_revert` is needed any more; say so where the skill handles "after approval".
- **`doc_reload`** reloads a doc only when nothing unsaved is lost (refused with `dirty` or, while its proposal waits, `proposed`). `doc_revert`, which drops edits, is now annotated destructive.
- **The owner's unsaved edits.** `doc_status` lists `unsavedInApp`: docs open in Pixel Lab with unsaved edits. Before proposing against one, tell the owner; `doc_save` returns a `warnings` line when it happens.
- **Pixel Lab no longer keeps showing old art.** When a doc the owner has open with unsaved edits changes on disk, the app shows a bar with Merge (a three-way merge, refused only when both changed the same lines), Reload and Keep Mine; approving a proposal for such a doc merges it in the same way. The paragraph "The owner's hand edits land on disk … `doc_save` from the app will conflict" can shrink to: if the owner sees the bar, Merge keeps both; only a clash on the same lines needs them to choose.
