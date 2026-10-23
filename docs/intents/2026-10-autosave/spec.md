# Spec: Autosave, with version history (from intent.md 2026-10-23)
Status: approved (written in the intent's PR, at the owner's request; approved 2026-10-23 with the recommended answers to the areas of concern).

Words used here: the **document** is one `.pixellab` folder; a **doc** is one scene or object in it (`scene:hero-island`), the unit the session opens, undoes and saves; the **owner** of a document is the one process that writes it while the app has it open (R22).

## Requirements

**Autosave**

- **R1. Edits save themselves.** A doc with edits is written about a second after its last edit, whoever made the edit (the owner in the app, an agent, the CLI). It is also written straight away when the window stops being the key window, when the doc or document closes, when the app quits, before Publish, before Save As and before a chat message is sent.
- **R2. Autosave never writes an invalid doc.** A doc with problems stays unsaved. The title bar's subtitle says "Not saved: 2 problems", and the right panel shows the problems with a way to fix them (Check). The doc saves on its own once it's valid again. Nothing pops up while the owner edits.
- **R3. The window shows the save state.** The subtitle reads "Saved", "Edited" (a save is pending) or "Not saved: <n> problems", and the window's close button shows the edited dot only while a save is pending or blocked. VoiceOver reads the same state.
- **R4. Closing and quitting don't ask, unless something can't be saved.** With every doc saved, closing and quitting just happen. With a doc that has problems, one alert names it and offers **Review** (cancels and shows the problems), **Save Anyway** (writes it as it is) and **Discard Changes**.
- **R5. The Save button goes; ⌘S saves a version.** The right panel loses its Save button. File › Save (⌘S) stays, as on the Mac: it saves at once and starts a new version (R9). File › Revert to Saved becomes File › Revert To › Last Opened and Browse All Versions… (R12).
- **R6. Undo survives saves.** Undo and redo work across autosaves exactly as they do across saves today. A save is never an undo step.
- **R7. Changes from outside Pixel Lab still merge.** When a doc's file changes on disk (a `git pull`, an editor), a clean doc reloads, as today. A doc with a pending save gets the change merged into it, three ways, and saves. Only a merge where both sides changed the same lines shows the banner, now offering **Reload** and **Keep Mine** (Keep Mine overwrites the file with the open copy).

**Version history**

- **R8. Every write is in the history.** Whoever writes the document (the app, an agent, the CLI) records what it changed in the document's version history, with who changed it: the owner, an agent (its name, and the chat it worked in), the CLI, or a change from outside Pixel Lab.
- **R9. A version is a stretch of one author's work.** Writes by the same author run into one version until someone else writes, the author pauses for a minute, ⌘S is pressed, the chat turn that made them ends, or the document closes. A version lists its edits ("Placed pine at 3,-1,1") and the docs it touched.
- **R10. History thins out over time, as the Mac's does.** Every version from the last day is kept, then one a day for a month, then one a week. Thinning folds versions together and never loses the state of the document at a kept version.
- **R11. History lives with Pixel Lab, not in the document.** Nothing is added to the `.pixellab` folder, so the site repo and what it publishes don't change. History belongs to the document's location on disk.
- **R12. The owner browses versions in the app.** File › Revert To › Browse All Versions… opens the Versions sheet. It lists versions newest first, grouped by day, each with its time, author, edits and docs. Selecting one shows the open scene or object as it was then, as a ghost on the stage, with its diff a click away.
- **R13. Going back.** From the Versions sheet: **Restore This Version** puts the whole document back as it was at that version. **Restore <doc> Only** does that for the doc on stage. **Undo These Changes** takes back just what that version changed, keeping everything since, and refuses with an explanation when later edits touched the same lines. Each is one undo step and becomes a new version itself ("Restored to 3:02 PM"), so going back can be undone too. File › Revert To › Last Opened restores the document as it was when this window opened it.
- **R14. Undoing what an agent just did is one step.** A chat's change card (R18) and the version row both offer **Undo These Changes** for that agent's version.
- **R15. History reaches all three doors.** Listing, showing, rendering, restoring and undoing a version are commands, so the CLI, the app and agents over MCP can all use them.

**Agents' edits apply right away**

