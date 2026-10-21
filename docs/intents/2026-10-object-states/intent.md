# Intent: Object states
Author: J. Law Cordova (owner). Status: accepted.

## Problem
In the owner's words:

> I'm thinking it should be possible to make an object have its own state machine for its animation.

Today an object's animation is one loop that always plays. An object can't have named animations of its own, such as idle, walk or wave, and nothing can switch it from one to another. Actions can move an item, but they belong to an item in a scene, not to the object, and they don't change its frames. So a character can hop across a scene, but its drawing can't change to match the hop.

The owner also plans to use the engine for video output later (`../2026-10-video-export/intent.md`), and the states should serve a scene's animation there too, not only the site.

## Proposed outcome
- An object can have states: named animations of its own, with one state it starts in, and states that lead to another when they end (wave once, then back to idle).
- States belong to the object, so every placement of it shares them. Each placed item is in its own state, so changing one pine's state doesn't change the others. Detaching a placed item so it gets its own states may come later if it's needed.
- Actions become part of states. A move that belongs to the object, such as a hop, goes with its state. A path across a scene, such as the train's route, stays the scene's motion, because it depends on where the item is placed.
- On the site, anything the page notices can change a state: hover, click, scroll and so on. The page only says which state to play; the art still plays by itself.
- A scene's own animation can use an object's states too, by changing an item's state at a point in the scene's time. That animation is exactly the same every time it plays, so the later video export can render it frame for frame.
- In the app, an object's state machine is edited in the timeline panel on the left. To try it, the owner focuses the object on the canvas and plays its states from the panel.

## Affected users and systems
- The owner in the app: the timeline panel, object mode, and trying states on the canvas.
- Agents and the CLI, which should be able to define states, change a scene item's state over time, and preview a state.
- Objects and scenes in the document, including the documents that already have actions, and the examples.
- What Publish writes for the site, and the site contract the site's pages rely on.
- The video export to come, which should build on these states rather than having its own.

## Constraints
- The published art stays self-contained and plays without script; a page only switches states.
- A state change gives the same frames every time, with nothing random or dependent on the browser, so a scene renders the same in the app, on the site and in a video.
- Reduced motion keeps working for everything that's animated.
- Pixel art stays crisp: nearest-neighbor, integer zoom.
- Every action is something the CLI, the app and agents can all do; agents' edits stay proposals.
- No new dependencies without asking.
- Out of scope: detaching a placed item's states from its object.
- Out of scope: the video export itself, which is its own intent.

## Open questions
The owner left these for the spec.

- Do the frame loops objects have today become states, or stay alongside them?
- When a scene's animation changes an item's state, does that play on the site by itself, or only in the app and in a video?
- What happens when the page asks for a new state while another is still playing: does it switch at once, or wait for the current one to end?
