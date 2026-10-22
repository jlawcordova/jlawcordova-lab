#!/bin/zsh
# The acceptance pass of docs/intents/2026-10-mac-app/plan.md ("What ready to use means"), end to end
# on a temp copy of the site: scene edits → save → publish, object paint →
# save (scripts/acceptance/scene-object.txt), then an agent over MCP saves two
# proposals (mcp-agent.mjs, through the app bundle's program) and the Agent window
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
# The app runs the pixellab program in its bundle (Contents/Helpers/pixellab) and reads format 2 only (docs/intents/2026-10-rust-engine/):
# the temp site, the example copies and the unsorted document are migrated with the program before their scripts run, and
# migrate.txt starts the app on a copy left in format 1: R9's problem, Migrate…, Cancel leaves it, Migrate converts it.
# The real checkout is only read (copied). Exits 0 if every step passed.
#
#   scripts/acceptance.sh [--only <name>[,<name>...]] [out-dir]      (default: a new temp dir)
#
# --only (or PIXELLAB_ACCEPT_ONLY=<name>[,<name>...]; --only wins when both are given) runs only the named
# stages. A name is a file in scripts/acceptance/ without its extension: a stage script
# (layers, document, examples-town, chat-relaunch-2, ...) or one of the .mjs agents (mcp-agent, stale-agent).
# The shared setup always runs (the temp site copy, its migrate and sort, the git base commit). A stage that
# reads what an earlier one left needs that one named too: agent needs mcp-agent, stale needs stale-agent, and
# chat-relaunch-2 and -3 need chat-relaunch-1. An unknown name fails before anything runs and lists the valid
# ones. The skipped stages are printed as they're passed, and the end says which ran and that this was a
# partial pass. It exits 0 if every selected stage passed; running the stages a change touches is enough to
# verify it (CLAUDE.md, "Verifying your work"). Without a filter every stage runs, as before.
#
# Needs build/Pixel Lab.app (scripts/build-app.sh). The scripts expect the
# site as it was on 2026-10-21 (28 scenes, 204 objects, 6 SVGs and 13 thumbnails, the house at
# px -59,59); update their `expect` lines when the art changes.
set -euo pipefail

