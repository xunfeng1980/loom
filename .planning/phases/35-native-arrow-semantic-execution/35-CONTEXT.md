# Phase 35: Native Arrow Semantic Execution - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 35 adds engine-neutral native execution evidence for verifier-accepted
Arrow semantic artifacts. The input artifact is the Phase 33 default
`LMC2(LMA1)` wrapper, with direct `LMA1` retained only as an explicit bridge or
regression input.

Native correctness in this phase is not DuckDB correctness. DuckDB may consume
the evidence later, but the first proof must live in `loom-core`: accepted
artifact bytes are verified, decoded to Arrow semantic payloads, copied through
a native Rust execution path, and compared against the decoded reference Arrow
record batch.

This phase must not count route scaffolding, zero-filled buffers, toolchain
skips, or interpreter fallback as positive native semantic evidence.
</domain>

<decisions>
## Implementation Decisions

### Native Evidence Shape
- The first positive slice should support one-record-batch `LMC2(LMA1)` and
  explicit direct `LMA1` artifacts with fixed-width primitive columns.
- Nullable primitive columns are in scope. The executor must preserve null
  semantics and copy real values for valid rows.
- Boolean is in scope because it is fixed-width semantically but bit-packed in
  Arrow buffers; testing it catches validity/value bitmap mistakes.
- Utf8, Date32 logical, List, and Struct are explicit fail-closed unsupported
  shapes until a later native plan adds variable buffer or child-array kernels.

### Trust Boundary
- Verification must happen before native support is claimed.
- `LMC2` unwrap and `LMA1` decode should use existing Rust helpers; no host
  adapter should parse container grammar for this phase.
- The executor should produce a new `RecordBatch`, not return the decoded batch
  by reference.
- Equivalence checks should compare the native result with the decoded Arrow
  semantic reference batch.

### Runtime And Cache
- Runtime policy should be able to distinguish native Arrow semantic support
  from interpreter fallback and fail-closed unsupported shapes.
- Cache identity should include artifact bytes, projection, and a native Arrow
  semantic backend identity so direct/wrapped artifacts and projections cannot
  alias accidentally.
- Native evidence remains engine-neutral; DuckDB integration evidence may be
  recorded as a consumer, not the source of correctness.
</decisions>

<code_context>
## Existing Code Insights

- `crates/loom-core/src/arrow_semantic_codec.rs` owns `LMA1` decode and `LMC2`
  unwrap/decode helpers.
- `crates/loom-core/src/artifact_verifier.rs` accepts direct `LMA1` and wrapped
  `LMC2` as `"Arrow semantic payload"` but currently marks lowering readiness
  as deferred.
- `crates/loom-core/src/production_native_lowering.rs` supports native facts for
  legacy `LMP1`/`LMT1`; it rejects `"Arrow semantic payload"`.
- `crates/loom-core/src/arrow_buffer_lowering.rs` contains primitive buffer plan
  vocabulary, but its current MLIR paths are legacy decode-dialect-oriented.
- Phase 34 added DuckDB SQL over `LMC2(LMA1)`, including stable unsupported
  diagnostics for Date32 and Struct SQL. That is query evidence, not native
  execution evidence.
</code_context>

<specifics>
## Specific Ideas

Implement a new `native_arrow_semantic` module in `loom-core` that:

- verifies accepted artifact reports before execution;
- decodes `LMC2(LMA1)` or direct `LMA1`;
- supports one-batch primitive nullable Arrow arrays;
- copies values/nulls into a new Arrow `RecordBatch`;
- reports stable diagnostics for unsupported payload, multi-batch, Utf8,
  logical, nested, malformed, or verifier-rejected inputs;
- exposes an equivalence report comparing native output to the decoded
  reference batch.
</specifics>

<deferred>
## Deferred Ideas

- Variable-width native kernels for Utf8 and Binary.
- Positive native execution for Date32 logical types and nested List/Struct
  arrays.
- DuckDB native route consumption of Arrow semantic execution evidence.
- StarRocks runtime integration.
</deferred>
