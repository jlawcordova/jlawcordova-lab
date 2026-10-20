#!/bin/bash
# The fake installer (docs/intents/2026-10-agent-chat/spec.md, D16): what the app runs instead of
# `curl -fsSL https://claude.ai/install.sh | bash` under the script hook (PIXELLAB_CLAUDE_INSTALLER).
# Like the real one, it installs into $HOME/.local/share/claude and links $HOME/.local/bin/claude,
# where HOME is the hook's temp home (PIXELLAB_CLAUDE_HOME), never the owner's.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
echo "Downloading Claude Code for macOS (arm64)…"
sleep 0.2
dest="$HOME/.local/share/claude/fake"
mkdir -p "$dest" "$HOME/.local/bin"
cp "$here/claude" "$here/turns.json" "$dest/"
chmod +x "$dest/claude"
echo "Checking the download…"
ln -sf "$dest/claude" "$HOME/.local/bin/claude"
echo "Installed claude to ~/.local/bin/claude"
