# Phase 14 Plan 02 Summary

**Plan:** 14-02 — Textual MLIR emission
**Status:** Complete
**Date:** 2026-06-08

## Changed Files

- `crates/loom-core/src/native_lowering.rs`
- `crates/loom-core/tests/native_lowering.rs`
- `.planning/phases/14-mlir-native-lowering-spike/14-LOWERING-CONTRACT.md`

## What Changed

- Added `LoweringBackend`, `LoweringArtifact`, and `lower_to_textual_mlir`.
- Emitted deterministic textual MLIR for the bounded Int32 copy slice with
  `func`, `arith`, `scf`, and `memref` operations.
- Added artifact metadata for backend, entry symbol, facts linkage, and row
  count.
- Added inline snapshot tests for MLIR text and a negative test proving
  unsupported programs do not produce partial MLIR.
- Updated the lowering contract with the emitted dialect stack and scope limits.

## Verification

- `rg -n "LoweringArtifact|lower_to_textual_mlir|func.func @loom_l2core_copy_i32|scf.for|memref.load|memref.store" crates/loom-core/src/native_lowering.rs`
- `cargo test -p loom-core native_lowering`
- `rg -n "func|arith|scf|memref|LLVM lowering|custom Loom dialect|textual artifact" .planning/phases/14-mlir-native-lowering-spike/14-LOWERING-CONTRACT.md`
- `git diff --check`

## Requirements

- `LOWER-02`: Closed.

## Follow-Up

Plan 14-03 adds typed primitive equivalence evidence and an optional MLIR
toolchain gate.
