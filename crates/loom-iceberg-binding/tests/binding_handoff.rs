use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use arrow_schema::DataType;
use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::container_codec::{decode_table_payload_maybe_container, wrap_table_payload};
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::table_codec::{
    decode_table_to_array_data, encode_table_payload, TableColumn, TableDescription,
};
use loom_iceberg_binding::{bind_iceberg_ref_from_paths, IcebergBindingStatus};
use loom_source_ingress::{
    SourceEmissionDisposition, SourceEmissionKind, SourceIngressStatus, SourceOracleStrategy,
};

fn local_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/local")
        .join(name)
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "loom-iceberg-binding-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn raw_i32_desc(values: &[i32]) -> LayoutDescription {
    LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::Raw {
            data: values.iter().flat_map(|value| value.to_le_bytes()).collect(),
            elem_size: 4,
            count: values.len(),
        },
        row_count: values.len(),
    }
}

fn accepted_lmc1_table_bytes() -> Vec<u8> {
    let table = TableDescription {
        row_count: 3,
        columns: vec![TableColumn {
            name: "id".to_string(),
            layout: raw_i32_desc(&[7, -1, 42]),
        }],
    };
    let payload = encode_table_payload(&table).expect("encode table payload");
    wrap_table_payload(&payload).expect("wrap LMT1 table")
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut child = Command::new("shasum")
        .args(["-a", "256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn shasum");
    child
        .stdin
        .as_mut()
        .expect("shasum stdin")
        .write_all(bytes)
        .expect("write bytes to shasum");
    let output = child.wait_with_output().expect("read shasum output");
    assert!(output.status.success(), "shasum failed");
    String::from_utf8(output.stdout)
        .expect("utf8 shasum output")
        .split_whitespace()
        .next()
        .expect("sha256 digest")
        .to_string()
}

fn decode_i32_table(bytes: &[u8]) -> Vec<i32> {
    let registry = L2KernelRegistry::default_for_mvp0();
    let table = decode_table_payload_maybe_container(bytes).expect("decode table container");
    assert_eq!(table.row_count, 3);
    assert_eq!(
        table
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        vec!["id"]
    );
    let arrays = decode_table_to_array_data(&table, &registry).expect("decode table arrays");
    let ids = arrow_array::Int32Array::from(arrays[0].clone());
    (0..ids.len()).map(|idx| ids.value(idx)).collect()
}

fn assert_verifier_accepts(bytes: &[u8]) {
    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_artifact(bytes, &registry, &Default::default());
    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
    let facts = report.facts().expect("accepted verifier facts");
    assert_eq!(facts.artifact_kind, "LMC1");
    assert_eq!(facts.payload_kind.as_deref(), Some("LMT1 table"));
}

fn write_json(path: &Path, text: String) {
    std::fs::write(path, text).unwrap_or_else(|error| {
        panic!("write {}: {error}", path.display());
    });
}

fn accepted_sidecar_json(
    artifact_path: &Path,
    artifact_sha256: &str,
    evidence_path: &Path,
) -> String {
    format!(
        r#"{{
  "table_uuid": "9f1a03d0-61f7-4f6d-a7a4-3d8b983cbe30",
  "table_name": "demo.events",
  "schema_id": 7,
  "snapshot_id": 314159,
  "ref_name": "main",
  "ref_type": "branch",
  "loom_artifact_path": "{}",
  "loom_artifact_sha256": "{}",
  "source_oracle_evidence_path": "{}",
  "source_evidence": {{
    "accepted": true,
    "status": "accepted",
    "path": "tests/fixtures/local/source/demo-events.parquet"
  }},
  "verifier_evidence": {{
    "accepted": true,
    "status": "accepted",
    "summary": "sidecar accepted claim is descriptive only"
  }},
  "oracle_evidence": {{
    "accepted": true,
    "status": "accepted",
    "strategy": "decoded-row-fixture"
  }}
}}"#,
        artifact_path.display(),
        artifact_sha256,
        evidence_path.display()
    )
}

fn accepted_evidence_json(artifact_sha256: &str) -> String {
    format!(
        r#"{{
  "row_count": 3,
  "table_uuid": "9f1a03d0-61f7-4f6d-a7a4-3d8b983cbe30",
  "schema_id": 7,
  "snapshot_id": 314159,
  "artifact_sha256": "{}",
  "source": {{
    "accepted": true,
    "status": "accepted"
  }},
  "decoded_row_fixture": {{
    "identity": "demo.events#snapshot=314159#schema=7",
    "strategy": "decoded-row-fixture",
    "row_count": 3,
    "accepted": true,
    "oracle_accepted": true,
    "status": "accepted"
  }}
}}"#,
        artifact_sha256
    )
}

