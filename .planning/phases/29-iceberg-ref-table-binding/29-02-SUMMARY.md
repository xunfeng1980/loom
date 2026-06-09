---
phase: 29-iceberg-ref-table-binding
plan: 02
subsystem: source-adapter
tags: [rust, iceberg, serde_json, source-ingress, dependency-boundary]
requires:
  - phase: 29-01
    provides: adapter-local Iceberg binding crate and report model
provides:
  - Typed local Iceberg metadata and Loom sidecar parser
  - Descriptive table/ref binding facts for local JSON fixtures
  - Byte-free unsupported source-ingress reports for valid metadata
  - Rejected diagnostics for malformed or missing-identity metadata
  - Focused gate coverage for parser fixtures and parser tests
affects: [phase-28, iceberg-binding, source-ingress, release-gates]
tech-stack:
  added: []
  patterns: [typed serde_json parser, descriptive sidecar facts, byte-free unsupported report]
key-files:
  created:
    - crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-metadata.json
    - crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-loom-binding.json
    - crates/loom-iceberg-binding/tests/fixtures/local/unsupported-remote-metadata.json
    - crates/loom-iceberg-binding/tests/fixtures/local/rejected-missing-identity.json
  modified:
    - crates/loom-iceberg-binding/src/binding_contract.rs
    - crates/loom-iceberg-binding/src/lib.rs
    - crates/loom-iceberg-binding/tests/binding_contract.rs
    - scripts/iceberg-binding-test.sh
key-decisions:
  - "Keep Plan 29-02 parser output descriptive only; verifier/hash/source/oracle acceptance remains deferred to Plan 29-03."
  - "Treat local Iceberg metadata with remote/catalog/object-store/credential markers as unsupported and byte-free rather than accepted."
  - "Use typed serde_json deserialization for bounded Iceberg metadata and Loom sidecar fields instead of ad hoc metadata string extraction."
requirements-completed: [PHASE-29]
duration: 8min
completed: 2026-06-08T22:31:47Z
---

# Phase 29 Plan 02: Iceberg Local Parser Summary

**Typed local Iceberg metadata and Loom sidecar parsing into descriptive, byte-free binding facts**

## Performance

- **Duration:** 8 min
- **Completed:** 2026-06-08T22:31:47Z
- **Tasks:** 3
- **Files modified:** 10

## Accomplishments

- Added local Iceberg-style metadata and standalone Loom sidecar fixtures for accepted-looking, unsupported-remote, and rejected missing-identity cases.
- Implemented typed `serde_json` parsing for bounded metadata fields, snapshot/ref lookup, sidecar identity matching, and local-only policy checks.
- Exposed `iceberg_binding_facts_from_paths` and `source_ingress_report_from_iceberg_metadata_path`.
- Kept all Plan 29-02 outputs descriptive: no artifact bytes are emitted, no hashes are recomputed, `verify_artifact` is not called, and sidecar verifier/oracle claims do not construct accepted bindings.
- Extended `scripts/iceberg-binding-test.sh` to require context/research/patterns files and parser fixtures, then run dependency-boundary and parser tests while staying unwired from `scripts/mvp0-verify.sh`.

## Task Commits

1. **Task 1: Add local metadata and sidecar parser tests** - `bae336e` (`test`)
2. **Task 2: Implement typed serde_json fact extraction** - `6ca6240` (`feat`)
3. **Task 3: Extend focused gate to cover parser fixtures and no-SDK boundary** - `57fef61` (`test`)

## Files Created/Modified

- `crates/loom-iceberg-binding/src/binding_contract.rs` - Added typed metadata/sidecar structs, parser entry points, source report mapping, local-only policy checks, and diagnostics.
- `crates/loom-iceberg-binding/src/lib.rs` - Exported parser entry points.
- `crates/loom-iceberg-binding/tests/binding_contract.rs` - Added parser behavior tests for descriptive facts, unsupported byte-free reports, rejected diagnostics, malformed JSON, and sidecar accepted-claim non-acceptance.
- `crates/loom-iceberg-binding/tests/fixtures/local/*.json` - Added local accepted-looking, unsupported remote/catalog, and rejected missing-identity fixtures.
- `scripts/iceberg-binding-test.sh` - Added context and parser fixture checks before focused parser/dependency tests.

## Decisions Made

- Sidecar fields `verifier_evidence`, `source_evidence`, and `oracle_evidence` are parsed only as untrusted descriptive JSON in this plan.
- The parser rejects malformed or missing required identity before exposing trusted binding facts.
- Valid metadata that includes remote/catalog/object-store/credential markers remains unsupported with facts and no bytes.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Corrected accepted fixture local path policy**
- **Found during:** Task 2
- **Issue:** The initial accepted metadata fixture used `warehouse` in its local path, but Plan 29-02 treats `warehouse` as a catalog/control marker.
- **Fix:** Changed the accepted fixture location to `tests/fixtures/local/tables/demo/events`; the remote fixture still carries `warehouse` and `s3://` markers for unsupported coverage.
- **Files modified:** `crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-metadata.json`
- **Commit:** `6ca6240`

## Issues Encountered

- `gsd-tools` was not available on PATH, so STATE/ROADMAP updates were applied directly.

## Verification

- `cargo test -p loom-iceberg-binding --test binding_contract`
- `cargo test -p loom-iceberg-binding --test dependency_boundary`
- `bash -n scripts/iceberg-binding-test.sh`
- `bash scripts/iceberg-binding-test.sh`
- `! rg -q "iceberg-binding-test\\.sh" scripts/mvp0-verify.sh`
- `rg -n "serde_json::" crates/loom-iceberg-binding/src/binding_contract.rs`
- `rg -n "split\\(|contains\\(\"table-uuid\"|\"current-snapshot-id\"" crates/loom-iceberg-binding/src/binding_contract.rs` returned no matches.

## Known Stubs

None. Empty terminal color variables in `scripts/iceberg-binding-test.sh` are intentional non-TTY fallback values.

## User Setup Required

None.

## Next Phase Readiness

Plan 29-03 can validate referenced artifact bytes, recompute hashes, run verifier acceptance, and compare source/oracle evidence against the descriptive facts created here.

## Self-Check: PASSED

- Created files exist: parser fixtures, parser tests, parser implementation, and focused gate.
- Task commits exist: `bae336e`, `6ca6240`, and `57fef61`.
- Overall verification passed.
- No accidental tracked-file deletions were reported after task commits.

---
*Phase: 29-iceberg-ref-table-binding*
*Completed: 2026-06-08T22:31:47Z*
