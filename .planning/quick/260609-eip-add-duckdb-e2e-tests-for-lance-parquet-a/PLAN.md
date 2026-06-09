---
quick_id: 260609-eip
slug: add-duckdb-e2e-tests-for-lance-parquet-a
status: in_progress
created_at: 2026-06-09T02:27:14Z
---

# Quick Task Plan

Task: Add DuckDB e2e tests for Lance, Parquet, and Vortex semantic sources; create `mvp1-verify` that includes `mvp0-verify` plus these e2e checks.

## Steps

1. Add a real `LMA1` single-column DuckDB execution path through `loom_decode` and `loom_scan`.
2. Add adapter-local fixture emitters for Parquet, Lance, and Vortex so source SDK dependencies stay isolated.
3. Add `scripts/duckdb-source-e2e-test.sh` to generate source files, emit verifier-accepted `LMA1`, and query them in DuckDB.
4. Add `scripts/mvp1-verify.sh` that runs `mvp0-verify` and then the new DuckDB source e2e gate.
5. Verify focused tests, script gates, and update quick summary/state.

## Tradeoffs

- DuckDB e2e initially covers a real single-column `Int32` `LMA1` slice for each source. This proves source -> `LMA1` -> verifier -> DuckDB SQL, while broader nested/logical Arrow DuckDB projection remains a separate adapter expansion.
- Parquet/Lance/Vortex fixture generation stays inside adapter crates to preserve dependency boundaries.
