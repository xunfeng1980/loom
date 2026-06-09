---
status: complete
completed: 2026-06-09
---

# Summary

Fixed a real fake-pass risk in DuckDB-focused gates: CMake configure output was
piped through `grep ... || true`, which could hide a configure failure and allow
a stale build directory to continue.

## Changes

- `scripts/duckdb-smoke-test.sh` now captures CMake configure output to a log and
  fails immediately on non-zero configure status.
- `scripts/dual-query-surface-test.sh` now uses the same fail-closed CMake
  configure pattern.

## Verification

- `bash -n scripts/duckdb-smoke-test.sh`
- `bash -n scripts/dual-query-surface-test.sh`
- `git diff --check`
- `bash scripts/dual-query-surface-test.sh`
- `bash scripts/duckdb-smoke-test.sh`

## Remaining Skip Classification

Explicit `LOOM_ALLOW_NATIVE_TOOL_SKIP=1` / `LOOM_ALLOW_SOLVER_SKIP=1` paths are
still present in native/solver gates. They are explicit opt-in managed skips, not
default success paths. Phase 29 remains `3/5`: DuckDB real execution is complete,
while full StarRocks/dual-surface closeout is pending.
