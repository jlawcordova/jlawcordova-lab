#!/bin/zsh
# The acceptance pass of docs/plan.md ("What ready to use means"), end to end
# on a temp copy of the site: scene edits → save → build art, object paint →
# save (scripts/acceptance/scene-object.txt), then an agent over MCP saves two
# proposals (mcp-agent.mjs, through the app bundle's CLI) and the Agent window
# ghosts, approves and rejects them (agent.txt). The real checkout is only
# read (copied). Exits 0 if every step passed.
#
#   scripts/acceptance.sh [out-dir]      (default: a new temp dir)
#
# Needs build/Pixel Lab.app (scripts/build-app.sh). The scripts expect the
# site as it was on 2026-10-19 (15 scenes, 161 objects, the house at
# px -59,59); update their `expect` lines when the art changes.
set -euo pipefail

root=${0:A:h:h}
real=${PIXELLAB_REAL_SITE:-$HOME/Projects/jlawcordova.github.io}
out=${1:-$(mktemp -d -t pixellab-acceptance)}
app="$root/build/Pixel Lab.app"
[[ -x "$app/Contents/MacOS/PixelLab" ]] || { echo "no $app: run scripts/build-app.sh" >&2; exit 1; }

site="$out/site" state="$out/state" shots="$out/shots"
rm -rf "$site" "$state" && mkdir -p "$site" "$state" "$shots"
cp -R "$real/src" "$real/scripts" "$real/package.json" "$real/package-lock.json" "$site/"
[[ -d "$real/node_modules" ]] && ln -s "$real/node_modules" "$site/node_modules"
git -C "$site" init -q && git -C "$site" add -A && git -C "$site" -c user.email=t@t -c user.name=t commit -qm base

run() { # run <script> <log>
  sed "s#@OUT@#$shots#g" "$root/scripts/acceptance/$1" > "$out/$1"
  PIXELLAB_SCRIPT="$out/$1" PIXELLAB_SITE="$site" PIXELLAB_STATE="$state" "$app/Contents/MacOS/PixelLab" 2>&1 | grep '^script:' > "$out/$2" || true
  tail -1 "$out/$2"
  grep -q 'all steps passed' "$out/$2"
}

echo "==> scene and object mode, save, build art"
run scene-object.txt scene-object.log
git -C "$site" add -A && git -C "$site" -c user.email=t@t -c user.name=t commit -qm "after scene-object"
echo "==> an agent over MCP proposes"
PIXELLAB_CLI_PATH="$app/Contents/Resources/cli/bin/pixellab.mjs" node "$root/scripts/acceptance/mcp-agent.mjs" "$site" "$state"
echo "==> the Agent window: ghost, approve, reject"
run agent.txt agent.log
echo "==> the temp site after the run"
git -C "$site" diff --stat HEAD~1 HEAD
git -C "$site" diff --stat
echo "passed; logs, snapshots and the temp site are in $out"
