# Parquet Legacy Fixture Manifest

source_family: parquet
source_fixture: legacy-v1.parquet
generator_crate: parquet
generator_version: 57.0.0
generator_command: cargo run --manifest-path /tmp/loom-legacy-writers.*/parquet57/Cargo.toml -- crates/loom-parquet-ingress/tests/fixtures/legacy/legacy-v1.parquet
schema: id:Int32 non-null, score:Int64 non-null, ratio32:Float32 non-null, ratio64:Float64 non-null
rows: [(1,10,1.25,1.5), (2,20,-2.5,2.5), (3,30,3.75,3.5)]
source_fixture_sha256: d45ed9f08ade6bd60336cff99ccca5572436e7cbb5524ed8ec84f0191b3ede69
paired_loom_artifact: legacy-v1.loom
paired_loom_sha256: bfd64231ed85db9febd189d1148a3ad9397d9190714ea0034f0542fd54a5909c
paired_loom_generator: current loom-parquet-ingress emit_source_ingress_lmc1_from_parquet_path
paired_loom_verifier: verify_artifact accepted with LMC1/LMT1 table payload
current_source_read_proof: current parquet 58.3.0 ParquetRecordBatchReaderBuilder reads legacy-v1.parquet in cargo test -p loom-parquet-ingress --test legacy_readability
current_rewrite_proof: cargo test -p loom-parquet-ingress --test legacy_readability

The source fixture is an actual Parquet file produced by the older `parquet`
57.0.0 writer crate, not a manifest-only record. The paired Loom artifact is
kept as a sibling file and is not embedded in a Parquet footer.
