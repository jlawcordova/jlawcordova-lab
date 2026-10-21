# Intent: Video export and scene transitions
Author: J. Law Cordova (owner). Status: draft.

## Problem
In the owner's words:

> Supporting video generation with equipment built into other formats and support scene transitions.

Today a scene's animation only exists as an animated SVG. There's no way to turn it into a video or another format to share, and no way to go from one scene to another.

## Proposed outcome
- Pixel Lab can generate a video of a scene's animation, with export to other formats built in.
- Scenes can transition from one to another.

## Affected users and systems
- The owner in the app: exporting, and the timeline.
- Agents and the CLI, which should be able to export too.
- Scenes in the document, if transitions are kept there.
- What Publish writes for the site, if transitions reach it.

## Constraints
- Pixel art stays crisp: nearest-neighbor, integer scaling.
- Every action is something the CLI, the app and agents can all do.
- No new dependencies without asking.

## Open questions
- "Equipment built into other formats" was dictated; the draft reads it as "export built in to other formats". Is that right?
- Which formats: video (MP4, MOV), GIF, a sprite sheet, PNG frames?
- Are transitions only for videos, or does the site get them too?
- What kinds of transition, and are they set per pair of scenes or as a sequence of scenes?
- What controls the export needs: length, frame rate, size, loops?
