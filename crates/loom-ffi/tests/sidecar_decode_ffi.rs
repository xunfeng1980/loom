//! End-to-end FFI test for `loom_sidecar_decode` (Plan 2).
//!
//! Builds a real sidecar overlay that passes all four routing gates, calls the
//! C ABI entry point, and asserts the Loom-native path now returns a non-empty
//! **bare Arrow IPC stream** that reads back correctly — then frees both
//! outputs through the C ABI free functions (no-leak / double-free contract).

use std::ffi::{c_char, CStr};

use arrow_array::{Array, Int32Array};

use loom_ffi::ffi::{
    loom_sidecar_decode, loom_sidecar_decode_carray, loom_sidecar_free_bytes,
    loom_sidecar_free_cstr,
};
use loom_ir_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, L2DataType,
    OutputBuilderCapability, ResourceBudget, ScalarExpr, ScalarValue,
};
use loom_ir_core::l2core_codec::{encode_l2core_program, l2core_program_hash};
use loom_ir_core::sidecar::{compute_chunk_hash, ChunkBinding, SidecarOverlay};

/// A verifier-accepted program that appends a constant i32 ten times. It needs
/// no input reads, so it isolates the decode→IPC path from input windowing.
fn const_append_program(rows: u64, host_len: u64) -> L2CoreProgram {
    L2CoreProgram {
        artifact_version: 1,
        required_features: vec!["l2core.copy.v0".to_string()],
        optional_features: vec![],
        capabilities: vec![
            Capability::InputSlice(InputSliceCapability {
                id: "input".to_string(),
                offset: 0,
                length: host_len,
            }),
            Capability::OutputBuilder(OutputBuilderCapability {
                id: "output".to_string(),
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
            body: vec![L2CoreStmt::AppendValue {
                builder: "output".to_string(),
                value: ScalarExpr::Const(ScalarValue::Int32(42)),
            }],
        }],
    }
}

#[test]
fn loom_native_decode_returns_readable_ipc() {
    let host = vec![0u8; 100];
    let program = const_append_program(10, host.len() as u64);
    let ir_bytes = encode_l2core_program(&program);

    // Binding whose content hash matches the host window → Gate 3 passes.
    let binding = ChunkBinding {
        granule_id: "output".to_string(),
        host_data_range: (0, host.len() as u64),
        content_hash: compute_chunk_hash(&host),
        ir_identity: l2core_program_hash(&program),
    };
    let overlay = SidecarOverlay {
        ir_bytes,
        bindings: vec![binding],
    };
    let overlay_bytes = overlay.encode();

    let mut out_json: *const c_char = std::ptr::null();
    let mut out_ipc: *mut u8 = std::ptr::null_mut();
    let mut out_ipc_len: usize = 0;

    let code = unsafe {
        loom_sidecar_decode(
            overlay_bytes.as_ptr(),
            overlay_bytes.len(),
            host.as_ptr(),
            host.len(),
            &mut out_json,
            &mut out_ipc,
            &mut out_ipc_len,
        )
    };
    assert_eq!(code, 0, "decode should return Success");
    assert!(!out_json.is_null(), "json must be set");

    let json = unsafe { CStr::from_ptr(out_json) }
        .to_str()
        .expect("utf8 json")
        .to_string();
    assert!(json.contains("\"route\":\"loom-native\""), "json: {json}");
    assert!(json.contains("\"row_count\":10"), "json: {json}");
    assert!(json.contains("\"column_count\":1"), "json: {json}");

    // Non-empty bare IPC stream that reads back to the decoded column.
    assert!(out_ipc_len > 0, "loom-native must return non-empty IPC");
    let ipc = unsafe { std::slice::from_raw_parts(out_ipc, out_ipc_len) }.to_vec();
    let reader =
        arrow_ipc::reader::StreamReader::try_new(std::io::Cursor::new(ipc), None).expect("reader");
    let batches: Vec<_> = reader.map(|b| b.expect("batch")).collect();
    assert_eq!(batches.len(), 1);
    let col = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<Int32Array>()
        .expect("i32 column");
    assert_eq!(col.values(), &[42i32; 10]);

    // Free both outputs through the C ABI (no-leak / safe contract).
    let free_bytes = unsafe { loom_sidecar_free_bytes(out_ipc, out_ipc_len) };
    assert_eq!(free_bytes, 0, "free_bytes should succeed");
    let free_json = unsafe { loom_sidecar_free_cstr(out_json as *mut c_char) };
    assert_eq!(free_json, 0, "free_cstr should succeed");
}

#[test]
fn loom_native_decode_carray_roundtrips_via_c_data_interface() {
    use arrow::ffi::{from_ffi, FFI_ArrowArray, FFI_ArrowSchema};
    use arrow_array::StructArray;

    let host = vec![0u8; 100];
    // const-append program: appends Int32(7) five times (no input reads).
    let program = L2CoreProgram {
        artifact_version: 1,
        required_features: vec!["l2core.copy.v0".to_string()],
        optional_features: vec![],
        capabilities: vec![
            Capability::InputSlice(InputSliceCapability {
                id: "input".to_string(),
                offset: 0,
                length: host.len() as u64,
            }),
            Capability::OutputBuilder(OutputBuilderCapability {
                id: "output".to_string(),
                arrow_type: L2DataType::Int32,
                nullable: false,
                max_events: 5,
            }),
        ],
        resource_budget: ResourceBudget::bounded_rows(5),
        body: vec![L2CoreStmt::ForRange {
            index: "i".to_string(),
            start: ScalarExpr::Const(ScalarValue::UInt64(0)),
            end: ScalarExpr::Const(ScalarValue::UInt64(5)),
            body: vec![L2CoreStmt::AppendValue {
                builder: "output".to_string(),
                value: ScalarExpr::Const(ScalarValue::Int32(7)),
            }],
        }],
    };
    let ir_bytes = encode_l2core_program(&program);
    let binding = ChunkBinding {
        granule_id: "output".to_string(),
        host_data_range: (0, host.len() as u64),
        content_hash: compute_chunk_hash(&host),
        ir_identity: l2core_program_hash(&program),
    };
    let overlay = SidecarOverlay {
        ir_bytes,
        bindings: vec![binding],
    };
    let overlay_bytes = overlay.encode();

    // Caller-allocated empty C structs.
    let mut ffi_schema = FFI_ArrowSchema::empty();
    let mut ffi_array = FFI_ArrowArray::empty();

    let code = unsafe {
        loom_sidecar_decode_carray(
            overlay_bytes.as_ptr(),
            overlay_bytes.len(),
            host.as_ptr(),
            host.len(),
            &mut ffi_schema as *mut FFI_ArrowSchema as *mut std::ffi::c_void,
            &mut ffi_array as *mut FFI_ArrowArray as *mut std::ffi::c_void,
        )
    };
    assert_eq!(code, 0, "decode_carray should succeed");

    // Reconstruct the Arrow struct array from the C Data Interface and verify.
    let data = unsafe { from_ffi(ffi_array, &ffi_schema) }.expect("from_ffi");
    let struct_arr = StructArray::from(data);
    assert_eq!(struct_arr.num_columns(), 1);
    assert_eq!(struct_arr.len(), 5);
    let col = struct_arr
        .column(0)
        .as_any()
        .downcast_ref::<Int32Array>()
        .expect("i32 column");
    assert_eq!(col.values(), &[7i32; 5]);
}
