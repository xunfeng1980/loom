//! Host-neutral production backend request/report model.
//!
//! This module is the Phase 23 bridge from `loom_core::runtime_abi` into the
//! optional native backend crate. It performs policy preflight only; later plans
//! attach ODS, LLVM lowering, and JIT execution evidence behind the same model.

use std::fmt;

use loom_core::production_native_lowering::{
    ProductionLoweringBackend, ProductionLoweringFacts, ProductionNativeKernel,
};
use loom_core::runtime_abi::{
    RuntimeAbiVersion, RuntimeCacheKey, RuntimeExecutionDecision, RuntimePlan,
};

use crate::report::MlirToolchainFacts;
use crate::toolchain::EXPECTED_MLIR_MAJOR;

pub const NATIVE_BACKEND_NAME: &str = "loom-native-melior";
pub const PRODUCTION_BACKEND_PIPELINE_ID: &str = "phase23-preflight-v0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBackendCapabilities {
    pub ods_manifest: bool,
    pub llvm_lowering: bool,
    pub jit_execution: bool,
    pub supported_kernels: Vec<String>,
}

impl NativeBackendCapabilities {
    pub fn phase23_preflight() -> Self {
        Self {
            ods_manifest: false,
            llvm_lowering: false,
            jit_execution: false,
            supported_kernels: vec![
                ProductionNativeKernel::BitpackPrimitiveUnpack
                    .as_str()
                    .to_string(),
                ProductionNativeKernel::FrameOfReferencePrimitiveDecode
                    .as_str()
                    .to_string(),
            ],
        }
    }

