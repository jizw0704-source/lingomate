#!/usr/bin/env bash
set -euo pipefail
PROJECT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
export CARGO_HOME="$PROJECT_DIR/runtime/cargo"
export RUSTUP_HOME="$PROJECT_DIR/runtime/rustup"
export PATH="$CARGO_HOME/bin:/opt/homebrew/bin:/usr/bin:/bin"
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
export CARGO_TARGET_DIR="$PROJECT_DIR/native-windows/target"
cd "$PROJECT_DIR/native-windows"
exec cargo "$@"
