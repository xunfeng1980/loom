# Phase 24: DuckDB Native Execution Integration MVP - Context

**Gathered:** 2026-06-08T15:28:01Z
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 24 proves the first concrete host adapter for Loom native execution:
existing DuckDB `loom_scan(path)` should consume the Phase 22 runtime contract
and Phase 23 production backend report path over complete-reader artifacts.

The phase delivers DuckDB adapter wiring, not a new ABI, not a broader native
backend, and not arbitrary Vortex semantic compatibility. It should preserve the
existing interpreter path and SQL smoke behavior while routing eligible
non-null primitive raw/table artifacts through runtime planning and the Phase 23
native backend seed.
</domain>

<decisions>
## Implementation Decisions

### DuckDB Lifecycle Mapping

- **D-01: Runtime planning happens in DuckDB `Bind`.** `Bind` reads the payload,
  derives schema/column metadata, constructs Phase 22 runtime planning inputs,
  records projection shape when DuckDB exposes it, and builds the
  `RuntimePlan`/`RuntimeCacheKey` inputs needed by later phases.
  - **Tradeoff:** This makes bind heavier and can surface verifier/runtime
    diagnostics before scan startup, but it keeps schema, projection, and
    runtime shape decisions together. It avoids lazy scan-time planning where
    errors, worker state, and output schema would become tangled.
- **D-02: Backend prepare/JIT seed happens in DuckDB global init.** `GlobalInit`
  consumes the bound runtime plan/cache identity and calls the Phase 23 backend
  prepare path only for native candidates. It stores the backend report in scan
  state for the scan function.
  - **Tradeoff:** This keeps LLVM/toolchain/backend work out of bind while still
    failing before rows are produced. It defers less than scan-time lazy prepare
    and avoids repeated per-worker native preparation.
- **D-03: Phase 24 is single-worker / serialized scan.** Do not implement
  parallel splits or per-worker backend preparation in this phase.
  - **Tradeoff:** This delays full concurrency evidence, but keeps Phase 24
    focused on proving the adapter lifecycle. Parallel split execution, local
    worker state, and per-worker cache behavior belong to Phase 25 hardening or
    later execution work.
- **D-04: Phase 24 keeps single-batch scan semantics.** Preserve the existing
  `loom_scan` behavior: fill one `DataChunk` with the decoded table/column and
  then return end-of-scan on the next call.
  - **Tradeoff:** This is not a production streaming scanner, but it minimizes
    state-machine risk and lets the phase isolate runtime/backend integration.
    Chunked output, offsets, batch-local release, and cancellation mid-stream
    stay deferred.

### Output Delivery

- **D-05: Keep direct DuckDB `DataChunk` population as the delivery boundary.**
  Phase 24 should reuse and refactor the existing typed fill helpers rather than
  introduce ArrowArrayStream or a public record-batch ABI.
  - **Tradeoff:** Direct `DataChunk` population is already proven for mixed
    table payloads and keeps the adapter small. It does not validate the future
    ArrowArrayStream/table-batch ABI and may require adapter-specific fill logic,
    but Phase 24's goal is one host integration, not stream ABI design.
- **D-06: Native output should adapt into the same fill path.** The native path
  may produce Phase 23 primitive value-buffer evidence, but DuckDB should see
  the same logical output contract as interpreter output. Planner should prefer
  shared internal helpers for fixed-width primitive columns where practical.
  - **Tradeoff:** Sharing the fill path reduces divergent native/interpreter SQL
    behavior. It may require small adapter structs around native value buffers,
    but avoids exposing native buffer internals as a new public ABI.

### Native Failure SQL Behavior

- **D-07: Runtime policy controls fallback.** If runtime policy permits
  interpreter fallback and backend preparation is skipped/unsupported, DuckDB may
  transparently use the interpreter path. If runtime policy is fail-closed,
  DuckDB should surface a stable error containing runtime/backend diagnostic
  codes and paths.
  - **Tradeoff:** Policy-controlled fallback preserves SQL usability for
    unsupported native cases, but can hide that native was skipped unless tests
    inspect diagnostics. Stable diagnostics and focused tests must make the
    routing visible.
- **D-08: Native output mismatch fails closed.** A Phase 23
  `native-output-mismatch` diagnostic is treated as a correctness-risk signal,
  not as an ordinary unsupported-native skip. Do not silently fallback after a
  mismatch in Phase 24.
  - **Tradeoff:** This is stricter than "always fallback" but avoids masking
    wrong native output. Broader equivalence and cache hardening remain Phase 25.
