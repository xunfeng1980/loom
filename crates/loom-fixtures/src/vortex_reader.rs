//! Vortex → `loom-core` LayoutNode bridge (D-02 isolation layer).
//!
//! This module is the **only** place in the Loom workspace that calls into the
//! Vortex ecosystem for decoding purposes (D-02). It inspects an in-memory
//! Vortex `BitPackedArray` or `FoRArray` and emits a
//! [`loom_core::l1_model::LayoutNode`] + raw packed bytes.
//!
//! # D-02 invariant
//!
//! `loom-core` has zero `vortex-*` dependencies. Every Vortex type is
//! translated here into plain Rust primitives (`Vec<u8>`, `Option<Vec<bool>>`,
//! `i128`) before being handed to `loom-core`. The D-02 boundary is enforced
//! by `cargo tree -p loom-core | grep -c -E 'vortex|fastlanes'` == 0.

use loom_core::l1_model::LayoutNode;
use vortex_array::arrays::bool::BoolArrayExt;
use vortex_array::arrays::dict::DictArraySlotsExt;
use vortex_array::arrays::primitive::PrimitiveArrayExt;
use vortex_array::arrays::{Bool, BoolArray, Dict, DictArray, Primitive, PrimitiveArray};
use vortex_array::dtype::PType;
use vortex_array::scalar::PValue;
use vortex_array::validity::Validity;
use vortex_array::ArrayRef;
use vortex_array::VortexSessionExecute;
use vortex_array::LEGACY_SESSION;
use vortex_fastlanes::BitPackedArray;
use vortex_fastlanes::BitPackedArrayExt;
use vortex_fastlanes::FoRArray;
use vortex_fastlanes::FoRArrayExt;
use vortex_fastlanes::{RLEArray, RLEArrayExt};

// ---------------------------------------------------------------------------
// packed_bytes — confirmed BufferHandle accessor (Wave-0 check: Pitfall 5)
// ---------------------------------------------------------------------------

/// Extract the raw packed bytes from a [`BitPackedArray`]'s `BufferHandle`.
///
/// # BufferHandle access: confirmed as_host().as_ref()
///
/// Verified against `vortex-buffer-0.74.0/src/buffer.rs`:
/// - `BufferHandle::as_host()` returns `&ByteBuffer` (= `&Buffer<u8>`).
/// - `Buffer<u8>` implements `AsRef<[u8]>` and `Deref<Target=[u8]>`.
/// - `.as_ref()` on `ByteBuffer` gives `&[u8]` directly (option A from RESEARCH
///   Pitfall 5).
pub fn packed_bytes(arr: &BitPackedArray) -> Vec<u8> {
    // BufferHandle access: confirmed as_host().as_ref()
    arr.packed().as_host().as_ref().to_vec()
}

// ---------------------------------------------------------------------------
// extract_validity — Validity enum -> (Option<Vec<bool>>, bool)
// ---------------------------------------------------------------------------

/// Flatten a Vortex [`Validity`] enum into an `Option<Vec<bool>>` + `all_null`
/// flag so that `loom-core` never needs any Vortex type.
///
/// Returns `(validity_vec, all_null)` where:
/// - `(None, false)` — `NonNullable` or `AllValid` (no nulls).
/// - `(None, true)` — `AllInvalid` (every row is null).
/// - `(Some(bits), false)` — per-row bitmap; `true` = valid, `false` = null.
///
/// For `Validity::Array`, the boolean array is executed to canonical
/// `BoolArray` inside `vortex_reader` so `loom-core` never calls Vortex.
/// (T-03-04 mitigation: validity flatten is fully contained here.)
pub fn extract_validity(validity: Validity, len: usize) -> (Option<Vec<bool>>, bool) {
    match validity {
        Validity::NonNullable | Validity::AllValid => (None, false),
        Validity::AllInvalid => (None, true),
        Validity::Array(bool_arr) => {
            // Execute the boolean array to canonical BoolArray, then collect
            // the bit buffer into a Vec<bool>. This is the only Vortex call
            // in the validity path — it stays inside loom-fixtures.
            let mut ctx = LEGACY_SESSION.create_execution_ctx();
            let canonical = bool_arr
                .execute::<BoolArray>(&mut ctx)
                .expect("validity BoolArray execute failed");
            let bit_buf = canonical.to_bit_buffer();
            let bools: Vec<bool> = bit_buf.iter().take(len).collect();
            (Some(bools), false)
        }
    }
}

