//! FFI surface — `extern "C"` entry point for the Loom decoder.
//!
//! # Contract (locked in Phase 1, Plan 02)
//!
//! - **Signature:** `loom_decode(input_ptr, input_len, out_array, out_schema) -> i32`
//! - **Error strategy:** integer return code. `0` = success; nonzero = error.
//!   All error paths are explicitly enumerated by [`LoomError`].
//! - **No `loom_free`:** buffer teardown is owned by the Arrow release callback
//!   installed by `to_ffi`. The C++ side must call `array.release(&array)` and
//!   `schema.release(&schema)` when done (ARROW-03, PITFALLS P1, T-01-06).
//! - **Panic safety:** the entire body of `loom_decode` is wrapped in
//!   `std::panic::catch_unwind`. Any caught panic maps to
//!   [`LoomError::Panicked`] (DUCK-04, PITFALLS P3, T-01-05).
//!
//! # Ownership protocol (PITFALLS P1, T-01-06)
//!
//! `to_ffi` returns owned `FFI_ArrowArray` and `FFI_ArrowSchema` values.
//! Each is moved into the caller-provided slot via exactly one
//! `std::ptr::write`. After the write the Rust binding is forgotten — the
//! release callback is the sole remaining owner. **Never clone these structs.**

use std::panic::{self, AssertUnwindSafe};

use arrow::array::Array;
use arrow::ffi::{to_ffi, FFI_ArrowArray, FFI_ArrowSchema};
use loom_core::container_codec::decode_layout_payload_maybe_container;
use loom_core::l1_model::decode_layout_to_array_data;
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::verifier::verify_layout;

// ---------------------------------------------------------------------------
// Error enum
// ---------------------------------------------------------------------------

/// Errors that can be returned across the FFI boundary.
///
/// Each variant maps to a distinct nonzero `i32` code (T-01-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum LoomError {
    /// A required output pointer (`out_array` or `out_schema`) was null, or
    /// the input pointer was null with a non-zero length (T-01-08).
    NullPointer = 1,
    /// The decode operation failed (e.g. Arrow error during `to_ffi`).
    DecodeFailed = 2,
    /// A panic was caught inside the inner decode body (T-01-05, DUCK-04).
    Panicked = 3,
}

impl LoomError {
    /// Convert to the wire `i32` code.
    #[inline]
    pub fn code(self) -> i32 {
        self as i32
    }
}

// ---------------------------------------------------------------------------
// Test panic sentinel
// ---------------------------------------------------------------------------

