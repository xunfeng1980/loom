---
phase: 03-l1-bitpack-for-and-arrow-builders
plan: "02"
subsystem: loom-fixtures
tags: [vortex-reader, oracle, wave0-checks, bitpack-roundtrip, for-roundtrip, d02-isolation]

dependency_graph:
  requires:
    - "03-01 (loom-core LayoutNode + synthesized_read_loop + OutputBuilder)"
  provides:
    - "vortex_reader: from_bitpacked_array / from_for_array / extract_validity / packed_bytes"
    - "oracle: decode_i32_oracle / decode_u32_oracle / extract_null_flags"
    - "Wave-0 BLOCKING checks: fl_transpose_matches_fastlanes, bitpack_11bit_roundtrip, nullable_roundtrip"
    - "bitpack_roundtrip: 3-bit/17-bit non-byte-aligned, all_null AllInvalid, nullable scattered-null"
    - "for_roundtrip: FoR-over-BitPacked, negative reference, nullable FoR"
  affects:
    - "03-03 (FSST L2 kernel — will consume oracle + vortex_reader patterns)"
    - "Phase 5 (DuckDB wiring — oracle validates end-to-end decode)"

tech_stack:
  added:
    - "loom-core = { path = ../loom-core } as [dependencies] in loom-fixtures"
    - "fastlanes =0.5.1 in [dev-dependencies] ONLY (T-03-SC accepted)"
    - "vortex-session =0.74.0 in [dev-dependencies]"
    - "vortex-buffer =0.74.0 in [dev-dependencies]"
    - "arrow = workspace in [dev-dependencies]"
    - "arrow-schema = workspace in [dev-dependencies]"
  patterns:
    - "BufferHandle access: arr.packed().as_host().as_ref() (option A confirmed)"
    - "BitPackedArrayExt::validity(arr) explicit UFCS to avoid ArrayRef::validity() ambiguity"
    - "as_opt::<BitPacked>() returns ArrayView<'_, BitPacked> — generic helper fn from_bitpacked_view<T: BitPackedArrayExt>"
    - "FoR+BitPack construction: FoR::try_new(bp.into_array(), reference.into()) wrapping pre-computed deltas"
    - "FoRData::encode does NOT bitpack inner (inner stays vortex.primitive) — manual delta+bitpack required"
    - "extract_validity: Validity::Array executes BoolArray to canonical inside vortex_reader (T-03-04)"

key_files:
  created:
    - crates/loom-fixtures/src/vortex_reader.rs
    - crates/loom-fixtures/src/oracle.rs
    - crates/loom-fixtures/tests/wave0_checks.rs
    - crates/loom-fixtures/tests/bitpack_roundtrip.rs
    - crates/loom-fixtures/tests/for_roundtrip.rs
  modified:
    - crates/loom-fixtures/Cargo.toml
    - crates/loom-fixtures/src/lib.rs

decisions:
  - "BufferHandle access: .as_host().as_ref() (option A) confirmed over .as_slice() / Deref indexing"
  - "BitPackedArrayExt::validity explicit UFCS required — inherent ArrayRef::validity returns VortexResult, trait method returns Validity"
  - "FoR test fixtures use manual delta computation + FoR::try_new, not FoRData::encode (which leaves inner as vortex.primitive, breaking from_for_array)"
  - "generic from_bitpacked_view<T: BitPackedArrayExt> helper needed because as_opt returns ArrayView, not BitPackedArray"
  - "D-02: all vortex-* types translated in loom-fixtures; loom-core dep tree shows 0 vortex/fastlanes entries"

metrics:
  duration: "~120 minutes (working time; session interrupted by context limit)"
  completed: "2026-06-07"
  tasks_completed: 3
  tasks_total: 3
  files_created: 5
  files_modified: 2
---

# Phase 03 Plan 02: Vortex Reader, Oracle, and Roundtrip Test Suites — Summary

**One-liner:** vortex_reader bridges in-memory Vortex BitPacked/FoR arrays to loom-core LayoutNodes with Validity flattened to Vec<bool>, and 10 roundtrip tests prove loom-core decode matches the Vortex oracle row-for-row with D-02 isolation confirmed.

## Tasks Completed

| # | Name | Commit | Files |
|---|------|--------|-------|
| 1+2 | Wave-0 BLOCKING checks + vortex_reader + oracle | 64efb32 | Cargo.toml, lib.rs, vortex_reader.rs, oracle.rs, wave0_checks.rs |
| 3 | Bitpack and FOR roundtrip test suites | 198c77f | bitpack_roundtrip.rs, for_roundtrip.rs |

## Test Coverage

### Wave-0 BLOCKING Checks (wave0_checks.rs) — all PASS

| Test | What it checks |
|------|----------------|
| `fl_transpose_matches_fastlanes` | loom-core fl_transpose_index(i) == fastlanes::transpose(i) for all 1024 i |
| `bitpack_11bit_roundtrip` | 11-bit two-chunk (1025 elements) decode matches oracle row-for-row |
| `nullable_roundtrip` | Scattered null pattern: ArrayData::nulls().is_null(i) matches Vortex validity bit-for-bit |

### Bitpack Roundtrip Tests (bitpack_roundtrip.rs) — all PASS

