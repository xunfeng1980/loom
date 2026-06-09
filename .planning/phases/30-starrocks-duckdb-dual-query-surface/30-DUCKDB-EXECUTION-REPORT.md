# Phase 30 DuckDB Execution Report

## Summary

DuckDB real execution is complete for the Phase 30 slice requested on 2026-06-09.
The proof uses a Phase 29 accepted Iceberg-bound Loom artifact, not a second
artifact format, and executes through the existing public DuckDB table function:
`loom_scan(path)`.

## Evidence Root

- Fixture generator: `crates/loom-dual-query-surface/src/fixture_bundle.rs`
- Accepted artifact: generated `demo-events.lmc1.loom`
- Binding trust root: `bind_iceberg_ref_from_paths`
- Table identity: `demo.events`
- Schema ID: `7`
- Snapshot ID: `314159`
- Rows: `7, -1, 42`

## Executable DuckDB Evidence

`scripts/dual-query-surface-test.sh` passed and executed:

- `SELECT id FROM loom_scan(path) ORDER BY id` -> `-1`, `7`, `42`
- `SELECT id FROM loom_scan(path) WHERE id >= 0 ORDER BY id` -> `7`, `42`
- `SELECT COUNT(*) FROM loom_scan(path)` -> `3`
- `SELECT SUM(id) FROM loom_scan(path)` -> `48`

The gate builds and loads `duckdb-ext/build/loom.duckdb_extension` before
executing the SQL, so this is not descriptor-only evidence.

## Current-Phase Tradeoff

DuckDB execution is fully proven for the bounded query matrix. Plans 30-04 and
30-05 later completed the bounded Phase 30 closeout with fail-closed negative
coverage, optional StarRocks runtime-smoke semantics, main release-gate wiring,
and `30-DUAL-QUERY-SURFACE-REPORT.md`. Live StarRocks runtime integration
remains optional and supplemental rather than canonical evidence.
