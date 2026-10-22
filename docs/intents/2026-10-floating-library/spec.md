# Spec: A canvas-first window, with a floating library (from intent.md 2026-10-21)
Status: approved (by the owner in chat on 2026-10-22; the plan follows in the same PR).

The design is the prototype canvas named in the intent:
- `Groups.dc.html` and `GroupsDay.dc.html`: the scene, the floating bar and Library, and going into an object and back.
- `Object.dc.html`: the object view.
- `Animation.dc.html`, `Documents.dc.html` and `AgentChat.dc.html`: the same window with its other panes.
- `Main.dc.html`: menus and shortcuts.

This spec settles what the prototype leaves open: input handling, the window's chrome on macOS, the keyboard path for every control that leaves the screen, the test hook and the docs. Where the app departs from the prototype, the spec says so in place.

This is a change to the Mac app, apart from one widened parameter: `scene.render`'s `hide` list also takes item paths, so a single item can be hidden (R19, D12). That reaches all three doors (CLI, `serve --stdio`, `serve --mcp`) through the one registry. Nothing an agent can do is lost, and the document, the engine and the published art don't change.

## Requirements

**Moving around the canvas**

- **R1. Pinch to zoom.**
  - A trackpad pinch on the stage steps the zoom through the open doc's whole levels: scenes 1×–4×, objects 4×–16× by 4. It keeps the art pixel under the pointer in place.
  - ⌘-scroll (a mouse wheel or two fingers with ⌘ held) does the same.
  - Art is always drawn at a whole zoom, including mid-pinch. There's no smooth scaling between levels.
- **R2. Zoom from the keyboard.**
  - View › Zoom In (⌘+) and Zoom Out (⌘−) step one level, centered on the stage.
  - Zoom to Fit (⌘0) and the per-level items (⌘1–⌘4) stay.
- **R3. No zoom control on screen.**
  - The zoom menu leaves the window.
  - When the zoom changes by pinch, scroll or keyboard, a small readout ("2×") shows over the stage's bottom-right corner for about a second, then fades.
  - With Reduce Motion on, the readout appears and disappears without fading.
  - VoiceOver announces the new zoom.
- **R4. Panning.**
  - Two-finger scroll pans the stage, as it does now.
  - The Hand tool drags the view.
  - Space held with any tool is a temporary hand, and Space + arrows pan, as now.
