use std::sync::Arc;

use arrow_array::{
    ArrayRef, BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array, RecordBatch,
    StringArray,
};
use arrow_schema::{DataType, Field, Schema};
use loom_core::arrow_semantic::ArrowSemanticPayload;
use loom_core::arrow_semantic_codec::encode_arrow_semantic_container_payload;
#[cfg(feature = "melior")]
use loom_core::native_arrow_semantic::prepare_native_arrow_semantic_codegen_support;
use loom_core::runtime_abi::{
    PredicateEnvelope, ProjectionSet, RuntimeFallbackPolicy, RuntimeSafetyPolicy, SplitDescriptor,
};
use loom_native_melior::backend::{NativeBackendCancellation, NativeBackendDiagnosticCode};
#[cfg(feature = "melior")]
use loom_native_melior::jit::{
    execute_arrow_semantic_codegen_jit, validate_arrow_semantic_codegen_production_route_output,
};
use loom_native_melior::jit::{
    execute_arrow_semantic_codegen_production_route, ArrowSemanticCodegenRouteStatus,
};

#[cfg(feature = "melior")]
#[test]
fn positive_route_uses_real_jit_validation_replay_and_cache_admission() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let route = execute_arrow_semantic_codegen_production_route(
        &bytes,
        &NativeBackendCancellation::default(),
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
        RuntimeSafetyPolicy::default(),
    );

    assert_eq!(route.status, ArrowSemanticCodegenRouteStatus::NativeCandidate);
    assert!(route.cacheable);
    assert!(route.diagnostics.is_empty(), "{:?}", route.diagnostics);
    assert!(route.support.is_supported());
    assert!(route.jit_output.is_some());
    assert!(route
        .execution
        .as_ref()
        .expect("execution")
        .is_supported());
    let replay = route.replay_evidence.expect("replay evidence");
    assert_eq!(replay.artifact_kind, "LMC2");
    assert!(replay
        .runtime_cache_canonical_input
        .contains("validation=native-model:phase40"));
}

#[cfg(feature = "melior")]
#[test]
fn route_output_divergence_fails_closed_or_falls_back_without_cache_admission() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let support = prepare_native_arrow_semantic_codegen_support(&bytes);
    let mut jit = execute_arrow_semantic_codegen_jit(&support, &NativeBackendCancellation::default())
        .expect("real JIT output");
    jit.columns[1].value_buffer[0] ^= 0x7f;

    let strict = validate_arrow_semantic_codegen_production_route_output(
        &bytes,
        support.clone(),
        jit.clone(),
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
        RuntimeSafetyPolicy::default(),
    );
    assert_eq!(strict.status, ArrowSemanticCodegenRouteStatus::FailClosed);
    assert!(!strict.cacheable);
    assert!(strict.replay_evidence.is_none());
    assert!(strict
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == NativeBackendDiagnosticCode::NativeOutputMismatch));

    let mut fallback_policy = RuntimeSafetyPolicy::default();
    fallback_policy.fallback = RuntimeFallbackPolicy::AllowInterpreter;
    let fallback = validate_arrow_semantic_codegen_production_route_output(
        &bytes,
        support,
        jit,
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
        fallback_policy,
    );
    assert_eq!(
        fallback.status,
        ArrowSemanticCodegenRouteStatus::InterpreterFallback
    );
    assert!(!fallback.cacheable);
    assert!(fallback.replay_evidence.is_none());
}

