# Windows native input method

- Rust TSF DLL loads the same-directory native core DLL through a background FIFO worker; retain the shared line-JSON CLI for other clients and regression checks. Preserve the upstream checkout and both Cargo.lock files. Windows API bindings are required only on Windows. Never read host text or clipboard, retain input logs, or access network/credentials in TSF keyboard callbacks. Optional translation and selected-word account access belong to the standalone, unpackaged desktop helper after explicit user enablement/configuration.
- All Windows development uses the single `codex/windows-dev` branch, with PR head `codex/windows-dev` and base `main`. Do not create extra Windows topic branches, push directly to `main` or merge without an explicit user request.
- Current target: Windows x64 desktop text applications. Keep standard COM object lifetimes, keyboard-disabled checks, asynchronous edit sessions, composition termination, focus changes and bounded worker failure handling. Expose the implemented extended activation interface; register only implemented capabilities, not immersive or UIElement support by declaration alone. Never replay commits or synthesize host/system keys.
- Pheno v1.4 applies to the candidate panel: neutral surfaces, native Chinese font fallback, compact paired candidates, >=44px choices, no animation or focus stealing. Do not claim actual Windows visual, DPI or accessibility acceptance from macOS checks.
- Installer touches only the owned CLSID and immutable LingoMate version paths. Preserve previous registration and files; never stop hosts, change other input methods, select a source, restart ctfmon or reboot automatically. Do not register TSF in CI. No generated dictionaries, packages or credentials in Git.
- Remote updates are user-authorized: use the fixed GitHub Windows preview source and a separate desktop process. Daily checking is opt-in, installation requires the user's confirmation, no input data is uploaded. Never run a downloaded installer script; the installed trusted core validates packages and registers only the owned DLL. Keep UTF-8 BOM for Chinese PowerShell sources consumed by Windows PowerShell 5.1. Do not publish packages until data distribution and real-machine acceptance are resolved.
- Windows feature/UI parity references the fixed Mac baseline in docs/WINDOWS-MAC-PARITY-IMPLEMENTATION-2026-10-13.md. Keep account, translation, input, appearance and learning in the independent settings helper; settings exchange only fixed enums/booleans and read back actual service state. API save never sends a test request. Online AI defaults OFF and receives only the exact current Chinese candidate plus a fixed instruction; no raw Pinyin, input history, personal vocabulary file or host context. Credentials use Windows Credential Manager in the helper only, scoped to the complete endpoint/account. Invalidate translations on input/candidate/page/context/configuration changes; validate candidate consumption and the remaining Pinyin through the real engine before a translated commit.
- Local sentence translation uses only loopback Ollama with cloud.disabled=true verified from /api/status and an already-installed local model with digest/size checks. Do not download models or silently route local requests to cloud. Account sync carries selected dictionary-word metadata only, never sentences; local learning remains independent. Packaged callers must relay into a verified unpackaged helper before personal configuration or credentials are accessed. Do not auto-merge old MSIX-private data.

## Windows setup, build, checks, install

Run from the repository root using 64-bit PowerShell and Rust/MSVC build tools:

```powershell
./native-windows/tools/setup.ps1
./native-windows/tools/build.ps1
./native-windows/tools/check.ps1
./native-windows/tools/install.ps1
./native-windows/tools/uninstall.ps1
./native-windows/tools/prepare-release.ps1
./native-windows/tools/build-installer.ps1
```

Build preparation also requires Python 3.10+ (standard library only); setup checks it. check.ps1 validates pinned CC-CEDICT conversion, official Google Books Ngram counts/permission, fixed WordNet POS/license and reproducible data before real-engine tests. Windows build/check default to --wordnet-in-memory to verify the official pinned archive's URL, size, SHA-256 and full license without rewriting an existing altered cache. Optional -WordNetSource <archive> selects --wordnet-source for an already-downloaded official archive and rechecks exactly the same pins/license; the two Python options are mutually exclusive. Never weaken provenance checks, replace the altered cache or load the legacy upstream word/gloss tables.

Setup pins Rust 1.96.0 and the existing upstream commit. Build uses locked dependencies. Check parses all PowerShell scripts, checks Rust format/Clippy, runs real-engine tests and loads the DLL factory without registration.

Check also runs `test_updates.ps1` under PowerShell 7 and Windows PowerShell 5.1, and renders `update.ps1 -SmokeTest -PreviewPath <temporary PNG>` without reading real installation or network state. `prepare-release.ps1` only creates ignored local release files; it does not publish. Actual registration, network release download and application reload require Windows 11 acceptance.

tools/test_helper_identity.ps1 and tests/support/identity_probe.rs are manual opt-in debugger fixtures only, not default checks or CI. Require exact metadata for a trusted already-installed package. Launch only OWN fixture processes and production helper --identity through Invoke-CommandInDesktopPackage; never register/remove packages, edit manifests, toggle Developer Mode/certificates, activate real app UI, select input sources, or read real configuration/credentials/input. A pass verifies the packaged-parent to unpackaged-relay identity chain under default/PreventBreakaway policies, not application keyboard, VFS, credentials, API or full grandchild cancellation behavior.

build-installer.ps1 requires separately installed Inno Setup 6; it validates the package and compiles packaging/lingomate.iss to an ignored local Setup EXE. No new runtime dependency. The CI initial-window check must never confirm installation or register TSF. Only generated screenshots may be uploaded; do not upload full installer/data artifacts until distribution review and real-machine acceptance are resolved.

The user requested GitHub installer uploads. The manual windows-package-draft.yml workflow may upload the verified EXE and checksum directly to an existing maintainer-only prerelease draft tied to the same source commit; never use public binary artifacts, publish the draft or change the update feed in that workflow. Public distribution and real-machine acceptance remain separate.

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

User authorized confirmed Chinese personal vocabulary and independent-character word memory, including heuristic unknown short words. Normal TSF Chinese memory uses USERPROFILE/.lingomate/PersonalVocabulary/words.json outside the MSIX AppData redirection boundary; tests use explicit temporary paths and default Bridge::start remains memory-disabled. The unpackaged helper copies the canonical old LOCALAPPDATA/LingoMate/PersonalVocabulary/words.json once only when the new path is absent, preserving the original; old MSIX-private histories are not auto-merged. Queued TSF receipts are FIFO, one-use, output/generation-bound and confirmed only after the whole host edit succeeds; retired scopes accept only their already-queued successful edits and never join the new scope. Cancel failures/edits/disabled contexts; keep worker and core modules loaded until their threads exit, and join retained stdio reader threads before DLL release. Pausing memory writes never deletes words. Chinese personal vocabulary never enters account sync or online translation requests.
