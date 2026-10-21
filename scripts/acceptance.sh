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
# the hero island's SVG byte-identical before and after they're saved (layers.txt);
# then the example documents' layers, read from a file, on temp copies of
# examples/ (examples-harbor.txt, examples-town.txt), and depth order on the harbor example: the front
# pine moved behind the red house draws behind its wall, the Depth override draws it over, and the saved
# file is in order (examples-depth.txt). The Agent chat (docs/intents/2026-10-agent-chat/) runs against the fake
# Claude Code in scripts/fake-claude/, from an empty temp home: install, sign in and chat on a temp harbor copy
# (chat-setup.txt), never the owner's Claude Code or plan. The temp site is put in depth order first, with the bundled CLI's
# scene sort --all, and that is the base it starts from; a copy of the site's document from before its depth-order change, unsorted,
# checks the Checks warning and its Sort button (sort.txt).
# The real checkout is only read (copied). Exits 0 if every step passed.
#
#   scripts/acceptance.sh [out-dir]      (default: a new temp dir)
#
# Needs build/Pixel Lab.app (scripts/build-app.sh). The scripts expect the
# site as it was on 2026-10-21 (28 scenes, 204 objects, the house at
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
# Every file Pixel Lab writes is in depth order (docs/intents/2026-10-depth-order/spec.md, R4), so the temp site starts
# that way: sorted by the bundled CLI, and that is the base the scripts' saves and merges are measured against.
echo "==> the temp site, put in depth order (scene sort --all)"
PIXELLAB_STATE="$state" node "$app/Contents/Resources/cli/bin/pixellab.mjs" scene sort --all --site "$site" | head -3
git -C "$site" init -q && git -C "$site" add -A && git -C "$site" -c user.email=t@t -c user.name=t commit -qm base

run() { # run <script> <log>
  sed "s#@OUT@#$shots#g" "$root/scripts/acceptance/$1" > "$out/$1"
  PIXELLAB_SCRIPT="$out/$1" PIXELLAB_SITE="$site" PIXELLAB_STATE="$state" "$app/Contents/MacOS/PixelLab" 2>&1 | grep '^script:' > "$out/$2" || true
  tail -1 "$out/$2"
  grep -q 'all steps passed' "$out/$2"
}

# The Agent chat's scripts run against the fake Claude Code (scripts/fake-claude/), installed and signed in
# in a temp home, never the owner's Claude Code or plan.
fake_home="$out/claude-signed-in"
rm -rf "$fake_home" && mkdir -p "$fake_home" && HOME="$fake_home" "$root/scripts/fake-claude/install.sh" > /dev/null
run_chat() { # run_chat <script> <log> [<example>]: on the temp site, or a temp copy of examples/<example>.pixellab
  sed "s#@OUT@#$shots#g" "$root/scripts/acceptance/$1" > "$out/$1"
  local where=(PIXELLAB_SITE="$site") st="$state"
  if [[ -n ${3:-} ]]; then
    # An example gets its own copy and its own state dir (its proposals count from #1).
    local doc="$out/examples/$3-chat.pixellab"
    rm -rf "$doc" "$out/state-$3-chat" && mkdir -p "$out/examples" && cp -R "$root/examples/$3.pixellab" "$doc"
    where=(PIXELLAB_DOC="$doc") st="$out/state-$3-chat"
  fi
  env FAKE_CLAUDE_SIGNED_IN=1 FAKE_CLAUDE_STATE="$fake_home/state" PIXELLAB_CLAUDE_HOME="$fake_home" "${where[@]}" \
    PIXELLAB_SCRIPT="$out/$1" PIXELLAB_STATE="$st" "$app/Contents/MacOS/PixelLab" 2>&1 | grep '^script:' > "$out/$2" || true
  tail -1 "$out/$2"
  grep -q 'all steps passed' "$out/$2"
}

