# Phase 24: DuckDB Native Execution Integration MVP - Research

**Researched:** 2026-06-08 [VERIFIED: system date]
**Domain:** DuckDB C++ table-function adapter over Loom runtime policy and native backend [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]
**Confidence:** HIGH for local architecture and adapter lifecycle; MEDIUM for exact DuckDB optimizer behavior beyond the vendored 1.5.3 header [VERIFIED: codebase grep] [CITED: duckdb-ext/vendor/duckdb-src/duckdb.hpp]

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
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

### Deferred Ideas (OUT OF SCOPE)
## Deferred Ideas

- Chunked `DataChunk` output and ArrowArrayStream/record-batch public ABI.
- Parallel split execution, `LocalTableFunctionState`, and per-worker native
  cache behavior.
- Predicate pushdown into runtime/native execution.
- Public SQL mode knobs such as `loom_scan_native`, `loom_scan_interpreter`, or
  `loom_scan(path, mode := ...)`.
- Persistent native artifact cache reuse/invalidation and broad equivalence
  matrices, reserved for Phase 25.
</user_constraints>

## Summary

Phase 24 should be planned as a thin DuckDB adapter over the Phase 22 runtime policy and Phase 23 backend report path, not as a second runtime ABI or a native compiler phase. [CITED: .planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RUNTIME-ABI-REPORT.md] [CITED: .planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md] The existing `duckdb-ext/loom_extension.cpp` already has `LoomBind`, `LoomInit`, `LoomScan`, direct `DataChunk` population, and release-safe Arrow C Data teardown, so the planner should refactor around that lifecycle instead of replacing it. [VERIFIED: codebase grep]

The key implementation gap is a Rust-side orchestration helper that C++ can call without freezing `loom_runtime.h`: parse/verify the artifact, derive lowering facts when available, construct `RuntimePlan` and `RuntimeCacheKey`, run backend prepare only for `NativeCandidate`, expose stable route diagnostics, and optionally return native primitive value-buffer evidence for the scan fill path. [VERIFIED: codebase grep] [CITED: crates/loom-core/src/runtime_abi.rs] [CITED: crates/loom-native-melior/src/backend.rs] This helper may live behind an internal `extern "C"` or C++-only bridge, but it must not become a public `loom_runtime.h` commitment. [CITED: .planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md]

**Primary recommendation:** implement a small internal DuckDB adapter bridge: `Bind` reads schema and creates runtime plan/cache inputs, `GlobalInit` prepares native or interpreter state according to runtime policy, `Scan` emits one direct `DataChunk`, and tests assert route diagnostics for native, fallback, strict fail-closed, release, error, and cancellation paths. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md] [CITED: duckdb-ext/loom_extension.cpp]

## Project Constraints (from AGENTS.md)

