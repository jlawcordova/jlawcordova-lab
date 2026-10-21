# Intent: A scene without a size, and keyboard block placement
Author: J. Law. Cordova (owner). Status: accepted.

## Problem
Block placement needs better controls. With a block or object selected, the arrow keys move its row and column, and the owner wants the keyboard to handle its level, and placing one block on top of another, just as easily.

Making a scene gives it a size in pixels from the start. The size is a guess made before anything is in the scene, and when it doesn't fit the objects, the art gets cut off in odd places. Fixing it means working out the right size by hand and changing it again whenever the scene grows or something moves.

## Proposed outcome
A scene has no size while it is being made. The canvas works as if it were infinite: the owner places things wherever they like and nothing is cut off. The size is decided only when the scene is published, so the published art fits what is in it and no longer has cut edges.

Block placement is easy to do from the keyboard alone: the owner moves a selected object or block across row and column, up and down a level, and places a new one on top of another, without the mouse. The keys that already work keep working, and the spec recommends the keys for what is missing.

## Affected users and systems
The owner making scenes in the app. Agents making scenes over the CLI or MCP get the same behavior. Published scenes in the site checkout: the owner's existing scenes must keep publishing exactly as they do now.

## Constraints
- The published art stays self-contained, as the site expects.
- No change to the site's art or pages.
- Existing documents and their scenes keep working and publish the same.
- Keyboard use, VoiceOver labels and Reduce Motion support stay as they are.
- The keys that work today keep working.
- Out of scope: changing how objects are sized.

## Open questions
- Should the owner still be able to fix a scene's published size on purpose, for example where a page depends on it?
- Should the size cover everything that moves during an animation, or only the scene at rest?
- Should the owner see, while editing, what size the scene would publish at?
- How much space should the published art leave around its contents?
- The spec recommends the keys for changing level and for placing on top of a block.
