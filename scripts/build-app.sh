#!/bin/zsh
# Builds "build/Pixel Lab.app": a release build of app/, Info.plist, the icon
# (AppIcon.icns from app/Sources/PixelLab/Resources/AppIcon.png), the pixellab
# program (cargo build --release) in Contents/Helpers/pixellab, the SwiftPM
# resource bundle, and an ad-hoc signature. Idempotent: it rebuilds the bundle
# from scratch each time. (The program is not in Contents/MacOS/: on a
# case-insensitive volume that name is the app's own executable, PixelLab.)
#
#   scripts/build-app.sh                       version from git describe (v0.4.0-3-gabc1234 → 0.4.0-3-gabc1234)
#   PIXELLAB_VERSION=0.4.0 scripts/build-app.sh   the release workflow's: the tag's version
#
# The version is stamped into the program (PIXELLAB_VERSION at its build, which
# `pixellab --version` and the MCP server report) and Info.plist, never into the
# repo. With no tag to describe, the bundle says 0.0.0-dev.
#
# Works with the Command Line Tools; set DEVELOPER_DIR for full Xcode if needed.
set -euo pipefail

root=${0:A:h:h}
app="$root/build/Pixel Lab.app"
icon_png="$root/app/Sources/PixelLab/Resources/AppIcon.png"
version=${PIXELLAB_VERSION:-}
if [[ -z $version ]]; then
  version=$(git -C "$root" describe --tags --match 'v[0-9]*' 2>/dev/null || true)
  version=${version#v}
fi
version=${version:-0.0.0-dev}
# Info.plist takes the numeric part: 0.4.0-3-gabc1234 → 0.4.0.
plist_version=${version%%-*}
echo "==> version $version"

echo "==> cargo build --release"
export PATH="$HOME/.cargo/bin:$PATH"
PIXELLAB_VERSION=$version cargo build --release --manifest-path "$root/Cargo.toml"

echo "==> swift build -c release"
swift build -c release --package-path "$root/app"
bin_dir=$(swift build -c release --package-path "$root/app" --show-bin-path)

echo "==> assembling $app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin_dir/PixelLab" "$app/Contents/MacOS/PixelLab"

# SwiftPM resource bundles (fonts, the icon PNG) go in Contents/Resources,
# where the app looks for them (AppResources in Theme.swift).
for bundle in "$bin_dir"/*.bundle(N); do
  cp -R "$bundle" "$app/Contents/Resources/"
done

# The program: the same one the CLI and the MCP server run.
mkdir -p "$app/Contents/Helpers"
cp "$root/target/release/pixellab" "$app/Contents/Helpers/pixellab"

echo "==> icon"
iconset=$(mktemp -d)/AppIcon.iconset
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z $size $size "$icon_png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) "$icon_png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/AppIcon.icns"
rm -rf "${iconset:h}"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>PixelLab</string>
  <key>CFBundleIdentifier</key><string>com.jlawcordova.pixellab</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>Pixel Lab</string>
  <key>CFBundleDisplayName</key><string>Pixel Lab</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$plist_version</string>
  <key>CFBundleVersion</key><string>$plist_version</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>LSMinimumSystemVersion</key><string>15.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.graphics-design</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
  <key>NSSupportsAutomaticTermination</key><false/>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>Pixel Lab Document</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Owner</string>
      <key>LSTypeIsPackage</key><true/>
      <key>LSItemContentTypes</key>
      <array><string>com.jlawcordova.pixellab.document</string></array>
    </dict>
  </array>
  <key>UTExportedTypeDeclarations</key>
  <array>
    <dict>
      <key>UTTypeIdentifier</key><string>com.jlawcordova.pixellab.document</string>
      <key>UTTypeDescription</key><string>Pixel Lab Document</string>
      <key>UTTypeConformsTo</key>
      <array><string>com.apple.package</string></array>
      <key>UTTypeTagSpecification</key>
      <dict>
        <key>public.filename-extension</key>
        <array><string>pixellab</string></array>
      </dict>
    </dict>
  </array>
</dict>
</plist>
PLIST
plutil -lint "$app/Contents/Info.plist"

echo "==> codesign (ad hoc)"
codesign --force --deep -s - "$app"
codesign --verify --deep --strict "$app"
echo "built $app"