- Use Rust for decoder core and Arrow via arrow-rs; C++ DuckDB extension remains the thinnest host wrapper. [CITED: AGENTS.md]
- Rust to C++ interop uses the Arrow C Data Interface; direct output remains Arrow-compatible and release-callback owned. [CITED: AGENTS.md] [CITED: https://arrow.apache.org/docs/format/CDataInterface.html]
- `loom-core` and `loom-ffi` must remain Vortex-free; Vortex crates are allowed only in fixture/ingress boundaries already scoped by the project. [CITED: AGENTS.md]
- MVP1 favors narrow, verifier-gated, fail-closed vertical slices over broad format coverage or unverified execution paths. [CITED: AGENTS.md]
- GSD workflow discipline applies to repo edits; this research artifact is produced as the Phase 24 planning input requested by the orchestrator. [CITED: AGENTS.md]
- No project-defined skills were present under `.codex/skills/` or `.agents/skills/` during research. [VERIFIED: codebase grep]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| SQL surface `loom_scan(path)` | DuckDB C++ extension | Rust FFI bridge | DuckDB owns table-function registration, bind/init/scan callbacks, and `DataChunk` output; Rust owns verification/runtime/backend decisions. [CITED: duckdb-ext/loom_extension.cpp] |
| Artifact verification and facts | Rust core | DuckDB bind adapter | `verify_artifact` and production-lowering support live in `loom-core`; DuckDB should pass bytes and surface diagnostics. [CITED: crates/loom-core/src/artifact_verifier.rs] |
| Runtime plan and fallback policy | Rust core | DuckDB bind/global init | `decide_runtime_execution`, projection, predicate, split, policy, and cache-key types live in `loom_core::runtime_abi`. [CITED: crates/loom-core/src/runtime_abi.rs] |
| Native backend prepare/JIT seed | `loom-native-melior` Rust crate | DuckDB global init | Backend validation and preparation consume `RuntimePlan` and `RuntimeCacheKey`; they do not decide fallback. [CITED: crates/loom-native-melior/src/backend.rs] [CITED: crates/loom-native-melior/src/pipeline.rs] |
| DataChunk population | DuckDB C++ extension | Rust helper output structs | Phase 24 keeps direct `DataChunk` population and should reuse fixed-width fill helpers for interpreter and native primitive buffers. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md] |
| Arrow release ownership | C++ scan state RAII | Rust `arrow::ffi::to_ffi` | Current scan state releases each `ArrowArray` and `ArrowSchema`; Arrow C Data requires producer release callbacks. [CITED: duckdb-ext/loom_extension.cpp] [CITED: https://arrow.apache.org/docs/format/CDataInterface.html] |
| Route diagnostics | Rust bridge report | DuckDB exception/test hooks | Stable runtime/backend diagnostic codes already exist and must be visible without public SQL mode knobs. [CITED: crates/loom-core/src/runtime_abi.rs] [CITED: crates/loom-native-melior/src/backend.rs] |

## Standard Stack

### Core

| Library / Component | Version | Purpose | Why Standard |
|---------------------|---------|---------|--------------|
| DuckDB C++ extension API | 1.5.3 vendored header and CLI | `TableFunction` registration plus bind/global init/local init/scan callbacks | Existing project host path and smoke gate are pinned to DuckDB 1.5.3. [VERIFIED: codebase grep] |
| `loom-core` | workspace `0.1.0` | Artifact verification, production-lowering facts, runtime policy, projection/split/cache models | Host-neutral trust and fallback logic already lives here. [VERIFIED: cargo metadata] |
| `loom-native-melior` | workspace `0.1.0` | Backend request/report model, MLIR/LLVM validation, JIT seed, native diagnostics | Phase 23 completed this as the backend consumed by Phase 24. [VERIFIED: cargo metadata] [CITED: .planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md] |
| `loom-ffi` | workspace `0.1.0` | Current `loom_decode` interpreter FFI and optional internal bridge home | Existing C ABI already handles Arrow C Data export and panic containment. [VERIFIED: cargo metadata] [CITED: crates/loom-ffi/src/ffi.rs] |
| Apache Arrow C Data Interface | Arrow spec v24 docs; Rust crate `arrow = 58.3.0` in workspace | Cross-language array/schema ownership and release callback model | Project already uses arrow-rs `to_ffi` and C++ release callbacks. [CITED: https://arrow.apache.org/docs/format/CDataInterface.html] [CITED: https://docs.rs/arrow/58.3.0/arrow/ffi/index.html] |

### Supporting

| Component | Version | Purpose | When to Use |
|-----------|---------|---------|-------------|
| `scripts/duckdb-smoke-test.sh` | local script | Builds extension, generates fixtures, runs SQL acceptance | Extend or pair with a Phase 24 route-aware SQL gate. [VERIFIED: codebase grep] |
| `scripts/production-backend-test.sh` | local script | Gates backend contract, pipeline, JIT seed, ODS validation | Keep as backend-only evidence and call from release gate before DuckDB native integration tests. [VERIFIED: codebase grep] |
| `scripts/runtime-abi-test.sh` | local script | Gates runtime policy/projection/split/cache and host-neutral ABI sketch | Keep as precondition evidence for adapter tests. [VERIFIED: codebase grep] |
| DuckDB `projection_pushdown` / `TableFunctionInitInput::column_ids` | vendored 1.5.3 C++ header | Receives projected column ids during init when projection pushdown is enabled | Use to map DuckDB selected columns to `ProjectionSet::Columns`. [CITED: duckdb-ext/vendor/duckdb-src/duckdb.hpp] |
| `GlobalTableFunctionState::MaxThreads()` | vendored 1.5.3 C++ header | Caps scan parallelism | Override to `1` for Phase 24 single-worker execution. [CITED: duckdb-ext/vendor/duckdb-src/duckdb.hpp] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Direct `DataChunk` fill | ArrowArrayStream | Deferred by user decision; stream ABI would expand public/output surface before the host adapter is proven. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md] |
| Internal Rust helper/FFI | Freeze `loom_runtime.h` now | Phase 23 explicitly leaves `loom_runtime.h` unfrozen; freezing now would overcommit the ABI before Phase 25 hardening and future hosts. [CITED: .planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md] |
| Policy-owned fallback | DuckDB exception-driven fallback | Phase 22 states fallback is policy-controlled and not host text dependent. [CITED: .planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RUNTIME-ABI-CONTRACT.md] |
| Single worker/single batch | Parallel splits and chunked scan | Deferred by context to avoid combining adapter proof with concurrency/cache hardening. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md] |

