use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow_array::{
    Array, ArrayRef, Float32Array, Float64Array, Int32Array, Int64Array, RecordBatch, StringArray,
    StructArray,
};
use arrow_schema::{DataType, Field, Schema};
use loom_core::arrow_semantic_codec::{
    decode_arrow_semantic_container_payload, is_arrow_semantic_container,
};
use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_parquet_ingress::{
    emit_source_ingress_lmc2_from_parquet_path, parquet_arrow_oracle_batches_from_path,
};
use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceDiagnosticCode, SourceEmissionDisposition,
    SourceEmissionKind, SourceIngressStatus, SourceLoweringDisposition, SourceOracleStrategy,
};
use parquet::arrow::ArrowWriter;
use tempfile::TempDir;

fn write_record_batch(path: &Path, batch: RecordBatch) {
    let file = File::create(path).expect("create parquet file");
    let mut writer =
        ArrowWriter::try_new(file, batch.schema(), None).expect("create parquet writer");
    writer.write(&batch).expect("write parquet batch");
    writer.close().expect("close parquet writer");
}

fn parquet_path_for_batch(temp: &TempDir, name: &str, batch: RecordBatch) -> std::path::PathBuf {
    let path = temp.path().join(format!("{name}.parquet"));
    write_record_batch(&path, batch);
    path
}

fn single_i32_path(temp: &TempDir) -> std::path::PathBuf {
    let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int32, false)]));
    let batch = RecordBatch::try_new(schema, vec![Arc::new(Int32Array::from(vec![7, -1, 42]))])
        .expect("record batch");
    parquet_path_for_batch(temp, "single-i32", batch)
}

fn primitive_table_path(temp: &TempDir) -> std::path::PathBuf {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int32, false),
        Field::new("score", DataType::Int64, false),
        Field::new("ratio32", DataType::Float32, false),
        Field::new("ratio64", DataType::Float64, false),
    ]));
    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(Int32Array::from(vec![1, 2, 3])),
            Arc::new(Int64Array::from(vec![10, 20, 30])),
            Arc::new(Float32Array::from(vec![1.25, -2.5, 3.75])),
            Arc::new(Float64Array::from(vec![1.5, 2.5, 3.5])),
        ],
    )
    .expect("record batch");
    parquet_path_for_batch(temp, "primitive-table", batch)
}

fn assert_emitted_artifact_is_verifier_accepted(bytes: &[u8]) {
    assert!(is_arrow_semantic_container(bytes));
    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_artifact(bytes, &registry, &Default::default());
    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
    let facts = report.facts().expect("accepted LMC2 facts");
    assert_eq!(facts.artifact_kind, "LMC2");
    assert_eq!(
        facts.payload_kind.as_deref(),
        Some("Arrow semantic payload")
    );
}

fn assert_arrow_oracle_batch(path: &Path, expected_schema: &[(&str, DataType)]) {
    let batches = parquet_arrow_oracle_batches_from_path(path).expect("Arrow oracle batches");
    assert_eq!(batches.len(), 1);
    let batch = &batches[0];
    assert_eq!(batch.num_rows(), 3);
    assert_eq!(batch.num_columns(), expected_schema.len());
    let schema = batch.schema();
    for (index, (name, data_type)) in expected_schema.iter().enumerate() {
        let field = schema.field(index);
        assert_eq!(field.name(), name);
        assert_eq!(field.data_type(), data_type);
        assert!(!field.is_nullable());
        assert_eq!(batch.column(index).null_count(), 0);
    }
}

fn assert_lmc2_matches_arrow_oracle(path: &Path, bytes: &[u8]) {
    let source = parquet_arrow_oracle_batches_from_path(path).expect("Arrow oracle batches");
    let decoded = decode_arrow_semantic_container_payload(bytes)
        .expect("decode LMC2")
        .to_record_batches()
        .expect("LMC2 record batches");
    assert_eq!(decoded, source);
}

#[test]
fn accepted_single_column_handoff_is_verifier_routed_lmc2() {
    let temp = TempDir::new().expect("tempdir");
    let path = single_i32_path(&temp);
    let accepted =
        emit_source_ingress_lmc2_from_parquet_path(&path).expect("accepted Parquet handoff");

    assert!(!accepted.bytes.is_empty());
    assert!(accepted.bytes.starts_with(b"LMC2"));
    assert_emitted_artifact_is_verifier_accepted(&accepted.bytes);
    assert_eq!(accepted.report.status, SourceIngressStatus::Accepted);
    assert_eq!(
        accepted.report.emission_kind,
        SourceEmissionKind::ArrowSemantic
    );
    assert_eq!(
        accepted.report.emission_disposition,
        SourceEmissionDisposition::SemanticArrow
    );
    assert_eq!(
        accepted.report.lowering_disposition,
        SourceLoweringDisposition::InterpreterOnly
    );
    assert!(accepted.report.artifact_verification.required);
    assert!(accepted.report.artifact_verification.accepted);
    assert_eq!(
        accepted.report.artifact_verification.artifact_byte_len,
        Some(accepted.bytes.len())
    );
    assert!(accepted
        .report
        .artifact_verification
        .summary
        .contains("LMC2"));
    assert!(accepted
        .report
        .artifact_verification
        .summary
        .contains("Arrow semantic payload"));
    assert_lmc2_matches_arrow_oracle(&path, &accepted.bytes);
    assert_arrow_oracle_batch(&path, &[("id", DataType::Int32)]);
}

