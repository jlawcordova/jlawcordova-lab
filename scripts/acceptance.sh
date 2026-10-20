#!/bin/zsh
# The acceptance pass of docs/intents/2026-10-mac-app/plan.md ("What ready to use means"), end to end
# on a temp copy of the site: scene edits → save → publish, object paint →
# save (scripts/acceptance/scene-object.txt), then an agent over MCP saves two
# proposals (mcp-agent.mjs, through the app bundle's CLI) and the Agent window
# ghosts, approves and rejects them (agent.txt); a doc with unsaved edits changes on
# disk and is merged, reloaded and kept, and another agent's proposal is approved into an
# unsaved copy (stale-agent.mjs, stale.txt); the document is saved as a
# copy, reopened and published into a folder (document.txt); and scene layers are
# made, picked, gone inside, hidden, locked, moved, placed into and ungrouped, with
# the hero island's SVG byte-identical before and after they're saved (layers.txt).
# The real checkout is only read (copied). Exits 0 if every step passed.
#
#   scripts/acceptance.sh [out-dir]      (default: a new temp dir)
#
# Needs build/Pixel Lab.app (scripts/build-app.sh). The scripts expect the
# site as it was on 2026-10-20 (15 scenes, 164 objects, the house at
# px -59,59); update their `expect` lines when the art changes.
set -euo pipefail

root=${0:A:h:h}
real=${PIXELLAB_REAL_SITE:-$HOME/Projects/jlawcordova.github.io}
out=${1:-$(mktemp -d -t pixellab-acceptance)}
app="$root/build/Pixel Lab.app"
[[ -x "$app/Contents/MacOS/PixelLab" ]] || { echo "no $app: run scripts/build-app.sh" >&2; exit 1; }

site="$out/site" state="$out/state" shots="$out/shots"
rm -rf "$site" "$state" "$shots" && mkdir -p "$site" "$state" "$shots"
cp -R "$real/src" "$real/scripts" "$real/package.json" "$real/package-lock.json" "$site/"
[[ -d "$real/node_modules" ]] && ln -s "$real/node_modules" "$site/node_modules"
git -C "$site" init -q && git -C "$site" add -A && git -C "$site" -c user.email=t@t -c user.name=t commit -qm base

run() { # run <script> <log>
  sed "s#@OUT@#$shots#g" "$root/scripts/acceptance/$1" > "$out/$1"
  PIXELLAB_SCRIPT="$out/$1" PIXELLAB_SITE="$site" PIXELLAB_STATE="$state" "$app/Contents/MacOS/PixelLab" 2>&1 | grep '^script:' > "$out/$2" || true
  tail -1 "$out/$2"
  grep -q 'all steps passed' "$out/$2"
}

echo "==> scene and object mode, save, publish"
run scene-object.txt scene-object.log
svgs=$(ls "$site"/src/assets/pixel-art/*.svg | wc -l | tr -d ' ')
[[ $svgs == 6 ]] || { echo "expected 6 published SVGs in the temp site, found $svgs" >&2; exit 1; }
echo "published $svgs SVGs into the temp site"
git -C "$site" add -A && git -C "$site" -c user.email=t@t -c user.name=t commit -qm "after scene-object"
echo "==> an agent over MCP proposes"
PIXELLAB_CLI_PATH="$app/Contents/Resources/cli/bin/pixellab.mjs" node "$root/scripts/acceptance/mcp-agent.mjs" "$site" "$state"
echo "==> the Agent window: ghost, approve, reject"
run agent.txt agent.log
echo "==> a doc changed on disk under unsaved edits: merge, reload, keep; an approve merged in"
PIXELLAB_CLI_PATH="$app/Contents/Resources/cli/bin/pixellab.mjs" node "$root/scripts/acceptance/stale-agent.mjs" "$site" "$state"
run stale.txt stale.log
echo "==> the document: save as, reopen the copy, publish into a folder"
run document.txt document.log
copied=$(ls "$shots"/copy-out/*.svg | wc -l | tr -d ' ')
[[ $copied == 6 ]] || { echo "expected 6 SVGs published from the copy, found $copied" >&2; exit 1; }
[[ -f "$shots/copy.pixellab/manifest.json" ]] || { echo "the copy has no manifest.json" >&2; exit 1; }
echo "==> scene layers: the panel, the stage, the menu, the inspector; the same SVG with layers"
run layers.txt layers.log
cmp "$shots/layers-before/hero-island.svg" "$shots/layers-after/hero-island.svg" || { echo "hero-island.svg changed when its layers were saved" >&2; exit 1; }
scene="$site/src/assets/pixel-art/jlawcordova.pixellab/scenes/hero-island.mjs"
grep -q '^  layers: \[$' "$scene" || { echo "the saved hero island has no layers" >&2; exit 1; }
if grep -qE 'hidden|locked' "$scene"; then echo "hidden or locked state went into the scene file" >&2; exit 1; fi
grep -q '"group-3"' "$state/layer-view.json" || { echo "the hidden Sky layer isn't in the app's view state" >&2; exit 1; }
echo "hero-island.svg identical with layers; the scene holds $(grep -c "layer: '" "$scene") items in layers and no view state"
echo "==> the temp site after the run"
git -C "$site" diff --stat HEAD~1 HEAD
git -C "$site" diff --stat
echo "passed; logs, snapshots and the temp site are in $out"
