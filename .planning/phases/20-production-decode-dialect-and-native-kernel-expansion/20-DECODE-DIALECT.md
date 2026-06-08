# Phase 20 `loom.decode` Dialect Contract

## Purpose

`loom.decode` is the post-verification decode dialect surface. It is emitted
only after an artifact has passed Loom's unified verifier and production
native-lowering gate.

## Trust Model

`loom.decode` is not a distribution format. It is not accepted from untrusted
storage. It is generated inside the engine/tooling boundary from accepted Loom
artifact facts. Every module must carry artifact kind, payload kind, row bound,
constraint status, and verified feature metadata.

## Operation Inventory

Initial op names:

- `loom.decode.module`
- `loom.decode.kernel`
- `loom.decode.input_slice`
- `loom.decode.column`
- `loom.decode.builder`
- `loom.decode.finish`
- `loom.decode.for_rows`
- `loom.decode.raw_copy`
- `loom.decode.bit_unpack`
- `loom.decode.for_delta`
- `loom.decode.validity_all_valid`
- `loom.decode.validity_copy`

Phase 20 requires `loom.decode.module`, `loom.decode.kernel`,
`loom.decode.column`, `loom.decode.builder`, `loom.decode.for_rows`,
`loom.decode.raw_copy`, `loom.decode.validity_all_valid`, and
`loom.decode.finish` for the initial primitive matrix. `bit_unpack`,
`for_delta`, and `validity_copy` may remain declared but unsupported until the
facts and kernel expansion are ready.

## Module Metadata

`loom.decode.module` records:

- artifact kind;
- payload kind;
- row bound;
- constraint status;
- backend name;
- supported column count.

## Input Ops

`loom.decode.input_slice` and `loom.decode.column` identify verified input and
logical output columns. The initial implementation may omit physical input-slice
details when the unified artifact facts do not yet expose stable per-column
source ranges.

## Builder Ops

`loom.decode.builder` declares one Arrow/raw-buffer builder per output column.
The first supported types are Int32, Int64, Float32, and Float64. Nullable,
variable-size, dictionary, RLE, FSST, ALP, and nested builders are unsupported
until later plans expand the matrix.

## Control Ops

`loom.decode.for_rows` expresses the finite row loop from the verified row
bound. It must not carry target SIMD width. Vectorization is a later MLIR
lowering decision.

## Decode Primitive Ops

`loom.decode.raw_copy` is the initial production primitive. `bit_unpack` and
`for_delta` are reserved for guarded expansion when discharged facts are
sufficient.

## Nullability Ops

`loom.decode.validity_all_valid` marks the initial non-null primitive output
path. `loom.decode.validity_copy` is reserved for copied validity bitmap support.

## Lowering to Standard MLIR

The dialect surface lowers to standard `func`, `arith`, `scf`, `memref`, and
eventually `vector` dialect operations. The standard dialect lowering is what
`mlir-opt` validates in Phase 20.

## ODS Registration Plan

A compiled C++/ODS dialect is optional and toolchain-gated in Phase 20. The hard
deliverable is the stable dialect contract plus deterministic textual surface.
ODS registration should become mandatory only after op names, attributes, and
lowering patterns stop moving.

## Non-Goals

- Accepting `loom.decode` from untrusted artifact storage.
- Replacing the Loom distribution format with MLIR.
- Host execution or DuckDB native integration.
- Arbitrary Vortex encoding coverage.
- Physical SIMD width selection.
