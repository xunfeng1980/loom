---
phase: 04-l1-dict-rle-and-l2-escape-infrastructure
plan: "02"
subsystem: testing
tags: [rust, vortex, fixtures, dictionary, rle, kernel-escape]
requires:
  - phase: 04-l1-dict-rle-and-l2-escape-infrastructure
    provides: 04-01 loom-core Dictionary, RunEnd, and KernelEscape support
provides:
  - DictArray to LayoutNode::Dictionary fixture bridge
  - FastLanes RLE to Loom RunEnd canonicalizing fixture bridge
  - dictionary, RLE/RunEnd, and KernelEscape integration tests
  - boolean oracle helper
affects: [loom-fixtures, loom-core, phase-05-fsst]
tech-stack:
  added: []
  patterns: [Vortex-only fixture bridge, oracle row comparison, documented RLE canonicalization]
key-files:
  created:
    - crates/loom-fixtures/tests/dict_roundtrip.rs
    - crates/loom-fixtures/tests/rle_roundtrip.rs
    - crates/loom-fixtures/tests/kernel_escape_roundtrip.rs
  modified:
    - crates/loom-core/src/l1_model.rs
    - crates/loom-fixtures/Cargo.toml
    - crates/loom-fixtures/src/oracle.rs
    - crates/loom-fixtures/src/vortex_reader.rs
    - crates/loom-fixtures/tests/bitpack_roundtrip.rs
    - crates/loom-fixtures/tests/for_roundtrip.rs
key-decisions:
  - "Dictionary bridge recursively preserves encoded children such as BitPackedArray codes and values."
  - "FastLanes RLE is chunk-index based, so Phase 4 canonicalizes decoded Vortex rows into simple Loom RunEnd for fixture comparison."
  - "Dictionary code decoding now selects Int32 for bitpacked/plain 32-bit codes and Int64 only for 8-byte raw code buffers."
patterns-established:
  - "Fixture bridges can touch Vortex internals, but only emit plain LayoutNode data before crossing into loom-core."
  - "RLE tests pair a live Vortex oracle case with direct Loom RunEnd fallback coverage for boolean and nullable expansion."
requirements-completed: [L1-05, L1-06, L2-01]
duration: 10min
completed: 2026-06-07
---

# Phase 04-02: Fixture and Oracle Coverage Summary

**Real Vortex DictArray and RLE fixtures now verify Loom dictionary, RunEnd, and KernelEscape behavior row-for-row where bridgeable**

## Performance

- **Duration:** 10 min
- **Started:** 2026-06-07T16:03:45Z
- **Completed:** 2026-06-07T16:13:24Z
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments

- Added `from_dict_array` using `DictArraySlotsExt` with recursive conversion of encoded codes and values.
- Added `from_rle_array` using `RLEArrayExt`, documented the FastLanes chunk-index mismatch, and canonicalized live Vortex RLE output into Loom RunEnd for verification.
- Added dict, RLE/RunEnd, and KernelEscape integration tests covering integer, nullable, boolean, id 0, and unknown-id cases.
- Fixed dictionary code decoding so Vortex bitpacked i32 codes are decoded as Int32, while raw 8-byte code buffers still decode as Int64.

## Task Commits

1. **Task 1-3: fixture bridges and integration coverage** - `ed9e84f` (feat)

**Plan metadata:** this summary commit.

## Files Created/Modified

- `crates/loom-fixtures/tests/dict_roundtrip.rs` - live Vortex DictArray integer and nullable oracle tests.
- `crates/loom-fixtures/tests/rle_roundtrip.rs` - live Vortex RLE integer test plus direct boolean/nullable Loom RunEnd tests.
- `crates/loom-fixtures/tests/kernel_escape_roundtrip.rs` - public decode helper tests for FSST id 0 and unknown id.
- `crates/loom-fixtures/src/vortex_reader.rs` - dictionary bridge, RLE canonicalizing bridge, primitive/bool/raw helpers.
- `crates/loom-fixtures/src/oracle.rs` - boolean oracle helper.
- `crates/loom-core/src/l1_model.rs` - dictionary code width selection for bitpacked Vortex codes.

## Decisions Made

- FastLanes RLE is not structurally the same as Loom's simple `RunEnd { run_ends, values }`; this phase keeps the gap visible with an explicit canonicalizing bridge and fallback tests.
- The no-file-fixture grep is treated literally, so comments were reworded to avoid matching the forbidden file API patterns while still documenting in-memory fixture construction.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Bitpacked dictionary codes decoded with the wrong native width**
- **Found during:** Task 1 dict oracle tests
- **Issue:** `decode_dictionary` always decoded codes as Int64, but Vortex bitpacked code buffers in tests use 32-bit native packing.
- **Fix:** Added `dictionary_code_data_type`, selecting Int32 for bitpacked/default code layouts and Int64 only for raw 8-byte code buffers.
- **Files modified:** `crates/loom-core/src/l1_model.rs`
- **Verification:** `RUSTC_WRAPPER= cargo test -p loom-fixtures --test dict_roundtrip`
- **Committed in:** `ed9e84f`

**2. [Rule 3 - Blocking] Forbidden fixture-file grep matched explanatory comments**
- **Found during:** Task 3 verification
- **Issue:** The grep intended to catch forbidden file APIs matched comments saying no on-disk fixture files are opened.
- **Fix:** Reworded comments and Cargo notes to keep the structural check literal.
- **Files modified:** `crates/loom-fixtures/Cargo.toml`, `crates/loom-fixtures/tests/bitpack_roundtrip.rs`, `crates/loom-fixtures/tests/for_roundtrip.rs`
- **Verification:** `rg -n "vortex_file|vortex-file|\\.vortex|VortexFile|from_path|read_file" crates/loom-fixtures`
- **Committed in:** `ed9e84f`

---

**Total deviations:** 2 auto-fixed (integration correctness and verification hygiene).
**Impact on plan:** Both were required for oracle-backed coverage and did not expand runtime scope.

## Issues Encountered

- `git diff --stat -- crates/loom-ffi duckdb-ext` still reports pre-existing dirty `loom-ffi` files in the working tree. Wave 2 did not stage or commit those files; the Phase 4 commits leave DuckDB and FFI code untouched.

## Verification

- `RUSTC_WRAPPER= cargo test -p loom-fixtures --test dict_roundtrip` - PASS, 2 tests.
- `RUSTC_WRAPPER= cargo test -p loom-fixtures --test rle_roundtrip` - PASS, 4 tests.
- `RUSTC_WRAPPER= cargo test -p loom-fixtures --test kernel_escape_roundtrip` - PASS, 2 tests.
- `RUSTC_WRAPPER= cargo test -p loom-fixtures` - PASS, all fixture tests.
- `RUSTC_WRAPPER= cargo test --workspace` - PASS, all workspace tests.
- `RUSTC_WRAPPER= cargo tree -p loom-core | grep -c -E 'vortex|fastlanes'` - PASS, printed `0`.
- `rg -n "vortex_file|vortex-file|\\.vortex|VortexFile|from_path|read_file" crates/loom-fixtures` - PASS, no matches.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 5 can build the real FSST L2 kernel on top of the public `L2KernelRegistry` and the fixture/oracle pattern established here. Core L1 dictionary and RunEnd requirements are covered by both unit and fixture-level tests.

---
*Phase: 04-l1-dict-rle-and-l2-escape-infrastructure*
*Completed: 2026-06-07*
