//! Loom-owned Iceberg table/ref binding report contract.
//!
//! These types carry bounded Iceberg table/ref identity plus verifier,
//! source-ingress, and oracle evidence. They deliberately do not expose Iceberg
//! SDK objects, catalog handles, object-store credentials, DuckDB routes, CLI
//! routes, public C ABI symbols, or manifest mutation controls.

use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceIngressReport, SourceIngressStatus,
    SourceOracleEvidence,
};

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
