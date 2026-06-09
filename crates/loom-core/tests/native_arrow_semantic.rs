use std::sync::Arc;

use arrow_array::{
    Array, ArrayRef, BooleanArray, Date32Array, Float32Array, Float64Array, Int32Array, Int64Array,
    RecordBatch, StringArray, StructArray,
};
use arrow_schema::{DataType, Field, Schema};
use loom_core::arrow_semantic::ArrowSemanticPayload;
use loom_core::arrow_semantic_codec::{
    encode_arrow_semantic_container_payload, encode_arrow_semantic_payload,
};
use loom_core::native_arrow_semantic::{
    decide_native_arrow_semantic_runtime, decide_validated_native_arrow_semantic_runtime,
    execute_native_arrow_semantic, native_arrow_semantic_backend_identity,
    native_arrow_semantic_runtime_cache_key, validated_native_arrow_semantic_runtime_cache_key,
    verify_native_arrow_semantic_equivalence, verify_native_arrow_semantic_model,
    verify_native_arrow_semantic_model_output, verify_native_arrow_semantic_output_equivalence,
    NativeArrowSemanticDiagnosticCode, NATIVE_ARROW_SEMANTIC_BACKEND,
};
use loom_core::runtime_abi::{
    ProjectionColumn, ProjectionSet, RuntimeExecutionDecision, RuntimeFallbackPolicy,
    RuntimeSafetyPolicy,
};

#[test]
fn wrapped_lmc2_nullable_primitives_execute_natively_and_equivalently() {
    let batch = primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let report = execute_native_arrow_semantic(&bytes);

    assert!(
        report.is_supported(),
        "unexpected diagnostics: {:?}",
        report.diagnostics()
    );
    assert_eq!(report.backend, NATIVE_ARROW_SEMANTIC_BACKEND);
    assert_eq!(report.artifact_kind, "LMC2");
    assert_eq!(report.payload_kind, "Arrow semantic payload");
    assert_eq!(report.row_count, 3);
    assert_eq!(report.column_count, 4);

    let output = report.output().expect("native output");
    assert_eq!(output, &batch);
    assert_ne!(
        output.column(0).as_ref() as *const _,
        batch.column(0).as_ref() as *const _,
        "native execution should produce a new Arrow array object"
    );

    let equivalence = verify_native_arrow_semantic_equivalence(&bytes);
    assert!(equivalence.is_equivalent(), "{equivalence:?}");
    assert_eq!(equivalence.backend, NATIVE_ARROW_SEMANTIC_BACKEND);
    assert_eq!(equivalence.artifact_kind, "LMC2");
    assert_eq!(equivalence.row_count, 3);
    assert_eq!(equivalence.column_count, 4);
}

#[test]
fn direct_lma1_bridge_executes_natively_as_regression_input() {
    let batch = primitive_nullable_batch();
    let bytes = encode_lma1(&batch);
    let report = execute_native_arrow_semantic(&bytes);

    assert!(
        report.is_supported(),
        "unexpected diagnostics: {:?}",
        report.diagnostics()
    );
    assert_eq!(report.artifact_kind, "LMA1");
    assert_eq!(report.output().expect("output"), &batch);
}

#[test]
fn unsupported_utf8_logical_and_nested_shapes_fail_closed() {
    for (bytes, expected_path) in [
        (encode_lmc2(&utf8_batch()), "$.schema.fields[0].type"),
        (encode_lmc2(&date32_batch()), "$.schema.fields[0].type"),
        (encode_lmc2(&struct_batch()), "$.schema.fields[0].type"),
    ] {
        let report = execute_native_arrow_semantic(&bytes);
        assert!(!report.is_supported());
        assert!(report.output().is_none());
        let diagnostic = report.first_error().expect("diagnostic");
        assert_eq!(diagnostic.code, NativeArrowSemanticDiagnosticCode::UnsupportedType);
        assert_eq!(diagnostic.path, expected_path);
    }
}

