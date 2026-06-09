//! Engine-neutral native execution for Arrow semantic artifacts.
//!
//! Phase 35 deliberately keeps this backend out of host adapter and FFI code. The
//! executor verifies `LMC2(LMA1)` or explicit direct `LMA1` bytes, decodes the
//! Arrow semantic payload, copies supported fixed-width primitive columns
//! through typed Arrow builders, and can compare the result with the decoded
//! reference batch.

use std::sync::Arc;

use arrow_array::{
    types::{Float32Type, Float64Type, Int32Type, Int64Type},
    Array, ArrayRef, BooleanArray, PrimitiveArray, RecordBatch,
};
use arrow_schema::DataType;

use crate::arrow_builder_output::OutputBuilder;
use crate::arrow_semantic_codec::{
    decode_arrow_semantic_container_payload, decode_arrow_semantic_payload,
    is_arrow_semantic_container, is_arrow_semantic_payload,
};
use crate::artifact_verifier::{
    verify_artifact, ArtifactVerificationOptions, ArtifactVerificationReport,
    ArtifactVerificationStatus,
};
use crate::l2_core::{
    Capability, L2CoreProgram, L2CoreStmt, OutputBuilderCapability, ResourceBudget, ScalarExpr,
    ScalarValue,
};
use crate::l2_core_reference_executor::{execute_reference, ReferenceStatus};
use crate::l2_kernel_registry::L2KernelRegistry;
use crate::runtime_abi::{
    decide_runtime_execution, PredicateEnvelope, ProjectionSet, RuntimeAbiVersion,
    RuntimeBackendIdentity, RuntimeCacheKey, RuntimeCacheKeyInput, RuntimeDiagnosticCode,
    RuntimeEmissionDisposition, RuntimeExecutionDecision, RuntimeFallbackPolicy,
    RuntimeLoweringDisposition, RuntimePlanDecisionReport, RuntimeReaderSupport,
    RuntimeSafetyPolicy, SplitDescriptor,
};

pub const NATIVE_ARROW_SEMANTIC_BACKEND: &str = "loom-native-arrow-semantic";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeArrowSemanticDiagnosticCode {
    VerifierRejected,
    UnsupportedArtifact,
    UnsupportedPayload,
    UnsupportedBatchShape,
    UnsupportedType,
    NativeOutputMismatch,
    NativeModelTraceMismatch,
}

impl NativeArrowSemanticDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VerifierRejected => "verifier-rejected",
            Self::UnsupportedArtifact => "unsupported-artifact",
            Self::UnsupportedPayload => "unsupported-payload",
            Self::UnsupportedBatchShape => "unsupported-batch-shape",
            Self::UnsupportedType => "unsupported-type",
            Self::NativeOutputMismatch => "native-output-mismatch",
            Self::NativeModelTraceMismatch => "native-model-trace-mismatch",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeArrowSemanticDiagnostic {
    pub code: NativeArrowSemanticDiagnosticCode,
    pub path: String,
    pub message: String,
}