- **D-09: Host cancellation maps to backend cancellation/error, not partial
  success.** If DuckDB interruption/cancel can be observed in the adapter, it
  should set the Phase 23 `Cancellation` model and return a DuckDB error/abort
  path without emitting partial native output.
  - **Tradeoff:** This keeps safety simple for the single-batch MVP. Fine-grained
    mid-batch cancellation is deferred with chunked scanning.

### Projection, Predicate, And Threading MVP

- **D-10: Prove projection, but not predicate pushdown.** Phase 24 should map
  DuckDB projection/schema decisions into the Phase 22 projection model and
  preserve output column ordering. Predicate envelopes remain `None` for the
  SQL path; DuckDB may filter after `loom_scan`.
  - **Tradeoff:** Projection is closely tied to bind/schema and is worth proving
    now. Predicate pushdown would add optimizer-specific behavior and native
    filter semantics before the backend supports it.
- **D-11: Keep full-scan or single row-range modeling only where already
  available.** The adapter should default to full-scan single worker. If tests
  exercise `SplitDescriptor`, they should do so as runtime planning evidence,
  not as real parallel execution.
  - **Tradeoff:** This respects the Phase 22 split model without committing
    DuckDB to parallel workers too early.
- **D-12: Default DuckDB adapter policy should favor fail-closed native claims
  plus explicit interpreter fallback for unsupported native.** The adapter must
  not call native unless runtime planning returns `NativeCandidate`.
  - **Tradeoff:** This avoids accidental native execution while preserving the
    existing interpreter SQL path. It may require tests to assert both strict and
    fallback routes.

### SQL API Shape

- **D-13: Keep `loom_scan(path)` as the public SQL API for Phase 24.** Native vs
  interpreter routing should be internal and policy-driven. Do not add public
  `loom_scan_native`, `loom_scan_interpreter`, or mode parameters as the primary
  user surface.
  - **Tradeoff:** This keeps the user story stable and avoids API churn while
    the ABI remains unfrozen. It gives users less manual control, so tests or
    internal diagnostics must expose which route was used.
- **D-14: Test-only controls are acceptable if they do not become public API.**
  Planner may add focused test hooks, environment variables, or internal helper
  functions to force strict/fallback behavior, as long as public SQL docs keep
  `loom_scan(path)` as the MVP surface.
  - **Tradeoff:** Test hooks make routing verifiable without widening the SQL
    API. They must be clearly labeled and not documented as stable user features.

### the agent's Discretion

The user selected the recommended path for all remaining gray areas after the
initial lifecycle questions. The planner may choose exact helper names, struct
layout, and test fixture organization, provided the decisions above and the
canonical references are preserved.
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope And State

- `.planning/ROADMAP.md` — Phase 24 boundary, ordering decision, and Phase 25
  deferrals.
- `.planning/STATE.md` — current handoff from Phase 23 to Phase 24.
- `.planning/PROJECT.md` — project value, constraints, and key decisions.
- `.planning/REQUIREMENTS.md` — existing DuckDB/FFI/table/verifier requirements.

### Runtime And Backend Contracts

- `.planning/phases/22-host-native-runtime-abi-and-execution-policy/22-CONTEXT.md`
  — locked runtime ABI decisions and non-goals.
- `.planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RUNTIME-ABI-CONTRACT.md`
  — runtime lifecycle and host-neutral contract.
- `.planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RUNTIME-ABI-REPORT.md`
  — delivered runtime model, policy, cache, and Phase 24 handoff.
- `.planning/phases/23-production-native-backend-implementation/23-CONTEXT.md`
  — backend boundary and Phase 24 non-goals.
- `.planning/phases/23-production-native-backend-implementation/23-BACKEND-CONTRACT.md`
  — backend request/report/cancellation/cache contract.
- `.planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md`
  — implemented backend surface, supported/deferred kernels, and DuckDB handoff.

### DuckDB Adapter Code

- `duckdb-ext/loom_extension.cpp` — existing `loom_scan` bind/init/scan,
  container/table parsing, direct `DataChunk` population, and Arrow release
  ownership.
- `duckdb-ext/CMakeLists.txt` — current C++ extension build and Rust staticlib
  link pattern.
