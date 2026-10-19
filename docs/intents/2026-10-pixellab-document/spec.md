# Spec: Pixel Lab owns the art: a `.pixellab` document, and a build that publishes (from intent.md 2026-10-19)
Status: draft (reviewed with its intent, plan and code in one PR, at the owner's request).

This change supersedes the "site is the source of truth" section of the architecture record (`../2026-10-mac-app/plan.md`): the engine's home is this repo, and the art lives in a document Pixel Lab owns. The site side is the change `2026-10-art-to-pixel-lab` in the site repo, which depends on this one.

## Requirements

**The document**

- **R1. A `.pixellab` document is a folder bundle.** It holds `manifest.json`, `palette.mjs`, `objects/*.mjs` and `scenes/*.mjs`, the sources exactly as the engine reads them today. Finder shows the folder as one document (the app declares the type as a package), so it can be copied, renamed and double-clicked like a file. A document is self-contained: nothing in it refers to a site checkout.
- **R2. The manifest says what's needed to build and preview.** `manifest.json` holds the format version, a name, the publish folder (relative to the document, optional) and per-output size budgets, plus the colors previews and the night theme use. Everything in it is optional but the format version; defaults are the site's values today, so a document without a manifest (a legacy `source/` folder) still builds as `npm run art` built it.
- **R3. Every door opens a document.** The CLI, `serve --stdio` and `serve --mcp` take `--document <path>`, else `PIXELLAB_DOC`, else `{"document": "<path>"}` in `~/.config/pixellab/config.json`. `--site <checkout>` and `PIXELLAB_SITE` keep working and mean "the document inside that checkout" (a `*.pixellab` under `src/assets/pixel-art/`, or its legacy `source/` folder), so the site's skills and scripts keep working through the transition. With nothing given, the document in the default site checkout is used when it exists; otherwise the error says how to name one. The default is a convenience for the owner, not a dependency: every command works on a document anywhere.
- **R4. Make, open and save as, with no site.** `document.new <path>` makes an empty document with the starter palette and a manifest. `document.copy --to <path>` copies the open document whole (Save As), refusing to overwrite. `document.info` replaces `site.info`. In the app: File › New Document…, Open Document… (⌘O), Save As… (⇧⌘S); the setup screen and Settings ask for a document, not a site checkout; the window title shows the document's name. Opening a `.pixellab` from Finder opens it in the app.
- **R5. Saves and approvals land in the document.** `doc.save` and an approved proposal (`proposal.apply`) write the document's source files, as they write the site's today. Nothing else changes in the proposals' format, the state dir or the Agent window: agents still propose, the owner still approves, ghosts and diffs still show.

**The engine and the build**

- **R6. The engine lives here.** `engine.mjs`, `iso.mjs`, `motion.mjs`, `serialize.mjs`, `edit.mjs`, `starter.mjs` and `svg.mjs` move from the site into this repo, unchanged in behavior, with the compiler (`optimize-pixel-art.mjs`'s parse, optimize, lossless check and budgets) and the PNG previews (`pixel-art-preview.mjs`). Nothing is imported from a site checkout at run time any more. The modules are exported from the package (`pixellab/engine`, `pixellab/compile`, …) so the site's tests can import them.
- **R7. `art build` compiles and publishes.** It validates every source, renders each scene with an `output` to a rect SVG, optimizes it to paths, checks the result lossless and within its budget, and writes `<publish dir>/<output>`. The publish folder is the manifest's, or `--out <dir>`; with neither it refuses and says so. The output bytes are what `npm run art` writes today (the existing art compiles byte-identical). It runs in-process: no npm, no `node_modules`. Its report is the same one-line-per-output text, and it still lists unsaved open docs it leaves out.
- **R8. The headless toolkit stays whole.** `check` (exists), `art preview <name>` (the card-colored PNGs at 1×–4× an agent reviews, and `palette` for the swatch sheet), `art sizes <scene>` (bytes per object and color against the document's git HEAD), `object new` and `scene new` (exist) cover everything `npm run art -- --check | --preview | --sizes | --new` did, with the same messages. `art build` stays out of MCP (a person publishes); the rest are tools.
- **R9. Tests run without a site checkout.** The engine's, compiler's and preview's tests come with the code (ported from the site, names kept). A small fixture document under `cli/test/fixtures/` lets the core suites run anywhere; the suites that pin the owner's art still copy it from the site checkout when one is present and skip when not, as today.

**The app**

- **R10. Publish (⌘B) replaces Build Art.** It runs `art build` and shows the report, the publish folder, and problems or over-budget outputs plainly. When the document has no publish folder, the app asks for one (a folder panel) and passes it as `--out`; it doesn't store it in the document.
- **R11. The setup screen and Settings.** They ask for a document (choose a `.pixellab`, or make a new one) and for Node, with the same clear problems as today. `PIXELLAB_DOC` (or `PIXELLAB_SITE`, legacy) overrides for the script hook and snapshots.
- **R12. The script hook and the acceptance pass cover the new flows:** `publish`, `expect outputs <n>`, `saveas <path>`, `expect document <name>`; the acceptance pass publishes into a temp copy of the site and checks the SVGs it writes. The hook still refuses the real site checkout, and now also the real document inside it.

**Cross-cutting**

- **R13. No new dependencies**, npm or Swift. The engine and compiler are zero-dependency ESM; previews use `node:zlib`.
- **R14. Docs.** The README (requirements, first launch, tour, CLI, MCP), `docs/architecture.md` (the document is the source of truth; the engine's home), `cli/README.md`, `app/README.md`, `docs/testing.md`, `CLAUDE.md` and the generated `docs/cli.md` describe the document, Publish and the moved engine. The architecture record gets a note at the top pointing here; it isn't rewritten.
- **R15. Install and release.** The app still bundles the CLI (now with the engine). `package.json` is named `pixellab`, exports the engine modules and lists the files a git install of the package needs, so the site can depend on it (`"pixellab": "github:jlawcordova/jlawcordova-lab#<ref>"`).

## Design

### D1. Files

```
cli/lib/engine/                 the engine's home (from the site's src/lib/pixel-art/ and scripts/)
  engine.mjs iso.mjs motion.mjs serialize.mjs edit.mjs starter.mjs svg.mjs   verbatim
  compile.mjs                   parse, optimizeSvg, verifyLossless, BUDGET, compileScene, sceneSizes, dependencies
  preview.mjs                   renderPreview, renderPalette, encodePng, MATERIALS, DEFAULT_PREVIEW colors
  starter-palette.mjs           the world and outfit tiers a new document starts from
cli/lib/document.mjs            replaces site.mjs: resolve, check and open a document; manifest; loadDocs
cli/lib/commands/art.mjs        art.build (in-process), art.preview, art.sizes
cli/lib/commands/document.mjs   document.info, document.new, document.copy (site.mjs → palette.get moves here)
cli/test/fixtures/sample.pixellab/   manifest, palette, two objects, one scene with an output
cli/test/engine.test.mjs compile.test.mjs preview.test.mjs art-commands.test.mjs document.test.mjs
app/…/Server/Locator.swift      documentProblem(path) instead of siteProblem
app/…/Views/SetupView.swift     Document chooser and New Document…; Settings
app/…/Views/PublishSheet.swift  renamed from BuildArtSheet
app/…/Model/AppModel.swift      documentPath, publish(), newDocument(), openDocument(), saveAs()
app/…/PixelLabApp.swift         menus; AppDelegate for application(_:open:)
scripts/build-app.sh            CFBundleDocumentTypes + UTExportedTypeDeclarations
```

### D2. The document

`manifest.json`:

```json
{
  "format": 1,
  "name": "jlawcordova",
  "publish": {
    "dir": "..",
    "budgets": { "hero-island.svg": { "raw": 768000, "gzip": 153600 }, "range-sprite.svg": { "raw": 204800, "gzip": 51200 } }
  },
  "preview": { "card": "#E9DCC6", "grid": "rgba(90, 62, 43, 0.06)" },
  "theme": { "darkPage": "#1C1712" }
}
```

- `format` is required and must be `1`. Unknown keys are an error naming the key. `publish.dir` is relative to the document folder (absolute allowed). `budgets` default to the engine's 100 KB raw / 25 KB gzip; the two above are the site's exceptions and go in the site's manifest. `preview.card` and `preview.grid` are what `previewColors()` read from the site's `variables.css`; `theme.darkPage` is what `darkPageColor()` read. The defaults in code are these values.
- **Resolution** (`document.mjs`): `--document` → `PIXELLAB_DOC` → config `document` → `--site`/`PIXELLAB_SITE`/config `site` → the default site. A site path resolves to the first `*.pixellab` folder under `src/assets/pixel-art/`, else the legacy `src/assets/pixel-art/source/` folder, which is treated as a document whose manifest is `{ format: 1, publish: { dir: '..', budgets: <the site's two> } }`. A document is checked by its `palette.mjs` (and its manifest, when present); the error names what's missing and how to name a document.
- `openDocument()` returns `{ root, from, kind: 'document' | 'legacy-site', sourceDir, manifest, publishDir | null, budgets, preview, theme, engine modules, loadDocs }`. The engine modules are imported from `cli/lib/engine/`, once. The `Site` type and `ctx.site` are renamed `Document`/`ctx.document` in code, with `site` kept as an alias getter for one release to limit the diff (removed in a follow-up).

### D3. Commands

| Command | Does | MCP |
| --- | --- | --- |
| `art build [--dir <folder>]` | R7. Result `{ ok, publishDir, outputs: [{ file, scene, raw, gzip, over, report }], previewOnly: [{ scene, … }], problems, ms, unsaved }`. Problems → `ok: false`, nothing written (as the site: validate everything first); over budget → `ok: false`, still written, as `npm run art` did. (`--dir`, not `--out`: `--out` is the global flag that writes a render's PNG.) | left out |
| `art preview <name> [--scale N]` | The card PNG: one image at `scale` (default 1), as `png` in the result; the global `--out <file.png>` writes it. `palette` renders the swatch sheet. An agent that wants the four sizes the site wrote calls it four times. | tool |
| `art sizes <scene>` | Bytes per object and color now and at HEAD (the document's git repo), the same tables as `--sizes`. | tool |
| `document info` | `{ root, from, kind, name, publishDir, branch, scenes, objects, colors, problems }`. | tool |
| `document new <path> [--name]` | Writes `manifest.json` and the starter palette; refuses an existing path; `<path>` gets `.pixellab` if it lacks it. | tool |
| `document copy --to <path>` | Copies the open document's folder; refuses an existing path. Returns `{ from, to }`. | left out (a person saves as) |
| `check`, `object new`, `scene new`, the rest | unchanged | as today |

`site.info` is removed (`document info` replaces it); `palette.get` moves to `document.mjs`'s command file. Help text and `docs/cli.md` are regenerated.

### D4. Servers and session

- `session.info` gains `document: { root, name, kind, publishDir }` and keeps `site` (= root) for the app until the app reads `document`.
- The stdio server watches `sourceDir` as today. The MCP server's `LEFT_OUT` gains `document.copy`; its instructions say "the document" instead of the site.
- `Session` save and `proposal.apply` write under `sourceDir` as today; no format change to proposals.

### D5. The app

- `AppModel.documentPath`: `PIXELLAB_DOC`, else `PIXELLAB_SITE` (passed through as `--site`), else Settings `documentPath`, else the default site's document. `ServerLaunch` passes `--document` or `--site` accordingly.
- Setup screen: "Document" row (path, problem, Choose…, New…). Choose… is an `NSOpenPanel` with `canChooseDirectories`, `treatsFilePackagesAsDirectories = false`, allowed type the exported UTI (and any folder holding `palette.mjs`, for legacy). New… is an `NSSavePanel` for a `.pixellab` name, then `document new` through a one-shot CLI run (the server isn't up yet) and a restart. Settings: the same row.
- Menus (File): New Scene…, New Object… (as today); New Document…, Open Document… ⌘O; Save ⌘S, Save As… ⇧⌘S (`document copy`, then restart on the copy), Revert; Publish ⌘B. The Build Art sheet becomes `PublishSheet`: title "Publish", the publish folder (or "Choose a folder…" when the manifest has none), each output's line, problems, over-budget in gold, the same success and failure states.
- `PixelLabApp` gets an `NSApplicationDelegateAdaptor` whose `application(_:open:)` sets the document path and restarts the server (after a save prompt when dirty, as Settings' restart does).
- Window title: the document's `name` (manifest) or folder name, then the open doc.
- `Info.plist` (build-app.sh): `CFBundleDocumentTypes` with `LSItemContentTypes = com.jlawcordova.pixellab.document`, `LSHandlerRank Owner`, `LSTypeIsPackage`; `UTExportedTypeDeclarations` conforming to `com.apple.package`, extension `pixellab`.
- ScriptRunner: `PIXELLAB_DOC` or `PIXELLAB_SITE` required; refuses the real checkout and any path inside it. New actions: `publish [dir]`, `saveas <path>`, `expect outputs <n>`, `expect document <name>`, `expect publishdir <path>`.

### D6. Tests

- `cli/test/engine.test.mjs`: the site's `pixel-art-engine.test.mjs`, imports repointed, the five lines that read the site's sources moved to the sample fixture. `compile.test.mjs`: `optimize-pixel-art.test.mjs` plus `compileScene` against the fixture and, when the site is present, byte-identity against its committed SVGs (`R7: the site's art compiles byte-identical`). `preview.test.mjs`: the preview parts. `art-commands.test.mjs`: the site's `pixel-art-cli.test.mjs` flows as `pixellab art …` on a temp copy. `document.test.mjs`: resolution (R3), manifest errors (R2), new and copy (R4).
- `helpers.mjs`: `makeSite()` becomes `makeDocument()`: copies the sample fixture by default, or, with `{ art: 'site' }`, the site checkout's document (bundle or legacy `source/`) into a temp `.pixellab` with a manifest; `needsSite` as today. `SITE_PARTS` loses the engine and scripts.
- `scripts/acceptance.sh`: still a temp copy of the site; `scene-object.txt` ends with `publish` and `expect outputs 6`; a new `document.txt` does `saveas` and `expect document`.

### D7. Package

`package.json`: `"name": "pixellab"`, `"exports": { "./engine": "./cli/lib/engine/engine.mjs", "./iso", "./motion", "./serialize", "./edit", "./starter", "./svg", "./compile", "./preview", "./document" }`, `"files": ["cli/bin", "cli/lib", "cli/README.md"]`, `"bin"` as today. `private` stays (never published to npm; installed from git).

### D8. Copy

- "Document" everywhere the app said "site checkout"; "Publish" for the build; the report's lines unchanged ("hero-island.svg: 512.3 KB / 98.1 KB gzip → … lossless").
- Setup: "Pixel Lab edits pixel art in a .pixellab document with Node.js." Problem: "There's no document at …", "… isn't a Pixel Lab document (no palette.mjs)."

## Areas of concern

- **The architecture record flips.** `CLAUDE.md`'s "The site checkout is the source of truth… never copy or port [the engine]" becomes "the engine lives here; the document is the source of truth; never write to the real document or site checkout from tests". Resolved by the intent: the owner asked for the flip. The record `2026-10-mac-app/plan.md` gets a pointer, not a rewrite.
- **A copy of the engine, not a move with history.** Copying files loses `git log` on them in this repo; `git subtree` would import the site's whole history. Decision: copy, with the site commit the copy was taken from named in the commit message and a header comment. The owner may prefer subtree; it changes nothing else.
- **Two repos in flight.** Until the site's PR merges, the site's `pixel-artist` agent and skills run `npm run art` against `source/`; this repo's `--site` keeps working on that layout (legacy mode), so neither PR breaks the other. After both merge, `--site` resolves to the bundle. The site pins this repo by commit until a release tag exists; bumping it to the tag is a follow-up.
- **The default document.** Keeping `~/Projects/jlawcordova.github.io` as the default when nothing is configured is owner-specific. It's a fallback with no dependency, and the app stores its own path; the owner can drop it later.
- **Node stays a requirement** (intent decision). The app bundles the CLI and finds Node as today.

## Open questions

- From the intent: where the site's art tests run. Answer: the engine's tests move here (R9); the tests of the site's own art stay in the site and import the engine from this package (site spec).
- From the intent: how a document says where its build goes. Answer: `publish.dir` in the manifest, relative to the document, so the site's document publishes next to itself and a copy saved elsewhere publishes next to the copy, or where `--out` says (D2, R7).
- Should the app remember several documents (Open Recent)? Carried forward; one document at a time now.
