#!/bin/bash
# Builds one target's program and packages it for a release (spec R17, R27):
#
#   scripts/package-program.sh <target> <out-dir>
#
# <target> is aarch64-apple-darwin or x86_64-unknown-linux-gnu. It runs
# `cargo build --release --target <target>` (PIXELLAB_VERSION, when set, is
# the version the program reports), writes <out-dir>/pixellab-<os>-<arch>.tar.gz
# holding the one executable, and adds its line to <out-dir>/SHA256SUMS
# (replacing an earlier one for the same file). The release runs it once per
# target; it runs locally the same way.
set -euo pipefail

if [ $# -ne 2 ]; then
  echo "usage: scripts/package-program.sh <aarch64-apple-darwin|x86_64-unknown-linux-gnu> <out-dir>" >&2
  exit 2
fi
target="$1"
out="$2"
case "$target" in
  aarch64-apple-darwin) name="pixellab-darwin-arm64" ;;
  x86_64-unknown-linux-gnu) name="pixellab-linux-x64" ;;
  *) echo "package-program: no release name for target $target" >&2; exit 2 ;;
esac

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"
export PATH="$HOME/.cargo/bin:$PATH"
rustup target add "$target" >/dev/null 2>&1 || true
cargo build --release --locked --target "$target" -p pixellab
program="target/$target/release/pixellab"
[ -x "$program" ] || { echo "package-program: $program wasn't built" >&2; exit 1; }

# On its own target, the program must say the version it was built with.
host="$(rustc -vV | sed -n 's/^host: //p')"
if [ "$host" = "$target" ] && [ -n "${PIXELLAB_VERSION:-}" ]; then
  said="$("$program" --version)"
  [ "$said" = "pixellab $PIXELLAB_VERSION" ] || { echo "package-program: the program says '$said', not 'pixellab $PIXELLAB_VERSION'" >&2; exit 1; }
fi

mkdir -p "$out"
out="$(cd "$out" && pwd)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
cp "$program" "$stage/pixellab"
tar -C "$stage" -czf "$out/$name.tar.gz" pixellab

cd "$out"
if command -v sha256sum >/dev/null 2>&1; then sum="$(sha256sum "$name.tar.gz")"; else sum="$(shasum -a 256 "$name.tar.gz")"; fi
touch SHA256SUMS
grep -v "  $name.tar.gz\$" SHA256SUMS > SHA256SUMS.tmp || true
echo "$sum" >> SHA256SUMS.tmp
sort -k2 SHA256SUMS.tmp > SHA256SUMS
rm -f SHA256SUMS.tmp
echo "packaged $out/$name.tar.gz"