- `scripts/duckdb-smoke-test.sh` — current SQL smoke gate shape.
- `scripts/mvp0-verify.sh` — top-level release gate that Phase 24 should extend.

### Rust Runtime, FFI, And Backend Code

- `crates/loom-core/src/runtime_abi.rs` — `RuntimePlan`, projection/predicate
  split planning, decisions, diagnostics, and `RuntimeCacheKey`.
- `crates/loom-core/src/artifact_verifier.rs` — accepted artifact/facts contract
  feeding runtime planning.
- `crates/loom-core/src/production_native_lowering.rs` — supported native
  lowering facts and kernel vocabulary.
- `crates/loom-native-melior/src/backend.rs` — `NativeBackendRequest`,
  identity, diagnostics, cancellation, and reports.
- `crates/loom-native-melior/src/pipeline.rs` — backend prepare and MLIR/LLVM
  validation report path.
- `crates/loom-native-melior/src/jit.rs` — production JIT seed and output
  comparison diagnostics.
- `crates/loom-ffi/src/ffi.rs` — current `loom_decode` interpreter FFI entry.
- `crates/loom-ffi/include/loom.h` — stable current decode header.
- `crates/loom-ffi/include/loom_runtime.h` — non-frozen runtime ABI sketch.
</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `duckdb-ext/loom_extension.cpp` already has `LoomBindData`, `LoomScanState`,
  `LoomBind`, `LoomInit`, `LoomScan`, and typed `Fill*Vector` helpers that can
  be refactored into interpreter/native shared fill paths.
- `crates/loom-core/src/runtime_abi.rs` already models projection,
  predicate-envelope policy, split descriptors, concurrency policy, runtime
  decisions, diagnostics, and cache keys.
- `crates/loom-native-melior/src/pipeline.rs` already exposes
  `validate_and_prepare_production_backend` for a Phase 23 backend report.
- `crates/loom-native-melior/src/jit.rs` already exposes
  `execute_prepared_production_jit` and native output comparison diagnostics.
- `crates/loom-ffi/src/ffi.rs` remains the interpreter-oriented Arrow C Data
  path and should not be replaced just to prove Phase 24.

### Established Patterns

- The C++ extension currently owns Arrow array/schema release in
  `LoomScanState::~LoomScanState` and never transfers those structs out of the
  state.
- `loom_scan` currently parses `LMC1`/`LMT1` enough in bind to declare DuckDB
  schema, then decodes each column in init and fills direct vectors in scan.
- Release gates are phase-specific scripts wired into `scripts/mvp0-verify.sh`;
  Phase 24 should add a DuckDB-native integration gate rather than overloading
  backend-only tests.
- Default workspace behavior should remain usable without requiring LLVM/MLIR
  unless strict native evidence is explicitly requested.

### Integration Points

- Add runtime planning fields to DuckDB bind data or a dedicated adapter struct
  produced by `LoomBind`.
- Add backend report/native execution state to global scan state created by
  `LoomInit`.
- Keep scan function as the only place that writes DuckDB `DataChunk`s.
- Extend SQL smoke tests with route-aware cases: native-eligible primitive table,
  fallback/unsupported artifact, fail-closed strict diagnostic, and Arrow release
  on error/cancel where practical.
</code_context>

<specifics>
## Specific Ideas

- Keep `loom_scan(path)` stable for users; native execution is an internal route
  selected by verifier/runtime/backend facts.
- Treat Phase 24 as "adapter proof first": `Bind` plans, `GlobalInit` prepares,
  `Scan` emits one batch.
- Record route diagnostics in tests so transparent fallback does not become
  invisible.
- Do not widen native support beyond Phase 23's non-null primitive raw/table
  evidence.
</specifics>

<deferred>
## Deferred Ideas

- Chunked `DataChunk` output and ArrowArrayStream/record-batch public ABI.
- Parallel split execution, `LocalTableFunctionState`, and per-worker native
  cache behavior.
- Predicate pushdown into runtime/native execution.
- Public SQL mode knobs such as `loom_scan_native`, `loom_scan_interpreter`, or
  `loom_scan(path, mode := ...)`.
- Persistent native artifact cache reuse/invalidation and broad equivalence
  matrices, reserved for Phase 25.
</deferred>

---

*Phase: 24-DuckDB Native Execution Integration MVP*
*Context gathered: 2026-06-08T15:28:01Z*
