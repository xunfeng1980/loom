//! Vortex oracle decoder — decodes arrays via Vortex's own `execute` path
//! for row-for-row comparison against `loom-core`.
//!
//! This module is part of the D-02 isolation: it is the "reference truth"
//! side that loom-core must match. It is only used in tests.

use vortex_array::arrays::bool::BoolArrayExt;
use vortex_array::arrays::primitive::PrimitiveArrayExt;
use vortex_array::arrays::{BoolArray, PrimitiveArray};
use vortex_array::validity::Validity;
use vortex_array::ArrayRef;
use vortex_array::VortexSessionExecute;
use vortex_array::LEGACY_SESSION;

/// Decode a Vortex `ArrayRef` to a `Vec<i32>` via Vortex's own execution path.
///
/// The array must have `PType::I32` (only i32 arrays). Returns `(values, null_flags)`:
/// - `values[i]` is the decoded value at position `i` (0 for null positions).
/// - `null_flags[i]` is `true` if position `i` is null.
pub fn decode_i32_oracle(array: &ArrayRef) -> (Vec<i32>, Vec<bool>) {
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let canonical = array
        .clone()
        .execute::<PrimitiveArray>(&mut ctx)
        .expect("oracle execute::<PrimitiveArray> failed");

    let values: Vec<i32> = canonical.as_slice::<i32>().to_vec();
    // Use the trait method via explicit UFCS to avoid ambiguity with ArrayRef::validity().
    let validity = PrimitiveArrayExt::validity(&canonical);
    let null_flags = extract_null_flags(&validity, canonical.as_ref().len());
    (values, null_flags)
}

/// Decode a Vortex `ArrayRef` to `Vec<u32>` for unsigned-typed arrays.
pub fn decode_u32_oracle(array: &ArrayRef) -> (Vec<u32>, Vec<bool>) {
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let canonical = array
        .clone()
        .execute::<PrimitiveArray>(&mut ctx)
        .expect("oracle execute::<PrimitiveArray> failed");

    let values: Vec<u32> = canonical.as_slice::<u32>().to_vec();
    let validity = PrimitiveArrayExt::validity(&canonical);
    let null_flags = extract_null_flags(&validity, canonical.as_ref().len());
    (values, null_flags)
}

/// Decode a Vortex `ArrayRef` to boolean values via Vortex's own execution path.
pub fn decode_bool_oracle(array: &ArrayRef) -> (Vec<bool>, Vec<bool>) {
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let canonical = array
        .clone()
        .execute::<BoolArray>(&mut ctx)
        .expect("oracle execute::<BoolArray> failed");

    let values: Vec<bool> = canonical
        .to_bit_buffer()
        .iter()
        .take(canonical.as_ref().len())
        .collect();
    let validity = BoolArrayExt::validity(&canonical);
    let null_flags = extract_null_flags(&validity, canonical.as_ref().len());
    (values, null_flags)
}

/// Build a null flags vector from a Vortex `Validity` (true = null).
///
/// Converts the enum into a per-row `Vec<bool>` so callers can compare
/// with Arrow's `ArrayData::nulls().is_null(i)` without holding Vortex types.
pub fn extract_null_flags(validity: &Validity, len: usize) -> Vec<bool> {
    match validity {
        Validity::NonNullable | Validity::AllValid => vec![false; len],
        Validity::AllInvalid => vec![true; len],
        Validity::Array(bool_arr) => {
            let mut ctx = LEGACY_SESSION.create_execution_ctx();
            let canonical = bool_arr
                .clone()
                .execute::<BoolArray>(&mut ctx)
                .expect("oracle validity BoolArray execute failed");
            let bit_buf = canonical.to_bit_buffer();
            // Bit = 1 means VALID in Vortex (same as Arrow); invert for null flag.
            bit_buf.iter().take(len).map(|valid| !valid).collect()
        }
    }
}
