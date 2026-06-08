# Phase 21 Coverage Report

## Scope

Phase 21 expanded the real Vortex reader coverage matrix without claiming
arbitrary Vortex support. The core deliverable is a stable separation between
reader support, artifact emission, oracle evidence, and native-lowering
disposition.

## Coverage Matrix

The implemented matrix is maintained in
`21-COVERAGE-MATRIX.md`. It covers:

- non-null primitive and non-null primitive table baseline;
- nullable primitive `i32/i64/f32/f64` fail-closed facts;
- chunked primitive row-order evidence;
- dictionary and run-end/RLE canonical raw evidence;
- bitpack and FOR canonical raw evidence;
- UTF-8/VarBin and wider compression deferrals.

## Accepted Emission Matrix

| Shape | Emission | Verification |
|---|---|---|
| Non-null primitive `i32/i64/f32/f64` | `LMP1` wrapped in `LMC1` | artifact verifier accepted |
| Non-null primitive struct/table | `LMT1` wrapped in `LMC1` | artifact verifier accepted |
| Chunked primitive fixture | canonical `LMP1` when Vortex scan produces deterministic primitive rows | row-order oracle + artifact verifier |
| Dictionary primitive fixture | canonical `LMP1` when Vortex file exposes scan-stable primitive rows | row oracle + artifact verifier |
| Run-end/RLE primitive fixture | canonical `LMP1` when Vortex file exposes scan-stable primitive rows | row oracle + artifact verifier |
| Bitpack integer fixture | canonical `LMP1` when Vortex file exposes scan-stable primitive rows | row oracle + artifact verifier |
| FOR integer fixture | canonical `LMP1` when Vortex file exposes scan-stable primitive rows | row oracle + artifact verifier |

Canonical raw emission is labelled as canonical raw. It is not a structured
`LayoutNode::Dictionary`, `RunEnd`, `BitPack`, or `FrameOfReference` claim.

## Unsupported and Deferred Matrix

| Shape | Status | Reason |
|---|---|---|
| Nullable primitive `i32/i64/f32/f64` | unsupported, fact-bearing | current Loom artifact emission does not represent validity for this real-ingress slice |
| UTF-8/VarBin | unsupported, fact-bearing | no Loom-owned string compression params are extracted from real Vortex files |
| ALP/PCodec/FSST-compatible real Vortex compression | deferred | no deterministic Loom-owned params are extracted and verifier-gated in Phase 21 |
| Structured native dictionary/run-end/bitpack/FOR lowering | deferred | Phase 20 production native gate intentionally rejects complex kernels |

## Oracle Evidence

Phase 21 added focused Vortex scan oracle tests:

- `nullable_primitive_coverage` preserves null positions for `i32/i64/f32/f64`;
- `chunked_primitive_coverage` preserves row order for chunked `i32`;
- `dictionary_runend_coverage` preserves dictionary and RLE-expanded rows;
- `bitpack_for_coverage` preserves bitpack and FOR-decoded rows.

## Artifact Verifier Evidence

Accepted emissions remain routed through existing `LMC1` artifact verification.
The Phase 21 gate also runs `cargo test -p loom-core --test artifact_verifier`
so malformed/unsupported artifact cases stay fail-closed.

## Lowering Disposition

`VortexEncodingCoverage` now records:

- `emission_disposition`: `none`, `canonical-raw`, `canonical-table`, or
  `structured-layout`;
- `lowering_disposition`: `interpreter-only`,
  `production-lowering-supported`, or `fail-closed/deferred`.

Production lowering support is only reported for currently supported emitted
artifact shapes. Original Vortex encodings that are canonicalized through scan
remain backend deltas unless the coverage fact explicitly proves otherwise.

## Phase 22 ABI Handoff

Phase 22 should consume the following facts:

- split/chunk presence and row ranges as scheduling and concurrency inputs;
- statistics presence as a predicate/projection pushdown design input;
- emission disposition as a host fallback policy input;
- fail-closed/deferred diagnostics as a stable host-visible error surface.

Phase 22 must decide reentrancy, memory ownership, thread ownership,
projection/predicate pushdown, cache key inputs, and interpreter fallback
policy before DuckDB-native execution.

## Phase 23 Backend Handoff

Phase 23 owns the production backend deltas for:

- structured dictionary lowering;
- structured run-end/RLE lowering;
- bitpack and FOR native kernels;
- nullable validity materialization;
- ALP/PCodec/string compression only after Loom-owned params are extracted and
  verifier-gated.

The Phase 20 native gate remains the guardrail: complex kernels are recognized
but fail closed until backend support is deliberately added.

## Commands Run

- `cargo test -p loom-vortex-ingress --test reader_facts_contract`
- `cargo test -p loom-vortex-ingress --test nullable_primitive_coverage`
- `cargo test -p loom-vortex-ingress --test chunked_primitive_coverage`
- `cargo test -p loom-vortex-ingress --test dictionary_runend_coverage`
- `cargo test -p loom-vortex-ingress --test bitpack_for_coverage`
- `cargo test -p loom-core --test artifact_verifier`
- `cargo test -p loom-core --test production_native_kernels`

## Residual Risks

- Real Vortex writer/scan may canonicalize some encoded arrays before Loom sees
  structured encoding internals. Phase 21 records this as canonical raw evidence,
  not structured encoding support.
- Nullable primitive artifact emission remains deferred until validity is
  represented and verifier-gated end to end.
- ALP/PCodec/FSST-compatible real file compression remains deferred until
  Loom-owned params can be extracted deterministically.
