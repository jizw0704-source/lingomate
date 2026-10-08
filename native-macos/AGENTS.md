# Native macOS bilingual IME experiment

- User authorized moving the existing prototype into a real input method. macOS is the first local test platform. Keep upstream pristine and preserve the browser prototype.
- Swift/AppKit/InputMethodKit shell, existing Rust bridge and local datasets. No chronological input logs, context harvesting or other input-method settings reads. User authorized optional MiniMax translation, default OFF; send only the current Chinese candidate to the explicitly configured HTTPS endpoint after enablement. User authorized a separate self-hosted email-OTP account service and per-user selected-word sync; account sync never uploads sentences, raw Pinyin, personal Chinese vocabulary or host content. The user-authorized local personal vocabulary stores only confirmed Chinese words, Pinyin, usage counts and recency; never load it in tests/previews.
- Pheno v1.4 applies to this native UI, with native system fonts as fallback and no borrowed Qingjian brand assets. Normal typing has no animation. Candidate buttons must not activate the IME or steal the host editor's focus.
- Installation is user-local under ~/Library/Input Methods/BilingualCompanion.app. Never overwrite an unrelated app or automatically log out/restart. Restore the previously selected input source after agent testing.
- Sentence translation uses Apple Translation on macOS 26+ with installed zh-Hans/en languages. Resource preparation is an explicit separate SwiftUI helper; optional MiniMax translation is independent of installed Apple languages. Keys are endpoint-bound in Keychain; the normal IME launches a cancellable helper via anonymous pipes, never puts text/key in argv, logs or temporary files. Configuration changes invalidate old tasks and ready output. Invalidate asynchronous results on composition/candidate changes and cancellation. Keep bridge English validation strict; consume the verified Chinese candidate before submitting locally generated English.

## Build and checks

```sh
bash tools/build.sh
bash tools/check.sh
```

build.sh uses the parent local Rust runtime with Cargo.lock and existing Apple Command Line Tools. The result is build/BilingualCompanion.app. check.sh runs Swift formatting/lint, type checking, keyboard-state self-tests, real bridge checks and code-signing validation.

## Preview, install and register

```sh
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview --preview-narrow
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview --theme dark
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --account-preview
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --account-preview --account-state words --preview-narrow --theme dark
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --account-test
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --ai-test
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --ai-settings-preview
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --ai-settings-preview --preview-narrow --theme dark
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --appearance-test
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview --ui-state overflow
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-paging
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-learning
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-punctuation
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-typing
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --typing-test
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --punctuation-test
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-sentence
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --setup-translation
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --sentence-integration-test
../runtime/python/bin/python tools/install.py
"$HOME/Library/Input Methods/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --register
```

Registration does not select the source. --select selects it for authorized testing; --sources lists enabled input sources; --select-id ID restores a known source. System settings may still require a fresh login to discover a newly added IME. This must be reported, not hidden by pretending preview-mode input proves system integration.

Sentence integration checks require downloaded languages; check.sh runs cancellation tests without them. Do not run two normal IMK servers with the same connection name. Earlier CUA targeted key delivery did not invoke either this IME or Apple's Pinyin in TextEdit; native preview delivery has also shown interference from the selected system IME. Isolate preview checks with ABC and restore the original source. Neither pathway proves physical-keyboard acceptance; CUA cannot send standalone Shift on this machine.

Paging uses absolute engine indices internally and local 1–5 slot numbers per page. Never pass a local slot directly to commit. Page changes invalidate sentence translation and expanded senses; first/last page boundaries preserve state. Engine candidates are capped at64, keep remaining Pinyin and English validation intact.

UI tokens are centralized in NativeTheme.swift. Candidate rows retain absolute callbacks, wrap Chinese, and truncate only inline English with full accessibility/tooltips. Narrow widths stack languages; flipped documents start at the top inside a fixed rounded scroll frame. ActionButton draws immediate hover/press/focus states without stealing host focus. UI fixtures (empty/loading/failed/missing/unavailable/overflow) require --preview --ui-state NAME, never load personal vocabulary, and never prove real translation success.

