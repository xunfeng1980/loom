use std::ffi::CStr;
use std::ptr;
use std::sync::Arc;

use arrow::array::{Array, BooleanArray, Int32Array, RecordBatch};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ffi::{from_ffi, FFI_ArrowArray, FFI_ArrowSchema};
use loom_core::arrow_semantic::ArrowSemanticPayload;
use loom_core::arrow_semantic_codec::{
    encode_arrow_semantic_container_payload, encode_arrow_semantic_payload,
};
use loom_core::container_codec::wrap_layout_payload;
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::layout_codec::encode_layout_payload;
use loom_ffi::duckdb_runtime::{
    loom_duckdb_arrow_semantic_column_count, loom_duckdb_arrow_semantic_column_format,
    loom_duckdb_arrow_semantic_column_name, loom_duckdb_arrow_semantic_create,
    loom_duckdb_arrow_semantic_destroy, loom_duckdb_arrow_semantic_export_column,
    loom_duckdb_arrow_semantic_row_count, loom_duckdb_plan_cache_input, loom_duckdb_plan_cache_key,
    loom_duckdb_plan_create, loom_duckdb_plan_create_projected, loom_duckdb_plan_decision,
    loom_duckdb_plan_destroy, loom_duckdb_plan_diagnostic, loom_duckdb_plan_diagnostic_count,
    loom_duckdb_prepare_create, loom_duckdb_prepare_destroy, loom_duckdb_prepare_native_buffer,
    loom_duckdb_prepare_native_buffer_count, loom_duckdb_prepare_route, LoomDuckDbArrowSemantic,
    LoomDuckDbDiagnostic, LoomDuckDbNativeBuffer, LoomDuckDbPlan, LoomDuckDbPrepared,
};

fn raw_i32_lmc1(row_count: usize) -> Vec<u8> {
    let desc = LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::Raw {
            data: (0..row_count as i32).flat_map(i32::to_le_bytes).collect(),
            elem_size: 4,
            count: row_count,
        },
        row_count,
    };
    wrap_layout_payload(&encode_layout_payload(&desc)).expect("valid LMC1")
}

fn arrow_semantic_record_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int32, false),
        Field::new("active", DataType::Boolean, true),
    ]));
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(Int32Array::from(vec![10, 20, 30])),
            Arc::new(BooleanArray::from(vec![Some(true), None, Some(false)])),
        ],
    )
    .expect("record batch")
}

fn arrow_semantic_lma1() -> Vec<u8> {
    let payload =
        ArrowSemanticPayload::from_record_batches(&[arrow_semantic_record_batch()]).expect("LMA1");
    encode_arrow_semantic_payload(&payload).expect("encode LMA1")
}

fn arrow_semantic_lmc2() -> Vec<u8> {
    let payload =
        ArrowSemanticPayload::from_record_batches(&[arrow_semantic_record_batch()]).expect("LMC2");
    encode_arrow_semantic_container_payload(&payload).expect("encode LMC2")
}

#[test]
fn plan_create_defaults_arrow_semantic_to_native_candidate_without_test_facts() {
    let artifact = arrow_semantic_lma1();
    let mut plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe { loom_duckdb_plan_create(artifact.as_ptr(), artifact.len(), true, &mut plan) },
        0
    );
    assert!(!plan.is_null());
    assert_eq!(unsafe { plan_decision_string(plan) }, "native-candidate");
    assert!(unsafe { plan_cache_input(plan) }.contains("duckdb-arrow-semantic-codegen"));

    let mut diagnostic_count = 0usize;
    assert_eq!(
        unsafe { loom_duckdb_plan_diagnostic_count(plan, &mut diagnostic_count) },
        0
    );
    assert!(diagnostic_count > 0);
    assert_eq!(unsafe { loom_duckdb_plan_destroy(plan) }, 0);
}

#[test]
fn raw_lmc1_plan_falls_back_or_fails_closed_without_native_buffers() {
    let artifact = raw_i32_lmc1(4);
    let mut fallback_plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_plan_create(artifact.as_ptr(), artifact.len(), true, &mut fallback_plan)
        },
        0
    );
    assert_eq!(
        unsafe { plan_decision_string(fallback_plan) },
        "interpreter-fallback"
    );

    let mut prepared: *mut LoomDuckDbPrepared = ptr::null_mut();
    assert_eq!(
        unsafe { loom_duckdb_prepare_create(fallback_plan, false, &mut prepared) },
        0
    );
    assert_eq!(
        unsafe { prepare_route_string(prepared) },
        "interpreter-fallback"
    );
    let mut buffer_count = usize::MAX;
    assert_eq!(
        unsafe { loom_duckdb_prepare_native_buffer_count(prepared, &mut buffer_count) },
        0
    );
    assert_eq!(buffer_count, 0);
    assert_eq!(unsafe { loom_duckdb_prepare_destroy(prepared) }, 0);
    assert_eq!(unsafe { loom_duckdb_plan_destroy(fallback_plan) }, 0);

    let mut strict_plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_plan_create(artifact.as_ptr(), artifact.len(), false, &mut strict_plan)
        },
        0
    );
    assert_eq!(unsafe { plan_decision_string(strict_plan) }, "fail-closed");
    assert_eq!(unsafe { loom_duckdb_plan_destroy(strict_plan) }, 0);
}

