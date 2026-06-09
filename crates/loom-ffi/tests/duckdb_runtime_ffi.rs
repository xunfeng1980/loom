use std::ffi::CStr;
use std::ptr;
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard};

use arrow::array::{Array, BooleanArray, Int32Array, RecordBatch, StringArray};
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
    duckdb_runtime_clear_native_preparation_cache_for_test,
    loom_duckdb_arrow_semantic_column_count, loom_duckdb_arrow_semantic_column_format,
    loom_duckdb_arrow_semantic_column_name, loom_duckdb_arrow_semantic_create,
    loom_duckdb_arrow_semantic_destroy, loom_duckdb_arrow_semantic_export_column,
    loom_duckdb_arrow_semantic_row_count, loom_duckdb_plan_cache_input, loom_duckdb_plan_cache_key,
    loom_duckdb_plan_create, loom_duckdb_plan_create_projected, loom_duckdb_plan_decision,
    loom_duckdb_plan_destroy, loom_duckdb_plan_diagnostic, loom_duckdb_plan_diagnostic_count,
    loom_duckdb_prepare_create, loom_duckdb_prepare_destroy, loom_duckdb_prepare_diagnostic,
    loom_duckdb_prepare_diagnostic_count, loom_duckdb_prepare_native_buffer,
    loom_duckdb_prepare_native_buffer_count, loom_duckdb_prepare_route, plan_duckdb_runtime,
    prepare_duckdb_runtime, DuckDbProjection, DuckDbRuntimePlanInput, DuckDbRuntimePolicy,
    DuckDbTestNativeFacts, LoomDuckDbArrowSemantic, LoomDuckDbDiagnostic, LoomDuckDbNativeBuffer,
    LoomDuckDbPlan, LoomDuckDbPrepared,
};
use loom_native_melior::backend::NativeBackendCancellation;

static PREPARE_CACHE_TEST_LOCK: Mutex<()> = Mutex::new(());

fn isolated_prepare_cache() -> MutexGuard<'static, ()> {
    let guard = PREPARE_CACHE_TEST_LOCK
        .lock()
        .expect("prepare cache test mutex poisoned");
    duckdb_runtime_clear_native_preparation_cache_for_test();
    guard
}

fn raw_i32_lmc1(row_count: u64) -> Vec<u8> {
    let values = (0..row_count as i32)
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    let desc = LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::Raw {
            data: values,
            elem_size: 4,
            count: row_count as usize,
        },
        row_count: row_count as usize,
    };
    let payload = encode_layout_payload(&desc);
    wrap_layout_payload(&payload).expect("valid LMC1 layout")
}

fn arrow_semantic_record_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int32, false),
        Field::new("label", DataType::Utf8, true),
        Field::new("active", DataType::Boolean, true),
    ]));
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(Int32Array::from(vec![10, 20, 30])),
            Arc::new(StringArray::from(vec![Some("alpha"), None, Some("gamma")])),
            Arc::new(BooleanArray::from(vec![Some(true), None, Some(false)])),
        ],
    )
    .expect("record batch")
}

fn arrow_semantic_lma1() -> Vec<u8> {
    let payload = ArrowSemanticPayload::from_record_batches(&[arrow_semantic_record_batch()])
        .expect("semantic payload");
    encode_arrow_semantic_payload(&payload).expect("encode direct LMA1")
}

fn arrow_semantic_lmc2() -> Vec<u8> {
    let payload = ArrowSemanticPayload::from_record_batches(&[arrow_semantic_record_batch()])
        .expect("semantic payload");
    encode_arrow_semantic_container_payload(&payload).expect("encode LMC2")
}

unsafe fn cstr(ptr: *const std::ffi::c_char) -> String {
    assert!(!ptr.is_null(), "expected non-null C string");
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
}

#[test]
fn plan_create_null_outputs_return_error() {
    let artifact = raw_i32_lmc1(4);
    let rc = unsafe {
        loom_duckdb_plan_create(
            artifact.as_ptr(),
            artifact.len(),
            true,
            false,
            ptr::null_mut(),
        )
    };

    assert_ne!(rc, 0, "null plan output pointer must fail closed");
}

