//! Loom-owned Iceberg table/ref binding report contract.
//!
//! These types carry bounded Iceberg table/ref identity plus verifier,
//! source-ingress, and oracle evidence. They deliberately do not expose Iceberg
//! SDK objects, catalog handles, object-store credentials, DuckDB routes, CLI
//! routes, public C ABI symbols, or manifest mutation controls.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceCoverage, SourceDiagnostic, SourceDiagnosticCode,
    SourceEmissionDisposition, SourceEmissionKind, SourceFacts, SourceIdentity,
    SourceIngressReport, SourceIngressStatus, SourceLayoutFact, SourceLoweringDisposition,
    SourceOracleEvidence, SourceSchemaFact,
};
use serde::Deserialize;

/// High-level binding classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IcebergBindingStatus {
    Accepted,
    Unsupported,
    Rejected,
}

impl IcebergBindingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Unsupported => "unsupported",
            Self::Rejected => "rejected",
        }
    }
}

/// Bounded Iceberg table/ref identity facts used by the binding adapter.
///
/// The fields are descriptive until matched against a verifier-accepted Loom
/// artifact, source evidence, oracle evidence, and sidecar hash identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IcebergTableRefIdentity {
    pub table_uuid: String,
    pub table_name: String,
    pub snapshot_id: i64,
    pub schema_id: i32,
    pub metadata_location: String,
    pub manifest_list_location: Option<String>,
    pub ref_name: String,
    pub ref_type: String,
}

/// Binding facts extracted from local Iceberg metadata plus a Loom sidecar/ref.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IcebergBindingFacts {
    pub identity: IcebergTableRefIdentity,
    pub artifact_path: String,
    pub artifact_sha256: String,
}

/// Evidence required before an Iceberg binding can be accepted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IcebergBindingEvidence {
    pub artifact_verification: SourceArtifactVerificationSummary,
    pub source_report: SourceIngressReport,
    pub oracle_evidence: SourceOracleEvidence,
}

/// Reviewer-visible binding report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IcebergBindingReport {
    pub status: IcebergBindingStatus,
    pub facts: Option<IcebergBindingFacts>,
    pub diagnostics: Vec<String>,
    pub evidence: Option<IcebergBindingEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IcebergBindingReportError {
    MissingFacts,
    MissingArtifactBytes,
    ArtifactVerificationNotAccepted,
    SourceEvidenceNotAccepted,
    OracleEvidenceNotAccepted,
    TableIdentityMismatch,
    SnapshotMismatch,
    SchemaMismatch,
    ArtifactHashMismatch,
}

/// Verifier-accepted Iceberg-bound Loom artifact handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IcebergBindingAcceptedArtifact {
    pub bytes: Vec<u8>,
    pub report: IcebergBindingReport,
}

impl IcebergBindingReport {
    pub fn accepted(
        facts: Option<IcebergBindingFacts>,
        evidence: IcebergBindingEvidence,
        table_identity_matched: bool,
        snapshot_matched: bool,
        schema_matched: bool,
        artifact_hash_matched: bool,
    ) -> Result<Self, IcebergBindingReportError> {
        let facts = facts.ok_or(IcebergBindingReportError::MissingFacts)?;

        let artifact_len = evidence.artifact_verification.artifact_byte_len;
        if !evidence.artifact_verification.required
            || !evidence.artifact_verification.accepted
            || artifact_len.is_none()
        {
            return Err(IcebergBindingReportError::ArtifactVerificationNotAccepted);
        }
        if artifact_len == Some(0) {
            return Err(IcebergBindingReportError::MissingArtifactBytes);
        }
        if evidence.source_report.status != SourceIngressStatus::Accepted {
            return Err(IcebergBindingReportError::SourceEvidenceNotAccepted);
        }
        if !evidence.oracle_evidence.accepted {
            return Err(IcebergBindingReportError::OracleEvidenceNotAccepted);
        }
        if !table_identity_matched {
            return Err(IcebergBindingReportError::TableIdentityMismatch);
        }
        if !snapshot_matched {
            return Err(IcebergBindingReportError::SnapshotMismatch);
        }
        if !schema_matched {
            return Err(IcebergBindingReportError::SchemaMismatch);
        }
        if !artifact_hash_matched {
            return Err(IcebergBindingReportError::ArtifactHashMismatch);
        }

        Ok(Self {
            status: IcebergBindingStatus::Accepted,
            facts: Some(facts),
            diagnostics: Vec::new(),
            evidence: Some(evidence),
        })
    }

