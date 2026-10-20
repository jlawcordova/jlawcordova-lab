# Spec: Scene layers (from intent.md 2026-10-20)
Status: approved (the owner agreed on the design through the prototype and asked for the spec, plan and code in one run, reviewed in the final PR).

The prototype named in the intent is the design for everything the owner sees. This spec settles what it leaves open (storage, commands, the edge cases), and the places where the app departs from it, each said where it happens.

## Requirements

**The data**

- **R1. Named layers that nest.** A scene can have layers. Each has an id, a name and optionally a parent layer. Any placed item can belong to one layer, so a scene's parts (the Mine, the Railway, the Train inside it) can be named and treated as one. A scene with no layers is exactly what it is today.
- **R2. Layers never change paint order or output.** Making, renaming, ungrouping and assigning layers never reorders the scene's items or moves them. Rendering and Publish ignore layers. `art build` for a document with layers writes byte-identical files to the same document without them, and a test proves it.
- **R3. Layers are separate from the SVG group wrappers.** The site's `{ group, items }` wrappers stay exactly as they are. `scene group set` keeps working. The Layers tree leaves the wrappers out. The app still shows a selected item's wrapper and its attributes, read-only, in the inspector. Moving a layer keeps each item's wrapper in step, as moving one item does.
- **R4. Existing documents have no layers.** Their items show at the top level of the Layers tree, in paint order. Nothing about layers is written to a scene until a layer is made in it. Tests and scripts never write to the owner's document in the site repo.

**Commands (the three doors)**

- **R5. Every layer action is a registry command:** make a layer from items and layers (`scene.layer.new`), rename (`scene.layer.rename`), ungroup (`scene.layer.ungroup`), move items and layers into a layer (`scene.layer.assign`), place into a layer (`scene.place --layer`), and move layers and items by tiles (`scene.layer.move`). `scene.items` lists the layers and each item's layer. `scene.render` can leave hidden layers out and render a layer on its own for the fade. The CLI, the app and MCP agents all have them; an MCP agent's layer edits become a proposal like any other edit. `docs/cli.md` is regenerated.

**The app**

- **R6. The sidebar.** In scene mode, the tabs under the scenes are **Layers** (first, and selected by default) and **Library**. Layers replaces the Items tab. The LAYERS heading, the SCENES heading, layer rows and scene rows show no counts.
- **R7. The Layers panel.** A tree of layers (dashed-square icon, name in Sora) with their items (cube icon, tile position on the right). Each layer row has a chevron, an eye to hide it and a lock that shows on hover, on selection or while locked. Clicking a row selects it, ⇧-click adds to the selection, double-clicking a layer goes inside it. It is the keyboard way to select (R20).
- **R8. Picking on the stage.** A click picks the outermost layer under the pointer, or the item when it's in no layer, hit-tested by the item's pixels, not its bounds. ⇧-click adds to (or takes from) the selection. A click on nothing clears the selection.
- **R9. Going inside a layer.** Double-clicking a layer (on the stage or in the tree), Go Inside, or Return in the tree goes inside it. Everything outside it fades. The breadcrumb pill at the stage's top left (`hero-island › Railway › Train`) gains a Done button. Clicks then pick the next level down. Done, a crumb, or Esc with nothing selected leaves.
- **R10. Moving.** Dragging moves every item in the selection, snapping to whole tiles, with a label showing the col and row change. The arrow keys and the inspector's nudges move the selection a tile at a time. Each move is one undo step. A single selected item moves exactly as it does today (⌥ by pixels, and past time 0 its key).
- **R11. Hide and lock.** A hidden layer doesn't draw and can't be picked. A locked layer can't be picked on the stage, except from inside it. Hidden and locked are view state: the app keeps them for each document and scene, across launches, and never writes them to the document or publishes them.
- **R12. The right-click menu.** Right-clicking the stage or a Layers row opens a menu: Group ⌘G and Ungroup ⇧⌘G, a separator, then Go Inside, Hide and Lock. Right-clicking something outside the selection selects it first; right-clicking something already selected keeps the selection. There are no toolbar buttons for grouping; ⌘G and ⇧⌘G are also in the Edit menu.
- **R13. Group and Ungroup.** Group makes a new layer under the selection's common parent, named "Group 1", "Group 2" and so on, and selects it; it's renamed in the inspector. Ungroup moves a layer's contents up one level, where they stay, and selects them.
- **R14. The inspector for a layer:** a name field, "n items · in Railway", and Go inside, Hide and Lock buttons; then "Moved by" with col and row fields and four diagonal nudge buttons, and a Place into it button.
- **R15. The inspector for an item and for several things.** An item shows its position (as today) and, when it's in a layer, a "Select <layer>" button. Several things selected show a count and a pointer to right-click › Group.
- **R16. Placing into a layer.** The Library shows a "New items go into <layer>" chip. Placing goes into the selected layer, or the selected item's layer, or the layer you're inside; with none of those, the item goes in at the top level.
- **R17. The stage has no hint bar.**

