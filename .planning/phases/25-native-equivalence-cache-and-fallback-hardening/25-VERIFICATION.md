---
phase: 25-native-equivalence-cache-and-fallback-hardening
verified: 2026-06-08T18:36:45Z
status: passed
score: 15/15 must-haves verified
overrides_applied: 0
gaps_remaining: []
---

# Phase 25: Native Equivalence, Cache, and Fallback Hardening Verification Report

**Phase Goal:** Harden the native execution path before source/table-format binding by adding bounded interpreter/reference equivalence, in-process native preparation cache reuse/invalidation, deterministic unsupported-route diagnostics, SQL-level `loom_scan(path)` hardening, cache smoke evidence, release-gate wiring, and bounded Phase 26 handoff.
**Verified:** 2026-06-08T18:36:45Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Runtime cache reuse requires exact `RuntimeCacheKey` `stable_id` and `canonical_input` equality. | VERIFIED | `RuntimeCacheKey::compatibility_with` returns miss on different `stable_id`, hit only on equal `canonical_input`, and key mismatch on same id/different input in `crates/loom-core/src/runtime_abi.rs:496`. `cargo test -p loom-core --test runtime_cache_key cache` passed. |
| 2 | Cache-key mismatch diagnostics are stable and host-neutral before DuckDB-specific code sees them. | VERIFIED | `RuntimeDiagnosticCode::CacheKeyMismatch` maps to `cache-key-mismatch` in `crates/loom-core/src/runtime_abi.rs:328`; mismatch path is `$.cache.key` at `runtime_abi.rs:513`. |
| 3 | Runtime cache identity includes artifact, facts, lowering, backend/toolchain, projection, predicate, split, and policy inputs. | VERIFIED | `RuntimeCacheKeyInput` fields cover all required inputs at `runtime_abi.rs:444`; canonical input formats them at `runtime_abi.rs:522`. Mutation tests passed in `runtime_cache_key.rs`. |
| 4 | Identical native-candidate plans can reuse accepted in-process preparation evidence. | VERIFIED | `NATIVE_PREPARATION_CACHE` is process-local `OnceLock<Mutex<HashMap<...>>>` in `crates/loom-ffi/src/duckdb_runtime.rs:157`; hit path returns cloned accepted report at `duckdb_runtime.rs:1274`. `duckdb_runtime_cache` hit/miss tests passed. |
| 5 | Projection, policy, backend/toolchain, lowering, artifact, or facts drift creates a miss or key-mismatch diagnostic before reuse. | VERIFIED | Cache keys are built from artifact/facts/lowering/backend/projection/predicate/split/policy at `duckdb_runtime.rs:1091`; projection and policy drift tests passed in `crates/loom-ffi/tests/duckdb_runtime_cache.rs:173`. |
| 6 | Failed, cancelled, skipped-toolchain, and native-output-mismatch routes never create reusable cache entries. | VERIFIED | Non-accepted or diagnostic-bearing backend reports return `cache-non-cacheable` before insertion at `duckdb_runtime.rs:945` and `duckdb_runtime.rs:957`; unsafe route tests passed in `duckdb_runtime_cache.rs:223`. |
| 7 | Supported raw non-null primitive native buffers match interpreter/reference bytes for single-column and table shapes. | VERIFIED | Equivalence tests compare builder id, Arrow type, row count bytes, and value buffers for `Int32`, `Int64`, `Float32`, `Float64` in `crates/loom-ffi/tests/duckdb_runtime.rs:151`. |
| 8 | Projection order is covered at helper level before SQL adapter evidence. | VERIFIED | Projected helper test asserts `output_to_source == [3,0,2]` and compares projected buffers at `duckdb_runtime.rs:245`. |
| 9 | Unsupported strings, nullable/compressed inputs, cancellation, native mismatch, and strict fallback routes are negative evidence, not native-success claims. | VERIFIED | Fallback/fail-closed helper matrix covers unsupported strings/compressed layouts, cancellation, native-output mismatch, projection/predicate/split fail-closed paths at `duckdb_runtime.rs:483` and `duckdb_runtime.rs:531`. Backend negative tests cover nullable/unsupported/malformed artifacts at `crates/loom-native-melior/tests/production_backend_jit.rs:189`. |
| 10 | Public SQL remains `loom_scan(path)` while native/cache/fallback evidence stays internal. | VERIFIED | SQL gate uses only `loom_scan(...)` queries in `scripts/native-hardening-test.sh:200`; forbidden public marker grep passed for route-specific functions/cache controls/Arrow stream/pushdown/split controls. Public header leakage test passed in `duckdb_runtime_ffi.rs:355`. |
| 11 | Repeated identical SQL scans produce matching rows and observable cache-hit or prepare-reuse smoke evidence. | VERIFIED | `scripts/native-hardening-test.sh` asserts cache miss/insert/hit order at lines 167-170 and aggregate equality at lines 200-231. `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/native-hardening-test.sh` passed. |
| 12 | Projection changes, unsupported payloads, strict fallback, malformed artifacts, cancellation, and mismatch routes emit deterministic diagnostics. | VERIFIED | SQL gate checks projection miss, FSST fallback, strict fail-closed diagnostic code/path, nullable/compressed fallback/fail-closed, cancellation, malformed recovery, and helper mismatch/non-cacheable evidence at `scripts/native-hardening-test.sh:240` through `scripts/native-hardening-test.sh:325`. |
| 13 | The main release gate runs Phase 25 native hardening after Phase 24 and before DuckDB smoke. | VERIFIED | `scripts/mvp0-verify.sh:117` runs Phase 24, `scripts/mvp0-verify.sh:121` runs Phase 25, and `scripts/mvp0-verify.sh:125` runs DuckDB smoke. Order assertion passed. |
| 14 | The final report lists supported equivalence rows, cache invalidation rules, fallback rules, performance smoke evidence, non-goals, and Phase 26 handoff assumptions. | VERIFIED | `25-NATIVE-HARDENING-REPORT.md` contains equivalence matrix, in-process cache design, invalidation, fallback/strict behavior, diagnostics, smoke-not-benchmark language, non-goals, and Phase 26 handoff sections. |
| 15 | Planning docs close Phase 25 without claiming persistent cache, public API controls, new native semantics, table binding, or arbitrary Vortex compatibility. | VERIFIED | `.planning/PROJECT.md:55`, `.planning/STATE.md:30`, and `.planning/ROADMAP.md:737` close Phase 25 and set Phase 26 next. Non-goals are explicit in `25-NATIVE-HARDENING-REPORT.md:141` and `.planning/PROJECT.md:61`. |