// ---------------------------------------------------------------------------
// from_bitpacked_array — BitPackedArray -> LayoutNode::BitPack
// ---------------------------------------------------------------------------

/// Inspect a Vortex [`BitPackedArray`] and emit a `LayoutNode::BitPack`.
///
/// The Vortex `Validity` enum is flattened to `Option<Vec<bool>>` by
/// [`extract_validity`] so `loom-core` receives only plain Rust types.
///
/// # D-02
///
/// No Vortex type escapes this function boundary; the returned `LayoutNode`
/// carries only `Vec<u8>`, `u8`, `u16`, `usize`, `Option<Vec<bool>>`, `bool`.
pub fn from_bitpacked_array(arr: &BitPackedArray) -> LayoutNode {
    let bit_width: u8 = arr.bit_width();
    let offset: u16 = arr.offset();
    let count: usize = arr.as_ref().len();
    // Extract packed bytes via confirmed accessor (Wave-0 check resolved).
    let values_buf: Vec<u8> = packed_bytes(arr);
    // Use BitPackedArrayExt::validity explicitly to avoid ambiguity with
    // ArrayRef::validity() which returns VortexResult<Validity>.
    let validity_enum: Validity = BitPackedArrayExt::validity(arr);
    let (validity, all_null) = extract_validity(validity_enum, count);

    LayoutNode::BitPack {
        values_buf,
        bit_width,
        offset,
        count,
        validity,
        all_null,
    }
}

// ---------------------------------------------------------------------------
// from_for_array — FoRArray -> LayoutNode::FrameOfReference
// ---------------------------------------------------------------------------

/// Inspect a Vortex [`FoRArray`] and emit a `LayoutNode::FrameOfReference`.
///
/// The reference scalar is widened to `i128` (anti-pattern A3: stores both
/// signed and unsigned references without truncation). Validity lives in the
/// inner `BitPackedArray` child (RESEARCH Pitfall 3, Q3: `ValidityChild<FoR>`
/// delegates to `encoded()`). This function does NOT attach validity at the
/// FOR node.
///
/// # Panics
///
/// Panics if `arr.encoded()` cannot be downcast to `BitPacked` encoding.
pub fn from_for_array(arr: &FoRArray) -> LayoutNode {
    // Extract the reference scalar as i128 (A3 — safe for all integer ptypes).
    let ref_scalar = arr.reference_scalar();
    let pvalue = ref_scalar
        .as_primitive()
        .pvalue()
        .expect("FoR reference must be non-null");
    let reference: i128 = pvalue_to_i128(pvalue);

    // Recurse into the inner BitPackedArray (validity lives here — Pitfall 3).
    // `encoded()` returns `&ArrayRef`. Use `as_opt::<BitPacked>()` which returns
    // `Option<ArrayView<'_, BitPacked>>`. `ArrayView<'_, BitPacked>` implements
    // `BitPackedArrayExt` (via `TypedArrayRef<BitPacked>`), so we can call the
    // same `from_bitpacked_view` helper.
    let inner_array_ref = arr.encoded();
    let inner_bp_view = inner_array_ref
        .as_opt::<vortex_fastlanes::BitPacked>()
        .expect("FoRArray inner must be a BitPackedArray (Phase 3)");
    let inner = from_bitpacked_view(&inner_bp_view);

    LayoutNode::FrameOfReference {
        reference,
        inner: Box::new(inner),
    }
}

// ---------------------------------------------------------------------------
// from_dict_array — DictArray -> LayoutNode::Dictionary
// ---------------------------------------------------------------------------

/// Inspect a Vortex [`DictArray`] and emit `LayoutNode::Dictionary`.
///
/// Codes and values are recursively bridged through [`from_array_ref`], so
/// encoded children such as `BitPackedArray` stay encoded in the Loom layout.
pub fn from_dict_array(arr: &DictArray) -> LayoutNode {
    from_dict_view(arr)
}

// ---------------------------------------------------------------------------
// from_rle_array — RLEArray -> LayoutNode::RunEnd
// ---------------------------------------------------------------------------

