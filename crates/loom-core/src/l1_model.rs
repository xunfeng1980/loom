//! L1 declarative layout model and synthesized read loop.
//!
//! This module owns:
//! - [`LayoutNode`] — the complete six-arm enum describing how a column's bytes
//!   are physically arranged (pure data, no executable code).
//! - [`LayoutDescription`] — top-level descriptor pairing a root [`LayoutNode`]
//!   with the target Arrow [`DataType`] and logical row count.
//! - [`synthesized_read_loop`] — a recursive match interpreter that decodes a
//!   [`LayoutNode`] tree by appending values/nulls into an [`OutputBuilder`].
//!
//! # Validity routing
//!
//! Validity (nulls) lives in [`LayoutNode::BitPack`], **not** in
//! [`LayoutNode::FrameOfReference`]. The FOR arm carries no validity field
//! because Vortex delegates validity to the inner [`LayoutNode::BitPack`] child
//! (see RESEARCH Pitfall 3 and `vortex-fastlanes` `ValidityChild<FoR>`). The
//! FOR arm reads `inner.validity` / `inner.all_null` when extracting the decoded
//! values; it does not add its own null routing.

pub mod bitpack;

use arrow_schema::DataType;

use crate::arrow_builder_output::OutputBuilder;
use crate::error::LoomDecodeError;

// ---------------------------------------------------------------------------
// LayoutNode — pure-data physical layout description (D-04)
// ---------------------------------------------------------------------------

/// A node in the physical layout tree for one Vortex column.
///
/// Every arm is pure data: no closures, no trait objects, no decoder code.
/// The [`synthesized_read_loop`] interprets this tree.
///
/// # Completeness (D-04)
///
/// All six arms are defined now so that downstream phases can fill in
/// `Dictionary`, `RunEnd`, and `KernelEscape` without changing the enum shape.
/// The three unimplemented arms return
/// [`LoomDecodeError::UnimplementedEncoding`] from the read loop rather than
/// panicking — the `catch_unwind` boundary in `loom-ffi` is never triggered by
/// normal unimplemented input.
#[derive(Debug, Clone)]
pub enum LayoutNode {
    /// Raw, unencoded values stored as little-endian bytes.
    ///
    /// Each logical element occupies exactly `elem_size` bytes. No bit packing,
    /// no reordering. The bytes in `data` are the direct representation.
    Raw {
        /// The raw byte buffer (little-endian, `elem_size` bytes per element).
        data: Vec<u8>,
        /// Bytes per element: 1, 2, 4, or 8.
        elem_size: u8,
        /// Number of logical elements.
        count: usize,
    },

    /// FastLanes-transposed bit-packed integers.
    ///
    /// Values are packed at `bit_width` bits each in the FastLanes 1024-element
    /// transposed layout. The raw packed bytes are stored in `values_buf`.
    ///
    /// `offset` is the Vortex array's `offset()` field (0..1024): logical index 0
    /// starts at packed position `offset` within the first 1024-element block.
    /// Always include `offset` when computing `index_to_decode` (Pitfall 2).
    ///
    /// Validity routing:
    /// - `all_null == true` → skip the unpack entirely, emit `count` nulls
    ///   (AllInvalid fast path, never reads `values_buf`).
    /// - `validity == Some(bits)` → per-row: `true` = valid, `false` = null.
    /// - `validity == None` → NonNullable / AllValid — no nulls.
    BitPack {
        /// Raw packed bytes from `BitPackedData::packed()`.
        values_buf: Vec<u8>,
        /// Bits per packed value (1..=64).
        bit_width: u8,
        /// Vortex array offset (start of logical index 0 in the first block).
        offset: u16,
        /// Number of logical values.
        count: usize,
        /// Per-row validity bitmap.
        ///
        /// `None` = NonNullable or AllValid (no nulls).
        /// `Some(bits)` = `true` if the row is valid, `false` if null.
        validity: Option<Vec<bool>>,
        /// If `true`, every row is null (Vortex `Validity::AllInvalid`).
        ///
        /// When `all_null` is set, `values_buf` must not be read.
        all_null: bool,
    },

