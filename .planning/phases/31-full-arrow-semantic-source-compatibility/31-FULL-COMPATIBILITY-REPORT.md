# Phase 31 Full Arrow Semantic Compatibility Report

## Status

Phase 31 is complete.

This is the first Loom phase that makes a full Arrow semantic source-compatibility
claim for external sources:

- Parquet: any schema the parquet-rs Arrow reader materializes can enter Loom as
  verifier-accepted `LMA1`.
- Lance: any schema the Lance scanner materializes can enter Loom as
  verifier-accepted `LMA1`.
- Vortex: any source/dtype the Vortex Arrow executor materializes can enter Loom
  as verifier-accepted `LMA1`.

Phases 27 and 28 remain bounded historical evidence. They proved source facts,
legacy readability, and a compatibility matrix over the previous narrow
`LMC1(LMP1/LMT1)` raw/table path. Phase 31 replaces that accepted source path
with Arrow semantic artifacts.

## Accepted Definition

A source is accepted only when all of these are true:

1. The source reader opens the source successfully.
2. The source reader materializes Arrow `RecordBatch` values.
3. Loom encodes those batches into `LMA1`.
4. `verify_artifact` accepts the `LMA1` bytes.
5. Decoded `LMA1` Arrow batches equal the source/oracle Arrow batches in focused
   e2e tests.

Malformed or unreadable sources remain rejected. Source-reader limitations
remain outside Loom's semantic claim.

## Artifact And Verifier Boundary

- `LMA1` is the implemented Arrow semantic payload.
- `LMA1` currently uses Arrow IPC stream bytes as the carrier.
- Arrow IPC is not the trust boundary: encode/decode runs through
  `verify_arrow_semantic_payload`, and `verify_artifact` accepts only decoded,
  validated Arrow semantic payloads.
- `LMC2` remains the documented container direction but is not required for the
  Phase 31 source e2e evidence.
- Native lowering remains separate. `LMA1` artifacts are verifier-accepted and
  interpreter/semantic-path evidence, not native-lowering-ready evidence.

## Source Matrix

| Source | Implemented accepted path | Evidence |
|---|---|---|
| Parquet | `emit_source_ingress_lma1_from_parquet_path` | parquet-rs Arrow scan -> `LMA1` -> verify -> decode equality |
| Lance | `emit_source_ingress_lma1_from_lance_path` | Lance scanner -> `LMA1` -> verify -> decode equality |
| Vortex | `emit_source_ingress_lma1_from_vortex_buffer` | Vortex Arrow executor -> `LMA1` -> verify -> decode equality |

Focused schema/dtype coverage includes nullable scalar values, bool, UTF-8,
struct, list, primitive, and Vortex root struct/table materialization.

## Command Evidence

- `cargo test -p loom-core --test arrow_semantic`
- `cargo test -p loom-parquet-ingress --test full_arrow_schema_compatibility`
- `cargo test -p loom-lance-ingress --test full_arrow_schema_compatibility`
- `cargo test -p loom-vortex-ingress --test full_arrow_dtype_semantic_compatibility`
- `scripts/full-arrow-semantic-compatibility-test.sh`

The Phase 31 gate is wired into `scripts/mvp0-verify.sh` after the Phase 28
bounded semantic compatibility gate and before the Phase 29 Iceberg binding
gate.

## Non-Goals

Phase 31 does not claim:

- DuckDB SQL can query every Arrow nested/logical type.
- StarRocks runtime integration exists for every Arrow type.
- MLIR/native lowering supports every Arrow type.
- Loom directly decodes Parquet Dremel pages, Lance pages, or every Vortex
  physical encoding inside `loom-core`.
- `LMC1(LMP1/LMT1)` is the full source-compatibility substrate.

## Residual Risks

- `LMC2` wrapping is still documented but not yet implemented as the production
  container around `LMA1`.
- The focused tests are representative rather than exhaustive across every
  Arrow logical family. Decimal, temporal, binary, map, dictionary, union,
  run-end, empty, and all-null fixture rows should be expanded next.
- Query-engine adapters and native lowering must continue to fail closed or
  route through semantic/interpreter paths for unsupported Arrow families.

## Tradeoffs

- Direct `LMA1` verifier acceptance allowed the source-compatibility claim to be
  made without mutating `LMC1` or adding one Loom layout node per Arrow type.
- Legacy Vortex `LMC1` helpers remain for older raw/table tests; Parquet and
  Lance public accepted emission helpers were renamed to `lma1` because Phase 31
  intentionally has no API compatibility burden.
