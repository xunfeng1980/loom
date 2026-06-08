# Phase 24: DuckDB Native Execution Integration MVP - Pattern Map

**Mapped:** 2026-06-08
**Files analyzed:** 10
**Analogs found:** 10 / 10

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `duckdb-ext/loom_extension.cpp` | route/adapter | request-response, file-I/O | `duckdb-ext/loom_extension.cpp` | exact |
| `duckdb-ext/CMakeLists.txt` | config | build/link | `duckdb-ext/CMakeLists.txt` | exact |
| `crates/loom-ffi/src/duckdb_runtime.rs` | service/FFI bridge | request-response, transform | `crates/loom-ffi/src/ffi.rs` + `crates/loom-core/src/runtime_abi.rs` | role-match |
| `crates/loom-ffi/src/ffi.rs` | FFI service | request-response, file-I/O | `crates/loom-ffi/src/ffi.rs` | exact |
| `crates/loom-ffi/include/loom_duckdb_internal.h` | config/FFI header | request-response | `crates/loom-ffi/include/loom.h` + `crates/loom-ffi/include/loom_runtime.h` | role-match |
| `crates/loom-ffi/include/loom.h` | FFI header | request-response | `crates/loom-ffi/include/loom.h` | exact |
| `crates/loom-native-melior/src/backend.rs` | service/model | request-response, transform | `crates/loom-native-melior/src/backend.rs` | exact |
| `crates/loom-native-melior/src/jit.rs` | service | request-response, transform | `crates/loom-native-melior/src/jit.rs` | exact |
| `scripts/duckdb-native-integration-test.sh` | test | batch, file-I/O | `scripts/duckdb-smoke-test.sh` + `scripts/production-backend-test.sh` | role-match |
| `scripts/mvp0-verify.sh` | test/config | batch | `scripts/mvp0-verify.sh` | exact |

## Pattern Assignments

### `duckdb-ext/loom_extension.cpp` (route/adapter, request-response + file-I/O)

**Analog:** `duckdb-ext/loom_extension.cpp`

**Imports and FFI include pattern** (lines 30-40):
```cpp
#define DUCKDB_EXTENSION_MAIN
#include "vendor/duckdb-src/duckdb.hpp"  // DuckDB v1.5.3 amalgamated header

extern "C" {
#include "../crates/loom-ffi/include/loom.h"  // Phase 1: loom_decode signature
}

#include <cstdint>
#include <cstddef>
#include <fstream>
#include <limits>
```

**Bind data copy/equality pattern** (lines 55-79):
```cpp
struct LoomBindData : TableFunctionData {
    string payload_path;
    vector<uint8_t> payload;
    vector<string> column_names;
    vector<LogicalType> column_types;
    vector<LoomValueKind> column_kinds;
    vector<vector<uint8_t>> column_payloads;

    unique_ptr<FunctionData> Copy() const override {
        auto copy = make_uniq<LoomBindData>();
        copy->payload_path = payload_path;
        copy->payload = payload;
        copy->column_names = column_names;
        copy->column_types = column_types;
        copy->column_kinds = column_kinds;
        copy->column_payloads = column_payloads;
        return std::move(copy);
    }
};
```

**File read and error pattern** (lines 81-99):
```cpp
static vector<uint8_t> ReadPayloadFile(const string &path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        throw IOException("loom_scan: could not open payload file '%s'", path.c_str());
    }
    file.seekg(0, std::ios::end);
    auto size = file.tellg();
    if (size < 0) {
        throw IOException("loom_scan: could not determine payload size for '%s'", path.c_str());
    }
    file.seekg(0, std::ios::beg);
    vector<uint8_t> payload(static_cast<idx_t>(size));
```

**Container/table schema parsing pattern** (lines 279-313):
```cpp
static void PopulateColumnSpecs(LoomBindData &bind_data) {
    auto bind_payload = ExtractContainerPayload(bind_data.payload);
    if (!IsTablePayload(bind_payload)) {
        bind_data.column_names.push_back("value");
        bind_data.column_kinds.push_back(PayloadKindFromHeader(bind_payload));
        bind_data.column_types.push_back(LogicalTypeForKind(bind_data.column_kinds.back()));
        bind_data.column_payloads.push_back(IsContainerPayload(bind_data.payload) ? bind_data.payload : bind_payload);
        return;
    }
    // LMT1 table payload parsing appends name/type/kind/payload per column.
}
```

