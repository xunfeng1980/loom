---
phase: 24-duckdb-native-execution-integration-mvp
reviewed: "2026-06-08T16:55:56Z"
depth: standard
files_reviewed: 13
files_reviewed_list:
  - crates/loom-ffi/Cargo.toml
  - crates/loom-ffi/src/lib.rs
  - crates/loom-ffi/src/duckdb_runtime.rs
  - crates/loom-ffi/include/loom_duckdb_internal.h
  - crates/loom-ffi/cbindgen.toml
  - crates/loom-ffi/build.rs
  - crates/loom-ffi/tests/duckdb_runtime.rs
  - crates/loom-ffi/tests/duckdb_runtime_ffi.rs
  - duckdb-ext/loom_extension.cpp
  - crates/loom-fixtures/src/bin/emit_duckdb_payloads.rs
  - scripts/duckdb-native-integration-test.sh
  - scripts/mvp0-verify.sh
  - scripts/check-core-invariants.sh
findings:
  critical: 2
  warning: 3
  info: 0
  total: 5
status: issues_found
---

# Phase 24: Code Review Report

**Reviewed:** 2026-06-08T16:55:56Z
**Depth:** standard
**Files Reviewed:** 13
**Status:** issues_found

## Summary

Reviewed the Phase 24 DuckDB runtime bridge, private C ABI, C++ `loom_scan` route/projection/native adapter, fixture generation, and release-gate scripts. `Cargo.lock` was read for dependency-boundary context and excluded from `files_reviewed_list` per the workflow lock-file filter.

The main defects are in the projection/native path: projected scans can re-enable interpreter fallback even when strict fail-closed mode was requested, and native buffer lookup can map reordered projections to the wrong source buffers.

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01: Projected Scans Bypass Strict Fail-Closed Policy

**Classification:** BLOCKER
**File:** `duckdb-ext/loom_extension.cpp:397`
**Issue:** `BuildProjectedRuntimePlan` creates a new runtime plan for non-all projections with `CreateRuntimePlan(bind_data.payload, true)`, hard-coding interpreter fallback on. `LoomBind` correctly reads `LOOM_DUCKDB_TEST_ALLOW_INTERPRETER_FALLBACK` at lines 735-737, but that policy is discarded for projected queries. A strict query that should fail closed can therefore succeed through interpreter fallback simply by selecting a projection such as `SELECT value FROM loom_scan(...)`.
**Fix:**
```cpp
struct LoomBindData : TableFunctionData {
    bool allow_interpreter_fallback = true;
    // ...
};

// Copy()/Equals() should preserve/compare allow_interpreter_fallback.

bind_data->allow_interpreter_fallback =
    !TestEnvDisabled("LOOM_DUCKDB_TEST_ALLOW_INTERPRETER_FALLBACK", false);
bind_data->runtime_plan =
    CreateRuntimePlan(bind_data->payload, bind_data->allow_interpreter_fallback);

// In BuildProjectedRuntimePlan:
auto projected_plan =
    CreateRuntimePlan(bind_data.payload, bind_data.allow_interpreter_fallback);
```
Add an integration assertion that `LOOM_DUCKDB_TEST_ALLOW_INTERPRETER_FALLBACK=0` still fails for a projected unsupported scan.

### CR-02: Native Buffer Mapping Breaks Reordered All-Column Projections

**Classification:** BLOCKER
**File:** `duckdb-ext/loom_extension.cpp:1029`
**Issue:** `NativeBufferForOutput` assumes buffers are compact/projected whenever `native_buffers.size() == projected_source_ids.size()`. The C++ adapter never passes a projection into the Rust C ABI (`loom_duckdb_plan_create` always uses `DuckDbProjection::All` at `crates/loom-ffi/src/duckdb_runtime.rs:290`), so native buffers are source-ordered. For a reordered projection containing all source columns, e.g. `[3, 2, 1, 0]`, the sizes match and this function reads buffers by output index instead of source index. With matching column types this silently returns columns in the wrong order; with differing types it fails closed even though the projection is valid.
**Fix:**
```cpp
static const LoomDuckDbNativeBuffer &NativeBufferForOutput(const LoomScanState &state,
                                                          idx_t output_idx) {
    const auto source_idx = state.projected_source_ids[output_idx];
    if (source_idx < state.native_buffers.size()) {
        return state.native_buffers[source_idx];
    }
    throw IOException("loom_scan[D-08/native-output-mismatch]: diagnostic code=native-output-mismatch path=$.native.buffers message=native buffer count does not match projected source columns");
}
```
If the ABI later supports projection-aware native buffers, add an explicit `native_buffers_are_projected` flag rather than inferring it from vector length.

## Warnings

### WR-01: Test Native Facts Misdescribe Table Payloads

**Classification:** WARNING
**File:** `crates/loom-ffi/src/duckdb_runtime.rs:632`
**Issue:** `test_native_facts_for_artifact` only calls `decode_layout_payload_maybe_container`. For an `LMT1` table container, that fails and the function falls back to `(0, Int32)`, so `LOOM_DUCKDB_TEST_USE_NATIVE_FACTS=1` can turn a valid primitive table into a native plan with zero rows and one Int32 output. That makes the test-only native route unrepresentative for the Phase 24 table fixture and can produce empty or mismatched native output.
**Fix:** Decode table containers explicitly and derive row count/types from all columns:
```rust
if let Ok(table) = decode_table_payload_maybe_container(artifact) {
    return DuckDbTestNativeFacts {
        row_count: table.row_count as u64,
        columns: table.columns.iter().map(|column| column.layout.data_type.clone()).collect(),
        test_jit_value_buffers: None,
    };
}
```

### WR-02: Route Gate Satisfies Toolchain Diagnostic Requirement With Synthetic Text

**Classification:** WARNING
**File:** `scripts/duckdb-native-integration-test.sh:141`
**Issue:** When no native toolchain skip/failure occurs, the script appends `toolchain-skipped: not observed...` and later `require_report 'toolchain-skipped|toolchain-failed'` passes. This means the final release gate cannot distinguish an actual backend diagnostic from a placeholder inserted by the test itself.
**Fix:** Do not write the required diagnostic token for non-observations. Track the case under a distinct string and gate conditionally:
```bash
if rg -q 'toolchain-skipped|toolchain-failed' "${LOOM_DUCKDB_TEST_ROUTE_REPORT}"; then
    ok "toolchain skip/failure diagnostic observed"
else
    echo "toolchain-not-observed: native route completed or fell back earlier" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
fi
```
Remove the unconditional final `require_report 'toolchain-skipped|toolchain-failed'`, or require it only in a scenario that forces the toolchain-skip path.

### WR-03: CMake Configure Failures Are Masked

**Classification:** WARNING
**File:** `scripts/duckdb-native-integration-test.sh:54`
**Issue:** The configure step pipes CMake output through `grep -v '^--' || true`. With `set -o pipefail`, any CMake configure failure is still swallowed by `|| true`, and the script proceeds to `cmake --build`. That can hide the real configure error or build from stale configuration state.
**Fix:**
```bash
cmake_out="${TMP_DIR}/cmake-configure.log"
if ! cmake -S "${REPO_ROOT}/duckdb-ext" \
          -B "${REPO_ROOT}/duckdb-ext/build" \
          -DCMAKE_BUILD_TYPE=Release \
          >"${cmake_out}" 2>&1; then
    cat "${cmake_out}" >&2
    fail "CMake configure failed"
fi
grep -v '^--' "${cmake_out}" || true
```

---

_Reviewed: 2026-06-08T16:55:56Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