**Installation:** no new packages should be installed for Phase 24. [VERIFIED: codebase grep]

```bash
# Use existing workspace crates and vendored DuckDB assets.
cargo build -p loom-ffi --release
cmake -S duckdb-ext -B duckdb-ext/build -DCMAKE_BUILD_TYPE=Release
cmake --build duckdb-ext/build
```

**Version verification:** existing relevant versions were verified locally with `cargo metadata`, `rustc --version`, `cargo --version`, and the cached DuckDB CLI. [VERIFIED: shell probe]

## Package Legitimacy Audit

Phase 24 should not add external packages. [VERIFIED: codebase grep] The package legitimacy gate is therefore not required for a new install set. [VERIFIED: no new package recommendation]

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| none | — | — | — | — | — | No package install recommended. [VERIFIED: no new package recommendation] |

**Packages removed due to slopcheck [SLOP] verdict:** none. [VERIFIED: no package install recommendation]
**Packages flagged as suspicious [SUS]:** none. [VERIFIED: no package install recommendation]

## Architecture Patterns

### System Architecture Diagram

```text
SQL query
  |
  v
DuckDB binder: loom_scan(path)
  |-- read LMC1/LMP1/LMT1 bytes and declare schema
  |-- derive initial projection shape / table columns
  |-- call internal Rust planning helper
  v
Rust runtime planning helper
  |-- verify artifact and facts
  |-- check production-lowering support
  |-- decide native / interpreter / fail-closed
  |-- build RuntimeCacheKey
  v
DuckDB global init
  |-- if NativeCandidate: call loom-native-melior prepare/JIT seed
  |-- if InterpreterFallback: call existing loom_decode path per selected column
  |-- if FailClosed: throw stable diagnostic-bearing DuckDB error
  v
DuckDB scan function
  |-- single worker, single batch
  |-- fill DataChunk from native primitive buffers or Arrow C Data buffers
  |-- set batch_emitted and return empty chunk on next call
  v
RAII teardown
  |-- release ArrowArray / ArrowSchema exactly once
  |-- discard route/backend state
```

This flow follows the Phase 22 lifecycle of plan, prepare, open scan/worker, next batch, close, while adapting it to DuckDB’s bind/init/scan callbacks. [CITED: .planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RUNTIME-ABI-CONTRACT.md] [CITED: duckdb-ext/vendor/duckdb-src/duckdb.hpp]

### Recommended Project Structure

```text
crates/loom-ffi/src/
├── ffi.rs                  # existing loom_decode interpreter FFI
└── duckdb_runtime.rs        # internal Phase 24 planning/route helper if needed

crates/loom-ffi/include/
├── loom.h                  # existing stable decode header
└── loom_duckdb_internal.h   # optional internal, explicitly non-public adapter header

duckdb-ext/
├── loom_extension.cpp      # keep public loom_scan(path), add route state and projection mapping
└── CMakeLists.txt          # keep Rust staticlib link; add generated/internal header only if needed

scripts/
├── duckdb-smoke-test.sh
└── duckdb-native-integration-test.sh
```

The exact helper names are at the planner’s discretion; the important boundary is that public `loom_runtime.h` remains unfrozen and `loom_scan(path)` remains the SQL API. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]

### Pattern 1: Bind Owns Schema And Runtime Planning Inputs

**What:** keep `LoomBind` responsible for path validation, file read, container/table schema discovery, DuckDB return names/types, and creating the runtime plan/cache input. [CITED: duckdb-ext/loom_extension.cpp] [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]

**When to use:** always for `loom_scan(path)` because DuckDB requires return schema during bind and Phase 24 locks planning to bind. [CITED: duckdb-ext/vendor/duckdb-src/duckdb.hpp]

**Example:**

```cpp
// Source: duckdb-ext/vendor/duckdb-src/duckdb.hpp and duckdb-ext/loom_extension.cpp
// Planner target: keep FunctionData immutable after bind.
struct LoomBindData : TableFunctionData {
    string payload_path;
    vector<uint8_t> payload;
    vector<string> column_names;
    vector<LogicalType> column_types;
    vector<LoomValueKind> column_kinds;
    // Add runtime route summary/cache identity here, not mutable scan state.
};
```