#[test]
fn projected_arrow_semantic_plan_records_projection_but_prepare_falls_back() {
    let artifact = arrow_semantic_lma1();
    let projection = [0u32];
    let mut plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_plan_create_projected(
                artifact.as_ptr(),
                artifact.len(),
                projection.as_ptr(),
                projection.len(),
                true,
                &mut plan,
            )
        },
        0
    );
    assert!(unsafe { plan_cache_input(plan) }.contains("projection=columns:0>0"));

    let mut prepared: *mut LoomDuckDbPrepared = ptr::null_mut();
    assert_eq!(
        unsafe { loom_duckdb_prepare_create(plan, false, &mut prepared) },
        0
    );
    assert_eq!(
        unsafe { prepare_route_string(prepared) },
        "interpreter-fallback"
    );
    assert_eq!(unsafe { loom_duckdb_prepare_destroy(prepared) }, 0);
    assert_eq!(unsafe { loom_duckdb_plan_destroy(plan) }, 0);
}

#[test]
fn cancelled_arrow_semantic_prepare_exposes_cancelled_route() {
    let artifact = arrow_semantic_lma1();
    let mut plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe { loom_duckdb_plan_create(artifact.as_ptr(), artifact.len(), false, &mut plan) },
        0
    );

    let mut prepared: *mut LoomDuckDbPrepared = ptr::null_mut();
    assert_eq!(
        unsafe { loom_duckdb_prepare_create(plan, true, &mut prepared) },
        0
    );
    assert_eq!(unsafe { prepare_route_string(prepared) }, "cancelled");
    assert_eq!(unsafe { loom_duckdb_prepare_destroy(prepared) }, 0);
    assert_eq!(unsafe { loom_duckdb_plan_destroy(plan) }, 0);
}

#[test]
fn native_buffer_ffi_exposes_row_count_and_optional_validity() {
    let artifact = arrow_semantic_lmc2();
    let mut plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe { loom_duckdb_plan_create(artifact.as_ptr(), artifact.len(), false, &mut plan) },
        0
    );
    let mut prepared: *mut LoomDuckDbPrepared = ptr::null_mut();
    assert_eq!(
        unsafe { loom_duckdb_prepare_create(plan, false, &mut prepared) },
        0
    );
    assert_eq!(
        unsafe { prepare_route_string(prepared) },
        "native-candidate"
    );

    let mut buffer_count = 0usize;
    assert_eq!(
        unsafe { loom_duckdb_prepare_native_buffer_count(prepared, &mut buffer_count) },
        0
    );
    assert_eq!(buffer_count, 2);

    let mut bool_buffer = LoomDuckDbNativeBuffer::default();
    assert_eq!(
        unsafe { loom_duckdb_prepare_native_buffer(prepared, 1, &mut bool_buffer) },
        0
    );
    assert_eq!(unsafe { cstr(bool_buffer.arrow_type) }, "Boolean");
    assert_eq!(bool_buffer.row_count, 3);
    assert!(!bool_buffer.value_ptr.is_null());
    assert_eq!(bool_buffer.value_len, 1);
    assert!(!bool_buffer.validity_ptr.is_null());
    assert_eq!(bool_buffer.validity_len, 1);

    assert_eq!(unsafe { loom_duckdb_prepare_destroy(prepared) }, 0);
    assert_eq!(unsafe { loom_duckdb_plan_destroy(plan) }, 0);
}

#[test]
fn arrow_semantic_handle_accepts_lmc2_and_exports_nullable_columns() {
    let artifact = arrow_semantic_lmc2();
    let mut handle: *mut LoomDuckDbArrowSemantic = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_arrow_semantic_create(artifact.as_ptr(), artifact.len(), &mut handle)
        },
        0
    );

    let mut column_count = 0usize;
    assert_eq!(
        unsafe { loom_duckdb_arrow_semantic_column_count(handle, &mut column_count) },
        0
    );
    assert_eq!(column_count, 2);
    let mut row_count = 0usize;
    assert_eq!(
        unsafe { loom_duckdb_arrow_semantic_row_count(handle, &mut row_count) },
        0
    );
    assert_eq!(row_count, 3);
    assert_eq!(unsafe { arrow_semantic_column_name(handle, 0) }, "id");
    assert_eq!(unsafe { arrow_semantic_column_format(handle, 0) }, "i");
    assert_eq!(unsafe { arrow_semantic_column_format(handle, 1) }, "b");

    let active = unsafe { export_arrow_semantic_bool_column(handle, 1) };
    assert_eq!(active.len(), 3);
    assert_eq!(active.null_count(), 1);
    assert_eq!(unsafe { loom_duckdb_arrow_semantic_destroy(handle) }, 0);
}

