#!/bin/zsh
# Self-snapshot of Pixel Lab: opens a doc, waits until it's rendered, writes
# the main window to a PNG and quits. Read-only (open + render, no saves).
#
#   scripts/snapshot.sh <out.png> [doc] [dark|light] [zoom]
#   scripts/snapshot.sh /tmp/hero.png scene:hero-island dark
#
# Uses build/Pixel Lab.app if it exists, else the debug build in app/.build.
# PIXELLAB_SITE overrides the site checkout; PIXELLAB_SNAPSHOT_SIZE (1440x900)
# the window size.
set -euo pipefail

root=${0:A:h:h}
out=${1:?usage: scripts/snapshot.sh <out.png> [doc] [dark|light] [zoom]}
doc=${2:-scene:hero-island}
appearance=${3:-dark}
zoom=${4:-}

app_bin="$root/build/Pixel Lab.app/Contents/MacOS/PixelLab"
[[ -x $app_bin ]] || app_bin="$root/app/.build/debug/PixelLab"
[[ -x $app_bin ]] || { echo "no app build: run scripts/build-app.sh" >&2; exit 1; }

[[ $out = /* ]] || out="$PWD/$out"
export PIXELLAB_SNAPSHOT=$out PIXELLAB_SNAPSHOT_DOC=$doc PIXELLAB_SNAPSHOT_APPEARANCE=$appearance
[[ -n $zoom ]] && export PIXELLAB_SNAPSHOT_ZOOM=$zoom

"$app_bin" &
pid=$!
# The app quits on its own (within 60 s); don't let a hang outlive us.
for _ in {1..90}; do
  kill -0 $pid 2>/dev/null || break
  sleep 1
done
if kill -0 $pid 2>/dev/null; then
  kill $pid
  echo "snapshot: the app didn't quit; killed it" >&2
  exit 1
fi
wait $pid
