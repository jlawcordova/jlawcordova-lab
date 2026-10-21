# Spec: The engine in Rust, and a site that only takes its output (from intent.md 2026-10-21)
Status: draft.

This spec turns the accepted intent, as amended on 2026-10-21 with the owner's decisions about the hop and the budgets, into requirements and a design. It supersedes parts of `../2026-10-pixellab-document/spec.md`: R6's "the modules are exported from the package so the site's tests can import them", R15's git-installed package, and R7's over-budget failure. Everything else in that change (the document as a folder bundle, resolution, publish folders, Save As) stands, now on the Rust program and JSON sources.

What the site uses today was read from `jlawcordova.github.io` at `main` on 2026-10-21, read-only:
- **Library imports:** seven test files and `scripts/blog-thumbnails.mjs` import `pixellab/*`.
- **Animation CSS:** `src/styles/pixel-art.css` holds the frame loops (`wf`/`ff`/`hf`), the clouds' drift (`pcloud`, `pc1`, `pc2`), the character's bob (`cbob`) and its hop (`chop`, on `.cbob.is-hopping`).
- **Triggers:** `Range.astro` toggles `is-hopping`, and `Hero.astro` and `Range.astro` play the "build itself" entrances from `.enter` groups with the Motion library.
- **The document:** 204 objects, 28 scenes and a palette. All of it is literal data: no imports, functions or spreads. Its only comments, apart from the generated header lines, are in `palette.mjs`.
- **Frame loops:** every one in the document already has the frame count `pixel-art.css` plays (`ff` 4, `wf` 5, `hf` 6).

## Requirements

### The program

- **R1. One Rust program replaces the Node CLI.** `pixellab` is a single native executable with the same commands, params, JSON results, human output, exit codes and errors as the Node CLI it replaces: every command in `docs/cli.md`, the Agent chat's `chat.ask` and `chat.context` included. It keeps all three doors: the CLI, `serve --stdio` (the app's JSON-RPC, same methods and notifications) and `serve --mcp` (the same tools, instructions and left-out commands, with saves still becoming proposals).
- **R2. It is the only engine.** Validating, resolving, rendering, depth order, motion, compiling, the lossless check, previews, edits and serializing are all in Rust. When the change is complete, no JavaScript engine is left in this repo, the npm package stops exporting `pixellab/*`, and no site imports one.
- **R3. Parity before cutover.** Until the Node CLI is removed, a parity check runs both programs over a corpus: the test fixture, both examples, and a temp copy of the owner's document migrated to format 2. Their outputs must agree in four ways:
  - every scene's SVG is byte-identical;
  - every preview PNG is pixel-identical (dimensions and decoded RGBA);
  - every read command's JSON result is equal (ignoring `ms`, ids and times);
  - validation problems are the same text.

  R3 holds for the outputs as they are today, before R11–R15 change them.
- **R4. Over budget warns, never fails.** A scene over its raw or gzip budget is still written, and its result is `ok: true` with a `warnings` list. This applies everywhere:
  - `art build` over the CLI prints the warning and exits 0;
  - the app shows it in the publish report;
  - over MCP, the tool result carries it;
  - the site's build in CI prints it.

  Problems in the sources (validation errors) still stop a build, and nothing is written.
- **R5. Dependencies.** The program uses only the crates this spec lists in D2. Any other crate needs the owner's approval, as npm and Swift packages do today.

### The document

- **R6. Format 2: JSON sources.** A format-2 document holds:
  - `manifest.json` with `"format": 2`;
  - `palette.json`;
  - `objects/<name>.json`;
  - `scenes/<name>.json`.

  Each source file holds what the `.mjs` file's default export holds today, as JSON, with the same keys and the same meaning. Every file equals the canonical serialization of itself (D4), so hand edits and saves give small, comparable diffs.
- **R7. Notes.** The palette keeps its comments:
  - `notes`, a list of lines, holds the header text;
  - `colorNotes` maps a color's name to its note, for example `"soil-3": "--color-earth"`.

  An object or scene may have `notes` too. The generated header line ("Pixel-art object. How to edit it: …") is dropped.
