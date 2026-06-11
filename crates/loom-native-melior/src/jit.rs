#[cfg(feature = "melior")]
use arrow_schema::DataType;
#[cfg(feature = "melior")]
use loom_core::arrow_buffer_lowering::{
    lower_arrow_raw_copy_to_standard_mlir, ArrowColumnBufferPlan, PrimitiveArrowType,
};
use loom_core::arrow_buffer_lowering::{
    plan_arrow_buffers_from_decode_dialect, ArrowTableBufferPlan,
};
use loom_core::full_verifier::FullVerificationReport;
use loom_core::l2_core::L2CoreProgram;
#[cfg(feature = "melior")]
use loom_core::native_arrow_semantic::NativeArrowSemanticCodegenBufferKind;
use loom_core::native_arrow_semantic::{
    decide_validated_native_arrow_semantic_codegen_runtime,
    native_arrow_semantic_codegen_replay_evidence, prepare_native_arrow_semantic_codegen_support,
    validate_native_arrow_semantic_codegen_output, NativeArrowSemanticCodegenExecutionReport,
    NativeArrowSemanticCodegenOutputColumn, NativeArrowSemanticCodegenReplayEvidence,
    NativeArrowSemanticCodegenSupportReport, NativeArrowSemanticDiagnosticCode,
};
use loom_core::native_lowering::{
    execute_supported_copy_i32, LoweringDiagnosticCode, LoweringSupportReport,
};

use crate::backend::{
    NativeBackendCancellation, NativeBackendDiagnostic, NativeBackendDiagnosticCode,
    NativeBackendReport, NativeBackendStatus,
};
use crate::builder::{build_melior_module, MeliorModuleArtifact};
#[cfg(feature = "melior")]
use crate::pipeline::LLVM_LOWERING_PIPELINE;
use crate::pipeline::{validate_translation_to_llvm_ir, MlirValidationOptions};
use crate::report::{MeliorBackendDiagnosticCode, MeliorBackendReport, ENTRY_SYMBOL};
use crate::toolchain::probe_toolchain;
use loom_core::runtime_abi::{
    PredicateEnvelope, ProjectionSet, RuntimeExecutionDecision, RuntimeFallbackPolicy,
    RuntimePlanDecisionReport, RuntimeSafetyPolicy, SplitDescriptor,
};

pub const PRODUCTION_JIT_ENTRY_SYMBOL: &str = "loom_decode_build_buffers";
pub const ARROW_SEMANTIC_CODEGEN_JIT_ENTRY_SYMBOL: &str = "loom_arrow_semantic_codegen_buffers";

// ---------------------------------------------------------------------------
// Per-shape native-route disable registry (Phase 48)  —  persistent store
// ---------------------------------------------------------------------------

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

const DISABLE_STORE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DisableStore {
    version: u32,
    #[serde(default)]
    disabled_shapes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_updated_secs: Option<u64>,
}

impl DisableStore {
    fn load_or_default(path: &std::path::Path) -> Self {
        if !path.exists() {
            return Self::default();
        }
        match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let dir = path.parent().ok_or("disable store path has no parent")?;
        fs::create_dir_all(dir).map_err(|e| format!("create dir: {e}"))?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("serialize: {e}"))?;
        let mut tmp = path.as_os_str().to_os_string();
        tmp.push(".tmp");
        let tmp_path = PathBuf::from(tmp);
        {
            let mut file = fs::File::create(&tmp_path)
                .map_err(|e| format!("create temp: {e}"))?;
            file.write_all(json.as_bytes())
                .map_err(|e| format!("write temp: {e}"))?;
            file.sync_all().map_err(|e| format!("sync temp: {e}"))?;
        }
        fs::rename(&tmp_path, path).map_err(|e| format!("rename: {e}"))?;
        Ok(())
    }
}

fn disable_store_path() -> PathBuf {
    if let Ok(env) = std::env::var("LOOM_DISABLE_STORE_PATH") {
        return PathBuf::from(env);
    }
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(xdg).join("loom").join("disabled-shapes.json");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".cache")
            .join("loom")
            .join("disabled-shapes.json");
    }
    // Fallback to current-dir (should never happen on normal OS)
    PathBuf::from("disabled-shapes.json")
}

static NATIVE_ROUTE_DISABLED_SHAPES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn disabled_shapes_registry() -> &'static Mutex<HashSet<String>> {
    NATIVE_ROUTE_DISABLED_SHAPES.get_or_init(|| {
        let path = disable_store_path();
        let store = DisableStore::load_or_default(&path);
        let set: HashSet<String> = store.disabled_shapes.into_iter().collect();
        Mutex::new(set)
    })
}

pub fn is_shape_disabled(schema_fingerprint: &str) -> bool {
    disabled_shapes_registry().lock().unwrap().contains(schema_fingerprint)
}

pub fn disable_shape(schema_fingerprint: &str) {
    let mut guard = disabled_shapes_registry().lock().unwrap();
    let inserted = guard.insert(schema_fingerprint.to_string());
    drop(guard);
    if inserted {
        let path = disable_store_path();
        let store = DisableStore {
            version: DISABLE_STORE_VERSION,
            disabled_shapes: disabled_shapes_registry().lock().unwrap().iter().cloned().collect(),
            last_updated_secs: Some(epoch_now_secs()),
        };
        // Best-effort persistence; failures are non-fatal.
        let _ = store.save(&path);
    }
}