fn accepted_fixture_bundle() -> (PathBuf, PathBuf, PathBuf, PathBuf, Vec<u8>, String) {
    let temp = unique_temp_dir("accepted");
    let artifact = temp.join("demo-events.lmc1.loom");
    let sidecar = temp.join("accepted-table-loom-binding.json");
    let evidence = temp.join("accepted-table-source-evidence.json");
    let bytes = accepted_lmc1_table_bytes();
    assert_verifier_accepts(&bytes);
    let artifact_sha256 = sha256_bytes(&bytes);
    std::fs::write(&artifact, &bytes).expect("write artifact");
    write_json(&evidence, accepted_evidence_json(&artifact_sha256));
    write_json(&sidecar, accepted_sidecar_json(&artifact, &artifact_sha256, &evidence));
    (
        local_fixture("accepted-table-metadata.json"),
        sidecar,
        artifact,
        evidence,
        bytes,
        artifact_sha256,
    )
}

#[test]
fn accepted_fixture_sidecar_references_concrete_source_oracle_evidence() {
    let sidecar = std::fs::read_to_string(local_fixture("accepted-table-loom-binding.json"))
        .expect("read accepted sidecar fixture");
    assert!(sidecar.contains("source_oracle_evidence_path"));
    assert!(sidecar.contains("accepted-table-source-evidence.json"));
    assert!(local_fixture("accepted-table-source-evidence.json").is_file());
}

#[test]
fn accepted_binding_requires_hash_verifier_and_source_oracle_evidence() {
    let (metadata, sidecar, artifact, _evidence, expected_bytes, expected_sha) =
        accepted_fixture_bundle();

    let accepted = bind_iceberg_ref_from_paths(&metadata, &sidecar, &artifact)
        .expect("accepted Iceberg binding");

    assert_eq!(accepted.bytes, expected_bytes);
    assert_eq!(decode_i32_table(&accepted.bytes), vec![7, -1, 42]);
    assert_eq!(accepted.report.status, IcebergBindingStatus::Accepted);
    let facts = accepted.report.facts.as_ref().expect("accepted facts");
    assert_eq!(facts.identity.table_uuid, "9f1a03d0-61f7-4f6d-a7a4-3d8b983cbe30");
    assert_eq!(facts.identity.table_name, "demo.events");
    assert_eq!(facts.identity.schema_id, 7);
    assert_eq!(facts.identity.snapshot_id, 314159);
    assert_eq!(facts.identity.ref_name, "main");
    assert_eq!(
        facts.identity.manifest_list_location.as_deref(),
        Some("tests/fixtures/local/metadata/snap-314159.avro")
    );
    assert_eq!(facts.artifact_sha256, expected_sha);

    let evidence = accepted.report.evidence.as_ref().expect("accepted evidence");
    assert!(evidence.artifact_verification.required);
    assert!(evidence.artifact_verification.accepted);
    assert_eq!(
        evidence.artifact_verification.artifact_byte_len,
        Some(accepted.bytes.len())
    );
    assert!(evidence.artifact_verification.summary.contains("LMC1"));
    assert!(evidence.artifact_verification.summary.contains("LMT1"));
    assert_eq!(evidence.source_report.status, SourceIngressStatus::Accepted);
    assert_eq!(evidence.source_report.emission_kind, SourceEmissionKind::Lmt1);
    assert_eq!(
        evidence.source_report.emission_disposition,
        SourceEmissionDisposition::CanonicalTable
    );
    let source_facts = evidence.source_report.facts.as_ref().expect("source facts");
    assert_eq!(source_facts.row_count, 3);
    assert_eq!(
        evidence.oracle_evidence.strategy,
        SourceOracleStrategy::DecodedRowFixture
    );
    assert!(evidence.oracle_evidence.accepted);
    assert_eq!(evidence.oracle_evidence.row_count_checked, Some(3));
    assert!(!evidence.oracle_evidence.source_native_scan_used);
    assert!(accepted.report.diagnostics.is_empty());
}

