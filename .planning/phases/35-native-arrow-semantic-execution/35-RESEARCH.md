# Phase 35: Native Arrow Semantic Execution - Research

**Gathered:** 2026-06-09
**Status:** Complete

## Research Question

How can Loom claim native execution for Arrow semantic artifacts without
confusing it with DuckDB SQL queryability or legacy native lowering scaffolds?

## Summary Recommendation

Add an engine-neutral `loom-core` native Arrow semantic executor. It should take
artifact bytes plus verifier options, require an accepted `LMC2(LMA1)` or direct
`LMA1` report, decode exactly one Arrow semantic record batch, run a native Rust
column-copy path for supported fixed-width primitive nullable columns, and emit
equivalence evidence against the decoded reference batch.

This makes the first native Arrow semantic claim narrow but real:

1. verifier-accepted artifact bytes are the trust boundary;
2. `LMC2` distribution wrapping is the default input;
3. native output is a newly-built Arrow batch, not a borrowed decoded batch;
4. unsupported Arrow shapes fail closed with stable diagnostics;
5. DuckDB remains a consumer, not the proof source.

## Current Code Findings

### Artifact Verification

- `verify_artifact` accepts direct `LMA1` and wrapped `LMC2` artifacts and sets
  `artifact_kind` to `LMA1` or `LMC2`.
- Both Arrow semantic artifact forms use payload kind `"Arrow semantic payload"`
  and expose `row_count_bound`.
- Lowering readiness currently reports `arrow-semantic-lowering-deferred`.
- The verifier does not currently expose Arrow field-level facts; a native
  Arrow semantic executor can derive shape from the decoded payload after
  verifier acceptance.

### Native Lowering

- `production_native_lowering` is legacy `LMP1`/`LMT1` oriented and intentionally
  rejects `"Arrow semantic payload"`.
- That module is still useful for diagnostic vocabulary and backend identity
  patterns, but forcing Arrow semantic execution through L2Core facts would add
  artificial coupling.
- Phase 35 can add a separate Arrow semantic backend identity and support report
  without weakening the existing legacy production-native contracts.

### Arrow Execution

- `ArrowSemanticPayload::to_record_batches` already reconstructs reference
  Arrow batches.
- Arrow `PrimitiveArray<T>` and `BooleanArray` can be downcast and copied
  through typed builders, preserving nulls row-by-row.
- Comparing native and reference arrays can use Arrow array equality. Tests
  should include nullable numeric and boolean columns so validity handling is
  load-bearing.

## Rejected Alternatives

### Mark Arrow semantic artifacts lowering-ready in artifact verifier only

That would be route metadata, not native execution. Phase 35 needs a concrete
native output and equivalence check.

### Reuse DuckDB's Arrow C Data scan as native evidence

DuckDB queryability was Phase 34. It proves SQL consumption, not an
engine-neutral native backend.

### Support all Arrow semantic shapes immediately

Utf8 variable buffers and nested children require different kernel and
equivalence mechanics. The first phase slice should support fixed-width
primitive nullable data and fail closed elsewhere.
