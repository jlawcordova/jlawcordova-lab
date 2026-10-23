# Spec: Object libraries and palette files (from intent.md 2026-10-23)
Status: approved (the owner asked in chat on 2026-10-23 for the spec and plan to be made and the change built, without settling the Areas of concern first; each concern below is settled with Claude's recommendation, marked **Settled**, for the owner to confirm or change in the final PR's review).

This spec is for the intent as revised on 2026-10-23 for autosave (`../2026-10-autosave/spec.md`) and the floating Library (`../2026-10-floating-library/spec.md`): edits apply as they're made, and the Library is the popover above the floating bar.

It adds three things:
- **Categories.** An object can carry a nested category, set in the object inspector and shown as tabs and headings in the Library popover.
- **Object libraries.** A new package, `.pixellib`, that a document exports its objects to and imports objects from, by copying.
- **Palette files.** A new file, `.pixelpal`, that a document exports its palette to and imports colors from, by copying.

It also covers putting the owner's existing art into categories: the site's document and the explainer document (R24–R26).

Every new action is a command in the registry, so the CLI, the app (`serve --stdio`) and agents (`serve --mcp`) can all take it. Categories never reach what Publish writes.

## Requirements

**Categories**

- **R1. An object can have a category.**
  - A category is a path of one or more names, outermost first: Nature › Trees.
  - The depth isn't limited.
  - An object has at most one category, or none.
  - Sprites and blocks can both have one.
- **R2. Category names are readable words.**
  - A name is 1–32 characters: letters, digits, spaces, `&`, `'` and `-`.
  - A name has no leading, trailing or doubled spaces.
  - Names compare without regard to case. Two objects in "trees" and "Trees" are in one category, which shows with the spelling of the alphabetically first object that uses it.
  - "All" and "Others" can't be the outermost name, because they're the popover's own tabs (R5).
  - `pixellab check` reports a name that breaks these rules as a problem, as it does any other.
- **R3. Categories never change the art.**
  - Adding, changing or clearing a category never changes a rendered object, a scene's render, or any file Publish writes. They stay byte-identical.
  - A category is not part of a scene and doesn't move or reorder anything.
- **R4. The object inspector sets the category.**
  - While an object is open, the right panel's Object section has a Category field.
  - The field takes a path typed with › or / between names ("Nature / Trees").
  - Its menu lists the document's categories to pick from, plus None.
  - A change is one edit and one undo step: "Set pine's category to Nature › Trees".
  - The field shows a typed name that breaks R2 in place, with the reason, and doesn't set it.
- **R5. The Library popover has category tabs.**
  - A row of tabs sits below the search field and above the grid.
  - **All** comes first and is chosen when the popover first opens.
  - Next, one tab for each outermost category, in alphabetical order.
  - **Others** comes last and holds every object with no category. It shows only when at least one object has no category.
  - When no object has a category, the row doesn't show, and the popover looks as it does today.
  - The chosen tab is remembered per document while the app runs.
  - When the row doesn't fit the popover's width, it scrolls sideways with a quiet scroller (`../2026-10-floating-library/spec.md`, R5). The chosen tab scrolls into view.
- **R6. Subcategories are headings inside a tab.**
  - In a category's tab, the objects in that category itself come first, with no heading.
  - Each subcategory follows under a heading, in alphabetical order.
  - A deeper subcategory's heading shows its path below the tab: "Pines › Snowy".
  - In All, every object shows, grouped under headings for its full category path, in alphabetical order. Objects with no category come last, under an "Others" heading.
  - Within a group, objects are in name order, as today.
- **R7. Search works within the chosen tab.**
  - The search filters the objects of the chosen tab, keeping their headings.
  - A heading with no matches hides.
  - Searching from All searches everything.
