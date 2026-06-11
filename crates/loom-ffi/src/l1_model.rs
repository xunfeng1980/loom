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

use arrow::array::{
    Array, BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array, StringArray,
};
use arrow_data::ArrayData;
use arrow_schema::DataType;

use crate::arrow_builder_output::OutputBuilder;
use loom_ir_core::error::LoomDecodeError;
use crate::l2_kernel_registry::L2KernelRegistry;
use crate::verify_layout_types::verify_layout;

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
    synthesized_read_loop_with_registry(node, builder, None)
}

fn synthesized_read_loop_with_registry(
    node: &LayoutNode,
    builder: &mut OutputBuilder,
    registry: Option<&L2KernelRegistry>,
) -> Result<(), LoomDecodeError> {
    match node {
        // ----------------------------------------------------------------
        // Raw: little-endian values, `elem_size` bytes each.
        // ----------------------------------------------------------------
        LayoutNode::Raw {
            data,
            elem_size,
            count,
        } => decode_raw(data, *elem_size, *count, builder),

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
        } => decode_bitpack(
            values_buf,
            *bit_width,
            *offset,
            *count,
            validity.as_deref(),
            *all_null,
            builder,
        ),

        // ----------------------------------------------------------------
        // FrameOfReference: wrapping-add the reference after inner decode.
        //
        // Validity lives in the inner BitPack node (Pitfall 3). This arm
        // does not carry a validity field.
        // ----------------------------------------------------------------
        LayoutNode::FrameOfReference { reference, inner } => {
            decode_for(*reference, inner, builder, registry)
        }

        // ----------------------------------------------------------------
        // Deferred arms — return a typed error, never panic (D-04, T-03-03).
        // ----------------------------------------------------------------
        LayoutNode::Dictionary { codes, values } => {
            decode_dictionary(codes, values, builder, registry)
        }
        LayoutNode::RunEnd {
            run_ends,
            values,
            count,
        } => decode_run_end(run_ends, values, *count, builder, registry),
        LayoutNode::KernelEscape { .. } => {
            Err(LoomDecodeError::UnimplementedEncoding("KernelEscape"))
        }
    }
}