**Proof and standards**

- **R18. The script hook and the acceptance pass.** The script hook has actions and `expect` lines for all of it (listed in `ScriptRunner.swift` and in `CLAUDE.md`), and `scripts/acceptance/` covers layers end to end, including that a published SVG is byte-identical before and after layers are made.
- **R19. Docs.** The README's tour and shortcuts (⌘G, ⇧⌘G), the docs that mention the Items tab, `CLAUDE.md`'s hook list, the MCP server's instructions and the generated `docs/cli.md` describe layers.
- **R20. Accessibility and the app's rules.** VoiceOver labels on every row, button, menu item and the stage; visible focus on the tree, the menu and the fields; a full keyboard path through the Layers tree; Reduce Motion turns off the fade's animation and the chevrons'. Colors from the board 03 tokens, no `@State`, no new npm or Swift dependencies.

## Design

### D1. Storage (R1, R4)

The owner's lean, in the scene file:

```js
export default {
  output: 'hero-island.svg',
  viewBox: [-182, -92, 446, 264],
  origin: [0, 8],
  layers: [
    { id: 'railway', name: 'Railway' },
    { id: 'train', name: 'Train', parent: 'railway' },
  ],
  items: [
    {
      group: { class: 'enter', 'data-enter': 'object' },
      items: [
        { object: 'mine-cart', id: 'cart', layer: 'train', at: { tile: [3, 4, 1] } },
      ],
    },
  ],
};
```

- `layers` is an optional list of `{ id, name, parent? }`. Its order is the tree's order among siblings. A placed item's optional `layer` names one of them. An item without `layer` is at the top level.
- The engine (`cli/lib/engine/engine.mjs`) validates them: `layers` must be a list; each entry has only `id`, `name` and `parent`; `id` is lowercase kebab-case and unique among the scene's layers (layer ids and item ids are separate names); `name` is 1 to 64 characters on one line, without leading or trailing spaces; `parent` is another layer's id, and no layer is inside itself; an item's `layer` names a layer of the scene. Rendering ignores both.
- The serializer writes `layers` between `origin` and `items`, one layer per line, so line merges and diffs stay small (`{ id: 'train', name: 'Train', parent: 'railway' }`). An item's keys go `object, id, class, layer, at`. `scene-ops.mjs`'s key reordering (class and id edits) keeps `layer`.
- When the last layer of a scene is ungrouped, `layers` is removed, so the file is exactly what it was before.
- A layer's id is made from its first name (`Group 1` → `group-1`, `Railway` → `railway`; `-2`, `-3` when taken; `layer-…` when the name doesn't start with a letter) and doesn't change on rename, so a rename touches one line. Commands take a layer by id, or by its exact name when only one layer has it.
- Why not a sidecar file or ids per wrapper: the item's own `layer` moves with it through every existing edit (raise, lower, duplicate, remove, undo, proposals, three-way merges) with no bookkeeping, where paths into the items would break on every insert.

