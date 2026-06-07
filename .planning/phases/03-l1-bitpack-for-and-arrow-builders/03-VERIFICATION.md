---
phase: 03-l1-bitpack-for-and-arrow-builders
verified: 2026-06-07T16:00:00Z
status: passed
score: 9/9 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 8/9
  gaps_closed:
    - "synthesized_read_loop decodes Raw, BitPack, and FrameOfReference nodes; no arm panics on malformed input"
  gaps_remaining: []
  regressions: []
---

# Phase 3: L1 Bitpack, FOR, and Arrow Builders — Verification Report

**Phase Goal:** L1 Bitpack, FOR, and Arrow Builders — core decode infrastructure (Arrow typed builders, vortex_reader, LayoutNode model) and the first two L1 decoders (BitPack, FrameOfReference) with null handling.
**Verified:** 2026-06-07T16:00:00Z
**Status:** passed
**Re-verification:** Yes — after gap closure (commit a2d4bd5 fixed CR-01 decode_raw unchecked multiply)

---

## Re-Verification Summary

The sole blocker from the initial verification (2026-06-07T14:00:00Z) was:

**CR-01** — `decode_raw` at `l1_model.rs:267` used an unchecked `count * stride` multiplication. A crafted `LayoutNode::Raw` with `count * elem_size > usize::MAX` could overflow `needed` to a small value, bypass the bounds check, and panic on slice indexing — violating the "no arm panics on malformed input" contract.

**Fix applied in commit a2d4bd5:**

- `crates/loom-core/src/l1_model.rs`: lines 267–275 now use
  `count.checked_mul(stride).ok_or(LoomDecodeError::BufferTooShort { needed: usize::MAX, got: data.len() })?`
  mirroring the `bitpack::unpack_all` pattern.
- Regression test `l1_model::tests::raw_count_overflow_returns_buffer_too_short` added and confirmed passing.

