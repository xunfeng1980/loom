//! Unified artifact-facing verifier report model.
//!
//! Phase 17 starts by defining the report and facts contract before wiring the
//! existing structural and `L2Core` verifiers into one pipeline.

use crate::arrow_semantic_codec::{
    arrow_semantic_container_feature_names, decode_arrow_semantic_container,
    decode_arrow_semantic_payload, is_arrow_semantic_container, is_arrow_semantic_payload,
};
use crate::container_codec::{decode_container, feature_names, ContainerDescription, SectionKind};
use loom_ir_core::full_verifier::verify_l2_core;
use loom_ir_core::l2_core::L2CoreProgram;
use loom_ir_core::l2_core::VerifiedArtifactFacts;
use crate::l2_kernel_registry::L2KernelRegistry;
use crate::native_lowering::check_lowering_support;
use crate::verifier::verify_container;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactVerificationStage {
    Container,
    Manifest,
    L1Structural,
    L2Core,
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
    /// In-TCB constraint discharge status. Always `false` in Phases A–C because
    /// no bounded prover is in the TCB yet. Phase D may upgrade this.
    pub constraints_discharged: bool,
    /// Out-of-TCB evidence only: whether the kloom spec-oracle produced a trace
    /// for this artifact. Never gates a production fact.
    pub spec_oracle_trace_validated: bool,
    pub lowering_ready: ArtifactLoweringReadiness,

    /// TCB status of the artifact kind. "in-tcb" for L2Core IR artifacts;
    /// "out-of-tcb" for LMC2/LMA1 (demoted to dev-time packaging in Phase 50.1).
    /// None for LMC1 artifacts (inherits default: L1 structural only).
    pub tcb_status: Option<String>,

    /// Production role of the artifact kind.
    /// "dev-time-reference-packaging" for LMC2/LMA1;
    /// "distribution-container" for LMC1 (backward compat);
    /// None when unspecified.
    pub artifact_role: Option<String>,
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
            constraints_discharged: false,
            spec_oracle_trace_validated: false,
            lowering_ready: ArtifactLoweringReadiness::default(),
            tcb_status: None,
            artifact_role: None,
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
    options: &ArtifactVerificationOptions,
) -> ArtifactVerificationReport {
    if is_arrow_semantic_payload(bytes) {
        return verify_arrow_semantic_artifact(bytes, options);
    }

    if is_arrow_semantic_container(bytes) {
        return verify_arrow_semantic_container_artifact(bytes, options);
    }

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
    if options.compute_lowering_readiness || options.require_l2_core_for_lowering {
        facts.lowering_ready = ArtifactLoweringReadiness::with_diagnostic(
            Some(lowering_backend(options)),
            ArtifactLoweringDiagnostic::new(
                "missing-l2core-facts",
                "$.facts.l2_core",
                "lowering readiness requires an associated accepted L2Core program",
            ),
        );
    }

    ArtifactVerificationReport::accepted(facts)
}

fn verify_arrow_semantic_container_artifact(
    bytes: &[u8],
    options: &ArtifactVerificationOptions,
) -> ArtifactVerificationReport {
    let container = match decode_arrow_semantic_container(bytes) {
        Ok(container) => container,
        Err(err) => {
            return ArtifactVerificationReport::rejected(vec![
                ArtifactVerificationDiagnostic::new(
                    ArtifactVerificationStage::Container,
                    "arrow-semantic-container",
                    "$.lmc2",
                    err.to_string(),
                ),
            ]);
        }
    };
    let payload = match decode_arrow_semantic_payload(&container.payload) {
        Ok(payload) => payload,
        Err(err) => {
            return ArtifactVerificationReport::rejected(vec![
                ArtifactVerificationDiagnostic::new(
                    ArtifactVerificationStage::L1Structural,
                    "arrow-semantic-payload",
                    "$.lmc2.payload",
                    err.to_string(),
                ),
            ]);
        }
    };

    let mut facts = ArtifactVerificationFacts::new("LMC2");
    facts.container_version = Some(container.version);
    facts.required_features = arrow_semantic_container_feature_names(container.required_features)
        .into_iter()
        .map(str::to_string)
        .collect();
    facts.optional_features = arrow_semantic_container_feature_names(container.optional_features)
        .into_iter()
        .map(str::to_string)
        .collect();
    facts.payload_kind = Some("Arrow semantic payload".to_string());
    facts.schema_section_present = true;
    facts.row_count_bound = Some(payload.row_count() as u64);
    if options.compute_lowering_readiness || options.require_l2_core_for_lowering {
        facts.lowering_ready = ArtifactLoweringReadiness::with_diagnostic(
            Some(lowering_backend(options)),
            ArtifactLoweringDiagnostic::new(
                "arrow-semantic-lowering-deferred",
                "$.facts.lowering_ready",
                "Arrow semantic artifacts are verifier-accepted but not native-lowering ready",
            ),
        );
    }

    facts.tcb_status = Some("out-of-tcb".to_string());
    facts.artifact_role = Some("dev-time-reference-packaging".to_string());

    ArtifactVerificationReport::accepted(facts)
}

