# Phase 31 Arrow Semantic Contract

## Scope

Phase 31 changes the source-compatibility substrate from narrow Loom layout
payloads to verifier-backed Arrow semantic artifacts.

The compatibility target is:

- any Lance source the Lance reader can materialize as Arrow;
- any Parquet source parquet-rs can materialize as Arrow;
- any Vortex source or dtype the Vortex reader can materialize as Arrow.

Malformed sources, unavailable credentials, encrypted sources without configured
access, and source features the upstream reader cannot materialize as Arrow are
rejected or unsupported and must not emit accepted Loom bytes.

## Artifact Names

- `LMC2`: Loom container for Arrow semantic artifacts and source facts.
- `LMA1`: Loom Arrow semantic payload v1.

`LMC1(LMP1/LMT1)` remains legacy narrow evidence. It must not be used for new
full-schema source-compatibility claims.

## Semantic Unit

The semantic unit is an Arrow schema plus one or more Arrow arrays or record
batches. At runtime, Loom represents this with Arrow `Schema`/`Field` and
`ArrayData` trees:

- data type;
- length and offset;
- null count and validity;
- buffers;
- child arrays;
- dictionary values;
- schema and field metadata.

The phase must not add one Loom layout node per Arrow type. Arrow's semantic
tree is the primary model.

## Acceptance

An artifact is accepted only when all of the following are true:

1. The upstream source reader materializes Arrow batches.
2. Loom encodes the schema and array trees into `LMA1`/`LMC2`.
3. Loom verifier accepts the artifact.
4. Loom decodes it back to Arrow.
5. Schema, values, nulls, nested offsets, dictionaries, and metadata compare
   equal to the source/oracle Arrow batches.

## Verification

The verifier must use Arrow full validation where available and add
Loom-specific checks:

- schema field count equals column count;
- all columns in a batch have the same row count;
- child arrays and offset buffers satisfy Arrow invariants;
- dictionary keys are in range;
- union type IDs and children are consistent;
- source identity and metadata hashes match the report.

## Non-Goals

- Native MLIR support for every Arrow type.
- DuckDB or StarRocks query support for every Arrow type.
- Direct Parquet Dremel decoding in `loom-core`.
- Direct Lance page decoding in `loom-core`.
- Direct all-layout Vortex physical decoding in `loom-core`.

## Tradeoff

This is a deliberate reset. It preserves old evidence as legacy but stops using
the old narrow raw/table layout path for full source compatibility. That avoids
the previous failure mode where routing, fallback, or canonical raw evidence was
mistaken for full semantic coverage.
