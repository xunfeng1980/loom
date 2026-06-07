---
phase: 05-fsst-l2-kernel-and-full-verification
plan: "01"
subsystem: core
tags: [rust, arrow, fsst, utf8, l2-kernel, dictionary]
requires:
  - phase: 04-l1-dict-rle-and-l2-escape-infrastructure
    provides: Dictionary/RunEnd L1 recursion and L2KernelRegistry id 0 stub
provides:
  - Loom-owned FSST params binary format
  - real FSST id 0 Utf8 L2 kernel
  - Utf8 OutputBuilder and DecodedArray materialization
  - registry-aware nested KernelEscape dispatch for Dictionary values
affects: [loom-core, phase-05-wave-2]
tech-stack:
  added:
    - fsst-rs = 0.5.11
  patterns: [validated params parser, kernel-owned ArrayData, registry-aware child materialization]
key-files:
  created:
    - crates/loom-core/src/fsst_params.rs
  modified:
    - Cargo.toml
    - crates/loom-core/Cargo.toml
    - crates/loom-core/src/lib.rs
    - crates/loom-core/src/error.rs
    - crates/loom-core/src/arrow_builder_output.rs
    - crates/loom-core/src/l2_kernel_registry.rs
    - crates/loom-core/src/l1_model.rs
key-decisions:
  - "FSST params use a Loom-owned LFS1 binary format; loom-core does not depend on vortex-fsst or fastlanes."
  - "FsstKernel wraps fsst-rs decompression in catch_unwind and converts decoder panics into typed LoomDecodeError values."
  - "Dictionary over KernelEscape(FSST) uses the existing Dictionary gather path with registry-aware child materialization."
patterns-established:
  - "FsstParams validates row counts, symbol metadata, offsets, validity bytes, codes length, truncation, and trailing bytes before decode."
  - "decode_layout_to_array_data can materialize nested KernelEscape children through the registry while synthesized_read_loop remains builder-only."
requirements-completed: [L2-02, L2-03]
duration: 5min
completed: 2026-06-08
---

# Phase 05-01: FSST L2 Kernel and Utf8 Integration Summary

**loom-core now decodes validated FSST params into Utf8 Arrow arrays and can gather Dictionary values from nested FSST KernelEscape nodes.**

## Performance

- **Duration:** 5 min
- **Started:** 2026-06-07T16:38:06Z
- **Completed:** 2026-06-07T16:43:51Z
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments

- Added `fsst-rs` as a pinned workspace dependency and introduced `FsstParams` with the stable `LFS1` little-endian binary format.
- Added typed FSST errors for malformed params, invalid symbol tables, invalid offsets, invalid UTF-8, and decoder failures.
- Replaced the Phase-4 empty Utf8 FSST stub with real `fsst::Decompressor` decode into `StringBuilder`, including null preservation and panic-to-error conversion.
- Extended `OutputBuilder` and `DecodedArray` to support Utf8 values/nulls.
- Updated `decode_layout_to_array_data` internals so Dictionary values can be `KernelEscape(FSST)` without a special dict-FSST kernel.

## Task Commits

1. **Task 1: FSST params format** - `09f1296` (feat)
2. **Task 2: FSST L2 kernel** - `b1daab4` (feat)
3. **Task 3: Utf8 dict-over-FSST** - `daa5325` (feat)

**Plan metadata:** this summary commit.

## Files Created/Modified

- `crates/loom-core/src/fsst_params.rs` - `FsstParams` struct, encoder, decoder, validation, and tests.
- `crates/loom-core/src/l2_kernel_registry.rs` - real FSST id 0 decode body and kernel behavior tests.
- `crates/loom-core/src/arrow_builder_output.rs` - `OutputBuilder::Utf8`, `append_string`, nulling, and finish support.
- `crates/loom-core/src/l1_model.rs` - `DecodedArray::Utf8`, registry-aware nested materialization, and dict-over-FSST tests.
- `crates/loom-core/src/error.rs` - FSST-specific typed decode errors.
- `crates/loom-core/src/lib.rs` - exposes `fsst_params`.
- `Cargo.toml`, `crates/loom-core/Cargo.toml` - adds the pinned `fsst-rs` dependency.

## Decisions Made

- Zero-row FSST decode now requires encoded zero-row `FsstParams`; empty params are malformed.
- Test fixtures use escape-coded FSST rows where that keeps behavior deterministic without depending on compressor training.
- Panic coverage triggers a safe `fsst-rs` assertion path with a trailing escape byte, avoiding unchecked-code out-of-bounds behavior inside the dependency.

## Deviations from Plan

None.

## Issues Encountered

- `grep -c` prints `0` for the dependency isolation check but returns a nonzero shell status when there are no matches. The final verification used `awk` to report the same count with exit 0.

## Verification

- `cargo test -p loom-core fsst_params` - PASS, 4 tests.
- `cargo test -p loom-core l2_kernel_registry` - PASS, 7 tests.
- `cargo test -p loom-core dictionary_over_fsst` - PASS, 1 test.
- `cargo test -p loom-core` - PASS, 51 unit tests plus doc tests.
- `cargo tree -p loom-core | awk '/vortex|fastlanes/{c++} END{print c+0}'` - PASS, printed `0`.

## User Setup Required

None.

## Next Phase Readiness

Wave 2 can now build fixture extraction and oracle coverage on top of a real FSST Utf8 kernel and the general Dictionary gather path.

---
*Phase: 05-fsst-l2-kernel-and-full-verification*
*Completed: 2026-06-08*