### Pattern 2: Enable Projection Pushdown And Map Init Column IDs

**What:** set `fn.projection_pushdown = true`, then read `TableFunctionInitInput::column_ids` / `projection_ids` in global init to derive selected columns and output order. [CITED: duckdb-ext/vendor/duckdb-src/duckdb.hpp]

**When to use:** for Phase 24 projection proof only; leave predicates as `PredicateEnvelope::None`. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]

**Example:**

```cpp
// Source: DuckDB 1.5.3 vendored header: TableFunctionInitInput has column_ids and projection_ids.
static unique_ptr<GlobalTableFunctionState> LoomInit(ClientContext &, TableFunctionInitInput &input) {
    const auto &bind = input.bind_data->Cast<LoomBindData>();
    // Map input.column_ids to Runtime ProjectionSet::Columns(source_index, output_index).
    // Keep MaxThreads() == 1 on the returned GlobalTableFunctionState.
}
```

### Pattern 3: Global Init Prepares Native But Does Not Emit Rows

**What:** global init should call backend prepare only after runtime planning returns native candidate, store route/backend state, and still leave `LoomScan` as the only row emitter. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]

**When to use:** for native-eligible non-null primitive raw/table artifacts. [CITED: .planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md]

**Example:**

```rust
// Source: crates/loom-native-melior/src/pipeline.rs
let report = validate_and_prepare_production_backend(input, ProductionBackendPipelineOptions::default());
// DuckDB adapter consumes report.status + diagnostics; fallback remains runtime-policy-owned.
```

### Pattern 4: Shared Fixed-Width Fill Path

**What:** factor current Arrow-buffer fixed-width vector filling so interpreter Arrow arrays and native primitive value buffers can converge at the `DataChunk` population boundary. [CITED: duckdb-ext/loom_extension.cpp]

**When to use:** Int32, Int64, Float32, and Float64 non-null primitive native output. [CITED: crates/loom-core/src/production_native_lowering.rs]

**Example:**

```cpp
// Source: duckdb-ext/loom_extension.cpp
template <class T>
static void FillFixedWidthVector(const ArrowArray &arr, Vector &vec, idx_t count, const char *kind);

// Planner target: add a sibling helper for raw contiguous native value bytes.
template <class T>
static void FillFixedWidthNativeBytes(const uint8_t *bytes, Vector &vec, idx_t count);
```

### Anti-Patterns to Avoid

