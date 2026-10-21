# Intent: A floating library
Author: J. Law Cordova (owner). Status: draft.

## Problem
In the owner's words:

> We need better library placing in the UX. Right now, the library is in the left sidebar, but we want to move it into something that floats in the middle, similar to what Figma does.

The Library is a tab in the left sidebar today, beside Layers. Placing from it means switching the sidebar away from Layers and picking from a narrow column.

## Proposed outcome
The Library moves out of the left sidebar into a panel that floats over the middle of the window, the way Figma's does, so picking something to place is closer to where it's placed.

## Affected users and systems
- The owner in the app's scene mode: the sidebar, the stage and placing.
- Object libraries, if they land (`../2026-10-object-libraries/intent.md`).

## Constraints
- Board 03 tokens; pixel art drawn nearest-neighbor at integer zoom.
- VoiceOver labels, visible focus, a keyboard path, comfortable hit targets and Reduce Motion.
- No new dependencies without asking.

## Open questions
- How is the floating Library opened and closed, and does it stay open while placing?
- Does the left sidebar keep anything of the Library, or become Layers only?
- Does it search, or group objects (by kind, or by library once there are several)?
- Is the design worked out in a prototype first, as with scene layers and the agent chat?
