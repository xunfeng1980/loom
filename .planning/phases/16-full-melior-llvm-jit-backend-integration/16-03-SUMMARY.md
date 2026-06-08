# Phase 16-03 Summary: MLIR Validation Pipeline and Skip-Aware Gate

## Result

Implemented the Phase 16 validation gate around the verifier-gated melior backend boundary:

- Added `loom_native_melior::pipeline`.
- Added `validate_with_mlir_opt` and `validate_translation_to_llvm_ir`.
- Added deterministic malformed-MLIR rejection before any external toolchain is required.
- Added skip-aware toolchain handling for normal validation and fail-closed strict handling for `LOOM_REQUIRE_MELIOR_JIT=1`.
- Added `scripts/melior-jit-test.sh`.
- Wired the Phase 16 gate into `scripts/mvp0-verify.sh` after Phase 15 and before DuckDB smoke.
- Updated `16-BACKEND-CONTRACT.md` with the normal-vs-strict gate contract.

## Verification

- `cargo fmt -p loom-native-melior` - PASSED
- `cargo test -p loom-native-melior pipeline` - PASSED
- `bash scripts/melior-jit-test.sh` - PASSED WITH SKIP
- `LOOM_REQUIRE_MELIOR_JIT=1 bash scripts/melior-jit-test.sh || true` - produced expected `[FAIL]` on local LLVM/MLIR major 21 vs expected 22
- `bash scripts/mvp0-verify.sh` - PASSED
- `git diff --check` - PASSED
- `rg -n "Phase 16|melior-jit-test|LOOM_REQUIRE_MELIOR_JIT|\\[SKIP\\]" ...` - PASSED

## Local Toolchain Evidence

Current machine has Homebrew LLVM/MLIR `21.1.2`; Phase 16 expects MLIR major `22` because the pinned optional `melior` crate pulls `mlir-sys v220`. Therefore:

- normal gate records `[SKIP] detected LLVM/MLIR major 21, expected 22`
- strict gate fails closed with `[FAIL] detected LLVM/MLIR major 21, expected 22`

## Self-Check

PASSED: Phase 16 backend validation is now visible in the release gate without making optional JIT evidence mandatory on machines lacking compatible MLIR/LLVM.