fn epoch_now_secs() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Clear the disabled-shapes registry **and** delete the persistent store.
/// Exposed for integration-test cleanup.  Never called in production code.
#[doc(hidden)]
pub fn reset_disabled_shapes() {
    disabled_shapes_registry().lock().unwrap().clear();
    let path = disable_store_path();
    let _ = fs::remove_file(&path);
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductionJitOptions {
    pub require_compatible_toolchain: bool,
    pub input_value_buffers: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionJitOutput {
    pub entry_symbol: String,
    pub row_count: u64,
    pub column_count: usize,
    pub value_buffers: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrowSemanticCodegenJitOutput {
    pub entry_symbol: String,
    pub row_count: u64,
    pub column_count: usize,
    pub backend_identity: String,
    pub columns: Vec<NativeArrowSemanticCodegenOutputColumn>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowSemanticCodegenRouteStatus {
    NativeCandidate,
    InterpreterFallback,
    FailClosed,
    Cancelled,
}

impl ArrowSemanticCodegenRouteStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeCandidate => "native-candidate",
            Self::InterpreterFallback => "interpreter-fallback",
            Self::FailClosed => "fail-closed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowSemanticCodegenCancellationCheckpoint {
    BeforeSupport,
    BeforeJit,
    BeforeValidation,
}

impl ArrowSemanticCodegenCancellationCheckpoint {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BeforeSupport => "before-support",
            Self::BeforeJit => "before-jit",
            Self::BeforeValidation => "before-validation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrowSemanticCodegenResourceEvidence {
    pub route_steps_completed: Vec<String>,
    pub cancellation_checkpoint: Option<String>,
    pub output_buffer_ownership: String,
    pub output_buffer_lifetime: String,
    pub release_assumption: String,
    pub output_value_buffer_bytes: u64,
    pub output_validity_buffer_bytes: u64,
    pub raw_pointer_identity_used: bool,
}

#[derive(Debug, Clone)]
pub struct ArrowSemanticCodegenProductionRouteReport {
    pub status: ArrowSemanticCodegenRouteStatus,
    pub support: NativeArrowSemanticCodegenSupportReport,
    pub jit_output: Option<ArrowSemanticCodegenJitOutput>,
    pub execution: Option<NativeArrowSemanticCodegenExecutionReport>,
    pub runtime_decision: Option<RuntimePlanDecisionReport>,
    pub replay_evidence: Option<NativeArrowSemanticCodegenReplayEvidence>,
    pub cacheable: bool,
    pub resource_evidence: ArrowSemanticCodegenResourceEvidence,
    pub diagnostics: Vec<NativeBackendDiagnostic>,
}

pub fn execute_arrow_semantic_codegen_jit(
    support: &NativeArrowSemanticCodegenSupportReport,
    cancellation: &NativeBackendCancellation,
) -> Result<ArrowSemanticCodegenJitOutput, NativeBackendDiagnostic> {
    if cancellation.cancelled {
        return Err(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::Cancelled,
            "$.cancellation.before_jit",
            cancellation.reason.clone().unwrap_or_else(|| {
                "production Arrow semantic codegen JIT was cancelled".to_string()
            }),
        ));
    }

    if !support.is_supported() {
        let message = support
            .first_error()
            .map(|diagnostic| diagnostic.message.clone())
            .unwrap_or_else(|| {
                "production Arrow semantic codegen requires supported inputs".to_string()
            });
        return Err(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::InvalidBackendArtifact,
            "$.codegen.support",
            message,
        ));
    }

    execute_arrow_semantic_codegen_jit_backend(support)
}

pub fn execute_arrow_semantic_codegen_production_route(
    bytes: &[u8],
    cancellation: &NativeBackendCancellation,
    projection: ProjectionSet,
    predicate: PredicateEnvelope,
    split: SplitDescriptor,
    policy: RuntimeSafetyPolicy,
) -> ArrowSemanticCodegenProductionRouteReport {
    execute_arrow_semantic_codegen_production_route_inner(
        bytes,
        cancellation,
        None,
        projection,
        predicate,
        split,
        policy,
    )
}

pub fn execute_arrow_semantic_codegen_production_route_with_cancellation_checkpoint(
    bytes: &[u8],
    checkpoint: ArrowSemanticCodegenCancellationCheckpoint,
    projection: ProjectionSet,
    predicate: PredicateEnvelope,
    split: SplitDescriptor,
    policy: RuntimeSafetyPolicy,
) -> ArrowSemanticCodegenProductionRouteReport {
    execute_arrow_semantic_codegen_production_route_inner(
        bytes,
        &NativeBackendCancellation::default(),
        Some(checkpoint),
        projection,
        predicate,
        split,
        policy,
    )
}

fn execute_arrow_semantic_codegen_production_route_inner(
    bytes: &[u8],
    cancellation: &NativeBackendCancellation,
    checkpoint: Option<ArrowSemanticCodegenCancellationCheckpoint>,
    projection: ProjectionSet,
    predicate: PredicateEnvelope,
    split: SplitDescriptor,
    policy: RuntimeSafetyPolicy,
) -> ArrowSemanticCodegenProductionRouteReport {
    let cancel_before_support = cancellation.cancelled
        || checkpoint == Some(ArrowSemanticCodegenCancellationCheckpoint::BeforeSupport);
    if cancellation.cancelled {
        return cancelled_arrow_semantic_codegen_route(
            unsupported_route_support(),
            None,
            ArrowSemanticCodegenCancellationCheckpoint::BeforeSupport,
            cancellation.reason.clone().unwrap_or_else(|| {
                "production Arrow semantic codegen route was cancelled before support extraction"
                    .to_string()
            }),
            Vec::new(),
        );
    }
    if cancel_before_support {
        return ArrowSemanticCodegenProductionRouteReport {
            status: ArrowSemanticCodegenRouteStatus::Cancelled,
            support: unsupported_route_support(),
            jit_output: None,
            execution: None,
            runtime_decision: None,
            replay_evidence: None,
            cacheable: false,
            resource_evidence: arrow_semantic_resource_evidence(
                &[],
                Some(ArrowSemanticCodegenCancellationCheckpoint::BeforeSupport),
                None,
            ),
            diagnostics: vec![NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::Cancelled,
                "$.cancellation.before_support",
                checkpoint
                    .map(|checkpoint| {
                        format!(
                            "production Arrow semantic codegen route was cancelled at {}",
                            checkpoint.as_str()
                        )
                    })
                    .unwrap_or_else(|| {
                    "production Arrow semantic codegen route was cancelled before support extraction"
                        .to_string()
                }),
            )],
        };
    }

    let support = prepare_native_arrow_semantic_codegen_support(bytes);
    if !support.is_supported() {
        let status = fallback_or_fail_closed(policy);
        let message = support
            .first_error()
            .map(|diagnostic| diagnostic.message.clone())
            .unwrap_or_else(|| {
                "production Arrow semantic codegen route requires supported inputs".to_string()
            });
        return ArrowSemanticCodegenProductionRouteReport {
            status,
            support,
            jit_output: None,
            execution: None,
            runtime_decision: None,
            replay_evidence: None,
            cacheable: false,
            resource_evidence: arrow_semantic_resource_evidence(&["support-rejected"], None, None),
            diagnostics: vec![NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::InvalidBackendArtifact,
                "$.codegen.support",
                message,
            )],
        };
    }

    // Phase 48 pre-check: if this shape has already been disabled because of a
    // prior native↔K trace divergence, short-circuit to fallback without running
    // the JIT or invoking the K oracle.
    let schema_fingerprint = support.schema_fingerprint.clone();
    if is_shape_disabled(&schema_fingerprint) {
        let status = fallback_or_fail_closed(policy);
        return ArrowSemanticCodegenProductionRouteReport {
            status,
            support,
            jit_output: None,
            execution: None,
            runtime_decision: None,
            replay_evidence: None,
            cacheable: false,
            resource_evidence: arrow_semantic_resource_evidence(
                &["support-extracted", "shape-disabled"],
                None,
                None,
            ),
            diagnostics: vec![NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::NativeShapeDisabled,
                "$.codegen.shape",
                format!(
                    "native route disabled for schema fingerprint {} due to prior divergence",
                    schema_fingerprint
                ),
            )],
        };
    }

    let before_jit_cancellation;
    let jit_cancellation =
        if checkpoint == Some(ArrowSemanticCodegenCancellationCheckpoint::BeforeJit) {
            before_jit_cancellation = NativeBackendCancellation::cancelled(
                "production Arrow semantic codegen route was cancelled before JIT execution",
            );
            &before_jit_cancellation
        } else {
            cancellation
        };
    let jit = match execute_arrow_semantic_codegen_jit(&support, jit_cancellation) {
        Ok(output) => output,
        Err(diagnostic) => {
            let status = if diagnostic.code == NativeBackendDiagnosticCode::Cancelled {
                ArrowSemanticCodegenRouteStatus::Cancelled
            } else {
                fallback_or_fail_closed(policy)
            };
            return ArrowSemanticCodegenProductionRouteReport {
                status,
                support,
                jit_output: None,
                execution: None,
                runtime_decision: None,
                replay_evidence: None,
                cacheable: false,
                resource_evidence: arrow_semantic_resource_evidence(
                    &["support-extracted"],
                    if diagnostic.code == NativeBackendDiagnosticCode::Cancelled {
                        Some(ArrowSemanticCodegenCancellationCheckpoint::BeforeJit)
                    } else {
                        None
                    },
                    None,
                ),
                diagnostics: vec![diagnostic],
            };
        }
    };

    let before_validation_cancellation;
    let validation_cancellation =
        if checkpoint == Some(ArrowSemanticCodegenCancellationCheckpoint::BeforeValidation) {
            before_validation_cancellation = NativeBackendCancellation::cancelled(
                "production Arrow semantic codegen route was cancelled before validation",
            );
            &before_validation_cancellation
        } else {
            cancellation
        };
    validate_arrow_semantic_codegen_production_route_output_with_cancellation(
        bytes,
        support,
        jit,
        validation_cancellation,
        projection,
        predicate,
        split,
        policy,
    )
}

pub fn validate_arrow_semantic_codegen_production_route_output(
    bytes: &[u8],
    support: NativeArrowSemanticCodegenSupportReport,
    jit_output: ArrowSemanticCodegenJitOutput,
    projection: ProjectionSet,
    predicate: PredicateEnvelope,
    split: SplitDescriptor,
    policy: RuntimeSafetyPolicy,
) -> ArrowSemanticCodegenProductionRouteReport {
    validate_arrow_semantic_codegen_production_route_output_with_cancellation(
        bytes,
        support,
        jit_output,
        &NativeBackendCancellation::default(),
        projection,
        predicate,
        split,
        policy,
    )
}

pub fn validate_arrow_semantic_codegen_production_route_output_with_cancellation(
    bytes: &[u8],
    support: NativeArrowSemanticCodegenSupportReport,
    jit_output: ArrowSemanticCodegenJitOutput,
    cancellation: &NativeBackendCancellation,
    projection: ProjectionSet,
    predicate: PredicateEnvelope,
    split: SplitDescriptor,
    policy: RuntimeSafetyPolicy,
) -> ArrowSemanticCodegenProductionRouteReport {
    if cancellation.cancelled {
        return cancelled_arrow_semantic_codegen_route(
            support,
            Some(jit_output),
            ArrowSemanticCodegenCancellationCheckpoint::BeforeValidation,
            cancellation.reason.clone().unwrap_or_else(|| {
                "production Arrow semantic codegen route was cancelled before validation"
                    .to_string()
            }),
            vec!["support-extracted", "jit-executed"],
        );
    }

    if let Err(diagnostic) = validate_arrow_semantic_jit_metadata(&support, &jit_output) {
        let status = fallback_or_fail_closed(policy);
        return ArrowSemanticCodegenProductionRouteReport {
            status,
            support,
            jit_output: Some(jit_output),
            execution: None,
            runtime_decision: None,
            replay_evidence: None,
            cacheable: false,
            resource_evidence: arrow_semantic_resource_evidence(
                &["support-extracted", "jit-executed"],
                None,
                None,
            ),
            diagnostics: vec![diagnostic],
        };
    }

    let execution = validate_native_arrow_semantic_codegen_output(
        bytes,
        &support,
        jit_output.backend_identity.clone(),
        jit_output.columns.clone(),
    );

    // Phase 48 post-validation: genuine divergence (NativeModelTraceMismatch)
    // disables the shape for the process lifetime.  Skip/unsupported outcomes
    // must NOT disable.
    let has_divergence = execution.validation().is_some_and(|v| {
        v.oracle_skip_reason.is_none()
            && v.diagnostics().iter().any(|d| {
                d.code == NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch
            })
    });

    let runtime_decision =
        decide_validated_native_arrow_semantic_codegen_runtime(&execution, policy);

    let replay_result = native_arrow_semantic_codegen_replay_evidence(
        bytes, &support, &execution, projection, predicate, split, policy,
    );
    let replay_diagnostic = replay_result.as_ref().err().cloned();
    let replay_evidence = replay_result.ok();

    let (cacheable, status, replay_evidence) = if has_divergence {
        disable_shape(&execution.schema_fingerprint);
        (
            false,
            fallback_or_fail_closed(policy),
            None,
        )
    } else {
        let cacheable = replay_evidence.is_some()
            && runtime_decision.decision == RuntimeExecutionDecision::NativeCandidate;
        let status = if cacheable {
            ArrowSemanticCodegenRouteStatus::NativeCandidate
        } else {
            fallback_or_fail_closed(policy)
        };
        (cacheable, status, replay_evidence)
    };

    let mut diagnostics: Vec<NativeBackendDiagnostic> = execution
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::NativeOutputMismatch,
                diagnostic.path.clone(),
                diagnostic.message.clone(),
            )
        })
        .collect();
    if has_divergence {
        diagnostics.push(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::NativeShapeDisabled,
            "$.codegen.shape",
            format!(
                "native route disabled for schema fingerprint {} due to trace divergence",
                execution.schema_fingerprint
            ),
        ));
    }
    if !has_divergence && execution.diagnostics().is_empty() {
        if let Some(diagnostic) = replay_diagnostic {
            diagnostics.push(NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::InvalidBackendArtifact,
                diagnostic.path,
                diagnostic.message,
            ));
        }
    }
    let resource_evidence = arrow_semantic_resource_evidence(
        &["support-extracted", "jit-executed", "validated"],
        None,
        Some(&jit_output),
    );

    ArrowSemanticCodegenProductionRouteReport {
        status,
        support,
        jit_output: Some(jit_output),
        execution: Some(execution),
        runtime_decision: Some(runtime_decision),
        replay_evidence,
        cacheable,
        resource_evidence,
        diagnostics,
    }
}

