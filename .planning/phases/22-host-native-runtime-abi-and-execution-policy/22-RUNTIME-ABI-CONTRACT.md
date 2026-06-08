# Phase 22 Runtime ABI Contract

## Scope

Phase 22 defines the host-neutral Loom runtime contract that later host engines
and native backends call. It is a policy and model boundary over verified Loom
artifacts, not a host-engine integration and not a production compiler.

The runtime contract consumes accepted artifact verifier reports, solver
discharge state, production-lowering support facts, reader coverage disposition,
host query-shape requests, cache identity, and fallback policy.

## Lifecycle

The host-neutral lifecycle is:

1. `verify`: produce an accepted artifact verifier report and facts.
2. `plan`: combine verified facts, production-lowering support, projection,
   predicate, split, cache, concurrency, and fallback policy into a runtime plan.
3. `prepare`: optionally build or load a native backend artifact only if the plan
   decision is native.
4. `open_scan`: create immutable scan-global state for a full scan, row range,
   or split.
5. `open_worker`: create worker-local state for a serialized or parallel scan.
6. `next_batch`: produce one Arrow-compatible output batch.
7. `close_worker` / `close_scan`: release worker-local and scan-global state.
8. `destroy_plan`: release plan-owned native/cache resources.

## Trust Boundary

The runtime never creates a native plan from raw host assertions. Native planning
requires:

- accepted artifact verification status
- trusted solver state: `Discharged` or `NotRequired`
- production-lowering support for the artifact shape
- reader/lowering disposition that permits native execution
- supported projection, predicate, split, and concurrency policy

Every other state must produce interpreter fallback only when policy explicitly
allows it, or fail closed.

## Handles

The host-neutral ABI uses opaque conceptual handles:

- `RuntimePlan`
- `ScanHandle`
- `WorkerHandle`
- `BatchHandle`

The model also records handle kinds so C, C++, Rust, and future host adapters can
share one vocabulary without exposing engine-owned object types.

## Batch Output

Arrow C Data is the batch handoff boundary. It is not the whole runtime API.

The Loom runtime owns plan/scan/worker policy and may export batches through
Arrow C Data, or later through a host-native vector adapter. Batch ownership must
remain producer-owned and release-callback-driven.

## Projection and Predicate

Projection is a runtime plan input. Supported Phase 22 projection forms are:

- single-column identity projection
- table column-id projection with explicit output order

Predicate pushdown is represented by a predicate envelope. Phase 22 accepts
`none`; unsupported predicates either degrade to scan-all or fail closed,
depending on host policy.

## Splits and Concurrency

Splits are explicit runtime inputs. Phase 22 supports full-scan and row-range
split descriptors. Scan-global state is immutable; worker-local state is
separate. Multi-worker requests over non-splittable scans fail closed unless a
host policy explicitly serializes them.

## Cache Identity

Cache identity must include every safety or execution-shape input:

- runtime ABI version
- artifact digest
- verified facts fingerprint
- solver identity
- production-lowering fingerprint
- backend/toolchain/target identity
- projection and output order
- predicate envelope
- split and concurrency policy
- fallback and safety policy

The cache key is a contract model, not an ad hoc debug string.

## Diagnostics

Runtime diagnostics use stable code strings and JSON-path-like paths. They must
not depend on host-engine text. Diagnostics distinguish verifier rejection,
constraint rejection, fallback-disabled decisions, unsupported projection,
unsupported predicate, unsafe concurrency, cache mismatch, and toolchain/ABI
mismatch.

## Fallback

Fallback is policy-controlled. Runtime decisions are:

- native candidate
- interpreter fallback
- fail closed
- diagnostic-only

Native candidate is allowed only after the trust-boundary rules pass. Interpreter
fallback is allowed only when host policy permits it and the artifact semantics
are otherwise accepted.

## Non-Goals

Phase 22 does not implement:

- DuckDB native table-function execution
- StarRocks connector or executor integration
- Iceberg metadata binding
- compiled `loom.decode` dialect registration
- production `melior` pass pipeline
- LLVM lowering or JIT execution
- arbitrary Vortex semantic compatibility
- new solver backend functionality
