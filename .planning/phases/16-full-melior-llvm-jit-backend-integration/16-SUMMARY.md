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
- `bash scripts/melior-jit-test.sh` - PASSED after local LLVM/MLIR 22 upgrade
- `bash scripts/mvp0-verify.sh` - PASSED
- `git diff --check` - PASSED

## Strict JIT Evidence

Strict JIT now passes locally after upgrading Homebrew LLVM/MLIR to `22.1.7`. It was run with:

```bash
LOOM_REQUIRE_MELIOR_JIT=1 bash scripts/melior-jit-test.sh
```

Result: PASSED.

## Deviations

- Feature-enabled JIT execution is now proven on this machine with Homebrew LLVM/MLIR `22.1.7`.
- `scripts/melior-jit-test.sh` injects the detected Homebrew LLVM bin directory into `PATH` before compiling feature-enabled `melior` tests, because Homebrew LLVM is keg-only.
- The default backend path remains toolchain-independent; incompatible machines still record optional JIT evidence as skip-aware unless strict mode is requested.

## Residual Risks

- Real ExecutionEngine evidence now exists locally for the bounded Int32 copy slice, but later phases still need a production ABI, wider lowering surface, and host-engine integration before depending on native execution.
- Phase 16 does not implement a custom Loom MLIR dialect, Arrow raw-buffer writes, DuckDB native execution, or complete Vortex reader support.
- The JIT API is intentionally limited to typed primitive `i32` buffers for the bounded Int32 copy slice.

## Self-Check: PASSED

Phase 16 is complete as optional verifier-gated backend evidence. It does not overclaim production native execution, and normal plus strict local gates now pass on the current machine.