fn validate_arrow_semantic_jit_metadata(
    support: &NativeArrowSemanticCodegenSupportReport,
    jit_output: &ArrowSemanticCodegenJitOutput,
) -> Result<(), NativeBackendDiagnostic> {
    if jit_output.entry_symbol != ARROW_SEMANTIC_CODEGEN_JIT_ENTRY_SYMBOL {
        return Err(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::JitSymbolMissing,
            "$.jit.arrow_semantic.entry_symbol",
            format!(
                "production Arrow semantic JIT returned entry symbol '{}', expected '{}'",
                jit_output.entry_symbol, ARROW_SEMANTIC_CODEGEN_JIT_ENTRY_SYMBOL
            ),
        ));
    }

    if jit_output.row_count != support.row_count {
        return Err(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::InvalidBackendArtifact,
            "$.jit.arrow_semantic.row_count",
            format!(
                "production Arrow semantic JIT returned row count {}, expected {}",
                jit_output.row_count, support.row_count
            ),
        ));
    }

    if jit_output.column_count != support.column_count {
        return Err(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::InvalidBackendArtifact,
            "$.jit.arrow_semantic.column_count",
            format!(
                "production Arrow semantic JIT returned column count {}, expected {}",
                jit_output.column_count, support.column_count
            ),
        ));
    }

    if jit_output.columns.len() != support.column_count {
        return Err(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::NativeOutputMismatch,
            "$.jit.arrow_semantic.columns",
            format!(
                "production Arrow semantic JIT returned {} output column(s), expected {}",
                jit_output.columns.len(),
                support.column_count
            ),
        ));
    }

    Ok(())
}

fn cancelled_arrow_semantic_codegen_route(
    support: NativeArrowSemanticCodegenSupportReport,
    jit_output: Option<ArrowSemanticCodegenJitOutput>,
    checkpoint: ArrowSemanticCodegenCancellationCheckpoint,
    message: String,
    completed_steps: Vec<&str>,
) -> ArrowSemanticCodegenProductionRouteReport {
    ArrowSemanticCodegenProductionRouteReport {
        status: ArrowSemanticCodegenRouteStatus::Cancelled,
        support,
        jit_output,
        execution: None,
        runtime_decision: None,
        replay_evidence: None,
        cacheable: false,
        resource_evidence: arrow_semantic_resource_evidence(
            &completed_steps,
            Some(checkpoint),
            None,
        ),
        diagnostics: vec![NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::Cancelled,
            format!("$.cancellation.{}", checkpoint.as_str().replace('-', "_")),
            message,
        )],
    }
}

fn arrow_semantic_resource_evidence(
    completed_steps: &[&str],
    cancellation_checkpoint: Option<ArrowSemanticCodegenCancellationCheckpoint>,
    jit_output: Option<&ArrowSemanticCodegenJitOutput>,
) -> ArrowSemanticCodegenResourceEvidence {
    let (output_value_buffer_bytes, output_validity_buffer_bytes) = jit_output
        .map(|output| {
            output.columns.iter().fold((0_u64, 0_u64), |acc, column| {
                (
                    acc.0 + column.value_buffer.len() as u64,
                    acc.1
                        + column
                            .validity_buffer
                            .as_ref()
                            .map(|buffer| buffer.len() as u64)
                            .unwrap_or(0),
                )
            })
        })
        .unwrap_or((0, 0));

    ArrowSemanticCodegenResourceEvidence {
        route_steps_completed: completed_steps
            .iter()
            .map(|step| (*step).to_string())
            .collect(),
        cancellation_checkpoint: cancellation_checkpoint
            .map(|checkpoint| checkpoint.as_str().to_string()),
        output_buffer_ownership: "owned-rust-vec".to_string(),
        output_buffer_lifetime: "route-report-owned-before-host-abi-handoff".to_string(),
        release_assumption: "dropped-by-rust-owner; no raw pointer release callback exposed yet"
            .to_string(),
        output_value_buffer_bytes,
        output_validity_buffer_bytes,
        raw_pointer_identity_used: false,
    }
}

fn fallback_or_fail_closed(policy: RuntimeSafetyPolicy) -> ArrowSemanticCodegenRouteStatus {
    if matches!(policy.fallback, RuntimeFallbackPolicy::AllowInterpreter) {
        ArrowSemanticCodegenRouteStatus::InterpreterFallback
    } else {
        ArrowSemanticCodegenRouteStatus::FailClosed
    }
}

fn unsupported_route_support() -> NativeArrowSemanticCodegenSupportReport {
    prepare_native_arrow_semantic_codegen_support(b"")
}

pub fn execute_prepared_production_jit(
    report: &NativeBackendReport,
    cancellation: &NativeBackendCancellation,
    options: ProductionJitOptions,
) -> Result<ProductionJitOutput, NativeBackendReport> {
    if cancellation.cancelled {
        return Err(report_with_diagnostic(
            report,
            NativeBackendStatus::Cancelled,
            NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::Cancelled,
                "$.cancellation",
                cancellation
                    .reason
                    .clone()
                    .unwrap_or_else(|| "production JIT request was cancelled".to_string()),
            ),
        ));
    }

    if report.status != NativeBackendStatus::Accepted || !report.diagnostics.is_empty() {
        return Err(report_with_diagnostic(
            report,
            NativeBackendStatus::FailClosed,
            NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::InvalidBackendArtifact,
                "$.backend_report.status",
                "production JIT requires an accepted backend report",
            ),
        ));
    }

    let Some(artifact) = report.artifact.as_ref() else {
        return Err(report_with_diagnostic(
            report,
            NativeBackendStatus::FailClosed,
            NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::InvalidBackendArtifact,
                "$.backend_report.artifact",
                "production JIT requires a prepared backend artifact",
            ),
        ));
    };

    if artifact.entry_symbol.as_deref() != Some(PRODUCTION_JIT_ENTRY_SYMBOL) {
        return Err(report_with_diagnostic(
            report,
            NativeBackendStatus::FailClosed,
            NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::JitSymbolMissing,
                "$.backend_report.artifact.entry_symbol",
                format!("JIT entry symbol '{PRODUCTION_JIT_ENTRY_SYMBOL}' was not found"),
            ),
        ));
    }

    let buffers = plan_arrow_buffers_from_decode_dialect(&artifact.lowering_facts);
    let Some(table) = buffers.table() else {
        let message = buffers
            .first_error()
            .map(|diagnostic| diagnostic.message.clone())
            .unwrap_or_else(|| "production JIT requires supported primitive buffers".to_string());
        return Err(report_with_diagnostic(
            report,
            NativeBackendStatus::FailClosed,
            NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::InvalidBackendArtifact,
                "$.backend_report.artifact.lowering_facts",
                message,
            ),
        ));
    };

    let toolchain = probe_toolchain();
    if !toolchain.compatible {
        let explicit_skip = std::env::var("LOOM_ALLOW_NATIVE_TOOL_SKIP")
            .map(|value| value == "1")
            .unwrap_or(false);
        let (status, code, message) = if explicit_skip && !options.require_compatible_toolchain {
            (
                NativeBackendStatus::SkippedToolchain,
                NativeBackendDiagnosticCode::ToolchainSkipped,
                "production JIT skipped by explicit LOOM_ALLOW_NATIVE_TOOL_SKIP=1",
            )
        } else {
            (
                NativeBackendStatus::FailClosed,
                NativeBackendDiagnosticCode::ToolchainFailed,
                "compatible MLIR/LLVM toolchain is required before production JIT execution",
            )
        };
        return Err(report_with_diagnostic(
            report,
            status,
            NativeBackendDiagnostic::new(code, "$.toolchain", message),
        ));
    }

    if cancellation.cancelled {
        return Err(report_with_diagnostic(
            report,
            NativeBackendStatus::Cancelled,
            NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::Cancelled,
                "$.cancellation",
                "production JIT request was cancelled before execution",
            ),
        ));
    }

    let value_buffers =
        execute_raw_copy_mlir(table, &options.input_value_buffers).map_err(|diagnostic| {
            report_with_diagnostic(report, NativeBackendStatus::FailClosed, diagnostic)
        })?;
    Ok(ProductionJitOutput {
        entry_symbol: PRODUCTION_JIT_ENTRY_SYMBOL.to_string(),
        row_count: table.row_count,
        column_count: table.columns.len(),
        value_buffers,
    })
}

