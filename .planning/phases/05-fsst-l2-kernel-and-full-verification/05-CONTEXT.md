# Phase 05: fsst-l2-kernel-and-full-verification - Context

**Gathered:** 2026-06-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 05 delivers the MVP0 acceptance gate: Loom must decode FSST-encoded Utf8
columns, dictionary columns whose values are FSST-encoded, and every supported
L1 encoding through the same verification harness that compares Loom output to
Vortex's own decoder. The phase also proves the result through DuckDB SQL over
`loom_scan(...)`.

This phase clarifies HOW to implement the already-scoped roadmap requirements:
`L2-02`, `L2-03`, `VERIFY-01`, `VERIFY-02`, and `VERIFY-03`. New compression
kernels, multi-column scans, timing comparisons, and human-readable layout
descriptors remain v2 scope.

</domain>

<decisions>
## Implementation Decisions

### FSST Payload and Dependency Boundary
- **D-01:** Keep the D-02 boundary from prior phases: `loom-core` must continue
  to have zero `vortex-*` / FastLanes dependencies. Vortex inspection and oracle
  decoding stay in `loom-fixtures`.
- **D-02:** Implement the real FSST L2 kernel using a Loom-owned serialized
  params format plus the pure Rust `fsst-rs` crate. The params should carry the
  decomposed Vortex FSST physical parts as plain data: `symbols`,
  `symbol_lengths`, `codes_bytes`, `codes_offsets`, per-row validity, and row
  count / length metadata as needed for validation.
- **D-03:** Do not hand-write a local FSST decoder unless `fsst-rs` proves
  unusable during research. The planner should prefer a small params parser plus
  `fsst::Decompressor` over reimplementing escape-code semantics.
- **D-04:** Do not add `vortex-fsst` to `loom-core`. `vortex-fsst` remains a
  fixture/oracle dependency only.

### Dict-over-FSST and Utf8 Integration
- **D-05:** Support dict-over-FSST through a general Arrow `ArrayData` gather
  path rather than a special dict-FSST kernel. The `KernelEscape(FSST)` values
  child should materialize to Utf8 `ArrayData`, then the existing Dictionary arm
  should gather by decoded codes.
- **D-06:** Extend the internal decoded-child representation to support Utf8
  values and nulls. This may require adding Utf8 support to `DecodedArray` and
  either extending `OutputBuilder` with a `StringBuilder` variant or introducing
  a narrow string append abstraction that still uses Arrow builders.
- **D-07:** Preserve the Phase 4 L2 contract: an L2 kernel owns and returns its
  output `ArrayData`; top-level `KernelEscape` still goes through
  `decode_layout_to_array_data` and the registry.

### DuckDB SQL Gate
- **D-08:** Extend the existing direct `DataChunk` population path in
  `duckdb-ext/loom_extension.cpp` for Phase 05. Add support for the Arrow types
  needed by the verification fixtures, especially Utf8 strings alongside the
  already-supported Int32 path.
- **D-09:** Do not switch Phase 05 back to ArrowArrayStream / `arrow_scan`
  unless direct `DataChunk` filling becomes a hard blocker. The record-batch
  path is more standard but larger than necessary for the MVP0 gate.
- **D-10:** `VERIFY-03` must use real DuckDB SQL over `loom_scan(...)`; a Rust
  SQL substitute is not acceptable for Phase 05 completion.

### Verification Corpus Scope
- **D-11:** Use a complete but controlled MVP0 verification matrix. Cover FSST
  edge cases, dict-over-FSST, all currently supported L1 encodings, row-for-row
  oracle comparison, and at least one DuckDB `SELECT` plus aggregate smoke.
- **D-12:** Required FSST edge cases include empty strings, all-escape-sequence
  strings, max-length 8-byte symbol strings, and null routing.
- **D-13:** Avoid an exhaustive combinatorial test matrix in this phase. The goal
  is acceptance confidence for MVP0, not a large v2 coverage suite.

### Carry-Forward Decisions From Earlier Phases
- **D-14:** `L2KernelRegistry::default_for_mvp0()` owns kernel id `0` for FSST.
  Unknown ids must continue to return typed `UnknownKernel` errors.
- **D-15:** Direct `synthesized_read_loop` on `KernelEscape` remains unsupported;
  callers that need L2 dispatch use `decode_layout_to_array_data`.
- **D-16:** Fixture bridges may canonicalize Vortex-specific encodings when
  representation mismatches exist, but every such fixture must still compare
  against Vortex's live oracle.