    /// Frame-of-reference encoding: `decoded[i] = unpacked[i].wrapping_add(reference)`.
    ///
    /// Wraps a [`LayoutNode::BitPack`] inner node. The reference is stored as
    /// `i128` so both signed (`i32`, `i64`) and large unsigned (`u64`) references
    /// fit without truncation (anti-pattern note A3 in RESEARCH).
    ///
    /// # Validity
    ///
    /// **`FrameOfReference` carries NO validity field.** Validity lives in the
    /// inner `BitPack` node — Vortex's `ValidityChild<FoR>` delegates to the
    /// encoded child. The read loop reads `inner.validity` / `inner.all_null`
    /// directly (Pitfall 3).
    FrameOfReference {
        /// The reference scalar. Stored as `i128` to accommodate both signed and
        /// unsigned source types without narrowing (wrapping arithmetic applied at
        /// emit time).
        reference: i128,
        /// The inner encoded node (always `BitPack` for Phase 3).
        inner: Box<LayoutNode>,
    },

    /// Dictionary encoding: logical value at index `i` = `values[codes[i]]`.
    ///
    /// **Phase 3:** returns [`LoomDecodeError::UnimplementedEncoding`] from the
    /// read loop. Defined now so Phase 4 can fill the arm without a schema change.
    Dictionary {
        /// Integer codes array (any [`LayoutNode`]).
        codes: Box<LayoutNode>,
        /// Values array indexed by code (any [`LayoutNode`]).
        values: Box<LayoutNode>,
    },

    /// Run-end encoding: `count` logical values stored as (run_ends, values) pairs.
    ///
    /// **Phase 3:** returns [`LoomDecodeError::UnimplementedEncoding`] from the
    /// read loop.
    RunEnd {
        /// Monotonically increasing run-end positions.
        run_ends: Box<LayoutNode>,
        /// One value per run.
        values: Box<LayoutNode>,
        /// Total logical element count (needed because the last run-end may not
        /// equal the count).
        count: usize,
    },

    /// L1→L2 escape: delegates to a registered L2 kernel by stable integer ID.
    ///
    /// **Phase 3:** returns [`LoomDecodeError::UnimplementedEncoding`] from the
    /// read loop. The `KernelEscape` arm is the only place L2 code runs —
    /// its presence here makes the L1/L2 split visible in the data model.
    KernelEscape {
        /// Stable kernel identifier (indexes into `L2KernelRegistry`).
        kernel_id: u32,
        /// Serialized kernel-specific parameters (e.g. FSST symbol-table bytes).
        params: Vec<u8>,
        /// Total logical element count produced by the kernel.
        count: usize,
    },
}

// ---------------------------------------------------------------------------
// LayoutDescription — top-level descriptor
// ---------------------------------------------------------------------------

/// Top-level descriptor for one Vortex column.
///
/// Pairs the physical layout tree ([`root`](LayoutDescription::root)) with the
/// target Arrow [`DataType`] and the logical row count.
#[derive(Debug, Clone)]
pub struct LayoutDescription {
    /// Arrow data type for the [`OutputBuilder`] constructor.
    pub data_type: DataType,
    /// Root of the physical layout tree.
    pub root: LayoutNode,
    /// Total logical row count.
    pub row_count: usize,
}

// ---------------------------------------------------------------------------
// synthesized_read_loop — the recursive match interpreter
// ---------------------------------------------------------------------------