impl NativeArrowSemanticDiagnostic {
    fn new(
        code: NativeArrowSemanticDiagnosticCode,
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

#[derive(Debug, Clone)]
pub struct NativeArrowSemanticExecutionReport {
    pub backend: String,
    pub artifact_kind: String,
    pub payload_kind: String,
    pub row_count: u64,
    pub column_count: usize,
    output: Option<RecordBatch>,
    diagnostics: Vec<NativeArrowSemanticDiagnostic>,
}

impl NativeArrowSemanticExecutionReport {
    pub fn is_supported(&self) -> bool {
        self.diagnostics.is_empty() && self.output.is_some()
    }

    pub fn output(&self) -> Option<&RecordBatch> {
        self.output.as_ref()
    }

    pub fn diagnostics(&self) -> &[NativeArrowSemanticDiagnostic] {
        &self.diagnostics
    }

    pub fn first_error(&self) -> Option<&NativeArrowSemanticDiagnostic> {
        self.diagnostics.first()
    }

    fn rejected(diagnostic: NativeArrowSemanticDiagnostic) -> Self {
        Self {
            backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
            artifact_kind: String::new(),
            payload_kind: String::new(),
            row_count: 0,
            column_count: 0,
            output: None,
            diagnostics: vec![diagnostic],
        }
    }

}

#[derive(Debug, Clone)]
pub struct NativeArrowSemanticEquivalenceReport {
    pub backend: String,
    pub artifact_kind: String,
    pub row_count: u64,
    pub column_count: usize,
    pub equivalent: bool,
    diagnostics: Vec<NativeArrowSemanticDiagnostic>,
}

impl NativeArrowSemanticEquivalenceReport {
    pub fn is_equivalent(&self) -> bool {
        self.equivalent && self.diagnostics.is_empty()
    }

    pub fn diagnostics(&self) -> &[NativeArrowSemanticDiagnostic] {
        &self.diagnostics
    }
}

#[derive(Debug, Clone)]
pub struct NativeArrowSemanticModelValidationReport {
    pub backend: String,
    pub artifact_kind: String,
    pub row_count: u64,
    pub column_count: usize,
    pub model_trace_matches: bool,
    pub value_equivalent: bool,
    reference_trace: Vec<String>,
    native_trace: Vec<String>,
    diagnostics: Vec<NativeArrowSemanticDiagnostic>,
}

impl NativeArrowSemanticModelValidationReport {
    pub fn is_validated(&self) -> bool {
        self.model_trace_matches && self.value_equivalent && self.diagnostics.is_empty()
    }

    pub fn reference_trace(&self) -> &[String] {
        &self.reference_trace
    }

    pub fn native_trace(&self) -> &[String] {
        &self.native_trace
    }

    pub fn diagnostics(&self) -> &[NativeArrowSemanticDiagnostic] {
        &self.diagnostics
    }

    pub fn first_error(&self) -> Option<&NativeArrowSemanticDiagnostic> {
        self.diagnostics.first()
    }
}

pub fn execute_native_arrow_semantic(
    bytes: &[u8],
) -> NativeArrowSemanticExecutionReport {
    execute_native_arrow_semantic_with_options(bytes, &ArtifactVerificationOptions::default())
}

pub fn execute_native_arrow_semantic_with_options(
    bytes: &[u8],
    options: &ArtifactVerificationOptions,
) -> NativeArrowSemanticExecutionReport {
    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_artifact(bytes, &registry, options);
    execute_verified_native_arrow_semantic(bytes, &verification)
}

pub fn execute_verified_native_arrow_semantic(
    bytes: &[u8],
    verification: &ArtifactVerificationReport,
) -> NativeArrowSemanticExecutionReport {
    if verification.status() != ArtifactVerificationStatus::Accepted || !verification.is_ok() {
        return NativeArrowSemanticExecutionReport::rejected(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::VerifierRejected,
            "$.verification",
            "native Arrow semantic execution requires an accepted artifact verifier report",
        ));
    }

    let Some(facts) = verification.facts() else {
        return NativeArrowSemanticExecutionReport::rejected(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::VerifierRejected,
            "$.facts",
            "accepted artifact verifier report did not expose facts",
        ));
    };

    if !matches!(facts.artifact_kind.as_str(), "LMC2" | "LMA1") {
        return NativeArrowSemanticExecutionReport::rejected(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedArtifact,
            "$.facts.artifact_kind",
            format!(
                "unsupported artifact kind '{}'; expected LMC2 or LMA1",
                facts.artifact_kind
            ),
        ));
    }

    if facts.payload_kind.as_deref() != Some("Arrow semantic payload") {
        return NativeArrowSemanticExecutionReport::rejected(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedPayload,
            "$.facts.payload_kind",
            "native Arrow semantic execution requires an Arrow semantic payload",
        ));
    }

    let reference = match decode_reference_batch(bytes) {
        Ok(batch) => batch,
        Err(diagnostic) => return NativeArrowSemanticExecutionReport::rejected(diagnostic),
    };

    let mut copied_columns = Vec::with_capacity(reference.num_columns());
    for (idx, column) in reference.columns().iter().enumerate() {
        match copy_supported_column(column.as_ref(), idx) {
            Ok(array) => copied_columns.push(array),
            Err(diagnostic) => return NativeArrowSemanticExecutionReport::rejected(diagnostic),
        }
    }

    let row_count = reference.num_rows() as u64;
    let column_count = reference.num_columns();
    let output = match RecordBatch::try_new(reference.schema(), copied_columns) {
        Ok(batch) => batch,
        Err(_) => {
            return NativeArrowSemanticExecutionReport::rejected(NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
                "$.native.output",
                "native Arrow semantic output batch construction failed",
            ));
        }
    };

    NativeArrowSemanticExecutionReport {
        backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
        artifact_kind: facts.artifact_kind.clone(),
        payload_kind: facts.payload_kind.clone().unwrap_or_default(),
        row_count,
        column_count,
        output: Some(output),
        diagnostics: Vec::new(),
    }
}