#[test]
fn created_plan_exposes_decision_cache_key_and_diagnostics() {
    let artifact = raw_i32_lmc1(4);
    let mut plan: *mut LoomDuckDbPlan = ptr::null_mut();
    let rc = unsafe {
        loom_duckdb_plan_create(
            artifact.as_ptr(),
            artifact.len(),
            false,
            true,
            &mut plan as *mut _,
        )
    };
    assert_eq!(rc, 0, "plan creation should succeed");
    assert!(!plan.is_null(), "plan handle must be populated");

    let mut decision = ptr::null();
    assert_eq!(
        unsafe { loom_duckdb_plan_decision(plan, &mut decision as *mut _) },
        0
    );
    assert_eq!(unsafe { cstr(decision) }, "native-candidate");

    let mut cache_key = ptr::null();
    assert_eq!(
        unsafe { loom_duckdb_plan_cache_key(plan, &mut cache_key as *mut _) },
        0
    );
    assert!(
        unsafe { cstr(cache_key) }.starts_with("loom-runtime-v"),
        "cache key should expose the runtime cache id"
    );

    let mut diagnostic_count = 0usize;
    assert_eq!(
        unsafe { loom_duckdb_plan_diagnostic_count(plan, &mut diagnostic_count as *mut _) },
        0
    );
    assert!(
        diagnostic_count > 0,
        "test-native-facts diagnostic should be visible"
    );

    let mut diagnostic = LoomDuckDbDiagnostic::default();
    assert_eq!(
        unsafe { loom_duckdb_plan_diagnostic(plan, 0, &mut diagnostic as *mut _) },
        0
    );
    assert!(
        unsafe { cstr(diagnostic.code) } == "test-native-facts"
            || unsafe { cstr(diagnostic.code) }.contains("lowering")
    );

    assert_eq!(unsafe { loom_duckdb_plan_destroy(plan) }, 0);
}

#[test]
fn projected_plan_create_wires_projection_into_runtime_cache_input() {
    let artifact = raw_i32_lmc1(4);
    let projection = [0u32];
    let mut plan: *mut LoomDuckDbPlan = ptr::null_mut();
    let rc = unsafe {
        loom_duckdb_plan_create_projected(
            artifact.as_ptr(),
            artifact.len(),
            projection.as_ptr(),
            projection.len(),
            false,
            true,
            &mut plan as *mut _,
        )
    };
    assert_eq!(rc, 0, "projected plan creation should succeed");
    assert!(!plan.is_null(), "projected plan handle must be populated");

    let mut cache_input = ptr::null();
    assert_eq!(
        unsafe { loom_duckdb_plan_cache_input(plan, &mut cache_input as *mut _) },
        0
    );
    let cache_input = unsafe { cstr(cache_input) };
    assert!(
        cache_input.contains("projection=columns:0>0"),
        "projected plan should enter the runtime cache input, got {cache_input}"
    );

    assert_eq!(unsafe { loom_duckdb_plan_destroy(plan) }, 0);
}

#[test]
fn projected_plan_create_validates_projection_pointer_and_empty_projection() {
    let artifact = raw_i32_lmc1(4);
    let mut plan: *mut LoomDuckDbPlan = ptr::null_mut();
    let null_projection_rc = unsafe {
        loom_duckdb_plan_create_projected(
            artifact.as_ptr(),
            artifact.len(),
            ptr::null(),
            1,
            true,
            false,
            &mut plan as *mut _,
        )
    };
    assert_ne!(
        null_projection_rc, 0,
        "non-empty projection with a null pointer must fail closed"
    );
    assert!(plan.is_null());

    let rc = unsafe {
        loom_duckdb_plan_create_projected(
            artifact.as_ptr(),
            artifact.len(),
            ptr::null(),
            0,
            true,
            false,
            &mut plan as *mut _,
        )
    };
    assert_eq!(rc, 0, "empty projection should return a diagnostic plan");
    assert_eq!(unsafe { plan_decision_string(plan) }, "fail-closed");
    assert!(
        unsafe { plan_diagnostic_codes(plan) }
            .iter()
            .any(|code| code == "unsupported-projection"),
        "empty projection must be diagnosed by runtime projection planning"
    );
    assert_eq!(unsafe { loom_duckdb_plan_destroy(plan) }, 0);
}

