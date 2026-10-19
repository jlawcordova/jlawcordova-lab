# Pixel Lab for Mac

A native macOS editor for the pixel art of [jlawcordova.github.io](https://github.com/jlawcordova/jlawcordova.github.io): the hero island and the other isometric scenes, and the library objects they're built from. It replaces the web lab at `/lab/pixel-art/` for day-to-day editing.

It's built CLI first: every action is a command in one registry, reachable three ways, so an agent can do everything a person can.

- **Pixel Lab.app**: a SwiftUI app (board 03, "Field notebook", night and day).
- **`pixellab`**: a zero-dependency Node CLI (`pixellab scene place hero-island pine --at 14,6,1`).
- **`pixellab serve --mcp`**: an MCP server for Claude Code and other agents. Agents' saves become **proposals** you approve in the app.

The site checkout stays the source of truth: the CLI imports the site's own pixel-art engine at runtime, validates and serializes with it, and writes the `.mjs` sources; **Build art** runs the site's `npm run art`.

![Pixel Lab, scene mode, night](docs/screenshots/final-scene-dark.png)

## Requirements

- macOS 15 or later, Apple silicon or Intel.
- Node.js 22 or later (`node` on your shell's PATH, in `/opt/homebrew/bin` or `/usr/local/bin`, or set in Settings).
- The site checkout, by default `~/Projects/jlawcordova.github.io`. Build art runs `npm run art` there, which needs only Node built-ins; run `npm install` in the site if a future build step needs packages.
- To build: the Xcode Command Line Tools (Swift 6). Full Xcode isn't needed.

## Install

### From a release

Each [release](https://github.com/jlawcordova/jlawcordova-lab/releases) has Pixel Lab.app for Apple silicon as a disk image (`PixelLab-darwin-arm64.dmg`) and a zip of the same app (`PixelLab-darwin-arm64.zip`). The repo is private, so download them signed in to GitHub, or with `gh`:

```sh
gh release download --repo jlawcordova/jlawcordova-lab --pattern 'PixelLab-darwin-arm64.dmg' --dir /tmp
```

Open the disk image and drag Pixel Lab to `~/Applications` (create the folder if needed). Then connect Claude Code with the `claude mcp add` command below. The app is ad-hoc signed and the download is quarantined, so the first launch is blocked: allow it under System Settings › Privacy & Security › Open Anyway. To update, quit Pixel Lab and replace the app with the newer one.

On an Intel Mac, install from source.

### From source

```sh
git clone git@github.com:jlawcordova/jlawcordova-lab.git ~/Projects/jlawcordova-lab
cd ~/Projects/jlawcordova-lab
scripts/install.sh
```

`scripts/install.sh` builds `build/Pixel Lab.app` (release build, ad-hoc signed) and copies it to `~/Applications` (it creates the folder if needed; no sudo, never `/Applications`). It ends by printing the `claude mcp add` command and an optional shell alias, both pointing at the CLI inside the installed app:

```sh
claude mcp add pixellab -- node "$HOME/Applications/Pixel Lab.app/Contents/Resources/cli/bin/pixellab.mjs" serve --mcp
alias pixellab='node "$HOME/Applications/Pixel Lab.app/Contents/Resources/cli/bin/pixellab.mjs"'   # in ~/.zshrc
```

Options: `--no-build` installs the existing build, `--dest <dir>` installs somewhere else. Run it again to update; quit Pixel Lab first.

The app is signed ad hoc, not notarized. If macOS won't open it the first time, Control-click it in Finder and choose Open, or allow it in System Settings › Privacy & Security.

## First launch

Pixel Lab opens `~/Projects/jlawcordova.github.io` if it's there and starts its own `pixellab serve --stdio` server against it. If the site or Node.js can't be found, a setup screen says what's missing, with Choose… buttons for the site folder and the node binary and Try Again. Change either later in **Settings** (⌘,), along with the appearance (Auto, Light, Dark).

It reopens the last scene or object you had open, else the hero island.

## A short tour

**Scene mode.** The left sidebar lists every scene and, under it, the **Library** of objects (or the scene's **Items** tree). The stage draws the scene crisply at Fit or 1×–4× on the 32×16 isometric grid; in the night appearance the hero island's drop shadow takes the page color, as the site's dark theme does. Pick an object in the library and click a tile with the Place tool (⌥-click places by pixel, or drag a thumbnail onto the stage). Click an item to select it; drag it, use the arrow keys, or edit Col, Row and Level (X and Y for pixel-placed items) in the inspector. Raise, Lower, Duplicate, Remove and Snap to Tile are in the inspector and the Edit menu. Everything undoes. Scenes with motion get a timeline: Play (⌥⌘P) and scrub.

**Object mode.** Open an object (double-click it in the library, or switch the Scene/Object control with an item selected). Paint with the Pencil, Eraser, Fill and Picker from the palette (the color cap is shown and enforced), switch layers and frames, turn a layer into a frame loop, toggle the onion skin, and watch the preview at 1× and 2× on a tile. File › New Scene… and New Object… make new docs from the engine's starters.

**Checks.** The inspector's CHECKS section validates the open doc with the engine after every edit; a problem that names an item selects it when clicked. Save (⌘S) refuses a doc with problems and explains why, and asks before overwriting a file that changed on disk.

**Build art.** ⌘B or the hammer runs the site's `npm run art` and shows its report, which docs it left out because they're unsaved, and a hint when the cause is a setup problem (npm not found, the site's `node_modules` missing).

**Agent window.** Window › Agent or View › Show Agent (⌥⌘A). It shows the `claude mcp add` command to copy, the live activity feed of every MCP tool call ("listed 15 scenes", "rendered hero-island at 2×", "pine placed at x -48, y 92"), and the agents' **proposals**. Select one to see its edits and a diff against the file as it is now, ghosted on the stage (added and moved items outlined in gold, following the timeline when it plays). Approve (⌘↩) writes it; Reject (⌫) deletes it. The Dock icon and the toolbar's sparkle show how many proposals wait.

![The Agent window](docs/screenshots/final-agent-dark.png)

## Keyboard shortcuts

| Keys | Does |
| --- | --- |
| V / A / H | Select / Place / Pan tool (scene mode) |
| B / E / G / I | Pencil / Eraser / Fill / Picker (object mode) |
| Space (hold) | Pan with any tool |
| 1 – 4, 0 (or ⌘1 – ⌘4, ⌘0) | Zoom 1×–4× (objects 4×–16×), Fit |
| ⌘' | Show or hide the grid (the pixel grid in object mode) |
| Arrows | Move the selection a tile (scene) or the pixel cursor (object; ⇧ moves 4) |
| ⌥ + arrows | Nudge the selection a pixel |
| ⇧↑ / ⇧↓, Page Up / Page Down | Selection up or down a level (scene); previous or next frame (object) |
| Return | Place at the stage cursor (scene); paint at the pixel cursor (object) |
| ⌫ | Remove the selection (scene); erase at the pixel cursor (object) |
| Esc | Select none |
| ] / [ (or ⌘] / ⌘[) | Raise / lower the selection (scene); next / previous layer (object) |
| . / , | Next / previous frame (object) |
| ⌘D | Duplicate |
| ⌘Z / ⇧⌘Z | Undo / Redo |
| ⌘S | Save |
| ⌘B | Build art |
| ⌥⌘P | Play / pause the timeline |
| ⌘N / ⇧⌘N | New Scene / New Object |
| ⌥⌘I | Show or hide the inspector |
| ⌥⌘A | Show the Agent window |
| ↑ / ↓, ⌘↩, ⌫, Esc | Agent window: previous / next proposal, Approve, Reject, hide the ghost |

## The CLI

`pixellab <group> <verb> [args] [--json] [--site <path>]`. Human output by default, JSON with `--json`; `pixellab help` and `pixellab <group> --help` list everything. The full reference is [docs/cli.md](docs/cli.md).

```sh
pixellab site info                                   # the checkout in use, counts, problems
pixellab scene list
pixellab scene render hero-island --scale 2 --out /tmp/hero.png
pixellab scene place hero-island pine --at 14,6,1 --dry-run   # the diff it would write
pixellab object paint house 20,30 21,30 --color water-1
pixellab check hero-island
pixellab proposal list && pixellab proposal apply <id>
```

Edits from the command line are one-shot: load, edit, validate, serialize with the engine, write atomically. They refuse docs with problems or files changed on disk unless `--force`. Inside the repo use `node cli/bin/pixellab.mjs`, or the alias from Install.

## MCP for Claude Code

```sh
claude mcp add pixellab -- node "$HOME/Applications/Pixel Lab.app/Contents/Resources/cli/bin/pixellab.mjs" serve --mcp
```

(The Agent window's Copy button gives the same command for the running app. Add `--site <path>` after `--mcp` for another checkout.)

Tools are the commands with `.` → `_` (`scene_place`, `object_paint`, `scene_render` returns the PNG so the agent can see it, …). **How proposals work:**

1. The agent's edits stay in its MCP server's memory, with undo.
2. `doc_save` validates the doc and writes a proposal to `~/Library/Application Support/Pixel Lab/proposals/<id>.json` (the full new source, the edits, the hash of the file it started from, the agent's name). Nothing in the site changes.
3. Pixel Lab shows it in the Agent window, ghosted on the stage. **Approve** checks the file still matches the hash (if it changed, it asks: apply anyway or reject), validates, and writes the source; **Reject** deletes the proposal. From a terminal: `pixellab proposal show <id>`, `pixellab proposal apply <id>`, `pixellab proposal reject <id>`.
4. After approving, Build art (⌘B) regenerates the site's SVGs. Agents can't approve, reject or build; `pixellab serve --mcp --direct` lets an agent write the site directly (off by default).

Every tool call is logged to `activity.jsonl` in the same folder and shown in the Agent window. `PIXELLAB_STATE` moves the state folder.

## Troubleshooting

- **"Pixel Lab needs Node.js 22…" / node not found.** The app looks in Settings, then your login shell's `command -v node`, then `/opt/homebrew/bin` and `/usr/local/bin`. Install Node 22+ (e.g. `brew install node`), or set its path in Settings › Node.js and Try Again.
- **Site not found.** The setup screen names the folder it tried and what's missing (`src/lib/pixel-art/engine.mjs`, the palette, `scripts/pixel-art-preview.mjs`). Choose the right checkout. The CLI resolves the site from `--site`, `PIXELLAB_SITE`, `~/.config/pixellab/config.json` (`{"site": "…"}`), then `~/Projects/jlawcordova.github.io`.
- **Build art fails.** The sheet shows npm's output and, for known causes, what to do: npm not next to node or on the PATH, or no `node_modules` in the site (`npm install` there). Unsaved docs aren't in a build; save first.
- **"The file changed on disk" when saving.** Someone (git, another editor, an approved proposal) changed the source while you had unsaved edits. Revert loads the file; Save Anyway overwrites it.
- **A proposal's base changed.** The file changed since the agent opened it. Approve asks whether to apply anyway (overwriting those changes) or reject; the diff shows what would change against the file as it is now.
- **The app won't open the first time.** It's ad-hoc signed; Control-click › Open, or allow it in System Settings › Privacy & Security (Open Anyway, which a downloaded release always needs).

## Development

```sh
npm test                                   # CLI tests (node:test), must show "# fail 0"
scripts/test-app.sh                        # app unit tests (Swift Testing, works with the Command Line Tools)
swift build --package-path app             # debug build; swift build -c release for release
scripts/build-app.sh                       # build/Pixel Lab.app (icon, bundled CLI, ad-hoc signature)
scripts/snapshot.sh /tmp/hero.png scene:hero-island dark   # read-only self-snapshot of the main window
scripts/acceptance.sh                      # the end-to-end acceptance pass on a temp copy of the site
npm run docs:cli                           # regenerate docs/cli.md from the command registry
```

**Script hook.** `PIXELLAB_SCRIPT=<file> PIXELLAB_SITE=<temp copy of the site> PIXELLAB_STATE=<temp dir> "build/Pixel Lab.app/Contents/MacOS/PixelLab"` drives the app's model headlessly, one action per line (`open scene:hero-island`, `library pine`, `place 4 -1 1`, `move 1 0`, `save`, `build`, `mode object`, `px 23 20`, `agent open`, `approve`, `snapshot <png>`, `expect …`); it refuses the real checkout, logs each step and exits 0 only if every step passed. The action list is at the top of `app/Sources/PixelLab/Support/ScriptRunner.swift`; [CLAUDE.md](CLAUDE.md) says how to make the temp site.

**Releases.** Tags `v<version>` publish a GitHub Release ([`.github/workflows/release.yml`](.github/workflows/release.yml)). The version is `VERSION` in `cli/lib/server-stdio.mjs`, which is also the app's `CFBundleShortVersionString` and the MCP server's version.

1. Bump `VERSION`, run `scripts/build-app.sh` and `scripts/acceptance.sh` (CI doesn't run the acceptance pass: it pins today's art), and merge to `main`.
2. Tag the merge on `main` and push the tag:

   ```sh
   git fetch origin && git tag -a v<version> -m "Pixel Lab <version>" origin/main && git push origin v<version>
   ```

The **Release Pixel Lab** workflow checks that the tag matches `VERSION` and that the commit is on `main`, runs the CLI tests (against a fresh clone of the site) and the app's unit tests, builds the app on Apple silicon, packages it, installs it from the disk image and smoke-tests it (signature, the bundled MCP server's version, the app opening `scene:hero-island` through the script hook), then publishes the release with generated notes. If it fails, delete the tag (`git push origin :refs/tags/v<version>`), fix it, and tag again.

Layout: `cli/` (Node ≥ 22, ESM, Node built-ins only), `app/` (SwiftPM, SwiftUI + AppKit, macOS 15+), `scripts/`, `docs/intents/` (each change's intent, spec and plan; `2026-10-mac-app/plan.md` is the architecture and design record), `docs/cli.md` (the command reference). No dependencies beyond Node and Swift.
