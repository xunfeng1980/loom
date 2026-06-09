# Phase 34 DuckDB LMC2 SQL Report

**Completed:** 2026-06-09
**Status:** Complete

## SQL Surface

DuckDB `loom_scan(path)` now accepts default verifier-backed `LMC2(LMA1)`
artifacts directly. The positive SQL surface is intentionally staged:

- one Arrow semantic record batch,
- multiple named columns,
- Bool, Int32, Int64, Utf8, Float32, and Float64,
- nullable values,
- projection, filter, aggregate, and null propagation.

## Artifact Routing

DuckDB uses an internal Rust FFI handle to inspect Arrow semantic artifacts. Rust
owns verifier acceptance, `LMC2` unwrap, direct `LMA1` bridge support, schema
facts, and Arrow C Data export. C++ owns bind/init/scan and DuckDB vector
population.

Default source artifacts are queried directly as `LMC2(LMA1)`. Direct `LMA1`
remains regression-only bridge evidence.

## Primitive Nullable Evidence

`scripts/duckdb-lmc2-sql-surface-test.sh` generates a multi-column
`LMC2(LMA1)` fixture and checks projection/filter/aggregate/null behavior over
Int32, Utf8, Boolean, Int64, and Float64 columns.

`scripts/duckdb-source-e2e-test.sh` now queries Parquet, Lance, and Vortex
default `*.loom` source artifacts directly through DuckDB.

## Logical Scope

Date32 logical artifacts are verifier-encoded as `LMC2(LMA1)` but are rejected
by DuckDB SQL with stable `unsupported Arrow semantic schema format`
diagnostics. Positive logical date/time/timestamp vector population is deferred.

## Nested Scope

Struct nested artifacts are verifier-encoded as `LMC2(LMA1)` but are rejected by
DuckDB SQL with stable `unsupported Arrow semantic schema format` diagnostics.
Positive nested/list/struct vector population is deferred.

## Direct LMA1 Regression Bridge

Direct `LMA1` remains accepted as an explicit regression input for the DuckDB
adapter. It is no longer the source e2e product path.

## Verifier/Runtime Boundary

The public `loom_decode` API remains unchanged. New Arrow semantic table-shaped
inspection lives behind `loom_duckdb_arrow_semantic_*` internal symbols declared
in `loom_duckdb_internal.h` and excluded from public `loom.h`.

Runtime projection planning now reads Arrow semantic schema column counts so
DuckDB projection ids match wrapped artifact schema facts.

## Non-Goals

- No native Arrow semantic execution was added.
- No ArrowArrayStream public ABI was added.
- No new SQL function or SQL flag was added.
- No positive logical Date32 or nested Struct SQL vector population is claimed.
- No StarRocks runtime integration was added.

## Risks Carried To Phase 35

- Phase 35 must provide engine-neutral native evidence rather than reusing
  DuckDB queryability as native correctness proof.
- Native Arrow semantic output needs its own support predicates, lowering facts,
  equivalence checks, cache/fallback behavior, and fail-closed diagnostics.

## Verification Commands

```bash
cargo test -p loom-ffi --test duckdb_runtime_ffi
cargo test -p loom-ffi --test roundtrip
bash scripts/duckdb-smoke-test.sh
bash scripts/duckdb-native-integration-test.sh
bash scripts/duckdb-lmc2-sql-surface-test.sh
bash scripts/duckdb-source-e2e-test.sh
LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp1-verify.sh
git diff --check
```

All passed during Phase 34 closeout.