#[test]
fn prepare_create_returns_handle_and_route_without_unwinding() {
    let _guard = isolated_prepare_cache();
    let artifact = raw_i32_lmc1(4);
    let mut plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_plan_create(
                artifact.as_ptr(),
                artifact.len(),
                false,
                true,
                &mut plan as *mut _,
            )
        },
        0
    );

    let mut prepared: *mut LoomDuckDbPrepared = ptr::null_mut();
    assert_eq!(
        unsafe { loom_duckdb_prepare_create(plan, false, &mut prepared as *mut _) },
        0
    );
    assert!(!prepared.is_null(), "prepared handle must be populated");

    let mut route = ptr::null();
    assert_eq!(
        unsafe { loom_duckdb_prepare_route(prepared, &mut route as *mut _) },
        0
    );
    let route = unsafe { cstr(route) };
    assert!(
        route == "native-candidate" || route == "fail-closed",
        "prepare route should be native-ready or diagnostic-bearing failure, got {route}"
    );

    let mut native_buffer_count = usize::MAX;
    assert_eq!(
        unsafe {
            loom_duckdb_prepare_native_buffer_count(prepared, &mut native_buffer_count as *mut _)
        },
        0
    );
    assert_ne!(
        native_buffer_count,
        usize::MAX,
        "buffer count output should be written"
    );

    assert_eq!(unsafe { loom_duckdb_prepare_destroy(prepared) }, 0);
    assert_eq!(unsafe { loom_duckdb_plan_destroy(plan) }, 0);
}

#[test]
fn prepare_diagnostic_accessors_expose_cache_evidence() {
    let _guard = isolated_prepare_cache();
    let artifact = raw_i32_lmc1(4);

    let mut miss_plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_plan_create(
                artifact.as_ptr(),
                artifact.len(),
                false,
                true,
                &mut miss_plan as *mut _,
            )
        },
        0
    );
    let mut miss_prepared: *mut LoomDuckDbPrepared = ptr::null_mut();
    assert_eq!(
        unsafe { loom_duckdb_prepare_create(miss_plan, false, &mut miss_prepared as *mut _) },
        0
    );
    let miss_codes = unsafe { prepare_diagnostic_codes(miss_prepared) };
    assert!(
        miss_codes.iter().any(|code| code == "cache-miss"),
        "prepare diagnostics should expose cache miss evidence: {miss_codes:?}"
    );
    assert_eq!(unsafe { loom_duckdb_prepare_destroy(miss_prepared) }, 0);
    assert_eq!(unsafe { loom_duckdb_plan_destroy(miss_plan) }, 0);

    let mut fallback_plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_plan_create(
                artifact.as_ptr(),
                artifact.len(),
                true,
                false,
                &mut fallback_plan as *mut _,
            )
        },
        0
    );
    let mut fallback_prepared: *mut LoomDuckDbPrepared = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_prepare_create(fallback_plan, false, &mut fallback_prepared as *mut _)
        },
        0
    );
    let fallback_codes = unsafe { prepare_diagnostic_codes(fallback_prepared) };
    assert!(
        fallback_codes
            .iter()
            .any(|code| code == "cache-non-cacheable"),
        "fallback prepare should expose non-cacheable cache evidence: {fallback_codes:?}"
    );
    assert_eq!(unsafe { loom_duckdb_prepare_destroy(fallback_prepared) }, 0);
    assert_eq!(unsafe { loom_duckdb_plan_destroy(fallback_plan) }, 0);

    duckdb_runtime_clear_native_preparation_cache_for_test();
    let seeded = plan_duckdb_runtime(DuckDbRuntimePlanInput {
        artifact_bytes: artifact.clone(),
        projection: DuckDbProjection::All,
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback: false,
            test_native_facts: Some(DuckDbTestNativeFacts {
                row_count: 4,
                columns: vec![DataType::Int32],
                test_jit_value_buffers: None,
            }),
        },
    })
    .expect("seed native plan");
    let seeded_route = prepare_duckdb_runtime(&seeded, NativeBackendCancellation::default());
    assert!(
        seeded_route
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "cache-inserted"),
        "Rust seed route should insert cache evidence"
    );

    let mut hit_plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_plan_create(
                artifact.as_ptr(),
                artifact.len(),
                false,
                true,
                &mut hit_plan as *mut _,
            )
        },
        0
    );
    let mut hit_prepared: *mut LoomDuckDbPrepared = ptr::null_mut();
    assert_eq!(
        unsafe { loom_duckdb_prepare_create(hit_plan, false, &mut hit_prepared as *mut _) },
        0
    );
    let hit_codes = unsafe { prepare_diagnostic_codes(hit_prepared) };
    assert!(
        hit_codes.iter().any(|code| code == "cache-hit"),
        "prepare diagnostics should expose cache hit evidence: {hit_codes:?}"
    );
    assert_eq!(unsafe { loom_duckdb_prepare_destroy(hit_prepared) }, 0);
    assert_eq!(unsafe { loom_duckdb_plan_destroy(hit_plan) }, 0);
}