pub fn verify_native_arrow_semantic_equivalence(
    bytes: &[u8],
) -> NativeArrowSemanticEquivalenceReport {
    let execution = execute_native_arrow_semantic(bytes);
    verify_native_arrow_semantic_equivalence_from_execution(bytes, &execution)
}

pub fn verify_native_arrow_semantic_equivalence_from_execution(
    bytes: &[u8],
    execution: &NativeArrowSemanticExecutionReport,
) -> NativeArrowSemanticEquivalenceReport {
    if !execution.is_supported() {
        return NativeArrowSemanticEquivalenceReport {
            backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
            artifact_kind: execution.artifact_kind.clone(),
            row_count: execution.row_count,
            column_count: execution.column_count,
            equivalent: false,
            diagnostics: execution.diagnostics.clone(),
        };
    }

    let reference = match decode_reference_batch(bytes) {
        Ok(batch) => batch,
        Err(diagnostic) => {
            return NativeArrowSemanticEquivalenceReport {
                backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
                artifact_kind: execution.artifact_kind.clone(),
                row_count: execution.row_count,
                column_count: execution.column_count,
                equivalent: false,
                diagnostics: vec![diagnostic],
            };
        }
    };

    let output = execution
        .output()
        .expect("supported execution report must expose output");
    let equivalent = output == &reference;
    let diagnostics = if equivalent {
        Vec::new()
    } else {
        vec![NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            "$.native.output",
            "native Arrow semantic output does not match decoded reference batch",
        )]
    };

    NativeArrowSemanticEquivalenceReport {
        backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
        artifact_kind: execution.artifact_kind.clone(),
        row_count: execution.row_count,
        column_count: execution.column_count,
        equivalent,
        diagnostics,
    }
}

pub fn verify_native_arrow_semantic_output_equivalence(
    bytes: &[u8],
    artifact_kind: impl Into<String>,
    output: &RecordBatch,
) -> NativeArrowSemanticEquivalenceReport {
    verify_native_arrow_semantic_equivalence_for_output(bytes, artifact_kind.into(), output)
}

pub fn verify_native_arrow_semantic_model(
    bytes: &[u8],
) -> NativeArrowSemanticModelValidationReport {
    let execution = execute_native_arrow_semantic(bytes);
    verify_native_arrow_semantic_model_from_execution(bytes, &execution)
}

pub fn verify_native_arrow_semantic_model_from_execution(
    bytes: &[u8],
    execution: &NativeArrowSemanticExecutionReport,
) -> NativeArrowSemanticModelValidationReport {
    if !execution.is_supported() {
        return NativeArrowSemanticModelValidationReport {
            backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
            artifact_kind: execution.artifact_kind.clone(),
            row_count: execution.row_count,
            column_count: execution.column_count,
            model_trace_matches: false,
            value_equivalent: false,
            reference_trace: Vec::new(),
            native_trace: Vec::new(),
            diagnostics: execution.diagnostics.clone(),
        };
    }

    let output = execution
        .output()
        .expect("supported execution report must expose output");
    verify_native_arrow_semantic_model_for_output(bytes, execution.artifact_kind.clone(), output)
}

pub fn verify_native_arrow_semantic_model_output(
    bytes: &[u8],
    artifact_kind: impl Into<String>,
    output: &RecordBatch,
) -> NativeArrowSemanticModelValidationReport {
    verify_native_arrow_semantic_model_for_output(bytes, artifact_kind.into(), output)
}

fn verify_native_arrow_semantic_equivalence_for_output(
    bytes: &[u8],
    artifact_kind: String,
    output: &RecordBatch,
) -> NativeArrowSemanticEquivalenceReport {
    let reference = match decode_reference_batch(bytes) {
        Ok(batch) => batch,
        Err(diagnostic) => {
            return NativeArrowSemanticEquivalenceReport {
                backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
                artifact_kind,
                row_count: output.num_rows() as u64,
                column_count: output.num_columns(),
                equivalent: false,
                diagnostics: vec![diagnostic],
            };
        }
    };

    let equivalent = output == &reference;
    let diagnostics = if equivalent {
        Vec::new()
    } else {
        vec![NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            "$.native.output",
            "native Arrow semantic output does not match decoded reference batch",
        )]
    };

    NativeArrowSemanticEquivalenceReport {
        backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
        artifact_kind,
        row_count: output.num_rows() as u64,
        column_count: output.num_columns(),
        equivalent,
        diagnostics,
    }
}

