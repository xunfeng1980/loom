use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use arrow_array::{Int32Array, RecordBatch, RecordBatchIterator};
use arrow_schema::{DataType, Field, Schema};
use lance::Dataset;
use loom_lance_ingress::{
    emit_source_ingress_lma1_from_lance_path, emit_source_ingress_lmc2_from_lance_path,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("emit_duckdb_lance_lma1_fixture: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let out_dir = env::args()
        .nth(1)
        .ok_or_else(|| "usage: emit_duckdb_lance_lma1_fixture <output-dir>".to_string())?;
    let out_dir = PathBuf::from(out_dir);
    fs::create_dir_all(&out_dir).map_err(|err| format!("create {}: {err}", out_dir.display()))?;

    let source_path = out_dir.join("source.lance");
    let loom_path = out_dir.join("lance.loom");
    let duckdb_bridge_path = out_dir.join("lance-duckdb-bridge-lma1.loom");
    if source_path.exists() {
        if source_path.is_dir() {
            fs::remove_dir_all(&source_path)
        } else {
            fs::remove_file(&source_path)
        }
        .map_err(|err| format!("remove existing {}: {err}", source_path.display()))?;
    }
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
    let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
    Dataset::write(
        reader,
        source_path.to_str().ok_or("non-utf8 output path")?,
        None,
    )
    .await
    .map_err(|err| format!("write Lance dataset: {err}"))?;

    let accepted = emit_source_ingress_lmc2_from_lance_path(&source_path)
        .await
        .map_err(|report| format!("emit LMC2 from Lance failed: {:?}", report.diagnostics))?;
    let duckdb_bridge = emit_source_ingress_lma1_from_lance_path(&source_path)
        .await
        .map_err(|report| {
            format!(
                "emit direct LMA1 from Lance failed: {:?}",
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