#[test]
fn public_header_excludes_internal_duckdb_symbols() {
    let public_header =
        std::fs::read_to_string(format!("{}/include/loom.h", env!("CARGO_MANIFEST_DIR")))
            .expect("read generated public header");
    assert!(public_header.contains("loom_decode"));
    assert!(!public_header.contains("loom_duckdb_"));
    assert!(!public_header.contains("LoomDuckDb"));
    assert!(!public_header.contains("cache"));
}

#[test]
fn internal_header_exposes_arrow_semantic_duckdb_symbols() {
    let internal_header = std::fs::read_to_string(format!(
        "{}/include/loom_duckdb_internal.h",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("read DuckDB internal header");
    for required in [
        "LoomDuckDbArrowSemantic",
        "loom_duckdb_arrow_semantic_create",
        "loom_duckdb_arrow_semantic_destroy",
        "loom_duckdb_arrow_semantic_column_count",
        "loom_duckdb_arrow_semantic_row_count",
        "loom_duckdb_arrow_semantic_column_name",
        "loom_duckdb_arrow_semantic_column_format",
        "loom_duckdb_arrow_semantic_export_column",
    ] {
        assert!(
            internal_header.contains(required),
            "internal header must expose {required}"
        );
    }
}

#[test]
fn arrow_semantic_handle_accepts_lmc2_and_exports_nullable_columns() {
    let artifact = arrow_semantic_lmc2();
    let mut handle: *mut LoomDuckDbArrowSemantic = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_arrow_semantic_create(
                artifact.as_ptr(),
                artifact.len(),
                &mut handle as *mut _,
            )
        },
        0
    );
    assert!(!handle.is_null(), "handle must be populated");

    let mut column_count = usize::MAX;
    assert_eq!(
        unsafe { loom_duckdb_arrow_semantic_column_count(handle, &mut column_count as *mut _) },
        0
    );
    assert_eq!(column_count, 3);

    let mut row_count = usize::MAX;
    assert_eq!(
        unsafe { loom_duckdb_arrow_semantic_row_count(handle, &mut row_count as *mut _) },
        0
    );
    assert_eq!(row_count, 3);

    assert_eq!(unsafe { arrow_semantic_column_name(handle, 0) }, "id");
    assert_eq!(unsafe { arrow_semantic_column_name(handle, 1) }, "label");
    assert_eq!(unsafe { arrow_semantic_column_name(handle, 2) }, "active");
    assert_eq!(unsafe { arrow_semantic_column_format(handle, 0) }, "i");
    assert_eq!(unsafe { arrow_semantic_column_format(handle, 1) }, "u");
    assert_eq!(unsafe { arrow_semantic_column_format(handle, 2) }, "b");

    let label_array = unsafe { export_arrow_semantic_string_column(handle, 1) };
    assert_eq!(label_array.len(), 3);
    assert_eq!(label_array.value(0), "alpha");
    assert!(
        label_array.is_null(1),
        "nullable Utf8 null must survive export"
    );
    assert_eq!(label_array.value(2), "gamma");

    let active_array = unsafe { export_arrow_semantic_bool_column(handle, 2) };
    assert_eq!(active_array.len(), 3);
    assert!(active_array.value(0));
    assert!(
        active_array.is_null(1),
        "nullable Bool null must survive export"
    );
    assert!(!active_array.value(2));

    assert_eq!(unsafe { loom_duckdb_arrow_semantic_destroy(handle) }, 0);
}