**RAII Arrow release pattern** (lines 349-370):
```cpp
struct LoomScanState : GlobalTableFunctionState {
    vector<ArrowArray> arrow_arrays;
    vector<ArrowSchema> arrow_schemas;
    vector<LoomValueKind> column_kinds;
    bool batch_emitted = false;

    ~LoomScanState() {
        for (auto &arrow_array : arrow_arrays) {
            if (arrow_array.release) {
                arrow_array.release(&arrow_array);
                arrow_array.release = nullptr;
            }
        }
        for (auto &arrow_schema : arrow_schemas) {
            if (arrow_schema.release) {
                arrow_schema.release(&arrow_schema);
                arrow_schema.release = nullptr;
            }
        }
    }
};
```

**Bind lifecycle pattern** (lines 377-400):
```cpp
static unique_ptr<FunctionData> LoomBind(
    ClientContext & /*ctx*/,
    TableFunctionBindInput &input,
    vector<LogicalType> &return_types,
    vector<string> &names)
{
    if (input.inputs.empty() || input.inputs[0].IsNull()) {
        throw BinderException("loom_scan requires a non-null payload file path");
    }

    auto bind_data = make_uniq<LoomBindData>();
    bind_data->payload_path = input.inputs[0].GetValue<string>();
    bind_data->payload = ReadPayloadFile(bind_data->payload_path);
    PopulateColumnSpecs(*bind_data);
```

**Init FFI return-code pattern** (lines 407-435):
```cpp
static unique_ptr<GlobalTableFunctionState> LoomInit(ClientContext &, TableFunctionInitInput &input) {
    auto state = make_uniq<LoomScanState>();
    auto &bind_data = input.bind_data->Cast<LoomBindData>();
    state->arrow_arrays.resize(bind_data.column_payloads.size());
    state->arrow_schemas.resize(bind_data.column_payloads.size());

    for (idx_t i = 0; i < bind_data.column_payloads.size(); i++) {
        int32_t rc = loom_decode(payload.data(), payload.size(),
            reinterpret_cast<FFI_ArrowArray *>(&state->arrow_arrays[i]),
            reinterpret_cast<FFI_ArrowSchema *>(&state->arrow_schemas[i]));
        if (rc != 0) {
            throw IOException("loom_decode failed for column %llu with code %d", ...);
        }
    }
    return state;
}
```

**Direct fixed-width fill pattern** (lines 465-482):
```cpp
template <class T>
static void FillFixedWidthVector(const ArrowArray &arr, Vector &vec, idx_t count, const char *kind) {
    RequireArrowBuffers(arr, 2, kind);
    auto *out_data = FlatVector::GetData<T>(vec);
    auto &validity = FlatVector::Validity(vec);
    const auto *values_buf = static_cast<const T *>(arr.buffers[1]);
    if (values_buf == nullptr) {
        throw IOException("loom_scan: decoded Arrow %s values buffer is null", kind);
    }

    for (idx_t i = 0; i < count; i++) {
        if (!ArrowValueIsValid(arr, i)) {
            validity.SetInvalid(i);
            continue;
        }
        out_data[i] = values_buf[i];
    }
}
```

**Single-batch scan pattern** (lines 526-588):
```cpp
static void LoomScan(ClientContext &, TableFunctionInput &data, DataChunk &output) {
    auto &state = data.global_state->Cast<LoomScanState>();
    if (state.batch_emitted) {
        output.SetCardinality(0);
        return;
    }
    output.SetCardinality(count);
    for (idx_t col = 0; col < state.arrow_arrays.size(); col++) {
        auto &vec = output.data[col];
        switch (state.column_kinds[col]) {
        case LoomValueKind::I32:
            FillFixedWidthVector<int32_t>(col_arr, vec, count, "Int32");
            break;
        }
    }
    state.batch_emitted = true;
}
```

