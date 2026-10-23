# Plan: Object libraries and palette files (from intent.md 2026-10-23)

This plan builds [`spec.md`](spec.md), R1–R30, as approved with its Areas of concern settled.

## Files that change

**Slice 1: the program** (`feat/object-libraries-program`)
- `crates/engine/src/engine.rs`:
  - `category` joins `OBJECT_PROPS` and `BLOCK_PROPS`.
  - A validator checks R2 (`category_problems`).
- `crates/engine/src/serialize.rs`: `category` comes after `notes` in the object and block key tables.
- `crates/engine/tests/engine.rs`: R1–R3 validation and output tests.
- `crates/pixellab/src/library.rs` (new):
  - Reads, validates and writes `.pixellib` packages and `.pixelpal` files (D3).
  - The import and palette plans (renames, `same`, `character` swap, differing, missing, nearest, caps), as pure functions over `Sources`.
- `crates/pixellab/src/commands/library.rs` (new): `library.info`, `library.category`, `library.export` and `library.import` (D2, D4, D5).
- `crates/pixellab/src/commands/palette.rs` (new): `palette.export` and `palette.import` (D4, D6). `palette.get` stays in `document.rs`.
- `crates/pixellab/src/commands/object.rs`: `object.list` gets a `category` filter and a `category` field. `object.show` gets `category`.
- `crates/pixellab/src/commands/object_edit.rs`: `object.category`.
- `crates/pixellab/src/commands/check.rs`: `--library`.
- `crates/pixellab/src/commands/mod.rs`, `crates/pixellab/src/registry.rs`: the new modules.
- `crates/pixellab/src/lib.rs`: `mod library`.
- `crates/pixellab/src/server_mcp.rs`: the instructions mention categories, libraries, palette files, and `chat_ask` before accepting differences (R28).
- `crates/pixellab/src/help.rs`: the two file kinds in `pixellab help` (R30).
- `crates/pixellab/tests/categories.rs` (new) and `crates/pixellab/tests/libraries.rs` (new): R1–R23 through the CLI and `serve --stdio`, with MCP tool listing in `servers.rs`.
- `crates/pixellab/tests/common/`: a `make_library()` helper, if the tests need one.
- `docs/cli.md`: generated.
- `scripts/make-examples.mjs`, `examples/harbor.pixellab/objects/*`, `examples/town.pixellab/objects/*`:
  - Categories for the example objects.
  - `examples/coast.pixellib/` (new) and `examples/coast.pixelpal` (new), made with the program's own `library export` and `palette export` (R27).
- `scripts/test/examples.test.mjs`: or whichever test checks the examples match; it covers the two new files too.

**Slice 2: the app** (`feat/object-libraries-app`)
- `app/Sources/PixelLab/Server/Protocol.swift`: `ObjectSummary.category`, and the import plan and result types.
- `app/Sources/PixelLab/Model/AppModel.swift`: `libraryTab` per document, `librarySections`, the export and import flows, Undo Import (see Departures), and opening a `.pixellib` (R18).
- `app/Sources/PixelLab/Model/LibraryCategories.swift` (new): tabs, sections and flattened cursor order as pure functions. The tests use it.
- `app/Sources/PixelLab/Model/ImportModel.swift` (new): the import sheets' state (renames, differences, conversions, keep/theirs, over-cap picks).
- `app/Sources/PixelLab/Views/LibraryPopover.swift`: the tab row, headings and keys (D7).
- `app/Sources/PixelLab/Views/ObjectPanels.swift`: the Category row in `ObjectFactsSection` (D8).
- `app/Sources/PixelLab/Views/ImportSheets.swift` (new): the library and palette import sheets (D5, D6).
- `app/Sources/PixelLab/Views/MainWindow.swift`: presents the sheets, and accepts a dropped `.pixellib` or `.pixelpal`.
- `app/Sources/PixelLab/PixelLabApp.swift`: the File menu's items (D10), and the Edit menu's Undo Import.
- `app/Sources/PixelLab/Support/ScriptRunner.swift`: D9's actions and expects.
- `app/Tests/PixelLabTests/LibraryTests.swift` (new): R5–R8 sections and keys, the R4 path parsing, and the import model.
- `scripts/build-app.sh`: UT types and document types for `pixellib` and `pixelpal` (D3).
- `scripts/acceptance/libraries.txt` (new) and `scripts/acceptance.sh`: R29 on temp copies of `examples/`.

**Integration** (the main session, on `feat/object-libraries`)
- `scripts/acceptance/site-categories.txt` (new), `scripts/acceptance/categories/site.json` (new), `scripts/acceptance/categories/explainer.json` (new) and `scripts/acceptance.sh`: R24–R26.
  - The script runs on a temp copy of the site's document and a `git archive` copy of the explainer document, read-only from the site checkout.
  - The JSON is made from the spec's appendices by `scripts/acceptance/categories/from-spec.mjs` (new), so the two never drift.
