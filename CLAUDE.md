# CLAUDE.md

Pixel Lab for Mac: a `pixellab` CLI and MCP server (Node, zero dependencies, also the pixel-art engine and compiler, exported as the `pixellab` package) and a SwiftUI Mac app, for isometric pixel art kept in a `.pixellab` document (a folder: `manifest.json`, `palette.mjs`, `objects/`, `scenes/`). The owner's document lives in the site repo `~/Projects/jlawcordova.github.io`, which publishes it as SVGs. Read `docs/intents/2026-10-mac-app/plan.md` first: it is the architecture and design record, including the look (mood board 03, "Field notebook"); `docs/intents/2026-10-pixellab-document/` is the change that moved the engine here and made the document. `README.md` has the user-facing tour, shortcuts and MCP setup; `docs/cli.md` is the command reference.

## Commands

- Test: `npm test` (node:test; must show `# fail 0`)
- CLI: `node cli/bin/pixellab.mjs help`
- CLI reference: `npm run docs:cli` regenerates `docs/cli.md` from the command registry. Run it after changing a command; never edit `docs/cli.md` by hand.
- Developer docs site: `npm run docs:site` builds `build/docs-site/` from the README files and `docs/` (`scripts/docs-site/`); it fails on a broken link, anchor or image. `.github/workflows/docs.yml` builds it on every PR and deploys `main` to GitHub Pages. Adding a page: `docs/contributing.md`, "The docs site".
- App: `swift build --package-path app`, then `scripts/build-app.sh` → `build/Pixel Lab.app`
- App snapshot (no screen recording needed; read-only): `scripts/snapshot.sh <out.png> [scene:hero-island] [dark|light] [zoom]`
- App unit tests (Swift Testing, works with the Command Line Tools): `scripts/test-app.sh`
- App test hook (drives the model headlessly, no UI automation): `PIXELLAB_SCRIPT=<file> PIXELLAB_DOC=<temp copy of a document> PIXELLAB_STATE=<temp dir> "build/Pixel Lab.app/Contents/MacOS/PixelLab"` (or `PIXELLAB_SITE=<temp site copy>`). One action per line (`open scene:hero-island`, `select 145.0`, `click <x> <y>`, `library pine`, `hover 3 -1 1`, `place 3 -1 1`, `key opt+up`, `drag 1 0 hold`, `mouse down|drag|up <x> <y> [opt]`, `save`, `publish [<dir>]`, `saveas <path.pixellab>`, `time 9400`, `snapshot <png>`, `expect at 4,-1,1`; object mode: `mode object`, `color ink`, `stroke 3,4 9,9`, `mousepath 3,4 6,6 9,9`, `erase 5,5`, `px 5 5 opt|shift`, `fill 7 23`, `pick 30 5`, `layer add|up|hide 0`, `frame next|add`, `animate wf`, `onion on`, `new object rock 12x8`, `new scene meadow`, `expect pixel 7,7 roof-1`; Agent window: `agent open`, `proposal select 2`, `proposal open`, `approve [force]`, `reject`, `answer cancel|force|reject|proceed`, `ghost on|off`, `agentkey down|esc|cmd+return|delete`, `snapshot agent <png>`, `menus`, `expect proposals|ghost|ghostchanges|ghosttime|base|question|badge <v>`; also `expect scenes|objects|zoom|theme|build|outputs|publishdir|document|documentkind <v>` and `expect scenepixel x,y #RRGGBB`, …; the full list is in `app/Sources/PixelLab/Support/ScriptRunner.swift`). It refuses to run without `PIXELLAB_DOC` or `PIXELLAB_SITE`, or against the real checkout or anything inside it, logs each step to stderr, and exits 0 only if every step and `expect` passed. Make the temp copy with `pixellab document copy --to <temp>.pixellab`, or copy the site's `src/`, `scripts/`, `package.json` and `package-lock.json` for a temp site.
- Acceptance pass (end to end on a temp site copy: scene edit → save → publish, object paint → save, MCP proposal → approve/reject, save as → reopen the copy → publish into a folder): `scripts/acceptance.sh [out-dir]`; its scripts are in `scripts/acceptance/`. Its `expect` lines pin the site's art as it was on 2026-10-19; update them when the art changes.
- Install for the owner: `scripts/install.sh` (builds, copies to `~/Applications`, prints the `claude mcp add` command; `--dest <dir>` for a test target, `--no-build`). Never install into `/Applications` and never run `claude mcp add` for the owner.
- Release: pushing a tag `v<version>` on `main`, matching `VERSION` in `cli/lib/server-stdio.mjs`, runs `.github/workflows/release.yml`, which tests, builds, smoke-tests and publishes a GitHub Release with `PixelLab-darwin-arm64.dmg` and `.zip`. Steps are in README.md, "Releases". Don't push a release tag unless the owner asks.
- App icon: `node scripts/make-icon.mjs` rewrites `app/Sources/PixelLab/Resources/AppIcon.png` (build-app.sh makes the .icns)

## Verifying your work

- CLI: `npm test` must show `# fail 0`. Never skip or delete a failing test.
- App, when `app/` changed: `swift build --package-path app` with no errors, and `scripts/test-app.sh` with every test passing.
- End to end, when `app/`, `cli/` or `scripts/acceptance/` changed: `scripts/build-app.sh`, then `scripts/acceptance.sh` exits 0.
- Commands: after changing one, `npm run docs:cli` leaves `git status` clean apart from your own edits.
- Docs: after changing a doc or a README, `npm run docs:site` builds with no broken links.
- UI: drive the change with the script hook and its `expect` lines, and snapshot each changed view in dark and light (`snapshot <png>` in a script, or `scripts/snapshot.sh`). Look at the snapshots before reporting.

