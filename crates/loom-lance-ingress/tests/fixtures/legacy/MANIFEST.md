# Lance Legacy Fixture Manifest

source_family: lance
source_fixture: legacy-v1.lance/
generator_crate: lance
generator_version: 6.0.0
generator_command: cargo run --manifest-path /tmp/loom-legacy-writers.*/lance60/Cargo.toml -- ingress/loom-lance-ingress/tests/fixtures/legacy/legacy-v1.lance
schema: id:Int32 non-null, score:Int64 non-null, ratio32:Float32 non-null, ratio64:Float64 non-null
rows: [(1,10,1.25,1.5), (2,20,-2.5,2.5), (3,30,3.75,3.5)]
source_fixture_tree_sha256: 33fa06145c8fd4e489db5dd0000c42b5644506b2aff571088908683a0b710c5e
paired_loom_artifact: legacy-v1.loom
paired_loom_sha256: bfd64231ed85db9febd189d1148a3ad9397d9190714ea0034f0542fd54a5909c
paired_loom_generator: historical loom-lance-ingress emit_source_ingress_lmc1_from_lance_path
paired_loom_verifier: verify_artifact accepted with LMC1/LMT1 table payload
current_source_read_proof: current lance 7.0.0 Dataset::open/scan reads legacy-v1.lance in cargo test -p loom-lance-ingress --test legacy_readability
current_rewrite_proof: current loom-lance-ingress emit_source_ingress_lma1_from_lance_path emits verifier-accepted LMA1 semantic payload in cargo test -p loom-lance-ingress --test legacy_readability

The source fixture is an actual Lance dataset directory produced by the older
`lance` 6.0.0 writer crate, not a manifest-only record. The paired Loom
artifact is kept as a sibling file and is not embedded in a Lance manifest.
