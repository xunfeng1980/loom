//! Internal DuckDB runtime bridge.
//!
//! This module keeps DuckDB as an adapter over the Phase 22 runtime ABI and
//! Phase 23 backend vocabulary. It is safe Rust only; later C ABI wrappers can
//! translate these owned reports into DuckDB-facing handles without duplicating
//! runtime policy in C++.

use std::ffi::{c_char, CStr, CString};
use std::panic::{self, AssertUnwindSafe};

use arrow::array::{Array, RecordBatch};
use arrow::datatypes::DataType;
use arrow::ffi::{to_ffi, FFI_ArrowArray, FFI_ArrowSchema};
use loom_core::arrow_semantic_codec::{
    decode_arrow_semantic_container_payload, decode_arrow_semantic_payload,
    is_arrow_semantic_container, is_arrow_semantic_payload,
};
use loom_core::artifact_verifier::{
    verify_artifact, ArtifactVerificationOptions, ArtifactVerificationReport,
    ArtifactVerificationStatus, ConstraintDischargeStatus,
};
use loom_core::container_codec::decode_table_payload_maybe_container;
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::native_arrow_semantic::prepare_native_arrow_semantic_codegen_support;
use loom_core::runtime_abi::{
    decide_runtime_execution, plan_projection, ConcurrencyPolicy, PredicateEnvelope,
    ProjectionColumn, ProjectionSet, RuntimeAbiVersion, RuntimeBackendIdentity, RuntimeCacheKey,
    RuntimeCacheKeyInput, RuntimeEmissionDisposition, RuntimeExecutionDecision,
    RuntimeFallbackPolicy, RuntimeLoweringDisposition, RuntimePlan, RuntimeReaderSupport,
    RuntimeSafetyPolicy, SplitDescriptor, UnsupportedPredicatePolicy,
};
use loom_native_melior::backend::{NativeBackendCancellation, NativeBackendDiagnostic};
use loom_native_melior::jit::{
    execute_arrow_semantic_codegen_production_route, ArrowSemanticCodegenRouteStatus,
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
}

