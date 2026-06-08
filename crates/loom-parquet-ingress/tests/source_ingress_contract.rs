use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use arrow_array::{Int32Array, Int64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use loom_parquet_ingress::parquet_source_facts_from_path;
use loom_source_ingress::SourceIngressStatus;
use parquet::arrow::ArrowWriter;
use tempfile::TempDir;

fn write_record_batch(path: &Path, batch: RecordBatch) {
    let file = File::create(path).expect("create parquet file");
    let mut writer =
        ArrowWriter::try_new(file, batch.schema(), None).expect("create parquet writer");
    writer.write(&batch).expect("write parquet batch");
    writer.close().expect("close parquet writer");
}

fn supported_int32_path(temp: &TempDir) -> std::path::PathBuf {
    let path = temp.path().join("supported-int32.parquet");
    let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int32, false)]));
    let batch = RecordBatch::try_new(schema, vec![Arc::new(Int32Array::from(vec![7, -1, 42]))])
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
        "Parquet SDK types must not leak into generic/core/ffi surfaces:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[allow(dead_code)]
fn _keeps_int64_import_available_for_later_table_tests(values: Vec<i64>) -> Int64Array {
    Int64Array::from(values)
}