- `README.md`: the tour's Library, the File menu, and importing.
- `docs/architecture.md`: the two file kinds and the library commands.
- `docs/libraries.md` (new): the docs-site page "Libraries and palette files", linked as `docs/contributing.md` says.
- `docs/intents/2026-10-object-libraries/plan.md`: departures, as they happen.

## Order of work

```
main ── spec/object-libraries (intent + spec, PR #53) ── feat/object-libraries ───── PR 4 (owner merges) ──▶ main
                                                          ├─ PR 1  program  ◀─ feat/object-libraries-program
                                                          ├─ PR 2  app      ◀─ feat/object-libraries-app   (after PR 1 merges)
                                                          └─ integration commits (docs, site categories) ─ then PR 4
```

- `feat/object-libraries` is cut from `spec/object-libraries`, so it carries the accepted intent and the approved spec, and this plan is its first commit of its own.
- PR #53 (intent and spec only) is closed in favor of PR 4, which carries them too.
- There are two slices, because the app is built on the program's commands: the program is the foundation, merged and checked before the app slice starts.
- The docs and the site-categories pass are small and need both slices, so the main session commits them straight onto `feat/object-libraries`.

**PR 1: the program (steps 1–6)**
1. **Categories in the engine.**
   - `category` is a valid object and block property, with R2's checks in `validate`, placed after `notes` in the key tables.
   - Tests: "R1: …", "R2: …" and "R3: categories never change a render or a compiled SVG", on the starter and the harbor example.
2. **The category commands.**
   - `object.category` (one or more names, set or clear, one session edit per object, labels "Set pine's category to Nature › Trees").
   - The `object.list` filter and `category` field, and `object.show`'s `category`.
   - Tests "R9: …".
3. **The library package and plans** (`library.rs`):
   - Read, validate (`check --library`) and write, canonical.
   - The import plan: renames, `same`, the `character` swap, differing, missing with the nearest color, wrong tier.
   - The palette plan: new, differing, other-tier, over the cap.
   - Unit tests for each plan case.
4. **Library commands:**
   - `library.export` (all, category or names, with bases, `replace`, a temp folder then a rename).
   - `library.import`, which writes like `history.restore`'s `put_files`: mark, create the objects, save each, mark. That makes one version, and its number is in the result.
   - `library.info` and `library.category`.
   - Tests "R10: …" to "R17: …", including the `differs` and `missing_colors` refusals, `dryRun`, a two-level `extends` rename, and the site-shaped `character` case.
5. **Palette commands:**
   - `palette.export`.
   - `palette.import`: one edit on the palette doc, saved, so one undo step and one version.
   - Tests "R19: …" to "R23: …".
6. **Doors, docs and examples.**
   - The MCP instructions and the help text.
   - `npm run docs:cli`.
   - `scripts/make-examples.mjs` gives harbor and town categories, then writes `coast.pixellib` and `coast.pixelpal` by running the built program on a temp copy of harbor. `npm run examples` makes them, and `npm test` checks they match (R27).
   - Tests "R28: every new command is an MCP tool".

**PR 2: the app (steps 7–11)**
7. **The protocol and model:**
   - `ObjectSummary.category`.
   - `LibraryCategories`: tabs, sections and the flattened order, in pure functions.
   - `AppModel.libraryTab` per document.
   - Swift tests "R5…R7".
8. **The Library popover** (D7):
   - The tab row, its quiet scroller and accent underline.
   - Headings as section headers, and search within the tab.
   - The keys from R8, and the VoiceOver labels.
   - Script actions `librarytab`, and expects `tabs`, `libtab`, `headings`.
9. **The inspector's Category row** (D8): path parsing (› or /), the menu of categories, and R2 errors shown in place. Script actions `category`, and expect `category`.
10. **Import and export** (D5, D6, D10, R18):
    - The File menu items, the save and open panels, and dropping a file on the window.
    - The library and palette import sheets, filled from `dryRun`, and Undo Import.
    - Opening a `.pixellib` offers Import.
    - UT and document types in `build-app.sh`.
    - Script actions `import`, `importsheet` and `export`, and expect `importsheet`, `libraryobjects`.
11. **Acceptance on the examples** (R29):
    - `scripts/acceptance/libraries.txt`, added to `acceptance.sh`: categories and tabs on harbor, export a category, import `coast.pixellib` into town (a rename, a conversion), a palette import with Keep Mine, and Undo Import.
    - Snapshots of the popover with tabs and the import sheet, in dark and light.

**Integration on `feat/object-libraries` (steps 12–14)**

12. **The site's categories** (R24–R26):
    - `categories/from-spec.mjs` writes `site.json` and `explainer.json` from the spec's appendices.
    - `site-categories.txt` runs R26's four steps through `cli` lines and expects on temp copies.
    - `acceptance.sh` runs it.
