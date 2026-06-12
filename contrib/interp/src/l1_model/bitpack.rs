//! Pure-Rust FastLanes bit-unpacking — zero `vortex-*` / `fastlanes` dependency.
//!
//! This module reimplements the FastLanes transposed bit-packing layout without
//! importing the `fastlanes` crate. loom-core must stay free of the Vortex
//! ecosystem (D-02) so this is the "independence proof" for Phase 3.
//!
//! # FastLanes layout overview
//!
//! A FastLanes BitPackedArray is laid out as a sequence of **1024-element blocks**.
//! Each block of 1024 native-type values (e.g. `u32` = 32 bits, `t_bits = 32`)
//! packed at bit-width `W` occupies exactly `128 * W` bytes.
//!
//! The values are NOT stored in sequential order. FastLanes uses a *transposed*
//! layout designed for SIMD: `LANES = 1024 / t_bits` SIMD lanes each processes
//! `t_bits` values. The mapping from logical index to packed position is given by
//! [`fl_index`].
//!
//! Buffer layout within one block:
//! - `elems_per_chunk = 128 * W / (t_bits / 8)` native-type elements.
//! - Element at packed position `(word_idx * LANES + lane)` occupies bytes
//!   `(word_idx * LANES + lane) * (t_bits/8)` in little-endian.
//!
//! # Sources
//!
//! - `fastlanes-0.5.1/src/transpose.rs` — `FL_ORDER`, `transpose()`
//! - `fastlanes-0.5.1/src/macros.rs` — `index()`, pack/unpack bit arithmetic
//! - `vortex-fastlanes-0.74.0/src/bitpacking/array/unpack_iter.rs` —
//!   `CHUNK_SIZE = 1024`, `elems_per_chunk` formula

use loom_ir_core::error::LoomDecodeError;

// ---------------------------------------------------------------------------
// FL_ORDER — the FastLanes lane permutation constant
// ---------------------------------------------------------------------------

/// FastLanes lane permutation table.
///
/// Source: `fastlanes-0.5.1/src/lib.rs` and `transpose.rs`.
///
/// Used in both [`fl_transpose_index`] (logical→transposed) and [`fl_index`]
/// (row/lane→logical) to reorder the 8 groups of rows for SIMD efficiency.
pub const FL_ORDER: [usize; 8] = [0, 4, 2, 6, 1, 5, 3, 7];

// ---------------------------------------------------------------------------
// fl_transpose_index — logical index → transposed storage index
// ---------------------------------------------------------------------------

/// Map a **logical** array index (0..1024) to the **transposed storage index**
/// used by the FastLanes pack/unpack layout.
///
/// Source: `fastlanes-0.5.1/src/transpose.rs` `transpose()` function — exact
/// replication of the published formula.
///
/// ```text
/// lane  = idx % 16
/// order = (idx / 16) % 8
/// row   = idx / 128
/// result = (lane * 64) + (FL_ORDER[order] * 8) + row
/// ```
#[inline(always)]
pub const fn fl_transpose_index(idx: usize) -> usize {
    let lane = idx % 16;
    let order = (idx / 16) % 8;
    let row = idx / 128;
    (lane * 64) + (FL_ORDER[order] * 8) + row
}

// ---------------------------------------------------------------------------
// fl_index — (row, lane) → logical index
// ---------------------------------------------------------------------------

/// Map a (row, lane) pair in the FastLanes pack/unpack loop to the
/// **logical array index** that element represents.
///
/// Source: `fastlanes-0.5.1/src/macros.rs` `index(row, lane)` macro.
///
/// The pack/unpack loop iterates `for lane in 0..LANES { for row in 0..t_bits }`.
/// For each `(row, lane)` this function gives the logical element index.
///
/// ```text
/// o      = row / 8
/// s      = row % 8
/// result = (FL_ORDER[o] * 16) + (s * 128) + lane
/// ```
#[inline(always)]
pub fn fl_index(row: usize, lane: usize) -> usize {
    let o = row / 8;
    let s = row % 8;
    (FL_ORDER[o] * 16) + (s * 128) + lane
}

