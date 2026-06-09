# Phase 34: DuckDB Arrow Semantic SQL Surface for LMC2(LMA1) - Research

**Gathered:** 2026-06-09
**Status:** Complete

## Research Question

How should DuckDB query the new default `LMC2(LMA1)` Arrow semantic artifact
without blurring public FFI semantics, native execution claims, or the direct
`LMA1` regression bridge?

## Summary Recommendation

Add a DuckDB-internal Rust FFI handle for Arrow semantic artifacts, then teach
the C++ `loom_scan(path)` adapter to bind and scan columns through that handle.
The handle should accept both default `LMC2(LMA1)` and explicit direct `LMA1`,
verify artifacts before exposing schema facts, require a single record batch,
and export projected columns as Arrow C Data arrays.

This keeps `loom_decode` narrow and backward-compatible while giving DuckDB the
table-shaped information it needs:

1. Rust owns `LMC2` unwrap, `LMA1` decode, verifier checks, schema extraction,
   and Arrow C Data export.
2. C++ owns DuckDB bind/init/scan, projection pushdown, vector population, and
   exact release-callback ownership.
3. The source e2e gate queries default `LMC2` artifacts directly; direct `LMA1`
   bridge files remain regression-only.
4. Logical and nested coverage are staged after primitive nullable support.
   Unsupported shapes must fail closed with stable diagnostics.

## Current Code Findings

### DuckDB Extension

- `duckdb-ext/loom_extension.cpp` recognizes direct `LMA1` with
  `IsArrowSemanticPayload`, but `IsContainerPayload` and
  `ExtractContainerPayload` only understand `LMC1`.
- `PopulateColumnSpecs` currently treats direct `LMA1` as a single column named
  `value` by calling public `loom_decode` and inspecting the exported Arrow
  schema format.
- `LoomInit` decodes each bound payload with public `loom_decode`. That works
  for raw layout/table payloads but cannot express one multi-column `LMA1`
  record batch cleanly.
- `LoomScan` already has nullable vector population for Bool, Int32, Int64,
  Utf8, Float32, and Float64, plus row-count equality checks across projected
  columns.
- Projection pushdown is already wired through `TableFunctionInitInput`.

### Rust FFI

- `crates/loom-ffi/src/ffi.rs` public `loom_decode` accepts direct `LMA1` only
  when it decodes to exactly one batch and one column.
- `crates/loom-ffi/src/duckdb_runtime.rs` already contains non-public DuckDB
  runtime symbols and status codes. It is the best home for a DuckDB-internal
  Arrow semantic handle.
- `crates/loom-ffi/include/loom_duckdb_internal.h` is a manual internal header;
  it can expose new DuckDB-only helpers without freezing the public ABI.
- `crates/loom-ffi/cbindgen.toml` excludes internal `loom_duckdb_*` symbols
  from generated public `loom.h`. New internal symbols must be added to that
  exclusion list if cbindgen would otherwise export them.

### Core Arrow Semantic Codec

- `crates/loom-core/src/arrow_semantic_codec.rs` provides
  `is_arrow_semantic_payload`, `is_arrow_semantic_container`,
  `unwrap_arrow_semantic_payload`, `decode_arrow_semantic_payload`, and
  `decode_arrow_semantic_container_payload`.
- Phase 33 made `LMC2` a semantic-specific wrapper around one required direct
  `LMA1` payload. The DuckDB adapter should use those helpers through Rust, not
  copy the wrapper grammar into C++.

### Gates

- `scripts/duckdb-source-e2e-test.sh` now emits default source artifacts as
  `LMC2(LMA1)` and keeps direct `LMA1` bridge fixtures for current bounded SQL
  evidence.
- Phase 34 should add a focused DuckDB LMC2 SQL gate and then cut the product
  e2e path over to querying default source artifacts directly.
- `scripts/mvp0-verify.sh` already runs the Phase 33 wrapper gate before later
  DuckDB/native gates; Phase 34 should wire its focused SQL gate after that.

## Architectural Decision

Use a Rust-side opaque handle, tentatively named `LoomDuckDbArrowSemantic`, with
DuckDB-internal functions for:

- create from artifact bytes,
- destroy,
- column count,
- row count,
- column name,
- column kind,
- decode/export one column by source index into `FFI_ArrowArray` and
  `FFI_ArrowSchema`.

The create function should accept `LMC2(LMA1)` and direct `LMA1`, verify the
artifact where possible, decode to one record batch, cache schema facts, and
reject unsupported multi-batch or unsupported field types before C++ bind
returns SQL columns.

## Rejected Alternatives

### Broaden public `loom_decode`

Public `loom_decode` returns one Arrow array and schema. Turning it into a
table-shaped or mode-dependent API would make existing call sites ambiguous and
could make direct `LMA1` compatibility look like the default product path again.

### Parse `LMC2` and Arrow IPC entirely in C++

C++ can cheaply read magic bytes, but Arrow semantic payload decoding and
wrapper verification already live in Rust. Reimplementing or partially copying
that behavior would split the safety boundary and make diagnostics harder to
keep stable.

### Add a new SQL function

The product surface is `loom_scan(path)`. A new function would avoid adapter
changes but create a second SQL contract and weaken the Phase 34 objective.

## Scope Staging

### Required First Layer

- Default `LMC2(LMA1)` accepted by `loom_scan(path)`.
- Direct `LMA1` remains accepted only as bridge/regression evidence.
- One record batch, multiple columns.
- Bool, Int32, Int64, Utf8, Float32, Float64.
- Nullable values preserved.
- Arrow schema field names preserved.
- Projection pushdown works over projected Arrow semantic columns.

### Later Layer In This Phase

- Add logical type mapping only after primitive nullable SQL is stable.
- If logical mapping is not safely available in the current adapter without
  large changes, document the exact unsupported diagnostics and carry the
  positive mapping to a follow-up sub-phase.

### Explicit Deferral Candidate

- Nested/list/struct DuckDB vector population is likely broader than the first
  SQL surface because child arrays need recursive Arrow C Data handling. It can
  be deferred if Phase 34 proves stable fail-closed diagnostics.

## Risks and Guardrails

- **Ownership risk:** exported Arrow arrays and schemas must be released exactly
  once by C++ scan state.
- **Overclaim risk:** Phase 34 must not call interpreter-backed Arrow semantic
  SQL support "native execution".
- **Bridge confusion risk:** direct `LMA1` fixtures must be named and documented
  as regression-only after source e2e cutover.
- **Diagnostic drift risk:** malformed `LMC2`, malformed `LMA1`, unsupported
  multi-batch, and unsupported field types should have distinct messages.
- **Header leak risk:** new `loom_duckdb_arrow_semantic_*` functions must remain
  in `loom_duckdb_internal.h`, not public `loom.h`.

## Verification Architecture

Use layered verification:

1. Rust FFI tests for creating Arrow semantic handles from direct `LMA1` and
   wrapped `LMC2(LMA1)`, including multi-column nullable batches.
2. Header contract tests proving the new symbols stay out of public `loom.h`.
3. DuckDB SQL gate for multi-column primitive/nullable `LMC2` artifacts,
   projection, filters, aggregates, and direct `LMA1` regression.
4. Source e2e gate querying default Parquet/Lance/Vortex `LMC2` outputs
   directly.
5. Broad MVP1 verifier with Phase 34 gate wired after the Phase 33 LMC2 gate.

## RESEARCH COMPLETE

