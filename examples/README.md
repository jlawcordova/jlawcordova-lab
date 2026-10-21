# Example documents (`examples/`)

Two small `.pixellab` documents with scene layers, to try the app on and to test with. They're in format 2 (JSON sources), made from scratch by [`scripts/make-examples.mjs`](../scripts/make-examples.mjs), which drives the `pixellab` program (`target/release/pixellab`) with the starter palette, so they're the same on every run. `npm run examples` rewrites them, and `npm test` checks the committed copies still match. Each scene is written in depth order, back to front by where its items stand, as every edit in the app leaves it; reorder nothing by hand. Neither is the site's art.

| Document | Scenes | What it has |
| --- | --- | --- |
| `harbor.pixellab` | `harbor`, `harbor-flat` | An 8×8 island: 89 items in 16 layers up to three deep, the site's `{ group }` wrappers around every tile and object, two rocks in no layer, an animated boat that hops (the `hop` action), and a barrel on the dock that bobs (an ambient loop). `harbor-flat` is the same scene with no layers, and publishes the same SVG (apart from the scene name its generated class and keyframe names are scoped by). |
| `town.pixellab` | `town` | A 16×16 island: 379 items in 59 layers four deep (district › street › house), with eight lamps in no layer. |

The harbor's layers:

```text
Ground       Grass, Paths, Beach, Water
Village      Houses (Red house, Wood house, Town hall), Garden
Harbor       Dock, Boat, Lamps
Trees
```

The town's: Land (Grass, Paths), then North-west, North-east, South-west and South-east, each holding Street 1 to 3 of three houses (walls and a roof) and its Trees.

## Open one

```sh
scripts/build-app.sh            # once, or after pulling app changes
scripts/try-example.sh          # the harbor
scripts/try-example.sh town
```

[`scripts/try-example.sh`](../scripts/try-example.sh) copies the example into a new temp folder and opens `build/Pixel Lab.app` on the copy, with a state folder of its own, so saving, publishing and proposals never touch the examples here, your own document or the installed app. It prints where the copy is. Quit Pixel Lab to end. Opening an example from the repo with **File › Open** works too, but then saves change the files in `examples/`.

## Things to try

- **Pick, then go inside.** Click a house on the harbor: the whole Village is picked. Double-click to go into it, then again for Houses, then a house; the breadcrumb pill shows `harbor › Village › Houses`, and **Done** or <kbd>Esc</kbd> comes back out. In the town it takes three double-clicks to reach one house's walls and roof.
- **Hide and lock.** Hide Ground and the island floats; lock Village and clicking a house picks nothing, until you go inside Village (right-click its row, **Go Inside**).
- **Move a layer.** Drag a district of the town, or press the arrow keys with it selected: its streets, houses and trees move together, by whole tiles. <kbd>⌘Z</kbd> puts them back.
- **Group and ungroup.** <kbd>⇧</kbd>-click the two rocks on the harbor and press <kbd>⌘G</kbd>, or right-click for the menu. <kbd>⇧⌘G</kbd> ungroups.
- **Place into a layer.** Select Trees, pick a pine in the Library (the chip says "New items go into Trees") and place it.

From the command line, name the document with `--document` (build the program first with `cargo build --release`; `pixellab` below is `target/release/pixellab`, or the one in the app bundle):

```sh
pixellab scene items town --document examples/town.pixellab
pixellab scene render harbor --hide village --out /tmp/harbor.png --document examples/harbor.pixellab
pixellab art build --dir /tmp/harbor-art --document examples/harbor.pixellab
# the same bytes once the scene's name is taken out: layers never change the art
diff <(sed 's/harbor-flat/harbor/g' /tmp/harbor-art/harbor-flat.svg) /tmp/harbor-art/harbor.svg && echo same
```

`art build` flags the harbor as over its 100 KB budget. That's a warning, not a failure: both SVGs are still written.

Commands that edit save into the folder you name, so point them at a copy (`pixellab document copy --to <path>.pixellab`).

## Tests that use them

- `scripts/test/examples.test.mjs`: the committed examples match the generator, each one checks clean, the harbor's and the town's layers nest as described and move as one, and `harbor` and `harbor-flat` build the same SVG.
- `scripts/acceptance/examples-harbor.txt` and `examples-town.txt`: [`scripts/acceptance.sh`](../scripts/acceptance.sh) runs them on temp copies, to pick, go inside, hide, lock, move and walk the tree from the keyboard in layers read from a file rather than made in the app.

## Changing them

Edit `scripts/make-examples.mjs`, then:

```sh
npm run examples
```

It rewrites `examples/` and fails if a document doesn't check clean. Commit the generator and the examples together; update the acceptance scripts' `expect` lines when what they pick or count changes.
