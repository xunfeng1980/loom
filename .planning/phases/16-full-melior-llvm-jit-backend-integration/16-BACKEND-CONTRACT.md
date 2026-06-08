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

Missing or incompatible tools fail release gates by default. Skip is permitted
only by explicit `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`, so absence of LLVM/MLIR is not
silently converted into passing evidence.

Normal release verification runs `bash scripts/melior-jit-test.sh` and requires
compatible LLVM/MLIR 22 tooling for feature-enabled evidence.

Managed native-backend evidence can be installed and checked with:

```bash
mise run external-tools
bash scripts/melior-jit-test.sh
```

Configured skip mode is explicit:

```bash
LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/melior-jit-test.sh
```

## JIT ABI Contract

The initial `execute_copy_i32_jit` ABI is deliberately narrow:

- typed primitive `i32` input and output buffers only
- no Arrow buffers and no DuckDB execution path
- row count comes from verifier `row_count_bound`
- stable entry symbol `loom_l2core_copy_i32`
- unsupported programs fail closed before ExecutionEngine creation
- short input buffers fail closed before native invocation
- missing JIT symbol reports `jit-symbol-missing`
- native/reference divergence reports `native-output-mismatch`

## Non-Goals

Phase 16 does not implement:

- a custom Loom MLIR decode dialect
- vectorization
- generated Arrow raw-buffer writes
- DuckDB native execution
- complete Vortex reader support
- arbitrary L1/L2 native kernel lowering
- a compiler correctness proof
