---
phase: 26-external-source-ingress-contract
plan: 03
subsystem: ingress
tags: [rust, vortex, source-ingress, verifier, oracle-evidence]

requires:
  - phase: 26-external-source-ingress-contract
    provides: Vortex source mapping into the generic source-ingress contract
provides:
  - Verifier-routed Vortex source artifact handoff for accepted LMC1 artifacts
  - Source-native oracle evidence recorded separately from Loom artifact acceptance
  - Fail-closed executable coverage for unsupported valid and malformed sources
affects: [loom-vortex-ingress, loom-source-ingress, artifact-verifier]

tech-stack:
  added: []
  patterns:
    - Adapter helper emits accepted source artifacts only after loom_core artifact verification accepts LMC1 bytes
    - Oracle evidence is metadata only and does not replace verifier-routed Loom decode

key-files:
  created:
    - crates/loom-vortex-ingress/tests/source_ingress_handoff.rs
  modified:
    - crates/loom-vortex-ingress/src/source_contract.rs
    - crates/loom-vortex-ingress/src/lib.rs

key-decisions:
  - "Accepted Vortex source handoff returns bytes only in SourceIngressAcceptedArtifact after verify_artifact accepts the emitted LMC1."
  - "Unsupported and rejected handoff results return Err(SourceIngressReport), so no partial artifact bytes are exposed."
  - "Source-native oracle scan records row-count/null evidence only; tests still decode verifier-accepted Loom artifacts for value checks."

patterns-established:
  - "Verifier-first source handoff: reader facts and source facts are descriptive until artifact verification accepts emitted LMC1."
  - "Fail-closed source adapter results: unsupported valid sources may carry facts, rejected sources carry no trusted facts, and neither returns bytes."

requirements-completed: [PHASE-26]

duration: 4min
completed: 2026-06-08
---

# Phase 26 Plan 03: Source Ingress Handoff Summary

**Verifier-routed Vortex source handoff with source-native oracle evidence and fail-closed unsupported/rejected reports**

## Performance

- **Duration:** 4 min
- **Started:** 2026-06-08T19:15:27Z
- **Completed:** 2026-06-08T19:19:13Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- Added `emit_source_ingress_lmc1_from_vortex_buffer`, returning `SourceIngressAcceptedArtifact` only after `loom_core::artifact_verifier::verify_artifact` accepts the emitted `LMC1`.
- Added source-native oracle evidence for accepted single-column `LMP1` and table `LMT1` handoffs, while keeping oracle output separate from the verifier/decode path.
- Added fail-closed tests for unsupported UTF-8, unsupported table shape, and malformed source buffers.

## Task Commits

1. **Task 1: Add verifier-routed source artifact handoff**
   - `cfe9bbc` test: add failing source handoff verifier tests
   - `821ed22` feat: add verifier-routed source handoff
   - `b28bdb9` style: format source handoff tests
2. **Task 2: Add oracle evidence for accepted single-column and table cases**
   - `256f6b6` test: add failing oracle evidence handoff tests
   - `605a2f7` feat: mark oracle evidence as metadata only
3. **Task 3: Lock unsupported-valid and rejected-malformed fail-closed behavior**
   - `1852a27` test: add failing fail-closed source handoff tests
   - `a9652a8` feat: lock fail-closed source handoff reports

## Files Created/Modified

- `crates/loom-vortex-ingress/src/source_contract.rs` - Added verifier-routed source artifact handoff, artifact verification summary creation, source-native oracle evidence, and stable unsupported conversion diagnostics.
- `crates/loom-vortex-ingress/src/lib.rs` - Re-exported the accepted artifact struct and handoff helper from the crate root.
- `crates/loom-vortex-ingress/tests/source_ingress_handoff.rs` - Added accepted `LMP1`/`LMT1`, oracle evidence, unsupported valid, and rejected malformed contract tests.

## Decisions Made

- Accepted reports are built through `SourceIngressReport::accepted`, which requires non-empty artifact metadata and accepted oracle evidence.
- Verification failures, unsupported valid sources, and rejected malformed sources return `Err(SourceIngressReport)` from the handoff helper, preventing artifact bytes from escaping on non-accepted paths.
- The oracle note explicitly states that source-native scan is metadata only and that Loom verification/decode remains the acceptance path.

## Verification

Passed:

- `cargo test -p loom-vortex-ingress --test source_ingress_handoff`
- `cargo test -p loom-vortex-ingress --test single_column_to_loom --test table_to_loom`
- `cargo test -p loom-core --test artifact_verifier`

## TDD Gate Compliance

- RED commits exist for all three tasks before their corresponding GREEN commits.
- GREEN commits pass the plan-required verification commands.
- No refactor commit was needed beyond `b28bdb9`, a rustfmt-only style commit after the Task 1 RED/GREEN cycle.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## Known Stubs

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 26-04 can consume a verifier-routed source-ingress handoff contract: accepted Vortex inputs now expose generic reports only after Loom artifact verification accepts the emitted `LMC1`, while unsupported and rejected cases fail closed.

## Self-Check: PASSED

- Key source/test/summary files exist on disk.
- All task commits are present in git history: `cfe9bbc`, `821ed22`, `b28bdb9`, `256f6b6`, `605a2f7`, `1852a27`, `a9652a8`.
- Plan-required verification commands passed.

---
*Phase: 26-external-source-ingress-contract*
*Completed: 2026-06-08*
