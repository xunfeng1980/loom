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
    decide_validated_native_arrow_semantic_codegen_runtime,
    prepare_native_arrow_semantic_codegen_support, validate_native_arrow_semantic_codegen_output,
    validated_native_arrow_semantic_codegen_runtime_cache_key,
    NativeArrowSemanticCodegenBufferKind, NativeArrowSemanticCodegenOutputColumn,
    NativeArrowSemanticDiagnosticCode, PRODUCTION_NATIVE_ARROW_SEMANTIC_CODEGEN_BACKEND,
};
use loom_core::runtime_abi::{
    ProjectionSet, RuntimeExecutionDecision, RuntimeFallbackPolicy, RuntimeSafetyPolicy,
};

#[test]
fn lmc2_arrow_semantic_codegen_support_extracts_real_phase35_buffers() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let report = prepare_native_arrow_semantic_codegen_support(&bytes);

    assert!(
        report.is_supported(),
        "unexpected diagnostics: {:?}",
        report.diagnostics()
    );
    assert_eq!(
        report.backend,
        PRODUCTION_NATIVE_ARROW_SEMANTIC_CODEGEN_BACKEND
    );
    assert_eq!(report.artifact_kind, "LMC2");
    assert_eq!(report.payload_kind, "Arrow semantic payload");
    assert_eq!(report.row_count, 9);
    assert_eq!(report.column_count, 5);
    assert!(!report.schema_fingerprint.is_empty());

    let columns = report.columns();
    assert_eq!(columns.len(), 5);
    assert_eq!(columns[0].name, "ok");
    assert_eq!(columns[0].data_type, DataType::Boolean);
    assert_eq!(
        columns[0].value_buffer_kind,
        NativeArrowSemanticCodegenBufferKind::BooleanValueBitmap
    );
    assert_eq!(columns[0].value_buffer_bytes(), 2);
    assert_eq!(columns[0].validity_buffer_bytes(), 2);
    assert!(columns[0].value_buffer.iter().any(|byte| *byte != 0));
    assert!(columns[0]
        .validity_buffer
        .as_ref()
        .expect("validity")
        .iter()
        .any(|byte| *byte != 0));

    for (idx, expected_width) in [(1, 4), (2, 8), (3, 4), (4, 8)] {
        let column = &columns[idx];
        assert_eq!(
            column.value_buffer_kind,
            NativeArrowSemanticCodegenBufferKind::FixedWidthValue
        );
        assert_eq!(column.value_buffer_bytes(), 9 * expected_width);
        assert_eq!(column.validity_buffer_bytes(), 2);
        assert_eq!(column.null_count, 2);
        assert!(column.value_buffer.iter().any(|byte| *byte != 0));
    }
}

#[test]
fn direct_lma1_codegen_support_remains_explicit_bridge_input() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lma1(&batch);
    let report = prepare_native_arrow_semantic_codegen_support(&bytes);

    assert!(report.is_supported(), "{report:?}");
    assert_eq!(report.artifact_kind, "LMA1");
    assert_eq!(report.row_count, 9);
    assert_eq!(report.columns().len(), 5);
}

#[test]
fn codegen_support_rejects_unsupported_arrow_semantic_shapes() {
    for (bytes, expected_path) in [
        (encode_lmc2(&utf8_batch()), "$.schema.fields[0].type"),
        (encode_lmc2(&date32_batch()), "$.schema.fields[0].type"),
        (encode_lmc2(&struct_batch()), "$.schema.fields[0].type"),
    ] {
        let report = prepare_native_arrow_semantic_codegen_support(&bytes);
        assert!(!report.is_supported());
        assert!(report.columns().is_empty());
        let diagnostic = report.first_error().expect("diagnostic");
        assert_eq!(
            diagnostic.code,
            NativeArrowSemanticDiagnosticCode::UnsupportedType
        );
        assert_eq!(diagnostic.path, expected_path);
    }
}

