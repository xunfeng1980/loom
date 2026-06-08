use std::ffi::CStr;
use std::ptr;

use arrow::datatypes::DataType;
use loom_core::container_codec::wrap_layout_payload;
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::layout_codec::encode_layout_payload;
use loom_ffi::duckdb_runtime::{
    loom_duckdb_plan_cache_key, loom_duckdb_plan_create, loom_duckdb_plan_decision,
    loom_duckdb_plan_destroy, loom_duckdb_plan_diagnostic, loom_duckdb_plan_diagnostic_count,
    loom_duckdb_prepare_create, loom_duckdb_prepare_destroy,
    loom_duckdb_prepare_native_buffer_count, loom_duckdb_prepare_route, LoomDuckDbDiagnostic,
    LoomDuckDbPlan, LoomDuckDbPrepared,
};

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
fn prepare_create_returns_handle_and_route_without_unwinding() {
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
fn public_header_excludes_internal_duckdb_symbols() {
    let public_header =
        std::fs::read_to_string(format!("{}/include/loom.h", env!("CARGO_MANIFEST_DIR")))
            .expect("read generated public header");
    assert!(public_header.contains("loom_decode"));
    assert!(!public_header.contains("loom_duckdb_"));
    assert!(!public_header.contains("LoomDuckDb"));
}