- **R8. Tabs and headings work from the keyboard and with VoiceOver.**
  - Tab moves from the search to the tab row, then to the grid. ← and → move between tabs.
  - ⌃Tab and ⌃⇧Tab choose the next and previous tab from anywhere in the popover.
  - In the grid, the arrow keys move through cells in reading order across headings. ↓ from a group's last row goes to the next group's first row, in the same column or the last cell.
  - VoiceOver reads the row as a tab group named "Categories", reads each heading as a heading, and reads a cell as "pine, Nature › Trees".
  - Tabs have visible focus and the popover's hit target sizes.
- **R9. Agents and the CLI read and set categories.**
  - `object.list` and `object.show` return each object's category.
  - `object.list` can filter to a category and everything below it.
  - `object.category` sets or clears one object's category, or several objects' at once, in one edit.

**Object libraries**

- **R10. A `.pixellib` holds objects without scenes.**
  - It's a package (a folder the Finder shows as one file), like a `.pixellab`.
  - It holds a manifest, its objects as the same files a document uses (with their categories), and a record of the palette colors its objects used when it was exported (R14).
  - It has no scenes, no palette the objects draw with, no publish settings and no link to a palette file.
  - Its files are canonical JSON, like a document's.
- **R11. A document exports a library.**
  - The export holds all of the document's objects, the objects of one category and everything below it, or a named list of objects.
  - It always includes every object those objects extend, so the library stands on its own.
  - Exporting writes a new `.pixellib`, or replaces one only when told to.
  - In the app, it's File › Export Library…: a save panel with an Objects menu (All Objects, or one category).
  - Exporting doesn't change the document.
- **R12. A document imports a library by copying.**
  - Importing copies the library's objects into the document with their categories, which show in the Library popover.
  - Nothing links the document to the library afterwards.
  - A document can import any number of libraries, and the same library more than once.
  - An import is one edit: one undo step and one version ("Imported 12 objects from explainer.pixellib").
  - In the app, it's File › Import Library…, or dropping a `.pixellib` on the window.
- **R13. Name clashes keep both.**
  - When an imported object's name is already taken, the imported one gets the first free suffix: `pine-2`, `pine-3`, and so on.
  - Every reference inside the import follows the renamed object, so an imported object that extends `pine` extends `pine-2`.
  - The document's own objects are never renamed or changed.
  - An imported object identical to the document's same-named one, apart from its category, isn't copied: the import uses the document's (Areas of concern 3).
  - A library's `character` that differs from the document's isn't copied either: its figures extend the document's `character`, and the import warns about it as it does about colors (R14, Areas of concern 2).
  - Before an import that renames, the app lists the renames. The command reports them in its result.
- **R14. Colors that differ are a warning.**
  - The document's palette colors the imported objects, by color name.
  - When the library's record says one of those names had a different value from the document's (wood-3 was #7A4E2D, the document's is #6E4428), the import warns before doing anything.
  - The app shows each differing color's two swatches, says the imported objects will look different from the library, and offers **Import Anyway** and **Cancel**.
  - The command refuses with the list unless told to accept the differences.
- **R15. Missing colors are converted, never added silently.**
  - A library object that uses a color name the document doesn't have can't be imported as it is.
  - For each missing color, the app offers the document's colors of the same tier to convert it to, with the nearest one picked first.
  - The app says plainly that converting changes how the imported objects look.
  - The command refuses with the list (and each nearest color) unless given a conversion for every missing color.
  - A converted color is rewritten in the imported objects' keys only.
  - An outfit color converts only to an outfit color, and a world color only to a world color.
- **R16. Imports never change existing art.** No import changes the document's palette, its existing objects or its scenes. The rendered scenes and everything Publish writes are byte-identical before and after an import (R26 proves it on the site's document).
- **R17. Agents and the CLI edit a library's categories.**
  - `library.info` lists a library's objects, categories and recorded colors.
  - `library.category` sets or clears categories in a `.pixellib`, as `object.category` does in a document.
  - There's no other way to edit a library: no painting, adding or removing objects in place (the intent's out-of-scope editors).