fn verify_arrow_semantic_artifact(
    bytes: &[u8],
    options: &ArtifactVerificationOptions,
) -> ArtifactVerificationReport {
    let payload = match decode_arrow_semantic_payload(bytes) {
        Ok(payload) => payload,
        Err(err) => {
            return ArtifactVerificationReport::rejected(vec![
                ArtifactVerificationDiagnostic::new(
                    ArtifactVerificationStage::L1Structural,
                    "arrow-semantic-payload",
                    "$.payload",
                    err.to_string(),
                ),
            ]);
        }
    };

    let mut facts = ArtifactVerificationFacts::new("LMA1");
    facts.payload_kind = Some("Arrow semantic payload".to_string());
    facts.schema_section_present = true;
    facts.row_count_bound = Some(payload.row_count() as u64);
    if options.compute_lowering_readiness || options.lowering_backend.is_some() {
        facts.lowering_ready = ArtifactLoweringReadiness::with_diagnostic(
            Some(lowering_backend(options)),
            ArtifactLoweringDiagnostic::new(
                "arrow-semantic-lowering-deferred",
                "$.facts.lowering_ready",
                "Arrow semantic artifacts are verifier-accepted but not native-lowering ready",
            ),
        );
    }

    facts.tcb_status = Some("out-of-tcb".to_string());
    facts.artifact_role = Some("dev-time-reference-packaging".to_string());

    ArtifactVerificationReport::accepted(facts)
}

pub fn verify_artifact_with_l2_core(
    bytes: &[u8],
    registry: &L2KernelRegistry,
    program: &L2CoreProgram,
    options: &ArtifactVerificationOptions,
) -> ArtifactVerificationReport {
    let artifact_report = verify_artifact(bytes, registry, options);
    if artifact_report.status() != ArtifactVerificationStatus::Accepted {
        return artifact_report;
    }
    let mut artifact_facts = artifact_report
        .into_facts()
        .expect("accepted artifact report must contain facts");

    let l2_report = verify_l2_core(program);
    if !l2_report.is_ok() {
        let diagnostics = l2_report
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                ArtifactVerificationDiagnostic::new(
                    ArtifactVerificationStage::L2Core,
                    diagnostic.code.as_str(),
                    diagnostic.path.clone(),
                    diagnostic.message.clone(),
                )
            })
            .collect();
        return ArtifactVerificationReport::rejected(diagnostics);
    }

    let Some(l2_facts) = l2_report.facts().cloned() else {
        return ArtifactVerificationReport::rejected(vec![ArtifactVerificationDiagnostic::new(
            ArtifactVerificationStage::Facts,
            "missing-l2core-facts",
            "$.l2_core.facts",
            "accepted L2Core report did not emit VerifiedArtifactFacts",
        )]);
    };

    artifact_facts.row_count_bound = l2_facts.row_count_bound;
    artifact_facts.constraint_ids = l2_facts.constraint_ids.clone();
    artifact_facts.proof_obligation_ids = l2_facts.proof_obligation_ids.clone();
    // Phase A–C: constraints_discharged stays false (no in-TCB prover).
    // kloom result is recorded as out-of-TCB evidence only.
    artifact_facts.constraints_discharged = false;
    artifact_facts.spec_oracle_trace_validated = l2_facts.kloom_discharged;
    artifact_facts.l2_core = Some(l2_facts);
    if options.compute_lowering_readiness || options.lowering_backend.is_some() {
        artifact_facts.lowering_ready = lowering_readiness_for(program, &l2_report, options);
    }

    ArtifactVerificationReport::accepted(artifact_facts)
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

fn lowering_readiness_for(
    program: &L2CoreProgram,
    report: &loom_ir_core::full_verifier::FullVerificationReport,
    options: &ArtifactVerificationOptions,
) -> ArtifactLoweringReadiness {
    let backend = lowering_backend(options);
    let support = check_lowering_support(program, report);
    // Phase A–C: lowering readiness = accepted ∧ supported-shape-has-a-rule.
    // No in-TCB constraint discharge is required.
    if support.is_supported() {
        return ArtifactLoweringReadiness::ready(backend);
    }

    let diagnostics = support
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            ArtifactLoweringDiagnostic::new(
                diagnostic.code.as_str(),
                diagnostic.path.clone(),
                diagnostic.message.clone(),
            )
        })
        .collect();
    ArtifactLoweringReadiness {
        ready: false,
        backend: Some(backend),
        diagnostics,
    }
}

fn lowering_backend(options: &ArtifactVerificationOptions) -> String {
    options
        .lowering_backend
        .clone()
        .unwrap_or_else(|| "textual-mlir".to_string())
}