pub fn compare_production_jit_output(
    report: &NativeBackendReport,
    expected: &[Vec<u8>],
    output: &ProductionJitOutput,
) -> Result<(), NativeBackendReport> {
    if expected == output.value_buffers.as_slice() {
        return Ok(());
    }

    Err(report_with_diagnostic(
        report,
        NativeBackendStatus::FailClosed,
        NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::NativeOutputMismatch,
            "$.jit.output",
            "production JIT output did not match interpreter/reference output",
        ),
    ))
}

fn execute_raw_copy_mlir(
    table: &ArrowTableBufferPlan,
    input_value_buffers: &[Vec<u8>],
) -> Result<Vec<Vec<u8>>, NativeBackendDiagnostic> {
    if input_value_buffers.is_empty() {
        return Err(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::InvalidBackendArtifact,
            "$.jit.input_value_buffers",
            "production JIT requires explicit artifact input value buffers",
        ));
    }

    if input_value_buffers.len() != table.columns.len() {
        return Err(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::InvalidBackendArtifact,
            "$.jit.input_value_buffers",
            format!(
                "production JIT expected {} input value buffer(s), got {}",
                table.columns.len(),
                input_value_buffers.len()
            ),
        ));
    }
    for (idx, (column, buffer)) in table.columns.iter().zip(input_value_buffers).enumerate() {
        let expected_len = column.primitive.value_buffer_bytes as usize;
        if buffer.len() != expected_len {
            return Err(NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::InvalidBackendArtifact,
                format!("$.jit.input_value_buffers[{idx}]"),
                format!(
                    "production JIT input buffer has {} bytes, expected exactly {}",
                    buffer.len(),
                    expected_len
                ),
            ));
        }
    }

    execute_raw_copy_mlir_backend(table, input_value_buffers)
}

#[cfg(feature = "melior")]
fn execute_arrow_semantic_codegen_jit_backend(
    support: &NativeArrowSemanticCodegenSupportReport,
) -> Result<ArrowSemanticCodegenJitOutput, NativeBackendDiagnostic> {
    use melior::dialect::DialectRegistry;
    use melior::ir::Module;
    use melior::pass;
    use melior::utility::{
        parse_pass_pipeline, register_all_dialects, register_all_llvm_translations,
        register_all_passes,
    };
    use melior::{Context, ExecutionEngine};

    let slot_plans = arrow_semantic_slot_plans(support)?;
    let mlir_text = lower_arrow_semantic_slots_to_standard_mlir(&slot_plans);

    let context = Context::new();
    let registry = DialectRegistry::new();
    register_all_dialects(&registry);
    context.append_dialect_registry(&registry);
    context.load_all_available_dialects();
    register_all_llvm_translations(&context);
    register_all_passes();

    let mut module = Module::parse(&context, &mlir_text).ok_or_else(|| {
        NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::JitUnavailable,
            "$.jit.arrow_semantic.mlir.parse",
            "production Arrow semantic JIT failed to parse generated MLIR module",
        )
    })?;

    let pass_manager = pass::PassManager::new(&context);
    parse_pass_pipeline(
        pass_manager.as_operation_pass_manager(),
        LLVM_LOWERING_PIPELINE,
    )
    .map_err(|err| {
        NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::JitUnavailable,
            "$.jit.arrow_semantic.mlir.pass_pipeline",
            format!(
                "production Arrow semantic JIT failed to parse LLVM lowering pipeline: {err:?}"
            ),
        )
    })?;
    pass_manager.run(&mut module).map_err(|err| {
        NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::JitUnavailable,
            "$.jit.arrow_semantic.mlir.lower_to_llvm",
            format!("production Arrow semantic JIT failed to lower MLIR module to LLVM: {err:?}"),
        )
    })?;

    let engine = ExecutionEngine::new(&module, 2, &[], false, false);
    if engine
        .lookup(ARROW_SEMANTIC_CODEGEN_JIT_ENTRY_SYMBOL)
        .is_null()
    {
        return Err(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::JitSymbolMissing,
            "$.jit.arrow_semantic.symbol",
            format!("JIT entry symbol '{ARROW_SEMANTIC_CODEGEN_JIT_ENTRY_SYMBOL}' was not found"),
        ));
    }

    let mut slots = slot_plans
        .iter()
        .map(ArrowSemanticJitSlotStorage::new)
        .collect::<Result<Vec<_>, _>>()?;
    let mut args = Vec::with_capacity(slots.len() * 2);
    for slot in slots.iter_mut() {
        args.push(slot.input_descriptor_ptr());
    }
    for slot in slots.iter_mut() {
        args.push(slot.output_descriptor_ptr());
    }

    unsafe {
        engine
            .invoke_packed(ARROW_SEMANTIC_CODEGEN_JIT_ENTRY_SYMBOL, &mut args)
            .map_err(|err| {
                NativeBackendDiagnostic::new(
                    NativeBackendDiagnosticCode::JitUnavailable,
                    "$.jit.arrow_semantic.invoke",
                    format!(
                        "production Arrow semantic JIT ExecutionEngine invocation failed: {err:?}"
                    ),
                )
            })?;
    }

    let columns = arrow_semantic_output_columns_from_slots(support, slots)?;
    Ok(ArrowSemanticCodegenJitOutput {
        entry_symbol: ARROW_SEMANTIC_CODEGEN_JIT_ENTRY_SYMBOL.to_string(),
        row_count: support.row_count,
        column_count: support.column_count,
        backend_identity: format!(
            "backend=loom-native-melior;entry={};pipeline={};feature=melior",
            ARROW_SEMANTIC_CODEGEN_JIT_ENTRY_SYMBOL, LLVM_LOWERING_PIPELINE
        ),
        columns,
    })
}

#[cfg(not(feature = "melior"))]
fn execute_arrow_semantic_codegen_jit_backend(
    _support: &NativeArrowSemanticCodegenSupportReport,
) -> Result<ArrowSemanticCodegenJitOutput, NativeBackendDiagnostic> {
    Err(NativeBackendDiagnostic::new(
        NativeBackendDiagnosticCode::JitUnavailable,
        "$.jit.arrow_semantic",
        "production Arrow semantic codegen JIT requires the loom-native-melior melior feature",
    ))
}

