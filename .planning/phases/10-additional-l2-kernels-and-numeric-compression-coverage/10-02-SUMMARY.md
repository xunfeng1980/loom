---
phase: 10-additional-l2-kernels-and-numeric-compression-coverage
plan: "02"
subsystem: loom-fixtures
tags: [alp, fixtures, ffi, vortex-oracle]
requirements_completed: []
completed: 2026-06-08
commit: ed9570f
---

# Phase 10-02: ALP Fixture and FFI Summary

Phase 10-02 added deterministic ALP Float32/Float64 fixture coverage and proved float payloads cross the Rust FFI boundary.

## Accomplishments

- Added Vortex primitive Float32 and Float64 oracle helpers that return values plus true-for-null flags.
- Added ALP Float32/Float64 synthetic known-value tests covering decimals, negatives, zero, repeats, and nulls.
- Documented the oracle boundary: Vortex 0.74.0 has no exposed ALP array API here, so Vortex primitive arrays provide row-value truth while Loom owns ALP params.
- Extended the fixture bridge and descriptor roundtrip coverage for non-null raw Float32/Float64 primitive arrays.
- Added FFI roundtrip and Arrow buffer-layout tests for ALP Float32/Float64 payloads.

## Verification

- `cargo test -p loom-fixtures oracle` - PASS.
- `cargo test -p loom-fixtures alp` - PASS.
- `cargo test -p loom-fixtures descriptor_roundtrips_raw_float_samples` - PASS.
- `cargo test -p loom-ffi roundtrip` - PASS.
- `cargo test -p loom-ffi buffer_layout` - PASS.
- `rg -n 'vortex_file|vortex-file|\.vortex|VortexFile|from_path|read_file' crates/loom-fixtures` - PASS, no matches.

## Notes

- Exact equality is used for selected finite representable decimal fixtures; no tolerance helper was needed.
