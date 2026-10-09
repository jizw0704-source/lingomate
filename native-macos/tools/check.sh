#!/usr/bin/env bash
set -euo pipefail
NATIVE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RESEARCH_DIR="$(cd "$NATIVE_DIR/.." && pwd)"
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
export SDKROOT="$DEVELOPER_DIR/SDKs/MacOSX.sdk"
"$DEVELOPER_DIR/usr/bin/swift-format" lint --strict --recursive "$NATIVE_DIR/Sources"
"$DEVELOPER_DIR/usr/bin/swift-format" lint --strict "$NATIVE_DIR/tools/make_input_icon.swift"
"$DEVELOPER_DIR/usr/bin/swift-format" lint --strict --recursive "$NATIVE_DIR/Installer"
"$DEVELOPER_DIR/usr/bin/swiftc" -swift-version 5 -warnings-as-errors -sdk "$SDKROOT" \
  -target arm64-apple-macosx13.0 "$NATIVE_DIR"/Installer/*.swift -framework AppKit \
  -o "$NATIVE_DIR/build/LingoMateInstaller"
"$NATIVE_DIR/build/LingoMateInstaller" --selftest
"$DEVELOPER_DIR/usr/bin/swiftc" -swift-version 5 -warnings-as-errors -typecheck -sdk "$SDKROOT" \
  -target arm64-apple-macosx13.0 "$NATIVE_DIR"/Sources/*.swift \
  -framework AppKit -framework InputMethodKit -framework Carbon -framework Translation -framework SwiftUI
"$RESEARCH_DIR/runtime/python/bin/ruff" format --check "$NATIVE_DIR/tools"
"$RESEARCH_DIR/runtime/python/bin/ruff" check "$NATIVE_DIR/tools"
"$RESEARCH_DIR/runtime/python/bin/python" -m compileall -q "$NATIVE_DIR/tools"
"$RESEARCH_DIR/runtime/python/bin/python" "$NATIVE_DIR/tools/test_install.py"
bash -n "$NATIVE_DIR/tools/build.sh" "$NATIVE_DIR/tools/check.sh" "$NATIVE_DIR/tools/package.sh"
bash "$RESEARCH_DIR/prototype/tools/cargo.sh" fmt --all -- --check
bash "$RESEARCH_DIR/prototype/tools/cargo.sh" clippy --release --locked --all-targets -- -D warnings
bash "$RESEARCH_DIR/prototype/tools/cargo.sh" test --release --locked
"$RESEARCH_DIR/runtime/python/bin/python" "$RESEARCH_DIR/prototype/tests/engine_test.py"
"$RESEARCH_DIR/runtime/python/bin/python" "$RESEARCH_DIR/prototype/tests/memory_test.py"
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --appearance-test
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --account-test
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --ai-test
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --input-diagnostic-test
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --service-lock-test
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --input-window-test
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --settings-test
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --engine-resilience-test
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --selftest
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --punctuation-test
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --typing-test
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --sentence-state-test
plutil -lint "$NATIVE_DIR/build/BilingualCompanion.app/Contents/Info.plist"
codesign --verify --deep --strict "$NATIVE_DIR/build/BilingualCompanion.app"