// Thread-local panic sentinel for testing the `catch_unwind` path.
//
// Using a thread-local avoids races between concurrently executing tests —
// each test thread has its own copy, so arming the sentinel in one test
// cannot accidentally affect a sibling test running on a different thread.
//
// This is always present (not `#[cfg(test)]`) so that integration tests in
// `tests/roundtrip.rs` (which link the library in non-test mode) can reach
// it.  It has no observable effect unless explicitly set via `set_panic_sentinel`.
thread_local! {
    static PANIC_SENTINEL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Arm the thread-local panic sentinel for the next `loom_decode` call on
/// this thread.
///
/// The sentinel is consumed (reset to `false`) atomically on the first call to
/// `loom_decode_inner` after it is set.  Call this from a test immediately
/// before calling `loom_decode` to exercise the `catch_unwind` path
/// (DUCK-04, T-01-05).
///
/// # Note
///
/// This function is intended for testing only.  Do not call it in production.
pub fn set_panic_sentinel() {
    PANIC_SENTINEL.with(|s| s.set(true));
}

// ---------------------------------------------------------------------------
// Inner (safe) decode function
// ---------------------------------------------------------------------------

/// Safe inner decode function — all real work happens here.
///
/// Returns `Ok(())` on success; the caller writes the produced FFI structs into
/// the caller-provided output slots via `ptr::write` (PITFALLS P1/P2).
///
/// # Why this exists
///
/// `loom_decode` is `unsafe extern "C"` and must not contain business logic
/// (ARCHITECTURE anti-pattern 3). This function is safe Rust; it is called
/// from within the `catch_unwind` wrapper so that any panic is caught at the
/// FFI boundary rather than unwinding past it.
fn loom_decode_inner(
    input: &[u8],
    out_array: *mut FFI_ArrowArray,
    out_schema: *mut FFI_ArrowSchema,
) -> Result<(), LoomError> {
    // Check the thread-local panic sentinel (set by tests via
    // `set_panic_sentinel()`).  Reads and resets the flag so it fires at most
    // once per arming.  In production the flag is always `false`; this branch
    // is never taken (DUCK-04, T-01-05).
    let sentinel_armed = PANIC_SENTINEL.with(|s| {
        let was_armed = s.get();
        if was_armed {
            s.set(false); // consume: fire exactly once
        }
        was_armed
    });
    if sentinel_armed {
        panic!("loom_decode_inner: panic sentinel triggered (test-only path)");
    }

    let array_data = if input.is_empty() {
        // Build a hardcoded minimal Int32Array: [1, 2, 3, null].
        // The null exercises the validity bitmap path (PITFALLS P7).
        //
        // We use the builder API (not from_vec) so the null is real — the builder
        // installs a proper null bitmap rather than a placeholder.
        use arrow::array::Int32Builder;
        let mut builder = Int32Builder::new();
        builder.append_value(1);
        builder.append_value(2);
        builder.append_value(3);
        builder.append_null();
        builder.finish().into_data()
    } else {
        let desc =
            decode_layout_payload_maybe_container(input).map_err(|_| LoomError::DecodeFailed)?;
        let registry = L2KernelRegistry::default_for_mvp0();
        let report = verify_layout(&desc, &registry);
        if !report.is_ok() {
            return Err(LoomError::DecodeFailed);
        }
        decode_layout_to_array_data(&desc, &registry).map_err(|_| LoomError::DecodeFailed)?
    };

    // Produce the FFI pair.  The `Field` for the schema: Int32, nullable.
    // `to_ffi` accepts a reference to ArrayData, so we borrow here.
    let (ffi_array, ffi_schema) = to_ffi(&array_data).map_err(|_| LoomError::DecodeFailed)?;

    // Move each struct into the caller's slot via exactly one `ptr::write`.
    //
    // PITFALLS P1: `ptr::write` moves the value bitwise; the local binding is
    // forgotten (no Drop runs). The release callback is now the sole owner.
    // PITFALLS P2: each struct is written independently — schema lifetime is
    // not tied to array lifetime.
    //
    // Safety: the caller (loom_decode) already checked that both pointers are
    // non-null and properly aligned for their respective types before calling
    // this inner function.
    unsafe {
        std::ptr::write(out_array, ffi_array); // one write for array  (ARROW-03)
        std::ptr::write(out_schema, ffi_schema); // one write for schema (ARROW-03)
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Public extern "C" entry point
// ---------------------------------------------------------------------------

/// Export a minimal Arrow array across the C Data Interface.
///
/// # Parameters
///
/// - `input_ptr` — pointer to the encoded input bytes (may be null only if
///   `input_len` is 0).
/// - `input_len` — number of valid bytes at `input_ptr`.
/// - `out_array` — caller-allocated `FFI_ArrowArray` shell (must be non-null).
/// - `out_schema` — caller-allocated `FFI_ArrowSchema` shell (must be non-null).
///
/// # Return value
///
/// `0` on success; nonzero on error (see [`LoomError`]).
///
/// # Safety
///
/// The caller must ensure:
/// - `out_array` and `out_schema` point to caller-allocated, properly aligned,
///   writeable memory for their respective types.
/// - `input_ptr` is either null (with `input_len == 0`) or points to at least
///   `input_len` valid bytes.
/// - The written `FFI_ArrowArray` and `FFI_ArrowSchema` are eventually released
///   by calling their respective `release` callbacks exactly once.
#[no_mangle]
pub unsafe extern "C" fn loom_decode(
    input_ptr: *const u8,
    input_len: usize,
    out_array: *mut FFI_ArrowArray,
    out_schema: *mut FFI_ArrowSchema,
) -> i32 {
    // --- Null-pointer guard (T-01-08) ---
    if out_array.is_null() || out_schema.is_null() {
        return LoomError::NullPointer.code();
    }
    if input_len > 0 && input_ptr.is_null() {
        return LoomError::NullPointer.code();
    }

    // Reconstruct the input slice.  Zero-length inputs are fine (stub ignores
    // the bytes; Phase 3+ will parse them).
    let input: &[u8] = if input_len == 0 {
        &[]
    } else {
        // Safety: caller guarantees validity; we checked non-null above.
        std::slice::from_raw_parts(input_ptr, input_len)
    };

    // --- catch_unwind wrapper (DUCK-04, PITFALLS P3, T-01-05) ---
    //
    // Any panic inside `loom_decode_inner` (or anything it calls) is caught
    // here.  The catch converts it to LoomError::Panicked so the process is
    // not aborted.
    //
    // `AssertUnwindSafe` is required because raw pointers are not `UnwindSafe`
    // by default; the closure captures nothing shared, so the assertion is
    // sound.
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        loom_decode_inner(input, out_array, out_schema)
    }));

    match result {
        Ok(Ok(())) => 0,
        Ok(Err(e)) => e.code(),
        Err(_panic_payload) => LoomError::Panicked.code(),
    }
}

