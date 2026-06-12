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
use arrow_buffer::{BooleanBuffer, Buffer, NullBuffer};
use arrow_data::ArrayData;
use arrow_schema::{DataType, Field};

use super::arrow_builder_output::OutputBuilder;
use super::arrow_semantic_codec::{
    decode_arrow_semantic_container_payload, decode_arrow_semantic_payload,
    is_arrow_semantic_container, is_arrow_semantic_payload,
};
use super::artifact_types::{
    verify_artifact, ArtifactVerificationOptions, ArtifactVerificationReport,
    ArtifactVerificationStatus,
};
use loom_ir_core::l2_core::{
    Capability, L2CoreProgram, L2CoreStmt, OutputBuilderCapability, ResourceBudget, ScalarExpr,
    ScalarValue,
};
use super::kloom_harness::{kloom_trace_for_program, KOracleOutcome};
use super::l2_kernel_registry::L2KernelRegistry;
use super::runtime_abi::{
    decide_runtime_execution, PredicateEnvelope, ProjectionSet, RuntimeAbiVersion,
    RuntimeBackendIdentity, RuntimeCacheKey, RuntimeCacheKeyInput, RuntimeDiagnosticCode,
    RuntimeEmissionDisposition, RuntimeExecutionDecision, RuntimeFallbackPolicy,
    RuntimeLoweringDisposition, RuntimePlanDecisionReport, RuntimeReaderSupport,
    RuntimeSafetyPolicy, SplitDescriptor,
};

