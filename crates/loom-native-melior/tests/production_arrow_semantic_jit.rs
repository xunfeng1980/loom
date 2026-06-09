use std::sync::Arc;

use arrow_array::{
    ArrayRef, BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array, RecordBatch,
};
use arrow_schema::{DataType, Field, Schema};
use loom_core::arrow_semantic::ArrowSemanticPayload;
use loom_core::arrow_semantic_codec::encode_arrow_semantic_container_payload;
use loom_core::native_arrow_semantic::prepare_native_arrow_semantic_codegen_support;
#[cfg(feature = "melior")]
use loom_core::native_arrow_semantic::{
    decide_validated_native_arrow_semantic_codegen_runtime,
    validate_native_arrow_semantic_codegen_output,
    validated_native_arrow_semantic_codegen_runtime_cache_key,
};
#[cfg(feature = "melior")]
use loom_core::runtime_abi::{ProjectionSet, RuntimeExecutionDecision, RuntimeSafetyPolicy};
use loom_native_melior::backend::NativeBackendCancellation;
#[cfg(not(feature = "melior"))]
use loom_native_melior::backend::NativeBackendDiagnosticCode;
use loom_native_melior::jit::execute_arrow_semantic_codegen_jit;
#[cfg(feature = "melior")]
use loom_native_melior::jit::ARROW_SEMANTIC_CODEGEN_JIT_ENTRY_SYMBOL;

#[cfg(feature = "melior")]
#[test]
fn arrow_semantic_jit_produces_validated_phase35_record_batch() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let support = prepare_native_arrow_semantic_codegen_support(&bytes);
    assert!(support.is_supported(), "{support:?}");

    let jit = execute_arrow_semantic_codegen_jit(&support, &NativeBackendCancellation::default())
        .expect("melior ExecutionEngine should produce Arrow semantic buffers");
    assert_eq!(jit.entry_symbol, ARROW_SEMANTIC_CODEGEN_JIT_ENTRY_SYMBOL);
    assert_eq!(jit.row_count, 9);
    assert_eq!(jit.column_count, 5);
    assert_eq!(jit.columns.len(), 5);
    for (expected, output) in support.columns().iter().zip(jit.columns.iter()) {
        assert_eq!(output.index, expected.index);
        assert_eq!(output.value_buffer, expected.value_buffer);
        assert_eq!(output.validity_buffer, expected.validity_buffer);
    }

    let execution = validate_native_arrow_semantic_codegen_output(
        &bytes,
        &support,
        jit.backend_identity,
        jit.columns,
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
    .expect("validated production codegen cache key");
    assert!(key.canonical_input.contains("phase43.1-production-codegen"));
    assert!(key
        .canonical_input
        .contains("validation=native-model:phase40"));
}

#[cfg(not(feature = "melior"))]
#[test]
fn arrow_semantic_jit_requires_melior_feature() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let support = prepare_native_arrow_semantic_codegen_support(&bytes);
    let err = execute_arrow_semantic_codegen_jit(&support, &NativeBackendCancellation::default())
        .expect_err("default build must not pretend production JIT succeeded");
    assert_eq!(err.code, NativeBackendDiagnosticCode::JitUnavailable);
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

fn encode_lmc2(batch: &RecordBatch) -> Vec<u8> {
    let payload = ArrowSemanticPayload::from_record_batches(&[batch.clone()]).expect("payload");
    encode_arrow_semantic_container_payload(&payload).expect("encode LMC2")
}