- **Re-deciding native eligibility in C++:** DuckDB should not duplicate verifier, solver, lowering, or fallback rules. [CITED: .planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RUNTIME-ABI-CONTRACT.md]
- **Calling backend after interpreter fallback:** `validate_backend_request` rejects non-native runtime plans and diagnostic-bearing plans. [CITED: crates/loom-native-melior/src/backend.rs]
- **Silently falling back after `native-output-mismatch`:** context locks mismatch as fail-closed. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]
- **Adding public SQL mode parameters:** public surface remains `loom_scan(path)`. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]
- **Freezing `loom_runtime.h`:** Phase 23 explicitly says it remains unfrozen. [CITED: .planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md]
- **Holding Arrow C Data without release:** Arrow C Data release callbacks are mandatory and current RAII must stay intact. [CITED: https://arrow.apache.org/docs/format/CDataInterface.html] [CITED: duckdb-ext/loom_extension.cpp]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Runtime eligibility and fallback | C++ switch over artifact kind and env vars | `loom_core::runtime_abi::decide_runtime_execution` | Runtime policy already encodes accepted facts, solver state, lowering, projection, split, concurrency, and fallback. [CITED: crates/loom-core/src/runtime_abi.rs] |
| Cache identity | DuckDB debug string or file path hash | `RuntimeCacheKey::build` | Cache identity must include ABI, artifact, facts, solver, lowering, backend identity, projection, predicate, split, and policy. [CITED: crates/loom-core/src/runtime_abi.rs] |
| Backend preparation | Direct MLIR/toolchain calls from C++ | `validate_and_prepare_production_backend` | Backend report owns toolchain, pipeline, artifact, cancellation, and diagnostics. [CITED: crates/loom-native-melior/src/pipeline.rs] |
| Native output comparison | Ad hoc byte equality in C++ | `compare_production_jit_output` or Rust-side helper | Backend already has stable `native-output-mismatch` diagnostics. [CITED: crates/loom-native-melior/src/jit.rs] |
| Arrow ownership | Manual free or copied Arrow structs | Arrow release callbacks and RAII scan state | C Data private data lifetime is handled by producer release callbacks. [CITED: https://arrow.apache.org/docs/format/CDataInterface.html] |
| Projection validation | DuckDB-only output-index remapping | `plan_projection` | Runtime projection validation already rejects duplicate/missing/out-of-range output mappings. [CITED: crates/loom-core/src/runtime_abi.rs] |

**Key insight:** the hard part is not invoking native code; it is preserving the trust chain from verified artifact facts through runtime policy and backend diagnostics while DuckDB remains a small host adapter. [CITED: .planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RUNTIME-ABI-CONTRACT.md] [CITED: .planning/phases/23-production-native-backend-implementation/23-BACKEND-CONTRACT.md]

## Common Pitfalls

### Pitfall 1: Planning Against Bind-Time Projection Only

**What goes wrong:** DuckDB return schema is known in bind, but concrete projected column ids are exposed through init inputs when projection pushdown is enabled. [CITED: duckdb-ext/vendor/duckdb-src/duckdb.hpp]
**Why it happens:** the context says bind records projection shape "when DuckDB exposes it", but the 1.5.3 C++ API exposes `column_ids` and `projection_ids` on `TableFunctionInitInput`. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md] [CITED: duckdb-ext/vendor/duckdb-src/duckdb.hpp]
**How to avoid:** bind should create schema and default/all-column planning inputs; global init should finalize projection from `input.column_ids` and rebuild or validate the runtime plan/cache if needed. [VERIFIED: codebase grep]
**Warning signs:** native route ignores SQL projection or emits unrequested columns. [ASSUMED]

### Pitfall 2: Native Route Becomes Invisible

**What goes wrong:** fallback succeeds and SQL output passes, but tests cannot tell whether native was attempted, skipped, or failed. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]
**Why it happens:** public SQL API has no mode parameter by design. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]
**How to avoid:** add test-only diagnostics via an internal helper, env-gated log/report file, or a non-public C++ test hook. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]
**Warning signs:** release gate only asserts result rows and never asserts `native-candidate`, `interpreter-fallback`, `fail-closed`, `toolchain-skipped`, or `native-output-mismatch`. [VERIFIED: codebase grep]

### Pitfall 3: Treating Skipped Toolchain As Ordinary Success

**What goes wrong:** missing MLIR/LLVM tooling returns a skip-aware backend report, but strict native tests pass as if native executed. [CITED: scripts/production-backend-test.sh] [CITED: crates/loom-native-melior/src/jit.rs]
**Why it happens:** Phase 23 allows `LOOM_ALLOW_NATIVE_TOOL_SKIP=1` for release-gate portability. [CITED: .planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md]
**How to avoid:** separate route-selection tests from strict-native evidence tests; strict-native tests should require compatible tools or assert explicit skip diagnostics. [VERIFIED: shell probe]
**Warning signs:** a native-eligible SQL test only runs with `LOOM_ALLOW_NATIVE_TOOL_SKIP=1` and never checks the backend report status. [ASSUMED]

### Pitfall 4: Losing Arrow Release On Error/Cancellation

**What goes wrong:** a thrown `IOException`, cancellation, or route failure leaks already-decoded Arrow arrays or double-releases them. [CITED: duckdb-ext/loom_extension.cpp]
**Why it happens:** global init currently decodes columns in a loop and owns arrays in `LoomScanState`; any new route state must preserve RAII for partial initialization. [CITED: duckdb-ext/loom_extension.cpp]
**How to avoid:** keep arrays in RAII state immediately after each successful `loom_decode`, release in the destructor, and never read output structs after nonzero return codes. [CITED: crates/loom-ffi/src/ffi.rs] [CITED: duckdb-ext/loom_extension.cpp]
**Warning signs:** raw `ArrowArray` locals are written before ownership is transferred into scan state. [ASSUMED]

### Pitfall 5: Freezing The C ABI Accidentally

**What goes wrong:** planner adds public functions to `loom_runtime.h` or documents them as stable. [CITED: .planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md]
**Why it happens:** C++ needs a callable helper, but Phase 23 says public runtime ABI is unfrozen. [CITED: .planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md]
**How to avoid:** put any Phase 24 bridge behind an internal header such as `loom_duckdb_internal.h`, with comments that it is non-public and test-host specific. [ASSUMED]
**Warning signs:** README documents a new `loom_runtime_*` function as stable. [ASSUMED]

## Code Examples

### Register `loom_scan` With Projection Pushdown

