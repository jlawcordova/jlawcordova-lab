# Testing

What proves a change works, from the fastest check to the slowest. Run the ones that match what you changed, and paste their output in the PR ([Contributing](contributing.md#before-you-open-a-pr) has the checklist). A check you couldn't run is "not verified", never passed.

| You changed | Run |
| --- | --- |
| anything in `crates/` | `cargo fmt --check`, `cargo test` (every test passes) |
| anything in `crates/`, `scripts/` or `package.json` | `npm test` (must show `# fail 0`) |
| a command | `npm run docs:cli`, then `cargo test`; `git status` shows only your edits |
| anything in `app/` | `swift build --package-path app`, `scripts/test-app.sh` |
| `app/`, `crates/` or `scripts/acceptance/` | `scripts/build-app.sh`, then `scripts/acceptance.sh --only <stages>` for the stages that cover it |
| something you can see | a script-hook run with `expect` lines, and snapshots in dark and light that you look at |
| the docs | `npm run docs:site` (fails on a broken link) |

Rust is installed with rustup, and `rust-toolchain.toml` pins the toolchain, so `cargo` picks it up on its own. If `cargo` isn't on your `PATH`, `export PATH="$HOME/.cargo/bin:$PATH"` first.

## The Rust suites

```sh
cargo fmt --check                      # rustfmt.toml: 140 columns
cargo test                             # every crate's tests/
cargo test -p pixellab-engine          # the engine alone
cargo test -p pixellab --test layers   # one file of the program's tests
cargo test r3_                         # by name (test names start with the requirement, in lower case)
```

The suites are integration tests in `crates/engine/tests/` and `crates/pixellab/tests/`, with the standard test harness and nothing else. They run on the **sample document** in `fixtures/sample.pixellab/` (format 2: a palette, two objects, two scenes), copied into a temp folder by `make_document()` in `crates/pixellab/tests/common/mod.rs`, so they need no site checkout. `make_site()` lays the same copy out as a site checkout (`src/assets/pixel-art/sample.pixellab`, with a `variables.css`), for the tests of `--site`. Both come with an empty `HOME` next to the copy, so no real config file is read. **Tests never touch the owner's site checkout or the documents in `examples/`:** they work on temp copies and point the program at them. `fixtures/format1/sample.pixellab/` is the same document in the old format, the input for the migrate tests. The program's tests run the real executable for the CLI and the two servers (`serve --stdio`, `serve --mcp`) and speak JSON-RPC to it.

| Suite | Covers |
| --- | --- |
| `engine/tests/engine.rs`, `iso.rs`, `svg_starter.rs`, `edit.rs` | the engine itself: parsing, validation messages, the grid and block faces, rendering, the rect SVG, the starters, the serializer, edits |
| `engine/tests/format.rs`, `jsnum.rs`, `migrate.rs` | format 2's canonical text (every fixture file is its own serialization), JavaScript's number printing, the format-1 reader |
| `engine/tests/compile.rs`, `output.rs`, `style.rs` | the optimizer and lossless check, compiling a scene or a document, the output being deterministic and self-contained, the SVG's own `<style>` |
| `engine/tests/depth.rs`, `depth_motion.rs`, `motion_site.rs`, `ambient.rs` | the depth order, motion and the copies it draws, ambient loops and actions |
| `engine/tests/preview.rs` | the PNG previews |
| `pixellab/tests/params.rs`, `help.rs`, `jsonrpc.rs` | `check_params`, the command-line parser, help text and exit codes, the JSON-RPC framing |
| `pixellab/tests/document.rs`, `migrate.rs`, `check_canonical.rs` | finding the document (flags, env, config, a site checkout), the manifest, `document new` and `copy`, `document migrate` and the old format's problem, `check`'s canonical-form warning |
| `pixellab/tests/commands.rs`, `render.rs` | the read commands; PNGs, item bounds, poses, the night theme |
| `pixellab/tests/art_commands.rs`, `art_budget.rs`, `art_images.rs` | `art preview`, `sizes` and `build`, budgets as warnings, a scene's image |
| `pixellab/tests/scene_ops.rs`, `scene_commands.rs`, `object_commands.rs`, `groups.rs` | the scene edits ported from the lab, the scene and object commands, group attributes following moves |
| `pixellab/tests/layers.rs`, `layer_motion_commands.rs`, `motion_ops.rs`, `ambient_commands.rs`, `depth_edit.rs`, `depth_motion.rs` | scene layers, tracks and keys, ambient loops and actions as commands, depth order after every edit, the preview at a time matching the SVG |
| `pixellab/tests/session.rs`, `edit_commands.rs`, `sync.rs`, `merge.rs`, `watch.rs` | sessions, undo and redo, saves and conflicts, an agent's saves, a clean doc following its file, `doc.reload`, `doc.merge`, the three-way line merge, watching by polling |
| `pixellab/tests/autosave.rs`, `history.rs`, `owner.rs` | autosave (the pause, problems, conflicts, undo across saves), authors and the Edit menu's labels, the version history and its commands, one writer per document (the lock, the socket, clients through the app, a killed owner, a second server refused) |
| `pixellab/tests/servers.rs`, `chat.rs`, `state.rs`, `retire.rs` | `serve --stdio` and `serve --mcp` (edits that apply, activity, `history.changed`, `doc.stale`), the Agent chat's CLI side, the state dir and activity summaries, waiting proposals set aside |

## The repo's Node tests

```sh
npm test       # cargo build --release, then node --test "scripts/test/**/*.test.mjs"
```

`npm test` builds the program first, then runs the suites for the repo's own tooling (they use `node:test` and `node:assert/strict`, nothing else):

| Suite | Covers |
| --- | --- |
| `examples.test.mjs` | the documents in `examples/`: they match `scripts/make-examples.mjs`, are format 2, check clean and canonical, nest and move as described, and the harbor builds the same SVG with and without layers |
| `docs-site.test.mjs` | the docs site: Markdown rendering, anchors, links, the full build |
| `package.test.mjs` | `package.json`: the name and version, and that nothing exports an engine or reads the removed test-only switch |

## Adding a test

- **Name it after the requirement it proves,** from the change's `spec.md`: `fn r3_gzip_sizes_are_within_budget()` in Rust, or `test('R3: …')` in `scripts/test/`. The test output should read as a list of requirements. Never skip, `#[ignore]` or delete a failing test; fix the code.
- **Put it in the crate it belongs to.** Something the engine does on data (validate, render, compile, order) is a test in `crates/engine/tests/`, using `tests/common.rs`. Something a command, a door or a session does is in `crates/pixellab/tests/`: `mod common;`, then `common::make_document()` or `make_site()`, and call the registry, the CLI or a server over the temp copy.
- **Temp copies only.** A test that writes uses a copy it made; it never reads or writes `~/Projects/jlawcordova.github.io`, a document in `examples/`, or the real state dir (point `PIXELLAB_STATE` at a temp folder).
- **Check bytes, not looks,** where output is meant to be stable: compare a rendered SVG or canonical text as a string. The tests of the program's output were checked against the old Node CLI when the engine moved to Rust (the parity and visual gates, recorded in PRs [#32](https://github.com/jlawcordova/jlawcordova-lab/pull/32) and [#34](https://github.com/jlawcordova/jlawcordova-lab/pull/34)); those scripts were removed at cutover.

## The app's unit tests

```sh
scripts/test-app.sh
```

Swift Testing (`import Testing`, `@Test`, `#expect`) in `app/Tests/PixelLabTests/`, for the pure logic: stage geometry and hit-testing, keys, strokes and pixel colors, the version ghost's matching and diffs, autosave's save states and the Versions panel's days and words, the scene layers' tree and picking rules. They don't start a server. Anything that needs one is checked with the script hook.

## The script hook

The hook drives the real app's model from a script, with no UI automation and no screen-recording permission:

```sh
PIXELLAB_SCRIPT=script.txt PIXELLAB_DOC=/tmp/copy.pixellab PIXELLAB_STATE=/tmp/state \
  "build/Pixel Lab.app/Contents/MacOS/PixelLab"
```

It refuses to run without `PIXELLAB_DOC` (or `PIXELLAB_SITE`, a temp copy of the site), or against the real checkout or anything inside it (scripts save and publish), logs each step to stderr as `script: …`, and exits 0 only if every step and `expect` passed (1 if any failed, 2 if it couldn't start).

**Make the temp copy.** A document is a folder, so copy it (`pixellab document copy --to /tmp/copy.pixellab` does it from the CLI). The acceptance pass copies the whole site instead, so `publish` lands where the site's SVGs are:

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

There are actions for scene mode (`select`, `click`, `hover`, `place`, `drag`, `key`, `move`, `nudge`, `set`, `undo`, `save`, `build`, `time`, …), object mode (`mode object`, `color`, `stroke`, `mousepath`, `erase`, `px`, `fill`, `pick`, `layer`, `frame`, `animate`, `onion`, `new object`, …), autosave and versions (`flush`, `wait saved`, `versions open`, `version select`, `version undo`, `version restore`, `ghost`, …), one writer (`cli-owned`, `mcpcall`, `secondapp`), the Agent window (`agent open`, …), scene layers (`select layer:<id>`, `click <x> <y> shift`, `dblclick`, `rightclick <x> <y>` and `rightclick row <target>` (a real right-click through the window), `menu group`, `group`, `ungroup`, `inside`, `hide`, `lock`, `rename`, `movedby`, `layernudge`, `placeinto`, `layerskey`, …), depth (`depth front|back|auto` sets the inspector's Depth control, `groupdepth <path>|selected front|back|auto` an SVG group's, `sort` presses the Checks panel's Sort button, `footprint <w>,<d>` sets the open sprite's footprint in the object inspector) the Agent chat (`righttab agent`, `setup install|signin|wait <state>`, `chat send <text>`, `chat wait`, `chat stop`, `chatkey tab|return|shift+return`, `choose <n>`, `link <text>`, `chip pin <target>`, `savenow`, `card approve|reject|ghost`, `altclick <x> <y>`, `selectuses`, `activity chats`, `askclaude`, …, run with the fake Claude Code below) and files changing on disk (`cli <args…>` runs the bundled program on the script's document, as an outside editor; `stale merge|reload|keep` presses the changed-on-disk bar's buttons), and `expect`s for each (`expect pixel 7,7 roof-1`, `expect scenepixel x,y #RRGGBB`, `expect savestate saved`, `expect version newest author agent`, `expect stale true`, `expect depth front`, `expect order pine@0,1,0 walls` (entries in list order: a path, `#id`, `selected`, an object name or `name@col,row,level`), `expect level 4`, `expect footprint 2,2`, `expect warning out_of_order`, `expect nowarning out_of_order`, `expect groupdepth 3 back`, `expect cli moved`, `expect selection layer:group-1`, `expect inside group-2`, `expect menu Group,Ungroup`, `expect setup ready`, `expect steps 5`, `expect chips pine 83,pine 84`, `expect context item 51 object=rock`, `expect cardstatus Undone`, …). The full list is the comment at the top of `app/Sources/PixelLab/Support/ScriptRunner.swift`; add to it when a feature needs a new action.

**The Agent chat's fake Claude Code.** Scripts never run the owner's `claude`: under the hook `PIXELLAB_CLAUDE_HOME` replaces `~` in the search (and skips Homebrew and the login shell), and `PIXELLAB_CLAUDE_INSTALLER` replaces Anthropic's installer. Point them at `scripts/fake-claude/` ([its README](../scripts/fake-claude/README.md)): `HOME=<temp home> scripts/fake-claude/install.sh` installs the fake there, `FAKE_CLAUDE_SIGNED_IN=1` starts it signed in, and its turns (`turns.json`) do their tool calls through the real MCP server, so edits, versions, activity and cards are real. It refuses a turn without the flags that keep the chat to Pixel Lab's tools.

## Snapshots

Snapshot every view you changed, in dark and light, and look at them before reporting:

- in a script: `appearance dark|light`, then `snapshot <png>` (or `snapshot agent <png>` for the Agent window);
- or read-only, against any checkout: `scripts/snapshot.sh <out.png> scene:hero-island dark [zoom]`.

Snapshots draw the window with `cacheDisplay`, so pixel art stays nearest-neighbor in them too. Screenshots that go in a PR live in `docs/screenshots/`.

## The acceptance pass

```sh
scripts/build-app.sh && scripts/acceptance.sh                  # every stage
scripts/acceptance.sh --only layers,examples-harbor            # only these stages
```

End to end on a temp copy of the site, which is migrated to format 2 with the bundled program first (the real checkout is only read): scene edits, save and publish (six SVGs into the temp site); object painting and save; an agent's edits over MCP applied with no app open, through the bundled program, then the Versions panel ghosting the version before them and taking the agent's back (`agent.txt`); a change on disk merged into a pending save, and a clash reloaded and kept (`stale.txt`); autosave and versions on a harbor copy: Save as a version, the panel's ghost, Undo These Changes, Restore, Last Opened and a doc with problems (`versions.txt`); one writer: a second copy refused, the CLI and an MCP client editing through the app, then the CLI writing itself once it quits (`owner.txt`); Save As to a copy, reopening it and publishing from it into a folder (`document.txt`); a document left in format 1, which the app meets with the old-format problem and **Migrate…** (`migrate.txt`); ambient loops and actions played on the stage, on a harbor copy (`actions.txt`); and scene layers (`layers.txt`): grouping, renaming, picking, going inside, the layer menu on the stage and on a row, hiding, locking, dragging and nudging, placing into a layer and ungrouping, with the runner checking that the hero island's SVG is byte-identical before and after its layers were saved and that the saved scene holds no hidden or locked state; then the example documents (`examples-harbor.txt`, `examples-town.txt`, each on a temp copy of an `examples/` document), with layers read from a file three and four levels deep; and the Agent chat against the fake Claude Code: its setup from no `claude` to signed in and chats on a harbor copy (`chat-setup.txt`), a chat's edits on the site applied with a change card, undone from it and shown in Versions, with the Agent window's Chats filter (`chat.txt`), the selection as context on a harbor copy: layer chips, ⌥-click, pins, unsaved items, a question and cards per chat (`chat-harbor.txt`); and a chat across three launches: its Claude session resumed, its version undone with the CLI while the app is closed and its card still knowing it, and a lost session started again (`chat-relaunch-1/2/3.txt`). Stages written before autosave run with a long autosave pause, so their edits wait for the saves they make; `versions` and `owner` run with a short one. Run the stages that cover a change whenever `app/`, `crates/` or `scripts/acceptance/` changed, with `--only <name>[,<name>...]` (a stage file's name without its extension; the shared setup always runs, and a stage that reads an earlier one's output needs it named too: `agent` needs `mcp-agent`, `stale` needs `stale-agent`, `chat-relaunch-2` and `-3` need `chat-relaunch-1`). Name the stages in the PR; the full pass isn't required. The scripts run the program from the app bundle (`Contents/Helpers/pixellab`), so what they check is the program the app ships; the agent and stale scripts (`mcp-agent.mjs`, `stale-agent.mjs`) and the fake Claude Code are Node, as repo tooling. Its `expect` lines pin the art as it was on 2026-10-21: after the site's art changes, update the scripts in `scripts/acceptance/`.

## CI

- **Tests** (`.github/workflows/test.yml`) runs on every pull request and push to `main`, on Linux with no site checkout: `cargo fmt --check`, `cargo test` and `npm test`, on the toolchain `rust-toolchain.toml` pins.
- **Docs site** (`.github/workflows/docs.yml`) runs on every pull request and push to `main`: it builds the program, checks `docs/cli.md` is current, runs the docs-site tests and builds the site, then deploys `main` to GitHub Pages.
- **Release Pixel Lab** (`.github/workflows/release.yml`) runs on a `v<version>` tag: `cargo test --locked`, the repo's Node tests, the app's unit tests, the app build, packaging of the app and of the program for macOS and Linux, and a smoke test of the installed app. It doesn't run the acceptance pass (that pins the art and needs a Mac with the owner's site), so run that before tagging.

The app's build, its unit tests and the acceptance pass don't run on pull requests: the PR's pasted output is the evidence for those.