/// Inspect a Vortex [`RLEArray`] and emit `LayoutNode::RunEnd`.
///
/// Vortex FastLanes RLE stores chunk-local value indices plus per-chunk value
/// offsets, not literal run-end positions. For Phase 4 fixture coverage, this
/// bridge canonicalizes through Vortex's own in-memory execute path and scans
/// the decoded primitive rows back into simple Loom run ends. The paired tests
/// still compare Loom output row-for-row with the live Vortex oracle.
pub fn from_rle_array(arr: &RLEArray) -> LayoutNode {
    // Touch the real RLE slots so accidental API drift is caught by compile
    // errors even though the Phase 4 bridge canonicalizes to simple run ends.
    let _ = (
        arr.values(),
        arr.indices(),
        arr.values_idx_offsets(),
        arr.offset(),
    );

    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let canonical = arr
        .as_array()
        .clone()
        .execute::<PrimitiveArray>(&mut ctx)
        .expect("RLE execute::<PrimitiveArray> failed");
    primitive_to_run_end(&canonical)
}

// ---------------------------------------------------------------------------
// Private helper: from an ArrayView<'_, BitPacked>
// ---------------------------------------------------------------------------

/// Build a `LayoutNode::BitPack` from an `ArrayView<'_, BitPacked>`.
///
/// `ArrayView<'_, BitPacked>` implements `TypedArrayRef<BitPacked>` and thus
/// `BitPackedArrayExt` — the same accessors as `BitPackedArray` (= `Array<BitPacked>`).
fn from_bitpacked_view<T: BitPackedArrayExt>(view: &T) -> LayoutNode {
    let bit_width: u8 = view.bit_width();
    let offset: u16 = view.offset();
    let count: usize = view.as_ref().len();
    // Extract packed bytes via confirmed accessor.
    let values_buf: Vec<u8> = view.packed().as_host().as_ref().to_vec();
    let validity_enum: Validity = BitPackedArrayExt::validity(view);
    let (validity, all_null) = extract_validity(validity_enum, count);

    LayoutNode::BitPack {
        values_buf,
        bit_width,
        offset,
        count,
        validity,
        all_null,
    }
}

fn from_dict_view<T: DictArraySlotsExt>(view: &T) -> LayoutNode {
    LayoutNode::Dictionary {
        codes: Box::new(from_array_ref(view.codes())),
        values: Box::new(from_array_ref(view.values())),
    }
}

fn from_array_ref(array: &ArrayRef) -> LayoutNode {
    if let Some(view) = array.as_opt::<vortex_fastlanes::BitPacked>() {
        return from_bitpacked_view(&view);
    }
    if let Some(view) = array.as_opt::<vortex_fastlanes::FoR>() {
        return from_for_view(&view);
    }
    if let Some(view) = array.as_opt::<Dict>() {
        return from_dict_view(&view);
    }
    if let Some(view) = array.as_opt::<Primitive>() {
        let mut ctx = LEGACY_SESSION.create_execution_ctx();
        let canonical = view
            .as_ref()
            .clone()
            .execute::<PrimitiveArray>(&mut ctx)
            .expect("primitive execute failed");
        return primitive_to_raw(&canonical);
    }
    if let Some(view) = array.as_opt::<Bool>() {
        let mut ctx = LEGACY_SESSION.create_execution_ctx();
        let canonical = view
            .as_ref()
            .clone()
            .execute::<BoolArray>(&mut ctx)
            .expect("bool execute failed");
        return bool_to_raw(&canonical);
    }
    panic!("unsupported Vortex array encoding for Loom fixture bridge");
}

fn from_for_view<T: FoRArrayExt>(view: &T) -> LayoutNode {
    let ref_scalar = view.reference_scalar();
    let pvalue = ref_scalar
        .as_primitive()
        .pvalue()
        .expect("FoR reference must be non-null");
    let reference: i128 = pvalue_to_i128(pvalue);

    let inner_array_ref = view.encoded();
    let inner = from_array_ref(inner_array_ref);

    LayoutNode::FrameOfReference {
        reference,
        inner: Box::new(inner),
    }
}

fn primitive_to_raw(arr: &PrimitiveArray) -> LayoutNode {
    assert!(
        !has_nulls(&PrimitiveArrayExt::validity(arr), arr.as_ref().len()),
        "nullable primitive arrays must stay in an encoding that carries validity"
    );

    match arr.ptype() {
        PType::U8 => raw_i32(arr.as_slice::<u8>().iter().map(|&v| v as i32)),
        PType::U16 => raw_i32(arr.as_slice::<u16>().iter().map(|&v| v as i32)),
        PType::U32 => raw_i32(arr.as_slice::<u32>().iter().map(|&v| v as i32)),
        PType::I32 => raw_i32(arr.as_slice::<i32>().iter().copied()),
        PType::I64 => LayoutNode::Raw {
            data: arr
                .as_slice::<i64>()
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
            elem_size: 8,
            count: arr.as_ref().len(),
        },
        other => panic!("unsupported primitive ptype for Loom fixture bridge: {other:?}"),
    }
}