```cpp
// Source: DuckDB 1.5.3 vendored C++ header; TableFunction has projection_pushdown.
static void LoadInternal(ExtensionLoader &loader) {
    TableFunction fn("loom_scan", {LogicalType::VARCHAR}, LoomScan, LoomBind, LoomInit);
    fn.projection_pushdown = true;
    loader.RegisterFunction(fn);
}
```

### Cap Threads At One

```cpp
// Source: DuckDB 1.5.3 vendored C++ header; GlobalTableFunctionState::MaxThreads defaults to 1.
struct LoomScanState : GlobalTableFunctionState {
    idx_t MaxThreads() const override {
        return 1;
    }
};
```

### Runtime Projection Mapping Shape

```rust
// Source: crates/loom-core/src/runtime_abi.rs
let projection = ProjectionSet::Columns(vec![
    ProjectionColumn { source_index: 0, output_index: 0 },
]);
let planned = plan_projection(&projection, column_count)?;
```

### Backend Prepare Path

```rust
// Source: crates/loom-native-melior/src/pipeline.rs
let report = validate_and_prepare_production_backend(
    NativeBackendRequestInput {
        runtime_plan,
        runtime_cache_key: Some(runtime_cache_key),
        lowering_facts: Some(lowering_facts),
        backend_identity: NativeBackendIdentity::preflight_only(),
        cancellation,
    },
    ProductionBackendPipelineOptions::default(),
);
```

### Fail-Closed Native Output Mismatch

