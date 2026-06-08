---
phase: 25-native-equivalence-cache-and-fallback-hardening
plan: 02
subsystem: runtime
tags: [rust, duckdb-runtime, native-cache, ffi-diagnostics, runtime-cache-key]

requires:
  - phase: 25-native-equivalence-cache-and-fallback-hardening
    provides: RuntimeCacheKey::compatibility_with and RuntimeCacheCompatibilityStatus from 25-01
provides:
  - Rust-owned process-local DuckDB native preparation cache
  - Deterministic cache hit, miss, insert, mismatch, and non-cacheable diagnostics
  - Cache behavior and public header leakage regression tests
affects: [duckdb-runtime, native-backend, ffi-tests, phase-25]

tech-stack:
  added: []
  patterns:
    - std::sync::OnceLock plus Mutex protects process-local runtime cache state
    - RuntimeCacheKey::compatibility_with is the only cache reuse validator
    - Cache evidence flows through existing internal diagnostics rather than public C APIs

key-files:
  created:
    - crates/loom-ffi/tests/duckdb_runtime_cache.rs
  modified:
    - crates/loom-ffi/src/duckdb_runtime.rs
    - crates/loom-ffi/tests/duckdb_runtime_ffi.rs

key-decisions:
  - "Kept cache in-process and Rust-owned with no persistent format, eviction policy, path/mtime semantics, SQL flags, public C API, or package additions."
  - "Stored accepted NativeBackendReport preparation evidence only; native buffers are rebuilt and compared before every returned native route."
  - "Failed, cancelled, fallback, skipped/failed toolchain, missing facts, and output mismatch routes emit cache-non-cacheable and do not insert entries."

patterns-established:
  - "Cache diagnostics are ordinary DuckDbRuntimeDiagnostic values under $.cache.native_preparation."
  - "Cache tests serialize around the shared process-local cache and clear it before each case."

requirements-completed: [PHASE-25]

duration: 10m32s
completed: 2026-06-08
---

# Phase 25 Plan 02: Native Preparation Cache Summary

**Rust-owned DuckDB native preparation cache with verifier-compatible reuse checks and internal-only cache diagnostics**

## Performance

- **Duration:** 10m32s
- **Started:** 2026-06-08T17:51:21Z
- **Completed:** 2026-06-08T18:01:53Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Added a process-local native preparation cache in `duckdb_runtime.rs`, keyed by `RuntimeCacheKey.stable_id` and validated with `RuntimeCacheKey::compatibility_with`.
- Inserted cache entries only after accepted backend preparation, successful native/reference comparison, and non-empty native buffers for the requested shape.
- Added focused cache tests for miss/insert/hit, projection and policy drift, canonical-input mismatch, non-cacheable routes, and post-hit output validation.
- Extended FFI diagnostics tests so existing internal prepare accessors expose cache miss, hit, and non-cacheable evidence while `loom.h` remains free of cache/API creep.

## Task Commits

1. **Task 1: Add cache-hit and invalidation tests** - `ca55f43` (test)
2. **Task 2: Implement Rust-owned in-process preparation cache** - `ebcd824` (feat)
3. **Task 3: Expose cache evidence through existing internal diagnostics** - `77bba11` (test)

## Files Created/Modified

- `crates/loom-ffi/src/duckdb_runtime.rs` - Adds process-local cache storage, lookup/insert logic, compatibility validation, and cache diagnostics.
- `crates/loom-ffi/tests/duckdb_runtime_cache.rs` - New integration tests for cache hit, miss, invalidation, mismatch, non-cacheable routes, and comparison-after-hit behavior.
- `crates/loom-ffi/tests/duckdb_runtime_ffi.rs` - Adds FFI diagnostic coverage for cache evidence and stricter public-header leakage checks.
- `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-02-SUMMARY.md` - Execution summary.

## Verification

- `cargo test -p loom-ffi --test duckdb_runtime_cache --no-run` - passed
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime_cache` - passed, 5 tests
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime_ffi --test duckdb_runtime_cache` - passed, 17 tests
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime_ffi --test duckdb_runtime` - passed, 21 tests

## Decisions Made

- The cache stores cloned `NativeBackendReport` preparation evidence and the exact `RuntimeCacheKey`; it does not store C++ vectors, external pointers, or persistent artifacts.
- Cache hits skip backend preparation reuse only, then still produce JIT/test output and run `compare_production_jit_output` before returning native buffers.
- Cache key mismatches remove the stale in-process entry and continue through normal preparation, preventing reuse when stable id and canonical input disagree.
- Public cache controls were not added; all evidence remains on the existing internal diagnostic path.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Serialized cache integration tests**
- **Found during:** Task 2
- **Issue:** Rust integration tests run concurrently, so a shared process-local cache made clear/prepare assertions race between tests.
- **Fix:** Added a test-local mutex and per-test cache clear helper in `duckdb_runtime_cache.rs`.
- **Files modified:** `crates/loom-ffi/tests/duckdb_runtime_cache.rs`
- **Verification:** `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime_cache`
- **Committed in:** `ebcd824`

**2. [Rule 1 - Bug] Made FFI cache diagnostic test independent of local native tool availability**
- **Found during:** Task 3
- **Issue:** The initial FFI test assumed toolchain-skipped native prepare would be non-cacheable, but this environment can produce an accepted route.
- **Fix:** Asserted cache miss on the native prepare, asserted cache-non-cacheable via a deterministic fallback route, then seeded a cache hit explicitly through the Rust bridge.
- **Files modified:** `crates/loom-ffi/tests/duckdb_runtime_ffi.rs`
- **Verification:** `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime_ffi --test duckdb_runtime_cache`
- **Committed in:** `77bba11`

**Total deviations:** 2 auto-fixed (2 Rule 1 bugs)
**Impact on plan:** Both fixes were test correctness/stability adjustments inside the owned files. No scope expansion.

## Known Stubs

None found in the files created or modified for this plan.

## Threat Flags

None beyond the plan threat model. The new cache surface is the planned in-process runtime cache boundary and has no public API or persistent storage.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

The DuckDB Rust bridge now has cache evidence and invalidation behavior suitable for downstream route-reporting and fallback hardening work. Later phases can consume diagnostics without owning cache policy in C++.

## Self-Check: PASSED

- Found `crates/loom-ffi/src/duckdb_runtime.rs`
- Found `crates/loom-ffi/tests/duckdb_runtime_cache.rs`
- Found `crates/loom-ffi/tests/duckdb_runtime_ffi.rs`
- Found `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-02-SUMMARY.md`
- Found task commits `ca55f43`, `ebcd824`, and `77bba11`

---
*Phase: 25-native-equivalence-cache-and-fallback-hardening*
*Completed: 2026-06-08*
