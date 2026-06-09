//! Source-neutral external-source ingress contract for Loom.
//!
//! This crate intentionally owns only Loom contract vocabulary. Source-specific
//! SDKs and artifact verifier implementations stay in adapter crates.

/// High-level source ingress classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceIngressStatus {
    Accepted,
    Unsupported,
    Rejected,
}

impl SourceIngressStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Unsupported => "unsupported",
            Self::Rejected => "rejected",
        }
    }
}

/// Loom-owned source identity facts. These fields are descriptive only and do
/// not imply a trusted artifact handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIdentity {
    pub source_kind: String,
    pub format: String,
    pub format_version: Option<String>,
    pub fingerprint: Option<String>,
    pub path_display: Option<String>,
}

impl SourceIdentity {
    pub fn new(source_kind: impl Into<String>, format: impl Into<String>) -> Self {
        Self {
            source_kind: source_kind.into(),
            format: format.into(),
            format_version: None,
            fingerprint: None,
            path_display: None,
        }
    }

    pub fn with_format_version(mut self, format_version: impl Into<String>) -> Self {
        self.format_version = Some(format_version.into());
        self
    }

    pub fn with_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.fingerprint = Some(fingerprint.into());
        self
    }

    pub fn with_path_display(mut self, path_display: impl Into<String>) -> Self {
        self.path_display = Some(path_display.into());
        self
    }
}

/// Stable diagnostic code vocabulary for generic source ingress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceDiagnosticCode {
    NotYetInspected,
    OpenFailed,
    ReadFailed,
    SchemaUnavailable,
    LayoutUnavailable,
    SplitUnavailable,
    UnsupportedSchema,
    UnsupportedLayout,
    UnsupportedConversion,
    VerificationFailed,
    OracleUnavailable,
}

/// Coarse diagnostic grouping for reviewer-visible reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceDiagnosticFamily {
    Open,
    Read,
    Schema,
    Layout,
    Support,
    Conversion,
    Verification,
    Oracle,
}

/// Reviewer-visible source ingress diagnostic. Adapter-private details may be
/// recorded as text, but handles, credentials, and SDK objects must stay out of
/// this contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDiagnostic {
    pub code: SourceDiagnosticCode,
    pub family: SourceDiagnosticFamily,
    pub path: String,
    pub message: String,
    pub source_detail: Option<String>,
}

impl SourceDiagnostic {
    pub fn new(
        code: SourceDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            family: code.family(),
            code,
            path: path.into(),
            message: message.into(),
            source_detail: None,
        }
    }

    pub fn with_source_detail(mut self, source_detail: impl Into<String>) -> Self {
        self.source_detail = Some(source_detail.into());
        self
    }
}