    pub fn as_key(&self) -> String {
        format!(
            "ods={};llvm={};jit={};kernels={}",
            self.ods_manifest,
            self.llvm_lowering,
            self.jit_execution,
            self.supported_kernels.join("+")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBackendIdentity {
    pub runtime_abi_version: RuntimeAbiVersion,
    pub backend: String,
    pub backend_version: String,
    pub expected_mlir_major: u32,
    pub detected_mlir_major: Option<u32>,
    pub llvm_config_version: Option<String>,
    pub toolchain_compatible: bool,
    pub target_triple: Option<String>,
    pub data_layout: Option<String>,
    pub pipeline_id: String,
    pub llvm_lowering_pipeline: Option<String>,
    pub capabilities: NativeBackendCapabilities,
}

impl NativeBackendIdentity {
    pub fn preflight_only() -> Self {
        Self {
            runtime_abi_version: RuntimeAbiVersion::CURRENT,
            backend: NATIVE_BACKEND_NAME.to_string(),
            backend_version: env!("CARGO_PKG_VERSION").to_string(),
            expected_mlir_major: EXPECTED_MLIR_MAJOR,
            detected_mlir_major: None,
            llvm_config_version: None,
            toolchain_compatible: false,
            target_triple: None,
            data_layout: None,
            pipeline_id: PRODUCTION_BACKEND_PIPELINE_ID.to_string(),
            llvm_lowering_pipeline: None,
            capabilities: NativeBackendCapabilities::phase23_preflight(),
        }
    }

    pub fn with_toolchain(mut self, toolchain: &MlirToolchainFacts) -> Self {
        self.detected_mlir_major = toolchain.detected_llvm_major;
        self.llvm_config_version = toolchain.llvm_config_version.clone();
        self.toolchain_compatible = toolchain.compatible;
        self
    }

    pub fn with_pipeline(
        mut self,
        pipeline_id: impl Into<String>,
        llvm_lowering_pipeline: Option<impl Into<String>>,
    ) -> Self {
        self.pipeline_id = pipeline_id.into();
        self.llvm_lowering_pipeline = llvm_lowering_pipeline.map(Into::into);
        self
    }

    pub fn as_key(&self) -> String {
        format!(
            "abi={};backend={}:{};mlir_expected={};mlir_detected={};llvm_config={};toolchain={};target={};layout={};pipeline={};llvm_lowering={};caps={}",
            self.runtime_abi_version.as_key(),
            self.backend,
            self.backend_version,
            self.expected_mlir_major,
            self.detected_mlir_major
                .map(|version| version.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            self.llvm_config_version
                .as_deref()
                .unwrap_or("unknown"),
            if self.toolchain_compatible {
                "compatible"
            } else {
                "unprobed-or-incompatible"
            },
            self.target_triple.as_deref().unwrap_or("unknown"),
            self.data_layout.as_deref().unwrap_or("unknown"),
            self.pipeline_id,
            self.llvm_lowering_pipeline.as_deref().unwrap_or("none"),
            self.capabilities.as_key()
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeBackendCancellation {
    pub cancelled: bool,
    pub reason: Option<String>,
}

impl NativeBackendCancellation {
    pub fn cancelled(reason: impl Into<String>) -> Self {
        Self {
            cancelled: true,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeBackendRequestInput {
    pub runtime_plan: RuntimePlan,
    pub runtime_cache_key: Option<RuntimeCacheKey>,
    pub lowering_facts: Option<ProductionLoweringFacts>,
    pub backend_identity: NativeBackendIdentity,
    pub cancellation: NativeBackendCancellation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeBackendRequest {
    pub runtime_plan: RuntimePlan,
    pub runtime_cache_key: RuntimeCacheKey,
    pub lowering_facts: ProductionLoweringFacts,
    pub backend_identity: NativeBackendIdentity,
    pub cancellation: NativeBackendCancellation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeBackendStatus {
    Accepted,
    Rejected,
    SkippedToolchain,
    Cancelled,
    FailClosed,
}

impl NativeBackendStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::SkippedToolchain => "skipped-toolchain",
            Self::Cancelled => "cancelled",
            Self::FailClosed => "fail-closed",
        }
    }
}

impl fmt::Display for NativeBackendStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeBackendDiagnosticCode {
    RuntimePlanNotNativeCandidate,
    RuntimePlanHadDiagnostics,
    MissingCacheKey,
    MissingLoweringFacts,
    UnsupportedLoweringFacts,
    Cancelled,
    ToolchainSkipped,
    ToolchainFailed,
    BackendFailed,
    InvalidBackendArtifact,
    JitUnavailable,
    JitSymbolMissing,
    NativeOutputMismatch,
    NativeShapeDisabled,
}

impl NativeBackendDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RuntimePlanNotNativeCandidate => "runtime-plan-not-native-candidate",
            Self::RuntimePlanHadDiagnostics => "runtime-plan-had-diagnostics",
            Self::MissingCacheKey => "missing-cache-key",
            Self::MissingLoweringFacts => "missing-lowering-facts",
            Self::UnsupportedLoweringFacts => "unsupported-lowering-facts",
            Self::Cancelled => "cancelled",
            Self::ToolchainSkipped => "toolchain-skipped",
            Self::ToolchainFailed => "toolchain-failed",
            Self::BackendFailed => "backend-failed",
            Self::InvalidBackendArtifact => "invalid-backend-artifact",
            Self::JitUnavailable => "jit-unavailable",
            Self::JitSymbolMissing => "jit-symbol-missing",
            Self::NativeOutputMismatch => "native-output-mismatch",
            Self::NativeShapeDisabled => "native-shape-disabled",
        }
    }
}

impl fmt::Display for NativeBackendDiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBackendDiagnostic {
    pub code: NativeBackendDiagnosticCode,
    pub path: String,
    pub message: String,
}

impl NativeBackendDiagnostic {
    pub fn new(
        code: NativeBackendDiagnosticCode,
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

#[derive(Debug, Clone, PartialEq)]
pub struct NativeBackendArtifact {
    pub artifact_id: String,
    pub runtime_cache_key: RuntimeCacheKey,
    pub backend_identity: NativeBackendIdentity,
    pub lowering_facts: ProductionLoweringFacts,
    pub entry_symbol: Option<String>,
    pub row_count: Option<u64>,
    pub column_count: Option<usize>,
    pub artifact_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeBackendReport {
    pub status: NativeBackendStatus,
    pub diagnostics: Vec<NativeBackendDiagnostic>,
    pub runtime_plan: RuntimePlan,
    pub runtime_cache_key: Option<RuntimeCacheKey>,
    pub backend_identity: NativeBackendIdentity,
    pub artifact: Option<NativeBackendArtifact>,
}

impl NativeBackendReport {
    pub fn accepted(request: &NativeBackendRequest) -> Self {
        Self {
            status: NativeBackendStatus::Accepted,
            diagnostics: Vec::new(),
            runtime_plan: request.runtime_plan.clone(),
            runtime_cache_key: Some(request.runtime_cache_key.clone()),
            backend_identity: request.backend_identity.clone(),
            artifact: Some(NativeBackendArtifact {
                artifact_id: format!(
                    "{}:{}",
                    request.backend_identity.pipeline_id, request.runtime_cache_key.stable_id
                ),
                runtime_cache_key: request.runtime_cache_key.clone(),
                backend_identity: request.backend_identity.clone(),
                lowering_facts: request.lowering_facts.clone(),
                entry_symbol: None,
                row_count: Some(request.lowering_facts.shape.row_count()),
                column_count: Some(request.lowering_facts.shape.columns().len()),
                artifact_summary: None,
            }),
        }
    }

    pub fn accepted_pipeline(
        request: &NativeBackendRequest,
        backend_identity: NativeBackendIdentity,
        entry_symbol: impl Into<String>,
        row_count: u64,
        column_count: usize,
        artifact_summary: impl Into<String>,
    ) -> Self {
        Self {
            status: NativeBackendStatus::Accepted,
            diagnostics: Vec::new(),
            runtime_plan: request.runtime_plan.clone(),
            runtime_cache_key: Some(request.runtime_cache_key.clone()),
            backend_identity: backend_identity.clone(),
            artifact: Some(NativeBackendArtifact {
                artifact_id: format!(
                    "{}:{}",
                    backend_identity.pipeline_id, request.runtime_cache_key.stable_id
                ),
                runtime_cache_key: request.runtime_cache_key.clone(),
                backend_identity,
                lowering_facts: request.lowering_facts.clone(),
                entry_symbol: Some(entry_symbol.into()),
                row_count: Some(row_count),
                column_count: Some(column_count),
                artifact_summary: Some(artifact_summary.into()),
            }),
        }
    }

    pub fn failed_from_request(
        status: NativeBackendStatus,
        request: &NativeBackendRequest,
        backend_identity: NativeBackendIdentity,
        diagnostics: Vec<NativeBackendDiagnostic>,
    ) -> Self {
        Self {
            status,
            diagnostics,
            runtime_plan: request.runtime_plan.clone(),
            runtime_cache_key: Some(request.runtime_cache_key.clone()),
            backend_identity,
            artifact: None,
        }
    }

    pub fn rejected(
        status: NativeBackendStatus,
        input: &NativeBackendRequestInput,
        diagnostics: Vec<NativeBackendDiagnostic>,
    ) -> Self {
        Self {
            status,
            diagnostics,
            runtime_plan: input.runtime_plan.clone(),
            runtime_cache_key: input.runtime_cache_key.clone(),
            backend_identity: input.backend_identity.clone(),
            artifact: None,
        }
    }

    pub fn is_ok(&self) -> bool {
        self.status == NativeBackendStatus::Accepted && self.diagnostics.is_empty()
    }
}

pub fn validate_backend_request(
    input: NativeBackendRequestInput,
) -> Result<NativeBackendRequest, NativeBackendReport> {
    let mut diagnostics = Vec::new();

    if input.cancellation.cancelled {
        diagnostics.push(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::Cancelled,
            "$.cancellation",
            input
                .cancellation
                .reason
                .clone()
                .unwrap_or_else(|| "native backend request was cancelled".to_string()),
        ));
        return Err(NativeBackendReport::rejected(
            NativeBackendStatus::Cancelled,
            &input,
            diagnostics,
        ));
    }

    if input.runtime_plan.decision != RuntimeExecutionDecision::NativeCandidate {
        diagnostics.push(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::RuntimePlanNotNativeCandidate,
            "$.runtime_plan.decision",
            format!(
                "native backend requires runtime decision '{}', got '{}'",
                RuntimeExecutionDecision::NativeCandidate.as_str(),
                input.runtime_plan.decision.as_str()
            ),
        ));
    }

    if !input.runtime_plan.diagnostics.is_empty() {
        diagnostics.push(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::RuntimePlanHadDiagnostics,
            "$.runtime_plan.diagnostics",
            "native backend requires a diagnostic-free runtime plan",
        ));
    }

    if input.runtime_cache_key.is_none() {
        diagnostics.push(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::MissingCacheKey,
            "$.runtime_cache_key",
            "native backend requires the Phase 22 runtime cache key",
        ));
    }

    let Some(lowering_facts) = input.lowering_facts.as_ref() else {
        diagnostics.push(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::MissingLoweringFacts,
            "$.lowering_facts",
            "native backend requires production lowering facts",
        ));
        return Err(NativeBackendReport::rejected(
            NativeBackendStatus::FailClosed,
            &input,
            diagnostics,
        ));
    };

    if !lowering_facts_supported(lowering_facts) {
        diagnostics.push(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::UnsupportedLoweringFacts,
            "$.lowering_facts",
            "production lowering facts are not supported by the native backend preflight",
        ));
    }

    if !diagnostics.is_empty() {
        return Err(NativeBackendReport::rejected(
            NativeBackendStatus::FailClosed,
            &input,
            diagnostics,
        ));
    }

    Ok(NativeBackendRequest {
        runtime_plan: input.runtime_plan,
        runtime_cache_key: input
            .runtime_cache_key
            .expect("missing cache key should have returned diagnostics"),
        lowering_facts: input
            .lowering_facts
            .expect("missing lowering facts should have returned diagnostics"),
        backend_identity: input.backend_identity,
        cancellation: input.cancellation,
    })
}

fn lowering_facts_supported(facts: &ProductionLoweringFacts) -> bool {
    facts.backend == ProductionLoweringBackend::LoomDecodeDialect
        && !facts.shape.columns().is_empty()
}