    pub fn unsupported(facts: Option<IcebergBindingFacts>, diagnostic: impl Into<String>) -> Self {
        Self {
            status: IcebergBindingStatus::Unsupported,
            facts,
            diagnostics: vec![diagnostic.into()],
            evidence: None,
        }
    }

    pub fn rejected(diagnostic: impl Into<String>) -> Self {
        Self {
            status: IcebergBindingStatus::Rejected,
            facts: None,
            diagnostics: vec![diagnostic.into()],
            evidence: None,
        }
    }
}

/// Extract descriptive Iceberg table/ref facts from local metadata plus a Loom
/// sidecar. This parser does not verify artifact bytes or construct accepted
/// binding reports; Plan 28-03 owns that trust decision.
pub fn iceberg_binding_facts_from_paths(
    metadata_path: &Path,
    sidecar_path: &Path,
) -> Result<IcebergBindingFacts, IcebergBindingReport> {
    let metadata = read_metadata_for_binding(metadata_path)?;
    let sidecar = read_sidecar(sidecar_path)?;
    let identity = table_ref_identity_from_metadata(metadata_path, &metadata)?;

    validate_sidecar_identity(&identity, &sidecar)?;

    let facts = IcebergBindingFacts {
        identity,
        artifact_path: required_sidecar_text(
            sidecar.loom_artifact_path.clone(),
            "$.loom_artifact_path",
            "Loom artifact path is required",
        )?,
        artifact_sha256: required_sidecar_text(
            sidecar.loom_artifact_sha256.clone(),
            "$.loom_artifact_sha256",
            "Loom artifact SHA-256 is required",
        )?,
    };

    if let Some(marker) = local_policy_marker_for_binding(&metadata, &sidecar) {
        return Err(IcebergBindingReport::unsupported(
            Some(facts),
            format!("remote or catalog path/control is unsupported in Plan 28-02: {marker}"),
        ));
    }

    Ok(facts)
}

