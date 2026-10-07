#!/usr/bin/env bash
# Reproduce the pinned research inputs without changing existing checkouts.
set -euo pipefail
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
for command_name in git curl uv pnpm; do
  command -v "$command_name" >/dev/null || { echo "Required command: $command_name" >&2; exit 1; }
done
if [[ "$(uname -s)" != Darwin || "$(uname -m)" != arm64 ]]; then
  echo 'This setup currently supports Apple Silicon macOS only.' >&2
  exit 1
fi
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
[[ -x "$DEVELOPER_DIR/usr/bin/swiftc" ]] || { echo 'Install Apple Command Line Tools first.' >&2; exit 1; }
export CARGO_HOME="$PROJECT_DIR/runtime/cargo"
export RUSTUP_HOME="$PROJECT_DIR/runtime/rustup"
mkdir -p "$PROJECT_DIR/runtime" "$PROJECT_DIR/upstream"
if [[ ! -x "$CARGO_HOME/bin/rustup" ]]; then
  installer="$PROJECT_DIR/runtime/rustup-init"
  curl --fail --location --proto '=https' --tlsv1.2 \
    https://static.rust-lang.org/rustup/dist/aarch64-apple-darwin/rustup-init -o "$installer"
  chmod +x "$installer"
  "$installer" -y --profile minimal --default-toolchain 1.96.0 --no-modify-path
fi
"$CARGO_HOME/bin/rustup" toolchain install 1.96.0 --profile minimal --component rustfmt --component clippy
"$CARGO_HOME/bin/rustup" default 1.96.0
ensure_checkout() {
  local name="$1" url="$2" commit="$3" checkout="$PROJECT_DIR/upstream/$1"
  if [[ -e "$checkout" ]]; then
    local current
    current="$(git -C "$checkout" rev-parse HEAD)"
    [[ "$current" == "$commit" ]] || { echo "Pinned commit mismatch: $name; existing files preserved." >&2; exit 1; }
    [[ -z "$(git -C "$checkout" status --porcelain)" ]] || { echo "Local edits in $name; existing files preserved." >&2; exit 1; }
  else
    git clone --filter=blob:none --no-checkout "$url" "$checkout"
    git -C "$checkout" fetch --depth=1 origin "$commit"
    git -C "$checkout" checkout --detach "$commit"
  fi
}
ensure_checkout qingjian https://github.com/qingjian-team/qingjian.git c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e
ensure_checkout rime-translate https://github.com/daocatt/rime-translate.git c5ffa0ed2fa2a5530a39929fc356dadd4cea161c
if [[ ! -f "$PROJECT_DIR/runtime/python/pyvenv.cfg" ]]; then
  uv venv --python 3.13 "$PROJECT_DIR/runtime/python"
fi
uv pip install --python "$PROJECT_DIR/runtime/python/bin/python" -r "$PROJECT_DIR/requirements-dev.txt"
pnpm --dir "$PROJECT_DIR/prototype" install --frozen-lockfile
echo 'Setup complete. Build: bash native-macos/tools/build.sh'