// ---------------------------------------------------------------------------
// unpack_all — full-block bit-unpack
// ---------------------------------------------------------------------------

/// Unpack `count` FastLanes-bit-packed values from `packed` into a `Vec<u64>`.
///
/// # Parameters
///
/// - `packed` — raw bytes of the packed buffer (`BitPackedData::packed()`).
/// - `bit_width` — bits per packed value `W` (1..=t_bits).
/// - `t_bits` — native type width in bits; must be `32` or `64`.
/// - `offset` — Vortex array `offset()` (0..1024); logical index 0 lives at
///   packed position `offset` within the first block (Pitfall 2: always add).
/// - `count` — number of logical values to decode.
///
/// # Returns
///
/// `Ok(Vec<u64>)` with exactly `count` elements in **logical order**.
/// All values are unsigned; the caller applies sign-extension or a FOR
/// reference after this call (Pitfall 4: unpack unsigned, then apply reference).
///
/// # Errors
///
/// - [`LoomDecodeError::BufferTooShort`] — `packed` is shorter than the minimum
///   required by the layout parameters (T-03-01 mitigation).
/// - [`LoomDecodeError::UnsupportedWidth`] — `t_bits` is not 32 or 64 (T-03-02).
/// - [`LoomDecodeError::BitWidthExceedsType`] — `bit_width > t_bits`.
pub fn unpack_all(
    packed: &[u8],
    bit_width: usize,
    t_bits: usize,
    offset: usize,
    count: usize,
) -> Result<Vec<u64>, LoomDecodeError> {
    // --- Validate t_bits (T-03-02) ---
    if t_bits != 32 && t_bits != 64 {
        return Err(LoomDecodeError::UnsupportedWidth(t_bits as u8));
    }

    // --- Validate bit_width ≤ t_bits (T-03-02) ---
    if bit_width > t_bits {
        return Err(LoomDecodeError::BitWidthExceedsType {
            bit_width: bit_width as u8,
            t_bits: t_bits as u8,
        });
    }

    // Handle zero-count trivially.
    if count == 0 {
        return Ok(Vec::new());
    }

    // Handle zero-width (all values are 0).
    if bit_width == 0 {
        return Ok(vec![0u64; count]);
    }

    let lanes = 1024 / t_bits;
    // elems_per_chunk = number of native-type words in one 1024-element block.
    // Source: `vortex-fastlanes` unpack_iter.rs `elems_per_chunk = 128 * bit_width / size_of::<T>()`
    // Using integer arithmetic to avoid floating-point: `128 * W / (t_bits/8)`.
    let elems_per_chunk = 128 * bit_width / (t_bits / 8);
    let byte_per_elem = t_bits / 8;

    // Number of 1024-element blocks needed to cover [0, offset+count).
    // div_ceil: `(offset + count + 1023) / 1024` (T-03-02: no overflow panic).
    let num_chunks = (offset + count).div_ceil(1024);

    // --- Buffer length validation (T-03-01) ---
    let required_bytes = num_chunks
        .checked_mul(elems_per_chunk)
        .and_then(|n| n.checked_mul(byte_per_elem))
        .ok_or(LoomDecodeError::BufferTooShort {
            needed: usize::MAX,
            got: packed.len(),
        })?;
    if packed.len() < required_bytes {
        return Err(LoomDecodeError::BufferTooShort {
            needed: required_bytes,
            got: packed.len(),
        });
    }

    // --- Unpack into logical-order output ---
    //
    // Allocate output indexed by logical position.
    let mut output = vec![0u64; count];

    let mask: u64 = if bit_width == 64 {
        u64::MAX
    } else {
        (1u64 << bit_width) - 1
    };

    for chunk_idx in 0..num_chunks {
        let chunk_bytes_start = chunk_idx * elems_per_chunk * byte_per_elem;

        for lane in 0..lanes {
            for row in 0..t_bits {
                // Logical index of this (row, lane) in the chunk.
                let logical_idx = fl_index(row, lane);
                let abs_logical = chunk_idx * 1024 + logical_idx;

                // Only decode values in [offset, offset+count).
                if abs_logical < offset || abs_logical >= offset + count {
                    continue;
                }

                // Bit position of this element in the packed block.
                let curr_word = (row * bit_width) / t_bits;
                let next_word = ((row + 1) * bit_width) / t_bits;
                let shift = (row * bit_width) % t_bits;

                // Helper: load a native-type word at word index `word_idx` for
                // this lane, from the current chunk.
                let load_word = |word_idx: usize| -> u64 {
                    let byte_off = chunk_bytes_start + (word_idx * lanes + lane) * byte_per_elem;
                    if t_bits == 32 {
                        u32::from_le_bytes(packed[byte_off..byte_off + 4].try_into().unwrap())
                            as u64
                    } else {
                        u64::from_le_bytes(packed[byte_off..byte_off + 8].try_into().unwrap())
                    }
                };

                // Extract the packed value, handling cross-word straddle.
                let val = if next_word > curr_word {
                    // Value straddles two words (non-byte-aligned width).
                    let remaining = ((row + 1) * bit_width) % t_bits;
                    let current_bits = bit_width - remaining;
                    let lo_mask: u64 = if current_bits == 64 {
                        u64::MAX
                    } else {
                        (1u64 << current_bits) - 1
                    };
                    let hi_mask: u64 = if remaining == 0 {
                        0u64
                    } else if remaining == 64 {
                        u64::MAX
                    } else {
                        (1u64 << remaining) - 1
                    };

                    let lo = (load_word(curr_word) >> shift) & lo_mask;
                    let hi = if remaining > 0 {
                        (load_word(next_word) & hi_mask) << current_bits
                    } else {
                        0
                    };
                    lo | hi
                } else {
                    // Single-word extract.
                    (load_word(curr_word) >> shift) & mask
                };

                output[abs_logical - offset] = val;
            }
        }
    }

    Ok(output)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Self-test: `fl_transpose_index(i)` over i=0..1024 must reproduce the same
    /// published formula independently (Wave-0 check #2 sanity).
    ///
    /// We verify that the function matches a local re-derivation of the formula
    /// so that any copy-paste error in the implementation is caught.
    #[test]
    fn transpose_matches_formula() {
        for idx in 0..1024usize {
            let lane = idx % 16;
            let order = (idx / 16) % 8;
            let row = idx / 128;
            let expected = (lane * 64) + (FL_ORDER[order] * 8) + row;
            let got = fl_transpose_index(idx);
            assert_eq!(
                got, expected,
                "fl_transpose_index({idx}): expected {expected}, got {got}"
            );
        }
    }

    /// fl_index and fl_transpose_index must form a consistent pair:
    /// for all (row, lane) pairs the round-trip property holds that
    /// fl_index covers the full set of logical indices [0, 1024).
    #[test]
    fn fl_index_covers_all_logical_indices() {
        let t_bits = 32usize;
        let lanes = 1024 / t_bits;
        let mut seen = vec![false; 1024];
        for row in 0..t_bits {
            for lane in 0..lanes {
                let logical = fl_index(row, lane);
                assert!(
                    logical < 1024,
                    "fl_index({row}, {lane}) = {logical} out of [0,1024)"
                );
                assert!(
                    !seen[logical],
                    "fl_index({row}, {lane}) = {logical} appeared twice"
                );
                seen[logical] = true;
            }
        }
        assert!(seen.iter().all(|&b| b), "not all 1024 indices were covered");
    }

    /// A too-short `packed` slice must return `BufferTooShort`, not panic (T-03-01).
    #[test]
    fn unpack_bounds_rejects_short_buffer() {
        // Request decoding 1025 values at bit_width=11, t_bits=32.
        // elems_per_chunk = 128*11/4 = 352; need 2 blocks → 2*352*4 = 2816 bytes.
        let packed = vec![0u8; 100]; // intentionally short
        let result = unpack_all(&packed, 11, 32, 0, 1025);
        assert!(
            matches!(result, Err(LoomDecodeError::BufferTooShort { .. })),
            "expected BufferTooShort, got {result:?}"
        );
    }

    /// Unsupported t_bits returns UnsupportedWidth (T-03-02).
    #[test]
    fn unpack_rejects_unsupported_t_bits() {
        let packed = vec![0u8; 64];
        let result = unpack_all(&packed, 8, 16, 0, 4);
        assert!(
            matches!(result, Err(LoomDecodeError::UnsupportedWidth(16))),
            "expected UnsupportedWidth(16), got {result:?}"
        );
    }

    /// bit_width > t_bits returns BitWidthExceedsType.
    #[test]
    fn unpack_rejects_bit_width_exceeding_t_bits() {
        let packed = vec![0u8; 64];
        let result = unpack_all(&packed, 33, 32, 0, 4);
        assert!(
            matches!(
                result,
                Err(LoomDecodeError::BitWidthExceedsType {
                    bit_width: 33,
                    t_bits: 32
                })
            ),
            "expected BitWidthExceedsType, got {result:?}"
        );
    }

    /// Zero-count unpack returns an empty Vec without accessing the buffer.
    #[test]
    fn unpack_zero_count_returns_empty() {
        let packed = vec![]; // empty buffer — must not be accessed
        let result = unpack_all(&packed, 11, 32, 0, 0);
        assert_eq!(result.unwrap(), Vec::<u64>::new());
    }

    /// Round-trip: pack small values and verify they unpack correctly.
    ///
    /// This uses the same `encode_test_values` helper from `l1_model::tests`
    /// to build a known buffer, then asserts `unpack_all` recovers the values.
    #[test]
    fn unpack_roundtrip_2bit_t32() {
        let values: Vec<u64> = vec![0, 1, 2, 3];
        let bit_width = 2usize;
        let t_bits = 32usize;
        let packed = encode_for_test(&values, bit_width, t_bits);

        let result = unpack_all(&packed, bit_width, t_bits, 0, values.len()).unwrap();
        assert_eq!(result.len(), values.len());
        for (i, (&expected, &got)) in values.iter().zip(result.iter()).enumerate() {
            assert_eq!(expected, got, "mismatch at logical index {i}");
        }
    }

    /// Round-trip for a larger set of values (not 1024-aligned).
    #[test]
    fn unpack_roundtrip_5bit_t32_not_aligned() {
        // 100 values, each 5 bits wide (values 0..31).
        let values: Vec<u64> = (0..100).map(|i: u64| i % 32).collect();
        let bit_width = 5usize;
        let t_bits = 32usize;
        let packed = encode_for_test(&values, bit_width, t_bits);

        let result = unpack_all(&packed, bit_width, t_bits, 0, values.len()).unwrap();
        assert_eq!(result.len(), values.len());
        for (i, (&expected, &got)) in values.iter().zip(result.iter()).enumerate() {
            assert_eq!(expected, got, "mismatch at logical index {i}");
        }
    }

    /// Round-trip for 11-bit width (non-byte-aligned, the canonical Phase 3 case).
    #[test]
    fn unpack_roundtrip_11bit_t32() {
        // 128 values (one full row per lane in a 32-lane block), values 0..2047.
        let values: Vec<u64> = (0..128).map(|i: u64| i % 2047).collect();
        let bit_width = 11usize;
        let t_bits = 32usize;
        let packed = encode_for_test(&values, bit_width, t_bits);

        let result = unpack_all(&packed, bit_width, t_bits, 0, values.len()).unwrap();
        assert_eq!(result.len(), values.len());
        for (i, (&expected, &got)) in values.iter().zip(result.iter()).enumerate() {
            assert_eq!(expected, got, "mismatch at logical index {i}");
        }
    }

    // -----------------------------------------------------------------------
    // Pack helper (test-only) — mirrors the inverse of unpack_all so round-trip
    // tests have a known-correct buffer to check against.
    // -----------------------------------------------------------------------

    /// Encode `values` into a FastLanes bit-packed buffer for testing purposes.
    /// Allocates full 1024-element blocks (padded with zeros).
    pub(crate) fn encode_for_test(values: &[u64], bit_width: usize, t_bits: usize) -> Vec<u8> {
        assert!(t_bits == 32 || t_bits == 64, "t_bits must be 32 or 64");
        assert!(bit_width <= t_bits, "bit_width must be <= t_bits");
        let lanes = 1024 / t_bits;
        let elems_per_chunk = 128 * bit_width / (t_bits / 8);
        let num_chunks = values.len().div_ceil(1024);
        let buf_bytes = num_chunks * elems_per_chunk * (t_bits / 8);
        let mut packed = vec![0u8; buf_bytes];

        for (logical_idx, &val) in values.iter().enumerate() {
            let chunk_idx = logical_idx / 1024;
            let idx_in_chunk = logical_idx % 1024;

            // Find (row, lane) such that fl_index(row, lane) == idx_in_chunk.
            let (found_row, found_lane) = find_pack_position(idx_in_chunk, lanes, t_bits);

            let curr_word = (found_row * bit_width) / t_bits;
            let next_word = ((found_row + 1) * bit_width) / t_bits;
            let shift = (found_row * bit_width) % t_bits;

            let chunk_bytes_start = chunk_idx * elems_per_chunk * (t_bits / 8);
            let byte_size = t_bits / 8;

            if next_word > curr_word {
                let remaining = ((found_row + 1) * bit_width) % t_bits;
                let current_bits = bit_width - remaining;
                let lo_mask: u64 = if current_bits == 64 {
                    u64::MAX
                } else {
                    (1u64 << current_bits) - 1
                };
                let hi_mask: u64 = if remaining == 64 {
                    u64::MAX
                } else if remaining == 0 {
                    0
                } else {
                    (1u64 << remaining) - 1
                };

                let lo = val & lo_mask;
                let curr_byte_off =
                    chunk_bytes_start + (curr_word * lanes + found_lane) * byte_size;
                or_word_le(&mut packed, curr_byte_off, t_bits, lo << shift);

                if remaining > 0 {
                    let hi = (val >> current_bits) & hi_mask;
                    let next_byte_off =
                        chunk_bytes_start + (next_word * lanes + found_lane) * byte_size;
                    or_word_le(&mut packed, next_byte_off, t_bits, hi);
                }
            } else {
                let curr_byte_off =
                    chunk_bytes_start + (curr_word * lanes + found_lane) * byte_size;
                let mask_all: u64 = if bit_width == 64 {
                    u64::MAX
                } else {
                    (1u64 << bit_width) - 1
                };
                or_word_le(
                    &mut packed,
                    curr_byte_off,
                    t_bits,
                    (val & mask_all) << shift,
                );
            }
        }

        packed
    }

    fn find_pack_position(idx_in_chunk: usize, lanes: usize, t_bits: usize) -> (usize, usize) {
        for row in 0..t_bits {
            for lane in 0..lanes {
                if fl_index(row, lane) == idx_in_chunk {
                    return (row, lane);
                }
            }
        }
        panic!("no position found for {idx_in_chunk}");
    }

    fn or_word_le(buf: &mut Vec<u8>, byte_off: usize, t_bits: usize, val: u64) {
        match t_bits {
            32 => {
                let existing = u32::from_le_bytes(buf[byte_off..byte_off + 4].try_into().unwrap());
                let new_val = existing | (val as u32);
                buf[byte_off..byte_off + 4].copy_from_slice(&new_val.to_le_bytes());
            }
            64 => {
                let existing = u64::from_le_bytes(buf[byte_off..byte_off + 8].try_into().unwrap());
                let new_val = existing | val;
                buf[byte_off..byte_off + 8].copy_from_slice(&new_val.to_le_bytes());
            }
            _ => panic!("unsupported t_bits {t_bits}"),
        }
    }
}
