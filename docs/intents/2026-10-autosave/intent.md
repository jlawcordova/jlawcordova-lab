# Intent: Autosave
Author: J. Law. Cordova (owner). Status: draft.

## Problem
Saving in Pixel Lab is something I have to remember to do. Edits stay in the app until I press Save or ⌘S, and only then are they checked and written to the document. Until then the work can be lost, an agent proposing against the same scene clashes with edits I haven't saved, a chat leaves my unsaved edits out of what it sends Claude and asks me to "Save now", and closing or quitting asks whether to save.

While redesigning the app's chrome (`../2026-10-floating-library/intent.md`), I said: "even the save, we can remove it and just have the system be on autosave". That redesign moves Save into the right panel for now, because autosave touches enough to need its own change: "we can have the autosave in a separate intent instead, since this could cover a lot."

## Proposed outcome
The document saves itself. I edit, and the edits reach the document without my pressing Save, so there's nothing to remember and nothing to lose when I close or quit. The Save button can go.

## Affected users and systems
- Me in the main window: scene mode and object mode, closing and quitting, Save As and Publish.
- Agents over MCP and the Agent chat: their proposals, how they see what I have open and unsaved, and approving a proposal while I'm editing the same doc.
- A doc that changes on disk while I have it open (the Merge, Reload and Keep Mine banner).
- The document in the site checkout, which is under git, and what the site publishes from it.
- The acceptance pass and the script hook, which save explicitly today.

## Constraints
- The document stays the source of truth, and a save still checks it: autosave must never write an invalid document.
- Agents' edits still go through proposals I approve; direct writes stay off.
- Undo keeps working as it does now.
- No new crates, npm or Swift dependencies. The app's rules hold: board 03 tokens, VoiceOver labels, visible focus and Reduce Motion support on anything new.
- Tests and the acceptance pass run on temp copies, never on my document or the examples.
- Out of scope: the chrome redesign itself, which keeps an explicit Save until this lands.

## Open questions
- When a save happens: after each edit, after a pause in editing, when the window loses focus, on quit, or some mix.
- What happens when an autosave finds the document invalid: does the doc stay unsaved, with a quiet warning rather than an alert?
- What becomes of "unsaved in Pixel Lab" for agents, and of the Merge / Reload / Keep Mine flow, once unsaved states last only a moment.
- Whether undo history survives a save, so I can still undo past it.
- Which Mac document conventions to follow: "Edited" in the title while a save is pending, and Versions or Revert To for going back.
- Frequent writes make noisy git diffs in the site repo, where the document lives. Is that acceptable, or should saves be less frequent there?
- What happens to Save As and Publish: do they stay as they are, and does Publish save first?
