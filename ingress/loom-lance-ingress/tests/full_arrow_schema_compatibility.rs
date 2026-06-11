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
use loom_ffi::arrow_semantic_codec::{
    decode_arrow_semantic_container_payload, encode_arrow_semantic_container_payload,
};
use loom_ffi::artifact_types::{verify_artifact, ArtifactVerificationStatus};
use loom_ffi::l2_kernel_registry::L2KernelRegistry;
use loom_lance_ingress::lance_source_facts_from_path;
use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceDiagnostic, SourceDiagnosticCode,
    SourceEmissionDisposition, SourceEmissionKind, SourceIngressAcceptedArtifact,
    SourceIngressReport, SourceIngressStatus, SourceLoweringDisposition, SourceOracleEvidence,
    SourceOracleStrategy,
};
use tempfile::TempDir;

async fn write_lance_dataset(path: &Path, batch: RecordBatch) {
    let schema = batch.schema();
    let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
    Dataset::write(reader, path.to_str().expect("utf-8 temp path"), None)
        .await
        .expect("write Lance dataset");
}

/// Dev-time oracle: read Arrow RecordBatches from a Lance dataset.
async fn dev_time_lance_oracle(path: &Path) -> Result<Vec<RecordBatch>, String> {
    let dataset = Dataset::open(path.to_str().ok_or("non-utf8 path")?)
        .await.map_err(|e| format!("open: {e}"))?;
    let scanner = dataset.scan();
    let stream = scanner.try_into_stream().await.map_err(|e| format!("scan: {e}"))?;
    stream.try_collect::<Vec<_>>().await.map_err(|e| format!("collect: {e}"))
}

/// Dev-time packaging: replicates old dev_time_emit_lmc2.
async fn dev_time_emit_lmc2(path: &Path) -> Result<SourceIngressAcceptedArtifact, SourceIngressReport> {
    let source_facts = lance_source_facts_from_path(path).await?;
    let batches = dev_time_lance_oracle(path).await.map_err(|msg| {
        SourceIngressReport::unsupported(Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", msg))
    })?;
    let schema = batches.first().map(RecordBatch::schema).ok_or_else(|| {
        SourceIngressReport::unsupported(Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", "no batches"))
    })?;
    let semantic = batches.iter().map(ArrowSemanticBatch::from_record_batch)
        .collect::<Result<Vec<_>, _>>().map_err(|err| {
            SourceIngressReport::unsupported(Some(source_facts.clone()),
                SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", format!("batch: {err}")))
        })?;
    let payload = ArrowSemanticPayload::try_new(schema, semantic).map_err(|err| {
        SourceIngressReport::unsupported(Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", format!("payload: {err}")))
    })?;
    let artifact_bytes = encode_arrow_semantic_container_payload(&payload).map_err(|err| {
        SourceIngressReport::unsupported(Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", format!("LMC2: {err}")))
    })?;
    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_artifact(&artifact_bytes, &registry, &Default::default());
    if verification.status() != ArtifactVerificationStatus::Accepted {
        return Err(SourceIngressReport::unsupported(Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.artifact", format!("verification: {}", verification.status().as_str()))));
    }
    let artifact_facts = verification.facts().expect("accepted");
    let artifact_summary = SourceArtifactVerificationSummary::accepted(artifact_bytes.len(),
        format!("{} verifier accepted {}", artifact_facts.artifact_kind,
            artifact_facts.payload_kind.as_deref().unwrap_or("unknown payload")));
    let row_count = batches.iter().map(|b| b.num_rows() as u64).sum();
    let mut oracle = SourceOracleEvidence::accepted(SourceOracleStrategy::SourceNativeScan, row_count);
    oracle.nulls_checked = true;
    oracle.notes.push("dev-time Lance source-native oracle evidence only".to_string());
    let report = SourceIngressReport::accepted(source_facts, SourceEmissionKind::ArrowSemantic,
        SourceEmissionDisposition::SemanticArrow, SourceLoweringDisposition::InterpreterOnly,
        artifact_summary, oracle).expect("accepted");
    Ok(SourceIngressAcceptedArtifact { bytes: artifact_bytes, report })
}

async fn semantic_case_path(temp: &TempDir, name: &str, batch: RecordBatch) -> std::path::PathBuf {
    let path = temp.path().join(format!("{name}.lance"));
    write_lance_dataset(&path, batch).await;
    path
}

async fn assert_lance_lmc2_roundtrip(path: &Path) {
    let accepted = dev_time_emit_lmc2(path)
        .await
        .expect("accepted Lance semantic handoff");
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

    let source = dev_time_lance_oracle(path)
        .await
        .expect("Lance Arrow source");
    let decoded = decode_arrow_semantic_container_payload(&accepted.bytes)
        .expect("decode LMC2")
        .to_record_batches()
        .expect("LMC2 batches");
    assert_eq!(decoded, source);
}

#[tokio::test(flavor = "current_thread")]
async fn lance_nullable_scalar_bool_utf8_struct_and_list_emit_lmc2() {
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
    let path = semantic_case_path(&temp, "nullable-scalars", nullable_scalars).await;
    assert_lance_lmc2_roundtrip(&path).await;

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
    let path = semantic_case_path(&temp, "nested", nested).await;
    assert_lance_lmc2_roundtrip(&path).await;
}
