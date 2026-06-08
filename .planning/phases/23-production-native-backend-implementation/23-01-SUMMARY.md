---
phase: 23-production-native-backend-implementation
plan: 01
subsystem: native-backend
tags: [runtime-abi, melior, llvm, jit, cache-key, diagnostics]

requires:
  - phase: 22-host-native-runtime-abi-and-execution-policy
    provides: RuntimePlan, RuntimeCacheKey, execution decisions, diagnostics, and runtime policy model
provides:
  - Phase 23 backend contract document
  - Native backend request/report/identity/cancellation model
  - Runtime-plan bridge validation tests
affects: [phase-23, phase-24, native-backend, duckdb-adapter]

tech-stack:
  added: []
  patterns: [host-neutral backend preflight over RuntimePlan and RuntimeCacheKey]

key-files:
  created:
    - .planning/phases/23-production-native-backend-implementation/23-BACKEND-CONTRACT.md
    - crates/loom-native-melior/src/backend.rs
    - crates/loom-native-melior/tests/production_backend_contract.rs
  modified:
    - crates/loom-native-melior/src/lib.rs

key-decisions:
  - "Backend requests require a native-candidate RuntimePlan and RuntimeCacheKey before any backend work."
  - "Public loom_runtime.h remains unfrozen; natural wrappers stay internal/test-oriented."
  - "Cancellation is modeled at backend preflight before long-running JIT work exists."

patterns-established:
  - "NativeBackendReport carries runtime plan/cache identity plus backend identity."
  - "Backend diagnostics use stable code strings and JSON-path-like paths."

requirements-completed: []

duration: 4min
completed: 2026-06-08
---

# Phase 23-01: Backend Contract and Runtime-Plan Bridge Summary

**Host-neutral native backend preflight over Phase 22 `RuntimePlan` and `RuntimeCacheKey`, with stable identity, diagnostics, and cancellation modeling**

## Performance

- **Duration:** 4 min
- **Started:** 2026-06-08T14:54:30Z
- **Completed:** 2026-06-08T14:58:03Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Wrote `23-BACKEND-CONTRACT.md` defining scope, inputs, backend identity,
  cancellation, cache identity, diagnostics, lifecycle, natural wrappers, and
  non-goals.
- Added `loom_native_melior::backend` with `NativeBackendRequest`,
  `NativeBackendIdentity`, capabilities, cancellation state, artifacts,
  diagnostics, reports, and `validate_backend_request`.
- Added focused tests proving native-candidate plans validate, interpreter and
  fail-closed plans reject before backend work, missing cache/lowering facts fail
  closed, unsupported facts reject, cancellation is distinct, and stable strings
  remain stable.

## Task Commits

1. **Task 1: Write the backend contract** - `4ce84d2`
2. **Tasks 2-3: Add backend model types and enforce runtime-plan bridge** - `5d08aec`
3. **Plan metadata:** pending summary commit

## Files Created/Modified

- `.planning/phases/23-production-native-backend-implementation/23-BACKEND-CONTRACT.md` - Phase 23 backend contract.
- `crates/loom-native-melior/src/backend.rs` - Backend request/report/identity/cancellation model and validation.
- `crates/loom-native-melior/src/lib.rs` - Exposes the backend module.
- `crates/loom-native-melior/tests/production_backend_contract.rs` - Contract and runtime-plan bridge tests.

## Decisions Made

- Backend preflight accepts only `RuntimeExecutionDecision::NativeCandidate` with
  no runtime diagnostics.
- `RuntimeCacheKey` is required before backend work; missing cache identity is a
  fail-closed diagnostic.
- `NativeBackendIdentity` records runtime ABI, backend version, expected/detected
  MLIR version, toolchain compatibility, optional target/layout, pipeline ID, and
  capabilities.
- Cancellation is represented by `NativeBackendCancellation` and returns
  `NativeBackendStatus::Cancelled` before lowering/JIT work.

## Deviations from Plan

The plan referenced `cargo test -p loom-core --test runtime_abi_decision`, but
the existing Phase 22 test file is named `runtime_execution_policy.rs`. Verification
used the existing test target instead.

**Total deviations:** 1 naming mismatch handled without scope change.  
**Impact on plan:** None; the intended Phase 22 runtime decision coverage ran and
passed.

## Issues Encountered

`cargo fmt --all` formatted two unrelated files. Those formatting-only changes
were inspected and reverted before committing, so no unrelated code was included.

## User Setup Required

None - no external service configuration required.

## Verification

- `rg -n "Scope|Inputs|Backend Identity|Cancellation|Cache Identity|Diagnostics|Lifecycle|Natural Wrappers|Non-Goals" .planning/phases/23-production-native-backend-implementation/23-BACKEND-CONTRACT.md`
- `rg -n "RuntimePlan|RuntimeCacheKey|loom_runtime\\.h|DuckDB|cache hardening" .planning/phases/23-production-native-backend-implementation/23-BACKEND-CONTRACT.md`
- `rg -n "NativeBackendRequest|NativeBackendIdentity|NativeBackendArtifact|NativeBackendReport|NativeBackendDiagnostic" crates/loom-native-melior/src`
- `cargo test -p loom-native-melior --test production_backend_contract`
- `cargo test -p loom-core --test runtime_execution_policy`
- `git diff --check`

All verification passed.

## Next Phase Readiness

Phase 23 can proceed to 23-02. The next plan should add compiled
`loom.decode` ODS/TableGen evidence and drift checks while consuming the backend
identity and diagnostics established here.

---
*Phase: 23-production-native-backend-implementation*
*Completed: 2026-06-08*
