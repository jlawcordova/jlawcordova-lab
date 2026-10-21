# Intent: Object libraries
Author: J. Law Cordova (owner). Status: draft.

## Problem
In the owner's words:

> The ability to support object libraries. So we can import object libraries into a specific Pixel Lab file. So we might need to support other types of Pixel Lab files like object library.

Today every object lives inside the one `.pixellab` document that uses it. There's no way to keep a set of objects on their own and bring them into another document.

## Proposed outcome
- A new kind of Pixel Lab file: an object library, holding objects without being a document of scenes.
- A document can import an object library, so its objects are available to place in that document's scenes.

## Affected users and systems
- The owner in the app: opening and editing a library, importing one into a document, and the Library they place from.
- Agents and the CLI, which should be able to do the same.
- `.pixellab` documents, the examples, and what Publish writes.

## Constraints
- Every action is something the CLI, the app and agents can all do; agents' edits stay proposals.
- Never write to the owner's real document from tests or scripts.
- No new dependencies without asking.

## Open questions
- Is an imported object copied into the document, or linked so changes to the library reach every document that uses it?
- If linked, what happens when a library changes or goes missing?
- Can a document import more than one library, and what happens when two objects share a name?
- Does a library carry its own palette, or use the document's?
- Can objects be edited from inside the document that imports them?
- Are there other kinds of Pixel Lab file the owner wants besides documents and object libraries?
