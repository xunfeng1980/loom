---
phase: 24-duckdb-native-execution-integration-mvp
plan: 03
subsystem: duckdb-runtime-adapter
tags: [duckdb, cpp, runtime-abi, native-backend, projection]

requires:
  - phase: 24-01
    provides: Safe Rust DuckDB runtime planning and prepare route reports
  - phase: 24-02
    provides: Internal non-public DuckDB C ABI over opaque plan/prepared handles
provides:
  - DuckDB bind-time ownership of internal runtime plan/cache route evidence
  - Global init projection mapping and native-candidate prepare routing
  - Single-worker scan state preserving interpreter fallback and direct DataChunk output
affects: [phase-24, phase-25, duckdb-extension, native-execution]

tech-stack:
  added: []
  patterns:
    - C++ RAII wrappers own internal DuckDB runtime opaque handles exactly once
    - DuckDB projection ids are finalized in global init while public SQL remains loom_scan(path)
    - Native route diagnostics surface stable code/path pairs before row emission

key-files:
  created: []
  modified:
    - duckdb-ext/loom_extension.cpp

key-decisions:
  - "DuckDB C++ consumes internal Rust route decisions instead of duplicating native eligibility policy."
  - "Projection pushdown is enabled internally through TableFunctionInitInput::column_ids without adding SQL mode parameters."
  - "Phase 24 keeps single-worker, single-batch direct DataChunk output; local worker state and stream APIs remain deferred."

patterns-established:
  - "Bind owns shared runtime plan evidence; init may reuse it or build a projected plan holder for DuckDB-selected columns."
  - "Native prepare is attempted only for native-candidate plans; fallback routes continue through loom_decode."
  - "Route failures use messages containing stable diagnostic code/path pairs."

requirements-completed: [PHASE-24]

duration: 5min
completed: 2026-06-08
---

# Phase 24 Plan 03: DuckDB Runtime Lifecycle Adapter Summary

**DuckDB loom_scan bind/init now owns runtime plan handles, maps projection ids, prepares native candidates, and preserves interpreter SQL behavior**

## Performance

- **Duration:** 5 min
- **Started:** 2026-06-08T16:26:38Z
- **Completed:** 2026-06-08T16:31:24Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments

- Added internal DuckDB runtime header consumption and C++ RAII holders for `LoomDuckDbPlan` and `LoomDuckDbPrepared`.
- Extended `LoomBindData` so bind reads payload/schema, creates the all-column runtime plan, and carries route decision, cache key, diagnostics, payload, schema, and column payloads through copy/equality.
- Enabled `fn.projection_pushdown = true` while preserving the single public `loom_scan(VARCHAR)` registration.
- Added global-init projection mapping from DuckDB `column_ids`, single-worker scan state, native-candidate prepare, stable route diagnostics, and interpreter fallback through the existing `loom_decode` Arrow RAII path.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add bind-time runtime plan ownership and projection pushdown** - `beec2f6` (feat)
2. **Task 2: Prepare projected native/interpreter route in global init** - `0c4c8b7` (feat)

**Plan metadata:** committed separately after SUMMARY/STATE/ROADMAP updates.

## Files Created/Modified

- `duckdb-ext/loom_extension.cpp` - DuckDB adapter lifecycle wiring from bind-time runtime plan ownership through projected init routing, native prepare, fallback decode, one-worker scan state, and stable diagnostics.

## Decisions Made

- Reused the internal Phase 24 `loom_duckdb_*` C ABI as the only route-control boundary; no public SQL native/interpreter mode was added.
- Kept runtime route planning in Rust; C++ stores and reacts to decision/cache/diagnostic strings instead of reconstructing policy.
- Used interpreter decode to preserve cardinality for empty projection cases such as `COUNT(*)`, because native output buffers are tied to materialized projected columns in this slice.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed stale forbidden stream marker from legacy comment**
- **Found during:** Task 1 acceptance verification
- **Issue:** The plan required `rg -n "loom_scan_native|loom_scan_interpreter|ArrowArrayStream|LocalTableFunctionState" duckdb-ext/loom_extension.cpp` to return no matches, but an old explanatory comment still contained the forbidden stream marker.
- **Fix:** Reworded the legacy comment without changing behavior.
- **Files modified:** `duckdb-ext/loom_extension.cpp`
- **Verification:** Forbidden-symbol `rg` returned no matches.
- **Committed in:** `beec2f6`

**2. [Rule 1 - Bug] Fixed projected native row-count buffer selection**
- **Found during:** Task 2 verification
- **Issue:** Native row count initially used the first native buffer even when DuckDB projected a later source column.
- **Fix:** Count rows from the same native buffer selected for output column 0 and added a forward declaration for C++ compile order.
- **Files modified:** `duckdb-ext/loom_extension.cpp`
- **Verification:** `cmake --build duckdb-ext/build` and `bash scripts/duckdb-smoke-test.sh` passed.
- **Committed in:** `0c4c8b7`

---

**Total deviations:** 2 auto-fixed (Rule 1 bugs).
**Impact on plan:** Both fixes tightened the planned adapter behavior and acceptance gates without widening public SQL or adding dependencies.

## Issues Encountered

- Task 2 compile verification caught a stale `source_idx` reference after simplifying native row-count logic. It was fixed before the task commit and the C++ extension build passed afterward.
- The internal FFI currently exposes projection planning to C++ through handle creation only, not a projection-parameterized C call. The adapter still maps DuckDB output column ids and prunes interpreter decode/output columns internally; a richer internal projection C ABI remains a possible Phase 25 hardening item.

## Verification

- `cargo build -p loom-ffi --release` - passed.
- `cmake -S duckdb-ext -B duckdb-ext/build -DCMAKE_BUILD_TYPE=Release` - passed.
- `cmake --build duckdb-ext/build` - passed.
- `bash scripts/duckdb-smoke-test.sh` - passed, covering bitpack, FOR, dict, RLE, FSST, dict-FSST, ALP Float32/Float64, and mixed-table SQL.
- Acceptance checks passed: internal header included; exactly one public `loom_scan` registration; `projection_pushdown = true`; no `loom_scan_native`, `loom_scan_interpreter`, forbidden stream marker, or `LocalTableFunctionState`; `LoomScanState::MaxThreads() const` returns `1`; `LoomInit` references both `loom_duckdb_prepare_create` and `loom_decode`.

## Known Stubs

None.

## Threat Flags

None. The touched surface matches the plan threat model: local SQL path bytes to bind, opaque runtime handles from bind to init, native prepare gated by runtime route approval, and Arrow/native teardown owned by scan state.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 24-04 can add route-aware DuckDB tests around native eligibility, fallback, strict diagnostics, and projection behavior on top of the lifecycle hooks now present in `loom_extension.cpp`.

## Self-Check: PASSED

- Found modified file: `duckdb-ext/loom_extension.cpp`
- Found task commits: `beec2f6`, `0c4c8b7`
- Found created summary path: `.planning/phases/24-duckdb-native-execution-integration-mvp/24-03-SUMMARY.md`
- Acceptance criteria checked: forbidden symbol grep clean, single `loom_scan` registration, projection pushdown present, MaxThreads override present, prepare and decode paths present, and plan-level verification passed.

---
*Phase: 24-duckdb-native-execution-integration-mvp*
*Completed: 2026-06-08*
