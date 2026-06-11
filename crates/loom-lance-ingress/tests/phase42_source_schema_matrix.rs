use std::path::Path;
use std::sync::Arc;

use arrow_array::types::Int32Type;
use arrow_array::{
    Array, ArrayRef, BooleanArray, Int32Array, ListArray, RecordBatch, RecordBatchIterator,
    StringArray, StructArray,
};
use arrow_schema::{DataType, Field, Schema};
use lance::Dataset;
use futures::TryStreamExt;
use loom_ffi::arrow_semantic::{ArrowSemanticBatch, ArrowSemanticPayload};
use loom_ffi::arrow_semantic_codec::encode_arrow_semantic_container_payload;
use loom_ffi::artifact_types::{verify_artifact, ArtifactVerificationStatus};
use loom_ffi::l2_kernel_registry::L2KernelRegistry;
use loom_lance_ingress::lance_source_facts_from_path;
use loom_source_ingress::{
    source_verified_native_coverage_row, validate_source_verified_native_coverage_row,
    SourceIngressStatus, SourceVerifiedNativeDisposition,
};
use tempfile::TempDir;

async fn write_lance_dataset(path: &Path, batch: RecordBatch) {
    let schema = batch.schema();
    let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
    Dataset::write(reader, path.to_str().expect("utf-8 temp path"), None)
        .await
        .expect("write Lance dataset");
}

/// Dev-time oracle + LMC2 emission (compact — test only needs artifact bytes).
async fn dev_time_lance_lmc2_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let dataset = Dataset::open(path.to_str().ok_or("non-utf8 path")?)
        .await.map_err(|e| format!("open: {e}"))?;
    let scanner = dataset.scan();
    let stream = scanner.try_into_stream().await.map_err(|e| format!("scan: {e}"))?;
    let batches: Vec<RecordBatch> = stream.try_collect::<Vec<_>>().await.map_err(|e| format!("collect: {e}"))?;
    let schema = batches.first().map(RecordBatch::schema).ok_or("no batches")?;
    let semantic = batches.iter().map(ArrowSemanticBatch::from_record_batch)
        .collect::<Result<Vec<_>, _>>().map_err(|e| format!("batch: {e}"))?;
    let payload = ArrowSemanticPayload::try_new(schema, semantic).map_err(|e| format!("payload: {e}"))?;
    encode_arrow_semantic_container_payload(&payload).map_err(|e| format!("LMC2: {e}"))
}

async fn write_single_column(
    temp: &TempDir,
    name: &str,
    field: Field,
    array: ArrayRef,
) -> std::path::PathBuf {
    let path = temp.path().join(format!("{name}.lance"));
    let schema = Arc::new(Schema::new(vec![field]));
    let batch = RecordBatch::try_new(schema, vec![array]).expect("record batch");
    write_lance_dataset(&path, batch).await;
    path
}

async fn assert_lmc2_verifier_accepts(path: &Path) {
    let artifact_bytes = dev_time_lance_lmc2_bytes(path).await.expect("Lance source should emit LMC2");
    let report = verify_artifact(
        &artifact_bytes,
        &L2KernelRegistry::default_for_mvp0(),
        &Default::default(),
    );
    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
    assert_eq!(report.facts().expect("facts").artifact_kind, "LMC2");
}

