use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use arrow_schema::DataType;
use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::container_codec::wrap_table_payload;
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::table_codec::{encode_table_payload, TableColumn, TableDescription};
use loom_iceberg_binding::{
    bind_iceberg_ref_from_paths, iceberg_binding_facts_from_paths,
    source_ingress_report_from_iceberg_metadata_path, IcebergBindingAcceptedArtifact,
    IcebergBindingReport, IcebergBindingStatus,
};
use loom_source_ingress::SourceIngressStatus;

fn local_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/local")
        .join(name)
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "loom-iceberg-mismatch-{name}-{}-{}",
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
            data: values
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect(),
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

fn assert_verifier_accepts(bytes: &[u8]) {
    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_artifact(bytes, &registry, &Default::default());
    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
}

fn write_json(path: &Path, text: String) {
    std::fs::write(path, text).unwrap_or_else(|error| {
        panic!("write {}: {error}", path.display());
    });
}

fn sidecar_json(
    artifact_path: &Path,
    artifact_sha256: &str,
    evidence_path: Option<&Path>,
    source_status: bool,
    verifier_status: bool,
    oracle_status: bool,
) -> String {
    let evidence_path = evidence_path
        .map(|path| {
            format!(
                r#",
  "source_oracle_evidence_path": "{}""#,
                path.display()
            )
        })
        .unwrap_or_default();
    format!(
        r#"{{
  "table_uuid": "9f1a03d0-61f7-4f6d-a7a4-3d8b983cbe30",
  "table_name": "demo.events",
  "schema_id": 7,
  "snapshot_id": 314159,
  "ref_name": "main",
  "ref_type": "branch",
  "loom_artifact_path": "{}",
  "loom_artifact_sha256": "{}"{},
  "source_evidence": {{
    "accepted": {},
    "status": "{}"
  }},
  "verifier_evidence": {{
    "accepted": {},
    "status": "{}"
  }},
  "oracle_evidence": {{
    "accepted": {},
    "status": "{}",
    "strategy": "decoded-row-fixture"
  }}
}}"#,
        artifact_path.display(),
        artifact_sha256,
        evidence_path,
        source_status,
        if source_status { "accepted" } else { "rejected" },
        verifier_status,
        if verifier_status { "accepted" } else { "rejected" },
        oracle_status,
        if oracle_status { "accepted" } else { "rejected" }
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

fn accepted_bundle() -> (PathBuf, PathBuf, PathBuf, PathBuf, String) {
    let temp = unique_temp_dir("bundle");
    let artifact = temp.join("demo-events.lmc1.loom");
    let sidecar = temp.join("accepted-table-loom-binding.json");
    let evidence = temp.join("accepted-table-source-evidence.json");
    let bytes = accepted_lmc1_table_bytes();
    assert_verifier_accepts(&bytes);
    let artifact_sha256 = sha256_bytes(&bytes);
    std::fs::write(&artifact, &bytes).expect("write artifact");
    write_json(&evidence, accepted_evidence_json(&artifact_sha256));
    write_json(
        &sidecar,
        sidecar_json(
            &artifact,
            &artifact_sha256,
            Some(&evidence),
            true,
            true,
            true,
        ),
    );
    (
        local_fixture("accepted-table-metadata.json"),
        sidecar,
        artifact,
        evidence,
        artifact_sha256,
    )
}

fn assert_no_accepted_bytes(result: Result<IcebergBindingAcceptedArtifact, IcebergBindingReport>) {
    match result {
        Ok(accepted) => panic!(
            "mismatch unexpectedly returned {} accepted bytes",
            accepted.bytes.len()
        ),
        Err(report) => {
            assert_ne!(report.status, IcebergBindingStatus::Accepted);
            assert!(report.evidence.is_none());
        }
    }
}

