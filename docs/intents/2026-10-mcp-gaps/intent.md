# Intent: Keep agents' docs in step with disk, and fill the MCP's gaps
Author: J. Law. Cordova (owner). Status: accepted (asked for directly on 2026-10-20; reviewed with its spec, plan and code in one PR, at the owner's request).

## Problem
An agent edited the site's art (jlawcordova.github.io) over the MCP while I had Pixel Lab open, and two kinds of trouble came up.

**Stale docs.**

1. The MCP session's copy goes stale after its own proposal is applied. `doc_save` in proposal mode leaves the doc dirty. Once I apply the proposal, `doc_status` reports it stale. The only way back was `doc_revert`, which a permission check blocks as destructive.
2. Proposals carry old edits. A proposal made after an earlier one was applied listed the earlier edits in its summary, though its diff held only the new one.
3. The app keeps showing old art. A doc open in the app with unsaved edits doesn't reload when its file changes on disk. I kept looking at old art without knowing it had changed.
4. Conflicts are only found at save time. `doc_status`'s stale flag shows up only when the agent asks, and the agent can't see that I have the same doc open with unsaved edits.

**What the MCP can't express.** These had to be written into the scene source by hand:

5. Group attributes. `scene_move` moves an item but leaves its enter group's `data-x` and `data-y` at the old point, and the site's tests pin them to the item's foot.
6. Items inside groups. `scene_place` can't create an item inside a new or existing group, such as `enter` or `data-icon` groups.
7. Motion tracks. Only move keys can be set, through `scene_move` with a time. Tracks can't be added or removed, and show and pose keys can't be set at all.
8. Off-grid moves. A move whose dx and dy aren't on the 2:1 or 1:1 grid steps in gcd(dx, dy) jumps, so (16, −3) is a single 16 px jump, and nothing says so.
9. Whole-map edits. Redrawing a map meant one `object_paint` call per color.

## Proposed outcome
- After a proposal from an MCP session is applied, or rejected, that session brings the doc back in line with disk on its own: it's clean when its content matches the file, and reloaded otherwise. The next edit starts from disk, with no revert.
- An agent can reload a doc, but only when doing so drops no unsaved work (such as edits already applied as a proposal); otherwise it's refused.
- A proposal's summary lists only the edits since the doc last matched disk.
- When a doc I have open with unsaved edits changes on disk, the app says so clearly and offers to merge the change into my open copy (a three-way merge against the version I opened), reload it, or keep mine. Approving a proposal for that doc goes through the same merge instead of leaving my copy stale.
- An agent can see which docs the app has open with unsaved edits, so it can warn before it proposes against one.
- Over the MCP, an agent can: keep a group's `data-x` and `data-y` in step when it moves the item they point at, or edit a group's attributes directly (both, if they're cheap); place an item inside a new or existing group; add and remove motion tracks, and set, move and delete show, pose and move keys; and replace a whole map of one layer or frame in one call and one undo step, checked against the palette and the size caps.
- `check` warns about any key-to-key move with fewer steps than its pixel length. Optionally, a helper turns a polyline into on-grid keys at a steady speed.
- A short note on each change goes to the site's pixel-art skill, so its "What the MCP can't express" list can shrink.

## Affected users and systems
- Agents editing over the MCP (`pixellab serve --mcp`), and the proposals they save.
- The owner in the app: open docs, the Agent window's Approve, saving.
- The CLI, which gets the same new commands.
- The site repo's pixel-art skill, which only receives notes; it isn't changed here.

## Constraints
- Plan before coding; add tests for each fix; update `docs/cli.md` and the MCP tool descriptions to match.
- Agents still propose and the owner still approves; nothing here lets an agent apply, reject or force a proposal.
- No new dependencies.
- Don't change the site repo, and don't commit there. Its art isn't touched; tests use temp copies.
- Done means the lab's tests pass, the docs and tool descriptions are updated, and the notes for the site's skill are written.

## Open questions
- None from the owner. The spec settles the design choices (names, merge granularity, what "keep mine" does).