/// Decode a [`LayoutNode`] tree, appending values and nulls into `builder`.
///
/// Returns `Ok(())` on success, or a [`LoomDecodeError`] for:
/// - `UnimplementedEncoding` — one of the three not-yet-implemented arms
///   (`Dictionary`, `RunEnd`, `KernelEscape`).
/// - `BufferTooShort` / `UnsupportedWidth` — malformed input detected by the
///   bit-unpack path.
///
/// # Validity routing
///
/// Nulls are routed through [`OutputBuilder::append_null`]; the Arrow builder
/// records them in the null bitmap automatically. No caller needs to manage the
/// bitmap directly (ARROW-01).
///
/// # FrameOfReference validity note (Pitfall 3)
///
/// The `FrameOfReference` arm does **not** apply nulls itself. Validity lives in
/// the inner `BitPack` node. The FOR arm decodes inner-BitPack values with their
/// validity already applied (via the `BitPack` arm logic), then broadcasts-adds
/// the reference scalar.
pub fn synthesized_read_loop(
    node: &LayoutNode,
    builder: &mut OutputBuilder,
) -> Result<(), LoomDecodeError> {
    match node {
        // ----------------------------------------------------------------
        // Raw: little-endian values, `elem_size` bytes each.
        // ----------------------------------------------------------------
        LayoutNode::Raw { data, elem_size, count } => {
            decode_raw(data, *elem_size, *count, builder)
        }

        // ----------------------------------------------------------------
        // BitPack: FastLanes transposed bit-packing with validity routing.
        // ----------------------------------------------------------------
        LayoutNode::BitPack {
            values_buf,
            bit_width,
            offset,
            count,
            validity,
            all_null,
        } => {
            decode_bitpack(
                values_buf,
                *bit_width,
                *offset,
                *count,
                validity.as_deref(),
                *all_null,
                builder,
            )
        }

        // ----------------------------------------------------------------
        // FrameOfReference: wrapping-add the reference after inner decode.
        //
        // Validity lives in the inner BitPack node (Pitfall 3). This arm
        // does not carry a validity field.
        // ----------------------------------------------------------------
        LayoutNode::FrameOfReference { reference, inner } => {
            decode_for(*reference, inner, builder)
        }

        // ----------------------------------------------------------------
        // Deferred arms — return a typed error, never panic (D-04, T-03-03).
        // ----------------------------------------------------------------
        LayoutNode::Dictionary { .. } => {
            Err(LoomDecodeError::UnimplementedEncoding("Dictionary"))
        }
        LayoutNode::RunEnd { .. } => {
            Err(LoomDecodeError::UnimplementedEncoding("RunEnd"))
        }
        LayoutNode::KernelEscape { .. } => {
            Err(LoomDecodeError::UnimplementedEncoding("KernelEscape"))
        }
    }
}

// ---------------------------------------------------------------------------
// Private decode helpers (filled in Task 3)
// ---------------------------------------------------------------------------

/// Decode a `Raw` node.
fn decode_raw(
    data: &[u8],
    elem_size: u8,
    count: usize,
    builder: &mut OutputBuilder,
) -> Result<(), LoomDecodeError> {
    let stride = elem_size as usize;
    let needed = count * stride;
    if data.len() < needed {
        return Err(LoomDecodeError::BufferTooShort {
            needed,
            got: data.len(),
        });
    }
    for i in 0..count {
        let bytes = &data[i * stride..(i + 1) * stride];
        match elem_size {
            4 => {
                let v = i32::from_le_bytes(bytes.try_into().unwrap());
                builder.append_i32(v);
            }
            8 => {
                let v = i64::from_le_bytes(bytes.try_into().unwrap());
                builder.append_i64(v);
            }
            _ => {
                // For other widths, attempt i32 promotion.
                // Widen to 4-byte signed little-endian for i32 builders.
                let v = match elem_size {
                    1 => i8::from_le_bytes([bytes[0]]) as i32,
                    2 => i16::from_le_bytes(bytes.try_into().unwrap()) as i32,
                    _ => {
                        return Err(LoomDecodeError::UnsupportedWidth(elem_size));
                    }
                };
                builder.append_i32(v);
            }
        }
    }
    Ok(())
}

