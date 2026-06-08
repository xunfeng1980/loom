# Phase 14 Plan 03 Summary

**Plan:** 14-03 — Equivalence gate and optional toolchain probe
**Status:** Complete
**Date:** 2026-06-08

## Changed Files

- `crates/loom-core/src/native_lowering.rs`
- `crates/loom-core/tests/native_lowering.rs`
- `scripts/native-lowering-test.sh`

## What Changed

- Added `execute_supported_copy_i32` as a tiny supported-slice reference helper.
- Added tests comparing the supported bounded Int32 copy slice against typed
  primitive output.
- Added a negative short-input test that fails closed through lowering
  diagnostics rather than panicking.
- Added `scripts/native-lowering-test.sh`.
- The gate runs focused Rust tests and probes `mlir-opt` only when available.

## Verification

- `rg -n "execute_supported_copy_i32|row_count_bound|Vec<i32>|LoweringSupportReport" crates/loom-core/src/native_lowering.rs`
- `cargo test -p loom-core native_lowering`
- `bash scripts/native-lowering-test.sh`
- `rg -n "mlir-opt|optional|skip|native_lowering|LOWER-04" scripts/native-lowering-test.sh`
- `git diff --check`

## Requirements

- `LOWER-03`: Closed.
- `LOWER-04`: Closed.

## Toolchain Evidence

`mlir-opt` was not installed on this machine, so optional textual MLIR validation
was explicitly skipped. This is expected for Phase 14.

## Follow-Up

Plan 14-04 wires the focused gate into the release gate and closes public docs.