- **R18. The app doesn't open a library as a document.** File › Open Document… and the Finder treat a `.pixellib` as a library. Opening one says that it's an object library, and offers to import it into the open document or cancel.

**Palette files**

- **R19. A `.pixelpal` is a palette on its own.**
  - It's one JSON file, with the same world and outfit tiers, names, values and color notes as a document's palette, plus a manifest line naming its format.
  - The tiers' caps (32 world colors, 16 outfit colors) apply to it too.
- **R20. A document exports its palette.**
  - Exporting writes the whole palette to a new `.pixelpal`, or replaces one only when told to.
  - In the app, it's File › Export Palette….
- **R21. A document imports a palette file by adding.**
  - Importing adds the colors the document doesn't have, by name, in their tier, at the end of the tier.
  - It never removes a color.
  - Colors with the same name and value are left alone.
  - It's one edit: one undo step and one version ("Imported 4 colors from coast.pixelpal").
  - In the app, it's File › Import Palette….
- **R22. Differing colors are the owner's choice.**
  - When a name the document has carries another value in the file, the import lists each one with both swatches.
  - Each is **Keep Mine**, the default, or **Use Theirs**. Using theirs says plainly that it changes how every object using that color looks.
  - A name that's in the other tier in the file is listed too, and stays as the document has it.
  - The command refuses with the list unless told, for every differing color, to keep or to use theirs.
- **R23. A palette import that won't fit is refused.**
  - When adding the new colors would pass a tier's cap, nothing is added.
  - The app and the command say how many colors would be over the cap. The import can be narrowed to a chosen list of colors.

**The owner's art**

- **R24. Recommended categories for the site's document and the explainer document.**
  - Appendix A gives a category for every object in the site's document (`jlawcordova.pixellab`).
  - Appendix B gives one for every object in the explainer document (`art/explainer-library.pixellab`).
  - The owner confirms or changes both in the final PR's review.
  - Setting them is a change in the site repo (Areas of concern 1).
  - This repo carries the mapping as data the acceptance pass uses (R26).
  - One `object.category` call per category sets a document's categories.
- **R25. The explainer document becomes a library.**
  - The explainer document stays where the explainer art is drawn and its contact sheets are kept. Libraries have no editor (the intent).
  - Exported, it makes `explainer.pixellib`, which any document can import.
  - Whether the site's repo keeps the exported library next to the document is the site repo's decision (Areas of concern 1).
- **R26. The acceptance pass proves it on copies.** On temp copies, never the site checkout:
  1. Categorize the site's document by Appendix A, with no problems, no object left in Others, and every published SVG byte-identical.
  2. Categorize the explainer document by Appendix B, with no problems and no object left in Others.
  3. Export the explainer document to a `.pixellib`.
  4. Import it into a new document and into the site's document copy. In the site's document copy, the import reports its renames (R13), the colors match, and the published SVGs stay byte-identical.

**Examples, doors and docs**

- **R27. The examples show it.**
  - `harbor.pixellab` and `town.pixellab` get categories.
  - `examples/` gains `coast.pixellib`, exported from harbor, and `coast.pixelpal`, harbor's palette.
  - `npm run examples` makes all four, and `npm test` checks they match.
- **R28. Every new command reaches all three doors.**
  - The tests are named after these requirements, and `docs/cli.md` is regenerated.
  - The MCP server's instructions mention categories, libraries and palette files.
  - An agent that hits a color warning asks the owner with `chat_ask` before accepting it.
- **R29. The script hook and the acceptance pass drive all of it,** on temp copies, never the owner's document or `examples/` (D9).
- **R30. The docs follow:**
  - README: the tour's Library, the File menu, and importing.
  - `docs/architecture.md`.
  - A docs-site page, "Libraries and palette files": what's copied, what warns, and how to export a document as a library.
  - `pixellab help`'s description of the two new file kinds.

## Design

