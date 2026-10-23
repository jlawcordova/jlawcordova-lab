# Plan: Autosave, with version history (from intent.md 2026-10-23)

## Files that change

**PR 1, the program (`feat/autosave-program`)**
- `crates/pixellab/src/author.rs` (new): `Author` (owner, agent, cli, outside, restore, several, opened) as JSON and a label suffix ("(Claude)").
- `crates/pixellab/src/history.rs` (new): the history folder of D3 (`document.json`, `versions.jsonl`, `blobs/`, `lock`), `record`, `mark`, version 0, outside changes, thinning, a file's text at a version.
- `crates/pixellab/src/owner.rs` (new): the owner lock and socket of D9 (key, `try_lock`, contents, stale socket, long paths, peer uid), and the listener thread that feeds the stdio server's channel.
- `crates/pixellab/src/client.rs` (new): D10's lookup, `client.hello`, forwarding a call, `owner-gone`, `version`, `doc.adopt`.
- `crates/pixellab/src/commands/history.rs` (new): `history.list`, `history.show`, `history.render`, `history.restore`, `history.undo`, `history.mark`.
- `crates/pixellab/src/session.rs`: `Step.author` and the request's author; autosave (`autosave_ms`, `dirty_since`, `blocked`), `flush`, `saveState` and labels in the status; a save records history and merges a conflict; client undo rules; `adopt`; proposals and `Mode::Propose` removed; `sync` keeps only the clean-doc reload.
- `crates/pixellab/src/commands/doc.rs`: `doc.flush`, `doc.adopt`; `doc.save` marks a version; `doc.undo`, `doc.redo`, `doc.revert` and `doc.close` from a client (D7).
- `crates/pixellab/src/commands/proposal.rs` (deleted; `activity.list` moves to `commands/activity.rs`, new), `crates/pixellab/src/open_docs.rs` (deleted in step 7, when the owner lock replaces it), `crates/pixellab/src/commands/mod.rs`, `crates/pixellab/src/registry.rs`, `crates/pixellab/src/lib.rs`.
- `crates/pixellab/src/state.rs`: proposal functions removed; first-run retirement of `proposals/` with `retired-proposals/notice.json` (R21); the doc comment's layout.
- `crates/pixellab/src/server_stdio.rs`: the owner claim, `Event::Remote`, autosave on the tick, shutdown flush, `owned` on `session.info`, `history.changed`; `proposals.changed` and open-docs removed.
- `crates/pixellab/src/server_mcp.rs`: forwarding through `client.rs`; a local session that saves after each mutating call; `--direct` ignored; `INSTRUCTIONS`, `LEFT_OUT`, tool descriptions.
- `crates/pixellab/src/cli.rs`, `crates/pixellab/src/serve.rs`: forwarding through `client.rs` (`save: true`, `--dry-run`), `document.migrate` refused while owned.
- `crates/pixellab/src/watch.rs`: the state watcher drops proposals.
- `crates/pixellab/src/help.rs`: the state dir's description; the MCP section.
- `crates/pixellab/src/activity_summary.rs`, `crates/pixellab/src/chat_context.rs`: proposal wording.
- Tests: `crates/pixellab/tests/autosave.rs`, `history.rs`, `owner.rs` (new); `servers.rs`, `session.rs`, `state.rs`, `chat.rs`, `watch.rs`, `retire.rs`, `edit_commands.rs`, `depth_edit.rs`, `ambient_commands.rs`, `scene_canvas.rs`, `layers.rs`, `params.rs`, `help.rs`, `commands.rs`, `groups.rs`, `motion_ops.rs`, `art_budget.rs` (proposal cases replaced by applied edits); `sync.rs` (trimmed, not deleted: its proposal settling cases go, and its reload, edits-since-disk and merge cases stay); `tests/common/server.rs` (new: the servers harness, shared).
- `docs/cli.md` (generated).

