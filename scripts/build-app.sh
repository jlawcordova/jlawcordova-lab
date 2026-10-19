#!/bin/zsh
# Builds "build/Pixel Lab.app": a release build of app/, Info.plist, the icon
# (AppIcon.icns from app/Sources/PixelLab/Resources/AppIcon.png), the CLI in
# Contents/Resources/cli (without tests), the SwiftPM resource bundle, and an
# ad-hoc signature. Idempotent: it rebuilds the bundle from scratch each time.
#
#   scripts/build-app.sh
#
# Works with the Command Line Tools; set DEVELOPER_DIR for full Xcode if needed.
set -euo pipefail

root=${0:A:h:h}
app="$root/build/Pixel Lab.app"
icon_png="$root/app/Sources/PixelLab/Resources/AppIcon.png"
version=$(grep -o "VERSION = '[^']*'" "$root/cli/lib/server-stdio.mjs" | cut -d"'" -f2)
version=${version:-0.0.0}

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

# The CLI, without its tests.
rsync -a --delete --exclude 'test/' --exclude '.DS_Store' "$root/cli/" "$app/Contents/Resources/cli/"
# The CLI is an ES module package: keep "type": "module" next to it.
cp "$root/package.json" "$app/Contents/Resources/package.json"

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
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
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
