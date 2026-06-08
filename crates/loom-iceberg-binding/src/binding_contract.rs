//! Loom-owned Iceberg table/ref binding report contract.
//!
//! These types carry bounded Iceberg table/ref identity plus verifier,
//! source-ingress, and oracle evidence. They deliberately do not expose Iceberg
//! SDK objects, catalog handles, object-store credentials, DuckDB routes, CLI
//! routes, public C ABI symbols, or manifest mutation controls.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceCoverage, SourceDiagnostic, SourceDiagnosticCode,
    SourceEmissionDisposition, SourceEmissionKind, SourceFacts, SourceIdentity,
    SourceIngressReport, SourceIngressStatus, SourceLayoutFact, SourceLoweringDisposition,
    SourceOracleEvidence, SourceOracleStrategy, SourceSchemaFact,
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

/// Bind a local Iceberg table/ref sidecar to verifier-accepted Loom artifact bytes.
///
/// Acceptance requires independent local evidence: the function reads the
/// artifact bytes, recomputes SHA-256 with `shasum`, runs `verify_artifact`,
/// and reads the concrete source/oracle evidence JSON referenced by the
/// sidecar. Sidecar accepted flags are necessary descriptive inputs only.
pub fn bind_iceberg_ref_from_paths(
    metadata_path: &Path,
    sidecar_path: &Path,
    artifact_path: &Path,
) -> Result<IcebergBindingAcceptedArtifact, IcebergBindingReport> {
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
            format!("remote or catalog path/control is unsupported in accepted binding: {marker}"),
        ));
    }

    if !artifact_path_matches(&facts.artifact_path, artifact_path) {
        return Err(IcebergBindingReport::unsupported(
            Some(facts),
            "explicit artifact path does not match sidecar Loom artifact path",
        ));
    }

    require_sidecar_evidence_accepted(
        sidecar.verifier_evidence.as_ref(),
        "sidecar verifier evidence accepted status is required",
    )?;
    require_sidecar_evidence_accepted(
        sidecar.source_evidence.as_ref(),
        "sidecar source evidence accepted status is required",
    )?;
    require_sidecar_evidence_accepted(
        sidecar.oracle_evidence.as_ref(),
        "sidecar oracle evidence accepted status is required",
    )?;

    let artifact_bytes = fs::read(artifact_path).map_err(|error| {
        IcebergBindingReport::rejected(format!(
            "referenced Loom artifact bytes could not be opened: {error}"
        ))
    })?;
    if artifact_bytes.is_empty() {
        return Err(IcebergBindingReport::unsupported(
            Some(facts),
            "referenced Loom artifact is empty",
        ));
    }

    let actual_sha256 = sha256_bytes(&artifact_bytes)
        .map_err(|diagnostic| IcebergBindingReport::unsupported(Some(facts.clone()), diagnostic))?;
    if actual_sha256 != facts.artifact_sha256 {
        return Err(IcebergBindingReport::unsupported(
            Some(facts),
            "recomputed artifact SHA-256 does not match sidecar hash",
        ));
    }

    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_artifact(&artifact_bytes, &registry, &Default::default());
    if verification.status() != ArtifactVerificationStatus::Accepted {
        return Err(IcebergBindingReport::unsupported(
            Some(facts),
            format!(
                "referenced Loom artifact verifier status was {}",
                verification.status().as_str()
            ),
        ));
    }
    let verifier_facts = verification.facts().ok_or_else(|| {
        IcebergBindingReport::unsupported(
            Some(facts.clone()),
            "accepted verifier report did not expose facts",
        )
    })?;
    let payload_kind = verifier_facts
        .payload_kind
        .as_deref()
        .unwrap_or("unknown payload");
    let artifact_verification = SourceArtifactVerificationSummary::accepted(
        artifact_bytes.len(),
        format!(
            "{} verifier accepted {}",
            verifier_facts.artifact_kind, payload_kind
        ),
    );

    let evidence_path = required_sidecar_text(
        sidecar.source_oracle_evidence_path.clone(),
        "$.source_oracle_evidence_path",
        "sidecar source/oracle evidence artifact path is required",
    )?;
    let evidence_path = resolve_sidecar_relative_path(sidecar_path, &evidence_path);
    let evidence = read_source_oracle_evidence(&evidence_path)
        .map_err(|diagnostic| IcebergBindingReport::unsupported(Some(facts.clone()), diagnostic))?;
    validate_source_oracle_evidence(
        &facts,
        &evidence,
        &actual_sha256,
        SourceOracleStrategy::DecodedRowFixture,
    )?;

    let oracle_evidence =
        SourceOracleEvidence::accepted(SourceOracleStrategy::DecodedRowFixture, evidence.row_count);
    let source_facts =
        accepted_source_facts_from_binding(metadata_path, &metadata, &facts, evidence.row_count);
    let emission_kind = if payload_kind.contains("LMT1") {
        SourceEmissionKind::Lmt1
    } else {
        SourceEmissionKind::Lmp1
    };
    let emission_disposition = if emission_kind == SourceEmissionKind::Lmt1 {
        SourceEmissionDisposition::CanonicalTable
    } else {
        SourceEmissionDisposition::CanonicalRaw
    };
    let source_report = SourceIngressReport::accepted(
        source_facts,
        emission_kind,
        emission_disposition,
        SourceLoweringDisposition::ProductionLoweringSupported,
        artifact_verification.clone(),
        oracle_evidence.clone(),
    )
    .map_err(|error| {
        IcebergBindingReport::unsupported(
            Some(facts.clone()),
            format!("accepted source report could not be constructed: {error:?}"),
        )
    })?;

    let binding_evidence = IcebergBindingEvidence {
        artifact_verification,
        source_report,
        oracle_evidence,
    };
    let report =
        IcebergBindingReport::accepted(Some(facts), binding_evidence, true, true, true, true)
            .map_err(|error| {
                IcebergBindingReport::unsupported(
                    None,
                    format!("accepted binding report could not be constructed: {error:?}"),
                )
            })?;

    Ok(IcebergBindingAcceptedArtifact {
        bytes: artifact_bytes,
        report,
    })
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
    source_oracle_evidence_path: Option<String>,
    source_evidence: Option<SidecarEvidence>,
    verifier_evidence: Option<SidecarEvidence>,
    oracle_evidence: Option<SidecarEvidence>,
}

