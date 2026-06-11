//! C ABI entry points for Loom sidecar operations — Phase 51-01.
//!
//! Four public `extern "C"` functions provide sidecar extract, verify, routing,
//! and memory-free operations through a lean FFI surface.  All entry points are
//! wrapped in `catch_unwind(AssertUnwindSafe(...))` so panics never unwind
//! across the C ABI (T-51-02).
//!
//! This FFI surface depends only on `loom-ir-core` and `loom-parquet-ingress`.
//! Zero transitive dependency on `loom-core`, `loom-container`, or Arrow types.
//!
//! # Error codes (LoomSidecarError, repr(i32))
//!
//! | Code | Name              | Meaning                                      |
//! |------|-------------------|----------------------------------------------|
//! | 0    | Success           | Operation completed successfully             |
//! | 1    | NullPointer       | A required pointer argument is null           |
//! | 2    | IoError           | File I/O or path resolution failed           |
//! | 3    | DecodeFailed      | Malformed, truncated, or invalid sidecar data |
//! | 4    | Panicked          | A Rust panic was caught at the FFI boundary  |
//! | 5    | NoSidecar         | No Loom sidecar found in the host file        |

use std::ffi::{c_char, CStr, CString};
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use loom_ir_core::l2core_codec;
use loom_ir_core::sidecar::SidecarOverlay;
use loom_ir_core::sidecar_routing::{
    decide_sidecar_routing, SidecarRoutingDecision, SidecarRoutingInput,
};
use loom_parquet_ingress::sidecar_parquet::extract_sidecar_from_parquet_metadata;

// ---------------------------------------------------------------------------
// Error codes
// ---------------------------------------------------------------------------

/// Module-private error type — not exposed through the C ABI.  Every
/// `extern "C"` function returns `i32`; consumers interpret these codes.
/// This enum is excluded from the cbindgen output so it never appears in
/// `loom.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
enum LoomSidecarError {
    Success = 0,
    NullPointer = 1,
    IoError = 2,
    DecodeFailed = 3,
    Panicked = 4,
    NoSidecar = 5,
}

impl LoomSidecarError {
    fn code(self) -> i32 {
        self as i32
    }
}

// ---------------------------------------------------------------------------
// loom_sidecar_extract
// ---------------------------------------------------------------------------

/// Extract the sidecar overlay from a Parquet file.
///
/// Opens the Parquet file at `file_path`, reads its metadata, and attempts to
/// extract a `"loom.sidecar.v1"` key-value entry.  On success, the encoded
/// overlay bytes are boxed and returned through `out_bytes`/`out_len`.
/// The caller must free the returned buffer via [`loom_sidecar_free_bytes`].
///
/// # Returns
///
/// * `0` — Sidecar found, bytes written to `out_bytes`/`out_len`.
/// * `1` — `file_path`, `out_bytes`, or `out_len` is null.
/// * `2` — File could not be opened or read.
/// * `3` — Sidecar data found but could not be decoded.
/// * `4` — Internal panic caught.
/// * `5` — No sidecar key found in the file's metadata.
#[no_mangle]
pub unsafe extern "C" fn loom_sidecar_extract(
    file_path: *const c_char,
    out_bytes: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    if file_path.is_null() || out_bytes.is_null() || out_len.is_null() {
        return LoomSidecarError::NullPointer.code();
    }

    let result: std::result::Result<std::result::Result<i32, LoomSidecarError>, _> =
        panic::catch_unwind(AssertUnwindSafe(|| {
        let path = CStr::from_ptr(file_path).to_string_lossy();
        let file = std::fs::File::open(Path::new(path.as_ref()))
            .map_err(|_| LoomSidecarError::IoError)?;

        let builder = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)
            .map_err(|_| LoomSidecarError::IoError)?;

        let metadata = builder.metadata();

        let overlay = extract_sidecar_from_parquet_metadata(metadata)
            .map_err(|_| LoomSidecarError::DecodeFailed)?;

        match overlay {
            None => Err(LoomSidecarError::NoSidecar),
            Some(sidecar) => {
                let encoded = sidecar.encode();
                let boxed = encoded.into_boxed_slice();
                let (ptr, len) = (boxed.as_ptr(), boxed.len());
                std::mem::forget(boxed);
                std::ptr::write(out_bytes, ptr as *mut u8);
                std::ptr::write(out_len, len);
                Ok(0)
            }
        }
    }));

    match result {
        Ok(Ok(0)) => LoomSidecarError::Success.code(),
        Ok(Err(e)) => e.code(),
        Ok(_) => LoomSidecarError::DecodeFailed.code(),
        Err(_) => LoomSidecarError::Panicked.code(),
    }
}

