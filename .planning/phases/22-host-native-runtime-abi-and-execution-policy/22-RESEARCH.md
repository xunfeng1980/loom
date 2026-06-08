# Phase 22 Research: Host Native Runtime ABI and Execution Policy

**Date:** 2026-06-08  
**Phase:** 22 — Host Native Runtime ABI and Execution Policy  
**Depends on:** Phase 17, Phase 18, Phase 19, Phase 20, Phase 21

## Executive Summary

Phase 22 should define the host-facing native runtime contract before Loom
touches a concrete engine integration. It should not implement DuckDB native
execution, StarRocks integration, Iceberg binding, a compiled MLIR dialect, or a
new JIT backend.

Recommended direction:

1. Define a stable Loom-owned C ABI around verified artifacts, runtime plans,
   scan handles, output batches, diagnostics, fallback decisions, and cache
   identity.
2. Treat `ArtifactVerificationFacts`, Bitwuzla discharge status, production
   lowering facts, and Phase 21 reader/lowering disposition as required inputs
   to runtime planning. No native plan may be created from unverifed or merely
   collected facts.
3. Use Arrow-compatible batch output as the semantic boundary, but do not expose
   a raw `ArrowArrayStream` as the only runtime ABI yet. The runtime should own a
   small scan/batch API that can export Arrow C Data or fill host-native vectors
   in later phases.
4. Make projection, predicate, split, and concurrency contracts first-class now.
   They shape cache keys, diagnostics, fallback, and host integration; adding
   them after DuckDB or StarRocks integration would make the ABI host-shaped.
5. Define engine independence as a falsifiable design goal: Phase 22 can design
   for both DuckDB and StarRocks constraints, but Phase 27 is the first real
   evidence that the boundary is not DuckDB-specific.

The central product of Phase 22 should be a runtime contract document plus
minimal Rust/C model types and negative tests. It is an architecture-locking
phase, not a performance phase.

## Local Starting Point

Completed prerequisites:

- Phase 17 unified artifact verification into `verify_artifact` /
  `verify_artifact_with_l2_core`, with accepted/rejected/unsupported status,
  `ArtifactVerificationFacts`, `ArtifactLoweringReadiness`, and stable
  diagnostics.
- Phase 18 established complete Vortex reader facts and real `.vortex` emission
  for an accepted primitive matrix.
- Phase 19 added solver-backed discharge and records trusted solver reports on
  artifact facts.
- Phase 20 added `check_production_lowering_support`, a production lowering
  fact model, raw primitive/table output shapes, and an initial
  `loom.decode` textual surface.
- Phase 21 widened real Vortex coverage and now records reader support,
  emission disposition, and native-lowering disposition separately.

Current implementation still lacks a host-native runtime layer:

- There is no Loom-owned `RuntimePlan`, `ScanPlan`, `ScanHandle`, `BatchHandle`,
  or runtime diagnostic envelope.
- There is no cache key contract spanning artifact bytes, verified facts,
  solver/backend versions, target triple, projection/predicate/split policy, and
  host ABI version.
- There is no runtime-level fallback policy that decides native vs interpreter
  vs fail-closed from one report.
- Projection/predicate pushdown and split/concurrency semantics exist only as
  roadmap pressure, not as modeled inputs.

## External Research Notes

### Arrow C Data Is a Good Batch Boundary, Not the Whole Runtime

The Arrow C Data Interface is explicitly ABI-stable, zero-copy oriented, and
designed for sharing Arrow memory between independent runtimes in one process.
It also places lifetime under producer-owned release callbacks. This aligns
with Loom's existing Arrow C Data boundary and should remain the lowest common
denominator for cross-language batch handoff.

The Arrow C Stream Interface builds on C Data and exposes a blocking pull-style
stream of batches with `get_schema`, `get_next`, `get_last_error`, and
`release`. It explicitly does not assume the stream source is thread-safe; a
consumer that calls `get_next` from multiple threads must serialize those calls.

Implications for Loom:

- Phase 22 should preserve Arrow C Data compatibility for produced batches.
- A one-size `ArrowArrayStream` runtime API is too narrow for Loom's next goals:
  split scheduling, local thread state, native/interpreter fallback, cache keys,
  and engine-native vector filling need Loom-owned handles and planning facts.
- If Loom later exports an Arrow stream, it should be an adapter over the Loom
  scan handle, not the core runtime contract.
- Batch release ownership must be explicit and testable: the producer owns
  buffers/private data; the consumer releases only through the exported callback.

Sources:

- https://arrow.apache.org/docs/format/CDataInterface.html
- https://arrow.apache.org/docs/format/CStreamInterface.html

### DuckDB Forces Projection and Thread-Local Scan State Into the ABI

