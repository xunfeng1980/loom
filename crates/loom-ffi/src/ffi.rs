//! C ABI entry points for Loom sidecar operations — Phase 51-01.
//!
//! Four public `extern "C"` functions provide sidecar extract, verify, routing,
//! and memory-free operations through a lean FFI surface.  All entry points are
//! wrapped in `catch_unwind(AssertUnwindSafe(...))` so panics never unwind
//! across the C ABI (T-51-02).
//!
//! This FFI surface depends on `loom-ir-core`, `loom-parquet-ingress`, and the
//! in-crate `interp` engine. The Loom-native decode path produces Arrow
//! (`arrow_array::RecordBatch`) via the general L2Core interpreter; zero
//! transitive dependency on `loom-core` or `loom-container`.
//!
//! # Error codes (LoomSidecarError, repr(i32))
//!
//! | Code | Name               | Meaning                                      |
//! |------|--------------------|----------------------------------------------|
//! | 0    | Success            | Operation completed successfully             |
//! | 1    | NullPointer        | A required pointer argument is null           |
//! | 2    | IoError            | File I/O or path resolution failed           |
//! | 3    | DecodeFailed       | Malformed, truncated, or invalid sidecar data |
//! | 4    | Panicked           | A Rust panic was caught at the FFI boundary  |
//! | 5    | NoSidecar          | No Loom sidecar found in the host file        |
//! | 6    | VerificationFailed | L2Core IR program failed semantic verification|

use std::ffi::{c_char, c_void, CStr, CString};
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use loom_ir_core::full_verifier::verify_l2_core_bytes;
use loom_ir_core::l2core_codec;
use loom_ir_core::sidecar::SidecarOverlay;
use loom_ir_core::sidecar_routing::{
    decide_sidecar_routing, SidecarRoutingDecision, SidecarRoutingInput,
};

// The set of L2Core IR features supported by this runtime.
// Gate 4 (encoding_supported) checks programs against this set.
const SUPPORTED_FEATURES: &[&str] = &["l2core.copy.v0"];

fn check_encoding_supported(ir_bytes: &[u8]) -> bool {
    let Ok(program) = l2core_codec::decode_l2core_program(ir_bytes) else {
        return false;
    };
    program
        .required_features
        .iter()
        .all(|f| SUPPORTED_FEATURES.contains(&f.as_str()))
}
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
    VerificationFailed = 6,
}

impl LoomSidecarError {
    fn code(self) -> i32 {
        self as i32
    }
}

// ---------------------------------------------------------------------------
// loom_sidecar_extract
// ---------------------------------------------------------------------------

