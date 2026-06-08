//! Unified artifact-facing verifier report model.
//!
//! Phase 17 starts by defining the report and facts contract before wiring the
//! existing structural and `L2Core` verifiers into one pipeline.

use crate::container_codec::{decode_container, feature_names, ContainerDescription, SectionKind};
use crate::l2_core::VerifiedArtifactFacts;
use crate::l2_kernel_registry::L2KernelRegistry;
use crate::verifier::verify_container;

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

pub fn verify_artifact(
    bytes: &[u8],
    registry: &L2KernelRegistry,
    _options: &ArtifactVerificationOptions,
) -> ArtifactVerificationReport {
    let container = match decode_container(bytes) {
        Ok(container) => container,
        Err(err) => {
            return ArtifactVerificationReport::rejected(vec![
                ArtifactVerificationDiagnostic::new(
                    ArtifactVerificationStage::Container,
                    "container-shape",
                    "$.container",
                    err.to_string(),
                ),
            ]);
        }
    };

    let payload_kind = payload_kind(&container);
    let Some(payload_kind) = payload_kind else {
        return ArtifactVerificationReport::unsupported(vec![ArtifactVerificationDiagnostic::new(
            ArtifactVerificationStage::Manifest,
            "unsupported-payload-kind",
            "$.sections",
            "artifact container does not contain a supported LMP1 or LMT1 payload",
        )]);
    };

    let structural = verify_container(bytes, registry);
    if !structural.is_ok() {
        let diagnostics = structural
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                ArtifactVerificationDiagnostic::new(
                    ArtifactVerificationStage::L1Structural,
                    diagnostic.code.as_str(),
                    diagnostic.path.clone(),
                    diagnostic.message.clone(),
                )
            })
            .collect();
        return ArtifactVerificationReport::rejected(diagnostics);
    }

    let mut facts = ArtifactVerificationFacts::new("LMC1");
    facts.container_version = Some(container.version);
    facts.required_features = feature_names(container.required_features)
        .into_iter()
        .map(str::to_string)
        .collect();
    facts.optional_features = feature_names(container.optional_features)
        .into_iter()
        .map(str::to_string)
        .collect();
    facts.payload_kind = Some(payload_kind.to_string());
    facts.schema_section_present = has_section(&container, SectionKind::Schema);
    facts.kernel_manifest_section_present = has_section(&container, SectionKind::KernelManifest);
    facts.stats_section_present = has_section(&container, SectionKind::Stats);

    ArtifactVerificationReport::accepted(facts)
}

fn payload_kind(container: &ContainerDescription) -> Option<&'static str> {
    if container
        .sections
        .iter()
        .any(|section| section.kind == SectionKind::LayoutPayload)
    {
        Some("LMP1 layout")
    } else if container
        .sections
        .iter()
        .any(|section| section.kind == SectionKind::TablePayload)
    {
        Some("LMT1 table")
    } else {
        None
    }
}

fn has_section(container: &ContainerDescription, kind: SectionKind) -> bool {
    container
        .sections
        .iter()
        .any(|section| section.kind == kind)
}