#[test]
fn static_mismatch_sidecars_fail_before_artifact_bytes_are_trusted() {
    for fixture in [
        "mismatch-schema-sidecar.json",
        "mismatch-snapshot-sidecar.json",
        "manifest-only-sidecar.json",
    ] {
        let report = iceberg_binding_facts_from_paths(
            &local_fixture("accepted-table-metadata.json"),
            &local_fixture(fixture),
        )
        .expect_err("mismatch sidecar must not yield accepted facts");
        assert_ne!(report.status, IcebergBindingStatus::Accepted, "{fixture}");
        assert!(report.evidence.is_none(), "{fixture}");
    }
}

#[test]
fn schema_snapshot_table_and_artifact_mismatches_return_no_bytes() {
    let (metadata, sidecar, artifact, evidence, artifact_sha256) = accepted_bundle();

    let schema_sidecar = sidecar.with_file_name("schema-mismatch.json");
    write_json(
        &schema_sidecar,
        sidecar_json(&artifact, &artifact_sha256, Some(&evidence), true, true, true)
            .replace(r#""schema_id": 7"#, r#""schema_id": 8"#),
    );
    assert_no_accepted_bytes(bind_iceberg_ref_from_paths(
        &metadata,
        &schema_sidecar,
        &artifact,
    ));

    let snapshot_sidecar = sidecar.with_file_name("snapshot-mismatch.json");
    write_json(
        &snapshot_sidecar,
        sidecar_json(&artifact, &artifact_sha256, Some(&evidence), true, true, true)
            .replace(r#""snapshot_id": 314159"#, r#""snapshot_id": 271828"#),
    );
    assert_no_accepted_bytes(bind_iceberg_ref_from_paths(
        &metadata,
        &snapshot_sidecar,
        &artifact,
    ));

    let table_sidecar = sidecar.with_file_name("table-mismatch.json");
    write_json(
        &table_sidecar,
        sidecar_json(&artifact, &artifact_sha256, Some(&evidence), true, true, true).replace(
            "9f1a03d0-61f7-4f6d-a7a4-3d8b983cbe30",
            "00000000-0000-0000-0000-000000000000",
        ),
    );
    assert_no_accepted_bytes(bind_iceberg_ref_from_paths(
        &metadata,
        &table_sidecar,
        &artifact,
    ));

    let hash_sidecar = sidecar.with_file_name("hash-mismatch.json");
    write_json(
        &hash_sidecar,
        sidecar_json(
            &artifact,
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            Some(&evidence),
            true,
            true,
            true,
        ),
    );
    assert_no_accepted_bytes(bind_iceberg_ref_from_paths(
        &metadata,
        &hash_sidecar,
        &artifact,
    ));
}

#[test]
fn verifier_status_rejected_bytes_and_missing_evidence_return_no_bytes() {
    let (metadata, sidecar, artifact, evidence, artifact_sha256) = accepted_bundle();

    let verifier_status_sidecar = sidecar.with_file_name("verifier-status-mismatch.json");
    write_json(
        &verifier_status_sidecar,
        sidecar_json(
            &artifact,
            &artifact_sha256,
            Some(&evidence),
            true,
            false,
            true,
        ),
    );
    assert_no_accepted_bytes(bind_iceberg_ref_from_paths(
        &metadata,
        &verifier_status_sidecar,
        &artifact,
    ));

    let malformed_artifact = artifact.with_file_name("malformed.lmc1.loom");
    let malformed_bytes = b"LMC1 malformed verifier-rejected bytes".to_vec();
    let malformed_sha = sha256_bytes(&malformed_bytes);
    std::fs::write(&malformed_artifact, malformed_bytes).expect("write malformed artifact");
    let malformed_sidecar = sidecar.with_file_name("malformed-sidecar.json");
    write_json(
        &malformed_sidecar,
        sidecar_json(
            &malformed_artifact,
            &malformed_sha,
            Some(&evidence),
            true,
            true,
            true,
        ),
    );
    assert_no_accepted_bytes(bind_iceberg_ref_from_paths(
        &metadata,
        &malformed_sidecar,
        &malformed_artifact,
    ));

    let missing_source = sidecar.with_file_name("missing-source-evidence.json");
    let missing_source_json =
        sidecar_json(&artifact, &artifact_sha256, Some(&evidence), true, true, true).replace(
            r#",
  "source_evidence": {
    "accepted": true,
    "status": "accepted"
  }"#,
            "",
        );
    write_json(&missing_source, missing_source_json);
    assert_no_accepted_bytes(bind_iceberg_ref_from_paths(
        &metadata,
        &missing_source,
        &artifact,
    ));

    let missing_oracle = sidecar.with_file_name("missing-oracle-evidence.json");
    let missing_oracle_json =
        sidecar_json(&artifact, &artifact_sha256, Some(&evidence), true, true, true).replace(
            r#",
  "oracle_evidence": {
    "accepted": true,
    "status": "accepted",
    "strategy": "decoded-row-fixture"
  }"#,
            "",
        );
    write_json(&missing_oracle, missing_oracle_json);
    assert_no_accepted_bytes(bind_iceberg_ref_from_paths(
        &metadata,
        &missing_oracle,
        &artifact,
    ));
}

#[test]
fn stale_source_and_forged_oracle_evidence_flags_return_no_bytes() {
    let (metadata, sidecar, artifact, _evidence, artifact_sha256) = accepted_bundle();

    for fixture in [
        "stale-source-evidence.json",
        "forged-oracle-evidence.json",
    ] {
        let forged_sidecar = sidecar.with_file_name(format!("{fixture}.sidecar.json"));
        write_json(
            &forged_sidecar,
            sidecar_json(
                &artifact,
                &artifact_sha256,
                Some(&local_fixture(fixture)),
                true,
                true,
                true,
            ),
        );
        assert_no_accepted_bytes(bind_iceberg_ref_from_paths(
            &metadata,
            &forged_sidecar,
            &artifact,
        ));
    }
}

#[test]
fn manifest_only_remote_credentials_and_public_route_scope_fail_closed() {
    let (metadata, sidecar, artifact, _evidence, artifact_sha256) = accepted_bundle();

    let manifest_only_sidecar = sidecar.with_file_name("manifest-only-dynamic.json");
    write_json(
        &manifest_only_sidecar,
        sidecar_json(&artifact, &artifact_sha256, None, true, true, true),
    );
    assert_no_accepted_bytes(bind_iceberg_ref_from_paths(
        &metadata,
        &manifest_only_sidecar,
        &artifact,
    ));

    let remote_report = source_ingress_report_from_iceberg_metadata_path(&local_fixture(
        "unsupported-remote-metadata.json",
    ));
    assert_eq!(remote_report.status, SourceIngressStatus::Unsupported);
    assert!(remote_report.facts.is_some());
    assert!(!remote_report.artifact_verification.accepted);
    assert!(remote_report.oracle_evidence.is_none());
    assert!(remote_report.diagnostics.iter().any(|diagnostic| {
        let text = format!("{} {:?}", diagnostic.message, diagnostic.source_detail);
        text.contains("s3://") || text.contains("warehouse") || text.contains("rest")
    }));

    let public_surfaces = [
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/loom-ffi/include/loom.h"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../duckdb-ext/loom_extension.cpp"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/loom-cli/src/main.rs"),
    ];
    for surface in public_surfaces {
        let text = std::fs::read_to_string(&surface)
            .unwrap_or_else(|error| panic!("read {}: {error}", surface.display()));
        for forbidden in [
            "loom_scan_iceberg",
            "loom_ingest_iceberg",
            "iceberg_catalog",
            "iceberg_rest",
            "StarRocks",
            "object_store",
            "aws_access_key",
            "secret_access_key",
            "predicate_pushdown_iceberg",
            "split_execution_iceberg",
            "native_iceberg_kernel",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} leaked forbidden public marker {forbidden}",
                surface.display()
            );
        }
    }
}
