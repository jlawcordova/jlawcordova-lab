# Plan: A canvas-first window, with a floating library (from intent.md 2026-10-21)

The intent, spec and plan merge to `main` together in PR #38 (`intent/floating-library`). The code then starts on `feat/floating-library`, cut from `main` after that merge.

The change is to the Mac app only (`app/`), its test hook and acceptance scripts, and the README. Nothing in `crates/`, `docs/cli.md`, the document format, the CLI or MCP changes.

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

**Tests and the test hook**
- `app/Sources/PixelLab/Support/ScriptRunner.swift`: the steps and expects in D10, plus aliases for `mode`, `sidebar` and `tool place`.
- `app/Tests/PixelLabTests/CanvasTests.swift` (new): Swift Testing suites named by requirement (`R1: …`).
- `app/Tests/PixelLabTests/StageTests.swift`: tests that used the Place tool or `sidebarTab` updated.
- `scripts/acceptance/canvas.txt` (new): the end-to-end run of the new window on the temp site copy.
- `scripts/acceptance/layers.txt`: `expect tab library` and `sidebar layers` become `expect library open` and `library close`.
- `scripts/acceptance.sh`: runs `canvas.txt` after `layers.txt`, and its header comment mentions it.

**Docs**
- `README.md`: the scene, object, publish and Agent paragraphs of the tour, and the shortcuts table (D11).

## Order of work

There's one slice: one branch and one PR into `main`. The steps form one chain. The window chrome, the bar, the Library and Back all touch `StageView`, `AppModel` and `ScriptRunner`, so parallel runners would fight over the same files. The work fits one sitting.