pub const NATIVE_ARROW_SEMANTIC_BACKEND: &str = "loom-native-arrow-semantic";
pub const PRODUCTION_NATIVE_ARROW_SEMANTIC_CODEGEN_BACKEND: &str =
    "loom-production-native-arrow-semantic-codegen";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeArrowSemanticDiagnosticCode {
    VerifierRejected,
    UnsupportedArtifact,
    UnsupportedPayload,
    UnsupportedBatchShape,
    UnsupportedType,
    UnsupportedQueryShape,
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
            Self::UnsupportedQueryShape => "unsupported-query-shape",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeArrowSemanticCodegenBufferKind {
    FixedWidthValue,
    BooleanValueBitmap,
}

impl NativeArrowSemanticCodegenBufferKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FixedWidthValue => "fixed-width-value",
            Self::BooleanValueBitmap => "boolean-value-bitmap",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeArrowSemanticCodegenColumnInput {
    pub index: usize,
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub row_count: u64,
    pub null_count: u64,
    pub value_buffer_kind: NativeArrowSemanticCodegenBufferKind,
    pub value_buffer: Vec<u8>,
    pub validity_buffer: Option<Vec<u8>>,
}

impl NativeArrowSemanticCodegenColumnInput {
    pub fn value_buffer_bytes(&self) -> usize {
        self.value_buffer.len()
    }

    pub fn validity_buffer_bytes(&self) -> usize {
        self.validity_buffer
            .as_ref()
            .map(|buffer| buffer.len())
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeArrowSemanticCodegenSupportReport {
    pub backend: String,
    pub artifact_kind: String,
    pub payload_kind: String,
    pub row_count: u64,
    pub column_count: usize,
    pub schema_fingerprint: String,
    columns: Vec<NativeArrowSemanticCodegenColumnInput>,
    diagnostics: Vec<NativeArrowSemanticDiagnostic>,
}

impl NativeArrowSemanticCodegenSupportReport {
    pub fn is_supported(&self) -> bool {
        self.diagnostics.is_empty() && !self.columns.is_empty()
    }

    pub fn columns(&self) -> &[NativeArrowSemanticCodegenColumnInput] {
        &self.columns
    }

    pub fn diagnostics(&self) -> &[NativeArrowSemanticDiagnostic] {
        &self.diagnostics
    }

    pub fn first_error(&self) -> Option<&NativeArrowSemanticDiagnostic> {
        self.diagnostics.first()
    }

    fn rejected(diagnostic: NativeArrowSemanticDiagnostic) -> Self {
        Self {
            backend: PRODUCTION_NATIVE_ARROW_SEMANTIC_CODEGEN_BACKEND.to_string(),
            artifact_kind: String::new(),
            payload_kind: String::new(),
            row_count: 0,
            column_count: 0,
            schema_fingerprint: String::new(),
            columns: Vec::new(),
            diagnostics: vec![diagnostic],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeArrowSemanticCodegenOutputColumn {
    pub index: usize,
    pub value_buffer: Vec<u8>,
    pub validity_buffer: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct NativeArrowSemanticCodegenExecutionReport {
    pub backend: String,
    pub backend_identity: String,
    pub artifact_kind: String,
    pub payload_kind: String,
    pub row_count: u64,
    pub column_count: usize,
    pub schema_fingerprint: String,
    output: Option<RecordBatch>,
    validation: Option<NativeArrowSemanticModelValidationReport>,
    diagnostics: Vec<NativeArrowSemanticDiagnostic>,
}

impl NativeArrowSemanticCodegenExecutionReport {
    pub fn is_supported(&self) -> bool {
        self.diagnostics.is_empty()
            && self.output.is_some()
            && self
                .validation
                .as_ref()
                .is_some_and(|report| report.is_validated())
    }

    pub fn output(&self) -> Option<&RecordBatch> {
        self.output.as_ref()
    }

    pub fn validation(&self) -> Option<&NativeArrowSemanticModelValidationReport> {
        self.validation.as_ref()
    }

    pub fn diagnostics(&self) -> &[NativeArrowSemanticDiagnostic] {
        &self.diagnostics
    }

    pub fn first_error(&self) -> Option<&NativeArrowSemanticDiagnostic> {
        self.diagnostics.first()
    }

    fn rejected(
        support: Option<&NativeArrowSemanticCodegenSupportReport>,
        backend_identity: impl Into<String>,
        diagnostic: NativeArrowSemanticDiagnostic,
    ) -> Self {
        Self {
            backend: PRODUCTION_NATIVE_ARROW_SEMANTIC_CODEGEN_BACKEND.to_string(),
            backend_identity: backend_identity.into(),
            artifact_kind: support
                .map(|report| report.artifact_kind.clone())
                .unwrap_or_default(),
            payload_kind: support
                .map(|report| report.payload_kind.clone())
                .unwrap_or_default(),
            row_count: support.map(|report| report.row_count).unwrap_or(0),
            column_count: support.map(|report| report.column_count).unwrap_or(0),
            schema_fingerprint: support
                .map(|report| report.schema_fingerprint.clone())
                .unwrap_or_default(),
            output: None,
            validation: None,
            diagnostics: vec![diagnostic],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeArrowSemanticCodegenReplayEvidence {
    pub backend: String,
    pub artifact_digest: String,
    pub artifact_kind: String,
    pub payload_kind: String,
    pub schema_fingerprint: String,
    pub support_fingerprint: String,
    pub output_buffer_fingerprint: String,
    pub reference_trace_fingerprint: String,
    pub native_trace_fingerprint: String,
    pub validation_fingerprint: String,
    pub runtime_cache_stable_id: String,
    pub runtime_cache_canonical_input: String,
    pub replay_fingerprint: String,
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
    /// When `Some`, the K oracle was skipped (referee absent or unsupported
    /// program).  The route should NOT fail-close; this field carries the
    /// skip reason for observability.
    pub oracle_skip_reason: Option<String>,
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

/// **Offline differential oracle — NOT a production decoder.**
///
/// This entry point consumes an `LMC2`/`LMA1` Arrow-semantic artifact whose
/// payload *already contains* the answer as embedded Arrow, decodes that
/// reference batch, and re-materializes each column under a support predicate.
/// It proves the native model reproduces a known-good Arrow batch; it does
/// **not** decode physical bytes via the L2Core IR.
///
/// The production decode path is [`crate::interp::l2core_interp::interpret_l2core`]
/// (wired into [`crate::ffi::loom_sidecar_decode`]). This LMA1 path is retained
/// only as the offline differential oracle that the interpreter is checked
/// against in tests (see `tests/interp_lma1_differential.rs`). Per
/// [`execute_verified_native_arrow_semantic`]'s Phase-50.1 note, LMC2/LMA1
/// acceptance must not leak into the production FFI surface.
pub fn execute_native_arrow_semantic(bytes: &[u8]) -> NativeArrowSemanticExecutionReport {
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

    // Phase 50.1: LMC2/LMA1 kept for backward compat with existing test fixtures.
    // Phase 50 will re-anchor native execution to sidecar overlay.
    // DO NOT remove LMC2/LMA1 acceptance until sidecar-native track is production-ready.
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
            return NativeArrowSemanticExecutionReport::rejected(
                NativeArrowSemanticDiagnostic::new(
                    NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
                    "$.native.output",
                    "native Arrow semantic output batch construction failed",
                ),
            );
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

/// Execute native Arrow semantic with internal builder-event trace.
///
/// This is Phase 1's traced variant of [`execute_verified_native_arrow_semantic`].
/// It uses [`TracedOutputBuilder`] so the trace is emitted *inside* the builder
/// API rather than reconstructed from the output [`RecordBatch`].
///
/// Returns the execution report together with the internal trace. The trace
/// format aligns with the K spec-oracle (Phase 40+):
/// `append-value:{builder_id}:{type_name}` / `append-null:{builder_id}:{type_name}`
/// followed by `terminal:finished`.
pub fn execute_verified_native_arrow_semantic_with_internal_trace(
    bytes: &[u8],
    verification: &ArtifactVerificationReport,
) -> (NativeArrowSemanticExecutionReport, Vec<String>) {
    if verification.status() != ArtifactVerificationStatus::Accepted || !verification.is_ok() {
        let report = NativeArrowSemanticExecutionReport::rejected(
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::VerifierRejected,
                "$.verification",
                "native Arrow semantic execution requires an accepted artifact verifier report",
            ),
        );
        return (report, Vec::new());
    }

    let Some(facts) = verification.facts() else {
        let report = NativeArrowSemanticExecutionReport::rejected(
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::VerifierRejected,
                "$.facts",
                "accepted artifact verifier report did not expose facts",
            ),
        );
        return (report, Vec::new());
    };

    // Phase 50.1: LMC2/LMA1 kept for backward compat with existing test fixtures.
    // Phase 50 will re-anchor native execution to sidecar overlay.
    // DO NOT remove LMC2/LMA1 acceptance until sidecar-native track is production-ready.
    if !matches!(facts.artifact_kind.as_str(), "LMC2" | "LMA1") {
        let report = NativeArrowSemanticExecutionReport::rejected(
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::UnsupportedArtifact,
                "$.facts.artifact_kind",
                format!(
                    "unsupported artifact kind '{}'; expected LMC2 or LMA1",
                    facts.artifact_kind
                ),
            ),
        );
        return (report, Vec::new());
    }

    if facts.payload_kind.as_deref() != Some("Arrow semantic payload") {
        let report = NativeArrowSemanticExecutionReport::rejected(
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::UnsupportedPayload,
                "$.facts.payload_kind",
                "native Arrow semantic execution requires an Arrow semantic payload",
            ),
        );
        return (report, Vec::new());
    }

    let reference = match decode_reference_batch(bytes) {
        Ok(batch) => batch,
        Err(diagnostic) => {
            return (NativeArrowSemanticExecutionReport::rejected(diagnostic), Vec::new())
        }
    };

    let mut copied_columns = Vec::with_capacity(reference.num_columns());
    let mut internal_trace = Vec::new();
    for (idx, (field, column)) in reference
        .schema()
        .fields()
        .iter()
        .zip(reference.columns())
        .enumerate()
    {
        match copy_supported_column_traced(column.as_ref(), field, idx) {
            Ok((array, trace)) => {
                copied_columns.push(array);
                internal_trace.extend(trace);
            }
            Err(diagnostic) => {
                return (
                    NativeArrowSemanticExecutionReport::rejected(diagnostic),
                    Vec::new(),
                )
            }
        }
    }
    internal_trace.push("terminal:finished".to_string());

    let row_count = reference.num_rows() as u64;
    let column_count = reference.num_columns();
    let output = match RecordBatch::try_new(reference.schema(), copied_columns) {
        Ok(batch) => batch,
        Err(_) => {
            return (
                NativeArrowSemanticExecutionReport::rejected(
                    NativeArrowSemanticDiagnostic::new(
                        NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
                        "$.native.output",
                        "native Arrow semantic output batch construction failed",
                    ),
                ),
                Vec::new(),
            );
        }
    };

    let report = NativeArrowSemanticExecutionReport {
        backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
        artifact_kind: facts.artifact_kind.clone(),
        payload_kind: facts.payload_kind.clone().unwrap_or_default(),
        row_count,
        column_count,
        output: Some(output),
        diagnostics: Vec::new(),
    };
    (report, internal_trace)
}

pub fn prepare_native_arrow_semantic_codegen_support(
    bytes: &[u8],
) -> NativeArrowSemanticCodegenSupportReport {
    prepare_native_arrow_semantic_codegen_support_with_options(
        bytes,
        &ArtifactVerificationOptions::default(),
    )
}

pub fn prepare_native_arrow_semantic_codegen_support_with_options(
    bytes: &[u8],
    options: &ArtifactVerificationOptions,
) -> NativeArrowSemanticCodegenSupportReport {
    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_artifact(bytes, &registry, options);
    prepare_verified_native_arrow_semantic_codegen_support(bytes, &verification)
}

pub fn prepare_verified_native_arrow_semantic_codegen_support(
    bytes: &[u8],
    verification: &ArtifactVerificationReport,
) -> NativeArrowSemanticCodegenSupportReport {
    if verification.status() != ArtifactVerificationStatus::Accepted || !verification.is_ok() {
        return NativeArrowSemanticCodegenSupportReport::rejected(
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::VerifierRejected,
                "$.verification",
                "production native Arrow semantic codegen requires an accepted artifact verifier report",
            ),
        );
    }

    let Some(facts) = verification.facts() else {
        return NativeArrowSemanticCodegenSupportReport::rejected(
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::VerifierRejected,
                "$.facts",
                "accepted artifact verifier report did not expose facts",
            ),
        );
    };

    // Phase 50.1: LMC2/LMA1 kept for backward compat with existing test fixtures.
    // Phase 50 will re-anchor native execution to sidecar overlay.
    // DO NOT remove LMC2/LMA1 acceptance until sidecar-native track is production-ready.
    if !matches!(facts.artifact_kind.as_str(), "LMC2" | "LMA1") {
        return NativeArrowSemanticCodegenSupportReport::rejected(
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::UnsupportedArtifact,
                "$.facts.artifact_kind",
                format!(
                    "unsupported artifact kind '{}'; expected LMC2 or LMA1",
                    facts.artifact_kind
                ),
            ),
        );
    }

    if facts.payload_kind.as_deref() != Some("Arrow semantic payload") {
        return NativeArrowSemanticCodegenSupportReport::rejected(
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::UnsupportedPayload,
                "$.facts.payload_kind",
                "production native Arrow semantic codegen requires an Arrow semantic payload",
            ),
        );
    }

    let reference = match decode_reference_batch(bytes) {
        Ok(batch) => batch,
        Err(diagnostic) => return NativeArrowSemanticCodegenSupportReport::rejected(diagnostic),
    };

    let mut columns = Vec::with_capacity(reference.num_columns());
    for (idx, field) in reference.schema().fields().iter().enumerate() {
        match extract_codegen_column_input(idx, field, reference.column(idx).as_ref()) {
            Ok(column) => columns.push(column),
            Err(diagnostic) => {
                return NativeArrowSemanticCodegenSupportReport::rejected(diagnostic)
            }
        }
    }

    if columns.is_empty() {
        return NativeArrowSemanticCodegenSupportReport::rejected(
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
                "$.schema.fields",
                "production native Arrow semantic codegen requires at least one supported column",
            ),
        );
    }

    NativeArrowSemanticCodegenSupportReport {
        backend: PRODUCTION_NATIVE_ARROW_SEMANTIC_CODEGEN_BACKEND.to_string(),
        artifact_kind: facts.artifact_kind.clone(),
        payload_kind: facts.payload_kind.clone().unwrap_or_default(),
        row_count: reference.num_rows() as u64,
        column_count: reference.num_columns(),
        schema_fingerprint: schema_fingerprint(&reference),
        columns,
        diagnostics: Vec::new(),
    }
}

pub fn validate_native_arrow_semantic_codegen_output(
    bytes: &[u8],
    support: &NativeArrowSemanticCodegenSupportReport,
    backend_identity: impl Into<String>,
    output_columns: Vec<NativeArrowSemanticCodegenOutputColumn>,
) -> NativeArrowSemanticCodegenExecutionReport {
    validate_native_arrow_semantic_codegen_output_inner(
        bytes,
        support,
        backend_identity.into(),
        output_columns,
    )
}

fn validate_native_arrow_semantic_codegen_output_inner(
    bytes: &[u8],
    support: &NativeArrowSemanticCodegenSupportReport,
    backend_identity: String,
    output_columns: Vec<NativeArrowSemanticCodegenOutputColumn>,
) -> NativeArrowSemanticCodegenExecutionReport {
    if !support.is_supported() {
        let diagnostic = support.first_error().cloned().unwrap_or_else(|| {
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::UnsupportedPayload,
                "$.codegen.support",
                "production native codegen output validation requires supported codegen inputs",
            )
        });
        return NativeArrowSemanticCodegenExecutionReport::rejected(
            Some(support),
            backend_identity,
            diagnostic,
        );
    }

    let batch = match record_batch_from_codegen_output(support, output_columns) {
        Ok(batch) => batch,
        Err(diagnostic) => {
            return NativeArrowSemanticCodegenExecutionReport::rejected(
                Some(support),
                backend_identity,
                diagnostic,
            );
        }
    };

    let validation =
        verify_native_arrow_semantic_model_for_output(bytes, support.artifact_kind.clone(), &batch, None);
    let diagnostics = validation.diagnostics().to_vec();
    NativeArrowSemanticCodegenExecutionReport {
        backend: PRODUCTION_NATIVE_ARROW_SEMANTIC_CODEGEN_BACKEND.to_string(),
        backend_identity,
        artifact_kind: support.artifact_kind.clone(),
        payload_kind: support.payload_kind.clone(),
        row_count: support.row_count,
        column_count: support.column_count,
        schema_fingerprint: support.schema_fingerprint.clone(),
        output: Some(batch),
        validation: Some(validation),
        diagnostics,
    }
}

pub fn decide_validated_native_arrow_semantic_codegen_runtime(
    execution: &NativeArrowSemanticCodegenExecutionReport,
    policy: RuntimeSafetyPolicy,
) -> RuntimePlanDecisionReport {
    let verifier_rejected = execution.first_error().is_some_and(|diagnostic| {
        diagnostic.code == NativeArrowSemanticDiagnosticCode::VerifierRejected
    });
    decide_runtime_execution(&crate::runtime_abi::RuntimeDecisionInput {
        artifact_status: if verifier_rejected {
            ArtifactVerificationStatus::Rejected
        } else {
            ArtifactVerificationStatus::Accepted
        },
        constraints_discharged: false,
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

pub fn validated_native_arrow_semantic_codegen_runtime_cache_key(
    bytes: &[u8],
    execution: &NativeArrowSemanticCodegenExecutionReport,
    projection: ProjectionSet,
    policy: RuntimeSafetyPolicy,
) -> Result<RuntimeCacheKey, NativeArrowSemanticDiagnostic> {
    validated_native_arrow_semantic_codegen_runtime_cache_key_with_shape(
        bytes,
        execution,
        projection,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan {
            row_count: execution.row_count,
        },
        policy,
    )
}

pub fn validated_native_arrow_semantic_codegen_runtime_cache_key_with_shape(
    bytes: &[u8],
    execution: &NativeArrowSemanticCodegenExecutionReport,
    projection: ProjectionSet,
    predicate: PredicateEnvelope,
    split: SplitDescriptor,
    policy: RuntimeSafetyPolicy,
) -> Result<RuntimeCacheKey, NativeArrowSemanticDiagnostic> {
    let decision = decide_validated_native_arrow_semantic_codegen_runtime(execution, policy);
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
            "$.cache.native_arrow_semantic_codegen",
            format!(
                "only Phase 40 validated production native codegen output may seed runtime cache keys{fallback_note}"
            ),
        ));
    }
    validate_supported_codegen_query_shape(execution, &projection, &predicate, &split)?;

    let validation = execution
        .validation()
        .expect("supported codegen execution must expose validation");
    Ok(RuntimeCacheKey::build(&RuntimeCacheKeyInput {
        abi_version: RuntimeAbiVersion::CURRENT,
        artifact_digest: stable_digest("artifact", bytes),
        facts_fingerprint: format!(
            "artifact_kind={};payload_kind={};schema={};model_trace={};native_trace={}",
            execution.artifact_kind,
            execution.payload_kind,
            execution.schema_fingerprint,
            stable_digest_for_lines("reference-trace", validation.reference_trace()),
            stable_digest_for_lines("native-trace", validation.native_trace())
        ),
        verifier_identity: "not-required".to_string(),
        production_lowering_fingerprint: format!(
            "backend={};identity={};validation=native-model:phase40;rows={};columns={};output={}",
            PRODUCTION_NATIVE_ARROW_SEMANTIC_CODEGEN_BACKEND,
            execution.backend_identity,
            execution.row_count,
            execution.column_count,
            output_buffer_fingerprint_for_execution(execution)
                .unwrap_or_else(|_| "output=unavailable".to_string())
        ),
        backend_identity: RuntimeBackendIdentity {
            backend: PRODUCTION_NATIVE_ARROW_SEMANTIC_CODEGEN_BACKEND.to_string(),
            backend_version: "phase43.1-production-codegen".to_string(),
            toolchain: execution.backend_identity.clone(),
            target_triple: "engine-neutral".to_string(),
            cpu_features: Vec::new(),
        },
        projection,
        predicate,
        split,
        policy,
    }))
}

pub fn native_arrow_semantic_codegen_replay_evidence(
    bytes: &[u8],
    support: &NativeArrowSemanticCodegenSupportReport,
    execution: &NativeArrowSemanticCodegenExecutionReport,
    projection: ProjectionSet,
    predicate: PredicateEnvelope,
    split: SplitDescriptor,
    policy: RuntimeSafetyPolicy,
) -> Result<NativeArrowSemanticCodegenReplayEvidence, NativeArrowSemanticDiagnostic> {
    if !support.is_supported() {
        let diagnostic = support.first_error().cloned().unwrap_or_else(|| {
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::UnsupportedPayload,
                "$.codegen.replay.support",
                "production native codegen replay evidence requires supported inputs",
            )
        });
        return Err(diagnostic);
    }
    if !execution.is_supported() {
        let diagnostic = execution.first_error().cloned().unwrap_or_else(|| {
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::UnsupportedPayload,
                "$.codegen.replay.execution",
                "production native codegen replay evidence requires Phase 40 validated execution",
            )
        });
        return Err(diagnostic);
    }

    let validation = execution
        .validation()
        .expect("supported codegen execution must expose validation");
    let cache_key = validated_native_arrow_semantic_codegen_runtime_cache_key_with_shape(
        bytes, execution, projection, predicate, split, policy,
    )?;
    let artifact_digest = stable_digest("artifact", bytes);
    let support_fingerprint = support_fingerprint(support);
    let output_buffer_fingerprint = output_buffer_fingerprint_for_execution(execution)?;
    let reference_trace_fingerprint =
        stable_digest_for_lines("reference-trace", validation.reference_trace());
    let native_trace_fingerprint =
        stable_digest_for_lines("native-trace", validation.native_trace());
    let validation_fingerprint = stable_digest(
        "validation",
        format!(
            "model={};values={};reference={reference_trace_fingerprint};native={native_trace_fingerprint}",
            validation.model_trace_matches, validation.value_equivalent,
        )
        .as_bytes(),
    );
    let replay_fingerprint = stable_digest(
        "codegen-replay",
        format!(
            "artifact={artifact_digest};support={support_fingerprint};output={output_buffer_fingerprint};validation={validation_fingerprint};cache={}:{}",
            cache_key.stable_id, cache_key.canonical_input
        )
        .as_bytes(),
    );

    Ok(NativeArrowSemanticCodegenReplayEvidence {
        backend: PRODUCTION_NATIVE_ARROW_SEMANTIC_CODEGEN_BACKEND.to_string(),
        artifact_digest,
        artifact_kind: execution.artifact_kind.clone(),
        payload_kind: execution.payload_kind.clone(),
        schema_fingerprint: execution.schema_fingerprint.clone(),
        support_fingerprint,
        output_buffer_fingerprint,
        reference_trace_fingerprint,
        native_trace_fingerprint,
        validation_fingerprint,
        runtime_cache_stable_id: cache_key.stable_id,
        runtime_cache_canonical_input: cache_key.canonical_input,
        replay_fingerprint,
    })
}