/// Decode a top-level layout into Arrow [`ArrayData`].
///
/// Builder-backed L1 nodes flow through [`synthesized_read_loop`]. A top-level
/// [`LayoutNode::KernelEscape`] is different: its L2 kernel owns the output
/// array, so this helper dispatches through [`L2KernelRegistry`] and returns the
/// kernel-produced `ArrayData` directly.
pub fn decode_layout_to_array_data(
    desc: &LayoutDescription,
    registry: &L2KernelRegistry,
) -> Result<ArrayData, LoomDecodeError> {
    let report = verify_layout(desc, registry);
    if let Some(err) = report.first_error() {
        return Err(err);
    }
    decode_node_to_array_data_with_registry(&desc.root, &desc.data_type, Some(registry))
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
    // Mirror `bitpack::unpack_all`: guard the size computation so a crafted
    // `count` cannot overflow, wrap to a small value, slip past the bounds
    // check below, and panic on slice indexing. Overflow => BufferTooShort.
    let needed = count
        .checked_mul(stride)
        .ok_or(LoomDecodeError::BufferTooShort {
            needed: usize::MAX,
            got: data.len(),
        })?;
    if data.len() < needed {
        return Err(LoomDecodeError::BufferTooShort {
            needed,
            got: data.len(),
        });
    }
    for i in 0..count {
        let bytes = &data[i * stride..(i + 1) * stride];
        match builder.data_type() {
            DataType::Boolean => {
                if elem_size != 1 {
                    return Err(LoomDecodeError::UnsupportedBuilderType {
                        operation: "decode_raw boolean",
                        data_type: data_type_name(&DataType::Boolean),
                    });
                }
                builder.append_bool(bytes[0] != 0);
            }
            DataType::Int32 => {
                let v = match elem_size {
                    1 => i8::from_le_bytes([bytes[0]]) as i32,
                    2 => i16::from_le_bytes(bytes.try_into().unwrap()) as i32,
                    4 => i32::from_le_bytes(bytes.try_into().unwrap()),
                    _ => return Err(LoomDecodeError::UnsupportedWidth(elem_size)),
                };
                builder.append_i32(v);
            }
            DataType::Int64 => {
                let v = match elem_size {
                    1 => i8::from_le_bytes([bytes[0]]) as i64,
                    2 => i16::from_le_bytes(bytes.try_into().unwrap()) as i64,
                    4 => i32::from_le_bytes(bytes.try_into().unwrap()) as i64,
                    8 => i64::from_le_bytes(bytes.try_into().unwrap()),
                    _ => return Err(LoomDecodeError::UnsupportedWidth(elem_size)),
                };
                builder.append_i64(v);
            }
            DataType::Float32 => {
                if elem_size != 4 {
                    return Err(LoomDecodeError::UnsupportedWidth(elem_size));
                }
                builder.append_f32(f32::from_le_bytes(bytes.try_into().unwrap()));
            }
            DataType::Float64 => {
                if elem_size != 8 {
                    return Err(LoomDecodeError::UnsupportedWidth(elem_size));
                }
                builder.append_f64(f64::from_le_bytes(bytes.try_into().unwrap()));
            }
            other => {
                return Err(LoomDecodeError::UnsupportedBuilderType {
                    operation: "decode_raw",
                    data_type: data_type_name(&other),
                });
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
    registry: Option<&L2KernelRegistry>,
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
        } => (
            values_buf,
            *bit_width,
            *offset,
            *count,
            validity.as_deref(),
            *all_null,
        ),
        _ => {
            // Non-BitPack inner: decode the child first, then apply the same
            // reference broadcast while preserving the child's nulls. Recursive
            // dict/RLE paths in Phase 4 make this reachable.
            let dtype = builder.data_type();
            let data = decode_node_to_array_data_with_registry(inner, &dtype, registry)?;
            let decoded = DecodedArray::from_array_data(data, &dtype)?;
            for i in 0..decoded.len() {
                decoded.append_value_plus_reference(i, reference, builder)?;
            }
            return Ok(());
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

fn decode_dictionary(
    codes: &LayoutNode,
    values: &LayoutNode,
    builder: &mut OutputBuilder,
    registry: Option<&L2KernelRegistry>,
) -> Result<(), LoomDecodeError> {
    let codes_dtype = dictionary_code_data_type(codes);
    let codes_data = decode_node_to_array_data_with_registry(codes, &codes_dtype, registry)?;
    let codes = DecodedArray::from_array_data(codes_data, &codes_dtype)?;

    let values_dtype = builder.data_type();
    let values_data = decode_node_to_array_data_with_registry(values, &values_dtype, registry)?;
    let values = DecodedArray::from_array_data(values_data, &values_dtype)?;

    for row in 0..codes.len() {
        let Some(code) = codes.value_as_i64(row)? else {
            builder.append_null();
            continue;
        };
        if code < 0 || code as usize >= values.len() {
            return Err(LoomDecodeError::InvalidDictionaryCode {
                index: row,
                code,
                values_len: values.len(),
            });
        }
        values.append_value_to_builder(code as usize, builder)?;
    }

    Ok(())
}

fn decode_run_end(
    run_ends: &LayoutNode,
    values: &LayoutNode,
    count: usize,
    builder: &mut OutputBuilder,
    registry: Option<&L2KernelRegistry>,
) -> Result<(), LoomDecodeError> {
    let run_ends_data =
        decode_node_to_array_data_with_registry(run_ends, &DataType::Int64, registry)?;
    let run_ends = DecodedArray::from_array_data(run_ends_data, &DataType::Int64)?;

    let values_dtype = builder.data_type();
    let values_data = decode_node_to_array_data_with_registry(values, &values_dtype, registry)?;
    let values = DecodedArray::from_array_data(values_data, &values_dtype)?;

    let mut previous = 0usize;
    for run_idx in 0..run_ends.len() {
        let Some(run_end) = run_ends.value_as_i64(run_idx)? else {
            return Err(LoomDecodeError::NonMonotonicRunEnd {
                index: run_idx,
                previous,
                current: previous,
            });
        };
        if run_end <= previous as i64 {
            return Err(LoomDecodeError::NonMonotonicRunEnd {
                index: run_idx,
                previous,
                current: run_end.max(0) as usize,
            });
        }
        let current = run_end as usize;
        if current > count {
            return Err(LoomDecodeError::RunEndOutOfBounds {
                index: run_idx,
                run_end: current,
                count,
            });
        }
        if run_idx >= values.len() {
            return Err(LoomDecodeError::InsufficientRunValues {
                run_index: run_idx,
                values_len: values.len(),
            });
        }
        for _ in previous..current {
            values.append_value_to_builder(run_idx, builder)?;
        }
        previous = current;
    }

    if previous != count {
        return Err(LoomDecodeError::RunEndTooShort {
            last_run_end: previous,
            count,
        });
    }

    Ok(())
}

fn decode_node_to_array_data_with_registry(
    node: &LayoutNode,
    data_type: &DataType,
    registry: Option<&L2KernelRegistry>,
) -> Result<ArrayData, LoomDecodeError> {
    if let LayoutNode::KernelEscape {
        kernel_id,
        params,
        count,
    } = node
    {
        let registry = registry.ok_or(LoomDecodeError::UnimplementedEncoding("KernelEscape"))?;
        let kernel = registry
            .get(*kernel_id)
            .ok_or(LoomDecodeError::UnknownKernel(*kernel_id))?;
        return kernel.decode(params, *count);
    }

    let mut builder = OutputBuilder::new(data_type);
    synthesized_read_loop_with_registry(node, &mut builder, registry)?;
    Ok(builder.finish())
}

fn dictionary_code_data_type(codes: &LayoutNode) -> DataType {
    match codes {
        LayoutNode::Raw { elem_size: 8, .. } => DataType::Int64,
        _ => DataType::Int32,
    }
}

enum DecodedArray {
    Boolean(BooleanArray),
    Int32(Int32Array),
    Int64(Int64Array),
    Float32(Float32Array),
    Float64(Float64Array),
    Utf8(StringArray),
}

impl DecodedArray {
    fn from_array_data(data: ArrayData, data_type: &DataType) -> Result<Self, LoomDecodeError> {
        match data_type {
            DataType::Boolean => Ok(DecodedArray::Boolean(BooleanArray::from(data))),
            DataType::Int32 => Ok(DecodedArray::Int32(Int32Array::from(data))),
            DataType::Int64 => Ok(DecodedArray::Int64(Int64Array::from(data))),
            DataType::Float32 => Ok(DecodedArray::Float32(Float32Array::from(data))),
            DataType::Float64 => Ok(DecodedArray::Float64(Float64Array::from(data))),
            DataType::Utf8 => Ok(DecodedArray::Utf8(StringArray::from(data))),
            other => Err(LoomDecodeError::UnsupportedBuilderType {
                operation: "materialize decoded child",
                data_type: data_type_name(other),
            }),
        }
    }

    fn len(&self) -> usize {
        match self {
            DecodedArray::Boolean(a) => a.len(),
            DecodedArray::Int32(a) => a.len(),
            DecodedArray::Int64(a) => a.len(),
            DecodedArray::Float32(a) => a.len(),
            DecodedArray::Float64(a) => a.len(),
            DecodedArray::Utf8(a) => a.len(),
        }
    }

    fn is_null(&self, index: usize) -> bool {
        match self {
            DecodedArray::Boolean(a) => a.is_null(index),
            DecodedArray::Int32(a) => a.is_null(index),
            DecodedArray::Int64(a) => a.is_null(index),
            DecodedArray::Float32(a) => a.is_null(index),
            DecodedArray::Float64(a) => a.is_null(index),
            DecodedArray::Utf8(a) => a.is_null(index),
        }
    }

    fn value_as_i64(&self, index: usize) -> Result<Option<i64>, LoomDecodeError> {
        if self.is_null(index) {
            return Ok(None);
        }
        match self {
            DecodedArray::Int32(a) => Ok(Some(a.value(index) as i64)),
            DecodedArray::Int64(a) => Ok(Some(a.value(index))),
            DecodedArray::Boolean(_)
            | DecodedArray::Float32(_)
            | DecodedArray::Float64(_)
            | DecodedArray::Utf8(_) => Err(LoomDecodeError::UnsupportedBuilderType {
                operation: "read integer code",
                data_type: data_type_name(&self.data_type()),
            }),
        }
    }

    fn append_value_to_builder(
        &self,
        index: usize,
        builder: &mut OutputBuilder,
    ) -> Result<(), LoomDecodeError> {
        if self.is_null(index) {
            builder.append_null();
            return Ok(());
        }
        match (self, builder.data_type()) {
            (DecodedArray::Boolean(a), DataType::Boolean) => builder.append_bool(a.value(index)),
            (DecodedArray::Int32(a), DataType::Int32) => builder.append_i32(a.value(index)),
            (DecodedArray::Int64(a), DataType::Int64) => builder.append_i64(a.value(index)),
            (DecodedArray::Float32(a), DataType::Float32) => builder.append_f32(a.value(index)),
            (DecodedArray::Float64(a), DataType::Float64) => builder.append_f64(a.value(index)),
            (DecodedArray::Utf8(a), DataType::Utf8) => builder.append_string(a.value(index)),
            (DecodedArray::Int32(a), DataType::Int64) => builder.append_i64(a.value(index) as i64),
            (DecodedArray::Int64(a), DataType::Int32) => builder.append_i32(a.value(index) as i32),
            (DecodedArray::Float32(a), DataType::Float64) => {
                builder.append_f64(a.value(index) as f64)
            }
            (DecodedArray::Float64(a), DataType::Float32) => {
                builder.append_f32(a.value(index) as f32)
            }
            (_, other) => {
                return Err(LoomDecodeError::UnsupportedBuilderType {
                    operation: "append decoded child",
                    data_type: data_type_name(&other),
                });
            }
        }
        Ok(())
    }

    fn data_type(&self) -> DataType {
        match self {
            DecodedArray::Boolean(_) => DataType::Boolean,
            DecodedArray::Int32(_) => DataType::Int32,
            DecodedArray::Int64(_) => DataType::Int64,
            DecodedArray::Float32(_) => DataType::Float32,
            DecodedArray::Float64(_) => DataType::Float64,
            DecodedArray::Utf8(_) => DataType::Utf8,
        }
    }

    fn append_value_plus_reference(
        &self,
        index: usize,
        reference: i128,
        builder: &mut OutputBuilder,
    ) -> Result<(), LoomDecodeError> {
        if self.is_null(index) {
            builder.append_null();
            return Ok(());
        }
        match (self, builder.data_type()) {
            (DecodedArray::Int32(a), DataType::Int32) => {
                builder.append_i32((a.value(index) as i128).wrapping_add(reference) as i32)
            }
            (DecodedArray::Int64(a), DataType::Int64) => {
                builder.append_i64((a.value(index) as i128).wrapping_add(reference) as i64)
            }
            (DecodedArray::Int32(a), DataType::Int64) => {
                builder.append_i64((a.value(index) as i128).wrapping_add(reference) as i64)
            }
            (DecodedArray::Int64(a), DataType::Int32) => {
                builder.append_i32((a.value(index) as i128).wrapping_add(reference) as i32)
            }
            (_, other) => {
                return Err(LoomDecodeError::UnsupportedBuilderType {
                    operation: "FrameOfReference over non-integer child",
                    data_type: data_type_name(&other),
                });
            }
        }
        Ok(())
    }
}

fn data_type_name(data_type: &DataType) -> &'static str {
    match data_type {
        DataType::Boolean => "Boolean",
        DataType::Int32 => "Int32",
        DataType::Int64 => "Int64",
        DataType::Float32 => "Float32",
        DataType::Float64 => "Float64",
        DataType::Utf8 => "Utf8",
        _ => "unsupported",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arrow_builder_output::OutputBuilder;
    use crate::fsst_params::FsstParams;

    use arrow::array::Array;

    // Helper: build a dummy OutputBuilder for Int32.
    fn int32_builder() -> OutputBuilder {
        OutputBuilder::new(&DataType::Int32)
    }

    fn fsst_params_for_strings(rows: &[&str]) -> Vec<u8> {
        let mut codes_offsets = Vec::with_capacity(rows.len() + 1);
        let mut uncompressed_lengths = Vec::with_capacity(rows.len());
        let mut codes_bytes = Vec::new();

        codes_offsets.push(0);
        for row in rows {
            uncompressed_lengths.push(row.len() as u64);
            for byte in row.as_bytes() {
                codes_bytes.push(fsst::ESCAPE_CODE);
                codes_bytes.push(*byte);
            }
            codes_offsets.push(codes_bytes.len() as u64);
        }

        FsstParams {
            symbols: vec![],
            symbol_lengths: vec![],
            codes_offsets,
            uncompressed_lengths,
            validity: None,
            codes_bytes,
        }
        .encode()
    }

    fn empty_fsst_params() -> Vec<u8> {
        FsstParams {
            symbols: vec![],
            symbol_lengths: vec![],
            codes_offsets: vec![0],
            uncompressed_lengths: vec![],
            validity: None,
            codes_bytes: vec![],
        }
        .encode()
    }

    /// Direct read-loop KernelEscape remains unsupported because kernels own
    /// their own ArrayData. Use decode_layout_to_array_data for registry-backed
    /// top-level KernelEscape routing.
    #[test]
    fn direct_kernel_escape_returns_typed_error() {
        let escape = LayoutNode::KernelEscape {
            kernel_id: 0,
            params: vec![],
            count: 0,
        };
        let mut b = int32_builder();
        let result = synthesized_read_loop(&escape, &mut b);
        assert!(
            matches!(
                result,
                Err(LoomDecodeError::UnimplementedEncoding("KernelEscape"))
            ),
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
        let data: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
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

    /// A Raw node whose `count * elem_size` overflows `usize` must return a
    /// typed `BufferTooShort` error, never panic. Guards the no-panic-on-
    /// malformed-input contract (regression for the unchecked-multiply gap).
    #[test]
    fn raw_count_overflow_returns_buffer_too_short() {
        let node = LayoutNode::Raw {
            data: vec![0u8; 8],
            elem_size: 4,
            // count * 4 overflows usize -> would wrap small and slip past the
            // bounds check, then panic on slice indexing without the guard.
            count: usize::MAX / 2,
        };
        let mut b = int32_builder();
        let result = synthesized_read_loop(&node, &mut b);
        assert!(
            matches!(result, Err(LoomDecodeError::BufferTooShort { .. })),
            "expected BufferTooShort on count overflow, got {result:?}"
        );
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

    #[test]
    fn dictionary_i32_lookup_preserves_nulls() {
        let code_values = [0u64, 0, 0, 0];
        let codes = LayoutNode::BitPack {
            values_buf: encode_test_values(&code_values, 2, 32),
            bit_width: 2,
            offset: 0,
            count: code_values.len(),
            validity: Some(vec![true, false, true, true]),
            all_null: false,
        };
        let value_values: Vec<i32> = vec![10, 20, 30];
        let values = LayoutNode::Raw {
            data: value_values.iter().flat_map(|v| v.to_le_bytes()).collect(),
            elem_size: 4,
            count: value_values.len(),
        };
        let node = LayoutNode::Dictionary {
            codes: Box::new(codes),
            values: Box::new(values),
        };

        let mut b = int32_builder();
        synthesized_read_loop(&node, &mut b).unwrap();
        let data = b.finish();
        assert_eq!(data.len(), 4);
        assert_eq!(data.null_count(), 1);
        let array = arrow::array::Int32Array::from(data);
        assert_eq!(array.value(0), 10);
        assert!(array.is_null(1));
        assert_eq!(array.value(2), 10);
        assert_eq!(array.value(3), 10);
    }

    #[test]
    fn dictionary_i32_raw_codes_lookup_values() {
        let codes = LayoutNode::Raw {
            data: vec![0i64, 1, 2, 1]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
            elem_size: 8,
            count: 4,
        };
        let values = LayoutNode::Raw {
            data: vec![10i32, 20, 30]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
            elem_size: 4,
            count: 3,
        };
        let node = LayoutNode::Dictionary {
            codes: Box::new(codes),
            values: Box::new(values),
        };

        let mut b = int32_builder();
        synthesized_read_loop(&node, &mut b).unwrap();
        let array = arrow::array::Int32Array::from(b.finish());
        assert_eq!(array.values(), &[10, 20, 30, 20]);
    }

    #[test]
    fn dictionary_i32_invalid_code_returns_typed_error() {
        let codes = LayoutNode::Raw {
            data: vec![0i64, 3].iter().flat_map(|v| v.to_le_bytes()).collect(),
            elem_size: 8,
            count: 2,
        };
        let values = LayoutNode::Raw {
            data: vec![10i32, 20]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
            elem_size: 4,
            count: 2,
        };
        let node = LayoutNode::Dictionary {
            codes: Box::new(codes),
            values: Box::new(values),
        };

        let mut b = int32_builder();
        let result = synthesized_read_loop(&node, &mut b);
        assert!(matches!(
            result,
            Err(LoomDecodeError::InvalidDictionaryCode {
                index: 1,
                code: 3,
                values_len: 2
            })
        ));
    }

    #[test]
    fn run_end_i32_expands_values_and_nulls() {
        let run_ends = LayoutNode::Raw {
            data: vec![2i64, 5, 6]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
            elem_size: 8,
            count: 3,
        };
        let values = LayoutNode::Dictionary {
            codes: Box::new(LayoutNode::BitPack {
                values_buf: encode_test_values(&[0u64, 0, 0], 2, 32),
                bit_width: 2,
                offset: 0,
                count: 3,
                validity: Some(vec![true, false, true]),
                all_null: false,
            }),
            values: Box::new(LayoutNode::Raw {
                data: vec![10i32].iter().flat_map(|v| v.to_le_bytes()).collect(),
                elem_size: 4,
                count: 1,
            }),
        };
        let node = LayoutNode::RunEnd {
            run_ends: Box::new(run_ends),
            values: Box::new(values),
            count: 6,
        };

        let mut b = int32_builder();
        synthesized_read_loop(&node, &mut b).unwrap();
        let data = b.finish();
        assert_eq!(data.len(), 6);
        assert_eq!(data.null_count(), 3);
        let array = arrow::array::Int32Array::from(data);
        assert_eq!(array.value(0), 10);
        assert_eq!(array.value(1), 10);
        assert!(array.is_null(2));
        assert!(array.is_null(3));
        assert!(array.is_null(4));
        assert_eq!(array.value(5), 10);
    }

    #[test]
    fn run_end_boolean_expands_values_and_nulls() {
        let run_ends = LayoutNode::Raw {
            data: vec![2i64, 4].iter().flat_map(|v| v.to_le_bytes()).collect(),
            elem_size: 8,
            count: 2,
        };
        let values = LayoutNode::Raw {
            data: vec![1u8, 0u8],
            elem_size: 1,
            count: 2,
        };
        let node = LayoutNode::RunEnd {
            run_ends: Box::new(run_ends),
            values: Box::new(values),
            count: 4,
        };

        let mut b = OutputBuilder::new(&DataType::Boolean);
        synthesized_read_loop(&node, &mut b).unwrap();
        let data = b.finish();
        assert_eq!(data.len(), 4);
        let array = arrow::array::BooleanArray::from(data);
        assert!(array.value(0));
        assert!(array.value(1));
        assert!(!array.value(2));
        assert!(!array.value(3));
    }

    #[test]
    fn run_end_non_monotonic_returns_typed_error() {
        let node = LayoutNode::RunEnd {
            run_ends: Box::new(LayoutNode::Raw {
                data: vec![2i64, 2].iter().flat_map(|v| v.to_le_bytes()).collect(),
                elem_size: 8,
                count: 2,
            }),
            values: Box::new(LayoutNode::Raw {
                data: vec![1i32, 2].iter().flat_map(|v| v.to_le_bytes()).collect(),
                elem_size: 4,
                count: 2,
            }),
            count: 2,
        };
        let mut b = int32_builder();
        assert!(matches!(
            synthesized_read_loop(&node, &mut b),
            Err(LoomDecodeError::NonMonotonicRunEnd { index: 1, .. })
        ));
    }

    #[test]
    fn kernel_escape_zero_returns_empty_utf8_array() {
        let desc = LayoutDescription {
            data_type: DataType::Utf8,
            root: LayoutNode::KernelEscape {
                kernel_id: 0,
                params: empty_fsst_params(),
                count: 0,
            },
            row_count: 0,
        };
        let registry = crate::l2_kernel_registry::L2KernelRegistry::default_for_mvp0();
        let data = decode_layout_to_array_data(&desc, &registry).unwrap();
        assert_eq!(data.data_type(), &DataType::Utf8);
        assert_eq!(data.len(), 0);
    }

    #[test]
    fn dictionary_over_fsst_gathers_utf8_values() {
        let codes = LayoutNode::Raw {
            data: vec![1i64, 0, 1]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
            elem_size: 8,
            count: 3,
        };
        let values = LayoutNode::KernelEscape {
            kernel_id: 0,
            params: fsst_params_for_strings(&["alpha", "beta"]),
            count: 2,
        };
        let desc = LayoutDescription {
            data_type: DataType::Utf8,
            root: LayoutNode::Dictionary {
                codes: Box::new(codes),
                values: Box::new(values),
            },
            row_count: 3,
        };
        let registry = crate::l2_kernel_registry::L2KernelRegistry::default_for_mvp0();

        let data = decode_layout_to_array_data(&desc, &registry).unwrap();
        let array = arrow::array::StringArray::from(data);

        assert_eq!(array.value(0), "beta");
        assert_eq!(array.value(1), "alpha");
        assert_eq!(array.value(2), "beta");
    }

    #[test]
    fn kernel_escape_unknown_id_returns_typed_error() {
        let desc = LayoutDescription {
            data_type: DataType::Utf8,
            root: LayoutNode::KernelEscape {
                kernel_id: 99,
                params: vec![],
                count: 0,
            },
            row_count: 0,
        };
        let registry = crate::l2_kernel_registry::L2KernelRegistry::default_for_mvp0();
        assert!(matches!(
            decode_layout_to_array_data(&desc, &registry),
            Err(LoomDecodeError::VerifierFailed { ref code, .. }) if code == "unknown-kernel"
        ));
    }

    #[test]
    fn for_over_raw_applies_reference_and_preserves_nulls() {
        let codes = LayoutNode::BitPack {
            values_buf: encode_test_values(&[0u64, 0, 0], 1, 32),
            bit_width: 1,
            offset: 0,
            count: 3,
            validity: Some(vec![true, false, true]),
            all_null: false,
        };
        let values = LayoutNode::Raw {
            data: vec![1i32].iter().flat_map(|v| v.to_le_bytes()).collect(),
            elem_size: 4,
            count: 1,
        };
        let inner = LayoutNode::Dictionary {
            codes: Box::new(codes),
            values: Box::new(values),
        };
        let node = LayoutNode::FrameOfReference {
            reference: 10,
            inner: Box::new(inner),
        };

        let mut b = int32_builder();
        synthesized_read_loop(&node, &mut b).unwrap();
        let data = b.finish();
        assert_eq!(data.len(), 3);
        assert_eq!(data.null_count(), 1);
        let array = arrow::array::Int32Array::from(data);
        assert_eq!(array.value(0), 11);
        assert!(array.is_null(1));
        assert_eq!(array.value(2), 11);
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
                let lo_mask: u64 = if current_bits == 64 {
                    u64::MAX
                } else {
                    (1u64 << current_bits) - 1
                };
                let hi_mask: u64 = if remaining == 64 {
                    u64::MAX
                } else {
                    (1u64 << remaining) - 1
                };

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
                let existing = u32::from_le_bytes(buf[byte_off..byte_off + 4].try_into().unwrap());
                let new = existing | (val as u32);
                buf[byte_off..byte_off + 4].copy_from_slice(&new.to_le_bytes());
            }
            64 => {
                let existing = u64::from_le_bytes(buf[byte_off..byte_off + 8].try_into().unwrap());
                let new = existing | val;
                buf[byte_off..byte_off + 8].copy_from_slice(&new.to_le_bytes());
            }
            _ => unreachable!("unsupported t_bits {t_bits} (test-only helper)"),
        }
    }
}
