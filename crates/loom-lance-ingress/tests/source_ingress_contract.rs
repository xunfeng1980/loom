use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use arrow_array::{
    ArrayRef, Date32Array, Float32Array, Float64Array, Int32Array, Int64Array, RecordBatch,
    RecordBatchIterator, StringArray, StructArray,
};
use arrow_schema::{DataType, Field, Schema};
use lance::Dataset;
use loom_lance_ingress::{lance_source_facts_from_path, source_ingress_report_from_lance_path};
use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceDiagnosticCode, SourceEmissionDisposition,
    SourceEmissionKind, SourceIngressStatus, SourceLoweringDisposition,
};
use tempfile::TempDir;

async fn write_lance_dataset(path: &Path, batch: RecordBatch) {
    let schema = batch.schema();
    let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
    Dataset::write(reader, path.to_str().expect("utf-8 temp path"), None)
        .await
        .expect("write Lance dataset");
}

async fn supported_int32_dataset(temp: &TempDir) -> std::path::PathBuf {
    let path = temp.path().join("supported-int32.lance");
    let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int32, false)]));
    let batch = RecordBatch::try_new(schema, vec![Arc::new(Int32Array::from(vec![7, -1, 42]))])
        .expect("record batch");
    write_lance_dataset(&path, batch).await;
    path
}

async fn lance_path_for_column(
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

async fn supported_table_path(temp: &TempDir) -> std::path::PathBuf {
    let path = temp.path().join("supported-table.lance");
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
    write_lance_dataset(&path, batch).await;
    path
}

#[tokio::test(flavor = "current_thread")]
async fn lance_facts_include_schema_version_and_fragment_metadata() {
    let temp = TempDir::new().expect("tempdir");
    let path = supported_int32_dataset(&temp).await;

    let facts = lance_source_facts_from_path(&path)
        .await
        .expect("Lance source facts");
    let root = facts.root_schema.as_ref().expect("root schema fact");
    let coverage = facts.coverage.as_ref().expect("coverage");

    assert_eq!(facts.identity.source_kind, "lance");
    assert_eq!(facts.identity.format, "external-source");
    assert_eq!(facts.identity.format_version.as_deref(), Some("1"));
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
            .any(|fact| fact.path == "$.manifest" && fact.row_count == Some(3)),
        "expected manifest layout fact"
    );
    assert!(
        facts
            .layout_facts
            .iter()
            .any(|fact| fact.path == "$.fragments[0]"
                && fact.row_count == Some(3)
                && fact.physical_refs.iter().any(|item| item == "data_files=1")
                && fact
                    .physical_refs
                    .iter()
                    .any(|item| item == "validation=ok")),
        "expected fragment metadata to be summarized"
    );
    assert_eq!(facts.split_facts.len(), 1);
    assert_eq!(facts.split_facts[0].index, 0);
    assert_eq!(facts.split_facts[0].start_row, 0);
    assert_eq!(facts.split_facts[0].end_row, 3);
    assert_eq!(facts.split_facts[0].row_count, 3);
    assert!(coverage.has_splits);
    assert_eq!(coverage.support, SourceIngressStatus::Accepted);
}