#[cfg(feature = "melior")]
fn execute_raw_copy_mlir_backend(
    table: &ArrowTableBufferPlan,
    input_value_buffers: &[Vec<u8>],
) -> Result<Vec<Vec<u8>>, NativeBackendDiagnostic> {
    use melior::dialect::DialectRegistry;
    use melior::ir::Module;
    use melior::pass;
    use melior::utility::{
        parse_pass_pipeline, register_all_dialects, register_all_llvm_translations,
        register_all_passes,
    };
    use melior::{Context, ExecutionEngine};

    let context = Context::new();
    let registry = DialectRegistry::new();
    register_all_dialects(&registry);
    context.append_dialect_registry(&registry);
    context.load_all_available_dialects();
    register_all_llvm_translations(&context);
    register_all_passes();

    let mlir_text = lower_arrow_raw_copy_to_standard_mlir(table).map_err(|report| {
        let message = report
            .first_error()
            .map(|diagnostic| diagnostic.message.clone())
            .unwrap_or_else(|| "production JIT raw-copy MLIR lowering failed".to_string());
        NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::InvalidBackendArtifact,
            "$.jit.mlir",
            message,
        )
    })?;
    let mut module = Module::parse(&context, &mlir_text).ok_or_else(|| {
        NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::JitUnavailable,
            "$.jit.mlir.parse",
            "production JIT failed to parse raw-copy MLIR module",
        )
    })?;

    let pass_manager = pass::PassManager::new(&context);
    parse_pass_pipeline(
        pass_manager.as_operation_pass_manager(),
        LLVM_LOWERING_PIPELINE,
    )
    .map_err(|err| {
        NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::JitUnavailable,
            "$.jit.mlir.pass_pipeline",
            format!("production JIT failed to parse LLVM lowering pipeline: {err:?}"),
        )
    })?;
    pass_manager.run(&mut module).map_err(|err| {
        NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::JitUnavailable,
            "$.jit.mlir.lower_to_llvm",
            format!("production JIT failed to lower MLIR module to LLVM: {err:?}"),
        )
    })?;

    let engine = ExecutionEngine::new(&module, 2, &[], false, false);
    if engine.lookup(PRODUCTION_JIT_ENTRY_SYMBOL).is_null() {
        return Err(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::JitSymbolMissing,
            "$.jit.symbol",
            format!("JIT entry symbol '{PRODUCTION_JIT_ENTRY_SYMBOL}' was not found"),
        ));
    }

    let mut columns = table
        .columns
        .iter()
        .zip(input_value_buffers)
        .map(|(column, input)| JitColumnStorage::new(column, input, table.row_count))
        .collect::<Result<Vec<_>, _>>()?;
    let mut rows = table.row_count as isize;
    let mut args = Vec::with_capacity(columns.len() * 2 + 1);
    for column in columns.iter_mut() {
        args.push(column.input_descriptor_ptr());
    }
    for column in columns.iter_mut() {
        args.push(column.output_descriptor_ptr());
    }
    args.push(&mut rows as *mut isize as *mut ());

    unsafe {
        engine
            .invoke_packed(PRODUCTION_JIT_ENTRY_SYMBOL, &mut args)
            .map_err(|err| {
                NativeBackendDiagnostic::new(
                    NativeBackendDiagnosticCode::JitUnavailable,
                    "$.jit.invoke",
                    format!("production JIT ExecutionEngine invocation failed: {err:?}"),
                )
            })?;
    }

    Ok(columns
        .into_iter()
        .map(JitColumnStorage::into_bytes)
        .collect())
}

#[cfg(not(feature = "melior"))]
fn execute_raw_copy_mlir_backend(
    _table: &ArrowTableBufferPlan,
    _input_value_buffers: &[Vec<u8>],
) -> Result<Vec<Vec<u8>>, NativeBackendDiagnostic> {
    Err(NativeBackendDiagnostic::new(
        NativeBackendDiagnosticCode::JitUnavailable,
        "$.jit",
        "production JIT requires the loom-native-melior melior feature",
    ))
}

#[cfg(feature = "melior")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArrowSemanticSlotKind {
    I32,
    I64,
    F32,
    F64,
    Bytes,
}

#[cfg(feature = "melior")]
impl ArrowSemanticSlotKind {
    fn mlir_type(self) -> &'static str {
        match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Bytes => "i8",
        }
    }
}

#[cfg(feature = "melior")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArrowSemanticSlotRole {
    Value,
    Validity,
}

#[cfg(feature = "melior")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct ArrowSemanticSlotPlan {
    column_index: usize,
    role: ArrowSemanticSlotRole,
    symbol: String,
    kind: ArrowSemanticSlotKind,
    input_bytes: Vec<u8>,
}

#[cfg(feature = "melior")]
fn arrow_semantic_slot_plans(
    support: &NativeArrowSemanticCodegenSupportReport,
) -> Result<Vec<ArrowSemanticSlotPlan>, NativeBackendDiagnostic> {
    let mut slots = Vec::new();
    for column in support.columns() {
        let value_kind = match (&column.value_buffer_kind, &column.data_type) {
            (NativeArrowSemanticCodegenBufferKind::BooleanValueBitmap, DataType::Boolean) => {
                ArrowSemanticSlotKind::Bytes
            }
            (NativeArrowSemanticCodegenBufferKind::FixedWidthValue, DataType::Int32) => {
                ArrowSemanticSlotKind::I32
            }
            (NativeArrowSemanticCodegenBufferKind::FixedWidthValue, DataType::Int64) => {
                ArrowSemanticSlotKind::I64
            }
            (NativeArrowSemanticCodegenBufferKind::FixedWidthValue, DataType::Float32) => {
                ArrowSemanticSlotKind::F32
            }
            (NativeArrowSemanticCodegenBufferKind::FixedWidthValue, DataType::Float64) => {
                ArrowSemanticSlotKind::F64
            }
            _ => {
                return Err(NativeBackendDiagnostic::new(
                    NativeBackendDiagnosticCode::InvalidBackendArtifact,
                    format!("$.codegen.columns[{}].value_buffer_kind", column.index),
                    "production Arrow semantic JIT received an unsupported value buffer kind/type pair",
                ));
            }
        };
        slots.push(ArrowSemanticSlotPlan {
            column_index: column.index,
            role: ArrowSemanticSlotRole::Value,
            symbol: format!("c{}_value", column.index),
            kind: value_kind,
            input_bytes: column.value_buffer.clone(),
        });
        if let Some(validity) = column.validity_buffer.as_ref() {
            slots.push(ArrowSemanticSlotPlan {
                column_index: column.index,
                role: ArrowSemanticSlotRole::Validity,
                symbol: format!("c{}_validity", column.index),
                kind: ArrowSemanticSlotKind::Bytes,
                input_bytes: validity.clone(),
            });
        }
    }
    Ok(slots)
}

#[cfg(feature = "melior")]
fn lower_arrow_semantic_slots_to_standard_mlir(slots: &[ArrowSemanticSlotPlan]) -> String {
    let input_args = slots
        .iter()
        .map(|slot| format!("%{}_in: memref<?x{}>", slot.symbol, slot.kind.mlir_type()));
    let output_args = slots
        .iter()
        .map(|slot| format!("%{}_out: memref<?x{}>", slot.symbol, slot.kind.mlir_type()));
    let args = input_args.chain(output_args).collect::<Vec<_>>().join(", ");

    let mut text = String::new();
    text.push_str("module {\n");
    text.push_str(&format!(
        "  func.func @{ARROW_SEMANTIC_CODEGEN_JIT_ENTRY_SYMBOL}({args}) attributes {{ llvm.emit_c_interface }} {{\n"
    ));
    text.push_str("    %c0 = arith.constant 0 : index\n");
    text.push_str("    %c1 = arith.constant 1 : index\n");
    for slot in slots {
        let ty = slot.kind.mlir_type();
        text.push_str(&format!(
            "    %len_{} = memref.dim %{}_in, %c0 : memref<?x{}>\n",
            slot.symbol, slot.symbol, ty
        ));
        text.push_str(&format!(
            "    scf.for %i_{} = %c0 to %len_{} step %c1 {{\n",
            slot.symbol, slot.symbol
        ));
        text.push_str(&format!(
            "      %value_{} = memref.load %{}_in[%i_{}] : memref<?x{}>\n",
            slot.symbol, slot.symbol, slot.symbol, ty
        ));
        text.push_str(&format!(
            "      memref.store %value_{}, %{}_out[%i_{}] : memref<?x{}>\n",
            slot.symbol, slot.symbol, slot.symbol, ty
        ));
        text.push_str("    }\n");
    }
    text.push_str("    return\n");
    text.push_str("  }\n");
    text.push_str("}\n");
    text
}

#[cfg(feature = "melior")]
#[repr(C)]
struct MemRef1D<T> {
    allocated: *mut T,
    aligned: *mut T,
    offset: isize,
    size0: isize,
    stride0: isize,
}

#[cfg(feature = "melior")]
impl<T> MemRef1D<T> {
    fn new(values: &mut [T]) -> Self {
        let ptr = values.as_mut_ptr();
        Self {
            allocated: ptr,
            aligned: ptr,
            offset: 0,
            size0: values.len() as isize,
            stride0: 1,
        }
    }
}

#[cfg(feature = "melior")]
struct ArrowSemanticJitSlotStorage {
    column_index: usize,
    role: ArrowSemanticSlotRole,
    inner: ArrowSemanticJitSlotInner,
}

#[cfg(feature = "melior")]
enum ArrowSemanticJitSlotInner {
    I32 {
        input: Vec<i32>,
        output: Vec<i32>,
        input_desc: MemRef1D<i32>,
        output_desc: MemRef1D<i32>,
        input_arg: *mut MemRef1D<i32>,
        output_arg: *mut MemRef1D<i32>,
    },
    I64 {
        input: Vec<i64>,
        output: Vec<i64>,
        input_desc: MemRef1D<i64>,
        output_desc: MemRef1D<i64>,
        input_arg: *mut MemRef1D<i64>,
        output_arg: *mut MemRef1D<i64>,
    },
    F32 {
        input: Vec<f32>,
        output: Vec<f32>,
        input_desc: MemRef1D<f32>,
        output_desc: MemRef1D<f32>,
        input_arg: *mut MemRef1D<f32>,
        output_arg: *mut MemRef1D<f32>,
    },
    F64 {
        input: Vec<f64>,
        output: Vec<f64>,
        input_desc: MemRef1D<f64>,
        output_desc: MemRef1D<f64>,
        input_arg: *mut MemRef1D<f64>,
        output_arg: *mut MemRef1D<f64>,
    },
    Bytes {
        input: Vec<u8>,
        output: Vec<u8>,
        input_desc: MemRef1D<u8>,
        output_desc: MemRef1D<u8>,
        input_arg: *mut MemRef1D<u8>,
        output_arg: *mut MemRef1D<u8>,
    },
}

