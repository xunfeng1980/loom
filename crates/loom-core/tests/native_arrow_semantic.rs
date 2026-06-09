use std::sync::Arc;

use arrow_array::{
    Array, ArrayRef, BooleanArray, Date32Array, Float64Array, Int32Array, Int64Array, RecordBatch,
    StringArray, StructArray,
};
use arrow_schema::{DataType, Field, Schema};
use loom_core::arrow_semantic::ArrowSemanticPayload;
use loom_core::arrow_semantic_codec::{
    encode_arrow_semantic_container_payload, encode_arrow_semantic_payload,
};
use loom_core::native_arrow_semantic::{
    execute_native_arrow_semantic, verify_native_arrow_semantic_equivalence,
    verify_native_arrow_semantic_output_equivalence, NativeArrowSemanticDiagnosticCode,
    NATIVE_ARROW_SEMANTIC_BACKEND,
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
