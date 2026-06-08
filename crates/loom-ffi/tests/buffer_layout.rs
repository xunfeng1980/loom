//! Wave-0 buffer-layout assertion test for `loom_decode`.
//!
//! # Purpose
//!
//! This test pins the exact Arrow C Data Interface buffer layout that
//! `loom_decode` produces — specifically that it is an Int32Array with:
//! - `length == 4`
//! - `null_count == 1`
//! - `n_buffers == 2`
//! - `buffers[0]` (validity bitmap) is non-null
//! - `buffers[1]` (int32 values) is non-null
//!
//! These indices are load-bearing for the OneShotStream/arrow_scan path in the
//! C++ extension (Plan 02-01, D-01).  If loom_decode's Arrow output ever changes
//! layout, this test fails before any C++ buffer logic is affected.
//!
//! # Requirements satisfied
//!
//! RESEARCH Open Question 2; Assumption A3 (buffers[0]=validity, buffers[1]=values).
//! DUCK-02: Wave-0 assurance that the C ABI seam carries correct data.
//! T-02-IDX: Arrow buffer index assumptions pinned before any C++ buffer logic.

use arrow::array::{Array, Float32Array, Float64Array, StringArray};
use arrow::datatypes::DataType;
use arrow::ffi::{from_ffi, FFI_ArrowArray, FFI_ArrowSchema};
use loom_core::alp_params::{AlpOutputType, AlpParams};
use loom_core::fsst_params::FsstParams;
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::layout_codec::encode_layout_payload;
use loom_ffi::ffi::loom_decode;

/// Asserts the Arrow buffer layout produced by `loom_decode`:
/// - return code 0
/// - length == 4, null_count == 1, n_buffers == 2
/// - buffers[0] (validity bitmap) is non-null
/// - buffers[1] (int32 values) is non-null
/// - Both release callbacks are fired exactly once via teardown.
#[test]
fn buffer_layout_n_buffers_validity_values() {
    // Allocate zeroed FFI shells (release == None / nullptr on entry).
    let mut array: FFI_ArrowArray = unsafe { std::mem::zeroed() };
    let mut schema: FFI_ArrowSchema = unsafe { std::mem::zeroed() };

    // --- Call loom_decode ---
    let rc = unsafe {
        loom_decode(
            std::ptr::null(),
            0,
            &mut array as *mut _,
            &mut schema as *mut _,
        )
    };

    // DUCK-02 / Pitfall P5: return code MUST be 0; never proceed on nonzero.
    assert_eq!(rc, 0, "loom_decode returned nonzero rc={}", rc);

    // --- Pin the buffer layout (Open Question 2, Assumption A3) ---

    // Int32Array [1, 2, 3, null] → length=4
    assert_eq!(array.length, 4, "expected length==4 for [1,2,3,null]");

    // Exactly one null (the fourth element).
    assert_eq!(array.null_count, 1, "expected null_count==1");

    // A flat Int32Array with a validity bitmap has exactly 2 buffers:
    //   buffers[0] = validity bitmap (non-null because there is a null element)
    //   buffers[1] = int32 values
    assert_eq!(
        array.n_buffers, 2,
        "expected n_buffers==2 (validity + values)"
    );

    // Dereference the C pointer array to confirm each slot is non-null.
    // Safety: loom_decode returned 0 and n_buffers==2, so `array.buffers`
    // points to a valid C array of 2 const-void pointers.
    assert!(
        !array.buffers.is_null(),
        "buffers pointer itself must be non-null"
    );

    let buf0 = unsafe { *array.buffers.add(0) };
    let buf1 = unsafe { *array.buffers.add(1) };

    assert!(
        !buf0.is_null(),
        "buffers[0] (validity bitmap) must be non-null — the array has one null element"
    );
    assert!(
        !buf1.is_null(),
        "buffers[1] (int32 values) must be non-null"
    );

    // --- Teardown: fire release callbacks exactly once (DUCK-03, PITFALLS P1) ---
    //
    // The arrow-data FFI_ArrowArray Drop impl calls release if release is Some.
    // We drop both structs here so teardown is explicit in the test output.
    // Dropping array calls array.release(&array) — sets release=None inside.
    // schema is dropped after.
    drop(array);
    drop(schema);

    // If this point is reached without a crash or ASAN report, the release path
    // is sound (no double-free, no leak detectable in the test harness).
}

