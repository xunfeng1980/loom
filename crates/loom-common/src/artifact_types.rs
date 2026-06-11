//! Production-core artifact verification types.
//!
//! Extracted from `loom-container::artifact_types` — zero dependency on
//! the legacy container packaging layer.

use loom_ir_core::l2_core::VerifiedArtifactFacts;

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

    pub fn is_accepted(&self) -> bool {
        self.status == ArtifactVerificationStatus::Accepted
    }

    pub fn is_rejected(&self) -> bool {
        self.status == ArtifactVerificationStatus::Rejected
    }
}

// ---------------------------------------------------------------------------
// Arrow-semantic artifact verification (plan 52-01: paths 1+2 live here)
// ---------------------------------------------------------------------------
// The full `verify_artifact` has three code paths:
//   1. Arrow semantic payload (LMA1)     — all deps in loom-common
//   2. Arrow semantic container (LMC2)   — all deps in loom-common
//   3. LMC1 container                    — needs legacy container packaging (stays in loom-container)
//
// In plan 52-01, paths 1+2 are available here. Path 3 is handled by
// loom-container which re-exports this function plus its own LMC1 path.

use crate::arrow_semantic_codec::{
    arrow_semantic_container_feature_names, decode_arrow_semantic_container,
    decode_arrow_semantic_payload, is_arrow_semantic_container, is_arrow_semantic_payload,
};

pub fn verify_artifact(
    bytes: &[u8],
    _registry: &crate::l2_kernel_registry::L2KernelRegistry,
    options: &ArtifactVerificationOptions,
) -> ArtifactVerificationReport {
    if is_arrow_semantic_payload(bytes) {
        return verify_arrow_semantic_artifact(bytes, options);
    }

    if is_arrow_semantic_container(bytes) {
        return verify_arrow_semantic_container_artifact(bytes, options);
    }

    // LMC1 path: not available in loom-common (needs legacy container packaging).
    // Callers of this function (native_arrow_semantic, etc.) only pass
    // Arrow-semantic artifacts through here.
    ArtifactVerificationReport::unsupported(vec![ArtifactVerificationDiagnostic::new(
        ArtifactVerificationStage::Container,
        "unsupported-container",
        "$.container",
        "LMC1 container verification requires loom-container",
    )])
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
            Some(options.lowering_backend.clone().unwrap_or_else(|| "textual-mlir".to_string())),
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
            Some(options.lowering_backend.clone().unwrap_or_else(|| "textual-mlir".to_string())),
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
