use std::path::PathBuf;

use loom_dual_query_surface::{
    duckdb_query_cases, starrocks_descriptors, write_accepted_fixture_bundle,
};

fn main() {
    let out_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/loom-dual-query-surface"));
    if out_dir.exists() {
        std::fs::remove_dir_all(&out_dir).unwrap_or_else(|error| {
            panic!(
                "remove existing output directory {}: {error}",
                out_dir.display()
            )
        });
    }
    let bundle = write_accepted_fixture_bundle(&out_dir).unwrap_or_else(|report| {
        panic!("Phase 28 accepted binding fixture was not accepted: {report:?}");
    });
    let descriptors =
        starrocks_descriptors(&bundle.accepted).expect("build StarRocks-compatible descriptors");
    let duckdb_cases =
        duckdb_query_cases(&bundle.artifact_path, &bundle.accepted).expect("build DuckDB cases");
    let descriptor_path = out_dir.join("starrocks-descriptors.json");
    let duckdb_expected_path = out_dir.join("duckdb-expected.json");
    std::fs::write(
        &descriptor_path,
        serde_json::to_string_pretty(&descriptors).expect("serialize descriptors"),
    )
    .expect("write descriptor JSON");
    std::fs::write(
        &duckdb_expected_path,
        serde_json::to_string_pretty(&duckdb_cases).expect("serialize DuckDB evidence"),
    )
    .expect("write DuckDB evidence JSON");

    println!("ARTIFACT_PATH={}", bundle.artifact_path.display());
    println!("DESCRIPTOR_PATH={}", descriptor_path.display());
    println!("DUCKDB_EXPECTED_PATH={}", duckdb_expected_path.display());
    println!("METADATA_PATH={}", bundle.metadata_path.display());
    println!("SIDECAR_PATH={}", bundle.sidecar_path.display());
}
