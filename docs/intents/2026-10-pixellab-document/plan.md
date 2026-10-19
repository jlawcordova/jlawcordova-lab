# Plan: Pixel Lab owns the art: a `.pixellab` document, and a build that publishes (from intent.md 2026-10-19)

## Files that change

- **Engine home (new):** `cli/lib/engine/{engine,iso,motion,serialize,edit,starter,svg}.mjs` (copied from the site's `src/lib/pixel-art/` at the commit named in the copy's message), `cli/lib/engine/compile.mjs` (from `scripts/optimize-pixel-art.mjs`), `cli/lib/engine/preview.mjs` (from `scripts/pixel-art-preview.mjs`), `cli/lib/engine/starter-palette.mjs`.
- **Document:** `cli/lib/document.mjs` (new; replaces `cli/lib/site.mjs`, deleted), `cli/lib/context.mjs`, `cli/lib/session.mjs`, `cli/lib/render.mjs`, `cli/lib/cli.mjs`, `cli/lib/serve.mjs`, `cli/lib/argv.mjs`, `cli/lib/help.mjs`, `cli/lib/server-stdio.mjs`, `cli/lib/server-mcp.mjs`, `cli/lib/commands/document.mjs` (new; replaces `cli/lib/commands/site.mjs`, deleted), `cli/lib/commands/art.mjs`, `cli/lib/commands/proposal.mjs`, `cli/lib/registry.mjs`.
- **Tests:** `cli/test/fixtures/sample.pixellab/` (new), `cli/test/engine.test.mjs`, `cli/test/compile.test.mjs`, `cli/test/preview.test.mjs`, `cli/test/art-commands.test.mjs`, `cli/test/document.test.mjs` (new), `cli/test/helpers.mjs`, `cli/test/site.test.mjs` (→ folded into `document.test.mjs`), every suite that names `makeSite` or `--site`.
- **Package:** `package.json` (name, exports, files), `docs/cli.md` (generated).
- **App:** `app/Sources/PixelLab/Server/Locator.swift`, `Server/ServerConnection.swift`, `Server/Protocol.swift`, `Model/AppModel.swift`, `Model/Settings.swift`, `Views/SetupView.swift`, `Views/PublishSheet.swift` (renamed from `BuildArtSheet.swift`), `Views/MainWindow.swift`, `PixelLabApp.swift`, `Support/ScriptRunner.swift`, `app/Tests/PixelLabTests/*` for the new model code, `scripts/build-app.sh` (document type in Info.plist).
- **Scripts:** `scripts/acceptance.sh`, `scripts/acceptance/scene-object.txt`, `scripts/acceptance/document.txt` (new), `scripts/acceptance/mcp-agent.mjs`, `scripts/snapshot.sh`.
- **Docs:** `README.md`, `CLAUDE.md`, `docs/architecture.md`, `docs/testing.md`, `cli/README.md`, `app/README.md`, `scripts/README.md`, `docs/intents/2026-10-mac-app/plan.md` (a pointer at the top only), `docs/screenshots/document-*.png` (new).
- **This change:** `docs/intents/2026-10-pixellab-document/{intent,spec,plan}.md` (new).

## Order of work

One slice, on `feat/pixellab-document`, straight to a PR into `main`: one runner (the main session) does it in one sitting and the owner asked for one PR per repo to review. One commit per step:

1. The change record: intent, spec and plan.
2. **The engine's home (R6, R9, R13):** copy the seven modules, port the compiler into `compile.mjs` (exports: `parse`, `optimizeSvg`, `verifyLossless`, `isRectGroup`, `rectPixels`, `attr`, `BUDGET`, `compileScene(sources, name, budgets)`, `compileAll`, `sceneSizes`, `headSources`, `countItems`) and the previews into `preview.mjs` (`previewColors(manifest.preview)` from the manifest or defaults); add the sample fixture document and `starter-palette.mjs`; port the engine, optimizer and preview tests with their names kept. *(As built: the preview tests stay inside `engine.test.mjs` as they were in the site; the site-art sections of the engine test, library, outfit and palette brand checks, go to the site's own tests. `--doc` clashed with the `doc` param of the `doc.*` commands, so the global flag is `--document`.)*
3. **The document (R1–R5):** `document.mjs` (resolution `--document` → `PIXELLAB_DOC` → config → `--site`/`PIXELLAB_SITE` → default site; manifest parsing and errors; legacy `source/` folders; `openDocument()`), `ctx.document` with `ctx.site` kept as an alias, `render.mjs` taking the theme and preview colors from the document, `document info | new | copy`, `palette get` moved, `session.info` with `document`; `helpers.mjs`'s `makeDocument()` (and `makeSite()` kept, laying the site's art out as a legacy checkout, so the art suites change little); `document.test.mjs`; every suite repointed.
4. **The art commands (R7, R8):** `art build` in-process with `--dir`, `art preview` (one PNG at `--scale`, written by the global `--out`), `art sizes`; `LEFT_OUT` for MCP; `art-commands.test.mjs` for preview and sizes, the publish tests in `edit-commands.test.mjs`; `npm run docs:cli`. *(Steps 2–4 landed as one commit: the document module, the engine's home and the art commands depend on each other, and the suite is green only with all three.)*
5. **The package (R15):** `package.json` name `pixellab`, `exports`, `files`; a test that every export resolves.
6. **The app (R10–R12):** `Locator.documentProblem`, `documentPath` and `--document`/`--site` launch, the setup screen and Settings rows, File menu (New Document…, Open Document… ⌘O, Save As… ⇧⌘S, Publish ⌘B), `PublishSheet`, the app delegate's `application(_:open:)`, the window title, the document type in `build-app.sh`; ScriptRunner's `publish`, `saveas`, `expect outputs|document|publishdir`; Swift tests for the model's path resolution and publish state.
7. **Acceptance and snapshots (R12):** `acceptance.sh` publishes into the temp site and checks the six SVGs; `document.txt` saves as and reopens; snapshots of the setup screen, Settings and the Publish sheet in dark and light.
8. **Docs (R14):** README, `CLAUDE.md` (the rules flip), `docs/architecture.md`, `docs/testing.md`, the three code READMEs, the pointer in the architecture record; `npm run docs:site` clean.
9. Verification run (Proof) and the PR.

## Workflow and reviews

The main session runs every step in order on `feat/pixellab-document` in its own worktree, commits each, pushes with `git push -u origin feat/pixellab-document` and opens the PR into `main` citing this plan. Owner review: the PR, where the owner reads the record, the diff and the snapshots, and tries the installed build if they wish; the owner merges. Independent review: none planned; the ported tests (byte-identity of the site's art, the engine's own suite), the Swift tests, the acceptance pass and the snapshots are the verification. The owner can ask for a `verify-change` run on the PR.

## Risks

- **The art changes by a byte.** The compiler is ported, not rewritten, and `compile.test.mjs` compares every committed SVG in the site checkout with what the ported compiler produces from its sources (`R7: the site's art compiles byte-identical`). If it fails, fix the port.
- **The two repos drift during the transition.** The lab's `--site` keeps resolving a legacy `source/` folder, so the site's agents keep working until its PR lands; the site pins the lab by commit. Tested on both layouts (the sample fixture, and the site checkout as it is).
- **The app's document type and open-from-Finder** can't be checked by the script hook. The plist is checked by `scripts/build-app.sh` (plutil lint) and the delegate path is unit-tested at the model level; opening from Finder is tried by hand once and noted in the PR.
- **Riskiest step: 3**, where every command's ctx and every test's helper change at once. It lands as one commit with `npm test` green on both layouts before step 4 starts.
- **`CLAUDE.md`'s rules** are read by every future session; the flip is written plainly ("the engine lives here") so no session re-imports the engine from the site.

## Proof

- `npm test` shows `# fail 0`, with the ported engine, compiler, preview and CLI suites and the new document tests, run twice: with the site checkout present and with `PIXELLAB_TEST_SITE` pointing nowhere (the core suites run on the fixture, the art suites skip).
- `npm run docs:cli` and `npm run docs:site` leave `git status` clean and build with no broken links.
- `swift build --package-path app` with no errors; `scripts/test-app.sh` passes.
- `scripts/build-app.sh` builds; `plutil -lint` passes on its Info.plist; `scripts/acceptance.sh` exits 0, with the publish step writing six byte-identical SVGs into the temp site.
- Script-hook runs with `expect outputs 6`, `expect document <name>` after `saveas`, and snapshots of the setup screen, Settings and the Publish sheet in dark and light, looked at before reporting.