fn verify_native_arrow_semantic_model_for_output(
    bytes: &[u8],
    artifact_kind: String,
    output: &RecordBatch,
) -> NativeArrowSemanticModelValidationReport {
    let reference = match decode_reference_batch(bytes) {
        Ok(batch) => batch,
        Err(diagnostic) => {
            return NativeArrowSemanticModelValidationReport {
                backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
                artifact_kind,
                row_count: output.num_rows() as u64,
                column_count: output.num_columns(),
                model_trace_matches: false,
                value_equivalent: false,
                reference_trace: Vec::new(),
                native_trace: Vec::new(),
                diagnostics: vec![diagnostic],
            };
        }
    };

    let reference_trace = match reference_model_trace_for_batch(&reference) {
        Ok(trace) => trace,
        Err(diagnostic) => {
            return NativeArrowSemanticModelValidationReport {
                backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
                artifact_kind,
                row_count: output.num_rows() as u64,
                column_count: output.num_columns(),
                model_trace_matches: false,
                value_equivalent: false,
                reference_trace: Vec::new(),
                native_trace: Vec::new(),
                diagnostics: vec![diagnostic],
            };
        }
    };

    let native_trace = match native_model_trace_for_batch(output) {
        Ok(trace) => trace,
        Err(diagnostic) => {
            return NativeArrowSemanticModelValidationReport {
                backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
                artifact_kind,
                row_count: output.num_rows() as u64,
                column_count: output.num_columns(),
                model_trace_matches: false,
                value_equivalent: false,
                reference_trace,
                native_trace: Vec::new(),
                diagnostics: vec![diagnostic],
            };
        }
    };

    let model_trace_matches = reference_trace == native_trace;
    let value_equivalent = output == &reference;
    let mut diagnostics = Vec::new();
    if !model_trace_matches {
        diagnostics.push(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch,
            "$.native.model_trace",
            "native Arrow semantic output trace does not match reference executor trace",
        ));
    }
    if !value_equivalent {
        diagnostics.push(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            "$.native.output",
            "native Arrow semantic output does not match decoded reference batch",
        ));
    }

    NativeArrowSemanticModelValidationReport {
        backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
        artifact_kind,
        row_count: output.num_rows() as u64,
        column_count: output.num_columns(),
        model_trace_matches,
        value_equivalent,
        reference_trace,
        native_trace,
        diagnostics,
    }
}

pub fn native_arrow_semantic_backend_identity() -> RuntimeBackendIdentity {
    RuntimeBackendIdentity {
        backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
        backend_version: "phase35".to_string(),
        toolchain: "rust-arrow-rs-58.3".to_string(),
        target_triple: "engine-neutral".to_string(),
        cpu_features: Vec::new(),
    }
}

pub fn decide_native_arrow_semantic_runtime(
    execution: &NativeArrowSemanticExecutionReport,
    policy: RuntimeSafetyPolicy,
) -> RuntimePlanDecisionReport {
    let verifier_rejected = execution
        .first_error()
        .is_some_and(|diagnostic| diagnostic.code == NativeArrowSemanticDiagnosticCode::VerifierRejected);
    decide_runtime_execution(&crate::runtime_abi::RuntimeDecisionInput {
        artifact_status: if verifier_rejected {
            ArtifactVerificationStatus::Rejected
        } else {
            ArtifactVerificationStatus::Accepted
        },
        constraint_status: crate::artifact_verifier::ConstraintDischargeStatus::NotRequired,
        production_lowering_supported: execution.is_supported(),
        reader_support: if verifier_rejected {
            RuntimeReaderSupport::Rejected
        } else {
            RuntimeReaderSupport::Accepted
        },
        emission_disposition: RuntimeEmissionDisposition::SemanticArrow,
        lowering_disposition: if execution.is_supported() {
            RuntimeLoweringDisposition::ProductionLoweringSupported
        } else {
            RuntimeLoweringDisposition::InterpreterOnly
        },
        projection_supported: true,
        predicate_supported: true,
        split_supported: true,
        concurrency_safe: true,
        policy,
    })
}

