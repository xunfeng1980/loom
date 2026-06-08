# Phase 16 Backend Contract

Phase 16 adds an optional verifier-gated `melior`/LLVM/JIT backend boundary for
the Phase 14 bounded Int32 copy slice. It is not a production dialect, not a
native-speed claim, and not a replacement for the interpreter or DuckDB path.

## Required Preconditions

- The input program must be accepted by `verify_l2_core`.
- The same `FullVerificationReport` must expose `VerifiedArtifactFacts`.
- The program must pass the Phase 14 `check_lowering_support` predicate.
- The supported shape is only bounded Int32 copy with feature
  `l2core.copy.v0`.
- Unsupported programs fail-closed before MLIR, LLVM, or JIT artifact creation.

## Dependency Boundary

- `loom-core` and `loom-ffi` must remain free of `melior`, `mlir`, and `llvm`
  dependencies.
- `crates/loom-native-melior` is the only crate that may own Phase 16 backend
  integration code.
- The `melior` dependency is optional and must not be required for default
  `cargo test --workspace`.

## Toolchain Contract

The backend records Loom-owned `MlirToolchainFacts` for:

- `llvm-config`
- `mlir-opt`
- `mlir-translate`
- `lli`
- detected LLVM/MLIR major version
- expected MLIR major version

Missing or incompatible tools are skip-aware in normal gates and fail-closed
when strict JIT evidence is required.

## Non-Goals

Phase 16 does not implement:

- a custom Loom MLIR decode dialect
- vectorization
- generated Arrow raw-buffer writes
- DuckDB native execution
- complete Vortex reader support
- arbitrary L1/L2 native kernel lowering
- a compiler correctness proof
