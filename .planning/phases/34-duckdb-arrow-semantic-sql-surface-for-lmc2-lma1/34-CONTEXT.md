# Phase 34: DuckDB Arrow Semantic SQL Surface for LMC2(LMA1) - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 34 broadens DuckDB `loom_scan(path)` over the default Arrow semantic
distribution artifact: `LMC2(LMA1)`. DuckDB must recognize the `LMC2` wrapper,
unwrap to the inner verifier-accepted `LMA1` payload, and scan Arrow semantic
data through a staged SQL surface.

The first acceptance layer is multi-column primitive plus nullable Arrow
semantic payloads. Logical types and nested/list/struct coverage are planned as
later layers or explicit deferrals if Arrow C FFI schema mapping and DuckDB
vector population become too large for one phase.

This phase is queryability and adapter correctness. It must not claim native
Arrow semantic execution, StarRocks runtime support, remote/source reader
integration, or a new public SQL surface beyond `loom_scan(path)`.

</domain>

<decisions>
## Implementation Decisions

### Artifact Entry Surface
- DuckDB should accept default `LMC2(LMA1)` artifacts directly; current direct
  `LMA1` bridge fixtures remain regression evidence only and should stop being
  the default source e2e query path once Phase 34 support lands.
- The adapter should unwrap `LMC2` before binding schema and before decoding
  rows, preserving verifier/artifact-fail-closed diagnostics rather than treating
  wrapper bytes as a raw layout container.
- Direct `LMA1` should continue to work as an explicit bridge/regression input,
  but no source-emission compatibility burden should be reintroduced.
- `loom_scan(path)` remains the public SQL entrypoint. New query modes, table
  function names, or public C ABI flags are out of scope.

### SQL Shape Staging
- Start with one record batch containing multiple primitive/UTF8/Boolean
  columns, including nullable columns, because this is the smallest meaningful
  expansion beyond the current single-column direct-LMA1 path.
- Preserve column names from the Arrow semantic schema in DuckDB bind output.
  Fallback names should be deterministic only for malformed/missing names if
  such a case is accepted by the Arrow semantic verifier.
- Support projection pushdown over Arrow semantic columns using the existing
  DuckDB `column_ids` mechanism; scan should only decode/populate projected
  columns where practical.
- Keep multi-batch Arrow semantic payloads rejected or explicitly unsupported
  unless planning finds the current codec/verifier already guarantees a safe
  single-batch shape.

### Logical And Nested Scope
- Primitive nullable SQL support is required before logical or nested support.
- Logical types should be added only after primitive nullable bind/scan is stable
  and should map to DuckDB logical types with focused tests and negative
  diagnostics.
- Nested/list/struct support should be either a clearly scoped later plan in
  this phase or explicitly deferred to a follow-up sub-phase if it would require
  broad Arrow C Data child-array vector population.
- Negative diagnostics should be stable and should say whether a shape is
  unsupported by DuckDB SQL, malformed as Arrow semantic payload, or rejected by
  artifact verification.

### Execution Claims
- Phase 34 uses interpreter-backed Arrow semantic decode through the existing
  Rust FFI path unless Phase 35 has already supplied engine-neutral native
  evidence. Wrapper acceptance and DuckDB queryability are not native execution.
- Existing native route/cache/fallback tests must remain accurate for legacy
  `LMC1` and direct raw/table paths.
- Release gates must prove default `LMC2(LMA1)` source artifacts are queried
  directly by DuckDB, not only verified separately and then re-encoded as a
  direct-LMA1 bridge.
- Any direct-LMA1 bridge fixture retained after Phase 34 should be labeled as
  regression-only evidence, not the product path.

### the agent's Discretion
The agent may choose whether to add a Rust-side batch/table FFI helper or extend
the existing C++ Arrow C Data handling directly, provided ownership remains
fail-closed and release callbacks are called exactly once on every teardown path.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/loom-core/src/arrow_semantic_codec.rs` provides `LMA1` encode/decode
  and `LMC2` wrap/unwrap helpers, including fail-closed malformed-wrapper
  checks.
- `crates/loom-ffi/src/ffi.rs` exposes `loom_decode`, currently decoding direct
  `LMA1` only when the payload has one batch and one column.
- `duckdb-ext/loom_extension.cpp` already has direct DataChunk population for
  Bool, Int32, Int64, Utf8, Float32, and Float64 arrays, including nullable
  validity-bit handling.
- Existing scripts generate default source `LMC2` artifacts plus
  `*-duckdb-bridge-lma1.loom` bridge fixtures for current bounded DuckDB e2e.

### Established Patterns
- Public SQL remains `loom_scan(path)`; projection pushdown is wired through
  DuckDB `TableFunctionInitInput::column_ids`.
- DuckDB adapter errors use stable `loom_scan:` messages and, for runtime
  routes, diagnostic codes/paths.
- C++ owns Arrow C Data structs inside `LoomScanState` and releases arrays and
  schemas on every teardown path.
- Broad release ordering runs source semantic compatibility, then LMC2 wrapper
  proof, then binding/query gates and DuckDB smoke.

### Integration Points
- `PopulateColumnSpecs` in `duckdb-ext/loom_extension.cpp` is the bind-time
  schema entrypoint that currently treats direct `LMA1` as a single `"value"`
  column.
- `LoomInit` decodes per-column payloads with `loom_decode`; multi-column
  Arrow semantic support likely needs either one table-shaped decode or a
  controlled split into per-column Arrow arrays.
- `LoomScan` already fills projected output vectors from `state.arrow_arrays`
  and checks equal column lengths.
- `scripts/duckdb-source-e2e-test.sh` is the product gate that should move from
  bridge fixture SQL to default `LMC2` SQL.

</code_context>

<specifics>
## Specific Ideas

The user explicitly wants Phase 34 to be named and scoped around
`LMC2(LMA1)`, not direct `LMA1`. The recommended route is:

- first: multi-column primitive plus nullable over default `LMC2(LMA1)`;
- then: logical types;
- then: nested/list/struct as explicit subtask or deferred sub-phase if needed.

</specifics>

<deferred>
## Deferred Ideas

- Native Arrow semantic execution belongs to Phase 35.
- Live StarRocks runtime integration remains out of scope.
- Remote artifact trust, signatures, encryption, and source-reader runtime
  integration remain out of scope.

</deferred>