- **R8. Migration.** `pixellab document migrate [--to <path>]` converts a format-1 document (`.mjs` sources, or the legacy `source/` folder) to format 2:
  - It converts in place by default, or writes a copy with `--to`.
  - It carries every comment into notes.
  - Before writing, it checks that the migrated document builds byte-identical SVGs to the original, and refuses if it doesn't.
  - It works over all three doors, and over MCP it is left out, like `document.copy`.
  - It is written in Rust, so it outlives the Node CLI.
- **R9. Format 1 is migrated, not edited.** After cutover, opening a format-1 document gives the problem "<name> uses the old format. Migrate it with `pixellab document migrate`, or in the app." The app's setup screen offers **Migrate…**, which asks first and then migrates in place.
- **R10. The examples and the fixture are format 2.** `examples/harbor.pixellab`, `examples/town.pixellab` and `cli/test/fixtures/sample.pixellab` are migrated. `npm run examples` regenerates them with the program, and `npm test` checks they match.

### Self-contained art

- **R11. Frame loops in the SVG.** An SVG whose art has a frame loop (`wf`, `ff`, `hf`, or any other loop the engine plays) carries that loop's keyframes, its per-frame timing and its reduced-motion rule in its own `<style>`. It plays the way `pixel-art.css` plays it today: the same cycle, the same frames, and frame 0 under reduced motion.
- **R12. Ambient loops.** A scene item and an object's map layer may carry an **ambient loop**: a repeating move of its own, with its own duration and offset, that steps a whole pixel at a time between keys. It plays in the SVG on its own, and holds still at rest under reduced motion. This replaces the site's `pcloud` and `cbob` classes, whose motion it can express exactly.
- **R13. Actions.** A scene item and an object's map layer may carry named **actions**: one-shot moves, stepped like ambient loops, that play only when asked. The SVG carries them, and the site plays them through a two-way contract:
  - **To play an action,** the page adds the action's name to a `data-play` attribute on the `<svg>`, or on any group that contains the item. The value is a space-separated list, for example `data-play="hop"`. While it plays, the action replaces the element's ambient loop.
  - **To restart it,** the page removes the name and adds it back.
  - **When it ends,** the browser fires its standard `animationend` event, which bubbles. Its `animationName` is the action's documented name (D7).
  - **Under reduced motion,** an action doesn't play and no event fires.

  This carries the Range character's hop.
- **R14. Crisp edges in the SVG.** Every compiled SVG says `shape-rendering="crispEdges"` itself, so it renders crisply without the site's `.pixel-art` rule.
- **R15. No clashes on a page.** Every class and keyframe name the compiler generates is scoped to its scene, so two SVGs inlined on one page never clash. The classes and `data-*` attributes a document gives its groups and items pass through unchanged (R19).
- **R16. The art moves the same.** For every scene in the owner's document, the new SVG on a bare page and the old SVG with `pixel-art.css` show the same pixels:
  - at time 0;
  - at sampled times across every loop;
  - under reduced motion;
  - for the hop, through one play.

  Timings converted from the site's CSS may differ by no more than 4 ms. A visual check in Chromium proves this (D9).

### The site takes only output

- **R17. Two builds per release.** Each GitHub Release publishes `pixellab-darwin-arm64.tar.gz` and `pixellab-linux-x64.tar.gz`, each holding the one executable, alongside a `SHA256SUMS` file and the app's `.dmg` and `.zip`. The asset names stay the same from release to release, so the site can pin a version and verify the download.
- **R18. One build makes everything the site uses.** `art build` writes every compiled SVG, and every PNG image a scene declares (D5), into the folders the manifest names. This takes over from the site's thumbnails script. The site's mapping of blog posts to thumbnails stays in the site, as content.
- **R19. A documented contract for what the site reaches into.** `docs/site-contract.md` says what a site may rely on in a compiled SVG:
  - the root's `data-pixellab` attribute;
  - the classes and `data-*` attributes the document's groups and items carry, such as `data-class`, `enter`, `data-enter`, `data-row` and `data-x`;
  - actions, played through `data-play` and ended by `animationend`;
  - what reduced motion does.

  Nothing else in the SVG is a promise.
