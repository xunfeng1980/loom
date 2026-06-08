# 12-02 Summary: Safety Contract Test Coverage

**Status:** Complete
**Date:** 2026-06-08
**Commit:** `b390355 test(12-02): add safety contract coverage`

## Completed

- Added `crates/loom-core/tests/safety_contract.rs` with focused no-panic/fail-closed coverage for malformed containers, raw payload parse failures, verifier diagnostics, decode-before-Arrow behavior, and table row-count mismatch.
- Added `crates/loom-ffi/tests/ffi_contract.rs` covering malformed `LMP1`, malformed `LMC1`, and panic-sentinel FFI behavior.
- Updated `12-PROOF-OBLIGATIONS.md` to reference the new executable evidence.

## Verification

- `cargo test -p loom-core --test safety_contract` — pass
- `cargo test -p loom-ffi ffi_contract` — pass
- `cargo test -p loom-core verifier` — pass
- `cargo test -p loom-core container_codec` — pass
- `rg -n "safety_contract|ffi_contract|OBL-12-0" .planning/phases/12-formal-verifier-safety-proof-mvp/12-PROOF-OBLIGATIONS.md` — pass
- `git diff --check` — pass

## Deviations from Plan

- The planned command `cargo test -p loom-core verifier container_codec` is not valid Cargo syntax because `cargo test` accepts only one test filter before `--`. Executed the equivalent checks as `cargo test -p loom-core verifier` and `cargo test -p loom-core container_codec`.

## Self-Check: PASSED

