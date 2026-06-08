# Phase 14 Plan 01 Summary

**Plan:** 14-01 — Lowering contract and support predicate
**Status:** Complete
**Date:** 2026-06-08

## Changed Files

- `.planning/phases/14-mlir-native-lowering-spike/14-LOWERING-CONTRACT.md`
- `crates/loom-core/src/native_lowering.rs`
- `crates/loom-core/src/lib.rs`
- `crates/loom-core/tests/native_lowering.rs`

## What Changed

- Added the Phase 14 lowering contract with verify-before-lower preconditions,
  supported subset, rejected shapes, Arrow boundary, diagnostics, optional
  toolchain evidence, and non-goals.
- Added `loom_core::native_lowering`.
- Added `check_lowering_support` and stable `LoweringDiagnosticCode` values.
- Added focused tests for accepted bounded Int32 copy and fail-closed rejection
  of verifier-rejected programs, missing facts, cursor loops, append-null,
  non-Int32 output, scratch capabilities, and unsupported expressions.

## Verification

- `rg -n "Scope|Lowering Preconditions|Supported Subset|Rejected Shapes|Arrow Boundary|Diagnostics|Optional Toolchain Evidence|Non-Goals" .planning/phases/14-mlir-native-lowering-spike/14-LOWERING-CONTRACT.md`
- `rg -n "pub mod native_lowering|LoweringSupportReport|LoweringDiagnosticCode|MissingVerifierFacts|UnsupportedLoopShape" crates/loom-core/src/lib.rs crates/loom-core/src/native_lowering.rs`
- `cargo test -p loom-core native_lowering`
- `git diff --check`

## Requirements

- `LOWER-01`: Closed.
- `VERIFIER-10`: Consumed as the lowering-precondition handoff.

## Follow-Up

Plan 14-02 emits textual MLIR only after this support predicate accepts.
