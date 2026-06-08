---
phase: 27-lance-parquet-archival-readability-dataset-ingress
plan: 02
subsystem: ingress
tags: [rust, parquet, arrow, source-ingress, metadata, diagnostics]
requires:
  - phase: 26-external-source-ingress-contract
    provides: Source-neutral facts, coverage, diagnostics, and report invariants
  - phase: 27-lance-parquet-archival-readability-dataset-ingress
    provides: Plan 27-01 adapter crates and dependency guards
provides:
  - Source-neutral Parquet facts from local files
  - Parquet schema, row-group, split, layout, and statistics-presence mapping
  - Supported-slice coverage classification for non-null Int32/Int64/Float32/Float64 raw and table shapes
  - Fact-bearing unsupported reports for nullable, string, nested, and logical shapes
  - Fail-closed rejected reports for unreadable and malformed local files
affects: [phase-27, parquet-ingress, source-ingress, archival-readability]
tech-stack:
  added: [arrow-array dev-dependency for adapter tests]
  patterns: [adapter-private SDK metadata mapping, byte-free classification reports, fail-closed local source diagnostics]
key-files:
  created:
    - crates/loom-parquet-ingress/src/source_contract.rs
    - crates/loom-parquet-ingress/tests/source_ingress_contract.rs
  modified:
    - Cargo.lock
    - crates/loom-parquet-ingress/Cargo.toml
    - crates/loom-parquet-ingress/src/lib.rs
    - crates/loom-parquet-ingress/tests/dependency_boundary.rs
key-decisions:
  - "Plan 27-02 classifies supported Parquet coverage but returns byte-free unsupported reports until artifact emission is implemented in Plan 27-04."
  - "Parquet metadata is summarized as strings, counts, booleans, and source facts; Parquet SDK types remain private to `loom-parquet-ingress`."
  - "Open and metadata parse failures reject before facts are trusted, using stable diagnostic paths `$.open` and `$.metadata`."
patterns-established:
  - "Source adapters may expose supported coverage independently from accepted artifact reports when a later plan owns verifier-routed emission."
  - "Valid unsupported source files preserve facts but use `SourceArtifactVerificationSummary::not_applicable()` and no oracle evidence."
requirements-completed: [PHASE-27]
duration: 6m
completed: 2026-06-08T20:32:25Z
---

# Phase 27 Plan 02: Parquet Fact Extraction and Classification Summary

**Local Parquet files now map into source-neutral schema, row-group, split, coverage, and fail-closed diagnostic reports without leaking Parquet SDK types.**

## Performance

- **Duration:** 6 min
- **Started:** 2026-06-08T20:28:53Z
- **Completed:** 2026-06-08T20:32:25Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Added `parquet_source_facts_from_path` and exported it from `loom-parquet-ingress`.
- Mapped Arrow schema, Parquet file metadata, row-group metadata, row splits, column paths, compression, statistics presence, and page-index availability into `SourceFacts`.
- Classified non-null Int32, Int64, Float32, Float64, and simple primitive tables as supported coverage while keeping reports byte-free in this plan.
- Returned fact-bearing unsupported reports for nullable, string, nested, and logical shapes with no artifact verification or oracle evidence.
- Rejected malformed bytes and missing local paths before trusting facts.

## Task Commits

Each task was committed atomically, with TDD gate commits where applicable:

1. **Task 1: Map Parquet metadata into SourceFacts** - `a8db424` (`test`) and `a6dfaa8` (`feat`)
2. **Task 2: Classify supported and unsupported Parquet shapes** - `aff8cf0` (`test`) and `8dcbea3` (`feat`)
3. **Task 3: Reject malformed Parquet files fail-closed** - `bc0af38` (`test`)

## Files Created/Modified

- `Cargo.lock` - Recorded the direct `arrow-array` dev-dependency edge for Parquet adapter tests.
- `crates/loom-parquet-ingress/Cargo.toml` - Added `arrow-array` as a dev-dependency for deterministic Arrow fixture arrays.
- `crates/loom-parquet-ingress/src/lib.rs` - Exported Parquet source contract helpers.
- `crates/loom-parquet-ingress/src/source_contract.rs` - Added local-file Parquet metadata extraction, coverage classification, byte-free reports, and rejected diagnostics.
- `crates/loom-parquet-ingress/tests/source_ingress_contract.rs` - Added fact, classification, unsupported, rejected, and dependency-leak contract coverage.
- `crates/loom-parquet-ingress/tests/dependency_boundary.rs` - Rustfmt-only formatting change.

