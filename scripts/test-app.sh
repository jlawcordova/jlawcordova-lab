#!/bin/zsh
# Runs the app's unit tests (Swift Testing, app/Tests/PixelLabTests) with the
# Command Line Tools: their TestingMacros plugin isn't on the default plugin
# path, so name it.
#
#   scripts/test-app.sh
set -euo pipefail
root=${0:A:h:h}
plugins=/Library/Developer/CommandLineTools/usr/lib/swift/host/plugins/testing
args=()
[[ -d $plugins ]] && args=(-Xswiftc -plugin-path -Xswiftc $plugins)
swift test --package-path "$root/app" "${args[@]}"
