# Phase 31 Context: Full Arrow Semantic Source Compatibility

## User Intent

The target has changed from bounded Phase 28 compatibility and "core 80%" source
coverage to full semantic compatibility for every Lance and Parquet schema that
the upstream readers can materialize as Arrow, plus Vortex semantic compatibility
for every Vortex dtype/array that the Vortex reader can materialize as Arrow.

No historical payload compatibility is required for this phase. Existing `LMC1`,
`LMP1`, and `LMT1` artifacts may remain as legacy fixtures, but Phase 31 should
not preserve their narrow layout model as the primary source-ingress path.

## Locked Decisions

D-31-01: Use Apache Arrow `Schema`/`Field`/`ArrayData` as the semantic contract.
Do not keep expanding hand-written `LayoutNode::Raw` variants as the source
compatibility substrate.

D-31-02: Introduce a new Loom-owned Arrow semantic artifact format, tentatively
`LMC2` container plus `LMA1` Arrow semantic payload, rather than mutating `LMP1`
or `LMT1`.

D-31-03: Accepted Lance and Parquet compatibility means the upstream reader can
materialize batches and Loom can encode, verify, decode, and compare Arrow
schema/values/nulls/metadata without semantic loss.

D-31-04: Accepted Vortex compatibility means Vortex can materialize the file or
array to Arrow and Loom can roundtrip the resulting Arrow semantics against the
Vortex oracle. Vortex encoding-shape preservation is separate evidence, not a
precondition for semantic compatibility.

D-31-05: Native MLIR/ExecutionEngine support is not required for full source
semantic compatibility. Native lowering remains an optimization layer and must
fail closed or report interpreter/Arrow semantic path when unsupported.

D-31-06: Query engines must not define the semantic coverage boundary. DuckDB or
StarRocks may lack support for some Arrow nested/logical types; Loom should
still preserve those artifacts and report query-surface limitations separately.

D-31-07: Existing incomplete `NullableRaw` WIP is superseded. Phase 31 should
either remove it or replace it with the broader Arrow semantic substrate before
running release gates.

D-31-08: "100%" excludes malformed files, encrypted/credentialed remote sources
without configured access, and any source feature the upstream Lance/Parquet/
Vortex reader itself cannot materialize as Arrow. Those cases are rejected or
unsupported with no accepted bytes.

## Non-Goals

- StarRocks runtime completion.
- Native lowering for every Arrow type.
- Direct Parquet Dremel decoder implementation inside Loom.
- Direct Lance file-page decoder implementation inside Loom.
- Direct Vortex physical encoding decoder for every layout inside `loom-core`.
- Public C ABI churn beyond the Arrow C Data Interface/ArrayStream surfaces
  needed to export verified Arrow semantic artifacts.

## Required Tradeoff Record

Phase 31 must explicitly record that it replaces the previous narrow
source-ingress artifact path for new full-compatibility claims. It should not
pretend Phase 27/28 already delivered arbitrary schema compatibility.
