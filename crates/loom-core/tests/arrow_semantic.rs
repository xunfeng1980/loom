use std::sync::Arc;

use arrow_array::types::Int32Type;
use arrow_array::{Array, ArrayRef, BooleanArray, Int32Array, ListArray, StringArray, StructArray};
use arrow_schema::{DataType, Field, Schema};
use loom_core::arrow_semantic::{ArrowSemanticBatch, ArrowSemanticPayload};
use loom_core::arrow_semantic_codec::{
    decode_arrow_semantic_container_payload, decode_arrow_semantic_payload,
    encode_arrow_semantic_container_payload, encode_arrow_semantic_payload,
    is_arrow_semantic_container, is_arrow_semantic_payload, unwrap_arrow_semantic_payload,
    wrap_arrow_semantic_payload,
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
fn arrow_semantic_container_wraps_lma1_payload() {
    let (schema, payload) = nullable_scalars_payload();

    let lma1_bytes = encode_arrow_semantic_payload(&payload).expect("encode direct LMA1");
    assert!(is_arrow_semantic_payload(&lma1_bytes));

    let wrapped = wrap_arrow_semantic_payload(&lma1_bytes).expect("wrap LMA1 in LMC2");
    assert!(is_arrow_semantic_container(&wrapped));
    assert!(!is_arrow_semantic_payload(&wrapped));

    let unwrapped = unwrap_arrow_semantic_payload(&wrapped).expect("unwrap LMC2");
    assert_eq!(unwrapped, lma1_bytes);

    let decoded = decode_arrow_semantic_container_payload(&wrapped).expect("decode LMC2");
    assert_eq!(decoded.schema().as_ref(), schema.as_ref());
    assert_eq!(decoded.row_count(), 3);
    assert_eq!(decoded.batches().len(), 1);
    assert!(verify_arrow_semantic_payload(&decoded).is_ok());

    let wrapped_from_payload =
        encode_arrow_semantic_container_payload(&payload).expect("encode LMC2 from payload");
    let decoded_from_payload =
        decode_arrow_semantic_container_payload(&wrapped_from_payload).expect("decode LMC2");
    assert_eq!(decoded_from_payload.schema().as_ref(), schema.as_ref());
    assert_eq!(decoded_from_payload.row_count(), 3);

    let (nested_schema, nested) = nested_payload();
    let nested_wrapped = encode_arrow_semantic_container_payload(&nested).expect("nested LMC2");
    let nested_decoded =
        decode_arrow_semantic_container_payload(&nested_wrapped).expect("decode nested LMC2");
    assert_eq!(nested_decoded.schema().as_ref(), nested_schema.as_ref());
    assert_eq!(nested_decoded.row_count(), 3);
}

#[test]
fn arrow_semantic_container_rejects_malformed_wrappers() {
    let (_, payload) = nullable_scalars_payload();
    let valid = encode_arrow_semantic_container_payload(&payload).expect("valid LMC2");

    let err = unwrap_arrow_semantic_payload(b"BAD!\x01").expect_err("wrong magic");
    assert!(err.to_string().contains("wrong magic"));

    let mut unsupported_version = valid.clone();
    unsupported_version[4..6].copy_from_slice(&2u16.to_le_bytes());
    let err = unwrap_arrow_semantic_payload(&unsupported_version).expect_err("bad version");
    assert!(err.to_string().contains("unsupported version"));

    let mut unknown_feature = valid.clone();
    unknown_feature[8..16].copy_from_slice(&(1u64 << 9).to_le_bytes());
    let err = unwrap_arrow_semantic_payload(&unknown_feature).expect_err("unknown feature");
    assert!(err.to_string().contains("unknown required feature"));

    let mut missing_payload = valid.clone();
    missing_payload[28..30].copy_from_slice(&2u16.to_le_bytes());
    missing_payload[30..32].copy_from_slice(&0u16.to_le_bytes());
    let err = unwrap_arrow_semantic_payload(&missing_payload).expect_err("missing payload");
    assert!(err.to_string().contains("missing arrow semantic payload"));

    let mut malformed_inner = valid.clone();
    let payload_offset = usize::try_from(u64::from_le_bytes(
        malformed_inner[32..40]
            .try_into()
            .expect("payload offset bytes"),
    ))
    .expect("payload offset fits");
    malformed_inner[payload_offset..payload_offset + 4].copy_from_slice(b"NOPE");
    let err = unwrap_arrow_semantic_payload(&malformed_inner).expect_err("bad inner payload");
    assert!(err.to_string().contains("malformed inner LMA1 payload"));

    let mut trailing = valid.clone();
    trailing.extend_from_slice(b"extra");
    let err = unwrap_arrow_semantic_payload(&trailing).expect_err("trailing bytes");
    assert!(err.to_string().contains("trailing section bytes"));

    let truncated = &valid[..12];
    let err = unwrap_arrow_semantic_payload(truncated).expect_err("truncated");
    assert!(err.to_string().contains("truncated"));

    let mut outside = valid.clone();
    outside[40..48].copy_from_slice(&(u64::MAX).to_le_bytes());
    let err = unwrap_arrow_semantic_payload(&outside).expect_err("section outside container");
    assert!(
        err.to_string().contains("section offset overflow")
            || err.to_string().contains("section outside container")
    );
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

fn nullable_scalars_payload() -> (Arc<Schema>, ArrowSemanticPayload) {
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
    (schema, payload)
}

fn nested_payload() -> (Arc<Schema>, ArrowSemanticPayload) {
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
    (schema, payload)
}
