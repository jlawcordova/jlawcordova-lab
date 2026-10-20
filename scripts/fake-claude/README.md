# The fake Claude Code

A stand-in for the `claude` CLI, so Pixel Lab's Agent chat is tested without the owner's Claude plan or the network ([spec](../../docs/intents/2026-10-agent-chat/spec.md), D16). Tests and the acceptance pass use it; the app never runs it on its own.

- `claude` (a shell launcher for `fake-claude.mjs`): answers `--version`, `auth status --json`, `auth login --claudeai`, `auth logout`, and headless turns (`-p --input-format stream-json --output-format stream-json …`). A turn reads the one user message, picks the first turn in `turns.json` whose `match` fits the text after the `<selection>` block, and plays it: each tool call goes through the real pixellab MCP server named in `--mcp-config`, and every line it writes has the real CLI's shape. Signed out, it writes the lines the real CLI wrote for an expired login. It refuses a turn without the flags that keep the chat to Pixel Lab's tools.
- `turns.json`: the turns. Steps are `say`, `tool` (with `input`, and `each: "items"` to repeat it per selected item), `ask` (a `chat_ask` with `options`) and `wait`. Templates: `{{scene}}`, `{{item}}`, `{{first}}` (the first item's object), `{{count}}`, `{{names}}`, `{{uses}}` (how often `first` is used, from the last `scene_items`), `{{proposal}}` (the last `doc_save`'s #n), and a field of an earlier result, such as `{{object_show.size.1+4}}`.
- `install.sh`: the fake installer. Like the real one it installs into `$HOME/.local/share/claude` and links `$HOME/.local/bin/claude`; under the script hook HOME is a temp home.

Environment: `FAKE_CLAUDE_STATE` (auth and sessions; default: beside the script's real path), `FAKE_CLAUDE_SIGNED_IN=1` (start signed in), `FAKE_CLAUDE_LOGIN_MS` (how long sign-in takes, default 800), `FAKE_CLAUDE_STEP_MS` (the pause between steps, default 60).

Try it by hand:

```bash
HOME=$(mktemp -d) scripts/fake-claude/install.sh
```