impl Default for DuckDbRuntimePolicy {
    fn default() -> Self {
        Self {
            allow_interpreter_fallback: true,
        }
    }
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
    pub artifact_bytes: Vec<u8>,
    pub production_fingerprint: String,
    pub diagnostics: Vec<DuckDbRuntimeDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DuckDbPreparedRoute {
    pub decision: DuckDbRouteDecision,
    pub native_buffers: Vec<DuckDbNativeBuffer>,
    pub diagnostics: Vec<DuckDbRuntimeDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DuckDbNativeBuffer {
    pub builder_id: String,
    pub arrow_type: DataType,
    pub row_count: u64,
    pub value_buffer: Vec<u8>,
    pub validity_buffer: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
enum LoomDuckDbStatus {
    NullPointer = 1,
    Panicked = 2,
    OutOfRange = 3,
    ArtifactUnsupported = 4,
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
pub struct LoomDuckDbArrowSemantic {
    batch: RecordBatch,
    column_names: Vec<CString>,
    column_formats: Vec<CString>,
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
    pub row_count: u64,
    pub value_ptr: *const u8,
    pub value_len: usize,
    pub validity_ptr: *const u8,
    pub validity_len: usize,
}

impl Default for LoomDuckDbNativeBuffer {
    fn default() -> Self {
        Self {
            builder_id: std::ptr::null(),
            arrow_type: std::ptr::null(),
            row_count: 0,
            value_ptr: std::ptr::null(),
            value_len: 0,
            validity_ptr: std::ptr::null(),
            validity_len: 0,
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
    row_count: u64,
    value_buffer: Vec<u8>,
    validity_buffer: Option<Vec<u8>>,
}

impl OwnedDuckDbNativeBuffer {
    fn from_buffer(buffer: &DuckDbNativeBuffer) -> Self {
        Self {
            builder_id: cstring_lossy(&buffer.builder_id),
            arrow_type: cstring_lossy(&format!("{:?}", buffer.arrow_type)),
            row_count: buffer.row_count,
            value_buffer: buffer.value_buffer.clone(),
            validity_buffer: buffer.validity_buffer.clone(),
        }
    }

    fn as_ffi(&self) -> LoomDuckDbNativeBuffer {
        let (validity_ptr, validity_len) = self
            .validity_buffer
            .as_ref()
            .map(|buffer| (buffer.as_ptr(), buffer.len()))
            .unwrap_or((std::ptr::null(), 0));
        LoomDuckDbNativeBuffer {
            builder_id: self.builder_id.as_ptr(),
            arrow_type: self.arrow_type.as_ptr(),
            row_count: self.row_count,
            value_ptr: self.value_buffer.as_ptr(),
            value_len: self.value_buffer.len(),
            validity_ptr,
            validity_len,
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
        let report =
            create_duckdb_plan_report(artifact, DuckDbProjection::All, allow_interpreter_fallback);
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
        let report = create_duckdb_plan_report(artifact, projection, allow_interpreter_fallback);
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

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_arrow_semantic_create(
    artifact_ptr: *const u8,
    artifact_len: usize,
    out_handle: *mut *mut LoomDuckDbArrowSemantic,
) -> i32 {
    if out_handle.is_null() || (artifact_len > 0 && artifact_ptr.is_null()) {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        std::ptr::write(out_handle, std::ptr::null_mut());
        let artifact = if artifact_len == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(artifact_ptr, artifact_len)
        };
        match LoomDuckDbArrowSemantic::from_artifact(artifact) {
            Ok(handle) => {
                std::ptr::write(out_handle, Box::into_raw(Box::new(handle)));
                0
            }
            Err(_) => LoomDuckDbStatus::ArtifactUnsupported.code(),
        }
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_arrow_semantic_destroy(
    handle: *mut LoomDuckDbArrowSemantic,
) -> i32 {
    if handle.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        drop(Box::from_raw(handle));
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_arrow_semantic_column_count(
    handle: *const LoomDuckDbArrowSemantic,
    out_count: *mut usize,
) -> i32 {
    if handle.is_null() || out_count.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        std::ptr::write(out_count, (*handle).batch.num_columns());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_arrow_semantic_row_count(
    handle: *const LoomDuckDbArrowSemantic,
    out_count: *mut usize,
) -> i32 {
    if handle.is_null() || out_count.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        std::ptr::write(out_count, (*handle).batch.num_rows());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_arrow_semantic_column_name(
    handle: *const LoomDuckDbArrowSemantic,
    index: usize,
    out_name: *mut *const c_char,
) -> i32 {
    if handle.is_null() || out_name.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let Some(name) = (&(*handle).column_names).get(index) else {
            return LoomDuckDbStatus::OutOfRange.code();
        };
        std::ptr::write(out_name, name.as_ptr());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_arrow_semantic_column_format(
    handle: *const LoomDuckDbArrowSemantic,
    index: usize,
    out_format: *mut *const c_char,
) -> i32 {
    if handle.is_null() || out_format.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let Some(format) = (&(*handle).column_formats).get(index) else {
            return LoomDuckDbStatus::OutOfRange.code();
        };
        std::ptr::write(out_format, format.as_ptr());
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => LoomDuckDbStatus::Panicked.code(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_duckdb_arrow_semantic_export_column(
    handle: *const LoomDuckDbArrowSemantic,
    index: usize,
    out_array: *mut FFI_ArrowArray,
    out_schema: *mut FFI_ArrowSchema,
) -> i32 {
    if handle.is_null() || out_array.is_null() || out_schema.is_null() {
        return LoomDuckDbStatus::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let Some(column) = (*handle).batch.columns().get(index) else {
            return LoomDuckDbStatus::OutOfRange.code();
        };
        let data = column.to_data();
        let Ok((ffi_array, ffi_schema)) = to_ffi(&data) else {
            return LoomDuckDbStatus::ArtifactUnsupported.code();
        };
        std::ptr::write(out_array, ffi_array);
        std::ptr::write(out_schema, ffi_schema);
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
        let status = route.decision.as_str();
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

impl LoomDuckDbArrowSemantic {
    fn from_artifact(artifact: &[u8]) -> Result<Self, String> {
        let registry = L2KernelRegistry::default_for_mvp0();
        let verifier_options = ArtifactVerificationOptions {
            require_l2_core_for_lowering: false,
            lowering_backend: Some("loom-decode-dialect".to_string()),
            compute_lowering_readiness: true,
        };
        let report = verify_artifact(artifact, &registry, &verifier_options);
        if !report.is_ok() {
            return Err(format!(
                "artifact verification status {}",
                report.status().as_str()
            ));
        }

        let payload = if is_arrow_semantic_container(artifact) {
            decode_arrow_semantic_container_payload(artifact).map_err(|err| format!("{err:?}"))?
        } else if is_arrow_semantic_payload(artifact) {
            decode_arrow_semantic_payload(artifact).map_err(|err| format!("{err:?}"))?
        } else {
            return Err("artifact is not LMC2(LMA1) or direct LMA1".to_string());
        };

        let mut batches = payload
            .to_record_batches()
            .map_err(|err| format!("{err:?}"))?;
        if batches.len() != 1 {
            return Err(format!(
                "DuckDB Arrow semantic scan requires exactly one record batch, got {}",
                batches.len()
            ));
        }
        let batch = batches.remove(0);

        let mut column_names = Vec::with_capacity(batch.num_columns());
        let mut column_formats = Vec::with_capacity(batch.num_columns());
        for (index, field) in batch.schema().fields().iter().enumerate() {
            column_names.push(cstring_lossy(field.name()));
            column_formats.push(arrow_c_format_for_column(batch.column(index).as_ref())?);
        }

        Ok(Self {
            batch,
            column_names,
            column_formats,
        })
    }
}

fn arrow_c_format_for_column(column: &dyn Array) -> Result<CString, String> {
    let data = column.to_data();
    let (_array, schema) = to_ffi(&data).map_err(|err| format!("{err:?}"))?;
    if schema.format.is_null() {
        return Err("Arrow schema has null format".to_string());
    }
    let format = unsafe { CStr::from_ptr(schema.format) }
        .to_string_lossy()
        .into_owned();
    Ok(cstring_lossy(&format))
}

fn create_duckdb_plan_report(
    artifact: &[u8],
    projection: DuckDbProjection,
    allow_interpreter_fallback: bool,
) -> DuckDbRuntimePlanReport {
    plan_duckdb_runtime(DuckDbRuntimePlanInput {
        artifact_bytes: artifact.to_vec(),
        projection,
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback,
        },
    })
    .unwrap_or_else(|report| report)
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
    let artifact_report = verify_artifact(&input.artifact_bytes, &registry, &verifier_options);
    let mut diagnostics = artifact_diagnostics(&artifact_report);

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
                artifact_report,
                diagnostics,
                Vec::new(),
                &input.artifact_bytes,
                "lowering:none".to_string(),
            );
            return Err(report);
        }
    };

    let predicate = PredicateEnvelope::None;
    let split = SplitDescriptor::FullScan {
        row_count: row_count_for(&artifact_report),
    };
    let policy = runtime_policy(&input.policy);
    let arrow_semantic_codegen_support = if is_arrow_semantic_container(&input.artifact_bytes)
        || is_arrow_semantic_payload(&input.artifact_bytes)
    {
        Some(prepare_native_arrow_semantic_codegen_support(
            &input.artifact_bytes,
        ))
    } else {
        None
    };
    let production_lowering_supported = arrow_semantic_codegen_support
        .as_ref()
        .map(|support| support.is_supported())
        .unwrap_or(false);
    if let Some(support) = arrow_semantic_codegen_support.as_ref() {
        if support.is_supported() {
            diagnostics.push(DuckDbRuntimeDiagnostic {
                code: "native-arrow-semantic-codegen-supported".to_string(),
                path: "$.native_arrow_semantic_codegen".to_string(),
                message: format!(
                    "DuckDB default route can use production Arrow semantic codegen for {} row(s), {} column(s)",
                    support.row_count, support.column_count
                ),
            });
        } else {
            diagnostics.extend(support.diagnostics().iter().map(|diagnostic| {
                DuckDbRuntimeDiagnostic {
                    code: diagnostic.code.as_str().to_string(),
                    path: diagnostic.path.clone(),
                    message: diagnostic.message.clone(),
                }
            }));
        }
    } else {
        diagnostics.push(DuckDbRuntimeDiagnostic {
            code: "lowering-unsupported".to_string(),
            path: "$.native_arrow_semantic_codegen".to_string(),
            message: "DuckDB native execution only supports LMC2(LMA1) or direct LMA1 Arrow semantic artifacts".to_string(),
        });
    }
    let production_fingerprint = arrow_semantic_codegen_support
        .as_ref()
        .filter(|support| support.is_supported())
        .map(|support| {
            format!(
                "backend=duckdb-arrow-semantic-codegen;rows={};columns={}",
                support.row_count, support.column_count
            )
        })
        .unwrap_or_else(|| "lowering:none".to_string());

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
        artifact_report,
        diagnostics,
        projection_plan.output_to_source,
        &input.artifact_bytes,
        production_fingerprint,
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
            native_buffers: Vec::new(),
            diagnostics,
        };
    }

    if !(is_arrow_semantic_container(&plan_report.artifact_bytes)
        || is_arrow_semantic_payload(&plan_report.artifact_bytes))
    {
        diagnostics.push(cache_non_cacheable_diagnostic(
            "DuckDB native route only accepts Arrow semantic artifacts",
        ));
        return DuckDbPreparedRoute {
            decision: fallback_or_fail_closed_decision(plan_report.policy),
            native_buffers: Vec::new(),
            diagnostics,
        };
    }

    let route = execute_arrow_semantic_codegen_production_route(
        &plan_report.artifact_bytes,
        &cancellation,
        plan_report.runtime_plan.projection.clone(),
        plan_report.runtime_plan.predicate.clone(),
        plan_report.runtime_plan.split.clone(),
        plan_report.policy,
    );
    diagnostics.extend(backend_route_diagnostics(&route.diagnostics));

    if route.status == ArrowSemanticCodegenRouteStatus::Cancelled {
        diagnostics.push(cache_non_cacheable_diagnostic(
            "cancelled native preparation is not cacheable",
        ));
        return DuckDbPreparedRoute {
            decision: DuckDbRouteDecision::Cancelled,
            native_buffers: Vec::new(),
            diagnostics,
        };
    }

    if route.status != ArrowSemanticCodegenRouteStatus::NativeCandidate || !route.cacheable {
        diagnostics.push(cache_non_cacheable_diagnostic(
            "Arrow semantic production route did not produce cacheable native output",
        ));
        return DuckDbPreparedRoute {
            decision: arrow_codegen_route_decision(route.status),
            native_buffers: Vec::new(),
            diagnostics,
        };
    };

    let Some(jit_output) = route.jit_output.as_ref() else {
        diagnostics.push(cache_non_cacheable_diagnostic(
            "Arrow semantic production route returned no JIT output",
        ));
        return DuckDbPreparedRoute {
            decision: DuckDbRouteDecision::FailClosed,
            native_buffers: Vec::new(),
            diagnostics,
        };
    };

    let native_buffers = arrow_semantic_native_buffers_from_route(&route, jit_output);
    diagnostics.push(DuckDbRuntimeDiagnostic {
        code: "native-arrow-semantic-codegen-output".to_string(),
        path: "$.jit.arrow_semantic.output".to_string(),
        message: format!(
            "production Arrow semantic codegen produced {} native column buffer(s)",
            native_buffers.len()
        ),
    });

    DuckDbPreparedRoute {
        decision: DuckDbRouteDecision::NativeCandidate,
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
    artifact_report: ArtifactVerificationReport,
    diagnostics: Vec<DuckDbRuntimeDiagnostic>,
    output_to_source: Vec<u32>,
    artifact_bytes: &[u8],
    production_fingerprint: String,
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
        production_lowering_fingerprint: production_fingerprint.clone(),
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
        artifact_bytes: artifact_bytes.to_vec(),
        production_fingerprint,
        diagnostics,
    }
}

fn arrow_codegen_route_decision(status: ArrowSemanticCodegenRouteStatus) -> DuckDbRouteDecision {
    match status {
        ArrowSemanticCodegenRouteStatus::NativeCandidate => DuckDbRouteDecision::NativeCandidate,
        ArrowSemanticCodegenRouteStatus::InterpreterFallback => {
            DuckDbRouteDecision::InterpreterFallback
        }
        ArrowSemanticCodegenRouteStatus::FailClosed => DuckDbRouteDecision::FailClosed,
        ArrowSemanticCodegenRouteStatus::Cancelled => DuckDbRouteDecision::Cancelled,
    }
}

fn fallback_or_fail_closed_decision(policy: RuntimeSafetyPolicy) -> DuckDbRouteDecision {
    if matches!(policy.fallback, RuntimeFallbackPolicy::AllowInterpreter) {
        DuckDbRouteDecision::InterpreterFallback
    } else {
        DuckDbRouteDecision::FailClosed
    }
}

fn backend_route_diagnostics(
    diagnostics: &[NativeBackendDiagnostic],
) -> Vec<DuckDbRuntimeDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| DuckDbRuntimeDiagnostic {
            code: diagnostic.code.as_str().to_string(),
            path: diagnostic.path.clone(),
            message: diagnostic.message.clone(),
        })
        .collect()
}

fn arrow_semantic_native_buffers_from_route(
    route: &loom_native_melior::jit::ArrowSemanticCodegenProductionRouteReport,
    jit_output: &loom_native_melior::jit::ArrowSemanticCodegenJitOutput,
) -> Vec<DuckDbNativeBuffer> {
    jit_output
        .columns
        .iter()
        .map(|output| {
            let expected = route
                .support
                .columns()
                .get(output.index)
                .expect("validated route output index is in support bounds");
            DuckDbNativeBuffer {
                builder_id: expected.name.clone(),
                arrow_type: expected.data_type.clone(),
                row_count: expected.row_count,
                value_buffer: output.value_buffer.clone(),
                validity_buffer: output.validity_buffer.clone(),
            }
        })
        .collect()
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

fn cache_diagnostic(code: &str, message: impl Into<String>) -> DuckDbRuntimeDiagnostic {
    DuckDbRuntimeDiagnostic {
        code: code.to_string(),
        path: "$.cache.arrow_semantic_production_route".to_string(),
        message: message.into(),
    }
}

fn cache_non_cacheable_diagnostic(message: impl Into<String>) -> DuckDbRuntimeDiagnostic {
    cache_diagnostic("cache-non-cacheable", message)
}

fn column_count_for(report: &ArtifactVerificationReport, input: &DuckDbRuntimePlanInput) -> u32 {
    if is_arrow_semantic_container(&input.artifact_bytes) {
        if let Ok(payload) = decode_arrow_semantic_container_payload(&input.artifact_bytes) {
            return payload.schema().fields().len() as u32;
        }
    }
    if is_arrow_semantic_payload(&input.artifact_bytes) {
        if let Ok(payload) = decode_arrow_semantic_payload(&input.artifact_bytes) {
            return payload.schema().fields().len() as u32;
        }
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
        Some("Arrow semantic payload") => RuntimeEmissionDisposition::SemanticArrow,
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
    RuntimeBackendIdentity {
        backend: "loom-duckdb-arrow-semantic-codegen".to_string(),
        backend_version: "phase43.2-production-route".to_string(),
        toolchain: "melior-mlir-llvm-required-at-prepare".to_string(),
        target_triple: "duckdb-host".to_string(),
        cpu_features: Vec::new(),
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

fn stable_fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