Run these before reporting any task complete, and paste the output. If a test fails, fix the code, not the test. Something you couldn't run is "not verified", never passed.

## Rules

- The document is the source of truth for the art, and the engine lives here (`cli/lib/engine/`: engine, compiler, previews). Never re-import the engine from a site; a change to the engine is a change here, released by tag, and the site bumps its `pixellab` pin. Never write to the real site checkout, or the real document inside it, from tests or scripts: use a temp copy (`makeDocument()`, `makeSite()` in `cli/test/helpers.mjs`). A change to the site's art is a change in the site repo, under its own workflow.
- No new npm or Swift dependencies without asking.
- Never use `xcode-select -s` or change system settings. Use `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer` per command if full Xcode is needed.
- The app builds with the Command Line Tools, which lack the SwiftUI macro plugin: don't use `@State` (keep view state in `AppModel`); `@Observable`, `@Environment`, `@AppStorage`, `@FocusState` are fine.
- Colors in the app come from the board 03 tokens in `app/Sources/PixelLab/Theme/Theme.swift`. Pixel art is drawn nearest-neighbor at integer zoom.
- Every action is a command in the registry (`cli/lib/commands/`), so the CLI, the app (`serve --stdio`) and agents (`serve --mcp`) can all take it. MCP edits become proposals the owner approves in the Agent window; `--direct` stays off by default.
- Keep VoiceOver labels, visible focus, comfortable hit targets and Reduce Motion support on anything interactive or animated.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. PR bodies end with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- No secrets, tokens or personal data in commits, PRs or comments. The repo is public, and so are the art it edits (published by the site) and the docs site.

## Architecture

The record has the full picture. In short: `cli/` is the Node CLI (`bin/pixellab.mjs`; `lib/engine/` is the engine, compiler and previews; `lib/document.mjs` resolves the document, or the one inside a site checkout, and reads its manifest; `lib/registry.mjs` and `lib/commands/` hold every command; `lib/scene-ops.mjs` is the scene editing logic ported from the site's old lab; `lib/session.mjs` holds open docs with undo and redo; `lib/server-stdio.mjs` and `lib/server-mcp.mjs` are the two servers; `lib/state.mjs` is the state dir for proposals and the activity log), with `node:test` suites in `cli/test/`. `app/` is the SwiftPM package (`Server/` talks JSON-RPC to `pixellab serve --stdio`; `Model/` holds `AppModel` and the editors; `Views/` the windows; `Theme/` the tokens; `Support/ScriptRunner.swift` the test hook), with Swift Testing suites in `app/Tests/`. `scripts/` builds, installs, snapshots and runs the acceptance pass.

## How changes flow

Planned work follows Anthropic's [AI-native SDLC playbook](https://claude.com/resources/articles/the-ai-native-sdlc-playbook), the same way as the site repo. Each stage commits an artifact, and the next stage starts by reading it:

1. **`intent.md`:** what's wanted and why, in the owner's words. Use the `write-intent` skill. The owner accepts it by merging.
2. **`spec.md`:** numbered requirements and design from the accepted intent, with areas of concern flagged. Use `write-spec`. The owner approves it by merging.
3. **`plan.md`:** files, order of work, workflow, risks and proof, written before any code. Use `write-plan`. Its workflow says who runs each step (the main session or subagents), which steps pause for the owner's review, and whether any independent review runs. Implement only against an approved plan, and update `plan.md` in the same commit when the work departs from it.
4. **Code and tests,** following the plan's workflow, then a PR that cites the plan and pastes the verification output. Name tests after the requirement they prove (`R3: …`). Run the steps back to back. Hand over to the owner only at a review the plan names, or when blocked (`write-plan`, "Workflow and reviews").
5. **Independent verification, when the plan calls for it:** a fresh session, given only "Use the `verify-change` skill on PR #<n>.", checks the PR against its intent, spec and plan and reports as a PR comment. It never pushes or commits. Fix its blocking findings before the PR merges. Without one in the plan, the implementer's checks in the PR are the verification. The owner can still ask for a run on any PR.

A small, self-contained fix can go straight to a PR into `main`.

Planned work runs on a main-session branch, `feat/<slug>`, where every slice PR (`feat/<slug>-<slice>`) merges, and reaches `main` in one final PR (`write-plan`, "Branches"). A one-slice change may go straight to `main`. Agents may merge slice PRs into `feat/<slug>` once their checks pass, after any review the plan names for them. Only the owner merges into `main`, because it's what Pixel Lab is installed from; a hook in `.claude/settings.json` blocks agents from doing it.

Each change lives in `docs/intents/YYYY-MM-<slug>/` (the month its intent was created), holding its `intent.md`, `spec.md`, `plan.md` and any notes. Don't create top-level `docs/specs/` or `docs/plans/` folders.
- Link within a change by sibling path (`spec.md`), and to another change with `../YYYY-MM-<slug>/intent.md`.
- When a doc moves, update every mention of it, including code comments.

If documents disagree, the intent wins over the spec and the spec over the plan. Fix the lower one. `2026-10-mac-app` predates these templates (it has only a plan); don't rewrite it to match. Its follow-ups list is a good source of intents.

## Things Claude gets wrong

When Claude makes the same mistake twice, add the correction here.

- Large pushes (screenshots) can fail on GitHub's default buffer: use `git -c http.postBuffer=157286400 push`.
- A branch cut from `origin/main` tracks it, so a bare `git push` targets `main` and the hook blocks it. Push with `git push -u origin <branch>`.
- `scripts/acceptance.sh` pins today's art; a failing `expect` after the site's art changed means updating the script, not the app.
