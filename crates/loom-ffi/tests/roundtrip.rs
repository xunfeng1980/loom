//! Roundtrip and panic-safety tests for the `loom_decode` FFI entry point.
//!
//! These tests run OUTSIDE DuckDB — they exercise the Arrow C Data Interface
//! ownership/release path in pure Rust (PITFALLS P1/P2, ARROW-03, DUCK-04).
//!
//! # Test matrix
//!
//! | Test | Requirement |
//! |------|-------------|
//! | `release_path_roundtrip` | ARROW-03, PITFALLS P1/P2 |
//! | `panic_does_not_abort` | DUCK-04, PITFALLS P3, T-01-05 |

use arrow::array::{Array, Float32Array, Float64Array, Int32Array, StringArray};
use arrow::datatypes::DataType;
use arrow::ffi::{from_ffi, FFI_ArrowArray, FFI_ArrowSchema};
use loom_core::alp_params::{AlpOutputType, AlpParams};
use loom_core::container_codec::{wrap_layout_payload, Feature};
use loom_core::fsst_params::FsstParams;
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::layout_codec::encode_layout_payload;
use loom_ffi::ffi::{loom_decode, set_panic_sentinel, LoomError};

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Call `loom_decode` into two zero-initialized FFI shells.
///
/// Returns the populated pair on success.  Panics if `loom_decode` returns
/// a nonzero code (signals a test-setup failure, not the case under test).
unsafe fn call_loom_decode(input: &[u8]) -> (FFI_ArrowArray, FFI_ArrowSchema) {
    let mut ffi_array: FFI_ArrowArray = std::mem::zeroed();
    let mut ffi_schema: FFI_ArrowSchema = std::mem::zeroed();
    let code = loom_decode(
        input.as_ptr(),
        input.len(),
        &mut ffi_array as *mut _,
        &mut ffi_schema as *mut _,
    );
    assert_eq!(code, 0, "loom_decode returned error code {}", code);
    (ffi_array, ffi_schema)
}

// ---------------------------------------------------------------------------
// Test 1: release-path roundtrip
// ---------------------------------------------------------------------------

/// Exercise the complete Arrow C Data Interface export → import → release path
/// in pure Rust, without DuckDB involvement (ARROW-03, PITFALLS P1/P2).
///
/// # What this verifies
///
/// 1. `loom_decode` returns 0 and writes into the caller-provided shells.
/// 2. The written `FFI_ArrowArray` has a non-null `release` callback (i.e.
///    `to_ffi` + `ptr::write` correctly transferred ownership).
/// 3. Importing the array via `from_ffi` reconstructs the correct `ArrayData`.
/// 4. The reconstructed array has the expected values: `[1, 2, 3, null]`.
/// 5. Dropping the imported array fires `release` exactly once (the release
///    callback sets `release = null` on the source struct as per the Arrow
///    C Data Interface specification).
#[test]
fn roundtrip_success_returns_expected_values_and_null() {
    // Step 1: call loom_decode into zero-initialized shells.
    let (ffi_array, ffi_schema) = unsafe { call_loom_decode(&[]) };

    // Step 2: confirm the release callback is set (non-null).
    // Safety: ffi_array was just written by loom_decode; its fields are valid.
    // The `release` field is the first field of FFI_ArrowArray per the Arrow
    // C Data Interface spec.
    assert!(
        ffi_array.release.is_some(),
        "FFI_ArrowArray.release must be non-null after successful loom_decode"
    );

    // Step 3: import the array back into Rust via `from_ffi`.
    //
    // `from_ffi` takes ownership of `ffi_array` (moves it out of our variable)
    // and borrows `ffi_schema`.  When the returned `ArrayData` is dropped,
    // the release callback fires exactly once (PITFALLS P1, ARROW-03).
    //
    // Safety: both structs were freshly written by loom_decode and have not
    // been moved, cloned, or released.
    let array_data = unsafe { from_ffi(ffi_array, &ffi_schema) }
        .expect("from_ffi must succeed on a valid FFI_ArrowArray");

    // Step 4: reconstruct a typed Int32Array and assert values + nulls.
    let array = Int32Array::from(array_data);

    assert_eq!(array.len(), 4, "expected 4 elements [1, 2, 3, null]");
    assert_eq!(array.null_count(), 1, "expected exactly one null");

    // Check non-null values.
    assert_eq!(array.value(0), 1);
    assert_eq!(array.value(1), 2);
    assert_eq!(array.value(2), 3);

    // Check the null position.
    assert!(array.is_null(3), "element 3 must be null");

    // Step 5: drop the array — this fires the release callback.
    //
    // After `from_ffi`, the Rust `ArrayData` owns the buffers.  Dropping it
    // calls the release callback exactly once (no double-free, no leak).
    // We confirm this implicitly: if a double-free occurred we would crash or
    // see AddressSanitizer errors.  The Arrow implementation sets
    // `release = null` after firing, so subsequent drops are no-ops.
    drop(array);

    // Step 6: the schema still needs to be released.  Because `from_ffi` only
    // borrows the schema (not moves it), `ffi_schema` still holds its release
    // pointer.  We must call its release callback to avoid a leak.
    //
    // Safety: ffi_schema was written by loom_decode and has not been released.
    if let Some(release_fn) = ffi_schema.release {
        unsafe { release_fn(&mut { ffi_schema } as *mut _) };
    }
    // After this point, ffi_schema.release is null (the C Data Interface
    // contract: release sets the pointer to null, preventing double-free).
}

