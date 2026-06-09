# Phase 31 Research: Arrow Semantic Container for Full Source Compatibility

## Summary

The correct substrate for arbitrary Lance/Parquet schema coverage is not another
set of Loom-specific scalar layout nodes. Lance and Parquet are Arrow-adjacent
sources, and Vortex can materialize semantic arrays through its reader. The
phase should promote Arrow `ArrayData`/`RecordBatch` semantics to a first-class
Loom artifact.

## External Source Facts

- Apache Arrow's Rust `DataType` covers primitive, string/binary, decimal,
  temporal, nested, dictionary, run-end, and view families. Arrow also provides
  `DataType` display/parse invariants that can help serialize schema identity.
- The Arrow columnar format defines validity bitmaps, offsets, child arrays, and
  nested array invariants. For nested types other than unions, each nested array
  has its own top-level validity bitmap independent of child validity.
- Parquet logical types extend physical types, and Parquet LIST/MAP/nested
  semantics rely on definition and repetition levels. The phase should rely on
  parquet-rs to materialize those semantics as Arrow and then verify/persist the
  Arrow result.
- Lance schema maps to Apache Arrow types and adds field IDs/parent IDs that
  support schema evolution. Loom should preserve those identifiers as source
  facts/metadata alongside the Arrow semantic artifact.
- Vortex uses a root `DType` instead of a traditional table schema. Semantic
  compatibility should compare materialized Arrow values/nulls against Vortex
  oracle output, while encoding-shape facts remain additional evidence.

## Recommended Architecture

1. Add an Arrow semantic artifact layer:
   - `LMA1` for one Arrow array or one record batch.
   - `LMC2` as a container wrapper carrying schema, payload, source facts,
     verifier report, and optional compatibility matrix evidence.
2. Store Arrow semantics through `arrow_data::ArrayData` trees, not by
   downcasting every concrete Arrow array into hand-written Loom layout variants.
3. Verify with Arrow's full validation plus Loom-specific invariants:
   - schema/column count alignment,
   - row-count consistency,
   - child length/offset validity,
   - dictionary key bounds,
   - union type-id and child consistency,
   - metadata/source identity hash consistency,
   - fail-closed unsupported reader features.
4. Keep Lance, Parquet, and Vortex SDKs isolated in adapter crates. `loom-core`
   may depend on Arrow, but not Lance/Parquet/Vortex reader crates.
5. Export verified artifacts through Arrow C Data Interface and, for tables,
   Arrow ArrayStream or a record-batch export path. Engine-specific query
   limitations must be reported separately from source semantic compatibility.

## Compatibility Definition

Accepted source compatibility requires:

- upstream reader succeeds;
- Loom encodes all batches and schema metadata into a new semantic artifact;
- Loom verifier accepts the artifact;
- Loom decoder reconstructs Arrow schema and arrays;
- Arrow semantic equality passes against source/oracle batches, including nulls,
  nested offsets, dictionaries, logical type metadata, and extension metadata
  where Arrow can represent it.

Rejected/unsupported cases must produce diagnostics and no accepted bytes.

## Risks

- Arrow IPC shortcut risk: using IPC as an opaque blob would be easy but would
  weaken Loom's verifier story. If IPC is used internally, Phase 31 must still
  expose and verify the Arrow schema/tree invariants.
- Nested equality risk: value equality must distinguish null list vs empty list,
  null struct vs child null, dictionary identity vs decoded values, and sliced
  array offsets.
- Engine overclaim risk: DuckDB SQL may not cover every Arrow type. Query
  evidence must not be treated as full semantic coverage.
- Workspace churn risk: reader crates may pull different Arrow/Parquet versions.
  Adapter dependencies must stay pinned and audited.

## Split Recommendation

Six plans:

1. Contract and dependency cleanup.
2. Arrow semantic payload codec/verifier/decode in `loom-core`.
3. Parquet arbitrary Arrow schema ingestion and e2e equality.
4. Lance arbitrary Arrow schema ingestion and e2e equality with field metadata.
5. Vortex arbitrary DType semantic materialization and e2e equality.
6. Compatibility matrix, release gate, docs, and old-claim correction.
