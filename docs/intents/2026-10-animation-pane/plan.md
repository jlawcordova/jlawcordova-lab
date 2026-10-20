# Plan: Edit the timeline's animation key by key (from intent.md 2026-10-20)

## Files that change

CLI:
- `cli/lib/motion-ops.mjs`: `setKey` takes a `pin` move (the item's move at `t`) when no values are given; `changed: false` for a key already there.
- `cli/lib/commands/scene-motion.mjs`: `scene.key.set` without values pins (D1); its description.
- `cli/test/motion-commands.test.mjs`: R12 tests.
- `docs/cli.md` (generated).

App:
- `app/Sources/PixelLab/Server/Protocol.swift`: `MotionKey`, `MotionTrack.keys`, `SceneShowMotion` decoding move, show and pose; `EditResult.key`, `.warnings`, `.pinned`.
- `app/Sources/PixelLab/Model/TimelineModel.swift`: the selected key, the pane's state, the key, track and loop edits, `handleKey`, the pure helpers (D2).
- `app/Sources/PixelLab/Model/SceneEditor.swift`: `select` deselects the key when the item changes; `move`/`nudge` past 0 select the key they touched.
- `app/Sources/PixelLab/Model/Settings.swift`: `SettingsKey.showTracks`.
- `app/Sources/PixelLab/Views/TimelineView.swift`: the header's loop field and key buttons; `AnimationPane` replaces `TracksList` (D3).
- `app/Sources/PixelLab/Views/KeyInspector.swift` (new): `KeyInspector`, `AnimationSection` (D4).
- `app/Sources/PixelLab/Views/InspectorView.swift`: shows them.
- `app/Sources/PixelLab/PixelLabApp.swift`: the Edit menu items (D5).
- `app/Sources/PixelLab/Support/ScriptRunner.swift`: the actions and expects (D5).
- `app/Tests/PixelLabTests/TimelineTests.swift` (new): key hit-testing, snapping, inherited values, `handleKey`.
- `scripts/acceptance/animation.txt` (new), `scripts/acceptance.sh`.

Docs:
- `README.md`, `CLAUDE.md`, `docs/testing.md`, `app/README.md`, `docs/intents/2026-10-mac-app/plan.md` (a follow-ups line), `docs/screenshots/animation-pane-{dark,light}.png` (new).

## Order of work

One slice, on `feat/animation-pane`, straight to a PR into `main`: the main session does every step in one sitting, and the app steps build on the one CLI change, so splitting buys nothing.

1. This intent, spec and plan.
2. `scene.key.set` pins without values (R12), with tests; `npm run docs:cli`.
3. The protocol and the timeline model: full keys, the selected key, the edits, `handleKey`, the helpers (R2–R6, R8–R10), with `TimelineTests`.
4. The pane: rows, keys, drag, focus and keys, the header's loop field and buttons (R1, R2, R5, R9–R11).
5. The inspector's KEY and ANIMATION sections and the Edit menu (R7, R8, R10).
6. The script hook's actions and expects, the acceptance script (R13).
7. Docs, snapshots in dark and light (R14).
8. The full verification (Proof), pasted into the PR.

## Workflow and reviews

The main session runs all eight steps on `feat/animation-pane`, back to back, one commit each, and opens one PR into `main` with the build attached as the owner asked; only the owner merges it. No subagents.

Owner review: at the PR, with the dark and light snapshots of the pane and the KEY section, and the build to try (the feel of clicking and dragging keys is what the owner judges). The spec's Areas of concern (click moves the playhead; the 4 s default loop) are the two things to confirm or change.

Independent review: none. The change writes nothing new to the document, only calls the existing motion commands; the script hook and tests cover it.

## Risks

- **SwiftUI focus in the pane** (`.focusable()`, `.onKeyPress`) may not take clicks or may fight the stage's first responder. Handled: the model's `handleKey` is independent of the view; if focus misbehaves, the pane gets an `NSView` key catcher like `StageInputView` in the same step. The riskiest step is 4.
- **Key selection going stale** after an edit, undo or a reload: `load` drops a selected key that no longer exists, and `docChanged` reloads the motion whenever the scene has tracks (it already does).
- **The stage's ⌫** removes the item; the pane's ⌫ deletes the key. They're separate responders, and the menu item for Delete Key is named so it can't be mistaken for Remove Item.
- **The acceptance pass** pins the island's art; `animation.txt` uses an item that exists today (`cart`, keys at 0, 7200, 10400, …) and ends with Revert, so later scripts see the file unchanged. If the art changes, update its `expect` lines, not the app.
- **`scene.key.set` without values used to be a usage error.** Agents that relied on the error get a pinned key instead; the MCP description says so, and the result's `pinned` tells them.

## Proof

- `npm test` shows `# fail 0` (with the new R12 tests).
- `npm run docs:cli` leaves `git status` clean apart from this change.
- `swift build --package-path app` builds with no errors; `scripts/test-app.sh` passes, with `TimelineTests`.
- `scripts/build-app.sh` builds `build/Pixel Lab.app`.
- A script-hook run on a temp copy of the site, `scripts/acceptance/animation.txt`: select `cart`, open the pane, select the key at 7200 (`expect key 7200`, `expect time 7200`), Add Key at 8000 (`expect keys 12`, `expect keymove …`), set its move and show, move it to 8500, step to the next and previous keys, delete it, Animate the pine placed in the run and remove its track, set the loop and have a cutting length refused, pane keys, Revert; snapshots `animation-pane-dark.png` and `animation-pane-light.png`.
- `scripts/acceptance.sh` exits 0 with the new script in it.
- `npm run docs:site` builds with no broken links.