root=${0:A:h:h}
# Stage names: every stage script and agent in scripts/acceptance/, without its extension.
names=("$root"/scripts/acceptance/*.(txt|mjs)(N:t:r))
filter=${PIXELLAB_ACCEPT_ONLY-} filtered=${${PIXELLAB_ACCEPT_ONLY:+1}:-0} positional=()
while (( $# )); do
  case $1 in
    --only) (( $# >= 2 )) || { echo "--only needs <name>[,<name>...]" >&2; exit 2; }; filter=$2 filtered=1; shift 2 ;;
    --only=*) filter=${1#--only=} filtered=1; shift ;;
    -*) echo "unknown option $1; usage: scripts/acceptance.sh [--only <name>[,<name>...]] [out-dir]" >&2; exit 2 ;;
    *) positional+=$1; shift ;;
  esac
done
(( ${#positional} <= 1 )) || { echo "one out-dir at most, got: ${positional[*]}" >&2; exit 2; }
only=(${(s:,:)filter})
if (( filtered )); then
  (( ${#only} )) || { echo "--only names no stage; valid names: ${(j:, :)names}" >&2; exit 2; }
  for name in $only; do
    (( ${names[(Ie)$name]} )) || { echo "unknown stage '$name'; valid names: ${(j:, :)names}" >&2; exit 2; }
  done
fi
ran=() skipped=()
stage() { # stage <name>: true when <name> runs (every stage without a filter); a skipped one is printed
  if (( ! filtered || ${only[(Ie)$1]} )); then ran+=$1; return 0; fi
  skipped+=$1
  echo "--> skipped $1 (not in --only)"
  return 1
}

real=${PIXELLAB_REAL_SITE:-$HOME/Projects/jlawcordova.github.io}
out=${positional[1]:-$(mktemp -d -t pixellab-acceptance)}
app="$root/build/Pixel Lab.app"
[[ -x "$app/Contents/MacOS/PixelLab" ]] || { echo "no $app: run scripts/build-app.sh" >&2; exit 1; }

site="$out/site" state="$out/state" shots="$out/shots"
rm -rf "$site" "$state" "$shots" && mkdir -p "$site" "$state" "$shots"
cp -R "$real/src" "$real/scripts" "$real/package.json" "$real/package-lock.json" "$site/"
[[ -d "$real/node_modules" ]] && ln -s "$real/node_modules" "$site/node_modules"
program="$app/Contents/Helpers/pixellab"
[[ -x $program ]] || { echo "no $program: run scripts/build-app.sh" >&2; exit 1; }
export PIXELLAB_PROGRAM="$program"
# The program reads format 2 only: a copy of a format-1 document is migrated (in place) before the app opens it.
migrate_doc() { # migrate_doc <doc.pixellab>: no-op when it is already format 2
  [[ -f "$1/palette.mjs" ]] && PIXELLAB_STATE="$state" "$program" document migrate --document "$1" | tail -1
  return 0
}
echo "==> the temp site, migrated to format 2 (document migrate)"
PIXELLAB_STATE="$state" "$program" document migrate --site "$site" | tail -1 || true
# Every file Pixel Lab writes is in depth order (docs/intents/2026-10-depth-order/spec.md, R4), so the temp site starts
# that way: sorted by the bundled CLI, and that is the base the scripts' saves and merges are measured against.
echo "==> the temp site, put in depth order (scene sort --all)"
PIXELLAB_STATE="$state" "$program" scene sort --all --site "$site" | head -3
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
    rm -rf "$doc" "$out/state-$3-chat" && mkdir -p "$out/examples" && cp -R "$root/examples/$3.pixellab" "$doc" && migrate_doc "$doc" > /dev/null
    where=(PIXELLAB_DOC="$doc") st="$out/state-$3-chat"
  fi
  env FAKE_CLAUDE_SIGNED_IN=1 FAKE_CLAUDE_STATE="$fake_home/state" PIXELLAB_CLAUDE_HOME="$fake_home" "${where[@]}" \
    PIXELLAB_SCRIPT="$out/$1" PIXELLAB_STATE="$st" "$app/Contents/MacOS/PixelLab" 2>&1 | grep '^script:' > "$out/$2" || true
  tail -1 "$out/$2"
  grep -q 'all steps passed' "$out/$2"
}

run_example() { # run_example <script> <log> <example>: on a temp copy of examples/<example>.pixellab
  local doc="$out/examples/$3.pixellab"
  rm -rf "$doc" && mkdir -p "$out/examples" && cp -R "$root/examples/$3.pixellab" "$doc" && migrate_doc "$doc" > /dev/null
  sed "s#@OUT@#$shots#g" "$root/scripts/acceptance/$1" > "$out/$1"
  PIXELLAB_SCRIPT="$out/$1" PIXELLAB_DOC="$doc" PIXELLAB_STATE="$state" "$app/Contents/MacOS/PixelLab" 2>&1 | grep '^script:' > "$out/$2" || true
  tail -1 "$out/$2"
  grep -q 'all steps passed' "$out/$2"
}

if stage scene-object; then
  echo "==> scene and object mode, save, publish"
  run scene-object.txt scene-object.log
  svgs=$(ls "$site"/src/assets/pixel-art/*.svg | wc -l | tr -d ' ')
  [[ $svgs == 6 ]] || { echo "expected 6 published SVGs in the temp site, found $svgs" >&2; exit 1; }
  echo "published $svgs SVGs into the temp site"
  git -C "$site" add -A && git -C "$site" -c user.email=t@t -c user.name=t commit -qm "after scene-object"
fi
if stage mcp-agent; then
  echo "==> an agent over MCP proposes"
  node "$root/scripts/acceptance/mcp-agent.mjs" "$site" "$state"
fi
if stage agent; then
  echo "==> the Agent window: ghost, approve, reject"
  run agent.txt agent.log
fi
if stage stale-agent; then
  echo "==> a doc changed on disk under unsaved edits: merge, reload, keep; an approve merged in"
  node "$root/scripts/acceptance/stale-agent.mjs" "$site" "$state"
fi
if stage stale; then
  (( ${ran[(Ie)stale-agent]} )) || echo "==> a doc changed on disk under unsaved edits: merge, reload, keep; an approve merged in"
  run stale.txt stale.log
fi
if stage chat; then
  echo "==> the Agent chat on the site: a chat's proposal ghosted, approved and rejected from its card; the Agent window"
  run_chat chat.txt chat.log
  grep -q 'closeup-front-trees' <(git -C "$site" diff --stat) || { echo "approving the chat's proposal didn't change closeup-front-trees" >&2; exit 1; }
  git -C "$site" add -A && git -C "$site" -c user.email=t@t -c user.name=t commit -qm "after chat"
fi
if stage document; then
  echo "==> the document: save as, reopen the copy, publish into a folder"
  run document.txt document.log
  copied=$(ls "$shots"/copy-out/*.svg | wc -l | tr -d ' ')
  [[ $copied == 6 ]] || { echo "expected 6 SVGs published from the copy, found $copied" >&2; exit 1; }
  thumbs=$(ls "$shots"/copy-site/static/public/thumbnails/*.png | wc -l | tr -d ' ')
  [[ $thumbs == 13 ]] || { echo "expected 13 thumbnails published from the copy, found $thumbs" >&2; exit 1; }
  [[ -f "$shots/copy-site/src/assets/pixel-art/copy.pixellab/manifest.json" ]] || { echo "the copy has no manifest.json" >&2; exit 1; }
fi
if stage layers; then
  echo "==> scene layers: the panel, the stage, the menu, the inspector; the same SVG with layers"
  run layers.txt layers.log
  cmp "$shots/layers-before/hero-island.svg" "$shots/layers-after/hero-island.svg" || { echo "hero-island.svg changed when its layers were saved" >&2; exit 1; }
  scene="$site/src/assets/pixel-art/jlawcordova.pixellab/scenes/hero-island.json"
  grep -q '^  "layers": \[$' "$scene" || { echo "the saved hero island has no layers" >&2; exit 1; }
  if grep -qE 'hidden|locked' "$scene"; then echo "hidden or locked state went into the scene file" >&2; exit 1; fi
  grep -q '"group-3"' "$state/layer-view.json" || { echo "the hidden Sky layer isn't in the app's view state" >&2; exit 1; }
  echo "hero-island.svg identical with layers; the scene holds $(grep -c '"layer": "' "$scene") items in layers and no view state"
fi
if stage examples-harbor; then
  echo "==> the example documents: layers from the file, three and four levels deep"
  run_example examples-harbor.txt examples-harbor.log harbor
fi
if stage examples-town; then
  (( ${ran[(Ie)examples-harbor]} )) || echo "==> the example documents: layers from the file, three and four levels deep"
  run_example examples-town.txt examples-town.log town
fi
if stage examples-depth; then
  echo "==> depth order: the pine moved behind the house, the override, the saved file"
  run_example examples-depth.txt examples-depth.log harbor
fi
if stage actions; then
  echo "==> ambient loops and actions: the barrel's loop on the timeline, the boat's hop played, mid-play and at rest, and under Reduce Motion"
  run_example actions.txt actions.log harbor
  for shot in actions-mid-play-light actions-rest-light; do [[ -s "$shots/$shot.png" ]] || { echo "actions.txt didn't snapshot $shot" >&2; exit 1; }; done
  cmp -s "$shots/actions-mid-play-light.png" "$shots/actions-rest-light.png" && { echo "the boat looks the same mid-hop as at rest" >&2; exit 1; }
  echo "the hop played: mid-play differs from the rest frame"
fi
if stage chat-setup; then
  echo "==> the Agent chat: Claude Code's setup (the fake), chats, steps, a question, Stop"
  claude_home="$out/claude-home" chat_state="$out/chat-state"
  rm -rf "$claude_home" "$chat_state" && mkdir -p "$claude_home" "$chat_state"
  doc="$out/examples/chat.pixellab"
  rm -rf "$doc" && cp -R "$root/examples/harbor.pixellab" "$doc" && migrate_doc "$doc" > /dev/null
  sed "s#@OUT@#$shots#g" "$root/scripts/acceptance/chat-setup.txt" > "$out/chat-setup.txt"
  FAKE_CLAUDE_INSTALL_S=1 FAKE_CLAUDE_LOGIN_MS=2500 PIXELLAB_CLAUDE_HOME="$claude_home" PIXELLAB_CLAUDE_INSTALLER="$root/scripts/fake-claude/install.sh" \
    PIXELLAB_SCRIPT="$out/chat-setup.txt" PIXELLAB_DOC="$doc" PIXELLAB_STATE="$chat_state" "$app/Contents/MacOS/PixelLab" -rightTab inspector 2>&1 | grep '^script:' > "$out/chat-setup.log" || true
  tail -1 "$out/chat-setup.log"
  grep -q 'all steps passed' "$out/chat-setup.log"
  proposal=$(ls "$chat_state"/proposals/*.json | head -1)
  grep -q '"chat": {' "$proposal" && grep -q '"number": 1' "$proposal" || { echo "the chat's proposal doesn't name its chat and number" >&2; exit 1; }
  grep -q '"chat":{"id"' "$chat_state/activity.jsonl" || { echo "no chat lines in the activity log" >&2; exit 1; }
  [[ $(ls "$chat_state"/chats/*.json | wc -l | tr -d ' ') == 2 ]] || { echo "expected 2 chats in the state dir" >&2; exit 1; }
  [[ -z $(find "$doc" -name '*.json' ! -name manifest.json ! -name palette.json ! -path "$doc/scenes/*" ! -path "$doc/objects/*") ]] || { echo "chat state went into the document" >&2; exit 1; }
  # R3: each chat is one Claude session, its follow-ups resumed (the stopped turn never got to save one).
  sessions=("$claude_home"/.local/share/claude/fake/fake-claude-state/sessions/*.json)
  [[ ${#sessions[@]} == 2 ]] || { echo "expected 2 Claude sessions for 2 chats, found ${#sessions[@]}: follow-ups didn't resume (R3)" >&2; exit 1; }
  grep -q '"turns":4' "${sessions[@]}" || { echo "the first chat's four turns weren't one resumed session (R3)" >&2; exit 1; }
  echo "the chat's proposals name their chat and number; 2 chats kept in the state dir, none in the document; one Claude session per chat"
fi
if stage chat-harbor; then
  echo "==> the Agent chat on the harbor example: layers as chips, Option-click, pins, unsaved items, a question, ghosts per chat"
  run_chat chat-harbor.txt chat-harbor.log harbor
fi
relaunch_doc="$out/examples/relaunch.pixellab" relaunch_state="$out/state-relaunch" relaunch_fake="$out/fake-relaunch"
relaunch() { # relaunch <part>
  sed "s#@OUT@#$shots#g" "$root/scripts/acceptance/chat-relaunch-$1.txt" > "$out/chat-relaunch-$1.txt"
  FAKE_CLAUDE_SIGNED_IN=1 FAKE_CLAUDE_STATE="$relaunch_fake" PIXELLAB_CLAUDE_HOME="$fake_home" PIXELLAB_DOC="$relaunch_doc" \
    PIXELLAB_SCRIPT="$out/chat-relaunch-$1.txt" PIXELLAB_STATE="$relaunch_state" "$app/Contents/MacOS/PixelLab" 2>&1 | grep '^script:' > "$out/chat-relaunch-$1.log" || true
  tail -1 "$out/chat-relaunch-$1.log"
  grep -q 'all steps passed' "$out/chat-relaunch-$1.log"
}
if stage chat-relaunch-1; then
  echo "==> the Agent chat across launches: the session resumed, a proposal settled while closed, a lost session"
  rm -rf "$relaunch_doc" "$relaunch_state" "$relaunch_fake" && cp -R "$root/examples/harbor.pixellab" "$relaunch_doc" && migrate_doc "$relaunch_doc" > /dev/null && mkdir -p "$relaunch_state"
  relaunch 1
fi
if stage chat-relaunch-2; then
  # Pixel Lab is closed: the chat's proposal is rejected from a terminal.
  proposal=$(basename "$(ls "$relaunch_state"/proposals/*.json | head -1)" .json)
  PIXELLAB_STATE="$relaunch_state" "$program" proposal reject "$proposal" --document "$relaunch_doc"
  relaunch 2
fi
if stage chat-relaunch-3; then
  # Claude Code loses the chat's session.
  rm -rf "$relaunch_fake/sessions"
  relaunch 3
fi
# The site's document is in depth order since its own depth-order change (f5d74e3), so the unsorted one is
# the document as it was just before it, read (not checked out) from the site's history.
unsorted_rev=${PIXELLAB_UNSORTED_REV:-6d19714}
old_document() { # old_document <dest>: the site's document at $unsorted_rev (format 1, unsorted), read from its history
  rm -rf "$1" "$out/old-src" && mkdir -p "$out/old-src"
  git -C "$real" archive "$unsorted_rev" src/assets/pixel-art/jlawcordova.pixellab | tar -x -C "$out/old-src"
  mv "$out/old-src/src/assets/pixel-art/jlawcordova.pixellab" "$1"
}
if stage sort; then
  echo "==> sort: a document written before depth order, the Checks warning, Sort, save"
  doc="$out/examples/unsorted.pixellab"
  old_document "$doc"
  migrate_doc "$doc" > /dev/null
  sed "s#@OUT@#$shots#g" "$root/scripts/acceptance/sort.txt" > "$out/sort.txt"
  PIXELLAB_SCRIPT="$out/sort.txt" PIXELLAB_DOC="$doc" PIXELLAB_STATE="$state" "$app/Contents/MacOS/PixelLab" 2>&1 | grep '^script:' > "$out/sort.log" || true
  tail -1 "$out/sort.log"
  grep -q 'all steps passed' "$out/sort.log"
fi
if stage migrate; then
  # (-documentPath "" keeps the owner's saved Settings out of the Settings snapshot.)
  echo "==> migrate: a format-1 document is R9's problem; Migrate… asks, Cancel leaves it, Migrate converts it"
  old="$out/examples/old-format.pixellab"
  old_document "$old"
  sed "s#@OUT@#$shots#g" "$root/scripts/acceptance/migrate.txt" > "$out/migrate.txt"
  PIXELLAB_SCRIPT="$out/migrate.txt" PIXELLAB_DOC="$old" PIXELLAB_STATE="$out/state-migrate" "$app/Contents/MacOS/PixelLab" -documentPath "" -rightTab inspector 2>&1 | grep '^script:' > "$out/migrate.log" || true
  tail -1 "$out/migrate.log"
  grep -q 'all steps passed' "$out/migrate.log"
  [[ -f "$old/palette.json" && ! -e "$old/palette.mjs" && -z $(find "$old" -name '*.mjs') ]] || { echo "the migrated document still has .mjs sources" >&2; exit 1; }
  echo "the old-format copy is migrated: $(ls "$old/scenes" | wc -l | tr -d ' ') scenes, $(ls "$old/objects" | wc -l | tr -d ' ') objects, all .json"
fi
if stage canvas; then
  echo "==> a scene without a size, and keyboard block placement: scene new, a stack, Tab, the drop key, far from the art, publish"
  canvas="$out/examples/canvas.pixellab" canvas_state="$out/state-canvas"
  rm -rf "$canvas" "$canvas_state" && mkdir -p "$out/examples" "$canvas_state" && cp -R "$root/examples/harbor.pixellab" "$canvas" && migrate_doc "$canvas" > /dev/null
  PIXELLAB_STATE="$canvas_state" "$program" scene new meadow --document "$canvas" > /dev/null
  # `scene new` writes no viewBox (R1); give the scene an output so it publishes.
  grep -q viewBox "$canvas/scenes/meadow.json" && { echo "scene new wrote a viewBox (R1)" >&2; exit 1; }
  perl -0pi -e 's/\A\{\n/{\n  "output": "meadow.svg",\n/' "$canvas/scenes/meadow.json"
  sed "s#@OUT@#$shots#g" "$root/scripts/acceptance/canvas.txt" > "$out/canvas.txt"
  PIXELLAB_SCRIPT="$out/canvas.txt" PIXELLAB_DOC="$canvas" PIXELLAB_STATE="$canvas_state" "$app/Contents/MacOS/PixelLab" -showFrame YES 2>&1 | grep '^script:' > "$out/canvas.log" || true
  tail -1 "$out/canvas.log"
  grep -q 'all steps passed' "$out/canvas.log"
  for f in harbor.svg harbor-flat.svg; do
    cmp "$shots/canvas-before/$f" "$shots/canvas-after/$f" || { echo "$f changed while a scene without a size was made (R4)" >&2; exit 1; }
  done
  meadow_width=$(sed -n 's/.*<svg[^>]* viewBox="[-0-9]* [-0-9]* \([0-9]*\) [0-9]*".*/\1/p' "$shots/canvas-after/meadow.svg" | head -1)
  [[ -n $meadow_width && $meadow_width -gt 128 ]] || { echo "the meadow's SVG is $meadow_width wide: it should fit what was placed far from the origin (R2)" >&2; exit 1; }
  echo "harbor SVGs byte-identical; the meadow publishes at ${meadow_width} px wide, fit to its content (the old default cut at 128)"
fi
echo "==> the temp site after the run"
# (A partial pass may have committed nothing after the base.)
git -C "$site" rev-parse -q --verify HEAD~1 > /dev/null && git -C "$site" diff --stat HEAD~1 HEAD
git -C "$site" diff --stat
echo "passed; logs, snapshots and the temp site are in $out"
if (( filtered )); then
  echo "ran: ${(j:, :)ran}"
  echo "skipped: ${${(j:, :)skipped}:-none}"
  echo "PARTIAL PASS (--only): only the stages above ran; this is not the full acceptance pass (scripts/acceptance.sh with no filter)"
fi
