# Local bilingual IME prototype

- This is an experimental page, not a registered system input source. Preserve pristine upstream checkouts. No cloud calls or personal configuration reads.
- Vue/React are not used: vanilla TypeScript + Vite, Python standard-library local HTTP server, and a Rust adapter over Qingjian. Production dependency is the GPL Qingjian engine; JS dependencies are development tools only.
- Use pnpm with the existing pnpm-lock.yaml, and the local uv Python environment. The parent Rust wrapper settings are inherited by `tools/cargo.sh`; preserve bridge/Cargo.lock.
- New/modified UI follows Pheno v1.4. See QA.md for evidence and remaining gaps. Do not manufacture brand/font assets.
- Candidate submission must run through the engine, including remaining Pinyin handling. Expanded sense lookup stays separate from the upstream two-sense annotation. Do not accept arbitrary client-provided English.

## Setup and build

From this directory, with the parent research runtime and pinned upstream checkouts present:

```sh
pnpm install --frozen-lockfile
bash tools/cargo.sh build --release --locked
pnpm build
```

## Run

```sh
pnpm serve
```

Open http://127.0.0.1:9037. For frontend hot reload, leave that server running and run `pnpm dev` in another terminal (default http://127.0.0.1:5173). The server serves dist; rebuild and reload after production UI edits.

## Checks

```sh
pnpm format:check
pnpm lint
pnpm typecheck
pnpm test
pnpm build
../runtime/python/bin/ruff format --check tools data tests
../runtime/python/bin/ruff check tools data tests
../runtime/python/bin/python -m compileall -q tools data tests
bash -n tools/cargo.sh
bash tools/cargo.sh fmt -- --check
bash tools/cargo.sh clippy --release --locked -- -D warnings
../runtime/python/bin/python tests/engine_test.py
../runtime/python/bin/python tests/memory_test.py
../runtime/python/bin/python tests/http_test.py
```

The HTTP checks require `pnpm serve`. Browser checks must exercise Chinese/English/third-sense commits, mode switching, remaining Pinyin, rapid queries, errors/retry, focus and narrow layouts. Do not claim true mobile-device or system IME verification from responsive browser checks.

Bridge personal vocabulary is opt-in with --memory PATH, only normal native service enables the real store. Web remains memory-disabled. Require explicit post-output confirmation and context-bound single-use receipts. Tests must use isolated temporary stores and never inspect user vocabulary.
