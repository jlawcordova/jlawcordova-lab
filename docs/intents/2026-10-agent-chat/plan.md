# Plan: Agent chat (from intent.md 2026-10-21)

## Files that change

**PR 1, the CLI (`feat/agent-chat-cli`)**
- `cli/lib/state.mjs`: `writeProposal` numbers a proposal once (`proposal-numbers.log` in the state dir); `numberOf(id)`.
- `cli/lib/session.mjs`: the session's `chat` (`{ id, title }`); `propose` writes `chat` into the proposal.
- `cli/lib/server-mcp.mjs`: reads `PIXELLAB_CHAT` / `PIXELLAB_CHAT_TITLE`; activity lines gain `agent` and `chat`; the instructions name `chat_ask`.
- `cli/lib/chat-context.mjs` (new): matching the working copy to the saved file, the block's text (D5).
- `cli/lib/commands/chat.mjs` (new): `chat.ask`, `chat.context`.
- `cli/lib/commands/proposal.mjs`: `number` and `chat` in `proposal.list` and `proposal.show`.
- `cli/lib/activity-summary.mjs`: `chat.ask`, `chat.context` lines.
- `cli/lib/registry.mjs`: the new command file.
- `cli/test/chat.test.mjs` (new): R8's MCP side, R10, R11, R15's tool, R23, R24. `cli/test/help.test.mjs`, `cli/test/params.test.mjs`: command counts and lists, if they pin them.
- `docs/cli.md` (generated), `cli/README.md`.

