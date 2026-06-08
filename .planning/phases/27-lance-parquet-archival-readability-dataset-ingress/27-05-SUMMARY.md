---
phase: 27-lance-parquet-archival-readability-dataset-ingress
plan: 05
subsystem: ingress
tags: [release-gate, lance, parquet, archival-readability, legacy-fixtures]
requires:
  - phase: 27-lance-parquet-archival-readability-dataset-ingress
    plan: 04
    provides: Actual older-version Lance and Parquet fixtures with paired Loom artifacts
provides:
  - Focused Phase 27 closeout gate
  - Main release verifier wiring for Phase 27 after Phase 26 and before DuckDB smoke
  - Final archival readability report with current and older-version fixture evidence
affects: [phase-27, source-ingress, mvp0-release-gate, phase-28]
tech-stack:
  added: []
  patterns: [fixture-hard closeout gate, report-backed release evidence, no public source routes]
key-files:
  created:
    - .planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-ARCHIVAL-READABILITY-REPORT.md
  modified:
    - scripts/lance-parquet-ingress-test.sh
    - scripts/mvp0-verify.sh
key-decisions:
  - "Phase 27 legacy proof remains hard-gated on actual older-version Lance and Parquet fixture paths plus paired verifier-accepted Loom artifacts."
  - "The main release verifier now runs Phase 27 after the Phase 26 source-ingress contract gate and before DuckDB SQL smoke."
  - "The final report treats manifests as provenance only, never as a substitute for actual older-version source fixtures."
requirements-completed: [PHASE-27]
duration: 57m
completed: 2026-06-08T22:06:00Z
---

# Phase 27 Plan 05: Release Gate and Archival Readability Closeout Summary

**Phase 27 is now release-gated with actual older-version Lance and Parquet fixtures, paired verifier-accepted Loom artifacts, and a bounded archival readability report.**

## Performance

- **Duration:** 57 min
- **Started:** 2026-06-08T21:09:04Z
- **Completed:** 2026-06-08T22:06:00Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- Expanded `scripts/lance-parquet-ingress-test.sh` from the scaffold guard into the final closeout gate.
- Required actual older-version fixture paths:
  - `crates/loom-parquet-ingress/tests/fixtures/legacy/legacy-v1.parquet`
  - `crates/loom-parquet-ingress/tests/fixtures/legacy/legacy-v1.loom`
  - `crates/loom-lance-ingress/tests/fixtures/legacy/legacy-v1.lance/`
  - `crates/loom-lance-ingress/tests/fixtures/legacy/legacy-v1.loom`
- Added report marker and language checks so manifest-only, record-only, or deterministic-record evidence cannot pass as legacy proof.
- Wired `scripts/lance-parquet-ingress-test.sh` into `scripts/mvp0-verify.sh` after Phase 26 and before DuckDB smoke.
- Wrote `27-ARCHIVAL-READABILITY-REPORT.md` with supported, unsupported, rejected, current-version, actual older-version, legacy, verifier, oracle, dependency, tradeoff, non-goal, release-gate, and Phase 28 handoff evidence.

## Task Commits

Each task was committed atomically:

1. **Task 1: Finalize focused Lance/Parquet ingress gate** - `3d1d910` (`test`)
2. **Task 2: Wire Phase 27 gate into the main release verifier** - `ddfa704` (`test`)
3. **Task 3: Write archival readability report and run closeout verification** - `e94543f` (`docs`)

## Files Created/Modified

- `scripts/lance-parquet-ingress-test.sh` - Final Phase 27 closeout gate requiring actual legacy fixtures, paired Loom artifacts, report evidence, focused tests, dependency guards, and public/API creep guards.
- `scripts/mvp0-verify.sh` - Added Phase 27 gate after Phase 26 source ingress and before DuckDB SQL smoke.
- `.planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-ARCHIVAL-READABILITY-REPORT.md` - Final bounded evidence report.

## Verification

- `bash -n scripts/lance-parquet-ingress-test.sh` passed.
- `bash scripts/lance-parquet-ingress-test.sh` passed.
- `bash -n scripts/mvp0-verify.sh` passed.
- `test -f crates/loom-parquet-ingress/tests/fixtures/legacy/legacy-v1.parquet && test -f crates/loom-parquet-ingress/tests/fixtures/legacy/legacy-v1.loom && test -d crates/loom-lance-ingress/tests/fixtures/legacy/legacy-v1.lance && test -f crates/loom-lance-ingress/tests/fixtures/legacy/legacy-v1.loom` passed.
- `rg -q "Actual Older-Version Fixtures" .planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-ARCHIVAL-READABILITY-REPORT.md` passed.
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp0-verify.sh` passed after deleting ignored generated `target/` build artifacts to recover disk headroom from an initial `No space left on device` failure.

## Decisions Made

- Kept the Phase 27 release claim tied to real checked-in fixture files/directories and paired `.loom` artifacts, not manifests alone.
- Kept source-family behavior out of public SQL, CLI, DuckDB extension routes, FFI headers, and generic/core/source-ingress dependencies.
- Recorded the disk-space failure as environment cleanup evidence rather than a product deviation; the same release-gate command passed after generated build artifacts were removed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Recovered from generated build artifact disk exhaustion**
- **Found during:** Task 3 (`LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp0-verify.sh`)
- **Issue:** The first main verifier run failed before reaching Phase 27 with `errno=28` / `No space left on device` while linking workspace tests.
- **Fix:** Removed ignored generated `target/` build artifacts, restoring disk headroom, then reran the same release-gate command successfully.
- **Files modified:** None.
- **Commit:** Not applicable; generated ignored build artifacts only.

## Issues Encountered

- `gsd-tools` was not on PATH, but the local GSD CLI was available through `node /Users/macintoshhd/.codex/gsd-core/bin/gsd-tools.cjs`.
- The final report was drafted before Task 1's gate commit so the report-dependent closeout script could be verified without weakening the required report checks. It was committed only in Task 3.

## Known Stubs

None. Stub scan of modified plan files found no TODO/FIXME/placeholder text or hardcoded empty values that affect runtime output. The report explicitly rejects manifest-only and record-only legacy evidence.

## Threat Flags

None. The new release-gate, report, and main-verifier ordering surfaces are the planned trust boundaries covered by T-27-05-01 through T-27-05-06.

## Auth Gates

None.

## User Setup Required

None. The actual older-version Lance and Parquet fixtures and paired Loom artifacts are checked into the repository.

## Next Phase Readiness

Phase 28 can rely on Phase 27's bounded adapter evidence: accepted source artifacts require verifier acceptance, oracle evidence, row-equivalence checks, actual source fixture files for legacy claims, and dependency/API isolation.

## Self-Check: PASSED

- Summary file exists at `.planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-05-SUMMARY.md`.
- Final report exists at `.planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-ARCHIVAL-READABILITY-REPORT.md`.
- Main release verifier includes `scripts/lance-parquet-ingress-test.sh` after `scripts/source-ingress-contract-test.sh` and before `scripts/duckdb-smoke-test.sh`.
- Task commits exist: `3d1d910`, `ddfa704`, `e94543f`.
- Focused and main release gates passed.

---
*Phase: 27-lance-parquet-archival-readability-dataset-ingress*
*Completed: 2026-06-08T22:06:00Z*