#[cfg(feature = "melior")]
impl ArrowSemanticJitSlotStorage {
    fn new(plan: &ArrowSemanticSlotPlan) -> Result<Self, NativeBackendDiagnostic> {
        let inner = match plan.kind {
            ArrowSemanticSlotKind::I32 => {
                let mut input = bytes_to_i32(&plan.input_bytes)?;
                let mut output = vec![0i32; input.len()];
                let input_desc = MemRef1D::new(&mut input);
                let output_desc = MemRef1D::new(&mut output);
                ArrowSemanticJitSlotInner::I32 {
                    input,
                    output,
                    input_desc,
                    output_desc,
                    input_arg: std::ptr::null_mut(),
                    output_arg: std::ptr::null_mut(),
                }
            }
            ArrowSemanticSlotKind::I64 => {
                let mut input = bytes_to_i64(&plan.input_bytes)?;
                let mut output = vec![0i64; input.len()];
                let input_desc = MemRef1D::new(&mut input);
                let output_desc = MemRef1D::new(&mut output);
                ArrowSemanticJitSlotInner::I64 {
                    input,
                    output,
                    input_desc,
                    output_desc,
                    input_arg: std::ptr::null_mut(),
                    output_arg: std::ptr::null_mut(),
                }
            }
            ArrowSemanticSlotKind::F32 => {
                let mut input = bytes_to_f32(&plan.input_bytes)?;
                let mut output = vec![0f32; input.len()];
                let input_desc = MemRef1D::new(&mut input);
                let output_desc = MemRef1D::new(&mut output);
                ArrowSemanticJitSlotInner::F32 {
                    input,
                    output,
                    input_desc,
                    output_desc,
                    input_arg: std::ptr::null_mut(),
                    output_arg: std::ptr::null_mut(),
                }
            }
            ArrowSemanticSlotKind::F64 => {
                let mut input = bytes_to_f64(&plan.input_bytes)?;
                let mut output = vec![0f64; input.len()];
                let input_desc = MemRef1D::new(&mut input);
                let output_desc = MemRef1D::new(&mut output);
                ArrowSemanticJitSlotInner::F64 {
                    input,
                    output,
                    input_desc,
                    output_desc,
                    input_arg: std::ptr::null_mut(),
                    output_arg: std::ptr::null_mut(),
                }
            }
            ArrowSemanticSlotKind::Bytes => {
                let mut input = plan.input_bytes.clone();
                let mut output = vec![0u8; input.len()];
                let input_desc = MemRef1D::new(&mut input);
                let output_desc = MemRef1D::new(&mut output);
                ArrowSemanticJitSlotInner::Bytes {
                    input,
                    output,
                    input_desc,
                    output_desc,
                    input_arg: std::ptr::null_mut(),
                    output_arg: std::ptr::null_mut(),
                }
            }
        };
        Ok(Self {
            column_index: plan.column_index,
            role: plan.role,
            inner,
        })
    }

    fn input_descriptor_ptr(&mut self) -> *mut () {
        match &mut self.inner {
            ArrowSemanticJitSlotInner::I32 {
                input_desc,
                input_arg,
                ..
            } => {
                *input_arg = input_desc as *mut MemRef1D<i32>;
                input_arg as *mut *mut MemRef1D<i32> as *mut ()
            }
            ArrowSemanticJitSlotInner::I64 {
                input_desc,
                input_arg,
                ..
            } => {
                *input_arg = input_desc as *mut MemRef1D<i64>;
                input_arg as *mut *mut MemRef1D<i64> as *mut ()
            }
            ArrowSemanticJitSlotInner::F32 {
                input_desc,
                input_arg,
                ..
            } => {
                *input_arg = input_desc as *mut MemRef1D<f32>;
                input_arg as *mut *mut MemRef1D<f32> as *mut ()
            }
            ArrowSemanticJitSlotInner::F64 {
                input_desc,
                input_arg,
                ..
            } => {
                *input_arg = input_desc as *mut MemRef1D<f64>;
                input_arg as *mut *mut MemRef1D<f64> as *mut ()
            }
            ArrowSemanticJitSlotInner::Bytes {
                input_desc,
                input_arg,
                ..
            } => {
                *input_arg = input_desc as *mut MemRef1D<u8>;
                input_arg as *mut *mut MemRef1D<u8> as *mut ()
            }
        }
    }

    fn output_descriptor_ptr(&mut self) -> *mut () {
        match &mut self.inner {
            ArrowSemanticJitSlotInner::I32 {
                output_desc,
                output_arg,
                ..
            } => {
                *output_arg = output_desc as *mut MemRef1D<i32>;
                output_arg as *mut *mut MemRef1D<i32> as *mut ()
            }
            ArrowSemanticJitSlotInner::I64 {
                output_desc,
                output_arg,
                ..
            } => {
                *output_arg = output_desc as *mut MemRef1D<i64>;
                output_arg as *mut *mut MemRef1D<i64> as *mut ()
            }
            ArrowSemanticJitSlotInner::F32 {
                output_desc,
                output_arg,
                ..
            } => {
                *output_arg = output_desc as *mut MemRef1D<f32>;
                output_arg as *mut *mut MemRef1D<f32> as *mut ()
            }
            ArrowSemanticJitSlotInner::F64 {
                output_desc,
                output_arg,
                ..
            } => {
                *output_arg = output_desc as *mut MemRef1D<f64>;
                output_arg as *mut *mut MemRef1D<f64> as *mut ()
            }
            ArrowSemanticJitSlotInner::Bytes {
                output_desc,
                output_arg,
                ..
            } => {
                *output_arg = output_desc as *mut MemRef1D<u8>;
                output_arg as *mut *mut MemRef1D<u8> as *mut ()
            }
        }
    }

    fn into_output_bytes(self) -> (usize, ArrowSemanticSlotRole, Vec<u8>) {
        let bytes = match self.inner {
            ArrowSemanticJitSlotInner::I32 { input, output, .. } => {
                let _keepalive = input;
                output.into_iter().flat_map(i32::to_le_bytes).collect()
            }
            ArrowSemanticJitSlotInner::I64 { input, output, .. } => {
                let _keepalive = input;
                output.into_iter().flat_map(i64::to_le_bytes).collect()
            }
            ArrowSemanticJitSlotInner::F32 { input, output, .. } => {
                let _keepalive = input;
                output.into_iter().flat_map(f32::to_le_bytes).collect()
            }
            ArrowSemanticJitSlotInner::F64 { input, output, .. } => {
                let _keepalive = input;
                output.into_iter().flat_map(f64::to_le_bytes).collect()
            }
            ArrowSemanticJitSlotInner::Bytes { input, output, .. } => {
                let _keepalive = input;
                output
            }
        };
        (self.column_index, self.role, bytes)
    }
}

#[cfg(feature = "melior")]
fn arrow_semantic_output_columns_from_slots(
    support: &NativeArrowSemanticCodegenSupportReport,
    slots: Vec<ArrowSemanticJitSlotStorage>,
) -> Result<Vec<NativeArrowSemanticCodegenOutputColumn>, NativeBackendDiagnostic> {
    let mut outputs = slots
        .into_iter()
        .map(ArrowSemanticJitSlotStorage::into_output_bytes)
        .collect::<Vec<_>>();
    let mut columns = Vec::with_capacity(support.column_count);
    for column in support.columns() {
        let value_pos = outputs
            .iter()
            .position(|(idx, role, _)| {
                *idx == column.index && *role == ArrowSemanticSlotRole::Value
            })
            .ok_or_else(|| {
                NativeBackendDiagnostic::new(
                    NativeBackendDiagnosticCode::NativeOutputMismatch,
                    format!("$.jit.arrow_semantic.columns[{}].value", column.index),
                    "production Arrow semantic JIT did not return a value buffer",
                )
            })?;
        let (_, _, value_buffer) = outputs.remove(value_pos);
        let validity_buffer = if column.validity_buffer.is_some() {
            let validity_pos = outputs
                .iter()
                .position(|(idx, role, _)| {
                    *idx == column.index && *role == ArrowSemanticSlotRole::Validity
                })
                .ok_or_else(|| {
                    NativeBackendDiagnostic::new(
                        NativeBackendDiagnosticCode::NativeOutputMismatch,
                        format!("$.jit.arrow_semantic.columns[{}].validity", column.index),
                        "production Arrow semantic JIT did not return a validity buffer",
                    )
                })?;
            let (_, _, validity) = outputs.remove(validity_pos);
            Some(validity)
        } else {
            None
        };
        columns.push(NativeArrowSemanticCodegenOutputColumn {
            index: column.index,
            value_buffer,
            validity_buffer,
        });
    }
    Ok(columns)
}

