use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use arrow_array::{
    ArrayRef, Date32Array, Float32Array, Float64Array, Int32Array, Int64Array, RecordBatch,
    StringArray, StructArray,
};
use arrow_schema::{DataType, Field, Schema};
use loom_parquet_ingress::parquet_source_facts_from_path;
use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceDiagnosticCode, SourceEmissionDisposition,
    SourceEmissionKind, SourceIngressAcceptedArtifact, SourceIngressReport, SourceIngressStatus,
    SourceLoweringDisposition,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use tempfile::TempDir;

use loom_ffi::arrow_semantic::{ArrowSemanticBatch, ArrowSemanticPayload};
use loom_ffi::arrow_semantic_codec::encode_arrow_semantic_container_payload;
use loom_ffi::artifact_types::{verify_artifact, ArtifactVerificationStatus};
use loom_ffi::l2_kernel_registry::L2KernelRegistry;
use loom_source_ingress::{
    SourceDiagnostic, SourceOracleEvidence, SourceOracleStrategy,
};

fn write_record_batch(path: &Path, batch: RecordBatch) {
    let file = File::create(path).expect("create parquet file");
    let mut writer =
        ArrowWriter::try_new(file, batch.schema(), None).expect("create parquet writer");
    writer.write(&batch).expect("write parquet batch");
    writer.close().expect("close parquet writer");
}

/// Dev-time oracle: read Arrow RecordBatches from a Parquet file.
fn dev_time_parquet_oracle(path: &Path) -> Result<Vec<RecordBatch>, String> {
    let file = File::open(path).map_err(|e| format!("open: {e}"))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("parquet reader build: {e}"))?;
    let reader = builder.build().map_err(|e| format!("parquet reader: {e}"))?;
    reader.map(|r| r.map_err(|e| format!("parquet read: {e}"))).collect()
}

/// Dev-time replacement for dev_time_parquet_source_report.
fn dev_time_parquet_source_report(path: &Path) -> SourceIngressReport {
    let source_facts = match parquet_source_facts_from_path(path) {
        Ok(facts) => facts,
        Err(report) => return report,
    };
    let batches = match dev_time_parquet_oracle(path) {
        Ok(b) => b,
        Err(msg) => {
            return SourceIngressReport::unsupported(
                Some(source_facts.clone()),
                SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", msg),
            );
        }
    };
    let schema = match batches.first().map(RecordBatch::schema) {
        Some(s) => s,
        None => {
            return SourceIngressReport::unsupported(
                Some(source_facts.clone()),
                SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", "no batches"),
            );
        }
    };
    let semantic_batches = match batches.iter().map(ArrowSemanticBatch::from_record_batch)
        .collect::<Result<Vec<_>, _>>() {
        Ok(b) => b,
        Err(err) => {
            return SourceIngressReport::unsupported(Some(source_facts.clone()),
                SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", format!("batch: {err}")));
        }
    };
    let payload = match ArrowSemanticPayload::try_new(schema, semantic_batches) {
        Ok(p) => p,
        Err(err) => {
            return SourceIngressReport::unsupported(Some(source_facts.clone()),
                SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", format!("payload: {err}")));
        }
    };
    let artifact_bytes = match encode_arrow_semantic_container_payload(&payload) {
        Ok(b) => b,
        Err(err) => {
            return SourceIngressReport::unsupported(Some(source_facts.clone()),
                SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", format!("LMC2: {err}")));
        }
    };
    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_artifact(&artifact_bytes, &registry, &Default::default());
    if verification.status() != ArtifactVerificationStatus::Accepted {
        return SourceIngressReport::unsupported(Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.artifact",
                format!("verification: {}", verification.status().as_str())));
    }
    let artifact_facts = verification.facts().expect("accepted");
    let artifact_summary = SourceArtifactVerificationSummary::accepted(
        artifact_bytes.len(),
        format!("{} verifier accepted {}", artifact_facts.artifact_kind,
            artifact_facts.payload_kind.as_deref().unwrap_or("unknown payload")),
    );
    let row_count = batches.iter().map(|b| b.num_rows() as u64).sum();
    let mut oracle = SourceOracleEvidence::accepted(SourceOracleStrategy::SourceNativeScan, row_count);
    oracle.nulls_checked = true;
    oracle.notes.push("source-native oracle evidence via dev-time Parquet read".to_string());
    SourceIngressReport::accepted(
        source_facts, SourceEmissionKind::ArrowSemantic, SourceEmissionDisposition::SemanticArrow,
        SourceLoweringDisposition::InterpreterOnly, artifact_summary, oracle,
    ).expect("accepted Parquet semantic facts map to an accepted source report")
}

fn supported_int32_path(temp: &TempDir) -> std::path::PathBuf {
    let path = temp.path().join("supported-int32.parquet");
    let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int32, false)]));
    let batch = RecordBatch::try_new(schema, vec![Arc::new(Int32Array::from(vec![7, -1, 42]))])
        .expect("record batch");
    write_record_batch(&path, batch);
    path
}