### D1. Where a category lives (R1–R3)
- **The field:** an object's JSON gains an optional `category`, a list of names, outermost first:
  - `"category": ["Nature", "Trees"]`
  - It's written after `notes` in canonical key order.
  - The engine's `OBJECT_PROPS` and `BLOCK_PROPS` accept it, and `validate` checks R2.
- **Why a list, not "Nature/Trees":** a list needs no escaping and no separator rule, and diffs one name per line.
- **Kept out of output:** `compile`, `svg` and `preview` never read `category`. A test renders every example and the hero island before and after categorizing, and compares bytes (R3).
- **Not a format bump:** the document stays format 2, as scene layers did. Older engines reject the unknown property, so a categorized document needs this release (Areas of concern 1).

### D2. Category commands (R4, R9, R17)
- **`object.category`:**
  - `{ names: [..], category: [..] | null }`
  - It sets or clears one or more objects' category in one session edit, recorded as one version edit.
  - The CLI's form: `pixellab object category pine tree --to "Nature/Trees"`, or `--clear`.
- **`object.list`:**
  - Gains a `category` filter, which matches the path and everything below it, without regard to case.
  - Each row gains `category`.
  - `object.show` gains `category`.
- **`library.info`:** `{ library: <path> }` returns the library's name, objects (name, kind, extends, category), categories and recorded colors.
- **`library.category`:**
  - `{ library, names, category | null }`
  - It writes the `.pixellib` in place, canonical, under a file lock, the way `document.copy` writes a folder.
  - A library has no session, undo or version history: it's a file, edited by the CLI and agents only.
- **The library param:** `library` is a path. The MCP tool takes it as given, resolved against the server's working folder, like `document.new`'s `path`.

### D3. The `.pixellib` package (R10)
```
explainer.pixellib/
  library.json     { "format": 1, "kind": "library", "name": "explainer",
                     "exportedFrom": "explainer-library", "colors": { "world": {..}, "outfit": {..} } }
  objects/*.json   the objects, as in a document (with category)
```
- **`colors`:** the record from R14. Each tier holds every color name the library's objects use, with its value at export. It's never read as a palette.
- **`exportedFrom`:** the document's name, as a note.
- **What it leaves out:** no dates and no paths, so the same export gives the same bytes.
- **The app's bundle:** declares `com.jlawcordova.pixellab.library` (extension `pixellib`, conforms to `com.apple.package`) and `com.jlawcordova.pixellab.palette` (extension `pixelpal`, conforms to `public.json`). The app is the viewer for the library and the editor for neither. A double-click on either opens the app's import flow (R18).
- **`check`:** with `--library <path>`, it validates a library: objects valid against its own recorded colors, `extends` resolving within it, categories valid.

### D4. Export (R11, R20)
- **`library.export`:**
  - `{ to, objects?: [..], category?: [..], replace?: bool }`
  - It collects the named objects, or the category's, or all of them, then adds each `extends` chain.
  - It writes the package to a temp folder beside `to`, then renames it into place.
  - It refuses `exists` without `replace`.
  - Its result lists the objects, with any added as bases marked `base: true`.
- **`palette.export`:** `{ to, replace? }` writes the document's `palette.json` content with `"format": 1, "kind": "palette"` added.
- **In the app:** both sit under File › Export Library… and Export Palette…, after Publish to Folder…. They use NSSavePanel, defaulting to the document's folder and name.