run_example() { # run_example <script> <log> <example>: on a temp copy of examples/<example>.pixellab
  local doc="$out/examples/$3.pixellab"
  rm -rf "$doc" && mkdir -p "$out/examples" && cp -R "$root/examples/$3.pixellab" "$doc"
  sed "s#@OUT@#$shots#g" "$root/scripts/acceptance/$1" > "$out/$1"
  PIXELLAB_SCRIPT="$out/$1" PIXELLAB_DOC="$doc" PIXELLAB_STATE="$state" "$app/Contents/MacOS/PixelLab" 2>&1 | grep '^script:' > "$out/$2" || true
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
echo "==> the Agent chat on the site: a chat's proposal ghosted, approved and rejected from its card; the Agent window"
run_chat chat.txt chat.log
grep -q 'closeup-front-trees' <(git -C "$site" diff --stat) || { echo "approving the chat's proposal didn't change closeup-front-trees" >&2; exit 1; }
git -C "$site" add -A && git -C "$site" -c user.email=t@t -c user.name=t commit -qm "after chat"
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
echo "==> the example documents: layers from the file, three and four levels deep"
run_example examples-harbor.txt examples-harbor.log harbor
run_example examples-town.txt examples-town.log town
echo "==> depth order: the pine moved behind the house, the override, the saved file"
run_example examples-depth.txt examples-depth.log harbor
echo "==> the Agent chat: Claude Code's setup (the fake), chats, steps, a question, Stop"
claude_home="$out/claude-home" chat_state="$out/chat-state"
rm -rf "$claude_home" "$chat_state" && mkdir -p "$claude_home" "$chat_state"
doc="$out/examples/chat.pixellab"
rm -rf "$doc" && cp -R "$root/examples/harbor.pixellab" "$doc"
sed "s#@OUT@#$shots#g" "$root/scripts/acceptance/chat-setup.txt" > "$out/chat-setup.txt"
FAKE_CLAUDE_INSTALL_S=1 FAKE_CLAUDE_LOGIN_MS=2500 PIXELLAB_CLAUDE_HOME="$claude_home" PIXELLAB_CLAUDE_INSTALLER="$root/scripts/fake-claude/install.sh" \
  PIXELLAB_SCRIPT="$out/chat-setup.txt" PIXELLAB_DOC="$doc" PIXELLAB_STATE="$chat_state" "$app/Contents/MacOS/PixelLab" 2>&1 | grep '^script:' > "$out/chat-setup.log" || true
tail -1 "$out/chat-setup.log"
grep -q 'all steps passed' "$out/chat-setup.log"
proposal=$(ls "$chat_state"/proposals/*.json | head -1)
grep -q '"chat": {' "$proposal" && grep -q '"number": 1' "$proposal" || { echo "the chat's proposal doesn't name its chat and number" >&2; exit 1; }
grep -q '"chat":{"id"' "$chat_state/activity.jsonl" || { echo "no chat lines in the activity log" >&2; exit 1; }
[[ $(ls "$chat_state"/chats/*.json | wc -l | tr -d ' ') == 2 ]] || { echo "expected 2 chats in the state dir" >&2; exit 1; }
[[ -z $(find "$doc" -name '*.json' ! -name manifest.json) ]] || { echo "chat state went into the document" >&2; exit 1; }
echo "the chat's proposals name their chat and number; 2 chats kept in the state dir, none in the document"
echo "==> the Agent chat on the harbor example: layers as chips, Option-click, pins, unsaved items, a question, ghosts per chat"
run_chat chat-harbor.txt chat-harbor.log harbor
echo "==> sort: a document written before depth order, the Checks warning, Sort, save"
# The site's document is in depth order since its own depth-order change (f5d74e3), so the unsorted one is
# the document as it was just before it, read (not checked out) from the site's history.
unsorted_rev=${PIXELLAB_UNSORTED_REV:-6d19714}
doc="$out/examples/unsorted.pixellab"
rm -rf "$doc" "$out/unsorted-src" && mkdir -p "$out/unsorted-src"
git -C "$real" archive "$unsorted_rev" src/assets/pixel-art/jlawcordova.pixellab | tar -x -C "$out/unsorted-src"
mv "$out/unsorted-src/src/assets/pixel-art/jlawcordova.pixellab" "$doc"
sed "s#@OUT@#$shots#g" "$root/scripts/acceptance/sort.txt" > "$out/sort.txt"
PIXELLAB_SCRIPT="$out/sort.txt" PIXELLAB_DOC="$doc" PIXELLAB_STATE="$state" "$app/Contents/MacOS/PixelLab" 2>&1 | grep '^script:' > "$out/sort.log" || true
tail -1 "$out/sort.log"
grep -q 'all steps passed' "$out/sort.log"
echo "==> the temp site after the run"
git -C "$site" diff --stat HEAD~1 HEAD
git -C "$site" diff --stat
echo "passed; logs, snapshots and the temp site are in $out"