#[tokio::test(flavor = "current_thread")]
async fn lance_phase42_schema_rows_record_native_and_interpreter_disposition() {
    let temp = TempDir::new().expect("tempdir");

    let primitive_path = write_single_column(
        &temp,
        "nullable-i32",
        Field::new("id", DataType::Int32, true),
        Arc::new(Int32Array::from(vec![Some(1), None, Some(3)])),
    )
    .await;
    assert_lmc2_verifier_accepts(&primitive_path).await;
    let primitive_facts = lance_source_facts_from_path(&primitive_path)
        .await
        .expect("primitive facts");
    let primitive_row = source_verified_native_coverage_row(
        "lance",
        "lance-nullable-i32",
        primitive_facts.coverage.as_ref().expect("coverage"),
        SourceVerifiedNativeDisposition::NativeSupported,
        [
            "verified-lineage-record",
            "native-model-validation",
            "phase35-fixed-width-primitive",
        ],
    );
    assert_eq!(primitive_row.source_status, SourceIngressStatus::Accepted);
    assert_eq!(
        primitive_row.emitted_loom_shape,
        "LMC2(LMA1)/semantic-arrow"
    );
    assert_eq!(
        primitive_row.native_disposition,
        SourceVerifiedNativeDisposition::NativeSupported
    );
    assert!(validate_source_verified_native_coverage_row(&primitive_row).is_empty());

    let utf8_path = write_single_column(
        &temp,
        "utf8",
        Field::new("name", DataType::Utf8, true),
        Arc::new(StringArray::from(vec![Some("alpha"), None, Some("beta")])),
    )
    .await;
    assert_lmc2_verifier_accepts(&utf8_path).await;
    let utf8_facts = lance_source_facts_from_path(&utf8_path)
        .await
        .expect("utf8 facts");
    let utf8_row = source_verified_native_coverage_row(
        "lance",
        "lance-utf8",
        utf8_facts.coverage.as_ref().expect("coverage"),
        SourceVerifiedNativeDisposition::InterpreterOnly,
        [
            "verified-lineage-record",
            "native-unsupported-shape-fail-closed",
        ],
    );
    assert_eq!(
        utf8_row.native_disposition,
        SourceVerifiedNativeDisposition::InterpreterOnly
    );
    assert!(validate_source_verified_native_coverage_row(&utf8_row).is_empty());

    let list = ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
        Some(vec![Some(1), Some(2)]),
        None,
        Some(vec![Some(3), None]),
    ]);
    let list_path = write_single_column(
        &temp,
        "list",
        Field::new("items", list.data_type().clone(), true),
        Arc::new(list),
    )
    .await;
    assert_lmc2_verifier_accepts(&list_path).await;
    let list_facts = lance_source_facts_from_path(&list_path)
        .await
        .expect("list facts");
    let list_row = source_verified_native_coverage_row(
        "lance",
        "lance-list-int32",
        list_facts.coverage.as_ref().expect("coverage"),
        SourceVerifiedNativeDisposition::InterpreterOnly,
        [
            "verified-lineage-record",
            "native-unsupported-shape-fail-closed",
        ],
    );
    assert!(list_row.source_schema_shape.contains("nested"));
    assert!(validate_source_verified_native_coverage_row(&list_row).is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn lance_phase42_struct_row_is_verified_but_interpreter_only() {
    let temp = TempDir::new().expect("tempdir");
    let struct_array = StructArray::from(vec![
        (
            Arc::new(Field::new("child_id", DataType::Int32, true)),
            Arc::new(Int32Array::from(vec![Some(10), None, Some(30)])) as ArrayRef,
        ),
        (
            Arc::new(Field::new("child_ok", DataType::Boolean, true)),
            Arc::new(BooleanArray::from(vec![Some(true), None, Some(false)])) as ArrayRef,
        ),
    ]);
    let path = write_single_column(
        &temp,
        "struct",
        Field::new("record", struct_array.data_type().clone(), true),
        Arc::new(struct_array),
    )
    .await;
    assert_lmc2_verifier_accepts(&path).await;
    let facts = lance_source_facts_from_path(&path)
        .await
        .expect("struct facts");
    let row = source_verified_native_coverage_row(
        "lance",
        "lance-struct",
        facts.coverage.as_ref().expect("coverage"),
        SourceVerifiedNativeDisposition::InterpreterOnly,
        [
            "verified-lineage-record",
            "native-unsupported-shape-fail-closed",
        ],
    );

    assert!(row.source_schema_shape.contains("nested"));
    assert_eq!(
        row.native_disposition,
        SourceVerifiedNativeDisposition::InterpreterOnly
    );
    assert!(validate_source_verified_native_coverage_row(&row).is_empty());
}