- **R5. Quiet scroll bars.** The stage's scroll bars follow the system's "Show scroll bars" setting (the owner's decision).
  - Set to show them automatically or when scrolling: thin overlay scrollers that show while scrolling, fade after and take no layout space, so nothing shifts when they appear.
  - Set to Always: they're always shown, in the small (thinner) size. Their room is always reserved, so nothing shifts either.
  - This applies to the stage and to the Library popover's grid (R8).

**The floating bar**

- **R6. A floating bar at the bottom center of the stage.**
  - **Contents in a scene:** Select (V) and Hand (H), a divider, then Library (L).
  - **Contents in an object:** Pencil (B), Eraser (E), Fill (G), Picker (I) and Hand (H).
  - **Buttons:**
    - Icons only, with the selected tool marked.
    - Each button's help tag names it and its key, and VoiceOver reads the same.
    - Hit targets are at least 36 pt.
  - **Layout:**
    - The bar floats over the art, above the timeline strip or the object status bar, and moves with neither scrolling nor zoom.
    - It's centered under the visible stage, between the sidebar and the right panel.
- **R7. Select and Hand are the scene's only tools.**
  - The Place tool and its key (A) go. Picking an object in the Library starts placing, as clicking a Library cell does today. Placing works as it does now: click a tile to place, keep clicking to place more, then Esc or Select (V) to stop.
  - Every other key the stage takes stays (nudges, ⌫, ⌘D, and the rest).

**The Library**

- **R8. The Library is a popover above the bar.**
  - **Opening and closing:** L, or the Library button, opens and closes it. Esc closes it when the stage doesn't need Esc for something else. It stays open while placing and dragging.
  - **Contents:**
    - A search field.
    - A grid of small, crisp thumbnails with no names, and no close button or instruction text (the owner's prototype review). Each cell's name is its help tag and its VoiceOver label.
    - A large library scrolls inside the popover's fixed height with a quiet scroller (R5).
  - **Using it:**
    - Clicking a cell starts placing that object (R7).
    - Dragging a cell onto the stage places one where it's dropped, onto the tile under the pointer. ⌥ while dropping places it by pixel, as placing does now.
    - Double-clicking a cell opens the object (R11).
    - Right-clicking a cell gives Place <name> and Edit Object.
    - The arrow keys move between cells and Return picks one.
- **R9. Where placed items go stays the same.** The rule for which layer a placed item joins doesn't change (`../2026-10-scene-layers/spec.md`, D13). The "New items go into" chip leaves with the sidebar's Library, so the placing preview on the stage names its target instead: "pine → Trees", as in the prototype.
- **R10. The sidebar loses its Library tab.**
  - **In a scene:** Scenes, then Layers.
  - **In an object:** Scenes, then the Palette.
  - The Layers | Library and Palette | Library switches go.

**Objects and scenes**

- **R11. No Scene/Object switch.**
  - **Into an object:**
    - Edit Object, from the Library's right-click menu or the stage's right-click menu on an item, opens that object. The stage's layer menu gains an Edit Object “<name>” item for the item under the pointer.
    - Double-clicking a Library cell does the same.
    - Edit › Edit Object (⌘↩) opens the selected item's object.
  - **Back out:**
    - In the object view, a pill at the stage's top left shows **‹ Back · hero-island › house**. Back, or the scene's name in it, returns to the scene the owner came from (the last scene, when they didn't come from one).
    - View › Back to <scene> (⌘[) does the same from the keyboard.
    - Clicking any scene in the sidebar opens that scene.
  - **Other ways in:** File › New Object stays, and so does opening an object from Search.

**The window**

- **R12. No toolbar.**
  - The window keeps its title and its Edited/Saved subtitle in a slim title bar, with no other controls.
  - These leave the screen: the Scene/Object switch, the tool pickers, the zoom menu, Grid, Publish, Save, the Agent button and the Inspector toggle.
  - Each keeps its menu item and shortcut, apart from the ones R7 and R13 remove:
    - Grid: View › Show/Hide Grid (⌘').
    - Publish: File › Publish (⌘B).
    - Save: File › Save (⌘S).
    - Agent: Window › Agent (⌥⌘A).
- **R13. The right panel is always shown.**
  - No button, menu item or shortcut hides it. View › Hide/Show Inspector (⌥⌘I) goes.
  - The panel's width stays resizable within today's limits.
- **R14. Save and Publish live in the right panel.**
  - The tab row reads **Inspector | ✦ Agent ⟨n⟩ … Publish · Save**.
  - Publish is an icon button: "Publish (⌘B)".
  - Save is the window's one accent button. It's disabled when there's nothing to save, and its VoiceOver label says whether anything is unsaved.
  - Both are visible in either tab, in scenes and objects.
  - The waiting-proposals badge that was on the toolbar's Agent button sits on the Agent tab.
- **R15. Nothing else changes.**
  - Saving stays explicit (autosave is `../2026-10-autosave/intent.md`).
  - The document, the engine and the published art don't change. The CLI and MCP change only by `scene.render`'s `hide` taking item paths (R19).
  - The Agent window, the chat and proposals behave as they do today, apart from where the badge sits.
  - There's no command to hide every panel; the owner doesn't need one for now.

**Layers**

- **R19. Hide and lock single items.** Added on 2026-10-22, from the owner's comment on the prototype.
  - In Layers, an item's row has the same lock and eye buttons as a group's row, in place of the position label it shows today (`col,row,level`, or `px x,y`).
  - The item's position stays in the inspector, and VoiceOver still reads the row as the item and its position.
  - **Hidden:**
    - The eye crosses out, the name turns muted, and the item isn't drawn on the stage, so it can't be picked.
    - Hiding a selected item deselects it.
    - Its row still selects from the tree.
  - **Locked:**
    - The lock shows filled, and the item can't be picked on the stage.
    - The tree, the inspector and the keys still select and move it.
  - As with layers, this is a view setting:
    - It's kept per document and scene.
    - It never changes the document, saving, publishing or what agents see.
    - It follows the item when an edit or a depth sort moves its path.

**Accessibility, tests and docs**

- **R16. Accessibility.**
  - Every control in the bar, the popover and the back pill has a VoiceOver label, visible keyboard focus and a keyboard path.
  - Everything that leaves the screen is reachable from a menu.
  - Reduce Motion turns off the readout's fade, the popover's open animation and the scrollers' fade.
- **R17. The test hook drives the new window** (D10). The acceptance pass runs green with the hero island's SVG byte-identical, as it does today.
- **R18. Docs.**
  - `README.md`'s tour and shortcuts table describe the new window.
  - The prototype canvas already matches it.

## Design

### D1. The window's chrome (R12, R13)
- **The toolbar:** `DocumentWindow` drops `DocumentToolbar` and `AgentToolbarButton`. It keeps `.navigationTitle` and `.navigationSubtitle`, so the title bar shows the doc and its Edited/Saved state.
- **No sidebar toggle:** the `NavigationSplitView` sidebar toggle comes out with `.toolbar(removing: .sidebarToggle)` (macOS 14+; the package targets macOS 15). The window style becomes `.windowToolbarStyle(.unifiedCompact)` with no items, so the title bar is the slim one in the prototype.
- **The right panel:**
  - `.inspector(isPresented:)` stays, bound to a constant `true`. `showInspector` and its menu item go.
  - The column keeps `.inspectorColumnWidth(min: 260, ideal: 340, max: 520)`.
  - The sidebar stays collapsible with the system's View › Hide Sidebar (⌃⌘S), which `NavigationSplitView` provides (the owner's decision).

### D2. Zoom (R1–R3)
- **Pinch and ⌘-scroll:**
  - `StageInput` overrides `magnify(with:)` and `scrollWheel(with:)`, the latter only when ⌘ is held; plain scrolls go to the enclosing `NSScrollView` as now.
  - Pinch deltas add up and step one level per threshold, about 0.25 of magnification. ⌘-scroll deltas step one level per notch on a mouse, and per threshold on a trackpad. The running total resets when the gesture ends.
- **Keeping the pixel under the pointer:** a step sets `model.zoom = .level(n)`. After the stage lays out at the new zoom, the clip view scrolls so the scene point that was under the pointer stays there. `StageGeometry` already maps both ways.
- **Fit:** a pinch from Fit starts at Fit's effective level.
- **Keyboard:**
  - Zoom In and Zoom Out are new View menu items (⌘= and ⌘−; ⌘+ is ⌘= on the keyboard) that call `AppModel.stepZoom(by:)`. They're centered on the stage, not the pointer.
  - `setZoomLevel(index:)` and Zoom to Fit stay.
- **The readout:**
  - `AppModel.zoomReadout` holds a value and an expiry, and is set on every change.
  - `StageView` overlays a `ZoomReadout` (the `Theme.halo` fill, `Theme.border` hairline, mono 12 pt) at the bottom right. It clears after about 1.1 s.
  - It posts an `AccessibilityNotification.Announcement` ("Zoom 3×").
  - Under Reduce Motion it appears and disappears with no fade.

### D3. Scroll bars (R5)
- The stage's `ScrollView` and the Library grid's keep the system's scroller style. Their underlying `NSScrollView`s, reached the way `StageInput` already reaches `enclosingScrollView`, don't set `scrollerStyle`, so AppKit follows the "Show scroll bars" setting and the input device.
- They do set `autohidesScrollers = true`, and set both scrollers' `controlSize` to `.small`, so a scroller that stays shown is the thin one.
- With overlay scrollers nothing takes layout space. With legacy scrollers the room is reserved from the start, so the content never shifts when a scroller appears.

### D4. The floating bar (R6, R7)
- **The view:**
  - A new `FloatingBar` view sits in a `StageView` overlay, aligned to the bottom center with 16 pt padding. It's outside the `ScrollView`, so it doesn't scroll or zoom.
  - It's drawn with `Theme.sidebar`, a `Theme.border` hairline, a soft shadow and 12 pt corners.
  - Its buttons are `FloatingBarButton`s: 36 × 36 pt, SF Symbols at 15 pt, and `Theme.selection` with an accent icon when on.
- **Scene tools:**
  - `StageTool` keeps `select` and `pan`, with `pan` titled Hand.
  - `place` stays as an internal state, `SceneEditor.placeObject != nil`, rather than a tool in the bar. The `effectiveTool` logic keys off that state instead of the `.place` tool. The `A` key handler goes.
- **Object tools:** the bar shows `PaintTool.allCases` plus a Hand button, which toggles a held pan mode equivalent to Space held.
- **Focus:** a bar click calls `editor.focusStage()`, as today's toolbar pickers do, so the keys keep working.

### D5. The Library popover (R8, R9)
- **Not an `NSPopover`.** A system popover closes when the pointer leaves it, which would break dragging onto the stage, and it wouldn't take the board 03 look. The popover is instead an in-window overlay, `LibraryPopover`, anchored above the bar at a fixed 452 pt width and a grid height of 236 pt.
- **State:** `AppModel.libraryOpen: Bool`. There's no `@State`, which the Command Line Tools build can't use.
- **Cells:**
  - `LibraryCell` gains a `compact` style: 44 × 42 pt, no name label, the `factor(for:in:)` thumbnail rule with ½ allowed, and `.help(name)` and `.accessibilityLabel("Place \(name)")`.
  - Eight columns.
  - The existing `.onDrag` (with `LibraryDrag`) and the stage's drop handling are reused unchanged.
- **The context menu:** Place <name> and Edit Object, renamed from "Open in Object Mode".
- **Search:** it filters with the existing `filteredObjects`.
- **The keyboard:**
  - L toggles the popover from the stage.
  - Esc closes it when nothing on the stage is in progress (placing first stops placing, then a second Esc closes it).
  - Focus moves into the search field when it opens. Tab reaches the grid, where the arrows move between cells and Return picks.
- **The placing preview:** the ghost tag `StageOverlay` draws while placing reads "<object> → <layer>", using the same rule the removed `PlaceIntoChip` used.

### D6. The sidebar (R10)
- `SidebarView` drops the two segmented pickers.
- In a scene it shows Scenes then Layers. In an object it shows Scenes then `ObjectPanels`' palette.
- `SceneEditor.sidebarTab` and `ObjectEditor.sidebarTab` go. The calls in `LayersModel` that switch to `.library` (`placeHere` and the like) now open the popover.

### D7. Into an object and back (R11)
- **Opening an object:** `AppModel.editObject(_ name: String)` records `returnScene = document?.ref` when the open doc is a scene, then opens `DocRef(.object, name)`.
- **Going back:** `AppModel.backToScene()` opens `returnScene`, or failing that the last scene, then hero-island, then the first scene. This is `switchMode(to: .scene)`'s fallback, which is kept for the test hook.
- **The stage's layer menu** (`StageMenu`) gains "Edit Object “<name>”" for the item under the pointer.
- **The menus:**
  - Edit › Edit Object (⌘↩) works on the selected item.
  - View › Back to <scene> (⌘[) is enabled in object view.
- **The back pill:** `BackPill` is a `StageView` overlay at the top left in object view. It shows a ‹ Back button, a divider, the scene's name as a button, then › and the object's name. It uses the same capsule style as the layers breadcrumb, so the two read as one way in and out.

### D8. The right panel's row (R14)
- `RightColumn`'s tab row becomes an `HStack`:
  - On the left, the two tabs. The Agent tab carries `agent.pendingCount` as a gold count badge, with the same rule and labels as today's `AgentToolbarButton`.
  - A `Spacer`.
  - A Publish icon button (`square.and.arrow.up`, help "Publish (⌘B)", disabled while a build runs).
  - Save, in `AccentButtonStyle(height: 26)`, disabled when `!model.canSave`, labelled "Save, unsaved changes" or "Save, all saved".
- The tabs no longer stretch to fill the row.
- `showAgentTab()` stops setting `showInspector`.

### D9. Menus (R2, R11, R12, R13)
In `PixelLabApp`'s commands:
- **Added:**
  - Zoom In (⌘=) and Zoom Out (⌘−).
  - Edit Object (⌘↩).
  - Back to <scene> (⌘[).
- **Removed:** Show/Hide Inspector (⌥⌘I).
- **Kept as they are:** Zoom to Fit (⌘0), the zoom levels (⌘1–⌘4), Show/Hide Grid (⌘'), Save, Save As, Revert, Publish, and Window › Agent and Agent Window.

### D10. The test hook (R17)
`ScriptRunner` keeps its existing steps working, so the acceptance scripts don't churn more than needed:

| Step | After this change |
|---|---|
| `mode scene\|object` | Does what it does today. `mode object` is now an Edit Object on the selection, or the last object. |
| `tool select\|pan` | Unchanged. `tool place` fails with a pointer to `library <name>`. |
| `sidebar library` | Opens the popover. |
| `sidebar layers\|palette` | Closes the popover. |
| `zoom <n>\|fit` | Unchanged. |

New steps, with the `expect` keys that check them:

| Step | What it does | Checked by |
|---|---|---|
| `pinch in\|out [<x> <y>]` | One zoom step anchored at a stage point | `expect zoom <n>`, `expect readout <n×\|none>` |
| `zoomkey in\|out` | ⌘= or ⌘− | `expect zoom <n>` |
| `library open\|close` | Opens or closes the popover | `expect library open\|closed` |
| `librarydrop <name> <col> <row> <level>` | A drop from the popover | the placed item, by path |
| `editobject [<name>]` | Edit Object | the object view opens |
| `back` | Back to the scene | `expect returnscene <name>` |
| (none) | — | `expect floatingbar <tools>` (for example `select,pan,library`), `expect toolbar none`, `expect inspector shown` |

The acceptance scripts that use `mode`, `sidebar` or `tool place` are updated to the new steps where the meaning changed. `layers.txt`, `scene-object.txt`, `chat-harbor.txt` and `stale.txt` are the ones that use them. Every changed view gets dark and light snapshots: the stage with the bar, the open popover, the object view with the back pill, and the right panel's row.

### D11. Docs (R18)
- `README.md`:
  - The tour describes the floating bar, the Library popover, Edit Object and Back, and Save and Publish in the right panel.
  - The shortcuts table:
    - Drops A and ⌥⌘I.
    - Adds L, ⌘=, ⌘−, ⌘↩ and ⌘[.
    - Notes pinch and ⌘-scroll for zoom.
- `docs/cli.md` is regenerated for `scene.render`'s widened `hide` (D12). No other command changes.
- The docs site is rebuilt to check links.

### D12. Hiding and locking single items (R19)
- **Storage:** `layer-view.json` gains `hiddenItems` and `lockedItems` per scene, next to `hidden` and `locked`. Each entry is an item path plus its object name: `{ "path": "145.0", "object": "pine" }`.
  - When an edit result's `reordered` moves a path, the entry follows it, as the selection already does (`LayerTarget.following`).
  - When a scene opens or reloads, entries whose path no longer holds that object are dropped. This covers a file changed on disk, a merge or a revert.
  - Only the app reads the file. The CLI, the servers, Publish and agents never do, as with layers (`../2026-10-scene-layers/spec.md`, D8).
- **Rendering:**
  - `scene.render`'s `hide` parameter takes item paths as well as layer ids and names. A string that names a layer hides the layer; otherwise a string that is an item's path hides that item.
  - `crates/pixellab/src/render.rs` applies it, the command's schema and description say so, and `docs/cli.md` is regenerated.
  - The app sends hidden items with the hidden layers (`LayersModel.renderParams`).
- **Picking:**
  - `LayerTree`'s pickability check skips hidden and locked items, the way it skips the items of hidden and locked layers.
  - Hover and ⌥-click (Select Every Use) skip them too.
- **The row:**
  - `LayersPanel`'s item row drops its position text and shows the same lock and eye buttons as a group row: the lock only on hover, selection or when locked, and the eye always.
  - The buttons have the same labels, with the item's object name ("Hide pine", "Lock pine").
  - The row's VoiceOver label keeps the position: "pine at 4,-1,1, hidden".
- **The test hook:** `hide|show|lock|unlock` take an item path as well as a layer id. `expect hiddenitems|lockeditems <paths,…|none>` checks them.

## Areas of concern
The owner settled the concerns raised in review on 2026-10-22:
- **How far pinch reaches:** today's levels (scenes 1×–4×, objects 4×–16×) are fine.
- **A way to hide every panel:** not needed for now. It's out of scope.
- **Shortcuts:**
  - Removed: A (Place) and ⌥⌘I (Inspector).
  - Added: L, ⌘= and ⌘−, ⌘↩ for Edit Object, and ⌘[ for Back.
  - The owner accepted all of these.
- **Scroll bars:** respect the system's "Show scroll bars" setting (R5, D3), rather than forcing overlay scrollers.
- **The sidebar:** it stays collapsible through View › Hide Sidebar.
- **Publish:** it stays beside Save in the right panel, as prototyped.

These remain as notes for the plan:
- **Item hide and lock widen the CLI and MCP surface** (R19, D12). The intent was amended on 2026-10-22 to allow this one change. Agents can now pass item paths in `scene.render`'s `hide`, which is read-only and changes nothing on disk.
- **Hidden items are keyed by path,** because most items have no id. The path follows edits made in the app (through `reordered`). After an outside change the entry is dropped when the path's object differs. That's safe, but in a rare case where an outside edit swaps two items of the same object, the hide can land on the other one.
- **Losing the toolbar's discoverability.** Apple's guidelines say a toolbar item can't be the only place for a command, and that hidden controls need a reliable way back. Every removed control keeps a menu item (D9), and the bar's help tags name their keys.
- **The Command Line Tools build has no SwiftUI macros.** All new view state (`libraryOpen`, `zoomReadout`, `returnScene`) lives in `AppModel`, as `CLAUDE.md` requires.

## Open questions
None. The intent's two questions are answered: pinch keeps today's levels, and a way to hide every panel isn't needed for now. The review's questions are answered above.
