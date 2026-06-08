---
phase: 24-duckdb-native-execution-integration-mvp
plan: 05
subsystem: duckdb-native-release-gate
tags: [duckdb, native-execution, release-gate, route-diagnostics, fixtures]

requires:
  - phase: 24-04
    provides: Route-selected native/interpreter DuckDB DataChunk output
provides:
  - Native-eligible non-null primitive DuckDB fixture
  - Route-aware Phase 24 DuckDB integration gate
  - Main release-gate wiring for runtime ABI, backend, DuckDB native integration, and SQL smoke evidence
  - Final Phase 24 DuckDB native execution report
affects: [phase-24, phase-25, duckdb-extension, release-gate, native-execution]

tech-stack:
  added: []
  patterns:
    - Test-only DuckDB route report controls are prefixed LOOM_DUCKDB_TEST_
    - SQL checks use only public loom_scan(path)
    - Route assertions combine DuckDB SQL evidence with helper-level mismatch/cancellation tests

key-files:
  created:
    - scripts/duckdb-native-integration-test.sh
    - .planning/phases/24-duckdb-native-execution-integration-mvp/24-DUCKDB-NATIVE-REPORT.md
  modified:
    - crates/loom-fixtures/src/bin/emit_duckdb_payloads.rs
    - duckdb-ext/loom_extension.cpp
    - scripts/mvp0-verify.sh
    - scripts/check-core-invariants.sh

key-decisions:
  - "Phase 24 route evidence is tested through public `loom_scan(path)` SQL plus internal `LOOM_DUCKDB_TEST_*` diagnostics, not public route-specific SQL."
  - "The native primitive DuckDB fixture is intentionally all-zero, non-null Int32/Int64/Float32/Float64 raw table data to avoid broader native semantics claims."
  - "The main release gate now runs Phase 24 after the Phase 23 backend gate and before the existing DuckDB SQL smoke gate."

requirements-completed: [PHASE-24]

duration: 8min
completed: 2026-06-08T16:50:23Z
---

# Phase 24 Plan 05: DuckDB Native Release Gate Summary

**Route-aware DuckDB native integration gate with a narrow primitive fixture, release-gate wiring, and final Phase 24 decision report**

## Performance

- **Duration:** 8 min
- **Started:** 2026-06-08T16:42:46Z
- **Completed:** 2026-06-08T16:50:23Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Added `native-primitives-table.loom` generation through `emit_duckdb_payloads`, backed by an `LMC1` wrapped `LMT1` table with non-null Int32, Int64, Float32, and Float64 raw zero columns.
- Added `scripts/duckdb-native-integration-test.sh`, a route-aware gate that builds the extension, runs SQL only through `loom_scan(path)`, captures temporary route diagnostics, checks projection order, fallback, strict fail-closed behavior, cancellation, mismatch, malformed artifact handling, single-worker, and single-batch evidence.
- Added internal DuckDB adapter route-report controls under `LOOM_DUCKDB_TEST_*` so tests can observe native/fallback/fail-closed/cancel routes without widening public SQL.
- Wired the Phase 24 gate into `scripts/mvp0-verify.sh` after Phase 23 production backend evidence and before the existing DuckDB SQL smoke gate.
- Added `24-DUCKDB-NATIVE-REPORT.md`, closing D-01 through D-14 and explicitly documenting Phase 24 non-goals.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add native primitive DuckDB fixture** - `1b6c2a5` (feat)
2. **Task 2: Add DuckDB native integration gate** - `1e3170c` (feat)
3. **Task 3: Wire Phase 24 release gate and report** - `860a444` (feat)

## Files Created/Modified

- `crates/loom-fixtures/src/bin/emit_duckdb_payloads.rs` - Adds deterministic `native-primitives-table.loom` generation and manifest row.
- `scripts/duckdb-native-integration-test.sh` - New Phase 24 route-aware DuckDB integration gate.
- `duckdb-ext/loom_extension.cpp` - Adds internal `LOOM_DUCKDB_TEST_*` route-report, strict fallback, test native facts, and cancellation controls.
- `scripts/mvp0-verify.sh` - Runs the Phase 24 gate before the DuckDB smoke test.
- `scripts/check-core-invariants.sh` - Updates the backend dependency guard for the Phase 24 internal `loom-ffi -> loom-native-melior` bridge while keeping `loom-core` isolated.
- `.planning/phases/24-duckdb-native-execution-integration-mvp/24-DUCKDB-NATIVE-REPORT.md` - Final Phase 24 report.