```
main ── PR #38 (intent, spec, plan; owner merges)
  └── feat/floating-library ── steps 1–10 ── PR "A canvas-first window" (owner merges) ──▶ main
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
5. **Edit Object and Back (R11).**
   - `editObject` and `backToScene` with `returnScene`.
   - `BackPill`.
   - Edit Object in `StageMenu` and the Library menu.
   - The menu items Edit › Edit Object (⌘↩) and View › Back to <scene> (⌘[).
   - Before this step, settle the ⌘[ clash (Risks).
   - Tests: `R11: Edit Object from a scene, then Back, returns to it`, `R11: Back with no scene to return to opens the last scene`.
6. **The window's chrome and the right panel (R12–R14).**
   - `DocumentToolbar`, `AgentToolbarButton` and the sidebar toggle are deleted, and the compact title bar is set.
   - The inspector is always presented, and `showInspector` and its menu item go.
   - `RightColumn`'s row gets the Agent badge, Publish and Save.
   - Tests: `R14: Save is disabled when clean and its label says so`, `R14: the Agent tab shows the waiting count`.
7. **The test hook (R17).**
   - The D10 steps and expects, and the `mode`, `sidebar` and `tool place` aliases.
   - `layers.txt` updated.
   - `canvas.txt` written and added to `acceptance.sh`, which runs on the temp site copy:
     - pinch in, out and keys, with `expect zoom` and `expect readout`;
     - `expect toolbar none`, `expect inspector shown` and `expect floatingbar select,pan,library`;
     - `library open`, `librarydrop pine 4 -1 1`, `expect at 4,-1,1`;
     - `editobject house`, `expect floatingbar pencil,eraser,fill,picker,pan`, then `back` and `expect returnscene hero-island`;
     - save, and the hero island's SVG byte-identical after publishing.
8. **Docs (R18).** The README tour and shortcuts table, then `npm run docs:site`.
9. **Snapshots (R16).** In dark and light: the scene stage with the bar, the open Library, the object view with the Back pill, and the right panel's row. Each is taken with `snapshot <png>` from a script on a temp document copy, then checked against the prototype boards.
10. **Full verification**, every check in Proof, pasted into the PR. Then open the PR and hand over.

## Workflow and reviews

| Step | Who | Where | Then |
|---|---|---|---|
| Merge PR #38 | The owner | `intent/floating-library` → `main` | The main session cuts `feat/floating-library` from `main` |
| Settle the ⌘[ clash | The owner, asked by the main session | chat | Step 5 follows their answer |
| 1–10 | The main session, back to back | `feat/floating-library`, in its own worktree (other sessions share the main checkout) | Opens the PR into `main` |
| Review | The owner | The PR: snapshots, the pasted checks, and an installed build (`scripts/install.sh`) to try pinch and drag on the trackpad | Feedback becomes commits on the same branch. The owner merges |

- **Runners and merging:**
  - No subagents. One runner keeps `StageView`, `AppModel` and `ScriptRunner` coherent.
  - If `main` moves, the main session merges it into `feat/floating-library` before the next step.
  - Only the owner merges into `main`.
- **Owner review:** one, at the PR. How pinch and drag feel on a real trackpad can't be checked by a script. The look is set by the approved prototype.
- **Independent review:** none. The change doesn't touch what's written to the site, proposals, approval or the MCP surface. The acceptance pass checks that the published art is byte-identical.

## Risks
- **⌘[ already has a job in objects.**
  - Today ] and [ step layers in an object, and the README lists ⌘] and ⌘[ as the same. A menu item on ⌘[ (spec D9, R11) would take ⌘[ before the stage sees it, in exactly the view where Back applies.
  - The main session asks the owner before step 5, with three options:
    - (a) Back takes ⌘[, and layers keep ] and [ without ⌘. The README row changes.
    - (b) Back uses another key.
    - (c) Back keeps the keyboard path through Esc in the object view.
  - The recommendation is (a). If the answer changes the spec, the spec is updated in the same commit as step 5.
- **The riskiest step is pinch zoom anchored under the pointer** (step 1).
  - The stage's content size changes with the zoom. Its `ScrollView` is SwiftUI on top of `NSScrollView`, so the clip view's origin has to be set after layout. Otherwise the art jumps.
  - Handled by setting it on the next layout pass from the anchor's scene point, which `StageGeometry` maps both ways. The R1 test checks the anchor stays within a pixel.
  - If it can't be made reliable, the step falls back to centering on the stage, and the plan is updated.
- **Magnify events and the art at whole zoom.** The stage never scales the image mid-pinch (the spec says no smooth zoom), so there's no blurry frame. The accumulators reset at the gesture's end (`event.phase`), so one pinch can't run away across many levels.
- **Space and the Hand in objects.** The Hand button's held pan has to stop when another tool is picked, or the pencil stops painting. Covered by a test in step 3.
- **Esc has several jobs.** On the stage it already clears the selection and leaves a layer, and it stops placing. With the popover open, it closes the popover only after the stage has nothing else to undo. The order is tested in step 4.
- **Drag from the popover.** The popover is an in-window overlay, not an `NSPopover`, so the existing `onDrag` and the stage's drop keep working (D5). Nothing new in the drag path.
- **The test hook and acceptance.** The aliases keep the existing scripts running. `layers.txt` is the only one whose expects change. A failing `expect` about the art means the code is wrong, not the script (`CLAUDE.md`), unless the site's art changed.
- **The Agent window's ⌘↩** (Approve) and Edit › Edit Object (⌘↩) live in different windows. The menu item is enabled only when the main window is key and a scene item is selected. Checked in step 5 with `agentkey cmd+return` in an existing chat script.
- **The Command Line Tools build.** No `@State`: all new state lives on `AppModel` or the editors.
- **Options not chosen:**
  - An `NSPopover` for the Library: it closes on a drag out, and doesn't take board 03's look.
  - Smooth zoom: it breaks whole-zoom pixel art.
  - Splitting the work into app slices: they'd share `StageView`, `AppModel` and `ScriptRunner`.

## Proof
Run from the `feat/floating-library` worktree, with the output pasted into the PR:
- `swift build --package-path app` with no errors, and `scripts/test-app.sh` with every test passing, including the new `R1`–`R14` tests in `CanvasTests.swift`.
- `cargo fmt --check`, `cargo test` with no failures, and `npm test` with `# fail 0`. Nothing in `crates/` changes, but `CLAUDE.md` asks for them.
- `scripts/build-app.sh`, then `scripts/acceptance.sh` exits 0, with `canvas.txt` in the log ending "all steps passed" and the hero island's SVG byte-identical.
- `npm run docs:site` builds with no broken links, and `npm run docs:cli` leaves `git status` clean (no command changed).
- Snapshots in dark and light of the four views in step 9, compared with the prototype boards `Groups.dc.html`, `Object.dc.html` and `AgentChat.dc.html`, and attached to the PR.
