//! Unified artifact-facing verifier report model.
//!
//! Phase 17 starts by defining the report and facts contract before wiring the
//! existing structural and `L2Core` verifiers into one pipeline.

use crate::l2_core::VerifiedArtifactFacts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactVerificationStage {
    Container,
    Manifest,
    L1Structural,
    L2Core,
    ConstraintDischarge,
    Facts,
    LoweringReadiness,
}

impl ArtifactVerificationStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Container => "container",
            Self::Manifest => "manifest",
            Self::L1Structural => "l1-structural",
            Self::L2Core => "l2core",
            Self::ConstraintDischarge => "constraint-discharge",
            Self::Facts => "facts",
            Self::LoweringReadiness => "lowering-readiness",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactVerificationStatus {
    Accepted,
    Rejected,
    Unsupported,
}

impl ArtifactVerificationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintDischargeStatus {
    NotRequired,
    CollectedOnly,
    Discharged,
    Failed,
    Unknown,
    Skipped,
}

impl ConstraintDischargeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not-required",
            Self::CollectedOnly => "collected-only",
            Self::Discharged => "discharged",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactVerificationDiagnostic {
    pub stage: ArtifactVerificationStage,
    pub code: String,
    pub path: String,
    pub message: String,
}

impl ArtifactVerificationDiagnostic {
    pub fn new(
        stage: ArtifactVerificationStage,
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            code: code.into(),
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactLoweringDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
}

impl ArtifactLoweringDiagnostic {
    pub fn new(
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactLoweringReadiness {
    pub ready: bool,
    pub backend: Option<String>,
    pub diagnostics: Vec<ArtifactLoweringDiagnostic>,
}

impl ArtifactLoweringReadiness {
    pub fn ready(backend: impl Into<String>) -> Self {
        Self {
            ready: true,
            backend: Some(backend.into()),
            diagnostics: Vec::new(),
        }
    }

    pub fn not_ready(backend: Option<impl Into<String>>) -> Self {
        Self {
            ready: false,
            backend: backend.map(Into::into),
            diagnostics: Vec::new(),
        }
    }

    pub fn with_diagnostic(
        backend: Option<impl Into<String>>,
        diagnostic: ArtifactLoweringDiagnostic,
    ) -> Self {
        Self {
            ready: false,
            backend: backend.map(Into::into),
            diagnostics: vec![diagnostic],
        }
    }
}

impl Default for ArtifactLoweringReadiness {
    fn default() -> Self {
        Self {
            ready: false,
            backend: None,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactVerificationFacts {
    pub artifact_kind: String,
    pub container_version: Option<u16>,
    pub required_features: Vec<String>,
    pub optional_features: Vec<String>,
    pub payload_kind: Option<String>,
    pub schema_section_present: bool,
    pub kernel_manifest_section_present: bool,
    pub stats_section_present: bool,
    pub row_count_bound: Option<u64>,
    pub l2_core: Option<VerifiedArtifactFacts>,
    pub constraint_ids: Vec<String>,
    pub proof_obligation_ids: Vec<String>,
    pub constraint_status: ConstraintDischargeStatus,
    pub lowering_ready: ArtifactLoweringReadiness,
}

impl ArtifactVerificationFacts {
    pub fn new(artifact_kind: impl Into<String>) -> Self {
        Self {
            artifact_kind: artifact_kind.into(),
            container_version: None,
            required_features: Vec::new(),
            optional_features: Vec::new(),
            payload_kind: None,
            schema_section_present: false,
            kernel_manifest_section_present: false,
            stats_section_present: false,
            row_count_bound: None,
            l2_core: None,
            constraint_ids: Vec::new(),
            proof_obligation_ids: Vec::new(),
            constraint_status: ConstraintDischargeStatus::NotRequired,
            lowering_ready: ArtifactLoweringReadiness::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactVerificationOptions {
    pub require_l2_core_for_lowering: bool,
    pub lowering_backend: Option<String>,
    pub compute_lowering_readiness: bool,
}

impl Default for ArtifactVerificationOptions {
    fn default() -> Self {
        Self {
            require_l2_core_for_lowering: false,
            lowering_backend: None,
            compute_lowering_readiness: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactVerificationReport {
    status: ArtifactVerificationStatus,
    diagnostics: Vec<ArtifactVerificationDiagnostic>,
    facts: Option<ArtifactVerificationFacts>,
}

impl ArtifactVerificationReport {
    pub fn accepted(facts: ArtifactVerificationFacts) -> Self {
        Self {
            status: ArtifactVerificationStatus::Accepted,
            diagnostics: Vec::new(),
            facts: Some(facts),
        }
    }

    pub fn rejected(diagnostics: Vec<ArtifactVerificationDiagnostic>) -> Self {
        Self {
            status: ArtifactVerificationStatus::Rejected,
            diagnostics,
            facts: None,
        }
    }

    pub fn unsupported(diagnostics: Vec<ArtifactVerificationDiagnostic>) -> Self {
        Self {
            status: ArtifactVerificationStatus::Unsupported,
            diagnostics,
            facts: None,
        }
    }

    pub fn status(&self) -> ArtifactVerificationStatus {
        self.status
    }

    pub fn is_ok(&self) -> bool {
        self.status == ArtifactVerificationStatus::Accepted && self.diagnostics.is_empty()
    }

    pub fn diagnostics(&self) -> &[ArtifactVerificationDiagnostic] {
        &self.diagnostics
    }

    pub fn first_error(&self) -> Option<&ArtifactVerificationDiagnostic> {
        self.diagnostics.first()
    }

    pub fn facts(&self) -> Option<&ArtifactVerificationFacts> {
        match self.status {
            ArtifactVerificationStatus::Accepted => self.facts.as_ref(),
            ArtifactVerificationStatus::Rejected | ArtifactVerificationStatus::Unsupported => None,
        }
    }

    pub fn into_facts(self) -> Option<ArtifactVerificationFacts> {
        match self.status {
            ArtifactVerificationStatus::Accepted => self.facts,
            ArtifactVerificationStatus::Rejected | ArtifactVerificationStatus::Unsupported => None,
        }
    }
}
