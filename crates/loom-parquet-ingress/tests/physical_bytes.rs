//! Plan 4 (building block): read a Parquet column chunk's raw physical bytes
//! directly (seek + byte_range), with no Arrow materialization, and hash them.

use std::sync::Arc;

use loom_ffi::arrow_array::{Int32Array, Int64Array, RecordBatch};
use loom_ffi::arrow_schema::{DataType, Field, Schema};
use loom_ffi::parquet::arrow::ArrowWriter;

use loom_parquet_ingress::{parquet_column_chunk_hash, read_column_chunk_physical_bytes};

fn write_two_col_parquet(path: &std::path::Path) {
    let schema = Arc::new(Schema::new(vec![
        Field::new("a", DataType::Int32, false),
        Field::new("b", DataType::Int64, false),
    ]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int32Array::from(vec![1, 2, 3, 4, 5, 6, 7, 8])),
            Arc::new(Int64Array::from(vec![10, 20, 30, 40, 50, 60, 70, 80])),
        ],
    )
    .expect("batch");
    let file = std::fs::File::create(path).expect("create");
    let mut w = ArrowWriter::try_new(file, schema, None).expect("writer");
    w.write(&batch).expect("write");
    w.close().expect("close");
}

#[test]
fn reads_physical_chunk_bytes_directly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("phys.parquet");
    write_two_col_parquet(&path);

    // Column 0 (Int32) and column 1 (Int64) physical chunks.
    let c0 = read_column_chunk_physical_bytes(&path, 0, 0).expect("read c0");
    let c1 = read_column_chunk_physical_bytes(&path, 0, 1).expect("read c1");

    assert!(!c0.is_empty(), "physical chunk 0 must be non-empty");
    assert!(!c1.is_empty(), "physical chunk 1 must be non-empty");
    // The two columns occupy distinct physical regions → distinct bytes.
    assert_ne!(c0, c1, "different columns have different physical bytes");

    // Reads are deterministic and the hash matches a direct hash of the bytes.
    let again = read_column_chunk_physical_bytes(&path, 0, 0).expect("re-read c0");
    assert_eq!(c0, again, "physical read is deterministic");
    let h0 = parquet_column_chunk_hash(&path, 0, 0).expect("hash c0");
    assert_eq!(h0, loom_ir_core::sidecar::compute_chunk_hash(&c0));
    assert!(h0.starts_with("blake3:"), "hash is a blake3 identity: {h0}");

    // Distinct columns hash differently.
    let h1 = parquet_column_chunk_hash(&path, 0, 1).expect("hash c1");
    assert_ne!(h0, h1);
}

#[test]
fn out_of_range_fails_closed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("phys2.parquet");
    write_two_col_parquet(&path);
    assert!(read_column_chunk_physical_bytes(&path, 0, 99).is_err());
    assert!(read_column_chunk_physical_bytes(&path, 99, 0).is_err());
}
