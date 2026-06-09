# 43-02 Summary: Cross-Engine Equivalence and Fail-Closed Runtime Gate

## Completed

- Added `scripts/starrocks-live-runtime-test.sh`.
- The gate validates Phase 43 markers, runtime contract tests, inherited Phase
  30 query-surface tests, DuckDB evidence tests, generated descriptors, and
  runtime report markers.
- Default local contract mode passes only after printing that live StarRocks
  runtime evidence is missing and not accepted.
- Strict live mode fails closed when live StarRocks env/client inputs are
  missing.
- The gate requires `STARROCKS_LOOM_ARTIFACT_SHA256` to match the accepted
  descriptor artifact identity before live StarRocks rows can be accepted.
- Added `43-STARROCKS-RUNTIME-REPORT.md` with query matrix, strict mode,
  artifact identity binding, fail-closed matrix, and non-claims.

## Verification

- `bash -n scripts/starrocks-live-runtime-test.sh`
- `bash scripts/starrocks-live-runtime-test.sh`
- `LOOM_REQUIRE_STARROCKS_LIVE=1 bash scripts/starrocks-live-runtime-test.sh`
  was checked to exit non-zero in this local environment without live runtime
  inputs.

## Live Evidence Status

No live StarRocks runtime query has been collected locally. This is recorded as
missing runtime evidence, not accepted live evidence.
