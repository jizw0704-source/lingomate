#!/usr/bin/env bash
set -euo pipefail
NATIVE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
APP="$NATIVE_DIR/build/BilingualCompanion.app"
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
export SDKROOT="$DEVELOPER_DIR/SDKs/MacOSX.sdk"
codesign --verify --deep --strict "$APP"
VERSION=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist")
OUTPUT="$NATIVE_DIR/build/packages"
mkdir -p "$OUTPUT"
DMG="$OUTPUT/lingomate-macos-arm64-$VERSION.dmg"
if [[ -e "$DMG" ]]; then
  echo "Output exists; preserved: $DMG" >&2
  exit 1
fi
STAGE=$(mktemp -d "$OUTPUT/.dmg-stage.XXXXXX")
trap 'rm -rf "$STAGE"' EXIT
INSTALLER="$STAGE/安装灵果.app"
mkdir -p "$INSTALLER/Contents/MacOS" "$INSTALLER/Contents/Resources"
"$DEVELOPER_DIR/usr/bin/swiftc" -swift-version 5 -warnings-as-errors -O -sdk "$SDKROOT" \
  -target arm64-apple-macosx13.0 "$NATIVE_DIR"/Installer/*.swift -framework AppKit \
  -o "$INSTALLER/Contents/MacOS/LingoMateInstaller"
"$INSTALLER/Contents/MacOS/LingoMateInstaller" --selftest
ditto "$APP" "$INSTALLER/Contents/Resources/BilingualCompanion.app"
cp "$NATIVE_DIR/packaging/安装与试用说明.txt" "$STAGE/安装与试用说明.txt"
cat > "$INSTALLER/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>org.local.bilingualcompanion.installer</string>
<key>CFBundleExecutable</key><string>LingoMateInstaller</string>
<key>CFBundleName</key><string>安装灵果</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$VERSION</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
codesign --force --sign - "$INSTALLER"
codesign --verify --deep --strict "$INSTALLER"
hdiutil create -volname "灵果 $VERSION" -srcfolder "$STAGE" -format UDZO "$DMG"
hdiutil verify "$DMG"
shasum -a 256 "$DMG" > "$DMG.sha256"
echo "Local test image: $DMG"
echo 'Not uploaded. Ad-hoc signed only; Developer ID signing, notarization and data distribution review remain pending.'
