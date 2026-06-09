//! Artifact-facing verified-lineage records for MVP1.5.
//!
//! A lineage record is provenance for safety and Arrow well-formedness claims.
//! It is not a correctness certificate, not verified compilation, and not a
//! signed attestation transport.

use crate::artifact_verifier::{
    ArtifactVerificationReport, ArtifactVerificationStatus, ConstraintDischargeStatus,
};
use crate::native_arrow_semantic::NativeArrowSemanticModelValidationReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifiedLineageDiagnosticCode {
    ArtifactNotAccepted,
    MissingVerifierFacts,
    ConstraintDischargeRequired,
    NativeModelValidationFailed,
}

impl VerifiedLineageDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ArtifactNotAccepted => "artifact-not-accepted",
            Self::MissingVerifierFacts => "missing-verifier-facts",
            Self::ConstraintDischargeRequired => "constraint-discharge-required",
            Self::NativeModelValidationFailed => "native-model-validation-failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedLineageDiagnostic {
    pub code: VerifiedLineageDiagnosticCode,
    pub path: String,
    pub message: String,
}

impl VerifiedLineageDiagnostic {
    pub fn new(
        code: VerifiedLineageDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifiedLineageEvidenceLayer {
    RustVerifierStructuralCheck,
    BitwuzlaSmtDischarge,
    LeanModeledSoundnessTheorem,
    LeanRustVerifierDifferential,
    ModelRustInterpreterDifferential,
    NativeModelValidation,
}

impl VerifiedLineageEvidenceLayer {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RustVerifierStructuralCheck => "rust-verifier-structural-check",
            Self::BitwuzlaSmtDischarge => "bitwuzla-smt-discharge",
            Self::LeanModeledSoundnessTheorem => "lean-modeled-soundness-theorem",
            Self::LeanRustVerifierDifferential => "lean-rust-verifier-differential",
            Self::ModelRustInterpreterDifferential => "model-rust-interpreter-differential",
            Self::NativeModelValidation => "native-model-validation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifiedLineageEvidenceStatus {
    Passed,
    /// True for the full release corpus via the CI gate
    /// (`scripts/verified-lineage-test.sh`); trusted-by-reference at artifact
    /// granularity, not re-validated when this individual record is built.
    CorpusValidated,
    Discharged,
    NotRequired,
    NotApplicable,
    NotRun,
    PerRunValidated,
}

impl VerifiedLineageEvidenceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::CorpusValidated => "corpus-validated",
            Self::Discharged => "discharged",
            Self::NotRequired => "not-required",
            Self::NotApplicable => "not-applicable",
            Self::NotRun => "not-run",
            Self::PerRunValidated => "per-run-validated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedLineageEvidence {
    pub layer: VerifiedLineageEvidenceLayer,
    pub status: VerifiedLineageEvidenceStatus,
    pub source: String,
    pub claim: String,
}

impl VerifiedLineageEvidence {
    pub fn new(
        layer: VerifiedLineageEvidenceLayer,
        status: VerifiedLineageEvidenceStatus,
        source: impl Into<String>,
        claim: impl Into<String>,
    ) -> Self {
        Self {
            layer,
            status,
            source: source.into(),
            claim: claim.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifiedLineageTcbAssumption {
    RustCompilerStd,
    LlvmMlirToolchain,
    RustCAbi,
    DuckDbHostProcess,
    ArrowCDataInterface,
}

impl VerifiedLineageTcbAssumption {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RustCompilerStd => "rust-compiler-std",
            Self::LlvmMlirToolchain => "llvm-mlir-toolchain",
            Self::RustCAbi => "rust-c-abi",
            Self::DuckDbHostProcess => "duckdb-host-process",
            Self::ArrowCDataInterface => "arrow-c-data-interface",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedLineageRecord {
    pub version: u16,
    pub artifact_kind: String,
    pub payload_kind: Option<String>,
    pub evidence: Vec<VerifiedLineageEvidence>,
    pub tcb_assumptions: Vec<VerifiedLineageTcbAssumption>,
    pub non_claims: Vec<String>,
}

impl VerifiedLineageRecord {
    pub const CURRENT_VERSION: u16 = 1;

    pub fn has_evidence_layer(&self, layer: VerifiedLineageEvidenceLayer) -> bool {
        self.evidence.iter().any(|evidence| evidence.layer == layer)
    }

    pub fn evidence_status(
        &self,
        layer: VerifiedLineageEvidenceLayer,
    ) -> Option<VerifiedLineageEvidenceStatus> {
        self.evidence
            .iter()
            .find(|evidence| evidence.layer == layer)
            .map(|evidence| evidence.status)
    }

    pub fn has_tcb_assumption(&self, assumption: VerifiedLineageTcbAssumption) -> bool {
        self.tcb_assumptions.contains(&assumption)
    }

    pub fn contains_non_claim(&self, needle: &str) -> bool {
        self.non_claims.iter().any(|claim| claim.contains(needle))
    }
}

pub fn build_verified_lineage_record(
    verification: &ArtifactVerificationReport,
    native_validation: Option<&NativeArrowSemanticModelValidationReport>,
) -> Result<VerifiedLineageRecord, VerifiedLineageDiagnostic> {
    if verification.status() != ArtifactVerificationStatus::Accepted || !verification.is_ok() {
        return Err(VerifiedLineageDiagnostic::new(
            VerifiedLineageDiagnosticCode::ArtifactNotAccepted,
            "$.verification",
            "verified-lineage records require an accepted artifact verifier report",
        ));
    }

    let Some(facts) = verification.facts() else {
        return Err(VerifiedLineageDiagnostic::new(
            VerifiedLineageDiagnosticCode::MissingVerifierFacts,
            "$.facts",
            "accepted artifact verifier report did not expose facts",
        ));
    };

    let mut evidence = vec![VerifiedLineageEvidence::new(
        VerifiedLineageEvidenceLayer::RustVerifierStructuralCheck,
        VerifiedLineageEvidenceStatus::Passed,
        "loom_core::artifact_verifier",
        "artifact/container/schema/facts acceptance passed fail-closed structural verification",
    )];

    if !facts.constraint_ids.is_empty()
        && facts.constraint_status != ConstraintDischargeStatus::Discharged
    {
        return Err(VerifiedLineageDiagnostic::new(
            VerifiedLineageDiagnosticCode::ConstraintDischargeRequired,
            "$.facts.constraint_status",
            "artifacts with collected constraints require discharged solver evidence before a positive verified-lineage record",
        ));
    }

    let solver_status = match facts.constraint_status {
        ConstraintDischargeStatus::Discharged => VerifiedLineageEvidenceStatus::Discharged,
        ConstraintDischargeStatus::NotRequired => VerifiedLineageEvidenceStatus::NotRequired,
        ConstraintDischargeStatus::CollectedOnly
        | ConstraintDischargeStatus::Failed
        | ConstraintDischargeStatus::Unknown
        | ConstraintDischargeStatus::Skipped => VerifiedLineageEvidenceStatus::NotRun,
    };
    evidence.push(VerifiedLineageEvidence::new(
        VerifiedLineageEvidenceLayer::BitwuzlaSmtDischarge,
        solver_status,
        "loom_core::artifact_verifier::apply_solver_discharge",
        "range/arithmetic bad-state obligations are discharged when required; not-required means this artifact exposed no solver obligations",
    ));

    let l2_scope_status = if facts.l2_core.is_some() {
        VerifiedLineageEvidenceStatus::CorpusValidated
    } else {
        VerifiedLineageEvidenceStatus::NotApplicable
    };
    evidence.push(VerifiedLineageEvidence::new(
        VerifiedLineageEvidenceLayer::LeanModeledSoundnessTheorem,
        l2_scope_status,
        "formal/lean/LoomCore.lean::accepted_program_safe",
        "Verified L2Core programs are safe over the Lean modeled executor only",
    ));
    evidence.push(VerifiedLineageEvidence::new(
        VerifiedLineageEvidenceLayer::LeanRustVerifierDifferential,
        VerifiedLineageEvidenceStatus::CorpusValidated,
        "scripts/lean-rust-correspondence-test.sh",
        "Lean and Rust verifier accept/reject classifications match over the release corpus",
    ));
    evidence.push(VerifiedLineageEvidence::new(
        VerifiedLineageEvidenceLayer::ModelRustInterpreterDifferential,
        VerifiedLineageEvidenceStatus::CorpusValidated,
        "scripts/model-rust-interpreter-consistency-test.sh",
        "Rust interpreter trace subject matches the reference modeled executor over the deterministic corpus",
    ));

    let native_status = match native_validation {
        Some(validation) if validation.is_validated() => {
            VerifiedLineageEvidenceStatus::PerRunValidated
        }
        Some(_) => {
            return Err(VerifiedLineageDiagnostic::new(
                VerifiedLineageDiagnosticCode::NativeModelValidationFailed,
                "$.native_model_validation",
                "native/model validation must succeed before it can be recorded as positive lineage evidence",
            ));
        }
        None => VerifiedLineageEvidenceStatus::NotRun,
    };
    evidence.push(VerifiedLineageEvidence::new(
        VerifiedLineageEvidenceLayer::NativeModelValidation,
        native_status,
        "scripts/native-model-validation-test.sh",
        "native Arrow semantic output is compared to the Phase 39 reference trace when native validation is supplied",
    ));

    Ok(VerifiedLineageRecord {
        version: VerifiedLineageRecord::CURRENT_VERSION,
        artifact_kind: facts.artifact_kind.clone(),
        payload_kind: facts.payload_kind.clone(),
        evidence,
        tcb_assumptions: default_tcb_assumptions(),
        non_claims: default_non_claims(),
    })
}

fn default_tcb_assumptions() -> Vec<VerifiedLineageTcbAssumption> {
    vec![
        VerifiedLineageTcbAssumption::RustCompilerStd,
        VerifiedLineageTcbAssumption::LlvmMlirToolchain,
        VerifiedLineageTcbAssumption::RustCAbi,
        VerifiedLineageTcbAssumption::DuckDbHostProcess,
        VerifiedLineageTcbAssumption::ArrowCDataInterface,
    ]
}

fn default_non_claims() -> Vec<String> {
    vec![
        "no source-data correctness claim".to_string(),
        "no upstream format semantic correctness claim".to_string(),
        "no end-to-end toolchain verification claim".to_string(),
        "no verified compilation claim".to_string(),
        "no production-readiness or performance claim".to_string(),
    ]
}
