# Spec: Object states and the animation pane (from intent.md 2026-10-24)
Status: draft.

This spec covers the intent as the owner revised it on 2026-10-24, with the animation pane folded in (`animation-pane.md`). It builds on what is in `main` today: autosave (`../2026-10-autosave/spec.md`: edits apply as they're made, MCP edits included, and every change is a version), the canvas-first window with the floating Library (`../2026-10-floating-library/spec.md`), the scene canvas (`../2026-10-scene-canvas/spec.md`), depth order (`../2026-10-depth-order/spec.md`) and the Rust engine's ambient loops and actions (`../2026-10-rust-engine/spec.md`, R12, R13, D6, D7).

It adds three things:
- **States.** A sprite can have named states. A state is a run of the sprite's frames at a frame rate, plus moves of its layers. It either loops, or plays once and then holds or hands over to another state. Every placement of the sprite shares its states, and each placed item is in its own state.
- **Switching states.** On the site, a page switches an item's state with a `data-state` attribute. In a scene, a motion key can change an item's state at a point in the scene's time, and that plays on the site by itself, the same every time.
- **The animation pane.** One pane under the stage edits both: a scene's tracks and keys in scene mode (the dope sheet the animation pane's spec designed, rebuilt on today's code), and an object's states in object mode, with a graph of how they lead into each other.

The game-engine models it borrows from:
- **Unity's Animator.** States hold clips, with a default (entry) state, and play once or loop. Transitions on "exit time" lead from one state to the next. Its Animation window is the dope sheet.
- **Unreal's animation state machines and Sequencer.** Unreal has a state graph with an entry and a montage-like one-shot, and Sequencer has an animation track with a labeled section per clip.
- **Aseprite's tags.** A named range of a sprite's frames, played forward at a frame rate.

What it leaves out on purpose: transitions on conditions or parameters (a page or a scene says which state to play, so nothing in the art decides), blending (pixel art steps), and curves (motion steps on the grid by design).

## Requirements

**States in the document**

- **R1. A sprite can have states.**
  - A state has a name, which follows the name rule (lowercase words joined by hyphens).
  - A state has a list of frame steps, each one a frame of the sprite, and one frame length in ms for every step.
  - A state either loops or plays once. A state that plays once either holds its last frame or hands over to another state (`then`).
  - A state may move any of the sprite's map layers, with keys `{ t, move }` over the state's length, stepping a whole pixel at a time, as actions do.
  - A sprite with states has a start state, which every placement plays unless something says otherwise.
  - A sprite has at most 32 states. A block can't have states.
- **R2. A frame is the same frame on every layer.** Frame `k` of a sprite is frame `k` of each frame-loop layer, with the map layers drawn as they are. In a sprite with states, every frame-loop layer has the same number of frames, and `check` reports one that doesn't.
- **R3. A sprite without states plays as it does today.** Its frame loops keep their own cycles (`wf`, `ff`, `hf` and 200 ms a frame for the others), and its SVG output is byte-identical to today's. The site's art and the acceptance pass's pinned output don't change until a document adds states.
- **R4. A sprite's first state is made from its frames as they play now.** Adding the first state to a sprite with a frame loop gives that state every frame, at the frame length its loop plays at (for example `ff` at 150 ms). It loops and is the start state, so the art looks and plays the same until the owner changes it. A still sprite's first state has frame 0 and 200 ms.
- **R5. Placements share states, and each item is in its own one.**
  - States live in the object, so every placement of it has the same states.
  - A placed item can start in a state other than the object's start (`state` on the item).
  - Switching one item's state, from a page or a scene key, changes only that item, plus any other items in the group the page switched.
  - A sprite that `extends` another has the base's states and start, and can't have its own.
- **R6. A state's run is the same every time.**
  - What an item shows at time `t` after it enters state `S` (which frame, and how far each layer has moved) is a pure function of the object, `S` and `t`.
  - Following `then` gives a finite run that ends in a loop or a hold. A chain of one-shot states that leads back to itself counts as a loop.
  - The engine computes this one function, the run. The app, the CLI's renders, the compiled SVG and the later video export all use it.
- **R7. Validation.** `check` reports each of these as a problem:
  - a state name that breaks the rule, or appears twice;
  - a `start` or a `then` that names no state of the sprite;
  - `then` on a looping state;
  - a frame step outside the sprite's frames, or a state with no steps;
  - a frame length that isn't a whole number from 20 to 10000 ms, or a state longer than 600000 ms;
  - a move on a layer that doesn't exist or isn't a map layer, or move keys that break the ambient key rules (first at 0, rising, within the state's length);
  - states on a block, or on a sprite that `extends` another;
  - more than 32 states;
  - an item `state` or a key `state` that names no state of the object the item shows.

**Ambient loops and actions become states**

- **R8. An object's ambient loop and actions can be made into states.** One command, and a button in the object inspector, converts a sprite's layer `ambient` and `actions` into states:
  - The ambient loop becomes the start state: it loops, with the layer's move, and its length is the loop's duration.
  - Each action becomes a state of the same name that plays once with the action's move and then hands over to the start state.
  - Today's frames carry over (R4).
  - The command refuses an ambient `offset` other than 0 and says why (a state starts at 0), and it refuses when two layers carry actions of the same name with different lengths.
  - `check` warns about a sprite that has both states and layer ambient loops or actions, since the states decide what plays.
  - A scene item's own `ambient` and `actions` stay as they are. They belong to the placement, not the object, as a path across a scene does.
- **R9. The site keeps working while the page catches up.**
  - A one-shot state of a sprite also plays when the page puts its name in `data-play`, as an action would.
  - When it ends, `animationend` fires with the `animationName` `<scene>--<state>`, as an action's does.
  - So `Range.astro`'s `data-play="hop"` keeps working after `character`'s hop becomes a state.
  - `data-play` stays in the site contract for actions on scene items.

**Switching states on the site**

- **R10. A page switches an item's state with `data-state`.**
  - The page puts a state's name on the `<svg>`, or on any group that contains the item: `data-state="wave"`. Every item below it whose object has a state of that name switches to it.
  - An item whose object has no such state keeps playing what it was playing.
  - The value may list several names. When more than one matches an item's object, the one listed later in the object wins.
  - Taking the attribute off returns the item to the state it starts in.
- **R11. The switch happens at once.**
  - The new state starts from its first frame the moment the attribute changes, with no wait for the current state to end.
  - A page that wants to wait listens for the current one-shot state's `animationend` first (R12).
  - Setting the same name again doesn't restart a state. To restart one, the page removes the name and adds it back, as with actions.
- **R12. A one-shot state says when it ends.**
  - When a state that plays once reaches the end of its one-shot part, `animationend` fires and bubbles. Its `animationName` is `<scene>--<state>`.
  - A hold fires once its last frame starts. A `then` fires when it hands over.
  - A looping state fires nothing.
- **R13. The art still plays by itself.** Everything a state does is in the SVG's own `<style>`, scoped to its scene. A page only sets attributes, and with no attribute set every item plays the state it starts in.
- **R14. Reduced motion.** When the visitor prefers reduced motion, a state doesn't move. Each item shows the first frame of the state its run settles in: a looping state's own first frame, a hold's last frame, or the first frame of the looping state a `then` chain ends in, with every layer at rest. `data-state` still switches which state that is. No `animationend` fires.

**States in a scene's animation**

- **R15. A scene key can change an item's state.**
  - A motion key may carry `state`, a state of the object the item shows at that key (its pose if it has one, otherwise its own object).
  - From that key on, the item plays that state from its first frame, following its run (R6), until a later key changes it again.
  - A track's first key without a state leaves the item in the state it starts in.
- **R16. A scene's state keys play on the site by themselves.** The compiled SVG plays them as part of the scene's motion loop, frame for frame. The run restarts at every loop, so time `t` of the scene shows the same frame in the app, in a render, on the site and in a video.
- **R17. The scene owns an item whose track keys a state.** A page's `data-state` doesn't change an item whose track has a `state` key. The site contract says so, and `check` warns when a group with a `data-state`-style class or data attribute holds such an item (see Areas of concern, 4).

**The animation pane: tracks and keys (scene mode)**

R18 to R25 carry the animation pane's spec (`animation-pane.md`, and its spec on `feat/animation-pane`) onto today's window, with the states additions marked.

- **R18. One pane under the stage.**
  - The timeline strip grows into a pane, as Unity's Animation window and Unreal's Sequencer dock below the viewport. It has the scrubber as its time ruler, and one row per motion track named by the item's id. Each track's keys sit on the ruler's scale, with the gold playhead across every row.
  - Show Tracks shows and hides the pane, and the choice is remembered. The pane opens by itself when an item is animated from the app or a key is picked from the stage.
  - Clicking a row selects its item, and the selected item's row is highlighted.
  - The pane is resizable: 160 pt by default, between 96 and 360 pt, remembered.
- **R19. Clicking a key selects it and moves the playhead to it.**
  - The selected key is drawn larger, in the accent, and the stage shows the item at that key.
  - A key that hides the item is hollow, and a key that poses it has a dot.
  - New: a key that changes the state has a short tick under it.
  - Esc, clicking the row away from a key, or selecting another item deselects the key.
  - ⌥-click selects a key without moving the playhead (Unity's way), for the owner who wants to compare.
- **R20. A track's state lane (new).**
  - A track whose item's object has states gets a second, thinner lane under its keys, like a skeletal mesh's animation track in Sequencer.
  - The lane shows, over the scene's time, which state the item is in, as labeled sections, one per state run. A section's label is the state's name, and a hand-over by `then` inside a run shows as a thin divider labeled with the next state.
  - Clicking a section selects the key that started it.
- **R21. Making, deleting and moving keys.**
  - **Add Key** (the pane's +, K with the pane focused, Edit › Add Key, ⌥⌘K) keys the selected item at the playhead and pins it where it is (`scene.key.set` with no values). At a time that already has a key, it selects that key.
  - **Delete Key** (−, ⌫ with the pane focused, Edit › Delete Key, ⌥⌘⌫) deletes the selected key. The key at 0 is the rest frame and stays (Remove Track does that).
  - Dragging a key moves it in time, snapped to 100 ms. The key at 0 doesn't drag, and a drop on another key's time is refused.
- **R22. The KEY section in the inspector** shows the selected key's time, Move X and Y, Show, Pose and (new) State.
  - Each value is either set on the key or inherited from the key before. An inherited value is dimmed, and Clear makes it inherit again.
  - State offers Inherit plus the states of the object the item shows at that key. Choosing one keys it.
  - Each edit is one undo step, and the track's off-grid warnings show in gold.
- **R23. Animating an item, and the loop.**
  - **Animate** in the ANIMATION section adds a track, with its rest key at 0, to the selected item. An item without an id gets one made from its object's name, made unique (`cart`, `cart-2`), and the result says so.
  - A scene without motion gets a 4 s loop.
  - **Remove Track** drops a track and its keys.
  - The pane's Loop field sets the loop's length. A length that would cut off keys is refused with the reason.
- **R24. The stage keys as before.** Moving an item on the stage with the playhead past 0 keys it (`scene.move` with the time), and the key it made or moved becomes the selected key.
- **R25. The item's start state (new).** For an item whose object has states, the inspector's selection section shows a State picker: the object's start, or another of its states (R5). It sets `state` on the item.

**The animation pane: states (object mode)**

- **R26. In object mode the pane edits the object's states.** Its left column is the states list, like Unity's clip menu or Rive's animation list:
  - Each row shows the state's name, a start badge on the start state, and an icon for loop, hold or `then`, with the next state's name.
  - Rows are reorderable, and the list has + (New State) and − (Delete State).
  - Selecting a state shows its dope sheet on the right.
  - A sprite without states shows its frame loop as one row named after the loop, marked "plays on its own", with **Make States** (R4, R8). A block shows "Blocks have no states".
- **R27. A state's dope sheet.**
  - The ruler is the state's own time, from 0 to its length.
  - A **Frames** row has one cell per frame step: the frame's thumbnail and number, the width of one frame length. Clicking a cell selects the step and shows that frame on the canvas, so painting edits that frame. Dragging a cell reorders the steps. + appends the frame shown, and ⌫ removes the selected step. A frame can appear more than once in a state.
  - Below that, one **Move** row per map layer the state moves (and an "Add Move" menu for the others), with keys as in scene mode. Add, delete and drag work the same, snapped to the frame length, and so do the KEY values (Move X and Y).
  - The header has the state's name (rename in place), the frame length in ms, Loop or Once, and for Once, Hold or Then with a state picker. It also has the state's length.
- **R28. The state graph.** A Graph toggle swaps the dope sheet for the state machine, laid out as Unity's Animator draws it:
  - an **Entry** node with an arrow to the start state;
  - a node per state, with the start state's node in the accent;
  - an arrow for each `then`, labeled "at end";
  - a looping state drawn with a loop mark;
  - a note node, **Any state**, with dashed arrows to every state, labeled "the page or a scene key".
  
  The graph is the same data as the list. Clicking a node selects the state, dragging an arrow's end onto another node sets `then`, and right-clicking a node gives Set as Start, Rename, Duplicate and Delete. Node positions are laid out automatically, left to right along the `then` chains, and aren't stored.
- **R29. Trying states on the canvas.**
  - With the pane focused or the object on the canvas, Play plays the selected state from its first frame, following its run. The canvas shows each frame nearest-neighbor at the canvas's zoom, and the playhead moves on the state's ruler.
  - A row of **Try** chips over the canvas, one per state, switches the playing preview to that state at once, as a page's `data-state` would. That's the owner's check of R11, and the chips' order is the states list.
  - Esc stops and shows the selected step's frame again.
  - Under Reduce Motion, Play is off and the canvas shows where the state settles (R14). Stepping frames still works.
- **R30. Frames and layers stay consistent.** Adding, duplicating, moving and deleting frames, and adding, moving and deleting layers, keep every state pointing at the same art:
  - frame steps are renumbered when frames move;
  - a deleted frame leaves the states that used it;
  - a state that would be left with no steps makes the delete refused, with the state's name;
  - a deleted layer's moves are dropped, and a moved layer's moves follow it.

**Keyboard, menus, access**

- **R31. The pane takes keys when focused,** with its focus ring visible:
  - ← and → step the playhead (100 ms in a scene, one frame length in a state);
  - ⇧← and ⇧→ go to the previous or next key and select it;
  - K adds a key, ⌫ deletes the selected key or step, Space plays or pauses, and Esc deselects.
  - In object mode, ↑ and ↓ move through the states list, and ⌘G toggles the graph.
  
  The Edit menu gets Add Key (⌥⌘K), Delete Key (⌥⌘⌫), Previous Key (⌥⌘←), Next Key (⌥⌘→), Animate Item or Remove Track, and in object mode New State (⌥⌘N) and Make States. Each is enabled when the pane's button is. The stage keeps its own keys.
- **R32. Accessibility.**
  - VoiceOver reads each key, for example "Key at 4.2 s: move 32, 16, hidden, pose crane-hook-drop, state drop". It reads each row ("Track cart, 5 keys, states rest, tip"), each state row ("hop, plays once then idle, 480 ms"), each frame step ("Step 3, frame 2"), each graph node and arrow ("hop leads to idle at end"), and the ruler's value.
  - The graph has a list equivalent: the states list carries the same facts, so nothing is graph-only.
  - Keys, steps and nodes are at least 12 pt targets with a 6 pt hit margin.
  - Nothing in the pane animates by itself. Playback stays off under Reduce Motion, and everything else works.
- **R33. The look.** Board 03's tokens and type, with one accent (the selected key, the start state and the selected node). The playhead and Play stay gold, and the pane is part of the timeline's surface (`Theme.sidebar`). Frame thumbnails and the canvas preview are drawn nearest-neighbor at integer zoom.

**Doors, tests, docs**

- **R34. Every edit is a command, in all three doors.** These are new commands, or changes to existing ones, in the registry (D3):
  - the state commands;
  - `scene.key.set` with `state`, and with no values pinning the item;
  - `scene.set` with `state`;
  - `scene.track.add` giving an item an id;
  - `object.render` and `scene.render` at a state and time;
  - the read command for a state's run.
  
  Agents' edits apply as they make them, through the app, as every edit does since autosave, and each is one undo step and part of a version.
- **R35. The script hook drives the pane:** actions and `expect`s for keys, tracks, the loop, state lanes, the states list, a state's steps and moves, the graph, Try and Play (D6). There are unit tests for the pane's pure logic (hitting a key, snapping, inherited values, laying out the graph, a run's sections), and acceptance scripts on temp copies (D6).
- **R36. Docs:**
  - the site contract (`data-state`, the state end event, reduced motion and `data-play` for states);
  - the document format (states, item and key `state`);
  - the README's tour and shortcuts;
  - `CLAUDE.md`'s script-hook list;
  - `docs/testing.md` and `app/README.md`;
  - `docs/cli.md`, regenerated;
  - a follow-ups note in the record (`../2026-10-mac-app/plan.md`).
  
  The examples get states on one object each (D7).

## Design

### D1. The document (R1–R5, R7, R15)

A sprite gains `start` and `states`, after `layers` in the canonical key order. `states` is an object keyed by state name, in the order the owner lists them, as `actions` is:

```json
{
  "kind": "sprite",
  "anchor": [20, 29],
  "keys": { "…": "…" },
  "layers": [
    { "map": ["…"] },
    { "class": "character", "map": ["…"] }
  ],
  "start": "idle",
  "states": {
    "idle": { "frames": [0], "ms": 1000, "moves": [{ "layer": 1, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 500, "move": [0, -1] }] }] },
    "hop": { "frames": [0], "ms": 480, "loop": false, "then": "idle", "moves": [{ "layer": 1, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 44, "move": [0, -2] }] }] }
  }
}
```

| Key | Meaning |
|---|---|
| `frames` | The frame steps: frame numbers of the sprite (R2), at least one, at most 256. |
| `ms` | How long each step shows: a whole number from 20 to 10000. The state's length is `frames.length × ms`. |
| `loop` | `false` plays once. It's left out when the state loops (the default). |
| `then` | For `loop: false` only: the state that follows. Without it, a state that plays once holds its last frame. |
| `moves` | `[{ layer, keys }]`: the index of a map layer, and ambient-style keys (`{ t, move }`, first at 0, rising, below the state's length). It's left out when no layer moves. A looping state's move steps back to its first key over the rest of the loop, as an ambient loop's does. A one-shot state holds its last key. |

- **`start`** is required once `states` exists, and names one of them. `object.state.add` sets it on the first state.
- **An item's `state`** (`{ "object": "character", "state": "wave", "at": … }`) is the state it starts in. It's left out when that's the object's start. Canonical order: after `depth`.
- **A key's `state`** (`{ "t": 2000, "state": "hop" }`) is a fifth key property beside `t`, `move`, `show` and `pose`. `KEY_PROPS` grows, and the canonical form keeps one line per key.
- Format: these are additions to format 2. `manifest.json` keeps `"format": 2`. A release before this one rejects a document with `states` (an unknown property), so the site pins this release before its document gets states (Areas of concern, 1).
- `crates/engine/src/format.rs` gets the key orders. `crates/engine/src/engine.rs` validation calls a new `states::validate` from the sprite's checks (R7). `motion::validate_motion` checks a key's `state` against the object the item shows at that key.

### D2. The engine: `crates/engine/src/states.rs` (R6, R10–R16)

A new module, beside `ambient.rs`, with one source of truth for timing:

- **`States`**, parsed once per sprite (as `ambient::moves_of` is): the states in order, each with its steps, `ms`, end (`Loop`, `Hold` or `Then(name)`) and per-layer `ambient::Key` lists.
- **`run(states, state) -> Run`**, the run of R6. It follows `then` from `state` into a list of **segments**: `{ state, from_ms, steps }`. The list ends in a **settle**: either `Loop { from_ms, period }` for a looping state or a chain that comes back on itself (the period is the whole cycle), or `Hold { from_ms, frame }`. A `then` chain visits each state at most once before it settles, so a run is at most 32 segments.
- **`at(run, t) -> Pose`**: the frame number, and each layer's move, `t` ms after entering. A layer's move inside a segment uses `ambient::move_at`'s stepping, so a state moves exactly as an action does today.
- **`changes(run, horizon) -> Vec<Change>`**: every time the frame or a move changes, up to the settle plus one period. The app plays from this list, and the CSS is built from it.
- **CSS (R10–R14),** added to `motion::scene_css`. For an item whose object has states:
  - The item's element gets a generated class, as elements with ambient loops do today (`<scene>--sN`).
  - Every frame the states use is drawn once inside it, as frame groups (`<scene>--sN-fK`).
  - Each layer that a state moves is wrapped in its own element (`<scene>--sN-lL`).
  - For each state `S`, from `changes(run(S))`:
    - **Frames.** One `@keyframes <scene>--sN-S-fK` per frame `K`, on opacity, with `step-end` stops. The finite part plays once (`1 forwards`), and the settle loop is a second animation on the same element, delayed by its `from_ms` and `infinite`. The later one in the `animation` list wins while both apply.
    - **Moves.** The same two-part pattern on `transform` per layer, built with `ambient`'s `frames()`.
    - **The end event.** For a run whose start state plays once, an animation named `<scene>--S` on the item's element, `1` iteration long over the one-shot part, animating a property nothing renders (`--pl-end`). That fires `animationend` with the contract's name (R12, R9).
  - **Selectors.** The start state, or the item's `state`, needs no attribute. State `S` applies under `[data-pixellab="s"] [data-state~="S"] .s--sN`, `[data-pixellab="s"][data-state~="S"] .s--sN`, and, for a one-shot `S`, the same two with `data-play~="S"` (R9). Rules for later states come after earlier ones, so the later-listed state wins (R10). The animation names differ per state, so a change of attribute restarts the animation, which is what makes the switch immediate (R11).
  - **Scene keys (R16).** For an item whose track keys `state`, the frames and moves are baked over the scene's `duration` instead. The engine walks the track's state changes and, for each, the run's `changes`, clipped at the next state key and at the loop's end. It writes one `@keyframes` per frame and per moved layer, at the scene's period, `step-end`, `infinite`, beside the track's own move. No `data-state` selectors are written for that item (R17).
  - **Reduced motion (R14).** The existing `@media (prefers-reduced-motion:reduce)` block also stops `[class*="<scene>--s"]`. For each state it shows the settle frame: an `opacity:1` rule for that frame's group under the state's selector, and `opacity:0` for the others.
- **Byte size.** States draw each used frame once per item. That's no more than a frame loop draws today, plus one rule set per state. A baked scene state track adds stops per frame change over the loop. `art sizes` reports states as a line of their own, and the budgets in `compile.rs` apply unchanged. Areas of concern, 5.
- **Renders.** `render.rs` and the engine's `render` at a time ask `states::at`, for an object render (`--state S --time t`, the time since entering) and for a scene render (each item at the scene's time, from its track's state keys or from its start state since time 0). The `motion::loop_frame` path stays for sprites without states (R3).

### D3. Commands (R4, R8, R15, R21–R25, R27, R30, R34)

New commands, registered in `registry.rs` under `crates/pixellab/src/commands/object_states.rs`, and an edit kind for every door. Each edit is one undo step with a label, as the motion commands are.

| Command | Does |
|---|---|
| `object state add <object> <name> [--frames 0,1,2] [--ms n] [--once [--then <state>]] [--after <state>]` | Adds a state, after `--after` or at the end. Without `--frames` it gets every frame. Without `--ms` it gets the frame loop's timing (R4) or 200. The first state becomes `start`. |
| `object state set <object> <name> [--rename <new>] [--frames …] [--ms n] [--loop \| --once] [--then <state> \| --hold] [--start]` | Changes a state. Renaming updates `start`, every `then` and every scene item and key that names it, in every scene, as one undo step per doc touched. The result lists them. |
| `object state delete <object> <name>` | Refused while a `then`, `start`, or a scene item or key names it, with the list. Deleting the last state removes `states` and `start`, and the frame loops play on their own again. |
| `object state duplicate <object> <name> [--as <new>]` | Copies a state just after it. |
| `object state move <object> <name> --by ±n` | Reorders the list (R10's "later wins", and the pane's order). |
| `object state step add\|delete\|move <object> <state> [--frame k] [--at i] [--by ±1]` | Edits a state's frame steps (R27). |
| `object state key set\|delete\|move <object> <state> --layer L --time t [--move x,y] [--to t]` | The state's moves, mirroring `scene key`. `set` with no `--move` pins the layer where it is. |
| `object states make <object>` | Turns the layers' `ambient` and `actions` into states, and takes them off the layers (R8). |
| `object state run <object> <state> [--horizon ms]` | Read. Returns the run: `{ segments, settle, changes: [{ t, frame, moves: { "<layer>": [x, y] } }] }`. |

Changes to existing commands:
- `scene key set` gains `--state <name>` and `--state clear`. With no values at all it pins the item (`animation-pane.md`: `{ t, move }` at the item's move at `t`, `pinned: true`, and `changed: false` when a key is already there).
- `scene set` gains `--state <name>` and `--state start` (to clear it).
- `scene track add` on an item without an id gives it one: the object's name, made unique in the scene, returned as `id`.
- `object render` gains `--state <name> --time <ms>`, and `scene render` places states at `--time`. Both report each item's `state` and `frame` in `items`.
- `object frame add|delete|duplicate|move` and `object layer add|delete|move|duplicate` keep states consistent (R30), and their results add `states` when steps or moves changed.
- `object show` and `object list` report states: their names, the start state and each one's length.
- `check` gives R7's problems, R8's warning and R17's warning.
- `scene show --doc` already returns `motion`. `scene items` adds an item's `state`.
- Over MCP the tools take the same names (`object_state_add`, …, `object_states_make`, `object_state_run`). The server's instructions get a States paragraph: what a state is, that a page switches it with `data-state`, and that the scene keys it with `scene_key_set state`.

Undo and versions: every command above goes through `session.rs`. A rename that touches several docs is one version, as `library import` is. History renders states, since `history render` uses the same render.

### D4. The app: model (R18–R30)

- **`TimelineModel`** gets the animation pane's model (`animation-pane.md`, rebuilt on today's `TimelineModel.swift`):
  - `MotionTrack.keys: [MotionKey]`, where `MotionKey { t, move?, show?, pose?, state? }` is decoded from `scene.show --doc`;
  - `selectedKey`, `paneHeight` and `showTracks` persisted (`SettingsKey`), plus `paneFocused` and `keyDrag`;
  - `selectKey(item, t, movePlayhead:)`, `addKey()`, `deleteKey()`, `moveKey(to:)`, `setKey(…state:)`, `addTrack()`, `removeTrack()`, `setDuration(_:)`, `stepKey(±1)` and `stepTime(±)`, each a command through `SceneEditor.edit`.
  
  New: `stateSections(for track) -> [StateSection]` (`{ from, to, state, handovers }`) for the lane. It's built from the track's keys and each object's run, which the model fetches once per object with `object.state.run` and caches by the object's version.
- **`StatesModel`** (new, `Model/StatesModel.swift`), owned by `ObjectEditor` in object mode:
  - the states (from `object.show`), the selected state and step, the selected move key, `graphShown` and the cached runs;
  - edit methods that call D3's commands;
  - `play()`, `stop()` and `try(state)`, which walk the run's `changes` on a timer and set the canvas's frame and layer offsets.
  
  The canvas draws a frame from the existing frame cache (`ObjectEditor.cached(frame:)`), so playing renders each frame once and applies each layer's move as an integer shift at the canvas zoom. Layer offsets need the frame per layer, so the cache renders a layer at a time (the `layers only` parameter it already has) when a state moves layers.
  
  `StatesModel.graphLayout(states) -> [Node]` is a pure function laying out nodes in columns along `then` chains, with the start state in the first column after Entry. It's unit-tested.
- **The selection.** `SceneEditor` shows the item's `state` and offers the object's states (R25).

### D5. The app: views (R18–R33)

- **`TimelineView.swift` → `AnimationPane`.**
  - The header row stays: Timeline, Play, the scrubber and the time. It gains Loop (scene) or the state's header (object), + and −, Previous and Next Key, and the tracks toggle.
  - Under it is the pane body, resizable from its top edge (an `NSView` split handle, as the right column's is), between 96 and 360 pt.
  - Scene mode: rows as `animation-pane.md` describes, each a `Canvas` for its keys, with a 10 pt state lane under any track with states. Section fills are `Theme.selection`, the labels SF Mono 10, and the selected section is outlined in the accent.
  - Object mode: the states list (200 pt, `List` with drag to reorder) on the left, and the dope sheet or the graph on the right. The graph is drawn in a `Canvas` with nodes as rounded rectangles (`Theme.card`, the start state in the accent), arrows in `Theme.inkMuted`, and the Any state note dashed. Hit-testing comes from `graphLayout`.
- **Focus and keys.** The pane gets a small AppKit key catcher (`NSViewRepresentable`, as the stage has), as `animation-pane.md` recommended, and its focus ring is drawn as the stage's (2 pt accent at 55%). The model's `handleKey(KeyInput)` holds the logic so the script hook sends the same keys.
- **Inspector.**
  - Scene mode: `KeyInspector.swift` (new) gets the KEY section with State, and `AnimationSection` with Animate and Remove Track. The selection section gets the item's State picker.
  - Object mode: `ObjectPanels.swift`'s ANIMATION section keeps painting frames (strip, onion skin, add and duplicate frame). Its loop-kind picker shows only for a sprite without states. It gains **Make States**, with the help text "Make states from this sprite's frames, ambient loop and actions" (R8).
  - The KEY section serves a state's move keys too, with Time snapped to the frame length, and Move X and Y.
- **Try chips.** In object mode, a row of chips floats above the floating bar, in `StageChrome` (`Theme.sidebar` surface, the playing chip in `Theme.selection`). It shows only while the pane shows states.
- **Menus.** In `PixelLabCommands`, after Select None, a divider, then R31's items, titled and enabled by the mode.
- **Copy.** Short labels as the app writes them today: "Plays once, then idle", "Holds its last frame", "Loops", "Start", "Make States", "Any state: the page or a scene key".

### D6. The script hook, tests and acceptance (R35)

- **Script hook actions** (`ScriptRunner.swift`):
  - from `animation-pane.md`: `key add|delete|next|prev`, `key select <ms> [alt]|none`, `key move <ms> <to>`, `keyset time|move|show|pose …`, `track add|remove`, `loop <ms>` and `panekey <spec>`;
  - new for scenes: `keyset state <name>|clear`, `itemstate <name>|start` and `lane select <ms>`;
  - new for object mode: `state add|delete|duplicate|select|rename|start <name>`, `state ms <n>`, `state end loop|hold|then <name>`, `state move <name> <±n>`, `step add <frame>|delete|select <i>|move <i> <to>`, `statekey add|delete|select <ms>|move <ms> <to>` with `--layer`, `states make`, `graph on|off`, `graph link <from> <to>`, `try <name>`, `stateplay` and `statestop`.
- **`expect`s:**
  - from `animation-pane.md`: `expect key|keys|keymove|keyshow|keypose|loop|tracks|pane`;
  - new: `expect keystate <name>|inherits`, `expect lane <item> <name@ms,…>`, `expect itemstate <name>`, `expect states <names>`, `expect start <name>`, `expect state <name> <frames>@<ms> loop|hold|then:<name>`, `expect steps <frames>`, `expect statekeys <layer> <ms,…>`, `expect graph <from>-><to>,…`, `expect trying <name>`, `expect preview frame <k> move <layer> <x,y>`.
- **Unit tests** in Rust:
  - `states::run`, `at` and `changes`, for loops, holds, chains, cycles and moves;
  - the CSS for each kind, with fixtures in `fixtures/`;
  - R3's byte-identical output for the fixture and examples without states;
  - each command, including renames across scenes and R30's remapping;
  - R8's conversion and its refusals;
  - R16's baked track, matched frame for frame against `scene render --time` at every 100 ms.
  
  In Swift Testing: the pane's hit-testing and snapping, inherited values, `stateSections`, `graphLayout`, and playing a run.
- **A browser check of the CSS (R10–R14),** as a Node script in `scripts/test/`. It compiles a fixture with states and checks the generated rules' shape: selectors, animation lists, the end animation's name and the reduced-motion block. Driving a real browser to watch `animationend` is listed in Areas of concern, 3, for the plan to decide.
- **Acceptance.** Three new scripts in `scripts/acceptance/`:
  - `states.txt`, on a temp copy of `examples/harbor.pixellab`: make states for an object, add a one-shot that hands over, link it in the graph, Try it, Play, and check the published SVG for `data-state` rules;
  - `animation.txt`, on a temp site copy: select the cart, add, move, edit and delete a key, set the loop, Animate an item and remove its track;
  - `scenestates.txt`, on the harbor copy: key an item's state, check its lane, and match `scene render` frames at three times.
  
  Each runs on a temp copy, as autosave requires. The hero island's SVG stays byte-identical where no states are added (R3).

### D7. Docs and examples (R36)

- **`docs/site-contract.md`** gets a "States" section beside "Actions":
  - `data-state` on the root or a group, the list rule and "later wins";
  - the switch is immediate, and how to restart a state;
  - the one-shot end event (`<scene>--<state>`);
  - `data-play` for one-shot states;
  - the scene's state keys owning their items;
  - reduced motion's settle frame;
  - an example: a button that waves, waiting for `animationend` to clear `data-state`.
  
  "Nothing else is a promise" adds the `<scene>--s…` names.
- **`docs/document-format.md`**: a "States" section under Sprites, `state` on items and keys, and the canonical order.
- **README**: the tour's timeline paragraph becomes the pane, plus a States paragraph and the shortcuts. `CLAUDE.md`, `docs/testing.md` and `app/README.md` name the new actions, views and models. `docs/cli.md` is regenerated.
- **Examples**: `scripts/make-examples.mjs` gives the harbor's `lamp` two states: a steady `idle` and a one-shot `flicker` that hands back to `idle`. It gives the harbor's `boat` a rocking `idle` made from an ambient loop by `object states make`, and a scene key in the town that switches a lamp to `flicker` once a loop. `npm run examples` rewrites both.
- **The record** gets a follow-up: an item's states detached from its object (out of scope), and per-step frame lengths if the owner asks.

## Areas of concern

1. **The site has to take a new release before its document gets states.** Today's site pins `pixellab` v0.6.0 (`scripts/pixellab.mjs`). That release, and v0.7.0, reject `states`, item `state` and key `state` as unknown properties. The site keeps building until its document uses them. Then it needs this release, and `Range.astro` keeps working through `data-play` (R9). A site change to use `data-state` is optional and belongs in the site repo under its own intent. The owner decides when the site's `character` becomes states (`object states make character`). That's a change to the site's art, in the site repo.
2. **Two ways to change what an item looks like over a scene: poses and states.** The hero island's 59 pose keys swap whole objects (`crane-hook`, `crane-hook-drop`, `crane-hook-low`), and those are states in all but name. This spec keeps poses, so nothing in the site's art changes, and adds states beside them. A key may carry both, with the state naming one of the pose's states. Folding pose families into one object with states (a "Make States from Poses" command) would make the hero island's tracks shorter, but it changes the site's art and is left as a follow-up. The owner should say whether they want it in this change.
3. **A CSS-only state machine is at the edge of what step animations do.** Immediate switching relies on a change of `animation-name` restarting an animation. "Then" relies on a second, delayed animation in the same list taking over. The end event relies on an animation of a custom property firing `animationend`. All three are standard CSS Animations behavior in current Safari, Chrome and Firefox, but the site's pages run in all three, and the existing tests only check strings. The plan should include a real-browser check: Playwright is already used by the site, not here, so it needs the owner's OK as a dev dependency, or a manual check recorded in the PR.
4. **A page's `data-state` against a scene's state keys.** R17 gives the scene the item, which keeps a scene's animation the same every time (the intent's constraint for video). The alternative, letting the page override a scene-keyed item while the attribute is set, is possible in CSS (later selectors win). It would make a page-driven scene differ from its video. The spec chooses the scene. The `check` warning in R17 can only guess which groups a page targets (any group with a `data-*` attribute that holds a state-keyed item), so it may be noisy. The plan can drop it if so.
5. **Bytes.** A state that switches frames draws each frame once per placement, as frame loops do today. The rules for each state cost about 150 to 400 bytes, and a baked scene state track costs about 30 bytes per change over the loop. A forest of 40 pines with three states each would add roughly 40 frame groups per extra frame. The engine's budgets catch a scene that grows past them, and `art sizes` shows the cost. An alternative that shares one set of frames per object across its placements (`<use>`) is smaller but changes the output's structure for every sprite. It isn't proposed here.
6. **Frames are per sprite, with frame-loop layers in lockstep (R2).** Today a sprite could, in principle, carry two frame-loop layers of different lengths (none of the site's 21 animated objects does). With states, such a sprite has to even them out before it gets states, and `check` says which layer. This is the Aseprite model (a frame is a column across all layers) and keeps the pane simple.
7. **Uniform frame length per state.** Aseprite and Unity allow a different length per frame. This spec gives a state one `ms`, and a held frame is the same frame repeated in the steps. Per-step lengths could come later without breaking the format (`ms` as a list).
8. **No conditions on transitions.** Unity and Unreal transitions test parameters. Here only `then` (at end) is stored, and anything else is the page's or the scene's job. The graph's Any state note makes this visible. If the owner wants transitions on events in the art itself (for example "on hover, go to wave" without page script), that's a site contract addition for later.
9. **Clicking a key moves the playhead** (Rive's way), with ⌥-click to select only (Unity's way). The animation pane's spec chose to move it, and the intent left it open. This spec keeps that and adds ⌥-click.

## Open questions

The intent's questions:
- **Do frame loops become states, or stay alongside them?** Both, in turn. A sprite without states plays its frame loops as today, byte for byte (R3), and the pane shows the loop as one row with Make States. Its first state is made from the loop's frames and timing, so nothing changes until the owner edits it (R4). From then on the states decide which frame shows. The ANIMATION section in object mode keeps painting frames, and the pane edits their timing.
- **Does a scene's state change play on the site by itself?** Yes. The compiled SVG plays it as part of the scene's motion loop, frame for frame (R16), so the site, the app and a video show the same thing.
- **When the page asks for a new state mid-state, switch at once or wait?** At once (R11). A page that wants to wait listens for the one-shot state's `animationend` (R12).
- **Where is the timeline?** Under the stage, where the strip is now, as a resizable pane (R18). This is where Unity's Animation window, Unreal's Sequencer and Rive's timeline dock. The left column stays Layers in scene mode.
- **Does clicking a key move the playhead?** Yes, with ⌥-click to select without moving (R19; Areas of concern, 9).
- **The default loop for a scene's first animated item?** 4 s (R23).

New questions for the owner:
- Should this change also fold the hero island's pose families into states (Areas of concern, 2)?
- Is a real-browser check of the state CSS worth a dev dependency (Areas of concern, 3)?
- Should the examples' state demos be the lamp and the boat (D7), or does the owner have other objects in mind?