**Registration pattern** (lines 595-603):
```cpp
static void LoadInternal(ExtensionLoader &loader) {
    TableFunction fn(
        "loom_scan",
        {LogicalType::VARCHAR},
        LoomScan,
        LoomBind,
        LoomInit);
    loader.RegisterFunction(fn);
}
```

Planner notes:
- Add `fn.projection_pushdown = true` next to this registration if projection mapping is implemented.
- Keep `MaxThreads() const override { return 1; }` on scan state if any future change could enable parallelism.
- Backend prepare belongs in global init; `LoomScan` remains the only `DataChunk` writer.

---

### `duckdb-ext/CMakeLists.txt` (config, build/link)

**Analog:** `duckdb-ext/CMakeLists.txt`

**Rust staticlib build trigger pattern** (lines 16-33):
```cmake
set(WORKSPACE_ROOT "${CMAKE_SOURCE_DIR}/..")
set(WORKSPACE_CARGO_TOML "${WORKSPACE_ROOT}/Cargo.toml")
set(LIBLOOM_FFI "${WORKSPACE_ROOT}/target/release/libloom_ffi.a")

add_custom_command(
    OUTPUT "${LIBLOOM_FFI}"
    COMMAND cargo build -p loom-ffi --release
            --manifest-path "${WORKSPACE_CARGO_TOML}"
    WORKING_DIRECTORY "${WORKSPACE_ROOT}"
    COMMENT "Building libloom_ffi.a (Rust staticlib, cargo build -p loom-ffi --release)"
)
add_custom_target(loom_ffi_build ALL
    DEPENDS "${LIBLOOM_FFI}"
)
```

**Extension target/include/link pattern** (lines 40-59):
```cmake
add_library(loom_loadable_extension SHARED
    "${CMAKE_SOURCE_DIR}/loom_extension.cpp"
)
add_dependencies(loom_loadable_extension loom_ffi_build)
target_include_directories(loom_loadable_extension PRIVATE
    "${CMAKE_SOURCE_DIR}/vendor/duckdb-src"
    "${WORKSPACE_ROOT}/crates/loom-ffi/include"
)
target_link_libraries(loom_loadable_extension PRIVATE
    "${LIBLOOM_FFI}"
)
```

**DuckDB symbol/footer pattern** (lines 76-99, 141-153):
```cmake
if(APPLE)
    target_link_options(loom_loadable_extension PRIVATE
        "-undefined" "dynamic_lookup"
        "-Wl,-exported_symbol,_loom_duckdb_cpp_init"
    )
endif()

set_target_properties(loom_loadable_extension PROPERTIES
    OUTPUT_NAME "loom"
    SUFFIX ".duckdb_extension"
    PREFIX ""
)

add_custom_command(
    TARGET loom_loadable_extension
    POST_BUILD
    COMMAND ${CMAKE_COMMAND}
        -DABI_TYPE=CPP
        -DEXTENSION=$<TARGET_FILE:loom_loadable_extension>
        -DVERSION_FIELD=v1.5.3
        -P ${CMAKE_SOURCE_DIR}/vendor/append_metadata.cmake
)
```

Planner notes:
- If adding an internal header, keep it under `crates/loom-ffi/include` so the existing include directory picks it up.
- Do not link `libduckdb`; DuckDB symbols resolve from the host CLI/process.

---

### `crates/loom-ffi/src/duckdb_runtime.rs` (service/FFI bridge, request-response + transform)

**Analogs:** `crates/loom-ffi/src/ffi.rs`, `crates/loom-core/src/runtime_abi.rs`, `crates/loom-native-melior/src/pipeline.rs`, `crates/loom-native-melior/src/jit.rs`

