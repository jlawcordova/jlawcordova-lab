# Intent: Object states
Author: J. Law Cordova (owner). Status: accepted. On 2026-10-24 the owner folded the animation pane (`animation-pane.md`) into this intent; the change is accepted when it merges.

## Problem
In the owner's words:

> I'm thinking it should be possible to make an object have its own state machine for its animation.

Today an object's animation is one loop that always plays. An object can't have named animations of its own, such as idle, walk or wave, and nothing can switch it from one to another. Actions can move an item, but they belong to an item in a scene, not to the object, and they don't change its frames. So a character can hop across a scene, but its drawing can't change to match the hop.

The owner also plans to use the engine for video output later (`../2026-10-video-export/intent.md`), and the states should serve a scene's animation there too, not only the site.

The timeline can't edit any of this either. It plays and scrubs a scene's motion and shows a row per animated item with dots where its keys are, but nothing in it can be edited. Keying an item from the app means moving it on the stage with the playhead past 0, which makes or moves a key the owner can't see or pick. Everything else needs the CLI, an agent, or the scene's file. That includes making a key at a time, deleting one, moving it in time, hiding an item from a key on, posing it as another object, animating an item that isn't animated yet, and setting the loop's length. In the owner's words, from the animation pane they asked for on 2026-10-20:

> There should be some way to edit the timeline animation per key. When I click on an object, I should be able to click on its key frames, make key frames, and so on.

Editing states needs that same timeline. So the animation pane is folded into this change rather than built on its own: one timeline that edits a scene's keys and an object's states.

## Proposed outcome
States:
- An object can have states: named animations of its own, with one state it starts in, and states that lead to another when they end (wave once, then back to idle).
- States belong to the object, so every placement of it shares them. Each placed item is in its own state, so changing one pine's state doesn't change the others. Detaching a placed item so it gets its own states may come later if it's needed.
- Actions become part of states. A move that belongs to the object, such as a hop, goes with its state. A path across a scene, such as the train's route, stays the scene's motion, because it depends on where the item is placed.
- On the site, anything the page notices can change a state: hover, click, scroll and so on. The page only says which state to play; the art still plays by itself.
- A scene's own animation can use an object's states too, by changing an item's state at a point in the scene's time. That animation is exactly the same every time it plays, so the later video export can render it frame for frame.

The timeline, as an animation pane (like Unity's Animation window or Rive's timeline):
- One pane edits both: a row per animated thing, with its keys on a shared time ruler under the playhead. Keys can be clicked to select and dragged to move, and buttons and shortcuts add and delete them.
- In a scene, clicking an item on the stage shows its keys. Clicking a key selects it and takes the playhead there, so the stage shows the item at that key.
- Keys can be made at the playhead, deleted and moved in time, from the pane and from the keyboard. The selected key's values can be seen and edited: where the item is, whether it shows, which object it poses as, and the state it changes the item to.
- An item that isn't animated yet can be animated from the app, and a scene without motion gets some. An item's track can be removed, and the loop's length can be set.
- Moving the item on the stage past time 0 keeps keying it as it does now, and the key it touches shows up selected in the pane.
- An object's state machine is edited in the same timeline. To try it, the owner focuses the object on the canvas and plays its states from the pane.
- Everything undoes, and an agent over MCP can do the same through the commands the pane uses.

## Affected users and systems
- The owner in the app: the timeline (the pane), the inspector, the Edit menu and the keyboard in scene mode, object mode, and trying states on the canvas.
- Agents and the CLI, which should be able to define states, change a scene item's state over time, preview a state, and make every key edit the pane makes.
- Objects and scenes in the document, including the documents that already have actions, and the examples.
- What Publish writes for the site, and the site contract the site's pages rely on.
- The script hook and the acceptance pass, which drive the pane.
- The video export to come, which should build on these states rather than having its own.

## Constraints
- The published art stays self-contained and plays without script; a page only switches states.
- A state change gives the same frames every time, with nothing random or dependent on the browser, so a scene renders the same in the app, on the site and in a video.
- Reduced motion keeps working for everything that's animated. Playing stays off with it on; everything else in the pane works.
- Pixel art stays crisp: nearest-neighbor, integer zoom.
- Board 03's look: the pane is part of the timeline, in its tokens and type, with one accent, and the playhead stays gold.
- VoiceOver labels, visible focus and comfortable hit targets on the pane and its keys.
- Every action is something the CLI, the app and agents can all do. The pane calls commands; it doesn't write the document itself. Agents' edits apply as they make them, through the app, as every edit does since autosave (`../2026-10-autosave/intent.md`).
- No new dependencies without asking. Tests and scripts don't write to the real site checkout.
- Out of scope: detaching a placed item's states from its object.
- Out of scope: the video export itself, which is its own intent.
- Out of scope: curves and easing (motion steps on the grid by design), selecting several keys at once, and copying and pasting keys.

## Open questions
The owner left these for the spec.

- Do the frame loops objects have today become states, or stay alongside them? If they stay, does the pane edit them too, or do they keep their own panel in object mode?
- When a scene's animation changes an item's state, does that play on the site by itself, or only in the app and in a video?
- What happens when the page asks for a new state while another is still playing: does it switch at once, or wait for the current one to end?
- Where is the timeline: under the stage, where the scene's timeline strip is now, or in a panel on the left, as the first draft of this intent said?
- Should clicking a key move the playhead to it (Rive's way) or only select it (Unity's)? The animation pane's spec chose to move it.
- What loop length should a scene get when its first item is animated from the app? The animation pane's spec chose 4 s.