pub fn decide_validated_native_arrow_semantic_runtime(
    validation: &NativeArrowSemanticModelValidationReport,
    policy: RuntimeSafetyPolicy,
) -> RuntimePlanDecisionReport {
    let verifier_rejected = validation.first_error().is_some_and(|diagnostic| {
        diagnostic.code == NativeArrowSemanticDiagnosticCode::VerifierRejected
    });
    decide_runtime_execution(&crate::runtime_abi::RuntimeDecisionInput {
        artifact_status: if verifier_rejected {
            ArtifactVerificationStatus::Rejected
        } else {
            ArtifactVerificationStatus::Accepted
        },
        constraint_status: crate::artifact_verifier::ConstraintDischargeStatus::NotRequired,
        production_lowering_supported: validation.is_validated(),
        reader_support: if verifier_rejected {
            RuntimeReaderSupport::Rejected
        } else {
            RuntimeReaderSupport::Accepted
        },
        emission_disposition: RuntimeEmissionDisposition::SemanticArrow,
        lowering_disposition: if validation.is_validated() {
            RuntimeLoweringDisposition::ProductionLoweringSupported
        } else {
            RuntimeLoweringDisposition::InterpreterOnly
        },
        projection_supported: true,
        predicate_supported: true,
        split_supported: true,
        concurrency_safe: true,
        policy,
    })
}

pub fn native_arrow_semantic_runtime_cache_key(
    bytes: &[u8],
    execution: &NativeArrowSemanticExecutionReport,
    projection: ProjectionSet,
    policy: RuntimeSafetyPolicy,
) -> Result<RuntimeCacheKey, NativeArrowSemanticDiagnostic> {
    let decision = decide_native_arrow_semantic_runtime(execution, policy);
    if decision.decision != RuntimeExecutionDecision::NativeCandidate || !execution.is_supported() {
        let fallback_disabled = decision
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == RuntimeDiagnosticCode::FallbackDisabled);
        let fallback_note = if fallback_disabled
            && matches!(policy.fallback, RuntimeFallbackPolicy::FailClosedOnly)
        {
            " and interpreter fallback is disabled"
        } else {
            ""
        };
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedPayload,
            "$.cache.native_arrow_semantic",
            format!(
                "only accepted native Arrow semantic executions may seed runtime cache keys{fallback_note}"
            ),
        ));
    }

    Ok(RuntimeCacheKey::build(&RuntimeCacheKeyInput {
        abi_version: RuntimeAbiVersion::CURRENT,
        artifact_digest: stable_digest("artifact", bytes),
        facts_fingerprint: format!(
            "artifact_kind={};payload_kind={};rows={};columns={}",
            execution.artifact_kind, execution.payload_kind, execution.row_count, execution.column_count
        ),
        solver_identity: "not-required".to_string(),
        production_lowering_fingerprint: format!(
            "backend={};rows={};columns={}",
            NATIVE_ARROW_SEMANTIC_BACKEND, execution.row_count, execution.column_count
        ),
        backend_identity: native_arrow_semantic_backend_identity(),
        projection,
        predicate: PredicateEnvelope::None,
        split: SplitDescriptor::FullScan {
            row_count: execution.row_count,
        },
        policy,
    }))
}

