# Intent: Scene layers
Author: J. Law. Cordova (owner). Status: accepted (asked for directly on 2026-10-20, with the design agreed through a prototype; reviewed with its spec, plan and code in one PR, at the owner's request).

## Problem
Scenes with hundreds of items are hard to work with. Today the Items tab lists hero-island's 219 one-item entrance wrappers (`{ group: { class: 'enter', 'data-enter': … }, items }`), which says nothing about what's in the scene. I can't name a part of a scene (the Mine, the Railway, the Train on it), select it, move it, hide it or lock it as one thing, or place new things into it.

## Proposed outcome
Named layers that can nest (the Mine, the Railway, the Train inside it) and that I can select, move, hide, lock, navigate and place into. The prototype (a Design canvas running on the real hero-island art, with its interactions wired: https://claude.ai/code/artifact/00b2cadf-75a5-4c2b-b510-21b31f44552f, `project/Main.dc.html`) is the design. In it:

- **Sidebar tabs:** Layers first and selected by default, then Library. Layers replaces the Items tab.
- **Layers panel:** a tree of layers (dashed-square icon, name in Sora) with their items (cube icon, tile position on the right). No counts on layer rows, the LAYERS heading, the SCENES heading or scene rows. Each layer row has a chevron, an eye to hide it, and a lock that shows on hover or while locked.
- **Stage:** a click picks the outermost layer under the pointer, hit-tested by pixels, not by bounds. A double-click goes inside a layer: everything outside it fades, a breadcrumb pill at the top left (`hero-island › Railway › Train`) gets a Done button, and clicks then pick the next level down. ⇧-click adds to the selection. Dragging moves every item in the selection, snapping to whole tiles, with a label that shows the col/row change. Hidden layers don't draw. Locked layers can't be picked on the stage, except from inside them. There's no hint bar at the bottom of the stage.
- **Right-click menu:** right-clicking the stage or a Layers row opens a menu with Group ⌘G and Ungroup ⇧⌘G, then a separator, then Go Inside, Hide and Lock. Right-clicking something outside the selection selects it first; right-clicking something already selected keeps the selection. There are no toolbar buttons for grouping. Group makes a new layer under the selection's common parent, named "Group 1", "Group 2" and so on, renamed in the inspector. Ungroup moves a layer's contents up one level, and they stay where they are.
- **Inspector:** for a layer, a name field, "n items · in Railway", and Go inside, Hide and Lock buttons; then "Moved by" with col and row fields and four diagonal nudge buttons, and a Place into it button. For an item, its position and a "Select <layer>" button. For several things selected, a count and a pointer to right-click › Group.
- **Library:** a "New items go into <layer>" chip. Placing goes into the selected layer, or the selected item's layer, or the layer I'm inside. With none of those, the item goes in at the top level (a change from the prototype).

## Affected users and systems
- The owner in the app's scene mode: the sidebar, the stage, the inspector, the Library and the Edit menu.
- Agents and the CLI: every layer action is something they can do too, and agents' edits stay proposals.
- Scene files in a `.pixellab` document, and what Publish writes from them.
- The site's entrance wrappers, which must keep working as they do.

## Constraints
1. Layers never change paint order or output. Every item still draws back to front by where it stands. Publishing a document with layers must give byte-identical output to the same document without them, and that's tested.
2. Layers are separate from the site's entrance `group` wrappers. Don't repurpose or break the wrappers; `scene group set` keeps working. Leave the wrappers out of the Layers tree.
3. My lean on storage: a scene-level `layers` list (`{ id, name, parent }`) plus a `layer` on each item, saved in the scene file so the CLI, the MCP server and agents all see them. Hidden and locked are view state: kept by the app for each document, never written to the document or published.
4. Every action is a registry command: make a layer from items, rename, ungroup, move items into a layer, place into a layer, and move a layer by tiles, so the CLI, the app and MCP agents all have them, and MCP edits stay proposals.
5. Existing documents have no layers; their items show at the top level of the tree. Never write to my document in the site repo.
6. The script hook gets actions and `expect` lines for all of it, and the acceptance pass covers layers.
7. VoiceOver labels, visible focus, and a keyboard path (the Layers tree is the keyboard way to select). Honor Reduce Motion for the fade. Board 03 tokens, no `@State`, no new dependencies.
8. Update the README (the tour and shortcuts, including ⌘G and ⇧⌘G) and anything in `docs/` that mentions the Items tab.

Out of scope: changing the site's art or its wrappers, and changing how the site publishes.

## Open questions
- Where layers are stored is the spec's call (my lean is above).
- Whether the app still shows the wrappers' attributes anywhere is the spec's call.
