---
phase: 27-lance-parquet-archival-readability-dataset-ingress
plan: 03
subsystem: ingress
tags: [rust, cargo, lance, source-ingress, async, dataset-metadata]
requires:
  - phase: 27-lance-parquet-archival-readability-dataset-ingress
    plan: 01
    provides: Isolated `loom-lance-ingress` adapter crate and Lance dependency boundary
  - phase: 26-external-source-ingress-contract
    provides: Source-neutral ingress report contract
provides:
  - Async `lance_source_facts_from_path` metadata extraction
  - Byte-free `source_ingress_report_from_lance_path` classification reports
  - Source-neutral Lance schema, manifest, fragment, and split facts
  - Fail-closed rejected reports for malformed/non-dataset local paths
affects: [phase-27, source-ingress, lance-ingress, archival-readability]
tech-stack:
  added: []
  patterns: [async-only Lance adapter APIs, source SDK objects stay adapter-private]
key-files:
  created:
    - ingress/loom-lance-ingress/src/source_contract.rs
    - ingress/loom-lance-ingress/tests/source_ingress_contract.rs
  modified:
    - Cargo.lock
    - ingress/loom-lance-ingress/Cargo.toml
    - ingress/loom-lance-ingress/src/lib.rs
key-decisions:
  - "Lance facts classify supported shapes in `SourceCoverage` only; accepted reports and artifact bytes remain deferred to Plan 27-04."
  - "Lance SDK objects and object-store state remain private to `loom-lance-ingress`; generic/core/ffi crates receive only source-neutral strings, counts, booleans, and diagnostics."
  - "Arrow extension metadata is treated as unsupported schema even when the physical storage type is a supported primitive."
requirements-completed: [PHASE-27]
duration: 5m23s
completed: 2026-06-08T20:40:48Z
---

# Phase 27 Plan 03: Lance Fact Extraction and Source-Ingress Mapping Summary

**Local Lance datasets now produce async source-neutral facts, support classification, and fail-closed diagnostics without exposing Lance SDK handles or emitting Loom bytes.**

## Performance

- **Duration:** 5 min 23 sec
- **Started:** 2026-06-08T20:35:25Z
- **Completed:** 2026-06-08T20:40:48Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added `lance_source_facts_from_path` and `source_ingress_report_from_lance_path` behind the Lance adapter crate root.
- Mapped Lance dataset schema, version id, manifest summary, fragment ids, data-file counts, logical/physical row counts, validation status, and split row ranges into `loom-source-ingress` facts.
- Classified non-null Int32/Int64/Float32/Float64 single-column datasets as canonical raw coverage and non-null primitive multi-column datasets as canonical table coverage.
- Kept nullable, string, nested, logical, and Arrow extension-metadata Lance shapes fact-bearing but unsupported with no artifact bytes and no oracle evidence.
- Rejected non-dataset and missing local paths with diagnostics only.

## Task Commits

Each task was committed atomically:

1. **Task 1: Map Lance dataset metadata into SourceFacts**
   - `ca6c789` (`test`) add failing Lance source facts contract test
   - `941540e` (`feat`) map Lance metadata to source facts
2. **Task 2: Classify supported and unsupported Lance shapes**
   - `1803f41` (`test`) add failing Lance shape classification test
   - `3e112d5` (`feat`) classify Lance supported and unsupported shapes
3. **Task 3: Reject malformed or non-dataset Lance paths fail-closed**
   - `546aedb` (`test`) cover malformed Lance path rejection

Additional cleanup:

- `aef7865` (`style`) format Lance source contract

## Files Created/Modified

- `ingress/loom-lance-ingress/src/source_contract.rs` - async Lance fact extraction, classification, report mapping, and sanitized diagnostics.
- `ingress/loom-lance-ingress/tests/source_ingress_contract.rs` - Lance schema/version/fragment facts, classification matrix, SDK-boundary guard, and rejection tests.
- `ingress/loom-lance-ingress/src/lib.rs` - exported the async source-contract helpers.
- `ingress/loom-lance-ingress/Cargo.toml` - added `arrow-array` as a dev-dependency for deterministic Lance fixture tests.
- `Cargo.lock` - recorded the new Lance adapter dev-dependency edge.

## Verification