#[test]
fn roundtrip_decode_payload_i32_values() {
    let values = [10i32, -20, 30];
    let payload = encode_layout_payload(&LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::Raw {
            data: values.iter().flat_map(|v| v.to_le_bytes()).collect(),
            elem_size: 4,
            count: values.len(),
        },
        row_count: values.len(),
    });

    let (ffi_array, ffi_schema) = unsafe { call_loom_decode(&payload) };
    let array_data =
        unsafe { from_ffi(ffi_array, &ffi_schema) }.expect("from_ffi must succeed for i32 payload");
    let array = Int32Array::from(array_data);

    assert_eq!(array.values(), values.as_slice());
    assert_eq!(array.null_count(), 0);

    drop(array);
    if let Some(release_fn) = ffi_schema.release {
        unsafe { release_fn(&mut { ffi_schema } as *mut _) };
    }
}

#[test]
fn roundtrip_decode_container_payload_i32_values() {
    let values = [10i32, -20, 30];
    let payload = encode_layout_payload(&LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::Raw {
            data: values.iter().flat_map(|v| v.to_le_bytes()).collect(),
            elem_size: 4,
            count: values.len(),
        },
        row_count: values.len(),
    });
    let wrapped = wrap_layout_payload(&payload).expect("wrap layout payload");

    let (ffi_array, ffi_schema) = unsafe { call_loom_decode(&wrapped) };
    let array_data = unsafe { from_ffi(ffi_array, &ffi_schema) }
        .expect("from_ffi must succeed for container i32 payload");
    let array = Int32Array::from(array_data);

    assert_eq!(array.values(), values.as_slice());
    assert_eq!(array.null_count(), 0);

    drop(array);
    if let Some(release_fn) = ffi_schema.release {
        unsafe { release_fn(&mut { ffi_schema } as *mut _) };
    }
}

#[test]
fn container_unknown_required_feature_returns_decode_failed() {
    let values = [10i32, -20, 30];
    let payload = encode_layout_payload(&LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::Raw {
            data: values.iter().flat_map(|v| v.to_le_bytes()).collect(),
            elem_size: 4,
            count: values.len(),
        },
        row_count: values.len(),
    });
    let mut wrapped = wrap_layout_payload(&payload).expect("wrap layout payload");
    let required_features_offset = 4 + 2 + 2;
    let unknown_required = Feature::SingleColumnLmp1.mask() | (1u64 << 63);
    wrapped[required_features_offset..required_features_offset + 8]
        .copy_from_slice(&unknown_required.to_le_bytes());

    let mut ffi_array: FFI_ArrowArray = unsafe { std::mem::zeroed() };
    let mut ffi_schema: FFI_ArrowSchema = unsafe { std::mem::zeroed() };
    let code = unsafe {
        loom_decode(
            wrapped.as_ptr(),
            wrapped.len(),
            &mut ffi_array as *mut _,
            &mut ffi_schema as *mut _,
        )
    };

    assert_eq!(code, LoomError::DecodeFailed.code());
}

#[test]
fn roundtrip_decode_payload_utf8_values() {
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

    let (ffi_array, ffi_schema) = unsafe { call_loom_decode(&payload) };
    let array_data = unsafe { from_ffi(ffi_array, &ffi_schema) }
        .expect("from_ffi must succeed for Utf8 payload");
    let array = StringArray::from(array_data);

    assert_eq!(array.len(), 3);
    assert_eq!(array.value(0), "alpha");
    assert!(array.is_null(1));
    assert_eq!(array.value(2), "beta");

    drop(array);
    if let Some(release_fn) = ffi_schema.release {
        unsafe { release_fn(&mut { ffi_schema } as *mut _) };
    }
}