pub fn validated_native_arrow_semantic_runtime_cache_key(
    bytes: &[u8],
    validation: &NativeArrowSemanticModelValidationReport,
    projection: ProjectionSet,
    policy: RuntimeSafetyPolicy,
) -> Result<RuntimeCacheKey, NativeArrowSemanticDiagnostic> {
    let decision = decide_validated_native_arrow_semantic_runtime(validation, policy);
    if decision.decision != RuntimeExecutionDecision::NativeCandidate || !validation.is_validated()
    {
        let fallback_disabled = decision
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == RuntimeDiagnosticCode::FallbackDisabled);
        let fallback_note = if fallback_disabled
            && matches!(policy.fallback, RuntimeFallbackPolicy::FailClosedOnly)
        {
            " and interpreter fallback is disabled"
        } else {
            ""
        };
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedPayload,
            "$.cache.native_arrow_semantic_model",
            format!(
                "only successful native/model validation may seed runtime cache keys{fallback_note}"
            ),
        ));
    }

    Ok(RuntimeCacheKey::build(&RuntimeCacheKeyInput {
        abi_version: RuntimeAbiVersion::CURRENT,
        artifact_digest: stable_digest("artifact", bytes),
        facts_fingerprint: format!(
            "artifact_kind={};rows={};columns={};model_trace={};native_trace={}",
            validation.artifact_kind,
            validation.row_count,
            validation.column_count,
            stable_digest_for_lines("reference-trace", validation.reference_trace()),
            stable_digest_for_lines("native-trace", validation.native_trace())
        ),
        solver_identity: "not-required".to_string(),
        production_lowering_fingerprint: format!(
            "backend={};validation=native-model:phase40;rows={};columns={}",
            NATIVE_ARROW_SEMANTIC_BACKEND, validation.row_count, validation.column_count
        ),
        backend_identity: RuntimeBackendIdentity {
            backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
            backend_version: "phase40-native-model-validation".to_string(),
            toolchain: "per-run-validation;mlir-llvm-native-lowering-tcb".to_string(),
            target_triple: "engine-neutral".to_string(),
            cpu_features: Vec::new(),
        },
        projection,
        predicate: PredicateEnvelope::None,
        split: SplitDescriptor::FullScan {
            row_count: validation.row_count,
        },
        policy,
    }))
}

fn decode_reference_batch(bytes: &[u8]) -> Result<RecordBatch, NativeArrowSemanticDiagnostic> {
    let payload = if is_arrow_semantic_container(bytes) {
        decode_arrow_semantic_container_payload(bytes)
    } else if is_arrow_semantic_payload(bytes) {
        decode_arrow_semantic_payload(bytes)
    } else {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedArtifact,
            "$.artifact",
            "native Arrow semantic execution requires LMC2(LMA1) or direct LMA1 bytes",
        ));
    }
    .map_err(|err| {
        NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::VerifierRejected,
            "$.payload",
            err.to_string(),
        )
    })?;

    let batches = payload.to_record_batches().map_err(|err| {
        NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
            "$.payload.batches",
            err.to_string(),
        )
    })?;
    if batches.len() != 1 {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
            "$.payload.batches",
            format!(
                "native Arrow semantic execution requires exactly one record batch, got {}",
                batches.len()
            ),
        ));
    }
    Ok(batches.into_iter().next().expect("one batch checked"))
}

fn reference_model_trace_for_batch(
    batch: &RecordBatch,
) -> Result<Vec<String>, NativeArrowSemanticDiagnostic> {
    let program = reference_program_for_batch(batch)?;
    let report = execute_reference(&program);
    if report.status != ReferenceStatus::Finished {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch,
            "$.reference.trace",
            "reference executor failed closed while building native/model trace",
        ));
    }
    Ok(report.trace_lines())
}

fn reference_program_for_batch(
    batch: &RecordBatch,
) -> Result<L2CoreProgram, NativeArrowSemanticDiagnostic> {
    let row_count = batch.num_rows() as u64;
    let column_count = batch.num_columns() as u64;
    let total_events = row_count.saturating_mul(column_count);
    let mut capabilities = Vec::with_capacity(batch.num_columns());
    let mut body = Vec::with_capacity(total_events as usize);

    for (column_index, field) in batch.schema().fields().iter().enumerate() {
        ensure_model_supported_type(field.data_type(), column_index)?;
        let builder = model_builder_id(column_index, field.name());
        capabilities.push(Capability::OutputBuilder(OutputBuilderCapability {
            id: builder.clone(),
            arrow_type: field.data_type().clone(),
            nullable: field.is_nullable(),
            max_events: row_count,
        }));

        let column = batch.column(column_index);
        for row_index in 0..batch.num_rows() {
            if column.is_null(row_index) {
                body.push(L2CoreStmt::AppendNull {
                    builder: builder.clone(),
                });
            } else {
                body.push(L2CoreStmt::AppendValue {
                    builder: builder.clone(),
                    value: scalar_expr_for_array_value(column.as_ref(), column_index, row_index)?,
                });
            }
        }
    }

    Ok(L2CoreProgram {
        artifact_version: 1,
        required_features: vec!["native-model-validation.v0".to_string()],
        optional_features: vec![],
        capabilities,
        resource_budget: ResourceBudget {
            max_steps: total_events.saturating_add(16),
            max_input_bytes_read: 0,
            max_scratch_bytes: 0,
            max_builder_events: total_events,
            max_rows: row_count,
            max_constraint_count: 0,
        },
        body,
    })
}