fn validate_supported_codegen_query_shape(
    execution: &NativeArrowSemanticCodegenExecutionReport,
    projection: &ProjectionSet,
    predicate: &PredicateEnvelope,
    split: &SplitDescriptor,
) -> Result<(), NativeArrowSemanticDiagnostic> {
    if !matches!(projection, ProjectionSet::All) {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedQueryShape,
            "$.runtime.projection",
            "production native Arrow semantic codegen currently supports full projection only",
        ));
    }

    if !matches!(predicate, PredicateEnvelope::None) {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedQueryShape,
            "$.runtime.predicate",
            "production native Arrow semantic codegen currently supports unfiltered scans only",
        ));
    }

    match split {
        SplitDescriptor::FullScan { row_count } if *row_count == execution.row_count => Ok(()),
        SplitDescriptor::FullScan { row_count } => Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedQueryShape,
            "$.runtime.split.row_count",
            format!(
                "production native Arrow semantic codegen full-scan row count {row_count} does not match execution row count {}",
                execution.row_count
            ),
        )),
        SplitDescriptor::RowRange { .. } => Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedQueryShape,
            "$.runtime.split",
            "production native Arrow semantic codegen currently supports full-scan splits only",
        )),
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
            oracle_skip_reason: None,
        };
    }

    let output = execution
        .output()
        .expect("supported execution report must expose output");
    verify_native_arrow_semantic_model_for_output(
        bytes,
        execution.artifact_kind.clone(),
        output,
        None,
    )
}