- **R20. The site's art tests come here.** The site's tests of the art are ported to this repo, keeping their names:
  - `pixel-art-island`;
  - `pixel-art-sources`, without its palette ↔ `variables.css` check;
  - `pixel-art-roundtrip`;
  - `pixel-art-motion`;
  - `range-art`;
  - `achievement-icons`.

  They run against a temp copy of the owner's document when a site checkout is present, and skip otherwise, as the art suites do today. They also run in this repo's CI against the site's `main` (R25).

  Tests that compare the art with the site's own files are dropped and not replaced. These are `range-classes` (against `src/data/home.ts`), the palette ↔ `variables.css` check, and the thumbnails' size and staleness checks.

### The app

- **R21. Nothing to install first.** The app bundles the program. It never looks for Node, and starting it needs nothing installed. The setup screen asks only for a document. Settings loses its Node.js field, and the stored setting is ignored.
- **R22. The app runs the bundled program everywhere.** That covers:
  - its own server (`serve --stdio`);
  - one-off runs (the script hook's `cli` action);
  - the Agent chat's MCP config, whose `command` becomes the bundled program.

  The `claude mcp add` command shown in the Agent window, printed by `install.sh` and given in the README, points at the bundled program.
- **R23. The app shows and plays what the SVG plays.** The stage plays ambient loops on the timeline, as it plays scene motion. An item or layer with actions shows them in the Inspector, each with a **Play** button that previews it once on the stage. Play has a VoiceOver label and visible focus, and under Reduce Motion it shows the rest frame. Editing ambient loops and actions is done by command in this change (R24). An editing UI is a follow-up.
- **R24. Every new action is a command.** Each is available over all three doors. Over MCP, the edits become proposals.
  - setting, changing and clearing an ambient loop on a scene item or an object layer;
  - adding, changing and deleting an action;
  - rendering a scene or object at a time, with an action at a time into its play.
- **R25. Clean slate for proposals.** Proposals written before format 2 are no longer valid. On first start, the program moves them to `<state dir>/retired-proposals/`, where they're no longer listed, approved or rejected. A chat card for one reads "Settled elsewhere", as for any proposal that left the list. Activity, chats and proposal numbers carry on.
- **R26. File names follow the format.** Every place the app or the program names a source file says `.json`. That includes watching, problems ("scenes/x.json: items[3]: …"), chat notes ("wrote scenes/harbor.json") and the acceptance scripts.

### Release, CI and docs

- **R27. Release.** `.github/workflows/release.yml`:
  - builds the darwin-arm64 program on macOS and the linux-x64 program on Linux;
  - runs `cargo test` on both;
  - bundles the macOS program in the app;
  - smoke-tests `serve --mcp` with the bundled program;
  - publishes everything in R17.

  The tag is still the version (`PIXELLAB_VERSION`). The program reports it in `pixellab --version` and in `serve --mcp`'s server info.
- **R28. Checks on every PR.** A new workflow runs `cargo test`, `npm test` (the repo tooling's suites, and the parity check while the Node CLI exists) and the ported art suites against a shallow clone of the site's `main`, all on Linux.
- **R29. Node stays only as repo tooling.** Node is still used by the docs site builder, the fake `claude`, the parity and visual checks, and the scripts that drive the program. The app, the CLI, agents and the site never need it. Repo scripts that import the Node engine (`make-examples.mjs`, `make-icon.mjs`, `docs-cli.mjs`) run the program instead.
- **R30. Docs.**
  - `CLAUDE.md`: its commands, architecture and rules become "the engine lives here in Rust; the site takes only output".
  - `README.md`: install, the MCP setup and no Node.
  - `docs/cli.md`: generated by the program.
  - `docs/architecture.md` and `docs/testing.md`.
  - The new `docs/site-contract.md` and `docs/document-format.md`.
  - The `2026-10-mac-app` record and the `pixellab-document` spec get a pointer to this change, not a rewrite.

## Design

### D1. Layout

```
Cargo.toml                         workspace: crates/engine, crates/pixellab
crates/engine/                     the library: no I/O beyond what callers pass in
  src/iso.rs depth.rs motion.rs    ports of cli/lib/engine/*.mjs, one module per file
      engine.rs edit.rs svg.rs
      compile.rs preview.rs
      serialize.rs starter.rs
  src/ambient.rs                   ambient loops and actions → CSS (R12, R13)
  src/format.rs                    format 2 read/write, canonical JSON (D4)
  src/migrate.rs                   format 1 → 2: the JS-literal reader (D3)
  src/jsnum.rs                     JS number semantics (D2, "Byte parity")
  src/deflate.rs png.rs sha256.rs  gzip/PNG/crc32/sha-256 around miniz_oxide (D2)
  tests/                           ported node:test suites, names kept (R1, R20)
crates/pixellab/                   the executable
  src/main.rs cli.rs argv.rs help.rs
  src/registry.rs commands/*.rs    one file per cli/lib/commands/*.mjs
  src/session.rs document.rs state.rs watch.rs merge.rs diff.rs
  src/server_stdio.rs server_mcp.rs jsonrpc.rs chat_context.rs
  tests/                           servers, commands, help, state, sync
scripts/parity.mjs                 R3: runs both programs over the corpus (removed at cutover)
scripts/visual-parity.mjs          R16: Chromium screenshots, old vs new (D9)
docs/site-contract.md              R19
docs/document-format.md            R6, R7: the format, for people and agents
```

The Node CLI in `cli/` stays, unchanged in behavior, until the cutover. Then `cli/` is removed, along with `package.json`'s `bin` and `exports`. `package.json` stays for the repo tooling.

### D2. The Rust program

- **Toolchain:** stable Rust, edition 2021. `rust-toolchain.toml` pins the version so the cloud container, the owner's Mac and CI agree.
- **Crates:**
  - `serde` and `serde_json`, with `preserve_order`, which brings in `indexmap`. JSON in and out, in the order the Node CLI writes keys.
  - `miniz_oxide`, for deflate and inflate, pure Rust with no C toolchain.

  Everything else is the standard library or a small module in `crates/engine`, each tested against published test vectors:
  - the gzip wrapper, CRC-32, PNG encode and decode, and SHA-256 (for `baseHash`);
  - JSON-RPC, argument parsing and help;
  - ISO timestamps and random ids (`/dev/urandom`).
- **Watching:** the stdio server polls the document's source files and the state dir every 250 ms, comparing size and mtime, and keeps the Node CLI's 150 ms debounce for bursts. That avoids a file-watching crate. A document of a few hundred files costs well under a millisecond per poll.
- **Git:** `art build`'s "against HEAD" reports run `git` as a subprocess, as `compile.mjs` does today.
- **Byte parity:** these are the places where Rust and JavaScript differ, each with a module and tests drawn from the corpus:
  - `jsnum::round` is `Math.round`, which rounds halves up (−2.5 → −2, where `f64::round` gives −3). It's used by `iso.rs` and the previews.
  - `jsnum::to_fixed` is `Number.prototype.toFixed`, which picks the larger neighbor on a tie. It's used by `pct`, `kb` and the motion stops.
  - `jsnum::to_string` is JS shortest round-trip number printing, including JS's exponent rules. It's used everywhere a number becomes text.
  - Key order: JSON objects keep insertion order (`preserve_order`). Where the Node CLI relied on JS putting integer-like keys first, the port sorts them explicitly. `serialize.mjs`'s `prop()` shows where that happens.
  - Sorting: Rust's `sort_by` is stable, like `Array.prototype.sort`. Comparators are ported as written (`byCodePoint`).
- **Gzip sizes:** `miniz_oxide` doesn't produce zlib's exact bytes, so reported gzip sizes can differ from the Node CLI's by a few bytes. Budgets only warn now (R4), so R3 compares raw sizes exactly and gzip sizes within 1%. PNGs are compared decoded for the same reason.
- **Errors:** the same codes and messages as the Node CLI (`CommandError`, `NotFoundError`, …), checked by the ported tests.

### D3. Migration (R8, R9)

`crates/engine/src/migrate.rs` reads the subset of JavaScript that `serialize.mjs` writes and the owner's palette uses:
- `//` comments;
- `export default` followed by one object literal;
- unquoted identifier keys, or single- and double-quoted keys;
- single-quoted strings with `\\` and `\'` escapes;
- numbers, `true`, `false`, arrays and nested objects, with trailing commas.

Anything else (an import, a function, a spread, a template string, an expression) is refused with its file, line and column. Comments go to notes:
- palette header lines → `notes`;
- a comment after a palette entry → `colorNotes[name]`;
- the generated header line → dropped;
- any other comment → the file's `notes`, prefixed with the path it sat at (`items[3]: …`).

`document migrate`:
1. reads every source;
2. converts it;
3. builds both versions in memory and compares every SVG;
4. writes the format-2 files and removes the `.mjs` files in place (or writes the copy with `--to`);
5. rewrites `manifest.json` with `"format": 2`.

A legacy `source/` folder becomes `<site>/src/assets/pixel-art/<name>.pixellab/`, as the pixellab-document spec's resolution expects. The result is `{ ok, root, files, notes, removed }`. The command refuses when the document has unsaved edits open in a session.

### D4. Format 2 (R6, R7)

- **Canonical JSON:**
  - two-space indentation;
  - keys in the order `serialize.mjs` uses today (one table per kind, in `format.rs`);
  - a map's rows one string per line, and number arrays (`size`, `at.tile`, `move`) on one line;
  - no trailing commas and a final newline.

  `format::serialize(parse(text)) == text` for every committed file, and `npm test` checks the examples and the fixture.
- **Palette:**

  ```json
  {
    "notes": [
      "The shared pixel-art palette …"
    ],
    "world": {
      "grass-1": "#B5C79C"
    },
    "outfit": {},
    "colorNotes": {
      "soil-3": "--color-earth"
    }
  }
  ```

  A name in `colorNotes` must be a color in `world` or `outfit`. Validation names any stray one.
- **Objects and scenes:** today's keys, plus an optional `notes` (a list of strings) first, plus the new `ambient` and `actions` (D6).
- **Manifest:** `format` must be `2`. A `1` gives R9's problem. `publish` gains `images`, a folder relative to the document, for D5's PNGs. Everything else is unchanged.

### D5. Images from scenes (R18)

A scene may declare `"image": { "file": "rest.png", "scale": 5 }`. `art build` renders the scene's viewBox on the manifest's preview card color at `scale`, and writes it to `<publish.images>/<file>`. That's exactly what `blog-thumbnails.mjs`'s `renderThumbnail` does today with `renderPreview` and `encodePng`. Validation rejects these:
- a scene with `image` and no `publish.images` in the manifest, at build time, with the same "no publish folder" problem `art build` gives today;
- a `file` that isn't a plain `name.png`;
- a `scale` that isn't 1–16.

The site's thumbnail scenes (`thumb-*`) gain `image` in the site's change. The build result lists images with the outputs:

```
{ file, scene, kind: "svg" | "png", raw, gzip?, warnings }
```

### D6. Ambient loops and actions in the document (R12, R13)

On a scene item, or on an object map layer (beside its `class` and `map`):

```json
"ambient": { "duration": 16000, "offset": 0, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 8000, "move": [12, 0] }] },
"actions": {
  "hop": { "duration": 480, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 44, "move": [0, -2] }] }
}
```

- **Keys** are scene motion's keys, with only `t` and `move`. `t` is in ms, from 0 up to `duration`, strictly rising. `move` is pixels from where the art is drawn. Between keys the element steps a whole pixel at a time, by the rule scene motion uses (`stepsFor`). The first key must be at `t: 0`.
- **Ambient loops:** after the last key, the loop steps back to the first key over the rest of `duration`. `offset` starts the loop that many ms in (−7000 in the site's CSS is `offset: 7000` of a 20000 ms loop). This expresses `pcloud`'s 16 s, 20 s and 13 s drifts and `cbob`'s 1 s bob exactly.
- **Actions:** the element holds the last key until `duration`, then returns to rest. Action names follow the engine's name rule (lowercase words joined by hyphens) and are unique per element.
- **Validation:**
  - keys in order;
  - `move` as two integers;
  - `duration` 1–600000 ms;
  - at most 16 actions per element;
  - an item can't have both scene motion and an ambient loop. Their transforms would fight, so validation names the item.
- **Engine time:** `sceneAt(time)` places ambient loops at `time` (with no actions playing), so renders, previews and the app's timeline see them. Renders take an optional `play: { action, at }`.

### D7. The SVG's `<style>` (R11–R15)

- **The root:** the root `<svg>` gains `data-pixellab="<scene>"` and `shape-rendering="crispEdges"`. It never gains a `class` attribute, because the site adds its own with a string replace (`Range.astro`).
- **Scoping:** every generated name is `<scene>--…`, and every rule's selector starts with `[data-pixellab="<scene>"]`:
  - scene motion: `mv-<id>` becomes `<scene>--mv-<id>`;
  - frame loops: `<scene>--wf`;
  - ambient loops: `<scene>--am-<n>`;
  - actions: `<scene>--<action>`.
- **Frame loops:** the elements keep their `wf w0`… classes. Rules like these are written once per loop the scene uses:

  ```css
  [data-pixellab="hero-island"] .wf{opacity:0;animation:hero-island--wf 1.0s step-end infinite}
  [data-pixellab="hero-island"] .wf.w1{animation-delay:-0.800s}
  ```

  They copy `pixel-art.css`'s values today: the cycles from `LOOP_CYCLES`, the frame counts from `SITE_LOOPS`. Any other loop uses the editor's 200 ms per frame.
- **Ambient loops:** an element with one gets a generated class and a keyframes rule built like scene motion's `transform:translate` steps. `offset` becomes a negative `animation-delay`.
- **Actions:** each element that has an action gets a generated class. The rule `[data-pixellab="s"] [data-play~="hop"] .s--a3,[data-pixellab="s"][data-play~="hop"] .s--a3{animation:s--hop 480ms step-end 1}` plays it. This overrides the ambient animation on the same element by specificity, the way `.cbob.is-hopping` overrides `.cbob` today. `animationend` fires with `animationName` `s--hop`. That `<scene>--<action>` is the documented name (R13, R19).
- **Reduced motion:** one rule turns everything off: `@media (prefers-reduced-motion:reduce){[data-pixellab="s"] [class*="s--"]{animation:none}}`, plus frame 0 shown for frame loops, as `pixel-art.css` does today.
- **Size:** the rules are short, and a scene without animation gets no `<style>` at all. Budgets apply to the output with its style, as they do today.

### D8. Commands (R4, R8, R18, R24)

| Command | Notes |
|---|---|
| `document migrate [--to <path>]` | R8, D3. Left out of MCP. |
| `scene ambient <scene> <item> --duration <ms> [--offset <ms>] --keys <json>` / `--clear` | Sets or clears an item's ambient loop. A tool over MCP, and saving it is a proposal. |
| `scene action <scene> <item> <name> --duration <ms> --keys <json>` / `--delete` | Adds, changes or deletes an action. A tool. |
| `object ambient <object> <layer> …` / `object action <object> <layer> <name> …` | The same, for an object's map layer. Tools. |
| `scene render`, `object render` | Gain `--play <action>@<ms>`, and render ambient loops at `--time`. |
| `art build` | Result `outputs[]` gains `kind`, `warnings`. `ok` is false only for problems (R4). Human output: `hero-island.svg: 780.1 KB / 98.1 KB gzip → … lossless — over its 750 KB budget (warning)`. |
| `art sizes`, `check` | Over budget is a warning line, not an error. |

Undo, redo, proposals, ghosts and diffs treat the new fields like any other edit. `docs/cli.md` is regenerated by `pixellab help --markdown` through `npm run docs:cli`.

### D9. Proving the art moves the same (R16)

`scripts/visual-parity.mjs` (Node, no new dependencies) uses the pre-installed Chromium in headless mode. For each scene of a temp copy of the owner's document, it compares two pages:
- **old:** the format-1 build, inlined with the site's `pixel-art.css`;
- **new:** the format-2 build, with the site's change applied (classes swapped for ambient loops and actions), inlined bare.

Each page pauses every animation at a time `t` through `document.getAnimations()` and takes a screenshot at 1×. The pages must match pixel for pixel at:
- `t = 0`;
- 24 times across each loop's cycle;
- every 40 ms through the hop, started at the same instant on both pages;
- `--force-prefers-reduced-motion`.

The check writes a report and the mismatched frames to a scratch folder. It runs in the cloud container and on the Mac.

### D10. The app (R21–R23, R25, R26)

- **Locator:** `findNode` and `runCli(node:cli:)` go. `Locator.findProgram()` returns `Contents/MacOS/pixellab` in the bundle, with an override through `PIXELLAB_PROGRAM` for tests and a development fallback of `target/release/pixellab` in the checkout. `ServerLaunch`, the script hook's `cli` action and `ClaudeTurn.mcpConfig` take the program's URL. `documentProblem` checks for `palette.json`, and gives R9's problem, with Migrate…, for a `palette.mjs`.
- **The setup screen and Settings:**
  - The Node.js row, Choose…, "Find Node Automatically" and `SettingsKey.node` go.
  - The setup copy becomes "Pixel Lab edits pixel art in a .pixellab document."
  - The startup phase "Finding Node.js…" goes.
  - **Migrate…** opens an alert: "Migrate <name> to the new format? Its sources become JSON files. The art doesn't change. You can undo this with git." It has Migrate and Cancel buttons.

  Colors come from the board 03 tokens, and focus, VoiceOver labels and hit targets follow the existing setup screen.
- **The chat's PATH:** today the child's `PATH` puts `node`'s folder first, because an npm-installed `claude` is a Node script. Now the app uses the login shell's `PATH`, resolved once with `zsh -lc 'echo $PATH'` (5 s limit, as today's lookups), after `claude`'s folder. This keeps an npm-installed `claude` working without Pixel Lab looking for Node.
- **Ambient loops on the stage:** the timeline's length becomes the longest of scene motion and the ambient loops, capped at 60 s for scrubbing. Playing renders through the server's `time`, as scene motion does now.
- **Actions in the Inspector:** a section, ACTIONS (Silkscreen label), with one row per action: its name, its duration, and a Play button (`play.fill`, labelled "Play hop"). Play renders the action's frames from the server and shows them once. Under Reduce Motion it shows the rest frame and announces "hop: shown at rest".
- **Text:** `.json` in `Protocol.swift`'s `file`, `MainWindow`'s problem trimming, `AppModel`'s watch handling and the chat's notes.
- **The script hook** gains `play <action> [<item>]` and `expect ambient|actions <v>`. Its `cli` action runs the program.

### D11. Proposals, clean slate (R25)

A proposal file gains `"format": 2`. On start, `state.rs` moves any proposal file without it to `retired-proposals/<id>.json`, and logs one activity line: "retired 3 proposals from before the format change". `proposal-numbers.log` is untouched, so numbers keep counting. A proposal's `source` holds the format-2 JSON text.

### D12. Tests (R1, R3, R16, R20, R28)

- **Ported suites:** every `cli/test/*.test.mjs` suite becomes a Rust test file in the crate that owns the code, with its test names kept (`R7: the site's art compiles byte-identical` stays readable in `cargo test` output). `helpers.mjs`'s `makeDocument()` and `makeSite()` become `tests/common` helpers that copy into a temp dir, and never the real checkout.
- **The site's art suites:** these land in `crates/engine/tests/site_art/`, one file per suite in R20. They find the site checkout as the art suites do now (`~/Projects/jlawcordova.github.io`, or `PIXELLAB_SITE_CHECKOUT`), and skip when it's absent.
- **The servers:** `servers.test.mjs`'s JSON-RPC and MCP transcripts are replayed against the program, unchanged apart from `.json` file names.
- **The app:** `scripts/test-app.sh` and the acceptance pass run against the bundled program. Changed scripts:
  - `document.txt` and `scene-object.txt`, for `.json` names;
  - new `migrate.txt`: opening a format-1 temp document, Migrate…, the expects;
  - new `actions.txt`: `play hop`, and the expects on a temp copy of the owner's document.
- **The PR workflow (R28):** `ubuntu-latest`. It installs the pinned toolchain, runs `cargo test`, runs `npm test` (repo tooling and the parity check), and runs the site-art suites with `PIXELLAB_SITE_CHECKOUT` set to a shallow clone of the public site repo.

### D13. What the site's change does (external, `2026-10-…` in `jlawcordova.github.io`)

For the site's own intent, which depends on this one:
1. Migrate its document with `pixellab document migrate`.
2. Replace the `pcloud`/`pc0–pc2` classes with ambient loops. Replace `cbob` with an ambient loop plus a `hop` action on the character's layer. Give the `thumb-*` scenes `image` and the manifest `publish.images`.
3. Remove `pixel-art.css`'s keyframes. `img.pixel-art`'s `image-rendering` and anything for the HTML wrappers (`floaty`) stay.
4. Make the Range's A button set `data-play="hop"` on the current class's group, and clear it on `animationend`.
5. Drop the `pixellab` devDependency, the art tests, `range-classes` and the thumbnails script. Gitignore the compiled SVGs and thumbnails.
6. In CI, download the pinned `pixellab-linux-x64` release, check it against `SHA256SUMS`, and run `pixellab art build` before `astro build`. Locally, publishing from the app fills the same folders.
7. Update `pixel-first-look.test.mjs`'s dependency check, and the site's skills and `pixel-artist` agent, for JSON and the CLI.

## Areas of concern

1. **The entrances are visitor-triggered too, and this spec leaves them on the site.** The hero island and the Range "build themselves" (`Hero.astro`, `Range.astro`): the site's JavaScript and the Motion library animate the document's 274 `.enter` groups with eased, staggered timing worked out from `data-enter`, `data-row` and `data-x`. That's the same kind of animation as the hop, but it's smooth rather than pixel-stepped, and its schedule is tuned to the site's layout. Moving it would need actions with easing and stagger, which is a much bigger feature. **Decision:** keep the entrances on the site, through the documented group attributes (R19), and make "entrances as actions" a follow-up? This spec assumes yes.
2. **"New features" vs self-contained animation.** The intent leaves new engine features out of scope, except what self-contained animation needs. Ambient loops and actions (R12, R13) are that exception, and the app only shows and plays them (R23). Editing them is by command. **Decision:** is a command-only editor acceptable for now, with the editing UI as a follow-up?
3. **Crates (R5).** This spec asks for `serde`, `serde_json` (with `indexmap`) and `miniz_oxide`, and nothing else (D2). The trade-off: polling instead of a watcher crate, and hand-written SHA-256, CRC-32 and PNG code. **Decision:** approve these three?
4. **Gzip numbers move slightly.** With `miniz_oxide`, reported gzip sizes can differ from today's by a few bytes (D2). Budgets only warn now, so nothing breaks. Linking the system zlib through `flate2` would match exactly, but adds a C dependency. **Decision:** accept the small drift?
5. **The site's art suites in this repo's CI.** Running them against the site's `main` (R20, R28) means a change to the site's art can turn this repo's PR checks red, with nothing in the site's own CI catching it first. The site's CI still runs `pixellab art build`, so validation and depth order surface there as problems or warnings. **Decision:** accept that, or run the art suites only on a schedule here?
6. **Generated names change (R15).** Scoping renames scene motion's `mv-…` classes in the hero island's SVG. Nothing in the site references them (checked), but any future page code must use `data-play` and the contract, not generated names.
7. **Outputs not committed.** `astro dev` needs the SVGs to exist. After the site's change, a fresh site clone shows broken imports until someone runs `pixellab art build`, or publishes from the app. **Decision:** should the site's dev script run the pinned program first? This is the site's call, and that change must make it.
8. **External dependency and order.** The site's change (D13) depends on this one's release, which includes R17's binaries. Until then, the site keeps its pin (`v0.5.0`) and keeps working on the Node package. This repo must not break `v0.5.0`'s consumers. They pin a tag, so a later release can't break them.
9. **A large port.** About 12,000 lines of Node source and 9,000 lines of tests are ported. The parity check (R3) and the ported tests are the safety net. The plan should port engine-first and keep the Node CLI shippable until cutover.

## Open questions

From the intent:
- **The Range hop:** carried by the art as an action, played with `data-play` and ended by `animationend` (R13, D7). Answered by the owner.
- **Comments:** the meaningful ones are kept, as `notes` and `colorNotes` (R7). Answered by the owner.
- **Tests that check the art against the site's pages and styles:** dropped, not kept in the site (R20). Answered by the owner.
- **Over budget:** a warning everywhere, never a failure (R4). Answered by the owner.

New, for the owner, from the Areas of concern:
- **Entrances:** do they stay on the site for now (concern 1)?
- **Ambient loops and actions:** are they edited by command only in this change (concern 2)?
- **Crates:** are the three approved, and is the gzip drift accepted (concerns 3, 4)?
- **Art suites in CI:** on every PR here, or on a schedule (concern 5)?
