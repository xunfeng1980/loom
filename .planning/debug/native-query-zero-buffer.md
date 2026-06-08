---
status: resolved
trigger: "Native DuckDB query path reports native plumbing but consumes zero-filled native buffers; real SQL values still come from interpreter fallback."
created: 2026-06-09
updated: 2026-06-09
---

# Debug Session: native-query-zero-buffer

## Symptoms

- Expected behavior: forced DuckDB native route should produce real query values from native execution buffers for supported primitive fixtures.
- Actual behavior: native route buffers are constructed from `reference_zeroed_value_bytes`, while real SQL assertions are satisfied by interpreter fallback.
- Error messages: no crash; verification gates accept fallback, toolchain skip, or toolchain failure as passing native hardening outcomes.
- Timeline: discovered during Phase 24/25 audit after native integration/hardening phases were marked complete.
- Reproduction: inspect `reference_zeroed_value_bytes`, `LOOM_DUCKDB_TEST_USE_NATIVE_FACTS`, `scripts/native-hardening-test.sh`, and native mismatch tests.

## Current Focus

- hypothesis: native route is a valid ABI/routing skeleton but not yet a real data-producing path for DuckDB SQL.
- test: trace native buffer creation and route decisions, then force a non-zero primitive fixture through native route and assert SQL output.
- expecting: current code either returns zero native buffers or falls back/skips while tests still pass.
- next_action: inspect native buffer creation and DuckDB route integration.

## Evidence

- `crates/loom-ffi/src/duckdb_runtime.rs` now extracts verified `Raw`
  primitive `LMP1`/`LMT1` artifact bytes as ExecutionEngine input buffers and
  requires `execute_prepared_production_jit` to produce the native output.
- `crates/loom-native-melior/src/jit.rs` now lowers raw-copy MLIR with input
  and output memrefs, runs it through Melior `ExecutionEngine::invoke_packed`,
  and fails closed when explicit artifact input buffers are missing.
- `crates/loom-fixtures/src/bin/emit_duckdb_payloads.rs` now emits non-zero
  `native-primitives-table.loom` values:
  `1,10,1.5,0.25|2,20,2.5,1.25|3,30,3.5,2.25|4,40,4.5,3.25`.
- `scripts/duckdb-native-integration-test.sh` now forces the native test route,
  checks `COUNT/SUM` as `4,10,100`, requires
  `native-execution-engine-output`, and fails if interpreter fallback or
  toolchain skip appears for the primitive native query.
- `scripts/native-hardening-test.sh` now checks non-zero aggregate evidence,
  cache miss/insert/hit ordering, projection values, and no fallback/toolchain
  skip for native primitive scans.
- `duckdb-ext/CMakeLists.txt` now links the loadable extension against the
  LLVM runtime reported by `llvm-config`, so Melior ExecutionEngine symbols
  resolve when DuckDB loads `loom.duckdb_extension`.

## Eliminated

- Zero-filled `reference_zeroed_value_bytes` is no longer a successful DuckDB
  native output path.
- Direct artifact raw-copy bypass is no longer accepted as DuckDB native
  success; public SQL gates require the MLIR ExecutionEngine output marker.
- Missing local `llvm-config` in the default shell PATH no longer prevents
  `loom-ffi`/DuckDB gates from compiling; `.cargo/config.toml` points
  `mlir-sys`/`tblgen` at Homebrew LLVM 22 unless the environment overrides it.
- Missing LLVM runtime linkage in the CMake-built DuckDB extension no longer
  causes `dlopen` failures on `llvm::APFloatBase` symbols.

## Resolution

- root_cause: Phase 24/25 native DuckDB tests used zero-value reference buffers
  and allowed fallback/toolchain-skip outcomes, so public SQL could pass without
  proving that MLIR ExecutionEngine produced real native buffers.
- fix: Added raw-copy MLIR lowering with input/output memrefs, wired Melior
  ExecutionEngine invocation into production JIT, fed verified artifact value
  buffers into that JIT from DuckDB runtime, updated fixtures to non-zero
  values, and hardened shell gates to reject fallback/toolchain-skip for the
  primitive native route.
- verification:
  - `cargo test -p loom-native-melior --features melior`
  - `cargo test -p loom-ffi`
  - `bash scripts/duckdb-native-integration-test.sh`
  - `bash scripts/native-hardening-test.sh`
- files_changed:
  - `crates/loom-ffi/src/duckdb_runtime.rs`
  - `crates/loom-ffi/tests/duckdb_runtime.rs`
  - `crates/loom-ffi/tests/duckdb_runtime_cache.rs`
  - `crates/loom-fixtures/src/bin/emit_duckdb_payloads.rs`
  - `crates/loom-native-melior/src/jit.rs`
  - `crates/loom-native-melior/tests/production_backend_jit.rs`
  - `scripts/duckdb-native-integration-test.sh`
  - `scripts/native-hardening-test.sh`
  - `.cargo/config.toml`
  - `duckdb-ext/CMakeLists.txt`
  - `crates/loom-core/src/arrow_buffer_lowering.rs`
  - `crates/loom-core/tests/arrow_buffer_lowering.rs`
