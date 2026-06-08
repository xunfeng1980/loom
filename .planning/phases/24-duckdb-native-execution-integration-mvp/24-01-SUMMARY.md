---
phase: 24-duckdb-native-execution-integration-mvp
plan: 01
subsystem: duckdb-runtime-ffi
tags: [rust, duckdb, runtime-abi, native-backend, jit, verifier]

requires:
  - phase: 22-host-native-runtime-abi-and-execution-policy
    provides: Runtime ABI planning, policy, projection, split, concurrency, and cache-key contracts
  - phase: 23-production-native-backend-implementation
    provides: Native backend prepare, cancellation, JIT seed, and diagnostics contracts
provides:
  - Internal `loom_ffi::duckdb_runtime` planning bridge for DuckDB route policy
  - Backend prepare route helper with mismatch/cancellation fail-closed behavior
  - Focused route policy tests for planning, projection, fallback, cancellation, mismatch, and toolchain diagnostics
affects: [phase-24, phase-25, duckdb-extension, loom-ffi, native-execution]

tech-stack:
  added: [loom-native-melior path dependency in loom-ffi]
  patterns:
    - Safe Rust internal bridge over Phase 22 runtime ABI and Phase 23 backend contracts
    - Test-only native facts hook emits explicit diagnostics and never becomes public SQL API

key-files:
  created:
    - crates/loom-ffi/src/duckdb_runtime.rs
    - crates/loom-ffi/tests/duckdb_runtime.rs
  modified:
    - Cargo.lock
    - crates/loom-ffi/Cargo.toml
    - crates/loom-ffi/src/lib.rs

key-decisions:
  - "DuckDB route planning remains inside Rust runtime policy; C++ should consume reports rather than duplicate native/fallback switches."
  - "Predicate pushdown and parallel split execution remain absent for this plan; reports use `PredicateEnvelope::None`, full scan, and single-worker policy."
  - "Native buffers are exposed only after backend prepare/JIT output comparison succeeds; mismatch and cancellation return no buffers."

patterns-established:
  - "DuckDB runtime reports preserve stable runtime/backend diagnostic codes as strings for later C ABI wrappers."
  - "Backend prepare is gated by `NativeCandidate` plus diagnostic-free runtime plans."

requirements-completed: [PHASE-24]

duration: 37min
completed: 2026-06-08
---

# Phase 24 Plan 01: Internal Rust DuckDB Runtime Bridge Summary

**DuckDB-facing Rust route planner and backend prepare bridge over Phase 22 runtime policy and Phase 23 native backend diagnostics**

## Performance

- **Duration:** 37 min
- **Started:** 2026-06-08T15:35:00Z
- **Completed:** 2026-06-08T16:12:53Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Added `loom_ffi::duckdb_runtime` with owned DuckDB planning types, runtime policy mapping, verifier-backed planning, projection validation, full-scan split modeling, single-worker concurrency, and deterministic cache-key construction.
- Added backend prepare routing that calls Phase 23 backend prepare/JIT paths only for native candidates, carries cancellation/toolchain/mismatch diagnostics forward, and emits typed fixed-width native buffers only after comparison succeeds.
- Added focused TDD coverage for route strings, projection ordering/rejection, no predicate pushdown, fallback vs fail-closed policy, cancellation, native-output-mismatch, and toolchain skip/failure visibility.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: DuckDB runtime planning tests** - `00a6ffa` (test)
2. **Task 1 GREEN: DuckDB runtime planning bridge** - `5d4cb3f` (feat)
3. **Task 2 RED: DuckDB runtime prepare tests** - `86cf116` (test)
4. **Task 2 GREEN: backend prepare routes** - `82823fa` (feat)

**Plan metadata:** committed separately after STATE/ROADMAP updates.

## Files Created/Modified

- `crates/loom-ffi/src/duckdb_runtime.rs` - Safe Rust internal planning and prepare bridge over artifact verification, runtime ABI policy, backend prepare, JIT comparison, and diagnostic preservation.
- `crates/loom-ffi/tests/duckdb_runtime.rs` - Route policy and prepare route tests for Task 1 and Task 2.
- `crates/loom-ffi/Cargo.toml` - Adds `loom-native-melior` path dependency to `loom-ffi`.
- `crates/loom-ffi/src/lib.rs` - Exposes the internal `duckdb_runtime` module.
- `Cargo.lock` - Records the new workspace path dependency edge.

## Decisions Made

- DuckDB projection is represented as output-ordered source indexes and converted into Phase 22 `ProjectionSet::Columns`; duplicate source/output and out-of-range mappings fail closed before backend work.
- The planner always uses `PredicateEnvelope::None`, `SplitDescriptor::FullScan`, and `ConcurrencyPolicy::SingleWorker` for this phase.
- Test-only native facts and test-only JIT buffers are explicitly named and emit diagnostics (`test-native-facts`, `test-jit-output`) so route tests do not imply public API or host-provided native trust.
- `native-output-mismatch` is handled as fail-closed with zero native buffers and is never downgraded to interpreter fallback.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Made toolchain diagnostic test robust to compatible local MLIR/LLVM**
- **Found during:** Task 2 (prepare route verification)
- **Issue:** The initial test asserted empty native buffers for the toolchain diagnostic case even when the local environment can successfully prepare and compare native buffers.
- **Fix:** Changed the test to require `toolchain-skipped` or `toolchain-failed` diagnostics only when no native buffers are emitted; if buffers are emitted, the decision must be `native-candidate`.
- **Files modified:** `crates/loom-ffi/tests/duckdb_runtime.rs`
- **Verification:** `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime prepare_routes -- --nocapture`
- **Committed in:** `82823fa`

---

**Total deviations:** 1 auto-fixed (Rule 1 bug).
**Impact on plan:** Test correction only; implementation scope stayed within the planned runtime/backend bridge.

## Issues Encountered

- RED test scaffolding initially used stale `LayoutNode::Raw` field names before the first test commit. The fixture was corrected to the current `data`, `elem_size`, and `count` shape so the RED gate failed only on missing DuckDB runtime API.

## Verification

- `cargo test -p loom-ffi --test duckdb_runtime runtime_planning -- --nocapture` - passed, 4 tests.
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime prepare_routes -- --nocapture` - passed, 4 tests.
- `cargo test -p loom-ffi --test duckdb_runtime` - passed, 8 tests.
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime` - passed, 8 tests.
- `cargo test -p loom-core --test runtime_execution_policy` - passed, 5 tests.
- `cargo test -p loom-native-melior --test production_backend_contract` - passed, 7 tests.

## Known Stubs

None.

## Threat Flags

None. The new trust surfaces were already represented in the plan threat model: DuckDB host to Rust planning bridge, runtime policy to native backend, and native output to DuckDB fill path.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 24-02 can wrap these owned Rust reports in an internal non-public DuckDB C ABI over opaque runtime/prepared handles. The public SQL surface remains unchanged, and C++ should consume route decisions and diagnostics from this helper rather than reimplementing runtime policy.

## Self-Check: PASSED

- Found created file: `crates/loom-ffi/src/duckdb_runtime.rs`
- Found created file: `crates/loom-ffi/tests/duckdb_runtime.rs`
- Found commits: `00a6ffa`, `5d4cb3f`, `86cf116`, `82823fa`
- Acceptance criteria checked: no predicate pushdown/parallel split/ArrowArrayStream markers in `duckdb_runtime.rs`; route strings and projection rejection assertions present in tests; prepare helper references backend prepare, JIT execute, and JIT comparison APIs.

---
*Phase: 24-duckdb-native-execution-integration-mvp*
*Completed: 2026-06-08*
