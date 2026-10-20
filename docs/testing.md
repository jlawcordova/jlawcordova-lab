# Testing

What proves a change works, from the fastest check to the slowest. Run the ones that match what you changed, and paste their output in the PR ([Contributing](contributing.md#before-you-open-a-pr) has the checklist). A check you couldn't run is "not verified", never passed.

| You changed | Run |
| --- | --- |
| anything in `cli/` | `npm test` (must show `# fail 0`) |
| a command | `npm run docs:cli`, then `npm test`; `git status` shows only your edits |
| anything in `app/` | `swift build --package-path app`, `scripts/test-app.sh` |
| `app/`, `cli/` or `scripts/acceptance/` | `scripts/build-app.sh`, then `scripts/acceptance.sh` |
| something you can see | a script-hook run with `expect` lines, and snapshots in dark and light that you look at |
| the docs | `npm run docs:site` (fails on a broken link) |

## The CLI suites

```sh
npm test                                        # node --test "cli/test/**/*.test.mjs"
node --test cli/test/scene-ops.test.mjs         # one file
node --test --test-name-pattern "R3" cli/test/  # by name
```

The suites use `node:test` and `node:assert/strict`, nothing else. The core suites run on the **sample document** in `cli/test/fixtures/sample.pixellab/` (a palette, two objects, two scenes), copied into a temp folder by `makeDocument()` in `cli/test/helpers.mjs`, so they need no site checkout. The suites that pin the owner's art **never touch the real site checkout**: `makeSite()` copies its art (the `*.pixellab`, or the older `source/` folder, laid out as a legacy checkout), `variables.css` and `package.json` into a temp folder, with an empty `HOME` next to it so no real config file is read, and every such test points `--site` at the copy. The copy is made from `~/Projects/jlawcordova.github.io`, or from `PIXELLAB_TEST_SITE` (CI clones the site and sets it). Without a site to copy, those suites are skipped with a message, not failed: `PIXELLAB_TEST_SITE=/nonexistent npm test` shows what runs on the fixture alone.

Other helpers: `runCli()` and `spawnCli()` run the real binary, `startServer()` starts `serve --stdio` or `--mcp` and speaks JSON-RPC to it, `decodePng()` reads a render's pixels, `readDoc()` imports a written source.

| Suite | Covers |
| --- | --- |
| `params.test.mjs` | `checkParams`, the command-line parser, registry definitions |
| `help.test.mjs` | help text, exit codes, `docs/cli.md` being current |
| `document.test.mjs` | finding the document (flags, env, config, a site checkout), the manifest, `document new` and `copy`, loading sources |
| `engine.test.mjs` | the engine itself (ported from the site): parsing, validation messages, rendering, the serializer, edits, previews |
| `compile.test.mjs` | the optimizer and lossless check (ported), `compileScene` and `compileAll`, the site's art compiling byte-identical |
| `art-commands.test.mjs` | `art preview` and `art sizes` |
| `package.test.mjs` | the package's exports, files and version |
| `commands.test.mjs` | the read commands |
| `render.test.mjs` | PNGs, item bounds, poses, the night theme |
| `scene-ops.test.mjs` | the scene edits ported from the lab |
| `edit-commands.test.mjs` | one-shot edits, dry runs, conflicts, sessions and undo, `art build` (publishing) |
| `servers.test.mjs` | `serve --stdio`, `serve --mcp` (proposals, activity), `--direct`, `doc.stale`, `open/<pid>.json`, settling over MCP |
| `sync.test.mjs` | settling proposals, `doc.reload`, edits since disk, the app's open docs, `doc.merge` and the merge in `proposal.apply` |
| `merge.test.mjs` | the three-way line merge |
| `groups.test.mjs` | group attributes following moves, `scene.group.set`, placing into groups |
| `layers.test.mjs` | scene layers: the format and its checks, the layer commands, placing into a layer, renders that hide or focus, the art staying byte-identical, merging |
| `examples.test.mjs` | the documents in `examples/`: they match `scripts/make-examples.mjs`, check clean, nest and move as described, and the harbor builds the same SVG with and without layers |
| `motion-commands.test.mjs` | tracks and keys, `check`'s off-grid warnings, `scene.motion.path` |
| `map-set.test.mjs` | `object.map.set` |
| `activity-and-proposals.test.mjs` | activity summaries, proposal order |
| `docs-site.test.mjs` | the docs site: Markdown rendering, anchors, links, the full build |

Name new tests after the requirement they prove, from the change's `spec.md`: `test('R3: …')`. Never skip or delete a failing test; fix the code.

## The app's unit tests

```sh
scripts/test-app.sh
```

Swift Testing (`import Testing`, `@Test`, `#expect`) in `app/Tests/PixelLabTests/`, for the pure logic: stage geometry and hit-testing, keys, strokes and pixel colors, the proposal ghost's matching and diffs, the scene layers' tree and picking rules. They don't start a server. Anything that needs one is checked with the script hook.

## The script hook

The hook drives the real app's model from a script, with no UI automation and no screen-recording permission:

```sh
PIXELLAB_SCRIPT=script.txt PIXELLAB_DOC=/tmp/copy.pixellab PIXELLAB_STATE=/tmp/state \
  "build/Pixel Lab.app/Contents/MacOS/PixelLab"
```

It refuses to run without `PIXELLAB_DOC` (or `PIXELLAB_SITE`, a temp copy of the site), or against the real checkout or anything inside it (scripts save and publish), logs each step to stderr as `script: …`, and exits 0 only if every step and `expect` passed (1 if any failed, 2 if it couldn't start).

**Make the temp copy.** A document is a folder, so copy it (`pixellab document copy --to /tmp/copy.pixellab` does it from the CLI, and gives a legacy `source/` folder a manifest). The acceptance pass copies the whole site instead, so `publish` lands where the site's SVGs are:

```sh
site=$(mktemp -d)/site && mkdir -p "$site"
real=~/Projects/jlawcordova.github.io
cp -R "$real/src" "$real/scripts" "$real/package.json" "$real/package-lock.json" "$site/"
```

**Write a script,** one action per line, `#` for comments:

```
open scene:hero-island
appearance dark
library pine
place 3 -1 1
expect at 3,-1,1
key opt+up
save
expect dirty false
snapshot /tmp/shots/placed-dark.png
appearance light
snapshot /tmp/shots/placed-light.png
```

There are actions for scene mode (`select`, `click`, `hover`, `place`, `drag`, `key`, `move`, `nudge`, `set`, `raise`, `undo`, `save`, `build`, `time`, …), object mode (`mode object`, `color`, `stroke`, `mousepath`, `erase`, `px`, `fill`, `pick`, `layer`, `frame`, `animate`, `onion`, `new object`, …), the Agent window (`agent open`, `proposal select`, `approve`, `reject`, `answer`, `ghost`, `agentkey`, …), scene layers (`select layer:<id>`, `click <x> <y> shift`, `dblclick`, `rightclick <x> <y>` and `rightclick row <target>` (a real right-click through the window), `menu group`, `group`, `ungroup`, `inside`, `hide`, `lock`, `rename`, `movedby`, `layernudge`, `placeinto`, `layerskey`, …) and files changing on disk (`cli <args…>` runs the bundled CLI on the script's document, as an outside editor; `stale merge|reload|keep` presses the changed-on-disk bar's buttons), and `expect`s for each (`expect pixel 7,7 roof-1`, `expect scenepixel x,y #RRGGBB`, `expect proposals 2`, `expect stale true`, `expect selection layer:group-1`, `expect inside group-2`, `expect menu Group,Ungroup`, …). The full list is the comment at the top of `app/Sources/PixelLab/Support/ScriptRunner.swift`; add to it when a feature needs a new action.

## Snapshots

Snapshot every view you changed, in dark and light, and look at them before reporting:

- in a script: `appearance dark|light`, then `snapshot <png>` (or `snapshot agent <png>` for the Agent window);
- or read-only, against any checkout: `scripts/snapshot.sh <out.png> scene:hero-island dark [zoom]`.

Snapshots draw the window with `cacheDisplay`, so pixel art stays nearest-neighbor in them too. Screenshots that go in a PR live in `docs/screenshots/`.

## The acceptance pass

```sh
scripts/build-app.sh && scripts/acceptance.sh
```

End to end on a temp copy of the site: scene edits, save and publish (six SVGs into the temp site); object painting and save; an agent saving two proposals over MCP through the bundled CLI; the Agent window ghosting, approving and rejecting them; a doc with unsaved edits changed on disk and merged, reloaded and kept, and another proposal approved into an unsaved copy (`stale.txt`); Save As to a copy, reopening it and publishing from it into a folder; and scene layers (`layers.txt`): grouping, renaming, picking, going inside, the layer menu on the stage and on a row, hiding, locking, dragging and nudging, placing into a layer and ungrouping, with the runner checking that the hero island's SVG is byte-identical before and after its layers were saved and that the saved scene holds no hidden or locked state; then the example documents (`examples-harbor.txt`, `examples-town.txt`, each on a temp copy of an `examples/` document), with layers read from a file three and four levels deep. Run it whenever `app/`, `cli/` or `scripts/acceptance/` changed. Its `expect` lines pin the art as it was on 2026-10-20: after the site's art changes, update the scripts in `scripts/acceptance/`.

## CI

- **Docs site** (`.github/workflows/docs.yml`) runs on every pull request and push to `main`: it checks `docs/cli.md` is current, runs the docs-site tests and builds the site, then deploys `main` to GitHub Pages.
- **Release Pixel Lab** (`.github/workflows/release.yml`) runs on a `v<version>` tag: the CLI tests against a fresh clone of the site, the app's unit tests, the app build, packaging and a smoke test of the installed app. It doesn't run the acceptance pass (that pins the art), so run that before tagging.

There is no CI run of the full test suite on pull requests yet: the PR's pasted output is the evidence.
