---
phase: 09-verifier-and-safety-boundary-mvp
plan: "02"
subsystem: decode-boundary
tags: [decode, ffi, fail-closed]
requirements_completed: [SAFE-03]
completed: 2026-06-08
---

# Phase 09-02: Decode and FFI Verifier Routing Summary

Phase 09-02 routed verifier checks through the public decode paths before Arrow output.

## Accomplishments

- Added `LoomDecodeError::VerifierFailed` for first-diagnostic decode failures.
- Updated `decode_layout_to_array_data` to call `verify_layout`.
- Updated `decode_table_to_array_data` to call `verify_table`.
- Updated `loom_decode_inner` to parse, verify, and then decode non-empty `LMP1` payloads.
- Added FFI regression coverage proving verifier-rejected payloads return `LoomError::DecodeFailed`.
- Preserved existing decode-time typed checks for deeper data-dependent invariants.

## Verification

- `cargo test -p loom-core` - PASS.
- `cargo test -p loom-ffi malformed_verified_payload_returns_decode_failed` - PASS.
- `cargo test -p loom-core for_over_raw_applies_reference_and_preserves_nulls` - PASS.