### D2. Paint order and output (R2)

- No layer command reorders `items`. `scene.layer.move` changes positions only; `scene.place --layer` inserts where `scene.place` would.
- The engine's `renderScene`, the compiler and the previews ignore `layers` and `layer`. `cli/test/layers.test.mjs` builds a document whose scene has nested layers, then the same with `layers` and every `layer` stripped, and compares every published file's bytes, and the PNG of `scene.render`.
- `compile.test.mjs`'s byte-identity against the site's committed SVGs passes unchanged.

### D3. Wrappers (R3)

- A wrapper's items can sit in different layers; a layer can hold items from many wrappers. Nothing about layers reads or writes wrapper attributes.
- `scene.layer.move` runs `followGroup` (D7 of `../2026-10-mcp-gaps/spec.md`) for each item it moves, so a wrapper's `data-x`/`data-y` and `data-col`/`data-row` follow as with `scene.move`; `keepGroup: false` turns it off.
- `groups.test.mjs` passes unchanged, and a test runs `scene group set` on a scene with layers.
- In the app the Layers tree leaves wrappers out. The item inspector keeps a read-only line for the item's wrapper, "SVG group 42 · .enter · enter=object" (its path, class and `data-*` other than the coordinates), so their attributes are still visible somewhere; they're edited with `scene group set`. They're called SVG groups in the app, since "group" now names the menu action that makes a layer.

### D4. Commands (R5)

New file `cli/lib/commands/scene-layers.mjs`, logic in `cli/lib/layer-ops.mjs`.