// ---------------------------------------------------------------------------
// Tests — these live in the same module so they can access private helpers.
// The integration tests in tests/roundtrip.rs call loom_decode directly.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: verify the error enum codes are distinct and nonzero.
    #[test]
    fn error_codes_are_nonzero_and_distinct() {
        assert_ne!(LoomError::NullPointer.code(), 0);
        assert_ne!(LoomError::DecodeFailed.code(), 0);
        assert_ne!(LoomError::Panicked.code(), 0);
        assert_ne!(
            LoomError::NullPointer.code(),
            LoomError::DecodeFailed.code()
        );
        assert_ne!(LoomError::NullPointer.code(), LoomError::Panicked.code());
        assert_ne!(LoomError::DecodeFailed.code(), LoomError::Panicked.code());
    }

    /// Null out_array → NullPointer code.
    #[test]
    fn null_out_array_returns_null_pointer_code() {
        let mut schema = unsafe { std::mem::zeroed::<FFI_ArrowSchema>() };
        let result = unsafe {
            loom_decode(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                &mut schema as *mut _,
            )
        };
        assert_eq!(result, LoomError::NullPointer.code());
    }

    /// Null out_schema → NullPointer code.
    #[test]
    fn null_out_schema_returns_null_pointer_code() {
        let mut array = unsafe { std::mem::zeroed::<FFI_ArrowArray>() };
        let result = unsafe {
            loom_decode(
                std::ptr::null(),
                0,
                &mut array as *mut _,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(result, LoomError::NullPointer.code());
    }

    #[test]
    fn malformed_verified_payload_returns_decode_failed() {
        use arrow::datatypes::DataType;
        use loom_core::l1_model::{LayoutDescription, LayoutNode};
        use loom_core::layout_codec::encode_layout_payload;

        let desc = LayoutDescription {
            data_type: DataType::Int64,
            root: LayoutNode::BitPack {
                values_buf: vec![],
                bit_width: 65,
                offset: 0,
                count: 1,
                validity: None,
                all_null: false,
            },
            row_count: 1,
        };
        let payload = encode_layout_payload(&desc);
        let mut array = unsafe { std::mem::zeroed::<FFI_ArrowArray>() };
        let mut schema = unsafe { std::mem::zeroed::<FFI_ArrowSchema>() };

        let result = unsafe {
            loom_decode(
                payload.as_ptr(),
                payload.len(),
                &mut array as *mut _,
                &mut schema as *mut _,
            )
        };

        assert_eq!(result, LoomError::DecodeFailed.code());
    }
}