**PR 2, the chat (`feat/agent-chat-app`)**
- `app/Sources/PixelLab/Chat/ClaudeLocator.swift` (new): finding `claude`, its version, auth status, the clean environment (D1, D2).
- `app/Sources/PixelLab/Chat/ClaudeTurn.swift` (new): one turn's process, stdin, the line reader, Stop (D1).
- `app/Sources/PixelLab/Chat/StreamParser.swift` (new, pure): stream-json lines → events (D1's table).
- `app/Sources/PixelLab/Chat/ChatStore.swift` (new): chats on disk, `Chat`, `ChatMessage` (D3).
- `app/Sources/PixelLab/Chat/ChatModel.swift` (new, `AppModel.chat`): setup state, chats, the open chat, draft, running turn, details, sending, questions (D2, D4).
- `app/Sources/PixelLab/Chat/ChatRows.swift` (new, pure): folding steps, sender groups, date headers (D9, D12).
- `app/Sources/PixelLab/Chat/StepText.swift` (new, pure): sentences and raw text (D9).
- `app/Sources/PixelLab/Chat/ChatLinks.swift` (new, pure): links in replies (D10).
- `app/Sources/PixelLab/Chat/ChatPrompt.swift` (new): the appended prompt (D11).
- `app/Sources/PixelLab/Views/RightColumn.swift` (new): the tab bar and the two tabs (D12).
- `app/Sources/PixelLab/Views/AgentTab.swift` (new): setup, the Chats list, the chat's header and conversation.
- `app/Sources/PixelLab/Views/ChatBubbles.swift` (new): bubbles, the avatar, steps, question card, typing bubble, notes.
- `app/Sources/PixelLab/Views/ChatInput.swift` (new): the `NSTextView` input, placeholder, send and Stop.
- `app/Sources/PixelLab/Views/MainWindow.swift`: the inspector column is `RightColumn`; the sparkle shows the Agent tab.
- `app/Sources/PixelLab/PixelLabApp.swift`: Window › Agent ⌥⌘A, Agent Window ⇧⌥⌘A; stopping a turn on quit.
- `app/Sources/PixelLab/Model/AppModel.swift`: `chat`, `rightTab`.
- `app/Sources/PixelLab/Theme/Theme.swift`: `claude`, `onClaude`.
- `app/Sources/PixelLab/Support/ScriptRunner.swift`: D15's setup, tab, chat, step, question and link actions.
- `app/Tests/PixelLabTests/ChatTests.swift` (new), `app/Tests/PixelLabTests/Fixtures/stream-*.jsonl` (new): the parser, rows, steps, links, store round trip, locator order. `app/Package.swift` if the fixtures need a resources entry for the test target.
- `scripts/fake-claude/claude`, `scripts/fake-claude/fake-claude.mjs`, `scripts/fake-claude/turns.json`, `scripts/fake-claude/install.sh`, `scripts/fake-claude/README.md` (all new): D16.
- `scripts/acceptance.sh`: runs `chat-setup.txt` with the fake, and checks the chat's proposal, activity lines and chats files.
- `scripts/acceptance/chat-setup.txt` (new): the setup flow, from no `claude` to the chat.

**PR 3, context, proposals and the Agent window (`feat/agent-chat-context`)**
- `app/Sources/PixelLab/Chat/ChatContext.swift` (new, pure): chips, the count line, suggestions (D5, D12).
- `app/Sources/PixelLab/Chat/ChatModel.swift`: pins, the block (`chat.context`), Save now, proposal cards and their status, ghosts per chat.
- `app/Sources/PixelLab/Views/ChatContextBar.swift` (new): chips, the unsaved warning, the disclosure.
- `app/Sources/PixelLab/Views/ChatBubbles.swift`: the proposal card.
- `app/Sources/PixelLab/Model/LayersModel.swift`, `app/Sources/PixelLab/Model/SceneEditor.swift`: ⌥-click every use, Select Every Use; pins follow `reordered`.
- `app/Sources/PixelLab/Views/InspectorView.swift`: Ask Claude about this.
- `app/Sources/PixelLab/Model/AgentModel.swift`, `app/Sources/PixelLab/Model/ProposalGhost.swift`: `number`, `chat`, `agent` decoding; the activity filter; telling the chat about approve and reject.
- `app/Sources/PixelLab/Views/AgentWindow.swift`: the CLAUDE CODE row, sources, Open the chat, All | Chats | Errors, the who column.
- `app/Sources/PixelLab/Views/StageOverlay.swift`: the banner's wording.
- `app/Sources/PixelLab/PixelLabApp.swift`: Edit › Select Every Use.
- `app/Sources/PixelLab/Support/ScriptRunner.swift`: chips, pins, Save now, cards, ⌥-click, the activity filter, the Agent window's expects.
- `app/Tests/PixelLabTests/ChatTests.swift`: chips, suggestions, every-use picking, card status.
- `scripts/acceptance/chat.txt` (new), `scripts/acceptance/chat-harbor.txt` (new), `scripts/acceptance.sh`: the chat end to end on the temp site and a temp harbor copy.
- `scripts/fake-claude/turns.json`: the turns PR 3's scripts use.
- `scripts/acceptance/scene-object.txt`, `document.txt`, `scripts/acceptance.sh` (`sort.txt`'s document) (as built: the site's art moved on).
- Docs: `README.md`, `app/README.md`, `docs/architecture.md`, `docs/testing.md`, `scripts/README.md`, `CLAUDE.md`, `docs/screenshots/chat-*.png` (new), and this plan's "As built".

## Order of work

```
main ── feat/agent-chat ─ (intent, spec, plan) ───────────────────── PR 4 (owner merges) ──▶ main
          ├─ PR 1  CLI: chat.ask, chat.context, numbers, sources ◀─ feat/agent-chat-cli      (steps 2–4)
          ├─ PR 2  the chat: Claude Code, setup, the Agent tab   ◀─ feat/agent-chat-app      (steps 5–11, cut after PR 1 merges)
          └─ PR 3  context, proposals, the Agent window, docs    ◀─ feat/agent-chat-context  (steps 12–18, cut after PR 2 merges)
```

Each slice branch is cut from `feat/agent-chat` in its own worktree (`../jlawcordova-lab-agent-chat-<slice>`).

1. The intent, spec and plan, committed straight onto `feat/agent-chat` (the owner waived their merges).

**PR 1: steps 2–4**

2. `state.mjs` numbers, the session's chat, the MCP server's env and activity fields; tests (R24), including two servers proposing at once and an older proposal without a number.
3. `chat-context.mjs` and `chat.mjs` (`chat.ask`, `chat.context`); `number`/`chat` in the proposal commands; tests: the block for a group, items, pinned, nothing selected; items moved down the list by a placement keep their saved paths; a placed, moved or changed item and a layer made since the save are left out; `chat_ask` with and without a chat over MCP; a chat's proposal carries the chat (R8 MCP side, R10, R11, R15, R23).
4. `npm run docs:cli`, `cli/README.md`; PR 1's checks (`npm test`); merge into `feat/agent-chat`.

**PR 2: steps 5–11**

5. The fake Claude Code and its installer (D16), with the captured lines; a quick node check that it plays a turn through the real MCP server and writes a proposal with the chat.
6. `ClaudeLocator`, `StreamParser`, `ClaudeTurn`, `ChatStore`; unit tests (parser on captured and fake lines, store round trip, the clean environment, the search order).
7. `ChatModel`: setup states and actions, chats, sending, the turn's events into the transcript, Stop, questions, resume and its fallback; `StepText`, `ChatRows`, `ChatLinks`, `ChatPrompt`; unit tests.
8. The views: `RightColumn`, `AgentTab`, `ChatBubbles`, `ChatInput`; the tokens; the menus and the toolbar.
9. The script hook's actions (D15, PR 2's part); `chat-setup.txt`, and a chat run on a temp harbor copy (sends, steps, a question, links, chats, Stop).
10. Snapshots of the setup states, the list and a chat in dark and light; looked at against the prototype.
11. PR 2's checks (`npm test`, `swift build`, `scripts/test-app.sh`, `scripts/build-app.sh`, `scripts/acceptance.sh` with the setup script added); merge into `feat/agent-chat`.