impl SourceDiagnosticCode {
    pub fn family(self) -> SourceDiagnosticFamily {
        match self {
            Self::NotYetInspected | Self::OpenFailed => SourceDiagnosticFamily::Open,
            Self::ReadFailed => SourceDiagnosticFamily::Read,
            Self::SchemaUnavailable | Self::UnsupportedSchema => SourceDiagnosticFamily::Schema,
            Self::LayoutUnavailable | Self::SplitUnavailable | Self::UnsupportedLayout => {
                SourceDiagnosticFamily::Layout
            }
            Self::UnsupportedConversion => SourceDiagnosticFamily::Conversion,
            Self::VerificationFailed => SourceDiagnosticFamily::Verification,
            Self::OracleUnavailable => SourceDiagnosticFamily::Oracle,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSchemaFact {
    pub path: String,
    pub logical_kind: String,
    pub nullable: Option<bool>,
    pub field_count: Option<usize>,
    pub field_names: Vec<String>,
    pub arrow_summary: Option<String>,
}

impl SourceSchemaFact {
    pub fn new(path: impl Into<String>, logical_kind: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            logical_kind: logical_kind.into(),
            nullable: None,
            field_count: None,
            field_names: Vec::new(),
            arrow_summary: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceLayoutFact {
    pub path: String,
    pub layout_class: String,
    pub row_count: Option<u64>,
    pub child_count: usize,
    pub child_names: Vec<String>,
    pub physical_refs: Vec<String>,
    pub metadata_byte_len: Option<usize>,
}

impl SourceLayoutFact {
    pub fn new(path: impl Into<String>, layout_class: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            layout_class: layout_class.into(),
            row_count: None,
            child_count: 0,
            child_names: Vec::new(),
            physical_refs: Vec::new(),
            metadata_byte_len: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSegmentFact {
    pub index: usize,
    pub start: u64,
    pub end: u64,
    pub length: u64,
    pub alignment: Option<String>,
    pub ordered_after_previous: bool,
    pub overlaps_previous: bool,
}

impl SourceSegmentFact {
    pub fn new(index: usize, start: u64, end: u64) -> Self {
        Self {
            index,
            start,
            end,
            length: end.saturating_sub(start),
            alignment: None,
            ordered_after_previous: true,
            overlaps_previous: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSplitFact {
    pub index: usize,
    pub start_row: u64,
    pub end_row: u64,
    pub row_count: u64,
}

impl SourceSplitFact {
    pub fn new(index: usize, start_row: u64, end_row: u64) -> Self {
        Self {
            index,
            start_row,
            end_row,
            row_count: end_row.saturating_sub(start_row),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCoverage {
    pub schema_family: String,
    pub nullability: Option<bool>,
    pub layout_class: String,
    pub array_encoding: String,
    pub has_splits: bool,
    pub has_statistics: bool,
    pub support: SourceIngressStatus,
    pub emission_kind: SourceEmissionKind,
    pub emission_disposition: SourceEmissionDisposition,
    pub lowering_disposition: SourceLoweringDisposition,
    pub notes: Vec<String>,
}

impl SourceCoverage {
    pub fn new(
        schema_family: impl Into<String>,
        layout_class: impl Into<String>,
        array_encoding: impl Into<String>,
    ) -> Self {
        Self {
            schema_family: schema_family.into(),
            nullability: None,
            layout_class: layout_class.into(),
            array_encoding: array_encoding.into(),
            has_splits: false,
            has_statistics: false,
            support: SourceIngressStatus::Unsupported,
            emission_kind: SourceEmissionKind::None,
            emission_disposition: SourceEmissionDisposition::None,
            lowering_disposition: SourceLoweringDisposition::FailClosedDeferred,
            notes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFacts {
    pub identity: SourceIdentity,
    pub row_count: u64,
    pub root_schema: Option<SourceSchemaFact>,
    pub schema_facts: Vec<SourceSchemaFact>,
    pub layout_facts: Vec<SourceLayoutFact>,
    pub segment_facts: Vec<SourceSegmentFact>,
    pub split_facts: Vec<SourceSplitFact>,
    pub coverage: Option<SourceCoverage>,
}

impl SourceFacts {
    pub fn new(identity: SourceIdentity, row_count: u64) -> Self {
        Self {
            identity,
            row_count,
            root_schema: None,
            schema_facts: Vec::new(),
            layout_facts: Vec::new(),
            segment_facts: Vec::new(),
            split_facts: Vec::new(),
            coverage: None,
        }
    }
}

/// Loom artifact payload kind described by an ingress report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceEmissionKind {
    None,
    Lmp1,
    Lmt1,
    ArrowSemantic,
}

impl SourceEmissionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Lmp1 => "LMP1",
            Self::Lmt1 => "LMT1",
            Self::ArrowSemantic => "LMA1",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceEmissionDisposition {
    None,
    CanonicalRaw,
    CanonicalTable,
    StructuredLayout,
    SemanticArrow,
}

impl SourceEmissionDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::CanonicalRaw => "canonical-raw",
            Self::CanonicalTable => "canonical-table",
            Self::StructuredLayout => "structured-layout",
            Self::SemanticArrow => "semantic-arrow",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceLoweringDisposition {
    InterpreterOnly,
    ProductionLoweringSupported,
    FailClosedDeferred,
}

impl SourceLoweringDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InterpreterOnly => "interpreter-only",
            Self::ProductionLoweringSupported => "production-lowering-supported",
            Self::FailClosedDeferred => "fail-closed/deferred",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceOracleStrategy {
    SourceNativeScan,
    ArrowScan,
    DecodedRowFixture,
    Unsupported,
}

impl SourceOracleStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SourceNativeScan => "source-native-scan",
            Self::ArrowScan => "arrow-scan",
            Self::DecodedRowFixture => "decoded-row-fixture",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceOracleEvidence {
    pub strategy: SourceOracleStrategy,
    pub accepted: bool,
    pub row_count_checked: Option<u64>,
    pub nulls_checked: bool,
    pub source_native_scan_used: bool,
    pub notes: Vec<String>,
}

impl SourceOracleEvidence {
    pub fn accepted(strategy: SourceOracleStrategy, row_count_checked: u64) -> Self {
        Self {
            strategy,
            accepted: true,
            row_count_checked: Some(row_count_checked),
            nulls_checked: false,
            source_native_scan_used: strategy == SourceOracleStrategy::SourceNativeScan,
            notes: Vec::new(),
        }
    }

    pub fn unsupported(note: impl Into<String>) -> Self {
        Self {
            strategy: SourceOracleStrategy::Unsupported,
            accepted: false,
            row_count_checked: None,
            nulls_checked: false,
            source_native_scan_used: false,
            notes: vec![note.into()],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceArtifactVerificationSummary {
    pub required: bool,
    pub accepted: bool,
    pub artifact_byte_len: Option<usize>,
    pub summary: String,
}

impl SourceArtifactVerificationSummary {
    pub fn accepted(artifact_byte_len: usize, summary: impl Into<String>) -> Self {
        Self {
            required: true,
            accepted: true,
            artifact_byte_len: Some(artifact_byte_len),
            summary: summary.into(),
        }
    }

    pub fn not_applicable() -> Self {
        Self {
            required: false,
            accepted: false,
            artifact_byte_len: None,
            summary: "not-applicable".to_string(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIngressReport {
    pub status: SourceIngressStatus,
    pub identity: SourceIdentity,
    pub facts: Option<SourceFacts>,
    pub diagnostics: Vec<SourceDiagnostic>,
    pub emission_kind: SourceEmissionKind,
    pub emission_disposition: SourceEmissionDisposition,
    pub lowering_disposition: SourceLoweringDisposition,
    pub artifact_verification: SourceArtifactVerificationSummary,
    pub oracle_evidence: Option<SourceOracleEvidence>,
}

/// Verifier-accepted source artifact handoff.
///
/// This wrapper is intentionally source-neutral: adapters may return bytes only
/// together with the accepted source-ingress report that justifies them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIngressAcceptedArtifact {
    pub bytes: Vec<u8>,
    pub report: SourceIngressReport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceIngressReportError {
    MissingArtifactEmission,
    ArtifactVerificationNotAccepted,
    OracleEvidenceNotAccepted,
}

impl SourceIngressReport {
    pub fn accepted(
        facts: SourceFacts,
        emission_kind: SourceEmissionKind,
        emission_disposition: SourceEmissionDisposition,
        lowering_disposition: SourceLoweringDisposition,
        artifact_verification: SourceArtifactVerificationSummary,
        oracle_evidence: SourceOracleEvidence,
    ) -> Result<Self, SourceIngressReportError> {
        if !matches!(
            emission_kind,
            SourceEmissionKind::Lmp1 | SourceEmissionKind::Lmt1 | SourceEmissionKind::ArrowSemantic
        ) {
            return Err(SourceIngressReportError::MissingArtifactEmission);
        }

        if !artifact_verification.accepted
            || !artifact_verification.required
            || artifact_verification.artifact_byte_len.is_none()
        {
            return Err(SourceIngressReportError::ArtifactVerificationNotAccepted);
        }

        if !oracle_evidence.accepted {
            return Err(SourceIngressReportError::OracleEvidenceNotAccepted);
        }

        Ok(Self {
            status: SourceIngressStatus::Accepted,
            identity: facts.identity.clone(),
            facts: Some(facts),
            diagnostics: Vec::new(),
            emission_kind,
            emission_disposition,
            lowering_disposition,
            artifact_verification,
            oracle_evidence: Some(oracle_evidence),
        })
    }

    pub fn unsupported(facts: Option<SourceFacts>, diagnostic: SourceDiagnostic) -> Self {
        let identity = facts
            .as_ref()
            .map(|facts| facts.identity.clone())
            .unwrap_or_else(|| SourceIdentity::new("unknown", "unknown"));

        Self {
            status: SourceIngressStatus::Unsupported,
            identity,
            facts,
            diagnostics: vec![diagnostic],
            emission_kind: SourceEmissionKind::None,
            emission_disposition: SourceEmissionDisposition::None,
            lowering_disposition: SourceLoweringDisposition::FailClosedDeferred,
            artifact_verification: SourceArtifactVerificationSummary::not_applicable(),
            oracle_evidence: None,
        }
    }

    pub fn rejected(identity: SourceIdentity, diagnostic: SourceDiagnostic) -> Self {
        Self {
            status: SourceIngressStatus::Rejected,
            identity,
            facts: None,
            diagnostics: vec![diagnostic],
            emission_kind: SourceEmissionKind::None,
            emission_disposition: SourceEmissionDisposition::None,
            lowering_disposition: SourceLoweringDisposition::FailClosedDeferred,
            artifact_verification: SourceArtifactVerificationSummary::not_applicable(),
            oracle_evidence: None,
        }
    }
}
