# 29-02 Summary: Accepted Binding Query Matrix

## Status

Complete for the DuckDB executable evidence slice.

## Completed

- Added deterministic Phase 28 accepted binding fixture generation for `demo.events`.
- The generated verifier-accepted `LMC1(LMT1)` artifact contains one non-null Int32 `id` column with values `7, -1, 42`.
- Canonical query evidence derives ordered rows `-1, 7, 42`, predicate rows `7, 42`, `COUNT(*) = 3`, and `SUM(id) = 48` from accepted artifact bytes.
- StarRocks-compatible descriptors preserve binding identity and typed expected result digests, but remain offline descriptor evidence only.

## Verification

- `cargo test -p loom-dual-query-surface --test query_surface_contract`
- `cargo run -p loom-dual-query-surface --bin emit_dual_query_fixture -- target/loom-dual-query-surface-test-manual`
- `bash scripts/dual-query-surface-test.sh`

## Tradeoff

The query matrix is deliberately small. It proves the shared accepted bytes drive row/predicate/count/sum evidence, but it is not full SQL compatibility and not full StarRocks runtime proof.
