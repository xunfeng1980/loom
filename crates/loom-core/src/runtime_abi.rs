//! Host-neutral runtime ABI and execution policy model.
//!
//! Phase 22 keeps this vocabulary inside `loom-core` so later host adapters can
//! consume one verifier-gated contract without importing host engine, Vortex, or
//! native compiler types.

use std::fmt;

use crate::artifact_verifier::{ArtifactVerificationStatus, ConstraintDischargeStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuntimeAbiVersion {
    pub major: u16,
    pub minor: u16,
}

impl RuntimeAbiVersion {
    pub const CURRENT: Self = Self { major: 0, minor: 1 };

    pub fn as_key(self) -> String {
        format!("{}.{}", self.major, self.minor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeHandleKind {
    Plan,
    Scan,
    Worker,
    Batch,
}

impl RuntimeHandleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Scan => "scan",
            Self::Worker => "worker",
            Self::Batch => "batch",
        }
    }
}

impl fmt::Display for RuntimeHandleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeExecutionDecision {
    NativeCandidate,
    InterpreterFallback,
    FailClosed,
    DiagnosticOnly,
}

impl RuntimeExecutionDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeCandidate => "native-candidate",
            Self::InterpreterFallback => "interpreter-fallback",
            Self::FailClosed => "fail-closed",
            Self::DiagnosticOnly => "diagnostic-only",
        }
    }
}

impl fmt::Display for RuntimeExecutionDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeFallbackPolicy {
    FailClosedOnly,
    AllowInterpreter,
    DiagnosticOnly,
}

