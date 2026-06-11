use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow_array::types::Int32Type;
use arrow_array::{
    Array, ArrayRef, BooleanArray, Int32Array, ListArray, RecordBatch, StringArray, StructArray,
};
use arrow_schema::{DataType, Field, Schema};
use loom_ffi::arrow_semantic::{ArrowSemanticBatch, ArrowSemanticPayload};
use loom_ffi::arrow_semantic_codec::{
    decode_arrow_semantic_container_payload, encode_arrow_semantic_container_payload,
};
use loom_ffi::artifact_types::{verify_artifact, ArtifactVerificationStatus};
use loom_ffi::l2_kernel_registry::L2KernelRegistry;
use loom_parquet_ingress::parquet_source_facts_from_path;
use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceDiagnostic, SourceDiagnosticCode,
    SourceEmissionDisposition, SourceEmissionKind, SourceIngressAcceptedArtifact,
    SourceIngressReport, SourceIngressStatus, SourceLoweringDisposition, SourceOracleEvidence,
    SourceOracleStrategy,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
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

/// Dev-time oracle: read Arrow RecordBatches from a Parquet file.
fn dev_time_parquet_oracle(path: &Path) -> Result<Vec<RecordBatch>, String> {
    let file = File::open(path).map_err(|e| format!("open: {e}"))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("parquet reader build: {e}"))?;
    let reader = builder.build().map_err(|e| format!("parquet reader: {e}"))?;
    reader.map(|r| r.map_err(|e| format!("parquet read: {e}"))).collect()
}

/// Dev-time packaging helper: replicates old dev_time_emit_lmc2_from_parquet_path.
fn dev_time_emit_lmc2_from_parquet_path(
    path: &Path,
) -> Result<SourceIngressAcceptedArtifact, SourceIngressReport> {
    let source_facts = parquet_source_facts_from_path(path)?;
    let batches = dev_time_parquet_oracle(path).map_err(|msg| {
        SourceIngressReport::unsupported(
            Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", msg),
        )
    })?;
    let schema = batches.first().map(RecordBatch::schema).ok_or_else(|| {
        SourceIngressReport::unsupported(
            Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", "oracle produced no batches"),
        )
    })?;
    let semantic_batches = batches.iter().map(ArrowSemanticBatch::from_record_batch)
        .collect::<Result<Vec<_>, _>>().map_err(|err| {
            SourceIngressReport::unsupported(Some(source_facts.clone()),
                SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", format!("ArrowSemanticBatch: {err}")))
        })?;
    let payload = ArrowSemanticPayload::try_new(schema, semantic_batches).map_err(|err| {
        SourceIngressReport::unsupported(Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", format!("ArrowSemanticPayload: {err}")))
    })?;
    let artifact_bytes = encode_arrow_semantic_container_payload(&payload).map_err(|err| {
        SourceIngressReport::unsupported(Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", format!("LMC2 encoding: {err}")))
    })?;
    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_artifact(&artifact_bytes, &registry, &Default::default());
    if verification.status() != ArtifactVerificationStatus::Accepted {
        return Err(SourceIngressReport::unsupported(Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.artifact",
                format!("verification failed: {}", verification.status().as_str()))));
    }
    let artifact_facts = verification.facts().expect("accepted artifact verification exposes facts");
    let artifact_summary = SourceArtifactVerificationSummary::accepted(
        artifact_bytes.len(),
        format!("{} verifier accepted {}", artifact_facts.artifact_kind,
            artifact_facts.payload_kind.as_deref().unwrap_or("unknown payload")),
    );
    let row_count = batches.iter().map(|b| b.num_rows() as u64).sum();
    let mut oracle = SourceOracleEvidence::accepted(SourceOracleStrategy::SourceNativeScan, row_count);
    oracle.nulls_checked = true;
    oracle.notes.push("source-native oracle evidence via dev-time Parquet read".to_string());
    let report = SourceIngressReport::accepted(
        source_facts, SourceEmissionKind::ArrowSemantic, SourceEmissionDisposition::SemanticArrow,
        SourceLoweringDisposition::InterpreterOnly, artifact_summary, oracle,
    ).expect("accepted Parquet semantic facts map to an accepted source report");
    Ok(SourceIngressAcceptedArtifact { bytes: artifact_bytes, report })
}

fn assert_parquet_lmc2_roundtrip(path: &Path) {
    let accepted = dev_time_emit_lmc2_from_parquet_path(path)
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

    let source = dev_time_parquet_oracle(path).expect("Parquet Arrow source");
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
