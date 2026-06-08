# Phase 16 Backend Report

## Scope

Phase 16 promotes the Phase 14 textual MLIR spike into an optional backend boundary for one verified slice only: the bounded Int32 copy program accepted by `verify_l2_core` and Phase 14 `check_lowering_support`.

The shipped backend is evidence, not a production native compiler. It preserves the verifier handoff, records MLIR/LLVM toolchain facts, emits deterministic backend artifacts, validates MLIR shape/tooling where possible, and exposes JIT boundary diagnostics.

## Local toolchain facts

- Expected MLIR major: `22`
- Local Homebrew LLVM/MLIR major observed by the gate: `21`
- Optional `melior` crate version: `0.27.0`
- Transitive MLIR binding expectation: `mlir-sys v220.0.2`
- Normal gate behavior: `[SKIP] detected LLVM/MLIR major 21, expected 22`
- Strict gate behavior: `[FAIL] detected LLVM/MLIR major 21, expected 22`
- Feature-enabled build skip reason: `mlir-sys v220.0.2` / `tblgen` require a compatible `llvm-config`

## Backend crate boundary

Phase 16 adds `crates/loom-native-melior` as the only owner of backend integration code.

- `loom-core` remains free of `melior`, `mlir`, and `llvm` dependencies.
- `loom-ffi` remains free of `melior`, `mlir`, and `llvm` dependencies.
- The `melior` dependency is optional and outside the default workspace test path.

## Verifier-gated artifact flow

The artifact flow is deliberately narrow:

1. `verify_l2_core(program)` must accept.
2. The same `FullVerificationReport` must expose `VerifiedArtifactFacts`.
3. `check_lowering_support(program, report)` must accept the bounded Int32 copy shape.
4. `build_melior_module(program, report)` may create a `MeliorModuleArtifact`.
5. `validate_with_mlir_opt` / `validate_translation_to_llvm_ir` may validate the emitted MLIR if compatible tools exist.
6. `execute_copy_i32_jit` may attempt JIT evidence only after verifier/support/reference-output checks pass.

Unsupported programs fail closed before native artifact creation.

## Programmatic MLIR evidence

`loom_native_melior::builder::build_melior_module` returns a deterministic artifact with:

- entry symbol: `loom_l2core_copy_i32`
- backend summary: `backend=melior-programmatic`
- facts linkage from the accepted verifier report
- row count from `VerifiedArtifactFacts.row_count_bound`
- MLIR text equivalent to the Phase 14 bounded Int32 copy module

## MLIR validation evidence

`loom_native_melior::pipeline` provides:

- `validate_with_mlir_opt`
- `validate_translation_to_llvm_ir`
- malformed-MLIR rejection before external tools are required
- strict-mode diagnostics for missing or incompatible MLIR/LLVM tools

Malformed MLIR returns `mlir-verification-failed` or `pass-pipeline-failed` depending on where validation fails.

## JIT evidence and skip reason

`loom_native_melior::jit::execute_copy_i32_jit` defines the typed primitive JIT boundary:

- `&L2CoreProgram`
- `&FullVerificationReport`
- `&[i32]`
- `Result<Vec<i32>, MeliorBackendReport>`

The local machine does not execute feature-enabled JIT because the available LLVM/MLIR major is `21` while Phase 16 expects `22`. This is recorded as optional evidence skip in normal mode and as fail-closed strict evidence in `LOOM_REQUIRE_MELIOR_JIT=1` mode.

## Fail-closed negative coverage

Coverage includes:

- verifier-rejected programs
- missing verifier facts
- optional features
- `CursorLoop`
- `AppendNull`
- scratch capabilities
- unsupported expression shapes
- short typed primitive input buffers
- malformed MLIR
- `jit-symbol-missing`
- `native-output-mismatch`
- `jit-unavailable`

## Commands run

- `cargo fmt -p loom-native-melior`
- `cargo test -p loom-native-melior builder`
- `cargo test -p loom-native-melior pipeline`
- `cargo test -p loom-native-melior jit`
- `cargo test -p loom-native-melior`
- `cargo test -p loom-core native_lowering`
- `cargo test -p loom-native-melior --features melior jit || true`
- `LOOM_REQUIRE_MELIOR_JIT=1 bash scripts/melior-jit-test.sh || true`
- `bash scripts/melior-jit-test.sh`
- `bash scripts/mvp0-verify.sh`
- `git diff --check`

Final closeout additionally reruns:

- `cargo test --workspace`
- `bash scripts/check-core-invariants.sh`

## Non-goals

Phase 16 non-goals:

- no custom Loom MLIR decode dialect
- no vectorization
- no Arrow raw-buffer native writes
- no DuckDB native execution
- no host-engine native execution
- no complete Vortex reader support
- no arbitrary L1/L2 native kernel lowering
- no compiler correctness proof

Phase 17 and later phases own the production native path.