**FFI panic boundary pattern** (from `ffi.rs` lines 198-239):
```rust
#[no_mangle]
pub unsafe extern "C" fn loom_decode(...) -> i32 {
    if out_array.is_null() || out_schema.is_null() {
        return LoomError::NullPointer.code();
    }
    if input_len > 0 && input_ptr.is_null() {
        return LoomError::NullPointer.code();
    }

    let input: &[u8] = if input_len == 0 { &[] } else {
        std::slice::from_raw_parts(input_ptr, input_len)
    };

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        loom_decode_inner(input, out_array, out_schema)
    }));

    match result {
        Ok(Ok(())) => 0,
        Ok(Err(e)) => e.code(),
        Err(_panic_payload) => LoomError::Panicked.code(),
    }
}
```

**Runtime vocabulary to use directly** (from `runtime_abi.rs` lines 50-94):
```rust
pub enum RuntimeExecutionDecision {
    NativeCandidate,
    InterpreterFallback,
    FailClosed,
    DiagnosticOnly,
}

pub enum RuntimeFallbackPolicy {
    FailClosedOnly,
    AllowInterpreter,
    DiagnosticOnly,
}
```

**Projection and cache key pattern** (from `runtime_abi.rs` lines 145-170, 453-480):
```rust
pub struct ProjectionColumn {
    pub source_index: u32,
    pub output_index: u32,
}

pub enum ProjectionSet {
    All,
    Columns(Vec<ProjectionColumn>),
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
```

**Projection validation pattern** (from `runtime_abi.rs` lines 491-548):
```rust
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
            // Reject out-of-range and duplicate source/output mappings.
        }
    }
}
```

**Runtime decision pattern** (from `runtime_abi.rs` lines 619-752):
```rust
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
    // Native only when verifier/facts/projection/predicate/split/concurrency/lowering all pass.
}
```

**Backend prepare call pattern** (from `pipeline.rs` lines 43-51):
```rust
pub fn validate_and_prepare_production_backend(
    input: NativeBackendRequestInput,
    options: ProductionBackendPipelineOptions,
) -> NativeBackendReport {
    match validate_backend_request(input) {
        Ok(request) => prepare_production_backend_pipeline(&request, options),
        Err(report) => report,
    }
}
```

**JIT/cancellation/mismatch pattern** (from `jit.rs` lines 34-175):
```rust
pub fn execute_prepared_production_jit(
    report: &NativeBackendReport,
    cancellation: &NativeBackendCancellation,
    options: ProductionJitOptions,
) -> Result<ProductionJitOutput, NativeBackendReport> {
    if cancellation.cancelled {
        return Err(report_with_diagnostic(... NativeBackendStatus::Cancelled, ...));
    }
    if report.status != NativeBackendStatus::Accepted || !report.diagnostics.is_empty() {
        return Err(report_with_diagnostic(... NativeBackendStatus::FailClosed, ...));
    }
    // Toolchain skip is explicit via LOOM_ALLOW_NATIVE_TOOL_SKIP=1.
}

pub fn compare_production_jit_output(...) -> Result<(), NativeBackendReport> {
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
```

Planner notes:
- Keep this bridge internal/non-public if added. Do not freeze `loom_runtime.h`.
- New extern functions must follow `loom_decode`: null guards, slice reconstruction after guards, `catch_unwind`, integer status codes, no unwinding over C ABI.
- Do not duplicate native eligibility in C++; let Rust runtime/backend report decide.

---

### `crates/loom-ffi/src/ffi.rs` (FFI service, request-response + file-I/O)

**Analog:** `crates/loom-ffi/src/ffi.rs`

**Error enum wire-code pattern** (lines 35-56):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum LoomError {
    NullPointer = 1,
    DecodeFailed = 2,
    Panicked = 3,
}

