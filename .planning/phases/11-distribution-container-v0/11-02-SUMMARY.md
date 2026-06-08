---
phase: 11-distribution-container-v0
plan: "02"
status: complete
completed_at: "2026-06-08T02:55:00Z"
commit: 28daa5c
requirements: [DIST-01, DIST-02, DIST-03]
---

# 11-02 Summary: LMC1 Verifier, Decode, and FFI Routing

## What Changed

- Added container-aware Rust payload helpers in `crates/loom-core/src/container_codec.rs`:
  - `extract_wrapped_payload`
  - `decode_layout_payload_maybe_container`
  - `decode_table_payload_maybe_container`
- Added `verify_container` in `crates/loom-core/src/verifier.rs`.
  - Validates `LMC1` structure before payload decode.
  - Reports unsupported required features through stable `$.required_features` diagnostics.
  - Delegates wrapped `LMP1` payloads to `verify_layout`.
  - Delegates wrapped `LMT1` payloads to `verify_table`.
- Routed `loom_decode` internals through the container-aware layout helper without changing the C ABI.
- Added FFI tests proving single-column `LMC1` containers return the same Arrow arrays as raw `LMP1` payloads.
- Added negative FFI coverage for unknown required container features returning `DecodeFailed`.

## Acceptance Criteria

- [x] Valid layout containers produce `VerificationReport::is_ok()`.
- [x] Valid table containers produce `VerificationReport::is_ok()`.
- [x] Unknown required features fail before wrapped payload decode.
- [x] Container diagnostics include stable paths for feature/header/section failures.
- [x] Raw `LMP1` payload decode remains compatible.
- [x] Raw `LMT1` payload decode remains compatible.
- [x] `loom_decode` accepts supported `LMC1` single-column payloads with no ABI churn.
- [x] Malformed or unsupported containers return typed errors rather than panicking.

## Verification

- `cargo test -p loom-core container_codec`
- `cargo test -p loom-core verifier`
- `cargo test -p loom-ffi`
- `git diff --check`

## Notes

- Table containers are now supported by Rust decode and verifier helpers.
- The existing FFI surface remains single-column. Table-shaped containers are intentionally not exposed through a new C ABI in Phase 11.
- `cargo fmt` reformats an unrelated timing helper in `crates/loom-fixtures/src/bin/loom_fixture_timing.rs`; that unrelated change was reverted before commit.

