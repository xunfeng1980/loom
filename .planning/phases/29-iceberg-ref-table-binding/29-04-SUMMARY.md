---
phase: 29-iceberg-ref-table-binding
plan: 04
subsystem: source-adapter
tags: [rust, iceberg, source-ingress, artifact-verifier, fail-closed, release-gate]
requires:
  - phase: 29-03
    provides: verifier/SHA/source/oracle-gated accepted Iceberg binding
provides:
  - Executable Iceberg binding mismatch and manifest-only fail-closed matrix
  - Static stale source and forged decoded-row/oracle evidence fixtures
  - Phase 29 Iceberg binding evidence report
  - Focused gate coverage for mismatch tests, report markers, and metadata-only trust wording
affects: [phase-28, phase-29, iceberg-binding, source-ingress, release-gates]
tech-stack:
  added: []
  patterns:
    - verifier-backed sidecar/reference binding
    - metadata-only claims fail closed
    - focused report language guard
key-files:
  created:
    - crates/loom-iceberg-binding/tests/mismatch_fail_closed.rs
    - crates/loom-iceberg-binding/tests/fixtures/local/mismatch-schema-sidecar.json
    - crates/loom-iceberg-binding/tests/fixtures/local/mismatch-snapshot-sidecar.json
    - crates/loom-iceberg-binding/tests/fixtures/local/manifest-only-sidecar.json
    - crates/loom-iceberg-binding/tests/fixtures/local/stale-source-evidence.json
    - crates/loom-iceberg-binding/tests/fixtures/local/forged-oracle-evidence.json
    - .planning/phases/29-iceberg-ref-table-binding/29-ICEBERG-BINDING-REPORT.md
  modified:
    - crates/loom-iceberg-binding/src/binding_contract.rs
    - scripts/iceberg-binding-test.sh
key-decisions:
  - "Stale source/oracle evidence row count must be checked against the verified Loom artifact row count, not only against itself."
  - "Manifest-only sidecars must fail before descriptive binding facts are considered complete."
  - "The Phase 29 report and gate continue to record no default official iceberg crate dependency."
patterns-established:
  - "Mismatch tests assert `Err(IcebergBindingReport)` and no accepted evidence/bytes for stale schema, stale snapshot, hash mismatch, verifier rejection, missing evidence, stale source evidence, forged oracle evidence, manifest-only claims, and scope creep."
  - "Focused gates can scan report language for metadata-only success claims before a report is accepted."
requirements-completed: [PHASE-29]
duration: 9min
completed: 2026-06-08T22:49:23Z
---

# Phase 29 Plan 04: Iceberg Mismatch and Report Summary

**Fail-closed Iceberg binding mismatch matrix with stale source/oracle evidence coverage and a bounded evidence report**

## Performance

- **Duration:** 9 min
- **Started:** 2026-06-08T22:41:38Z
- **Completed:** 2026-06-08T22:49:23Z
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments

- Added `mismatch_fail_closed.rs` covering stale schema, stale snapshot, table identity mismatch, artifact hash mismatch, verifier status mismatch, verifier-rejected bytes, missing source evidence, missing oracle evidence, stale source evidence, forged decoded-row/oracle evidence, manifest-only sidecars, and public-scope creep.
- Added static mismatch fixtures for schema, snapshot, manifest-only, stale source evidence, and forged oracle evidence.
- Tightened binding acceptance so manifest-only sidecars cannot yield complete facts and source/oracle evidence row count must match the verified Loom artifact row count.
- Wrote `29-ICEBERG-BINDING-REPORT.md` with binding schema, evidence, current-phase tradeoffs, non-goals, no-default-`iceberg` decision, and Phase 29 handoff.
- Expanded `scripts/iceberg-binding-test.sh` to run mismatch tests, check report markers, require stale/forged evidence fixtures, and reject metadata-only success language.

## Task Commits

1. **Task 1 RED: Add failing mismatch matrix** - `06a7864` (`test`)
2. **Task 1 GREEN: Fail closed on manifest and forged evidence** - `577eeb1` (`feat`)
3. **Task 2: Write Iceberg binding evidence report** - `00217ba` (`docs`)
4. **Task 3: Gate mismatch/report evidence** - `669e980` (`test`)

## Files Created/Modified

- `crates/loom-iceberg-binding/tests/mismatch_fail_closed.rs` - Executable D-08/D-15 mismatch matrix.
- `crates/loom-iceberg-binding/tests/fixtures/local/mismatch-schema-sidecar.json` - Stale schema sidecar fixture.
- `crates/loom-iceberg-binding/tests/fixtures/local/mismatch-snapshot-sidecar.json` - Stale snapshot sidecar fixture.
- `crates/loom-iceberg-binding/tests/fixtures/local/manifest-only-sidecar.json` - Manifest-only/metadata-only sidecar fixture.
- `crates/loom-iceberg-binding/tests/fixtures/local/stale-source-evidence.json` - Stale source evidence fixture with stale row-count evidence.
- `crates/loom-iceberg-binding/tests/fixtures/local/forged-oracle-evidence.json` - Forged decoded-row/oracle evidence fixture with accepted flags that are not sufficient.
- `crates/loom-iceberg-binding/src/binding_contract.rs` - Added sidecar evidence completeness checks and verified artifact row-count cross-checking.
- `.planning/phases/29-iceberg-ref-table-binding/29-ICEBERG-BINDING-REPORT.md` - Phase 29 binding evidence report.
- `scripts/iceberg-binding-test.sh` - Focused gate now covers mismatch tests, report markers, language guard, and stale/forged fixtures.

