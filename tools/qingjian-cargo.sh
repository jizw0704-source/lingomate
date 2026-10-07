#!/usr/bin/env bash
set -euo pipefail
RESEARCH_DIR="$(cd "$(dirname "$0")/.." && pwd)"
export CARGO_HOME="$RESEARCH_DIR/runtime/cargo"
export RUSTUP_HOME="$RESEARCH_DIR/runtime/rustup"
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
export SDKROOT="$DEVELOPER_DIR/SDKs/MacOSX.sdk"
export CC="$DEVELOPER_DIR/usr/bin/clang"
export CXX="$DEVELOPER_DIR/usr/bin/clang++"
export MACOSX_DEPLOYMENT_TARGET=13.0
export PATH="$CARGO_HOME/bin:$DEVELOPER_DIR/usr/bin:/opt/homebrew/bin:/usr/bin:/bin"
cd "$RESEARCH_DIR/upstream/qingjian"
exec cargo "$@"
