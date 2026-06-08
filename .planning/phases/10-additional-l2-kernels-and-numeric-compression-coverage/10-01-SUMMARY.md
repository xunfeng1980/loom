---
phase: 10-additional-l2-kernels-and-numeric-compression-coverage
plan: "01"
subsystem: loom-core
tags: [alp, l2-kernel, float, verifier]
requirements_completed: []
completed: 2026-06-08
commit: 7828c27
---

# Phase 10-01: ALP Core Kernel Summary

Phase 10-01 added the core Float32/Float64 and ALP L2 kernel foundation for COV-01.

## Accomplishments

- Added append-only core support for `Float32` and `Float64` descriptors, layout codec tags, raw decode, materialization, and output builders.
- Added stable checked `AlpParams` binary encoding with magic/version/type/count/exponent/mantissa/validity validation.
- Registered ALP as L2 kernel id `1` while preserving FSST at id `0`.
- Implemented ALP Float32/Float64 decode from integer mantissas plus decimal exponent, including null preservation and non-finite value rejection.
- Extended verifier coverage so FSST is restricted to UTF-8, ALP is restricted to Float32/Float64, malformed params are rejected, and ALP output type must match the layout data type.

## Verification

- `cargo test -p loom-core` - PASS, 98 tests.
- `test "$(cargo tree -p loom-core | awk '/vortex|fastlanes/{c++} END{print c+0}')" = "0"` - PASS.
- `git diff --check` - PASS.

## Notes

- Initial focused `cargo test` invocation used multiple filters, which Cargo rejected; the full `loom-core` suite was run instead.
- `cargo fmt` temporarily touched an unrelated fixtures timing helper; that formatting-only diff was reverted before the production commit.
