# Bilingual IME

- Native macOS implementation is user-authorized in `native-macos/`; retain the browser prototype in `prototype/`. Follow each directory's instructions.
- Keep pinned upstream checkouts pristine. Do not commit runtime, generated evidence, applications, personal configuration, input content or credentials. Preserve Cargo.lock and pnpm-lock.yaml.
- UI follows the installed Pheno v1.4 skill when available. Keep existing neutral native styling, system-font fallback and no typing animation; do not copy upstream branding.
- User authorized self-hosted email OTP and sync of selected word metadata only. The user also authorized optional MiniMax sentence translation: default OFF, only the current Chinese candidate may be sent to the explicitly configured endpoint after user enablement; never upload raw Pinyin, personal vocabulary, host context or input history. Account sync never uploads sentences. No clipboard reads or context harvesting. Keep local Apple translation available and API keys in Keychain, accessed only by settings/translation helper processes. Invalidate results on input/candidate/page changes. Always validate candidates and consume remaining Pinyin through the real engine.
- System installation is only within the user's authorized native testing scope. Preserve old app versions and restore the input source selected before testing. Never auto-log out or reboot.
- Keep README, native usage and QA, docs/PROGRESS.md and Obsidian mirrors consistent. State actual verification limits; do not count preview success as real keyboard acceptance.

## Setup

```sh
bash tools/setup.sh
```

Supports Apple Silicon macOS, requires Command Line Tools, uv and pnpm. Pins Rust1.96.0 and upstream commits. Existing mismatched or edited upstream directories are preserved and cause setup to stop.

## Build, run, check

```sh
bash native-macos/tools/build.sh
bash native-macos/tools/check.sh
native-macos/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-paging
runtime/python/bin/ruff format --check tools prototype/tools prototype/data prototype/tests native-macos/tools
runtime/python/bin/ruff check tools prototype/tools prototype/data prototype/tests native-macos/tools
bash -n tools/setup.sh tools/qingjian-cargo.sh
pnpm --dir prototype format:check
pnpm --dir prototype lint
pnpm --dir prototype typecheck
pnpm --dir prototype test
pnpm --dir prototype build
pnpm --dir prototype serve
runtime/python/bin/python prototype/tests/http_test.py
runtime/python/bin/python tools/test_sync_obsidian.py
runtime/python/bin/ruff format --check backend
runtime/python/bin/ruff check backend
runtime/python/bin/python -m unittest discover -s backend -v
runtime/python/bin/python -m compileall -q backend
```

HTTP tests need the local service. Sentence integration needs installed Chinese/English languages; see native instructions. Root Rust research commands use `tools/qingjian-cargo.sh`, fixtures disable logging/prediction. History and pinned dependencies: RESEARCH.md.

## Document mirror

```sh
runtime/python/bin/python tools/sync_obsidian.py --vault '/path/to/Obsidian Vault'
runtime/python/bin/python tools/sync_obsidian.py --vault '/path/to/Obsidian Vault' --check
```

Sync only project notes under 10 项目/中英输入法. The script preserves manual edits and does not create an automation.
