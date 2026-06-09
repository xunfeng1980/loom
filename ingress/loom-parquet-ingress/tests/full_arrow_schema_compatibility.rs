use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow_array::types::Int32Type;
use arrow_array::{
    Array, ArrayRef, BooleanArray, Int32Array, ListArray, RecordBatch, StringArray, StructArray,
};
use arrow_schema::{DataType, Field, Schema};
use loom_core::arrow_semantic_codec::decode_arrow_semantic_container_payload;
use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_parquet_ingress::{
    emit_source_ingress_lmc2_from_parquet_path, parquet_arrow_oracle_batches_from_path,
};
use loom_source_ingress::{
    SourceEmissionDisposition, SourceEmissionKind, SourceIngressStatus, SourceLoweringDisposition,
};
use parquet::arrow::ArrowWriter;
use tempfile::TempDir;

fn write_parquet(path: &Path, batch: RecordBatch) {
    let file = File::create(path).expect("create parquet file");
    let mut writer =
        ArrowWriter::try_new(file, batch.schema(), None).expect("create parquet writer");
    writer.write(&batch).expect("write parquet batch");
    writer.close().expect("close parquet writer");
}

fn semantic_case_path(temp: &TempDir, name: &str, batch: RecordBatch) -> std::path::PathBuf {
    let path = temp.path().join(format!("{name}.parquet"));
    write_parquet(&path, batch);
    path
}

fn assert_parquet_lmc2_roundtrip(path: &Path) {
    let accepted = emit_source_ingress_lmc2_from_parquet_path(path)
        .expect("accepted Parquet semantic handoff");
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

    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_artifact(&accepted.bytes, &registry, &Default::default());
    assert_eq!(verification.status(), ArtifactVerificationStatus::Accepted);
    let facts = verification.facts().expect("LMC2 verifier facts");
    assert_eq!(facts.artifact_kind, "LMC2");
    assert_eq!(
        facts.payload_kind.as_deref(),
        Some("Arrow semantic payload")
    );

    let source = parquet_arrow_oracle_batches_from_path(path).expect("Parquet Arrow source");
    let decoded = decode_arrow_semantic_container_payload(&accepted.bytes)
        .expect("decode LMC2")
        .to_record_batches()
        .expect("LMC2 batches");
    assert_eq!(decoded, source);
}

#[test]
fn parquet_nullable_scalar_bool_utf8_struct_and_list_emit_lmc2() {
    let temp = TempDir::new().expect("tempdir");

    let nullable_scalars = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("ok", DataType::Boolean, true),
            Field::new("id", DataType::Int32, true),
            Field::new("name", DataType::Utf8, true),
        ])),
        vec![
            Arc::new(BooleanArray::from(vec![Some(true), None, Some(false)])) as ArrayRef,
            Arc::new(Int32Array::from(vec![Some(1), None, Some(3)])) as ArrayRef,
            Arc::new(StringArray::from(vec![Some("alpha"), None, Some("beta")])) as ArrayRef,
        ],
    )
    .expect("nullable scalar batch");
    assert_parquet_lmc2_roundtrip(&semantic_case_path(
        &temp,
        "nullable-scalars",
        nullable_scalars,
    ));

    let nested_list = ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
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
    let nested = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("items", nested_list.data_type().clone(), true),
            Field::new("record", struct_array.data_type().clone(), true),
        ])),
        vec![
            Arc::new(nested_list) as ArrayRef,
            Arc::new(struct_array) as ArrayRef,
        ],
    )
    .expect("nested batch");
    assert_parquet_lmc2_roundtrip(&semantic_case_path(&temp, "nested", nested));
}