fn native_model_trace_for_batch(
    batch: &RecordBatch,
) -> Result<Vec<String>, NativeArrowSemanticDiagnostic> {
    let mut trace = Vec::new();
    for (column_index, field) in batch.schema().fields().iter().enumerate() {
        ensure_model_supported_type(field.data_type(), column_index)?;
        let builder = model_builder_id(column_index, field.name());
        let type_name = model_type_name(field.data_type(), column_index)?;
        let column = batch.column(column_index);
        for row_index in 0..batch.num_rows() {
            if column.is_null(row_index) {
                trace.push(format!("append-null:{builder}:{type_name}"));
            } else {
                trace.push(format!("append-value:{builder}:{type_name}"));
            }
        }
    }
    trace.push("terminal:finished".to_string());
    Ok(trace)
}

fn model_builder_id(column_index: usize, name: &str) -> String {
    format!("col{column_index}:{name}")
}

fn ensure_model_supported_type(
    data_type: &DataType,
    column_index: usize,
) -> Result<(), NativeArrowSemanticDiagnostic> {
    model_type_name(data_type, column_index).map(|_| ())
}

fn model_type_name(
    data_type: &DataType,
    column_index: usize,
) -> Result<&'static str, NativeArrowSemanticDiagnostic> {
    match data_type {
        DataType::Boolean => Ok("bool"),
        DataType::Int32 => Ok("int32"),
        DataType::Int64 => Ok("int64"),
        DataType::Float32 => Ok("float32"),
        DataType::Float64 => Ok("float64"),
        other => Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedType,
            format!("$.schema.fields[{column_index}].type"),
            format!(
                "unsupported native/model validation type {other:?}; expected Boolean, Int32, Int64, Float32, or Float64"
            ),
        )),
    }
}

fn scalar_expr_for_array_value(
    column: &dyn Array,
    column_index: usize,
    row_index: usize,
) -> Result<ScalarExpr, NativeArrowSemanticDiagnostic> {
    match column.data_type() {
        DataType::Boolean => {
            let Some(values) = column.as_any().downcast_ref::<BooleanArray>() else {
                return Err(downcast_diagnostic(column, column_index));
            };
            Ok(ScalarExpr::Const(ScalarValue::Bool(values.value(row_index))))
        }
        DataType::Int32 => {
            let Some(values) = column.as_any().downcast_ref::<PrimitiveArray<Int32Type>>() else {
                return Err(downcast_diagnostic(column, column_index));
            };
            Ok(ScalarExpr::Const(ScalarValue::Int32(values.value(row_index))))
        }
        DataType::Int64 => {
            let Some(values) = column.as_any().downcast_ref::<PrimitiveArray<Int64Type>>() else {
                return Err(downcast_diagnostic(column, column_index));
            };
            Ok(ScalarExpr::Const(ScalarValue::Int64(values.value(row_index))))
        }
        DataType::Float32 => {
            let Some(values) = column.as_any().downcast_ref::<PrimitiveArray<Float32Type>>() else {
                return Err(downcast_diagnostic(column, column_index));
            };
            Ok(ScalarExpr::Const(ScalarValue::Float32Bits(
                values.value(row_index).to_bits(),
            )))
        }
        DataType::Float64 => {
            let Some(values) = column.as_any().downcast_ref::<PrimitiveArray<Float64Type>>() else {
                return Err(downcast_diagnostic(column, column_index));
            };
            Ok(ScalarExpr::Const(ScalarValue::Float64Bits(
                values.value(row_index).to_bits(),
            )))
        }
        other => Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedType,
            format!("$.schema.fields[{column_index}].type"),
            format!(
                "unsupported native/model validation value type {other:?}; expected Boolean, Int32, Int64, Float32, or Float64"
            ),
        )),
    }
}

fn copy_supported_column(
    column: &dyn Array,
    column_index: usize,
) -> Result<ArrayRef, NativeArrowSemanticDiagnostic> {
    match column.data_type() {
        DataType::Boolean => copy_boolean_column(column, column_index),
        DataType::Int32 => copy_primitive_column::<Int32Type>(column, column_index),
        DataType::Int64 => copy_primitive_column::<Int64Type>(column, column_index),
        DataType::Float32 => copy_primitive_column::<Float32Type>(column, column_index),
        DataType::Float64 => copy_primitive_column::<Float64Type>(column, column_index),
        other => Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedType,
            format!("$.schema.fields[{column_index}].type"),
            format!(
                "unsupported native Arrow semantic type {other:?}; expected Boolean, Int32, Int64, Float32, or Float64"
            ),
        )),
    }
}

