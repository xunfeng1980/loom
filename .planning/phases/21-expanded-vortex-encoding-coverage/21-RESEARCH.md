# Phase 21 Research: Expanded Vortex Encoding Coverage

**Date:** 2026-06-08  
**Phase:** 21 — Expanded Vortex Encoding Coverage  
**Depends on:** Phase 18, Phase 19, Phase 20

## Executive Summary

Phase 21 should widen the real Vortex support matrix, but it should not be a
generic "support everything" phase and should not reopen the production backend
as a broad implementation sink.

Recommended direction:

1. Treat Vortex coverage as a matrix of `(dtype, array encoding, layout,
   nullability, segmentation, emission kind, lowering disposition)`.
2. Add representative coverage in risk order: nullable primitive/raw, chunked
   primitive layouts, dictionary primitive, run-end/sequence primitive,
   bitpack/FOR integer, and a narrow UTF-8/FSST-compatible path if facts are
   sufficient.
3. For every new accepted Vortex shape, record one of three dispositions:
   `interpreter-only`, `production-lowering-supported`, or
   `fail-closed/deferred`.
4. Keep `vortex-file`, `vortex-layout`, and Vortex crate details isolated to
   `loom-vortex-ingress`; emit only Loom-owned facts and artifacts.
5. Preserve Phase 20's production lowering seed as a consumer, not the owner, of
   expanded encoding semantics. Any native delta discovered in Phase 21 should
   be documented as input to Phase 23.

This is the phase that should make "expanded reader coverage" precise enough
for Phase 22 ABI design and Phase 23 production backend work.

## Local Starting Point

Completed prerequisites:

- Phase 18 established the complete reader boundary and emits `LMC1(LMP1)` or
  `LMC1(LMT1)` for non-null primitive Int32, Int64, Float32, Float64 single
  columns and primitive struct/table slices.
- Phase 19 added Bitwuzla-backed solver discharge and requires native consumers
  to trust only discharged artifact facts.
- Phase 20 added a production native-lowering seed over accepted artifacts,
  discharged facts, primitive Arrow/raw-buffer output, all-valid primitive
  columns, and raw primitive copy. Bitpack/FOR/dictionary/RLE/native expansion
  remain explicitly deferred.

Current accepted Vortex-to-Loom emission matrix:

| Vortex shape | Current reader result | Loom artifact | Current lowering |
|--------------|----------------------|---------------|------------------|
| Non-null primitive `i32` | Accepted | `LMC1(LMP1)` | raw primitive supported |
| Non-null primitive `i64` | Accepted | `LMC1(LMP1)` | raw primitive supported |
| Non-null primitive `f32` | Accepted | `LMC1(LMP1)` | raw primitive supported |
| Non-null primitive `f64` | Accepted | `LMC1(LMP1)` | raw primitive supported |
| Non-null primitive struct/table | Accepted | `LMC1(LMT1)` | raw primitive supported |
| UTF-8 / string field | Unsupported | none | unsupported |
| Nullable primitive | Unsupported | none | unsupported |
| Complex layouts/encodings | Facts may exist, emission unsupported | none | unsupported/deferred |

## External Research Notes

### Vortex File Complexity Lives in Layouts

The Vortex file format is intentionally small. The official file-format docs
state that most complexity is encapsulated in layouts, and the file is
essentially a serialized layout tree plus data segments. The file format also
separates dtype, layout, statistics, and footer segments.

Implication for Loom:

- Phase 21 should expand by layout/array support, not by treating the Vortex file
  header as the main compatibility problem.
- Reader facts should keep exposing layout tree, dtype, segment, statistics, and
  support classification in Loom-owned vocabulary.

Sources:

- https://docs.vortex.dev/specs/file-format
- https://docs.vortex.dev/concepts/layouts

### Vortex Arrays Are Extensible and Canonicalizable

Vortex arrays are tree structures with length, dtype, children, buffers,
statistics, and vtables. Vortex defines one canonical encoding per logical type
to avoid exponential combinations, but also ships many built-in and compressed
encodings: dictionary, chunked, constant, slice, list/varbin, ALP, FastLanes
bitpacking, delta, FOR, RLE, FSST, PCodec, RunEnd, Sequence, Sparse, ZigZag, and
ZStd.

Implication for Loom:

- Phase 21 should not attempt arbitrary Vortex semantics. It should choose a
  finite, reviewable matrix.
- Canonicalization is useful for oracle/equivalence, but native lowering needs
  explicit encoded facts rather than a black-box decompressed array.
- Each accepted encoding must preserve enough facts for verifier/lowering
  consumers; otherwise it should remain interpreter-only or fail closed.

Source:

- https://docs.vortex.dev/concepts/arrays

### Statistics, Layouts, and Pushdown Affect Later ABI

Vortex arrays/layouts carry statistics such as null count, run count,
constant-ness, sortedness, and layout-level zone maps. The Vortex Scan API
models filter/projection pushdown, split generation, and concurrent split
execution.

Implication for Loom:

- Phase 21 should record statistics availability even when it does not yet use
  statistics for pruning.
- Phase 21 should expose whether a layout is chunked/zoned/splittable because
  Phase 22 must decide predicate/projection pushdown and concurrency semantics.
- Do not defer all pushdown-related facts to Phase 22; Phase 22 needs concrete
  facts from Phase 21 to design the ABI.

Sources:

- https://docs.vortex.dev/concepts/arrays
- https://docs.vortex.dev/concepts/scanning.html
- https://docs.vortex.dev/api/cpp/scan

### Forward Compatibility and WASM Decompression Are Out of Scope

