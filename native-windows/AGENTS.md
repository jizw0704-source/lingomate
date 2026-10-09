# Windows native input method

- Rust TSF DLL and the existing line-JSON engine bridge; preserve the upstream checkout and both Cargo.lock files. Windows API bindings are required only on Windows. No host text, clipboard, input logs, automatic network translation or account access.
- Current target: Windows x64 desktop text applications. Keep standard COM object lifetimes, keyboard-disabled checks, asynchronous edit sessions, composition termination, focus changes and bounded bridge failure handling. Never replay commits or synthesize host/system keys.
- Pheno v1.4 applies to the candidate panel: neutral surfaces, native Chinese font fallback, compact paired candidates, >=44px choices, no animation or focus stealing. Do not claim actual Windows visual, DPI or accessibility acceptance from macOS checks.
- Installer touches only the owned CLSID and immutable LingoMate version paths. Preserve previous registration and files; never stop hosts, change other input methods, select a source, restart ctfmon or reboot automatically. Do not register TSF in CI. No generated dictionaries, packages or credentials in Git.

## Windows setup, build, checks, install

Run from the repository root using 64-bit PowerShell and Rust/MSVC build tools:

```powershell
./native-windows/tools/setup.ps1
./native-windows/tools/build.ps1
./native-windows/tools/check.ps1
./native-windows/tools/install.ps1
./native-windows/tools/uninstall.ps1
```

Setup pins Rust 1.96.0 and the existing upstream commit. Build uses locked dependencies. Check parses all PowerShell scripts, checks Rust format/Clippy, runs real-engine tests and loads the DLL factory without registration.

## macOS development checks

With the existing local Rust runtime and resources:

```sh
bash native-windows/tools/cargo.sh fmt -- --check
bash native-windows/tools/cargo.sh clippy --locked --all-targets --target x86_64-pc-windows-gnu -- -D warnings
bash prototype/tools/cargo.sh check --locked --target x86_64-pc-windows-gnu
LINGOMATE_TEST_BRIDGE="$PWD/upstream/qingjian/target/release/bilingual-ime-bridge" LINGOMATE_TEST_RESOURCES="$PWD" bash native-windows/tools/cargo.sh test --locked
runtime/python/bin/ruff format --check native-windows/tools
runtime/python/bin/ruff check native-windows/tools
```

Install the Windows Rust standard target into the local runtime before cross-checking. macOS cannot link MSVC, register TSF or validate Windows host keyboard input. Keep README/QA, root status and Obsidian mirrors explicit about those limits.