| Test | What it checks |
|------|----------------|
| `bitpack_non_byte_aligned_3bit` | 200 elements, 3-bit non-byte-aligned |
| `bitpack_non_byte_aligned_17bit` | 150 elements, 17-bit crosses 32-bit word boundary |
| `all_null_bitpack` | AllInvalid fast path: 32 elements all None, 5-bit |
| `nullable_bitpack` | 128 elements, i%7==0 nulls, 11-bit |

### FOR Roundtrip Tests (for_roundtrip.rs) — all PASS

| Test | What it checks |
|------|----------------|
| `for_roundtrip` | FoR(reference=1000) over 7-bit BitPack, 100 elements |
| `for_negative_reference` | FoR(reference=-500) over 7-bit BitPack (Open Q2 resolved) |
| `for_nullable` | FoR over 6-bit BitPack with nulls at i%4==0, 64 elements |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] FoRData::encode does NOT produce FoR-over-BitPacked**
- **Found during:** Task 3 initial test run
- **Issue:** `FoRData::encode(parray)` subtracts the minimum but leaves the inner array as `vortex.primitive` (plain PrimitiveArray). The plan assumed `FoRData::encode` would produce FoR-over-BitPack. `from_for_array` panicked with "FoRArray inner must be a BitPackedArray" when the inner was `vortex.primitive`.
- **Fix:** Rewrote for_roundtrip.rs to use the correct construction pattern: manually compute deltas (non-negative, fitting in bit_width bits), `BitPackedData::encode(&deltas.into_array(), bit_width, &mut ctx)`, then `FoR::try_new(bp.into_array(), reference.into())`. This is the same pattern used in vortex-fastlanes own tests (`test_decompress_fused`).
- **Files modified:** crates/loom-fixtures/tests/for_roundtrip.rs (complete rewrite from broken version)
- **Commit:** 198c77f

**2. [Rule 1 - Bug] validity() method ambiguity — ArrayRef::validity vs BitPackedArrayExt::validity**
- **Found during:** Task 2 compilation
- **Issue:** `arr.validity()` on a `BitPackedArray` resolved to `ArrayRef::validity()` returning `VortexResult<Validity>` instead of `BitPackedArrayExt::validity()` returning `Validity`. Same for `canonical.validity()` on `PrimitiveArray`.
- **Fix:** Explicit UFCS: `BitPackedArrayExt::validity(arr)` and `PrimitiveArrayExt::validity(&canonical)`.
- **Files modified:** vortex_reader.rs, oracle.rs
- **Commit:** 64efb32

**3. [Rule 1 - Bug] as_opt::<BitPacked>() returns ArrayView, not BitPackedArray**
- **Found during:** Task 2 compilation
- **Issue:** `inner_array_ref.as_opt::<BitPacked>()` returns `Option<ArrayView<'_, BitPacked>>`, not `Option<BitPackedArray>`. The `from_bitpacked_array` signature requires `&BitPackedArray`.
- **Fix:** Added generic helper `fn from_bitpacked_view<T: BitPackedArrayExt>(view: &T) -> LayoutNode` accepting any type implementing BitPackedArrayExt, since `ArrayView<'_, BitPacked>` implements `TypedArrayRef<BitPacked>` which provides BitPackedArrayExt.
- **Files modified:** vortex_reader.rs
- **Commit:** 64efb32

**4. [Rule 3 - Blocking] Missing dev-dependencies for test compilation**
- **Found during:** Task 1/2 test compilation
- **Issue:** Tests needed `arrow`, `arrow-schema`, `vortex-buffer`, `vortex-session` not in loom-fixtures Cargo.toml.
- **Fix:** Added all four to `[dev-dependencies]`.
- **Files modified:** crates/loom-fixtures/Cargo.toml
- **Commit:** 64efb32

## D-02 Isolation Verification

```
$ cargo tree -p loom-core | grep -c -E 'vortex|fastlanes'
0
```

loom-core has zero vortex-* and fastlanes entries in its dependency tree. All Vortex types are translated in loom-fixtures before being handed to loom-core as plain Rust primitives.

## D-03 Verification

`git diff --stat HEAD~3 HEAD -- crates/loom-ffi duckdb-ext` returns empty. loom_decode and loom_scan are untouched.

## Success Criteria Checklist

- [x] vortex_reader reads in-memory Vortex BitPacked/FoR into LayoutNode + bytes, no .vortex file (INPUT-01, INPUT-02)
- [x] fl_transpose_index == fastlanes::transpose for all 1024 indices (Wave-0 #2)
- [x] Non-byte-aligned BitPacked (3-bit, 11-bit two-chunk, 17-bit) decodes to Arrow matching oracle row-for-row (SC-1, L1-03)
- [x] FoR over BitPack with positive AND negative reference decodes with reference added (SC-2, L1-04, Open Q2 resolved)
- [x] Nullable columns (bitpack, all_null, for_nullable) roundtrip nulls bit-for-bit vs Vortex validity (SC-3, L1-07)
- [x] No .vortex file read; all fixtures via vortex-array/fastlanes builder APIs (SC-5)
- [x] Full workspace passes: cargo test --workspace exits 0

## Known Stubs

None. All roundtrip tests wire real data through real decode paths; no hardcoded outputs or placeholder returns.

## Threat Flags

No new threat surface beyond the plan's threat register. All mitigations applied:
- T-03-04: extract_validity executes BoolArray to canonical inside vortex_reader — implemented
- T-03-SC: fastlanes dev-dep confirmed in [dev-dependencies] only — cargo tree confirms isolation

## Self-Check: PASSED
