use std::sync::Arc;

use arrow_array::{Array, Int32Array, StringArray};
use arrow_schema::{DataType, Field, Schema};
use loom_core::arrow_semantic::{ArrowSemanticBatch, ArrowSemanticPayload};
use loom_core::arrow_semantic_codec::{is_arrow_semantic_container, is_arrow_semantic_payload};
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