**PR 2, the app, scripts and docs (`feat/autosave-app`)**
- `app/Sources/PixelLab/Server/Protocol.swift`: `DocStatus.saveState`, `problems`, `author`, `undoLabel`, `redoLabel`; `Version`, `HistoryList`, `HistoryShow`; proposal types removed.
- `app/Sources/PixelLab/Server/ServerConnection.swift`: `PIXELLAB_APP_PID`; the `owned` answer.
- `app/Sources/PixelLab/Model/AppModel.swift`: `flush` (replacing `save`'s alerts), the R1 triggers, `confirmUnsaved` for problems only, `saveVersion` (⌘S), `restoreLastOpened`, the `owned` hand-off (activate the other copy and quit).
- `app/Sources/PixelLab/Model/VersionsModel.swift` (new): the sheet's list, selection, ghost, restore and undo.
- `app/Sources/PixelLab/Model/ProposalGhost.swift` → `VersionGhost.swift` (renamed): the ghost fed by `history.render`.
- `app/Sources/PixelLab/Model/AgentModel.swift`: proposals, ghost and badges removed; the retirement notice.
- `app/Sources/PixelLab/Chat/ChatModel.swift`, `ChatStore.swift`, `ChatRows.swift`, `StepText.swift`, `ClaudeTurn.swift`, `ChatPrompt.swift`: change cards, `history.mark` at a turn's end, flush before send.
- `app/Sources/PixelLab/Views/VersionsSheet.swift` (new): D5.
- `app/Sources/PixelLab/Views/ChatBubbles.swift`: `ChangeCard` replacing the proposal card.
- `app/Sources/PixelLab/Views/AgentWindow.swift`, `AgentTab.swift`, `RightColumn.swift`, `StageOverlay.swift`, `StageView.swift`, `StaleBanner.swift`, `InspectorView.swift`, `MainWindow.swift`: proposals out, Save out, the problems row, the subtitle and edited dot, Reload and Keep Mine.
- `app/Sources/PixelLab/PixelLabApp.swift`, `app/Sources/PixelLab/Support/WindowGuard.swift`: File › Save (a version), Revert To › Last Opened / Browse All Versions…, flush on resign, quit and close.
- `app/Sources/PixelLab/Support/ScriptRunner.swift`: D12's lines.
- `app/Tests/PixelLabTests/AutosaveTests.swift`, `VersionsTests.swift` (new); `AgentTests.swift`, `ChatTests.swift`, `StaleTests.swift`, `CanvasTests.swift` (proposal cases replaced); `app/Tests/PixelLabTests/Fixtures/stream-fake-move.jsonl`.
- `scripts/acceptance/agent.txt`, `chat.txt`, `chat-harbor.txt`, `chat-setup.txt`, `chat-relaunch-1.txt`, `chat-relaunch-2.txt`, `stale.txt`, `mcp-agent.mjs`, `stale-agent.mjs`, `versions.txt` (new), `owner.txt` (new), `scripts/acceptance.sh`.
- `scripts/fake-claude/fake-claude.mjs`, `turns.json`, `README.md`.
- Docs: `README.md`, `app/README.md`, `docs/architecture.md`, `docs/testing.md`, `CLAUDE.md` (its proposal rule and script-hook lines), notes in `../2026-10-mac-app/plan.md` and `../2026-10-agent-chat/spec.md`, `docs/screenshots/versions-*.png` (new).

## Order of work

```
intent/autosave-history (PR #49: intent, spec) ─┐
main ─────────────────────────────────────────── feat/autosave ── (plan) ─────────── PR 4 (owner merges) ──▶ main
                                                   ├─ PR 2  the program   ◀─ feat/autosave-program   (steps 2–9)
                                                   └─ PR 3  app + scripts ◀─ feat/autosave-app       (steps 10–17, cut after PR 2 merges)
```

`feat/autosave` is cut from `intent/autosave-history`, which holds the approved intent and spec (PR #49, which the owner merges into `main`). When #49 merges, `main` is merged into `feat/autosave`, which is then a no-op apart from the merge commit.

1. This plan, committed as the first commit on `feat/autosave`.

**PR 2 (the program): steps 2–9**

2. Authors: `author.rs`; `Session.author` set per request; `Step.author`; `undoLabel` and `redoLabel` in the status. Tests `R20: …` for the labels.
3. History: `history.rs`; `Session::save` records each write; version 0 on first open; coalescing within 60 s per author; `mark`; outside changes recorded where the session reloads or merges a changed file; thinning on open. Tests `R8`–`R11`.
4. History commands: `commands/history.rs` and the registry; `history.render` reuses `scene.render` and `object.render` over the file text at that version; `restore` and `undo` are one undo step per doc, saved, and recorded as `restore`. Tests `R13`, `R15`.
5. Autosave: `autosave_ms`, `dirty_since`, `blocked`; `Session::autosave(now)` and `flush`; `saveState`; a conflict merges, a clash blocks and sends `doc.stale`; `doc.flush`; `doc.save` marks the version. Tests `R1`, `R2`, `R6`, `R7`.
6. Proposals out: `Mode` (both variants), `propose`, `proposal.*`, `Proposed`, `Entry.proposal`, the state's proposal functions, `proposals.changed` (the state watcher now reports `history.changed`); `State::new` retires `proposals/` with a notice; `serve --mcp --direct` is ignored with a note; MCP's local session saves after each mutating call; `INSTRUCTIONS`, `LEFT_OUT` and descriptions rewritten. Every test that proposed now expects the edit on disk. Tests `R16`, `R21`, `R23`.
7. Owning a document: `owner.rs`, replacing `open_docs.rs` (and its use in `document.migrate`); the stdio server takes the lock, binds the socket, and runs `Event::Remote` requests in its loop; `session.info` answers `owned` when another server holds it; shutdown flushes and marks. Tests `R25`, `R26`, `R27`, including a killed owner freeing the document.
8. Clients: `client.rs`; `serve --mcp` and the CLI forward to an owner; client undo rules; `owner-gone`; `version`; `doc.adopt`; `document.migrate` refused while owned; `--dry-run` through a flush. Tests `R22`, `R23`, `R24`, `R28`: an MCP client and the CLI edit through a running `serve --stdio` and the edit shows in its session and on disk.
9. `npm run docs:cli`; `pixellab help`'s state dir and MCP text; PR 2's checks; merge into `feat/autosave`.

**PR 3 (the app, scripts and docs): steps 10–17**

10. Protocol and model: `Protocol.swift`, `ServerConnection.swift` (`PIXELLAB_APP_PID`, `owned`), `AppModel` (`flush`, triggers, `confirmUnsaved`, `saveVersion`, `restoreLastOpened`, the hand-off), `AgentModel` without proposals.
11. Views: Save out of `RightColumn`, the problems row, the subtitle and edited dot, the menus, `StaleBanner`, the Agent window and tab without proposals, the retirement notice.
12. The Versions sheet: `VersionsModel`, `VersionsSheet`, `VersionGhost`, its keys and VoiceOver labels.
13. The chat: `ChangeCard`, `history.mark` at the end of a turn that edited, flush before send, `StepText`.
14. The script hook: D12's lines; `AutosaveTests`, `VersionsTests`; the proposal tests replaced.
15. Acceptance: the rewritten scripts, `versions.txt`, `owner.txt`, and the fake Claude without proposals; `scripts/acceptance.sh` runs them.
16. Docs: README, `app/README.md`, `docs/architecture.md`, `docs/testing.md`, `CLAUDE.md`, the two records' notes; `npm run docs:site`.
17. Snapshots of the changed views in dark and light (the window with its subtitle, the problems row, the Versions sheet with a ghost, a change card, the Agent window), looked at, and added under `docs/screenshots/`; PR 3's checks; merge into `feat/autosave`.

**PR 4:** `feat/autosave` → `main`, with the verification output. The owner merges it.

## Workflow and reviews

| Step | Who | Where | Then |
| --- | --- | --- | --- |
| 1 | main session | `feat/autosave` | push |
| 2–9 | main session | `feat/autosave-program`, cut from `feat/autosave` | PR 2 into `feat/autosave`; the main session merges it once `cargo fmt --check`, `cargo test` and `npm test` pass |
| 10–17 | main session | `feat/autosave-app`, cut from `feat/autosave` after PR 2 merges | PR 3 into `feat/autosave`; merged once `swift build`, `scripts/test-app.sh`, `scripts/build-app.sh` and `scripts/acceptance.sh` pass |
| PR 4 | main session opens it | `feat/autosave` → `main` | the owner merges it |

- **Runner.** The main session runs every step, in order, in its own worktree (other sessions share the main checkout). No subagents: the owner didn't ask for them, and the two slices depend on each other.
- **Why two slices.** The program changes the state dir's format (history, owners, proposals retired) and the protocol the app speaks. It lands and passes its checks before the app is changed to it. The app slice also rewrites the acceptance pass, which needs both halves.
- **Merges.** The main session merges PR 2 and PR 3 into `feat/autosave` with `gh pr merge --merge --delete-branch` (the owner prefers merge commits) once their checks pass. When `main` moves, the main session merges it into `feat/autosave` before the next slice starts. Conflicts are the main session's to resolve.
- **Owner reviews.** None before PR 4. The owner set the goal of finishing to a completed build, and the spec settled the look and the choices. PR 4 carries the snapshots and the verification output, and the owner reviews them there. Installing and trying the build (`scripts/install.sh`) is the owner's.
- **Independent review.** A `verify-change` run on PR 4, in a fresh session, because the change rewrites the MCP server's surface and removes the approval path. The main session offers it as a session the owner can start, and fixes any blocking finding on `feat/autosave` before the owner merges.

## Risks

- **The riskiest step is 7–8, ownership and forwarding.** Two processes, a lock and a socket, with ownership changing hands mid-session. Handled by: the stdio server's loop, which already runs requests one at a time; tests that start a real `serve --stdio` and drive it with real MCP and CLI processes (`owner.rs`); a test that kills the owner with SIGKILL and checks the next CLI command writes directly. A request whose answer never arrives fails with `owner-gone` and is never retried.
- **Linux CI.** The release workflow tests on Linux. `File::try_lock` and Unix sockets work there. `getpeereid` doesn't exist in glibc, so the peer check uses `SO_PEERCRED` through `getsockopt` on Linux, both declared `extern "C"`. If either fails to link, the 0700 folder alone guards the socket, and the plan is updated.
- **Socket paths over 104 bytes** in tests' temp state dirs: D9's `$TMPDIR` fallback, with a test that uses a deep state dir.
- **Undo changes meaning slightly.** The owner's ⌘Z can undo an agent's step (concern 7, decided). `session.rs` tests pin the labels, and the client-undo refusal.
- **Removing proposals touches many tests** (about 250 references across 18 Rust suites, 6 app files' tests and 9 acceptance files). Each proposal case becomes "the edit is on disk, with a version by that author", so the tests still cover the agent path.
- **The real site checkout.** No test or script touches it. Tests use `make_document()` and `make_site()`, the script hook refuses the real checkout, and every server test sets `PIXELLAB_STATE` to a temp dir, so history and owner files never land in the owner's state dir.
- **Autosave timing in the script hook.** Waiting a second per edit would slow scripts down and make them flaky. `wait saved` waits on `saveState`, and the hook's own runs pass a short `PIXELLAB_AUTOSAVE_MS` (100).
- **Retiring proposals** sets unapproved agent work aside (concern 4, decided): moved, never deleted, with a notice.
- **Other options considered.** Autosave in the app, not the session: rejected, because agents' and the CLI's edits wouldn't autosave the same way. macOS Versions (`NSFileVersion`): rejected in the spec, because writes come from the program. Each process writing directly with merges (no owner): rejected, because autosaving every second would make clashes constant.

## Proof

- Program: `cargo fmt --check`; `cargo test` with no failures, including the new `autosave`, `history` and `owner` suites; `npm test` with `# fail 0`; `npm run docs:cli` leaves `git status` clean apart from the change's own edits.
- App: `swift build --package-path app` with no errors; `scripts/test-app.sh` with every test passing.
- End to end: `scripts/build-app.sh`, then `scripts/acceptance.sh` exits 0. It includes `versions.txt` (edit, `wait saved`, `versions open`, `version restore`, `version undo`, `expect versions …`) and `owner.txt` (an MCP client and `cli-owned` edit through the app, `expect scenepixel` shows them on the stage, `secondapp` is refused, quitting hands over, and a later CLI edit writes the file).
- Docs: `npm run docs:site` builds with no broken links.
- UI: snapshots in dark and light of the window and subtitle, the problems row, the Versions sheet with its ghost, a change card and the Agent window, looked at before reporting.