- **R16. An agent's edit is an edit.** Over MCP, an edit changes the document as it's made, and autosave writes it. There are no proposals: no waiting, approving or rejecting, no `--direct`, and no proposal tools. Publishing stays the owner's: nothing reaches the site until the owner publishes, and agents still can't publish.
- **R17. The owner sees agents' edits live.** While the app has the document open, an agent's edit lands in the app's open copy and the stage redraws, exactly as if the owner had made it.
- **R18. The chat shows what changed, not a proposal.** In the Agent chat, a turn that edited ends with a change card: CHANGED, the docs and edits, **Undo These Changes**, and **Show in Versions**. After undoing, the card says Undone.
- **R19. The Agent window drops proposals.** It keeps Claude Code's status, the MCP setup and the activity feed. The toolbar and Dock badges for waiting proposals go.
- **R20. Undo with an agent in the document.** The owner's ⌘Z undoes the latest step in the doc, whoever made it, and its menu item says whose it is ("Undo Place Pine (Claude)"). An agent's `doc_undo` undoes only its own latest step, and refuses when someone else has edited the doc since.
- **R21. Proposals already waiting are kept, not lost.** On the first run of this version, waiting proposals move out of the way, and the Agent window says once how many and where their files are.

**One writer per document**

- **R22. The app owns the documents it has open.** While the app has a document open, it alone writes that document. The CLI and agents' MCP servers send their commands to it and get its answers. Their edits, reads and renders all see the app's open copy.
- **R23. Without the app, they write themselves.** When no app has the document open, the CLI and MCP servers open it and write it themselves, saving after each command, as the CLI does today.
- **R24. Ownership follows the app.** When the app opens a document, the CLI and MCP servers send it their next command. When it closes the document or quits, it saves everything first, and their next command writes the document themselves. A command in flight when the app goes away is reported, never run twice.
- **R25. A crash never leaves a document stuck.** If the app or its server dies, however it dies, the document is free at once for the next app, CLI or MCP server.
- **R26. Opening a document twice brings its window forward.** A second copy of the app opening a document another copy has open brings that copy's window to the front instead, and opens nothing itself.
- **R27. Only the owner's own user can reach it.** Nothing outside the owner's user account can send commands to the app.
- **R28. A mismatched program can't write through the app.** A `pixellab` of another protocol version than the app's gets a clear refusal for edits while the app has the document open, naming both versions; its reads still work, from disk.

**Cross-cutting**

- **R29. Every new command reaches all three doors,** with tests named after these requirements, `docs/cli.md` regenerated, and the MCP server's instructions and tool descriptions rewritten for edits that apply.
- **R30. The script hook drives all of it** (save state, versions, restore, undo, an MCP client forwarding through the app, a second owner refused), and the acceptance pass covers it on temp copies, never the owner's document or `examples/`.
- **R31. The docs follow:** README (tour, shortcuts, MCP setup, the Agent chat), `docs/architecture.md`, the state dir's description in `pixellab help`, and a "superseded in part" note in the records that say agents propose (`../2026-10-mac-app/plan.md`, `../2026-10-agent-chat/spec.md`).

## Design

### D1. Autosave in the session (R1, R2, R6)

Autosave lives in the program's session, not the app, so every edit is saved the same way whoever made it. `Options` gains `autosave_ms: Option<u64>`: the stdio server sets 1000; the CLI and the MCP server, running with no owner (D10), save at the end of each call instead, as the CLI does now.

- Each entry gets `dirty_since: Option<f64>`, set by an edit, undo or redo and cleared by a save. The stdio server's tick (already every 250 ms, `watch::POLL_MS`) calls `session.autosave(now)`, which saves each doc whose last change is at least `autosave_ms` old.
- A save that finds problems doesn't error at the user. The entry records `blocked: Problems(n)` and isn't retried until it changes again. A save that finds a conflict (the file changed since it was opened) runs `merge` (D4 of `../2026-10-mcp-gaps/spec.md`) and saves the result; a clash records `blocked: Conflict` and sends `doc.stale` as today (R7).
- New command `doc.flush {doc?}` saves every pending doc now (or one), returning the save state of each. The app calls it on the triggers in R1, and the CLI and clients call it where they need the file current (D10, `--dry-run`).
- `doc.changed` gains `saveState: "saved" | "pending" | "problems" | "conflict"`, `problems: n` and `author` (D8). `doc.save` stays: it flushes the doc and closes the current version (R5, D3).
- The undo stack is kept across saves (it already is). `coalesce` still stops at a save, so a repeat edit after an autosave is a new step. That's a change from today only in how often saves happen.

### D2. The app (R3, R4, R5, R7)