/// Extract the sidecar overlay from a host file.
///
/// Tries in order:
/// 1. External sidecar file at `<file_path>.loomsidecar` (production path).
/// 2. Embedded `"loom.sidecar.v1"` KeyValue metadata in Parquet metadata.
///
/// On success, the encoded overlay bytes are boxed and returned through
/// `out_bytes`/`out_len`. The caller must free the returned buffer via
/// [`loom_sidecar_free_bytes`].
///
/// # Returns
///
/// * `0` — Sidecar found, bytes written to `out_bytes`/`out_len`.
/// * `1` — `file_path`, `out_bytes`, or `out_len` is null.
/// * `2` — File could not be opened or read.
/// * `3` — Sidecar data found but could not be decoded.
/// * `4` — Internal panic caught.
/// * `5` — No sidecar found (neither external nor embedded).
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
        let path_str = CStr::from_ptr(file_path).to_string_lossy();
        let path = Path::new(path_str.as_ref());

        // P2-1: Try external sidecar file first (production path).
        let external_path_str = format!("{}.loomsidecar", path.display());
        let external = Path::new(&external_path_str);
        if external.exists() {
            let raw = std::fs::read(external)
                .map_err(|_| LoomSidecarError::IoError)?;
            // Validate that the bytes are a valid sidecar overlay.
            let _ = SidecarOverlay::decode(&raw)
                .map_err(|_| LoomSidecarError::DecodeFailed)?;
            let boxed = raw.into_boxed_slice();
            let (ptr, len) = (boxed.as_ptr(), boxed.len());
            std::mem::forget(boxed);
            std::ptr::write(out_bytes, ptr as *mut u8);
            std::ptr::write(out_len, len);
            return Ok(0);
        }

        // Fall back to embedded Parquet metadata.
        let file = std::fs::File::open(path)
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
/// Decodes the overlay bytes, runs full semantic verification of the inner
/// L2Core IR program via [`verify_l2_core_bytes`], and computes the FNV-1a
/// content-hash identity string. The hash is returned as a null-terminated
/// C string through `out_hash`. The caller must free this string via
/// `loom_sidecar_free_cstr`.
///
/// # Returns
///
/// * `0` — Overlay decoded, IR semantically verified, hash written to `out_hash`.
/// * `1` — `overlay_bytes` or `out_hash` is null.
/// * `3` — Overlay or IR bytes are malformed/truncated.
/// * `6` — IR program failed semantic verification.
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

        // Decode the inner L2Core IR program, then run full semantic
        // verification and compute the content-hash identity.
        let program = l2core_codec::decode_l2core_program(&overlay.ir_bytes)
            .map_err(|_| LoomSidecarError::DecodeFailed)?;

        let report = verify_l2_core_bytes(&overlay.ir_bytes);
        if !report.is_ok() {
            return Err(LoomSidecarError::VerificationFailed);
        }

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
                    recomputed_hash: "blake3:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                    matches: false,
                };
                hash_results.push(result);
            }
        }

        // Check encoding support against the runtime's supported feature set.
        let encoding_ok = check_encoding_supported(&overlay.ir_bytes);

        let input = SidecarRoutingInput {
            engine_integrated: true,
            sidecar: Some(overlay),
            hash_verification: hash_results,
            encoding_supported: encoding_ok,
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
// loom_sidecar_verify_json (P1-1: structured facts/diagnostics)
// ---------------------------------------------------------------------------

/// Verify a sidecar overlay's L2Core IR and return structured facts and
/// diagnostics as a JSON string.
///
/// Decodes the overlay, runs full semantic verification via
/// [`verify_l2_core_bytes`], and emits a JSON object with the verification
/// result, content-hash identity, artifact facts, and diagnostic messages.
/// The JSON is returned as a null-terminated C string through `out_json`.
/// The caller must free this string via `loom_sidecar_free_cstr`.
///
/// # JSON schema
///
/// ```json
/// {
///   "accepted": true,
///   "ir_hash": "blake3:...",
///   "artifact_version": 1,
///   "required_features": [],
///   "input_ranges": [],
///   "output_schema": [],
///   "row_count_bound": 1024,
///   "constraint_count": 0,
///   "proof_obligation_count": 0,
///   "diagnostics": []
/// }
/// ```
///
/// # Returns
///
/// * `0` — Verification completed, JSON written to `out_json`.
/// * `1` — A required pointer argument is null.
/// * `3` — Overlay or IR bytes are malformed/truncated.
/// * `4` — Internal panic caught.
#[no_mangle]
pub unsafe extern "C" fn loom_sidecar_verify_json(
    overlay_bytes: *const u8,
    overlay_len: usize,
    out_json: *mut *const c_char,
) -> i32 {
    if overlay_bytes.is_null() || out_json.is_null() {
        return LoomSidecarError::NullPointer.code();
    }

    let result: std::result::Result<std::result::Result<i32, LoomSidecarError>, _> =
        panic::catch_unwind(AssertUnwindSafe(|| {
        let bytes = std::slice::from_raw_parts(overlay_bytes, overlay_len);
        let overlay =
            SidecarOverlay::decode(bytes).map_err(|_| LoomSidecarError::DecodeFailed)?;

        let report = verify_l2_core_bytes(&overlay.ir_bytes);

        // Compute hash from successfully decoded program (or empty if
        // decode failed — fail-closed: no hash on malformed input).
        let ir_hash = l2core_codec::decode_l2core_program(&overlay.ir_bytes)
            .as_ref()
            .map(l2core_codec::l2core_program_hash)
            .unwrap_or_default();

        let facts = report.facts();
        let diags = report.diagnostics();

        let mut diagnostics_json = String::new();
        for (i, d) in diags.iter().enumerate() {
            if i > 0 {
                diagnostics_json.push(',');
            }
            diagnostics_json.push_str(&format!(
                r#"{{"code":"{:?}","path":"{}","message":"{}"}}"#,
                d.code, d.path, d.message
            ));
        }

        let mut input_ranges_json = String::new();
        let mut output_schema_json = String::new();
        let mut artifact_version: u16 = 0;
        let mut required_features_json = String::new();
        let mut row_count_bound: Option<u64> = None;
        let mut constraint_count: usize = 0;
        let mut proof_count: usize = 0;

        if let Some(f) = facts {
            artifact_version = f.artifact_version;
            row_count_bound = f.row_count_bound;
            constraint_count = f.constraint_ids.len();
            proof_count = f.proof_obligation_ids.len();

            for (i, feat) in f.required_features.iter().enumerate() {
                if i > 0 {
                    required_features_json.push(',');
                }
                required_features_json.push_str(&format!(r#""{}""#, feat));
            }

            for (i, ir) in f.input_ranges.iter().enumerate() {
                if i > 0 {
                    input_ranges_json.push(',');
                }
                input_ranges_json.push_str(&format!(
                    r#"{{"id":"{}","offset":{},"length":{}}}"#,
                    ir.capability_id, ir.offset, ir.length
                ));
            }

            for (i, os) in f.output_schema.iter().enumerate() {
                if i > 0 {
                    output_schema_json.push(',');
                }
                output_schema_json.push_str(&format!(
                    r#"{{"id":"{}","arrow_type":"{:?}","nullable":{}}}"#,
                    os.builder_id, os.arrow_type, os.nullable
                ));
            }
        }

        let json = format!(
            r#"{{"accepted":{},"ir_hash":"{}","artifact_version":{},"required_features":[{}],"input_ranges":[{}],"output_schema":[{}],"row_count_bound":{},"constraint_count":{},"proof_obligation_count":{},"diagnostics":[{}]}}"#,
            report.is_ok(),
            ir_hash,
            artifact_version,
            required_features_json,
            input_ranges_json,
            output_schema_json,
            row_count_bound.map_or("null".to_string(), |v| v.to_string()),
            constraint_count,
            proof_count,
            diagnostics_json,
        );

        let cstr =
            CString::new(json).map_err(|_| LoomSidecarError::DecodeFailed)?;
        let ptr = cstr.into_raw();
        std::ptr::write(out_json, ptr);
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
// loom_sidecar_decode (P1-3: full decode execution loop)
// ---------------------------------------------------------------------------

/// Execute a verified Loom-native program through the general L2Core
/// interpreter, returning the decoded Arrow [`RecordBatch`].
///
/// Input slices are windowed from the host data using each program
/// `InputSlice` capability's declared `offset`/`length`. Any failure (slice
/// out of bounds, interpreter rejection, ragged columns) returns `Err` so the
/// caller fails closed to a host-native reader — no partial Arrow is emitted.
fn decode_loom_native_batch(
    program: &loom_ir_core::l2_core::L2CoreProgram,
    host: &[u8],
) -> Result<arrow_array::RecordBatch, LoomSidecarError> {
    use crate::interp::l2core_interp::{interpret_l2core, schema_from_columns, InputSlices};
    use loom_ir_core::l2_core::Capability;

    let mut inputs: InputSlices = InputSlices::new();
    for capability in &program.capabilities {
        if let Capability::InputSlice(slice) = capability {
            let start = slice.offset as usize;
            let end = start
                .checked_add(slice.length as usize)
                .ok_or(LoomSidecarError::DecodeFailed)?;
            if end > host.len() {
                return Err(LoomSidecarError::DecodeFailed);
            }
            inputs.insert(slice.id.clone(), &host[start..end]);
        }
    }

    let columns =
        interpret_l2core(program, &inputs).map_err(|_| LoomSidecarError::DecodeFailed)?;
    let schema = std::sync::Arc::new(schema_from_columns(&columns));
    let arrays: Vec<arrow_array::ArrayRef> = columns
        .iter()
        .map(|c| arrow_array::make_array(c.data.clone()))
        .collect();
    arrow_array::RecordBatch::try_new(schema, arrays).map_err(|_| LoomSidecarError::DecodeFailed)
}

/// Serialize a decoded [`RecordBatch`] to a **bare Arrow IPC stream** (the
/// Arrow C-Data / `arrow_scan`-consumable encoding — no `LMA1` container
/// header). Returned through `out_ipc_bytes` so DuckDB / nanoarrow can ingest
/// it directly without unwrapping a Loom container.
fn record_batch_to_ipc(batch: &arrow_array::RecordBatch) -> Result<Vec<u8>, LoomSidecarError> {
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut writer = arrow_ipc::writer::StreamWriter::try_new(&mut buf, &batch.schema())
            .map_err(|_| LoomSidecarError::DecodeFailed)?;
        writer
            .write(batch)
            .map_err(|_| LoomSidecarError::DecodeFailed)?;
        writer.finish().map_err(|_| LoomSidecarError::DecodeFailed)?;
    }
    Ok(buf)
}

// ---------------------------------------------------------------------------
// loom_sidecar_decode_carray — decode + export via Arrow C Data Interface
// ---------------------------------------------------------------------------

/// Decode a sidecar overlay and export the decoded columns as a single Arrow
/// C Data Interface **struct array**: one `ArrowSchema` + one `ArrowArray`
/// whose children are the output columns. This is the zero-copy boundary a
/// host engine (e.g. the DuckDB extension) uses to materialize typed rows.
///
/// `out_schema` / `out_array` are pointers to caller-allocated C
/// `ArrowSchema` / `ArrowArray` structs (passed as `void*`). On success (`0`)
/// both are populated and the **caller owns them** — it must invoke each
/// struct's `release` callback per the Arrow C Data Interface contract. On any
/// non-success code the out structs are left untouched and the caller falls
/// back to a host-native reader.
///
/// # Returns
/// * `0` — decoded; struct array written to the out pointers.
/// * `1` — a required pointer argument is null.
/// * `3` — overlay/IR malformed, or the program is not materializable.
/// * `4` — internal panic caught.
/// * `6` — the L2Core IR failed semantic verification.
///
/// # Safety
/// `overlay_bytes`/`host_data` must be valid for their stated lengths;
/// `out_schema`/`out_array` must point to writable `ArrowSchema`/`ArrowArray`.
#[no_mangle]
pub unsafe extern "C" fn loom_sidecar_decode_carray(
    overlay_bytes: *const u8,
    overlay_len: usize,
    host_data: *const u8,
    host_data_len: usize,
    out_schema: *mut c_void,
    out_array: *mut c_void,
) -> i32 {
    if overlay_bytes.is_null() || out_schema.is_null() || out_array.is_null() {
        return LoomSidecarError::NullPointer.code();
    }

    let result: std::result::Result<std::result::Result<i32, LoomSidecarError>, _> =
        panic::catch_unwind(AssertUnwindSafe(|| {
            let bytes = std::slice::from_raw_parts(overlay_bytes, overlay_len);
            let overlay =
                SidecarOverlay::decode(bytes).map_err(|_| LoomSidecarError::DecodeFailed)?;
            let program = l2core_codec::decode_l2core_program(&overlay.ir_bytes)
                .map_err(|_| LoomSidecarError::DecodeFailed)?;
            let report = verify_l2_core_bytes(&overlay.ir_bytes);
            if !report.is_ok() {
                return Err(LoomSidecarError::VerificationFailed);
            }

            let host_slice = if host_data.is_null() {
                &[][..]
            } else {
                std::slice::from_raw_parts(host_data, host_data_len)
            };
            let batch = decode_loom_native_batch(&program, host_slice)?;

            // Export the whole batch as one Arrow C struct array.
            let struct_array = arrow_array::StructArray::from(batch);
            let data = arrow_data::ArrayData::from(struct_array);
            let (ffi_array, ffi_schema) =
                arrow::ffi::to_ffi(&data).map_err(|_| LoomSidecarError::DecodeFailed)?;

            std::ptr::write(out_schema as *mut arrow::ffi::FFI_ArrowSchema, ffi_schema);
            std::ptr::write(out_array as *mut arrow::ffi::FFI_ArrowArray, ffi_array);
            Ok(0)
        }));

    match result {
        Ok(Ok(0)) => LoomSidecarError::Success.code(),
        Ok(Err(e)) => e.code(),
        Ok(_) => LoomSidecarError::DecodeFailed.code(),
        Err(_) => LoomSidecarError::Panicked.code(),
    }
}

/// Decode a sidecar overlay through the full Loom execution loop.
///
/// 1. Decodes the sidecar overlay and inner L2Core IR.
/// 2. Runs semantic verification.
/// 3. Evaluates the 4-gate routing decision.
/// 4. If Loom-native, runs the general L2Core interpreter and returns the
///    decoded columns as a **bare Arrow IPC stream** through `out_ipc_bytes` /
///    `out_ipc_len`. The buffer is a plain Arrow IPC stream (the encoding
///    produced by `arrow_ipc::writer::StreamWriter`) with **no `LMA1`/`LMC2`
///    container header** — consume it directly with `arrow_scan` / nanoarrow /
///    `StreamReader` without unwrapping a Loom container.
/// 5. Returns routing+execution metadata as a JSON string through `out_json`.
///
/// On any non-`loom-native` route (host-native fallback, verifier rejection,
/// unsupported encoding), `out_ipc_len` is `0` and the caller must use a
/// host-native reader. Always check the JSON `route` field before reading IPC.
///
/// The caller must free both outputs: `loom_sidecar_free_cstr` for the JSON,
/// `loom_sidecar_free_bytes` for the IPC buffer (safe to call on a zero-length
/// buffer).
///
/// # JSON schema
///
/// ```json
/// {
///   "route": "loom-native" | "host-native",
///   "reason": "all-gates-passed" | "hash-mismatch" | "encoding-unsupported" | ...,
///   "decode_status": "ok" | "unsupported" | "error",
///   "ir_hash": "blake3:...",
///   "row_count": 1024,
///   "column_count": 1,
///   "diagnostics": []
/// }
/// ```
///
/// # Returns
///
/// * `0` — Decode completed (check JSON `route` field for outcome).
/// * `1` — A required pointer argument is null.
/// * `3` — Overlay or IR bytes are malformed/truncated.
/// * `4` — Internal panic caught.
#[no_mangle]
pub unsafe extern "C" fn loom_sidecar_decode(
    overlay_bytes: *const u8,
    overlay_len: usize,
    host_data: *const u8,
    host_data_len: usize,
    out_json: *mut *const c_char,
    out_ipc_bytes: *mut *mut u8,
    out_ipc_len: *mut usize,
) -> i32 {
    if overlay_bytes.is_null()
        || out_json.is_null()
        || out_ipc_bytes.is_null()
        || out_ipc_len.is_null()
    {
        return LoomSidecarError::NullPointer.code();
    }

    let result: std::result::Result<std::result::Result<i32, LoomSidecarError>, _> =
        panic::catch_unwind(AssertUnwindSafe(|| {
        let bytes = std::slice::from_raw_parts(overlay_bytes, overlay_len);
        let overlay =
            SidecarOverlay::decode(bytes).map_err(|_| LoomSidecarError::DecodeFailed)?;

        let program = l2core_codec::decode_l2core_program(&overlay.ir_bytes)
            .map_err(|_| LoomSidecarError::DecodeFailed)?;

        let report = verify_l2_core_bytes(&overlay.ir_bytes);
        let ir_hash = l2core_codec::l2core_program_hash(&program);
        let encoding_ok = program
            .required_features
            .iter()
            .all(|f| SUPPORTED_FEATURES.contains(&f.as_str()));

        // Build hash verification results for routing
        let host_slice = if host_data.is_null() {
            &[][..]
        } else {
            std::slice::from_raw_parts(host_data, host_data_len)
        };
        let mut hash_results = Vec::with_capacity(overlay.bindings.len());
        for binding in &overlay.bindings {
            let (offset, length) = binding.host_data_range;
            let end = offset.saturating_add(length);
            if (end as usize) <= host_slice.len() && (offset as usize) <= host_slice.len() {
                let chunk = &host_slice[(offset as usize)..(end as usize)];
                hash_results.push(loom_ir_core::sidecar::verify_chunk_binding(binding, chunk));
            } else {
                hash_results.push(loom_ir_core::sidecar::HashVerificationResult {
                    granule_id: binding.granule_id.clone(),
                    binding: binding.clone(),
                    recomputed_hash: "blake3:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                    matches: false,
                });
            }
        }

        let routing = decide_sidecar_routing(SidecarRoutingInput {
            engine_integrated: true,
            sidecar: Some(overlay),
            hash_verification: hash_results,
            encoding_supported: encoding_ok,
        });

        // Loom-native decode runs the general L2Core interpreter and serializes
        // its output to a bare Arrow IPC stream. On any interpreter failure we
        // fail closed to a host-native reader (empty IPC) rather than emitting a
        // partial result.
        let mut decoded_batch: Option<arrow_array::RecordBatch> = None;
        let (route, reason, decode_status, row_count, col_count): (
            &str,
            String,
            &str,
            u64,
            usize,
        ) = match &routing {
            SidecarRoutingDecision::LoomNative { .. } => {
                if !report.is_ok() {
                    ("host-native", "verifier-rejected".to_string(), "error", 0, 0)
                } else {
                    match decode_loom_native_batch(&program, host_slice) {
                        Ok(batch) => {
                            let rows = batch.num_rows() as u64;
                            let cols = batch.num_columns();
                            decoded_batch = Some(batch);
                            ("loom-native", "all-gates-passed".to_string(), "ok", rows, cols)
                        }
                        Err(_) => (
                            "host-native",
                            "decode-unsupported".to_string(),
                            "unsupported",
                            0,
                            0,
                        ),
                    }
                }
            }
            SidecarRoutingDecision::HostNativeReader { reason: r, .. } => {
                ("host-native", r.to_string(), "unsupported", 0, 0)
            }
        };

        // Serialize the decoded batch to Arrow IPC (loom-native only); other
        // routes return an empty buffer and the caller falls back to a host
        // reader.
        let ipc_output: Vec<u8> = match &decoded_batch {
            Some(batch) => record_batch_to_ipc(batch)?,
            None => Vec::new(),
        };

        let json = format!(
            r#"{{"route":"{}","reason":"{}","decode_status":"{}","ir_hash":"{}","row_count":{},"column_count":{},"accepted":{}}}"#,
            route, reason, decode_status, ir_hash, row_count, col_count, report.is_ok()
        );

        let cstr = CString::new(json).map_err(|_| LoomSidecarError::DecodeFailed)?;
        let ptr = cstr.into_raw();
        std::ptr::write(out_json, ptr);

        // Return empty IPC buffer (caller checks route field)
        let ipc_box = ipc_output.into_boxed_slice();
        let ipc_len = ipc_box.len();
        let ipc_ptr = Box::into_raw(ipc_box) as *mut u8;
        std::ptr::write(out_ipc_bytes, ipc_ptr);
        std::ptr::write(out_ipc_len, ipc_len);

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

#[cfg(test)]
mod decode_tests {
    use super::*;
    use arrow_array::{Array, Int32Array};
    use loom_ir_core::l2_core::{
        Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, L2DataType,
        OutputBuilderCapability, ResourceBudget, ScalarExpr, ScalarValue,
    };

    #[test]
    fn extract_external_sidecar_from_assets() {
        // Verify the external sidecar extract path works on the bundled assets.
        use std::ffi::CString;
        let path = CString::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/data.parquet")
                .to_string_lossy()
                .as_ref(),
        )
        .unwrap();
        let mut out_bytes: *mut u8 = std::ptr::null_mut();
        let mut out_len: usize = 0;
        let rc = unsafe { loom_sidecar_extract(path.as_ptr(), &mut out_bytes, &mut out_len) };
        assert_eq!(rc, 0, "extract failed with code {rc}");
        assert!(out_len > 0);
        // Verify the bytes are a valid sidecar overlay
        let bytes = unsafe { std::slice::from_raw_parts(out_bytes, out_len) };
        let overlay = SidecarOverlay::decode(bytes).expect("valid sidecar");
        assert!(!overlay.ir_bytes.is_empty());
        unsafe { loom_sidecar_free_bytes(out_bytes, out_len) };
    }

    fn i32_copy_program(rows: u64) -> L2CoreProgram {
        L2CoreProgram {
            artifact_version: 1,
            required_features: vec!["l2core.copy.v0".to_string()],
            optional_features: vec![],
            capabilities: vec![
                Capability::InputSlice(InputSliceCapability {
                    id: "in".to_string(),
                    offset: 0,
                    length: rows * 4,
                }),
                Capability::OutputBuilder(OutputBuilderCapability {
                    id: "output_col".to_string(),
                    arrow_type: L2DataType::Int32,
                    nullable: false,
                    max_events: rows,
                }),
            ],
            resource_budget: ResourceBudget::bounded_rows(rows),
            body: vec![L2CoreStmt::ForRange {
                index: "i".to_string(),
                start: ScalarExpr::Const(ScalarValue::UInt64(0)),
                end: ScalarExpr::Const(ScalarValue::UInt64(rows)),
                body: vec![
                    L2CoreStmt::ReadInput {
                        capability: "in".to_string(),
                        offset: ScalarExpr::Mul(
                            Box::new(ScalarExpr::Var("i".to_string())),
                            Box::new(ScalarExpr::Const(ScalarValue::UInt64(4))),
                        ),
                        width: ScalarExpr::Const(ScalarValue::UInt64(4)),
                        bind: "v".to_string(),
                    },
                    L2CoreStmt::AppendValue {
                        builder: "output_col".to_string(),
                        value: ScalarExpr::Var("v".to_string()),
                    },
                ],
            }],
        }
    }

    #[test]
    fn decode_loom_native_batch_i32_roundtrip() {
        let vals = [5i32, 6, 7, 8];
        let program = i32_copy_program(vals.len() as u64);
        let host: Vec<u8> = vals.iter().flat_map(|v| v.to_le_bytes()).collect();
        let batch = decode_loom_native_batch(&program, &host).expect("decode ok");
        assert_eq!(batch.num_rows(), 4);
        assert_eq!(batch.num_columns(), 1);
        assert_eq!(batch.schema().field(0).name(), "col");
        let arr = batch
            .column(0)
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        assert_eq!(arr.values(), &vals);
    }

    #[test]
    fn decode_loom_native_batch_short_host_fails_closed() {
        // Program needs 4 rows (16 bytes) but host only provides 8.
        let program = i32_copy_program(4);
        let host = vec![0u8; 8];
        assert!(decode_loom_native_batch(&program, &host).is_err());
    }

    #[test]
    fn record_batch_to_ipc_roundtrips_via_stream_reader() {
        let vals = [101i32, 202, 303];
        let program = i32_copy_program(vals.len() as u64);
        let host: Vec<u8> = vals.iter().flat_map(|v| v.to_le_bytes()).collect();
        let batch = decode_loom_native_batch(&program, &host).expect("decode ok");

        let ipc = record_batch_to_ipc(&batch).expect("serialize ipc");
        assert!(!ipc.is_empty(), "IPC buffer must be non-empty for loom-native");

        // Read the bare IPC stream back and confirm values survive the boundary.
        let reader = arrow_ipc::reader::StreamReader::try_new(std::io::Cursor::new(ipc), None)
            .expect("stream reader");
        let batches: Vec<_> = reader.map(|b| b.expect("batch")).collect();
        assert_eq!(batches.len(), 1);
        let out = &batches[0];
        assert_eq!(out.num_rows(), 3);
        assert_eq!(out.schema().field(0).name(), "col");
        let arr = out
            .column(0)
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        assert_eq!(arr.values(), &vals);
    }
}