#[derive(Debug, Deserialize)]
struct SidecarEvidence {
    accepted: Option<bool>,
    status: Option<String>,
    path: Option<String>,
    summary: Option<String>,
    strategy: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SourceOracleEvidenceArtifact {
    row_count: u64,
    table_uuid: String,
    schema_id: i32,
    snapshot_id: i64,
    artifact_sha256: String,
    source: EvidenceStatus,
    decoded_row_fixture: DecodedRowFixtureEvidence,
}

#[derive(Debug, Deserialize)]
struct EvidenceStatus {
    accepted: bool,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DecodedRowFixtureEvidence {
    identity: String,
    strategy: String,
    row_count: u64,
    accepted: bool,
    oracle_accepted: bool,
    status: Option<String>,
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

fn accepted_source_facts_from_binding(
    metadata_path: &Path,
    metadata: &LocalIcebergMetadata,
    facts: &IcebergBindingFacts,
    row_count: u64,
) -> SourceFacts {
    let mut source_facts = SourceFacts::new(
        SourceIdentity::new("iceberg-binding", "external-source")
            .with_format_version(
                metadata
                    .format_version
                    .map(|version| version.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            )
            .with_fingerprint(facts.identity.table_uuid.clone())
            .with_path_display(metadata_path.display().to_string()),
        row_count,
    );

    let mut root_schema = SourceSchemaFact::new("$.schema", "iceberg-table");
    root_schema.field_count = Some(1);
    root_schema.field_names = vec![facts.identity.table_name.clone()];
    root_schema.arrow_summary = Some(format!("schema-id={}", facts.identity.schema_id));
    source_facts.root_schema = Some(root_schema.clone());
    source_facts.schema_facts.push(root_schema);

    let mut metadata_layout = SourceLayoutFact::new("$.metadata", "iceberg-table-binding");
    metadata_layout.child_count = 1;
    metadata_layout.child_names = vec![facts.identity.ref_name.clone()];
    metadata_layout.physical_refs = vec![
        format!("table_uuid={}", facts.identity.table_uuid),
        format!("schema_id={}", facts.identity.schema_id),
        format!("snapshot_id={}", facts.identity.snapshot_id),
        format!("artifact_sha256={}", facts.artifact_sha256),
    ];
    if let Some(manifest_list) = &facts.identity.manifest_list_location {
        metadata_layout
            .physical_refs
            .push(format!("manifest_list={manifest_list}"));
    }
    source_facts.layout_facts.push(metadata_layout);

    let mut coverage =
        SourceCoverage::new("iceberg-table", "metadata-reference", "sidecar-reference");
    coverage.support = SourceIngressStatus::Accepted;
    coverage.emission_kind = SourceEmissionKind::Lmt1;
    coverage.emission_disposition = SourceEmissionDisposition::CanonicalTable;
    coverage.lowering_disposition = SourceLoweringDisposition::ProductionLoweringSupported;
    coverage.notes.push(
        "accepted only after local artifact hash, verifier, and decoded-row fixture evidence matched"
            .to_string(),
    );
    source_facts.coverage = Some(coverage);

    source_facts
}

fn artifact_path_matches(sidecar_artifact_path: &str, artifact_path: &Path) -> bool {
    if sidecar_artifact_path == artifact_path.display().to_string() {
        return true;
    }

    let sidecar_path = Path::new(sidecar_artifact_path);
    match (sidecar_path.canonicalize(), artifact_path.canonicalize()) {
        (Ok(sidecar_path), Ok(argument_path)) => sidecar_path == argument_path,
        _ => false,
    }
}

fn require_sidecar_evidence_accepted(
    evidence: Option<&SidecarEvidence>,
    diagnostic: &str,
) -> Result<(), IcebergBindingReport> {
    let evidence = evidence.ok_or_else(|| IcebergBindingReport::unsupported(None, diagnostic))?;
    if evidence.accepted == Some(true) && evidence.status.as_deref() == Some("accepted") {
        Ok(())
    } else {
        Err(IcebergBindingReport::unsupported(None, diagnostic))
    }
}

fn sha256_bytes(bytes: &[u8]) -> Result<String, String> {
    let mut child = Command::new("shasum")
        .args(["-a", "256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("shasum SHA-256 helper could not be started: {error}"))?;

    child
        .stdin
        .as_mut()
        .ok_or_else(|| "shasum SHA-256 helper stdin was unavailable".to_string())?
        .write_all(bytes)
        .map_err(|error| format!("artifact bytes could not be written to shasum: {error}"))?;

    let output = child
        .wait_with_output()
        .map_err(|error| format!("shasum SHA-256 helper output could not be read: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "shasum SHA-256 helper failed with status {}",
            output.status
        ));
    }

    let output = String::from_utf8(output.stdout)
        .map_err(|error| format!("shasum SHA-256 helper output was not UTF-8: {error}"))?;
    let digest = output
        .split_whitespace()
        .next()
        .ok_or_else(|| "shasum SHA-256 helper returned no digest".to_string())?;
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "shasum SHA-256 helper returned invalid digest: {digest}"
        ));
    }

    Ok(digest.to_ascii_lowercase())
}

fn resolve_sidecar_relative_path(sidecar_path: &Path, referenced_path: &str) -> PathBuf {
    let referenced = Path::new(referenced_path);
    if referenced.is_absolute() {
        return referenced.to_path_buf();
    }

    if let Some(parent) = sidecar_path.parent() {
        let sibling = parent.join(referenced);
        if sibling.exists() {
            return sibling;
        }
    }

    PathBuf::from(referenced)
}

fn read_source_oracle_evidence(path: &Path) -> Result<SourceOracleEvidenceArtifact, String> {
    let text = fs::read_to_string(path).map_err(|error| {
        format!("sidecar-referenced source/oracle evidence artifact could not be opened: {error}")
    })?;
    serde_json::from_str(&text).map_err(|error| {
        format!("sidecar-referenced source/oracle evidence artifact could not be parsed: {error}")
    })
}

fn validate_source_oracle_evidence(
    facts: &IcebergBindingFacts,
    evidence: &SourceOracleEvidenceArtifact,
    artifact_sha256: &str,
    expected_strategy: SourceOracleStrategy,
) -> Result<(), IcebergBindingReport> {
    let decoded = &evidence.decoded_row_fixture;
    let expected_identity = format!(
        "{}#snapshot={}#schema={}",
        facts.identity.table_name, facts.identity.snapshot_id, facts.identity.schema_id
    );

    if evidence.table_uuid != facts.identity.table_uuid {
        return Err(IcebergBindingReport::unsupported(
            Some(facts.clone()),
            "source/oracle evidence table UUID does not match Iceberg metadata",
        ));
    }
    if evidence.schema_id != facts.identity.schema_id {
        return Err(IcebergBindingReport::unsupported(
            Some(facts.clone()),
            "source/oracle evidence schema ID does not match Iceberg metadata",
        ));
    }
    if evidence.snapshot_id != facts.identity.snapshot_id {
        return Err(IcebergBindingReport::unsupported(
            Some(facts.clone()),
            "source/oracle evidence snapshot ID does not match Iceberg metadata",
        ));
    }
    if evidence.artifact_sha256 != artifact_sha256
        || evidence.artifact_sha256 != facts.artifact_sha256
    {
        return Err(IcebergBindingReport::unsupported(
            Some(facts.clone()),
            "source/oracle evidence artifact SHA-256 does not match recomputed artifact hash",
        ));
    }
    if !evidence.source.accepted || evidence.source.status.as_deref() != Some("accepted") {
        return Err(IcebergBindingReport::unsupported(
            Some(facts.clone()),
            "source evidence artifact status is not accepted",
        ));
    }
    if decoded.strategy != expected_strategy.as_str() {
        return Err(IcebergBindingReport::unsupported(
            Some(facts.clone()),
            "decoded-row fixture evidence strategy does not match expected oracle strategy",
        ));
    }
    if decoded.identity != expected_identity {
        return Err(IcebergBindingReport::unsupported(
            Some(facts.clone()),
            "decoded-row fixture evidence identity does not match table/ref identity",
        ));
    }
    if decoded.row_count != evidence.row_count {
        return Err(IcebergBindingReport::unsupported(
            Some(facts.clone()),
            "decoded-row fixture row count does not match source evidence row count",
        ));
    }
    if !decoded.accepted
        || !decoded.oracle_accepted
        || decoded.status.as_deref() != Some("accepted")
    {
        return Err(IcebergBindingReport::unsupported(
            Some(facts.clone()),
            "decoded-row fixture oracle evidence status is not accepted",
        ));
    }

    Ok(())
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
