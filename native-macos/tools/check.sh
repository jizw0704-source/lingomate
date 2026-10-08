#!/usr/bin/env bash
set -euo pipefail
NATIVE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RESEARCH_DIR="$(cd "$NATIVE_DIR/.." && pwd)"
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
export SDKROOT="$DEVELOPER_DIR/SDKs/MacOSX.sdk"
"$DEVELOPER_DIR/usr/bin/swift-format" lint --strict --recursive "$NATIVE_DIR/Sources"
"$DEVELOPER_DIR/usr/bin/swiftc" -swift-version 5 -warnings-as-errors -typecheck -sdk "$SDKROOT" \
  -target arm64-apple-macosx13.0 "$NATIVE_DIR"/Sources/*.swift \
  -framework AppKit -framework InputMethodKit -framework Carbon -framework Translation -framework SwiftUI
"$RESEARCH_DIR/runtime/python/bin/ruff" format --check "$NATIVE_DIR/tools"
"$RESEARCH_DIR/runtime/python/bin/ruff" check "$NATIVE_DIR/tools"
"$RESEARCH_DIR/runtime/python/bin/python" -m compileall -q "$NATIVE_DIR/tools"
bash -n "$NATIVE_DIR/tools/build.sh" "$NATIVE_DIR/tools/check.sh"
bash "$RESEARCH_DIR/prototype/tools/cargo.sh" fmt --all -- --check
bash "$RESEARCH_DIR/prototype/tools/cargo.sh" clippy --release --locked --all-targets -- -D warnings
bash "$RESEARCH_DIR/prototype/tools/cargo.sh" test --release --locked
"$RESEARCH_DIR/runtime/python/bin/python" "$RESEARCH_DIR/prototype/tests/engine_test.py"
"$RESEARCH_DIR/runtime/python/bin/python" "$RESEARCH_DIR/prototype/tests/memory_test.py"
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --appearance-test
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --selftest
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --punctuation-test
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --typing-test
"$NATIVE_DIR/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --sentence-state-test
plutil -lint "$NATIVE_DIR/build/BilingualCompanion.app/Contents/Info.plist"
codesign --verify --deep --strict "$NATIVE_DIR/build/BilingualCompanion.app"