#[test]
fn multi_batch_payload_fails_closed_before_native_output() {
    let batch = primitive_nullable_batch();
    let payload =
        ArrowSemanticPayload::from_record_batches(&[batch.clone(), batch]).expect("multi batch");
    let bytes = encode_arrow_semantic_container_payload(&payload).expect("encode LMC2");
    let report = execute_native_arrow_semantic(&bytes);

    assert!(!report.is_supported());
    assert!(report.output().is_none());
    assert_eq!(
        report.first_error().expect("diagnostic").code,
        NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape
    );
}

#[test]
fn malformed_or_non_arrow_semantic_inputs_do_not_execute() {
    for bytes in [b"NOPE".as_slice(), b"LMC2".as_slice()] {
        let report = execute_native_arrow_semantic(bytes);
        assert!(!report.is_supported());
        assert!(report.output().is_none());
        assert_eq!(
            report.first_error().expect("diagnostic").code,
            NativeArrowSemanticDiagnosticCode::VerifierRejected
        );
    }
}

#[test]
fn injected_native_output_mismatch_is_explicit_equivalence_failure() {
    let batch = primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let wrong_batch = RecordBatch::try_new(
        batch.schema(),
        vec![
            Arc::new(BooleanArray::from(vec![Some(false), None, Some(true)])) as ArrayRef,
            batch.column(1).clone(),
            batch.column(2).clone(),
            batch.column(3).clone(),
        ],
    )
    .expect("wrong batch");
    let equivalence = verify_native_arrow_semantic_output_equivalence(&bytes, "LMC2", &wrong_batch);
    assert!(!equivalence.is_equivalent());
    assert_eq!(
        equivalence.diagnostics()[0].code,
        NativeArrowSemanticDiagnosticCode::NativeOutputMismatch
    );
}

#[test]
fn native_model_validation_matches_reference_trace_for_lmc2_supported_matrix() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let validation = verify_native_arrow_semantic_model(&bytes);

    assert!(
        validation.is_validated(),
        "unexpected diagnostics: {:?}",
        validation.diagnostics()
    );
    assert_eq!(validation.backend, NATIVE_ARROW_SEMANTIC_BACKEND);
    assert_eq!(validation.artifact_kind, "LMC2");
    assert_eq!(validation.row_count, 3);
    assert_eq!(validation.column_count, 5);
    assert!(validation.model_trace_matches);
    assert!(validation.value_equivalent);
    assert_eq!(validation.reference_trace(), validation.native_trace());
    assert!(validation
        .reference_trace()
        .iter()
        .any(|line| line == "append-value:col3:ratio:float32"));
    assert!(validation
        .reference_trace()
        .iter()
        .any(|line| line == "append-value:col4:score:float64"));
}

#[test]
fn native_model_validation_covers_direct_lma1_bridge() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lma1(&batch);
    let validation = verify_native_arrow_semantic_model(&bytes);

    assert!(validation.is_validated(), "{validation:?}");
    assert_eq!(validation.artifact_kind, "LMA1");
    assert_eq!(validation.reference_trace(), validation.native_trace());
}

#[test]
fn injected_native_model_trace_divergence_fails_validation() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let wrong_batch = RecordBatch::try_new(
        batch.schema(),
        vec![
            Arc::new(BooleanArray::from(vec![Some(true), Some(true), Some(false)])) as ArrayRef,
            batch.column(1).clone(),
            batch.column(2).clone(),
            batch.column(3).clone(),
            batch.column(4).clone(),
        ],
    )
    .expect("wrong batch");
    let validation = verify_native_arrow_semantic_model_output(&bytes, "LMC2", &wrong_batch);

    assert!(!validation.is_validated());
    assert!(!validation.model_trace_matches);
    assert!(!validation.value_equivalent);
    assert_ne!(validation.reference_trace(), validation.native_trace());
    assert!(validation.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch
            && diagnostic.path == "$.native.model_trace"
    }));
    assert!(validation.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == NativeArrowSemanticDiagnosticCode::NativeOutputMismatch
    }));
}

