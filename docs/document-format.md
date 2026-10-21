# Document format

A `.pixellab` document is a folder of JSON files, **format 2**. The Mac app, the `pixellab` CLI and agents over MCP all read and write it, and a site publishes SVGs built from it. This page is the reference for the files. The design is in [the Rust engine spec](intents/2026-10-rust-engine/spec.md) (R6, R7, D4 to D6); what comes out of a build is in [the site contract](site-contract.md). Run `pixellab check` to see any problem in a document, with the file it's in.

The examples below are from `fixtures/sample.pixellab`, the test fixture.

## The folder

```
sample.pixellab/
  manifest.json      the document's settings
  palette.json       the colors
  objects/<name>.json   sprites and blocks
  scenes/<name>.json    scenes: objects placed on the grid
```

Object and scene names are lowercase words joined by hyphens (`pine-tall`, `hero-island`); the file name is the name.

## manifest.json

```json
{
  "format": 2,
  "name": "sample",
  "publish": {
    "dir": "../sample-out"
  },
  "preview": {
    "card": "#E9DCC6",
    "grid": "rgba(90, 62, 43, 0.06)"
  },
  "theme": {
    "darkPage": "#1C1712"
  }
}
```

| Key | Meaning |
|---|---|
| `format` | Must be `2`. A `1` is the old format (see below). |
| `name` | The document's name. |
| `publish.dir` | Where `art build` writes the SVGs, a path relative to the document. |
| `publish.images` | Where `art build` writes the PNGs that scenes declare with `image`, a path relative to the document. |
| `publish.budgets` | Per output, `{ "<output>.svg": { "raw": <bytes>, "gzip": <bytes> } }`. Over a budget is a warning, never a failure. |
| `preview.card`, `preview.grid` | The card color and grid line color of previews and scene images. |
| `theme.darkPage` | A `#RRGGBB` color for the night theme. |

Any other key is a problem.

## palette.json

```json
{
  "notes": ["The shared pixel-art palette."],
  "world": {
    "ink": "#2E2418",
    "grass-1": "#B5C79C",
    "soil-3": "#5A3E2B"
  },
  "outfit": {
    "teal-1": "#7FA89B"
  },
  "colorNotes": {
    "soil-3": "--color-earth"
  }
}
```

- `world` and `outfit` map a color name to an uppercase `#RRGGBB` (no alpha). Names are lowercase kebab-case and unique across both tiers. The caps are 32 world colors and 16 outfit colors.
- Only objects that extend `character` may use an `outfit` color.
- `notes` is a list of lines: the file's header text, carried over from the comments of format 1.
- `colorNotes` maps a color's name to its note. A name that isn't a color of `world` or `outfit` is a problem.

## Objects

An object is a **sprite** (`"kind": "sprite"`) or a **block** (`"kind": "block"`). Either may have `notes`, a list of strings.

### Sprites

```json
{
  "kind": "sprite",
  "anchor": [2, 3],
  "keys": {
    "a": "grass-4",
    "b": "grass-2",
    "c": "ink"
  },
  "layers": [
    {
      "map": [
        ".bb.",
        "baab",
        "caac",
        ".cc."
      ]
    }
  ]
}
```

| Key | Meaning |
|---|---|
| `anchor` | `[x, y]`, the pixel of the sprite that sits on the point it's placed at. |
| `footprint` | `[wide, deep]`, the tiles a large sprite covers, 1 to 16 each. Depth order uses it. |
| `keys` | One printable character (not a space or `.`) to a palette color name. In a map, `.` is empty. |
| `layers` | At least one layer, painted in order. A layer is a map layer or a frame loop. |
| `extends` | The name of another sprite. The object then gives `rows`, not `layers`: `{ "<layer>": { "<row>": "<string>" } }` replaces rows of the base's layers. |

A **map layer** is `{ "class", "map" }`: `map` is a list of strings, one string per row, all the same width, with `class` optional (lowercase class names separated by single spaces). A sprite is at most 64 by 64 pixels and uses at most 12 colors.