## Decisions Made

- Use the existing Loom artifact container/table decoders to derive row-count evidence from verifier-accepted bytes instead of trusting source/oracle evidence row counts.
- Treat sidecar `source_evidence`, `verifier_evidence`, `oracle_evidence`, and `source_oracle_evidence_path` as required for complete binding facts.
- Keep `scripts/mvp0-verify.sh` unwired until Plan 29-05, while making the focused gate authoritative for Plan 29-04.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added verified artifact row-count cross-check**
- **Found during:** Task 1 RED mismatch tests
- **Issue:** Source/oracle evidence row count was checked for internal consistency but not against the verified Loom artifact. A forged evidence artifact could keep stale row-count fields internally consistent.
- **Fix:** Added `artifact_row_count_bound` in `binding_contract.rs` and reject evidence whose row count differs from the verified `LMP1`/`LMT1` artifact row count.
- **Files modified:** `crates/loom-iceberg-binding/src/binding_contract.rs`
- **Verification:** `cargo test -p loom-iceberg-binding --test mismatch_fail_closed`
- **Committed in:** `577eeb1`

**2. [Rule 2 - Missing Critical] Required sidecar evidence completeness for binding facts**
- **Found during:** Task 1 RED mismatch tests
- **Issue:** A manifest-only sidecar could still produce descriptive binding facts before accepted binding was attempted.
- **Fix:** Required source, verifier, oracle, and source/oracle evidence-artifact fields during `iceberg_binding_facts_from_paths`.
- **Files modified:** `crates/loom-iceberg-binding/src/binding_contract.rs`
- **Verification:** `cargo test -p loom-iceberg-binding --test mismatch_fail_closed`
- **Committed in:** `577eeb1`

**3. [Rule 1 - Bug] Tightened ambiguous report wording caught by the new gate**
- **Found during:** Task 3 focused gate
- **Issue:** One report artifact-table line mentioned manifest-only claims and evidence without explicit negating language, causing the new report language guard to fail.
- **Fix:** Reworded the line to state manifest-only claims must not be accepted as evidence.
- **Files modified:** `.planning/phases/29-iceberg-ref-table-binding/29-ICEBERG-BINDING-REPORT.md`
- **Verification:** `bash -n scripts/iceberg-binding-test.sh && bash scripts/iceberg-binding-test.sh`
- **Committed in:** `669e980`

---

**Total deviations:** 3 auto-fixed (2 Rule 2, 1 Rule 1)
**Impact on plan:** All fixes strengthened the planned fail-closed trust boundary and report accuracy. No scope expansion, package addition, public route, or main release-gate wiring was introduced.

## Issues Encountered

- `gsd-tools` is not available as a shell command on PATH, so state operations use `node $HOME/.codex/gsd-core/bin/gsd-tools.cjs` where possible.
- The TDD RED failure first exposed manifest-only descriptive-facts acceptance; the same GREEN fix also closed the stale/forged row-count evidence gap targeted by the critical gate.

## Verification

- `cargo test -p loom-iceberg-binding --test mismatch_fail_closed` - passed
- `cargo test -p loom-iceberg-binding --test binding_handoff` - passed
- `cargo test -p loom-iceberg-binding --test binding_contract` - passed
- `test -f .planning/phases/29-iceberg-ref-table-binding/29-ICEBERG-BINDING-REPORT.md` - passed
- `rg -q "Binding Schema" .planning/phases/29-iceberg-ref-table-binding/29-ICEBERG-BINDING-REPORT.md` - passed
- `rg -q "Mismatch Fail-Closed Matrix" .planning/phases/29-iceberg-ref-table-binding/29-ICEBERG-BINDING-REPORT.md` - passed
- `bash -n scripts/iceberg-binding-test.sh && bash scripts/iceberg-binding-test.sh` - passed
- `! rg -q "iceberg-binding-test\\.sh" scripts/mvp0-verify.sh` - passed

## Known Stubs

None. Empty terminal color variables and the Python `bad = []` accumulator in `scripts/iceberg-binding-test.sh` are intentional runtime defaults/control-flow state, not product stubs.

## Threat Flags

None. New file access and public-surface scans are test/gate-only and match the plan's threat model for local mismatch fixtures and report acceptance.

## User Setup Required

None.

## Next Phase Readiness

Plan 29-05 can wire the focused Iceberg binding gate into `scripts/mvp0-verify.sh` after Phase 27 and before downstream query-surface work. The binding report records that evidence is still focused-gate-only until that wiring happens.

## Self-Check: PASSED

- Created files exist: mismatch test, five mismatch fixtures, Phase 29 binding report, and this summary.
- Modified key files exist: `binding_contract.rs` and `scripts/iceberg-binding-test.sh`.
- Task commits exist: `06a7864`, `577eeb1`, `00217ba`, and `669e980`.
- Verification commands passed.
- No accidental tracked-file deletions were reported after task commits.

---
*Phase: 29-iceberg-ref-table-binding*
*Completed: 2026-06-08T22:49:23Z*