// ---------------------------------------------------------------------------
// loom_sidecar_verify
// ---------------------------------------------------------------------------

/// Verify a sidecar overlay's L2Core IR and compute its content-hash identity.
///
/// Decodes the overlay bytes, decodes the inner IR bytes into a
/// [`L2CoreProgram`], and computes the FNV-1a content-hash identity string.
/// The hash is returned as a null-terminated C string through `out_hash`.
/// The caller must free this string via `loom_sidecar_free_cstr`.
///
/// # Returns
///
/// * `0` — Overlay decoded and verified, hash written to `out_hash`.
/// * `1` — `overlay_bytes` or `out_hash` is null.
/// * `3` — Overlay or IR bytes are malformed/truncated.
/// * `4` — Internal panic caught.
#[no_mangle]
pub unsafe extern "C" fn loom_sidecar_verify(
    overlay_bytes: *const u8,
    overlay_len: usize,
    out_hash: *mut *const c_char,
) -> i32 {
    if overlay_bytes.is_null() || out_hash.is_null() {
        return LoomSidecarError::NullPointer.code();
    }

    let result: std::result::Result<std::result::Result<i32, LoomSidecarError>, _> =
        panic::catch_unwind(AssertUnwindSafe(|| {
        let bytes = std::slice::from_raw_parts(overlay_bytes, overlay_len);
        let overlay =
            SidecarOverlay::decode(bytes).map_err(|_| LoomSidecarError::DecodeFailed)?;

        // Decode the inner L2Core IR bytes to validate them, then compute
        // the content-hash identity over the full L2IR-format encoding.
        let program = l2core_codec::decode_l2core_program(&overlay.ir_bytes)
            .map_err(|_| LoomSidecarError::DecodeFailed)?;
        let hash = l2core_codec::l2core_program_hash(&program);

        let cstr =
            CString::new(hash).map_err(|_| LoomSidecarError::DecodeFailed)?;
        let ptr = cstr.into_raw();
        std::ptr::write(out_hash, ptr);
        Ok(0)
    }));

    match result {
        Ok(Ok(0)) => LoomSidecarError::Success.code(),
        Ok(Err(e)) => e.code(),
        Ok(_) => LoomSidecarError::DecodeFailed.code(),
        Err(_) => LoomSidecarError::Panicked.code(),
    }
}

// ---------------------------------------------------------------------------
// loom_sidecar_route
// ---------------------------------------------------------------------------