```rust
// Source: crates/loom-native-melior/src/jit.rs
compare_production_jit_output(&report, &expected_value_buffers, &native_output)?;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| DuckDB extension directly calls `loom_decode` for every column | DuckDB adapter should call runtime planning first, then native backend only for native candidates, otherwise interpreter fallback by policy | Phase 24 planning after Phase 22/23 | Prevents DuckDB from owning trust/fallback decisions. [CITED: .planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RUNTIME-ABI-REPORT.md] |
| Backend tests prove native path in isolation | Host integration must prove SQL route selection and direct `DataChunk` output | Phase 24 | Native evidence becomes observable through DuckDB without adding public SQL modes. [CITED: .planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md] |
| Projection accepted at DuckDB SQL level only | Projection should feed Phase 22 `ProjectionSet` and cache key | Phase 22/24 | Cache and backend identity include output shape. [CITED: crates/loom-core/src/runtime_abi.rs] |
| Arrow C Data as only batch handoff concept | Phase 24 may use host-native direct vector fill while preserving Arrow release for interpreter path | Phase 22/24 | Fits user decision for direct `DataChunk` delivery. [CITED: .planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RUNTIME-ABI-CONTRACT.md] |

**Deprecated/outdated:**
- ArrowArrayStream for Phase 24 output is out of scope because direct `DataChunk` population is locked. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]
- Public `loom_scan_native` or SQL mode parameters are out of scope. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]
- Treating `loom_runtime.h` as frozen is out of scope. [CITED: .planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Warning signs and helper naming recommendations are inferred implementation guidance, not verified behavior. | Common Pitfalls / Recommended Project Structure | Planner may need to adjust exact hook names after implementation inspection. |
| A2 | An internal `loom_duckdb_internal.h` is an acceptable way to expose a non-public helper if C++ needs it. | Architecture Patterns / Pitfall 5 | Planner may choose a different bridge mechanism, such as keeping all orchestration inside existing `loom.h` build plumbing without a new header. |

## Open Questions

1. **Where should route diagnostics be exposed for tests?**
   - What we know: public SQL API must stay `loom_scan(path)`, and test-only controls are allowed. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]
   - What's unclear: whether the implementation should use env-gated stderr/report files, a private SQL/debug function, or a C++ unit-test hook. [ASSUMED]
   - Recommendation: prefer an internal env-gated JSON/text report consumed only by `scripts/duckdb-native-integration-test.sh`, because it avoids public SQL API expansion. [ASSUMED]

2. **Can DuckDB interruption be observed directly in this extension path?**
   - What we know: Phase 23 has explicit `NativeBackendCancellation`, and Phase 24 should map host cancellation if observable. [CITED: crates/loom-native-melior/src/backend.rs] [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]
   - What's unclear: the current `LoomScan` signature uses `ClientContext &`, but no cancellation API has been verified in local extension code. [VERIFIED: codebase grep]
   - Recommendation: include cancellation model tests at the Rust helper level, and add DuckDB host cancellation only if a stable 1.5.3 API is found during implementation. [ASSUMED]

3. **How should native value buffers map to actual decoded values?**
   - What we know: Phase 23 JIT seed currently produces deterministic primitive value-buffer evidence and has mismatch diagnostics. [CITED: crates/loom-native-melior/src/jit.rs]
   - What's unclear: whether Phase 24 must execute actual artifact-derived native values or can prove the adapter over current Phase 23 zero-buffer seed while comparing/failing closed. [CITED: .planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md]
   - Recommendation: plan the MVP to call current Phase 23 JIT seed, compare against interpreter/reference buffers, and only emit native buffers when comparison succeeds. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust compiler | Workspace crates | yes | `rustc 1.92.0` | none needed. [VERIFIED: shell probe] |
| Cargo | Workspace tests/build | yes | `cargo 1.92.0` | none needed. [VERIFIED: shell probe] |
| CMake | DuckDB extension build | yes | `4.1.1` | none needed. [VERIFIED: shell probe] |
| Ninja | CMake build generator if used | yes | `1.13.1` | Makefiles may work if CMake config allows. [VERIFIED: shell probe] |
| DuckDB CLI | SQL smoke/integration gate | yes | `v1.5.3 (Variegata) 14eca11bd9` cached under `duckdb-ext/vendor/duckdb-cli/duckdb` | Script downloads/caches when missing. [VERIFIED: shell probe] [CITED: scripts/duckdb-smoke-test.sh] |
| `llvm-config` | Strict native/ODS validation | no | — | Existing native gates support explicit skip with `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`. [VERIFIED: shell probe] [CITED: scripts/production-backend-test.sh] |
| `mlir-opt` | Strict MLIR validation | no | — | Existing native gates support explicit skip with `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`. [VERIFIED: shell probe] [CITED: scripts/production-backend-test.sh] |
| `mlir-tblgen` | Strict ODS validation | no | — | Existing native gates support explicit skip with `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`. [VERIFIED: shell probe] [CITED: scripts/production-backend-test.sh] |
| `mlir-translate` | Strict LLVM translation validation | no | — | Existing native gates support explicit skip with `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`. [VERIFIED: shell probe] [CITED: scripts/production-backend-test.sh] |

**Missing dependencies with no fallback:**
- None for planning or default route/fallback tests. [VERIFIED: shell probe]

**Missing dependencies with fallback:**
- LLVM/MLIR command-line tools are missing on PATH; default planning should keep skip-aware behavior unless strict-native evidence is explicitly required. [VERIFIED: shell probe]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | No auth surface in local DuckDB table-function adapter. [VERIFIED: codebase grep] |
| V3 Session Management | no | No session state or remote service. [VERIFIED: codebase grep] |
| V4 Access Control | limited | Local file path access is inherited from DuckDB process permissions; do not add remote fetch or privilege logic. [CITED: duckdb-ext/loom_extension.cpp] |
| V5 Input Validation | yes | Use existing container/artifact verifier and typed diagnostics before native execution. [CITED: crates/loom-core/src/artifact_verifier.rs] |
| V6 Cryptography | no | No new crypto in Phase 24. [VERIFIED: codebase grep] |
| V8 Data Protection | yes | Preserve fail-closed behavior and do not emit partial native output on cancellation/error. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md] |
| V14 Configuration | yes | Test-only env controls must not become public SQL API or production-stable knobs. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md] |

### Known Threat Patterns for DuckDB Native Adapter

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malformed artifact triggers native route | Tampering | Runtime native candidate requires accepted verification, solver state, lowering support, and supported projection/split/concurrency. [CITED: crates/loom-core/src/runtime_abi.rs] |
| Native/compiler diagnostic hidden by fallback | Repudiation | Record stable runtime/backend route diagnostics in focused tests. [CITED: crates/loom-native-melior/src/backend.rs] |
| Panic or unwind crosses FFI into DuckDB | Denial of Service | Existing `loom_decode` wraps body in `catch_unwind`; any new FFI helper must follow the same pattern. [CITED: crates/loom-ffi/src/ffi.rs] |
| Arrow release leak or double-free | Tampering / DoS | Keep producer release callback ownership and RAII destructor guards. [CITED: https://arrow.apache.org/docs/format/CDataInterface.html] [CITED: duckdb-ext/loom_extension.cpp] |
| Test-only mode becomes public control plane | Elevation of Privilege | Keep controls undocumented as public SQL and scoped to scripts/tests. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md] |

## Validation Architecture

Skipped because `.planning/config.json` sets `workflow.nyquist_validation` to `false`. [VERIFIED: .planning/config.json]

## Recommended Release Gate Shape

| Gate | Command | Purpose |
|------|---------|---------|
| Runtime policy precondition | `bash scripts/runtime-abi-test.sh` | Keeps projection/split/cache/fallback model green. [CITED: scripts/runtime-abi-test.sh] |
| Backend precondition | `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/production-backend-test.sh` | Keeps backend contract, pipeline, and JIT seed green with explicit skip behavior. [CITED: scripts/production-backend-test.sh] |
| Existing SQL smoke | `bash scripts/duckdb-smoke-test.sh` | Preserves interpreter and mixed-table SQL behavior. [CITED: scripts/duckdb-smoke-test.sh] |
| New Phase 24 SQL route gate | `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/duckdb-native-integration-test.sh` | Should assert native-eligible route diagnostics, fallback, strict fail-closed, projection, and Arrow release/error paths. [ASSUMED] |
| Full release gate | `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp0-verify.sh` | Should include the new Phase 24 gate after Phase 23 backend gate and before/with DuckDB smoke. [CITED: scripts/mvp0-verify.sh] |

## Sources

### Primary (HIGH confidence)

- `.planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md` - locked user decisions and deferrals. [VERIFIED: codebase grep]
- `.planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RUNTIME-ABI-CONTRACT.md` - runtime lifecycle and trust boundary. [VERIFIED: codebase grep]
- `.planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RUNTIME-ABI-REPORT.md` - runtime implementation handoff. [VERIFIED: codebase grep]
- `.planning/phases/23-production-native-backend-implementation/23-BACKEND-CONTRACT.md` - backend request/report/cancellation/cache contract. [VERIFIED: codebase grep]
- `.planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md` - supported kernel paths and DuckDB handoff. [VERIFIED: codebase grep]
- `duckdb-ext/loom_extension.cpp` - current DuckDB adapter lifecycle and direct fill helpers. [VERIFIED: codebase grep]
- `duckdb-ext/vendor/duckdb-src/duckdb.hpp` - exact vendored DuckDB 1.5.3 C++ table-function API. [VERIFIED: codebase grep]
- `crates/loom-core/src/runtime_abi.rs` - runtime policy/projection/split/cache model. [VERIFIED: codebase grep]
- `crates/loom-core/src/artifact_verifier.rs` - artifact verification/facts model. [VERIFIED: codebase grep]
- `crates/loom-core/src/production_native_lowering.rs` - current production lowering support. [VERIFIED: codebase grep]
- `crates/loom-native-melior/src/backend.rs`, `pipeline.rs`, `jit.rs` - backend prepare/JIT/mismatch/cancellation model. [VERIFIED: codebase grep]
- `crates/loom-ffi/src/ffi.rs` and `crates/loom-ffi/include/loom.h` - existing FFI/panic/Arrow C Data contract. [VERIFIED: codebase grep]
- `https://duckdb.org/docs/current/clients/c/table_functions.html` - official DuckDB C table-function lifecycle, projection pushdown, init, local init, and max-thread APIs. [CITED: duckdb.org]
- `https://arrow.apache.org/docs/format/CDataInterface.html` - official Arrow C Data Interface release and structure semantics. [CITED: arrow.apache.org]
- `https://docs.rs/arrow/58.3.0/arrow/ffi/index.html` - arrow-rs FFI `to_ffi` / `from_ffi` module docs. [CITED: docs.rs]

### Secondary (MEDIUM confidence)

- `.planning/STATE.md`, `.planning/ROADMAP.md`, `.planning/REQUIREMENTS.md` - phase history, scope, and completed requirement context. [VERIFIED: codebase grep]

### Tertiary (LOW confidence)

- None used for factual claims. [VERIFIED: source review]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new stack is recommended; all core components are existing local code or official DuckDB/Arrow interfaces. [VERIFIED: codebase grep]
- Architecture: HIGH - locked context and existing code define the adapter lifecycle. [CITED: .planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md] [CITED: duckdb-ext/loom_extension.cpp]
- Pitfalls: MEDIUM - release, fallback, ABI, and route-visibility risks are grounded in code/context; exact DuckDB cancellation API remains unverified. [VERIFIED: codebase grep]

**Research date:** 2026-06-08 [VERIFIED: system date]
**Valid until:** 2026-07-08 for local architecture; re-check DuckDB/Arrow docs if upgrading DuckDB or arrow-rs. [ASSUMED]