**Test run:** `cargo test --workspace` — **44 tests, 0 failed** (25 loom-core, 3 loom-ffi, 1 buffer_layout, 2 roundtrip, 4 bitpack_roundtrip, 3 for_roundtrip, 3 wave0_checks).

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A non-byte-aligned BitPacked Vortex array (11-bit, 1025 elements) decodes to Arrow Int32Array matching the original input row-for-row | VERIFIED | `wave0_checks::bitpack_11bit_roundtrip` passes; bitpack_roundtrip tests for 3-bit, 17-bit also pass |
| 2 | A FrameOfReference column over bitpacking decodes with the reference scalar added to every unpacked value | VERIFIED | `for_roundtrip.rs`: for_roundtrip (ref=1000) and for_negative_reference (ref=-500) both pass |
| 3 | Nullable columns per encoding (bitpack, FOR) round-trip nulls intact | VERIFIED | `wave0_checks::nullable_roundtrip`, `bitpack_roundtrip::nullable_bitpack`, `for_roundtrip::for_nullable` — all pass; null positions match Vortex validity bit-for-bit |
| 4 | arrow_builder_output::finish() produces ArrayData exportable via to_ffi without compile errors | VERIFIED | arrow_builder_output tests pass; OutputBuilder uses same `finish().into_data()` chain proven in `loom-ffi/ffi.rs` |
| 5 | No .vortex file read or written; all inputs constructed via vortex-array builder APIs | VERIFIED | grep for `vortex_file/VortexFile/.vortex/read_file` in `crates/loom-fixtures/` returns nothing; all fixtures use `BitPackedData::encode` / `FoR::try_new` |
| 6 | The full LayoutNode enum (Raw, BitPack, FrameOfReference, Dictionary, RunEnd, KernelEscape) exists as pure data | VERIFIED | `l1_model.rs`: all six arms defined; `LayoutDescription` struct present |
| 7 | fl_transpose_index matches fastlanes::transpose for all 1024 indices | VERIFIED | `wave0_checks::fl_transpose_matches_fastlanes` passes (BLOCKING Wave-0 check #2 explicit crate comparison) |
| 8 | Validity routes through append_null so nulls land in the Arrow null bitmap | VERIFIED | AllInvalid fast path, per-row bitmap, and FOR inner validity — all tested and passing |
| 9 | synthesized_read_loop decodes Raw, BitPack, and FrameOfReference with no panic on malformed input | VERIFIED | `decode_raw` now uses `checked_mul` (lines 267–275, commit a2d4bd5). Regression test `raw_count_overflow_returns_buffer_too_short` passes — overflow input returns `Err(BufferTooShort)`, never panics. BitPack and FOR arms use `checked_mul` inside `unpack_all` (unchanged). All three arms now satisfy the T-03-01/T-03-03 no-panic contract. |

**Score:** 9/9 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/loom-core/src/error.rs` | LoomDecodeError enum (UnimplementedEncoding/BufferTooShort/UnsupportedWidth/BitWidthExceedsType) | VERIFIED | 4 variants, Display + Error impl, 2 tests |
| `crates/loom-core/src/l1_model.rs` | LayoutNode enum + synthesized_read_loop | VERIFIED | 797 lines; all 6 arms; synthesized_read_loop + decode_raw/decode_bitpack/decode_for helpers; 8 unit tests (2 added by gap fix) |
| `crates/loom-core/src/l1_model/bitpack.rs` | FL_ORDER, fl_transpose_index, fl_index, unpack_all | VERIFIED | FL_ORDER = [0,4,2,6,1,5,3,7]; fl_transpose_index formula exact; unpack_all bounds-checked with checked_mul; 9 unit tests |
| `crates/loom-core/src/arrow_builder_output.rs` | OutputBuilder enum (Int32/Int64), append_i32/append_i64/append_null/t_bits/finish | VERIFIED | Typed builders only; finish() returns ArrayData; 7 unit tests |
| `crates/loom-fixtures/src/vortex_reader.rs` | from_bitpacked_array / from_for_array / extract_validity / packed_bytes | VERIFIED | All four functions present and substantive; D-02 isolation enforced |
| `crates/loom-fixtures/src/oracle.rs` | decode_i32_oracle / decode_u32_oracle / extract_null_flags | VERIFIED | Vortex execute::<PrimitiveArray> path; explicit UFCS for validity ambiguity |
| `crates/loom-fixtures/tests/wave0_checks.rs` | fl_transpose_matches_fastlanes, bitpack_11bit_roundtrip, nullable_roundtrip | VERIFIED | All 3 BLOCKING checks pass |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/loom-core/src/l1_model.rs` | `crates/loom-core/src/l1_model/bitpack.rs` | BitPack arm calls bitpack::unpack_all | WIRED | decode_bitpack and decode_for both call bitpack::unpack_all; confirmed in source |
| `crates/loom-core/src/l1_model.rs` | `crates/loom-core/src/arrow_builder_output.rs` | read loop appends through OutputBuilder | WIRED | All three decode helpers accept &mut OutputBuilder and call append_i32/append_i64/append_null |
| `crates/loom-fixtures/src/vortex_reader.rs` | `crates/loom-core/src/l1_model.rs` | constructs LayoutNode::BitPack and ::FrameOfReference | WIRED | from_bitpacked_array returns LayoutNode::BitPack; from_for_array returns LayoutNode::FrameOfReference |
| `crates/loom-fixtures/tests/bitpack_roundtrip.rs` | `loom_core::synthesized_read_loop` | decodes the reader's LayoutNode and compares to oracle | WIRED | All 4 bitpack_roundtrip tests call synthesized_read_loop |

---

## Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| wave0_checks::bitpack_11bit_roundtrip | oracle_values, decoded | BitPackedData::encode in-memory fixture → vortex_reader → synthesized_read_loop → oracle::decode_i32_oracle | Yes — 1025 real values compared | FLOWING |
| for_roundtrip::for_negative_reference | decoded | FoR::try_new in-memory fixture → from_for_array → synthesized_read_loop | Yes — 100 real i32 values with reference=-500 | FLOWING |
| nullable_bitpack | array_data | BitPackedData::encode with PrimitiveArray::from_option_iter → from_bitpacked_array → synthesized_read_loop | Yes — 128 values + scattered nulls cross-checked with oracle | FLOWING |

---

## Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| cargo test --workspace exits 0 | `cargo test --workspace` | 44 tests passed: 25 loom-core, 3 loom-ffi, 1 buffer_layout, 2 roundtrip, 4 bitpack_roundtrip, 3 for_roundtrip, 3 wave0_checks | PASS |
| raw_count_overflow_returns_buffer_too_short passes | included in loom-core test run | `test l1_model::tests::raw_count_overflow_returns_buffer_too_short ... ok` | PASS |
| loom-core tree has 0 vortex/fastlanes entries | `cargo tree -p loom-core \| grep -c -E 'vortex\|fastlanes'` | 0 | PASS |
| No unsafe in loom-core | `grep -rn 'unsafe' crates/loom-core/src/` | only `#![forbid(unsafe_code)]` declaration in lib.rs | PASS |
| No into_canonical/into_arrow delegation | `grep -rn 'into_canonical\|into_arrow' crates/loom-core/src/` | 0 matches | PASS |
| No todo!/panic!/unimplemented! in loom-core src (non-test) | `grep -n 'panic!\|todo!\|unimplemented!' crates/loom-core/src/l1_model.rs` | 0 matches in production code | PASS |
| No .vortex file access in loom-fixtures | `grep -rn 'vortex_file\|VortexFile\|\.vortex\|read_file' crates/loom-fixtures/` | 0 matches | PASS |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| INPUT-01 | 03-02 | In-memory Vortex array read without .vortex file | SATISFIED | vortex_reader reads BitPackedArray/FoRArray in-process; no file I/O |
| INPUT-02 | 03-02 | Fixtures constructed programmatically | SATISFIED | BitPackedData::encode / FoR::try_new only; grep confirms no vortex_file |
| L1-01 | 03-01 | LayoutNode data model | SATISFIED | Six-arm enum + LayoutDescription in l1_model.rs |
| L1-02 | 03-01 | Synthesized read loop | SATISFIED | synthesized_read_loop in l1_model.rs decodes all planned encodings |
| L1-03 | 03-01, 03-02 | Decode bit-packed including non-byte-aligned | SATISFIED | 3-bit, 11-bit (two-chunk), 17-bit all pass oracle comparison |
| L1-04 | 03-01, 03-02 | Decode FOR over bitpacking | SATISFIED | for_roundtrip, for_negative_reference, for_nullable all pass |
| L1-07 | 03-01, 03-02 | Null/validity preserved through all L1 paths | SATISFIED | AllInvalid fast path, per-row bitmap, FOR inner validity — all tested |
| ARROW-01 | 03-01 | Decoded values emitted only through typed Arrow builders | SATISFIED | OutputBuilder wraps Int32Builder/Int64Builder; no raw buffer writes |
| ARROW-02 | 03-01 | Output materializes as Arrow ArrayData | SATISFIED | OutputBuilder::finish() returns ArrayData; chain to to_ffi unblocked |

All 9 requirements for Phase 3 are satisfied. No orphaned requirements found.

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/loom-core/src/l1_model.rs` | 389–392 | decode_for non-BitPack fallback delegates to synthesized_read_loop(inner, builder) without applying reference | WARNING (CR-02) | Silent wrong-result path: FrameOfReference over a non-BitPack inner returns inner values without the reference applied. Not reachable by any current Phase 3 fixture (vortex_reader always constructs BitPack inner). No panic, but incorrect output. Should return UnimplementedEncoding or apply reference after recursive decode in a future phase. Low risk for Phase 3 goal; becomes a landmine if Phase 4 constructs FOR-over-non-BitPack. |
| `crates/loom-core/src/l1_model/bitpack.rs` | 480, 499 | `panic!()` in test-only encode_for_test helper | INFO | Test-only (#[cfg(test)]); not reachable from production code. Not a violation of the no-panic contract. |
| `crates/loom-core/src/arrow_builder_output.rs` | 67, 81, 96 | `panic!()` in OutputBuilder::new and type-mismatch guards | INFO | Guards against programming errors (wrong builder type), not malformed input. Not attacker-controlled in Phase 3. |
| `crates/loom-core/src/l1_model.rs` | 712–728 | encode_test_values straddle branch (test module copy) lacks remaining>0 guard | WARNING | Test-only code; two copies of pack logic have diverged. Does not affect production decode path. |

**Debt markers:** `grep -rn 'TBD|FIXME|XXX' crates/loom-core/src/ crates/loom-fixtures/src/` returns 0 matches. No unresolved debt markers.

**CR-01 status:** RESOLVED. The unchecked multiply that was a BLOCKER in the initial verification has been replaced with `checked_mul` + `BufferTooShort` propagation. The regression test confirms the fix. The CR-02 WARNING remains and is carried forward unchanged.

---

## Human Verification Required

None. All phase-3 behaviors are verifiable programmatically. The row-for-row oracle comparison is automated in wave0_checks.rs and the roundtrip test suites.

---

## Gaps Summary

No gaps. The single blocker (CR-01 decode_raw unchecked multiply) was resolved in commit a2d4bd5. All 9 must-have truths are now VERIFIED. The CR-02 WARNING (decode_for non-BitPack fallback missing reference application) is a non-blocking concern that does not affect any reachable code path in Phase 3.

---

_Verified: 2026-06-07T16:00:00Z_
_Verifier: Claude (gsd-verifier)_