**Score:** 15/15 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/loom-core/src/runtime_abi.rs` | Host-neutral cache-key compatibility helper and diagnostics | VERIFIED | Substantive cache compatibility and diagnostic code implementation at lines 318-535. |
| `crates/loom-core/tests/runtime_cache_key.rs` | Cache identity and compatibility mutation tests | VERIFIED | Tests cover deterministic key, artifact/facts, solver/backend/toolchain, query/policy, hit/miss/mismatch. |
| `crates/loom-core/tests/runtime_execution_policy.rs` | Stable strict/fallback policy diagnostics | VERIFIED | Tests passed and cover fail-closed, fallback, predicate, split policy behavior. |
| `crates/loom-ffi/src/duckdb_runtime.rs` | Rust-owned in-process native preparation cache and diagnostics | VERIFIED | Cache storage, lookup, insertion, non-cacheable diagnostics, runtime-cache-key construction, and FFI diagnostic mapping exist. |
| `crates/loom-ffi/tests/duckdb_runtime_cache.rs` | Cache hit/miss/invalidation/non-cacheable route tests | VERIFIED | Seven tests passed; includes miss/insert/hit, projection/policy drift, key mismatch, unsafe routes, post-error replay. |
| `crates/loom-ffi/tests/duckdb_runtime_ffi.rs` | Internal FFI visibility and public header non-leakage | VERIFIED | FFI tests passed; public `loom.h` excludes `loom_duckdb_` and route/cache symbols. |
| `crates/loom-ffi/tests/duckdb_runtime.rs` | Helper equivalence and fallback matrix | VERIFIED | Equivalence and prepare-route tests passed. |
| `crates/loom-native-melior/tests/production_backend_jit.rs` | Backend mismatch/cancel/toolchain diagnostics | VERIFIED | Eight tests passed; covers cancellation, unsupported/nullable primitive shapes, malformed artifacts, output mismatch. |
| `scripts/native-hardening-test.sh` | SQL/cache/fallback release gate | VERIFIED | Syntax check passed; full Phase 25 gate passed through DuckDB public SQL. |
| `scripts/mvp0-verify.sh` | Main release-gate wiring | VERIFIED | Phase 25 gate is invoked after Phase 24 and before DuckDB smoke; full captured gate log ended with `=== MVP0 release gate PASSED ===`. |
| `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-NATIVE-HARDENING-REPORT.md` | Final bounded hardening report | VERIFIED | Report is 172 lines and contains required sections/non-goals. |
| `.planning/PROJECT.md`, `.planning/STATE.md`, `.planning/ROADMAP.md` | Phase closeout and Phase 26 handoff | VERIFIED | Docs mark Phase 25 complete and Phase 26 next while preserving non-goals. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `runtime_abi.rs` | `duckdb_runtime.rs` | `RuntimeCacheKey`, `RuntimeDiagnosticCode::CacheKeyMismatch` | WIRED | `duckdb_runtime.rs:32` imports runtime cache vocabulary and maps diagnostics at `duckdb_runtime.rs:1450`. |
| `duckdb_runtime.rs` | `loom-native-melior` backend | Accepted `NativeBackendReport` preparation evidence | WIRED | Imports backend report/status at `duckdb_runtime.rs:39`; caches only accepted diagnostic-free reports at `duckdb_runtime.rs:1313`. |
| `duckdb_runtime.rs` | `loom.h` public boundary | Internal cache diagnostics excluded from public header | WIRED | FFI tests verify diagnostic accessors and public header non-leakage; public grep gate passed. |
| `duckdb_runtime.rs` tests | runtime preparation path | `plan_duckdb_runtime` and `prepare_duckdb_runtime` | WIRED | Helper tests call the actual runtime plan/prepare APIs and passed. |
| `scripts/native-hardening-test.sh` | DuckDB extension | `LOOM_DUCKDB_TEST_ROUTE_REPORT` | WIRED | Script exports route report at line 33; C++ appends the report from env at `duckdb-ext/loom_extension.cpp:266`. |
| `scripts/mvp0-verify.sh` | `scripts/native-hardening-test.sh` | Release gate invocation | WIRED | Invocation exists at `scripts/mvp0-verify.sh:121`; full gate passed. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `duckdb_runtime.rs` cache | `cache_key`, `backend_report` | `RuntimeCacheKey::build` from artifact/facts/lowering/backend/query/policy plus `NativeBackendReport` from backend preparation | Yes | FLOWING - cached entry is reused only after compatibility check and native output comparison. |
| `duckdb_runtime.rs` native buffers | `native_buffers` | `reference_value_buffers` + test or real JIT output + `compare_production_jit_output` | Yes | FLOWING - mismatch returns fail-closed with no buffers. |
| `scripts/native-hardening-test.sh` route report | `LOOM_DUCKDB_TEST_ROUTE_REPORT` | DuckDB extension appends Rust route diagnostics during public `loom_scan(path)` scans | Yes | FLOWING - SQL gate asserted cache/fallback/cancel/mismatch diagnostics and passed. |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Core cache and policy tests | `cargo test -p loom-core --test runtime_cache_key cache && cargo test -p loom-core --test runtime_execution_policy` | 11 tests passed | PASS |
| FFI/helper/backend Phase 25 suites | `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-ffi --test duckdb_runtime_cache && ... duckdb_runtime equivalence_matrix && ... prepare_routes && ... duckdb_runtime_ffi && ... loom-native-melior --test production_backend_jit` | 36 focused tests passed | PASS |
| DuckDB SQL hardening gate | `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/native-hardening-test.sh` | Phase 25 gate passed | PASS |
| Main release gate | `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp0-verify.sh` | Captured log reached Phase 25, DuckDB smoke, and `=== MVP0 release gate PASSED ===` | PASS |
| Public API creep gate | Forbidden route/cache/Arrow stream/predicate/split marker grep | No forbidden public markers found | PASS |

### Probe Execution

No `scripts/**/tests/probe-*.sh` files or Phase 25-declared probe scripts were found. Step 7c: SKIPPED (no probes declared for this phase).

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| PHASE-25 | 25-01 through 25-05 | Roadmap requirement for native equivalence/cache/fallback hardening | SATISFIED | All five plans declare PHASE-25; all 15 plan must-have truths verified. No `.planning/REQUIREMENTS.md` PHASE-25 entry was present to cross-reference. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---:|---|---|---|
| None | - | No unreferenced `TBD`, `FIXME`, or `XXX` markers in inspected Phase 25 files | - | No blocker debt markers found. |
| `duckdb-ext/loom_extension.cpp` | 630 | `crc32 placeholder; Phase 11 v0 does not enforce it` | INFO | Historical Phase 11 container comment, not Phase 25 incomplete work and not involved in native hardening behavior. |

### Human Verification Required

None. The phase goal is programmatically verifiable through Rust tests, shell gates, route-report diagnostics, and planning-document checks.

### Gaps Summary

No blocking gaps found. One direct interactive `mvp0-verify.sh` run initially surfaced a transient `loom-ffi` test failure in `prepare_routes::skipped_or_failed_toolchain_is_diagnostic_only_without_native_buffers`; the same target then passed with default parallelism, passed single-threaded, passed five repeated default runs, and the captured full `mvp0-verify.sh` run completed successfully. This is not carried as a Phase 25 blocker because the final release gate and focused checks are passing, but the test remains a useful watch point if future flakiness appears.

---

_Verified: 2026-06-08T18:36:45Z_
_Verifier: the agent (gsd-verifier)_
