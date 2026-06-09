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
    bind_iceberg_ref_from_paths, IcebergBindingAcceptedArtifact, IcebergBindingReport,
};

pub const TABLE_UUID: &str = "9f1a03d0-61f7-4f6d-a7a4-3d8b983cbe30";
pub const TABLE_NAME: &str = "demo.events";
pub const SCHEMA_ID: i32 = 7;
pub const SNAPSHOT_ID: i64 = 314159;
pub const ROW_VALUES: [i32; 3] = [7, -1, 42];
const VALUES_SHA256: &str = "82b7236a02334902a5e27c157bcc767f1451246e11959dc13f5c56e028da8d58";

#[derive(Clone, Debug)]
pub struct AcceptedFixtureBundle {
    pub root_dir: PathBuf,
    pub metadata_path: PathBuf,
    pub sidecar_path: PathBuf,
    pub evidence_path: PathBuf,
    pub source_path: PathBuf,
    pub artifact_path: PathBuf,
    pub artifact_sha256: String,
    pub accepted: IcebergBindingAcceptedArtifact,
}

pub fn accepted_fixture_bundle() -> Result<AcceptedFixtureBundle, IcebergBindingReport> {
    let root = std::env::temp_dir().join(format!(
        "loom-dual-query-surface-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    ));
    write_accepted_fixture_bundle(root)
}

pub fn write_accepted_fixture_bundle(
    root_dir: impl AsRef<Path>,
) -> Result<AcceptedFixtureBundle, IcebergBindingReport> {
    let root_dir = root_dir.as_ref().to_path_buf();
    let artifact_path = root_dir.join("demo-events.lmc1.loom");
    let sidecar_path = root_dir.join("accepted-table-loom-binding.json");
    let metadata_path = root_dir.join("accepted-table-metadata.json");
    let evidence_path = root_dir.join("accepted-table-source-evidence.json");
    let source_dir = root_dir.join("source");
    let source_path = source_dir.join("demo-events.parquet");

    std::fs::create_dir_all(&source_dir).map_err(rejected_io("create fixture directory"))?;

    let artifact_bytes = accepted_lmc1_table_bytes();
    assert_verifier_accepts(&artifact_bytes).map_err(IcebergBindingReport::rejected)?;
    let artifact_sha256 = sha256_bytes(&artifact_bytes)
        .map_err(|error| IcebergBindingReport::unsupported(None, error))?;
    let source_bytes = b"demo.events source fixture\nsnapshot=314159\nschema=7\nrows=7,-1,42\n";
    let source_sha256 = sha256_bytes(source_bytes)
        .map_err(|error| IcebergBindingReport::unsupported(None, error))?;

    std::fs::write(&artifact_path, &artifact_bytes).map_err(rejected_io("write artifact"))?;
    std::fs::write(&source_path, source_bytes).map_err(rejected_io("write source fixture"))?;
    std::fs::write(&metadata_path, metadata_json()).map_err(rejected_io("write metadata"))?;
    std::fs::write(
        &evidence_path,
        evidence_json(&artifact_sha256, &source_sha256),
    )
    .map_err(rejected_io("write source/oracle evidence"))?;
    std::fs::write(
        &sidecar_path,
        sidecar_json(&artifact_path, &artifact_sha256, &evidence_path),
    )
    .map_err(rejected_io("write sidecar"))?;

    let accepted = bind_iceberg_ref_from_paths(&metadata_path, &sidecar_path, &artifact_path)?;
    Ok(AcceptedFixtureBundle {
        root_dir,
        metadata_path,
        sidecar_path,
        evidence_path,
        source_path,
        artifact_path,
        artifact_sha256,
        accepted,
    })
}

fn rejected_io(label: &'static str) -> impl FnOnce(std::io::Error) -> IcebergBindingReport {
    move |error| IcebergBindingReport::rejected(format!("{label}: {error}"))
}

fn accepted_lmc1_table_bytes() -> Vec<u8> {
    let table = TableDescription {
        row_count: ROW_VALUES.len(),
        columns: vec![TableColumn {
            name: "id".to_string(),
            layout: raw_i32_desc(&ROW_VALUES),
        }],
    };
    let payload = encode_table_payload(&table).expect("encode table payload");
    wrap_table_payload(&payload).expect("wrap table payload")
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

fn assert_verifier_accepts(bytes: &[u8]) -> Result<(), String> {
    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_artifact(bytes, &registry, &Default::default());
    if report.status() != ArtifactVerificationStatus::Accepted {
        return Err(format!(
            "fixture artifact verifier status was {}",
            report.status().as_str()
        ));
    }
    Ok(())
}

fn sha256_bytes(bytes: &[u8]) -> Result<String, String> {
    let mut child = Command::new("shasum")
        .args(["-a", "256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("shasum helper could not start: {error}"))?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| "shasum stdin unavailable".to_string())?
        .write_all(bytes)
        .map_err(|error| format!("bytes could not be written to shasum: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("shasum output unavailable: {error}"))?;
    if !output.status.success() {
        return Err(format!("shasum failed with status {}", output.status));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|error| format!("shasum output was not utf8: {error}"))?;
    text.split_whitespace()
        .next()
        .filter(|digest| digest.len() == 64)
        .map(ToOwned::to_owned)
        .ok_or_else(|| "shasum returned no SHA-256 digest".to_string())
}

fn metadata_json() -> String {
    format!(
        r#"{{
  "format-version": 2,
  "table-uuid": "{TABLE_UUID}",
  "location": "tests/fixtures/local/tables/demo/events",
  "current-schema-id": {SCHEMA_ID},
  "current-snapshot-id": {SNAPSHOT_ID},
  "snapshots": [
    {{
      "snapshot-id": {SNAPSHOT_ID},
      "schema-id": {SCHEMA_ID},
      "manifest-list": "tests/fixtures/local/metadata/snap-314159.avro"
    }}
  ],
  "refs": {{
    "main": {{
      "snapshot-id": {SNAPSHOT_ID},
      "type": "branch"
    }}
  }},
  "properties": {{
    "loom.table.name": "{TABLE_NAME}",
    "loom.metadata.location": "tests/fixtures/local/metadata/v1.metadata.json"
  }}
}}"#
    )
}

fn sidecar_json(artifact_path: &Path, artifact_sha256: &str, evidence_path: &Path) -> String {
    let artifact_ref = artifact_path.file_name().unwrap().to_string_lossy();
    let evidence_ref = evidence_path.file_name().unwrap().to_string_lossy();
    format!(
        r#"{{
  "table_uuid": "{TABLE_UUID}",
  "table_name": "{TABLE_NAME}",
  "schema_id": {SCHEMA_ID},
  "snapshot_id": {SNAPSHOT_ID},
  "ref_name": "main",
  "ref_type": "branch",
  "loom_artifact_path": "{artifact_ref}",
  "loom_artifact_sha256": "{artifact_sha256}",
  "source_oracle_evidence_path": "{evidence_ref}",
  "source_evidence": {{
    "accepted": true,
    "status": "accepted",
    "path": "source/demo-events.parquet"
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
}}"#
    )
}

fn evidence_json(artifact_sha256: &str, source_sha256: &str) -> String {
    format!(
        r#"{{
  "row_count": 3,
  "table_uuid": "{TABLE_UUID}",
  "schema_id": {SCHEMA_ID},
  "snapshot_id": {SNAPSHOT_ID},
  "artifact_sha256": "{artifact_sha256}",
  "source": {{
    "accepted": true,
    "status": "accepted",
    "path": "source/demo-events.parquet",
    "sha256": "{source_sha256}"
  }},
  "decoded_row_fixture": {{
    "identity": "demo.events#snapshot=314159#schema=7",
    "strategy": "decoded-row-fixture",
    "row_count": 3,
    "values_sha256": "{VALUES_SHA256}",
    "accepted": true,
    "oracle_accepted": true,
    "status": "accepted"
  }}
}}"#
    )
}
