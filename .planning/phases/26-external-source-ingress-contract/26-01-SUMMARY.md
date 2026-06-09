---
phase: 26-external-source-ingress-contract
plan: 01
subsystem: ingress
tags: [rust, source-ingress, contract, dependency-hygiene, tdd]

requires:
  - phase: 25-native-equivalence-cache-and-fallback-hardening
    provides: hardened verifier-backed artifact and native execution boundary that source ingress must not widen
provides:
  - dependency-light loom-source-ingress workspace crate
  - source-neutral ingress facts, diagnostics, status, emission, lowering, oracle, verifier handoff, and report vocabulary
  - executable accepted/unsupported/rejected fail-closed report invariant tests
  - dependency and source-vocabulary hygiene checks for the generic contract crate
affects: [phase-26, phase-27, source-ingress-contract, external-source-adapters]

tech-stack:
  added: []
  patterns:
    - dependency-free Rust contract crate
    - checked report constructors for fail-closed source ingress invariants
    - test-side dependency and vocabulary hygiene guard

key-files:
  created:
    - ingress/loom-source-ingress/Cargo.toml
    - ingress/loom-source-ingress/src/lib.rs
    - ingress/loom-source-ingress/tests/source_ingress_contract.rs
  modified:
    - Cargo.toml
    - Cargo.lock

key-decisions:
  - "Placed generic source ingress vocabulary in a new dependency-free loom-source-ingress crate instead of loom-core or a source-specific adapter crate."
  - "Kept artifact verifier handoff as plain source-contract data so loom-core remains downstream-source-neutral."
  - "Encoded accepted/unsupported/rejected report invariants in constructors and tests before any new source adapter implementation."

patterns-established:
  - "Accepted source ingress reports require facts, LMP1 or LMT1 emission, verifier-accepted artifact summary, and accepted oracle evidence."
  - "Unsupported source ingress reports may carry facts and diagnostics but always emit no artifact bytes."
  - "Rejected source ingress reports carry diagnostics only and expose no trusted facts, verifier acceptance, oracle acceptance, or artifact byte metadata."

requirements-completed: [PHASE-26]

duration: 3m27s
completed: 2026-06-09
---

# Phase 26 Plan 01: Source Ingress Contract Crate Summary

**Dependency-free `loom-source-ingress` contract crate with stable source-neutral ingress vocabulary, fail-closed report invariants, and dependency hygiene evidence.**

## Performance

- **Duration:** 3m27s
- **Started:** 2026-06-08T18:58:39Z
- **Completed:** 2026-06-08T19:02:06Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added `loom-source-ingress` as a workspace member with an empty runtime dependency section.
- Defined source-neutral public contract types for source identity, diagnostics, facts, schema/layout/segment/split facts, coverage, emission, lowering, oracle evidence, artifact verification summary, and ingress reports.
- Added checked accepted/unsupported/rejected report helpers that enforce fail-closed behavior before future source-specific adapters consume the contract.
- Added executable tests for stable vocabulary, report invariants, manifest dependency hygiene, and source-neutral public vocabulary.

## Task Commits

Each task was committed atomically with TDD red/green commits:

1. **Task 1 RED: Add failing source ingress crate contract test** - `093d7ba` (test)
2. **Task 1 GREEN: Add source ingress contract crate types** - `300483a` (feat)
3. **Task 2 RED: Add failing source ingress invariant tests** - `67e4940` (test)
4. **Task 2 GREEN: Enforce source ingress report invariants** - `a2d1399` (feat)
5. **Task 3 RED: Add failing dependency hygiene assertions** - `5598a79` (test)
6. **Task 3 GREEN: Document source ingress dependency hygiene** - `5f2a17f` (chore)

## Files Created/Modified

- `Cargo.toml` - Adds `ingress/loom-source-ingress` to workspace members.
- `Cargo.lock` - Records the new internal workspace package.
- `ingress/loom-source-ingress/Cargo.toml` - Declares the dependency-light generic contract crate with no runtime dependencies.
- `ingress/loom-source-ingress/src/lib.rs` - Defines the source-neutral contract vocabulary and checked report constructors.
- `ingress/loom-source-ingress/tests/source_ingress_contract.rs` - Covers stable vocabulary, report invariants, dependency hygiene, and source-neutral public vocabulary.

## Decisions Made

- New crate over core module: keeps source provenance/admission concepts outside `loom-core` while still giving Phase 27 a Vortex-free contract target.
- Plain verifier handoff data: avoids a `loom-core` dependency and keeps the artifact verifier downstream-source-neutral.
- Constructor-enforced invariants: accepted reports cannot be built through the helper unless artifact emission, verifier acceptance, and oracle evidence agree.

## Verification

Passed:

- `cargo test -p loom-source-ingress`
- `cargo tree -p loom-source-ingress | awk '/vortex|fastlanes|lance|parquet|iceberg|mcap|zarr|object_store|duckdb|melior/{found=1} END{exit found?1:0}'`
- `! rg -n "Vortex|vortex|Lance|Parquet|Iceberg|MCAP|Zarr|LeRobot" ingress/loom-source-ingress/src ingress/loom-source-ingress/tests`

## Deviations from Plan

None - plan executed exactly as written.

**Total deviations:** 0 auto-fixed.
**Impact on plan:** No scope change.

## Issues Encountered

None.

## Known Stubs

None. The contract contains descriptive optional fields by design, but no placeholder implementation blocks or unwired UI/data stubs.

## Threat Flags

None. The plan added source-contract data types and tests only; it introduced no network endpoint, auth path, credential surface, file-ingress implementation, public SQL/API, FFI, host-engine integration, or native kernel.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 26-02 can map the existing source-specific adapter facts into `loom-source-ingress` without changing this generic crate's dependency boundary. Later source adapters should target these types, declare oracle strategy, and emit only verifier-accepted `LMC1` artifacts for accepted reports.

## Self-Check: PASSED

- Summary file exists at `.planning/phases/26-external-source-ingress-contract/26-01-SUMMARY.md`.
- Task commits found: `093d7ba`, `300483a`, `67e4940`, `a2d1399`, `5598a79`, `5f2a17f`.
- Required verification re-ran successfully:
  - `cargo test -p loom-source-ingress`
  - `cargo tree -p loom-source-ingress | awk '/vortex|fastlanes|lance|parquet|iceberg|mcap|zarr|object_store|duckdb|melior/{found=1} END{exit found?1:0}'`
  - `! rg -n "Vortex|vortex|Lance|Parquet|Iceberg|MCAP|Zarr|LeRobot" ingress/loom-source-ingress/src ingress/loom-source-ingress/tests`

---
*Phase: 26-external-source-ingress-contract*
*Completed: 2026-06-09*
