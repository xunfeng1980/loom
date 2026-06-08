use std::ffi::CStr;
use std::ptr;

use arrow::datatypes::DataType;
use loom_core::container_codec::wrap_layout_payload;
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::layout_codec::encode_layout_payload;
use loom_ffi::duckdb_runtime::{
    loom_duckdb_plan_cache_key, loom_duckdb_plan_create, loom_duckdb_plan_decision,
    loom_duckdb_plan_destroy, loom_duckdb_plan_diagnostic, loom_duckdb_plan_diagnostic_count,
    loom_duckdb_prepare_create, loom_duckdb_prepare_destroy, loom_duckdb_prepare_diagnostic,
    loom_duckdb_prepare_diagnostic_count, loom_duckdb_prepare_native_buffer,
    loom_duckdb_prepare_native_buffer_count, loom_duckdb_prepare_route, LoomDuckDbDiagnostic,
    LoomDuckDbNativeBuffer, LoomDuckDbPlan, LoomDuckDbPrepared,
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