#[test]
fn validated_native_model_runtime_and_cache_require_successful_validation() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let validation = verify_native_arrow_semantic_model(&bytes);
    assert!(validation.is_validated(), "{validation:?}");

    let decision =
        decide_validated_native_arrow_semantic_runtime(&validation, RuntimeSafetyPolicy::default());
    assert_eq!(decision.decision, RuntimeExecutionDecision::NativeCandidate);
    assert!(decision.diagnostics.is_empty());

    let key = validated_native_arrow_semantic_runtime_cache_key(
        &bytes,
        &validation,
        ProjectionSet::All,
        RuntimeSafetyPolicy::default(),
    )
    .expect("validated native/model cache key");
    assert!(key
        .canonical_input
        .contains("backend=loom-native-arrow-semantic:phase40-native-model-validation"));
    assert!(key
        .canonical_input
        .contains("validation=native-model:phase40"));
    assert!(key.canonical_input.contains("reference-trace"));
    assert!(key.canonical_input.contains("native-trace"));
}

#[test]
fn divergent_native_model_validation_fails_closed_and_is_not_cacheable() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let wrong_batch = RecordBatch::try_new(
        batch.schema(),
        vec![
            Arc::new(BooleanArray::from(vec![Some(true), Some(true), Some(false)])) as ArrayRef,
            batch.column(1).clone(),
            batch.column(2).clone(),
            batch.column(3).clone(),
            batch.column(4).clone(),
        ],
    )
    .expect("wrong batch");
    let validation = verify_native_arrow_semantic_model_output(&bytes, "LMC2", &wrong_batch);
    assert!(!validation.is_validated());

    let decision =
        decide_validated_native_arrow_semantic_runtime(&validation, RuntimeSafetyPolicy::default());
    assert_eq!(decision.decision, RuntimeExecutionDecision::FailClosed);

    let err = validated_native_arrow_semantic_runtime_cache_key(
        &bytes,
        &validation,
        ProjectionSet::All,
        RuntimeSafetyPolicy::default(),
    )
    .expect_err("divergent native/model validation must not be cacheable");
    assert_eq!(
        err.code,
        NativeArrowSemanticDiagnosticCode::UnsupportedPayload
    );
    assert_eq!(err.path, "$.cache.native_arrow_semantic_model");
    assert!(err
        .message
        .contains("only successful native/model validation may seed runtime cache keys"));
}

#[test]
fn native_arrow_semantic_runtime_cache_identity_is_engine_neutral() {
    let batch = primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let execution = execute_native_arrow_semantic(&bytes);
    let identity = native_arrow_semantic_backend_identity();
    assert_eq!(identity.backend, NATIVE_ARROW_SEMANTIC_BACKEND);
    assert_eq!(identity.target_triple, "engine-neutral");

    let decision = decide_native_arrow_semantic_runtime(&execution, RuntimeSafetyPolicy::default());
    assert_eq!(decision.decision, RuntimeExecutionDecision::NativeCandidate);
    assert!(decision.diagnostics.is_empty());

    let all = native_arrow_semantic_runtime_cache_key(
        &bytes,
        &execution,
        ProjectionSet::All,
        RuntimeSafetyPolicy::default(),
    )
    .expect("native cache key");
    assert!(all
        .canonical_input
        .contains("backend=loom-native-arrow-semantic:phase35"));
    assert!(all.canonical_input.contains("projection=all"));

    let projected = native_arrow_semantic_runtime_cache_key(
        &bytes,
        &execution,
        ProjectionSet::Columns(vec![ProjectionColumn {
            source_index: 1,
            output_index: 0,
        }]),
        RuntimeSafetyPolicy::default(),
    )
    .expect("projected native cache key");
    assert_ne!(all, projected);
    assert!(projected.canonical_input.contains("projection=columns:1>0"));
}

