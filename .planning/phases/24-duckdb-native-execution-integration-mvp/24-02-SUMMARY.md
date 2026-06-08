---
phase: 24-duckdb-native-execution-integration-mvp
plan: 02
subsystem: duckdb-runtime-ffi
tags: [rust, ffi, duckdb, runtime-abi, native-backend]

requires:
  - phase: 24-01
    provides: Safe Rust DuckDB runtime planning and prepare route reports
provides:
  - Internal non-public DuckDB C ABI over opaque plan/prepared handles
  - Panic-safe `loom_duckdb_*` wrappers with borrowed diagnostics and native buffers
  - Public header leakage tests proving DuckDB route controls stay out of `loom.h`
affects: [phase-24, duckdb-extension, loom-ffi, native-execution]

tech-stack:
  added: []
  patterns:
    - Opaque handle-owned FFI strings and buffers borrowed for handle lifetime
    - Internal manual C header for DuckDB adapter symbols, excluded from generated public header
    - Panic-safe integer-status extern wrappers with null guards before pointer use

key-files:
  created:
    - crates/loom-ffi/include/loom_duckdb_internal.h
    - crates/loom-ffi/tests/duckdb_runtime_ffi.rs
  modified:
    - crates/loom-ffi/src/duckdb_runtime.rs
    - crates/loom-ffi/cbindgen.toml
    - crates/loom-ffi/build.rs

key-decisions:
  - "DuckDB route controls are exposed only through `loom_duckdb_internal.h`; generated public `loom.h` excludes every `loom_duckdb_*` symbol and `LoomDuckDb*` type."
  - "FFI diagnostics, cache keys, decisions, and native buffers are borrowed from opaque handles and remain valid only for the handle lifetime."
  - "Task 2 became a test-only proof expansion because Task 1's required handle implementation already covered cancellation, out-of-range, and native-buffer access behavior."

requirements-completed: [PHASE-24]

duration: 5min
completed: 2026-06-08
---

# Phase 24 Plan 02: DuckDB Internal FFI Handles Summary

**Internal DuckDB C ABI with opaque runtime/prepared handles, panic-safe wrappers, diagnostics, and public-header isolation**

## Performance

- **Duration:** 5 min
- **Started:** 2026-06-08T16:17:23Z
- **Completed:** 2026-06-08T16:23:09Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Added `loom_duckdb_internal.h` as an explicitly internal, non-public DuckDB adapter header with opaque `LoomDuckDbPlan` and `LoomDuckDbPrepared` handles.
- Added panic-safe `loom_duckdb_*` extern wrappers for plan creation/destruction, route decision, cache key, diagnostics, prepare creation/destruction, prepare status/route, diagnostic access, and native buffer access.
- Kept all DuckDB route controls out of generated public `loom.h` through explicit cbindgen exclusions and source/header tests.
- Added focused FFI tests for null guards, handle lifetime, fallback/strict routing diagnostics, cancellation diagnostics, out-of-range access, native-buffer exposure rules, and public header leakage gates.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: internal FFI handle contract tests** - `b158daa` (test)
2. **Task 1 GREEN: internal FFI handles and header** - `c336c37` (feat)
3. **Task 2: FFI behavior and public ABI proof tests** - `9f388fd` (test)

**Plan metadata:** committed separately after SUMMARY/STATE/ROADMAP updates.

## Files Created/Modified

- `crates/loom-ffi/include/loom_duckdb_internal.h` - Internal DuckDB adapter header, non-public and not a frozen `loom_runtime.h` ABI.
- `crates/loom-ffi/src/duckdb_runtime.rs` - Opaque plan/prepared handles plus panic-safe internal C ABI wrappers.
- `crates/loom-ffi/tests/duckdb_runtime_ffi.rs` - Internal FFI contract, diagnostics, cancellation, native-buffer, and public-header leakage tests.
- `crates/loom-ffi/cbindgen.toml` - Excludes all `loom_duckdb_*` functions and `LoomDuckDb*` types from generated public `loom.h`.
- `crates/loom-ffi/build.rs` - Re-runs public header generation when `src/duckdb_runtime.rs` changes.

## Decisions Made

- Internal test native facts remain behind the internal `loom_duckdb_plan_create` control and are not surfaced through public SQL or public headers.
- Native buffers expose borrowed pointers only while `LoomDuckDbPrepared` is alive; non-native routes expose zero buffers.
- Public ABI creep is blocked by both cbindgen exclusion and tests that reject `loom_duckdb_`, `LoomDuckDb`, `loom_scan_native`, `loom_scan_interpreter`, and `ArrowArrayStream` in `loom.h`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Made raw-pointer slice access explicit in FFI wrappers**
- **Found during:** Task 1 GREEN verification
- **Issue:** Rust 1.92 denied implicit autoref when calling `.get()` through raw pointer dereferences in diagnostic/native-buffer accessors.
- **Fix:** Changed those accesses to explicit references before indexing.
- **Files modified:** `crates/loom-ffi/src/duckdb_runtime.rs`
- **Verification:** `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime_ffi -- --nocapture`
- **Committed in:** `c336c37`

---

**Total deviations:** 1 auto-fixed (Rule 1 bug).
**Impact on plan:** Implementation remained within the planned internal FFI boundary.

## TDD Gate Compliance

- Task 1 followed RED/GREEN: `b158daa` introduced failing tests for missing internal FFI symbols; `c336c37` implemented the handles and wrappers.
- Task 2 added proof tests after Task 1 implementation. The new tests passed immediately because Task 1's required implementation already covered the behavior. No additional GREEN source commit was needed.

## Issues Encountered

- `cargo fmt --check` reports unrelated formatting differences in existing files outside this plan. I formatted only `crates/loom-ffi/tests/duckdb_runtime_ffi.rs` and left unrelated files untouched.

## Verification

- `cargo build -p loom-ffi --release` - passed.
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime_ffi -- --nocapture` - passed, 9 tests.
- `grep -v '^#' crates/loom-ffi/include/loom.h | grep -c 'loom_duckdb_' | grep '^0$'` - passed, output `0`.
- Internal header checks passed: contains `DuckDB adapter internal`, contains no `ArrowArrayStream`, contains no `loom_scan_native`.
- Wrapper safety scan passed: every new `loom_duckdb_*` extern wrapper has a null guard and `panic::catch_unwind`.

## Known Stubs

None.

## Threat Flags

None. The new C ABI trust surface, public-header leakage boundary, and borrowed native-buffer lifetime were all included in the plan threat model and mitigated here.

## User Setup Required

None - no external services or package installs required.

## Next Phase Readiness

Plan 24-03 can consume `loom_duckdb_internal.h` from the DuckDB C++ adapter without widening public SQL or freezing `loom_runtime.h`.

## Self-Check: PASSED

- Found created file: `crates/loom-ffi/include/loom_duckdb_internal.h`
- Found created file: `crates/loom-ffi/tests/duckdb_runtime_ffi.rs`
- Found commits: `b158daa`, `c336c37`, `9f388fd`
- Acceptance criteria checked: internal header phrase and forbidden symbols; public `loom.h` leakage grep; focused FFI tests; release build.

---
*Phase: 24-duckdb-native-execution-integration-mvp*
*Completed: 2026-06-08*
