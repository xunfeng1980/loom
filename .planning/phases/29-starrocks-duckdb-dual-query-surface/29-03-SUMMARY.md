# 29-03 Summary: DuckDB Executable Evidence

## Status

Complete.

## Completed

- Added `duckdb_evidence.rs` to map canonical query evidence to exact DuckDB SQL over existing public `loom_scan(path)`.
- Added `emit_dual_query_fixture` to write a Phase 28 accepted artifact, StarRocks-compatible descriptor JSON, and DuckDB expected-output JSON.
- Added `scripts/dual-query-surface-test.sh` as the focused DuckDB executable evidence gate.
- The gate builds `loom.duckdb_extension`, loads it through DuckDB CLI, and executes real SQL over the generated accepted artifact.

## Verified DuckDB Queries

- `SELECT id FROM loom_scan(path) ORDER BY id` -> `-1`, `7`, `42`
- `SELECT id FROM loom_scan(path) WHERE id >= 0 ORDER BY id` -> `7`, `42`
- `SELECT COUNT(*) FROM loom_scan(path)` -> `3`
- `SELECT SUM(id) FROM loom_scan(path)` -> `48`

## Verification

- `cargo test -p loom-dual-query-surface`
- `cargo run -p loom-dual-query-surface --bin emit_dual_query_fixture -- target/loom-dual-query-surface-test-manual`
- `bash -n scripts/dual-query-surface-test.sh`
- `bash scripts/dual-query-surface-test.sh`

## Tradeoff

This is strong DuckDB executable evidence over Phase 28 accepted bytes. It is not yet complete Phase 29 dual-surface evidence because Plan 29-04 negative hardening and Plan 29-05 main release wiring/final report remain incomplete.
