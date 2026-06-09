# Plan 34-03 Summary: Focused LMC2 SQL Gate and Source E2E Cutover

**Completed:** 2026-06-09
**Status:** Complete

## What Changed

- Added `scripts/duckdb-lmc2-sql-surface-test.sh`, a focused DuckDB SQL gate
  for default `LMC2(LMA1)` artifacts.
- Added `emit_arrow_semantic_lmc2_sql_fixture` in `loom-fixtures` to generate:
  - `multi-column-lmc2.loom` as the default wrapped SQL artifact,
  - `multi-column-direct-lma1.loom` as an explicit regression bridge.
- The focused gate checks multi-column primitive/nullable SQL over Int32, Utf8,
  Boolean, Int64, and Float64 columns, including projection, filter, aggregate,
  null propagation, and direct `LMA1` regression.
- Updated `scripts/duckdb-source-e2e-test.sh` so Parquet, Lance, and Vortex
  product queries target default `*.loom` `LMC2` artifacts directly. Bridge
  files remain checked only as direct `LMA1` regression evidence.
- Fixed DuckDB runtime projection planning so `LMC2`/direct `LMA1` Arrow
  semantic artifacts report their real Arrow schema field count instead of
  falling back to one column.

## Evidence

- `bash scripts/duckdb-lmc2-sql-surface-test.sh` passed.
- `bash scripts/duckdb-source-e2e-test.sh` passed with default `LMC2` product
  queries and bridge regression checks.
- `cargo test -p loom-ffi --test duckdb_runtime_ffi` passed.
- `git diff --check` passed.

## Verification Commands

```bash
bash scripts/duckdb-lmc2-sql-surface-test.sh
bash scripts/duckdb-source-e2e-test.sh
cargo test -p loom-ffi --test duckdb_runtime_ffi
git diff --check
```

All passed.

## Carried Forward

- Plan 34-04 owns logical and nested positive support or stable unsupported
  diagnostics.
- Plan 34-05 should wire `scripts/duckdb-lmc2-sql-surface-test.sh` into broad
  release verification.

