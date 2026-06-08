---
phase: 28-iceberg-ref-table-binding
plan: 05
subsystem: release-gates
tags:
  - rust
  - iceberg
  - source-ingress
  - release-gate
  - duckdb
dependency_graph:
  requires:
    - 28-01-SUMMARY.md
    - 28-02-SUMMARY.md
    - 28-03-SUMMARY.md
    - 28-04-SUMMARY.md
    - 28-ICEBERG-BINDING-REPORT.md
  provides:
    - Focused Iceberg binding gate closeout with prior-summary, fixture, report, and dependency-family checks
    - Main release verifier wiring after Phase 27 Lance/Parquet and before DuckDB smoke
    - Final Phase 28 release evidence and Phase 29 handoff
    - Deterministic DuckDB runtime FFI cache tests for release-verifier execution
  affects:
    - phase-28
    - phase-29-handoff
    - scripts/mvp0-verify.sh
tech_stack:
  added: []
  patterns:
    - focused gate before main verifier wiring
    - release-gate order assertion
    - serialized shared-cache test setup
key_files:
  created:
    - .planning/phases/28-iceberg-ref-table-binding/28-05-SUMMARY.md
  modified:
    - scripts/iceberg-binding-test.sh
    - scripts/mvp0-verify.sh
    - crates/loom-iceberg-binding/tests/dependency_boundary.rs
    - crates/loom-ffi/tests/duckdb_runtime_ffi.rs
    - .planning/phases/28-iceberg-ref-table-binding/28-ICEBERG-BINDING-REPORT.md
key_decisions:
  - Wire Phase 28 into mvp0-verify only after the focused Iceberg binding gate passed.
  - Keep Phase 28 as binding evidence only; no Iceberg query route, catalog, credential, StarRocks, CLI, DuckDB, or public C ABI surface was added.
  - Serialize DuckDB runtime FFI cache-sensitive tests to keep the existing release verifier deterministic under parallel cargo test execution.
metrics:
  duration: ~30min
  tasks: 3
  files_modified: 5
  completed: 2026-06-08T23:00:00Z
---

# Phase 28 Plan 05: Iceberg Gate Closeout Summary

Focused Iceberg binding release gate wired into MVP0 verification after Lance/Parquet and before DuckDB smoke, with final evidence report and no query-surface expansion.

## Accomplishments

- Finalized `scripts/iceberg-binding-test.sh` as a closeout gate: it now checks all four prior plan summaries, required Iceberg binding fixtures, report evidence markers, adapter dependency boundaries, public-surface non-goals, and duplicate Arrow/Parquet dependency-family drift.
- Wired the focused gate into `scripts/mvp0-verify.sh` after the Phase 27 Lance/Parquet gate and before DuckDB smoke, matching the critical release-order requirement.
- Updated `crates/loom-iceberg-binding/tests/dependency_boundary.rs` so the test suite enforces the main verifier order and prevents accidental Phase 28 gate removal.
- Closed out `.planning/phases/28-iceberg-ref-table-binding/28-ICEBERG-BINDING-REPORT.md` with command evidence for the focused gate, verifier syntax/order checks, and full release verification.
- Stabilized existing DuckDB runtime FFI cache tests that blocked the full verifier by serializing cache-sensitive setup around the shared native preparation cache.

## Task Commits

| Task | Description | Commit |
|------|-------------|--------|
| 1 | Finalize the focused Iceberg binding gate | `487577b` |
| 2 | Wire the Iceberg gate into the main verifier in the required order | `3f5ddb1` |
| 3a | Auto-fix blocking DuckDB runtime FFI cache-test isolation issue | `1e5e17c` |
| 3b | Record final Iceberg binding release evidence | `d9255f3` |

## Verification

- `bash -n scripts/iceberg-binding-test.sh && bash scripts/iceberg-binding-test.sh`
- `bash -n scripts/mvp0-verify.sh`
- Direct verifier-order check confirmed `source-ingress-contract-test.sh < lance-parquet-ingress-test.sh < iceberg-binding-test.sh < duckdb-smoke.sh`.
- `cargo test -p loom-iceberg-binding --test dependency_boundary`
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi`
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/check-core-invariants.sh`
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp0-verify.sh`

## Decisions Made

- Phase 28 is now part of the main release gate, but remains limited to local Iceberg metadata/reference binding evidence.
- The gate explicitly rejects accidental official Iceberg SDK adoption, public C ABI expansion, DuckDB/CLI SQL route expansion, StarRocks route expansion, catalog/network/object-store credential surface, branch/tag mutation semantics, and default `iceberg` SDK scope.
- Duplicate Arrow/Parquet family checks are enforced in the focused gate so future dependency drift is caught before verifier wiring can pass.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking Issue] Serialized DuckDB runtime FFI cache-sensitive tests**

- **Found during:** Task 3 full release verification.
- **Issue:** `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp0-verify.sh` reached the existing Phase 18 CORE invariant sub-gate and failed in `loom-ffi` DuckDB runtime tests because parallel tests shared and cleared the native preparation cache.
- **Fix:** Added a local mutex-backed `isolated_prepare_cache()` helper in `crates/loom-ffi/tests/duckdb_runtime_ffi.rs` and applied it to tests that clear or seed the native preparation cache.
- **Files modified:** `crates/loom-ffi/tests/duckdb_runtime_ffi.rs`
- **Verification:** `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi`, `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/check-core-invariants.sh`, and `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp0-verify.sh`.
- **Commit:** `1e5e17c`

## Known Stubs

None.

## Threat Flags

None. This plan added no network endpoint, credential path, public API route, query surface, schema migration, catalog integration, or object-store access path.

## Self-Check: PASSED

- Summary file created at `.planning/phases/28-iceberg-ref-table-binding/28-05-SUMMARY.md`.
- Required task commits exist: `487577b`, `3f5ddb1`, `1e5e17c`, `d9255f3`.
- Focused gate passes: `bash scripts/iceberg-binding-test.sh`.
- Main verifier passes with Phase 28 ordered after Phase 27 and before DuckDB smoke: `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp0-verify.sh`.
- No tracked file deletions were introduced by the task commits.