#[cfg(feature = "melior")]
enum JitColumnStorage {
    I32 {
        input: Vec<i32>,
        output: Vec<i32>,
        input_desc: MemRef1D<i32>,
        output_desc: MemRef1D<i32>,
        input_arg: *mut MemRef1D<i32>,
        output_arg: *mut MemRef1D<i32>,
    },
    I64 {
        input: Vec<i64>,
        output: Vec<i64>,
        input_desc: MemRef1D<i64>,
        output_desc: MemRef1D<i64>,
        input_arg: *mut MemRef1D<i64>,
        output_arg: *mut MemRef1D<i64>,
    },
    F32 {
        input: Vec<f32>,
        output: Vec<f32>,
        input_desc: MemRef1D<f32>,
        output_desc: MemRef1D<f32>,
        input_arg: *mut MemRef1D<f32>,
        output_arg: *mut MemRef1D<f32>,
    },
    F64 {
        input: Vec<f64>,
        output: Vec<f64>,
        input_desc: MemRef1D<f64>,
        output_desc: MemRef1D<f64>,
        input_arg: *mut MemRef1D<f64>,
        output_arg: *mut MemRef1D<f64>,
    },
}

#[cfg(feature = "melior")]
impl JitColumnStorage {
    fn new(
        column: &ArrowColumnBufferPlan,
        input_bytes: &[u8],
        row_count: u64,
    ) -> Result<Self, NativeBackendDiagnostic> {
        match column.primitive.primitive_type {
            PrimitiveArrowType::Int32 => {
                let mut input = bytes_to_i32(input_bytes)?;
                let mut output = vec![0i32; row_count as usize];
                let input_desc = MemRef1D::new(&mut input);
                let output_desc = MemRef1D::new(&mut output);
                Ok(Self::I32 {
                    input,
                    output,
                    input_desc,
                    output_desc,
                    input_arg: std::ptr::null_mut(),
                    output_arg: std::ptr::null_mut(),
                })
            }
            PrimitiveArrowType::Int64 => {
                let mut input = bytes_to_i64(input_bytes)?;
                let mut output = vec![0i64; row_count as usize];
                let input_desc = MemRef1D::new(&mut input);
                let output_desc = MemRef1D::new(&mut output);
                Ok(Self::I64 {
                    input,
                    output,
                    input_desc,
                    output_desc,
                    input_arg: std::ptr::null_mut(),
                    output_arg: std::ptr::null_mut(),
                })
            }
            PrimitiveArrowType::Float32 => {
                let mut input = bytes_to_f32(input_bytes)?;
                let mut output = vec![0f32; row_count as usize];
                let input_desc = MemRef1D::new(&mut input);
                let output_desc = MemRef1D::new(&mut output);
                Ok(Self::F32 {
                    input,
                    output,
                    input_desc,
                    output_desc,
                    input_arg: std::ptr::null_mut(),
                    output_arg: std::ptr::null_mut(),
                })
            }
            PrimitiveArrowType::Float64 => {
                let mut input = bytes_to_f64(input_bytes)?;
                let mut output = vec![0f64; row_count as usize];
                let input_desc = MemRef1D::new(&mut input);
                let output_desc = MemRef1D::new(&mut output);
                Ok(Self::F64 {
                    input,
                    output,
                    input_desc,
                    output_desc,
                    input_arg: std::ptr::null_mut(),
                    output_arg: std::ptr::null_mut(),
                })
            }
        }
    }

    fn input_descriptor_ptr(&mut self) -> *mut () {
        match self {
            Self::I32 {
                input_desc,
                input_arg,
                ..
            } => {
                *input_arg = input_desc as *mut MemRef1D<i32>;
                input_arg as *mut *mut MemRef1D<i32> as *mut ()
            }
            Self::I64 {
                input_desc,
                input_arg,
                ..
            } => {
                *input_arg = input_desc as *mut MemRef1D<i64>;
                input_arg as *mut *mut MemRef1D<i64> as *mut ()
            }
            Self::F32 {
                input_desc,
                input_arg,
                ..
            } => {
                *input_arg = input_desc as *mut MemRef1D<f32>;
                input_arg as *mut *mut MemRef1D<f32> as *mut ()
            }
            Self::F64 {
                input_desc,
                input_arg,
                ..
            } => {
                *input_arg = input_desc as *mut MemRef1D<f64>;
                input_arg as *mut *mut MemRef1D<f64> as *mut ()
            }
        }
    }

    fn output_descriptor_ptr(&mut self) -> *mut () {
        match self {
            Self::I32 {
                output_desc,
                output_arg,
                ..
            } => {
                *output_arg = output_desc as *mut MemRef1D<i32>;
                output_arg as *mut *mut MemRef1D<i32> as *mut ()
            }
            Self::I64 {
                output_desc,
                output_arg,
                ..
            } => {
                *output_arg = output_desc as *mut MemRef1D<i64>;
                output_arg as *mut *mut MemRef1D<i64> as *mut ()
            }
            Self::F32 {
                output_desc,
                output_arg,
                ..
            } => {
                *output_arg = output_desc as *mut MemRef1D<f32>;
                output_arg as *mut *mut MemRef1D<f32> as *mut ()
            }
            Self::F64 {
                output_desc,
                output_arg,
                ..
            } => {
                *output_arg = output_desc as *mut MemRef1D<f64>;
                output_arg as *mut *mut MemRef1D<f64> as *mut ()
            }
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        match self {
            Self::I32 { input, output, .. } => {
                let _keepalive = input;
                output.into_iter().flat_map(i32::to_le_bytes).collect()
            }
            Self::I64 { input, output, .. } => {
                let _keepalive = input;
                output.into_iter().flat_map(i64::to_le_bytes).collect()
            }
            Self::F32 { input, output, .. } => {
                let _keepalive = input;
                output.into_iter().flat_map(f32::to_le_bytes).collect()
            }
            Self::F64 { input, output, .. } => {
                let _keepalive = input;
                output.into_iter().flat_map(f64::to_le_bytes).collect()
            }
        }
    }
}

#[cfg(feature = "melior")]
fn bytes_to_i32(bytes: &[u8]) -> Result<Vec<i32>, NativeBackendDiagnostic> {
    chunks_to_values(bytes, i32::from_le_bytes, "Int32")
}

#[cfg(feature = "melior")]
fn bytes_to_i64(bytes: &[u8]) -> Result<Vec<i64>, NativeBackendDiagnostic> {
    chunks_to_values(bytes, i64::from_le_bytes, "Int64")
}

#[cfg(feature = "melior")]
fn bytes_to_f32(bytes: &[u8]) -> Result<Vec<f32>, NativeBackendDiagnostic> {
    chunks_to_values(bytes, f32::from_le_bytes, "Float32")
}

#[cfg(feature = "melior")]
fn bytes_to_f64(bytes: &[u8]) -> Result<Vec<f64>, NativeBackendDiagnostic> {
    chunks_to_values(bytes, f64::from_le_bytes, "Float64")
}

#[cfg(feature = "melior")]
fn chunks_to_values<const N: usize, T>(
    bytes: &[u8],
    convert: impl Fn([u8; N]) -> T,
    kind: &str,
) -> Result<Vec<T>, NativeBackendDiagnostic> {
    if bytes.len() % N != 0 {
        return Err(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::InvalidBackendArtifact,
            "$.jit.input_value_buffers",
            format!(
                "{kind} input buffer length {} is not {N}-byte aligned",
                bytes.len()
            ),
        ));
    }
    Ok(bytes
        .chunks_exact(N)
        .map(|chunk| {
            let mut array = [0u8; N];
            array.copy_from_slice(chunk);
            convert(array)
        })
        .collect())
}

fn report_with_diagnostic(
    source: &NativeBackendReport,
    status: NativeBackendStatus,
    diagnostic: NativeBackendDiagnostic,
) -> NativeBackendReport {
    let mut diagnostics = source.diagnostics.clone();
    diagnostics.push(diagnostic);
    NativeBackendReport {
        status,
        diagnostics,
        runtime_plan: source.runtime_plan.clone(),
        runtime_cache_key: source.runtime_cache_key.clone(),
        backend_identity: source.backend_identity.clone(),
        artifact: None,
    }
}

pub fn execute_copy_i32_jit(
    program: &L2CoreProgram,
    report: &FullVerificationReport,
    input: &[i32],
) -> Result<Vec<i32>, MeliorBackendReport> {
    let artifact = build_melior_module(program, report)?;
    let reference =
        execute_supported_copy_i32(program, report, input).map_err(map_lowering_report)?;

    let validation = validate_translation_to_llvm_ir(
        &artifact,
        MlirValidationOptions {
            require_compatible_toolchain: true,
        },
    );
    if !validation.is_ok() || !validation.supported {
        return Err(jit_unavailable(
            &artifact,
            Some(validation),
            "compatible MLIR/LLVM ExecutionEngine toolchain is unavailable",
        ));
    }

    execute_copy_i32_with_backend(&artifact, &reference)
}

pub fn compare_native_output(reference: &[i32], native: &[i32]) -> Result<(), MeliorBackendReport> {
    if reference == native {
        return Ok(());
    }

    Err(MeliorBackendReport::diagnostic(
        MeliorBackendDiagnosticCode::NativeOutputMismatch,
        "$.jit.output",
        format!(
            "native output mismatch: expected {:?}, got {:?}",
            reference, native
        ),
    ))
}