#[test]
fn codegen_support_rejects_multi_batch_payloads_before_buffers() {
    let batch = full_primitive_nullable_batch();
    let payload =
        ArrowSemanticPayload::from_record_batches(&[batch.clone(), batch]).expect("multi batch");
    let bytes = encode_arrow_semantic_container_payload(&payload).expect("encode LMC2");
    let report = prepare_native_arrow_semantic_codegen_support(&bytes);

    assert!(!report.is_supported());
    assert!(report.columns().is_empty());
    assert_eq!(
        report.first_error().expect("diagnostic").code,
        NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape
    );
}

#[test]
fn validated_codegen_output_is_native_candidate_and_cacheable() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let support = prepare_native_arrow_semantic_codegen_support(&bytes);
    assert!(support.is_supported(), "{support:?}");

    let execution = validate_native_arrow_semantic_codegen_output(
        &bytes,
        &support,
        "melior-jit:test-pipeline",
        mirrored_output_columns(&support),
    );
    assert!(
        execution.is_supported(),
        "unexpected diagnostics: {:?}",
        execution.diagnostics()
    );
    assert_eq!(execution.output().expect("output"), &batch);
    assert!(execution.validation().expect("validation").is_validated());

    let decision = decide_validated_native_arrow_semantic_codegen_runtime(
        &execution,
        RuntimeSafetyPolicy::default(),
    );
    assert_eq!(decision.decision, RuntimeExecutionDecision::NativeCandidate);

    let key = validated_native_arrow_semantic_codegen_runtime_cache_key(
        &bytes,
        &execution,
        ProjectionSet::All,
        RuntimeSafetyPolicy::default(),
    )
    .expect("validated codegen cache key");
    assert!(key.canonical_input.contains(
        "backend=loom-production-native-arrow-semantic-codegen:phase43.1-production-codegen"
    ));
    assert!(key
        .canonical_input
        .contains("validation=native-model:phase40"));
}

#[test]
fn divergent_codegen_output_fails_closed_and_is_not_cacheable() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let support = prepare_native_arrow_semantic_codegen_support(&bytes);
    let mut outputs = mirrored_output_columns(&support);
    outputs[1].value_buffer[0] ^= 0x7f;

    let execution = validate_native_arrow_semantic_codegen_output(
        &bytes,
        &support,
        "melior-jit:test-pipeline",
        outputs,
    );
    assert!(!execution.is_supported());
    assert!(execution.diagnostics().iter().any(
        |diagnostic| diagnostic.code == NativeArrowSemanticDiagnosticCode::NativeOutputMismatch
    ));

    let strict = decide_validated_native_arrow_semantic_codegen_runtime(
        &execution,
        RuntimeSafetyPolicy::default(),
    );
    assert_eq!(strict.decision, RuntimeExecutionDecision::FailClosed);

    let mut fallback = RuntimeSafetyPolicy::default();
    fallback.fallback = RuntimeFallbackPolicy::AllowInterpreter;
    let fallback_decision =
        decide_validated_native_arrow_semantic_codegen_runtime(&execution, fallback);
    assert_eq!(
        fallback_decision.decision,
        RuntimeExecutionDecision::InterpreterFallback
    );

    let err = validated_native_arrow_semantic_codegen_runtime_cache_key(
        &bytes,
        &execution,
        ProjectionSet::All,
        RuntimeSafetyPolicy::default(),
    )
    .expect_err("divergent output must not be cacheable");
    assert_eq!(err.path, "$.cache.native_arrow_semantic_codegen");
}

fn mirrored_output_columns(
    support: &loom_core::native_arrow_semantic::NativeArrowSemanticCodegenSupportReport,
) -> Vec<NativeArrowSemanticCodegenOutputColumn> {
    support
        .columns()
        .iter()
        .map(|column| NativeArrowSemanticCodegenOutputColumn {
            index: column.index,
            value_buffer: column.value_buffer.clone(),
            validity_buffer: column.validity_buffer.clone(),
        })
        .collect()
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

fn date32_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![Field::new("day", DataType::Date32, true)]));
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