/// Decode a `BitPack` node with validity routing.
fn decode_bitpack(
    values_buf: &[u8],
    bit_width: u8,
    offset: u16,
    count: usize,
    validity: Option<&[bool]>,
    all_null: bool,
    builder: &mut OutputBuilder,
) -> Result<(), LoomDecodeError> {
    // AllInvalid fast path: skip unpack entirely, emit count nulls (Pitfall / anti-pattern note).
    if all_null {
        for _ in 0..count {
            builder.append_null();
        }
        return Ok(());
    }

    // Determine native type width from the builder kind.
    let t_bits: usize = builder.t_bits();

    // Unpack unsigned values; caller casts / sign-extends after FOR add (Pitfall 4).
    let unpacked = bitpack::unpack_all(
        values_buf,
        bit_width as usize,
        t_bits,
        offset as usize,
        count,
    )?;

    // Append with validity routing.
    match validity {
        None => {
            // NonNullable / AllValid — no nulls.
            for val in &unpacked {
                match t_bits {
                    32 => builder.append_i32(*val as i32),
                    64 => builder.append_i64(*val as i64),
                    _ => unreachable!(),
                }
            }
        }
        Some(bits) => {
            for (i, val) in unpacked.iter().enumerate() {
                if bits.get(i).copied().unwrap_or(false) {
                    match t_bits {
                        32 => builder.append_i32(*val as i32),
                        64 => builder.append_i64(*val as i64),
                        _ => unreachable!(),
                    }
                } else {
                    builder.append_null();
                }
            }
        }
    }

    Ok(())
}

