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

The suites use `node:test` and `node:assert/strict`, nothing else. They **never touch the real site checkout**: `makeSite()` in `cli/test/helpers.mjs` copies the parts the CLI needs (the engine, the preview script, `variables.css`, the art sources, `package.json`) into a temp folder, with an empty `HOME` next to it so no real config file is read, and every test points `--site` at the copy. The copy is made from `~/Projects/jlawcordova.github.io`, or from `PIXELLAB_TEST_SITE` (CI clones the site and sets it). Without a site to copy, the suites that need one are skipped with a message, not failed.

Other helpers: `runCli()` and `spawnCli()` run the real binary, `startServer()` starts `serve --stdio` or `--mcp` and speaks JSON-RPC to it, `decodePng()` reads a render's pixels, `readDoc()` imports a written source.

| Suite | Covers |
| --- | --- |
| `params.test.mjs` | `checkParams`, the command-line parser, registry definitions |
| `help.test.mjs` | help text, exit codes, `docs/cli.md` being current |
| `site.test.mjs` | finding the site, loading sources |
| `commands.test.mjs` | the read commands |
| `render.test.mjs` | PNGs, item bounds, poses, the night theme |
| `scene-ops.test.mjs` | the scene edits ported from the lab |
| `edit-commands.test.mjs` | one-shot edits, dry runs, conflicts, sessions and undo, `art.build` |
| `servers.test.mjs` | `serve --stdio`, `serve --mcp` (proposals, activity), `--direct` |
| `activity-and-proposals.test.mjs` | activity summaries, proposal order |
| `docs-site.test.mjs` | the docs site: Markdown rendering, anchors, links, the full build |

Name new tests after the requirement they prove, from the change's `spec.md`: `test('R3: …')`. Never skip or delete a failing test; fix the code.

## The app's unit tests

```sh
scripts/test-app.sh
```

Swift Testing (`import Testing`, `@Test`, `#expect`) in `app/Tests/PixelLabTests/`, for the pure logic: stage geometry and hit-testing, keys, strokes and pixel colors, the proposal ghost's matching and diffs. They don't start a server. Anything that needs one is checked with the script hook.

## The script hook

The hook drives the real app's model from a script, with no UI automation and no screen-recording permission:

```sh
PIXELLAB_SCRIPT=script.txt PIXELLAB_SITE=/tmp/site PIXELLAB_STATE=/tmp/state \
  "build/Pixel Lab.app/Contents/MacOS/PixelLab"
```

It refuses to run without `PIXELLAB_SITE` or against the real checkout (scripts save and build), logs each step to stderr as `script: …`, and exits 0 only if every step and `expect` passed (1 if any failed, 2 if it couldn't start).

**Make the temp site** by copying the site's `src/`, `scripts/`, `package.json` and `package-lock.json`, and symlinking its `node_modules`, so `build` can run `npm run art`:

```sh
site=$(mktemp -d)/site && mkdir -p "$site"
real=~/Projects/jlawcordova.github.io
cp -R "$real/src" "$real/scripts" "$real/package.json" "$real/package-lock.json" "$site/"
ln -s "$real/node_modules" "$site/node_modules"
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

There are actions for scene mode (`select`, `click`, `hover`, `place`, `drag`, `key`, `move`, `nudge`, `set`, `raise`, `undo`, `save`, `build`, `time`, …), object mode (`mode object`, `color`, `stroke`, `mousepath`, `erase`, `px`, `fill`, `pick`, `layer`, `frame`, `animate`, `onion`, `new object`, …) and the Agent window (`agent open`, `proposal select`, `approve`, `reject`, `answer`, `ghost`, `agentkey`, …), and `expect`s for each (`expect pixel 7,7 roof-1`, `expect scenepixel x,y #RRGGBB`, `expect proposals 2`, …). The full list is the comment at the top of `app/Sources/PixelLab/Support/ScriptRunner.swift`; add to it when a feature needs a new action.

## Snapshots

Snapshot every view you changed, in dark and light, and look at them before reporting:

- in a script: `appearance dark|light`, then `snapshot <png>` (or `snapshot agent <png>` for the Agent window);
- or read-only, against any checkout: `scripts/snapshot.sh <out.png> scene:hero-island dark [zoom]`.

Snapshots draw the window with `cacheDisplay`, so pixel art stays nearest-neighbor in them too. Screenshots that go in a PR live in `docs/screenshots/`.

## The acceptance pass

```sh
scripts/build-app.sh && scripts/acceptance.sh
```

End to end on a temp copy of the site: scene edits, save and build art; object painting and save; an agent saving two proposals over MCP through the bundled CLI; the Agent window ghosting, approving and rejecting them. Run it whenever `app/`, `cli/` or `scripts/acceptance/` changed. Its `expect` lines pin the art as it was on 2026-10-19: after the site's art changes, update the scripts in `scripts/acceptance/`.

## CI

- **Docs site** (`.github/workflows/docs.yml`) runs on every pull request and push to `main`: it checks `docs/cli.md` is current, runs the docs-site tests and builds the site, then deploys `main` to GitHub Pages.
- **Release Pixel Lab** (`.github/workflows/release.yml`) runs on a `v<version>` tag: the CLI tests against a fresh clone of the site, the app's unit tests, the app build, packaging and a smoke test of the installed app. It doesn't run the acceptance pass (that pins the art), so run that before tagging.

There is no CI run of the full test suite on pull requests yet: the PR's pasted output is the evidence.
