# Phase 22 Runtime ABI Report

## Delivered

Phase 22 defines the first executable host-neutral runtime ABI and execution
policy model for Loom artifacts.

Delivered artifacts:

- `22-RUNTIME-ABI-CONTRACT.md`
- `loom_core::runtime_abi`
- focused runtime ABI, execution-policy, scan-planning, and cache-key tests
- `crates/loom-ffi/include/loom_runtime.h` as a non-frozen C ABI sketch
- `scripts/runtime-abi-test.sh`, wired into `scripts/mvp0-verify.sh`

## Runtime Contract

The runtime lifecycle is now documented as verify, plan, prepare, open scan,
open worker, produce next batch, close worker/scan, and destroy plan.

The typed model exposes:

- `RuntimeAbiVersion`
- `RuntimePlanRequest`
- `RuntimePlan`
- `RuntimeHandleKind`
- `RuntimeExecutionDecision`
- `RuntimeFallbackPolicy`
- `RuntimeSafetyPolicy`
- `RuntimeDiagnostic`

The contract keeps Arrow C Data as the batch handoff boundary, but not as the
whole runtime API.

## Decision Policy

`decide_runtime_execution` deterministically chooses:

- `native-candidate`
- `interpreter-fallback`
- `fail-closed`
- `diagnostic-only`

Native requires accepted artifact status, discharged or not-required
constraints, accepted reader support, emitted artifact disposition, supported
projection/predicate/split/concurrency shape, and production-lowering support.

Collected-only, failed, unknown, skipped, rejected, unsupported, missing, or
deferred states never choose native.

## Projection Predicate Split Policy

Phase 22 supports:

- all-column projection
- explicit table column-id projection with output remapping
- `PredicateEnvelope::None`
- unsupported predicate handling by either scan-all or fail-closed policy
- full-scan and row-range split descriptors
- single-worker, serialized shared scan, and parallel split policy modeling

Unsafe multi-worker requests over non-splittable scans fail closed.

## Cache and Diagnostics

`RuntimeCacheKey` uses a deterministic stable FNV-1a style key over:

- runtime ABI version
- artifact digest
- facts fingerprint
- solver identity
- production-lowering fingerprint
- backend/toolchain/target identity
- projection
- predicate
- split
- fallback, predicate, and concurrency policy

Runtime diagnostics expose stable code strings and JSON-path-like paths.

## Release Gate

Focused gate:

```bash
bash scripts/runtime-abi-test.sh
```

Main gate integration:

```bash
bash scripts/mvp0-verify.sh
```

Focused tests:

- `cargo test -p loom-core --test runtime_abi_contract`
- `cargo test -p loom-core --test runtime_execution_policy`
- `cargo test -p loom-core --test runtime_scan_planning`
- `cargo test -p loom-core --test runtime_cache_key`

## Non-Claims

Phase 22 does not implement DuckDB native execution, StarRocks integration,
Iceberg binding, compiled ODS dialects, a production `melior` pass pipeline,
LLVM lowering, JIT execution, arbitrary Vortex semantic compatibility, or a new
solver backend.

`loom_runtime.h` is a contract sketch, not a frozen ABI.

## Handoff

Phase 23 must consume the runtime plan/cache/diagnostic model when implementing
the production backend.

Phase 24 must adapt DuckDB to the runtime contract instead of inventing
artifact identity, fallback, projection, cache, and output ownership inside the
DuckDB table-function implementation.

Phase 26 should bind table metadata to artifact identity and verified facts that
can feed this runtime contract.

Phase 27 should validate whether the same runtime contract works across DuckDB
and StarRocks query surfaces.