fn parquet_path_for_column(
    temp: &TempDir,
    name: &str,
    data_type: DataType,
    nullable: bool,
    array: ArrayRef,
) -> std::path::PathBuf {
    parquet_path_for_field(temp, name, Field::new(name, data_type, nullable), array)
}

fn parquet_path_for_field(
    temp: &TempDir,
    name: &str,
    field: Field,
    array: ArrayRef,
) -> std::path::PathBuf {
    let path = temp.path().join(format!("{name}.parquet"));
    let schema = Arc::new(Schema::new(vec![field]));
    let batch = RecordBatch::try_new(schema, vec![array]).expect("record batch");
    write_record_batch(&path, batch);
    path
}

fn supported_table_path(temp: &TempDir) -> std::path::PathBuf {
    let path = temp.path().join("supported-table.parquet");
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int32, false),
        Field::new("score", DataType::Int64, false),
    ]));
    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(Int32Array::from(vec![1, 2, 3])),
            Arc::new(Int64Array::from(vec![10, 20, 30])),
        ],
    )
    .expect("record batch");
    write_record_batch(&path, batch);
    path
}

#[test]
fn parquet_facts_include_schema_and_row_group_metadata() {
    let temp = TempDir::new().expect("tempdir");
    let path = supported_int32_path(&temp);

    let facts = parquet_source_facts_from_path(&path).expect("parquet facts");
    let root = facts.root_schema.as_ref().expect("root schema fact");
    let coverage = facts.coverage.as_ref().expect("coverage");

    assert_eq!(facts.identity.source_kind, "parquet");
    assert_eq!(facts.identity.format, "external-source");
    assert_eq!(facts.row_count, 3);
    assert_eq!(root.path, "$.schema");
    assert_eq!(root.logical_kind, "struct");
    assert_eq!(root.nullable, Some(false));
    assert_eq!(root.field_count, Some(1));
    assert_eq!(root.field_names, vec!["id"]);
    assert!(
        root.arrow_summary
            .as_deref()
            .expect("arrow summary")
            .contains("Int32"),
        "expected Arrow summary to include Int32"
    );
    assert!(
        facts
            .schema_facts
            .iter()
            .any(|fact| fact.path == "$.schema.id"
                && fact.logical_kind == "primitive"
                && fact.nullable == Some(false)),
        "expected a field-level primitive schema fact"
    );
    assert!(
        facts
            .layout_facts
            .iter()
            .any(|fact| fact.path == "$.metadata" && fact.row_count == Some(3)),
        "expected file metadata layout fact"
    );
    assert!(
        facts
            .layout_facts
            .iter()
            .any(|fact| fact.path == "$.row_groups[0]" && fact.child_count == 1),
        "expected row-group layout fact"
    );
    assert!(
        facts.layout_facts.iter().any(|fact| fact
            .physical_refs
            .iter()
            .any(|item| item.contains("statistics="))),
        "expected column statistics presence to be summarized"
    );
    assert_eq!(facts.split_facts.len(), 1);
    assert_eq!(facts.split_facts[0].index, 0);
    assert_eq!(facts.split_facts[0].start_row, 0);
    assert_eq!(facts.split_facts[0].end_row, 3);
    assert_eq!(facts.split_facts[0].row_count, 3);
    assert!(coverage.has_splits);
    assert!(coverage.has_statistics);
    assert_eq!(coverage.support, SourceIngressStatus::Accepted);
}

