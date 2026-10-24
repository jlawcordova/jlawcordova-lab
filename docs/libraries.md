# Libraries and palette files

A document can share its objects and its colors with other documents, by copying. There are two kinds of file:

- **An object library, `.pixellib`:** a package (a folder the Finder shows as one file) holding objects without scenes.
- **A palette file, `.pixelpal`:** one JSON file holding a palette on its own.

Nothing links a document to a library or palette file after an import. Importing the same file again later copies again; editing the file later changes nothing in the documents it was imported into.

Objects also carry **categories**, which is how a library's objects arrive sorted. The design record is [the change's spec](intents/2026-10-object-libraries/spec.md).

## Categories

An object's `category` is a list of names, outermost first: `["Nature", "Trees"]`, shown as Nature › Trees.

- A name is 1 to 32 letters, digits, spaces, `&`, `'` and `-`, with no leading, trailing or doubled spaces.
- Names compare without regard to case.
- `All` and `Others` can't be the outermost name: they're the Library's own tabs.
- A category never changes how an object renders, a scene, or anything Publish writes.

In the app, the object inspector's **Category** field sets it: type a path with › or / between names ("Nature / Trees"), or pick one from its menu. The Library popover shows a tab for each outermost category (All first, Others last, for objects without one), and subcategories as headings inside a tab.

From a terminal or an agent:

```sh
pixellab object category pine tree --to "Nature/Trees"
pixellab object category pine --clear
pixellab object list --category Nature
```

## What a library holds

```
coast.pixellib/
  library.json     { "format": 1, "kind": "library", "name": "coast", "exportedFrom": "harbor", "colors": { "world": {..}, "outfit": {..} } }
  objects/*.json   the objects, as in a document, with their categories
```

- `colors` records the value of every color name the objects use, at export. It's never used as a palette: it's how an import knows when a color differs.
- A library has no scenes, no palette of its own and no publish settings. Its files are canonical JSON, with no dates or paths, so the same export gives the same bytes.
- `pixellab check --library coast.pixellib` validates one. `pixellab library info coast.pixellib` lists its objects, categories and colors.
- A library isn't edited in place, apart from its categories (`pixellab library category`). To change its art, change the document it came from and export again.

## Exporting a document as a library

In the app, **File › Export Library…** asks where to save, with an Objects menu: All Objects, or one category (and everything below it).

```sh
pixellab library export ~/Art/explainer.pixellib
pixellab library export ~/Art/trees.pixellib --category Nature/Trees
pixellab library export ~/Art/pine.pixellib --objects pine
```

An export always includes every object the exported ones extend, so the library stands on its own. It writes a new file, and replaces one only with `--replace`. Exporting doesn't change the document.

**File › Export Palette…** (or `pixellab palette export ~/Art/coast.pixelpal`) writes the whole palette, both tiers, names, values and color notes.

## Importing a library

**File › Import Library…**, or drop a `.pixellib` on the window, or open one from the Finder: Pixel Lab offers to import it into the open document. From a terminal, `pixellab library import <file>`; add `--dry-run` to see the plan first.

What's copied:

- Every object, with its category. They show in the Library popover at once.
- **A name that's taken** keeps both: the imported object becomes `pine-2` (the first free suffix), and everything in the import that extends `pine` extends `pine-2`. The document's own objects are never renamed or changed.
- **An object identical to the document's**, apart from its category, isn't copied: the import uses the document's.
- **`character`**, the figures' base, is special. When the library's `character` differs from the document's, it isn't copied, and the imported figures extend the document's own `character`. Pixel Lab says so first, because they may look different.

What warns first:

- **Colors that differ.** The document's palette colors the imported objects, by name. When a name had another value in the library (wood-3 was #7A4E2D, the document's is #6E4428), the sheet shows both swatches and offers **Import Anyway** or **Cancel**. The command refuses with `differs` unless given `--accept-differences`.
- **Colors the document doesn't have.** They're never added to the palette by an import. Each is converted to one of the document's colors of the same tier, with the nearest picked first, and the sheet says plainly that this changes how the imported objects look. The command refuses with `missing_colors` (listing each one and its nearest color) unless `--convert '{"wood-5":"wood-3"}'` names a color for every missing one. An outfit color converts only to an outfit color.

An import never changes the document's palette, its existing objects or its scenes, so everything Publish writes stays byte-identical. It's one version in the history ("Imported 12 objects from explainer.pixellib"): **Edit › Undo Import** takes it back while nothing has followed it, and the Versions panel's **Undo These Changes** does it any time.

An agent that hits a color warning asks the owner (`chat_ask`) before it accepts or converts.

## Importing a palette file

**File › Import Palette…**, or drop a `.pixelpal` on the window, or `pixellab palette import <file> [--dry-run]`.

- It **adds** the colors the document doesn't have, by name, at the end of their tier. It never removes one, and colors with the same name and value are left alone.
- **A name with another value** in the file is the owner's choice, one by one: **Keep Mine** (the default) or **Use Theirs**, which changes how every object using that color looks. The command refuses with `palette_differs` unless each is in `--keep` or `--use-theirs`.
- A name that's in the other tier in the file is listed, and stays as the document has it.
- **The caps** (32 world colors, 16 outfit colors) hold: an import that would pass one is refused (`palette_full`, with how many are over), and `--only` narrows it to a chosen list of colors.

It's one edit: one undo step and one version ("Imported 4 colors from coast.pixelpal").

## Try it

The examples have both kinds of file: [`examples/coast.pixellib`](../examples/coast.pixellib/library.json) is exported from the harbor, and [`examples/coast.pixelpal`](../examples/coast.pixelpal) is its palette. Importing them into a copy of the town shows a rename (`rock-2`), a missing color to convert, a differing color, and a palette import that asks about it.

```sh
cp -R examples/town.pixellab /tmp/town.pixellab
pixellab library import examples/coast.pixellib --dry-run --document /tmp/town.pixellab
```
