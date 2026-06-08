---
phase: 25-native-equivalence-cache-and-fallback-hardening
plan: 01
subsystem: runtime
tags: [rust, loom-core, runtime-cache, diagnostics, fallback-policy]

requires:
  - phase: 22-host-native-runtime-abi-and-execution-policy
    provides: Runtime ABI vocabulary, cache key inputs, and execution policy diagnostics.
provides:
  - Host-neutral RuntimeCacheKey compatibility statuses for hit, miss, and key-mismatch.
  - Expanded cache identity mutation coverage across artifact, facts, lowering, backend, query, split, and policy inputs.
  - Focused runtime-owned strict fallback and unsupported-input diagnostic assertions.
affects: [phase-25-cache, phase-25-duckdb-runtime, runtime-abi]

tech-stack:
  added: []
  patterns:
    - Host-neutral compatibility helpers live in loom-core runtime_abi, not host adapters.
    - Cache mismatch diagnostics are emitted only for stable-id collision with canonical input drift.

key-files:
  created:
    - .planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-01-SUMMARY.md
  modified:
    - crates/loom-core/src/runtime_abi.rs
    - crates/loom-core/tests/runtime_cache_key.rs
    - crates/loom-core/tests/runtime_execution_policy.rs

key-decisions:
  - "Runtime cache compatibility is exact-key based: stable_id mismatch is a miss, stable_id match plus canonical_input mismatch is key-mismatch."
  - "Concurrency cache identity now includes ParallelSplits requested_workers while preserving the existing as_str display vocabulary."
  - "Fallback and unsupported-input diagnostics remain asserted in Rust runtime tests instead of being duplicated in host adapters."

patterns-established:
  - "RuntimeCacheKey::compatibility_with returns host-neutral status plus diagnostics."
  - "RuntimeCacheCompatibilityStatus::as_str exposes stable hit/miss/key-mismatch vocabulary."
  - "Runtime cache mutation tests should change one input dimension at a time."

requirements-completed: [PHASE-25]

duration: 25min
completed: 2026-06-08
---

# Phase 25 Plan 01: Native Equivalence Cache Contract Summary

**Exact runtime cache compatibility with host-neutral hit/miss/key-mismatch statuses and Rust-owned fallback diagnostics**

## Performance

- **Duration:** 25 min
- **Started:** 2026-06-08T17:24:00Z
- **Completed:** 2026-06-08T17:49:25Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Added `RuntimeCacheCompatibilityStatus` and `RuntimeCacheCompatibility` beside `RuntimeCacheKey`.
- Added `RuntimeCacheKey::compatibility_with`, returning hit, miss, or key-mismatch without host-specific cache storage semantics.
- Emitted `RuntimeDiagnosticCode::CacheKeyMismatch` at `$.cache.key` only when stable ids match but canonical inputs differ.
- Expanded mutation coverage so every `RuntimeCacheKeyInput` dimension affects identity, including backend identity and safety policy fields.
- Tightened strict fallback, unsupported predicate, and invalid split assertions to prove diagnostics remain owned by `loom-core`.

## Task Commits

1. **Tasks 1-3: Runtime cache compatibility, mutation coverage, and fallback diagnostics** - `96f21c4` (feat)

## Files Created/Modified

- `crates/loom-core/src/runtime_abi.rs` - Added cache compatibility status/report types, compatibility helper, and worker-count-sensitive concurrency key material.
- `crates/loom-core/tests/runtime_cache_key.rs` - Added compatibility hit/miss/key-mismatch coverage and one-field mutation assertions for all cache-key input dimensions.
- `crates/loom-core/tests/runtime_execution_policy.rs` - Added focused fallback-disabled, lowering-unsupported, unsupported-predicate, and invalid-split diagnostic assertions.
- `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-01-SUMMARY.md` - Recorded execution results.

## Verification

- `cargo test -p loom-core --test runtime_cache_key` - passed, 6 tests.
- `cargo test -p loom-core --test runtime_execution_policy --test runtime_scan_planning` - passed, 12 tests across 2 test binaries.

## Decisions Made

- Kept cache compatibility in `loom-core` as reusable runtime vocabulary; storage, eviction, path/mtime, persistent cache format, and DuckDB types remain out of scope.
- Treated stable-id mismatch as a normal cache miss with no diagnostics, even when canonical input differs, because only same-stable-id canonical drift indicates key corruption or collision.
- Preserved `ConcurrencyPolicy::as_str()` for existing diagnostic/display callers and added `as_key()` for canonical cache identity.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Included requested worker count in concurrency cache identity**
- **Found during:** Task 2 cache mutation coverage.
- **Issue:** `ConcurrencyPolicy::ParallelSplits { requested_workers }` previously canonicalized only as `parallel-splits`, so changing worker count would not change `RuntimeCacheKey`.
- **Fix:** Added `ConcurrencyPolicy::as_key()` and used it in `canonical_cache_input`.
- **Files modified:** `crates/loom-core/src/runtime_abi.rs`, `crates/loom-core/tests/runtime_cache_key.rs`
- **Verification:** `cargo test -p loom-core --test runtime_cache_key`
- **Committed in:** `96f21c4`

**Total deviations:** 1 auto-fixed Rule 1 issue.
**Impact on plan:** Required for Task 2 correctness; no scope expansion beyond owned runtime cache identity files.

## Issues Encountered

- `gsd-tools` was unavailable on PATH in this shell, so SDK-backed state/roadmap updates could not run.
- Per the user's strict ownership scope, `.planning/STATE.md`, `.planning/ROADMAP.md`, and other Phase 25 plan files were not modified.

## User Setup Required

None.

## Next Phase Readiness

Phase 25 cache storage and DuckDB integration plans can consume the host-neutral `RuntimeCacheKey::compatibility_with` contract and stable runtime diagnostics without adding host-side policy branches.

## Self-Check: PASSED

- Found all owned source/test files and `25-01-SUMMARY.md` on disk.
- Found implementation commit `96f21c4` in git history.
- Confirmed implementation commit did not delete tracked files.
- Stub scan found no introduced TODO/FIXME/placeholder stubs; the only match was Rust `format!` placeholder syntax in `canonical_cache_input`.

---
*Phase: 25-native-equivalence-cache-and-fallback-hardening*
*Completed: 2026-06-08*