fn primitive_to_run_end(arr: &PrimitiveArray) -> LayoutNode {
    assert!(
        !has_nulls(&PrimitiveArrayExt::validity(arr), arr.as_ref().len()),
        "nullable FastLanes RLE is covered by the hand-written Loom RunEnd fallback test"
    );

    match arr.ptype() {
        PType::U8 => run_end_i32(arr.as_slice::<u8>().iter().map(|&v| v as i32)),
        PType::U16 => run_end_i32(arr.as_slice::<u16>().iter().map(|&v| v as i32)),
        PType::U32 => run_end_i32(arr.as_slice::<u32>().iter().map(|&v| v as i32)),
        PType::I32 => run_end_i32(arr.as_slice::<i32>().iter().copied()),
        other => panic!("unsupported RLE ptype for Loom fixture bridge: {other:?}"),
    }
}

fn bool_to_raw(arr: &BoolArray) -> LayoutNode {
    assert!(
        !has_nulls(&BoolArrayExt::validity(arr), arr.as_ref().len()),
        "nullable BoolArray cannot be represented as Loom Raw"
    );
    let values: Vec<u8> = arr
        .to_bit_buffer()
        .iter()
        .take(arr.as_ref().len())
        .map(u8::from)
        .collect();
    LayoutNode::Raw {
        data: values,
        elem_size: 1,
        count: arr.as_ref().len(),
    }
}

fn raw_i32<I>(values: I) -> LayoutNode
where
    I: IntoIterator<Item = i32>,
{
    let values: Vec<i32> = values.into_iter().collect();
    LayoutNode::Raw {
        data: values.iter().flat_map(|v| v.to_le_bytes()).collect(),
        elem_size: 4,
        count: values.len(),
    }
}

fn run_end_i32<I>(values: I) -> LayoutNode
where
    I: IntoIterator<Item = i32>,
{
    let values: Vec<i32> = values.into_iter().collect();
    if values.is_empty() {
        return LayoutNode::RunEnd {
            run_ends: Box::new(LayoutNode::Raw {
                data: vec![],
                elem_size: 8,
                count: 0,
            }),
            values: Box::new(raw_i32([])),
            count: 0,
        };
    }

    let mut run_values = Vec::new();
    let mut run_ends = Vec::new();
    let mut current = values[0];
    for (idx, value) in values.iter().copied().enumerate().skip(1) {
        if value != current {
            run_values.push(current);
            run_ends.push(idx as i64);
            current = value;
        }
    }
    run_values.push(current);
    run_ends.push(values.len() as i64);

    LayoutNode::RunEnd {
        run_ends: Box::new(LayoutNode::Raw {
            data: run_ends.iter().flat_map(|v| v.to_le_bytes()).collect(),
            elem_size: 8,
            count: run_ends.len(),
        }),
        values: Box::new(raw_i32(run_values)),
        count: values.len(),
    }
}

fn has_nulls(validity: &Validity, len: usize) -> bool {
    match validity {
        Validity::NonNullable | Validity::AllValid => false,
        Validity::AllInvalid => len > 0,
        Validity::Array(_) => true,
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Widen any integer [`PValue`] variant to `i128` for storage in
/// `LayoutNode::FrameOfReference.reference`.
///
/// Unsigned values (u8/u16/u32/u64) are zero-extended; signed values
/// (i8/i16/i32/i64) are sign-extended. The wrapping-add in
/// `synthesized_read_loop` handles the arithmetic correctly regardless.
fn pvalue_to_i128(pv: PValue) -> i128 {
    match pv {
        PValue::U8(v) => v as i128,
        PValue::U16(v) => v as i128,
        PValue::U32(v) => v as i128,
        PValue::U64(v) => v as i128,
        PValue::I8(v) => v as i128,
        PValue::I16(v) => v as i128,
        PValue::I32(v) => v as i128,
        PValue::I64(v) => v as i128,
        other => panic!("FoR reference must be an integer PValue, got {:?}", other),
    }
}
