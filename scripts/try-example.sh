#!/bin/zsh
# Opens Pixel Lab on a temp copy of one of the example documents (examples/), so
# you can try scene layers without touching the examples in the repo, your own
# document or the installed app's proposals. Quit Pixel Lab to end.
#
#   scripts/try-example.sh [harbor|town]      (default: harbor)
#
# Needs build/Pixel Lab.app (scripts/build-app.sh).
set -euo pipefail

root=${0:A:h:h}
name=${1:-harbor}
src="$root/examples/$name.pixellab"
if [[ ! -d $src ]]; then
  echo "no example called $name; the examples are: ${(j:, :)${(@)${(f)$(ls "$root/examples" | grep '\.pixellab$')}%.pixellab}}" >&2
  exit 2
fi
app="$root/build/Pixel Lab.app/Contents/MacOS/PixelLab"
[[ -x $app ]] || { echo "no build/Pixel Lab.app: run scripts/build-app.sh" >&2; exit 1; }

tmp=$(mktemp -d -t pixellab-example)
cp -R "$src" "$tmp/"
mkdir -p "$tmp/state"
echo "Pixel Lab on a copy of examples/$name.pixellab: $tmp/$name.pixellab"
echo "Saves go to the copy. Quit Pixel Lab to end."
PIXELLAB_DOC="$tmp/$name.pixellab" PIXELLAB_STATE="$tmp/state" "$app"