| command | params | does |
| --- | --- | --- |
| `scene.layer.new` | `name`, `layerName?`, `id?`, `items?`, `layers?`, `parent?` | Makes a layer and moves the given items (paths or ids) and layers (ids or names) into it. Without `layerName` it's "Group n", one more than the highest n among the scene's "Group n" names and `group-n` ids (so renaming Group 1 doesn't bring back a second Group 1). Its parent is `parent` (`''` for the top level), else the deepest layer holding every member, else the top level. A member already inside another member stays where it is. It goes into `layers` before the first member layer, else before the first layer with the same parent, else last. Returns `layer: { id, name, parent? }`, `items` (how many joined) and `layers` (the ids that joined). |
| `scene.layer.rename` | `name`, `layer`, `to` | Renames it (D1's name rules). |
| `scene.layer.ungroup` | `name`, `layer` | Removes the layer. Its layers and items move to its parent; nothing moves on the stage. Returns `parent`, and the `layers` and `items` (paths) it held. |
| `scene.layer.assign` | `name`, `to`, `items?`, `layers?` | Moves items and layers into `to` (`''` for the top level). Refuses to put a layer inside itself or a layer inside it (`layer_cycle`). |
| `scene.layer.move` | `name`, `layer?`, `layers?`, `items?`, `by`, `keepGroup?` | Moves every item in the layers (and the layers inside them) and the items by `col,row[,level]` tiles, each once, in one undo step; rapid repeats coalesce like `scene.move`. Placements move, at rest: an animated item's keys are offsets from its placement, so its motion moves with it. Wrappers follow (D3). Returns `moved` (how many items) and `by`. |
| `scene.place` | adds `layer` | The new item joins that layer. Works with `group`, `newGroup` and `index` as before. |
| `scene.items` | — | Each item adds `layer` when it has one; the result adds `layers: [{ id, name, parent?, items }]`, `items` counting everything inside it. The CLI's table adds `^<layer id>`. |
| `scene.show` | — | Adds `layers` (the count) and a line naming them. |
| `scene.render` | adds `hide`, `focus` | `hide` (layer ids or names): items in those layers, or inside them, aren't drawn and aren't in `items`. `focus` (a layer): the result adds `focusPng`, the same render with only that layer's items (and those inside it, less hidden ones), which the app draws over the faded render. |

```
pixellab scene layer new hero-island --layer-name Railway --items 140.0,141.0,142.0
pixellab scene layer new hero-island --layer-name Train --items cart --parent railway
pixellab scene layer rename hero-island group-1 "Island trees"
pixellab scene layer assign hero-island --to railway --items 150.0
pixellab scene layer move hero-island railway --by 1,0
pixellab scene layer ungroup hero-island railway
pixellab scene place hero-island pine --at 3,1,1 --layer railway
```

Errors use the existing codes (`not_found`, `usage`, `exists`) plus `layer_cycle`. Duplicating an item keeps its layer; removing an item leaves its layer in place, even if empty. The MCP tools are `scene_layer_new`, `scene_layer_rename`, `scene_layer_ungroup`, `scene_layer_assign` and `scene_layer_move`; the server's instructions name them; they're edits like any other, so `doc_save` proposes them. Hidden and locked aren't in any command: they're the app's (D8).

### D5. The app's model

- `Model/LayerTree.swift` (new, pure, unit-tested): built from `scene.items`. Layers by id, child layers in order, each layer's direct items in paint order, an item's or layer's chain of layers (outermost first), the rows of the tree for a set of expanded layers, and the picking rules of D6.
- `LayerTarget`: `.layer(id)` or `.item(path)`; written `layer:<id>` or `<path>` in the script hook.
- `DocumentModel.selection: [LayerTarget]`. `selectedPath` becomes a computed property over it (the one selected item, or nil), so the existing item code (the inspector, the timeline, the hook's `expect selected`) keeps working.
- `Model/LayersModel.swift` (new, `AppModel.layers`, `@Observable`): `inside` (the layer you're in), `expanded`, `hidden` and `locked` (D8), the hover target, the open menu (D9), the name draft and the Moved by baseline (D12). The stage and keys call into it from `SceneEditor`, so `SceneEditor` changes little.
- Renders: the app's `scene.render` passes `hide` (the scene's hidden layers) and, inside a layer, `focus`. Changing either renders again and clears the timeline's frame cache.

### D6. Picking on the stage (R8, R9, R11)

- **The target of an item**, with chain `[L1, L2, …]`: not inside a layer, it's `L1` (the item when the chain is empty). Inside `X`, an item whose chain doesn't hold `X` is outside and can't be picked; otherwise the target is the layer after `X` in the chain, or the item.
- **Pickable:** drawn (the render leaves hidden layers out), and no locked layer in its chain after `X` (so a locked layer's contents can be picked from inside it).
- **Hit-testing** keeps today's pixel test (`HitTester`: render bounds pick candidates, topmost first; each candidate's own art decides), skipping items that aren't pickable.
- **Clicks:** select the target; ⇧-click toggles it; a click on nothing clears (not with ⇧). A double-click on a layer goes inside it and selects the next level under the pointer.
- **Outlines:** a selected layer, a dashed accent box round its drawn items with a tag "Railway · 12"; a selected item, a solid accent box with its name; the target under the pointer (Select tool), a muted dashed box with its name on the halo. Inside a layer, its box is outlined in gold at 70%.
- **The fade:** inside a layer, the full render draws at 22% and `focusPng` at full over it, so the layer's own items stay in full view (where an outside item stood in front of one, the inside item now shows over it). The change animates over 0.2 s, and is immediate with Reduce Motion.
- **The breadcrumb pill** (top left, over the stage, scene mode): the scene icon, `hero-island › Railway › Train` (the current crumb semibold), and Done (accent) while inside. A crumb goes to that level; Done goes up one level and selects the layer you were in.
- **The place pill** (top right, Place tool): "Placing pine into Railway: click a tile" and Done (back to Select), as in the prototype.
- **No hint bar** (R17): the stage gains none; the notice bar for errors stays.
- The stage's VoiceOver label: "hero-island stage, inside Railway. Use the Layers list to select from the keyboard."

### D7. Moving (R10)

- **Dragging:** pressing on a selected target drags the whole selection; pressing on anything else selects it and drags it. One item alone drags as today (`ItemDrag`: tiles, ⌥ pixels, keyed past time 0). Layers, or several things, drag by tiles (col and row from the pointer's tile at level 0, ⌥ ignored): each moved item's art follows at 65% (up to 300 items; beyond, the box alone), and the first selection tag reads `Railway  +1 col  −2 row` (`3 things  +1 col  −2 row`). Release sends `scene.layer.move` with the selection's layers and items.
- **Keys and nudges:** arrows move the selection a tile (`scene.layer.move`), ⇧↑ ⇧↓ and Page Up/Down a level. ⌥-arrows move one item by a pixel; with layers selected they say "Pixel moves are for one item" (VoiceOver and the notice line). A single item keeps `scene.move`.
- One undo step per drag or key press; held keys coalesce.

### D8. Hide and lock (R11)

- Per document and scene, in `<state dir>/layer-view.json`: `{ "<document root>": { "<scene>": { "hidden": ["train"], "locked": ["ground"] } } }`. The app reads it when a scene opens and writes it on each change. The CLI, the servers, Publish and agents never read it. Under the script hook the state dir is the script's temp dir, so the owner's file is never touched.
- **Hidden:** the eye crosses out, the name turns muted and regular, its items aren't drawn (`hide`) and so can't be picked; hiding a selected layer deselects it. Its rows still select from the tree.
- **Locked:** the lock shows filled; its items can't be picked on the stage unless you're inside it (or inside a layer within it). The tree, the inspector and the keys still select and move it.

### D9. The right-click menu (R12)

- Drawn in the window: a board 03 card with a border and shadow, 210 pt wide, at the pointer (kept inside the window), over everything, with a click-catcher behind it. Items: **Group** ⌘G, **Ungroup** ⇧⌘G, a separator, **Go Inside**, **Hide** (Show when hidden), **Lock** (Unlock when locked). Group is off with nothing selected; the others are off unless exactly one layer is selected. The hovered or keyboard-focused item takes the accent.
- **Opening:** right-click or ⌃-click on the stage (`StageInputView.rightMouseDown`) or on a Layers row (a small AppKit view over each row that only takes right-clicks). The target under the pointer, if it isn't in the selection, becomes the selection first; one in the selection keeps it; on the stage, right-clicking nothing keeps the selection.
- **Keyboard and VoiceOver:** the menu takes focus; ↑↓ move, Return chooses, Esc or a click outside closes. VoiceOver reads it as "Layer actions", each item a button labelled with its shortcut ("Ungroup, Shift Command G"); SwiftUI can't give a drawn view the menu role.
- **Why in the window and not an `NSMenu`:** the prototype draws it in the window, and an `NSMenu` runs its own tracking loop that the script hook can't drive and window snapshots can't capture. The same actions are in the Edit menu (Group ⌘G, Ungroup ⇧⌘G, under Select None) and the inspector (Go inside, Hide, Lock). No toolbar buttons.

### D10. Group and Ungroup (R13)

- **Group** (⌘G, the menu): `scene.layer.new` with the selection's layers and items, no name, so the server names it (D4). The new layer is selected and its row revealed, the Layers tab shown, and VoiceOver hears "Grouped 12 items as Group 1. Rename it in the inspector."
- **Ungroup** (⇧⌘G, the menu; one layer selected): `scene.layer.ungroup`. The selection becomes what it held (its layers, then its items). Inside it, you're now inside its parent.

### D11. The sidebar and the Layers panel (R6, R7, R20)

- Search field; SCENES (no count) and scene rows (icon and name, no count); in scene mode a segmented Layers | Library (`SceneEditor.sidebarTab` defaults to `.layers`); object mode keeps Palette | Library. `Views/ItemsTree.swift` is replaced by `Views/LayersPanel.swift`.
- The Layers tab: LAYERS (no count), then the tree, in its own scroll area filling the sidebar under the tabs.
- **Layer row** (28 pt): a chevron button (turns 90°; no animation with Reduce Motion), `square.dashed`, the name in Sora 12.5 semibold (inkMuted, regular, when hidden), a gold IN badge while you're inside it, the lock button (24 pt; shown on hover, on selection or while locked), the eye button (24 pt). Indented 14 pt a level.
- **Item row:** `cube`, the object name, and its position on the right in SF Mono (`14,6,1`, or `px 31,6`).
- **Order:** a layer's child layers (in `layers` order), then its items in paint order; the top level is the same. Layers start collapsed; selecting something on the stage expands its chain and scrolls its row into view.
- Selected rows take `Theme.selection`, hovered rows `Theme.card`; inside a layer, rows outside it dim to 40%.
- **Search:** a flat list of the items (object, id, class) and layers (name) that match.
- **Keyboard:** clicking the tree (or Tab) focuses it, with an accent focus ring. ↑↓ select the previous or next row, ⇧↑ ⇧↓ add it to the selection, → expands a layer (or goes to its first row), ← collapses it (or goes to its parent), Return goes inside the selected layer, Esc leaves the layer you're inside.
- **VoiceOver:** "Layer Railway, 12 items, hidden, locked" with actions Go Inside, Hide or Show, Lock or Unlock, Expand or Collapse; "pine, column 14, row 6, level 1, in Railway"; the selected trait on selected rows; "Hide Railway", "Lock Railway", "Expand Railway" on the buttons.

### D12. The inspector (R14, R15)

- **A layer.** SELECTED: "Layer name" field (Sora 13 semibold; Return renames), "12 items · in Railway" ("· top level"), and Go inside, Hide (Show), Lock (Unlock). MOVED BY: Col and Row fields and the four diagonal nudges (up-left col −1, up-right row −1, down-left row +1, down-right col +1, the arrow keys' directions), with "Drag on the stage, or nudge a tile at a time. Every item in the layer moves together." LAYER: Place into it (the Library, the Place tool, this layer as the target), with "Right-click › Ungroup (⇧⌘G) to undo it. Paint order doesn't change: each item still draws back to front by where it stands."
  - **Moved by** counts from when the layer was selected: the offset of its first item on a tile (else its first item, in whole tiles) from where it was then, so it reads 0 on selecting and follows drags, nudges and undo. Typing a value and Return moves the layer by the difference.
- **An item:** the existing SELECTED and POSITION sections, plus "in Railway" with **Select Railway** (or "Not in a layer"), and the wrapper line of D3.
- **Several things:** "3 selected", "14 items in all. Group them to name, hide and move them as one.", "Right-click › Group (⌘G) to make them one layer."; MOVE TOGETHER with the four nudges.
- **Nothing:** "Nothing selected", "Click the stage to pick a layer, or a row in Layers. Double-click a layer to work inside it."
- The document card says "219 items · 3 layers · motion 28.0 s".

### D13. Placing into a layer (R16)

- The target: one layer selected → it; one item selected → its layer; else the layer you're inside; else the top level (the prototype fell back to the layer the object's kind lives in; the owner changed that).
- The Library tab shows "New items go into" and a chip: `square.dashed` and the layer's name, or the scene icon and "Top level". The note under the grid: "Select a layer, or go inside one, and what you place joins it. With neither, it goes in at the top level." The Place inspector says "into Railway" too.
- Placing passes `layer` to `scene.place`; the placed item is selected, so the next one goes into the same layer.

### D14. The script hook (R18)

`ScriptRunner.swift` (and `CLAUDE.md`'s list):

- `select layer:<id>|<path>|#<id> …` selects one or more targets; `select add <target>` adds one; `click <x> <y> [shift]` and `dblclick <x> <y>` click the stage in scene pixels through the stage's picking; `pointer <x> <y>` moves the pointer there (the hover outline); `rightclick <x> <y>` and `rightclick row <target>` open the menu; `menu group|ungroup|inside|hide|lock|close` chooses from it.
- `group`, `ungroup`, `inside <layer>|done|top`, `crumb <n>`, `hide <layer>`, `show <layer>`, `lock <layer>`, `unlock <layer>`, `expand <layer>`, `collapse <layer>`, `rename <name…>`, `movedby <col> <row>`, `layernudge upleft|upright|downleft|downright`, `placeinto`, `layerskey <spec>` (a key into the tree: up, down, shift+down, right, left, return, esc), `sidebar layers|library` (`items` stays an alias of `layers`).
- `move`, `drag` and the arrow keys act on the whole selection.
- `expect selection <targets,…|none>`, `inside <id|none>`, `scenelayers <n>` (`layers` is object mode's), `layername <name>`, `itemlayer <id|none>`, `hiddenlayers <ids|none>`, `lockedlayers <ids|none>`, `into <name>`, `crumbs <a›b›c>`, `rows <n>`, `movedby <+c,−r>`, `menu <enabled items|none>`, `tab layers|library`, `inspector layer|item|multi|none`, `faded true|false`, `hover <target|none>`.

### D15. Docs (R19)

README (the Scene mode tour: the Layers tab, the stage, the menu, the inspector, the Library chip; the shortcuts table: ⌘G, ⇧⌘G, double-click, Esc), `app/README.md` (`LayersPanel` and the new model files), `docs/architecture.md` (layers in the scene format), `docs/testing.md` (the hook's layer actions, `layers.txt`), `cli/README.md` (the layer commands), `CLAUDE.md` (the hook list), the MCP instructions, and `npm run docs:cli`.

## Areas of concern

- **The site's pinned engine (external dependency).** A scene file with `layers` or an item `layer` fails validation in every engine before this change ("unknown property"). The site pins `pixellab` at `v0.3.0` and runs `pixellab art build --site .` and its art tests with it. So before the owner makes layers in the site's document, the site bumps its pin to the release with this change (a change in the site repo, under its own workflow). Nothing here writes to the site, and documents without layers are unaffected.
- **A menu drawn in the window, not an `NSMenu`.** Mac convention is an `NSMenu`, but the prototype draws its own and an `NSMenu` can be neither scripted nor snapshotted by the hook (D9). The drawn menu has the keyboard, VoiceOver roles, and the same actions in the Edit menu and inspector. If the owner prefers a native menu, it's a contained swap in `StageMenu`.
- **The fade draws the layer over the rest** (D6), not in paint order as the prototype does, so where an outside item stood in front of an inside one, the inside one shows. It keeps what you work on in view and costs one extra image per render.
- **Moved by counts from selection** (D12): the prototype counted from the session's start, which undo and other edits make wrong; counting from selection stays right under undo.
- **Layer ids don't follow renames** (D1): `Group 1` renamed to `Railway` keeps the id `group-1` in the file, so a rename is one line. Agents can still use the name.
- **View state in the state dir** (D8): next to proposals rather than in user defaults, so the script hook's temp state keeps the owner's untouched and the hook can check it.
- **The open animation pane (PR #17)** edits the same app files (`SceneEditor`, `ScriptRunner`, `Protocol`, `PixelLabApp`, `SelectionInspector`, the README, `CLAUDE.md`). Layer code goes in new files where it can; if #17 lands first, `main` is merged in before the final PR.

## Open questions

- From the intent, where layers are stored: in the scene file, as the owner leaned (D1).
- From the intent, whether the app still shows the wrappers' attributes: yes, read-only, in the item inspector (D3).
- None carried forward.