fn copy_boolean_column(
    column: &dyn Array,
    column_index: usize,
) -> Result<ArrayRef, NativeArrowSemanticDiagnostic> {
    let Some(values) = column.as_any().downcast_ref::<BooleanArray>() else {
        return Err(downcast_diagnostic(column, column_index));
    };
    let mut builder = OutputBuilder::new(&DataType::Boolean);
    for row in 0..values.len() {
        if values.is_null(row) {
            builder.append_null();
        } else {
            builder.append_bool(values.value(row));
        }
    }
    Ok(arrow_array::make_array(builder.finish()))
}

fn copy_primitive_column<T>(
    column: &dyn Array,
    column_index: usize,
) -> Result<ArrayRef, NativeArrowSemanticDiagnostic>
where
    T: arrow_array::types::ArrowPrimitiveType,
{
    let Some(values) = column.as_any().downcast_ref::<PrimitiveArray<T>>() else {
        return Err(downcast_diagnostic(column, column_index));
    };
    let mut builder = OutputBuilder::new(column.data_type());
    for row in 0..values.len() {
        if values.is_null(row) {
            builder.append_null();
        } else {
            append_primitive_value::<T>(&mut builder, values.value(row));
        }
    }
    Ok(arrow_array::make_array(builder.finish()))
}

fn append_primitive_value<T>(builder: &mut OutputBuilder, value: T::Native)
where
    T: arrow_array::types::ArrowPrimitiveType,
{
    match builder {
        OutputBuilder::Int32(_) => builder.append_i32(value_to_i32::<T>(value)),
        OutputBuilder::Int64(_) => builder.append_i64(value_to_i64::<T>(value)),
        OutputBuilder::Float32(_) => builder.append_f32(value_to_f32::<T>(value)),
        OutputBuilder::Float64(_) => builder.append_f64(value_to_f64::<T>(value)),
        OutputBuilder::Boolean(_) | OutputBuilder::Utf8(_) => {
            panic!("primitive native copy called with non-primitive builder")
        }
    }
}

fn value_to_i32<T>(value: T::Native) -> i32
where
    T: arrow_array::types::ArrowPrimitiveType,
{
    let any = &value as &dyn std::any::Any;
    *any.downcast_ref::<i32>()
        .expect("Int32 builder must receive i32 values")
}

fn value_to_i64<T>(value: T::Native) -> i64
where
    T: arrow_array::types::ArrowPrimitiveType,
{
    let any = &value as &dyn std::any::Any;
    *any.downcast_ref::<i64>()
        .expect("Int64 builder must receive i64 values")
}

fn value_to_f32<T>(value: T::Native) -> f32
where
    T: arrow_array::types::ArrowPrimitiveType,
{
    let any = &value as &dyn std::any::Any;
    *any.downcast_ref::<f32>()
        .expect("Float32 builder must receive f32 values")
}

fn value_to_f64<T>(value: T::Native) -> f64
where
    T: arrow_array::types::ArrowPrimitiveType,
{
    let any = &value as &dyn std::any::Any;
    *any.downcast_ref::<f64>()
        .expect("Float64 builder must receive f64 values")
}

fn downcast_diagnostic(column: &dyn Array, column_index: usize) -> NativeArrowSemanticDiagnostic {
    NativeArrowSemanticDiagnostic::new(
        NativeArrowSemanticDiagnosticCode::UnsupportedType,
        format!("$.columns[{column_index}]"),
        format!(
            "Arrow array data type {:?} did not match expected concrete array",
            column.data_type()
        ),
    )
}

fn stable_digest(label: &str, bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{label}:{hash:016x}")
}

fn stable_digest_for_lines(label: &str, lines: &[String]) -> String {
    let mut bytes = Vec::new();
    for line in lines {
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(b'\n');
    }
    stable_digest(label, &bytes)
}

#[allow(dead_code)]
fn _assert_record_batch_is_owned(batch: &RecordBatch) -> Vec<ArrayRef> {
    batch
        .columns()
        .iter()
        .map(|column| Arc::clone(column))
        .collect()
}