DuckDB's C table-function API has a `bind` / `init` / optional `local_init` /
main scan-function lifecycle. It can enable projection pushdown; if enabled,
DuckDB provides the required column list during `init` through
`duckdb_init_get_column_count` and `duckdb_init_get_column_index`. It also has a
thread-local init callback for scan-local state. Replacement scans can route a
logical table reference to a table function with parameters.

Implications for Loom:

- Runtime planning must separate bind-time artifact validation from init-time
  projection/split planning.
- Projection pushdown cannot be bolted on later; the runtime plan needs a
  projected-column set and output schema remapping.
- The ABI needs global scan state plus local worker state, even if Phase 22 does
  not integrate DuckDB yet.
- DuckDB should be Phase 24's first integration surface, but Phase 22 must avoid
  encoding DuckDB-specific names or `DataChunk` ownership into the generic ABI.

Sources:

- https://duckdb.org/docs/current/clients/c/table_functions
- https://duckdb.org/docs/current/clients/c/replacement_scans

### StarRocks Makes Columnar Output, Pushdown, and Metadata Binding Non-Negotiable

StarRocks documents a fully vectorized execution engine that stores, organizes,
and computes data column-wise, uses CPU cache effectively, and uses SIMD. It
also supports Iceberg catalogs that directly query Iceberg data and rely on
storage/metastore configuration and metadata caching.

Implications for Loom:

- The generic runtime ABI must be columnar and batch-oriented, not row-callback
  oriented.
- Predicate/projection pushdown, statistics, and split/zoning facts must be
  represented in a host-neutral way before StarRocks is attempted.
- Iceberg table binding in Phase 26 should pass artifact identity and verified
  facts through table metadata; StarRocks in Phase 27 should consume the same
  bound artifacts, not a StarRocks-specific Loom format.
- Engine independence remains unproven until a second consumer such as
  StarRocks exercises the same runtime contract.

Sources:

- https://docs.starrocks.io/docs/introduction/Features/
- https://docs.starrocks.io/docs/data_source/catalog/iceberg/iceberg_catalog/

## Recommended Phase 22 Scope

In scope:

- `22-RUNTIME-ABI-CONTRACT.md` defining the Loom host runtime lifecycle:
  artifact verification, runtime planning, optional native preparation, scan
  creation, worker-local state, batch production, diagnostics, fallback, and
  release.
- Loom-owned model types for:
  - artifact identity and facts fingerprints
  - runtime ABI version
  - target/toolchain/backend identity
  - projection set and output column remapping
  - predicate envelope and accepted pushdown subset
  - scan split identity and row-range/chunk ownership
  - concurrency/reentrancy/thread-local state policy
  - cache key and invalidation inputs
  - execution decision: native, interpreter fallback, or fail closed
  - runtime diagnostics with stable codes and paths
- A narrow C ABI sketch/header or Rust model that is host-neutral and does not
  include DuckDB or StarRocks types.
- Tests that prove unsupported facts, missing discharge, stale cache identity,
  unsupported projection/predicate, and invalid concurrency modes fail closed.
- Handoff documents for:
  - Phase 23 production backend implementation
  - Phase 24 DuckDB native integration
  - Phase 25 cache/fallback/equivalence hardening
  - Phase 26 Iceberg binding
  - Phase 27 StarRocks + DuckDB dual query surface

Out of scope:

- DuckDB table-function native execution implementation.
- StarRocks connector/executor implementation.
- Iceberg metadata format implementation.
- Compiled ODS dialect, production `melior` pass pipeline, LLVM lowering, or JIT
  execution. Those remain Phase 23.
- Expanded Vortex encoding semantics. Phase 21 provides finite matrix evidence;
  arbitrary semantic compatibility remains Phase 28.
- New solver backend functionality.
- Performance claims beyond shape-level ABI feasibility.

## Runtime Lifecycle Recommendation

Recommended host-neutral lifecycle:

1. `verify`: host or Loom verifies an `LMC1` artifact and obtains accepted
   `ArtifactVerificationFacts`.
2. `plan`: Loom combines artifact facts, solver status, production-lowering
   support, reader/lowering disposition, host requested projection/predicate,
   target/toolchain identity, and policy flags into a `RuntimePlan`.
3. `prepare`: optional native backend compiles or loads a cached native artifact
   only if the plan decision is native and all facts are trusted.
4. `open_scan`: host creates a scan handle for a row range, file split, table
   split, or full artifact.
5. `open_worker`: host creates local worker state when it wants parallel scans.
6. `next_batch`: runtime emits one batch through Arrow C Data-compatible output
   or a later host-native vector adapter.
7. `close_worker` / `close_scan`: release local and global state.
8. `destroy_plan`: release plan-owned native/cache resources.

Native execution must be an implementation detail of `prepare`/`next_batch`.
Every externally visible result must still be explainable as a verified artifact
plus a runtime policy decision.

## Execution Decision Matrix

