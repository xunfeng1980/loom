---
phase: 05-fsst-l2-kernel-and-full-verification
plan: "03"
subsystem: ffi
tags: [rust, ffi, arrow, layout-codec, utf8]
requires:
  - phase: 05-fsst-l2-kernel-and-full-verification
    plan: "01"
    provides: real FSST L2 kernel and Utf8 output
provides:
  - MVP0 layout payload codec
  - non-empty loom_decode input byte decoding
  - Utf8 Arrow C Data Interface buffer layout coverage
affects: [loom-core, loom-ffi, phase-05-wave-3]
tech-stack:
  added: []
  patterns: [checked recursive payload parser, FFI payload dispatch, Arrow C buffer layout pinning]
key-files:
  created:
    - crates/loom-core/src/layout_codec.rs
  modified:
    - crates/loom-core/src/lib.rs
    - crates/loom-core/src/error.rs
    - crates/loom-ffi/src/ffi.rs
    - crates/loom-ffi/tests/roundtrip.rs
    - crates/loom-ffi/tests/buffer_layout.rs
key-decisions:
  - "Empty input to loom_decode preserves the legacy [1,2,3,NULL] smoke fixture."
  - "Non-empty input is parsed as a checked MVP0 LMP1 layout payload and decoded through loom-core."
  - "Utf8 FFI output must expose validity, offsets, and data buffers for DuckDB consumption."
patterns-established:
  - "layout_codec encodes recursive LayoutNode trees without adding Vortex dependencies."
  - "FFI tests build payloads in Rust and import Arrow C Data Interface output with from_ffi."
requirements-completed: [VERIFY-02, VERIFY-03]
duration: 5min
completed: 2026-06-08
---

# Phase 05-03: Layout Payload Codec and FFI Wiring Summary

**`loom_decode` now decodes non-empty MVP0 layout payload bytes through loom-core while keeping the old empty-input smoke fixture intact.**

## Performance

- **Duration:** 5 min
- **Completed:** 2026-06-08T00:05:12Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Added `layout_codec.rs` with deterministic `LMP1` encode/decode for Boolean, Int32, Int64, Utf8, and the supported `LayoutNode` variants.
- Added malformed layout payload errors and parser tests for Raw, KernelEscape, and truncation.
- Wired `loom_decode_inner` so empty input returns the legacy Int32 smoke array and non-empty input decodes a payload with `L2KernelRegistry::default_for_mvp0()`.
- Added FFI roundtrip tests for Raw Int32 payloads and Utf8 FSST KernelEscape payloads.
- Added raw FFI buffer layout coverage for nullable Utf8 arrays: validity, offsets, and data buffers.

## Task Commits

1. **Task 1: Layout payload codec** - `0d2e46c` (feat)
2. **Task 2: FFI payload decode** - `7bedc93` (feat)
3. **Task 3: Utf8 FFI buffer layout** - `0da0927` (test)

**Plan metadata:** this summary commit.

## Files Created/Modified

- `crates/loom-core/src/layout_codec.rs` - MVP0 payload encoder/decoder and tests.
- `crates/loom-core/src/lib.rs` - exposes `layout_codec`.
- `crates/loom-core/src/error.rs` - layout payload typed error.
- `crates/loom-ffi/src/ffi.rs` - non-empty input decode path.
- `crates/loom-ffi/tests/roundtrip.rs` - payload roundtrip and renamed acceptance tests.
- `crates/loom-ffi/tests/buffer_layout.rs` - Utf8 raw Arrow C buffer layout test.

## Decisions Made

- `LMP1` is intentionally an MVP0 fixture/FFI format, not a stable public storage format.
- The FFI boundary maps codec, core decode, and Arrow export failures to `LoomError::DecodeFailed`; panic safety stays at the outer `catch_unwind`.
- Tests construct FSST payloads with escape-coded bytes and no symbol table for deterministic Utf8 FFI fixtures.

## Deviations from Plan

None.

## Issues Encountered

- `loom-ffi` tests use `arrow::datatypes::DataType` rather than adding a direct `arrow-schema` dependency.

## Verification

- `cargo test -p loom-core layout_codec` - PASS, 3 tests.
- `cargo test -p loom-ffi` - PASS, unit tests plus roundtrip and buffer layout integration tests.
- `cargo test --workspace` - PASS, full workspace.

## User Setup Required

None.

## Next Phase Readiness

Plan 05-04 can update the DuckDB SQL smoke path to feed real layout payload bytes through `loom_decode` and verify end-to-end SQL results.

---
*Phase: 05-fsst-l2-kernel-and-full-verification*
*Completed: 2026-06-08*
