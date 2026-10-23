#!/bin/zsh
# Installs Pixel Lab for the current user: builds "build/Pixel Lab.app" and
# copies it to ~/Applications (created if missing; no sudo, never
# /Applications). Then prints the commands that connect Claude Code to the
# installed app's bundled program. It doesn't run them.
#
#   scripts/install.sh                 build, then install into ~/Applications
#   scripts/install.sh --no-build      install the existing build/Pixel Lab.app
#   scripts/install.sh --dest <dir>    install into <dir> instead (for testing)
set -euo pipefail

root=${0:A:h:h}
dest=$HOME/Applications
build=1
while (( $# )); do
  case $1 in
    --dest) dest=${2:?--dest needs a folder}; shift 2 ;;
    --no-build) build=0; shift ;;
    -h|--help) sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "install.sh: unknown option $1 (try --help)" >&2; exit 2 ;;
  esac
done
[[ $dest = /* ]] || dest="$PWD/$dest"
if [[ ${dest:A} = /Applications ]]; then
  echo "install.sh: not installing into /Applications; use ~/Applications (the default)" >&2
  exit 2
fi

built="$root/build/Pixel Lab.app"
if (( build )); then
  "$root/scripts/build-app.sh"
fi
[[ -d $built ]] || { echo "install.sh: no $built; run without --no-build" >&2; exit 1; }

target="$dest/Pixel Lab.app"
if pgrep -f "$target/Contents/MacOS/PixelLab" >/dev/null 2>&1; then
  echo "install.sh: Pixel Lab is running from $target; quit it first" >&2
  exit 1
fi

echo "==> installing into $dest"
mkdir -p "$dest"
rm -rf "$target"
ditto "$built" "$target"
codesign --verify --deep --strict "$target"

program="$target/Contents/Helpers/pixellab"
cat <<EOF

Installed $target

Open it from Finder or Spotlight, or:
  open "$target"
It's signed ad hoc, so the first launch may ask you to confirm: if macOS says
it can't check the app, Control-click it in Finder › Open, or allow it under
System Settings › Privacy & Security.

Connect Claude Code (MCP; an agent's edits apply as it goes, and File › Revert To › Browse All Versions… takes any back):
  claude mcp add pixellab -- "$program" serve --mcp

Optional: the program as a shell command (add to ~/.zshrc):
  alias pixellab='"$program"'
EOF
