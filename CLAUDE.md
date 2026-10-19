# CLAUDE.md

Pixel Lab for Mac: a `pixellab` CLI and MCP server (Node, zero dependencies) and a SwiftUI Mac app, for the pixel art in the site repo `~/Projects/jlawcordova.github.io`. Read `docs/plan.md` first: it is the architecture, the look (mood board 03, "Field notebook") and the build workflow.

## Commands

- Test: `npm test` (node:test; must show `# fail 0`)
- CLI: `node cli/bin/pixellab.mjs help`
- App: `swift build --package-path app`, then `scripts/build-app.sh` → `build/Pixel Lab.app`

## Rules

- The site checkout is the source of truth for art and engine. Import the engine from it at runtime; never copy or port it. Never write to the real site checkout from tests: use a temp copy.
- No new npm or Swift dependencies without asking.
- Work on a `slice/<name>` branch off `feat/mac-app`, PR into `feat/mac-app`, merge when checks pass. Never merge into `main`.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. PR bodies end with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- Never use `xcode-select -s` or change system settings. Use `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer` per command if full Xcode is needed.
