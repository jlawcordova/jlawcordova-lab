# CLAUDE.md

Pixel Lab for Mac: a `pixellab` CLI and MCP server (Node, zero dependencies) and a SwiftUI Mac app, for the pixel art in the site repo `~/Projects/jlawcordova.github.io`. Read `docs/plan.md` first: it is the architecture, the look (mood board 03, "Field notebook") and the build workflow.

## Commands

- Test: `npm test` (node:test; must show `# fail 0`)
- CLI: `node cli/bin/pixellab.mjs help`
- App: `swift build --package-path app`, then `scripts/build-app.sh` → `build/Pixel Lab.app`
- App snapshot (no screen recording needed; read-only): `scripts/snapshot.sh <out.png> [scene:hero-island] [dark|light] [zoom]`
- App unit tests (Swift Testing, works with the Command Line Tools): `scripts/test-app.sh`
- App test hook (drives the model headlessly, no UI automation): `PIXELLAB_SCRIPT=<file> PIXELLAB_SITE=<temp site copy> PIXELLAB_STATE=<temp dir> "build/Pixel Lab.app/Contents/MacOS/PixelLab"`. One action per line (`open scene:hero-island`, `select 145.0`, `click <x> <y>`, `library pine`, `hover 3 -1 1`, `place 3 -1 1`, `key opt+up`, `drag 1 0 hold`, `mouse down|drag|up <x> <y> [opt]`, `save`, `build`, `time 9400`, `snapshot <png>`, `expect at 4,-1,1`; object mode: `mode object`, `color ink`, `stroke 3,4 9,9`, `mousepath 3,4 6,6 9,9`, `erase 5,5`, `px 5 5 opt|shift`, `fill 7 23`, `pick 30 5`, `layer add|up|hide 0`, `frame next|add`, `animate wf`, `onion on`, `new object rock 12x8`, `new scene meadow`, `expect pixel 7,7 roof-1`; Agent window: `agent open`, `proposal select 2`, `proposal open`, `approve [force]`, `reject`, `answer cancel|force|reject|proceed`, `ghost on|off`, `agentkey down|esc|cmd+return|delete`, `snapshot agent <png>`, `menus`, `expect proposals|ghost|ghostchanges|ghosttime|base|question|badge <v>`; also `expect scenes|objects|zoom|theme|hint <v>` and `expect scenepixel x,y #RRGGBB`, …; the full list is in `app/Sources/PixelLab/Support/ScriptRunner.swift`). It refuses to run without `PIXELLAB_SITE` or against the real checkout, logs each step to stderr, and exits 0 only if every step and `expect` passed. Make the temp site by copying the site's `src/`, `scripts/`, `package.json`, `package-lock.json` and symlinking its `node_modules` (so `build` can run `npm run art`).
- Acceptance pass (end to end on a temp site copy: scene edit → save → build, object paint → save, MCP proposal → approve/reject): `scripts/acceptance.sh [out-dir]`; its scripts are in `scripts/acceptance/`.
- Install for the owner: `scripts/install.sh` (builds, copies to `~/Applications`, prints the `claude mcp add` command; `--dest <dir>` for a test target, `--no-build`). Never install into `/Applications` and never run `claude mcp add` for the owner.
- App icon: `node scripts/make-icon.mjs` rewrites `app/Sources/PixelLab/Resources/AppIcon.png` (build-app.sh makes the .icns)
- The app builds with the Command Line Tools, which lack the SwiftUI macro plugin: don't use `@State` (keep view state in `AppModel`); `@Observable`, `@Environment`, `@AppStorage`, `@FocusState` are fine.

## Rules

- The site checkout is the source of truth for art and engine. Import the engine from it at runtime; never copy or port it. Never write to the real site checkout from tests: use a temp copy.
- No new npm or Swift dependencies without asking.
- Work on a `slice/<name>` branch off `feat/mac-app`, PR into `feat/mac-app`, merge when checks pass. Never merge into `main`.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. PR bodies end with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- Never use `xcode-select -s` or change system settings. Use `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer` per command if full Xcode is needed.