/// Evaluate the 4-gate sidecar routing decision.
///
/// Decodes the overlay, verifies each chunk binding against the provided host
/// data bytes (if any), and runs the full routing gate.  The routing decision
/// is returned as a JSON string through `out_decision`.
/// The caller must free this string via `loom_sidecar_free_cstr`.
///
/// # Returns
///
/// * `0` — Routing decision computed, JSON written to `out_decision`.
/// * `1` — A required pointer argument is null.
/// * `3` — Overlay decode or routing failure.
/// * `4` — Internal panic caught.
#[no_mangle]
pub unsafe extern "C" fn loom_sidecar_route(
    overlay_bytes: *const u8,
    overlay_len: usize,
    host_data: *const u8,
    host_data_len: usize,
    out_decision: *mut *const c_char,
) -> i32 {
    if overlay_bytes.is_null() || out_decision.is_null() {
        return LoomSidecarError::NullPointer.code();
    }

    let result: std::result::Result<std::result::Result<i32, LoomSidecarError>, _> =
        panic::catch_unwind(AssertUnwindSafe(|| {
        let bytes = std::slice::from_raw_parts(overlay_bytes, overlay_len);
        let overlay =
            SidecarOverlay::decode(bytes).map_err(|_| LoomSidecarError::DecodeFailed)?;

        // Build hash verification results.  Only verify bindings whose
        // host_data_range is within the provided host_data slice bounds.
        let mut hash_results = Vec::with_capacity(overlay.bindings.len());

        let host_slice = if host_data.is_null() {
            &[][..]
        } else {
            std::slice::from_raw_parts(host_data, host_data_len)
        };

        for binding in &overlay.bindings {
            let (offset, length) = binding.host_data_range;
            let end = offset.saturating_add(length);
            if (end as usize) <= host_slice.len() && (offset as usize) <= host_slice.len() {
                let chunk = &host_slice[(offset as usize)..(end as usize)];
                let result =
                    loom_ir_core::sidecar::verify_chunk_binding(binding, chunk);
                hash_results.push(result);
            } else {
                // Binding range out of host_data bounds — create a non-matching
                // result with the expected hash but a distinct recomputed hash.
                let result = loom_ir_core::sidecar::HashVerificationResult {
                    granule_id: binding.granule_id.clone(),
                    binding: binding.clone(),
                    recomputed_hash: "l2ir:0000000000000000".to_string(),
                    matches: false,
                };
                hash_results.push(result);
            }
        }

        let input = SidecarRoutingInput {
            engine_integrated: true,
            sidecar: Some(overlay),
            hash_verification: hash_results,
            encoding_supported: true,
        };

        let decision = decide_sidecar_routing(input);
        let json = routing_decision_to_json(&decision)
            .map_err(|_| LoomSidecarError::DecodeFailed)?;

        let cstr =
            CString::new(json).map_err(|_| LoomSidecarError::DecodeFailed)?;
        let ptr = cstr.into_raw();
        std::ptr::write(out_decision, ptr);
        Ok(0)
    }));

    match result {
        Ok(Ok(0)) => LoomSidecarError::Success.code(),
        Ok(Err(e)) => e.code(),
        Ok(_) => LoomSidecarError::DecodeFailed.code(),
        Err(_) => LoomSidecarError::Panicked.code(),
    }
}

// ---------------------------------------------------------------------------
// loom_sidecar_free_bytes
// ---------------------------------------------------------------------------

/// Free a byte buffer previously returned by [`loom_sidecar_extract`].
///
/// Reconstructs the `Vec<u8>` from the pointer and length and drops it.
/// The caller must ensure `ptr` and `len` came from a prior call to
/// `loom_sidecar_extract` and that this function is called at most once
/// per allocation.
///
/// # Returns
///
/// * `0` — Buffer freed.
/// * `1` — `ptr` is null.
#[no_mangle]
pub unsafe extern "C" fn loom_sidecar_free_bytes(ptr: *mut u8, len: usize) -> i32 {
    if ptr.is_null() {
        return LoomSidecarError::NullPointer.code();
    }

    let result: std::result::Result<std::result::Result<i32, LoomSidecarError>, _> =
        panic::catch_unwind(AssertUnwindSafe(|| {
        // Safety: ptr+len must describe a valid allocation from the global
        // allocator (guaranteed by the contract — caller must pass values
        // obtained from loom_sidecar_extract).  The capacity equals the
        // length because the buffer was constructed from an owned Vec<u8>
        // via into_boxed_slice().
        unsafe {
            let _ = Vec::from_raw_parts(ptr, len, len);
        }
        Ok(0)
    }));

    match result {
        Ok(Ok(0)) => LoomSidecarError::Success.code(),
        Ok(Err(e)) => e.code(),
        Ok(_) => LoomSidecarError::DecodeFailed.code(),
        Err(_) => LoomSidecarError::Panicked.code(),
    }
}

