# Intent: Object libraries and palette files
Author: J. Law Cordova (owner). Status: accepted (in chat on 2026-10-23; a revision of the intent accepted in PR #42, for the autosave and floating Library changes that merged since).

## Problem
In the owner's words:

> The ability to support object libraries. So we can import object libraries into a specific Pixel Lab file. So we might need to support other types of Pixel Lab files like object library.

Today every object lives inside the one `.pixellab` document that uses it. There's no way to keep a set of objects on their own and bring them into another document.

A flat list of objects is also hard to navigate. A library that holds many objects needs some way to group them.

Palettes have the same problem. Each document has its own, so moving objects between documents means checking colors by hand, and nothing keeps two documents' palettes the same.

## Proposed outcome
Object libraries:
- A new kind of Pixel Lab file, `.pixellib`: an object library, holding objects without being a document of scenes.
- The app can export a document's objects to a `.pixellib`.
- Categories can be nested, so the owner can find an object without scanning a flat list.
- In the Library popover, categories are tabs below the search. A category's subcategories show as headings inside its tab.
- The first tab, All, is the default and shows every object, grouped under its category headings. Objects with no category, such as every object in today's documents, are in their own tab, Others, placed last.
- An object's category is set in the object inspector, while editing the object, before exporting.
- A document can import a `.pixellib`. Importing copies the library's objects into the document, so the document stays self-contained and later changes to the library don't reach it. The imported objects keep their categories, which show in the document's Library popover.
- A document can import more than one library. When two objects share a name, both are kept and the newer one gets a suffix.
- The document's world palette colors the imported objects. A library keeps the values its objects' colors had when it was exported, as a record rather than a palette the document uses. If those differ from the document's, the owner gets a warning, so an import never changes how an object looks without saying so.
- A library object that uses a palette color the document doesn't have can't be imported as it is. The app offers to convert the missing color to one the document already has, and says plainly that this changes how the imported object looks.
- The app has no way to edit a `.pixellib` in this change. The CLI, and so agents, can edit its categories.

Palette files:
- A new kind of Pixel Lab file: a palette, kept on its own so more than one document can use the same colors.
- A document's palette can be exported to a palette file.
- A document can import a palette file. Importing copies it, like a library, so later changes to the palette file don't reach the document.
- Importing a palette file adds the colors the document doesn't have yet. It doesn't replace the document's palette. Where a color the document already has differs, the owner gets a warning and decides.
- A library neither links to nor carries a palette file.

Both kinds of import follow one rule: an import never changes how existing art looks without telling the owner. Imported objects are the document's own once copied, and can be edited like any other.

## Affected users and systems
- The owner in the app: setting categories in the object inspector, finding objects by category in the Library popover, exporting a library, importing a library into a document, and using a palette file.
- Agents and the CLI, which should be able to do the same, plus editing a library's categories.
- `.pixellab` documents, the examples, and what Publish writes.

## Constraints
- Every action is something the CLI, the app and agents can all do. Agents' edits apply as they're made, like any other edit, and the version history takes them back.
- Never write to the owner's real document from tests or scripts.
- No new dependencies without asking.
- Out of scope: editors in the app for `.pixellib` and palette files. Both stay future options.
- Out of scope: linking a document to a library or a palette file, so either one changing or going missing is not a case to handle.

## Open questions
None: the owner answered the earlier ones while drafting. The spec may raise its own.