#[test]
fn sidecar_hash_or_mutated_artifact_bytes_cannot_force_acceptance() {
    let (metadata, sidecar, artifact, _evidence, mut bytes, artifact_sha256) =
        accepted_fixture_bundle();

    let stale_sidecar = sidecar.with_file_name("stale-sidecar.json");
    write_json(
        &stale_sidecar,
        accepted_sidecar_json(
            &artifact,
            "0000000000000000000000000000000000000000000000000000000000000000",
            &sidecar.with_file_name("accepted-table-source-evidence.json"),
        ),
    );
    let report = bind_iceberg_ref_from_paths(&metadata, &stale_sidecar, &artifact)
        .expect_err("stale sidecar hash must fail closed");
    assert_ne!(report.status, IcebergBindingStatus::Accepted);
    assert!(report.evidence.is_none());

    let last_index = bytes.len() - 1;
    bytes[last_index] ^= 0xff;
    let mutated_artifact = artifact.with_file_name("mutated-demo-events.lmc1.loom");
    std::fs::write(&mutated_artifact, bytes).expect("write mutated artifact");
    let mutated_sidecar = sidecar.with_file_name("mutated-sidecar.json");
    write_json(
        &mutated_sidecar,
        accepted_sidecar_json(
            &mutated_artifact,
            &artifact_sha256,
            &sidecar.with_file_name("accepted-table-source-evidence.json"),
        ),
    );
    let report = bind_iceberg_ref_from_paths(&metadata, &mutated_sidecar, &mutated_artifact)
        .expect_err("mutated artifact must fail closed");
    assert_ne!(report.status, IcebergBindingStatus::Accepted);
    assert!(report.evidence.is_none());
}

#[test]
fn sidecar_oracle_claim_is_not_sufficient_without_matching_evidence_artifact() {
    let (metadata, sidecar, artifact, evidence, _bytes, artifact_sha256) =
        accepted_fixture_bundle();

    let missing_evidence_sidecar = sidecar.with_file_name("missing-evidence-sidecar.json");
    write_json(
        &missing_evidence_sidecar,
        accepted_sidecar_json(&artifact, &artifact_sha256, &evidence.with_file_name("missing-evidence.json")),
    );
    let report = bind_iceberg_ref_from_paths(&metadata, &missing_evidence_sidecar, &artifact)
        .expect_err("missing evidence file must fail closed");
    assert_ne!(report.status, IcebergBindingStatus::Accepted);
    assert!(report.evidence.is_none());

    let cases = vec![
        (
            "row-count",
            r#""row_count": 3"#.to_string(),
            r#""row_count": 4"#.to_string(),
        ),
        (
            "table-uuid",
            r#""table_uuid": "9f1a03d0-61f7-4f6d-a7a4-3d8b983cbe30""#.to_string(),
            r#""table_uuid": "00000000-0000-0000-0000-000000000000""#.to_string(),
        ),
        (
            "schema-id",
            r#""schema_id": 7"#.to_string(),
            r#""schema_id": 8"#.to_string(),
        ),
        (
            "snapshot-id",
            r#""snapshot_id": 314159"#.to_string(),
            r#""snapshot_id": 271828"#.to_string(),
        ),
        (
            "artifact-sha",
            format!(r#""artifact_sha256": "{}""#, artifact_sha256),
            r#""artifact_sha256": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff""#
                .to_string(),
        ),
        (
            "source-status",
            r#""accepted": true"#.to_string(),
            r#""accepted": false"#.to_string(),
        ),
        (
            "oracle-status",
            r#""oracle_accepted": true"#.to_string(),
            r#""oracle_accepted": false"#.to_string(),
        ),
    ];

    for (name, from, to) in cases {
        let mutated_evidence = evidence.with_file_name(format!("{name}-evidence.json"));
        let text = std::fs::read_to_string(&evidence)
            .expect("read accepted evidence")
            .replacen(&from, &to, 1);
        write_json(&mutated_evidence, text);
        let mutated_sidecar = sidecar.with_file_name(format!("{name}-sidecar.json"));
        write_json(
            &mutated_sidecar,
            accepted_sidecar_json(&artifact, &artifact_sha256, &mutated_evidence),
        );

        let report = bind_iceberg_ref_from_paths(&metadata, &mutated_sidecar, &artifact)
            .expect_err("evidence mismatch must fail closed");
        assert_ne!(report.status, IcebergBindingStatus::Accepted, "{name}");
        assert!(report.evidence.is_none(), "{name}");
    }
}