#[test]
fn accepted_table_handoff_is_verifier_routed_lmc2_and_arrow_equivalent() {
    let temp = TempDir::new().expect("tempdir");
    let path = primitive_table_path(&temp);
    let accepted =
        emit_source_ingress_lmc2_from_parquet_path(&path).expect("accepted Parquet handoff");

    assert!(!accepted.bytes.is_empty());
    assert!(accepted.bytes.starts_with(b"LMC2"));
    assert_emitted_artifact_is_verifier_accepted(&accepted.bytes);
    assert_eq!(accepted.report.status, SourceIngressStatus::Accepted);
    assert_eq!(
        accepted.report.emission_kind,
        SourceEmissionKind::ArrowSemantic
    );
    assert_eq!(
        accepted.report.emission_disposition,
        SourceEmissionDisposition::SemanticArrow
    );
    assert_lmc2_matches_arrow_oracle(&path, &accepted.bytes);
    assert_arrow_oracle_batch(
        &path,
        &[
            ("id", DataType::Int32),
            ("score", DataType::Int64),
            ("ratio32", DataType::Float32),
            ("ratio64", DataType::Float64),
        ],
    );
}

#[test]
fn accepted_handoff_records_arrow_scan_oracle_evidence() {
    let temp = TempDir::new().expect("tempdir");
    let path = primitive_table_path(&temp);
    let accepted =
        emit_source_ingress_lmc2_from_parquet_path(&path).expect("accepted Parquet handoff");

    let oracle = accepted
        .report
        .oracle_evidence
        .as_ref()
        .expect("Arrow oracle evidence");
    assert_eq!(oracle.strategy, SourceOracleStrategy::ArrowScan);
    assert!(oracle.accepted);
    assert_eq!(oracle.row_count_checked, Some(3));
    assert!(oracle.nulls_checked);
    assert!(!oracle.source_native_scan_used);
    assert!(oracle
        .notes
        .iter()
        .any(|note| note.contains("evidence only")));
}

#[test]
fn nullable_extension_nested_and_string_paths_emit_semantic_lmc2() {
    let temp = TempDir::new().expect("tempdir");
    let nullable_schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int32, true)]));
    let nullable = RecordBatch::try_new(
        nullable_schema,
        vec![Arc::new(Int32Array::from(vec![Some(1), None, Some(3)]))],
    )
    .expect("nullable batch");
    let nullable_path = parquet_path_for_batch(&temp, "nullable", nullable);

    let accepted = emit_source_ingress_lmc2_from_parquet_path(&nullable_path)
        .expect("nullable Parquet emits LMC2");
    assert_eq!(accepted.report.status, SourceIngressStatus::Accepted);
    assert_lmc2_matches_arrow_oracle(&nullable_path, &accepted.bytes);

    let extension_field = Field::new("ext_i32", DataType::Int32, false).with_metadata(
        [(
            "ARROW:extension:name".to_string(),
            "loom.test.extension".to_string(),
        )]
        .into_iter()
        .collect(),
    );
    let extension_schema = Arc::new(Schema::new(vec![extension_field]));
    let extension = RecordBatch::try_new(
        extension_schema,
        vec![Arc::new(Int32Array::from(vec![1, 2, 3]))],
    )
    .expect("extension batch");
    let extension_path = parquet_path_for_batch(&temp, "extension", extension);
    let accepted = emit_source_ingress_lmc2_from_parquet_path(&extension_path)
        .expect("extension Parquet emits LMC2");
    assert_lmc2_matches_arrow_oracle(&extension_path, &accepted.bytes);

    let nested_field = Arc::new(Field::new("nested_id", DataType::Int32, false));
    let nested_array: ArrayRef = Arc::new(StructArray::from(vec![(
        nested_field.clone(),
        Arc::new(Int32Array::from(vec![1, 2, 3])) as ArrayRef,
    )]));
    let nested_schema = Arc::new(Schema::new(vec![Field::new(
        "nested",
        DataType::Struct(vec![nested_field].into()),
        false,
    )]));
    let nested = RecordBatch::try_new(nested_schema, vec![nested_array]).expect("nested batch");
    let nested_path = parquet_path_for_batch(&temp, "nested", nested);
    let accepted = emit_source_ingress_lmc2_from_parquet_path(&nested_path)
        .expect("nested Parquet emits LMC2");
    assert_lmc2_matches_arrow_oracle(&nested_path, &accepted.bytes);

    let string_schema = Arc::new(Schema::new(vec![Field::new("name", DataType::Utf8, false)]));
    let string = RecordBatch::try_new(
        string_schema,
        vec![Arc::new(StringArray::from(vec!["a", "b", "c"]))],
    )
    .expect("string batch");
    let string_path = parquet_path_for_batch(&temp, "string", string);
    let accepted = emit_source_ingress_lmc2_from_parquet_path(&string_path)
        .expect("string Parquet emits LMC2");
    assert_lmc2_matches_arrow_oracle(&string_path, &accepted.bytes);
}

#[test]
fn malformed_path_returns_rejected_report_without_artifact_bytes() {
    let temp = TempDir::new().expect("tempdir");
    let malformed = temp.path().join("malformed.parquet");
    std::fs::write(&malformed, b"not a parquet file").expect("write malformed bytes");
    let report = emit_source_ingress_lmc2_from_parquet_path(&malformed)
        .expect_err("malformed Parquet is rejected");
    assert_eq!(report.status, SourceIngressStatus::Rejected);
    assert!(report.facts.is_none());
    assert_eq!(report.diagnostics[0].code, SourceDiagnosticCode::ReadFailed);
    assert_eq!(
        report.artifact_verification,
        SourceArtifactVerificationSummary::not_applicable()
    );
    assert!(report.oracle_evidence.is_none());
}