- `AppModel.save` becomes `flush`. Its alerts go: `invalid` shows as the save state, and `conflict` is handled by the merge.
- The subtitle comes from `saveState`. `NSWindow.isDocumentEdited` is true while any open doc is `pending`, `problems` or `conflict`.
- `confirmUnsaved` asks only about docs in `problems` or `conflict`: "Hero Island has problems and can't be saved." **Review** / **Save Anyway** (`doc.save {force}`) / **Discard Changes**, with Review the default button and Discard destructive.
- `RightColumn` drops Save (and `saveLabel`). The tab row reads **Inspector | ✦ Agent … Publish**. A doc in `problems` shows a gold "Not saved: 2 problems" row at the top of the Inspector, with Check, which runs `check` and lists the problems, as today.
- Menus: File › Save (⌘S) → `doc.save` on the open doc (flush and close the version). Save As… stays and flushes first. Revert to Saved is replaced by a Revert To submenu: Last Opened, Browse All Versions…. Publish flushes first and refuses, as today, while a doc has problems.
- `WindowGuard` and `applicationShouldTerminate` call `flush` and then `confirmUnsaved`. `NSApplication.willResignActive` and the window's `didResignKey` call `flush`, and so does `ChatModel` before it sends.
- `StaleBanner` keeps Reload and Keep Mine and loses Merge, since a merge is tried first.

### D3. Versions on disk (R8–R11)

In the state dir, per document:

```text
history/<key>/            key: the first 16 hex of sha256 of the document's absolute path
  document.json           { path, createdAt }
  versions.jsonl          one version per line
  blobs/<sha256>.json     a source file's text, by its hash (shared by every version that has it)
  lock                    held (File::lock) while a writer changes this folder
```

A version is `{ id, number, startedAt, endedAt, author, edits: [label], changes: [{ file, before, after }], open }`. `before` and `after` are blob hashes, with `null` for a file that didn't exist. `author` is `{ kind: "owner" | "agent" | "cli" | "outside" | "restore", name?, chat?: { id, title } }`. Version 0, written the first time a document with no history is opened, has `author.kind: "opened"` and every source file as `after`, so any version's full state is version 0 plus the changes up to it.

