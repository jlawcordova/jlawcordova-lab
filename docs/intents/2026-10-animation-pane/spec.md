# Spec: Edit the timeline's animation key by key (from intent.md 2026-10-20)
Status: approved (reviewed with its intent, plan and code in one PR, at the owner's request).

## Requirements

**The pane**

- **R1. An animation pane under the stage.** The timeline strip's tracks list becomes a dope sheet: the scrubber is the time ruler, and under it one row per motion track, named by the item's id, with the track's keys drawn as diamonds on the ruler's scale and the gold playhead crossing every row. Show Tracks shows and hides it (remembered between launches); it opens on its own when an item is animated from the app or a key is selected from the stage. Clicking a row selects its item on the stage, and the selected item's row is highlighted, so the selection and the row always agree.
- **R2. Clicking a key selects it.** The selected key is drawn larger, in the accent; the playhead moves to its time, so the stage shows the item at that key, and the key's item becomes the selected item. A key that hides the item (`show: false`) is hollow; a key with a pose carries a dot. Esc, clicking the row away from a key, or selecting another item deselects the key.
- **R3. Making a key.** Add Key (the pane's + button, K with the pane focused, Edit › Add Key, ⌥⌘K) makes a key at the playhead for the selected item's track and selects it. The key pins the item where it is at that time (its move at that time, which `scene.key.set` with no values now does), so the item's motion doesn't change until the key is edited. At time 0, or at a time that already has a key, Add Key selects the key there.
- **R4. Deleting a key.** Delete Key (the pane's − button, ⌫ with the pane focused and a key selected, Edit › Delete Key, ⌥⌘⌫) deletes the selected key. The key at 0 is the rest frame and can't be deleted: the button is disabled with a hint to remove the track instead.
- **R5. Dragging a key moves it in time,** snapped to the timeline's 100 ms steps, with a ghost of the key while it drags; the key at 0 doesn't drag, and a drop on another key's time is refused and leaves the key where it was. The pane's inspector field sets an exact time.
- **R6. Moving the item on the stage past 0 keys it,** as before (`scene.move` with the time), and the key it made or moved becomes the selected key.

**The key's values**

- **R7. A KEY section in the inspector** shows the selected key: its time, Move X and Y, Show and Pose. Each value is either set on the key or inherited from the key before (move [0, 0], shown, the item's own object at the start). Inherited values show dimmed with "inherits"; setting one keys it, Clear takes it off the key again. Pose offers every object in the document. Time moves the key (as R5). Each edit is one undo step through `scene.key.set`, `scene.key.move` or `scene.key.delete`, and the track's off-grid warnings, when it has any, show under the values in gold.

**Tracks and the loop**

- **R8. Animating an item.** For a selected item with an id and no track, the ANIMATION section (in the inspector, under POSITION) offers Animate, which adds a track with its rest key and opens the pane; a scene without motion gets a 4 s loop. An item without an id gets the ID field's hint instead ("Give it an ID to animate it") and no button. For an animated item the section says how many keys it has and offers Remove Track, which drops the track and its keys (one undo step).
- **R9. The loop's length** is a field in the pane's header (seconds, one decimal, as the time is shown). A length that would cut off keys is refused with the server's reason, and the field returns to the loop's length.

**Keyboard, menus, access**

- **R10. The pane takes keys when focused** (click in it, or Tab to it; its focus ring is visible): ← and → step the playhead 100 ms, ⇧← and ⇧→ go to the selected track's previous or next key (and select it), K adds a key, ⌫ deletes the selected key, Space plays or pauses, Esc deselects the key. The Edit menu gets Add Key (⌥⌘K), Delete Key (⌥⌘⌫), Previous Key (⌥⌘←), Next Key (⌥⌘→), and Animate Item / Remove Track, enabled as the pane's buttons are. The stage keeps its own keys: ⌫ on the stage still removes the item.
- **R11. Accessibility.** VoiceOver reads each key ("Key at 4.2 s: move 32, 16, hidden, pose crane-hook-down"), each row ("Track cart, 5 keys"), the ruler's value, and every button; the pane's focus is visible; keys are at least 12 pt targets with a 6 pt hit margin; nothing in the pane animates (playback is the timeline's, and stays off under Reduce Motion).

**Doors, tests, docs**

- **R12. `scene.key.set` without values pins the item** at that time, in all three doors, and its result says so. The other motion commands are unchanged.
- **R13. The script hook drives the pane:** actions for keys, tracks, the loop and the pane's keys, with `expect`s for the selected key, its values, the key count, the loop and the pane's state; a unit test for the pane's pure logic (key hit-testing, snapping, inherited values); an acceptance script that keys the hero island and leaves it unsaved.
- **R14. Docs:** the README's tour and shortcuts, `CLAUDE.md`'s script-hook list, `docs/testing.md`, `app/README.md`, `docs/cli.md` regenerated, and a follow-ups note in the record.

## Design

### D1. `scene.key.set` with no values (R3, R12)

`scene.key.set --time t` with none of `move`, `show`, `pose` or `clear` pins the item: when there's no key at `t`, the command computes the item's move at `t` with the engine (`stateAt(track, item.object, duration, t).move`) and writes a key `{ t, move }`. Its result adds `pinned: true` and its message reads "Made <id>'s key at <t> ms, pinned at <x>, <y>". When a key is already there, nothing changes (`changed: false`, "<id> already has a key at <t> ms") and the undo stack is left alone. The command's description says so; `needOne` is no longer applied.

### D2. The timeline model (R1–R6, R8–R10)

`MotionTrack` grows to `keys: [MotionKey]` with `MotionKey { t: Int, move: [Int]?, show: Bool?, pose: String? }` decoded from `scene.show --doc` (which already returns the whole `motion`). `MotionKey.filled(in track)` gives the inherited values (the carry-forward of `filledKeys` in `motion.mjs`, without the stepping). `TimelineModel` gains:

- `selectedKey: SelectedKey?` (`item`, `t`), `showTracks` persisted in `UserDefaults` (`SettingsKey.showTracks`), `paneFocused`, `keyDrag: KeyDrag?` (`item`, `from`, `to`), `lastWarnings: [String]` (the warnings of the last key edit, cleared when the doc changes).
- `selectTrack(item)`: selects the item on the stage (via `SceneEditor.select(id:)`); `selectKey(item, t)`: sets `selectedKey`, scrubs to `t`, selects the item; `deselectKey()`.
- `addKey()`, `deleteKey()`, `moveKey(to:)`, `setKey(move:show:pose:clear:)`, `addTrack()`, `removeTrack()`, `setDuration(ms)`, `stepKey(+1 | -1)`, `stepTime(±100)`: each a `scene.*` call through `SceneEditor.edit` (one undo step each, the item's path and the scene's name filled in). Results are `EditResult` with new optional fields `key` (`{ t, move?, show?, pose? }`), `warnings: [String]` and `pinned`.
- `load(_:)` keeps `selectedKey` when the key still exists (same item and time) and drops it otherwise; after `scene.move` past 0 the model selects the key at the render time (R6).
- `keyAt(x, in width, track)` and `snap(ms)` are static, pure functions for the pane and its unit test: a key hits within 6 pt of its diamond; snapping rounds to 100 ms and clamps to `[1, duration − 1]`.

### D3. The pane (R1, R2, R5, R10, R11)

`TimelineView.swift`: `TracksList` is replaced by `AnimationPane`, 160 pt tall, scrolling vertically when the tracks don't fit, with the header row above it unchanged except for the loop field and two buttons:

- Header (right end, where the time reads): the time, then `Loop` (a `TextField` of the length in seconds, one decimal; Return sets it), then **+** and **−** (Add Key, Delete Key, `.borderless`, with help) and the tracks toggle.
- Each row: the id in SF Mono 10.5 at `TimelineStrip.leading` width (selected: ink; else ink-muted), then a `Canvas` the scrubber's width drawing the baseline, the keys (diamonds 9 pt across in the accent for the selected track and ink-muted at 70% for others; the selected key 13 pt across with a halo; hollow when `show == false`; a 3 pt dot above for a pose), the drag ghost (the same diamond at 50%), and the playhead; then the key count at `TimelineStrip.trailing`.
- Gestures on the canvas: a `DragGesture(minimumDistance: 0)`; on its first change the pane takes focus, hit-tests the key under the start (`keyAt`), and either starts a key drag (not the key at 0) or marks a click. On end, a click selects the key hit (or, with none, selects the row's item and deselects the key); a drag with movement calls `moveKey(to: snap(ms))` and clears the ghost.
- Keys: the pane is `.focusable()` with `@FocusState`, `.focusEffectDisabled()` and its own 2 pt accent ring at 55% (as the stage's), `.onKeyPress` for ←, →, ⇧←, ⇧→, K, ⌫, Space and Esc (R10). `TimelineModel.handleKey(KeyInput)` holds the logic so the script hook can send the same keys.
- Accessibility: each row is an element with the label of R11 and `isSelected`; keys are reached through the row's `accessibilityChildren`, each a button that selects it; the buttons have labels and help.

### D4. The inspector (R7, R8)

`KeyInspector` (new file `app/Sources/PixelLab/Views/KeyInspector.swift`): `InspectorSection("Key")` shown under POSITION when `selectedKey` belongs to the selected item.

- Time: a `CoordField`-style number field in ms (Return: `moveKey(to:)`); the key at 0 shows "0 (rest frame)" static.
- Move X, Y: two fields; when the key has no move they show the inherited value dimmed with a trailing "inherits" caption; typing sets `move` (both values, the other from the inherited move); a Clear button (×, `.borderless`, "Inherit the move from the key before") clears it. Hidden for the key at 0 when its move is unset? No: the rest frame's move can be set too (`{ t: 0, move: [-48, 24] }` exists in the island), so the same fields show.
- Show: a `Picker` (menu style) with Inherit, Shown, Hidden.
- Pose: a `Picker` with Inherit and every object name (sorted), with a small thumbnail of the pose next to it.
- Warnings: the track's off-grid warnings from the last result, in `.caption`, gold triangle, as CHECKS draws them, trimmed of the file prefix.

`AnimationSection` (same file): `InspectorSection("Animation")` under POSITION for a selected item: no id → "Give it an ID to animate it"; id, no track → **Animate** (`ActionButton`, "Add a motion track: a rest key at 0; a 4 s loop if the scene has none"); a track → "<n> keys" and **Remove Track**.

### D5. Menus and the script hook (R10, R13)

`PixelLabCommands`: after Select None, a divider, then Add Key (⌥⌘K), Delete Key (⌥⌘⌫), Previous Key (⌥⌘←), Next Key (⌥⌘→), Animate Item / Remove Track (one button whose title follows the selection); all disabled outside scene mode or without what they need.

Script hook actions: `key add|delete|next|prev`, `key select <ms>|none`, `key move <ms> <to>` (a drag), `keyset time <ms>|move <x,y>|move clear|show on|off|clear|pose <object>|clear`, `track add|remove`, `loop <ms>`, `panekey <spec>` (a key into the focused pane: `left`, `shift+right`, `k`, `delete`, `esc`, `space`); `expect key <ms>|none`, `expect keys <n>` (the selected track's), `expect keymove x,y|inherits`, `expect keyshow true|false|inherits`, `expect keypose <object>|inherits`, `expect loop <ms>`, `expect tracks <n>`, `expect pane on|off`.

### D6. Docs (R14)

README: the tour's timeline sentence becomes a paragraph on the pane; the shortcuts table gets the pane's keys and the menu shortcuts. `CLAUDE.md`'s script-hook list, `docs/testing.md`'s action list and `app/README.md`'s view and model tables name the pane. The record's follow-ups note that the tracks list is now the pane. `scripts/acceptance/animation.txt` is added to `scripts/acceptance.sh`.

## Areas of concern

- **Clicking a key moves the playhead.** Unity selects a key without moving the playhead (a double-click does); Rive and most dope sheets move it. Moving it here is what makes the stage useful for editing the key (arrow keys on the stage then move that key). The intent left it open; this spec chooses to move it. If the owner prefers Unity's way, R2 changes to "⌥-click also moves the playhead".
- **Pinning on Add Key changes a track's shape without changing its motion.** A pinned key holds the item's current stepped position, so a later edit of the keys around it leaves a visible stop there. That's what every keyframe editor does, and it's what "make a key frame at the playhead" means; the alternative, an empty key, isn't something `scene.key.set` can write today and would be invisible on the stage.
- **The 4 s default loop** for a scene without motion is this spec's number; the owner asked which to use. The loop field changes it at once.
- **Focus in SwiftUI.** The pane uses `.focusable()` and `.onKeyPress`, which the stage avoids in favour of an AppKit view. If focus proves unreliable in testing, the pane gets a small `NSView` key catcher like the stage's; the model's `handleKey` is the same either way.
- **Many tracks.** The hero island has over a dozen tracks; the pane scrolls at 160 pt and the selected row is scrolled into view when the selection changes.

## Open questions

- Click selects and moves the playhead (decided, above).
- The default loop is 4 s (decided, above; the owner can say otherwise).
