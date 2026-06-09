use std::env;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::Arc;

use arrow_array::{Int32Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use loom_parquet_ingress::{
    emit_source_ingress_lma1_from_parquet_path, emit_source_ingress_lmc2_from_parquet_path,
};
use parquet::arrow::ArrowWriter;

fn main() {
    if let Err(err) = run() {
        eprintln!("emit_duckdb_parquet_lma1_fixture: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let out_dir = env::args()
        .nth(1)
        .ok_or_else(|| "usage: emit_duckdb_parquet_lma1_fixture <output-dir>".to_string())?;
    let out_dir = PathBuf::from(out_dir);
    fs::create_dir_all(&out_dir).map_err(|err| format!("create {}: {err}", out_dir.display()))?;

    let source_path = out_dir.join("source.parquet");
    let loom_path = out_dir.join("parquet.loom");
    let duckdb_bridge_path = out_dir.join("parquet-duckdb-bridge-lma1.loom");
    let schema = Arc::new(Schema::new(vec![Field::new(
        "value",
        DataType::Int32,
        false,
    )]));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![Arc::new(Int32Array::from(vec![7, -1, 42]))],
    )
    .map_err(|err| format!("build record batch: {err}"))?;

    let file = File::create(&source_path)
        .map_err(|err| format!("create {}: {err}", source_path.display()))?;
    let mut writer =
        ArrowWriter::try_new(file, schema, None).map_err(|err| format!("create writer: {err}"))?;
    writer
        .write(&batch)
        .map_err(|err| format!("write parquet batch: {err}"))?;
    writer
        .close()
        .map_err(|err| format!("close parquet writer: {err}"))?;

    let accepted = emit_source_ingress_lmc2_from_parquet_path(&source_path)
        .map_err(|report| format!("emit LMC2 from Parquet failed: {:?}", report.diagnostics))?;
    let duckdb_bridge =
        emit_source_ingress_lma1_from_parquet_path(&source_path).map_err(|report| {
            format!(
                "emit direct LMA1 from Parquet failed: {:?}",
                report.diagnostics
            )
        })?;
    fs::write(&loom_path, &accepted.bytes)
        .map_err(|err| format!("write {}: {err}", loom_path.display()))?;
    fs::write(&duckdb_bridge_path, duckdb_bridge.bytes)
        .map_err(|err| format!("write {}: {err}", duckdb_bridge_path.display()))?;

    println!("source: {}", source_path.display());
    println!("loom: {}", loom_path.display());
    println!("duckdb_bridge_lma1: {}", duckdb_bridge_path.display());
    println!("status: {}", accepted.report.status.as_str());
    println!("emission_kind: {}", accepted.report.emission_kind.as_str());
    Ok(())
}