#[test]
fn roundtrip_decode_payload_alp_float32_values() {
    let payload = encode_layout_payload(&alp_desc(
        DataType::Float32,
        AlpParams {
            output_type: AlpOutputType::Float32,
            decimal_exponent: -2,
            mantissas: vec![125, -250, 0, 125],
            validity: Some(vec![true, false, true, true]),
        },
    ));

    let (ffi_array, ffi_schema) = unsafe { call_loom_decode(&payload) };
    let array_data = unsafe { from_ffi(ffi_array, &ffi_schema) }
        .expect("from_ffi must succeed for ALP Float32 payload");
    assert_eq!(array_data.data_type(), &DataType::Float32);
    let array = Float32Array::from(array_data);

    assert_eq!(array.len(), 4);
    assert_eq!(array.null_count(), 1);
    assert_eq!(array.value(0), 1.25);
    assert!(array.is_null(1));
    assert_eq!(array.value(2), 0.0);
    assert_eq!(array.value(3), 1.25);

    drop(array);
    if let Some(release_fn) = ffi_schema.release {
        unsafe { release_fn(&mut { ffi_schema } as *mut _) };
    }
}

#[test]
fn roundtrip_decode_payload_alp_float64_values() {
    let payload = encode_layout_payload(&alp_desc(
        DataType::Float64,
        AlpParams {
            output_type: AlpOutputType::Float64,
            decimal_exponent: -3,
            mantissas: vec![10125, -3500, 0, 10125],
            validity: Some(vec![true, true, false, true]),
        },
    ));

    let (ffi_array, ffi_schema) = unsafe { call_loom_decode(&payload) };
    let array_data = unsafe { from_ffi(ffi_array, &ffi_schema) }
        .expect("from_ffi must succeed for ALP Float64 payload");
    assert_eq!(array_data.data_type(), &DataType::Float64);
    let array = Float64Array::from(array_data);

    assert_eq!(array.len(), 4);
    assert_eq!(array.null_count(), 1);
    assert_eq!(array.value(0), 10.125);
    assert_eq!(array.value(1), -3.5);
    assert!(array.is_null(2));
    assert_eq!(array.value(3), 10.125);

    drop(array);
    if let Some(release_fn) = ffi_schema.release {
        unsafe { release_fn(&mut { ffi_schema } as *mut _) };
    }
}

// ---------------------------------------------------------------------------
// Test 2: panic-safety
// ---------------------------------------------------------------------------

/// Verify that a panic inside `loom_decode_inner` is caught by `catch_unwind`
/// and returns the `Panicked` error code rather than aborting the process.
///
/// # Mechanism
///
/// `set_panic_sentinel()` sets an `AtomicBool` inside `loom_decode_inner`.
/// The next call to `loom_decode_inner` checks the flag, resets it, and
/// `panic!`s.  The `catch_unwind` wrapper in `loom_decode` catches that panic
/// and maps it to `LoomError::Panicked` (DUCK-04, PITFALLS P3, T-01-05).
///
/// If `catch_unwind` did NOT work, the test process would abort — the test
/// runner itself would crash, and the test suite would report an error rather
/// than this test passing.  The fact that this test passes proves that no panic
/// unwound past the `extern "C"` frame.
#[test]
fn panic_sentinel_returns_panicked_code_without_aborting() {
    // Allocate output shells so we can pass valid non-null pointers.
    // The inner function panics before it writes to them.
    let mut ffi_array: FFI_ArrowArray = unsafe { std::mem::zeroed() };
    let mut ffi_schema: FFI_ArrowSchema = unsafe { std::mem::zeroed() };

    // Arm the panic sentinel — the next call to loom_decode_inner will panic.
    set_panic_sentinel();

    // Call loom_decode — loom_decode_inner panics; catch_unwind catches it.
    let code = unsafe {
        loom_decode(
            std::ptr::null(),
            0,
            &mut ffi_array as *mut _,
            &mut ffi_schema as *mut _,
        )
    };

    // The catch_unwind wrapper must have caught the panic and returned the
    // Panicked code.  If the process is still alive at this point, catch_unwind
    // worked correctly (DUCK-04).
    assert_eq!(
        code,
        LoomError::Panicked.code(),
        "a panic inside loom_decode_inner must return the Panicked code ({}), got {}",
        LoomError::Panicked.code(),
        code
    );

    // Execution reaching here confirms the test process did not abort.
    // The test runner reports success, which is the proof of panic safety.
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

fn alp_desc(data_type: DataType, params: AlpParams) -> LayoutDescription {
    let count = params.mantissas.len();
    LayoutDescription {
        data_type,
        root: LayoutNode::KernelEscape {
            kernel_id: 1,
            params: params.encode(),
            count,
        },
        row_count: count,
    }
}
