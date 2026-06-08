# Phase 16-02 Summary: Verifier-Gated Melior Builder Boundary

## Result

Implemented the first native-backend artifact builder boundary in `loom-native-melior`:

- Added `loom_native_melior::builder`.
- Added `build_melior_module(&L2CoreProgram, &FullVerificationReport)`.
- Added `MeliorModuleArtifact` with entry symbol, deterministic MLIR text, facts linkage, row count, and artifact summary.
- Preserved the verifier-report-to-artifact chain: standalone facts are not accepted.
- Reused Phase 14 `check_lowering_support` and `lower_to_textual_mlir` so unsupported accepted programs fail closed before artifact creation.
- Added external negative builder coverage for verifier rejection, missing facts, optional features, `CursorLoop`, `AppendNull`, scratch capabilities, and unsupported expression shape.

## Verification

- `cargo fmt -p loom-native-melior` - PASSED
- `cargo test -p loom-native-melior builder` - PASSED
- `cargo test -p loom-core native_lowering` - PASSED
- `cargo test --workspace` - PASSED
- `git diff --check` - PASSED
- `rg -n "build_melior_module|check_lowering_support|FullVerificationReport|MeliorModuleArtifact" crates/loom-native-melior/src` - PASSED

## Toolchain Note

Default builds remain independent of MLIR/LLVM. A direct `cargo test -p loom-native-melior --features melior builder` currently fails before Rust code compilation on this machine because `mlir-sys v220.0.2` cannot find a compatible LLVM/MLIR 22 `llvm-config`. Phase 16-03 will convert that environment condition into an explicit skip-aware backend gate and strict-mode failure.

## Self-Check

PASSED: The bounded Int32 copy builder only emits an artifact after `verify_l2_core` acceptance and Phase 14 lowering support, while unsupported programs return stable backend diagnostics.
