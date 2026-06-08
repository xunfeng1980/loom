# Phase 16-04 Summary: JIT Boundary and Equivalence Diagnostics

## Result

Implemented the Phase 16 JIT execution boundary for the verified bounded Int32 copy slice:

- Added `loom_native_melior::jit`.
- Added `execute_copy_i32_jit(&L2CoreProgram, &FullVerificationReport, &[i32])`.
- Preserved verifier-report ownership; standalone facts are not accepted.
- Reused Phase 14 `execute_supported_copy_i32` as the Rust reference output.
- Added typed primitive ABI documentation: `i32` buffers only, no Arrow buffers, no DuckDB execution path.
- Added stable diagnostics for `jit-unavailable`, `jit-symbol-missing`, and `native-output-mismatch`.
- Updated `scripts/melior-jit-test.sh` to run default JIT boundary tests and feature-enabled JIT equivalence only when compatible tooling exists.

## Verification

- `cargo fmt -p loom-native-melior` - PASSED
- `cargo test -p loom-native-melior jit` - PASSED
- `cargo test -p loom-native-melior` - PASSED
- `cargo test -p loom-core native_lowering` - PASSED
- `bash scripts/melior-jit-test.sh` - PASSED WITH SKIP
- `LOOM_REQUIRE_MELIOR_JIT=1 bash scripts/melior-jit-test.sh || true` - produced expected `[FAIL]` on local LLVM/MLIR major 21 vs expected 22
- `cargo test -p loom-native-melior --features melior jit || true` - failed before Rust compilation because local `llvm-config` is not available for `mlir-sys v220.0.2`
- `bash scripts/mvp0-verify.sh` - PASSED
- `git diff --check` - PASSED

## Local Toolchain Evidence

Current local MLIR/LLVM remains incompatible with the pinned optional melior stack:

- detected Homebrew LLVM/MLIR major: `21`
- expected MLIR major: `22`
- feature-enabled `mlir-sys v220.0.2` build asks for `llvm-config`

The normal gate records this as skip-aware optional evidence. Strict mode fails closed.

## Self-Check

PASSED: JIT execution is optional evidence, unsupported programs reject before backend invocation, and native output equivalence has stable diagnostics for future compatible ExecutionEngine runs.
