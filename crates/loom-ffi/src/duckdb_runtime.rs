//! Internal DuckDB runtime bridge.
//!
//! This module keeps DuckDB as an adapter over the Phase 22 runtime ABI and
//! Phase 23 backend vocabulary. It is safe Rust only; later C ABI wrappers can
//! translate these owned reports into DuckDB-facing handles without duplicating
//! runtime policy in C++.

use arrow::datatypes::DataType;
use loom_core::artifact_verifier::{
    verify_artifact, ArtifactVerificationFacts, ArtifactVerificationOptions,
    ArtifactVerificationReport, ArtifactVerificationStatus, ConstraintDischargeStatus,
};
use loom_core::l2_core::{OutputSchemaFact, ResourceBudget, VerifiedArtifactFacts};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::production_native_lowering::{
    check_production_lowering_support, ProductionLoweringFacts,
};
use loom_core::runtime_abi::{
    decide_runtime_execution, plan_projection, ConcurrencyPolicy, PredicateEnvelope,
    ProjectionColumn, ProjectionSet, RuntimeAbiVersion, RuntimeBackendIdentity, RuntimeCacheKey,
    RuntimeCacheKeyInput, RuntimeEmissionDisposition, RuntimeExecutionDecision,
    RuntimeFallbackPolicy, RuntimeLoweringDisposition, RuntimePlan, RuntimeReaderSupport,
    RuntimeSafetyPolicy, SplitDescriptor, UnsupportedPredicatePolicy,
};
use loom_native_melior::backend::{NativeBackendIdentity, NATIVE_BACKEND_NAME};