#[test]
fn lance_contract_does_not_leak_sdk_types_to_generic_crates() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    let output = Command::new("rg")
        .args([
            "-n",
            "pub struct Lance|Dataset|FileFragment|object_store",
            "crates/loom-source-ingress",
            "crates/loom-core",
            "crates/loom-ffi",
        ])
        .current_dir(&workspace_root)
        .output()
        .expect("run rg source-neutral guard");

    assert_eq!(
        output.status.code(),
        Some(1),
        "Lance SDK types must not leak into generic/core/ffi surfaces:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lance_classifies_supported_and_unsupported_shapes() {
    let temp = TempDir::new().expect("tempdir");

    let supported_cases = [
        lance_path_for_column(
            &temp,
            "i32",
            Field::new("i32", DataType::Int32, false),
            Arc::new(Int32Array::from(vec![1, 2, 3])),
        )
        .await,
        lance_path_for_column(
            &temp,
            "i64",
            Field::new("i64", DataType::Int64, false),
            Arc::new(Int64Array::from(vec![1, 2, 3])),
        )
        .await,
        lance_path_for_column(
            &temp,
            "f32",
            Field::new("f32", DataType::Float32, false),
            Arc::new(Float32Array::from(vec![1.0, 2.0, 3.0])),
        )
        .await,
        lance_path_for_column(
            &temp,
            "f64",
            Field::new("f64", DataType::Float64, false),
            Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0])),
        )
        .await,
    ];

    for path in supported_cases {
        let facts = lance_source_facts_from_path(&path)
            .await
            .expect("supported facts");
        let coverage = facts.coverage.as_ref().expect("coverage");
        assert_eq!(coverage.support, SourceIngressStatus::Accepted);
        assert_eq!(coverage.emission_kind, SourceEmissionKind::Lmp1);
        assert_eq!(
            coverage.emission_disposition,
            SourceEmissionDisposition::CanonicalRaw
        );
        assert_eq!(
            coverage.lowering_disposition,
            SourceLoweringDisposition::ProductionLoweringSupported
        );
    }

    let table_facts = lance_source_facts_from_path(&supported_table_path(&temp).await)
        .await
        .expect("table facts");
    let table_coverage = table_facts.coverage.as_ref().expect("table coverage");
    assert_eq!(table_coverage.support, SourceIngressStatus::Accepted);
    assert_eq!(table_coverage.emission_kind, SourceEmissionKind::Lmt1);
    assert_eq!(
        table_coverage.emission_disposition,
        SourceEmissionDisposition::CanonicalTable
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
    let unsupported_cases = [
        (
            lance_path_for_column(
                &temp,
                "nullable_i32",
                Field::new("nullable_i32", DataType::Int32, true),
                Arc::new(Int32Array::from(vec![Some(1), None, Some(3)])),
            )
            .await,
            SourceDiagnosticCode::UnsupportedSchema,
        ),
        (
            lance_path_for_column(
                &temp,
                "name",
                Field::new("name", DataType::Utf8, false),
                Arc::new(StringArray::from(vec!["a", "b", "c"])),
            )
            .await,
            SourceDiagnosticCode::UnsupportedConversion,
        ),
        (
            lance_path_for_column(
                &temp,
                "nested",
                Field::new("nested", DataType::Struct(vec![nested_field].into()), false),
                nested_array,
            )
            .await,
            SourceDiagnosticCode::UnsupportedSchema,
        ),
        (
            lance_path_for_column(
                &temp,
                "day",
                Field::new("day", DataType::Date32, false),
                Arc::new(Date32Array::from(vec![0, 1, 2])),
            )
            .await,
            SourceDiagnosticCode::UnsupportedConversion,
        ),
        (
            lance_path_for_column(
                &temp,
                "ext_i32",
                extension_field,
                Arc::new(Int32Array::from(vec![1, 2, 3])),
            )
            .await,
            SourceDiagnosticCode::UnsupportedSchema,
        ),
    ];

    for (path, expected_code) in unsupported_cases {
        let report = source_ingress_report_from_lance_path(&path).await;
        assert_eq!(report.status, SourceIngressStatus::Unsupported);
        assert!(report.facts.is_some());
        assert_eq!(report.emission_kind, SourceEmissionKind::None);
        assert_eq!(report.emission_disposition, SourceEmissionDisposition::None);
        assert_eq!(
            report.lowering_disposition,
            SourceLoweringDisposition::FailClosedDeferred
        );
        assert_eq!(
            report.artifact_verification,
            SourceArtifactVerificationSummary::not_applicable()
        );
        assert!(report.oracle_evidence.is_none());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == expected_code),
            "expected unsupported diagnostic {expected_code:?}, got {:?}",
            report.diagnostics
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn lance_non_dataset_paths_are_rejected_without_facts() {
    let temp = TempDir::new().expect("tempdir");
    let regular_file = temp.path().join("not-a-dataset.lance");
    std::fs::write(&regular_file, b"not a Lance dataset").expect("write non-dataset file");

    let regular_report = source_ingress_report_from_lance_path(&regular_file).await;
    assert_eq!(regular_report.status, SourceIngressStatus::Rejected);
    assert!(regular_report.facts.is_none());
    assert_eq!(regular_report.emission_kind, SourceEmissionKind::None);
    assert_eq!(
        regular_report.emission_disposition,
        SourceEmissionDisposition::None
    );
    assert_eq!(
        regular_report.artifact_verification,
        SourceArtifactVerificationSummary::not_applicable()
    );
    assert!(regular_report.oracle_evidence.is_none());
    assert_eq!(regular_report.diagnostics.len(), 1);
    assert_eq!(
        regular_report.diagnostics[0].code,
        SourceDiagnosticCode::OpenFailed
    );
    assert_eq!(regular_report.diagnostics[0].path, "$.open");

    let missing = temp.path().join("missing.lance");
    let missing_report = source_ingress_report_from_lance_path(&missing).await;
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