/// Build a byte-free source-ingress style report from local Iceberg metadata.
pub fn source_ingress_report_from_iceberg_metadata_path(path: &Path) -> SourceIngressReport {
    match read_metadata_for_source(path)
        .and_then(|metadata| source_facts_from_metadata(path, &metadata))
    {
        Ok((facts, unsupported_marker)) => {
            let diagnostic = if let Some(marker) = unsupported_marker {
                SourceDiagnostic::new(
                    SourceDiagnosticCode::UnsupportedConversion,
                    "$.location",
                    format!(
                        "remote or catalog path/control is unsupported in Plan 28-02: {marker}"
                    ),
                )
            } else {
                SourceDiagnostic::new(
                    SourceDiagnosticCode::UnsupportedConversion,
                    "$.binding",
                    "valid Iceberg metadata is descriptive only until verifier-backed binding acceptance",
                )
            };
            SourceIngressReport::unsupported(Some(facts), diagnostic)
        }
        Err(report) => report,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LocalIcebergMetadata {
    format_version: Option<u8>,
    table_uuid: Option<String>,
    location: Option<String>,
    current_schema_id: Option<i32>,
    current_snapshot_id: Option<i64>,
    #[serde(default)]
    snapshots: Vec<LocalSnapshot>,
    #[serde(default)]
    refs: BTreeMap<String, LocalSnapshotRef>,
    #[serde(default)]
    properties: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LocalSnapshot {
    snapshot_id: i64,
    manifest_list: Option<String>,
    schema_id: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LocalSnapshotRef {
    snapshot_id: i64,
    #[serde(rename = "type")]
    ref_type: String,
}

#[derive(Debug, Deserialize)]
struct LoomBindingSidecar {
    table_uuid: Option<String>,
    table_name: Option<String>,
    schema_id: Option<i32>,
    snapshot_id: Option<i64>,
    ref_name: Option<String>,
    ref_type: Option<String>,
    loom_artifact_path: Option<String>,
    loom_artifact_sha256: Option<String>,
    source_evidence: Option<SidecarEvidence>,
    verifier_evidence: Option<SidecarEvidence>,
    oracle_evidence: Option<SidecarEvidence>,
}

#[derive(Debug, Deserialize)]
struct SidecarEvidence {
    status: Option<String>,
    path: Option<String>,
    summary: Option<String>,
    strategy: Option<String>,
}

fn read_metadata_for_binding(path: &Path) -> Result<LocalIcebergMetadata, IcebergBindingReport> {
    let text = fs::read_to_string(path).map_err(|error| {
        IcebergBindingReport::rejected(format!(
            "local Iceberg metadata could not be opened: {error}"
        ))
    })?;
    serde_json::from_str(&text).map_err(|error| {
        IcebergBindingReport::rejected(format!(
            "local Iceberg metadata could not be parsed: {error}"
        ))
    })
}

fn read_metadata_for_source(path: &Path) -> Result<LocalIcebergMetadata, SourceIngressReport> {
    let identity = SourceIdentity::new("iceberg-binding", "external-source")
        .with_path_display(path.display().to_string());
    let text = fs::read_to_string(path).map_err(|error| {
        SourceIngressReport::rejected(
            identity.clone(),
            SourceDiagnostic::new(
                SourceDiagnosticCode::OpenFailed,
                "$.open",
                "local Iceberg metadata could not be opened",
            )
            .with_source_detail(error.to_string()),
        )
    })?;
    serde_json::from_str(&text).map_err(|error| {
        SourceIngressReport::rejected(
            identity,
            SourceDiagnostic::new(
                SourceDiagnosticCode::ReadFailed,
                "$.metadata",
                "local Iceberg metadata could not be parsed",
            )
            .with_source_detail(error.to_string()),
        )
    })
}

fn read_sidecar(path: &Path) -> Result<LoomBindingSidecar, IcebergBindingReport> {
    let text = fs::read_to_string(path).map_err(|error| {
        IcebergBindingReport::rejected(format!("Loom binding sidecar could not be opened: {error}"))
    })?;
    serde_json::from_str(&text).map_err(|error| {
        IcebergBindingReport::rejected(format!("Loom binding sidecar could not be parsed: {error}"))
    })
}

fn table_ref_identity_from_metadata(
    metadata_path: &Path,
    metadata: &LocalIcebergMetadata,
) -> Result<IcebergTableRefIdentity, IcebergBindingReport> {
    let table_uuid = required_metadata_text(
        metadata.table_uuid.as_deref(),
        "$.table-uuid",
        "Iceberg table UUID is required",
    )?;
    let schema_id = metadata
        .current_schema_id
        .ok_or_else(|| IcebergBindingReport::rejected("Iceberg current schema ID is required"))?;
    let snapshot_id = metadata
        .current_snapshot_id
        .ok_or_else(|| IcebergBindingReport::rejected("Iceberg current snapshot ID is required"))?;
    let snapshot = metadata.snapshots.iter().find(|snapshot| {
        snapshot.snapshot_id == snapshot_id && snapshot.schema_id.unwrap_or(schema_id) == schema_id
    });
    let snapshot = snapshot.ok_or_else(|| {
        IcebergBindingReport::rejected(
            "Iceberg current snapshot and schema ID must identify a snapshot",
        )
    })?;
    let (ref_name, snapshot_ref) = metadata
        .refs
        .iter()
        .find(|(_, snapshot_ref)| snapshot_ref.snapshot_id == snapshot_id)
        .ok_or_else(|| {
            IcebergBindingReport::rejected(
                "Iceberg current snapshot ID must be reachable through a table ref",
            )
        })?;

    Ok(IcebergTableRefIdentity {
        table_uuid,
        table_name: table_name(metadata)?,
        snapshot_id,
        schema_id,
        metadata_location: metadata_location(metadata_path, metadata),
        manifest_list_location: snapshot.manifest_list.clone(),
        ref_name: ref_name.clone(),
        ref_type: snapshot_ref.ref_type.clone(),
    })
}

fn source_facts_from_metadata(
    path: &Path,
    metadata: &LocalIcebergMetadata,
) -> Result<(SourceFacts, Option<String>), SourceIngressReport> {
    let binding_identity = table_ref_identity_from_metadata(path, metadata).map_err(|report| {
        SourceIngressReport::rejected(
            SourceIdentity::new("iceberg-binding", "external-source")
                .with_path_display(path.display().to_string()),
            SourceDiagnostic::new(
                SourceDiagnosticCode::SchemaUnavailable,
                "$.identity",
                report
                    .diagnostics
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Iceberg table identity is incomplete".to_string()),
            ),
        )
    })?;

    let mut facts = SourceFacts::new(
        SourceIdentity::new("iceberg-binding", "external-source")
            .with_format_version(
                metadata
                    .format_version
                    .map(|version| version.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            )
            .with_fingerprint(binding_identity.table_uuid.clone())
            .with_path_display(path.display().to_string()),
        0,
    );

    let mut root_schema = SourceSchemaFact::new("$.schema", "iceberg-table");
    root_schema.field_count = Some(0);
    root_schema.field_names = vec![binding_identity.table_name.clone()];
    root_schema.arrow_summary = Some(format!("schema-id={}", binding_identity.schema_id));
    facts.root_schema = Some(root_schema.clone());
    facts.schema_facts.push(root_schema);

    let mut metadata_layout = SourceLayoutFact::new("$.metadata", "iceberg-table-metadata");
    metadata_layout.child_count = metadata.snapshots.len();
    metadata_layout.child_names = metadata
        .snapshots
        .iter()
        .map(|snapshot| format!("snapshot:{}", snapshot.snapshot_id))
        .collect();
    metadata_layout.physical_refs = vec![
        format!("table_uuid={}", binding_identity.table_uuid),
        format!("snapshot_id={}", binding_identity.snapshot_id),
        format!("schema_id={}", binding_identity.schema_id),
        format!(
            "ref={}:{}",
            binding_identity.ref_name, binding_identity.ref_type
        ),
    ];
    if let Some(location) = &binding_identity.manifest_list_location {
        metadata_layout
            .physical_refs
            .push(format!("manifest_list={location}"));
    }
    facts.layout_facts.push(metadata_layout);

    let mut coverage =
        SourceCoverage::new("iceberg-table", "metadata-reference", "sidecar-reference");
    coverage.support = SourceIngressStatus::Unsupported;
    coverage.emission_kind = SourceEmissionKind::None;
    coverage.emission_disposition = SourceEmissionDisposition::None;
    coverage.lowering_disposition = SourceLoweringDisposition::FailClosedDeferred;
    coverage.notes.push(
        "Iceberg metadata facts are descriptive until Plan 28-03 verifier binding".to_string(),
    );
    facts.coverage = Some(coverage);

    let unsupported_marker = local_policy_marker_for_metadata(metadata);
    Ok((facts, unsupported_marker))
}

fn validate_sidecar_identity(
    identity: &IcebergTableRefIdentity,
    sidecar: &LoomBindingSidecar,
) -> Result<(), IcebergBindingReport> {
    let table_uuid = required_sidecar_text(
        sidecar.table_uuid.clone(),
        "$.table_uuid",
        "sidecar table UUID is required",
    )?;
    let table_name = required_sidecar_text(
        sidecar.table_name.clone(),
        "$.table_name",
        "sidecar table name is required",
    )?;
    let schema_id = sidecar
        .schema_id
        .ok_or_else(|| IcebergBindingReport::rejected("sidecar schema ID is required"))?;
    let snapshot_id = sidecar
        .snapshot_id
        .ok_or_else(|| IcebergBindingReport::rejected("sidecar snapshot ID is required"))?;
    let ref_name = required_sidecar_text(
        sidecar.ref_name.clone(),
        "$.ref_name",
        "sidecar ref name is required",
    )?;
    let ref_type = required_sidecar_text(
        sidecar.ref_type.clone(),
        "$.ref_type",
        "sidecar ref type is required",
    )?;

    if table_uuid != identity.table_uuid
        || table_name != identity.table_name
        || schema_id != identity.schema_id
        || snapshot_id != identity.snapshot_id
        || ref_name != identity.ref_name
        || ref_type != identity.ref_type
    {
        return Err(IcebergBindingReport::rejected(
            "sidecar identity does not match Iceberg table/ref metadata",
        ));
    }

    Ok(())
}

fn table_name(metadata: &LocalIcebergMetadata) -> Result<String, IcebergBindingReport> {
    required_metadata_text(
        metadata
            .properties
            .get("loom.table.name")
            .map(String::as_str),
        "$.properties.loom.table.name",
        "Iceberg Loom table name property is required",
    )
}

fn metadata_location(metadata_path: &Path, metadata: &LocalIcebergMetadata) -> String {
    metadata
        .properties
        .get("loom.metadata.location")
        .cloned()
        .unwrap_or_else(|| metadata_path.display().to_string())
}

fn required_metadata_text(
    value: Option<&str>,
    _path: &str,
    diagnostic: &str,
) -> Result<String, IcebergBindingReport> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| IcebergBindingReport::rejected(diagnostic))
}

fn required_sidecar_text(
    value: Option<String>,
    _path: &str,
    diagnostic: &str,
) -> Result<String, IcebergBindingReport> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| IcebergBindingReport::rejected(diagnostic))
}

fn local_policy_marker_for_binding(
    metadata: &LocalIcebergMetadata,
    sidecar: &LoomBindingSidecar,
) -> Option<String> {
    local_policy_marker_for_metadata(metadata).or_else(|| {
        sidecar
            .loom_artifact_path
            .as_deref()
            .and_then(forbidden_local_marker)
            .or_else(|| sidecar_evidence_marker(sidecar.source_evidence.as_ref()))
            .or_else(|| sidecar_evidence_marker(sidecar.verifier_evidence.as_ref()))
            .or_else(|| sidecar_evidence_marker(sidecar.oracle_evidence.as_ref()))
    })
}

fn local_policy_marker_for_metadata(metadata: &LocalIcebergMetadata) -> Option<String> {
    metadata
        .location
        .as_deref()
        .and_then(forbidden_local_marker)
        .or_else(|| {
            metadata
                .snapshots
                .iter()
                .filter_map(|snapshot| snapshot.manifest_list.as_deref())
                .find_map(forbidden_local_marker)
        })
        .or_else(|| {
            metadata
                .properties
                .values()
                .find_map(|value| forbidden_local_marker(value))
        })
}

fn sidecar_evidence_marker(evidence: Option<&SidecarEvidence>) -> Option<String> {
    let evidence = evidence?;
    evidence
        .path
        .as_deref()
        .and_then(forbidden_local_marker)
        .or_else(|| evidence.summary.as_deref().and_then(forbidden_local_marker))
        .or_else(|| evidence.status.as_deref().and_then(forbidden_local_marker))
        .or_else(|| {
            evidence
                .strategy
                .as_deref()
                .and_then(forbidden_local_marker)
        })
}

fn forbidden_local_marker(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    [
        "://",
        "s3:",
        "gs:",
        "abfs:",
        "warehouse",
        "credential",
        "secret",
        "rest",
        "catalog",
    ]
    .iter()
    .find(|marker| lower.contains(**marker))
    .map(|marker| format!("{value} ({marker})"))
}