#[test]
fn arrow_semantic_handle_accepts_direct_lma1_bridge() {
    let artifact = arrow_semantic_lma1();
    let mut handle: *mut LoomDuckDbArrowSemantic = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_arrow_semantic_create(
                artifact.as_ptr(),
                artifact.len(),
                &mut handle as *mut _,
            )
        },
        0
    );
    assert!(
        !handle.is_null(),
        "direct LMA1 bridge handle must be populated"
    );
    assert_eq!(unsafe { arrow_semantic_column_name(handle, 0) }, "id");
    assert_eq!(unsafe { arrow_semantic_column_format(handle, 0) }, "i");
    let id_array = unsafe { export_arrow_semantic_i32_column(handle, 0) };
    assert_eq!(id_array.values(), &[10, 20, 30]);
    assert_eq!(unsafe { loom_duckdb_arrow_semantic_destroy(handle) }, 0);
}

#[test]
fn arrow_semantic_handle_rejects_invalid_inputs_and_multibatch() {
    let mut handle: *mut LoomDuckDbArrowSemantic = ptr::null_mut();
    let rc = unsafe { loom_duckdb_arrow_semantic_create(ptr::null(), 3, &mut handle as *mut _) };
    assert_ne!(rc, 0, "null non-empty input must fail");
    assert!(handle.is_null());

    let garbage = b"not an artifact";
    let rc = unsafe {
        loom_duckdb_arrow_semantic_create(garbage.as_ptr(), garbage.len(), &mut handle as *mut _)
    };
    assert_ne!(rc, 0, "garbage bytes must fail closed");
    assert!(handle.is_null());

    let batch = arrow_semantic_record_batch();
    let payload = ArrowSemanticPayload::from_record_batches(&[batch.clone(), batch])
        .expect("multi-batch semantic payload");
    let multibatch =
        encode_arrow_semantic_container_payload(&payload).expect("encode multi-batch LMC2");
    let rc = unsafe {
        loom_duckdb_arrow_semantic_create(
            multibatch.as_ptr(),
            multibatch.len(),
            &mut handle as *mut _,
        )
    };
    assert_ne!(rc, 0, "multi-batch LMC2 must be unsupported for DuckDB SQL");
    assert!(handle.is_null());
}

#[test]
fn fallback_and_strict_modes_expose_runtime_policy_diagnostics() {
    let artifact = raw_i32_lmc1(4);

    let mut fallback_plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_plan_create(
                artifact.as_ptr(),
                artifact.len(),
                true,
                false,
                &mut fallback_plan as *mut _,
            )
        },
        0
    );
    assert_eq!(
        unsafe { plan_decision_string(fallback_plan) },
        "interpreter-fallback"
    );
    assert!(
        unsafe { plan_diagnostic_codes(fallback_plan) }
            .iter()
            .any(|code| code == "lowering-unsupported"),
        "fallback mode should expose runtime/lowering diagnostics"
    );
    assert_eq!(unsafe { loom_duckdb_plan_destroy(fallback_plan) }, 0);

    let mut strict_plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_plan_create(
                artifact.as_ptr(),
                artifact.len(),
                false,
                false,
                &mut strict_plan as *mut _,
            )
        },
        0
    );
    assert_eq!(unsafe { plan_decision_string(strict_plan) }, "fail-closed");
    assert!(
        unsafe { plan_diagnostic_codes(strict_plan) }
            .iter()
            .any(|code| code == "fallback-disabled" || code == "lowering-unsupported"),
        "strict mode should expose fail-closed policy diagnostics"
    );
    assert_eq!(unsafe { loom_duckdb_plan_destroy(strict_plan) }, 0);
}

#[test]
fn diagnostic_out_of_range_returns_error_without_mutating_output() {
    let artifact = raw_i32_lmc1(4);
    let mut plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_plan_create(
                artifact.as_ptr(),
                artifact.len(),
                true,
                false,
                &mut plan as *mut _,
            )
        },
        0
    );

    let sentinel = LoomDuckDbDiagnostic {
        code: std::ptr::dangling(),
        path: std::ptr::dangling(),
        message: std::ptr::dangling(),
    };
    let mut diagnostic = sentinel;
    let rc = unsafe { loom_duckdb_plan_diagnostic(plan, usize::MAX, &mut diagnostic as *mut _) };
    assert_ne!(rc, 0, "out-of-range diagnostic access must fail");
    assert_eq!(diagnostic.code, sentinel.code);
    assert_eq!(diagnostic.path, sentinel.path);
    assert_eq!(diagnostic.message, sentinel.message);

    assert_eq!(unsafe { loom_duckdb_plan_destroy(plan) }, 0);
}

