# Intent: Autosave, with version history
Author: J. Law. Cordova (owner). Status: accepted.

## Problem
Saving in Pixel Lab is something I have to remember to do. Edits stay in the app until I press Save or ⌘S, and only then are they checked and written to the document. Until then the work can be lost. An agent proposing against the same scene clashes with edits I haven't saved. A chat leaves my unsaved edits out of what it sends Claude and asks me to "Save now". Closing or quitting asks whether to save.

While redesigning the app's chrome (`../2026-10-floating-library/intent.md`), I said: "even the save, we can remove it and just have the system be on autosave". That redesign moves Save into the right panel for now, because autosave touches enough to need its own change: "we can have the autosave in a separate intent instead, since this could cover a lot."

Agents work the same way today: what they change waits as a proposal until I approve it in the Agent window. Starting this change, I said: "we may no longer need the proposal state for agents and the MCP. Anything the agent does immediately applies or autosaves." That's less risky with autosave, because "we should have the version history of the file available", as Apple's guidelines for Mac apps suggest.

Today more than one thing can write the same document: the app, the CLI in a terminal and each agent, each holding its own copy. I want "just one writer, even with the MCP".

## Proposed outcome
- **The document saves itself.** I edit, and the edits reach the document without my pressing Save, so there's nothing to remember and nothing to lose when I close or quit. The Save button can go.
- **Version history, the Mac way.** I can browse earlier versions of the document and go back to one, as Pages, Keynote and TextEdit let me. Each version says who made it: me, or an agent and what it did. Undoing what an agent just did is one step.
- **Agents' edits apply right away.** Nothing waits for approval: an agent's change lands in the document, and I see it on the canvas as it happens. Proposals, and approving them in the Agent window, go away. Version history is how I undo an agent's work, and publishing to the site is still my choice, so nothing reaches the site until I publish.
- **One writer per document, and it's the app.** While the app has a document open, it alone writes that document. The CLI and agents send their edits to it. When the app doesn't have the document open, the CLI and agents write it themselves.
- **Opening a document twice.** Opening a document that's already open, in another window or another copy of the app, brings the window that has it to the front instead of opening a second copy, as Mac apps do.
- **A crash never leaves a document stuck.** If the app quits or crashes, the CLI and agents can carry on with the document straight away.

## Affected users and systems
- Me in the main window: scene mode and object mode, closing and quitting, Save As and Publish.
- Agents over MCP and in the Agent chat: how their edits reach the document, and what the Agent window is for once proposals are gone.
- The CLI in a terminal, when the app has the same document open and when it doesn't.
- A document that changes on disk while I have it open (the Merge, Reload and Keep Mine banner), for example after a `git pull` in the site repo.
- The document in the site checkout, which is under git, and what the site publishes from it.
- `../2026-10-agent-coworker/intent.md`: its constraint "What agents change stays a proposal the owner approves" no longer holds once this lands.
- The acceptance pass and the script hook, which save and approve proposals explicitly today.

## Constraints
- The document stays the source of truth, and a save still checks it: autosave must never write an invalid document.
- Nothing reaches the site except through Publish.
- Undo keeps working as it does now.
- No new crates, npm or Swift dependencies. The app's rules hold: board 03 tokens, VoiceOver labels, visible focus and Reduce Motion support on anything new.
- Tests and the acceptance pass run on temp copies, never on my document or the examples.
- Out of scope: the chrome redesign itself, which keeps an explicit Save until this lands; comments and the agent-coworker work.

## Open questions
- When a save happens: after each edit, after a pause in editing, when the window loses focus, on quit, or some mix.
- What happens when an autosave finds the document invalid: does the doc stay unsaved, with a quiet warning rather than an alert?
- Undo with an agent in the document: does ⌘Z undo the agent's last change too, or only mine, with the agent's undone through version history?
- How far back version history goes, and whether agents can browse it and go back to a version themselves.
- Whether I want any way to look over what an agent changed after the fact, beyond the version it made.
- Frequent writes make noisy git diffs in the site repo, where the document lives. Is that acceptable, or should saves be less frequent there?
- What happens to Save As and Publish: do they stay as they are, and does Publish save first?
- In conversation I agreed with a per-document claim that the system lets go of when the app dies, so the CLI and agents can tell whether the app has a document open. The spec should take that up.
