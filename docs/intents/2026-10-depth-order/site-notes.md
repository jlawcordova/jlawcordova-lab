# Notes for the site's migration to depth order

What `jlawcordova.github.io` does to adopt `2026-10-depth-order`, from the spec's [Areas of concern](spec.md#areas-of-concern), written for the site's own workflow (an intent in its repo). Nothing in this change writes to the site. The steps apply once the site pins a Pixel Lab release that has this change; older engines refuse `depth`, `level` and `footprint`.

Its art changes when the new engine draws it, even before any key is added, because the order is now worked out from where each item stands. The numbers below say how much.

## The numbers

Taken on a temp copy of the site's document (`src/assets/pixel-art/jlawcordova.pixellab`, as of 2026-10-20), on this change's branch with the sort follow-up (the rest sort moving fewer items) merged.

```sh
pixellab scene sort --all --dry-run --json --document=<temp copy>
pixellab check --json --document=<temp copy>
```

| Scene | Items moved | Published pixels that change (time 0) |
| --- | ---: | ---: |
| `hero-island` | 35 | 281 |
| `closeup-water` | 9 | 124 |
| `closeup-front-trees` | 13 | 76 |
| `closeup-office` | 1 | 36 |
| `closeup-crane` | 2 | 20 |
| `closeup-road-end` | 6 | 12 |
| `closeup-land` | 12 | 6 |
| `range-sprite` | 4 | 0 |
| the other seven (`achievement-frames`, `achievement-icons`, `closeup-sky`, `footer-dirt`, `footer-grass`, `library-demo`, `outfit-preview`) | 0 | 0 |

"Items moved" is the items the sort takes out of their place, and it is the number `check`'s `out_of_order` warning gives for each scene (the two agree). Across the document the sort moves 82 items and 555 published pixels change at time 0, in seven of the 15 scenes (eight have items that move). The pixel count compares the scene drawn in its file's list order (as the old engine publishes it) with depth order, so it is what the site sees when it bumps the pin.

Also on that copy:

- `check` has no problems, and its warnings are the 15 `off_grid` ones it already had plus the eight `out_of_order` above. It reports no `copies`: no moving item takes more than one place today, so the SVGs gain no copies. The SVGs the new engine builds are the same size as the published ones (`hero-island.svg` 518,568 bytes, `range-sprite.svg` 150,788; the four small ones are byte-identical), against budgets of 768,000 and 204,800 bytes.
- After the sort, `check` has no `out_of_order` warnings.
- `hero-island.svg` and `range-sprite.svg` differ from the published ones (`range-sprite` changes no pixel at time 0, so what differs is how the pieces are ordered or where moving ones are drawn; see "random" below for what the order affects).

## Migration steps

1. **Bump the pin** in `package.json` and `package-lock.json` to the Pixel Lab release with this change.
2. **Run `pixellab scene sort --all --dry-run`** and review each scene's moves and `changedPixels` against the table. Render before and after (`pixellab scene render <scene> --out …`, or Pixel Lab's stage) for `hero-island`, `closeup-water` and `closeup-front-trees`, the three with the most to see. Then `pixellab scene sort --all` writes the files in order (one undo step per scene in the app; the dry run is the CLI's global `--dry-run`).
3. **`depth: 'back'` on the `island-shadow` group.** The drop shadow (the `enter` group with `data-enter: 'shadow'` at the top of `hero-island`) is a run of pixel-placed `island-shadow` tiles at level 0, and should sit under the whole island: `{ group: { class: 'enter', 'data-enter': 'shadow' }, depth: 'back', items: [ … ] }`, or `pixellab scene set hero-island <group path> --depth back`.
4. **Levels on raised pixel items.** Every pixel item is read as standing on level 0 until it has a `level`, so raised ones may sort too far back. The crane jib (`jib`) and hook (`hook`), and the three clouds (`cloud-a`, `cloud-b`, `cloud-c`) are the known ones: `pixellab scene set hero-island jib --level 4` (pick the level from a render: how many tiles' worth of height the jib stands above its foot). `--level` only affects depth, not where the item draws. If a cloud should be over everything, `depth: 'front'` on it says so more plainly than a high level.
5. **Footprints on large sprites,** so they sort by the ground they cover: `house`, `locomotive`, the crane mast and the like, `pixellab object footprint house --to 2,2`. Blocks use their `size`. Check each with a render, and run `pixellab scene sort --all` again, since a footprint changes where the scenes that use it sort (`check` warns until they are sorted).
6. **Group the crane's parts.** Today `crane-foundation`, `crane-mast` plus `jib`, and `hook` are three separate `enter` groups. Put them in one group so the crane sorts as one object and the moving jib and hook reorder only among its own members, while the group keeps its rest depth (R10). That changes the `<g>` structure and the enter animation, so it is the site's call; keep the `data-x`/`data-y` the entrance uses.
7. **Re-publish** (`pixellab art build`, or the site's `npm run art`), review the SVG diff and commit the new SVGs with the document.
8. **Raise `hero-island.svg`'s budget if copies need it.** None do today (above), but moving items that gain a level or footprint can take more places, and each place is a copy of the item in the SVG. `check` lists the copies per scene before publishing; if the output goes over its budget it is still written and flagged, and the budget in `manifest.json` (`publish.budgets`) is the site's to raise (the owner has agreed).

## The "random" entrance order

The site's entrance animation seeds each piece's delay with its index in the SVG (`src/lib/island-entrance.mjs`, the `random` object order), so re-sorting reshuffles which piece enters when. The other orders (`ltr`, `same`, `back-front`, `front-back`, `together`) read `data-col`/`data-row`/`data-x`/`data-y` and are unchanged. If the owner wants the old shuffle, the seed needs something other than the index (the piece's `data-x`/`data-y`); otherwise just expect a different, equally random, order.

## Also

- The site's pixel-art engine spec says items paint in source order; that is superseded here (the record in this repo has a pointer), and updating the site's spec is the site's to do.
- If the site's `pixel-art` skill mentions raising or lowering, drop it: `scene_raise`, `scene_lower` and `scene_place`'s `index` are gone, and agents never order items (the override is `scene_set` with `depth`).
- An item moved in one branch and edited in another can show as a clash in a three-way merge now that sorting moves lines; the merged result is sorted again.