pub fn jit_symbol_missing(symbol: &str) -> MeliorBackendReport {
    MeliorBackendReport::diagnostic(
        MeliorBackendDiagnosticCode::JitSymbolMissing,
        "$.jit.symbol",
        format!("JIT entry symbol '{symbol}' was not found"),
    )
}

fn execute_copy_i32_with_backend(
    artifact: &MeliorModuleArtifact,
    reference: &[i32],
) -> Result<Vec<i32>, MeliorBackendReport> {
    let _ = reference;
    Err(jit_unavailable(
        artifact,
        None,
        "melior ExecutionEngine invocation is not available in this build",
    ))
}

fn jit_unavailable(
    artifact: &MeliorModuleArtifact,
    validation: Option<MeliorBackendReport>,
    message: impl Into<String>,
) -> MeliorBackendReport {
    let mut report = MeliorBackendReport {
        entry_symbol: Some(ENTRY_SYMBOL.to_string()),
        row_count: Some(artifact.row_count),
        artifact_summary: Some(artifact.artifact_summary.clone()),
        ..MeliorBackendReport::default()
    };
    if let Some(validation) = validation {
        report.toolchain = validation.toolchain;
        for diagnostic in validation.diagnostics {
            report.diagnostics.push(diagnostic);
        }
    }
    report.push(
        MeliorBackendDiagnosticCode::JitUnavailable,
        "$.jit",
        message,
    );
    report
}

fn map_lowering_report(report: LoweringSupportReport) -> MeliorBackendReport {
    let mut backend = MeliorBackendReport::default();
    for diagnostic in report.diagnostics() {
        backend.push(
            map_lowering_code(diagnostic.code),
            diagnostic.path.clone(),
            diagnostic.message.clone(),
        );
    }
    backend
}

fn map_lowering_code(code: LoweringDiagnosticCode) -> MeliorBackendDiagnosticCode {
    match code {
        LoweringDiagnosticCode::VerifierRejected => MeliorBackendDiagnosticCode::VerifierRejected,
        LoweringDiagnosticCode::MissingVerifierFacts => {
            MeliorBackendDiagnosticCode::MissingVerifierFacts
        }
        _ => MeliorBackendDiagnosticCode::UnsupportedLoweringShape,
    }
}

#[cfg(test)]
mod tests {
    use loom_core::full_verifier::{verify_l2_core, FullVerificationReport};
    use loom_core::l2_core::{
        Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, L2DataType,
        OutputBuilderCapability, ResourceBudget, ScalarExpr,
    };

    use super::*;

    fn sample_program() -> L2CoreProgram {
        L2CoreProgram {
            artifact_version: 1,
            required_features: vec!["l2core.copy.v0".to_string()],
            optional_features: vec![],
            capabilities: vec![
                Capability::InputSlice(InputSliceCapability {
                    id: "input0".to_string(),
                    offset: 0,
                    length: 16,
                }),
                Capability::OutputBuilder(OutputBuilderCapability {
                    id: "out0".to_string(),
                    arrow_type: L2DataType::Int32,
                    nullable: true,
                    max_events: 4,
                }),
            ],
            resource_budget: ResourceBudget::bounded_rows(4),
            body: vec![L2CoreStmt::ForRange {
                index: "i".to_string(),
                start: ScalarExpr::u64(0),
                end: ScalarExpr::u64(4),
                body: vec![
                    L2CoreStmt::ReadInput {
                        capability: "input0".to_string(),
                        offset: ScalarExpr::var("i"),
                        width: ScalarExpr::u64(4),
                        bind: "value".to_string(),
                    },
                    L2CoreStmt::AppendValue {
                        builder: "out0".to_string(),
                        value: ScalarExpr::var("value"),
                    },
                ],
            }],
        }
    }

    #[test]
    fn jit_reports_unavailable_after_supported_preconditions() {
        let program = sample_program();
        let report = verify_l2_core(&program);
        let err = execute_copy_i32_jit(&program, &report, &[10, 20, 30, 40, 50])
            .expect_err("local JIT should be unavailable without compatible toolchain");
        assert!(err
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == MeliorBackendDiagnosticCode::JitUnavailable }));
        assert_eq!(err.entry_symbol.as_deref(), Some("loom_l2core_copy_i32"));
    }

    #[test]
    fn jit_rejects_missing_facts_before_toolchain() {
        let program = sample_program();
        let err = execute_copy_i32_jit(
            &program,
            &FullVerificationReport::default(),
            &[10, 20, 30, 40],
        )
        .expect_err("missing facts must reject before JIT");
        assert_eq!(
            err.diagnostics[0].code,
            MeliorBackendDiagnosticCode::MissingVerifierFacts
        );
    }

    #[test]
    fn jit_rejects_unsupported_program_before_toolchain() {
        let mut program = sample_program();
        program.optional_features.push("debug.extra".to_string());
        let report = verify_l2_core(&program);
        let err = execute_copy_i32_jit(&program, &report, &[10, 20, 30, 40])
            .expect_err("unsupported shape must reject before JIT");
        assert!(err.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == MeliorBackendDiagnosticCode::UnsupportedLoweringShape
        }));
    }

    #[test]
    fn jit_rejects_short_input_before_backend() {
        let program = sample_program();
        let report = verify_l2_core(&program);
        let err = execute_copy_i32_jit(&program, &report, &[10, 20, 30])
            .expect_err("short input must reject before JIT");
        assert!(err.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == MeliorBackendDiagnosticCode::UnsupportedLoweringShape
        }));
    }

    #[test]
    fn native_output_mismatch_has_stable_code() {
        let err = compare_native_output(&[10, 20, 30, 40], &[10, 20, 30, 41])
            .expect_err("mismatch should reject");
        assert_eq!(
            err.diagnostics[0].code,
            MeliorBackendDiagnosticCode::NativeOutputMismatch
        );
        assert_eq!(err.diagnostics[0].code.as_str(), "native-output-mismatch");
    }

    #[test]
    fn jit_symbol_missing_has_stable_code() {
        let err = jit_symbol_missing("missing_symbol");
        assert_eq!(
            err.diagnostics[0].code,
            MeliorBackendDiagnosticCode::JitSymbolMissing
        );
        assert_eq!(err.diagnostics[0].code.as_str(), "jit-symbol-missing");
    }

    // -----------------------------------------------------------------------
    // Persistent disable-store tests (Phase 48 P3)
    // -----------------------------------------------------------------------

    #[test]
    fn disable_store_round_trip() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path();
        let store = DisableStore {
            version: 1,
            disabled_shapes: vec!["fp-a".to_string(), "fp-b".to_string()],
            last_updated_secs: Some(42),
        };
        store.save(path).unwrap();
        let loaded = DisableStore::load_or_default(path);
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.disabled_shapes.len(), 2);
        assert!(loaded.disabled_shapes.contains(&"fp-a".to_string()));
        assert!(loaded.disabled_shapes.contains(&"fp-b".to_string()));
        assert_eq!(loaded.last_updated_secs, Some(42));
    }

    #[test]
    fn disable_store_missing_file_returns_default() {
        let path = std::path::Path::new("/nonexistent/path/disabled-shapes.json");
        let store = DisableStore::load_or_default(path);
        assert_eq!(store.version, 0);
        assert!(store.disabled_shapes.is_empty());
    }

    #[test]
    fn disable_store_path_env_override() {
        let prev = std::env::var("LOOM_DISABLE_STORE_PATH").ok();
        std::env::set_var("LOOM_DISABLE_STORE_PATH", "/tmp/custom.json");
        assert_eq!(disable_store_path(), std::path::PathBuf::from("/tmp/custom.json"));
        match prev {
            Some(v) => std::env::set_var("LOOM_DISABLE_STORE_PATH", v),
            None => std::env::remove_var("LOOM_DISABLE_STORE_PATH"),
        }
    }

    #[test]
    fn disable_store_path_xdg_cache() {
        let prev_loom = std::env::var("LOOM_DISABLE_STORE_PATH").ok();
        let prev_xdg = std::env::var("XDG_CACHE_HOME").ok();
        std::env::remove_var("LOOM_DISABLE_STORE_PATH");
        std::env::set_var("XDG_CACHE_HOME", "/xdg_cache");
        assert_eq!(
            disable_store_path(),
            std::path::PathBuf::from("/xdg_cache/loom/disabled-shapes.json")
        );
        match prev_loom {
            Some(v) => std::env::set_var("LOOM_DISABLE_STORE_PATH", v),
            None => std::env::remove_var("LOOM_DISABLE_STORE_PATH"),
        }
        match prev_xdg {
            Some(v) => std::env::set_var("XDG_CACHE_HOME", v),
            None => std::env::remove_var("XDG_CACHE_HOME"),
        }
    }

    #[test]
    fn registry_disable_and_check() {
        reset_disabled_shapes();
        assert!(!is_shape_disabled("fp-test-1"));
        disable_shape("fp-test-1");
        assert!(is_shape_disabled("fp-test-1"));
        assert!(!is_shape_disabled("fp-test-2"));
        reset_disabled_shapes();
        assert!(!is_shape_disabled("fp-test-1"));
    }
}