#[test]
fn arrow_semantic_handle_rejects_invalid_inputs_and_multibatch() {
    let mut handle: *mut LoomDuckDbArrowSemantic = ptr::null_mut();
    let rc = unsafe { loom_duckdb_arrow_semantic_create(ptr::null(), 3, &mut handle) };
    assert_ne!(rc, 0);
    assert!(handle.is_null());

    let batch = arrow_semantic_record_batch();
    let payload =
        ArrowSemanticPayload::from_record_batches(&[batch.clone(), batch]).expect("multi batch");
    let multibatch = encode_arrow_semantic_container_payload(&payload).expect("encode");
    let rc = unsafe {
        loom_duckdb_arrow_semantic_create(multibatch.as_ptr(), multibatch.len(), &mut handle)
    };
    assert_ne!(rc, 0);
    assert!(handle.is_null());
}

#[test]
fn diagnostic_out_of_range_returns_error_without_mutating_output() {
    let artifact = raw_i32_lmc1(4);
    let mut plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe { loom_duckdb_plan_create(artifact.as_ptr(), artifact.len(), true, &mut plan) },
        0
    );

    let sentinel = LoomDuckDbDiagnostic {
        code: ptr::dangling(),
        path: ptr::dangling(),
        message: ptr::dangling(),
    };
    let mut diagnostic = sentinel;
    let rc = unsafe { loom_duckdb_plan_diagnostic(plan, usize::MAX, &mut diagnostic) };
    assert_ne!(rc, 0);
    assert_eq!(diagnostic.code, sentinel.code);
    assert_eq!(diagnostic.path, sentinel.path);
    assert_eq!(diagnostic.message, sentinel.message);
    assert_eq!(unsafe { loom_duckdb_plan_destroy(plan) }, 0);
}

#[test]
fn internal_and_public_headers_keep_expected_boundaries() {
    let internal_header = std::fs::read_to_string(format!(
        "{}/include/loom_duckdb_internal.h",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("read internal header");
    for required in [
        "LoomDuckDbNativeBuffer",
        "validity_ptr",
        "row_count",
        "loom_duckdb_prepare_native_buffer",
        "loom_duckdb_arrow_semantic_create",
    ] {
        assert!(internal_header.contains(required));
    }
    assert!(!internal_header.contains("use_test_native_facts"));

    let public_header =
        std::fs::read_to_string(format!("{}/include/loom.h", env!("CARGO_MANIFEST_DIR")))
            .expect("read public header");
    for forbidden in [
        "loom_duckdb_",
        "LoomDuckDb",
        "duckdb_runtime",
        "native_preparation",
        "loom_scan_native",
        "loom_scan_interpreter",
    ] {
        assert!(!public_header.contains(forbidden));
    }
}

unsafe fn plan_decision_string(plan: *mut LoomDuckDbPlan) -> String {
    let mut decision = ptr::null();
    assert_eq!(loom_duckdb_plan_decision(plan, &mut decision), 0);
    cstr(decision)
}

unsafe fn plan_cache_input(plan: *mut LoomDuckDbPlan) -> String {
    let mut cache_input = ptr::null();
    assert_eq!(loom_duckdb_plan_cache_input(plan, &mut cache_input), 0);
    cstr(cache_input)
}

#[allow(dead_code)]
unsafe fn plan_cache_key(plan: *mut LoomDuckDbPlan) -> String {
    let mut cache_key = ptr::null();
    assert_eq!(loom_duckdb_plan_cache_key(plan, &mut cache_key), 0);
    cstr(cache_key)
}

unsafe fn prepare_route_string(prepared: *mut LoomDuckDbPrepared) -> String {
    let mut route = ptr::null();
    assert_eq!(loom_duckdb_prepare_route(prepared, &mut route), 0);
    cstr(route)
}

unsafe fn arrow_semantic_column_name(handle: *mut LoomDuckDbArrowSemantic, index: usize) -> String {
    let mut name = ptr::null();
    assert_eq!(
        loom_duckdb_arrow_semantic_column_name(handle, index, &mut name),
        0
    );
    cstr(name)
}

unsafe fn arrow_semantic_column_format(
    handle: *mut LoomDuckDbArrowSemantic,
    index: usize,
) -> String {
    let mut format = ptr::null();
    assert_eq!(
        loom_duckdb_arrow_semantic_column_format(handle, index, &mut format),
        0
    );
    cstr(format)
}

unsafe fn export_arrow_semantic_bool_column(
    handle: *mut LoomDuckDbArrowSemantic,
    index: usize,
) -> BooleanArray {
    let (array, schema) = export_arrow_semantic_column(handle, index);
    let data = from_ffi(array, &schema).expect("from ffi");
    BooleanArray::from(data)
}

unsafe fn export_arrow_semantic_column(
    handle: *mut LoomDuckDbArrowSemantic,
    index: usize,
) -> (FFI_ArrowArray, FFI_ArrowSchema) {
    let mut array = FFI_ArrowArray::empty();
    let mut schema = FFI_ArrowSchema::empty();
    assert_eq!(
        loom_duckdb_arrow_semantic_export_column(handle, index, &mut array, &mut schema),
        0
    );
    (array, schema)
}

unsafe fn cstr(ptr: *const std::ffi::c_char) -> String {
    assert!(!ptr.is_null());
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
}