impl LoomError {
    #[inline]
    pub fn code(self) -> i32 {
        self as i32
    }
}
```

**Safe inner function pattern** (lines 105-168):
```rust
fn loom_decode_inner(
    input: &[u8],
    out_array: *mut FFI_ArrowArray,
    out_schema: *mut FFI_ArrowSchema,
) -> Result<(), LoomError> {
    let desc =
        decode_layout_payload_maybe_container(input).map_err(|_| LoomError::DecodeFailed)?;
    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_layout(&desc, &registry);
    if !report.is_ok() {
        return Err(LoomError::DecodeFailed);
    }
    let array_data = decode_layout_to_array_data(&desc, &registry).map_err(|_| LoomError::DecodeFailed)?;
    let (ffi_array, ffi_schema) = to_ffi(&array_data).map_err(|_| LoomError::DecodeFailed)?;
    unsafe {
        std::ptr::write(out_array, ffi_array);
        std::ptr::write(out_schema, ffi_schema);
    }
    Ok(())
}
```

Planner notes:
- Any Phase 24 addition in `ffi.rs` should preserve this separation: public unsafe extern wrapper plus safe inner helper.
- Keep `loom-core` and `loom-ffi` Vortex-free.

---

### `crates/loom-ffi/include/loom_duckdb_internal.h` (config/FFI header, request-response)

**Analogs:** `crates/loom-ffi/include/loom.h`, `crates/loom-ffi/include/loom_runtime.h`

**Stable decode header shape** (from `loom.h` lines 1-14):
```c
/* Generated by cbindgen — do not edit by hand. */
/* Loom FFI surface — Phase 1, Plan 02 (CORE-03) */

#ifndef LOOM_H
#define LOOM_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct ArrowArray FFI_ArrowArray;
typedef struct ArrowSchema FFI_ArrowSchema;
```

**Function declaration style** (from `loom.h` lines 41-44):
```c
int32_t loom_decode(const uint8_t *input_ptr,
                    uintptr_t input_len,
                    FFI_ArrowArray *out_array,
                    FFI_ArrowSchema *out_schema);
```

**Unfrozen runtime warning pattern** (from `loom_runtime.h` lines 1-7):
```c
/* Phase 22 Loom runtime ABI sketch.
 *
 * This header is a contract sketch, not a frozen production ABI. It models the
 * host-neutral handles and callbacks that future runtime adapters should expose
 * after artifact verification and runtime planning.
 */
```

**Diagnostic struct shape** (from `loom_runtime.h` lines 39-43):
```c
typedef struct LoomRuntimeDiagnostic {
    const char *code;
    const char *path;
    const char *message;
} LoomRuntimeDiagnostic;
```

Planner notes:
- If an internal DuckDB header is added, name it as internal and non-public.
- Prefer fixed-size POD structs, `const char *` diagnostics, and caller-owned output slots.
- Avoid documenting any new internal function as a stable public runtime ABI.

---

### `crates/loom-ffi/include/loom.h` (FFI header, request-response)

**Analog:** `crates/loom-ffi/include/loom.h`

**Header guard and Arrow forward declarations** (lines 4-14):
```c
#ifndef LOOM_H
#define LOOM_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct ArrowArray FFI_ArrowArray;
typedef struct ArrowSchema FFI_ArrowSchema;
```

**Safety documentation pattern** (lines 31-39):
```c
 * # Safety
 *
 * The caller must ensure:
 * - `out_array` and `out_schema` point to caller-allocated, properly aligned,
 *   writeable memory for their respective types.
 * - `input_ptr` is either null (with `input_len == 0`) or points to at least
 *   `input_len` valid bytes.
 * - The written `FFI_ArrowArray` and `FFI_ArrowSchema` are eventually released
 *   by calling their respective `release` callbacks exactly once.
```

Planner notes:
- If `loom.h` changes through cbindgen, keep docs explicit about caller ownership and Arrow release.
- Prefer adding internal DuckDB-only declarations to a separate internal header.

---

### `crates/loom-native-melior/src/backend.rs` (service/model, request-response + transform)

**Analog:** `crates/loom-native-melior/src/backend.rs`

**Imports and runtime dependency pattern** (lines 9-18):
```rust
use loom_core::production_native_lowering::{
    ProductionLoweringBackend, ProductionLoweringFacts, ProductionNativeKernel,
};
use loom_core::runtime_abi::{
    RuntimeAbiVersion, RuntimeCacheKey, RuntimeExecutionDecision, RuntimePlan,
};
```

**Backend identity pattern** (lines 62-138):
```rust
pub struct NativeBackendIdentity {
    pub runtime_abi_version: RuntimeAbiVersion,
    pub backend: String,
    pub backend_version: String,
    pub expected_mlir_major: u32,
    pub detected_mlir_major: Option<u32>,
    pub llvm_config_version: Option<String>,
    pub toolchain_compatible: bool,
    pub pipeline_id: String,
    pub capabilities: NativeBackendCapabilities,
}