#[derive(Debug, Clone, PartialEq)]
pub struct DuckDbRuntimePlanInput {
    pub artifact_bytes: Vec<u8>,
    pub projection: DuckDbProjection,
    pub policy: DuckDbRuntimePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuckDbRuntimePolicy {
    pub allow_interpreter_fallback: bool,
    pub test_native_facts: Option<DuckDbTestNativeFacts>,
}

impl Default for DuckDbRuntimePolicy {
    fn default() -> Self {
        Self {
            allow_interpreter_fallback: true,
            test_native_facts: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuckDbTestNativeFacts {
    pub row_count: u64,
    pub columns: Vec<DataType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuckDbProjection {
    All,
    Columns(Vec<u32>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuckDbRouteDecision {
    NativeCandidate,
    InterpreterFallback,
    FailClosed,
    DiagnosticOnly,
    Cancelled,
}

impl DuckDbRouteDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeCandidate => "native-candidate",
            Self::InterpreterFallback => "interpreter-fallback",
            Self::FailClosed => "fail-closed",
            Self::DiagnosticOnly => "diagnostic-only",
            Self::Cancelled => "cancelled",
        }
    }
}

impl From<RuntimeExecutionDecision> for DuckDbRouteDecision {
    fn from(decision: RuntimeExecutionDecision) -> Self {
        match decision {
            RuntimeExecutionDecision::NativeCandidate => Self::NativeCandidate,
            RuntimeExecutionDecision::InterpreterFallback => Self::InterpreterFallback,
            RuntimeExecutionDecision::FailClosed => Self::FailClosed,
            RuntimeExecutionDecision::DiagnosticOnly => Self::DiagnosticOnly,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuckDbRuntimeDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DuckDbRuntimePlanReport {
    pub decision: DuckDbRouteDecision,
    pub runtime_plan: RuntimePlan,
    pub cache_key: RuntimeCacheKey,
    pub output_to_source: Vec<u32>,
    pub policy: RuntimeSafetyPolicy,
    pub artifact_report: ArtifactVerificationReport,
    pub lowering_facts: Option<ProductionLoweringFacts>,
    pub diagnostics: Vec<DuckDbRuntimeDiagnostic>,
}

pub fn plan_duckdb_runtime(
    input: DuckDbRuntimePlanInput,
) -> Result<DuckDbRuntimePlanReport, DuckDbRuntimePlanReport> {
    let registry = L2KernelRegistry::default_for_mvp0();
    let verifier_options = ArtifactVerificationOptions {
        require_l2_core_for_lowering: false,
        lowering_backend: Some("loom-decode-dialect".to_string()),
        compute_lowering_readiness: true,
    };
    let mut artifact_report = verify_artifact(&input.artifact_bytes, &registry, &verifier_options);
    let mut diagnostics = artifact_diagnostics(&artifact_report);
    if let Some(test_facts) = input.policy.test_native_facts.as_ref() {
        artifact_report = attach_test_native_facts(artifact_report, test_facts);
        diagnostics.push(DuckDbRuntimeDiagnostic {
            code: "test-native-facts".to_string(),
            path: "$.policy.test_native_facts".to_string(),
            message: "test-only native-capable facts attached for DuckDB route coverage"
                .to_string(),
        });
    }

    let column_count = column_count_for(&artifact_report, &input);
    let projection = duckdb_projection_to_runtime(&input.projection);
    let projection_plan = match plan_projection(&projection, column_count) {
        Ok(plan) => plan,
        Err(diagnostic) => {
            diagnostics.push(runtime_diagnostic(diagnostic));
            let report = build_plan_report(
                DuckDbRouteDecision::FailClosed,
                RuntimeExecutionDecision::FailClosed,
                projection,
                PredicateEnvelope::None,
                SplitDescriptor::FullScan {
                    row_count: row_count_for(&artifact_report),
                },
                runtime_policy(&input.policy),
                None,
                artifact_report,
                diagnostics,
                Vec::new(),
                &input.artifact_bytes,
            );
            return Err(report);
        }
    };

    let predicate = PredicateEnvelope::None;
    let split = SplitDescriptor::FullScan {
        row_count: row_count_for(&artifact_report),
    };
    let policy = runtime_policy(&input.policy);
    let lowering_support = check_production_lowering_support(&artifact_report);
    diagnostics.extend(lowering_support.diagnostics().iter().map(|diagnostic| {
        DuckDbRuntimeDiagnostic {
            code: diagnostic.code.as_str().to_string(),
            path: diagnostic.path.clone(),
            message: diagnostic.message.clone(),
        }
    }));

    let runtime_decision =
        decide_runtime_execution(&loom_core::runtime_abi::RuntimeDecisionInput {
            artifact_status: artifact_report.status(),
            constraint_status: constraint_status_for(&artifact_report),
            production_lowering_supported: lowering_support.is_supported(),
            reader_support: reader_support_for(&artifact_report),
            emission_disposition: emission_disposition_for(&artifact_report),
            lowering_disposition: lowering_disposition_for(lowering_support.is_supported()),
            projection_supported: true,
            predicate_supported: true,
            split_supported: true,
            concurrency_safe: policy.concurrency == ConcurrencyPolicy::SingleWorker,
            policy,
        });
    diagnostics.extend(
        runtime_decision
            .diagnostics
            .iter()
            .cloned()
            .map(runtime_diagnostic),
    );

    let report = build_plan_report(
        runtime_decision.decision.into(),
        runtime_decision.decision,
        projection_plan.projection,
        predicate,
        split,
        policy,
        lowering_support.facts().cloned(),
        artifact_report,
        diagnostics,
        projection_plan.output_to_source,
        &input.artifact_bytes,
    );

    Ok(report)
}

fn build_plan_report(
    decision: DuckDbRouteDecision,
    runtime_decision: RuntimeExecutionDecision,
    projection: ProjectionSet,
    predicate: PredicateEnvelope,
    split: SplitDescriptor,
    policy: RuntimeSafetyPolicy,
    lowering_facts: Option<ProductionLoweringFacts>,
    artifact_report: ArtifactVerificationReport,
    diagnostics: Vec<DuckDbRuntimeDiagnostic>,
    output_to_source: Vec<u32>,
    artifact_bytes: &[u8],
) -> DuckDbRuntimePlanReport {
    let runtime_plan = RuntimePlan {
        abi_version: RuntimeAbiVersion::CURRENT,
        decision: runtime_decision,
        projection: projection.clone(),
        predicate: predicate.clone(),
        split,
        diagnostics: diagnostics
            .iter()
            .filter_map(|diagnostic| duckdb_diagnostic_to_runtime(diagnostic))
            .collect(),
    };
    let cache_key = RuntimeCacheKey::build(&RuntimeCacheKeyInput {
        abi_version: RuntimeAbiVersion::CURRENT,
        artifact_digest: artifact_digest(artifact_bytes),
        facts_fingerprint: facts_fingerprint(&artifact_report),
        solver_identity: "duckdb-no-solver".to_string(),
        production_lowering_fingerprint: lowering_fingerprint(lowering_facts.as_ref()),
        backend_identity: runtime_backend_identity(),
        projection,
        predicate,
        split,
        policy,
    });

    DuckDbRuntimePlanReport {
        decision,
        runtime_plan,
        cache_key,
        output_to_source,
        policy,
        artifact_report,
        lowering_facts,
        diagnostics,
    }
}

fn duckdb_projection_to_runtime(projection: &DuckDbProjection) -> ProjectionSet {
    match projection {
        DuckDbProjection::All => ProjectionSet::All,
        DuckDbProjection::Columns(columns) => ProjectionSet::Columns(
            columns
                .iter()
                .enumerate()
                .map(|(output_index, source_index)| ProjectionColumn {
                    source_index: *source_index,
                    output_index: output_index as u32,
                })
                .collect(),
        ),
    }
}

fn runtime_policy(policy: &DuckDbRuntimePolicy) -> RuntimeSafetyPolicy {
    RuntimeSafetyPolicy {
        fallback: if policy.allow_interpreter_fallback {
            RuntimeFallbackPolicy::AllowInterpreter
        } else {
            RuntimeFallbackPolicy::FailClosedOnly
        },
        unsupported_predicate: UnsupportedPredicatePolicy::FailClosed,
        concurrency: ConcurrencyPolicy::SingleWorker,
    }
}

fn attach_test_native_facts(
    report: ArtifactVerificationReport,
    test_facts: &DuckDbTestNativeFacts,
) -> ArtifactVerificationReport {
    if report.status() != ArtifactVerificationStatus::Accepted {
        return report;
    }
    let Some(mut facts) = report.into_facts() else {
        return ArtifactVerificationReport::accepted(ArtifactVerificationFacts::new("LMC1"));
    };
    facts.row_count_bound = Some(test_facts.row_count);
    facts.constraint_status = ConstraintDischargeStatus::Discharged;
    facts.l2_core = Some(VerifiedArtifactFacts {
        artifact_version: 1,
        required_features: vec!["test.duckdb-native".to_string()],
        optional_features: Vec::new(),
        accepted_feature_set: vec!["test.duckdb-native".to_string()],
        input_ranges: Vec::new(),
        output_schema: test_facts
            .columns
            .iter()
            .enumerate()
            .map(|(idx, data_type)| OutputSchemaFact {
                builder_id: format!("col{idx}"),
                arrow_type: data_type.clone(),
                nullable: false,
            })
            .collect(),
        row_count_bound: Some(test_facts.row_count),
        loop_bounds: Vec::new(),
        resource_bounds: ResourceBudget::bounded_rows(test_facts.row_count),
        builder_event_types: Vec::new(),
        capability_summary: Vec::new(),
        constraint_ids: Vec::new(),
        proof_obligation_ids: Vec::new(),
    });
    ArtifactVerificationReport::accepted(facts)
}

fn column_count_for(report: &ArtifactVerificationReport, input: &DuckDbRuntimePlanInput) -> u32 {
    if let Some(test_facts) = input.policy.test_native_facts.as_ref() {
        return test_facts.columns.len() as u32;
    }
    report
        .facts()
        .and_then(|facts| facts.l2_core.as_ref())
        .map(|facts| facts.output_schema.len() as u32)
        .unwrap_or(1)
}

fn row_count_for(report: &ArtifactVerificationReport) -> u64 {
    report
        .facts()
        .and_then(|facts| facts.row_count_bound)
        .unwrap_or(0)
}

fn constraint_status_for(report: &ArtifactVerificationReport) -> ConstraintDischargeStatus {
    report
        .facts()
        .map(|facts| facts.constraint_status)
        .unwrap_or(ConstraintDischargeStatus::Failed)
}

fn reader_support_for(report: &ArtifactVerificationReport) -> RuntimeReaderSupport {
    match report.status() {
        ArtifactVerificationStatus::Accepted => RuntimeReaderSupport::Accepted,
        ArtifactVerificationStatus::Unsupported => RuntimeReaderSupport::Unsupported,
        ArtifactVerificationStatus::Rejected => RuntimeReaderSupport::Rejected,
    }
}

fn emission_disposition_for(report: &ArtifactVerificationReport) -> RuntimeEmissionDisposition {
    match report
        .facts()
        .and_then(|facts| facts.payload_kind.as_deref())
    {
        Some("LMP1 layout") => RuntimeEmissionDisposition::CanonicalRaw,
        Some("LMT1 table") => RuntimeEmissionDisposition::CanonicalTable,
        _ => RuntimeEmissionDisposition::None,
    }
}

fn lowering_disposition_for(supported: bool) -> RuntimeLoweringDisposition {
    if supported {
        RuntimeLoweringDisposition::ProductionLoweringSupported
    } else {
        RuntimeLoweringDisposition::InterpreterOnly
    }
}

fn runtime_backend_identity() -> RuntimeBackendIdentity {
    let identity = NativeBackendIdentity::preflight_only();
    let backend_key = identity.as_key();
    RuntimeBackendIdentity {
        backend: NATIVE_BACKEND_NAME.to_string(),
        backend_version: identity.backend_version,
        toolchain: backend_key,
        target_triple: identity
            .target_triple
            .unwrap_or_else(|| "unknown".to_string()),
        cpu_features: identity.capabilities.supported_kernels,
    }
}

fn artifact_diagnostics(report: &ArtifactVerificationReport) -> Vec<DuckDbRuntimeDiagnostic> {
    report
        .diagnostics()
        .iter()
        .map(|diagnostic| DuckDbRuntimeDiagnostic {
            code: diagnostic.code.clone(),
            path: diagnostic.path.clone(),
            message: diagnostic.message.clone(),
        })
        .collect()
}

fn runtime_diagnostic(
    diagnostic: loom_core::runtime_abi::RuntimeDiagnostic,
) -> DuckDbRuntimeDiagnostic {
    DuckDbRuntimeDiagnostic {
        code: diagnostic.code.as_str().to_string(),
        path: diagnostic.path,
        message: diagnostic.message,
    }
}

fn duckdb_diagnostic_to_runtime(
    diagnostic: &DuckDbRuntimeDiagnostic,
) -> Option<loom_core::runtime_abi::RuntimeDiagnostic> {
    use loom_core::runtime_abi::{RuntimeDiagnostic, RuntimeDiagnosticCode};
    let code = match diagnostic.code.as_str() {
        "verifier-rejected" => RuntimeDiagnosticCode::VerifierRejected,
        "constraints-not-discharged" | "constraint-rejected" => {
            RuntimeDiagnosticCode::ConstraintRejected
        }
        "missing-artifact-facts" | "missing-l2-facts" | "missing-row-count-bound" => {
            RuntimeDiagnosticCode::MissingArtifactFacts
        }
        "lowering-unsupported"
        | "unsupported-type"
        | "unsupported-nullability"
        | "unsupported-payload"
        | "unsupported-shape"
        | "unsupported-multi-column-shape" => RuntimeDiagnosticCode::LoweringUnsupported,
        "fallback-disabled" => RuntimeDiagnosticCode::FallbackDisabled,
        "unsupported-projection" => RuntimeDiagnosticCode::UnsupportedProjection,
        "unsupported-predicate" => RuntimeDiagnosticCode::UnsupportedPredicate,
        "unsafe-concurrency" => RuntimeDiagnosticCode::UnsafeConcurrency,
        "cache-key-mismatch" => RuntimeDiagnosticCode::CacheKeyMismatch,
        "abi-mismatch" => RuntimeDiagnosticCode::AbiMismatch,
        "toolchain-mismatch" => RuntimeDiagnosticCode::ToolchainMismatch,
        "invalid-split" => RuntimeDiagnosticCode::InvalidSplit,
        _ => return None,
    };
    Some(RuntimeDiagnostic::new(
        code,
        diagnostic.path.clone(),
        diagnostic.message.clone(),
    ))
}

fn artifact_digest(bytes: &[u8]) -> String {
    format!("fnv1a64:{:016x}", stable_fnv1a64(bytes))
}

fn facts_fingerprint(report: &ArtifactVerificationReport) -> String {
    let Some(facts) = report.facts() else {
        return "facts:none".to_string();
    };
    format!(
        "kind={};payload={};rows={};constraints={};features={}",
        facts.artifact_kind,
        facts.payload_kind.as_deref().unwrap_or("none"),
        facts
            .row_count_bound
            .map(|row_count| row_count.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        facts.constraint_status.as_str(),
        facts.required_features.join("+")
    )
}

fn lowering_fingerprint(facts: Option<&ProductionLoweringFacts>) -> String {
    let Some(facts) = facts else {
        return "lowering:none".to_string();
    };
    format!(
        "backend={};payload={};rows={};columns={}",
        facts.backend.as_str(),
        facts.payload_kind,
        facts.shape.row_count(),
        facts.shape.columns().len()
    )
}

fn stable_fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
