# Phase 23 Backend Contract

## Scope

Phase 23 defines the host-neutral production native backend boundary inside
`loom-native-melior`. The backend is a consumer of the Phase 22 runtime policy,
not a replacement for it and not a host-engine integration layer.

The backend may prepare MLIR, LLVM, and JIT evidence only after runtime planning
has selected native execution. It does not decide whether a host should fall back
to the interpreter; that decision belongs to `loom-core::runtime_abi`.

## Inputs

The mandatory backend inputs are:

- `RuntimePlan`: the Phase 22 plan whose decision must be
  `NativeCandidate` and whose diagnostics must be empty before backend work can
  begin.
- `RuntimeCacheKey`: the deterministic cache identity for the exact artifact,
  verifier facts, solver identity, lowering facts, backend identity, projection,
  predicate, split, fallback policy, and concurrency shape.
- `ProductionLoweringFacts`: the Phase 20+ production lowering facts that prove
  the artifact has a supported Loom decode dialect shape.
- Backend options and cancellation state.

The backend must reject missing cache identity, interpreter/fail-closed runtime
plans, unsupported lowering facts, and cancelled requests before ODS, MLIR, LLVM,
or JIT work begins.

## Backend Identity

Every accepted backend request and backend artifact records:

- Loom runtime ABI version.
- Backend name and backend version.
- Expected LLVM/MLIR major version.
- Detected LLVM/MLIR version when probing has run.
- Toolchain compatibility state.
- Pass pipeline identity.
- Target triple and data layout when available.
- Backend capabilities, including ODS manifest, LLVM lowering, JIT, and supported
  native kernel names.

This identity is part of cache safety. It is not debug text.

## Cancellation

Phase 23 models cancellation before long-running native work exists. A cancelled
request must return a cancellation diagnostic and must not produce an accepted
backend request or executable artifact.

The first implementation may check cancellation only at preflight and before
later preparation/execution steps, but the request/report model must carry the
state so Phase 24 host adapters can map interrupts cleanly.

## Cache Identity

The runtime cache key remains the canonical cache contract. Backend reports must
carry the cache key they consumed so Phase 24 can attribute host execution to the
same identity and Phase 25 can harden reuse/invalidation.

Backend-local cache keys or filenames may exist later, but they must be derived
from `RuntimeCacheKey` plus backend artifact identity. They must not omit ABI,
toolchain, target/layout, pipeline, projection, predicate, split, or policy
inputs.

## Diagnostics

Backend diagnostics use stable code strings and JSON-path-like paths. They must
distinguish at least:

- runtime plan is not native
- missing runtime cache key
- missing lowering facts
- unsupported lowering facts
- cancellation
- toolchain skipped/missing/mismatched
- backend preparation failure

Diagnostics must not depend on DuckDB text, host exception types, or MLIR tool
stderr as their only stable signal.

## Lifecycle

The Phase 23 backend lifecycle is:

1. `validate_request`: accept only a native-candidate `RuntimePlan`,
   `RuntimeCacheKey`, supported production lowering facts, backend identity, and
   non-cancelled state.
2. `prepare_mlir`: later Phase 23 plans validate ODS/MLIR evidence for the
   accepted request.
3. `lower_llvm`: later Phase 23 plans produce LLVM lowering evidence and record
   pipeline identity.
4. `prepare_jit`: later Phase 23 plans prepare optional JIT execution for
   supported kernels only.
5. `execute_native`: later Phase 23/24 work may execute prepared kernels and
   compare output against interpreter/reference evidence.

Only step 1 is required for 23-01.

## Natural Wrappers

Rust and C++ natural wrappers are allowed for tests and ergonomics. They may use
RAII, `Result`, typed builders, and iterator-style APIs over backend reports.

They are not the stable ABI. The public `loom_runtime.h` sketch remains
explicitly unfrozen through Phase 23, and no wrapper may add trust, skip runtime
planning, or define extra semantics unavailable to other hosts.

## Non-Goals

Phase 23 backend work does not implement:

- DuckDB native execution integration.
- A frozen public `loom_runtime.h` ABI.
- Persistent native cache hardening.
- Iceberg or StarRocks integration.
- Arbitrary Vortex semantic compatibility.
- A compiler correctness proof beyond focused verifier-gated equivalence for the
  supported kernel slice.