impl RuntimeFallbackPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FailClosedOnly => "fail-closed-only",
            Self::AllowInterpreter => "allow-interpreter",
            Self::DiagnosticOnly => "diagnostic-only",
        }
    }

    pub fn allows_interpreter(self) -> bool {
        matches!(self, Self::AllowInterpreter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnsupportedPredicatePolicy {
    ScanAll,
    FailClosed,
}

impl UnsupportedPredicatePolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ScanAll => "scan-all",
            Self::FailClosed => "fail-closed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConcurrencyPolicy {
    SingleWorker,
    SerializeSharedScan,
    ParallelSplits { requested_workers: u16 },
}

impl ConcurrencyPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SingleWorker => "single-worker",
            Self::SerializeSharedScan => "serialize-shared-scan",
            Self::ParallelSplits { .. } => "parallel-splits",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuntimeSafetyPolicy {
    pub fallback: RuntimeFallbackPolicy,
    pub unsupported_predicate: UnsupportedPredicatePolicy,
    pub concurrency: ConcurrencyPolicy,
}

impl Default for RuntimeSafetyPolicy {
    fn default() -> Self {
        Self {
            fallback: RuntimeFallbackPolicy::FailClosedOnly,
            unsupported_predicate: UnsupportedPredicatePolicy::FailClosed,
            concurrency: ConcurrencyPolicy::SingleWorker,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectionColumn {
    pub source_index: u32,
    pub output_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProjectionSet {
    All,
    Columns(Vec<ProjectionColumn>),
}

impl ProjectionSet {
    pub fn as_key(&self) -> String {
        match self {
            Self::All => "all".to_string(),
            Self::Columns(columns) => {
                let items = columns
                    .iter()
                    .map(|column| format!("{}>{}", column.source_index, column.output_index))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("columns:{items}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionPlan {
    pub projection: ProjectionSet,
    pub output_to_source: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PredicateOperator {
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
}

impl PredicateOperator {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::NotEq => "not-eq",
            Self::Lt => "lt",
            Self::LtEq => "lt-eq",
            Self::Gt => "gt",
            Self::GtEq => "gt-eq",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PredicateEnvelope {
    None,
    PrimitiveComparison {
        column_index: u32,
        op: PredicateOperator,
        literal_i64: i64,
    },
    Unsupported {
        reason: String,
    },
}

impl PredicateEnvelope {
    pub fn as_key(&self) -> String {
        match self {
            Self::None => "none".to_string(),
            Self::PrimitiveComparison {
                column_index,
                op,
                literal_i64,
            } => format!("cmp:{column_index}:{}:{literal_i64}", op.as_str()),
            Self::Unsupported { reason } => format!("unsupported:{reason}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SplitDescriptor {
    FullScan { row_count: u64 },
    RowRange { start: u64, end: u64 },
}

impl SplitDescriptor {
    pub fn as_key(self) -> String {
        match self {
            Self::FullScan { row_count } => format!("full:{row_count}"),
            Self::RowRange { start, end } => format!("range:{start}:{end}"),
        }
    }

    pub fn is_empty(self) -> bool {
        matches!(self, Self::RowRange { start, end } if start >= end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanShape {
    pub column_count: u32,
    pub row_count: u64,
    pub splittable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeReaderSupport {
    Accepted,
    Unsupported,
    Rejected,
}

impl RuntimeReaderSupport {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Unsupported => "unsupported",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeEmissionDisposition {
    None,
    CanonicalRaw,
    CanonicalTable,
    StructuredLayout,
}

impl RuntimeEmissionDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::CanonicalRaw => "canonical-raw",
            Self::CanonicalTable => "canonical-table",
            Self::StructuredLayout => "structured-layout",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeLoweringDisposition {
    InterpreterOnly,
    ProductionLoweringSupported,
    FailClosedDeferred,
}

impl RuntimeLoweringDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InterpreterOnly => "interpreter-only",
            Self::ProductionLoweringSupported => "production-lowering-supported",
            Self::FailClosedDeferred => "fail-closed/deferred",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeDiagnosticCode {
    VerifierRejected,
    ConstraintRejected,
    MissingArtifactFacts,
    LoweringUnsupported,
    FallbackDisabled,
    UnsupportedProjection,
    UnsupportedPredicate,
    UnsafeConcurrency,
    CacheKeyMismatch,
    AbiMismatch,
    ToolchainMismatch,
    InvalidSplit,
}

impl RuntimeDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VerifierRejected => "verifier-rejected",
            Self::ConstraintRejected => "constraint-rejected",
            Self::MissingArtifactFacts => "missing-artifact-facts",
            Self::LoweringUnsupported => "lowering-unsupported",
            Self::FallbackDisabled => "fallback-disabled",
            Self::UnsupportedProjection => "unsupported-projection",
            Self::UnsupportedPredicate => "unsupported-predicate",
            Self::UnsafeConcurrency => "unsafe-concurrency",
            Self::CacheKeyMismatch => "cache-key-mismatch",
            Self::AbiMismatch => "abi-mismatch",
            Self::ToolchainMismatch => "toolchain-mismatch",
            Self::InvalidSplit => "invalid-split",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDiagnostic {
    pub code: RuntimeDiagnosticCode,
    pub path: String,
    pub message: String,
}

impl RuntimeDiagnostic {
    pub fn new(
        code: RuntimeDiagnosticCode,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDecisionInput {
    pub artifact_status: ArtifactVerificationStatus,
    pub constraint_status: ConstraintDischargeStatus,
    pub production_lowering_supported: bool,
    pub reader_support: RuntimeReaderSupport,
    pub emission_disposition: RuntimeEmissionDisposition,
    pub lowering_disposition: RuntimeLoweringDisposition,
    pub projection_supported: bool,
    pub predicate_supported: bool,
    pub split_supported: bool,
    pub concurrency_safe: bool,
    pub policy: RuntimeSafetyPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePlanDecisionReport {
    pub decision: RuntimeExecutionDecision,
    pub diagnostics: Vec<RuntimeDiagnostic>,
}

impl RuntimePlanDecisionReport {
    pub fn is_native_candidate(&self) -> bool {
        self.decision == RuntimeExecutionDecision::NativeCandidate && self.diagnostics.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePlanRequest {
    pub abi_version: RuntimeAbiVersion,
    pub artifact_digest: String,
    pub projection: ProjectionSet,
    pub predicate: PredicateEnvelope,
    pub split: SplitDescriptor,
    pub policy: RuntimeSafetyPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePlan {
    pub abi_version: RuntimeAbiVersion,
    pub decision: RuntimeExecutionDecision,
    pub projection: ProjectionSet,
    pub predicate: PredicateEnvelope,
    pub split: SplitDescriptor,
    pub diagnostics: Vec<RuntimeDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuntimeBackendIdentity {
    pub backend: String,
    pub backend_version: String,
    pub toolchain: String,
    pub target_triple: String,
    pub cpu_features: Vec<String>,
}

impl RuntimeBackendIdentity {
    pub fn as_key(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}",
            self.backend,
            self.backend_version,
            self.toolchain,
            self.target_triple,
            self.cpu_features.join("+")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuntimeCacheKeyInput {
    pub abi_version: RuntimeAbiVersion,
    pub artifact_digest: String,
    pub facts_fingerprint: String,
    pub solver_identity: String,
    pub production_lowering_fingerprint: String,
    pub backend_identity: RuntimeBackendIdentity,
    pub projection: ProjectionSet,
    pub predicate: PredicateEnvelope,
    pub split: SplitDescriptor,
    pub policy: RuntimeSafetyPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuntimeCacheKey {
    pub stable_id: String,
    pub canonical_input: String,
}

impl RuntimeCacheKey {
    pub fn build(input: &RuntimeCacheKeyInput) -> Self {
        let canonical_input = canonical_cache_input(input);
        let hash = stable_fnv1a64(canonical_input.as_bytes());
        Self {
            stable_id: format!("loom-runtime-v{}-{hash:016x}", input.abi_version.as_key()),
            canonical_input,
        }
    }
}

fn canonical_cache_input(input: &RuntimeCacheKeyInput) -> String {
    format!(
        "abi={};artifact={};facts={};solver={};lowering={};backend={};projection={};predicate={};split={};fallback={};predicate_policy={};concurrency={}",
        input.abi_version.as_key(),
        input.artifact_digest,
        input.facts_fingerprint,
        input.solver_identity,
        input.production_lowering_fingerprint,
        input.backend_identity.as_key(),
        input.projection.as_key(),
        input.predicate.as_key(),
        input.split.as_key(),
        input.policy.fallback.as_str(),
        input.policy.unsupported_predicate.as_str(),
        input.policy.concurrency.as_str(),
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

pub fn plan_projection(
    projection: &ProjectionSet,
    column_count: u32,
) -> Result<ProjectionPlan, RuntimeDiagnostic> {
    match projection {
        ProjectionSet::All => Ok(ProjectionPlan {
            projection: ProjectionSet::All,
            output_to_source: (0..column_count).collect(),
        }),
        ProjectionSet::Columns(columns) => {
            if columns.is_empty() {
                return Err(RuntimeDiagnostic::new(
                    RuntimeDiagnosticCode::UnsupportedProjection,
                    "$.projection.columns",
                    "projection must include at least one column",
                ));
            }

            let mut output_seen = std::collections::BTreeSet::new();
            let mut source_seen = std::collections::BTreeSet::new();
            for column in columns {
                if column.source_index >= column_count {
                    return Err(RuntimeDiagnostic::new(
                        RuntimeDiagnosticCode::UnsupportedProjection,
                        "$.projection.columns",
                        format!(
                            "projected source column {} exceeds column count {}",
                            column.source_index, column_count
                        ),
                    ));
                }
                if !output_seen.insert(column.output_index) {
                    return Err(RuntimeDiagnostic::new(
                        RuntimeDiagnosticCode::UnsupportedProjection,
                        "$.projection.columns",
                        format!("duplicate output column {}", column.output_index),
                    ));
                }
                if !source_seen.insert(column.source_index) {
                    return Err(RuntimeDiagnostic::new(
                        RuntimeDiagnosticCode::UnsupportedProjection,
                        "$.projection.columns",
                        format!("duplicate source column {}", column.source_index),
                    ));
                }
            }

            let mut sorted = columns.clone();
            sorted.sort_by_key(|column| column.output_index);
            Ok(ProjectionPlan {
                projection: ProjectionSet::Columns(columns.clone()),
                output_to_source: sorted
                    .iter()
                    .map(|column| column.source_index)
                    .collect::<Vec<_>>(),
            })
        }
    }
}

pub fn plan_predicate(
    predicate: &PredicateEnvelope,
    policy: UnsupportedPredicatePolicy,
) -> Result<bool, RuntimeDiagnostic> {
    match predicate {
        PredicateEnvelope::None => Ok(true),
        PredicateEnvelope::PrimitiveComparison { .. } | PredicateEnvelope::Unsupported { .. } => {
            let diagnostic = RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::UnsupportedPredicate,
                "$.predicate",
                "Phase 22 records predicate envelopes but does not push predicates down",
            );
            match policy {
                UnsupportedPredicatePolicy::ScanAll => Ok(false),
                UnsupportedPredicatePolicy::FailClosed => Err(diagnostic),
            }
        }
    }
}

pub fn plan_split(
    split: SplitDescriptor,
    shape: ScanShape,
    concurrency: ConcurrencyPolicy,
) -> Result<SplitDescriptor, RuntimeDiagnostic> {
    if split.is_empty() {
        return Err(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::InvalidSplit,
            "$.split",
            "split row range must be non-empty",
        ));
    }

    match split {
        SplitDescriptor::FullScan { row_count } if row_count != shape.row_count => {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::InvalidSplit,
                "$.split.row_count",
                format!(
                    "full scan row count {} does not match shape row count {}",
                    row_count, shape.row_count
                ),
            ));
        }
        SplitDescriptor::RowRange { end, .. } if end > shape.row_count => {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::InvalidSplit,
                "$.split.end",
                format!(
                    "split end {} exceeds shape row count {}",
                    end, shape.row_count
                ),
            ));
        }
        _ => {}
    }

    if matches!(concurrency, ConcurrencyPolicy::ParallelSplits { .. }) && !shape.splittable {
        return Err(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::UnsafeConcurrency,
            "$.concurrency",
            "parallel split execution requires a splittable scan",
        ));
    }

    Ok(split)
}

pub fn decide_runtime_execution(input: &RuntimeDecisionInput) -> RuntimePlanDecisionReport {
    let mut diagnostics = Vec::new();

    if input.artifact_status != ArtifactVerificationStatus::Accepted {
        diagnostics.push(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::VerifierRejected,
            "$.artifact.status",
            "runtime planning requires an accepted artifact verifier report",
        ));
        return fail_or_diagnostic(input.policy.fallback, diagnostics);
    }

    if !matches!(
        input.constraint_status,
        ConstraintDischargeStatus::Discharged | ConstraintDischargeStatus::NotRequired
    ) {
        diagnostics.push(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::ConstraintRejected,
            "$.artifact.constraint_status",
            format!(
                "native runtime planning requires discharged or not-required constraints, got {}",
                input.constraint_status.as_str()
            ),
        ));
        if input.policy.fallback.allows_interpreter()
            && input.reader_support == RuntimeReaderSupport::Accepted
            && input.emission_disposition != RuntimeEmissionDisposition::None
        {
            return RuntimePlanDecisionReport {
                decision: RuntimeExecutionDecision::InterpreterFallback,
                diagnostics,
            };
        }
        return fail_or_diagnostic(input.policy.fallback, diagnostics);
    }

    if input.reader_support != RuntimeReaderSupport::Accepted {
        diagnostics.push(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::VerifierRejected,
            "$.reader.support",
            format!(
                "runtime planning requires accepted reader support, got {}",
                input.reader_support.as_str()
            ),
        ));
        return fail_or_diagnostic(input.policy.fallback, diagnostics);
    }

    if input.emission_disposition == RuntimeEmissionDisposition::None {
        diagnostics.push(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::MissingArtifactFacts,
            "$.reader.emission_disposition",
            "runtime planning requires an emitted Loom artifact",
        ));
        return fail_or_diagnostic(input.policy.fallback, diagnostics);
    }

    if !input.projection_supported {
        diagnostics.push(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::UnsupportedProjection,
            "$.projection",
            "runtime projection is not supported by this artifact",
        ));
        return fail_or_diagnostic(input.policy.fallback, diagnostics);
    }

    if !input.predicate_supported {
        diagnostics.push(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::UnsupportedPredicate,
            "$.predicate",
            "runtime predicate envelope is not supported",
        ));
        if input.policy.unsupported_predicate == UnsupportedPredicatePolicy::FailClosed {
            return fail_or_diagnostic(input.policy.fallback, diagnostics);
        }
    }

    if !input.split_supported {
        diagnostics.push(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::InvalidSplit,
            "$.split",
            "runtime split descriptor is not supported",
        ));
        return fail_or_diagnostic(input.policy.fallback, diagnostics);
    }

    if !input.concurrency_safe {
        diagnostics.push(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::UnsafeConcurrency,
            "$.concurrency",
            "runtime concurrency request is unsafe for this scan",
        ));
        return fail_or_diagnostic(input.policy.fallback, diagnostics);
    }

    match input.lowering_disposition {
        RuntimeLoweringDisposition::ProductionLoweringSupported
            if input.production_lowering_supported =>
        {
            RuntimePlanDecisionReport {
                decision: RuntimeExecutionDecision::NativeCandidate,
                diagnostics,
            }
        }
        RuntimeLoweringDisposition::InterpreterOnly
            if input.policy.fallback.allows_interpreter() =>
        {
            diagnostics.push(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::LoweringUnsupported,
                "$.lowering.disposition",
                "artifact is accepted but marked interpreter-only for native lowering",
            ));
            RuntimePlanDecisionReport {
                decision: RuntimeExecutionDecision::InterpreterFallback,
                diagnostics,
            }
        }
        RuntimeLoweringDisposition::FailClosedDeferred => {
            diagnostics.push(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::LoweringUnsupported,
                "$.lowering.disposition",
                "artifact lowering is explicitly fail-closed/deferred",
            ));
            fail_or_diagnostic(input.policy.fallback, diagnostics)
        }
        _ => {
            diagnostics.push(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::FallbackDisabled,
                "$.policy.fallback",
                "native lowering is unavailable and interpreter fallback is disabled",
            ));
            fail_or_diagnostic(input.policy.fallback, diagnostics)
        }
    }
}

fn fail_or_diagnostic(
    fallback: RuntimeFallbackPolicy,
    diagnostics: Vec<RuntimeDiagnostic>,
) -> RuntimePlanDecisionReport {
    RuntimePlanDecisionReport {
        decision: match fallback {
            RuntimeFallbackPolicy::DiagnosticOnly => RuntimeExecutionDecision::DiagnosticOnly,
            RuntimeFallbackPolicy::FailClosedOnly | RuntimeFallbackPolicy::AllowInterpreter => {
                RuntimeExecutionDecision::FailClosed
            }
        },
        diagnostics,
    }
}