// ---------------------------------------------------------------------------
// loom_sidecar_free_cstr
// ---------------------------------------------------------------------------

/// Free a C string previously returned by [`loom_sidecar_verify`] or
/// [`loom_sidecar_route`].
///
/// Reconstructs the `CString` from the raw pointer and drops it.  The caller
/// must ensure `ptr` came from a prior call to `loom_sidecar_verify` or
/// `loom_sidecar_route` and that this function is called at most once per
/// allocation.
///
/// # Returns
///
/// * `0` — String freed.
/// * `1` — `ptr` is null.
#[no_mangle]
pub unsafe extern "C" fn loom_sidecar_free_cstr(ptr: *mut c_char) -> i32 {
    if ptr.is_null() {
        return LoomSidecarError::NullPointer.code();
    }

    let result: std::result::Result<std::result::Result<i32, LoomSidecarError>, _> =
        panic::catch_unwind(AssertUnwindSafe(|| {
        // Safety: ptr must describe a valid CString allocation from the
        // global allocator (guaranteed by the contract — caller must pass
        // values obtained from loom_sidecar_verify or loom_sidecar_route).
        unsafe {
            let _ = CString::from_raw(ptr);
        }
        Ok(0)
    }));

    match result {
        Ok(Ok(0)) => LoomSidecarError::Success.code(),
        Ok(Err(e)) => e.code(),
        Ok(_) => LoomSidecarError::DecodeFailed.code(),
        Err(_) => LoomSidecarError::Panicked.code(),
    }
}

// ---------------------------------------------------------------------------
// Routing decision → JSON formatting
// ---------------------------------------------------------------------------

/// Serialize a routing decision into a compact JSON string.
///
/// Mirrors the Java-friendly sidecar-routing output format.  The result is a
/// single-line JSON value without whitespace, suitable for C consumers to parse
/// with minimal tooling.
fn routing_decision_to_json(
    decision: &SidecarRoutingDecision,
) -> Result<String, std::fmt::Error> {
    use std::fmt::Write;

    let mut buf = String::with_capacity(512);
    match decision {
        SidecarRoutingDecision::LoomNative {
            ref verified_bindings,
            ..
        } => {
            buf.push_str("{\"decision\":\"LoomNative\",\"verified_bindings\":[");
            for (i, b) in verified_bindings.iter().enumerate() {
                if i > 0 {
                    buf.push(',');
                }
                write!(
                    &mut buf,
                    "{{\"granule_id\":{},\"content_hash\":{}}}",
                    json_string(&b.granule_id),
                    json_string(&b.content_hash)
                )?;
            }
            buf.push_str("]}");
        }
        SidecarRoutingDecision::HostNativeReader {
            reason,
            ref diagnostics,
        } => {
            // Serialize the reason using its stable Display representation.
            let reason_str = format!("{reason}");
            buf.push_str("{\"decision\":\"HostNativeReader\",");
            write!(&mut buf, "\"reason\":\"{reason_str}\",")?;
            buf.push_str("\"diagnostics\":[");
            for (i, d) in diagnostics.iter().enumerate() {
                if i > 0 {
                    buf.push(',');
                }
                write!(
                    &mut buf,
                    "{{\"code\":{},\"path\":{},\"message\":{}}}",
                    json_string(&d.code.to_string()),
                    json_string(&d.path),
                    json_string(&d.message)
                )?;
            }
            buf.push_str("]}");
        }
    }
    Ok(buf)
}

/// Format a Rust string as a JSON string value (with surrounding quotes).
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0C' => out.push_str("\\f"),
            c if c < ' ' => {
                use std::fmt::Write;
                write!(&mut out, "\\u{:04x}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
