use std::sync::Arc;

use arrow_array::types::Int32Type;
use arrow_array::{Array, ArrayRef, BooleanArray, Int32Array, ListArray, StringArray, StructArray};
use arrow_schema::{DataType, Field, Schema};
use loom_core::arrow_semantic::{ArrowSemanticBatch, ArrowSemanticPayload};
use loom_core::arrow_semantic_codec::{
    decode_arrow_semantic_payload, encode_arrow_semantic_payload, is_arrow_semantic_container,
    is_arrow_semantic_payload,
};
use loom_core::arrow_semantic_verifier::{
    verify_arrow_semantic_batch, verify_arrow_semantic_payload, ArrowSemanticVerificationStatus,
};

#[test]
fn arrow_semantic_markers_are_stable() {
    assert!(is_arrow_semantic_payload(b"LMA1\x01"));
    assert!(is_arrow_semantic_container(b"LMC2\x01"));
    assert!(!is_arrow_semantic_payload(b"LMP1\x01"));
    assert!(!is_arrow_semantic_container(b"LMC1\x01"));
}

#[test]
fn arrow_semantic_batch_accepts_matching_schema_and_columns() {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int32, true),
        Field::new("name", DataType::Utf8, true),
    ]));
    let ids = Int32Array::from(vec![Some(1), None, Some(3)]).into_data();
    let names = StringArray::from(vec![Some("alpha"), None, Some("beta")]).into_data();

    let batch = ArrowSemanticBatch::try_new(schema.clone(), vec![ids, names])
        .expect("matching Arrow semantic batch");
    assert_eq!(batch.row_count(), 3);

    let report = verify_arrow_semantic_batch(&batch);
    assert_eq!(report.status(), ArrowSemanticVerificationStatus::Accepted);
    assert!(report.is_ok());

    let payload =
        ArrowSemanticPayload::try_new(schema, vec![batch]).expect("matching payload schema");
    let report = verify_arrow_semantic_payload(&payload);
    assert!(report.is_ok());
    assert_eq!(payload.row_count(), 3);
}

#[test]
fn arrow_semantic_payload_roundtrips_nullable_scalars_and_utf8() {
    let schema = Arc::new(Schema::new(vec![
        Field::new("ok", DataType::Boolean, true),
        Field::new("id", DataType::Int32, true),
        Field::new("name", DataType::Utf8, true),
    ]));
    let batch = arrow_array::RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(BooleanArray::from(vec![Some(true), None, Some(false)])) as ArrayRef,
            Arc::new(Int32Array::from(vec![Some(1), None, Some(3)])) as ArrayRef,
            Arc::new(StringArray::from(vec![Some("alpha"), None, Some("beta")])) as ArrayRef,
        ],
    )
    .expect("record batch");
    let payload = ArrowSemanticPayload::from_record_batches(&[batch]).expect("payload");

    let bytes = encode_arrow_semantic_payload(&payload).expect("encode LMA1");
    assert!(is_arrow_semantic_payload(&bytes));

    let decoded = decode_arrow_semantic_payload(&bytes).expect("decode LMA1");
    assert_eq!(decoded.schema().as_ref(), schema.as_ref());
    assert_eq!(decoded.row_count(), 3);
    assert_eq!(decoded.batches().len(), 1);
    assert!(verify_arrow_semantic_payload(&decoded).is_ok());
}

#[test]
fn arrow_semantic_payload_roundtrips_nested_list_and_struct() {
    let list = ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
        Some(vec![Some(1), Some(2)]),
        None,
        Some(vec![Some(3), None]),
    ]);
    let struct_array = StructArray::from(vec![
        (
            Arc::new(Field::new("child_id", DataType::Int32, true)),
            Arc::new(Int32Array::from(vec![Some(10), None, Some(30)])) as ArrayRef,
        ),
        (
            Arc::new(Field::new("child_name", DataType::Utf8, true)),
            Arc::new(StringArray::from(vec![Some("x"), None, Some("z")])) as ArrayRef,
        ),
    ]);
    let schema = Arc::new(Schema::new(vec![
        Field::new("items", list.data_type().clone(), true),
        Field::new("record", struct_array.data_type().clone(), true),
    ]));
    let batch = arrow_array::RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(list) as ArrayRef,
            Arc::new(struct_array) as ArrayRef,
        ],
    )
    .expect("nested batch");
    let payload = ArrowSemanticPayload::from_record_batches(&[batch]).expect("payload");

    let bytes = encode_arrow_semantic_payload(&payload).expect("encode nested LMA1");
    let decoded = decode_arrow_semantic_payload(&bytes).expect("decode nested LMA1");
    assert_eq!(decoded.schema().as_ref(), schema.as_ref());
    assert_eq!(decoded.row_count(), 3);
    assert!(verify_arrow_semantic_payload(&decoded).is_ok());
}

#[test]
fn arrow_semantic_batch_rejects_field_column_mismatch() {
    let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int32, false)]));
    let ids = Int32Array::from(vec![1, 2, 3]).into_data();
    let names = StringArray::from(vec!["a", "b", "c"]).into_data();

    let err = ArrowSemanticBatch::try_new(schema, vec![ids, names])
        .expect_err("field/column mismatch should fail");
    assert!(err.to_string().contains("field/column count mismatch"));
}

#[test]
fn arrow_semantic_batch_rejects_row_count_mismatch() {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int32, false),
        Field::new("name", DataType::Utf8, false),
    ]));
    let ids = Int32Array::from(vec![1, 2, 3]).into_data();
    let names = StringArray::from(vec!["a", "b"]).into_data();

    let err = ArrowSemanticBatch::try_new(schema, vec![ids, names])
        .expect_err("row count mismatch should fail");
    assert!(err.to_string().contains("row count mismatch"));
}

#[test]
fn arrow_semantic_core_manifest_has_no_source_reader_dependencies() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = std::fs::read_to_string(manifest_path).expect("read loom-core manifest");

    let dependency_lines = manifest
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    for forbidden in ["vortex-", "vortex_", "lance", "parquet"] {
        assert!(
            !dependency_lines.contains(forbidden),
            "loom-core manifest must not contain source reader dependency marker {forbidden:?}"
        );
    }
}