#[test]
fn parquet_contract_does_not_leak_sdk_types_to_generic_crates() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    let output = Command::new("rg")
        .args([
            "-n",
            "pub struct Parquet|ParquetMetaData|ParquetRecordBatchReader",
            "ingress/loom-source-ingress",
            "crates/loom-core",
        ])
        .current_dir(&workspace_root)
        .output()
        .expect("run rg source-neutral guard");

    assert_eq!(
        output.status.code(),
        Some(1),
        "Parquet SDK types must not leak into generic/core/ffi surfaces:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn parquet_classifies_materializable_shapes_as_arrow_semantic() {
    let temp = TempDir::new().expect("tempdir");

    let supported_cases = [
        parquet_path_for_column(
            &temp,
            "i32",
            DataType::Int32,
            false,
            Arc::new(Int32Array::from(vec![1, 2, 3])),
        ),
        parquet_path_for_column(
            &temp,
            "i64",
            DataType::Int64,
            false,
            Arc::new(Int64Array::from(vec![1, 2, 3])),
        ),
        parquet_path_for_column(
            &temp,
            "f32",
            DataType::Float32,
            false,
            Arc::new(Float32Array::from(vec![1.0, 2.0, 3.0])),
        ),
        parquet_path_for_column(
            &temp,
            "f64",
            DataType::Float64,
            false,
            Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0])),
        ),
    ];

    for path in supported_cases {
        let facts = parquet_source_facts_from_path(&path).expect("supported facts");
        let coverage = facts.coverage.as_ref().expect("coverage");
        assert_eq!(coverage.support, SourceIngressStatus::Accepted);
        assert_eq!(coverage.emission_kind, SourceEmissionKind::ArrowSemantic);
        assert_eq!(
            coverage.emission_disposition,
            SourceEmissionDisposition::SemanticArrow
        );
        assert_eq!(
            coverage.lowering_disposition,
            SourceLoweringDisposition::InterpreterOnly
        );
    }

    let table_facts =
        parquet_source_facts_from_path(&supported_table_path(&temp)).expect("table facts");
    let table_coverage = table_facts.coverage.as_ref().expect("table coverage");
    assert_eq!(table_coverage.support, SourceIngressStatus::Accepted);
    assert_eq!(
        table_coverage.emission_kind,
        SourceEmissionKind::ArrowSemantic
    );
    assert_eq!(
        table_coverage.emission_disposition,
        SourceEmissionDisposition::SemanticArrow
    );

    let nested_field = Arc::new(Field::new("nested_id", DataType::Int32, false));
    let nested_array: ArrayRef = Arc::new(StructArray::from(vec![(
        nested_field.clone(),
        Arc::new(Int32Array::from(vec![1, 2, 3])) as ArrayRef,
    )]));
    let extension_field = Field::new("ext_i32", DataType::Int32, false).with_metadata(
        [(
            "ARROW:extension:name".to_string(),
            "loom.test.extension".to_string(),
        )]
        .into_iter()
        .collect(),
    );
    let semantic_cases = [
        parquet_path_for_column(
            &temp,
            "nullable_i32",
            DataType::Int32,
            true,
            Arc::new(Int32Array::from(vec![Some(1), None, Some(3)])),
        ),
        parquet_path_for_column(
            &temp,
            "name",
            DataType::Utf8,
            false,
            Arc::new(StringArray::from(vec!["a", "b", "c"])),
        ),
        parquet_path_for_column(
            &temp,
            "nested",
            DataType::Struct(vec![nested_field].into()),
            false,
            nested_array,
        ),
        parquet_path_for_column(
            &temp,
            "day",
            DataType::Date32,
            false,
            Arc::new(Date32Array::from(vec![0, 1, 2])),
        ),
        parquet_path_for_field(
            &temp,
            "ext_i32",
            extension_field,
            Arc::new(Int32Array::from(vec![1, 2, 3])),
        ),
    ];

    for path in semantic_cases {
        let report = dev_time_parquet_source_report(&path);
        assert_eq!(report.status, SourceIngressStatus::Accepted);
        assert!(report.facts.is_some());
        assert_eq!(report.emission_kind, SourceEmissionKind::ArrowSemantic);
        assert_eq!(
            report.emission_disposition,
            SourceEmissionDisposition::SemanticArrow
        );
        assert_eq!(
            report.lowering_disposition,
            SourceLoweringDisposition::InterpreterOnly
        );
        assert!(report.artifact_verification.required);
        assert!(report.artifact_verification.accepted);
        assert!(report.oracle_evidence.is_some());
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    }
}

#[test]
fn parquet_malformed_files_are_rejected_without_facts() {
    let temp = TempDir::new().expect("tempdir");
    let malformed = temp.path().join("malformed.parquet");
    std::fs::write(&malformed, b"not a parquet file").expect("write malformed bytes");

    let malformed_report = dev_time_parquet_source_report(&malformed);
    assert_eq!(malformed_report.status, SourceIngressStatus::Rejected);
    assert!(malformed_report.facts.is_none());
    assert_eq!(malformed_report.emission_kind, SourceEmissionKind::None);
    assert_eq!(
        malformed_report.emission_disposition,
        SourceEmissionDisposition::None
    );
    assert_eq!(
        malformed_report.artifact_verification,
        SourceArtifactVerificationSummary::not_applicable()
    );
    assert!(malformed_report.oracle_evidence.is_none());
    assert_eq!(malformed_report.diagnostics.len(), 1);
    assert_eq!(
        malformed_report.diagnostics[0].code,
        SourceDiagnosticCode::ReadFailed
    );
    assert_eq!(malformed_report.diagnostics[0].path, "$.metadata");

    let missing = temp.path().join("missing.parquet");
    let missing_report = dev_time_parquet_source_report(&missing);
    assert_eq!(missing_report.status, SourceIngressStatus::Rejected);
    assert!(missing_report.facts.is_none());
    assert_eq!(missing_report.emission_kind, SourceEmissionKind::None);
    assert_eq!(
        missing_report.artifact_verification,
        SourceArtifactVerificationSummary::not_applicable()
    );
    assert!(missing_report.oracle_evidence.is_none());
    assert_eq!(missing_report.diagnostics.len(), 1);
    assert_eq!(
        missing_report.diagnostics[0].code,
        SourceDiagnosticCode::OpenFailed
    );
    assert_eq!(missing_report.diagnostics[0].path, "$.open");
}