pub fn verify_native_arrow_semantic_model_output(
    bytes: &[u8],
    artifact_kind: impl Into<String>,
    output: &RecordBatch,
) -> NativeArrowSemanticModelValidationReport {
    verify_native_arrow_semantic_model_for_output(bytes, artifact_kind.into(), output, None)
}

/// Phase 1: verify using the internal builder-event trace rather than the
/// post-hoc reconstructed trace.
///
/// This is the entry point for the transitioned trace path. It executes the
/// artifact through [`execute_verified_native_arrow_semantic_with_internal_trace`],
/// then validates the internal trace against the K spec-oracle trace.
pub fn verify_native_arrow_semantic_model_with_internal_trace(
    bytes: &[u8],
) -> NativeArrowSemanticModelValidationReport {
    let registry = L2KernelRegistry::default_for_mvp0();
    let options = ArtifactVerificationOptions::default();
    let verification = verify_artifact(bytes, &registry, &options);

    let (execution, internal_trace) =
        execute_verified_native_arrow_semantic_with_internal_trace(bytes, &verification);

    if !execution.is_supported() {
        return NativeArrowSemanticModelValidationReport {
            backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
            artifact_kind: execution.artifact_kind.clone(),
            row_count: execution.row_count,
            column_count: execution.column_count,
            model_trace_matches: false,
            value_equivalent: false,
            reference_trace: Vec::new(),
            native_trace: internal_trace,
            diagnostics: execution.diagnostics.clone(),
            oracle_skip_reason: None,
        };
    }

    let output = execution
        .output()
        .expect("supported execution report must expose output");

    // Phase 2: independent trace checker (mirrors Lean checkAppendTrace).
    let reference_batch = match decode_reference_batch(bytes) {
        Ok(batch) => batch,
        Err(diagnostic) => {
            return NativeArrowSemanticModelValidationReport {
                backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
                artifact_kind: execution.artifact_kind.clone(),
                row_count: execution.row_count,
                column_count: execution.column_count,
                model_trace_matches: false,
                value_equivalent: false,
                reference_trace: Vec::new(),
                native_trace: internal_trace.clone(),
                diagnostics: vec![diagnostic],
                oracle_skip_reason: None,
            };
        }
    };
    let reference_program = match reference_program_for_batch(&reference_batch) {
        Ok(program) => program,
        Err(diagnostic) => {
            return NativeArrowSemanticModelValidationReport {
                backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
                artifact_kind: execution.artifact_kind.clone(),
                row_count: execution.row_count,
                column_count: execution.column_count,
                model_trace_matches: false,
                value_equivalent: false,
                reference_trace: Vec::new(),
                native_trace: internal_trace.clone(),
                diagnostics: vec![diagnostic],
                oracle_skip_reason: None,
            };
        }
    };
    if let Err(diagnostic) = check_native_model_trace(&internal_trace, &reference_program) {
        return NativeArrowSemanticModelValidationReport {
            backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
            artifact_kind: execution.artifact_kind.clone(),
            row_count: execution.row_count,
            column_count: execution.column_count,
            model_trace_matches: false,
            value_equivalent: false,
            reference_trace: Vec::new(),
            native_trace: internal_trace,
            diagnostics: vec![diagnostic],
            oracle_skip_reason: None,
        };
    }

    verify_native_arrow_semantic_model_for_output(
        bytes,
        execution.artifact_kind.clone(),
        output,
        Some(internal_trace),
    )
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
    internal_trace: Option<Vec<String>>,
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
                oracle_skip_reason: None,
            };
        }
    };

    let reference_trace = match reference_model_trace_for_batch(&reference) {
        Ok(KOracleOutcome::ProducedTrace(trace)) => trace,
        Ok(KOracleOutcome::SkippedRefereeAbsent { reason }) => {
            let value_equivalent = output == &reference;
            return NativeArrowSemanticModelValidationReport {
                backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
                artifact_kind,
                row_count: output.num_rows() as u64,
                column_count: output.num_columns(),
                model_trace_matches: true,
                value_equivalent,
                reference_trace: Vec::new(),
                native_trace: Vec::new(),
                diagnostics: Vec::new(),
                oracle_skip_reason: Some(reason),
            };
        }
        Ok(KOracleOutcome::UnsupportedProgram { reason }) => {
            let value_equivalent = output == &reference;
            return NativeArrowSemanticModelValidationReport {
                backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
                artifact_kind,
                row_count: output.num_rows() as u64,
                column_count: output.num_columns(),
                model_trace_matches: true,
                value_equivalent,
                reference_trace: Vec::new(),
                native_trace: Vec::new(),
                diagnostics: Vec::new(),
                oracle_skip_reason: Some(reason),
            };
        }
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
                oracle_skip_reason: None,
            };
        }
    };

    // Phase 1 transition: if an internal trace is provided, use it as the
    // native trace and additionally compare it against the post-hoc trace as a
    // regression guard.  When the transition is complete the post-hoc path can
    // be removed.
    let (native_trace, posthoc_trace) = match internal_trace {
        Some(internal) => {
            let posthoc = match native_model_trace_for_batch(output) {
                Ok(t) => t,
                Err(diagnostic) => {
                    return NativeArrowSemanticModelValidationReport {
                        backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
                        artifact_kind,
                        row_count: output.num_rows() as u64,
                        column_count: output.num_columns(),
                        model_trace_matches: false,
                        value_equivalent: false,
                        reference_trace,
                        native_trace: internal,
                        diagnostics: vec![diagnostic],
                        oracle_skip_reason: None,
                    };
                }
            };
            (internal, Some(posthoc))
        }
        None => {
            let posthoc = match native_model_trace_for_batch(output) {
                Ok(t) => t,
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
                        oracle_skip_reason: None,
                    };
                }
            };
            (posthoc, None)
        }
    };

    let model_trace_matches = reference_trace == native_trace;
    let value_equivalent = output == &reference;
    let mut diagnostics = Vec::new();
    if !model_trace_matches {
        diagnostics.push(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch,
            "$.native.model_trace",
            "native Arrow semantic output trace does not match K spec-oracle trace",
        ));
    }
    if !value_equivalent {
        diagnostics.push(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            "$.native.output",
            "native Arrow semantic output does not match decoded reference batch",
        ));
    }

    // Transition-period guard: internal trace must agree with post-hoc trace.
    // This is observational only — a mismatch here does not fail validation,
    // but it signals that the internal instrumentation or the post-hoc
    // reconstruction has drifted.
    if let Some(posthoc) = posthoc_trace {
        if native_trace != posthoc {
            diagnostics.push(NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch,
                "$.native.internal_posthoc_divergence",
                "internal builder trace diverges from post-hoc reconstructed trace; this is a Phase 1 transition warning",
            ));
        }
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
        oracle_skip_reason: None,
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
    let verifier_rejected = execution.first_error().is_some_and(|diagnostic| {
        diagnostic.code == NativeArrowSemanticDiagnosticCode::VerifierRejected
    });
    decide_runtime_execution(&crate::runtime_abi::RuntimeDecisionInput {
        artifact_status: if verifier_rejected {
            ArtifactVerificationStatus::Rejected
        } else {
            ArtifactVerificationStatus::Accepted
        },
        constraints_discharged: false,
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
        constraints_discharged: false,
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
            execution.artifact_kind,
            execution.payload_kind,
            execution.row_count,
            execution.column_count
        ),
        verifier_identity: "not-required".to_string(),
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
        verifier_identity: "not-required".to_string(),
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

fn extract_codegen_column_input(
    column_index: usize,
    field: &Field,
    column: &dyn Array,
) -> Result<NativeArrowSemanticCodegenColumnInput, NativeArrowSemanticDiagnostic> {
    if field.data_type() != column.data_type() {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
            format!("$.schema.fields[{column_index}].type"),
            format!(
                "schema field type {:?} does not match column type {:?}",
                field.data_type(),
                column.data_type()
            ),
        ));
    }

    let data = column.to_data();
    let row_count = data.len() as u64;
    let null_count = data.null_count() as u64;
    let validity_buffer = data.nulls().map(|nulls| nulls.validity().to_vec());

    let (value_buffer_kind, value_buffer) = match field.data_type() {
        DataType::Boolean => {
            let Some(values) = column.as_any().downcast_ref::<BooleanArray>() else {
                return Err(downcast_diagnostic(column, column_index));
            };
            (
                NativeArrowSemanticCodegenBufferKind::BooleanValueBitmap,
                BooleanBuffer::collect_bool(data.len(), |row| values.value(row))
                    .sliced()
                    .as_slice()
                    .to_vec(),
            )
        }
        DataType::Int32 => (
            NativeArrowSemanticCodegenBufferKind::FixedWidthValue,
            fixed_width_value_bytes(&data, column_index, 4)?,
        ),
        DataType::Int64 => (
            NativeArrowSemanticCodegenBufferKind::FixedWidthValue,
            fixed_width_value_bytes(&data, column_index, 8)?,
        ),
        DataType::Float32 => (
            NativeArrowSemanticCodegenBufferKind::FixedWidthValue,
            fixed_width_value_bytes(&data, column_index, 4)?,
        ),
        DataType::Float64 => (
            NativeArrowSemanticCodegenBufferKind::FixedWidthValue,
            fixed_width_value_bytes(&data, column_index, 8)?,
        ),
        other => {
            return Err(NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::UnsupportedType,
                format!("$.schema.fields[{column_index}].type"),
                format!(
                    "unsupported production native codegen type {other:?}; expected Boolean, Int32, Int64, Float32, or Float64"
                ),
            ));
        }
    };

    Ok(NativeArrowSemanticCodegenColumnInput {
        index: column_index,
        name: field.name().clone(),
        data_type: field.data_type().clone(),
        nullable: field.is_nullable(),
        row_count,
        null_count,
        value_buffer_kind,
        value_buffer,
        validity_buffer,
    })
}

