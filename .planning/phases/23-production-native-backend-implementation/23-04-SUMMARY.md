---
phase: 23-production-native-backend-implementation
plan: 04
subsystem: native-backend
tags: [jit, native-backend, interpreter-equivalence, cancellation, diagnostics]

requires:
  - phase: 23-production-native-backend-implementation
    provides: 23-03 production backend pipeline and accepted backend artifacts
provides:
  - Production JIT seed API over accepted NativeBackendReport artifacts
  - Primitive reference-buffer equivalence checks
  - Cancellation and unsupported-shape JIT diagnostics
affects: [phase-23, phase-24, native-backend, jit]

tech-stack:
  added: []
  patterns: [accepted backend artifact only JIT entry, deterministic primitive reference output]

key-files:
  created:
    - crates/loom-native-melior/tests/production_backend_jit.rs
  modified:
    - crates/loom-native-melior/src/backend.rs
    - crates/loom-native-melior/src/jit.rs

key-decisions:
  - "Production JIT seed accepts NativeBackendReport artifacts, not raw MLIR or verifier facts."
  - "Current seed outputs deterministic primitive zero buffers matching Phase 20 production MLIR semantics."
  - "Cancellation and missing/invalid JIT artifacts return NativeBackendReport diagnostics."

patterns-established:
  - "execute_prepared_production_jit returns output or NativeBackendReport failure."
  - "compare_production_jit_output reports NativeOutputMismatch through backend diagnostics."

requirements-completed: []

duration: 6min
completed: 2026-06-08
---

# Phase 23-04: Verifier-Gated JIT Execution Seed Summary

**Accepted backend artifacts can now enter a narrow production JIT seed with cancellation, toolchain, unsupported-shape, and reference-output diagnostics**

## Performance

- **Duration:** 6 min
- **Started:** 2026-06-08T15:07:48Z
- **Completed:** 2026-06-08T15:13:38Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- Added production JIT seed types and entry points:
  `ProductionJitOptions`, `ProductionJitOutput`,
  `execute_prepared_production_jit`, and `compare_production_jit_output`.
- Extended backend diagnostics with invalid artifact, JIT unavailable, missing
  symbol, and native-output mismatch codes.
- Enforced that production JIT execution starts from an accepted
  `NativeBackendReport` artifact with the expected `loom_decode_build_buffers`
  entry symbol.
- Added cancellation checks before preparation/execution and typed cancellation
  reports.
- Added tests for accepted-artifact execution, preflight-only rejection, missing
  symbol, unsupported UTF8-like facts, cancellation, output equivalence, mismatch
  diagnostics, and strict toolchain behavior.

## Task Commits

1. **Tasks 1-3: Production JIT seed, equivalence checks, and cancellation coverage** - `e648248`
2. **Plan metadata:** pending summary commit

## Files Created/Modified

- `crates/loom-native-melior/src/backend.rs` - Added JIT-related backend diagnostic codes.
- `crates/loom-native-melior/src/jit.rs` - Added production JIT seed and reference-output comparison.
- `crates/loom-native-melior/tests/production_backend_jit.rs` - Added focused production JIT tests.

## Decisions Made

- The Phase 23 JIT seed is intentionally narrow. It validates artifact shape and
  returns deterministic primitive zero buffers that match the current production
  MLIR lowering semantics; it does not claim a full ExecutionEngine invocation.
- Missing local toolchain is explicit: non-strict execution may return
  `SkippedToolchain` only when `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`; strict execution
  fails closed.
- Unsupported shapes are checked before toolchain probing so invalid native
  artifacts do not hide behind toolchain availability.

## Deviations from Plan

The plan referenced `crates/loom-core/tests/vortex_encoding_coverage.rs`, but the
current repository does not have that test target. Verification used existing
`production_native_lowering` and `jit` tests.

`cargo fmt --package loom-native-melior` reformatted a 23-02 test file; that
format-only diff was inspected and reverted before committing.

**Total deviations:** 2 execution/formatting adjustments.  
**Impact on plan:** None; JIT seed, cancellation, unsupported-shape, and
equivalence diagnostics are covered.

## Issues Encountered

None beyond the missing referenced test file noted above.

## User Setup Required

None.

## Verification

- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-native-melior --test production_backend_jit`
- `cargo test -p loom-core --test production_native_lowering`
- `cargo test -p loom-native-melior --test jit`
- `cargo test -p loom-native-melior jit`
- `git diff --check`

All verification passed.

## Next Phase Readiness

Phase 23 can proceed to 23-05. The backend now has contract, ODS evidence,
pipeline identity, LLVM lowering evidence, and a narrow JIT seed; the next plan
should wire the production backend gate into the release gate and write the final
backend report.

---
*Phase: 23-production-native-backend-implementation*
*Completed: 2026-06-08*
