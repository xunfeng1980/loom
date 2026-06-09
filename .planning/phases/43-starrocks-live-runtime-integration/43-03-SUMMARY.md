# 43-03 Summary: ABI Findings and MVP2 Wiring

## Completed

- Added `43-ABI-FINDINGS.md`.
- Classified DuckDB-shaped ABI assumptions into fixed-now, accepted asymmetry,
  and Phase 44 input.
- Wired `scripts/starrocks-live-runtime-test.sh` into `scripts/mvp2-verify.sh`
  after the Phase 42 gate.

## Verification

Planned verification:

- `bash scripts/starrocks-live-runtime-test.sh`
- `LOOM_REQUIRE_STARROCKS_LIVE=1 bash scripts/starrocks-live-runtime-test.sh`
  exits non-zero without live runtime env/client.
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp2-verify.sh`
- `git diff --check`

## Remaining

`ENGINE-01` remains incomplete until live StarRocks runtime rows are collected
from a bound accepted artifact. Phase 43 should stay in progress/pending live
evidence rather than closing as fully complete.
