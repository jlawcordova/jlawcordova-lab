# Intent: Edit the timeline's animation key by key
Author: J. Law. Cordova (owner). Status: accepted (asked for directly on 2026-10-20; reviewed with its spec, plan and code in one PR, at the owner's request).

## Problem
The timeline under the stage plays and scrubs a scene's motion, and shows a row per animated item with dots where its keys are, but nothing in it can be edited. Keying an item from the app means moving it on the stage with the playhead past 0, which makes or moves a key I can't see or pick; everything else (making a key at a time, deleting one, moving it in time, hiding an item from a key on, posing it as another object, animating an item that isn't yet, the loop's length) means the CLI, an agent, or editing the scene's source by hand.

There should be some way to edit the timeline animation per key. When I click on an object, I should be able to click on its key frames, make key frames, and so on. An animation pane may be needed, the way tools like Unity (its Animation window) or Rive (its timeline) do it: a row per animated thing, its keys on a shared time ruler with the playhead, keys you click to select and drag to move, and buttons and shortcuts to add and delete them.

## Proposed outcome
- Clicking an item on the stage shows its keys in the timeline, and clicking a key selects it and takes the playhead there, so the stage shows the item at that key.
- Keys can be made at the playhead, deleted, and moved in time, from the pane and from the keyboard; the key's values (where the item is, whether it shows, which object it poses as) can be seen and edited for the selected key.
- An item that isn't animated yet can be animated from the app, and a scene without motion gets some; an item's track can be removed. The loop's length can be set.
- Moving the item on the stage past time 0 keeps keying it as it does now, and the key it touches shows up selected in the pane.
- Everything undoes, and an agent over MCP can do the same through the commands the pane uses.

## Affected users and systems
- The owner in the app: scene mode's timeline, inspector, Edit menu and keyboard.
- Agents over the CLI and MCP, which get any new command or option the pane needs.
- The script hook and the acceptance pass, which drive the pane.

## Constraints
- The existing motion commands (tracks, keys, the loop's length, the path helper) are the way edits happen; the pane calls them. Nothing new is written to the document's format.
- Board 03's look: the pane is part of the timeline strip, in its tokens and type, with one accent; the playhead stays gold.
- VoiceOver labels, visible focus, comfortable hit targets, Reduce Motion (playing stays off with it on; everything else works).
- No new dependencies. The real site checkout isn't written to by tests or scripts.
- Out of scope: curves and easing (motion steps on the grid by design), multi-key selection, copy and paste of keys, keying anything but move, show and pose, and the object-mode frame loops (they have their own panel).

## Open questions
- Should clicking a key move the playhead to it, or only select it? (The spec decides; the owner can say otherwise in review.)
- What loop length should a scene get when its first item is animated from the app?