#[test]
fn cancelled_prepare_exposes_backend_diagnostic_and_no_native_buffers() {
    let _guard = isolated_prepare_cache();
    let artifact = raw_i32_lmc1(4);
    let mut plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_plan_create(
                artifact.as_ptr(),
                artifact.len(),
                false,
                true,
                &mut plan as *mut _,
            )
        },
        0
    );

    let mut prepared: *mut LoomDuckDbPrepared = ptr::null_mut();
    assert_eq!(
        unsafe { loom_duckdb_prepare_create(plan, true, &mut prepared as *mut _) },
        0
    );
    assert_eq!(unsafe { prepare_route_string(prepared) }, "cancelled");
    assert!(
        unsafe { prepare_diagnostic_codes(prepared) }
            .iter()
            .any(|code| code == "cancelled"),
        "cancelled prepare should expose backend cancellation diagnostic"
    );

    let mut buffer_count = usize::MAX;
    assert_eq!(
        unsafe { loom_duckdb_prepare_native_buffer_count(prepared, &mut buffer_count as *mut _) },
        0
    );
    assert_eq!(buffer_count, 0);

    assert_eq!(unsafe { loom_duckdb_prepare_destroy(prepared) }, 0);
    assert_eq!(unsafe { loom_duckdb_plan_destroy(plan) }, 0);
}

#[test]
fn native_buffer_access_is_empty_unless_route_is_native_candidate() {
    let _guard = isolated_prepare_cache();
    let artifact = raw_i32_lmc1(4);
    let mut plan: *mut LoomDuckDbPlan = ptr::null_mut();
    assert_eq!(
        unsafe {
            loom_duckdb_plan_create(
                artifact.as_ptr(),
                artifact.len(),
                false,
                true,
                &mut plan as *mut _,
            )
        },
        0
    );

    let mut prepared: *mut LoomDuckDbPrepared = ptr::null_mut();
    assert_eq!(
        unsafe { loom_duckdb_prepare_create(plan, false, &mut prepared as *mut _) },
        0
    );

    let route = unsafe { prepare_route_string(prepared) };
    let mut buffer_count = usize::MAX;
    assert_eq!(
        unsafe { loom_duckdb_prepare_native_buffer_count(prepared, &mut buffer_count as *mut _) },
        0
    );

    if route == "native-candidate" {
        assert!(
            buffer_count > 0,
            "native route should expose native buffers"
        );
        let mut buffer = LoomDuckDbNativeBuffer::default();
        assert_eq!(
            unsafe { loom_duckdb_prepare_native_buffer(prepared, 0, &mut buffer as *mut _) },
            0
        );
        assert!(!buffer.builder_id.is_null());
        assert!(!buffer.arrow_type.is_null());
        assert!(!buffer.value_ptr.is_null());
        assert!(buffer.value_len > 0);
    } else {
        assert_eq!(
            buffer_count, 0,
            "non-native route {route} must not expose native buffers"
        );
    }

    assert_eq!(unsafe { loom_duckdb_prepare_destroy(prepared) }, 0);
    assert_eq!(unsafe { loom_duckdb_plan_destroy(plan) }, 0);
}

#[test]
fn public_header_leakage_gate_blocks_route_and_stream_symbols() {
    let public_header =
        std::fs::read_to_string(format!("{}/include/loom.h", env!("CARGO_MANIFEST_DIR")))
            .expect("read generated public header");
    for forbidden in [
        "loom_duckdb_",
        "LoomDuckDb",
        "loom_duckdb_arrow_semantic",
        "LoomDuckDbArrowSemantic",
        "duckdb_runtime",
        "cache",
        "native_preparation",
        "loom_scan_native",
        "loom_scan_interpreter",
        "ArrowArrayStream",
    ] {
        assert!(
            !public_header.contains(forbidden),
            "public loom.h must not expose {forbidden}"
        );
    }
}

