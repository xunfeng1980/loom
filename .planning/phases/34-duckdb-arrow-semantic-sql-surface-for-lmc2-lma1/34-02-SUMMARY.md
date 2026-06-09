# Plan 34-02 Summary: DuckDB Adapter LMC2 Arrow Semantic Bind/Scan

**Completed:** 2026-06-09
**Status:** Complete

## What Changed

- Updated `duckdb-ext/loom_extension.cpp` so `loom_scan(path)` first tries the
  DuckDB-internal Arrow semantic handle for default `LMC2(LMA1)` and explicit
  direct `LMA1` artifacts.
- Bind now preserves Arrow semantic field names and maps Arrow C schema formats
  for Bool, Int32, Int64, Utf8, Float32, and Float64 into DuckDB logical types.
- Init now exports projected Arrow semantic source columns through
  `loom_duckdb_arrow_semantic_export_column` instead of calling public
  `loom_decode` per column.
- Legacy `LMC1`/`LMP1`/`LMT1` bind/init/scan behavior remains on its old path.
- Arrow semantic artifacts are kept interpreter-backed for Phase 34; native
  execution claims remain Phase 35 scope.

## Evidence

- Existing `scripts/duckdb-smoke-test.sh` passed after the adapter change,
  covering legacy single-column and mixed-table `LMC1` fixtures.
- `scripts/duckdb-source-e2e-test.sh` passed, preserving the current bridge
  regression gate.
- Manual SQL over default source artifacts succeeded directly:
  - `parquet/parquet.loom`: rows `7, -1, 42`, aggregate `3,48,-1,42`
  - `lance/lance.loom`: rows `7, -1, 42`, aggregate `3,48,-1,42`
  - `vortex/vortex.loom`: rows `7, -1, 42`, aggregate `3,48,-1,42`
- `scripts/duckdb-native-integration-test.sh` passed, preserving existing route,
  fallback, cancellation, and public SQL/API evidence.

## Verification Commands

```bash
cargo build -p loom-ffi --release
cmake -S duckdb-ext -B duckdb-ext/build -DCMAKE_BUILD_TYPE=Release
cmake --build duckdb-ext/build
bash scripts/duckdb-smoke-test.sh
bash scripts/duckdb-source-e2e-test.sh
bash scripts/duckdb-native-integration-test.sh
git diff --check
```

All passed.

## Carried Forward

- Plan 34-03 should promote the manual default-source `LMC2` SQL evidence into
  scripted product-gate checks and add a dedicated multi-column primitive plus
  nullable `LMC2` SQL surface gate.
- Plan 34-04 still owns logical and nested positive support or stable
  unsupported diagnostics.

