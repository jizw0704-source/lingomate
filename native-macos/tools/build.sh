#!/usr/bin/env bash
set -euo pipefail
NATIVE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RESEARCH_DIR="$(cd "$NATIVE_DIR/.." && pwd)"
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
export SDKROOT="$DEVELOPER_DIR/SDKs/MacOSX.sdk"
export MACOSX_DEPLOYMENT_TARGET=13.0
APP="$NATIVE_DIR/build/BilingualCompanion.app"
bash "$RESEARCH_DIR/prototype/tools/cargo.sh" build --release --locked
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/zh-Hans.lproj" "$APP/Contents/Resources/en.lproj"
"$DEVELOPER_DIR/usr/bin/swiftc" -swift-version 5 -warnings-as-errors -O -sdk "$SDKROOT" -target arm64-apple-macosx13.0 \
  "$NATIVE_DIR"/Sources/*.swift "$NATIVE_DIR/Installer/InstallCore.swift" -framework AppKit -framework InputMethodKit -framework Carbon -Xlinker -weak_framework -Xlinker Translation -framework SwiftUI \
  -o "$APP/Contents/MacOS/BilingualCompanion"
cp "$NATIVE_DIR/resources/Info.plist" "$APP/Contents/Info.plist"
"$DEVELOPER_DIR/usr/bin/swiftc" -swift-version 5 -warnings-as-errors -sdk "$SDKROOT" \
  "$NATIVE_DIR/tools/make_input_icon.swift" -o "$NATIVE_DIR/build/make-input-icon"
"$NATIVE_DIR/build/make-input-icon" "$NATIVE_DIR/build/input-icons"
cp "$NATIVE_DIR/build/input-icons/GuoMenuTemplate.tiff" "$NATIVE_DIR/build/input-icons/GuoMenuSelected.tiff" "$APP/Contents/Resources/"
iconutil --convert icns "$NATIVE_DIR/build/input-icons/GuoApp.iconset" --output "$APP/Contents/Resources/GuoApp.icns"
cp "$RESEARCH_DIR/upstream/qingjian/target/release/bilingual-ime-bridge" "$APP/Contents/Resources/"
cp "$RESEARCH_DIR/upstream/qingjian/assets/lexicon/dict.tsv" "$APP/Contents/Resources/"
cp "$RESEARCH_DIR/upstream/qingjian/assets/glossary/glossary-en.tsv" "$APP/Contents/Resources/"
cp "$RESEARCH_DIR/prototype/data/details.json" "$APP/Contents/Resources/"
cp "$RESEARCH_DIR/prototype/LICENSE" "$APP/Contents/Resources/LICENSE"
cp "$RESEARCH_DIR/upstream/qingjian/assets/glossary/README.md" "$APP/Contents/Resources/GLOSSARY-NOTICE.md"
cat "$RESEARCH_DIR/upstream/qingjian/assets/lexicon/README.md" \
  "$RESEARCH_DIR/docs/licenses/LEXICON-ATTRIBUTION.txt" > "$APP/Contents/Resources/LEXICON-NOTICE.md"
cat > "$APP/Contents/Resources/zh-Hans.lproj/InfoPlist.strings" <<'STRINGS'
"CFBundleName" = "灵果";
"CFBundleDisplayName" = "灵果";
"org.local.bilingualcompanion.Hans" = "灵果";
STRINGS
cat > "$APP/Contents/Resources/en.lproj/InfoPlist.strings" <<'STRINGS'
"CFBundleName" = "LingoMate";
"CFBundleDisplayName" = "LingoMate";
"org.local.bilingualcompanion.Hans" = "LingoMate";
STRINGS
printf 'APPL????' > "$APP/Contents/PkgInfo"
xattr -cr "$APP"
codesign --force --sign - "$APP/Contents/Resources/bilingual-ime-bridge"
codesign --force --sign - "$APP"
codesign --verify --deep --strict "$APP"
echo "构建完成：$APP"
