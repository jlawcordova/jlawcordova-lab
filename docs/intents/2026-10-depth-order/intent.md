# Intent: Draw a scene's items by where they stand
Author: J. Law. Cordova (owner). Status: accepted.

## Problem
After moving a block in the harbor example, things overlapped the wrong way: an item I'd moved behind another still drew over it. I asked whether the drawing order follows the order items are listed in the scene. It does, and moving an item never changes that order.

What was found, on temp copies of `examples/harbor.pixellab`:

- A scene's items paint in the order they're listed, first at the back.
- Placing an item slots it into that list back to front, by its tile's row and column and then its level. Moving it afterwards (by command, in the inspector, as part of a moved layer, or by dragging or the arrow keys in the app) leaves it where it was in the list.
- So in `harbor-flat`, moving the front pine (tile 5,6, level 0) by −5,−5 to tile 0,1 puts it behind the red house, but it still draws over the house's wall.
- Even where placing does the sorting, it only looks at the tile an item stands on. It can't order a block wider than one tile (the hall's 2×1 walls and roof) or a raised item against the tiles around it.
- The README and the scene-layers docs say each item "draws back to front by where it stands". After a move that stops being true.

## Proposed outcome
In the owner's words: the z-order should be "based on the row, column, and level instead, and then having a separate property to allow it to overlap everything else".

- An item's depth comes from where it stands: its column, row and level, and the size of its block. Moving an item, however it's moved, draws it at its new depth, with no reordering by hand.
- Each item can also carry an explicit override that draws it in front of everything else, or behind, for the cases where standing position gives the wrong answer.
- What the docs say ("each item draws back to front by where it stands") is true again, after any edit.

## Affected users and systems
- The owner editing scenes in the app: placing, dragging, nudging, moving layers, and the inspector.
- Agents editing scenes over the CLI and MCP, who should get the same result.
- The published SVGs: the site's document and the documents in `examples/`.
- The site, which takes the engine through its `pixellab` pin.

## Constraints
- The document stays the source of truth, and the engine change is made here and released by tag; the site adopts it by bumping its pin, under its own workflow.
- Scene layers still never change paint order on their own.
- The site's `{ group }` wrappers (the SVG's `<g>`s with their enter animations) can't be split apart.
- No new dependencies.

## Open questions
For the spec to answer or carry forward:

- Is the order worked out when a scene is drawn, or does editing keep the list sorted after every change?
- What happens to Raise and Lower once depth comes from position: do they become the override, or go away?
- How are items placed by pixel, rather than on a tile, ordered?
- How do the site's `{ group }` wrappers sort, given they can't be split?
- How are items with motion tracks ordered, when they move across tiles over time?
- Published SVGs for existing art (the site's document and `examples/`) may change. The site would then need a `pixellab` pin bump, and the acceptance pass's pinned expectations would need updating. Is that acceptable, or should existing art come out unchanged?
