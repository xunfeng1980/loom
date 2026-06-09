use std::sync::Arc;

use arrow::array::{Int32Array, RecordBatch};
use arrow::datatypes::{DataType, Field, Schema};
use loom_core::arrow_semantic::ArrowSemanticPayload;
use loom_core::arrow_semantic_codec::encode_arrow_semantic_payload;
use loom_core::container_codec::wrap_layout_payload;
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::layout_codec::encode_layout_payload;
use loom_ffi::duckdb_runtime::{
    plan_duckdb_runtime, prepare_duckdb_runtime, DuckDbNativeBuffer, DuckDbProjection,
    DuckDbRouteDecision, DuckDbRuntimeDiagnostic, DuckDbRuntimePlanInput, DuckDbRuntimePlanReport,
    DuckDbRuntimePolicy,
};
use loom_native_melior::backend::NativeBackendCancellation;

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

fn lma1_i32(row_count: usize) -> Vec<u8> {
    let schema = Arc::new(Schema::new(vec![Field::new(
        "value",
        DataType::Int32,
        false,
    )]));
    let batch = RecordBatch::try_new(
        schema,
        vec![Arc::new(Int32Array::from(
            (0..row_count as i32).collect::<Vec<_>>(),
        ))],
    )
    .expect("record batch");
    let payload = ArrowSemanticPayload::from_record_batches(&[batch]).expect("payload");
    encode_arrow_semantic_payload(&payload).expect("encode LMA1")
}

fn native_plan() -> DuckDbRuntimePlanReport {
    plan_duckdb_runtime(DuckDbRuntimePlanInput {
        artifact_bytes: lma1_i32(4),
        projection: DuckDbProjection::All,
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback: false,
        },
    })
    .expect("native plan")
}

fn diagnostic_codes(diagnostics: &[DuckDbRuntimeDiagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

fn buffer_fingerprints(buffers: &[DuckDbNativeBuffer]) -> Vec<(String, DataType, u64, Vec<u8>)> {
    buffers
        .iter()
        .map(|buffer| {
            (
                buffer.builder_id.clone(),
                buffer.arrow_type.clone(),
                buffer.row_count,
                buffer.value_buffer.clone(),
            )
        })
        .collect()
}

#[test]
fn repeated_arrow_semantic_prepare_is_deterministic_without_raw_copy_cache() {
    let plan = native_plan();

    let first = prepare_duckdb_runtime(&plan, NativeBackendCancellation::default());
    let second = prepare_duckdb_runtime(&plan, NativeBackendCancellation::default());

    assert_eq!(first.decision, DuckDbRouteDecision::NativeCandidate);
    assert_eq!(second.decision, DuckDbRouteDecision::NativeCandidate);
    assert_eq!(
        buffer_fingerprints(&first.native_buffers),
        buffer_fingerprints(&second.native_buffers)
    );
    let first_codes = diagnostic_codes(&first.diagnostics);
    let second_codes = diagnostic_codes(&second.diagnostics);
    assert!(!first_codes.contains(&"cache-hit"));
    assert!(!first_codes.contains(&"cache-inserted"));
    assert!(!second_codes.contains(&"cache-hit"));
    assert!(!second_codes.contains(&"cache-inserted"));
    assert!(first_codes.contains(&"native-arrow-semantic-codegen-output"));
    assert!(second_codes.contains(&"native-arrow-semantic-codegen-output"));
}

#[test]
fn unsafe_routes_are_non_cacheable_and_do_not_expose_native_buffers() {
    let plan = native_plan();
    let cancelled = prepare_duckdb_runtime(
        &plan,
        NativeBackendCancellation::cancelled("duckdb interrupt"),
    );
    assert_eq!(cancelled.decision, DuckDbRouteDecision::Cancelled);
    assert!(cancelled.native_buffers.is_empty());
    assert!(diagnostic_codes(&cancelled.diagnostics).contains(&"cache-non-cacheable"));

    let fallback = plan_duckdb_runtime(DuckDbRuntimePlanInput {
        artifact_bytes: raw_i32_lmc1(4),
        projection: DuckDbProjection::All,
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback: true,
        },
    })
    .expect("fallback plan");
    assert_eq!(fallback.decision, DuckDbRouteDecision::InterpreterFallback);
    let fallback = prepare_duckdb_runtime(&fallback, NativeBackendCancellation::default());
    assert_eq!(fallback.decision, DuckDbRouteDecision::InterpreterFallback);
    assert!(fallback.native_buffers.is_empty());
    assert!(diagnostic_codes(&fallback.diagnostics).contains(&"cache-non-cacheable"));
}
