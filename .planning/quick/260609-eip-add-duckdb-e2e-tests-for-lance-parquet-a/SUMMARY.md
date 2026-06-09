---
quick_id: 260609-eip
slug: add-duckdb-e2e-tests-for-lance-parquet-a
status: complete
completed_at: 2026-06-09T02:40:00Z
---

# Quick Task Summary

## Completed

- Added `LMA1` single-column decode support to `loom_decode`.
- Added DuckDB bind support for `LMA1` by decoding Arrow schema through the FFI boundary and mapping supported Arrow formats to DuckDB types.
- Added runtime ABI support for semantic Arrow emission so `LMA1` plans route to interpreter fallback instead of fail-closed.
- Added adapter-local DuckDB e2e fixture emitters for Parquet, Lance, and Vortex.
- Added `scripts/duckdb-source-e2e-test.sh`.
- Added `scripts/mvp1-verify.sh`, which runs `scripts/mvp0-verify.sh` and then the DuckDB source e2e gate.
- Updated README, README-zh, and STATE.

## Verification

- `cargo fmt`
- `RUSTC_WRAPPER= cargo test -p loom-core --test runtime_abi_contract`
- `RUSTC_WRAPPER= cargo test -p loom-ffi --test roundtrip roundtrip_decode_lma1_single_i32_column_values`
- `RUSTC_WRAPPER= cargo test -p loom-ffi --test duckdb_runtime arrow_semantic_lma1_uses_interpreter_fallback`
- `RUSTC_WRAPPER= bash scripts/duckdb-source-e2e-test.sh`

## Tradeoffs

- DuckDB source e2e currently proves the real Parquet/Lance/Vortex source -> `LMA1` -> verifier -> DuckDB SQL path for a single non-null Int32 column.
- Full DuckDB SQL support for arbitrary nested/logical Arrow schemas remains outside this quick task; those artifacts remain verifier-accepted source semantics and should route through supported interpreter/fail-closed paths until DuckDB adapter support expands.
