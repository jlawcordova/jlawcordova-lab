# Plan: Keep agents' docs in step with disk, and fill the MCP's gaps (from intent.md 2026-10-20)

## Files that change

CLI and servers:
- `cli/lib/session.mjs`: step sequence numbers and `baseSeq` (edits since disk), `proposed`, `sync()`, `reload()`, `merge()`, `synced` in details.
- `cli/lib/merge.mjs` (new): `merge3(base, ours, theirs)`.
- `cli/lib/open-docs.mjs` (new): write, remove and read `<state>/open/<pid>.json`.
- `cli/lib/commands/doc.mjs`: `doc.reload`, `doc.merge`, `unsavedInApp` in `doc.status`.
- `cli/lib/commands/proposal.mjs`: `proposal.apply` merges into a dirty open copy.
- `cli/lib/server-mcp.mjs`: `sync()` before each call, annotations (`doc_revert` destructive), instructions, `doc_save` description.
- `cli/lib/server-stdio.mjs`: `doc.stale`, the open-docs file.
- `cli/lib/scene-ops.mjs`: `foot()`, `followGroup()`, `groupAt()`, `setGroupAttrs()`, placing into groups.
- `cli/lib/commands/scene-edit.mjs`: `keepGroup` on move, set and snap; `group`, `newGroup`, `foot`, `index` on place; `scene.group.set`.
- `cli/lib/motion-ops.mjs` (new): tracks, keys, `offGridMoves()`, `motionPath()`.
- `cli/lib/commands/scene-motion.mjs` (new): `scene.track.add|remove`, `scene.key.set|move|delete`, `scene.motion.set|path`.
- `cli/lib/commands/check.mjs`: `warnings`.
- `cli/lib/commands/object-edit.mjs`: `object.map.set`.
- `cli/lib/registry.mjs`: the new command file.
- Tests: `cli/test/sync.test.mjs` (new: R1–R8), `cli/test/merge.test.mjs` (new), `cli/test/groups.test.mjs` (new: R9–R11), `cli/test/motion-commands.test.mjs` (new: R12–R14), `cli/test/edit-commands.test.mjs` (R15), `cli/test/servers.test.mjs` (doors, `doc.stale`, open docs).

App:
- `app/Sources/PixelLab/Server/Protocol.swift`: `doc.stale`, `ProposalApplyResult.merged/conflict`, `MergeResult`.
- `app/Sources/PixelLab/Model/AppModel.swift`: `staleDocs`, `keptStale`, `mergeFromDisk()`, `reloadFromDisk()`, `keepMine()`.
- `app/Sources/PixelLab/Model/AgentModel.swift`: the approve warning and result line.
- `app/Sources/PixelLab/Views/StaleBanner.swift` (new), `app/Sources/PixelLab/Views/MainWindow.swift` (shows it).
- `app/Sources/PixelLab/Support/ScriptRunner.swift`: `cli`, `stale`, `expect stale`.
- `app/Tests/PixelLabTests/`: the banner's text and state (pure logic).
- `scripts/acceptance/stale.txt` (new), `scripts/acceptance/stale-agent.mjs` (new), `scripts/acceptance.sh`, and `scripts/acceptance/agent.txt` if its dirty-approve wording changes.

Docs:
- `scripts/docs-cli.mjs` (`doc.stale`, `open/`, the summary rule), `docs/cli.md` (generated), `README.md`, `docs/architecture.md`, `docs/testing.md`, `cli/README.md`, `app/README.md`, `CLAUDE.md` (script-hook list).
- `docs/intents/2026-10-mcp-gaps/site-notes.md` (new): the notes for the site's pixel-art skill.

## Order of work

One slice, on `feat/mcp-gaps`, straight to a PR into `main`: the main session does every step in one sitting, and the CLI steps and the app step share `session.mjs`'s behaviour, so splitting them buys nothing.

1. This intent, spec and plan.
2. Edits since disk, settling, `doc.reload`, MCP annotations (R1–R4), with `sync.test.mjs`.
3. `merge3`, `doc.merge`, the merge in `proposal.apply`, `doc.stale`, the open-docs file and `unsavedInApp` (R6–R8, the server side of R5), with `merge.test.mjs` and server tests.
4. Groups: following, `scene.group.set`, placing into groups (R9–R11), with `groups.test.mjs`.
5. Motion: tracks, keys, `scene.motion.set`, `check` warnings, `scene.motion.path` (R12–R14), with `motion-commands.test.mjs`.
6. `object.map.set` (R15).
7. The app: the banner, the approve merge, the script hook, unit tests, the acceptance script (R5, R6 in the app).
8. Docs, `npm run docs:cli`, the MCP instructions, the site notes (R16, R17).
9. The full verification (Proof), pasted into the PR.

## Workflow and reviews

The main session runs all nine steps on `feat/mcp-gaps`, back to back, one commit each, and opens one PR into `main`; only the owner merges it. No subagents: the steps build on each other's session behaviour.

Owner review: none during the run. The banner is a new view, so its dark and light snapshots go in the PR for the owner to look at with the rest.

Independent review: a `verify-change` run on the PR, because it changes the proposals path (settling, apply's merge) and the MCP server's surface. A fresh session runs it, given only "Use the `verify-change` skill on PR #<n>."; the owner starts it (the implementer offers it as a session card). Its blocking findings are fixed before the PR merges.

## Risks

- **Settling the wrong way loses an agent's edits.** The riskiest step is 2. Every row of the settle table (spec D2) gets a test, and the only case that drops edits (reloading after a rejection with no later edits) drops exactly the rejected ones.
- **`baseSeq` breaks the app's undo.** The undo stack isn't changed, only which steps a proposal lists; the existing undo, redo and save tests must pass unchanged.
- **A merge that loads but is wrong.** Merges are refused on any overlap, re-imported and re-serialized; tests cover clean merges of items and rows, clashes, identical changes and adjacent hunks.
- **Group following surprises.** Only the item's innermost group, only on equality, reported in the result, and off with `keepGroup: false`; tests cover a tile cube's ground group, a pixel sprite's object group, a non-matching group and a keyed move (which never follows).
- **Acceptance scripts pin the art.** The new `stale.txt` uses its own placements and counts relative to them; `agent.txt` changes only if the approve wording it checks changes.
- **The real site checkout.** Every test and script works on temp copies (`makeDocument()`, `makeSite()`, the acceptance temp site).

## Proof

- `npm test` shows `# fail 0`, with the new suites' `R1:`–`R15:` tests in it.
- `npm run docs:cli`, then `git status` shows only this change's edits.
- `swift build --package-path app` with no errors; `scripts/test-app.sh` passes.
- `scripts/build-app.sh`, then `scripts/acceptance.sh` exits 0, including `stale.txt` (an outside edit to a dirty doc: banner, merge, reload, keep; an approve merged into a dirty copy).
- Snapshots of the banner in dark and light, looked at.
- `npm run docs:site` builds with no broken links.
