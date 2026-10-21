# Spec: A scene without a size, and keyboard block placement (from intent.md 2026-10-21)
Status: approved.

## Requirements

Traced to the intent's Problem and Proposed outcome: R1–R13 for the scene's size (a size guessed up front, art cut off where it doesn't fit; no size while editing, decided at publish, no cut edges) and R14–R19 for block placement from the keyboard.

- **R1.** A new scene has no size. `scene new` writes no `viewBox`, over the CLI, the app and MCP.
- **R2.** A scene without a `viewBox` publishes at the smallest whole-pixel box that holds everything it paints, plus its margin (R5). Nothing a scene paints is cut off.
- **R3.** "Everything it paints" covers the whole scene over its loop: the items at rest, every place a moving item takes (scene motion), and how far actions and ambient loops move or grow their layers. The published SVG never draws outside its box at any time.
- **R4.** A scene that does carry a `viewBox` keeps it: it publishes at exactly that box, and every scene already in a document publishes byte-identical SVGs and images to today.
- **R5.** The space left around the painted pixels is the scene's `margin`, in whole pixels, default 0.
- **R6.** A scene with nothing in it has a defined size (one tile around the origin), so it still compiles, renders and previews.
- **R7.** Every place that uses a scene's size uses the resolved one: `art build` and `art preview`, a scene's `image`, `scene.render` and the app's renders, `check`, and the size shown by `scene show` and `scene list`.
- **R8.** The owner can set a fixed size on purpose, and take it off again so the scene fits its content again. The same is possible for agents over MCP, as a proposal like every other edit.
- **R9.** In the app, the stage has no edge. The owner can place, select and move things anywhere, scroll to anywhere, and nothing is clipped while editing. Placing works in empty space, not only on the art already drawn.
- **R10.** The app shows what would publish: an outline of the resolved box on the stage, which can be hidden, and its size in pixels in the inspector. It reads as "Fits the content" or "Fixed" accordingly.
- **R11.** `check` warns when a fixed `viewBox` cuts off painted pixels, naming the scene and how many pixels are outside, so the cuts in the Problem can be found in scenes that keep a fixed box.
- **R12.** A scene without a `viewBox` is valid in format 2 and stays format 2. A scene file with a `viewBox` is unchanged.
- **R13.** Keyboard use, VoiceOver labels, visible focus and Reduce Motion behave as they do for the rest of the stage; colors come from the board 03 tokens.
- **R14.** The keys that work today keep their meaning: arrows move the selection a column or row, ⇧↑ / ⇧↓ and Page Up / Page Down a level, ⌥ + arrows a pixel, Return places at the cursor, Delete removes.
- **R15.** The owner can select an object or block from the keyboard: Tab and ⇧Tab step through the items of the scene (or of the layer being worked inside), so a scene can be edited without the mouse.
- **R16.** The owner can drop the selection onto whatever it stands over: it moves down to the level where it rests on the top of the block under it, or on the ground. One key does it, as one undo step.
- **R17.** Placing a new item on a tile that already holds a block puts it on top of that block, from the keyboard (Return) and the mouse, so building a stack needs no level change. A level set on purpose still wins.
- **R18.** The selection or cursor stays in view as it is moved, including past the edge of what is drawn (R9), and its position (column, row, level) is announced to VoiceOver as it is today.
- **R19.** Agents have the same two actions over MCP: placing on top, and dropping; and every key above is a menu item with its shortcut shown, so the keys can be found.

## Design

### The size, resolved in one place (R2, R3, R5, R6, R7)
`crates/engine` gets one function that answers "what box does this scene publish at": the scene's own `viewBox` when it has one, else the fit. Everything that reads `scene.get("viewBox")` today (`render_scene` in `engine.rs`, `svg::to_rect_svg`, `preview::render_preview`, `compile`'s images, `render.rs`'s `view_box_of`, `check`, `scene show`, `scene list`) goes through it, so the box can't be worked out two ways.

The fit is computed from the pixels the scene paints, not from item boxes. `render_scene` and `composite` already give the exact painted pixels of the tree, and a scene's moving items are already drawn once for each place they take (`with_copies`), so the tree holds R3's scene motion. Ambient loops and actions move layers by CSS in the published file, so their extents are added the way `object render --time` already grows its box to hold moved layers (`ambient::moves_of`). The result is floored and ceiled to whole pixels, then the margin is added on every side. It is deterministic and needs no clock or sampling, so the same document always compiles to the same SVG.

An empty scene resolves to `[-16, -8, 32, 16]` (one tile around the origin) and `check` notes that it is empty (R6).

`origin`, the pixel of tile `[0, 0, 0]`, is unchanged. Only the box moves with the content, so a scene's art never moves inside its own SVG.

### Format (R1, R12)
`viewBox` becomes optional in the scene's validation (`validate_scene` in `engine.rs`: today it adds "viewBox must be four whole numbers" when it is missing). When present it is validated as now. A new optional `margin` (a whole number, 0 or more) is added to the scene's keys, in the canonical order next to `viewBox`. `scene new` and the starter scene (`engine::starter`) write no `viewBox`. `docs/document-format.md` and `pixellab help` describe both. `document migrate` carries `viewBox` over as it does now.

### Commands (R7, R8)
The registry (`crates/pixellab/src/commands/`, `registry.rs`) gets one new command, `scene.frame`, so the CLI, `serve --stdio` and `serve --mcp` all have it:

- `scene frame <name> --box x,y,w,h` fixes the size.
- `scene frame <name> --fit` removes the `viewBox`.
- `scene frame <name> --margin <n>` sets the margin.
- With none of them it reports the scene's frame: fixed or fit, its box and its size.

Over MCP it is `scene_frame` and, like every edit, becomes a proposal. `docs/cli.md` is regenerated with `npm run docs:cli`.

Existing commands change as follows:

- `scene.place` with no position starts at the tile at the middle of the view (`scene_edit.rs`). With no `viewBox` it starts at the middle of the scene's current content, or `[0, 0, 0]` when it is empty.
- `scene.render` and `scene.show` return the resolved `viewBox`, plus `frame: "fit" | "fixed"`.
- `scene.render` takes an optional `view: [x, y, w, h]` to render a window of the scene instead of its box. The app uses it (below); agents can use it to look at a part of a big scene. Without it nothing changes.
- `scene.list` and `scene.show` print the size as "fits 272×200" or "fixed 272×200".

### Checks (R11)
`check` adds a warning per scene whose fixed `viewBox` leaves painted pixels outside it ("hero-island: 14 pixels outside its viewBox"). It is a warning and does not fail `art build`, so a document that has always cropped on purpose still builds.

### The app (R9, R10, R13)
- **Stage.** `StageGeometry` (`Views/StageGeometry.swift`) today centers an art rectangle of `viewBox` size inside the scroll content. For a scene without a fixed box it uses a workspace instead: the content's bounds plus a margin of empty space on every side (at least a viewport's worth), so there is always room to scroll to and place in. The workspace grows when something is placed or moved toward its edge and does not shrink under the owner's pointer while they drag. Tile and pixel math already works from `origin` and does not depend on the box, so hit-testing and placing in empty space need no art under the pointer.
- **Rendering.** The model asks `scene.render` for the visible window with `view`, so the engine never renders an unbounded image. The window is cached with the existing timeline and thumbnail stores.
- **Fit and zoom.** Fit zooms to the content (the resolved box), not the workspace. 1×–4× are unchanged, still nearest-neighbor at integer zoom.
- **Published frame.** The resolved box is drawn on the stage as a one-point outline in the theme's accent token, over the art, never part of it. View › Show Published Frame toggles it (the choice is remembered, not saved in the document). The inspector's scene section shows "Publishes at 272×200 · Fits the content" and, for a fixed scene, "Fixed" with a **Fit to content** button and a field for the box. The outline, the line and the button have VoiceOver labels; Reduce Motion has nothing to change because nothing animates.
- **Proposals.** A proposal's ghost (`ProposalGhost.swift`, `AgentModel.swift`) renders the base and the proposed scene in the same window (the union of the two resolved boxes), so a proposal that grows the scene is not clipped in the preview.
- **Script hook.** `ScriptRunner.swift` gains `frame fit`, `frame box x,y,w,h`, `frame margin n`, `frame outline on|off`, and `expect frame fit|fixed`, `expect size w,h`, `expect outline on|off`.