#[test]
fn buffer_layout_utf8_validity_offsets_values() {
    let rows = [Some("alpha"), None, Some("beta")];
    let payload = encode_layout_payload(&LayoutDescription {
        data_type: DataType::Utf8,
        root: LayoutNode::KernelEscape {
            kernel_id: 0,
            params: fsst_params_for_strings(&rows),
            count: rows.len(),
        },
        row_count: rows.len(),
    });

    let mut array: FFI_ArrowArray = unsafe { std::mem::zeroed() };
    let mut schema: FFI_ArrowSchema = unsafe { std::mem::zeroed() };

    let rc = unsafe {
        loom_decode(
            payload.as_ptr(),
            payload.len(),
            &mut array as *mut _,
            &mut schema as *mut _,
        )
    };
    assert_eq!(rc, 0, "loom_decode returned nonzero rc={}", rc);

    assert_eq!(array.length, 3);
    assert_eq!(array.null_count, 1);
    assert!(
        array.n_buffers >= 3,
        "Utf8 array must expose validity, offsets, and data buffers"
    );
    assert!(
        !array.buffers.is_null(),
        "buffers pointer itself must be non-null"
    );

    let validity = unsafe { *array.buffers.add(0) };
    let offsets = unsafe { *array.buffers.add(1) };
    let data = unsafe { *array.buffers.add(2) };
    assert!(!validity.is_null(), "Utf8 validity buffer must be non-null");
    assert!(!offsets.is_null(), "Utf8 offsets buffer must be non-null");
    assert!(!data.is_null(), "Utf8 data buffer must be non-null");

    let array_data =
        unsafe { from_ffi(array, &schema) }.expect("from_ffi must succeed for Utf8 payload");
    assert_eq!(array_data.data_type(), &DataType::Utf8);
    let decoded = StringArray::from(array_data);
    assert_eq!(decoded.value(0), "alpha");
    assert!(decoded.is_null(1));
    assert_eq!(decoded.value(2), "beta");
    drop(decoded);

    if let Some(release_fn) = schema.release {
        unsafe { release_fn(&mut { schema } as *mut _) };
    }
}

#[test]
fn buffer_layout_alp_float32_validity_values() {
    assert_float_buffer_layout(
        DataType::Float32,
        AlpParams {
            output_type: AlpOutputType::Float32,
            decimal_exponent: -2,
            mantissas: vec![125, -250, 0],
            validity: Some(vec![true, false, true]),
        },
    );
}

#[test]
fn buffer_layout_alp_float64_validity_values() {
    assert_float_buffer_layout(
        DataType::Float64,
        AlpParams {
            output_type: AlpOutputType::Float64,
            decimal_exponent: -3,
            mantissas: vec![10125, -3500, 0],
            validity: Some(vec![true, true, false]),
        },
    );
}

fn fsst_params_for_strings(rows: &[Option<&str>]) -> Vec<u8> {
    let mut codes_offsets = Vec::with_capacity(rows.len() + 1);
    let mut uncompressed_lengths = Vec::with_capacity(rows.len());
    let mut validity = Vec::with_capacity(rows.len());
    let mut codes_bytes = Vec::new();

    codes_offsets.push(0);
    for row in rows {
        match row {
            Some(value) => {
                validity.push(true);
                uncompressed_lengths.push(value.len() as u64);
                for byte in value.as_bytes() {
                    codes_bytes.push(255);
                    codes_bytes.push(*byte);
                }
            }
            None => {
                validity.push(false);
                uncompressed_lengths.push(0);
            }
        }
        codes_offsets.push(codes_bytes.len() as u64);
    }

    FsstParams {
        symbols: vec![],
        symbol_lengths: vec![],
        codes_offsets,
        uncompressed_lengths,
        validity: Some(validity),
        codes_bytes,
    }
    .encode()
}

fn assert_float_buffer_layout(data_type: DataType, params: AlpParams) {
    let count = params.mantissas.len();
    let payload = encode_layout_payload(&LayoutDescription {
        data_type: data_type.clone(),
        root: LayoutNode::KernelEscape {
            kernel_id: 1,
            params: params.encode(),
            count,
        },
        row_count: count,
    });

    let mut array: FFI_ArrowArray = unsafe { std::mem::zeroed() };
    let mut schema: FFI_ArrowSchema = unsafe { std::mem::zeroed() };

    let rc = unsafe {
        loom_decode(
            payload.as_ptr(),
            payload.len(),
            &mut array as *mut _,
            &mut schema as *mut _,
        )
    };
    assert_eq!(rc, 0, "loom_decode returned nonzero rc={}", rc);

    assert_eq!(array.length, count as i64);
    assert_eq!(array.null_count, 1);
    assert_eq!(
        array.n_buffers, 2,
        "float array should expose validity + values"
    );
    assert!(
        !array.buffers.is_null(),
        "buffers pointer itself must be non-null"
    );

    let validity = unsafe { *array.buffers.add(0) };
    let values = unsafe { *array.buffers.add(1) };
    assert!(
        !validity.is_null(),
        "float validity buffer must be non-null"
    );
    assert!(!values.is_null(), "float values buffer must be non-null");

    let array_data =
        unsafe { from_ffi(array, &schema) }.expect("from_ffi must succeed for float payload");
    assert_eq!(array_data.data_type(), &data_type);
    match data_type {
        DataType::Float32 => {
            let decoded = Float32Array::from(array_data);
            assert_eq!(decoded.null_count(), 1);
            drop(decoded);
        }
        DataType::Float64 => {
            let decoded = Float64Array::from(array_data);
            assert_eq!(decoded.null_count(), 1);
            drop(decoded);
        }
        _ => unreachable!("test helper only supports floats"),
    }

    if let Some(release_fn) = schema.release {
        unsafe { release_fn(&mut { schema } as *mut _) };
    }
}
