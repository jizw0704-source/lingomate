# Bilingual IME

- Platform maintainers: repository owner jizw0704-source leads macOS; Wright (LightningFlashEvE) leads Windows. Collaboration and exact setup/check commands: CONTRIBUTING.md. Shared bridge, protocol, vocabulary format and data changes need both-platform checks and explicit real-device verification limits.
- Platform-only changes are checked and accepted by the corresponding maintainer; routine review by the other platform is not required. Review shared changes at a stable PR commit, focusing on shared code and compatibility with the reviewing platform. Keep any required GitHub approvals and merge authorization separate from this review scope. Hand off stable Mac feature commits with dependencies and acceptance cases; Windows schedules implementation independently rather than following every Mac commit.
- All Windows development stays on the single `codex/windows-dev` branch; do not create additional Windows topic branches. Submit Windows changes through Pull Requests with head `codex/windows-dev` and base `main`; do not push Windows changes directly to `main` or merge their PRs without an explicit user request. Keep each PR focused, preserve unrelated uncommitted work, and sync the development branch with `origin/main` after a PR is merged.
- Mac-to-Windows feature handoffs follow CONTRIBUTING.md and `.github/ISSUE_TEMPLATE/macos-to-windows.md`. Link the Mac reference PR/commit and shared acceptance cases; record Mac completion, Windows implementation and Windows real-device verification separately. Use `Refs` until real-device acceptance is complete; do not auto-close pending handoffs when code is merged.
- Windows native development is user-authorized in `native-windows/`; follow its instructions, preserve macOS behavior and pinned upstream sources.
- Native macOS implementation is user-authorized in `native-macos/`; retain the browser prototype in `prototype/`. Follow each directory's instructions.
- Release data is generated only by tools/prepare_lexicon.py from data/source.json (CC-CEDICT CC BY-SA 4.0). Keep source snapshots/generated tables ignored; never fall back to upstream mixed lexicon or glossary. Native builds prepare data automatically. frequency-source.json pins the official Google Books Ngram export and permission; only exact existing word counts enter ranking. ranking.json is independently authored CC BY-SA 4.0 prior data. Preserve both provenance records and packaged notices. data/pos-source.json pins WordNet 3.0; exact matching adds display-only POS comments and the full WordNet license to glossary/NOTICE files. Unknown senses remain untagged; never append POS to committed text.
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
bash native-macos/tools/package.sh
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
runtime/python/bin/python tools/test_prepare_lexicon.py
runtime/python/bin/python tools/test_frequency.py
runtime/python/bin/python tools/test_part_of_speech.py
runtime/python/bin/python prototype/tests/ranking_test.py
runtime/python/bin/python tools/prepare_lexicon.py --check
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
