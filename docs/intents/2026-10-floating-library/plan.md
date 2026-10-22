# Plan: A canvas-first window, with a floating library (from intent.md 2026-10-21)

The intent, spec and plan merge to `main` together in PR #38 (`intent/floating-library`). The code then starts on `feat/floating-library`, cut from `main` after that merge.

The change is to the Mac app (`app/`), its test hook and acceptance scripts, and the README. There's one exception: `scene.render`'s `hide` also takes item paths, so single items can be hidden (spec R19, D12). That's a small change in `crates/pixellab` and `docs/cli.md`. The document format, the engine and the published art don't change.

## Files that change

**The app**
- `app/Sources/PixelLab/Views/MainWindow.swift`
  - `DocumentToolbar` and `AgentToolbarButton` deleted (the badge moves to `RightColumn`).
  - No sidebar toggle, the compact title bar, and the inspector always presented (D1).
- `app/Sources/PixelLab/PixelLabApp.swift`
  - The window toolbar style.
  - Menus:
    - Added: Zoom In (⌘=), Zoom Out (⌘−), Edit › Edit Object (⌘↩), View › Back to <scene> (⌘[) (D9).
    - Removed: Show/Hide Inspector (⌥⌘I).
- `app/Sources/PixelLab/Model/AppModel.swift`
  - Added: `stepZoom(by:anchor:)`, `zoomReadout`, `libraryOpen`, `returnScene`, `editObject(_:)` and `backToScene()`.
  - Removed: `showInspector`.
  - `switchMode(to:)` is kept for the test hook only.
- `app/Sources/PixelLab/Model/SceneEditor.swift`
  - `StageTool` becomes `select, pan`, with Pan titled Hand.
  - Placing is the state `placeObject != nil`, and `effectiveTool` follows it.
  - The `A` key goes, and `sidebarTab` is removed.
- `app/Sources/PixelLab/Model/ObjectEditor.swift`
  - `sidebarTab` removed.
  - The Hand button's held pan (`handHeld`), which acts like Space held.
- `app/Sources/PixelLab/Model/LayersModel.swift`: the two `sidebarTab` writes become `libraryOpen` and placing (D6).
- `app/Sources/PixelLab/Views/StageInput.swift`
  - `magnify(with:)`, and `scrollWheel(with:)` with ⌘, step the zoom anchored under the pointer (D2).
  - The Hand tool drags the view in objects too.
- `app/Sources/PixelLab/Views/StageView.swift`
  - Overlays: `FloatingBar`, `LibraryPopover`, `ZoomReadout` and `BackPill`.
  - The scroll view's scrollers are small and autohide, keeping the system's style (D3).
  - The scroll position is kept after a zoom step.
- `app/Sources/PixelLab/Views/FloatingBar.swift` (new): `FloatingBar` and `FloatingBarButton` (D4).
- `app/Sources/PixelLab/Views/LibraryPopover.swift` (new): `LibraryPopover`, with its search, grid and keyboard handling (D5).
- `app/Sources/PixelLab/Views/StageChrome.swift` (new): `ZoomReadout`, `BackPill`, and the small `NSScrollView` configurator (D2, D3, D7).
- `app/Sources/PixelLab/Views/SidebarView.swift`
  - The segmented pickers go: Scenes then Layers, or Scenes then the Palette.
  - `LibraryCell` gains its `compact` style, the context menu renamed to Place / Edit Object, and the library grid moves out to `LibraryPopover` (D5, D6).
  - `PlaceIntoChip` is deleted, and its rule moves to `LayersModel.placeIntoName`.
- `app/Sources/PixelLab/Views/StageOverlay.swift`: the placing ghost's tag reads "<object> → <layer>" (R9).
- `app/Sources/PixelLab/Views/StageMenu.swift`: the layer menu gains Edit Object “<name>” for the item under the pointer (D7).
- `app/Sources/PixelLab/Views/RightColumn.swift`: the tab row, Inspector | ✦ Agent ⟨n⟩ … Publish · Save (D8).
- `app/Sources/PixelLab/Views/ObjectPanels.swift`: the palette shown without the Palette | Library switch, if the switch lives there (D6).

**Layers: hide and lock single items (R19)**
- `crates/pixellab/src/render.rs`: `hide` matches item paths as well as layer ids and names (a layer wins).
- `crates/pixellab/src/commands/scene.rs`: the `hide` schema and the command description mention item paths.
- `crates/pixellab/tests/render.rs`: `R19: hide takes an item path` and `R19: a layer name wins over a path`.
- `docs/cli.md` (generated): `npm run docs:cli`.
- `app/Sources/PixelLab/Model/LayersModel.swift`:
  - `LayerViewState` gains `hiddenItems` and `lockedItems` (path and object), with the toggles.
  - Entries follow `reordered` and are dropped on open or reload when the path's object differs.
  - `renderParams` sends the hidden items.
- `app/Sources/PixelLab/Model/LayerTree.swift`: the pickability check skips hidden and locked items.
- `app/Sources/PixelLab/Views/LayersPanel.swift`: the item row loses its position text and gains the group row's lock and eye buttons.
- `app/Tests/PixelLabTests/LayerTests.swift`: the R19 tests in step 6.

**Tests and the test hook**
- `app/Sources/PixelLab/Support/ScriptRunner.swift`:
  - The steps and expects in D10, plus aliases for `mode`, `sidebar` and `tool place`.
  - `hide|show|lock|unlock` take item paths, and `expect hiddenitems|lockeditems` checks them (D12).
- `app/Tests/PixelLabTests/CanvasTests.swift` (new): Swift Testing suites named by requirement (`R1: …`).
- `app/Tests/PixelLabTests/StageTests.swift`: tests that used the Place tool or `sidebarTab` updated.
- `scripts/acceptance/window.txt` (new; `canvas.txt` is the scene-canvas stage): the end-to-end run of the new window on the temp site copy.
- `scripts/acceptance/layers.txt`: `expect tab library` and `sidebar layers` become `expect library open` and `library close`.
- `scripts/acceptance.sh`: runs the `window` stage before `layers`, on the same temp site, and its header comment mentions it.

**Docs**
- `README.md`: the scene, object, publish and Agent paragraphs of the tour, and the shortcuts table (D11).

## Order of work

There's one slice: one branch and one PR into `main`. The steps form one chain. The window chrome, the bar, the Library, Back and the item rows all touch `StageView`, `AppModel`, `LayersModel` and `ScriptRunner`, so parallel runners would fight over the same files. The program change in step 5 is small enough to stay in the same slice.

```
main ── PR #38 (intent, spec, plan; owner merges)
  └── feat/floating-library ── steps 1–12 ── PR "A canvas-first window" (owner merges) ──▶ main
```

**PR: A canvas-first window, with a floating library** (`feat/floating-library` → `main`). Each step is one commit, and `swift build --package-path app` and `scripts/test-app.sh` pass after each one.

1. **Zoom (R1–R3).**
   - `AppModel.stepZoom(by:anchor:)` steps through `Zoom.levels`, starting from Fit's effective level.
   - The magnify and ⌘-scroll accumulators in `StageInput`.
   - The scroll position is kept so the anchor stays under the pointer.
   - Menus: Zoom In (⌘=) and Zoom Out (⌘−).
   - `ZoomReadout`, with its VoiceOver announcement and no fade under Reduce Motion.
   - Tests: `R1: a pinch steps one level and keeps the anchor`, `R1: never past the kind's levels`, `R2: ⌘= and ⌘− step from Fit`, `R3: the readout clears`.
2. **Scroll bars (R5).**
   - The scroll-view configurator: small scrollers, autohide, and `scrollerStyle` left to the system.
   - It's applied to the stage now, and to the Library grid in step 4.
   - Test: `R5: the stage keeps the system's scroller style`.
3. **Select, Hand and the floating bar (R4, R6, R7).**
   - `StageTool` loses `.place`, and placing becomes `placeObject != nil`. The A key goes.
   - `LayersModel` and `SceneEditor.choosePlaceObject` start placing without a tool switch.
   - `FloatingBar` in scene and object views, and the object Hand button (`handHeld`).
   - Tests: `R7: picking a Library object starts placing; Esc and V stop it`, `R6: the bar's tools per doc kind`.
4. **The Library popover and the sidebar (R8–R10).**
   - `LibraryPopover`, the compact `LibraryCell`, `libraryOpen`, and L and Esc. Drag and drop reuse `LibraryDrag`.
   - The context menu becomes Place / Edit Object.
   - The ghost tag names the target layer.
   - The sidebar's pickers and both `sidebarTab`s are removed, and `PlaceIntoChip` is deleted.
   - Tests: `R8: L opens and Esc closes, placing first`, `R9: the ghost tag names the layer placing goes into` (the cases from `../2026-10-scene-layers/spec.md` D13), `R10: the sidebar has no Library`.
5. **Item paths in `scene.render`'s `hide` (R19, program).**
   - `render.rs` matches item paths after layer ids and names.
   - The schema and description are updated, and `docs/cli.md` is regenerated.
   - Tests in `crates/pixellab/tests/render.rs`: `R19: hide takes an item path` and `R19: a layer name wins over a path`.
   - `cargo fmt --check`, `cargo test` and `npm test` pass.
6. **Hide and lock single items (R19, app).**
   - `LayerViewState.hiddenItems` and `lockedItems`, with their toggles, following `reordered` and dropped on a mismatch.
   - `renderParams` sends the hidden items, and `LayerTree` skips hidden and locked items when picking.
   - The `LayersPanel` item row gets the lock and eye buttons in place of its position.
   - Tests:
     - `R19: a hidden item isn't drawn or picked, and leaves the selection`
     - `R19: a locked item can't be picked on the stage but selects from the tree`
     - `R19: a hide follows a depth sort's reordered path`
     - `R19: a stale entry is dropped on reload`
7. **Edit Object and Back (R11).**
   - `editObject` and `backToScene` with `returnScene`.
   - `BackPill`.
   - Edit Object in `StageMenu` and the Library menu.
   - The menu items Edit › Edit Object (⌘↩) and View › Back to <scene> (⌘[).
   - The object view's ⌘] and ⌘[ stop stepping layers; plain ] and [ still do. Test: `R11: ⌘[ goes back and [ still steps the layer`.
   - Tests: `R11: Edit Object from a scene, then Back, returns to it`, `R11: Back with no scene to return to opens the last scene`.
8. **The window's chrome and the right panel (R12–R14).**
   - `DocumentToolbar`, `AgentToolbarButton` and the sidebar toggle are deleted, and the compact title bar is set.
   - The inspector is always presented, and `showInspector` and its menu item go.
   - `RightColumn`'s row gets the Agent badge, Publish and Save.
   - Tests: `R14: Save is disabled when clean and its label says so`, `R14: the Agent tab shows the waiting count`.
9. **The test hook (R17).**
   - The D10 steps and expects, and the `mode`, `sidebar` and `tool place` aliases.
   - `layers.txt` updated.
   - `window.txt` written and added to `acceptance.sh`, which runs on the temp site copy:
     - pinch in, out and keys, with `expect zoom` and `expect readout`;
     - `expect toolbar none`, `expect inspector shown` and `expect floatingbar select,pan,library`;
     - `library open`, `librarydrop pine 4 -1 1`, `expect at 4,-1,1`;
     - `editobject house`, `expect floatingbar pencil,eraser,fill,picker,pan`, then `back` and `expect returnscene hero-island`;
     - `hide` and `lock` on an item path, `expect hiddenitems`, a click that passes through the locked item, then `show` and `unlock`;
     - save, and the hero island's SVG byte-identical after publishing.
10. **Docs (R18).** The README tour and shortcuts table, then `npm run docs:site`.
11. **Snapshots (R16).** In dark and light: the scene stage with the bar, the open Library, the object view with the Back pill, the right panel's row, and Layers with a hidden and a locked item. Each is taken with `snapshot <png>` from a script on a temp document copy, then checked against the prototype boards.
12. **Full verification**, every check in Proof, pasted into the PR. Then open the PR and hand over.

## Workflow and reviews

| Step | Who | Where | Then |
|---|---|---|---|
| Merge PR #38 | The owner | `intent/floating-library` → `main` | The main session cuts `feat/floating-library` from `main` |
| 1–12 | The main session, back to back | `feat/floating-library`, in its own worktree (other sessions share the main checkout) | Opens the PR into `main` |
| Review | The owner | The PR: snapshots, the pasted checks, and an installed build (`scripts/install.sh`) to try pinch and drag on the trackpad | Feedback becomes commits on the same branch. The owner merges |

- **Runners and merging:**
  - No subagents. One runner keeps `StageView`, `AppModel` and `ScriptRunner` coherent.
  - If `main` moves, the main session merges it into `feat/floating-library` before the next step.
  - Only the owner merges into `main`.
- **Owner review:** one, at the PR. How pinch and drag feel on a real trackpad can't be checked by a script. The look is set by the approved prototype.
- **Independent review:** none.
  - The change doesn't touch what's written to the site, proposals or approval.
  - Its one change to the MCP surface is a read-only render parameter that writes nothing (step 5), covered by its tests and `docs/cli.md`.
  - The acceptance pass checks that the published art is byte-identical.

## Risks
- **⌘[ already had a job in objects.**
  - ] and [ step layers in an object, and ⌘] and ⌘[ did too.
  - The owner chose (2026-10-22) that Back takes ⌘[ and layers keep plain ] and [.
  - Step 7 drops the ⌘ variants, step 10 updates the README row, and the R11 test pins both.
- **The riskiest step is pinch zoom anchored under the pointer** (step 1).
  - The stage's content size changes with the zoom. Its `ScrollView` is SwiftUI on top of `NSScrollView`, so the clip view's origin has to be set after layout. Otherwise the art jumps.
  - Handled by setting it on the next layout pass from the anchor's scene point, which `StageGeometry` maps both ways. The R1 test checks the anchor stays within a pixel.
  - If it can't be made reliable, the step falls back to centering on the stage, and the plan is updated.
- **Magnify events and the art at whole zoom.** The stage never scales the image mid-pinch (the spec says no smooth zoom), so there's no blurry frame. The accumulators reset at the gesture's end (`event.phase`), so one pinch can't run away across many levels.
- **Space and the Hand in objects.** The Hand button's held pan has to stop when another tool is picked, or the pencil stops painting. Covered by a test in step 3.
- **Esc has several jobs.** On the stage it already clears the selection and leaves a layer, and it stops placing. With the popover open, it closes the popover only after the stage has nothing else to undo. The order is tested in step 4.
- **Drag from the popover.** The popover is an in-window overlay, not an `NSPopover`, so the existing `onDrag` and the stage's drop keep working (D5). Nothing new in the drag path.
- **The test hook and acceptance.** The aliases keep the existing scripts running. `layers.txt` is the only one whose expects change. A failing `expect` about the art means the code is wrong, not the script (`CLAUDE.md`), unless the site's art changed.
- **The Agent window's ⌘↩** (Approve) and Edit › Edit Object (⌘↩) live in different windows. The menu item is enabled only when the main window is key and a scene item is selected. Checked in step 7 with `agentkey cmd+return` in an existing chat script.
- **The Command Line Tools build.** No `@State`: all new state lives on `AppModel` or the editors.
- **Hidden items are keyed by path** (D12).
  - A path moves when an edit or depth sort reorders items. The app's own edits report that in `reordered`, and the entry follows it.
  - Outside changes can't be followed, so an entry whose path no longer holds its object is dropped.
  - Tested in step 6. The rare case of an outside edit that swaps two items of the same object is accepted (spec, Areas of concern).
- **A layer and an item path with the same text in `hide`.** The layer wins, and the test in step 5 pins it. Layer names are words; item paths are numbers with dots.
- **Options not chosen:**
  - An `NSPopover` for the Library: it closes on a drag out, and doesn't take board 03's look.
  - Smooth zoom: it breaks whole-zoom pixel art.
  - Splitting the work into app slices: they'd share `StageView`, `AppModel` and `ScriptRunner`.

## Proof
Run from the `feat/floating-library` worktree, with the output pasted into the PR:
- `swift build --package-path app` with no errors, and `scripts/test-app.sh` with every test passing, including the new `R1`–`R14` tests in `CanvasTests.swift` and the `R19` tests in `LayerTests.swift`.
- `cargo fmt --check`, `cargo test` with no failures (including the R19 render tests), and `npm test` with `# fail 0`.
- `scripts/build-app.sh`, then `scripts/acceptance.sh --only window,scene-object,canvas,layers,document,agent,mcp-agent,stale,stale-agent,chat,chat-harbor,examples-harbor,examples-town` exits 0 (the stages this change touches, and the ones whose script changed), with `window.txt` in the log ending "all steps passed" and the hero island's SVG byte-identical.
- `npm run docs:site` builds with no broken links, and `npm run docs:cli` leaves `git status` clean apart from the committed `scene.render` change.
- Snapshots in dark and light of the four views in step 9, compared with the prototype boards `Groups.dc.html`, `Object.dc.html` and `AgentChat.dc.html`, and attached to the PR.

## Departures
What was built differently from the plan above (the order of the steps held; one commit each, apart from the docs commit landing before the test hook's):
- **The acceptance stage is `window`,** not `canvas`: `scripts/acceptance/canvas.txt` is the scene-canvas change's stage. Verification follows `CLAUDE.md` on `main`: `scripts/acceptance.sh --only` with the stages in Proof.
- **Pinch and the scene workspace (step 1).** The anchor reuses the scene canvas's `keepScroll`: a zoom step records the scene point under the pointer (or the middle of the clip) before the zoom changes, and `keepScroll` puts it back after layout. A workspace that grows later keeps the visible middle, as it did. Nothing fell back to centering. `pinch`, `zoomkey` and `expect readout|anchor` landed in step 1, not 9, so the step could be checked.
- **The zoom is seamless (owner, 2026-10-22, after trying the build).** R1 and R3 said a pinch steps through whole levels and art is always drawn at a whole zoom; the owner found that off and asked for a seamless zoom. `Zoom` gains `.scale(Double)`, and a pinch (and a trackpad's ⌘-scroll) multiplies the zoom by each event's magnification, within the kind's first and last level (1×–4× scenes, 4×–16× objects: the range is unchanged). Between levels the art is nearest-neighbor at a fractional scale, so pixels are uneven widths; ⌘= and ⌘− go to the next whole level, ⌘0–⌘4 are as they were. VoiceOver hears the zoom once, when the gesture ends. The spec's R1, R2 and D2 are superseded by this.
- **The pinch is the scroll view's own, and there's no readout (owner, same day).** The first seamless version re-laid the stage out on every magnify event and shook. The pinch is now `NSScrollView`'s native magnification (`allowsMagnification`, limited to the kind's range): the stage scales on the GPU about the fingers, with nearest-neighbor layers, and when they lift `commitMagnification` turns the magnification into the zoom, lays the stage out crisply and keeps the top-left of what was visible. ⌘-scroll on a trackpad uses the same magnification, committed when the scrolling pauses. The zoom readout (R3) is removed. `pinchby <factor>` drives the native path in the script hook.
- **`tool place` fails** with a pointer to `library <name>` (spec D10), rather than being an alias; `scripts/acceptance/layers.txt` changed in step 4, where `sidebarTab` went (`expect tab library` → `expect library open`, `sidebar layers` → `library close`, and the `expect tool place` line was dropped).
- **A drop does not start placing.** Placing is `placeObject != nil`, and a drop used to leave its object chosen; it now places one and leaves the state alone.
- **Esc order:** a drag, the layer menu, the selection, the layer you're inside, then placing, then the Library.
- **Search finds objects.** With the Library gone from the sidebar, a sidebar search lists matching objects under Scenes, and a row opens one, so "opening an object from Search" (R11) still works.
- **`editObject` and `returnScene` came in step 4** (the popover's double-click needed them); step 7 added Back, its pill and the menus. `returnScene` is cleared when any scene opens.
- **Edit › Edit Object (⌘↩)** is enabled only while the main window is key, so the Agent window's Approve (⌘↩) keeps its key; the `agent` stage's `agentkey cmd+return` checks it. The Show/Hide Inspector menu item went with step 7's menu edits, not step 8's.
- **⌘] and ⌘[ never stepped layers in code** (the object editor ignores ⌘ keys); only the README said so. Back takes ⌘[ as a menu shortcut and the README row is corrected.
- **The sidebar toggle** is removed with `.toolbar(removing: .sidebarToggle)` on the sidebar column; on the split view it left the button in place.
- **Snapshots** are in `snapshots/` beside this plan, made by `snapshots/snapshots.txt` on a temp copy of the harbor example. The Agent tab isn't among them: showing it starts the chat, and the fake Claude Code is for the acceptance stages.
- **A zoom keeps the view where it was.** The stage's scroll view centered itself again whenever its content changed size (`defaultScrollAnchor(.center)` also applies to size changes). It now centers only when a doc opens (`for: .initialOffset`), and the scroll is put back three times after a zoom (at once, 50 ms and 200 ms later), since SwiftUI settles a resized scroll view on its own schedule.