A **frame loop** layer is `{ "loop", "prefix", "frames" }`: `loop` and `prefix` are single lowercase class names, and `frames` is a list of at least one map. The compiled SVG plays the loop on its own (the site contract's [animation section](site-contract.md#animation-plays-by-itself)): the editor's 200 ms per frame, and for the loops named `wf`, `ff` and `hf`, the cycles the site has always played.

Either kind of layer may also carry an `ambient` loop and `actions` (below).

### Blocks

```json
{
  "kind": "block",
  "size": [1, 1, 1],
  "faces": {
    "top": "grass-2",
    "left": "soil-2",
    "right": "soil-3"
  }
}
```

- `size` is `[columns, rows, levels]`: two whole numbers of at least 1 and a height of 0 or more. A block with 0 levels is a flat tile and has no `left` or `right` face.
- `faces` are `top`, `left`, `right` and `edge`, each a palette color name. `top` is required; a block with levels needs `left` and `right`.
- `surface` textures the top with one tile's map (`{ "keys", "map" }`, or `{ "keys", "loop", "prefix", "frames" }`), and `sides` textures the sides (`{ "keys", "left", "right" }`, one level high).
- A block has no `footprint`: its size is its footprint.

## Scenes

```json
{
  "output": "sample.svg",
  "viewBox": [-16, -16, 64, 48],
  "items": [
    { "object": "block", "at": { "tile": [0, 0, 0] } },
    { "object": "pebble", "at": { "tile": [0, 0, 1] } }
  ]
}
```

| Key | Meaning |
|---|---|
| `output` | The SVG's file name (`kebab-case.svg`), unique across scenes. A scene without one is only for the app and previews. |
| `viewBox` | `[x, y, width, height]`, whole numbers, in pixels. |
| `origin` | `[x, y]`, the pixel of tile `[0, 0, 0]`'s anchor. |
| `image` | `{ "file": "rest.png", "scale": 5 }`: `art build` also renders the viewBox on the manifest's preview card color at `scale` (1 to 16) and writes it to `publish.images`. The file is a plain `name.png`, and the manifest needs `publish.images`. |
| `layers` | Named parts of the scene for the editor: `{ "id", "name", "parent" }`. They never change paint order or the SVG. |
| `items` | Objects and groups, listed back to front. |
| `motion` | Scene motion: `{ "duration", "tracks" }`, each track `{ "item", "keys" }` for an item with an `id`. A key is `{ "t", "move", "show", "pose" }`. |
| `notes` | A list of strings. |

### Items

`{ "object": "<name>", "at": … }` places an object. `at` is one of:

- `{ "tile": [col, row, level] }`: on the isometric grid, three whole numbers.
- `{ "px": [x, y] }`, with an optional `"level"`: at a pixel point.

Other item keys:

| Key | Meaning |
|---|---|
| `id` | Lowercase kebab-case, unique in the scene. Needed for a motion track to name the item. |
| `class` | Class names (lowercase, separated by single spaces) written on the item in the SVG. |
| `layer` | The id of one of the scene's `layers`. |
| `depth` | `"front"` or `"back"`: draw over or under everything else in its list. Without it, items sort by where they stand. |
| `ambient`, `actions` | See below. |

A **group** is `{ "group": { … }, "items": [ … ] }`. The `group` object holds the attributes written on the `<g>`: `class` and `data-*` only, as strings without `"`, `<`, `>` or `&`. They pass through to the SVG unchanged ([the site contract](site-contract.md)). A group may take `depth` too. An item can't have scene motion and an ambient loop at once.

## Ambient loops and actions

On a scene item, or on an object's map layer (beside its `class` and `map`):

```json
"ambient": { "duration": 16000, "offset": 0, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 8000, "move": [12, 0] }] },
"actions": {
  "hop": { "duration": 480, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 44, "move": [0, -2] }] }
}
```

- **Keys** have only `t` and `move`. `t` is in milliseconds, from 0 up to `duration`, strictly rising, and the first key is at `t: 0`. `move` is two whole numbers, pixels from where the art is drawn. Between keys the element steps a whole pixel at a time, as scene motion does.
- **`duration`** is 1 to 600000 ms.
- **An ambient loop** repeats on its own. After the last key it steps back to the first over the rest of `duration`. `offset` starts the loop that many ms in.
- **An action** plays only when the page asks, and replaces the ambient loop while it plays. It holds its last key until `duration`, then returns to rest. Action names follow the name rule (lowercase words joined by hyphens), are unique per element, and an element has at most 16.

How a page plays an action is in [the site contract](site-contract.md#actions). Over the CLI, `scene ambient`, `scene action`, `object ambient` and `object action` edit these ([commands](cli.md)).

## Canonical form

Every file in a document is stored in one **canonical form**, so a hand edit and a save give small, comparable diffs. Saving in the app or with `pixellab` writes it, and `pixellab check` warns about a file that isn't in it (`isn't in canonical form`):

- two-space indentation, no trailing commas, and a final newline;
- keys in a fixed order for each kind of file (for example a sprite's `notes`, `kind`, `extends`, `anchor`, `footprint`, `keys`, `rows`, `layers`; a scene's `notes`, `output`, `image`, `viewBox`, `origin`, `layers`, `items`, `motion`); keys the order doesn't know come after;
- a list of numbers on one line (`"size": [1, 1, 1]`);
- any other list one element per line, so a map's rows are one string per line;
- one line for each placed item, scene layer, motion key and ambient or action key, and for a scene's `image`, so a scene diffs a line per item;
- an item's `at.level` left out when it is 0.

## Format 1

Format 1 kept the same art in JavaScript files (`palette.mjs`, `objects/*.mjs`, `scenes/*.mjs`, `manifest.json` with `"format": 1`), or a site's legacy `source/` folder. A format-1 document opens with the problem "<name> uses the old format. Migrate it with `pixellab document migrate`, or in the app."

```sh
pixellab document migrate                       # in place
pixellab document migrate --to new.pixellab     # a copy
```

Migrating checks, before it writes anything, that the format-2 document builds byte-identical SVGs to the original. Comments become `notes` and `colorNotes`; the generated header line is dropped. It refuses if the document has unsaved edits open. It isn't offered over MCP. In the app, the setup screen offers **Migrate…**.
