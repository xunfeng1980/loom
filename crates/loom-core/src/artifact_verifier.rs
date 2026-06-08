//! Unified artifact-facing verifier report model.
//!
//! Phase 17 starts by defining the report and facts contract before wiring the
//! existing structural and `L2Core` verifiers into one pipeline.

use std::collections::BTreeSet;

use crate::container_codec::{decode_container, feature_names, ContainerDescription, SectionKind};
use crate::full_verifier::verify_l2_core;
use crate::l2_core::L2CoreProgram;
use crate::l2_core::VerifiedArtifactFacts;
use crate::l2_kernel_registry::L2KernelRegistry;
use crate::native_lowering::check_lowering_support;
use crate::solver::{SolverDischargeReport, SolverObligationStatus};
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
    pub solver_report: Option<SolverDischargeReport>,
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
            solver_report: None,
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

pub fn apply_solver_discharge(
    mut report: ArtifactVerificationReport,
    solver_report: SolverDischargeReport,
) -> ArtifactVerificationReport {
    if report.status != ArtifactVerificationStatus::Accepted {
        return report;
    }

    let Some(mut facts) = report.facts.take() else {
        report.diagnostics.push(ArtifactVerificationDiagnostic::new(
            ArtifactVerificationStage::Facts,
            "missing-artifact-facts",
            "$.facts",
            "accepted artifact report did not expose facts for solver discharge",
        ));
        return report;
    };

    if facts.constraint_ids.is_empty() {
        facts.constraint_status = ConstraintDischargeStatus::NotRequired;
        facts.solver_report = Some(solver_report);
        report.facts = Some(facts);
        return report;
    }

    let diagnostics = solver_discharge_diagnostics(&facts, &solver_report);
    let discharged = diagnostics.is_empty() && solver_report.is_successful();
    facts.constraint_status = if discharged {
        ConstraintDischargeStatus::Discharged
    } else if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code.ends_with("mismatch"))
    {
        ConstraintDischargeStatus::Failed
    } else {
        constraint_status_from_solver_report(&solver_report)
    };

    if discharged {
        facts.lowering_ready = promote_solver_blocked_lowering(facts.lowering_ready);
    } else if facts.lowering_ready.ready {
        facts.lowering_ready = solver_blocked_lowering(facts.lowering_ready.backend.clone());
    }

    facts.solver_report = Some(solver_report);
    report.diagnostics.extend(diagnostics);
    report.facts = Some(facts);
    report
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
    if _options.compute_lowering_readiness || _options.require_l2_core_for_lowering {
        facts.lowering_ready = ArtifactLoweringReadiness::with_diagnostic(
            Some(lowering_backend(_options)),
            ArtifactLoweringDiagnostic::new(
                "missing-l2core-facts",
                "$.facts.l2_core",
                "lowering readiness requires an associated accepted L2Core program",
            ),
        );
    }

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
    artifact_facts.constraint_status = constraint_status_for(&artifact_facts.constraint_ids);
    artifact_facts.l2_core = Some(l2_facts);
    if options.compute_lowering_readiness || options.lowering_backend.is_some() {
        artifact_facts.lowering_ready = lowering_readiness_for(
            program,
            &l2_report,
            options,
            artifact_facts.constraint_status,
        );
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

fn constraint_status_for(constraint_ids: &[String]) -> ConstraintDischargeStatus {
    if constraint_ids.is_empty() {
        ConstraintDischargeStatus::NotRequired
    } else {
        ConstraintDischargeStatus::CollectedOnly
    }
}

fn lowering_readiness_for(
    program: &L2CoreProgram,
    report: &crate::full_verifier::FullVerificationReport,
    options: &ArtifactVerificationOptions,
    constraint_status: ConstraintDischargeStatus,
) -> ArtifactLoweringReadiness {
    let backend = lowering_backend(options);
    let support = check_lowering_support(program, report);
    if support.is_supported() && !constraint_status_blocks_lowering(constraint_status) {
        return ArtifactLoweringReadiness::ready(backend);
    }

    if support.is_supported() {
        return solver_blocked_lowering(Some(backend));
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

fn solver_discharge_diagnostics(
    facts: &ArtifactVerificationFacts,
    solver_report: &SolverDischargeReport,
) -> Vec<ArtifactVerificationDiagnostic> {
    let mut diagnostics = Vec::new();
    let expected = facts
        .constraint_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let actual = solver_report
        .backend_results
        .iter()
        .map(|result| result.obligation_id.clone())
        .collect::<BTreeSet<_>>();

    if expected != actual {
        diagnostics.push(ArtifactVerificationDiagnostic::new(
            ArtifactVerificationStage::ConstraintDischarge,
            "solver-obligation-mismatch",
            "$.solver.backend_results",
            format!(
                "solver obligations do not match artifact constraints: expected {:?}, got {:?}",
                expected, actual
            ),
        ));
    }

    if solver_report.required_obligation_count != facts.constraint_ids.len() {
        diagnostics.push(ArtifactVerificationDiagnostic::new(
            ArtifactVerificationStage::ConstraintDischarge,
            "solver-required-count-mismatch",
            "$.solver.required_obligation_count",
            format!(
                "solver required obligation count {} does not match artifact constraint count {}",
                solver_report.required_obligation_count,
                facts.constraint_ids.len()
            ),
        ));
    }

    if !solver_report.is_successful() {
        diagnostics.push(ArtifactVerificationDiagnostic::new(
            ArtifactVerificationStage::ConstraintDischarge,
            solver_failure_code(solver_report.status),
            "$.solver.status",
            format!(
                "solver discharge status {} does not discharge all required obligations",
                solver_report.status.as_str()
            ),
        ));
    }

    diagnostics
}

fn solver_failure_code(status: SolverObligationStatus) -> &'static str {
    match status {
        SolverObligationStatus::Discharged => "solver-malformed-report",
        SolverObligationStatus::Failed => "solver-discharge-failed",
        SolverObligationStatus::Unknown => "solver-discharge-unknown",
        SolverObligationStatus::TimedOut => "solver-discharge-timed-out",
        SolverObligationStatus::Error => "solver-discharge-error",
        SolverObligationStatus::Skipped => "solver-discharge-skipped",
    }
}

fn constraint_status_from_solver_report(
    solver_report: &SolverDischargeReport,
) -> ConstraintDischargeStatus {
    match solver_report.status {
        SolverObligationStatus::Discharged => ConstraintDischargeStatus::Discharged,
        SolverObligationStatus::Unknown => ConstraintDischargeStatus::Unknown,
        SolverObligationStatus::Skipped => ConstraintDischargeStatus::Skipped,
        SolverObligationStatus::Failed
        | SolverObligationStatus::TimedOut
        | SolverObligationStatus::Error => ConstraintDischargeStatus::Failed,
    }
}

fn constraint_status_blocks_lowering(status: ConstraintDischargeStatus) -> bool {
    matches!(
        status,
        ConstraintDischargeStatus::CollectedOnly
            | ConstraintDischargeStatus::Failed
            | ConstraintDischargeStatus::Unknown
            | ConstraintDischargeStatus::Skipped
    )
}

fn solver_blocked_lowering(backend: Option<String>) -> ArtifactLoweringReadiness {
    ArtifactLoweringReadiness::with_diagnostic(
        Some(backend.unwrap_or_else(|| "textual-mlir".to_string())),
        ArtifactLoweringDiagnostic::new(
            "constraints-not-discharged",
            "$.facts.constraint_status",
            "lowering readiness requires discharged solver-backed constraints",
        ),
    )
}

fn promote_solver_blocked_lowering(
    readiness: ArtifactLoweringReadiness,
) -> ArtifactLoweringReadiness {
    if readiness.ready {
        return readiness;
    }
    let only_solver_block = readiness.diagnostics.len() == 1
        && readiness.diagnostics[0].code == "constraints-not-discharged";
    if only_solver_block {
        ArtifactLoweringReadiness::ready(
            readiness
                .backend
                .unwrap_or_else(|| "textual-mlir".to_string()),
        )
    } else {
        readiness
    }
}
