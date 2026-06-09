---
phase: 27-lance-parquet-archival-readability-dataset-ingress
plan: 01
subsystem: ingress
tags: [rust, cargo, lance, parquet, source-ingress, dependency-boundary]
requires:
  - phase: 26-external-source-ingress-contract
    provides: Source-neutral ingress report contract and Vortex reference adapter pattern
provides:
  - Isolated `loom-lance-ingress` and `loom-parquet-ingress` workspace crates
  - Exact Lance, Parquet, futures, tokio, and tempfile workspace dependency pins
  - Source-neutral `SourceIngressAcceptedArtifact` bytes-plus-report handoff type
  - Adapter-local dependency boundary tests for direct Lance/Parquet placement
  - Initial Phase 27 scaffold and scope guard script
affects: [phase-27, phase-28, source-ingress, archival-readability]
tech-stack:
  added: [lance =7.0.0, parquet =58.3.0, futures =0.3.32, tokio =1.52.3, tempfile =3.27.0]
  patterns: [source-specific adapter crates own SDK dependencies, generic ingress APIs remain SDK-free]
key-files:
  created:
    - ingress/loom-lance-ingress/Cargo.toml
    - ingress/loom-lance-ingress/src/lib.rs
    - ingress/loom-lance-ingress/tests/dependency_boundary.rs
    - ingress/loom-parquet-ingress/Cargo.toml
    - ingress/loom-parquet-ingress/src/lib.rs
    - ingress/loom-parquet-ingress/tests/dependency_boundary.rs
    - scripts/lance-parquet-ingress-test.sh
  modified:
    - Cargo.toml
    - Cargo.lock
    - ingress/loom-source-ingress/src/lib.rs
    - ingress/loom-source-ingress/tests/source_ingress_contract.rs
key-decisions:
  - "Kept Lance and Parquet SDK dependencies isolated to adapter crates while using workspace-level exact pins for resolver consistency."
  - "Added the generic accepted-artifact handoff in `loom-source-ingress` without replacing the existing Vortex adapter-local handoff type."
  - "Left `scripts/mvp0-verify.sh` unwired until Plan 27-05, as required by the plan."
patterns-established:
  - "Adapter manifests may consume source SDK workspace pins; generic/core/ffi manifests must remain SDK-free."
  - "Phase 27 guard scripts build forbidden markers from pieces and scan explicit files to avoid self-matching."
requirements-completed: [PHASE-27]
duration: 13m
completed: 2026-06-08T20:25:00Z
---

# Phase 27 Plan 01: Adapter Crate Scaffolding and Dependency Guards Summary

**Lance and Parquet are now isolated local-file adapter crate boundaries with exact dependency pins, a source-neutral accepted-artifact handoff, and scaffold-level scope guards.**

## Performance

- **Duration:** 13 min
- **Started:** 2026-06-08T20:11:49Z
- **Completed:** 2026-06-08T20:25:00Z
- **Tasks:** 3
- **Files modified:** 11

## Accomplishments

- Registered `loom-lance-ingress` and `loom-parquet-ingress` as workspace members with exact researched dependency pins.
- Added `SourceIngressAcceptedArtifact { bytes, report }` to `loom-source-ingress` without introducing source SDK vocabulary.
- Added adapter-local dependency boundary tests proving direct `lance` and `parquet` dependencies only appear in their adapter manifests.
- Added `scripts/lance-parquet-ingress-test.sh` for Phase 27 scaffold compile/test smoke and source/API scope guards.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add adapter workspace crates and exact dependency pins** - `a6f4e15` (`feat`)
2. **Task 2: Add generic accepted artifact handoff type and adapter boundary tests** - `153db65` (`feat`)
3. **Task 3: Add initial Phase 27 guard script** - `4ab9415` (`test`)

## Files Created/Modified

- `Cargo.toml` - Registered adapter crates and exact workspace pins.
- `Cargo.lock` - Resolved Lance, Parquet, and adapter-local dependency graph.
- `ingress/loom-lance-ingress/Cargo.toml` - Lance adapter dependency boundary.
- `ingress/loom-lance-ingress/src/lib.rs` - Lance local-file boundary documentation.
- `ingress/loom-lance-ingress/tests/dependency_boundary.rs` - Direct dependency placement and generic vocabulary guards.
- `ingress/loom-parquet-ingress/Cargo.toml` - Parquet adapter dependency boundary.
- `ingress/loom-parquet-ingress/src/lib.rs` - Parquet local-file boundary documentation.
- `ingress/loom-parquet-ingress/tests/dependency_boundary.rs` - Direct dependency placement and generic vocabulary guards.
- `ingress/loom-source-ingress/src/lib.rs` - Added source-neutral accepted-artifact handoff.
- `ingress/loom-source-ingress/tests/source_ingress_contract.rs` - Added handoff contract coverage.
- `scripts/lance-parquet-ingress-test.sh` - Initial Phase 27 scaffold and scope guard.

## Verification

- `cargo check -p loom-lance-ingress -p loom-parquet-ingress` passed.
- `cargo test -p loom-source-ingress --test source_ingress_contract` passed.
- `cargo test -p loom-lance-ingress --test dependency_boundary` passed.
- `cargo test -p loom-parquet-ingress --test dependency_boundary` passed.
- `bash -n scripts/lance-parquet-ingress-test.sh` passed.
- `bash scripts/lance-parquet-ingress-test.sh` passed.

## Decisions Made

- Used the researched exact pins without feature adjustment; Cargo accepted `lance = "=7.0.0"` with default features disabled and `parquet = "=58.3.0"` with only `arrow`.
- Kept `tokio` and `tempfile` as adapter dev-dependencies, and `futures` as a Lance adapter dependency for later async scan collection.
- Kept the Phase 27 guard out of `scripts/mvp0-verify.sh`; Plan 27-05 owns release-gate wiring.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `gsd-tools` was not on PATH in this shell. Used `node /Users/macintoshhd/.codex/gsd-core/bin/gsd-tools.cjs` for GSD state queries instead.
- `.planning/STATE.md` had pre-existing uncommitted workflow edits before execution. The task commits left it unstaged; the final metadata step reconciles planning state.

## Known Stubs

None. Stub scan found only color fallback empty-string assignments in the shell guard, which are runtime formatting defaults rather than user-visible placeholders.

## Threat Flags

None. New source SDK dependency and guard-script surfaces are the planned Phase 27 trust boundaries covered by T-27-01-01 through T-27-01-04.

## TDD Gate Notes

The plan frontmatter is `type: execute`, while individual tasks were marked `tdd="true"`. The implementation added focused tests/guards before final verification for Task 2 and Task 3, but did not create separate RED commits because Task 1 and Task 3 are scaffold/guard tasks rather than behavior-transforming APIs.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 27-02 can implement Parquet fact extraction inside `loom-parquet-ingress` while relying on the common accepted-artifact handoff and the dependency boundary guards added here. Plan 27-03 can do the same for Lance without touching generic/core/ffi/public surfaces.

## Self-Check: PASSED

- Summary file exists at `.planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-01-SUMMARY.md`.
- Created files exist for both adapter crate manifests and the Phase 27 guard script.
- Task commits exist: `a6f4e15`, `153db65`, `4ab9415`.

---
*Phase: 27-lance-parquet-archival-readability-dataset-ingress*
*Completed: 2026-06-08T20:25:00Z*