#[test]
fn unsupported_arrow_semantic_execution_cannot_seed_native_cache() {
    let bytes = encode_lmc2(&utf8_batch());
    let execution = execute_native_arrow_semantic(&bytes);
    assert!(!execution.is_supported());

    let decision = decide_native_arrow_semantic_runtime(&execution, RuntimeSafetyPolicy::default());
    assert_eq!(decision.decision, RuntimeExecutionDecision::FailClosed);

    let err = native_arrow_semantic_runtime_cache_key(
        &bytes,
        &execution,
        ProjectionSet::All,
        RuntimeSafetyPolicy::default(),
    )
    .expect_err("unsupported native execution must not be cacheable");
    assert_eq!(
        err.code,
        NativeArrowSemanticDiagnosticCode::UnsupportedPayload
    );
    assert_eq!(err.path, "$.cache.native_arrow_semantic");

    let mut fallback_policy = RuntimeSafetyPolicy::default();
    fallback_policy.fallback = RuntimeFallbackPolicy::AllowInterpreter;
    let fallback_decision = decide_native_arrow_semantic_runtime(&execution, fallback_policy);
    assert_eq!(
        fallback_decision.decision,
        RuntimeExecutionDecision::InterpreterFallback
    );
    assert!(native_arrow_semantic_runtime_cache_key(
        &bytes,
        &execution,
        ProjectionSet::All,
        fallback_policy,
    )
    .is_err());
}

fn primitive_nullable_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("ok", DataType::Boolean, true),
        Field::new("id", DataType::Int32, true),
        Field::new("count", DataType::Int64, true),
        Field::new("score", DataType::Float64, true),
    ]));
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(BooleanArray::from(vec![Some(true), None, Some(false)])) as ArrayRef,
            Arc::new(Int32Array::from(vec![Some(7), None, Some(-1)])) as ArrayRef,
            Arc::new(Int64Array::from(vec![Some(70), None, Some(-10)])) as ArrayRef,
            Arc::new(Float64Array::from(vec![Some(1.5), None, Some(-2.25)])) as ArrayRef,
        ],
    )
    .expect("primitive nullable batch")
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
            Arc::new(BooleanArray::from(vec![Some(true), None, Some(false)])) as ArrayRef,
            Arc::new(Int32Array::from(vec![Some(7), None, Some(-1)])) as ArrayRef,
            Arc::new(Int64Array::from(vec![Some(70), None, Some(-10)])) as ArrayRef,
            Arc::new(Float32Array::from(vec![Some(0.25), None, Some(-1.5)])) as ArrayRef,
            Arc::new(Float64Array::from(vec![Some(1.5), None, Some(-2.25)])) as ArrayRef,
        ],
    )
    .expect("full primitive nullable batch")
}

fn utf8_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![Field::new(
        "name",
        DataType::Utf8,
        true,
    )]));
    RecordBatch::try_new(
        schema,
        vec![Arc::new(StringArray::from(vec![Some("alpha"), None, Some("beta")])) as ArrayRef],
    )
    .expect("utf8 batch")
}

fn date32_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![Field::new(
        "day",
        DataType::Date32,
        true,
    )]));
    RecordBatch::try_new(
        schema,
        vec![Arc::new(Date32Array::from(vec![Some(1), None, Some(3)])) as ArrayRef],
    )
    .expect("date32 batch")
}

fn struct_batch() -> RecordBatch {
    let child = Arc::new(Field::new("id", DataType::Int32, true));
    let struct_array = StructArray::from(vec![(
        child,
        Arc::new(Int32Array::from(vec![Some(1), None, Some(3)])) as ArrayRef,
    )]);
    let schema = Arc::new(Schema::new(vec![Field::new(
        "record",
        struct_array.data_type().clone(),
        true,
    )]));
    RecordBatch::try_new(schema, vec![Arc::new(struct_array) as ArrayRef]).expect("struct batch")
}

fn encode_lmc2(batch: &RecordBatch) -> Vec<u8> {
    let payload = ArrowSemanticPayload::from_record_batches(&[batch.clone()]).expect("payload");
    encode_arrow_semantic_container_payload(&payload).expect("encode LMC2")
}

fn encode_lma1(batch: &RecordBatch) -> Vec<u8> {
    let payload = ArrowSemanticPayload::from_record_batches(&[batch.clone()]).expect("payload");
    encode_arrow_semantic_payload(&payload).expect("encode LMA1")
}