| Input condition | Runtime decision | Reason |
|-----------------|------------------|--------|
| Verifier rejected/unsupported | fail closed | No runtime plan may exist for invalid artifacts |
| Constraints collected only / failed / unknown | fail closed for native; optional interpreter only if policy explicitly permits and semantics are safe | Native trust requires discharged or not-required constraints |
| Reader says accepted but lowering disposition is `interpreter-only` | interpreter fallback | Correctness evidence exists, native support does not |
| Lowering disposition is `fail-closed/deferred` | fail closed unless host explicitly chooses diagnostic-only planning | Avoid silent semantic widening |
| Production lowering support accepts, constraints discharged, projection supported | native candidate | Eligible for Phase 23 backend |
| Predicate cannot be represented in Phase 22 predicate envelope | no pushdown; scan full accepted rows, or fail closed if host requires pushdown | Preserve semantic equivalence |
| Cache key mismatch or backend/toolchain identity mismatch | rebuild or fallback by policy | Prevent stale native artifact reuse |
| Concurrent use requested for non-splittable scan | serialize, single-worker, or fail closed by policy | Avoid unsound shared-state access |

## ABI Shape Recommendation

Use opaque handles and plain C-compatible structs:

- `LoomRuntimePlanHandle`
- `LoomScanHandle`
- `LoomWorkerHandle`
- `LoomBatchHandle`
- `LoomRuntimeStatus`
- `LoomRuntimeDiagnostic`
- `LoomRuntimePlanRequest`
- `LoomProjectionSet`
- `LoomPredicateEnvelope`
- `LoomSplitDescriptor`
- `LoomExecutionDecision`
- `LoomCacheKey`

Do not expose Rust enums, Vortex types, DuckDB `DataChunk`, StarRocks chunks, or
MLIR handles in the host-neutral ABI. Those can be adapters around the generic
contract in later phases.

## Pushdown and Concurrency Policy

Projection:

- Phase 22 should support column-id projection for `LMT1` and single-column
  identity projection for `LMP1`.
- Projected output order must be explicit and part of the plan/cache key.
- Unsupported projection over missing columns, nested paths, or reordered
  unsupported artifacts must fail closed.

Predicate:

- Phase 22 should define a predicate envelope even if it accepts only `none` and
  a narrow future-ready shape such as column comparison over primitive columns.
- If predicate pushdown is requested but unsupported, the policy must choose
  either `no_pushdown_scan_all` or `fail_closed_required_pushdown`.
- Predicate semantics must be separate from statistics pruning; both affect
  cache identity.

Splits and concurrency:

- A scan must declare whether it is splittable.
- Worker-local state must be separate from scan-global immutable state.
- Shared state must be immutable or internally synchronized.
- The ABI should allow parallel split execution, but Phase 22 tests can start
  with single-worker and explicit rejection of unsafe concurrent modes.

## Cache Key Inputs

Minimum cache key components:

- runtime ABI version
- artifact digest
- artifact kind and payload kind
- verifier facts fingerprint
- solver report/script id or `not-required` marker
- production lowering facts fingerprint
- backend name and version
- target triple / CPU feature policy
- LLVM/MLIR/melior/backend toolchain identity where native is used
- projection set and output column order
- predicate envelope
- split/chunk policy
- fallback policy
- safety policy flags

Cache identity must be a documented contract, not a hash of ad hoc debug output.

## Recommended Plan Split

### 22-01: Runtime ABI Contract and Lifecycle Model

Write `22-RUNTIME-ABI-CONTRACT.md` and add Loom-owned runtime model types for
plans, handles, decisions, diagnostics, projection, predicates, splits, and
cache keys. No host engine types.

### 22-02: Verified Facts Handoff and Execution Decision Policy

Implement/report the deterministic decision function from artifact verifier
facts, solver status, production-lowering support, reader/lowering disposition,
and host policy to native/interpreter/fail-closed.

### 22-03: Projection, Predicate, and Split Planning Envelope

Add tests and docs for projection remapping, predicate support/fallback, split
descriptors, and concurrency/reentrancy policy. Start narrow, but make unsupported
cases explicit.

### 22-04: Cache Key, Diagnostics, and ABI Sketch

Define cache-key inputs, invalidation behavior, stable diagnostic codes, and a C
ABI sketch/header or equivalent model. Add negative coverage for stale or
incompatible keys.

### 22-05: Report, Release Gate, and Phase 23/24/26/27 Handoff

Write final Phase 22 report, update public/planning docs, add a focused runtime
ABI gate script, and hand off exact contract requirements to production backend,
DuckDB integration, Iceberg binding, and StarRocks/DuckDB dual query surface.

## Recommendation

Proceed with Phase 22 as a five-plan phase. The implementation should be mostly
contract/model/test work, with just enough code to make the policy executable
and fail-closed. That gives Phase 23 a stable target for native backend work and
prevents Phase 24 DuckDB integration from silently becoming the generic ABI.