unsafe fn plan_decision_string(plan: *mut LoomDuckDbPlan) -> String {
    let mut decision = ptr::null();
    assert_eq!(loom_duckdb_plan_decision(plan, &mut decision as *mut _), 0);
    cstr(decision)
}

unsafe fn plan_diagnostic_codes(plan: *mut LoomDuckDbPlan) -> Vec<String> {
    let mut diagnostic_count = 0usize;
    assert_eq!(
        loom_duckdb_plan_diagnostic_count(plan, &mut diagnostic_count as *mut _),
        0
    );
    (0..diagnostic_count)
        .map(|index| {
            let mut diagnostic = LoomDuckDbDiagnostic::default();
            assert_eq!(
                loom_duckdb_plan_diagnostic(plan, index, &mut diagnostic as *mut _),
                0
            );
            cstr(diagnostic.code)
        })
        .collect()
}

unsafe fn prepare_route_string(prepared: *mut LoomDuckDbPrepared) -> String {
    let mut route = ptr::null();
    assert_eq!(loom_duckdb_prepare_route(prepared, &mut route as *mut _), 0);
    cstr(route)
}

unsafe fn arrow_semantic_column_name(handle: *mut LoomDuckDbArrowSemantic, index: usize) -> String {
    let mut name = ptr::null();
    assert_eq!(
        loom_duckdb_arrow_semantic_column_name(handle, index, &mut name as *mut _),
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
        loom_duckdb_arrow_semantic_column_format(handle, index, &mut format as *mut _),
        0
    );
    cstr(format)
}

unsafe fn export_arrow_semantic_i32_column(
    handle: *mut LoomDuckDbArrowSemantic,
    index: usize,
) -> Int32Array {
    let (array, schema) = export_arrow_semantic_column(handle, index);
    let array_data = from_ffi(array, &schema).expect("from_ffi i32 column");
    let array = Int32Array::from(array_data);
    release_schema(schema);
    array
}

unsafe fn export_arrow_semantic_string_column(
    handle: *mut LoomDuckDbArrowSemantic,
    index: usize,
) -> StringArray {
    let (array, schema) = export_arrow_semantic_column(handle, index);
    let array_data = from_ffi(array, &schema).expect("from_ffi Utf8 column");
    let array = StringArray::from(array_data);
    release_schema(schema);
    array
}

unsafe fn export_arrow_semantic_bool_column(
    handle: *mut LoomDuckDbArrowSemantic,
    index: usize,
) -> BooleanArray {
    let (array, schema) = export_arrow_semantic_column(handle, index);
    let array_data = from_ffi(array, &schema).expect("from_ffi Bool column");
    let array = BooleanArray::from(array_data);
    release_schema(schema);
    array
}

unsafe fn export_arrow_semantic_column(
    handle: *mut LoomDuckDbArrowSemantic,
    index: usize,
) -> (FFI_ArrowArray, FFI_ArrowSchema) {
    let mut array: FFI_ArrowArray = std::mem::zeroed();
    let mut schema: FFI_ArrowSchema = std::mem::zeroed();
    assert_eq!(
        loom_duckdb_arrow_semantic_export_column(
            handle,
            index,
            &mut array as *mut _,
            &mut schema as *mut _,
        ),
        0
    );
    (array, schema)
}

fn release_schema(mut schema: FFI_ArrowSchema) {
    if let Some(release_fn) = schema.release {
        unsafe { release_fn(&mut schema as *mut _) };
    }
}

unsafe fn prepare_diagnostic_codes(prepared: *mut LoomDuckDbPrepared) -> Vec<String> {
    let mut diagnostic_count = 0usize;
    assert_eq!(
        loom_duckdb_prepare_diagnostic_count(prepared, &mut diagnostic_count as *mut _),
        0
    );
    (0..diagnostic_count)
        .map(|index| {
            let mut diagnostic = LoomDuckDbDiagnostic::default();
            assert_eq!(
                loom_duckdb_prepare_diagnostic(prepared, index, &mut diagnostic as *mut _),
                0
            );
            cstr(diagnostic.code)
        })
        .collect()
}