### Keyboard block placement (R14–R19)
**What exists.** In scene mode `SceneEditor.handleKey` already maps the arrows to column and row, ⇧↑ / ⇧↓ and Page Up / Page Down to a level, ⌥ + arrows to a pixel nudge, Return to place at the cursor and Delete to remove, and announces the cursor. R14 only pins them with tests.

**Recommended keys** (the intent leaves the choice to this spec; each is a menu item under a new Edit › Move submenu, which lists the existing keys too, R19):

| Key | Does |
|---|---|
| Tab / ⇧Tab | Select the next / previous item, in the scene's depth order (R15) |
| ⌘↓ | Drop the selection onto what is below it (R16) |
| Return (Place tool) | Place the object on top of the block at the cursor (R17) |

⌘↓ is free on the stage: the stage is not a text field, and ⌘↑ / ⌘↓ have no meaning there today. Tab is free while the stage has focus. A lift counterpart (⌘↑) is left out on purpose: "up to what?" has no single answer, and ⇧↑ already raises a level at a time.

**Dropping and stacking.** One function in `scene_ops.rs` answers "what is the top at this column and row": the highest `level + height` of any item whose block size or footprint covers that tile (`depth::block_box` / `sprite_box` already know both), in whole levels, 0 for bare ground. Drop sets the item's level to the top under its own footprint, ignoring itself. Place with no level given sets the new item's level to the top at its tile. A `level` given explicitly, including by the inspector or the stepper, is used as given.

