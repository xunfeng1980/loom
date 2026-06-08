---
phase: 26-external-source-ingress-contract
plan: 02
subsystem: source-ingress
tags: [rust, source-ingress, vortex-adapter, diagnostics, tdd]

requires:
  - phase: 26-external-source-ingress-contract
    provides: Source-neutral ingress contract crate from Plan 26-01
  - phase: 18-complete-vortex-reader
    provides: Vortex reader facts, support classification, diagnostics, and fail-closed behavior
  - phase: 21-expanded-vortex-encoding-coverage
    provides: Vortex coverage, emission disposition, and lowering disposition vocabulary
provides:
  - Vortex reader facts mapped into SourceFacts and SourceCoverage
  - Vortex diagnostics mapped into source-neutral diagnostic code families
  - Source-neutral buffer/path helpers in loom-vortex-ingress without breaking existing Vortex APIs
  - Tests proving supported, unsupported, rejected, compatibility, and generic-neutrality behavior
affects: [phase-26, phase-27, source-ingress-contract, vortex-ingress]

tech-stack:
  added: [loom-source-ingress local path dependency for loom-vortex-ingress]
  patterns:
    - Source-specific adapters convert into dependency-light Source* contract types
    - Valid unsupported inputs keep facts but expose no artifact verification or oracle evidence
    - Rejected malformed inputs expose diagnostics and no trusted facts

key-files:
  created:
    - crates/loom-vortex-ingress/src/source_contract.rs
    - crates/loom-vortex-ingress/tests/source_ingress_contract.rs
  modified:
    - Cargo.lock
    - crates/loom-vortex-ingress/Cargo.toml
    - crates/loom-vortex-ingress/src/lib.rs
    - crates/loom-source-ingress/src/lib.rs

key-decisions:
  - "Vortex remains the first adapter while source-neutral helpers live beside, not instead of, existing Vortex APIs."
  - "Plan 26-02 maps facts and dispositions only; artifact verification and oracle acceptance remain deferred to Plan 26-03."
  - "UnsupportedConversion diagnostics are classified as the generic Conversion family, not Support."

patterns-established:
  - "Adapter mapping modules should return SourceFacts for valid inputs and SourceIngressReport errors for malformed rejected inputs."
  - "Source-neutral reports for unsupported valid inputs keep facts but use no artifact verification or oracle evidence."

requirements-completed: [PHASE-26]

duration: 9min
completed: 2026-06-08
---

# Phase 26 Plan 02: Vortex Source Mapping Summary

**Vortex reader facts now project into the dependency-light source-ingress contract while preserving the existing Vortex API surface.**

## Performance

- **Duration:** 9 min
- **Started:** 2026-06-08T19:04:18Z
- **Completed:** 2026-06-08T19:13:05Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Added `crates/loom-vortex-ingress/src/source_contract.rs`, mapping `VortexReaderFacts`, `VortexEncodingCoverage`, reader diagnostics, and ingress reports into `loom-source-ingress` types.
- Added source-neutral buffer/path helpers and conversion helper re-exports from `loom-vortex-ingress` while leaving `reader_facts_from_vortex_buffer`, `inspect_vortex_buffer`, and `emit_supported_lmc1_from_vortex_buffer` intact.
- Added focused tests for accepted primitive/table facts, unsupported UTF-8 facts, rejected malformed buffers, diagnostic family mapping, old/new API compatibility, and generic-crate neutrality.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Add failing Vortex source mapping tests** - `7212c2f` (test)
2. **Task 1 GREEN: Add Vortex-to-source facts mapping** - `a1a8b08` (feat)
3. **Task 2 RED: Preserve diagnostics and rejected semantics tests** - `15e947e` (test)
4. **Task 2 GREEN: Fix conversion diagnostic family** - `1afe1ec` (fix)
5. **Task 3 RED: Add compatibility and neutrality guards** - `c25d575` (test)
6. **Task 3 GREEN: Export source contract conversion helpers** - `10c20fe` (feat)

## Files Created/Modified

- `Cargo.lock` - Recorded the local `loom-source-ingress` dependency for `loom-vortex-ingress`.
- `crates/loom-vortex-ingress/Cargo.toml` - Added the local source-ingress contract dependency.
- `crates/loom-vortex-ingress/src/lib.rs` - Exported the source contract module and helper functions without changing existing Vortex APIs.
- `crates/loom-vortex-ingress/src/source_contract.rs` - New adapter mapping Vortex reader facts, coverage, diagnostics, and reports into source-neutral contract types.
- `crates/loom-vortex-ingress/tests/source_ingress_contract.rs` - New TDD contract coverage for supported, unsupported, rejected, compatibility, and generic-neutrality behavior.
- `crates/loom-source-ingress/src/lib.rs` - Corrected `UnsupportedConversion` diagnostic family classification to `Conversion`.

## Decisions Made

- Kept Vortex identity values source-neutral in the generic mapping (`external-source` plus source kind/version) while retaining Vortex-specific names only in the adapter crate.
- Used source-neutral report fields directly for Plan 26-02 mapping evidence; verifier-accepted artifact and oracle evidence remain intentionally absent until Plan 26-03.
- Re-exported conversion helpers from `loom-vortex-ingress` root so callers can import old Vortex APIs and new Source* helpers side by side.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed source conversion diagnostic family**
- **Found during:** Task 2 (Preserve diagnostics and rejected semantics)
- **Issue:** `SourceDiagnosticCode::UnsupportedConversion` was classified as `SourceDiagnosticFamily::Support`, but Phase 26 requires conversion diagnostics to remain a source-neutral conversion family.
- **Fix:** Updated the generic contract family mapping to `SourceDiagnosticFamily::Conversion`.
- **Files modified:** `crates/loom-source-ingress/src/lib.rs`
- **Verification:** `cargo test -p loom-vortex-ingress --test source_ingress_contract`
- **Committed in:** `1afe1ec`

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** The fix was required for diagnostic correctness and did not introduce source-specific vocabulary or new source implementations.

## Issues Encountered

- `Cargo.lock` changed when adding the local dependency; it was included in the Task 1 GREEN commit as task-related dependency metadata.
- `cargo fmt` briefly formatted an unrelated existing generic contract test file; that formatting-only change was discarded before final verification.

## Known Stubs

None.

## Authentication Gates

None.

## Verification

All required plan checks passed:

- `cargo test -p loom-vortex-ingress --test source_ingress_contract`
- `cargo test -p loom-vortex-ingress --test reader_facts_contract`
- `cargo test -p loom-vortex-ingress --test single_column_to_loom --test table_to_loom`
- `! rg -n "Vortex|vortex" crates/loom-source-ingress/src crates/loom-source-ingress/tests`

## TDD Gate Compliance

- RED commit present before Task 1 GREEN: `7212c2f` -> `a1a8b08`
- RED commit present before Task 2 GREEN: `15e947e` -> `1afe1ec`
- RED commit present before Task 3 GREEN: `c25d575` -> `10c20fe`

## Self-Check: PASSED

- Key created files exist on disk.
- All six task commits are present in git history.
- Required plan verification commands passed after implementation.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 26-03 can consume `SourceFacts`, `SourceCoverage`, and source-neutral reports from the Vortex adapter to add verifier-accepted artifact and oracle handoff evidence. No Lance, Parquet, Iceberg, MCAP, Zarr, LeRobot, object-store, host-engine, predicate-pushdown, split-execution, public SQL/API, or native-kernel work was implemented.

---
*Phase: 26-external-source-ingress-contract*
*Completed: 2026-06-08*