impl NativeBackendIdentity {
    pub fn preflight_only() -> Self { ... }
    pub fn as_key(&self) -> String { ... }
}
```

**Cancellation pattern** (lines 140-153):
```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeBackendCancellation {
    pub cancelled: bool,
    pub reason: Option<String>,
}

impl NativeBackendCancellation {
    pub fn cancelled(reason: impl Into<String>) -> Self {
        Self { cancelled: true, reason: Some(reason.into()) }
    }
}
```

**Stable backend diagnostic codes** (lines 200-235):
```rust
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
}
```

**Request validation pattern** (lines 376-466):
```rust
pub fn validate_backend_request(
    input: NativeBackendRequestInput,
) -> Result<NativeBackendRequest, NativeBackendReport> {
    let mut diagnostics = Vec::new();

    if input.cancellation.cancelled {
        diagnostics.push(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::Cancelled,
            "$.cancellation",
            input.cancellation.reason.clone().unwrap_or_else(|| "native backend request was cancelled".to_string()),
        ));
        return Err(NativeBackendReport::rejected(NativeBackendStatus::Cancelled, &input, diagnostics));
    }

    if input.runtime_plan.decision != RuntimeExecutionDecision::NativeCandidate {
        diagnostics.push(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::RuntimePlanNotNativeCandidate,
            "$.runtime_plan.decision",
            ...
        ));
    }
}
```

Planner notes:
- DuckDB adapter must not call backend prepare unless runtime decision is `NativeCandidate`.
- Backend reports already encode `skipped-toolchain`, `cancelled`, and `fail-closed`; surface those strings in test diagnostics.

---

### `crates/loom-native-melior/src/jit.rs` (service, request-response + transform)

**Analog:** `crates/loom-native-melior/src/jit.rs`

**Imports and output model pattern** (lines 1-19, 26-32):
```rust
use loom_core::arrow_buffer_lowering::{
    plan_arrow_buffers_from_decode_dialect, reference_zeroed_value_bytes,
};
use crate::backend::{
    NativeBackendCancellation, NativeBackendDiagnostic, NativeBackendDiagnosticCode,
    NativeBackendReport, NativeBackendStatus,
};

pub const PRODUCTION_JIT_ENTRY_SYMBOL: &str = "loom_decode_build_buffers";

pub struct ProductionJitOutput {
    pub entry_symbol: String,
    pub row_count: u64,
    pub column_count: usize,
    pub value_buffers: Vec<Vec<u8>>,
}
```

**Prepare-to-output guard pattern** (lines 34-88):
```rust
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
                cancellation.reason.clone().unwrap_or_else(|| "production JIT request was cancelled".to_string()),
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

    if artifact.entry_symbol.as_deref() != Some(PRODUCTION_JIT_ENTRY_SYMBOL) {
        return Err(report_with_diagnostic(... NativeBackendDiagnosticCode::JitSymbolMissing ...));
    }
}
```

**Explicit toolchain skip pattern** (lines 107-130):
```rust
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
    return Err(report_with_diagnostic(report, status, NativeBackendDiagnostic::new(code, "$.toolchain", message)));
}
```

**Native mismatch fail-closed pattern** (lines 157-175):
```rust
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
```

Planner notes:
- Native buffers may feed DuckDB only after accepted report, compatible/explicitly handled toolchain state, and successful comparison.
- Never fallback silently after `NativeOutputMismatch`.

---

### `scripts/duckdb-native-integration-test.sh` (test, batch + file-I/O)

**Analogs:** `scripts/duckdb-smoke-test.sh`, `scripts/production-backend-test.sh`, `scripts/runtime-abi-test.sh`

**Shell gate prelude pattern** (from `duckdb-smoke-test.sh` lines 1-29):
```bash
#!/usr/bin/env bash
set -euo pipefail

