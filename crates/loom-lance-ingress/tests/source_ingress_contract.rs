use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use arrow_array::{Int32Array, RecordBatch, RecordBatchIterator};
use arrow_schema::{DataType, Field, Schema};
use lance::Dataset;
use loom_lance_ingress::lance_source_facts_from_path;
use loom_source_ingress::SourceIngressStatus;
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
