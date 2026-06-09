# Plan 34-01 Summary: Internal Arrow Semantic DuckDB FFI Contract

**Completed:** 2026-06-09
**Status:** Complete

## What Changed

- Added the DuckDB-internal opaque `LoomDuckDbArrowSemantic` handle in
  `crates/loom-ffi/src/duckdb_runtime.rs`.
- Added internal functions to create/destroy the handle, read column count,
  row count, column names, Arrow C schema formats, and export one source column
  as Arrow C Data.
- The handle accepts verifier-accepted default `LMC2(LMA1)` artifacts and
  explicit direct `LMA1` bridge artifacts.
- The handle requires exactly one Arrow semantic record batch and rejects
  malformed bytes, non-Arrow artifacts, and multi-batch payloads before exposing
  schema facts.
- Public `loom_decode` semantics were not broadened.
- Added declarations to `crates/loom-ffi/include/loom_duckdb_internal.h` and
  cbindgen exclusions so new symbols remain out of public `loom.h`.

## Evidence

- `arrow_semantic_handle_accepts_lmc2_and_exports_nullable_columns` proves a
  wrapped multi-column batch exposes names, formats, row count, nullable Utf8,
  and nullable Bool values through Arrow C Data.
- `arrow_semantic_handle_accepts_direct_lma1_bridge` preserves direct `LMA1`
  bridge evidence.
- `arrow_semantic_handle_rejects_invalid_inputs_and_multibatch` proves null
  non-empty input, unrelated bytes, and multi-batch `LMC2` fail closed.
- `internal_header_exposes_arrow_semantic_duckdb_symbols` and the existing
  public-header leakage gates prove the symbols are internal-only.

## Verification Commands

```bash
cargo test -p loom-ffi --test duckdb_runtime_ffi
cargo test -p loom-ffi --test roundtrip
git diff --check
```

All passed.

## Carried Forward

- Plan 34-02 should consume `loom_duckdb_arrow_semantic_*` from
  `duckdb-ext/loom_extension.cpp` for bind/init/scan.
- Unsupported logical and nested formats are intentionally not expanded here;
  Phase 34 later plans own positive mappings or stable diagnostics.