fn record_batch_from_codegen_output(
    support: &NativeArrowSemanticCodegenSupportReport,
    output_columns: Vec<NativeArrowSemanticCodegenOutputColumn>,
) -> Result<RecordBatch, NativeArrowSemanticDiagnostic> {
    if output_columns.len() != support.columns.len() {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
            "$.codegen.output.columns",
            format!(
                "production native codegen returned {} column(s), expected {}",
                output_columns.len(),
                support.columns.len()
            ),
        ));
    }

    let mut arrays = Vec::with_capacity(support.columns.len());
    for (expected, output) in support.columns.iter().zip(output_columns) {
        if output.index != expected.index {
            return Err(NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
                format!("$.codegen.output.columns[{}].index", expected.index),
                format!(
                    "production native codegen returned column index {}, expected {}",
                    output.index, expected.index
                ),
            ));
        }
        arrays.push(array_from_codegen_column(expected, output)?);
    }

    let fields = support
        .columns
        .iter()
        .map(|column| Field::new(&column.name, column.data_type.clone(), column.nullable))
        .collect::<Vec<_>>();
    RecordBatch::try_new(Arc::new(arrow_schema::Schema::new(fields)), arrays).map_err(|err| {
        NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
            "$.codegen.output.record_batch",
            format!("production native codegen output RecordBatch construction failed: {err}"),
        )
    })
}