The Vortex file-format docs describe future forward compatibility with embedded
WebAssembly decompression logic for encodings/layouts added after a minimum
reader version.

Implication for Loom:

- Phase 21 should not implement WASM decompression or arbitrary extension
  encodings.
- Unknown/extension encodings should produce stable unsupported diagnostics and
  no Loom artifact emission.
- The roadmap should keep extension/WASM support as a later compatibility topic,
  not as a Phase 21 deliverable.

Source:

- https://docs.vortex.dev/specs/file-format

## Recommended Phase 21 Scope

In scope:

- A `VortexEncodingCoverage` / support-matrix document or type that records
  dtype, nullability, array/layout encoding, emission kind, verifier status,
  oracle coverage, and lowering disposition.
- Reader facts for nullable primitive validity, chunked layouts, dictionary
  layouts/arrays, run-end/sequence-like layouts, FastLanes bitpack/FOR integer
  arrays, and string/varbin facts.
- A finite accepted expansion matrix with Vortex scan oracle evidence.
- Stable unsupported diagnostics for valid-but-deferred encodings.
- `LMC1` emission only when Loom structural verifier, artifact verifier, and
  solver/lowering handoff facts are sufficient.
- Explicit handoff notes for Phase 22 ABI and Phase 23 production backend.

Out of scope:

- Full arbitrary Vortex encoding support.
- WASM decompression support.
- Production compiled ODS dialect or `melior`/LLVM/JIT backend implementation.
- DuckDB native execution integration.
- Iceberg binding or multi-engine query surface.
- New solver backend strategy.

## Candidate Coverage Matrix

Recommended priority order:

| Priority | Shape | Reader goal | Artifact goal | Lowering disposition |
|----------|-------|-------------|---------------|----------------------|
| P1 | Nullable primitive `i32/i64/f32/f64` | Accept validity facts | Emit `LMP1`/`LMT1` with validity | interpreter-only first; Phase 20 seed needs validity delta |
| P1 | Chunked primitive / row partitions | Accept split/chunk facts | Emit table/layout if row order is deterministic | interpreter-only unless raw-copy lowering can compose chunks |
| P2 | Dictionary primitive | Accept dictionary/codes/value facts | Emit existing `LayoutNode::Dictionary` where safe | interpreter-only first; lowering deferred |
| P2 | RunEnd / Sequence primitive | Accept run facts | Emit `LayoutNode::RunEnd` or sequence-equivalent | interpreter-only first; lowering deferred |
| P2 | Bitpack integer | Accept bit width, block, validity, length facts | Emit `LayoutNode::BitPack` when facts are complete | lowering-supported only after native delta is planned |
| P2 | FOR integer | Accept reference + inner bitpack facts | Emit `LayoutNode::FrameOfReference` when facts are complete | lowering-supported only after native delta is planned |
| P3 | UTF-8 / VarBin / FSST-compatible | Accept string facts | Emit only if Loom-owned FSST/kernel params are available | interpreter-only or deferred |
| P3 | ALP / PCodec floats | Accept compressed float facts | Emit only if Loom-owned params can be derived safely | interpreter-only or deferred |
| P4 | Zoned/statistics layouts | Record pruning facts | No artifact expansion unless data child is supported | ABI handoff only |
| P4 | Custom/extension/WASM encodings | Stable unsupported facts | No emission | fail-closed/deferred |

## Acceptance Rule

For any candidate Vortex shape, Phase 21 should require:

1. Real Vortex fixture generation or fixture loading.
2. Reader facts that identify dtype, layout/encoding, nullability, row count,
   child structure, segment ranges, and statistics availability.
3. Stable `accepted`, `unsupported`, or `rejected` classification.
4. Vortex scan oracle evidence for accepted emission.
5. Artifact verifier acceptance for emitted `LMC1`.
6. Solver status recorded for artifact facts when the shape feeds native
   lowering.
7. Lowering disposition recorded explicitly:
   - `interpreter-only`
   - `production-lowering-supported`
   - `fail-closed/deferred`

## Recommended Plan Split

### 21-01: Coverage Matrix and Reader Fact Contract

Define the support matrix vocabulary and extend reader facts to record encoding,
nullability, layout class, chunk/split shape, statistics presence, and lowering
disposition without importing Vortex types into `loom-core`.

### 21-02: Nullable Primitive and Chunked Primitive Coverage

Add the lowest-risk coverage expansion: nullable primitive validity and chunked
primitive layout facts. Emit artifacts only where verifier/oracle evidence is
complete.

### 21-03: Dictionary, RunEnd, and Sequence Coverage

Map Vortex dictionary/run-end/sequence-like shapes into Loom-owned facts and
existing L1 model support where safe. Default native lowering disposition should
be interpreter-only or deferred.

### 21-04: Bitpack/FOR and Numeric Compression Coverage

Add FastLanes bitpack/FOR integer facts and evaluate whether Loom can emit
existing `LayoutNode::BitPack` / `FrameOfReference` from real Vortex facts. Keep
native lowering disposition explicit; do not silently claim Phase 23 support.

### 21-05: Report, Release Gate, and Phase 22/23 Handoff

Create the Phase 21 report, release gate, public docs update, and explicit
handoff: pushdown/concurrency facts for Phase 22 and native-delta backlog for
Phase 23.

## Recommendation

Proceed with Phase 21 as a five-plan phase focused on a finite coverage matrix.
The core deliverable is not raw breadth; it is disciplined coverage expansion
that makes every accepted Vortex shape explicit about verifier, artifact,
oracle, and lowering consequences.