- `cargo test -p loom-lance-ingress --test source_ingress_contract lance_facts_include_schema_version_and_fragment_metadata` passed.
- `cargo test -p loom-lance-ingress --test source_ingress_contract lance_classifies_supported_and_unsupported_shapes` passed.
- `cargo test -p loom-lance-ingress --test source_ingress_contract lance_non_dataset_paths_are_rejected_without_facts` passed.
- `cargo test -p loom-lance-ingress --test source_ingress_contract` passed.
- `cargo test -p loom-lance-ingress --test dependency_boundary` passed.
- `cargo tree -p loom-core | awk '/lance|parquet|object_store/{found=1} END{exit found?1:0}'` passed.
- `cargo tree -p loom-source-ingress | awk '/lance|parquet|object_store/{found=1} END{exit found?1:0}'` passed.
- `rg -n "pub struct Lance|Dataset|FileFragment|object_store" ingress/loom-source-ingress crates/loom-core crates/loom-ffi` returned no matches.

## Decisions Made

- Lance source reports remain byte-free in this plan even when coverage says a shape is supported; Plan 27-04 owns accepted artifact emission.
- Lance fragment validation is summarized as `validation=ok` or `validation=failed` in source-neutral layout refs, not exposed as Lance objects.
- Diagnostics include sanitized first-line local error detail only when it does not look like credentials, tokens, object-store config, or remote URI text.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `arrow-array` dev-dependency for Lance contract tests**
- **Found during:** Task 1
- **Issue:** Deterministic Lance dataset fixtures require Arrow arrays and `RecordBatchIterator`, but `loom-lance-ingress` did not expose `arrow-array` to integration tests.
- **Fix:** Added `arrow-array = { workspace = true }` under `dev-dependencies` and committed the lockfile update.
- **Files modified:** `ingress/loom-lance-ingress/Cargo.toml`, `Cargo.lock`
- **Commit:** `ca6c789`, `941540e`

**2. [Rule 2 - Missing Critical Functionality] Implemented fail-closed Lance rejection while mapping facts**
- **Found during:** Task 1
- **Issue:** Safe fact extraction needs open/read/schema failures to reject without trusted facts before later tasks can rely on the helper.
- **Fix:** Added rejected report mapping for invalid UTF-8, non-local URI-like paths, open failures, empty schemas, and row-count read failures.
- **Files modified:** `ingress/loom-lance-ingress/src/source_contract.rs`
- **Commit:** `941540e`

**3. [Rule 1 - Bug] Treated Arrow extension metadata as unsupported schema**
- **Found during:** Task 2
- **Issue:** Extension-style primitive fields were initially classified by physical storage type and could be marked supported.
- **Fix:** Added field metadata detection for `ARROW:extension:name`, mapped those facts to logical kind `extension`, and returned unsupported schema diagnostics.
- **Files modified:** `ingress/loom-lance-ingress/src/source_contract.rs`
- **Commit:** `3e112d5`

## Issues Encountered

- `cargo fmt --check` reported pre-existing formatting drift in unrelated crates (`loom-cli`, `loom-fixtures`, `loom-native-melior`, and `loom-source-ingress`) as well as this plan's new file. Only the touched Lance source-contract file was formatted and committed; unrelated formatting drift was left unchanged.
- Task 3's RED test passed immediately because the rejection behavior had already been implemented as critical fail-closed functionality during Task 1. The test was committed as regression coverage.

## Known Stubs

None. Stub scan of files created/modified by this plan found no placeholder/TODO/FIXME or hardcoded empty values that flow to UI or runtime output.

## Threat Flags

None. The only new trust-boundary surface is the planned local Lance dataset metadata adapter covered by T-27-03-01 through T-27-03-05.

## Auth Gates

None.

## TDD Gate Compliance

- RED commit exists for Task 1: `ca6c789`.
- GREEN commit exists for Task 1: `941540e`.
- RED commit exists for Task 2: `1803f41`.
- GREEN commit exists for Task 2: `3e112d5`.
- Task 3 regression coverage exists: `546aedb`; its behavior was already implemented in Task 1 as a Rule 2 fail-closed mitigation.

## User Setup Required

None - no external service, credentials, or remote Lance/object-store configuration is required.

## Next Phase Readiness

Plan 27-04 can consume these Lance facts and supported coverage classifications to implement verifier-routed `LMC1` emission and source-native oracle evidence without widening generic source-ingress or core dependencies.

## Self-Check: PASSED

- Summary file exists at `.planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-03-SUMMARY.md`.
- Created files exist: `ingress/loom-lance-ingress/src/source_contract.rs`, `ingress/loom-lance-ingress/tests/source_ingress_contract.rs`.
- Task commits exist: `ca6c789`, `941540e`, `1803f41`, `3e112d5`, `546aedb`, `aef7865`.

---
*Phase: 27-lance-parquet-archival-readability-dataset-ingress*
*Completed: 2026-06-08T20:40:48Z*