- **Recording.** `Session::save` (every writer's save, D10) calls `history.record(file, before, after, author, labels)` after the file is written. Under `lock`, it extends the last version when that version is `open`, has the same author and ended less than 60 s ago, keeping the first `before` and replacing `after`; otherwise it closes it and appends a new one. Extending rewrites only the last line of `versions.jsonl`.
- **Closing.** `history.mark {label?}` closes the open version. `doc.save` (⌘S) calls it. So do the app at the end of a chat turn that edited, the stdio server at shutdown, and anything that writes as another author.
- **Outside changes.** When the session reloads or merges a file changed on disk, it records a version authored `outside` ("Changed outside Pixel Lab") for that file, from the file's last known hash to its new one.
- **Thinning** runs when a document opens. Versions older than a day are folded into one per calendar day; older than 30 days, one per ISO week. A fold keeps the first `before` and last `after` of each file, the union of edits and the list of authors (`author.kind: "several"`). Unreferenced blobs are deleted.
- **Size.** Documents are small (the site's is about 550 KB over 107 files), and a version stores only the files it changed, deduplicated by hash.

### D4. History commands (R12–R15)

| Command | Params | Does |
| --- | --- | --- |
| `history.list` | `limit?`, `doc?` | versions newest first: number, times, author, edits, docs |
| `history.show` | `version`, `doc?` | one version's edits and its unified diff per file |
| `history.render` | `version`, `doc`, render params | the doc as it was after that version, as a PNG (`scene.render`'s and `object.render`'s output) |
| `history.restore` | `version`, `doc?` | puts every file (or one doc's) back as it was after that version: one undo step per doc, saved, recorded as a `restore` version |
| `history.undo` | `version` | takes back that version's changes: for each change, `merge3(base: after, mine: current, theirs: before)`; refuses (code `clash`, the files in `details`) when any file clashes, writing nothing |
| `history.mark` | `label?` | closes the open version |

All are in the registry, reach the CLI, `serve --stdio` and `serve --mcp`, and are forwarded to the owner like any other command (D10). "Last Opened" is `history.restore` with the version the stdio server noted when it opened the document.

### D5. The Versions sheet (R12, R13)

A sheet on the main window, in board 03 tokens, night and day:

- **Left: the list.** Versions grouped under day headings, each row with its time, an author mark (the owner's initial, the chat's asterisk avatar, a terminal glyph for the CLI, a dashed outline for outside changes), the first edit and "+n more", and the docs touched. The open version is on top, marked "Now".
- **Right: the detail.** All its edits, the docs it touched, and **Show diff**. Its buttons: **Restore This Version**, **Restore <doc> Only** (when the doc on stage changed then), **Undo These Changes**. Restore asks first ("Restore the whole document to 3:02 PM? You can undo this."); Undo These Changes doesn't, since it's one ⌘Z away.
- **The stage** behind the sheet shows the selected version of the doc on stage as a ghost, through the existing ghost overlay (`ProposalGhost`, renamed `VersionGhost`) fed by `history.render`.
- **Keys:** ↑/↓ move through versions, Return restores (with the confirmation), ⌘⌫ undoes these changes, Esc closes. Every row has a VoiceOver label ("Version at 3:02 PM by Claude in Pine forest: placed 3 pines, in Hero Island"), focus is visible, and the ghost doesn't fade under Reduce Motion.

### D6. Agents (R16–R19, R21)

- `Mode::Propose`, `propose`, `sync`'s proposal settling, `Proposed` and `Entry.proposal` go. So do `proposal.list`, `proposal.show`, `proposal.render`, `proposal.apply` and `proposal.reject`, the `proposals.changed` notification, and `State`'s proposal functions (`proposal-numbers.log` stays readable for old chat cards).
- `serve --mcp --direct` is accepted and ignored, with one line on stderr ("--direct is no longer needed: edits apply"), so registrations made with it keep working.
- `doc_save` stays a tool: it flushes and marks the version, and its description says edits are already saved.
- `INSTRUCTIONS` is rewritten. Edits apply to the document as you make them and save themselves. While Pixel Lab has the document open, the owner sees them live. `doc_undo` takes back your last edit. `history_list`, `history_show` and `history_undo` let you look back or take back a whole turn. The `unsavedInApp` paragraph goes. The `LEFT_OUT` entries for proposals go; `art.build`, `document.copy` and `document.migrate` stay left out.
- **First run (R21):** `State::new` moves `proposals/` to `retired-proposals/<date>/`, as R25/D11 of the agent-chat spec did for format 1, and writes `retired-proposals/notice.json` with the count. The Agent window reads it, shows "3 waiting proposals were retired when edits started applying. Their files are in …" with Show in Finder, and deletes the notice when dismissed.
- **The chat.** `ChatModel` stops looking for `doc_save` proposals. At the end of a turn that made edits, it calls `history.mark` and keeps the version number, which the card shows (`ChangeCard`, replacing the proposal card): CHANGED, docs, edits, **Undo These Changes** (`history.undo`), **Show in Versions** (opens the sheet on that version), and Undone after. `StepText` says "saved" for `doc_save`. The Agent window loses `ProposalsSection` and its keys; its activity feed takes the space. The toolbar and Dock badges and the AgentModel proposal state go.
- **The selection block** (`chat_context.rs`) keeps naming targets by their path in the saved file. Since the app flushes before sending, the only targets left out ("not sent") are ones in a doc with problems.

### D7. Undo across authors (R20)

`Step` gains `author: Author`, the author of the request that made it (D8). The app's `doc.undo` and `doc.redo` take the top step as today, and `doc.changed` adds `undoLabel` and `redoLabel` with the author for anyone but the owner ("Place pine (Claude)"), which the Edit menu shows. From a client (D10), `doc.undo` undoes the top step only if it's that client's; otherwise it refuses with code `not-yours` ("The owner has edited hero-island since; use history_undo to take back your version"). `doc.redo` is the same. `doc.revert` from a client refuses (code `owned`); `doc.close` from a client succeeds without closing the app's copy.

### D8. Authors

Every request the session runs carries an author. The app's own requests on stdin are `owner`. A client connection's author comes from its `client.hello` (D10): `{ kind: "agent", name, chat? }` from `serve --mcp` (the MCP client's name, and `PIXELLAB_CHAT` if set), `{ kind: "cli" }` from the CLI. A local session (D10) uses its own author. The author reaches the undo step, the history version and `doc.changed`.

### D9. Owning a document (R22, R25–R27)

Per document, in the state dir:

```text
owners/<key>.lock    { pid, appPid, document, socket, protocol, version, startedAt }, held with File::lock by the owner
owners/<key>.sock    the owner's Unix socket (0600; owners/ is 0700)
```

- **Taking it.** `serve --stdio` with a document opens `owners/<key>.lock` and calls `File::try_lock()` (std, Rust 1.89+, the pinned toolchain is 1.99.1). If it gets the lock, it writes the contents, removes any stale `.sock`, binds the socket and starts a listener thread. If the lock is held, it answers the app's first request, `session.info`, with error code `owned` and the holder's `appPid`, and exits. The app then calls `NSRunningApplication(processIdentifier: appPid)?.activate()` and, having nothing else open, quits (R26). The app passes its own pid as `PIXELLAB_APP_PID`.
- **The OS lets go.** The lock is released when the process exits, however it exits (R25). A `.sock` left by a crash is removed by the next owner before it binds. A client that finds the lock free never trusts a `.sock` it sees.
- **Switching documents** in the app stops the server, which releases the lock, and starts one for the new document, which takes its own.
- **Long socket paths.** macOS caps a socket path at 104 bytes. When `owners/<key>.sock` is longer (a deep `PIXELLAB_STATE` in tests), the socket goes in `$TMPDIR/pixellab-<key>.sock` and the lock records where.
- **The event loop.** Each accepted connection gets a reader thread that sends `Event::Remote { conn, line }` into the stdio server's existing channel, so remote requests run one at a time, in order, between the app's own requests and ticks. Replies go back on the connection; notifications aren't sent to clients.
- **Security (R27).** The socket and `owners/` are readable and writable by the user alone, and the listener checks the peer's uid with `getpeereid`, declared as an `extern "C"` function since the std accessor is unstable and no crate may be added, and drops other users. Anything running as the user can already edit the document's files, so this adds no new reach.
- `open_docs.rs` (`open/<pid>.json`) and `unsavedInApp` are removed. The lock says everything they said.

### D10. Clients (R22–R24, R28)

`serve --mcp` before each tool call, and the CLI before each command that touches the document:

1. Compute the key and `try_lock` the lock file. **Got it:** there's no owner. Release it at once, and run the command in a local session that saves after each mutating command (the CLI's behavior today; `serve --mcp` now does the same). **Held:** read it and connect to its socket (reusing a live connection).
2. Send `client.hello { kind, name, version, protocol, chat? }`. The owner answers `{ protocol, version, document }`. A different `protocol` ends here: edits refuse with code `version` ("Pixel Lab (0.8.0) has this document open, and this pixellab is 0.7.2. Use the app's bundled pixellab, or close the document in Pixel Lab."), and read-only commands run locally against disk after asking the owner to `doc.flush` (R28).
3. Send the command as JSON-RPC (the stdio methods and params). CLI requests add `save: true`, so the owner flushes the docs it touched before replying and the CLI's "Saved …" stays true. Then print or return the owner's result exactly as a local run would.

Around that:

- **Not forwarded:** `help`, `--version`, `serve`, `document.new` (no document yet), and `document.migrate`, which refuses while owned ("Close the document in Pixel Lab first; it can migrate it too"). `--dry-run` flushes through the owner, then runs locally on the files.
- **Owner gone (R24).** If the connection ends with a request unanswered, the call fails with code `owner-gone` ("Pixel Lab closed while scene_place ran; check scene_items before trying again") and isn't retried. The next call starts again at step 1. At shutdown, the owner stops accepting, answers what it has queued, flushes, marks the version and exits.
- **Owner arrives.** An MCP server with local docs that are unsaved (they had problems) when it finds an owner sends each with `doc.adopt { doc, base, source }`. The owner merges it into its copy as one step by that author. On a clash, the doc stays local, and the next result says so.
- **Two local writers** (two MCP servers, no app) keep today's protection: the hash check before each save and the clean-doc reload before each call (`sync`, minus proposals), with a conflict merged as in D1.

### D11. Notifications and the app's model

`doc.changed` adds `saveState`, `problems`, `author`, `undoLabel` and `redoLabel`, and is sent for remote edits like local ones, so the stage redraws (R17). New `history.changed { versions: [latest] }` lets the open Versions sheet and chat cards follow. `proposals.changed` goes. `AgentModel` loses proposals and ghosts; a `VersionsModel` holds the sheet's list, selection and ghost.

### D12. Script hook and acceptance (R30)

New script lines: `expect savestate saved|pending|problems`, `wait saved`, `flush`, `versions open|close`, `version select <n>`, `version restore [doc]`, `version undo`, `expect versions <n>`, `expect version <n> author|edits <v>`, `card undo`, `cli-owned <args…>` (the bundled CLI against the app's open document, through the socket), `expect owner true|false`, `secondapp` (starts a second server on the same document and expects `owned`). `approve`, `reject`, `proposal …`, `ghost` (for proposals), `expect proposals|ghostchanges|base` and `card approve|reject|ghost|diff` go. `stale merge` goes; `stale reload|keep` stay.

The acceptance scripts that approve proposals (`agent.txt`, `chat*.txt`, `stale.txt`, `mcp-agent.mjs`, `stale-agent.mjs`) are rewritten. An MCP client edits through the open app and the edit shows on the stage. The chat's change card undoes a turn. A restore and an undo of a version run. Quitting hands ownership to the MCP client, whose next edit writes the file. A second server is refused. The fake Claude (`scripts/fake-claude/`) stops expecting a proposal from `doc_save`.

## Areas of concern

1. **The record says agents propose and people approve.** `../2026-10-mac-app/plan.md` ("Agents propose, people approve"), `CLAUDE.md`'s rule "MCP edits become proposals the owner approves in the Agent window; `--direct` stays off by default", and `../2026-10-agent-chat/spec.md` R2, R16, R17, R21 and R24 all say so. The accepted intent overrides them. The plan should update `CLAUDE.md`'s rule and add "superseded in part" notes to the two records (R31). **Decided:** this change updates `CLAUDE.md` and the records.
2. **The agent-coworker intent still says "What agents change stays a proposal the owner approves."** It's a separate draft intent. **Decided:** left for that intent's spec to settle.
3. **Agents can restore and undo versions** (D4 puts `history.restore` and `history.undo` in the MCP tools). That's consistent with edits that apply and are themselves undoable, but an agent restoring the whole document to last week is a big move. **Decided:** agents keep both.
4. **Retiring waiting proposals (R21)** sets aside agent work nobody approved. It isn't deleted, but it isn't applied either. **Decided:** retire them, as R21 says.
5. **History follows the path.** Moving or renaming the `.pixellab` folder (or cloning the site repo elsewhere) starts a new history; the old one stays under its key. Save As starts the copy's own history with a version 0 "Copied from …". Tying history to a document id would need a new manifest field, which changes the format. **Decided:** history is keyed by path.
6. **The site's pinned `pixellab` (R28).** The site's `npm run art` runs its own pinned release. Reads and builds work from disk, but if a pinned binary of an older protocol tries to edit while the app has the document open, it's refused. Only edits are affected, and the site doesn't edit through its pin today. No change to the site is needed.
7. **A shared undo stack (R20)** means the owner's ⌘Z can take back an agent's edit, and the menu names it so it's never a surprise. The alternative (each author undoes only their own) would break "Undo keeps working as it does now" for the owner. This is the intent's open question, answered here; the owner can overrule it.
8. **Autosave every second in the site checkout** leaves the document's files modified in git as soon as anything changes, as an explicit save does now, only sooner. Nothing is committed for the owner. No change is proposed.

## Open questions

- **When a save happens:** answered in R1. A second after the last edit, and at once on losing focus, closing, quitting, Publish, Save As and sending a chat message.
- **An invalid autosave:** answered in R2 and R4. The doc stays unsaved with a quiet subtitle and Inspector row. The only alert is on close or quit.
- **Undo with an agent:** answered in R20 (one shared stack, with authors named, and agents limited to their own steps); see concern 7.
- **How far back history goes, and whether agents can use it:** answered in R10 (a day, then daily for a month, then weekly) and D4; agents' reach is concern 3.
- **Looking over an agent's changes after the fact:** the version, its diff and its ghost on the stage (R12), and the chat's change card (R18). Carried forward: a highlight on the stage of what an agent just changed, if the owner wants more.
- **Git noise:** answered in concern 8. Writes happen only when content changes, in the canonical format, so the diffs are the real changes.
- **Save As and Publish:** answered in D2. Both flush first; Publish still refuses while a doc has problems; Save As starts a fresh history.
- **How the CLI and agents find the app:** answered in D9 and D10. A per-document lock the OS releases on exit, and a socket beside it.
- **New:** should ⌘S's version be named? D3 allows a label, and the sheet could ask for one. Left out for now; the owner can ask for it.