#[test]
fn unsupported_route_fails_closed_or_falls_back_without_jit_or_cache() {
    let bytes = encode_lmc2(&utf8_batch());
    let strict = execute_arrow_semantic_codegen_production_route(
        &bytes,
        &NativeBackendCancellation::default(),
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 3 },
        RuntimeSafetyPolicy::default(),
    );
    assert_eq!(strict.status, ArrowSemanticCodegenRouteStatus::FailClosed);
    assert!(!strict.cacheable);
    assert!(strict.jit_output.is_none());
    assert!(strict.replay_evidence.is_none());
    assert!(strict
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == NativeBackendDiagnosticCode::InvalidBackendArtifact));

    let mut fallback_policy = RuntimeSafetyPolicy::default();
    fallback_policy.fallback = RuntimeFallbackPolicy::AllowInterpreter;
    let fallback = execute_arrow_semantic_codegen_production_route(
        &bytes,
        &NativeBackendCancellation::default(),
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 3 },
        fallback_policy,
    );
    assert_eq!(
        fallback.status,
        ArrowSemanticCodegenRouteStatus::InterpreterFallback
    );
    assert!(!fallback.cacheable);
}

#[test]
fn route_cancellation_is_distinct_and_non_cacheable() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let route = execute_arrow_semantic_codegen_production_route(
        &bytes,
        &NativeBackendCancellation::cancelled("phase43.2 route cancellation"),
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
        RuntimeSafetyPolicy::default(),
    );
    assert_eq!(route.status, ArrowSemanticCodegenRouteStatus::Cancelled);
    assert!(!route.cacheable);
    assert!(route.replay_evidence.is_none());
    assert!(route
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == NativeBackendDiagnosticCode::Cancelled));
}

#[cfg(not(feature = "melior"))]
#[test]
fn default_route_requires_melior_feature_and_cannot_seed_cache() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let route = execute_arrow_semantic_codegen_production_route(
        &bytes,
        &NativeBackendCancellation::default(),
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
        RuntimeSafetyPolicy::default(),
    );
    assert_eq!(route.status, ArrowSemanticCodegenRouteStatus::FailClosed);
    assert!(!route.cacheable);
    assert!(route.replay_evidence.is_none());
    assert!(route
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == NativeBackendDiagnosticCode::JitUnavailable));
}

fn full_primitive_nullable_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("ok", DataType::Boolean, true),
        Field::new("id", DataType::Int32, true),
        Field::new("count", DataType::Int64, true),
        Field::new("ratio", DataType::Float32, true),
        Field::new("score", DataType::Float64, true),
    ]));
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(BooleanArray::from(vec![
                Some(true),
                None,
                Some(false),
                Some(true),
                Some(false),
                None,
                Some(true),
                Some(false),
                Some(true),
            ])) as ArrayRef,
            Arc::new(Int32Array::from(vec![
                Some(7),
                None,
                Some(-1),
                Some(128),
                Some(-2048),
                None,
                Some(33),
                Some(44),
                Some(55),
            ])) as ArrayRef,
            Arc::new(Int64Array::from(vec![
                Some(70),
                None,
                Some(-10),
                Some(7000),
                Some(-9000),
                None,
                Some(330),
                Some(440),
                Some(550),
            ])) as ArrayRef,
            Arc::new(Float32Array::from(vec![
                Some(0.25),
                None,
                Some(-1.5),
                Some(3.75),
                Some(-8.5),
                None,
                Some(9.25),
                Some(10.5),
                Some(11.75),
            ])) as ArrayRef,
            Arc::new(Float64Array::from(vec![
                Some(1.5),
                None,
                Some(-2.25),
                Some(4.5),
                Some(-16.75),
                None,
                Some(18.25),
                Some(20.5),
                Some(22.75),
            ])) as ArrayRef,
        ],
    )
    .expect("full primitive nullable batch")
}

fn utf8_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![Field::new("name", DataType::Utf8, true)]));
    RecordBatch::try_new(
        schema,
        vec![Arc::new(StringArray::from(vec![Some("alpha"), None, Some("beta")])) as ArrayRef],
    )
    .expect("utf8 batch")
}

fn encode_lmc2(batch: &RecordBatch) -> Vec<u8> {
    let payload = ArrowSemanticPayload::from_record_batches(&[batch.clone()]).expect("payload");
    encode_arrow_semantic_container_payload(&payload).expect("encode LMC2")
}
