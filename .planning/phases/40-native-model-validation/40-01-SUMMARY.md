# Phase 40-01 Summary: Native Model Trace Check

**Status:** Complete
**Date:** 2026-06-09
**Plan:** `40-01-PLAN.md`

## What Changed

- Extended Rust L2Core and Lean modeled scalar vocabulary with Float32/Float64
  bit-pattern constants and scalar type tags.
- Extended the Rust full verifier, Phase 39 reference executor, and
  observer-only production trace subject so Float32/Float64 builder events have
  stable trace names.
- Added float cases to the Lean/Rust correspondence corpus:
  `fuzz-003-float32-builder` and `fuzz-004-float64-nullable-builder`.
- Added `NativeArrowSemanticModelValidationReport` in
  `loom_core::native_arrow_semantic`.
- Built the model side by constructing a narrow L2Core append/null program from
  supported Arrow semantic batches and running
  `l2_core_reference_executor::execute_reference`.
- Built the native side by tracing the native output `RecordBatch` into the same
  append-value / append-null / terminal vocabulary.
- Validation now requires both exact model/native trace equality and the Phase
  35 decoded-reference value equality.
- Added positive default `LMC2(LMA1)` and direct `LMA1` validation tests across
  nullable Boolean, Int32, Int64, Float32, and Float64.
- Added injected trace-divergence coverage with stable
  `native-model-trace-mismatch` diagnostics.

## Verification

All checks passed:

```sh
lean formal/lean/LoomCore.lean
cargo test -p loom-core --test l2_core_reference_executor
cargo test -p loom-core --test native_arrow_semantic
bash scripts/lean-rust-correspondence-test.sh
git diff --check
```

## Evidence

- `verify_native_arrow_semantic_model` validates the default wrapped
  `LMC2(LMA1)` supported matrix.
- `verify_native_arrow_semantic_model_output` supports injected divergence tests
  without mutating production native execution.
- Reference trace and native trace are both exposed for diagnostics, making
  divergence auditable.

## Non-Claims

- This plan does not yet make runtime/cache eligibility depend on successful
  native/model validation; that is 40-02.
- This is not verified compilation of MLIR/LLVM.
- This does not add Utf8, logical, nested, multi-batch, new format, or DuckDB
  native-route coverage.

Self-Check: PASSED