**PR 3: steps 12–18**

12. `ChatContext`, pins and the block in `ChatModel`, `ChatContextBar`, Save now; unit tests.
13. ⌥-click and Select Every Use; Ask Claude about this; unit tests of every-use picking.
14. Proposal cards, their status, ghosts per chat, the banner; `AgentModel` numbers, sources and the filter; the Agent window's row, sources and Open the chat.
15. The hook's remaining actions; `chat.txt` (the temp site: select, pin, ask, a proposal ghosted, approved from the card, the Agent window's source and filter, a rejected one, Save now) and `chat-harbor.txt` (layers as chips, inside a layer, ⌥-click, a question answered); `acceptance.sh`.
16. Snapshots of every new view in dark and light (`docs/screenshots/chat-*.png`), looked at.
17. Docs (D17).
18. PR 3's checks; merge into `feat/agent-chat`.

**PR 4: steps 19–21**

19. `git fetch origin`, merge the latest `origin/main` into `feat/agent-chat`, re-run every check.
20. A draft PR into `main`; an independent `verify-change` run on it in a fresh session; fix its blocking findings.
21. The PR ready for the owner, with the verification output, the checks' output and the snapshots; hand over.

**As built.**
- PR 1 (#22): the numbers log is `proposal-numbers.log` beside `proposals/`, not inside it, so that folder holds only proposals (spec D7 says so). Three tests that pinned the old shapes (a proposal file's keys, an activity line's keys, the command list) take the new fields. `chat.context` refuses an unknown item or layer (`not_found`) rather than calling it unsaved.
- PR 2 (#23): checking the real CLI showed that a `claude` started from inside a Claude Code session inherits its variables and fails auth, so the environment drops every `CLAUDE*` and `ANTHROPIC_*` variable (spec D1). The fake is a shell launcher plus `fake-claude.mjs`, so the Node the app finds (20.17 here, from the login shell) reads it as a module; every quick `claude` run gets the app's node on its PATH. A failed check after an install says what `claude --version` printed. The hook's chat answer is `choose` and its question check `expect chatquestion` (the Agent window already has `answer` and `expect question`); spec D15 lists the actions as built. Proposal cards follow the reply about them and are titled by their one edit or "2 edits to harbor". `scripts/acceptance.sh` runs `chat-setup.txt` in PR 2 (the plan had it in PR 3's runner changes).
- PR 3: a new chat with a pin (and no message yet) is kept, so New chat doesn't drop its pins. The context is fetched again when the selection, the pins, the dirty state or the items change (not on every render). `chat.txt` runs on `closeup-front-trees`, which no other script edits, right after the terminal agent's scripts and sharing their state dir (so its proposals are #4 and #5); `chat-harbor.txt` gets its own state dir. The banner's title is `ProposalSummary.bannerTitle`, which the hook reads too.
- The site's art moved on during the build (its depth-order merge, `f5d74e3`, and new scenes: 28 scenes and 204 objects): `scene-object.txt` and `document.txt` expect the new counts, and `sort.txt` runs on the site's document as it was before that merge (`6d19714`, read with `git archive`, never checked out), since the site's own document is now in depth order. `CLAUDE.md` and `docs/testing.md` say the scripts pin the art of 2026-10-21.

## Workflow and reviews

| Step | Who | Where | Then |
| --- | --- | --- | --- |
| 1 | main session | `feat/agent-chat` | push, go on |
| 2–4 | main session | `feat/agent-chat-cli` (worktree) → PR 1 | merge into `feat/agent-chat` when its checks pass |
| 5–11 | main session | `feat/agent-chat-app` (worktree) → PR 2 | merge when its checks pass |
| 12–18 | main session | `feat/agent-chat-context` (worktree) → PR 3 | merge when its checks pass |
| 19–20 | main session; a fresh session for `verify-change` | `feat/agent-chat` → PR 4 (draft) | fix blocking findings, re-check |
| 21 | main session | PR 4 | hand over to the owner |

- **One runner.** The main session runs every slice, in order. PR 2 and PR 3 share `ScriptRunner.swift`, `ChatModel.swift`, `AppModel.swift` and the fake's `turns.json`, and PR 3 builds on PR 2's views, so parallel subagents would only collide. Three slices rather than one: PR 1 changes what's written to the state dir (proposals' numbers and sources, the activity lines), which is merged and checked before the app reads it; and the app's work is two reviewable halves (running the chat; context and proposals).
- **Merging.** The main session merges each slice PR into `feat/agent-chat` with a merge commit (`gh pr merge --merge --delete-branch`; the owner wants merge commits, not squashes) once the checks pasted in its PR pass. If `main` moves, it's merged into `feat/agent-chat` before the next slice. Only the owner merges PR 4.
- **Owner reviews:** one, PR 4 (the owner waived the intent, spec and plan merges and asked to check only the final PR). Its snapshots are for the owner's look at every new view in dark and light.
- **Independent review:** a `verify-change` run on PR 4 while it's a draft, because the change touches the proposals and approval path and the MCP surface, and starts a process with the owner's login. A fresh session is given only "Use the `verify-change` skill on PR #<n>." (a subagent if a session can't be started). Its blocking findings are fixed on `feat/agent-chat`, and its comment is pasted into PR 4.

## Risks

- **The real CLI's stream differs from what the fake plays.** The parser follows the captured lines and ignores unknown types; the fake writes the documented shapes. The owner's first real message after merging is the live check (spec, Areas of concern); the PR asks for it.
- **The sign-in can't be completed headless** (a pasted-code flow). The card falls back to Terminal (D2). Not testable without the owner's account; flagged in the PR.
- **The flags drift in a later Claude Code.** The fake refuses a turn without D1's safety flags, so the acceptance pass catches the app dropping one; the README says which version they were checked against.
- **A child process left running** (a stuck turn or sign-in). Stop and quit end it (SIGINT, then SIGTERM); the hook's `chat stop` and the acceptance pass check that the next turn runs.
- **The selection refactor isn't touched** (the chat reads `DocumentModel.selection`), but ⌥-click changes `stageMouseUp`: the scene-layers and scene-object acceptance scripts must pass unchanged.
- **The state-dir change** (PR 1): older proposals without `number` still list, show, apply and reject (a test), and two servers proposing at once get distinct numbers (a test).
- **The riskiest step is 7** (the turn's lifecycle: streaming into the transcript, Stop, resume, one turn at a time). Unit tests cover the parser and the rows; the hook drives sends, Stop and a second turn on the fake.
- **The real site checkout and the owner's install:** every test and script runs on temp copies, the state dir is the run's, and `PIXELLAB_CLAUDE_HOME` keeps the hook away from the owner's `claude` (D2).

## Proof

- `npm test` shows `# fail 0`, with `R…:` tests in `chat.test.mjs`.
- `npm run docs:cli`, then `git status` shows only this change's edits.
- `swift build --package-path app` with no errors; `scripts/test-app.sh` passes, with `ChatTests`.
- `scripts/build-app.sh`, then `scripts/acceptance.sh` exits 0: the existing passes, `chat-setup.txt` (no `claude` → install → signed out → sign in → waiting → ready → sign out), `chat.txt` and `chat-harbor.txt`, with the runner checking that the chat's proposal file has `chat` and a `number`, that the activity log has chat lines, and that no chat file or view state went into the document.
- `npm run docs:site` builds with no broken links.
- Snapshots in dark and light of the setup states, the Chats list, a chat with steps, a question and a proposal card, the context bar with a group chip, a pinned chip and a not-saved chip, the stage banner, and the Agent window with a chat proposal, looked at, in `docs/screenshots/` and PR 4.
