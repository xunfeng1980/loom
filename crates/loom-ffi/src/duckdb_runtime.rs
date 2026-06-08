//! Internal DuckDB runtime bridge.
//!
//! This module keeps DuckDB as an adapter over the Phase 22 runtime ABI and
//! Phase 23 backend vocabulary. It is safe Rust only; later C ABI wrappers can
//! translate these owned reports into DuckDB-facing handles without duplicating
//! runtime policy in C++.

use std::collections::HashMap;
use std::ffi::{c_char, CString};
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Mutex, OnceLock};

use arrow::datatypes::DataType;
use loom_core::artifact_verifier::{
    verify_artifact, ArtifactVerificationFacts, ArtifactVerificationOptions,
    ArtifactVerificationReport, ArtifactVerificationStatus, ConstraintDischargeStatus,
};
use loom_core::container_codec::{
    decode_layout_payload_maybe_container, decode_table_payload_maybe_container,
};
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::l2_core::{OutputSchemaFact, ResourceBudget, VerifiedArtifactFacts};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::production_native_lowering::{
    check_production_lowering_support, ProductionColumnShape, ProductionLoweringFacts,
    ProductionLoweringShape,
};
use loom_core::runtime_abi::{
    decide_runtime_execution, plan_projection, ConcurrencyPolicy, PredicateEnvelope,
    ProjectionColumn, ProjectionSet, RuntimeAbiVersion, RuntimeBackendIdentity,
    RuntimeCacheCompatibilityStatus, RuntimeCacheKey, RuntimeCacheKeyInput,
    RuntimeEmissionDisposition, RuntimeExecutionDecision, RuntimeFallbackPolicy,
    RuntimeLoweringDisposition, RuntimePlan, RuntimeReaderSupport, RuntimeSafetyPolicy,
    SplitDescriptor, UnsupportedPredicatePolicy,
};
use loom_native_melior::backend::{
    validate_backend_request, NativeBackendCancellation, NativeBackendIdentity,
    NativeBackendReport, NativeBackendRequestInput, NativeBackendStatus, NATIVE_BACKEND_NAME,
};
use loom_native_melior::jit::{
    compare_production_jit_output, execute_prepared_production_jit, ProductionJitOptions,
    ProductionJitOutput, PRODUCTION_JIT_ENTRY_SYMBOL,
};
use loom_native_melior::pipeline::{
    validate_and_prepare_production_backend, ProductionBackendPipelineOptions,
};

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
    pub test_jit_value_buffers: Option<Vec<Vec<u8>>>,
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
    pub artifact_value_buffers: Option<Vec<Vec<u8>>>,
    pub test_jit_value_buffers: Option<Vec<Vec<u8>>>,
    pub diagnostics: Vec<DuckDbRuntimeDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DuckDbPreparedRoute {
    pub decision: DuckDbRouteDecision,
    pub backend_report: Option<NativeBackendReport>,
    pub native_buffers: Vec<DuckDbNativeBuffer>,
    pub diagnostics: Vec<DuckDbRuntimeDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DuckDbNativeBuffer {
    pub builder_id: String,
    pub arrow_type: DataType,
    pub value_buffer: Vec<u8>,
}

#[derive(Debug, Clone)]
struct NativePreparationCacheEntry {
    cache_key: RuntimeCacheKey,
    backend_report: NativeBackendReport,
}

static NATIVE_PREPARATION_CACHE: OnceLock<Mutex<HashMap<String, NativePreparationCacheEntry>>> =
    OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
enum LoomDuckDbStatus {
    NullPointer = 1,
    Panicked = 2,
    OutOfRange = 3,
}

impl LoomDuckDbStatus {
    fn code(self) -> i32 {
        self as i32
    }
}

#[repr(C)]
pub struct LoomDuckDbPlan {
    report: DuckDbRuntimePlanReport,
    decision: CString,
    cache_key: CString,
    cache_input: CString,
    diagnostics: Vec<OwnedDuckDbDiagnostic>,
}

#[repr(C)]
pub struct LoomDuckDbPrepared {
    route: DuckDbPreparedRoute,
    status: CString,
    decision: CString,
    diagnostics: Vec<OwnedDuckDbDiagnostic>,
    native_buffers: Vec<OwnedDuckDbNativeBuffer>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LoomDuckDbDiagnostic {
    pub code: *const c_char,
    pub path: *const c_char,
    pub message: *const c_char,
}

impl Default for LoomDuckDbDiagnostic {
    fn default() -> Self {
        Self {
            code: std::ptr::null(),
            path: std::ptr::null(),
            message: std::ptr::null(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LoomDuckDbNativeBuffer {
    pub builder_id: *const c_char,
    pub arrow_type: *const c_char,
    pub value_ptr: *const u8,
    pub value_len: usize,
}

impl Default for LoomDuckDbNativeBuffer {
    fn default() -> Self {
        Self {
            builder_id: std::ptr::null(),
            arrow_type: std::ptr::null(),
            value_ptr: std::ptr::null(),
            value_len: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct OwnedDuckDbDiagnostic {
    code: CString,
    path: CString,
    message: CString,
}

impl OwnedDuckDbDiagnostic {
    fn from_diagnostic(diagnostic: &DuckDbRuntimeDiagnostic) -> Self {
        Self {
            code: cstring_lossy(&diagnostic.code),
            path: cstring_lossy(&diagnostic.path),
            message: cstring_lossy(&diagnostic.message),
        }
    }

    fn as_ffi(&self) -> LoomDuckDbDiagnostic {
        LoomDuckDbDiagnostic {
            code: self.code.as_ptr(),
            path: self.path.as_ptr(),
            message: self.message.as_ptr(),
        }
    }
}

#[derive(Debug, Clone)]
struct OwnedDuckDbNativeBuffer {
    builder_id: CString,
    arrow_type: CString,
    value_buffer: Vec<u8>,
}

impl OwnedDuckDbNativeBuffer {
    fn from_buffer(buffer: &DuckDbNativeBuffer) -> Self {
        Self {
            builder_id: cstring_lossy(&buffer.builder_id),
            arrow_type: cstring_lossy(&format!("{:?}", buffer.arrow_type)),
            value_buffer: buffer.value_buffer.clone(),
        }
    }

    fn as_ffi(&self) -> LoomDuckDbNativeBuffer {
        LoomDuckDbNativeBuffer {
            builder_id: self.builder_id.as_ptr(),
            arrow_type: self.arrow_type.as_ptr(),
            value_ptr: self.value_buffer.as_ptr(),
            value_len: self.value_buffer.len(),
        }
    }
}

/// Create an internal DuckDB runtime plan handle.
///
/// This is intentionally DuckDB-adapter internal and is excluded from the
/// generated public `loom.h`.
#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_plan_create(
    artifact_ptr: *const u8,
    artifact_len: usize,
    allow_interpreter_fallback: bool,
    use_test_native_facts: bool,
    out_plan: *mut *mut LoomDuckDbPlan,
) -> i32 {
    if out_plan.is_null() || (artifact_len > 0 && artifact_ptr.is_null()) {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let artifact = if artifact_len == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(artifact_ptr, artifact_len)
        };
        let report = create_duckdb_plan_report(
            artifact,
            DuckDbProjection::All,
            allow_interpreter_fallback,
            use_test_native_facts,
        );
        let handle = Box::new(LoomDuckDbPlan::from_report(report));
        std::ptr::write(out_plan, Box::into_raw(handle));
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

/// Create an internal DuckDB runtime plan handle for a projected scan.
///
/// The projected column ids are source-column indexes in DuckDB output order.
/// This is intentionally adapter-internal and excluded from generated public
/// `loom.h`; the public SQL surface remains `loom_scan(path)`.
#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_plan_create_projected(
    artifact_ptr: *const u8,
    artifact_len: usize,
    projection_ptr: *const u32,
    projection_len: usize,
    allow_interpreter_fallback: bool,
    use_test_native_facts: bool,
    out_plan: *mut *mut LoomDuckDbPlan,
) -> i32 {
    if out_plan.is_null()
        || (artifact_len > 0 && artifact_ptr.is_null())
        || (projection_len > 0 && projection_ptr.is_null())
    {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let artifact = if artifact_len == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(artifact_ptr, artifact_len)
        };
        let projection = if projection_len == 0 {
            DuckDbProjection::Columns(Vec::new())
        } else {
            DuckDbProjection::Columns(
                std::slice::from_raw_parts(projection_ptr, projection_len).to_vec(),
            )
        };
        let report = create_duckdb_plan_report(
            artifact,
            projection,
            allow_interpreter_fallback,
            use_test_native_facts,
        );
        let handle = Box::new(LoomDuckDbPlan::from_report(report));
        std::ptr::write(out_plan, Box::into_raw(handle));
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_plan_destroy(plan: *mut LoomDuckDbPlan) -> i32 {
    if plan.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        drop(Box::from_raw(plan));
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_plan_decision(
    plan: *const LoomDuckDbPlan,
    out_decision: *mut *const c_char,
) -> i32 {
    if plan.is_null() || out_decision.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        std::ptr::write(out_decision, (*plan).decision.as_ptr());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_plan_cache_key(
    plan: *const LoomDuckDbPlan,
    out_cache_key: *mut *const c_char,
) -> i32 {
    if plan.is_null() || out_cache_key.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        std::ptr::write(out_cache_key, (*plan).cache_key.as_ptr());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_plan_cache_input(
    plan: *const LoomDuckDbPlan,
    out_cache_input: *mut *const c_char,
) -> i32 {
    if plan.is_null() || out_cache_input.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        std::ptr::write(out_cache_input, (*plan).cache_input.as_ptr());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_plan_diagnostic_count(
    plan: *const LoomDuckDbPlan,
    out_count: *mut usize,
) -> i32 {
    if plan.is_null() || out_count.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        std::ptr::write(out_count, (*plan).diagnostics.len());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_plan_diagnostic(
    plan: *const LoomDuckDbPlan,
    index: usize,
    out_diagnostic: *mut LoomDuckDbDiagnostic,
) -> i32 {
    if plan.is_null() || out_diagnostic.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let Some(diagnostic) = (&(*plan).diagnostics).get(index) else {
            return LoomDuckDbStatus::OutOfRange.code();
        };
        std::ptr::write(out_diagnostic, diagnostic.as_ffi());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_prepare_create(
    plan: *const LoomDuckDbPlan,
    cancelled: bool,
    out_prepared: *mut *mut LoomDuckDbPrepared,
) -> i32 {
    if plan.is_null() || out_prepared.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let cancellation = if cancelled {
            NativeBackendCancellation::cancelled("duckdb adapter cancellation")
        } else {
            NativeBackendCancellation::default()
        };
        let route = prepare_duckdb_runtime(&(*plan).report, cancellation);
        let handle = Box::new(LoomDuckDbPrepared::from_route(route));
        std::ptr::write(out_prepared, Box::into_raw(handle));
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_prepare_destroy(prepared: *mut LoomDuckDbPrepared) -> i32 {
    if prepared.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        drop(Box::from_raw(prepared));
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_prepare_status(
    prepared: *const LoomDuckDbPrepared,
    out_status: *mut *const c_char,
) -> i32 {
    if prepared.is_null() || out_status.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        std::ptr::write(out_status, (*prepared).status.as_ptr());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_prepare_route(
    prepared: *const LoomDuckDbPrepared,
    out_route: *mut *const c_char,
) -> i32 {
    if prepared.is_null() || out_route.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        std::ptr::write(out_route, (*prepared).decision.as_ptr());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_prepare_diagnostic_count(
    prepared: *const LoomDuckDbPrepared,
    out_count: *mut usize,
) -> i32 {
    if prepared.is_null() || out_count.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        std::ptr::write(out_count, (*prepared).diagnostics.len());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_prepare_diagnostic(
    prepared: *const LoomDuckDbPrepared,
    index: usize,
    out_diagnostic: *mut LoomDuckDbDiagnostic,
) -> i32 {
    if prepared.is_null() || out_diagnostic.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let Some(diagnostic) = (&(*prepared).diagnostics).get(index) else {
            return LoomDuckDbStatus::OutOfRange.code();
        };
        std::ptr::write(out_diagnostic, diagnostic.as_ffi());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_prepare_native_buffer_count(
    prepared: *const LoomDuckDbPrepared,
    out_count: *mut usize,
) -> i32 {
    if prepared.is_null() || out_count.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        std::ptr::write(out_count, (*prepared).native_buffers.len());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_prepare_native_buffer(
    prepared: *const LoomDuckDbPrepared,
    index: usize,
    out_buffer: *mut LoomDuckDbNativeBuffer,
) -> i32 {
    if prepared.is_null() || out_buffer.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let Some(buffer) = (&(*prepared).native_buffers).get(index) else {
            return LoomDuckDbStatus::OutOfRange.code();
        };
        std::ptr::write(out_buffer, buffer.as_ffi());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

impl LoomDuckDbPlan {
    fn from_report(report: DuckDbRuntimePlanReport) -> Self {
        let decision = cstring_lossy(report.decision.as_str());
        let cache_key = cstring_lossy(&report.cache_key.stable_id);
        let cache_input = cstring_lossy(&report.cache_key.canonical_input);
        let diagnostics = report
            .diagnostics
            .iter()
            .map(OwnedDuckDbDiagnostic::from_diagnostic)
            .collect();
        Self {
            report,
            decision,
            cache_key,
            cache_input,
            diagnostics,
        }
    }
}

impl LoomDuckDbPrepared {
    fn from_route(route: DuckDbPreparedRoute) -> Self {
        let status = route
            .backend_report
            .as_ref()
            .map(|report| report.status.as_str())
            .unwrap_or(route.decision.as_str());
        let diagnostics = route
            .diagnostics
            .iter()
            .map(OwnedDuckDbDiagnostic::from_diagnostic)
            .collect();
        let native_buffers = route
            .native_buffers
            .iter()
            .map(OwnedDuckDbNativeBuffer::from_buffer)
            .collect();
        Self {
            status: cstring_lossy(status),
            decision: cstring_lossy(route.decision.as_str()),
            diagnostics,
            native_buffers,
            route,
        }
    }
}

fn create_duckdb_plan_report(
    artifact: &[u8],
    projection: DuckDbProjection,
    allow_interpreter_fallback: bool,
    use_test_native_facts: bool,
) -> DuckDbRuntimePlanReport {
    plan_duckdb_runtime(DuckDbRuntimePlanInput {
        artifact_bytes: artifact.to_vec(),
        projection,
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback,
            test_native_facts: if use_test_native_facts {
                Some(test_native_facts_for_artifact(artifact))
            } else {
                None
            },
        },
    })
    .unwrap_or_else(|report| report)
}

fn test_native_facts_for_artifact(artifact: &[u8]) -> DuckDbTestNativeFacts {
    if let Ok(table) = decode_table_payload_maybe_container(artifact) {
        return DuckDbTestNativeFacts {
            row_count: table.row_count as u64,
            columns: table
                .columns
                .iter()
                .map(|column| column.layout.data_type.clone())
                .collect(),
            test_jit_value_buffers: None,
        };
    }

    let (row_count, data_type) = decode_layout_payload_maybe_container(artifact)
        .map(|desc| (desc.row_count as u64, desc.data_type))
        .unwrap_or((0, DataType::Int32));
    DuckDbTestNativeFacts {
        row_count,
        columns: vec![data_type],
        test_jit_value_buffers: None,
    }
}

fn cstring_lossy(value: &str) -> CString {
    CString::new(value.replace('\0', "\\0")).expect("interior NULs replaced")
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
                None,
                artifact_report,
                diagnostics,
                Vec::new(),
                &input.artifact_bytes,
                input
                    .policy
                    .test_native_facts
                    .and_then(|facts| facts.test_jit_value_buffers),
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
    let mut lowering_facts = lowering_support.facts().cloned();
    let mut artifact_value_buffers = None;
    if let Some(facts) = lowering_facts.as_ref() {
        match artifact_raw_value_buffers(&input.artifact_bytes, facts) {
            Ok(buffers) => artifact_value_buffers = Some(buffers),
            Err(mut failed) => {
                diagnostics.append(&mut failed);
                lowering_facts = None;
            }
        }
    }
    let production_lowering_supported =
        lowering_facts.is_some() && artifact_value_buffers.is_some();

    let runtime_decision =
        decide_runtime_execution(&loom_core::runtime_abi::RuntimeDecisionInput {
            artifact_status: artifact_report.status(),
            constraint_status: constraint_status_for(&artifact_report),
            production_lowering_supported,
            reader_support: reader_support_for(&artifact_report),
            emission_disposition: emission_disposition_for(&artifact_report),
            lowering_disposition: lowering_disposition_for(production_lowering_supported),
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
        lowering_facts,
        artifact_value_buffers,
        artifact_report,
        diagnostics,
        projection_plan.output_to_source,
        &input.artifact_bytes,
        input
            .policy
            .test_native_facts
            .and_then(|facts| facts.test_jit_value_buffers),
    );

    Ok(report)
}

pub fn prepare_duckdb_runtime(
    plan_report: &DuckDbRuntimePlanReport,
    cancellation: NativeBackendCancellation,
) -> DuckDbPreparedRoute {
    let mut diagnostics = plan_report.diagnostics.clone();

    if plan_report.runtime_plan.decision != RuntimeExecutionDecision::NativeCandidate
        || !plan_report.runtime_plan.diagnostics.is_empty()
    {
        diagnostics.push(cache_non_cacheable_diagnostic(
            "runtime route is not an eligible native candidate",
        ));
        return DuckDbPreparedRoute {
            decision: plan_report.decision,
            backend_report: None,
            native_buffers: Vec::new(),
            diagnostics,
        };
    }

    let request_input = NativeBackendRequestInput {
        runtime_plan: plan_report.runtime_plan.clone(),
        runtime_cache_key: Some(plan_report.cache_key.clone()),
        lowering_facts: plan_report.lowering_facts.clone(),
        backend_identity: NativeBackendIdentity::preflight_only(),
        cancellation: cancellation.clone(),
    };
    let mut cache_hit = false;
    let mut backend_report = if cancellation.cancelled {
        validate_and_prepare_production_backend(
            request_input.clone(),
            ProductionBackendPipelineOptions::default(),
        )
    } else if let Some(cached) =
        lookup_native_preparation_cache(&plan_report.cache_key, &mut diagnostics)
    {
        cache_hit = true;
        cached
    } else if plan_report.artifact_value_buffers.is_some() {
        accepted_execution_engine_backend_report(request_input.clone(), &mut diagnostics)
    } else {
        validate_and_prepare_production_backend(
            request_input.clone(),
            ProductionBackendPipelineOptions::default(),
        )
    };

    if !cancellation.cancelled && !cache_hit {
        if let Some(test_buffers) = plan_report.test_jit_value_buffers.as_ref() {
            backend_report =
                accepted_execution_engine_backend_report(request_input.clone(), &mut diagnostics);
            diagnostics.push(DuckDbRuntimeDiagnostic {
                code: "test-jit-output".to_string(),
                path: "$.policy.test_native_facts.test_jit_value_buffers".to_string(),
                message: format!(
                    "test-only JIT value buffers supplied for {} column(s)",
                    test_buffers.len()
                ),
            });
        }
    }
    diagnostics.extend(backend_diagnostics(&backend_report));

    if backend_report.status == NativeBackendStatus::Cancelled {
        diagnostics.push(cache_non_cacheable_diagnostic(
            "cancelled native preparation is not cacheable",
        ));
        return DuckDbPreparedRoute {
            decision: DuckDbRouteDecision::Cancelled,
            backend_report: Some(backend_report),
            native_buffers: Vec::new(),
            diagnostics,
        };
    }

    if backend_report.status != NativeBackendStatus::Accepted
        || !backend_report.diagnostics.is_empty()
    {
        diagnostics.push(cache_non_cacheable_diagnostic(
            "native backend did not produce an accepted diagnostic-free preparation",
        ));
        return DuckDbPreparedRoute {
            decision: decision_for_backend_status(backend_report.status, plan_report.policy),
            backend_report: Some(backend_report),
            native_buffers: Vec::new(),
            diagnostics,
        };
    }

    let Some(lowering_facts) = plan_report.lowering_facts.as_ref() else {
        diagnostics.push(DuckDbRuntimeDiagnostic {
            code: "missing-lowering-facts".to_string(),
            path: "$.lowering_facts".to_string(),
            message: "native prepare requires production lowering facts".to_string(),
        });
        diagnostics.push(cache_non_cacheable_diagnostic(
            "missing production lowering facts prevent cache insertion",
        ));
        return DuckDbPreparedRoute {
            decision: DuckDbRouteDecision::FailClosed,
            backend_report: Some(backend_report),
            native_buffers: Vec::new(),
            diagnostics,
        };
    };

    let expected_buffers = match plan_report.artifact_value_buffers.as_ref() {
        Some(buffers) => buffers.clone(),
        None => {
            diagnostics.push(DuckDbRuntimeDiagnostic {
                code: "missing-native-value-buffers".to_string(),
                path: "$.artifact_value_buffers".to_string(),
                message: "native prepare requires raw artifact value buffers".to_string(),
            });
            diagnostics.push(cache_non_cacheable_diagnostic(
                "missing raw artifact buffers prevent native comparison",
            ));
            return DuckDbPreparedRoute {
                decision: DuckDbRouteDecision::FailClosed,
                backend_report: Some(backend_report),
                native_buffers: Vec::new(),
                diagnostics,
            };
        }
    };

    let jit_output = if let Some(test_buffers) = plan_report.test_jit_value_buffers.clone() {
        ProductionJitOutput {
            entry_symbol: PRODUCTION_JIT_ENTRY_SYMBOL.to_string(),
            row_count: lowering_facts.shape.row_count(),
            column_count: lowering_facts.shape.columns().len(),
            value_buffers: test_buffers,
        }
    } else {
        match execute_prepared_production_jit(
            &backend_report,
            &cancellation,
            ProductionJitOptions {
                require_compatible_toolchain: true,
                input_value_buffers: expected_buffers.clone(),
            },
        ) {
            Ok(output) => {
                diagnostics.push(DuckDbRuntimeDiagnostic {
                    code: "native-execution-engine-output".to_string(),
                    path: "$.jit.output".to_string(),
                    message: format!(
                        "MLIR ExecutionEngine produced {} native value buffer(s)",
                        output.value_buffers.len()
                    ),
                });
                output
            }
            Err(report) => {
                diagnostics.extend(backend_diagnostics(&report));
                diagnostics.push(cache_non_cacheable_diagnostic(
                    "native JIT execution failed before comparison",
                ));
                return DuckDbPreparedRoute {
                    decision: decision_for_backend_status(report.status, plan_report.policy),
                    backend_report: Some(report),
                    native_buffers: Vec::new(),
                    diagnostics,
                };
            }
        }
    };

    if let Err(report) =
        compare_production_jit_output(&backend_report, &expected_buffers, &jit_output)
    {
        diagnostics.extend(backend_diagnostics(&report));
        diagnostics.push(cache_non_cacheable_diagnostic(
            "native output mismatch prevents cache insertion",
        ));
        return DuckDbPreparedRoute {
            decision: DuckDbRouteDecision::FailClosed,
            backend_report: Some(report),
            native_buffers: Vec::new(),
            diagnostics,
        };
    }

    let native_buffers = native_buffers_from_output(lowering_facts, jit_output);
    if !cache_hit {
        insert_native_preparation_cache(
            &plan_report.cache_key,
            &backend_report,
            &native_buffers,
            &mut diagnostics,
        );
    }

    DuckDbPreparedRoute {
        decision: DuckDbRouteDecision::NativeCandidate,
        backend_report: Some(backend_report),
        native_buffers,
        diagnostics,
    }
}

fn build_plan_report(
    decision: DuckDbRouteDecision,
    runtime_decision: RuntimeExecutionDecision,
    projection: ProjectionSet,
    predicate: PredicateEnvelope,
    split: SplitDescriptor,
    policy: RuntimeSafetyPolicy,
    lowering_facts: Option<ProductionLoweringFacts>,
    artifact_value_buffers: Option<Vec<Vec<u8>>>,
    artifact_report: ArtifactVerificationReport,
    diagnostics: Vec<DuckDbRuntimeDiagnostic>,
    output_to_source: Vec<u32>,
    artifact_bytes: &[u8],
    test_jit_value_buffers: Option<Vec<Vec<u8>>>,
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
        artifact_value_buffers,
        test_jit_value_buffers,
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

fn artifact_raw_value_buffers(
    artifact_bytes: &[u8],
    lowering_facts: &ProductionLoweringFacts,
) -> Result<Vec<Vec<u8>>, Vec<DuckDbRuntimeDiagnostic>> {
    match &lowering_facts.shape {
        ProductionLoweringShape::SingleColumnPrimitive { row_count, column } => {
            let layout = decode_layout_payload_maybe_container(artifact_bytes).map_err(|err| {
                vec![DuckDbRuntimeDiagnostic {
                    code: "unsupported-payload".to_string(),
                    path: "$.artifact".to_string(),
                    message: format!("native raw-copy expected LMP1 layout payload: {err}"),
                }]
            })?;
            raw_value_buffer_from_layout(&layout, column, *row_count, "$.layout")
                .map(|buffer| vec![buffer])
                .map_err(|diagnostic| vec![diagnostic])
        }
        ProductionLoweringShape::PrimitiveTable { row_count, columns } => {
            let table = decode_table_payload_maybe_container(artifact_bytes).map_err(|err| {
                vec![DuckDbRuntimeDiagnostic {
                    code: "unsupported-payload".to_string(),
                    path: "$.artifact".to_string(),
                    message: format!("native raw-copy expected LMT1 table payload: {err}"),
                }]
            })?;
            if table.row_count as u64 != *row_count {
                return Err(vec![DuckDbRuntimeDiagnostic {
                    code: "unsupported-shape".to_string(),
                    path: "$.table.row_count".to_string(),
                    message: format!(
                        "native raw-copy table row_count {} does not match lowering row_count {}",
                        table.row_count, row_count
                    ),
                }]);
            }
            if table.columns.len() != columns.len() {
                return Err(vec![DuckDbRuntimeDiagnostic {
                    code: "unsupported-shape".to_string(),
                    path: "$.table.columns".to_string(),
                    message: format!(
                        "native raw-copy table has {} column(s), expected {}",
                        table.columns.len(),
                        columns.len()
                    ),
                }]);
            }

            let mut buffers = Vec::with_capacity(columns.len());
            let mut diagnostics = Vec::new();
            for (idx, column) in columns.iter().enumerate() {
                let path = format!("$.table.columns[{idx}].layout");
                match raw_value_buffer_from_layout(
                    &table.columns[idx].layout,
                    column,
                    *row_count,
                    &path,
                ) {
                    Ok(buffer) => buffers.push(buffer),
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }
            if diagnostics.is_empty() {
                Ok(buffers)
            } else {
                Err(diagnostics)
            }
        }
    }
}

fn raw_value_buffer_from_layout(
    layout: &LayoutDescription,
    column: &ProductionColumnShape,
    row_count: u64,
    path: &str,
) -> Result<Vec<u8>, DuckDbRuntimeDiagnostic> {
    if layout.data_type != column.arrow_type {
        return Err(DuckDbRuntimeDiagnostic {
            code: "unsupported-type".to_string(),
            path: format!("{path}.data_type"),
            message: format!(
                "native raw-copy layout type {:?} does not match lowering type {:?}",
                layout.data_type, column.arrow_type
            ),
        });
    }
    if layout.row_count as u64 != row_count {
        return Err(DuckDbRuntimeDiagnostic {
            code: "unsupported-shape".to_string(),
            path: format!("{path}.row_count"),
            message: format!(
                "native raw-copy layout row_count {} does not match lowering row_count {}",
                layout.row_count, row_count
            ),
        });
    }
    let Some(width) = primitive_byte_width(&column.arrow_type) else {
        return Err(DuckDbRuntimeDiagnostic {
            code: "unsupported-type".to_string(),
            path: format!("{path}.data_type"),
            message: format!(
                "native raw-copy unsupported type {:?}; expected Int32, Int64, Float32, or Float64",
                column.arrow_type
            ),
        });
    };

    let LayoutNode::Raw {
        data,
        elem_size,
        count,
    } = &layout.root
    else {
        return Err(DuckDbRuntimeDiagnostic {
            code: "unsupported-kernel".to_string(),
            path: format!("{path}.root"),
            message: "native raw-copy only supports Raw primitive layouts".to_string(),
        });
    };

    if usize::from(*elem_size) != width {
        return Err(DuckDbRuntimeDiagnostic {
            code: "unsupported-shape".to_string(),
            path: format!("{path}.root.elem_size"),
            message: format!(
                "native raw-copy elem_size {} does not match {:?} byte width {}",
                elem_size, column.arrow_type, width
            ),
        });
    }
    if *count as u64 != row_count {
        return Err(DuckDbRuntimeDiagnostic {
            code: "unsupported-shape".to_string(),
            path: format!("{path}.root.count"),
            message: format!(
                "native raw-copy count {} does not match row_count {}",
                count, row_count
            ),
        });
    }
    let expected_len = width.saturating_mul(*count);
    if data.len() != expected_len {
        return Err(DuckDbRuntimeDiagnostic {
            code: "unsupported-shape".to_string(),
            path: format!("{path}.root.data"),
            message: format!(
                "native raw-copy data has {} bytes, expected exactly {}",
                data.len(),
                expected_len
            ),
        });
    }
    Ok(data.clone())
}

fn primitive_byte_width(data_type: &DataType) -> Option<usize> {
    match data_type {
        DataType::Int32 | DataType::Float32 => Some(4),
        DataType::Int64 | DataType::Float64 => Some(8),
        _ => None,
    }
}

fn native_buffers_from_output(
    lowering_facts: &ProductionLoweringFacts,
    output: ProductionJitOutput,
) -> Vec<DuckDbNativeBuffer> {
    lowering_facts
        .shape
        .columns()
        .iter()
        .zip(output.value_buffers)
        .map(|(column, value_buffer)| DuckDbNativeBuffer {
            builder_id: column.builder_id.clone(),
            arrow_type: column.arrow_type.clone(),
            value_buffer,
        })
        .collect()
}

fn accepted_execution_engine_backend_report(
    request_input: NativeBackendRequestInput,
    diagnostics: &mut Vec<DuckDbRuntimeDiagnostic>,
) -> NativeBackendReport {
    match validate_backend_request(request_input) {
        Ok(request) => {
            diagnostics.push(DuckDbRuntimeDiagnostic {
                code: "native-execution-engine-backend".to_string(),
                path: "$.backend.execution_engine".to_string(),
                message: "accepted verified MLIR ExecutionEngine backend request".to_string(),
            });
            NativeBackendReport::accepted_pipeline(
                &request,
                request.backend_identity.clone(),
                PRODUCTION_JIT_ENTRY_SYMBOL,
                request.lowering_facts.shape.row_count(),
                request.lowering_facts.shape.columns().len(),
                "phase=24;backend=melior-execution-engine;execution=mlir-raw-copy",
            )
        }
        Err(report) => report,
    }
}

fn decision_for_backend_status(
    status: NativeBackendStatus,
    policy: RuntimeSafetyPolicy,
) -> DuckDbRouteDecision {
    match status {
        NativeBackendStatus::Accepted => DuckDbRouteDecision::NativeCandidate,
        NativeBackendStatus::Cancelled => DuckDbRouteDecision::Cancelled,
        NativeBackendStatus::SkippedToolchain
            if policy.fallback == RuntimeFallbackPolicy::AllowInterpreter =>
        {
            DuckDbRouteDecision::InterpreterFallback
        }
        NativeBackendStatus::Rejected
        | NativeBackendStatus::SkippedToolchain
        | NativeBackendStatus::FailClosed => DuckDbRouteDecision::FailClosed,
    }
}

fn backend_diagnostics(report: &NativeBackendReport) -> Vec<DuckDbRuntimeDiagnostic> {
    report
        .diagnostics
        .iter()
        .map(|diagnostic| DuckDbRuntimeDiagnostic {
            code: diagnostic.code.as_str().to_string(),
            path: diagnostic.path.clone(),
            message: diagnostic.message.clone(),
        })
        .collect()
}

fn native_preparation_cache() -> &'static Mutex<HashMap<String, NativePreparationCacheEntry>> {
    NATIVE_PREPARATION_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lookup_native_preparation_cache(
    cache_key: &RuntimeCacheKey,
    diagnostics: &mut Vec<DuckDbRuntimeDiagnostic>,
) -> Option<NativeBackendReport> {
    let mut cache = native_preparation_cache()
        .lock()
        .expect("native preparation cache mutex poisoned");
    let Some(entry) = cache.get(&cache_key.stable_id) else {
        diagnostics.push(cache_diagnostic(
            "cache-miss",
            "no accepted in-process native preparation entry matched the runtime cache key",
        ));
        return None;
    };

    let compatibility = entry.cache_key.compatibility_with(cache_key);
    match compatibility.status {
        RuntimeCacheCompatibilityStatus::Hit => {
            diagnostics.push(cache_diagnostic(
                "cache-hit",
                "reused accepted in-process native preparation evidence",
            ));
            Some(entry.backend_report.clone())
        }
        RuntimeCacheCompatibilityStatus::Miss => {
            diagnostics.push(cache_diagnostic(
                "cache-miss",
                "cached stable id did not match the candidate runtime cache key",
            ));
            None
        }
        RuntimeCacheCompatibilityStatus::KeyMismatch => {
            diagnostics.extend(
                compatibility
                    .diagnostics
                    .into_iter()
                    .map(runtime_diagnostic),
            );
            cache.remove(&cache_key.stable_id);
            None
        }
    }
}

fn insert_native_preparation_cache(
    cache_key: &RuntimeCacheKey,
    backend_report: &NativeBackendReport,
    native_buffers: &[DuckDbNativeBuffer],
    diagnostics: &mut Vec<DuckDbRuntimeDiagnostic>,
) {
    if native_buffers.is_empty() {
        diagnostics.push(cache_non_cacheable_diagnostic(
            "accepted native preparation produced no buffers for the requested shape",
        ));
        return;
    }
    if backend_report.status != NativeBackendStatus::Accepted
        || !backend_report.diagnostics.is_empty()
        || backend_report.artifact.is_none()
    {
        diagnostics.push(cache_non_cacheable_diagnostic(
            "only accepted diagnostic-free backend artifacts may be cached",
        ));
        return;
    }

    let mut cache = native_preparation_cache()
        .lock()
        .expect("native preparation cache mutex poisoned");
    cache.insert(
        cache_key.stable_id.clone(),
        NativePreparationCacheEntry {
            cache_key: cache_key.clone(),
            backend_report: backend_report.clone(),
        },
    );
    diagnostics.push(cache_diagnostic(
        "cache-inserted",
        "stored accepted in-process native preparation evidence",
    ));
}

fn cache_diagnostic(code: &str, message: impl Into<String>) -> DuckDbRuntimeDiagnostic {
    DuckDbRuntimeDiagnostic {
        code: code.to_string(),
        path: "$.cache.native_preparation".to_string(),
        message: message.into(),
    }
}

fn cache_non_cacheable_diagnostic(message: impl Into<String>) -> DuckDbRuntimeDiagnostic {
    cache_diagnostic("cache-non-cacheable", message)
}

fn column_count_for(report: &ArtifactVerificationReport, input: &DuckDbRuntimePlanInput) -> u32 {
    if let Some(test_facts) = input.policy.test_native_facts.as_ref() {
        return test_facts.columns.len() as u32;
    }
    if let Ok(table) = decode_table_payload_maybe_container(&input.artifact_bytes) {
        return table.columns.len() as u32;
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

pub fn duckdb_runtime_clear_native_preparation_cache_for_test() {
    native_preparation_cache()
        .lock()
        .expect("native preparation cache mutex poisoned")
        .clear();
}

pub fn duckdb_runtime_corrupt_cached_canonical_input_for_test(
    stable_id: &str,
    canonical_input: impl Into<String>,
) -> bool {
    let mut cache = native_preparation_cache()
        .lock()
        .expect("native preparation cache mutex poisoned");
    let Some(entry) = cache.get_mut(stable_id) else {
        return false;
    };
    entry.cache_key.canonical_input = canonical_input.into();
    true
}
