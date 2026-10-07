# Native macOS bilingual IME experiment

- User authorized moving the existing prototype into a real input method. macOS is the first local test platform. Keep upstream pristine and preserve the browser prototype.
- Swift/AppKit/InputMethodKit shell, existing Rust bridge and local datasets. No cloud requests, chronological input logs, context harvesting or other input-method settings reads. The user-authorized local personal vocabulary stores only confirmed Chinese words, Pinyin, usage counts and recency; never load it in tests/previews.
- Pheno v1.4 applies to this native UI, with native system fonts as fallback and no borrowed Qingjian brand assets. Normal typing has no animation. Candidate buttons must not activate the IME or steal the host editor's focus.
- Installation is user-local under ~/Library/Input Methods/BilingualCompanion.app. Never overwrite an unrelated app or automatically log out/restart. Restore the previously selected input source after agent testing.
- Sentence translation uses Apple Translation on macOS 26+ with installed zh-Hans/en languages. Resource preparation is an explicit separate SwiftUI helper; never send input to a cloud API. Invalidate asynchronous results on composition/candidate changes and cancellation. Keep bridge English validation strict; consume the verified Chinese candidate before submitting locally generated English.

## Build and checks

```sh
bash tools/build.sh
bash tools/check.sh
```

build.sh uses the parent local Rust runtime with Cargo.lock and existing Apple Command Line Tools. The result is build/BilingualCompanion.app. check.sh runs Swift formatting/lint, type checking, keyboard-state self-tests, real bridge checks and code-signing validation.

## Preview, install and register

```sh
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-paging
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-learning
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-sentence
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --setup-translation
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --sentence-integration-test
../runtime/python/bin/python tools/install.py
"$HOME/Library/Input Methods/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --register
```

Registration does not select the source. --select selects it for authorized testing; --sources lists enabled input sources; --select-id ID restores a known source. System settings may still require a fresh login to discover a newly added IME. This must be reported, not hidden by pretending preview-mode input proves system integration.

Sentence integration checks require downloaded languages; check.sh runs cancellation tests without them. Do not run two normal IMK servers with the same connection name. CUA's targeted key delivery did not invoke either this IME or Apple's Pinyin in TextEdit on this machine; do not count that pathway as physical-keyboard acceptance.

Paging uses absolute engine indices internally and local 1–5 slot numbers per page. Never pass a local slot directly to commit. Page changes invalidate sentence translation and expanded senses; first/last page boundaries preserve state. Engine candidates are capped at64, keep remaining Pinyin and English validation intact.

Learning requires a valid context-bound commit receipt confirmed after host insertText. Cancel, raw submission and edits discard pending receipts/composition chains. English output learns the underlying Chinese word but never joins an English output into a composed new word. Personal vocabulary lives outside the repository in ~/Library/Application Support/BilingualCompanion/PersonalVocabulary/words.json. Keep malformed files intact. Memory integration tests use temporary stores; --preview-learning seeds and reloads its own temporary store. Pause is session-only and does not erase existing vocabulary.