fn array_from_codegen_column(
    expected: &NativeArrowSemanticCodegenColumnInput,
    output: NativeArrowSemanticCodegenOutputColumn,
) -> Result<ArrayRef, NativeArrowSemanticDiagnostic> {
    if output.value_buffer.len() != expected.value_buffer.len() {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            format!("$.codegen.output.columns[{}].value_buffer", expected.index),
            format!(
                "production native codegen value buffer has {} bytes, expected {}",
                output.value_buffer.len(),
                expected.value_buffer.len()
            ),
        ));
    }

    let expected_validity_len = expected
        .validity_buffer
        .as_ref()
        .map(|buffer| buffer.len())
        .unwrap_or(0);
    let output_validity_len = output
        .validity_buffer
        .as_ref()
        .map(|buffer| buffer.len())
        .unwrap_or(0);
    if output_validity_len != expected_validity_len {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            format!(
                "$.codegen.output.columns[{}].validity_buffer",
                expected.index
            ),
            format!(
                "production native codegen validity buffer has {} bytes, expected {}",
                output_validity_len, expected_validity_len
            ),
        ));
    }

    let nulls = null_buffer_from_codegen_column(expected, output.validity_buffer)?;
    let value_buffer = codegen_value_buffer_for_array(expected, output.value_buffer);
    let data = ArrayData::builder(expected.data_type.clone())
        .len(expected.row_count as usize)
        .add_buffer(value_buffer)
        .nulls(nulls)
        .build()
        .map_err(|err| {
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
                format!("$.codegen.output.columns[{}]", expected.index),
                format!("production native codegen Arrow array construction failed: {err}"),
            )
        })?;

    match expected.data_type {
        DataType::Boolean => Ok(Arc::new(BooleanArray::from(data)) as ArrayRef),
        DataType::Int32 => Ok(Arc::new(PrimitiveArray::<Int32Type>::from(data)) as ArrayRef),
        DataType::Int64 => Ok(Arc::new(PrimitiveArray::<Int64Type>::from(data)) as ArrayRef),
        DataType::Float32 => Ok(Arc::new(PrimitiveArray::<Float32Type>::from(data)) as ArrayRef),
        DataType::Float64 => Ok(Arc::new(PrimitiveArray::<Float64Type>::from(data)) as ArrayRef),
        _ => Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedType,
            format!("$.schema.fields[{}].type", expected.index),
            "unsupported production native codegen output type",
        )),
    }
}

