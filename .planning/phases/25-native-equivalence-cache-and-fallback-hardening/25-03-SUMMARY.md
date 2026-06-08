---
phase: 25-native-equivalence-cache-and-fallback-hardening
plan: 03
subsystem: testing
tags: [rust, loom-ffi, loom-native-melior, native-equivalence, runtime-cache]

requires:
  - phase: 25-native-equivalence-cache-and-fallback-hardening
    provides: RuntimeCacheKey compatibility and in-process native preparation cache from 25-01/25-02
provides:
  - Supported primitive helper equivalence matrix for native buffers vs interpreter/reference bytes
  - Fallback and fail-closed negative route matrix for unsupported native programs
  - Cache replay and post-error determinism tests for native preparation reuse
affects: [phase-25, native-runtime, duckdb-runtime-cache, production-backend-jit]

tech-stack:
  added: []
  patterns:
    - Rust helper injection for routes SQL cannot naturally trigger
    - Native success asserted only after byte/type/metadata comparison to interpreter/reference buffers
    - Failed native routes remain non-cacheable and emit no partial output

key-files:
  created:
    - .planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-03-SUMMARY.md
  modified:
    - crates/loom-ffi/tests/duckdb_runtime.rs
    - crates/loom-ffi/tests/duckdb_runtime_cache.rs
    - crates/loom-native-melior/tests/production_backend_jit.rs

key-decisions:
  - "Interpreter/reference zeroed value buffers remain the primary oracle for native helper success; existing fixture evidence is not broadened into arbitrary Vortex semantic compatibility."
  - "Unsupported strings, nullability, compressed layouts, predicates, projections, and split routes are recorded as fallback/fail-closed evidence rather than native-success claims."
  - "Cache evidence proves reuse correctness and deterministic post-error behavior, not native-speed improvement."

patterns-established:
  - "Compare native buffer builder id, Arrow type, and bytes for cache replay and primitive equivalence assertions."
  - "Use runtime/backend helper injection for unsupported predicate, split, malformed artifact, mismatch, and cancellation diagnostics."

requirements-completed: [PHASE-25]

duration: 45min
completed: 2026-06-08T18:13:26Z
---

# Phase 25 Plan 03: Native Equivalence, Cache, and Fallback Matrix Summary

**Verifier-gated native helper tests now prove primitive buffer equivalence, unsupported-route fail-closure, and cache replay safety without broadening native semantics.**

## Performance

- **Duration:** 45 min
- **Started:** 2026-06-08T17:28:00Z
- **Completed:** 2026-06-08T18:13:26Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Added `equivalence_matrix` coverage for raw non-null Int32, Int32/Int64/Float32/Float64 table output, and projected output-order mapping before native buffer comparison.
- Extended DuckDB runtime negative-route tests for missing facts, strict fallback, cancellation, native-output mismatch, unsupported strings/compressed layouts, and helper-injected projection/predicate/split fail-closed routes.
- Added backend JIT diagnostics for nullable primitive shapes, missing artifacts, malformed empty-column artifacts, mismatch, cancellation, and toolchain skip/fail behavior.
- Strengthened cache tests so hits compare the same validated native buffer bytes and failure routes cannot poison later valid scans.

## Task Commits

1. **Tasks 1-3: Native equivalence, fallback, backend, and cache replay tests** - `e9609e0` (test)
2. **Plan summary** - recorded in the docs commit for this summary.

## Files Created/Modified

- `crates/loom-ffi/tests/duckdb_runtime.rs` - Primitive equivalence matrix plus fallback/fail-closed route diagnostics.
- `crates/loom-ffi/tests/duckdb_runtime_cache.rs` - Cache hit buffer replay checks and deterministic post-error scan assertions.
- `crates/loom-native-melior/tests/production_backend_jit.rs` - Backend invalid/malformed/nullable artifact negative coverage.
- `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-03-SUMMARY.md` - Execution record.

## Verification

- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime` - passed, 14 tests.
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime_cache` - passed, 7 tests.
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-native-melior --test production_backend_jit` - passed, 8 tests.

Focused checks also passed before the full commands:

- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime equivalence_matrix`
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime prepare_routes`

## Decisions Made

- Kept all changes test-only because the existing runtime already exposes the helper seams needed for the plan.
- Used zeroed interpreter/reference value buffers as the equality oracle for the current production backend slice.
- Kept unsupported strings, nullability, bitpack/compressed layouts, predicate, projection, split, malformed artifact, and cancellation paths as negative evidence only.
- Did not run workspace-wide `cargo fmt` because unrelated files outside the allowed scope currently fail `cargo fmt --check`; only the three owned Rust test files were formatted with `rustfmt`.

## Deviations from Plan

None - plan scope was preserved. SQL-inaccessible routes were covered with Rust helper injection as requested.

## Issues Encountered

- `cargo fmt --check` reports pre-existing formatting drift in files outside Plan 25-03 ownership (`crates/loom-cli/src/main.rs`, `crates/loom-fixtures/src/bin/loom_fixture_timing.rs`, and `crates/loom-native-melior/tests/decode_dialect_manifest.rs`). Those files were not modified.
- Initial helper assertions used a stale `RuntimeEmissionDisposition` variant and an over-specific nullable diagnostic message. Both were corrected inside owned test files before verification.

## Known Stubs

None found in files created or modified by this plan.

## Threat Flags

None. The changes add tests only and introduce no new endpoint, auth path, file access pattern, schema, public mode switch, native kernel, or Vortex dependency in `loom-ffi`.

## Next Phase Readiness

Phase 25 now has helper-level evidence that supported primitive native buffers are trusted only after interpreter/reference comparison, unsupported native shapes remain fallback/fail-closed evidence, and cache hits replay validated preparations without poisoning later scans.

## Self-Check: PASSED

- Found summary file: `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-03-SUMMARY.md`
- Found task commit: `e9609e0`

---
*Phase: 25-native-equivalence-cache-and-fallback-hardening*
*Completed: 2026-06-08*
