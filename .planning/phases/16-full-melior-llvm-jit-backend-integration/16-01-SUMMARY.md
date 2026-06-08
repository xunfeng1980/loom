# 16-01 Summary

Implemented the Phase 16 optional backend boundary and toolchain contract.

Changed:

- Added `crates/loom-native-melior` as a workspace member with default features that do not require `melior` or installed MLIR/LLVM.
- Added Loom-owned backend report, diagnostic, toolchain fact, and tool status types.
- Added MLIR/LLVM toolchain probing with `EXPECTED_MLIR_MAJOR = 22`.
- Added `16-BACKEND-CONTRACT.md`.
- Extended `scripts/check-core-invariants.sh` to guard `loom-core` and `loom-ffi` against `melior|mlir|llvm` dependency leakage.

Verification:

- `cargo test -p loom-native-melior`
- `cargo test --workspace`
- `bash scripts/check-core-invariants.sh`
- `cargo tree -p loom-core | rg -i "melior|mlir|llvm" || true`
- `cargo tree -p loom-ffi | rg -i "melior|mlir|llvm" || true`
- `rg -n "optional|verify_l2_core|VerifiedArtifactFacts|fail-closed|bounded Int32 copy|not a production dialect" .planning/phases/16-full-melior-llvm-jit-backend-integration/16-BACKEND-CONTRACT.md`

Deviations:

- None - plan executed exactly as written.

Self-Check: PASSED