fn codegen_value_buffer_for_array(
    expected: &NativeArrowSemanticCodegenColumnInput,
    value_buffer: Vec<u8>,
) -> Buffer {
    if !value_buffer.is_empty() {
        return Buffer::from(value_buffer);
    }

    match expected.data_type {
        DataType::Int32 => Buffer::from_vec(Vec::<i32>::new()),
        DataType::Int64 => Buffer::from_vec(Vec::<i64>::new()),
        DataType::Float32 => Buffer::from_vec(Vec::<f32>::new()),
        DataType::Float64 => Buffer::from_vec(Vec::<f64>::new()),
        _ => Buffer::from(value_buffer),
    }
}

fn null_buffer_from_codegen_column(
    expected: &NativeArrowSemanticCodegenColumnInput,
    validity_buffer: Option<Vec<u8>>,
) -> Result<Option<NullBuffer>, NativeArrowSemanticDiagnostic> {
    let Some(buffer) = validity_buffer else {
        if expected.null_count == 0 {
            return Ok(None);
        }
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            format!(
                "$.codegen.output.columns[{}].validity_buffer",
                expected.index
            ),
            "production native codegen omitted a required nullable validity buffer",
        ));
    };

    let nulls = NullBuffer::from_unsliced_buffer(Buffer::from(buffer), expected.row_count as usize);
    if expected.null_count > 0 && nulls.is_none() {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            format!("$.codegen.output.columns[{}].validity_buffer", expected.index),
            "production native codegen validity buffer reported all-valid for a nullable column with nulls",
        ));
    }
    if let Some(nulls) = nulls.as_ref() {
        let actual_null_count = nulls.null_count() as u64;
        if actual_null_count != expected.null_count {
            return Err(NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
                format!("$.codegen.output.columns[{}].validity_buffer", expected.index),
                format!(
                    "production native codegen validity buffer has null count {actual_null_count}, expected {}",
                    expected.null_count
                ),
            ));
        }
    }
    Ok(nulls)
}

fn fixed_width_value_bytes(
    data: &arrow_data::ArrayData,
    column_index: usize,
    byte_width: usize,
) -> Result<Vec<u8>, NativeArrowSemanticDiagnostic> {
    let Some(buffer) = data.buffers().first() else {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
            format!("$.columns[{column_index}].buffers[0]"),
            "fixed-width Arrow column did not expose a value buffer",
        ));
    };
    let offset = data.offset().saturating_mul(byte_width);
    let len = data.len().saturating_mul(byte_width);
    if offset.saturating_add(len) > buffer.len() {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
            format!("$.columns[{column_index}].buffers[0]"),
            format!(
                "fixed-width value buffer has {} bytes but offset {} plus length {} was requested",
                buffer.len(),
                offset,
                len
            ),
        ));
    }
    Ok(buffer.slice_with_length(offset, len).as_slice().to_vec())
}

fn schema_fingerprint(batch: &RecordBatch) -> String {
    let mut text = format!("rows={};columns={}", batch.num_rows(), batch.num_columns());
    for (idx, field) in batch.schema().fields().iter().enumerate() {
        text.push_str(&format!(
            ";field[{idx}]={}:{:?}:nullable={}",
            field.name(),
            field.data_type(),
            field.is_nullable()
        ));
    }
    stable_digest("schema", text.as_bytes())
}

fn support_fingerprint(support: &NativeArrowSemanticCodegenSupportReport) -> String {
    if !support.is_supported() {
        return stable_digest("codegen-support", b"unsupported");
    }
    let mut text = format!(
        "backend={};artifact={};payload={};rows={};columns={};schema={}",
        support.backend,
        support.artifact_kind,
        support.payload_kind,
        support.row_count,
        support.column_count,
        support.schema_fingerprint
    );
    for column in support.columns() {
        text.push_str(&format!(
            ";column[{}]={}:{:?}:nullable={}:rows={}:nulls={}:kind={}:value={}:validity={}",
            column.index,
            column.name,
            column.data_type,
            column.nullable,
            column.row_count,
            column.null_count,
            column.value_buffer_kind.as_str(),
            stable_digest("value-buffer", &column.value_buffer),
            column
                .validity_buffer
                .as_ref()
                .map(|buffer| stable_digest("validity-buffer", buffer))
                .unwrap_or_else(|| "none".to_string())
        ));
    }
    stable_digest("codegen-support", text.as_bytes())
}

fn output_buffer_fingerprint_for_execution(
    execution: &NativeArrowSemanticCodegenExecutionReport,
) -> Result<String, NativeArrowSemanticDiagnostic> {
    let Some(output) = execution.output() else {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedPayload,
            "$.codegen.replay.output",
            "production native codegen replay evidence requires output buffers",
        ));
    };
    output_buffer_fingerprint_for_batch(output)
}

fn output_buffer_fingerprint_for_batch(
    batch: &RecordBatch,
) -> Result<String, NativeArrowSemanticDiagnostic> {
    let mut text = format!("rows={};columns={}", batch.num_rows(), batch.num_columns());
    for (idx, field) in batch.schema().fields().iter().enumerate() {
        let column = extract_codegen_column_input(idx, field, batch.column(idx).as_ref())?;
        text.push_str(&format!(
            ";column[{}]={}:{:?}:nullable={}:rows={}:nulls={}:kind={}:value={}:validity={}",
            column.index,
            column.name,
            column.data_type,
            column.nullable,
            column.row_count,
            column.null_count,
            column.value_buffer_kind.as_str(),
            stable_digest("value-buffer", &column.value_buffer),
            column
                .validity_buffer
                .as_ref()
                .map(|buffer| stable_digest("validity-buffer", buffer))
                .unwrap_or_else(|| "none".to_string())
        ));
    }
    Ok(stable_digest("codegen-output", text.as_bytes()))
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
) -> Result<KOracleOutcome, NativeArrowSemanticDiagnostic> {
    let program = reference_program_for_batch(batch)?;
    kloom_trace_for_program(&program).map_err(|e| {
        NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch,
            "$.reference.trace",
            format!("kloom harness error: {e}"),
        )
    })
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
            arrow_type: crate::arrow_to_l2(field.data_type()).ok_or_else(|| {
                NativeArrowSemanticDiagnostic::new(
                    NativeArrowSemanticDiagnosticCode::UnsupportedType,
                    format!("$.schema.fields[{column_index}].type"),
                    format!(
                        "unsupported Arrow type for L2Core IR: {:?}",
                        field.data_type()
                    ),
                )
            })?,
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
            max_rows: total_events,
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
    format!("col{column_index}_{name}")
}