13. **Docs:** README, `docs/architecture.md`, `docs/libraries.md` and its docs-site link. `npm run docs:site` builds with no broken links.
14. **The final run:**
    - Every check in Proof, on the merged branch.
    - Build the app and open it on temp copies of the examples (`scripts/try-example.sh`), with `coast.pixellib` and `coast.pixelpal` in a temp folder, for the owner to try.
    - Open PR 4.

## Workflow and reviews

| Step | Who | Where | Then |
|---|---|---|---|
| Plan | main session | `feat/object-libraries` | commit, push |
| PR 1, steps 1–6 | subagent (Sonnet), in its own worktree | `feat/object-libraries-program` | opens PR 1 into `feat/object-libraries`. The main session reads the diff, reruns the program checks, and merges it |
| PR 2, steps 7–11 | subagent (Sonnet), in its own worktree, after PR 1 merges | `feat/object-libraries-app` | opens PR 2. The main session reads the diff, looks at the snapshots, reruns the app checks, and merges it |
| Steps 12–14 | main session | `feat/object-libraries` | PR 4 into `main`, and the app running for the owner |
| Review | subagent (Opus), with only "Use the `verify-change` skill on PR #<PR 4>." | read-only | PR comment. The main session fixes blocking findings on `feat/object-libraries` before handing over |

**Merging and ownership**
- The main session merges PRs 1 and 2 into `feat/object-libraries` with `gh pr merge --squash --delete-branch`, once their checks pass. Only the owner merges PR 4 into `main`.
- When `main` moves, the main session merges it into `feat/object-libraries` before the next slice starts.
- The slices run one after the other, so no two runners own the same files.
- Slice 1 doesn't edit `app/`.
- Slice 2 doesn't edit `crates/`. If it needs a command change, it stops and reports, and the main session makes the change on `feat/object-libraries` first.

**Reviews**
- **Owner:** the owner reviews only at the end: PR 4, with the app running on example copies to try, and the Settled concerns in the spec to confirm. The owner asked for the change to be built to completion, so nothing pauses before then.
- **Independent:** one `verify-change` run on PR 4. The change adds a file format other documents will depend on, a new MCP surface, and import paths that must never change existing art.

## Risks

- **Undo for an import that spans many docs.**
  - The session's undo is per doc, so "one undo step" (R12) can't be one `doc.undo`.
  - Handled: the import is one version (`put_files`'s pattern). The app's Edit › Undo Import runs `history.undo` on that version while no edit has followed, and Versions offers the same thing.
  - Recorded under Departures.
- **Categories reaching output** (R3). This is the riskiest rule, because it touches what the site publishes.
  - Step 1's test compiles the harbor and town examples with and without categories and compares bytes.
  - Step 12 does the same on a copy of the site's document.
  - Stop if either differs.
- **An older engine refusing `category`.** The site's pin must move first (spec, Areas of concern 1). Nothing in this change writes to the site repo. Step 12 reads it only through `git archive` and temp copies.
- **The `extends` rewrite on rename.** A missed reference leaves an imported object extending the document's own same-named object, and silently changes its look. The tests cover a two-level chain, and a rename whose base is `same`.
- **The `character` swap.** It must count as a difference and never happen silently. A test imports a library with a differing `character` and expects `differs`.
- **The popover's height and keys.** The tab row takes room from the grid, and the cursor must cross headings. Swift tests cover the flattened order. The snapshots check the fixed height.
- **Long-running checks.** `scripts/acceptance.sh` takes minutes. Run it in the background.

## Proof

- **Program:**
  - `cargo fmt --check`
  - `cargo test`, with no failures, including the new "R…" tests
  - `npm test`, showing `# fail 0`, which includes the examples matching
  - `npm run docs:cli`, leaving `git status` clean apart from the change's own files
- **App:**
  - `swift build --package-path app`, with no errors
  - `scripts/test-app.sh`, with every test passing
  - `scripts/build-app.sh`
- **End to end:**
  - `scripts/acceptance.sh`, exiting 0, with `libraries.txt` and `site-categories.txt` passing
  - The site's published SVGs byte-identical before and after categorizing and importing
- **UI:** snapshots in dark and light of:
  - the Library popover with tabs and headings (harbor, categorized)
  - the inspector's Category row
  - the library import sheet with a rename and a conversion
  - the palette import sheet
  Each is looked at before it's reported, and they're attached to PR 4.
- **Docs:** `npm run docs:site`, building with no broken links.
- **For the owner:** the built app open on a temp copy of an example, ready to try.

## Departures

- **R12 and D5, "one undo step":** the session's undo is per doc. A library import is instead one version, which Edit › Undo Import (while nothing has followed it) and Versions' Undo These Changes take back whole.
- **R9, "several objects' at once, in one edit":** `object.category` on several objects is one command and one version, with one undo step per object.