## Decisions Made

- Route visibility is tested through temporary internal diagnostics, not public SQL modes.
- The fixture stays deliberately narrow and all-zero so Phase 24 proves routing and adapter behavior without claiming arbitrary primitive semantics.
- Mismatch and cancellation remain helper-level evidence where DuckDB host cancellation/output mismatch are not naturally observable through public SQL.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed raw fixture elem_size type**
- **Found during:** Task 1 verification
- **Issue:** The new raw fixture helper initially used `u32` for `LayoutNode::Raw.elem_size`, but the current model expects `u8`.
- **Fix:** Changed the helper parameter to `u8` and formatted only the touched fixture file.
- **Files modified:** `crates/loom-fixtures/src/bin/emit_duckdb_payloads.rs`
- **Commit:** `1b6c2a5`

**2. [Rule 2 - Missing Critical Functionality] Added internal route-report controls**
- **Found during:** Task 2 implementation
- **Issue:** SQL row checks alone could not prove native/fallback/fail-closed routes, and the adapter had no route report output.
- **Fix:** Added `LOOM_DUCKDB_TEST_ROUTE_REPORT`, `LOOM_DUCKDB_TEST_ALLOW_INTERPRETER_FALLBACK`, `LOOM_DUCKDB_TEST_USE_NATIVE_FACTS`, and `LOOM_DUCKDB_TEST_CANCEL_PREPARE` controls in the DuckDB adapter.
- **Files modified:** `duckdb-ext/loom_extension.cpp`
- **Commit:** `1e3170c`

**3. [Rule 1 - Bug] Fixed single-worker grep in the integration script**
- **Found during:** Task 2 verification
- **Issue:** The script over-escaped the `MaxThreads()` regex and failed despite the guard being present.
- **Fix:** Corrected the grep pattern and reran the Phase 24 gate.
- **Files modified:** `scripts/duckdb-native-integration-test.sh`
- **Commit:** `1e3170c`

**4. [Rule 3 - Blocking Issue] Updated stale backend dependency invariant**
- **Found during:** Task 3 full release-gate verification
- **Issue:** `scripts/check-core-invariants.sh` still treated any transitive `loom-ffi` MLIR/backend dependency as invalid, conflicting with the Phase 24 internal `loom-ffi -> loom-native-melior` bridge introduced in earlier plans.
- **Fix:** Kept `loom-core` fully isolated and changed the `loom-ffi` guard to allow only the direct `loom-native-melior` bridge, rejecting direct `melior`/`mlir`/`llvm` dependencies.
- **Files modified:** `scripts/check-core-invariants.sh`
- **Commit:** `860a444`

## Verification

- `cargo run -p loom-fixtures --bin emit_duckdb_payloads` - passed.
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/duckdb-native-integration-test.sh` - passed.
- `bash scripts/duckdb-smoke-test.sh` - passed as part of the full release gate.
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp0-verify.sh` - passed.
- `bash scripts/check-core-invariants.sh` - passed after updating the stale backend dependency guard.
- `git diff --check` - passed.

## Known Stubs

None.

## Threat Flags

None. The new test controls are under the planned D-14 internal-test boundary, and the release-gate evidence now asserts route diagnostics instead of trusting SQL rows alone.

## User Setup Required

None. The gate uses existing project tooling and honors `LOOM_ALLOW_NATIVE_TOOL_SKIP=1` for managed native-toolchain skip behavior.

## Next Phase Readiness

Phase 24 is closed. Phase 25 can start native equivalence, cache, and fallback hardening with release-gated evidence that DuckDB now exercises runtime ABI, backend prepare, route diagnostics, projection, fallback, strict fail-closed, mismatch, cancellation, and SQL smoke behavior together.

## Self-Check: PASSED

- Found generated fixture: `target/loom-duckdb-fixtures/native-primitives-table.loom`
- Found created script: `scripts/duckdb-native-integration-test.sh`
- Found final report: `.planning/phases/24-duckdb-native-execution-integration-mvp/24-DUCKDB-NATIVE-REPORT.md`
- Found commits: `1b6c2a5`, `1e3170c`, `860a444`
- Acceptance criteria checked: D-01 through D-14 present in the report; required non-goal strings present; `scripts/mvp0-verify.sh` invokes the Phase 24 gate; forbidden public SQL/API markers absent from the Phase 24 gate script; full release gate passed.

---
*Phase: 24-duckdb-native-execution-integration-mvp*
*Completed: 2026-06-08*