// ---------------------------------------------------------------------------
// Phase 2: lightweight independent trace checker
// ---------------------------------------------------------------------------

/// Check a native model trace independently of the K spec-oracle.
///
/// This mirrors Lean `checkAppendTrace` at the Rust level: validates that
/// every event targets a declared builder with matching type and nullability,
/// and that the trace length is within the row budget.
fn check_native_model_trace(
    trace: &[String],
    program: &L2CoreProgram,
) -> Result<(), NativeArrowSemanticDiagnostic> {
    let mut event_count: u64 = 0;
    for line in trace {
        if line == "terminal:finished" {
            continue;
        }
        // Parse: {kind}:{builder_id}:{type_name}
        // builder_id itself may contain ':' (e.g. "col0:ok"), so we split
        // from the right: kind is the first segment, type_name is the last.
        let first_colon = line.find(':').ok_or_else(|| {
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch,
                "$.native.model_trace.format",
                format!("malformed trace line (no colon): {line}"),
            )
        })?;
        let last_colon = line.rfind(':').ok_or_else(|| {
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch,
                "$.native.model_trace.format",
                format!("malformed trace line (no colon): {line}"),
            )
        })?;
        if first_colon == last_colon {
            return Err(NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch,
                "$.native.model_trace.format",
                format!("malformed trace line (only one colon): {line}"),
            ));
        }
        let kind = &line[..first_colon];
        let builder_id = &line[first_colon + 1..last_colon];
        let type_name = &line[last_colon + 1..];

        // Locate the declared output builder.
        let builder = program
            .capabilities
            .iter()
            .find_map(|cap| match cap {
                Capability::OutputBuilder(b) if b.id == builder_id => Some(b),
                _ => None,
            })
            .ok_or_else(|| {
                NativeArrowSemanticDiagnostic::new(
                    NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch,
                    "$.native.model_trace.builder",
                    format!("trace references undeclared builder: {builder_id}"),
                )
            })?;

        // Verify type name matches the builder's Arrow type.
        let arrow_dt = crate::l2_to_arrow(&builder.arrow_type);
        let expected = model_type_name(&arrow_dt, 0).map_err(|_| {
            NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch,
                "$.native.model_trace.type",
                format!("unsupported builder type for trace check: {:?}", builder.arrow_type),
            )
        })?;
        if expected != type_name {
            return Err(NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch,
                "$.native.model_trace.type",
                format!(
                    "type mismatch for builder {builder_id}: expected {expected}, got {type_name}"
                ),
            ));
        }

        // append-null requires nullable builder.
        if kind == "append-null" && !builder.nullable {
            return Err(NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch,
                "$.native.model_trace.nullability",
                format!("append-null on non-nullable builder: {builder_id}"),
            ));
        }

        event_count += 1;
    }

    // Phase 2 note: length check against max_builder_events rather than max_rows,
    // because a pure-append program emits row_count * column_count events.
    if event_count > program.resource_budget.max_builder_events {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch,
            "$.native.model_trace.length",
            format!(
                "trace event count {event_count} exceeds max_builder_events {}",
                program.resource_budget.max_builder_events
            ),
        ));
    }

    Ok(())
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

// ---------------------------------------------------------------------------
// Traced copy variants — Phase 1: internal trace instrumentation
// ---------------------------------------------------------------------------

fn copy_supported_column_traced(
    column: &dyn Array,
    field: &arrow_schema::Field,
    column_index: usize,
) -> Result<(ArrayRef, Vec<String>), NativeArrowSemanticDiagnostic> {
    let builder_id = model_builder_id(column_index, field.name());
    let type_name = model_type_name(field.data_type(), column_index)?;
    match column.data_type() {
        DataType::Boolean => copy_boolean_column_traced(column, &builder_id, &type_name),
        DataType::Int32 => {
            copy_primitive_column_traced::<Int32Type>(column, &builder_id, &type_name)
        }
        DataType::Int64 => {
            copy_primitive_column_traced::<Int64Type>(column, &builder_id, &type_name)
        }
        DataType::Float32 => {
            copy_primitive_column_traced::<Float32Type>(column, &builder_id, &type_name)
        }
        DataType::Float64 => {
            copy_primitive_column_traced::<Float64Type>(column, &builder_id, &type_name)
        }
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

// Traced variants — emit builder-event trace inside the append API.

fn copy_boolean_column_traced(
    column: &dyn Array,
    builder_id: &str,
    type_name: &str,
) -> Result<(ArrayRef, Vec<String>), NativeArrowSemanticDiagnostic> {
    use super::arrow_builder_output::TracedOutputBuilder;
    let Some(values) = column.as_any().downcast_ref::<BooleanArray>() else {
        return Err(downcast_diagnostic(column, 0));
    };
    let mut builder =
        TracedOutputBuilder::new(&DataType::Boolean, builder_id.to_string(), type_name.to_string());
    for row in 0..values.len() {
        if values.is_null(row) {
            builder.append_null();
        } else {
            builder.append_bool(values.value(row));
        }
    }
    let trace = builder.take_trace();
    Ok((arrow_array::make_array(builder.finish()), trace))
}

fn copy_primitive_column_traced<T>(
    column: &dyn Array,
    builder_id: &str,
    type_name: &str,
) -> Result<(ArrayRef, Vec<String>), NativeArrowSemanticDiagnostic>
where
    T: arrow_array::types::ArrowPrimitiveType,
{
    use super::arrow_builder_output::TracedOutputBuilder;
    let Some(values) = column.as_any().downcast_ref::<PrimitiveArray<T>>() else {
        return Err(downcast_diagnostic(column, 0));
    };
    let mut builder = TracedOutputBuilder::new(
        column.data_type(),
        builder_id.to_string(),
        type_name.to_string(),
    );
    for row in 0..values.len() {
        if values.is_null(row) {
            builder.append_null();
        } else {
            append_primitive_value_traced::<T>(&mut builder, values.value(row));
        }
    }
    let trace = builder.take_trace();
    Ok((arrow_array::make_array(builder.finish()), trace))
}

fn append_primitive_value_traced<T>(builder: &mut crate::arrow_builder_output::TracedOutputBuilder, value: T::Native)
where
    T: arrow_array::types::ArrowPrimitiveType,
{
    let any = &value as &dyn std::any::Any;
    if let Some(v) = any.downcast_ref::<i32>() {
        builder.append_i32(*v);
    } else if let Some(v) = any.downcast_ref::<i64>() {
        builder.append_i64(*v);
    } else if let Some(v) = any.downcast_ref::<f32>() {
        builder.append_f32(*v);
    } else if let Some(v) = any.downcast_ref::<f64>() {
        builder.append_f64(*v);
    } else {
        panic!("unsupported primitive type for TracedOutputBuilder")
    }
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