### D5. Library import (R12–R16)
- **`library.import`:**
  - `{ from, acceptDifferences?: bool, convert?: { missing: existing }, dryRun?: bool }`
  - **Plan:** read the library and validate it (D3). Compute the renames (R13), the differing colors (R14) and the missing colors (R15).
  - **Refusals:**
    - With differences (differing colors, or a `character` swap) and no `acceptDifferences`, it refuses `differs`.
    - With missing colors not all in `convert`, it refuses `missing_colors`, with each one's tier and nearest same-tier color.
    - A conversion to a color of the other tier refuses `wrong_tier`.
    - `dryRun` returns the plan and changes nothing. The app uses it to fill its sheet.
  - **Apply:** add each object under its final name, rewrite `extends` through the rename map, rewrite converted keys (and a block's face colors), and keep `category`. It's one `session.record` with one label.
- **Nearest color:** the smallest squared RGB distance within the tier. Ties go to the palette's order.
- **Identical objects:** an imported object whose JSON equals the document's same-named object, apart from `category`, isn't copied. References to it point at the document's object, and the result lists it as `same` (Areas of concern 3).
- **`character`:** the engine treats the object named `character` as the figure base (only figures that extend it may use outfit colors). When the library has a `character` that differs from the document's, it isn't copied. The imported figures extend the document's `character`, and the import counts that as a difference: the app's sheet says "Figures will use this document's character, so they may look different", and the command needs `acceptDifferences` (Areas of concern 2). A library `character` imported into a document without one keeps its name.
- **In the app:** File › Import Library… (an NSOpenPanel for `.pixellib`) runs the dry run, then:
  - With no renames, differences or missing colors, it imports at once and announces "Imported 12 objects".
  - Otherwise a sheet shows the renames, then the differing colors (swatch pairs), then the missing colors, each with a color menu set to the nearest one.
  - The sheet's copy is plain: "These objects will look different from the library: wood-3 is darker here."
  - Its buttons are **Import** (**Import Anyway** when there are differences or conversions) and **Cancel**.
  - The import is one undo step, Edit › Undo Import.

### D6. Palette import (R21–R23)
- **`palette.import`:**
  - `{ from, keep?: [..], useTheirs?: [..], only?: [..], dryRun? }`
  - The plan has new colors per tier, differing colors and tier conflicts.
  - It refuses `palette_differs` unless every differing name is in `keep` or `useTheirs`.
  - It refuses `palette_full` when a tier would pass its cap, with the count over.
  - `only` narrows the colors added.
- **Apply:** append the new colors to each tier, and set `useTheirs` values, as one edit. `colorNotes` from the file are added for the new colors only.
- **In the app:** File › Import Palette… runs the dry run. A sheet lists the new colors as swatches, and each differing color with a Keep Mine / Use Theirs segmented control (default Keep Mine). When over the cap, each new color has a checkbox and Import stays disabled until the count fits. The import is one undo step.

### D7. The Library popover (R5–R8)
- **The model:** `ObjectSummary` gains `category: [String]?`. `AppModel` gains `libraryTab` (per document) and a derived `librarySections: [(heading: String?, objects: [ObjectSummary])]` for the chosen tab and query.
- **The tab row:** a horizontal `ScrollView` of text tabs, 24 pt tall, in `Theme.inkMuted`. The chosen tab is in `Theme.ink` with a 2 pt `Theme.accent` underline, the Field notebook's tab look in the right column. With the row showing, the grid's height shrinks by the row's 28 pt, so the popover keeps its height.
- **Headings:** headings are `SectionLabel`s inside the `LazyVGrid`, as full-width pinned section headers.
- **The cursor:** `libraryCursor` indexes the flattened list, so arrows cross headings (R8). ↑/↓ find the cell in the same column of the neighboring row within the flattened layout.
- **Reduce Motion:** the tab change and the underline don't animate.

### D8. The object inspector (R4)
- The Object section (`ObjectFactsSection`) gains a Category row below the name: a text field with a menu button listing existing categories (as paths with ›) and None.
- Return or leaving the field sets it through `object.category`. Esc reverts.
- An invalid path shows the R2 reason in `Theme.warning` under the field.
- VoiceOver label: "Category". Value: "Nature, Trees".

### D9. Test hook and acceptance (R26, R29)
- **New script actions:**
  - `category Nature/Trees` and `category none` (the open object)
  - `librarytab <name>|all|others`
  - `import library <path>` and `import palette <path>`
  - `importsheet convert <missing> <existing>`, `importsheet keep|theirs <name>`, `importsheet import|cancel`
  - `export library <path> [category]` and `export palette <path>`
- **New expects:** `expect tabs All,Nature,Others`, `expect libtab <v>`, `expect headings <a>,<b>`, `expect category <v>`, `expect importsheet renames|differs|missing <v>`, `expect libraryobjects <n>`.
- **`scripts/acceptance/libraries.txt`:** runs on temp copies of `examples/` (categories, tabs, export, import with a rename and a conversion, palette import with Keep Mine, undo).
- **`scripts/acceptance/site-categories.txt`:** R26 on a temp copy of the site document and a read-only `git archive` copy of the explainer document. The category mapping is `scripts/acceptance/categories/site.json` and `explainer.json`, generated from the appendices.
- **The pinned art:** the hero island's and every published SVG's bytes are compared before and after.

### D10. Menus (R11, R12, R18, R20, R21)
- File gains, after Publish to Folder…:
  - Import Library…
  - Import Palette…
  - a divider
  - Export Library…
  - Export Palette…
- None has a shortcut.
- Edit's Undo item names the import ("Undo Import").

## Areas of concern

1. **The site has to change, in its own repo.**
   - **The pin:** the site pins a `pixellab` release. Engines before this change refuse `category` as an unknown property. So the site must bump its pin to this release before either of its documents gets categories (as with scene layers).
   - **Setting the categories:** applying Appendices A and B is a site-repo change under its own intent, after the bump. The site's `explainer-library.test.mjs` compares objects only by their art, so categories shouldn't break it. The site's change confirms that.
   - **Where the library lives:** whether the site keeps an exported `art/explainer.pixellib` beside the document, or exports it on demand, is the site repo's decision.
   - **Settled:** this change makes no edit to the site repo. The site's change (bump the pin, apply the appendices, decide where the explainer library lives) follows this release under the site's own intent.
2. **`character` is special by name** (D5).
   - Two documents' `character` objects already differ: the explainer's and the site's differ at line 58.
   - Importing the explainer library into the site's document today would rename the explainer's base to `character-2`. Its chef, waiter, staff and robots would then fail the outfit rule.
   - The spec skips identical copies, which the intent didn't ask for. That doesn't cover this case.
   - **The owner decides one of:**
     - **(a)** Figures stay valid when they extend any object that itself extends or is named `character`, including a renamed `character-2`. This changes the engine rule to "a figure base is an object named `character`, or imported from one". It needs a marker in the object, such as `"base": "character"`.
     - **(b)** When the import would rename `character`, the imported figures extend the document's `character` instead, with a warning that they may look different.
     - **(c)** Such an import is refused until the two match.
   - **Settled: (b).** It follows the intent's "warn, never silently change", and adds no engine concept (D5).
3. **Skipping identical objects departs from "both are kept"** (the intent's name-clash rule).
   - Skipping a byte-identical copy (apart from category) avoids `pine-2` duplicates when the same library is imported twice, or when documents share bases.
   - **Settled:** identical objects are skipped (D5).
4. **Palette caps are tight.**
   - The site's world tier is full: 32 of 32.
   - So any palette import that brings a new world color is refused (R23), and any library using a world color the site lacks must be converted (R15).
   - This follows the intent and the engine's caps. Raising the caps is a site and engine art-rules question, out of scope here.
   - **Settled:** no change; R15 and R23 handle it.
5. **The extension `.pixelpal`** is Claude's choice, a short sibling of `.pixellib`. The owner may prefer another name, such as `.pixelpalette`.
   - **Settled:** `.pixelpal`. Renaming later is a constant and the bundle's type declaration.
6. **Recommended categories** (Appendices A and B) are Claude's proposal from the objects' names, the explainer's contact sheets and the site's scenes. The owner approves or edits them here.
   - **Settled:** as written, to be confirmed in the final PR's review.

## Open questions

- The intent had none.
- New questions were Areas of concern 1–3, 5 and 6, each settled with Claude's recommendation for the owner to confirm.
- Carried forward for a later intent: should a library import offer to add a missing color to the palette, when there's room, instead of only converting it? The intent's answer today is "convert"; this spec doesn't add it.

## Appendix A: the site's document (`jlawcordova.pixellab`)

Eight outermost categories. Within each, a heading per subcategory.

| Category | Objects |
|---|---|
| Terrain | block, block-left, block-right, tile, path, pebble, island-shadow |
| Terrain › Footer | footer-dirt, footer-grass |
| Terrain › Water | water, river, block-river, waterfall-face |
| Nature › Trees | pine, tree, tree-small, tree-shade |
| Nature › Flowers | flower, flower-gold, flower-pink |
| Nature › Sky | cloud-a, cloud-b, cloud-c |
| Buildings | house, fence, flag |
| Buildings › Lab | block-lab, block-lab-left, block-lab-right |
| Buildings › Mine | block-mine, block-mine-left, block-mine-right |
| Buildings › Office | block-office, block-office-corner, block-office-left, block-office-right |
| Buildings › Rampart | block-rampart, block-rampart-corner, block-rampart-left, block-rampart-right |
| Buildings › Server | block-server, block-server-corner, block-server-left, block-server-right |
| Buildings › Site | block-site, block-site-corner, block-site-left, block-site-right |
| Buildings › Studio | block-studio, block-studio-corner, block-studio-left, block-studio-right |
| Buildings › Tower | block-tower, block-tower-corner, block-tower-left, block-tower-right |
| Transport › Road | block-road, tunnel-road, bridge, level-crossing, crossing-gate, crossing-gate-down, crossing-gate-down-far, crossing-signal, crossing-signal-left, crossing-signal-right |
| Transport › Trucks | truck, truck-green, truck-open, truck-open-green, truck-open-tipped, truck-open-tipped-green |
| Transport › Rail | track, block-rail, bridge-rail, tunnel-rail, tunnel-rail-front, locomotive, boxcar, flatcar |
| Industry › Mine | track-mine, mine-cart, mine-cart-tipped, ore, stone, ore-screen, hopper, processor |
| Industry › Conveyor | conveyor, conveyor-corner, conveyor-feed, conveyor-low, conveyor-ramp, conveyor-row |
| Industry › Crane | crane-mast, crane-foundation, crane-jib, crane-jib-back, crane-jib-side, crane-hook, crane-hook-drop, crane-hook-low, crane-box |
| Industry › Cargo | container, boxes, parcel |
| Roles | character |
| Roles › Back end | outfit-back-end, prop-back-end-mug, prop-back-end-network, prop-back-end-rack |
| Roles › Cloud & DevOps | outfit-cloud-devops, prop-cloud-devops-cone, prop-cloud-devops-pipes, prop-cloud-devops-scaffold |
| Roles › Data engineering | outfit-data-engineering, prop-data-engineering-bench, prop-data-engineering-book, prop-data-engineering-cauldron, prop-data-engineering-shelf |
| Roles › Front end | outfit-front-end, prop-front-end-candles, prop-front-end-crystal-ball, prop-front-end-lectern, prop-front-end-telescope |
| Roles › Project management | outfit-project-management, prop-project-management-board, prop-project-management-chair, prop-project-management-desk, prop-project-management-plant |
| Roles › Security & governance | outfit-security-governance, prop-security-governance-banner, prop-security-governance-brazier, prop-security-governance-parapet-left, prop-security-governance-parapet-right |
| Roles › UX design | outfit-ux-design, prop-ux-design-cabinet, prop-ux-design-cans, prop-ux-design-easel, prop-ux-design-stool |
| Achievements › Frames | frame-plain, frame-fix, frame-make, frame-milestone, frame-people, frame-think |
| Achievements › Icons | icon-apple, icon-book, icon-bug, icon-chest, icon-flask, icon-hammer, icon-heart, icon-key, icon-lock, icon-magnifier, icon-rocket, icon-shield, icon-signpost, icon-speech, icon-sprout, icon-star, icon-trophy, icon-wrench |
| Thumbnails › 2023 in review | thumb-2023-in-review-zero, thumb-2023-in-review-two, thumb-2023-in-review-three |
| Thumbnails › Caching | thumb-caching-client, thumb-caching-database, thumb-caching-road, thumb-caching-store |
| Thumbnails › Cloud resume | thumb-cloud-resume-cloud, thumb-cloud-resume-cloud-left, thumb-cloud-resume-cloud-right, thumb-cloud-resume-laptop, thumb-cloud-resume-phone |
| Thumbnails › Coverlet | thumb-coverlet-report |
| Thumbnails › Docker | thumb-docker-cargo, thumb-docker-ship-bow, thumb-docker-ship-stern |
| Thumbnails › ELK | thumb-elk-board, thumb-elk-cable, thumb-elk-server |
| Thumbnails › GitHub Actions | thumb-github-actions-belt, thumb-github-actions-crate, thumb-github-actions-station |
| Thumbnails › GraphQL | thumb-graphql-cable, thumb-graphql-graph, thumb-graphql-screen |
| Thumbnails › JWT | thumb-jwt-header, thumb-jwt-key, thumb-jwt-payload, thumb-jwt-signature, thumb-jwt-stud |
| Thumbnails › NSwag | thumb-nswag-bar, thumb-nswag-open |
| Thumbnails › Prism | thumb-prism-dark, thumb-prism-red |
| Thumbnails › REST | thumb-rest-client, thumb-rest-road, thumb-rest-server |
| Thumbnails › UML | thumb-uml-board, thumb-uml-hat, thumb-uml-heart |

## Appendix B: the explainer document (`art/explainer-library.pixellab`)

Seven outermost categories: five follow its contact sheets (kitchen, office, cloud, AI and "glue"), plus People and Animals for what several sheets share.

| Category | Objects |
|---|---|
| Kitchen | block-kitchen, tile-kitchen, stove, sink, fridge, kitchen-pass, ticket-rail, recipe-book |
| Kitchen › Tables | table, table-long, plate, plate-dish, plate-stack |
| Office | block-office, tile-office, whiteboard, water-dispenser, bookshelf, filing-cabinet, bulb, bulb-off |
| Office › Desks | desk, desk-white-oak, desk-l, desk-l-white-oak, chair, chair-office, chair-turned, cubicle-wall, cubicle-wall-turned, cubicle-corner |
| Office › Devices | computer, laptop, monitor-wall |
| Office › Plants | plant-bush, plant-cactus, plant-tall |
| Office › Charts | chart-bar, chart-line-up, chart-line-down, chart-pie |
| Cloud | cloud-platform, cloud-platform-shadow, globe, firewall, padlock, vault, data-packet |
| Cloud › Servers | server-rack, server-tower, router, database, database-stack |
| AI | robot, robot-small, token, token-gold, token-water, toolbox, sandbox, scale, plug-hub |
| Glue › Arrows | arrow-up, arrow-ne, arrow-nw, arrow-se, arrow-sw |
| Glue › Bubbles | bubble-speech, bubble-thought, bubble-typing, bubble-alert, bubble-check, bubble-question |
| Glue › Markers | marker-1, marker-2, marker-3, marker-4, marker-5, placard, signpost |
| Glue › Wires | wire, wire-turned, wire-junction, wire-plug, wire-corner-n, wire-corner-e, wire-corner-s, wire-corner-w |
| People | character, chef, waiter, staff-analyst, staff-developer, staff-manager |
| Animals › Cats | cat, cat-black, cat-calico, cat-gray, cat-tabby, cat-tuxedo, cat-white |
