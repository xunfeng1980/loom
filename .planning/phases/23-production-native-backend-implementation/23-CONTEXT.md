# Phase 23 Context: Production Native Backend Implementation

## Starting Point

Phase 23 starts after Phase 22 closed the host-neutral runtime ABI and execution
policy. The backend is no longer allowed to accept arbitrary lowering inputs or
engine-specific parameters. Its mandatory inputs are:

- `RuntimePlan`, including native/interpreter/fail-closed execution decision.
- `RuntimeCacheKey`, including artifact, projection, predicate, split,
  concurrency, ABI, and capability identity.
- Accepted artifact-verifier facts and production-lowering facts from Phases 17,
  19, and 20.
- The Phase 21 coverage/lowering disposition matrix.

## Locked Decisions

- `loom-core` and `loom-ffi` remain free of MLIR, LLVM, DuckDB, and Vortex runtime
  backend dependencies.
- The public `loom_runtime.h` sketch remains explicitly unfrozen through Phase 23.
  Phase 23 may define backend-internal typed Rust/C++ helpers, but those helpers
  must not become the stable C ABI by accident.
- DuckDB integration is Phase 24. Phase 23 may add host-neutral batch/JIT evidence
  and reports, but it must not edit the DuckDB table function as the primary
  delivery.
- Cache hardening and broad native/interpreter fallback matrix expansion are Phase
  25. Phase 23 must carry cache identity, but it does not need to prove every cache
  reuse/invalidation story.
- Default workspace builds must remain usable without a local LLVM/MLIR install.
  Strict backend gates may require managed LLVM/MLIR 22.

## Existing Code to Consume

- `crates/loom-core/src/runtime_abi.rs` defines `RuntimePlan`,
  `RuntimePlanRequest`, `RuntimeCacheKey`, execution decisions, projection,
  predicate, split, concurrency, diagnostics, and ABI version model.
- `crates/loom-core/src/production_native_lowering.rs` defines the production
  lowering support gate and kernel/facts vocabulary.
- `crates/loom-core/src/decode_dialect.rs` defines the textual `loom.decode`
  surface from Phase 20. Phase 23 should move this toward compiled ODS evidence,
  not invent a second dialect surface.
- `crates/loom-native-melior/src/pipeline.rs` already owns MLIR validation and
  LLVM lowering pipeline evidence behind optional tooling.
- `crates/loom-native-melior/src/toolchain.rs` already probes strict/skip
  toolchain modes and managed LLVM/MLIR paths.

## Backend Surface Target

The Phase 23 backend should expose a host-neutral API shape inside
`loom-native-melior` such as:

- `NativeBackendRequest`: runtime plan, cache key, artifact/native facts, backend
  options, and cancellation state.
- `NativeBackendIdentity`: backend name, version, LLVM/MLIR major version,
  pipeline ID, target triple/layout where available, and supported capabilities.
- `NativeBackendArtifact`: verified MLIR/LLVM/JIT preparation evidence and the
  backend identity that produced it.
- `NativeBackendReport`: accepted/rejected/fail-closed diagnostics with stable
  codes suitable for Phase 24 host adapters.

Exact names may change to fit existing code, but the shape must preserve the
Phase 22 contract.

## Non-Goals

- No DuckDB runtime wiring.
- No public ABI freeze.
- No arbitrary Vortex semantic compatibility.
- No replacement of the interpreter.
- No claim of compiler correctness beyond focused verifier-gated equivalence for
  supported kernels.
