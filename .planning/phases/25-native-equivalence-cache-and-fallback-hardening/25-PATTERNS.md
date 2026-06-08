# Phase 25: Native Equivalence, Cache, and Fallback Hardening - Pattern Map

**Mapped:** 2026-06-09
**Files analyzed:** 11 likely new/modified files
**Analogs found:** 11 / 11

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/loom-core/src/runtime_abi.rs` | model/utility | request-response, transform | `crates/loom-core/src/runtime_abi.rs` | exact |
| `crates/loom-core/tests/runtime_cache_key.rs` | test | transform | `crates/loom-core/tests/runtime_cache_key.rs` | exact |
| `crates/loom-core/tests/runtime_execution_policy.rs` | test | request-response | `crates/loom-core/tests/runtime_execution_policy.rs` | exact |
| `crates/loom-core/tests/runtime_scan_planning.rs` | test | request-response | `crates/loom-core/tests/runtime_scan_planning.rs` | exact |
| `crates/loom-ffi/src/duckdb_runtime.rs` | service/adapter | request-response, transform | `crates/loom-ffi/src/duckdb_runtime.rs` | exact |
| `crates/loom-ffi/tests/duckdb_runtime.rs` | test | request-response | `crates/loom-ffi/tests/duckdb_runtime.rs` | exact |
| `crates/loom-ffi/tests/duckdb_runtime_ffi.rs` | test | FFI request-response | `crates/loom-ffi/tests/duckdb_runtime_ffi.rs` | exact |
| `crates/loom-native-melior/src/backend.rs` | service/model | request-response | `crates/loom-native-melior/src/backend.rs` | exact |
| `crates/loom-native-melior/src/jit.rs` | service | transform | `crates/loom-native-melior/src/jit.rs` | exact |
| `duckdb-ext/loom_extension.cpp` | adapter/controller | request-response, file-I/O | `duckdb-ext/loom_extension.cpp` | exact |
| `scripts/native-hardening-test.sh` | release gate | batch | `scripts/duckdb-native-integration-test.sh` | role-match |
| `scripts/mvp0-verify.sh` | release gate | batch | `scripts/mvp0-verify.sh` | exact |

## Ownership Boundaries

| Area | Owns | Must Not Own |
|---|---|---|
| `loom-core/src/runtime_abi.rs` | Stable runtime vocabulary, `RuntimeCacheKey`, cache-key inputs, policy decisions, diagnostic codes/paths/messages. | DuckDB, MLIR, Vortex, tool invocation, native artifact storage. |
| `loom-ffi/src/duckdb_runtime.rs` | Internal DuckDB runtime bridge, plan/prepared handles, internal evidence hooks, in-process prepare/cache orchestration, comparison-gated native buffers. | Public SQL/API, route policy duplication, persistent cache format, C++ output policy. |
| `loom-native-melior/src/*` | Backend identity, toolchain/pipeline identity, validated native artifact/report model, cancellation/toolchain/mismatch diagnostics. | DuckDB-specific behavior, fallback policy, public cache controls. |
| `duckdb-ext/loom_extension.cpp` | DuckDB bind/init/scan lifecycle, projected column ids from DuckDB, direct `DataChunk` population, route report file append. | Native eligibility, cache eligibility, fallback policy, persistent cache, predicate/split semantics. |
| `scripts/*.sh` | Deterministic gate orchestration, fixture generation, route-report grep evidence, public API creep guards. | Benchmark claims or public feature flags. |

## Pattern Assignments

### `crates/loom-core/src/runtime_abi.rs` (model/utility, request-response + transform)

**Analog:** `crates/loom-core/src/runtime_abi.rs`

**Imports and host-neutral boundary** (lines 1-10):
```rust
//! Host-neutral runtime ABI and execution policy model.
//!
//! Phase 22 keeps this vocabulary inside `loom-core` so later host adapters can
//! consume one verifier-gated contract without importing host engine, Vortex, or
//! native compiler types.

use std::fmt;

use crate::artifact_verifier::{ArtifactVerificationStatus, ConstraintDischargeStatus};
```

**Stable diagnostic code pattern** (lines 308-340):
```rust
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
```

**Cache key canonicalization pattern** (lines 433-480):
```rust
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

**Policy decision pattern** (lines 619-751):
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
    ...
}
```

**Phase 25 guidance:** add cache hit/miss/revalidation diagnostic vocabulary here if it is shared by multiple hosts. Keep any new key fields in `RuntimeCacheKeyInput` and `canonical_cache_input`; do not add backend-specific cache structs here.

### `crates/loom-core/tests/runtime_cache_key.rs` (test, transform)

**Analog:** `crates/loom-core/tests/runtime_cache_key.rs`

**Base key fixture pattern** (lines 8-30):
```rust
fn key_input() -> RuntimeCacheKeyInput {
    RuntimeCacheKeyInput {
        abi_version: RuntimeAbiVersion::CURRENT,
        artifact_digest: "artifact-a".to_string(),
        facts_fingerprint: "facts-a".to_string(),
        solver_identity: "bitwuzla-script-a".to_string(),
        production_lowering_fingerprint: "lowering-a".to_string(),
        backend_identity: backend_identity(),
        projection: ProjectionSet::All,
        predicate: PredicateEnvelope::None,
        split: SplitDescriptor::FullScan { row_count: 4 },
        policy: RuntimeSafetyPolicy::default(),
    }
}
```

**Mutation matrix pattern** (lines 42-99):
```rust
let input = key_input();
let baseline = RuntimeCacheKey::build(&input);

let mut changed = input.clone();
changed.artifact_digest = "artifact-b".to_string();
assert_ne!(baseline, RuntimeCacheKey::build(&changed));
```

**Stable diagnostic string pattern** (lines 101-114):
```rust
let diagnostic = RuntimeDiagnostic::new(
    RuntimeDiagnosticCode::CacheKeyMismatch,
    "$.cache.key",
    "cache key mismatch",
);
assert_eq!(diagnostic.code.as_str(), "cache-key-mismatch");
```

**Phase 25 guidance:** extend this test for every new cache-contract input: backend identity drift, toolchain identity drift, lowering facts fingerprint, projection, predicate, split, fallback policy, and artifact/facts fingerprint. Use mutation-per-field assertions, not a single snapshot.

### `crates/loom-ffi/src/duckdb_runtime.rs` (service/adapter, request-response + transform)

**Analog:** `crates/loom-ffi/src/duckdb_runtime.rs`

**Boundary comment and imports pattern** (lines 1-6, 27-44):
```rust
//! This module keeps DuckDB as an adapter over the Phase 22 runtime ABI and
//! Phase 23 backend vocabulary. It is safe Rust only; later C ABI wrappers can
//! translate these owned reports into DuckDB-facing handles without duplicating
//! runtime policy in C++.

use loom_core::runtime_abi::{
    decide_runtime_execution, plan_projection, ConcurrencyPolicy, PredicateEnvelope,
    ProjectionColumn, ProjectionSet, RuntimeAbiVersion, RuntimeBackendIdentity, RuntimeCacheKey,
    RuntimeCacheKeyInput, RuntimeEmissionDisposition, RuntimeExecutionDecision,
    RuntimeFallbackPolicy, RuntimeLoweringDisposition, RuntimePlan, RuntimeReaderSupport,
    RuntimeSafetyPolicy, SplitDescriptor, UnsupportedPredicatePolicy,
};
```

**Report model to extend for internal cache evidence** (lines 120-139):
```rust
pub struct DuckDbRuntimePlanReport {
    pub decision: DuckDbRouteDecision,
    pub runtime_plan: RuntimePlan,
    pub cache_key: RuntimeCacheKey,
    pub output_to_source: Vec<u32>,
    pub policy: RuntimeSafetyPolicy,
    pub artifact_report: ArtifactVerificationReport,
    pub lowering_facts: Option<ProductionLoweringFacts>,
    pub test_jit_value_buffers: Option<Vec<Vec<u8>>>,
    pub diagnostics: Vec<DuckDbRuntimeDiagnostic>,
}
```

**Internal FFI accessor pattern** (lines 397-435):
```rust
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
    ...
}
```

**Plan-building/cache input pattern** (lines 1010-1058):
```rust
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
```

**Prepare/fail-closed comparison pattern** (lines 856-1007):
```rust
if plan_report.runtime_plan.decision != RuntimeExecutionDecision::NativeCandidate
    || !plan_report.runtime_plan.diagnostics.is_empty()
{
    return DuckDbPreparedRoute {
        decision: plan_report.decision,
        backend_report: None,
        native_buffers: Vec::new(),
        diagnostics,
    };
}
...
if let Err(report) =
    compare_production_jit_output(&backend_report, &expected_buffers, &jit_output)
{
    diagnostics.extend(backend_diagnostics(&report));
    return DuckDbPreparedRoute {
        decision: DuckDbRouteDecision::FailClosed,
        backend_report: Some(report),
        native_buffers: Vec::new(),
        diagnostics,
    };
}
```

**Phase 25 guidance:** implement in-process native artifact cache here or in a small sibling module consumed only here. Cache only accepted/prepared artifacts or deterministic preparation evidence after comparison succeeds. Expose hit/miss/revalidation counters through internal `loom_duckdb_*` accessors or route diagnostics; keep public `loom.h` clean.

### `crates/loom-ffi/tests/duckdb_runtime.rs` (test, request-response)

**Analog:** `crates/loom-ffi/tests/duckdb_runtime.rs`

**Deterministic fixture pattern** (lines 11-26, 28-66):
```rust
fn raw_i32_lmc1(row_count: u64) -> Vec<u8> {
    let values = (0..row_count as i32)
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    ...
    wrap_layout_payload(&payload).expect("valid LMC1 layout")
}
```

**Route policy/no predicate pattern** (lines 89-105, 175-183):
```rust
let report = plan_duckdb_runtime(native_input()).expect("runtime plan");
assert_eq!(report.decision, DuckDbRouteDecision::NativeCandidate);
assert_eq!(report.runtime_plan.predicate, PredicateEnvelope::None);
assert!(report.cache_key.canonical_input.contains("predicate=none"));
```

**Fallback/strict pattern** (lines 144-173):
```rust
let report = plan_duckdb_runtime(unsupported).expect("fallback runtime plan");
assert_eq!(report.decision, DuckDbRouteDecision::InterpreterFallback);
assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.code == "lowering-unsupported"));

let report = plan_duckdb_runtime(strict).expect("strict runtime plan");
assert_eq!(report.decision, DuckDbRouteDecision::FailClosed);
```

**Native mismatch/cancel pattern** (lines 249-276):
```rust
let prepared = prepare_duckdb_runtime(
    &native,
    NativeBackendCancellation::cancelled("duckdb interrupt"),
);
assert_eq!(prepared.decision, DuckDbRouteDecision::Cancelled);
assert!(prepared.native_buffers.is_empty());

let prepared = prepare_duckdb_runtime(&native, NativeBackendCancellation::default());
assert_eq!(prepared.decision, DuckDbRouteDecision::FailClosed);
assert!(prepared.native_buffers.is_empty());
```

**Phase 25 guidance:** put helper-level cache reuse/invalidation tests here if they need direct access to counters or fake JIT buffers. Add equivalence matrix rows here for primitive/table/projection shapes that SQL cannot easily force.

### `crates/loom-ffi/tests/duckdb_runtime_ffi.rs` (test, FFI request-response)

**Analog:** `crates/loom-ffi/tests/duckdb_runtime_ffi.rs`

**Opaque-handle accessor pattern** (lines 56-109):
```rust
let mut plan: *mut LoomDuckDbPlan = ptr::null_mut();
let rc = unsafe {
    loom_duckdb_plan_create(
        artifact.as_ptr(),
        artifact.len(),
        false,
        true,
        &mut plan as *mut _,
    )
};
assert_eq!(rc, 0, "plan creation should succeed");
...
assert!(unsafe { cstr(cache_key) }.starts_with("loom-runtime-v"));
```

**Projected cache input pattern** (lines 112-142):
```rust
let cache_input = unsafe { cstr(cache_input) };
assert!(
    cache_input.contains("projection=columns:0>0"),
    "projected plan should enter the runtime cache input, got {cache_input}"
);
```

**Public API leakage gate pattern** (lines 430-447):
```rust
for forbidden in [
    "loom_duckdb_",
    "LoomDuckDb",
    "loom_scan_native",
    "loom_scan_interpreter",
    "ArrowArrayStream",
] {
    assert!(
        !public_header.contains(forbidden),
        "public loom.h must not expose {forbidden}"
    );
}
```

**Phase 25 guidance:** add FFI tests for any internal cache evidence accessors. Also add forbidden markers for public cache/native/fallback controls if new internal names risk leaking into `loom.h`.

### `crates/loom-native-melior/src/backend.rs` (service/model, request-response)

**Analog:** `crates/loom-native-melior/src/backend.rs`

**Identity key pattern** (lines 62-137):
```rust
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
```

**Backend artifact identity pattern** (lines 264-284):
```rust
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
```

**Request validation fail-closed pattern** (lines 376-466):
```rust
if input.runtime_cache_key.is_none() {
    diagnostics.push(NativeBackendDiagnostic::new(
        NativeBackendDiagnosticCode::MissingCacheKey,
        "$.runtime_cache_key",
        "native backend requires the Phase 22 runtime cache key",
    ));
}
...
if !diagnostics.is_empty() {
    return Err(NativeBackendReport::rejected(
        NativeBackendStatus::FailClosed,
        &input,
        diagnostics,
    ));
}
```

**Phase 25 guidance:** cache identity drift belongs in `NativeBackendIdentity::as_key` and `RuntimeCacheKeyInput`, not DuckDB C++. Add backend-level diagnostics only for backend facts/toolchain/artifact validity; fallback decisions remain above this layer.

### `crates/loom-native-melior/src/jit.rs` (service, transform)

**Analog:** `crates/loom-native-melior/src/jit.rs`

**Cancellation and accepted-artifact guard pattern** (lines 34-76):
```rust
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
    return Err(report_with_diagnostic(...));
}
```

**Equivalence comparison pattern** (lines 157-175):
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

**Phase 25 guidance:** reuse this comparison boundary for helper-level native/interpreter equivalence. Do not cache unchecked JIT output if this comparison fails.

### `duckdb-ext/loom_extension.cpp` (adapter/controller, request-response + file-I/O)

**Analog:** `duckdb-ext/loom_extension.cpp`

**Internal headers and RAII handles** (lines 33-36, 70-144):
```cpp
extern "C" {
#include "../crates/loom-ffi/include/loom.h"
#include "../crates/loom-ffi/include/loom_duckdb_internal.h"
}

struct LoomDuckDbPlanHolder {
    LoomDuckDbPlan *plan = nullptr;
    ~LoomDuckDbPlanHolder() { Reset(); }
    void Reset() {
        if (plan != nullptr) {
            loom_duckdb_plan_destroy(plan);
            plan = nullptr;
        }
    }
};
```

**Route report pattern** (lines 262-280):
```cpp
static void AppendTestRouteReport(const char *phase,
                                  const string &route,
                                  const vector<LoomRouteDiagnostic> &diagnostics,
                                  const string &cache_key = string()) {
    const char *report_path = std::getenv("LOOM_DUCKDB_TEST_ROUTE_REPORT");
    if (report_path == nullptr || report_path[0] == '\0') {
        return;
    }
    std::ofstream out(report_path, std::ios::app);
    out << phase << "\troute=" << route;
    if (!cache_key.empty()) {
        out << "\tcache_key=" << cache_key;
    }
    out << "\t" << FormatRouteDiagnostics(diagnostics) << "\n";
}
```

**Projection-to-runtime pattern** (lines 391-462):
```cpp
static LoomRuntimePlanSelection BuildProjectedRuntimePlan(const LoomBindData &bind_data,
                                                          const TableFunctionInitInput &input) {
    auto projected_ids = ProjectedSourceColumnIds(input, bind_data.column_payloads.size());
    if (IsAllColumnProjection(projected_ids, bind_data.column_payloads.size())) {
        return { bind_data.runtime_plan, bind_data.route_decision, bind_data.route_cache_key,
                 bind_data.route_cache_input, bind_data.route_diagnostics, std::move(projected_ids), true };
    }

    auto projected_plan =
        CreateProjectedRuntimePlan(bind_data.payload, projected_ids, bind_data.allow_interpreter_fallback);
    ...
}
```

**Bind/init route pattern** (lines 789-800, 833-887):
```cpp
bind_data->runtime_plan =
    CreateRuntimePlan(bind_data->payload, bind_data->allow_interpreter_fallback);
bind_data->route_decision = ReadPlanDecision(*bind_data->runtime_plan);
bind_data->route_cache_key = ReadPlanCacheKey(*bind_data->runtime_plan);
bind_data->route_cache_input = ReadPlanCacheInput(*bind_data->runtime_plan);
AppendTestRouteReport("bind", bind_data->route_decision, bind_data->route_diagnostics, bind_data->route_cache_input);

if (state->route_decision == "native-candidate" && state->output_column_count > 0) {
    auto prepared = CreatePreparedRoute(*runtime_plan.runtime_plan, cancelled);
    auto prepared_route = ReadPreparedRoute(prepared);
    ...
}
```

**Direct DataChunk pattern** (lines 1137-1181):
```cpp
if (state.route_decision == "native-candidate") {
    if (state.native_buffers.empty()) {
        throw IOException("%s", FormatRouteError("D-12/native-claim-without-buffers",
                                                 state.route_decision,
                                                 state.route_diagnostics).c_str());
    }
    const auto count = NativeRowCount(state);
    for (idx_t col = 0; col < state.output_column_count; col++) {
        const auto &buffer = NativeBufferForOutput(state, col);
        FillNativeBufferIntoVector(buffer, state.column_kinds[col], output.data[col], count);
    }
    output.SetCardinality(count);
    state.batch_emitted = true;
    AppendTestRouteReport("scan", state.route_decision, state.route_diagnostics, state.route_cache_input);
    return;
}
```

**Phase 25 guidance:** C++ may append cache evidence returned by Rust to `LOOM_DUCKDB_TEST_ROUTE_REPORT`. It must not implement cache eligibility, key comparison, fallback policy, or public SQL controls.

### `scripts/native-hardening-test.sh` (release gate, batch)

**Analog:** `scripts/duckdb-native-integration-test.sh`

**Gate skeleton pattern** (lines 4-34, 118-125):
```bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/loom-duckdb-native-XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT

export LOOM_DUCKDB_TEST_ROUTE_REPORT="${TMP_DIR}/route-report.tsv"
: >"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"

require_report() {
    local pattern="$1"
    rg -q "${pattern}" "${LOOM_DUCKDB_TEST_ROUTE_REPORT}" || {
        echo "Route report:" >&2
        cat "${LOOM_DUCKDB_TEST_ROUTE_REPORT}" >&2
        fail "route report missing ${pattern}"
    }
}
```

**SQL helper pattern** (lines 100-116):
```bash
sql_to_file() {
    local sql="$1"
    local out="$2"
    "${DUCKDB_BIN}" -unsigned -c \
        "LOAD '${EXT_PATH}'; COPY (${sql}) TO '${out}' (FORMAT CSV, HEADER FALSE);" \
        >/dev/null
}

sql_expect_failure() {
    local sql="$1"
    local err="$2"
    set +e
    "${DUCKDB_BIN}" -unsigned -c "LOAD '${EXT_PATH}'; ${sql}" >"${TMP_DIR}/failed-query.out" 2>"${err}"
    local status=$?
    set -e
    [ "${status}" -ne 0 ] || fail "expected DuckDB query to fail: ${sql}"
}
```

**Public API creep pattern** (lines 223-243):
```bash
for suffix in native interpreter; do
    if rg -n "${route_prefix}${suffix}" scripts/duckdb-native-integration-test.sh duckdb-ext/loom_extension.cpp crates/loom-ffi/include/loom.h; then
        fail "found forbidden public route function marker"
    fi
done
```

**Phase 25 guidance:** create `scripts/native-hardening-test.sh` as a focused Phase 25 gate, then call existing Phase 22/23/24 tests plus new cache/equivalence/diagnostic tests. Assert smoke evidence such as second identical scan hits cache or avoids prepare counter increment; do not claim benchmark speedup.

## Shared Patterns

### Route Policy Ownership

**Source:** `crates/loom-core/src/runtime_abi.rs` lines 619-751; `crates/loom-ffi/src/duckdb_runtime.rs` lines 813-853.
**Apply to:** runtime ABI, DuckDB runtime bridge, C++ adapter.

All fallback/fail-closed/native decisions flow through `decide_runtime_execution`; DuckDB adapter code reads route strings and diagnostics. Preserve this by adding cache invalidation decisions in Rust reports, not C++ branches.

### Cache Key Material

**Source:** `crates/loom-core/src/runtime_abi.rs` lines 433-480; `crates/loom-ffi/src/duckdb_runtime.rs` lines 1035-1046.
**Apply to:** runtime cache key tests, DuckDB runtime cache, native backend identity.

Key fields already include ABI, artifact digest, facts fingerprint, solver identity, production lowering fingerprint, backend identity, projection, predicate, split, and policy. Phase 25 should add only deliberate key material, with mutation tests for each field.

### Deterministic Diagnostics

**Source:** `crates/loom-core/src/runtime_abi.rs` lines 308-361; `crates/loom-native-melior/src/backend.rs` lines 200-261; `duckdb-ext/loom_extension.cpp` lines 220-244.
**Apply to:** all negative tests and release gates.

Use stable `code`, JSON-ish `path`, and deterministic message text. Shell gates should grep for `diagnostic code=.*path=` and specific route codes rather than brittle full stderr.

### Native Equivalence Boundary

**Source:** `crates/loom-native-melior/src/jit.rs` lines 157-175; `crates/loom-ffi/src/duckdb_runtime.rs` lines 990-1007.
**Apply to:** native mismatch, equivalence matrix, cache population.

Native buffers become usable only after reference/interpreter buffer comparison succeeds. Failed comparison returns fail-closed and empty native buffers.

### SQL Integration Evidence

**Source:** `scripts/duckdb-native-integration-test.sh` lines 131-179, 212-221.
**Apply to:** SQL equivalence, projection order, repeated scans, performance smoke.

Reuse public `loom_scan(path)` only. Use `LOOM_DUCKDB_TEST_ROUTE_REPORT` as internal evidence for routes, cache status, and projection cache input.

### Release Gate Wiring

**Source:** `scripts/mvp0-verify.sh` lines 108-122.
**Apply to:** `scripts/native-hardening-test.sh`, `scripts/mvp0-verify.sh`.

Wire Phase 25 after Phase 24 DuckDB native integration and before `scripts/duckdb-smoke-test.sh`.

## Anti-Patterns / Non-Goals To Preserve

| Anti-Pattern | Guardrail |
|---|---|
| Public SQL/API creep | Keep public SQL as `loom_scan(path)`. Extend only internal `loom_duckdb_*` or `LOOM_DUCKDB_TEST_*` evidence hooks. Preserve `loom.h` leakage tests. |
| C++ route policy duplication | C++ must not decide native eligibility, fallback permission, cache validity, or unsupported lowering policy. It should consume Rust route reports. |
| Persistent cache by accident | Phase 25 cache should be in-process unless explicitly planned otherwise. Do not create on-disk formats or path/mtime semantics. |
| Arbitrary native widening | Do not add native strings, nullable execution, compression expansion, predicate pushdown, parallel splits, or arbitrary Vortex compatibility. Unsupported cases are fallback/fail-closed evidence. |
| Caching failed native output | Never cache unchecked buffers after `native-output-mismatch`, cancellation, malformed artifacts, or backend diagnostics. |
| Benchmark overclaiming | Performance evidence is a smoke proof of reuse/invalidation, not a native-speed benchmark claim. |

## Suggested File-To-Plan Ownership Hints

| Plan Slice | Primary Files | Supporting Tests/Gates |
|---|---|---|
| Runtime cache contract and diagnostics | `crates/loom-core/src/runtime_abi.rs` | `crates/loom-core/tests/runtime_cache_key.rs`, `runtime_abi_contract.rs` |
| DuckDB in-process cache and evidence hooks | `crates/loom-ffi/src/duckdb_runtime.rs`, `crates/loom-ffi/include/loom_duckdb_internal.h`, `crates/loom-ffi/cbindgen.toml` | `crates/loom-ffi/tests/duckdb_runtime.rs`, `duckdb_runtime_ffi.rs` |
| Native backend identity/invalidation checks | `crates/loom-native-melior/src/backend.rs`, `pipeline.rs`, `jit.rs` | `production_backend_contract.rs`, `production_backend_pipeline.rs`, `production_backend_jit.rs` |
| SQL equivalence and route report hardening | `duckdb-ext/loom_extension.cpp` | `scripts/native-hardening-test.sh`, existing `scripts/duckdb-native-integration-test.sh` |
| Release gate and final evidence | `scripts/native-hardening-test.sh`, `scripts/mvp0-verify.sh`, Phase 25 report | `bash -n scripts/native-hardening-test.sh scripts/mvp0-verify.sh`; full gate with `LOOM_ALLOW_NATIVE_TOOL_SKIP=1` |

## No Analog Found

None. Phase 25 is a hardening extension over existing Phase 22/23/24 patterns.

## Metadata

**Analog search scope:** `crates/loom-core`, `crates/loom-ffi`, `crates/loom-native-melior`, `duckdb-ext`, `scripts`, Phase 24 planning artifacts.
**Files scanned:** 40+ via `rg --files`, with targeted reads of runtime ABI, FFI adapter/tests, native backend/tests, C++ adapter, gate scripts, and Phase 24 verification/report.
**Pattern extraction date:** 2026-06-09