DUCKDB_VERSION="v1.5.3"
REPO_ROOT="$(git rev-parse --show-toplevel)"
EXT_PATH="${REPO_ROOT}/duckdb-ext/build/loom.duckdb_extension"
CLI_CACHE_DIR="${REPO_ROOT}/duckdb-ext/vendor/duckdb-cli"
PAYLOAD_DIR="${REPO_ROOT}/target/loom-duckdb-fixtures"

info()  { echo "${YLW}[smoke-test]${RST} $*"; }
ok()    { echo "${GRN}[PASS]${RST} $*"; }
fail()  { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT
```

**Fixture/build pattern** (from `duckdb-smoke-test.sh` lines 34-89):
```bash
info "Generating deterministic Loom payloads..."
cargo run -p loom-fixtures --bin emit_duckdb_payloads >/dev/null
test -f "${PAYLOAD_DIR}/mixed-table.loom"

info "Building loom.duckdb_extension..."
cargo build -p loom-ffi --release
rm -f "${EXT_PATH}"
cmake -S "${REPO_ROOT}/duckdb-ext" \
      -B "${REPO_ROOT}/duckdb-ext/build" \
      -DCMAKE_BUILD_TYPE=Release \
      2>&1 | grep -v '^--' || true
cmake --build "${REPO_ROOT}/duckdb-ext/build" 2>&1
```

**SQL assertion pattern** (from `duckdb-smoke-test.sh` lines 130-155):
```bash
sql_to_file() {
    local sql="$1"
    local out="$2"
    "${DUCKDB_BIN}" -unsigned -c \
        "LOAD '${EXT_PATH}'; COPY (${sql}) TO '${out}' (FORMAT CSV, HEADER FALSE);" \
        >/dev/null
}

check_rows() {
    local name="$1"
    local expected="$2"
    local payload="${PAYLOAD_DIR}/${name}.loom"
    local out="${TMP_DIR}/${name}-rows.csv"
    sql_to_file "SELECT COALESCE(CAST(value AS VARCHAR), 'NULL') FROM loom_scan('${payload}')" "${out}"
    local actual
    actual="$(cat "${out}")"
    if [ "${actual}" != "${expected}" ]; then
        fail "row mismatch for ${name}"
    fi
}
```

**Skip-aware native toolchain pattern** (from `production-backend-test.sh` lines 48-60):
```bash
set +e
llvm_bin_dir="$(toolchain_llvm_bin_dir)"
tool_status=$?
set -e
if [ "${tool_status}" -eq 2 ]; then
    skip "strict ODS validation skipped by explicit LOOM_ALLOW_NATIVE_TOOL_SKIP=1"
    echo ""
    echo "${GRN}=== Phase 23 production native backend gate PASSED WITH SKIP ===${RST}"
    exit 0
elif [ "${tool_status}" -ne 0 ]; then
    fail "managed MLIR/LLVM toolchain is unavailable or incompatible"
fi
```

**Runtime precondition test pattern** (from `runtime-abi-test.sh` lines 26-40):
```bash
info "Running runtime ABI contract tests..."
cargo test -p loom-core --test runtime_abi_contract
ok "runtime_abi_contract"

info "Running runtime execution policy tests..."
cargo test -p loom-core --test runtime_execution_policy
ok "runtime_execution_policy"
```

Planner notes:
- New script should assert route diagnostics, not just SQL rows.
- Include native-eligible primitive, fallback unsupported, strict fail-closed, projection ordering, and mismatch/cancel helper-level cases where host cancel is not available.
- Keep `LOOM_ALLOW_NATIVE_TOOL_SKIP=1` explicit for portable gates.

---

### `scripts/mvp0-verify.sh` (test/config, batch)

**Analog:** `scripts/mvp0-verify.sh`

**Release gate style** (lines 21-31):
```bash
info() { echo "${YLW}[mvp0-verify]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

echo "=== Loom MVP0 release gate ==="
echo "Repository: ${REPO_ROOT}"

info "Running workspace tests..."
cargo test --workspace
ok "cargo test --workspace"
```

**Dependency boundary guard pattern** (lines 33-45):
```bash
info "Checking loom-core has no Vortex/FastLanes dependencies..."
dep_count="$(cargo tree -p loom-core | awk '/vortex|fastlanes/{c++} END{print c+0}')"
if [ "${dep_count}" != "0" ]; then
    fail "loom-core dependency guard failed: found ${dep_count} vortex/fastlanes entries"
fi
ok "loom-core dependency guard printed 0"
```

**Phase gate append pattern** (lines 108-118):
```bash
info "Running Phase 22 runtime ABI gate..."
bash scripts/runtime-abi-test.sh
ok "scripts/runtime-abi-test.sh"

info "Running Phase 23 production native backend gate..."
bash scripts/production-backend-test.sh
ok "scripts/production-backend-test.sh"

info "Running DuckDB SQL smoke test..."
bash scripts/duckdb-smoke-test.sh
ok "scripts/duckdb-smoke-test.sh"
```

Planner notes:
- Add Phase 24 gate after Phase 23 backend and before or alongside DuckDB smoke.
- Preserve dependency guards for `loom-core` and `loom-ffi`; internal DuckDB bridge must not introduce Vortex dependencies.

## Shared Patterns

### FFI Panic And Return Codes
**Source:** `crates/loom-ffi/src/ffi.rs` lines 198-239  
**Apply to:** `crates/loom-ffi/src/duckdb_runtime.rs`, any new `extern "C"` helper.

Use null guards before slice reconstruction, wrap all body logic in `panic::catch_unwind(AssertUnwindSafe(...))`, return integer codes, and never unwind across C ABI.

### Arrow C Data Ownership
**Source:** `crates/loom-ffi/src/ffi.rs` lines 149-166 and `duckdb-ext/loom_extension.cpp` lines 349-370  
**Apply to:** interpreter path, any partially initialized scan state, error/cancel paths.

Rust writes `FFI_ArrowArray` and `FFI_ArrowSchema` exactly once with `std::ptr::write`; C++ scan state releases both callbacks exactly once in RAII destructors.

### Runtime Policy Is Authoritative
**Source:** `crates/loom-core/src/runtime_abi.rs` lines 619-752  
**Apply to:** DuckDB bind/global init route selection.

Do not reimplement eligibility in C++. Runtime decisions must use `NativeCandidate`, `InterpreterFallback`, `FailClosed`, or `DiagnosticOnly`, with diagnostics preserved by code/path/message.

### Backend Reports Are Authoritative
**Source:** `crates/loom-native-melior/src/backend.rs` lines 376-466; `pipeline.rs` lines 43-154; `jit.rs` lines 34-175  
**Apply to:** native prepare, JIT seed, strict/fallback routing.

Only prepare native for diagnostic-free native plans. Treat `native-output-mismatch` as fail-closed. Treat explicit toolchain skip as a diagnostic, not as proof native executed.

### DuckDB Scan Lifecycle
**Source:** `duckdb-ext/loom_extension.cpp` lines 377-603  
**Apply to:** `loom_scan(path)` public API.

Bind declares schema and stores immutable planning inputs. Global init prepares interpreter/native state. Scan writes exactly one `DataChunk` batch for Phase 24 and returns empty on repeated calls.

### Release Gate Style
**Source:** `scripts/duckdb-smoke-test.sh`, `scripts/production-backend-test.sh`, `scripts/mvp0-verify.sh`  
**Apply to:** `scripts/duckdb-native-integration-test.sh`, `scripts/mvp0-verify.sh`.

Use `set -euo pipefail`, repo-root discovery, color helpers, `mktemp` cleanup, cargo/cmake build steps, explicit skip handling, and exact stdout/CSV comparisons.

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| none | — | — | All Phase 24 target files have close local analogs. |

## Metadata

**Analog search scope:** `duckdb-ext/`, `crates/loom-ffi/`, `crates/loom-core/src/runtime_abi.rs`, `crates/loom-native-melior/src/`, `scripts/`  
**Files scanned:** 95 source/script paths from `rg --files duckdb-ext crates scripts` plus phase context/research  
**Pattern extraction date:** 2026-06-08