## Verification

- `cargo test -p loom-parquet-ingress --test source_ingress_contract parquet_facts_include_schema_and_row_group_metadata` passed.
- `cargo test -p loom-parquet-ingress --test source_ingress_contract parquet_classifies_supported_and_unsupported_shapes` passed.
- `cargo test -p loom-parquet-ingress --test source_ingress_contract parquet_malformed_files_are_rejected_without_facts` passed.
- `cargo test -p loom-parquet-ingress --test source_ingress_contract` passed.
- `cargo test -p loom-parquet-ingress --test dependency_boundary` passed.
- `cargo tree -p loom-core | awk '/lance|parquet|object_store/{found=1} END{exit found?1:0}'` passed.
- `cargo tree -p loom-source-ingress | awk '/lance|parquet|object_store/{found=1} END{exit found?1:0}'` passed.
- `rg -n "pub struct Parquet|ParquetMetaData|ParquetRecordBatchReader" crates/loom-source-ingress crates/loom-core crates/loom-ffi` returned no matches.

## Decisions Made

- Kept accepted report construction out of Plan 27-02. Supported Parquet shapes are visible through `SourceCoverage`, while `source_ingress_report_from_parquet_path` remains byte-free until the emission plan adds verifier and oracle evidence.
- Used `ParquetRecordBatchReaderBuilder::try_new(File)` as the single local-file inspection path, matching the plan's D-01 and D-03 boundary.
- Stored Parquet row-group and column metadata as source-neutral strings and counts in `SourceLayoutFact` and `SourceSplitFact`, not as SDK objects.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added fail-closed malformed/open handling while implementing Task 1**
- **Found during:** Task 1 (Map Parquet metadata into SourceFacts)
- **Issue:** Metadata extraction could not be correct or secure unless local open and Parquet metadata parse failures rejected before facts were trusted.
- **Fix:** Implemented rejected reports for file-open and metadata-read failures in `parquet_source_facts_from_path`; Task 3 later added explicit regression coverage.
- **Files modified:** `crates/loom-parquet-ingress/src/source_contract.rs`, `crates/loom-parquet-ingress/tests/source_ingress_contract.rs`
- **Verification:** `parquet_malformed_files_are_rejected_without_facts` passed.
- **Committed in:** `a6dfaa8`, regression coverage in `bc0af38`

---

**Total deviations:** 1 auto-fixed (Rule 2)
**Impact on plan:** The mitigation was required by the plan threat model and did not broaden scope.

## Issues Encountered

- Task 3's RED test passed immediately because the fail-closed rejection path was already implemented as the Task 1 Rule 2 mitigation. The test was still committed to make the Task 3 acceptance criteria explicit.

## Known Stubs

None. Stub scan over the created/modified Parquet adapter files found no TODO/FIXME/placeholder markers or hardcoded empty UI-facing data.

## Threat Flags

None. New trust-boundary behavior is the planned local Parquet file inspection and metadata-to-facts mapping covered by T-27-02-01 through T-27-02-04.

## TDD Gate Notes

- Task 1 had RED (`a8db424`) and GREEN (`a6dfaa8`) commits.
- Task 2 had RED (`aff8cf0`) and GREEN (`8dcbea3`) commits.
- Task 3's explicit RED test passed because the behavior was already implemented during Task 1's threat-model mitigation; no additional implementation commit was needed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 27-04 can consume `parquet_source_facts_from_path` and the supported coverage classification to add verifier-routed `LMC1(LMP1)` / `LMC1(LMT1)` emission and Arrow-scan oracle evidence. Generic/core/ffi crates remain free of Parquet SDK types.

## Self-Check: PASSED

- Summary file exists at `.planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-02-SUMMARY.md`.
- Created files exist: `crates/loom-parquet-ingress/src/source_contract.rs` and `crates/loom-parquet-ingress/tests/source_ingress_contract.rs`.
- Task commits exist: `a8db424`, `a6dfaa8`, `aff8cf0`, `8dcbea3`, `bc0af38`.
- Verification commands listed above passed.

---
*Phase: 27-lance-parquet-archival-readability-dataset-ingress*
*Completed: 2026-06-08T20:32:25Z*
