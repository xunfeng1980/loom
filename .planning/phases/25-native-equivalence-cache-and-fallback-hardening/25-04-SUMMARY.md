---
phase: 25-native-equivalence-cache-and-fallback-hardening
plan: 04
subsystem: testing
tags: [duckdb, loom-scan, native-cache, fallback, fail-closed, route-report]

requires:
  - phase: 25-native-equivalence-cache-and-fallback-hardening
    provides: "Plans 25-01 through 25-03 established native equivalence, cache diagnostics, and DuckDB route-report forwarding."
provides:
  - "Phase 25 DuckDB SQL hardening gate for cache, projection, fallback, strict, malformed, cancellation, and helper-only mismatch routes."
affects: [duckdb-native-integration, phase-26-source-ingress, release-gates]

tech-stack:
  added: []
  patterns:
    - "Public SQL verification stays on loom_scan(path); route/cache assertions use LOOM_DUCKDB_TEST_ROUTE_REPORT."
    - "Helper cargo tests cover injected mismatch and cache non-cacheable routes that public SQL cannot naturally trigger."

key-files:
  created:
    - scripts/native-hardening-test.sh
  modified: []

key-decisions:
  - "Left duckdb-ext/loom_extension.cpp unchanged because existing CollectPreparedDiagnostics route reports already expose cache diagnostics."
  - "Kept cache/fallback controls internal and test-only; no public SQL flags or route-specific SQL names were added."
  - "Used same-process DuckDB CLI statements for repeated-scan cache assertions because the native preparation cache is in-process."

patterns-established:
  - "Forbidden public marker gates construct searched patterns from pieces to avoid matching their own literal checks."
  - "Cache smoke evidence is asserted via route-report diagnostics, not benchmark timing."

requirements-completed: [PHASE-25]

duration: "~35min"
completed: 2026-06-08
---

# Phase 25 Plan 04: Native Hardening Gate Summary

**DuckDB SQL hardening gate for native cache reuse, projection invalidation, fallback, strict fail-closed, cancellation, and helper-only cache safety routes**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-06-08T17:45:00Z
- **Completed:** 2026-06-08T18:18:54Z
- **Tasks:** 3
- **Files modified:** 1

## Accomplishments

- Added `scripts/native-hardening-test.sh`, following the Phase 24 DuckDB native integration script style.
- Verified public `SELECT`/`COPY` over `loom_scan(path)` for native-primitives aggregate equality, reordered projection equality, repeated identical scan equality, cache miss/insert/hit smoke evidence, and projection cache-key drift miss evidence.
- Covered FSST fallback, strict unsupported string/native fail-closed behavior, nullable/compressed fallback or fail-closed behavior, malformed artifact recovery, and cancellation diagnostics.
- Added helper-level cargo test gates for native-output-mismatch, cache-non-cacheable, and cache-key-mismatch routes that SQL cannot naturally inject.
- Added forbidden public marker gates for route-specific SQL names, cache mode spellings, Arrow stream exposure, predicate pushdown controls, and parallel split controls.

## Task Commits

1. **Task 1: Add native-hardening SQL and cache smoke gate** - `a07b65b` (feat)
2. **Task 2: Gate fallback, strict, and unsupported routes through SQL** - `a07b65b` (feat)
3. **Task 3: Preserve C++ as a report consumer** - `a07b65b` (feat)

## Files Created/Modified

- `scripts/native-hardening-test.sh` - Phase 25 release gate for DuckDB SQL/cache/fallback hardening.

## Verification

- `bash -n scripts/native-hardening-test.sh` - passed
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/native-hardening-test.sh` - passed
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/duckdb-native-integration-test.sh` - passed
- Additional forbidden-marker check equivalent to the plan grep gate - passed

## Decisions Made

- **No C++ change:** `duckdb-ext/loom_extension.cpp` already forwards prepared diagnostics from Rust through `CollectPreparedDiagnostics` and `AppendTestRouteReport`, including `cache-miss`, `cache-inserted`, `cache-hit`, `cache-non-cacheable`, and canonical cache input text.
- **No public API creep:** The new gate uses only `loom_scan(path)` from SQL. Native facts, cancellation, and route reports remain internal env-hook test controls.
- **No timing evidence:** Cache behavior is asserted through deterministic diagnostics rather than benchmark timing.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed forbidden-marker gate self-match**
- **Found during:** Task 3 verification
- **Issue:** The initial script failure text contained the literal phrase searched by the cache marker gate.
- **Fix:** Constructed the failure text from shell variables so the script cannot match its own forbidden literal.
- **Files modified:** `scripts/native-hardening-test.sh`
- **Verification:** `bash -n scripts/native-hardening-test.sh`, forbidden-marker grep, and full Phase 25 gate passed.
- **Committed in:** `a07b65b`

**Total deviations:** 1 auto-fixed bug.
**Impact on plan:** No scope expansion; the fix was necessary for the planned API-creep gate to work.

## Issues Encountered

- None remaining.

## Known Stubs

None.

## Threat Flags

None. The plan added a release gate only and introduced no new endpoints, auth paths, file access surface beyond existing fixture reads, or schema trust boundaries.

## User Setup Required

None.

## Next Phase Readiness

Phase 26+ work can rely on a dedicated Phase 25 release gate that proves cache smoke evidence and fallback/fail-closed routes without widening public SQL or moving native/cache policy into C++.

## Self-Check: PASSED

- Created file exists: `scripts/native-hardening-test.sh`
- Summary file exists: `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-04-SUMMARY.md`
- Task commit exists: `a07b65b`

---
*Phase: 25-native-equivalence-cache-and-fallback-hardening*
*Completed: 2026-06-08*