**Commands.** `scene.place` stacks by default when no level is given (R17); it also takes `--on-top` so an agent can say it explicitly, and a new `scene.drop <name> <path>` drops an item, so both are `scene_place` and `scene_drop` over MCP, proposals like every edit. Each is one undo step (R16, R17).

**Staying in view.** With R9 the stage can scroll anywhere, so after a move by key the stage scrolls the cursor into view, at the zoom the owner has, with the same margin as a click on the edge. VoiceOver keeps the announcement `Cursor at column 3, row 1, level 2`, and says "on top of pine" or "on the ground" after a drop or a stack.

### Tests (named after the requirement)
Rust: `R2: a scene without a viewBox publishes at its painted bounds`, `R3: a moving item's every place is inside the box`, `R3: an ambient loop's reach is inside the box`, `R4: a scene with a viewBox publishes the same bytes` (the hero island, harbor and town in `examples/`), `R5`, `R6`, `R7` (the image and the preview use the resolved box), `R8` (`scene.frame` both ways and as a proposal), `R11`, `R12` (validation, canonical order, migrate). Rust: `R16: drop lands on the top of the block under it`, `R17: placing on a block puts the item on top`, `R17: a level given wins`. Swift: workspace growth and hit-testing in empty space (R9); `R14` the existing keys, `R15` Tab order, `R16` ⌘↓, `R17` Return on a block, `R18` scroll into view; menu items present (R19). The script hook gains `drop`, `key tab` and `expect level`/`expect at` lines for these. The acceptance pass gets a scene made with `scene new`, filled past where the old default would have cut it, saved and published, and the hero island's SVG stays byte-identical.

## Areas of concern

1. **The site must take a new release first (external dependency).** The site pins a `pixellab` release; a release before this change rejects a scene with no `viewBox`. Existing scenes keep theirs (R4), so nothing breaks until a scene is made without one. The site should bump its pin before the owner's document gets such a scene, as it did for scene layers. No change to the site's art or engine is needed; the work is a pin bump in the site repo under its own workflow.
2. **A fit box moves with the content.** A page that places a scene's SVG against other content sees its size change when the owner adds something far from the rest. That is the point of the intent, but it can shift a page. Fixing the size (R8) keeps a scene still. The owner decides which of the owner's scenes should stay fixed.
3. **Tight by default can clip the page's effects. (Owner agreed to 0.)** With a margin of 0 the box is exactly the pixels. The site draws the hero island's drop shadow and dark-theme shadow from CSS outside the SVG, and an SVG clips its own content but not a CSS `filter` on the element; whether the page shows the shadow in full depends on the page's `overflow`, which this repo can't see. The owner should check the island on the site when it adopts a fit scene, or set a `margin`. The owner agreed to a margin of 0 by default; the site check above remains the owner's.
4. **"Infinite" is bounded in practice.** The program and the file hold whole-number pixels, and the engine's 64×64 cap applies to objects, not scenes. The workspace grows with content and the app renders a window, so large scenes don't cost more than the visible part, but a published scene's SVG grows with its content and its `budgets` (per-output size limits in the manifest) still apply. A scene that outgrows its budget fails `art build` as before; the owner may want a note about it when the fit grows a scene past one.
5. **`check` may warn on existing art.** R11 could report pixels outside the viewBox of an existing scene that crops on purpose. It is a warning only; the acceptance pass pins the site's art as it was on 2026-10-21 and should not change. If any scene warns, the owner decides whether to fit it or leave the crop.
6. **Rendering cost of the app's stage.** The stage renders a window, not the scene; a scene with motion and a large window re-renders per scrubbed frame. The timeline's cache is keyed by time only today and needs the window too. The plan should measure this on the town example before the stage is built on it.

## Open questions

Carried from the intent:

1. **Can the owner still fix a size on purpose?** Yes (R4, R8): a `viewBox` stays valid and wins, `scene frame --box` sets it, `--fit` removes it.
2. **The whole animation or the scene at rest?** The whole loop (R3), so nothing moving ever leaves the frame.
3. **Does the owner see the would-be size while editing?** Yes (R10): an outline on the stage and the size in the inspector.
4. **How much space around the art?** 0 by default with a per-scene `margin` (R5). *Agreed by the owner.*

New:

5. **Should hidden layers count?** No change needed: hiding is app-only and never saved or published, so a hidden layer still publishes and counts. The stage's workspace therefore includes hidden layers.
6. **Name of the command.** `scene.frame`, agreed by the owner.
7. **Which keys change the level and stack?** Recommended in the design: the existing ⇧↑ / ⇧↓ and Page Up / Page Down, plus ⌘↓ to drop, Tab / ⇧Tab to select, and Return to stack. *Agreed by the owner as recommended.*
8. **Arrow direction.** The arrows move along the grid (← and → change the column, ↑ and ↓ the row), which runs diagonally on the screen. The spec keeps that, as R14 says, because the owner described it as working. Say so if the arrows should follow the screen instead.