### the agent's Discretion
The user selected the recommended option for every discussed gray area. The
planner has discretion over exact binary params layout, helper function names,
and fixture file organization as long as the decisions above and canonical refs
are preserved.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` - Phase 05 goal, success criteria, dependency on Phase
  04, and MVP0 DuckDB acceptance gate.
- `.planning/REQUIREMENTS.md` - Locked requirements `L2-02`, `L2-03`,
  `VERIFY-01`, `VERIFY-02`, and `VERIFY-03`.
- `.planning/PROJECT.md` - Project-level value statement and milestone
  constraints.

### Prior Phase Contracts
- `.planning/phases/04-l1-dict-rle-and-l2-escape-infrastructure/04-CONTEXT.md`
  - L1/L2 split, D-02 dependency boundary, registry contract, and fixture
  isolation rules.
- `.planning/phases/04-l1-dict-rle-and-l2-escape-infrastructure/04-01-SUMMARY.md`
  - Phase 4 core implementation summary for dict, run-end, and L2 registry.
- `.planning/phases/04-l1-dict-rle-and-l2-escape-infrastructure/04-02-SUMMARY.md`
  - Fixture/oracle coverage added in Phase 4 and known pre-existing dirty areas.
- `.planning/phases/03-l1-bitpack-for-and-arrow-builders/03-CONTEXT.md`
  - Builder-backed Arrow output and bitpack/FOR implementation constraints.
- `.planning/phases/02-duckdb-extension-scaffold/02-CONTEXT.md` - DuckDB direct
  `DataChunk` path and the deferred Arrow stream decision.

### Code Entry Points
- `crates/loom-core/src/l2_kernel_registry.rs` - FSST kernel id `0`, L2 trait,
  and current Phase 4 stub.
- `crates/loom-core/src/l1_model.rs` - `LayoutNode`, `decode_layout_to_array_data`,
  Dictionary/RunEnd paths, and decoded-child materialization.
- `crates/loom-core/src/arrow_builder_output.rs` - Arrow builder abstraction that
  currently supports Boolean, Int32, and Int64 only.
- `crates/loom-core/src/error.rs` - Typed decode errors; malformed FSST params
  should report typed errors, not panic.
- `crates/loom-fixtures/src/vortex_reader.rs` - Vortex-to-Loom bridge and the
  right place to inspect `vortex-fsst` arrays.
- `crates/loom-fixtures/src/oracle.rs` - Vortex oracle helpers; needs Utf8 oracle
  support for Phase 05.
- `duckdb-ext/loom_extension.cpp` - Existing direct Arrow-to-DuckDB DataChunk
  population path.
- `crates/loom-ffi/src/ffi.rs` - FFI Arrow export and panic boundary.

### External Crate APIs To Confirm During Research
- `~/.cargo/registry/src/index.crates.io-*/vortex-fsst-0.74.0/src/array.rs` -
  Decomposed FSST physical layout: symbols, symbol lengths, compressed codes
  bytes, codes offsets, uncompressed lengths, and validity slots.
- `~/.cargo/registry/src/index.crates.io-*/vortex-fsst-0.74.0/src/compress.rs`
  - Fixture construction helpers for training and compressing FSST arrays.
- `~/.cargo/registry/src/index.crates.io-*/fsst-rs-0.5.11/src/lib.rs` -
  `fsst::Decompressor` API and escape-code semantics.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `L2KernelRegistry` already provides the stable kernel dispatch surface; Phase
  05 should fill `FsstKernel::decode` rather than redesign registry ownership.
- `decode_layout_to_array_data` is already the public path for top-level L2
  output ownership.
- `DecodedArray` already centralizes child materialization for Dictionary,
  RunEnd, and non-BitPack FOR fallback; extending it for Utf8 is the least
  surprising dict-over-FSST integration point.
- `loom-fixtures` already contains the Vortex bridge and oracle boundary. It can
  add FSST extraction and Utf8 oracle helpers without contaminating `loom-core`.
- `loom_extension.cpp` already knows how to read Arrow C Data Interface buffers
  and populate a DuckDB `DataChunk` directly.

### Established Patterns
- Normal decode failures return `LoomDecodeError`, not panics. FSST params
  parsing and malformed offset/validity cases should follow this pattern.
- Arrow output is built through Arrow builder APIs, not raw writes. Utf8 support
  should preserve this invariant on the Rust side.
- Vortex-only APIs are isolated to `loom-fixtures`; `cargo tree -p loom-core`
  must remain free of `vortex` and `fastlanes` crates.
- Fixtures compare Loom output against Vortex's live execution path rather than
  copied expected literals where practical.

### Integration Points
- Add a Loom-owned FSST params encoder/decoder at the `loom-core`/fixture
  boundary.
- Extend `from_array_ref` in `loom-fixtures/src/vortex_reader.rs` to recognize
  `vortex_fsst::FSST` arrays and emit `LayoutNode::KernelEscape`.
- Extend oracle helpers to decode Utf8/string arrays through Vortex canonical
  execution.
- Extend DuckDB direct vector population for Arrow Utf8 and any primitive types
  required by the controlled verification corpus.

</code_context>

<specifics>
## Specific Ideas

The user selected the recommended approach for all four gray areas:
Loom-owned FSST params with `fsst-rs`, general Utf8 gather for dict-over-FSST,
direct DuckDB `DataChunk` extension for the SQL gate, and a controlled MVP0
verification matrix.

</specifics>

<deferred>
## Deferred Ideas

- ArrowArrayStream / record-batch based DuckDB integration remains deferred
  unless direct `DataChunk` population blocks Phase 05.
- Exhaustive combinatorial verification across every possible nested encoding
  combination remains v2 scope.
- Additional L2 kernels, multi-column table functions, layout descriptor UX, and
  timing comparisons remain v2 scope per roadmap.

### Reviewed Todos (not folded)
- `.planning/todos/pending/cr-02-decode-for-non-bitpack-reference.md` matched
  Phase 05 by keyword but is not folded into this phase. The code currently
  contains the Phase 4 non-BitPack FOR fallback that applies the reference via
  `append_value_plus_reference`; any remaining cleanup belongs with todo triage,
  not Phase 05 FSST scope.

</deferred>

---

*Phase: 05-fsst-l2-kernel-and-full-verification*
*Context gathered: 2026-06-08*
