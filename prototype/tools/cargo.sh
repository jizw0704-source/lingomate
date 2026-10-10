#!/usr/bin/env bash
set -euo pipefail
PROTOTYPE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RESEARCH_DIR="$(cd "$PROTOTYPE_DIR/.." && pwd)"
export CARGO_HOME="$RESEARCH_DIR/runtime/cargo"
export RUSTUP_HOME="$RESEARCH_DIR/runtime/rustup"
export CARGO_TARGET_DIR="$RESEARCH_DIR/upstream/qingjian/target"
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
export SDKROOT="$DEVELOPER_DIR/SDKs/MacOSX.sdk"
export CC="$DEVELOPER_DIR/usr/bin/clang"
export CXX="$DEVELOPER_DIR/usr/bin/clang++"
export MACOSX_DEPLOYMENT_TARGET=13.0
export PATH="$CARGO_HOME/bin:$DEVELOPER_DIR/usr/bin:/opt/homebrew/bin:/usr/bin:/bin"
if [[ "${1:-}" == build ]]; then
  "$RESEARCH_DIR/runtime/python/bin/python" "$RESEARCH_DIR/tools/prepare_lexicon.py"
fi
cd "$PROTOTYPE_DIR/bridge"
exec cargo "$@"