/// Decode a `FrameOfReference` node.
///
/// Reads validity from the inner `BitPack` node; the FOR arm itself has no
/// validity field (Pitfall 3, RESEARCH Q3 "FoRArray Validity").
fn decode_for(
    reference: i128,
    inner: &LayoutNode,
    builder: &mut OutputBuilder,
) -> Result<(), LoomDecodeError> {
    // The inner node must be a BitPack (Phase 3 contract).
    let (values_buf, bit_width, offset, count, validity, all_null) = match inner {
        LayoutNode::BitPack {
            values_buf,
            bit_width,
            offset,
            count,
            validity,
            all_null,
        } => (values_buf, *bit_width, *offset, *count, validity.as_deref(), *all_null),
        _ => {
            // Non-BitPack inner: apply the full loop (supports nested FOR trees).
            // For Phase 3 this path is unreachable in practice; delegate for correctness.
            return synthesized_read_loop(inner, builder);
        }
    };

    // AllInvalid fast path: skip unpack, emit nulls.
    if all_null {
        for _ in 0..count {
            builder.append_null();
        }
        return Ok(());
    }

    let t_bits: usize = builder.t_bits();

    let unpacked = bitpack::unpack_all(
        values_buf,
        bit_width as usize,
        t_bits,
        offset as usize,
        count,
    )?;

    // Broadcast-add the reference with wrapping arithmetic (Pitfall 4):
    // packed values are always unsigned; adding the signed reference (which may
    // be negative, i.e. the FOR minimum) produces the signed final value.
    match validity {
        None => {
            for val in &unpacked {
                match t_bits {
                    32 => {
                        let result = (*val as i128).wrapping_add(reference) as i32;
                        builder.append_i32(result);
                    }
                    64 => {
                        let result = (*val as i128).wrapping_add(reference) as i64;
                        builder.append_i64(result);
                    }
                    _ => unreachable!(),
                }
            }
        }
        Some(bits) => {
            for (i, val) in unpacked.iter().enumerate() {
                if bits.get(i).copied().unwrap_or(false) {
                    match t_bits {
                        32 => {
                            let result = (*val as i128).wrapping_add(reference) as i32;
                            builder.append_i32(result);
                        }
                        64 => {
                            let result = (*val as i128).wrapping_add(reference) as i64;
                            builder.append_i64(result);
                        }
                        _ => unreachable!(),
                    }
                } else {
                    builder.append_null();
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arrow_builder_output::OutputBuilder;

    use arrow::array::Array;

    // Helper: build a dummy OutputBuilder for Int32.
    fn int32_builder() -> OutputBuilder {
        OutputBuilder::new(&DataType::Int32)
    }

    /// The three deferred arms must return a typed UnimplementedEncoding error,
    /// never panic (D-04, T-03-03).
    #[test]
    fn unimplemented_arms_return_typed_error() {
        // Dictionary
        let dict = LayoutNode::Dictionary {
            codes: Box::new(LayoutNode::Raw {
                data: vec![],
                elem_size: 4,
                count: 0,
            }),
            values: Box::new(LayoutNode::Raw {
                data: vec![],
                elem_size: 4,
                count: 0,
            }),
        };
        let mut b = int32_builder();
        let result = synthesized_read_loop(&dict, &mut b);
        assert!(
            matches!(result, Err(LoomDecodeError::UnimplementedEncoding("Dictionary"))),
            "expected Dictionary error, got {result:?}"
        );

        // RunEnd
        let run_end = LayoutNode::RunEnd {
            run_ends: Box::new(LayoutNode::Raw {
                data: vec![],
                elem_size: 4,
                count: 0,
            }),
            values: Box::new(LayoutNode::Raw {
                data: vec![],
                elem_size: 4,
                count: 0,
            }),
            count: 0,
        };
        let mut b = int32_builder();
        let result = synthesized_read_loop(&run_end, &mut b);
        assert!(
            matches!(result, Err(LoomDecodeError::UnimplementedEncoding("RunEnd"))),
            "expected RunEnd error, got {result:?}"
        );

        // KernelEscape
        let escape = LayoutNode::KernelEscape {
            kernel_id: 0,
            params: vec![],
            count: 0,
        };
        let mut b = int32_builder();
        let result = synthesized_read_loop(&escape, &mut b);
        assert!(
            matches!(result, Err(LoomDecodeError::UnimplementedEncoding("KernelEscape"))),
            "expected KernelEscape error, got {result:?}"
        );
    }

    /// A Raw node with zero elements decodes successfully.
    #[test]
    fn raw_empty_succeeds() {
        let node = LayoutNode::Raw {
            data: vec![],
            elem_size: 4,
            count: 0,
        };
        let mut b = int32_builder();
        assert!(synthesized_read_loop(&node, &mut b).is_ok());
        let data = b.finish();
        assert_eq!(data.len(), 0);
    }

    /// A Raw node with known i32 values decodes correctly.
    #[test]
    fn raw_i32_decodes_values() {
        let values: Vec<i32> = vec![1, -2, 3, -4];
        let data: Vec<u8> = values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let node = LayoutNode::Raw {
            data,
            elem_size: 4,
            count: 4,
        };
        let mut b = int32_builder();
        synthesized_read_loop(&node, &mut b).unwrap();
        let array_data = b.finish();
        assert_eq!(array_data.len(), 4);
        assert_eq!(array_data.null_count(), 0);
        let array = arrow::array::Int32Array::from(array_data);
        for (i, expected) in values.iter().enumerate() {
            assert_eq!(array.value(i), *expected, "mismatch at index {i}");
        }
    }

    /// BitPack with all_null=true emits all nulls without touching values_buf.
    #[test]
    fn bitpack_all_null_emits_all_nulls() {
        // Passing an empty values_buf with all_null=true must succeed — the
        // buffer must never be accessed (AllInvalid fast path).
        let node = LayoutNode::BitPack {
            values_buf: vec![], // empty — would panic if indexed
            bit_width: 11,
            offset: 0,
            count: 5,
            validity: None,
            all_null: true,
        };
        let mut b = int32_builder();
        synthesized_read_loop(&node, &mut b).unwrap();
        let data = b.finish();
        assert_eq!(data.len(), 5);
        assert_eq!(data.null_count(), 5, "all rows must be null");
    }

    /// BitPack with per-row validity routes nulls correctly.
    #[test]
    fn bitpack_per_row_validity_routes_nulls() {
        // Build a 2-bit packed buffer for values [0, 1, 2, 3] using t_bits=32.
        // 2 bits per value, 4 values → need 1 block of 1024 elements (padded with zeros).
        // elems_per_chunk = 128 * 2 / 4 = 64 u32 elements.
        // We'll use bit_width=2 which is simpler to hand-construct.
        // For a 2-bit pack with values 0,1,2,3 in the FastLanes layout, we
        // use a 1024-element block; the first 4 logical positions carry our values.
        //
        // The easiest approach: construct the buffer by calling unpack in reverse
        // (hand-pack via known layout). Since this is complex, we use a minimal
        // fixture that we know will produce specific output via our own unpack_all.
        //
        // Simpler: construct a Raw node and test validity routing there instead,
        // since validity routing logic is the same regardless of encoding.
        // Per-row validity on a Raw node exercises the same path in the builder.
        // For the BitPack path specifically, we test with a known packed buffer.
        //
        // For a clean in-core test, we hand-construct a 1-bit packed buffer
        // with values all 0 or 1, then check that validity correctly marks
        // specific positions as null regardless of the unpacked value.
        //
        // We use 2-bit width (values 0–3) for 4 logical elements in one block.
        // Construct packed bytes for the known formula.
        let count = 4usize;
        let bit_width: u8 = 2;
        let t_bits = 32usize;

        // Encode values [1, 0, 3, 2] manually using our bitpack formula.
        let packed = encode_test_values(&[1u64, 0, 3, 2], bit_width as usize, t_bits);

        let validity = vec![true, false, true, false];
        let node = LayoutNode::BitPack {
            values_buf: packed,
            bit_width,
            offset: 0,
            count,
            validity: Some(validity),
            all_null: false,
        };

        let mut b = int32_builder();
        synthesized_read_loop(&node, &mut b).unwrap();
        let data = b.finish();
        assert_eq!(data.len(), 4);
        // positions 0 and 2 are valid; positions 1 and 3 are null
        assert_eq!(data.null_count(), 2);
        let array = arrow::array::Int32Array::from(data);
        assert!(!array.is_null(0));
        assert!(array.is_null(1));
        assert!(!array.is_null(2));
        assert!(array.is_null(3));
        assert_eq!(array.value(0), 1);
        assert_eq!(array.value(2), 3);
    }

    /// FOR with a negative reference applies wrapping_add correctly.
    #[test]
    fn for_wrapping_add_with_negative_reference() {
        // Values to encode: [-500, -499, -498, -497]
        // We store deltas above a reference of -500:
        // deltas = [0, 1, 2, 3]
        // decoded[i] = delta[i] + reference = delta[i] + (-500)
        let count = 4usize;
        let bit_width: u8 = 2; // 2 bits fits values 0–3
        let t_bits = 32usize;
        let reference: i128 = -500;

        let deltas: Vec<u64> = vec![0, 1, 2, 3];
        let packed = encode_test_values(&deltas, bit_width as usize, t_bits);

        let inner = LayoutNode::BitPack {
            values_buf: packed,
            bit_width,
            offset: 0,
            count,
            validity: None,
            all_null: false,
        };

        let node = LayoutNode::FrameOfReference {
            reference,
            inner: Box::new(inner),
        };

        let mut b = int32_builder();
        synthesized_read_loop(&node, &mut b).unwrap();
        let data = b.finish();
        assert_eq!(data.len(), 4);
        assert_eq!(data.null_count(), 0);
        let array = arrow::array::Int32Array::from(data);
        for (i, delta) in deltas.iter().enumerate() {
            let expected = ((*delta as i128) + reference) as i32;
            assert_eq!(array.value(i), expected, "mismatch at index {i}");
        }
    }

    // -----------------------------------------------------------------------
    // Test helper: pack a small Vec<u64> into a FastLanes bit-packed buffer.
    // Used only in unit tests; mirrors the pack logic exactly so we have
    // a known-correct buffer to test against.
    // -----------------------------------------------------------------------

    /// Pack `values` (each fitting in `bit_width` bits) into a FastLanes
    /// transposed buffer with native type width `t_bits` (32 or 64).
    ///
    /// Allocates a full 1024-element block (padded with zeros). Only
    /// `values.len()` logical elements are stored; the rest are zero-padded.
    #[cfg(test)]
    fn encode_test_values(values: &[u64], bit_width: usize, t_bits: usize) -> Vec<u8> {
        let lanes = 1024 / t_bits;
        let elems_per_chunk = 128 * bit_width / (t_bits / 8);
        // One full 1024-element block.
        let buf_bytes = elems_per_chunk * (t_bits / 8);
        let mut packed = vec![0u8; buf_bytes];

        for (logical_idx, &val) in values.iter().enumerate() {
            // Find (lane, row) pair such that fl_index(row, lane) == logical_idx.
            // Since fl_index is a bijection over [0, 1024), we can find it by search.
            // For a small test vector this is fine.
            let (found_row, found_lane) = find_packed_position(logical_idx, lanes, t_bits);

            let curr_word = (found_row * bit_width) / t_bits;
            let next_word = ((found_row + 1) * bit_width) / t_bits;
            let shift = (found_row * bit_width) % t_bits;

            let byte_size = t_bits / 8;
            let curr_byte_off = (curr_word * lanes + found_lane) * byte_size;

            if next_word > curr_word {
                let remaining = ((found_row + 1) * bit_width) % t_bits;
                let current_bits = bit_width - remaining;
                let lo_mask: u64 = if current_bits == 64 { u64::MAX } else { (1u64 << current_bits) - 1 };
                let hi_mask: u64 = if remaining == 64 { u64::MAX } else { (1u64 << remaining) - 1 };

                // Write low bits into curr_word.
                let lo = val & lo_mask;
                set_word_le(&mut packed, curr_byte_off, t_bits, lo << shift);

                // Write high bits into next_word.
                let hi = (val >> current_bits) & hi_mask;
                let next_byte_off = (next_word * lanes + found_lane) * byte_size;
                set_word_le(&mut packed, next_byte_off, t_bits, hi);
            } else {
                set_word_le(&mut packed, curr_byte_off, t_bits, val << shift);
            }
        }

        packed
    }

    /// Find (row, lane) such that `fl_index(row, lane) == logical_idx`.
    fn find_packed_position(logical_idx: usize, lanes: usize, t_bits: usize) -> (usize, usize) {
        for row in 0..t_bits {
            for lane in 0..lanes {
                if bitpack::fl_index(row, lane) == logical_idx {
                    return (row, lane);
                }
            }
        }
        // This function is only ever called from test code with valid logical indices
        // in [0, 1024). fl_index is a bijection so every index in [0, LANES*t_bits)
        // is covered; reaching this point indicates a test bug.
        unreachable!("no packed position found for logical_idx {logical_idx} (test-only helper)")
    }

    /// OR a little-endian word into `buf` at `byte_off`.
    fn set_word_le(buf: &mut Vec<u8>, byte_off: usize, t_bits: usize, val: u64) {
        match t_bits {
            32 => {
                let existing = u32::from_le_bytes(
                    buf[byte_off..byte_off + 4].try_into().unwrap()
                );
                let new = existing | (val as u32);
                buf[byte_off..byte_off + 4].copy_from_slice(&new.to_le_bytes());
            }
            64 => {
                let existing = u64::from_le_bytes(
                    buf[byte_off..byte_off + 8].try_into().unwrap()
                );
                let new = existing | val;
                buf[byte_off..byte_off + 8].copy_from_slice(&new.to_le_bytes());
            }
            _ => unreachable!("unsupported t_bits {t_bits} (test-only helper)"),
        }
    }
}
