# Phase 16 Summary: Full melior/LLVM/JIT Backend Integration

## Shipped

- Added optional backend crate `loom-native-melior`.
- Added MLIR/LLVM toolchain probing and stable backend diagnostics.
- Added verifier-gated `build_melior_module` for the bounded Int32 copy slice.
- Added MLIR validation pipeline with skip-aware normal mode and strict fail-closed mode.
- Added JIT boundary API `execute_copy_i32_jit`.
- Added stable diagnostics for `jit-unavailable`, `jit-symbol-missing`, and `native-output-mismatch`.
- Added `scripts/melior-jit-test.sh`.
- Wired Phase 16 into `scripts/mvp0-verify.sh`.
- Updated README, README-zh, PROJECT, ROADMAP, and STATE.
- Wrote final backend report: `16-BACKEND-REPORT.md`.

## Verification

- `cargo test --workspace` - PASSED
- `cargo test -p loom-native-melior` - PASSED
- `bash scripts/check-core-invariants.sh` - PASSED
- `bash scripts/melior-jit-test.sh` - PASSED WITH SKIP
- `bash scripts/mvp0-verify.sh` - PASSED
- `git diff --check` - PASSED

## Strict JIT Evidence

Strict JIT was not marked as passed locally. It was run with:

```bash
LOOM_REQUIRE_MELIOR_JIT=1 bash scripts/melior-jit-test.sh || true
```

Result: expected fail-closed behavior because local LLVM/MLIR major is `21` and Phase 16 expects `22` for the pinned optional `melior`/`mlir-sys v220` stack.

## Deviations

- Compatible feature-enabled JIT execution was not proven on this machine.
- `cargo test -p loom-native-melior --features melior jit || true` fails before Rust compilation because `mlir-sys v220.0.2` cannot find a compatible `llvm-config`.
- The default backend path remains toolchain-independent and records optional JIT evidence as skip-aware.

## Residual Risks

- Real ExecutionEngine invocation still needs a compatible LLVM/MLIR 22 environment before Phase 17/production lowering can depend on it.
- Phase 16 does not implement a custom Loom MLIR dialect, Arrow raw-buffer writes, DuckDB native execution, or complete Vortex reader support.
- The JIT API is intentionally limited to typed primitive `i32` buffers for the bounded Int32 copy slice.

## Self-Check: PASSED

Phase 16 is complete as optional verifier-gated backend evidence. It does not overclaim production native execution, and all non-strict gates pass on the current machine.
