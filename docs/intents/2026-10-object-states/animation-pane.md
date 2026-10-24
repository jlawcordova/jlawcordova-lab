# Notes: the animation pane, folded into object states

On 2026-10-20 the owner asked for an animation pane: a way to edit the timeline key by key. It was built as its own change, with its intent, spec, plan and code in one PR (`jlawcordova/jlawcordova-lab#17`, branch `feat/animation-pane`). That PR never merged. On 2026-10-24 the owner folded it into this change (`intent.md`), because editing an object's states needs the same timeline.

These notes keep what that work decided and found, so the spec can build on it. The PR's code isn't reused as it is. Its program side was written against the Node CLI (`cli/lib/motion-ops.mjs`, `cli/lib/commands/scene-motion.mjs`) before the Rust port. Its app side came before the scene canvas and the canvas-first window. Read the branch for how it worked, then rebuild it on today's code.

## What the pane did
- **Rows and keys.** It turned the timeline strip's track list into a dope sheet. The scrubber was the time ruler, with one row per motion track named by the item's id. Keys showed as diamonds: hollow where a key hides the item, with a dot where it poses. The gold playhead crossed every row. Clicking a row selected its item, and the selected item's row was highlighted.
- **Selecting a key.** Clicking a key selected it, drew it larger in the accent, moved the playhead to its time and selected its item. Esc deselected it.
- **Adding and deleting.** Add Key (+, K, Edit › Add Key, ⌥⌘K) keyed the selected item at the playhead, pinned where it was, so its motion didn't change until the key was edited. Delete Key (−, ⌫, ⌥⌘⌫) deleted the selected key. The key at 0 is the rest frame and couldn't go; Remove Track does that.
- **Moving in time.** Dragging a key moved it in time, snapped to 100 ms. The key at 0 didn't drag, and a drop on another key's time was refused.
- **KEY in the inspector.** It showed the selected key's time, Move X and Y, Show and Pose. Each value was either set on the key or inherited from the key before: dimmed, with Clear to inherit again. The track's off-grid warnings showed in gold.
- **ANIMATION in the inspector.** Animate added a track to an item with an id. Remove Track dropped one. The pane's Loop field set the loop's length, and a length that cut off keys was refused.
- **The stage.** Moving an item on the stage past 0 keyed it as before, and selected that key.
- **Keyboard.** With the pane focused: ← → stepped the playhead, ⇧← ⇧→ walked the keys, and K, ⌫, Space and Esc worked as above. The Edit menu had Previous and Next Key (⌥⌘← and ⌥⌘→).
- **The program.** `scene.key.set` with no values pinned the item at that time, in all three doors.

## What it decided, and left for the owner
- **Clicking a key moves the playhead** (Rive's way). Unity selects without moving. If the owner prefers Unity's way, ⌥-click would be the one that moves. This is an open question in `intent.md`.
- **A scene without motion gets a 4 s loop** when its first item is animated from the app. This is also an open question in `intent.md`.

## What it found
- **Pinning on Add Key.** It changes a track's shape without changing its motion: a later edit of the keys around a pinned key leaves a visible stop there. Every keyframe editor works this way, and an empty key would be invisible on the stage.
- **Focus.** The pane used SwiftUI's `.focusable()` and `.onKeyPress`, where the stage uses an AppKit key view. If focus proves unreliable, give the pane a small `NSView` key catcher like the stage's.
- **Many tracks.** The hero island has dozens of tracks (39 on 2026-10-20). The pane scrolled at 160 pt and kept the selected row in view.
- **Snapshots.** In the day snapshot, the Show and Pose pickers looked pale. That is how a snapshot draws a popup button in a window that isn't key, not a theme problem.
- **Tests.** The pane's pure logic (hitting a key, snapping, inherited values) was unit-tested. An acceptance script, `scripts/acceptance/animation.txt` on that branch, keyed the hero island's cart and left the document unsaved. Since autosave, edits save themselves, so a new script has to work on a temp copy it can throw away.