Learning requires a valid context-bound commit receipt confirmed after host insertText. Cancel, raw submission and edits discard pending receipts/composition chains. English output learns the underlying Chinese word but never joins an English output into a composed new word. Personal vocabulary lives outside the repository in ~/Library/Application Support/BilingualCompanion/PersonalVocabulary/words.json. Keep malformed files intact. Memory integration tests use temporary stores; --preview-learning seeds and reloads its own temporary store. Pause is session-only and does not erase existing vocabulary.

Punctuation state is per-controller and follows confirmed Chinese/English output; manual override is process-local. Never inspect host text. Keep apostrophe within Pinyin and paging shortcuts intact. Submit punctuation only after a successful full Chinese commit; preserve leftover Pinyin after partial commits. Numeric protection is automatic; fixed Chinese can force a sentence period. Recognized schemes/www/email or fixed English preserve raw letters and use temporary ASCII passthrough until a boundary. --punctuation-test uses bundled real engine without a personal store; --preview-punctuation checks shared rule rendering, not IMK keyboard delivery.

Typing mode is process-local, default Chinese. Standalone Shift uses flagsChanged on press/release with a 0.7s tap limit; any intervening key, other modifier, mouse/focus boundary or two Shift keys cancels the gesture. Include mouse-down in recognizedEvents and explicitly raw-commit/pass it because the SDK default mouse handling applies only to the keyDown-only mask. English keyDown returns false before engine/punctuation/shortcut processing. Switching preserves raw Pinyin, cancels translation/learning and resets gesture state. --typing-test uses synthetic metadata and isolated engine; --preview-typing uses local native events and a standard editable field, not IMK acceptance. Never synthesize system keys outside CUA.

AppearanceSettings stores only the authorized app-owned appearanceChoice (light/dark/system), defaults to white, and broadcasts only that setting to its helper. Previews use memory-only settings; appearance tests use a temporary preferences suite and subprocess readback, never real preferences or vocabulary. Dynamic AppKit colors and layer viewDidChangeEffectiveAppearance update existing views without requerying/committing/resetting input. System mode removes the app appearance override; never change global macOS appearance for testing without user authorization.

Account helpers (--account / --sync-learning) run before engine initialization and use Keychain; the normal IME only reads account identity and queues verified English glossary selections after host insertText. Store uses file locks, per-login session identity and event UUIDs; logout/relogin cannot accept stale queued submissions. Preview/test stores are temporary, fake transport/vault only in --account-test; never contact SMTP or use real Keychain there. Read ../backend/README.md for deployment. Real public login remains unavailable until an HTTPS service and SMTP are configured.

AI test/preview modes use temporary settings, memory-only credentials and fake transport. Never read real keys or make paid test calls. Endpoint presets fill fields only; save/enable never sends a request. MiniMax uses reasoning_split, only content is shown; reject incomplete/empty/thinking-tag responses. No silent local fallback on API failure: preserve Chinese, allow retry or explicit local switch. Keep the word glossary offline and exclude sentence translations from learning-account sync.

--input-status is read-only: request only the existing normal service, timeout after 2 seconds, never initialize engine/Keychain/preferences or a new IMKServer in the query process. Keep status limited to process-local counters, booleans, mode and version; no key characters, composition text, candidates, host identifiers or input logs. --input-diagnostic-test checks this schema and the registered ObjC class/handle selector. Explicitly reference the controller class before IMKServer creation; this is startup validation, not proof of host compatibility.

Normal IMK service must use InputPresentation.configure (accessory policy) so its nonactivating candidate panel may create windows. Do not activate the app or make candidates key/main. `build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --input-window-test` checks 560/440pt show/hide and foreground/key focus with an isolated engine; check.sh includes it. This is not physical host acceptance.
