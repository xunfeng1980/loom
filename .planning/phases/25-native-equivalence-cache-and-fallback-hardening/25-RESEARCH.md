# Phase 25: Native Equivalence, Cache, and Fallback Hardening - Research

**Researched:** 2026-06-09
**Domain:** Rust runtime/native backend hardening, DuckDB adapter evidence, in-process native cache contract
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

Copied verbatim from `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-CONTEXT.md`. [VERIFIED: repo grep]

### Locked Decisions

## Implementation Decisions

### Equivalence Scope
- Recommended: make interpreter equivalence the primary oracle for native DuckDB output, with existing Vortex/fixture oracle evidence used where already available. Tradeoff: this validates host behavior without claiming arbitrary Vortex semantic compatibility.
- Recommended: cover supported raw non-null primitive single-column and table shapes, projection order, repeated scans, and helper-level native buffer comparison. Tradeoff: this keeps the matrix tied to Phase 23/24 supported shapes instead of widening kernels.
- Recommended: compare real row/value outputs through both Rust helper tests and public `loom_scan(path)` SQL. Tradeoff: SQL catches adapter regressions, while Rust helpers can inject mismatch/cancel/cache cases that SQL cannot naturally trigger.
- Recommended: treat unsupported string/compression/nullable/native-expansion cases as explicit fallback or fail-closed evidence, not as native equivalence targets.

### Cache Contract
- Recommended: introduce a host-neutral, in-process native artifact/cache contract keyed by `RuntimeCacheKey`, backend identity, toolchain identity, lowering facts, projection, predicate, split, policy, and artifact/facts fingerprints. Tradeoff: in-process cache evidence is enough to prove reuse/invalidation semantics without freezing an on-disk cache format.
- Recommended: cache only validated/prepared native artifacts or deterministic preparation evidence, never unchecked native buffers after a failed comparison. Tradeoff: maximizes safety but may limit observed speedup in the MVP slice.
- Recommended: invalidation is key-driven, not path/mtime-driven. Cache hits require exact key equality; mismatches produce deterministic diagnostics and recompute or fail closed by policy.
- Recommended: add test-only counters or reports for cache hit/miss/revalidation evidence under internal hooks. Tradeoff: observability stays internal and does not add public SQL/API controls.

### Fallback And Negative Coverage
- Recommended: preserve Phase 22 policy ownership. DuckDB/C++ must not duplicate native eligibility, cache eligibility, or fallback decisions.
- Recommended: unsupported native paths fall back only when `allow_interpreter_fallback` permits it; strict mode fails closed with stable code/path/message diagnostics.
- Recommended: negative coverage must include cache key mismatch, unsupported lowering facts, toolchain/backend identity drift where representable, native output mismatch, cancellation, malformed artifacts, unsupported projection/predicate/split inputs, and repeated post-error scans.
- Recommended: diagnostics should remain deterministic and route-specific (`cache-key-mismatch`, `fallback-disabled`, `native-output-mismatch`, `cancelled`, toolchain/backend codes). Tradeoff: stable diagnostics may require small helper APIs rather than relying only on SQL stderr strings.

### Release And Performance Evidence
- Recommended: add a dedicated `scripts/native-hardening-test.sh` or equivalent Phase 25 gate, then wire it into `scripts/mvp0-verify.sh` after Phase 24 and before later source/table-format phases.
- Recommended: performance evidence should be a smoke-level proof such as "second identical scan hits cache / avoids prepare counter increment", not a native-speed benchmark claim. Tradeoff: this gives regression signal without overclaiming production performance.
- Recommended: keep public API unchanged (`loom_scan(path)` only) and keep test controls internal/prefixed. Tradeoff: hardening improves confidence without exposing unstable native/cache knobs.
- Recommended: final report must explicitly list supported equivalence matrix rows, cache invalidation rules, fallback rules, non-goals, and remaining Phase 26+ handoff assumptions.

### the agent's Discretion
- Choose the smallest cache representation that can prove reuse and invalidation end-to-end.
- Prefer existing runtime/backend/DuckDB helper patterns over a new abstraction unless repeated cache/fallback logic becomes materially duplicated.
- Keep tradeoffs visible in the Phase 25 plan and final report when choosing narrower evidence over broader native coverage.

### Deferred Ideas (OUT OF SCOPE)

- Persistent on-disk/native artifact cache format.
- Public cache/native/fallback SQL flags or functions.
- Predicate pushdown, parallel split execution, and worker-local scheduling.
- Native execution for strings, nullable primitives, bitpack/FOR expansion, dictionary/RLE, or arbitrary Vortex layouts.
- External source ingress, Lance/Parquet archival readability, Iceberg binding, StarRocks/DuckDB dual query surface, and full Vortex semantic compatibility.
</user_constraints>

## Project Constraints (from AGENTS.md)

- Keep `loom-core` and `loom-ffi` Vortex-free; Vortex crates are allowed only in fixture/oracle/ingress boundaries. [VERIFIED: AGENTS.md]
- Keep the Rust core as the decoder/runtime implementation and the C++ DuckDB extension as the thin host adapter. [VERIFIED: AGENTS.md]
- Keep the Rust/C++ boundary on Arrow C Data Interface and existing internal C ABI surfaces; do not add public route-specific SQL/API for Phase 25. [VERIFIED: AGENTS.md; VERIFIED: `.planning/phases/24-duckdb-native-execution-integration-mvp/24-DUCKDB-NATIVE-REPORT.md`]
- Preserve MVP1 scope discipline: prefer narrow, verifier-gated vertical slices over broad format coverage or unverified execution paths. [VERIFIED: AGENTS.md]
- Do not edit source code during research; this artifact is the only Phase 25 research output. [VERIFIED: user request]
- No project-specific skills were found in `.codex/skills` or `.agents/skills`. [VERIFIED: repo grep]

## Summary

Phase 25 should harden the existing Phase 24 DuckDB native execution path, not widen it. The supported positive equivalence matrix should stay on raw, non-null primitive `LMC1`/`LMT1` shapes and projections already exercised by `native-primitives-table.loom`; unsupported strings, nullable shapes, bitpack/FOR native expansion, dictionary/RLE, predicate pushdown, and split/parallel execution should be negative or fallback evidence. [VERIFIED: `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-CONTEXT.md`; VERIFIED: `scripts/duckdb-native-integration-test.sh`; VERIFIED: `.planning/phases/24-duckdb-native-execution-integration-mvp/24-DUCKDB-NATIVE-REPORT.md`]

The cache contract should be host-neutral and in-process only. The existing `RuntimeCacheKey` already includes artifact digest, artifact facts fingerprint, solver identity, production lowering fingerprint, backend identity, projection, predicate, split, and policy in a canonical string, so Phase 25 should build reuse/invalidation semantics on exact `RuntimeCacheKey` equality rather than path or mtime. [VERIFIED: `crates/loom-core/src/runtime_abi.rs`; VERIFIED: `crates/loom-ffi/src/duckdb_runtime.rs`]

Fallback hardening should preserve Phase 22 policy ownership in Rust. The C++ DuckDB adapter already consumes route/prepared decisions and emits no rows on fail-closed, cancellation, or native mismatch paths; Phase 25 should add cache-specific diagnostics, repeated-scan cache evidence, and stricter unsupported-program matrices while keeping public SQL as `loom_scan(path)`. [VERIFIED: `crates/loom-ffi/src/duckdb_runtime.rs`; VERIFIED: `duckdb-ext/loom_extension.cpp`; VERIFIED: `scripts/duckdb-native-integration-test.sh`]

**Primary recommendation:** Implement a small in-process native artifact cache behind the internal DuckDB runtime bridge, keyed by exact `RuntimeCacheKey`, with Rust helper tests plus SQL evidence proving interpreter/native equivalence, cache hit/miss/invalidation diagnostics, strict/fallback behavior, and smoke-level reuse without making native speed claims. [VERIFIED: repo grep]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Equivalence matrix | Rust helper/tests | DuckDB SQL adapter | Rust can inject mismatch/cancel/cache cases; SQL proves public `loom_scan(path)` row behavior through the host adapter. [VERIFIED: `scripts/duckdb-native-integration-test.sh`] |
| Interpreter oracle | Rust core/FFI | DuckDB SQL | Interpreter output is the primary oracle for native DuckDB output because Phase 24 already compares native buffers to reference output before exposing native buffers. [VERIFIED: `crates/loom-ffi/src/duckdb_runtime.rs`] |
| Vortex/fixture oracle | Fixture/ingress crates | Rust tests | `loom-fixtures` and `loom-vortex-ingress` own Vortex oracle evidence while `loom-core` and `loom-ffi` remain Vortex-free. [VERIFIED: `crates/loom-fixtures/src/oracle.rs`; VERIFIED: `crates/loom-vortex-ingress/tests/single_column_to_loom.rs`; VERIFIED: `scripts/mvp0-verify.sh`] |
| Native cache | Rust internal runtime/FFI | DuckDB C++ test report | Cache eligibility, identity, hit/miss, and invalidation belong beside `prepare_duckdb_runtime` and `RuntimeCacheKey`; C++ should only consume reports. [VERIFIED: `crates/loom-ffi/src/duckdb_runtime.rs`; VERIFIED: `duckdb-ext/loom_extension.cpp`] |
| Fallback/strict policy | Rust runtime ABI | DuckDB C++ error surfacing | `RuntimeSafetyPolicy` owns fallback/concurrency/predicate policy, and DuckDB already reads route decisions instead of reimplementing policy. [VERIFIED: `crates/loom-core/src/runtime_abi.rs`; VERIFIED: `crates/loom-ffi/src/duckdb_runtime.rs`] |
| Public query surface | DuckDB C++ extension | Rust FFI | The host-visible SQL surface remains `loom_scan(VARCHAR)` with projection pushdown enabled; no public native/interpreter mode flags should be added. [VERIFIED: `duckdb-ext/loom_extension.cpp`; CITED: https://duckdb.org/docs/stable/extensions/overview.html] |
| Release gate | Shell scripts | Cargo/CMake/DuckDB CLI | Existing gates run Rust tests, build `loom-ffi`, build the DuckDB extension, run DuckDB SQL, and are wired through `mvp0-verify.sh`. [VERIFIED: `scripts/duckdb-native-integration-test.sh`; VERIFIED: `scripts/mvp0-verify.sh`] |

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PHASE-25 | Harden native equivalence, cache reuse/invalidation, fallback/negative diagnostics, performance smoke evidence, and release-gate wiring before table-format-visible work. [VERIFIED: `.planning/ROADMAP.md`] | Use the equivalence matrix, cache contract, fallback policy, and plan slices in this research. [VERIFIED: repo grep] |
</phase_requirements>

## Standard Stack

### Core

| Library/Crate | Version | Purpose | Why Standard |
|---------------|---------|---------|--------------|
| `loom-core` | workspace path | Runtime ABI, `RuntimeCacheKey`, execution policy, projection/predicate/split planning, diagnostics. | Owns host-neutral runtime decisions and must remain the source of policy truth. [VERIFIED: `crates/loom-core/src/runtime_abi.rs`] |
| `loom-ffi` | workspace path | Internal DuckDB runtime bridge and C ABI handles for plan/prepared/native buffers. | Already bridges Phase 22 runtime policy to Phase 23 backend and DuckDB without exposing public symbols. [VERIFIED: `crates/loom-ffi/src/duckdb_runtime.rs`; VERIFIED: `crates/loom-ffi/include/loom_duckdb_internal.h`] |
| `loom-native-melior` | 0.1.0 local crate | Backend identity, production backend prepare, JIT seed, native/reference output comparison. | Existing backend reports carry cache key, backend identity, artifact identity, cancellation, toolchain, and mismatch diagnostics. [VERIFIED: `crates/loom-native-melior/Cargo.toml`; VERIFIED: `crates/loom-native-melior/src/backend.rs`; VERIFIED: `crates/loom-native-melior/src/jit.rs`] |
| DuckDB C++ extension | DuckDB CLI gate pinned to `v1.5.3` in script | Host SQL adapter through `loom_scan(path)`, direct `DataChunk` output, projection pushdown. | Phase 24 established DuckDB as the first host adapter and release-gated it through public SQL. [VERIFIED: `scripts/duckdb-native-integration-test.sh`; VERIFIED: `duckdb-ext/loom_extension.cpp`] |
| `arrow` / `arrow-array` / `arrow-schema` / `arrow-data` | `=58.3.0` | Arrow C Data and typed arrays. | Workspace pins a single Arrow family version with `ffi` enabled to avoid type skew. [VERIFIED: `Cargo.toml`] |

### Supporting

| Library/Tool | Version | Purpose | When to Use |
|--------------|---------|---------|-------------|
| `loom-fixtures` | workspace path | Deterministic `.loom` payload generation and in-memory Vortex oracle helpers. | Use for interpreter/native equivalence fixtures and Vortex-backed oracle rows where they already exist. [VERIFIED: `crates/loom-fixtures/src/oracle.rs`; VERIFIED: `crates/loom-fixtures/src/bin/emit_duckdb_payloads.rs`] |
| `loom-vortex-ingress` | workspace path | Real Vortex reader facts, supported emission, and source-reader oracle tests. | Use only for Vortex evidence already in accepted ingress/coverage boundaries, not inside native runtime/cache code. [VERIFIED: `crates/loom-vortex-ingress/tests/single_column_to_loom.rs`; VERIFIED: `scripts/mvp0-verify.sh`] |
| CMake | local `4.1.1` | Build the DuckDB extension. | Existing DuckDB native gate builds `duckdb-ext/build/loom.duckdb_extension`. [VERIFIED: environment probe; VERIFIED: `scripts/duckdb-native-integration-test.sh`] |
| Rust toolchain | local `rustc 1.92.0`, `cargo 1.92.0` | Build/test Rust workspace. | Existing gates run `cargo test`, `cargo build -p loom-ffi --release`, and focused crate tests. [VERIFIED: environment probe; VERIFIED: `scripts/mvp0-verify.sh`] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| In-process cache keyed by `RuntimeCacheKey` | Persistent on-disk native cache | Persistent cache would force format/version/eviction/security decisions explicitly deferred by the user. [VERIFIED: `25-CONTEXT.md`] |
| Rust-owned cache policy | C++ `unordered_map` in `duckdb-ext` | C++ cache ownership would duplicate runtime policy and make the DuckDB adapter less host-neutral. [VERIFIED: `crates/loom-ffi/src/duckdb_runtime.rs`; VERIFIED: `25-CONTEXT.md`] |
| Interpreter oracle as primary | Vortex oracle as primary for every native SQL case | Vortex oracle exists for fixture/ingress boundaries but Phase 25 must not claim arbitrary Vortex semantic compatibility. [VERIFIED: `crates/loom-fixtures/src/oracle.rs`; VERIFIED: `.planning/PROJECT.md`] |
| Smoke reuse evidence | Benchmark suite claiming native speedup | The phase needs regression evidence that cache reuse works, not production speed claims. [VERIFIED: `25-CONTEXT.md`; VERIFIED: `.planning/PROJECT.md`] |

**Installation:**

```bash
# No new external packages should be installed for Phase 25. [VERIFIED: Cargo.toml; VERIFIED: 25-CONTEXT.md]
```

**Version verification:** Existing workspace dependency versions were verified from `Cargo.toml`; no new package versions are recommended. [VERIFIED: `Cargo.toml`]

## Package Legitimacy Audit

Phase 25 should install no new external packages. [VERIFIED: `25-CONTEXT.md`; VERIFIED: `Cargo.toml`]

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| none | n/a | n/a | n/a | n/a | n/a | No package install planned. [VERIFIED: repo grep] |

**Packages removed due to slopcheck [SLOP] verdict:** none. [VERIFIED: no package install planned]
**Packages flagged as suspicious [SUS]:** none. [VERIFIED: no package install planned]

`slopcheck` was not installed locally, but the legitimacy gate is not required because Phase 25 should not add package dependencies. [VERIFIED: environment probe]

## Architecture Patterns

### System Architecture Diagram

```text
Public SQL: loom_scan(path)
        |
        v
DuckDB Bind reads artifact and schema
        |
        v
Rust runtime plan via loom_duckdb_plan_create[_projected]
        |
        +--> verifier/runtime rejects -> fail-closed diagnostic
        |
        +--> interpreter-only + fallback allowed -> loom_decode interpreter output
        |
        v
native-candidate RuntimePlan + RuntimeCacheKey
        |
        v
In-process native cache lookup by exact RuntimeCacheKey
        |
        +--> hit with accepted prepared artifact -> native buffer route
        |
        +--> miss/mismatch -> backend prepare/JIT/reference compare
                              |
                              +--> accepted and compared -> cache prepared artifact/evidence
                              +--> mismatch/cancel/toolchain unsupported -> no buffers, route diagnostic
        |
        v
DuckDB direct DataChunk fill
        |
        v
SQL rows compared against interpreter/Vortex-backed fixture evidence
```

This flow matches the existing Bind/Init/Scan adapter and the Phase 24 route model; Phase 25 adds only cache lookup/reuse/invalidation and broader evidence. [VERIFIED: `duckdb-ext/loom_extension.cpp`; VERIFIED: `crates/loom-ffi/src/duckdb_runtime.rs`; VERIFIED: `25-CONTEXT.md`]

### Recommended Project Structure

```text
crates/
|-- loom-core/src/runtime_abi.rs               # keep cache identity and diagnostic vocabulary host-neutral
|-- loom-ffi/src/duckdb_runtime.rs             # add in-process cache and helper-visible cache reports
|-- loom-ffi/tests/duckdb_runtime.rs           # Rust helper equivalence/cache/fallback tests
|-- loom-ffi/tests/duckdb_runtime_ffi.rs       # internal C ABI cache/diagnostic tests
|-- loom-native-melior/tests/                  # backend identity/toolchain drift support tests if needed
duckdb-ext/
`-- loom_extension.cpp                         # consume cache reports, do not own cache policy
scripts/
|-- native-hardening-test.sh                   # new Phase 25 gate
`-- mvp0-verify.sh                             # call Phase 25 gate after Phase 24 gate
```

This structure reuses existing module ownership and script style. [VERIFIED: repo grep]

### Pattern 1: Equivalence Matrix

**What:** Build a finite matrix with three evidence layers: Rust helper buffer equivalence, public DuckDB SQL row equivalence, and Vortex/fixture oracle evidence where already available. [VERIFIED: `25-CONTEXT.md`; VERIFIED: `scripts/duckdb-native-integration-test.sh`; VERIFIED: `crates/loom-fixtures/src/oracle.rs`]

**When to use:** Use this for supported raw non-null primitive single/table shapes, projection order, repeated scans, and cache-hit replays. [VERIFIED: `crates/loom-fixtures/src/bin/emit_duckdb_payloads.rs`; VERIFIED: `scripts/duckdb-native-integration-test.sh`]

**Recommended matrix:**

| Case | Shape | Oracle | Evidence Type | Gate |
|------|-------|--------|---------------|------|
| Native primitive table all columns | `i32/i64/f32/f64` non-null raw table | Interpreter/reference output | SQL aggregate and Rust helper bytes | `native-hardening-test.sh` [VERIFIED: `emit_duckdb_payloads.rs`; VERIFIED: `duckdb_runtime.rs`] |
| Native primitive table reordered projection | `f64_col, i32_col` | Interpreter/reference output | SQL rows plus cache input `projection=columns:3>0,0>1` | `native-hardening-test.sh` [VERIFIED: `scripts/duckdb-native-integration-test.sh`] |
| Repeated identical scan | Same payload/projection/policy | First accepted prepare | SQL result equality plus cache hit counter/report | `native-hardening-test.sh` [VERIFIED: `25-CONTEXT.md`] |
| Cache invalidation by projection | Same payload, different projection | Key inequality | Rust helper and route report `cache-miss`/`cache-key-mismatch` | `loom-ffi` tests [VERIFIED: `runtime_cache_key.rs`] |
| Cache invalidation by policy | Same payload, fallback allowed vs strict | Key inequality | Rust helper diagnostics | `loom-core`/`loom-ffi` tests [VERIFIED: `runtime_abi.rs`] |
| Unsupported string payload | `fsst-utf8.loom` | Interpreter fallback | SQL aggregate and strict fail-closed error | `native-hardening-test.sh` [VERIFIED: `scripts/duckdb-native-integration-test.sh`] |
| Nullable/compression native expansion | nullable, bitpack/FOR/dict/RLE | Vortex/fixture rows where available | Negative route: fallback or fail-closed, not native success | `native-hardening-test.sh` [VERIFIED: `25-CONTEXT.md`; VERIFIED: `crates/loom-vortex-ingress/tests/bitpack_for_coverage.rs`] |

### Pattern 2: Host-Neutral In-Process Cache

**What:** Add a process-local cache in Rust near `prepare_duckdb_runtime`, storing only accepted prepared backend artifacts or deterministic preparation evidence keyed by exact `RuntimeCacheKey.stable_id` and guarded by canonical input equality. [VERIFIED: `crates/loom-core/src/runtime_abi.rs`; VERIFIED: `crates/loom-ffi/src/duckdb_runtime.rs`]

**When to use:** Use only after a plan is `NativeCandidate`, has no runtime diagnostics, backend prepare succeeds, and JIT/reference output comparison succeeds. [VERIFIED: `crates/loom-ffi/src/duckdb_runtime.rs`; VERIFIED: `crates/loom-native-melior/src/jit.rs`]

**Cache may store:**
- Accepted backend reports with accepted artifacts, backend identity, runtime cache key, row count, column count, and artifact summary. [VERIFIED: `crates/loom-native-melior/src/backend.rs`]
- Deterministic preparation evidence needed to avoid re-running prepare for the same key. [VERIFIED: `25-CONTEXT.md`]
- Internal counters/reports: `lookup`, `hit`, `miss`, `revalidated`, `invalidated`, `bypass`, and `not-cacheable`. [VERIFIED: `25-CONTEXT.md`]

**Cache must never store:**
- Native value buffers from a failed or mismatched comparison. [VERIFIED: `crates/loom-ffi/src/duckdb_runtime.rs`]
- Routes with cancellation, toolchain failure/skip, backend diagnostics, missing lowering facts, or unsupported lowering facts. [VERIFIED: `crates/loom-native-melior/src/backend.rs`; VERIFIED: `crates/loom-native-melior/src/jit.rs`]
- Path/mtime-only identity. [VERIFIED: `25-CONTEXT.md`]
- Any public SQL-visible cache knobs. [VERIFIED: `25-CONTEXT.md`; VERIFIED: `scripts/duckdb-native-integration-test.sh`]

### Pattern 3: Policy-Owned Fallback

**What:** Preserve `RuntimeSafetyPolicy` as the fallback source of truth; C++ should render diagnostics and choose interpreter/native emission only from Rust route reports. [VERIFIED: `crates/loom-core/src/runtime_abi.rs`; VERIFIED: `duckdb-ext/loom_extension.cpp`]

**When to use:** Use for unsupported lowering, unsupported predicates/projections/splits, cancellation, toolchain unavailable, and native output mismatch. [VERIFIED: `crates/loom-core/src/runtime_abi.rs`; VERIFIED: `crates/loom-ffi/src/duckdb_runtime.rs`; VERIFIED: `crates/loom-native-melior/src/backend.rs`]

**Diagnostics that should remain stable:** `cache-key-mismatch`, `fallback-disabled`, `lowering-unsupported`, `native-output-mismatch`, `cancelled`, `toolchain-skipped`, `toolchain-failed`, `unsupported-projection`, `unsupported-predicate`, `invalid-split`. [VERIFIED: `crates/loom-core/src/runtime_abi.rs`; VERIFIED: `crates/loom-native-melior/src/backend.rs`; VERIFIED: `crates/loom-ffi/src/duckdb_runtime.rs`]

### Anti-Patterns to Avoid

- **Public cache/native SQL mode flags:** Phase 24 deliberately kept `loom_scan(path)` as the only public SQL route, and Phase 25 should not freeze test controls as product API. [VERIFIED: `24-DUCKDB-NATIVE-REPORT.md`; VERIFIED: `25-CONTEXT.md`]
- **Path/mtime cache invalidation:** Path-based identity can reuse stale artifacts when content or verifier facts change; `RuntimeCacheKey` already includes content/facts/lowering/backend/query/policy fields. [VERIFIED: `crates/loom-core/src/runtime_abi.rs`]
- **Caching failed native outputs:** `prepare_duckdb_runtime` currently returns empty buffers on mismatch/cancel/failure, and cache logic must preserve that invariant. [VERIFIED: `crates/loom-ffi/src/duckdb_runtime.rs`]
- **C++ owns eligibility policy:** DuckDB should remain an adapter over Rust reports because Phase 24 explicitly verified Rust-owned policy and projection/cache input. [VERIFIED: `24-VERIFICATION.md`; VERIFIED: `duckdb-ext/loom_extension.cpp`]
- **Overclaiming speed:** Cache smoke evidence should prove fewer prepares or a cache hit report on identical scans, not native execution speed. [VERIFIED: `25-CONTEXT.md`; VERIFIED: `.planning/PROJECT.md`]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cache key hashing | New ad hoc path/mtime hash | Existing `RuntimeCacheKey::build` | It already canonicalizes artifact, facts, solver, lowering, backend, projection, predicate, split, and policy. [VERIFIED: `runtime_abi.rs`] |
| Fallback policy | C++ switch over payload names or route strings | `decide_runtime_execution` and Rust plan/prepared reports | Runtime diagnostics and fallback behavior already exist in the Rust runtime ABI. [VERIFIED: `runtime_abi.rs`; VERIFIED: `duckdb_runtime.rs`] |
| Native output validation | Directly trust JIT buffers | Existing `compare_production_jit_output` before exposing buffers | Phase 24 only exposes native buffers after reference comparison succeeds. [VERIFIED: `duckdb_runtime.rs`; VERIFIED: `jit.rs`] |
| Vortex semantic oracle inside core/FFI | Add Vortex deps to `loom-core` or `loom-ffi` | `loom-fixtures` / `loom-vortex-ingress` oracle tests | Project dependency boundary forbids Vortex in core/FFI. [VERIFIED: `Cargo.toml`; VERIFIED: `scripts/mvp0-verify.sh`] |
| Performance benchmark harness | New broad benchmark framework | Shell smoke: cache hit/miss counter and repeated SQL equality | Phase 25 needs regression evidence without production speed claims. [VERIFIED: `25-CONTEXT.md`] |

**Key insight:** Phase 25 is about trust hardening. The standard solution is to reuse the verified runtime/backend identities and add observable cache/fallback invariants, not to widen native code generation or invent new host APIs. [VERIFIED: repo grep]

## Common Pitfalls

### Pitfall 1: Cache Hit Without Canonical Equality
**What goes wrong:** A cache lookup by `stable_id` alone could hide a collision or a stale canonical input. [VERIFIED: `runtime_abi.rs`]
**Why it happens:** `stable_id` is a compact hash, while `canonical_input` is the full source-of-truth identity string. [VERIFIED: `runtime_abi.rs`]
**How to avoid:** On lookup, require both `stable_id` and `canonical_input` equality; otherwise emit `cache-key-mismatch` and treat as miss or fail closed by policy. [VERIFIED: `25-CONTEXT.md`; VERIFIED: `runtime_abi.rs`]
**Warning signs:** Tests only assert cache hit count and never mutate projection, policy, artifact bytes, backend identity, or lowering facts. [VERIFIED: `runtime_cache_key.rs`]

### Pitfall 2: SQL-Only Evidence Cannot Exercise All Failure Modes
**What goes wrong:** Cancellation, injected native mismatch, cache identity drift, and backend/toolchain edge cases may be hard to trigger naturally through public SQL. [VERIFIED: `scripts/duckdb-native-integration-test.sh`]
**Why it happens:** Public SQL intentionally has no route controls. [VERIFIED: `24-DUCKDB-NATIVE-REPORT.md`]
**How to avoid:** Pair SQL output checks with Rust helper tests and internal `LOOM_DUCKDB_TEST_*` route reports. [VERIFIED: `scripts/duckdb-native-integration-test.sh`; VERIFIED: `crates/loom-ffi/tests/duckdb_runtime.rs`]
**Warning signs:** A gate passes only `SELECT COUNT(*)` but has no helper-level mismatch/cancel/cache assertions. [VERIFIED: `scripts/duckdb-native-integration-test.sh`]

### Pitfall 3: Caching Native Buffers Instead Of Prepared Artifacts
**What goes wrong:** Cached raw buffers can outlive their valid ownership context or preserve unsafe output after comparison failure. [VERIFIED: `duckdb_runtime.rs`; VERIFIED: `duckdb-ext/loom_extension.cpp`]
**Why it happens:** Phase 24 native buffers are prepared route outputs copied into DuckDB vectors, not a stable public artifact format. [VERIFIED: `duckdb_runtime.rs`; VERIFIED: `duckdb-ext/loom_extension.cpp`]
**How to avoid:** Cache accepted backend artifacts/preparation evidence, then regenerate or validate output buffers per scan. [VERIFIED: `25-CONTEXT.md`; VERIFIED: `crates/loom-native-melior/src/backend.rs`]
**Warning signs:** Cache entry type contains raw `value_ptr` or C++ `Vector` ownership. [VERIFIED: `duckdb-ext/loom_extension.cpp`]

### Pitfall 4: Treating Toolchain Skip As A Successful Native Cache Entry
**What goes wrong:** The cache may record a toolchain skip/failure and later report a hit even though no accepted native artifact exists. [VERIFIED: `jit.rs`; VERIFIED: `backend.rs`]
**Why it happens:** Current gates allow `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`, but that is a diagnostic path, not native success. [VERIFIED: `scripts/duckdb-native-integration-test.sh`; VERIFIED: `jit.rs`]
**How to avoid:** Mark skipped/failed toolchain reports as non-cacheable and require accepted backend report plus successful output comparison before cache insert. [VERIFIED: `duckdb_runtime.rs`; VERIFIED: `backend.rs`; VERIFIED: `jit.rs`]
**Warning signs:** Cache hit counters increase in a run where route report contains only `toolchain-skipped` or `toolchain-failed`. [VERIFIED: `scripts/duckdb-native-integration-test.sh`]

### Pitfall 5: Expanding Native Semantics During Hardening
**What goes wrong:** Phase 25 could drift into nullable/string/bitpack/FOR/dict/RLE native execution and delay the cache/fallback closeout. [VERIFIED: `25-CONTEXT.md`]
**Why it happens:** Equivalence matrices invite broader coverage unless unsupported cases are explicitly negative evidence. [VERIFIED: `25-CONTEXT.md`]
**How to avoid:** Use unsupported cases to prove fallback/fail-closed diagnostics, not native success. [VERIFIED: `25-CONTEXT.md`]
**Warning signs:** New dialect kernels or Vortex deps appear in `loom-core`/`loom-ffi`. [VERIFIED: `scripts/mvp0-verify.sh`; VERIFIED: `Cargo.toml`]

## Suggested Phase 25 Plan Slices And Acceptance Gates

| Slice | Scope | Acceptance Gate |
|-------|-------|-----------------|
| 25-01 Equivalence matrix and helper scaffolding | Define supported/unsupported matrix, add Rust helper assertions for interpreter/reference/native byte equality and Vortex-backed oracle references where available. [VERIFIED: `25-CONTEXT.md`] | `cargo test -p loom-ffi --test duckdb_runtime` plus targeted fixture/oracle tests pass. [VERIFIED: existing test layout] |
| 25-02 In-process cache contract | Add Rust-owned cache entry/report model keyed by exact `RuntimeCacheKey`; prove hit, miss, policy/projection invalidation, non-cacheable failure routes. [VERIFIED: `runtime_abi.rs`; VERIFIED: `duckdb_runtime.rs`] | Rust tests show identical keys reuse prepare evidence and key mutations miss/invalidate with stable diagnostics. [VERIFIED: `runtime_cache_key.rs`] |
| 25-03 DuckDB route-report and SQL cache evidence | Extend internal test route report with cache status and run repeated public SQL scans without new public API. [VERIFIED: `duckdb-ext/loom_extension.cpp`; VERIFIED: `scripts/duckdb-native-integration-test.sh`] | SQL rows/aggregates match and route report shows second identical scan hits cache or avoids prepare counter increment. [VERIFIED: `25-CONTEXT.md`] |
| 25-04 Fallback and negative hardening | Add strict/fallback cases for unsupported lowering facts, cache-key mismatch, cancellation, toolchain skip/failure, native mismatch, malformed artifacts, unsupported projection/predicate/split. [VERIFIED: `runtime_abi.rs`; VERIFIED: `backend.rs`; VERIFIED: `jit.rs`] | All negative routes emit deterministic code/path/message and no partial rows/native buffers. [VERIFIED: `duckdb_runtime.rs`; VERIFIED: `duckdb-ext/loom_extension.cpp`] |
| 25-05 Release gate and report | Add `scripts/native-hardening-test.sh`, wire into `mvp0-verify.sh` after Phase 24, and write final supported matrix/cache/fallback report. [VERIFIED: `25-CONTEXT.md`; VERIFIED: `scripts/mvp0-verify.sh`] | `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/native-hardening-test.sh` and full `mvp0-verify.sh` pass; final report records non-goals and Phase 26 handoff. [VERIFIED: `25-CONTEXT.md`] |

## Code Examples

Verified patterns from existing sources:

### Use Existing Cache Identity

```rust
// Source: crates/loom-core/src/runtime_abi.rs [VERIFIED: repo grep]
let cache_key = RuntimeCacheKey::build(&RuntimeCacheKeyInput {
    abi_version: RuntimeAbiVersion::CURRENT,
    artifact_digest,
    facts_fingerprint,
    solver_identity,
    production_lowering_fingerprint,
    backend_identity,
    projection,
    predicate,
    split,
    policy,
});
```

### Cache Insert Only After Successful Native Comparison

```rust
// Source: crates/loom-ffi/src/duckdb_runtime.rs [VERIFIED: repo grep]
if compare_production_jit_output(&backend_report, &expected_buffers, &jit_output).is_err() {
    return DuckDbPreparedRoute {
        decision: DuckDbRouteDecision::FailClosed,
        backend_report: Some(report),
        native_buffers: Vec::new(),
        diagnostics,
    };
}
// Phase 25 cache insert belongs after this point, never before. [VERIFIED: repo grep]
```

### Keep DuckDB As Report Consumer

```cpp
// Source: duckdb-ext/loom_extension.cpp [VERIFIED: repo grep]
auto prepared = CreatePreparedRoute(*runtime_plan.runtime_plan, cancelled);
auto prepared_route = ReadPreparedRoute(prepared);
auto prepared_diagnostics = CollectPreparedDiagnostics(prepared);
// Phase 25 should extend this report path with cache status, not duplicate policy here. [VERIFIED: repo grep]
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Interpreter-only DuckDB path | DuckDB native route can be selected internally for eligible primitive artifacts, with interpreter fallback and fail-closed diagnostics. | Phase 24, verified 2026-06-08. [VERIFIED: `24-VERIFICATION.md`] | Phase 25 can harden native evidence instead of building host integration from scratch. [VERIFIED: repo grep] |
| Route-specific public controls | Internal `LOOM_DUCKDB_TEST_*` controls and route report only. | Phase 24. [VERIFIED: `24-DUCKDB-NATIVE-REPORT.md`; VERIFIED: `scripts/duckdb-native-integration-test.sh`] | Cache/fallback observability should remain internal. [VERIFIED: `25-CONTEXT.md`] |
| Cache identity as design model only | `RuntimeCacheKey` is built from canonical runtime inputs and exposed through internal DuckDB plan cache input. | Phase 22/24. [VERIFIED: `runtime_abi.rs`; VERIFIED: `duckdb_runtime.rs`; VERIFIED: `24-VERIFICATION.md`] | Phase 25 should add reuse/invalidation semantics, not redefine identity. [VERIFIED: repo grep] |
| Native buffer trust by route | Native buffers are only exposed after backend prepare, JIT output, and reference comparison succeed. | Phase 24. [VERIFIED: `duckdb_runtime.rs`; VERIFIED: `jit.rs`] | Cache inserts must happen after comparison success. [VERIFIED: repo grep] |

**Deprecated/outdated:**
- Public native/interpreter SQL functions: explicitly forbidden by Phase 24 public API creep checks. [VERIFIED: `scripts/duckdb-native-integration-test.sh`]
- Persistent native cache format: explicitly deferred by Phase 25 context. [VERIFIED: `25-CONTEXT.md`]
- Vortex dependencies in core/FFI: forbidden by project dependency guard. [VERIFIED: `scripts/mvp0-verify.sh`; VERIFIED: `Cargo.toml`]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| none | All implementation-relevant claims are grounded in repo evidence, user context, or official OWASP/DuckDB sources. | n/a | n/a |

## Open Questions

1. **Should the in-process cache be process-global or tied to internal DuckDB plan/prepared handles?**
   - What we know: User context asks for host-neutral, in-process semantics; the current Rust bridge owns `prepare_duckdb_runtime` and internal handles. [VERIFIED: `25-CONTEXT.md`; VERIFIED: `duckdb_runtime.rs`]
   - What's unclear: The exact storage lifetime has not been selected. [VERIFIED: repo grep]
   - Recommendation: Prefer a Rust-owned process-local cache with explicit test reset/report hooks, because it proves repeated scan reuse across DuckDB plan instances without adding public API. [VERIFIED: `25-CONTEXT.md`]

2. **Should cache hits replay prepared backend artifacts or replay native output buffers?**
   - What we know: Native buffers are exposed only after successful comparison and then copied into DuckDB vectors. [VERIFIED: `duckdb_runtime.rs`; VERIFIED: `duckdb-ext/loom_extension.cpp`]
   - What's unclear: Existing backend artifact representation may not yet contain enough material to skip every prepare/JIT step. [VERIFIED: `backend.rs`]
   - Recommendation: Cache the smallest accepted preparation evidence available first, and make the smoke gate "prepare counter avoided or cache hit report observed" rather than "JIT execution skipped." [VERIFIED: `25-CONTEXT.md`]

3. **How strict should toolchain drift invalidation be when local LLVM/MLIR is unavailable?**
   - What we know: Toolchain skip/failure is already represented as diagnostics, and `LOOM_ALLOW_NATIVE_TOOL_SKIP=1` is allowed in release gates. [VERIFIED: `jit.rs`; VERIFIED: `scripts/mvp0-verify.sh`]
   - What's unclear: Local toolchain availability varies by machine. [VERIFIED: environment probe]
   - Recommendation: Treat accepted backend identity drift as invalidation when toolchain facts exist; treat skipped/failed toolchain routes as non-cacheable diagnostic evidence. [VERIFIED: `backend.rs`; VERIFIED: `jit.rs`]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust `rustc` | Cargo tests/builds | yes | `rustc 1.92.0` | none needed. [VERIFIED: environment probe] |
| Cargo | Workspace tests and fixture generation | yes | `cargo 1.92.0` | none needed. [VERIFIED: environment probe] |
| CMake | DuckDB extension build | yes | `4.1.1` | none needed. [VERIFIED: environment probe] |
| DuckDB CLI | SQL gates | yes, cached | `v1.5.3 (Variegata)` | Script downloads `v1.5.3` if cache missing and `DUCKDB_CLI` unset. [VERIFIED: environment probe; VERIFIED: `scripts/duckdb-native-integration-test.sh`] |
| Built DuckDB extension | Local SQL smoke | yes, present | `duckdb-ext/build/loom.duckdb_extension` | Rebuilt by gate. [VERIFIED: environment probe; VERIFIED: `scripts/duckdb-native-integration-test.sh`] |
| `rg` | Script grep checks | yes | `14.1.1` | use `grep` manually if missing, but scripts expect `rg`. [VERIFIED: environment probe; VERIFIED: scripts] |
| `curl` / `unzip` | DuckDB CLI download | yes | curl `8.16.0`, unzip `6.00` | pre-set `DUCKDB_CLI`. [VERIFIED: environment probe; VERIFIED: `scripts/duckdb-native-integration-test.sh`] |
| LLVM/MLIR tools (`llvm-config`, `mlir-opt`, `mlir-translate`) | Strict native backend validation/JIT | no PATH result in probe | n/a | Existing gates support explicit `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`; strict no-skip native evidence depends on local toolchain. [VERIFIED: environment probe; VERIFIED: `jit.rs`; VERIFIED: `scripts/mvp0-verify.sh`] |

**Missing dependencies with no fallback:**
- None for research and skip-aware Phase 25 planning. [VERIFIED: environment probe]

**Missing dependencies with fallback:**
- LLVM/MLIR command-line tools are absent from PATH; use existing skip-aware diagnostics unless a plan explicitly requires strict native execution evidence. [VERIFIED: environment probe; VERIFIED: `jit.rs`]

## Security Domain

Security enforcement is enabled in `.planning/config.json`, and ASVS categories are included as a planning checklist. [VERIFIED: `.planning/config.json`; CITED: https://devguide.owasp.org/en/03-requirements/05-asvs/]

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | Phase 25 adds no user authentication surface. [VERIFIED: `25-CONTEXT.md`; CITED: https://devguide.owasp.org/en/03-requirements/05-asvs/] |
| V3 Session Management | no | Phase 25 adds no browser/server session surface. [VERIFIED: `25-CONTEXT.md`; CITED: https://devguide.owasp.org/en/03-requirements/05-asvs/] |
| V4 Access Control | no | Phase 25 is local runtime/native hardening with no authorization model change. [VERIFIED: `25-CONTEXT.md`; CITED: https://devguide.owasp.org/en/03-requirements/05-asvs/] |
| V5 Validation, Sanitization and Encoding | yes | Validate artifact bytes, verifier facts, cache keys, projection inputs, backend reports, native buffer shape/type/length, and deterministic diagnostics before output. [VERIFIED: `runtime_abi.rs`; VERIFIED: `duckdb_runtime.rs`; VERIFIED: `duckdb-ext/loom_extension.cpp`; CITED: https://devguide.owasp.org/en/03-requirements/05-asvs/] |
| V6 Stored Cryptography | no | Phase 25 should not add signatures, encryption, or persistent cache storage. [VERIFIED: `25-CONTEXT.md`; CITED: https://devguide.owasp.org/en/03-requirements/05-asvs/] |

### Known Threat Patterns for Native Runtime Cache

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Stale native artifact reuse | Tampering | Exact `RuntimeCacheKey` plus canonical input equality; cache invalidation on artifact/facts/lowering/backend/query/policy drift. [VERIFIED: `runtime_abi.rs`] |
| Unsafe native output after mismatch | Tampering / Elevation of Privilege | Insert into cache only after reference comparison succeeds; mismatch emits no native buffers. [VERIFIED: `duckdb_runtime.rs`; VERIFIED: `jit.rs`] |
| Public test knob exposure | Information Disclosure / Tampering | Keep `LOOM_DUCKDB_TEST_*` internal and grep public header/SQL API creep. [VERIFIED: `scripts/duckdb-native-integration-test.sh`] |
| Toolchain drift | Tampering | Include backend/toolchain identity in cache identity and treat skipped/failed toolchain routes as non-cacheable. [VERIFIED: `backend.rs`; VERIFIED: `runtime_abi.rs`] |
| Partial row emission on errors | Tampering | Preserve fail-closed/no-row-emission behavior for fail-closed, diagnostic-only, cancelled, and mismatch routes. [VERIFIED: `duckdb-ext/loom_extension.cpp`] |

## Sources

### Primary (HIGH confidence)
- `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-CONTEXT.md` - User decisions, tradeoffs, deferred scope. [VERIFIED: repo grep]
- `.planning/PROJECT.md`, `.planning/STATE.md`, `.planning/ROADMAP.md` - Phase ordering, Phase 25/26 handoff, project constraints. [VERIFIED: repo grep]
- `.planning/phases/24-duckdb-native-execution-integration-mvp/24-DUCKDB-NATIVE-REPORT.md` and `24-VERIFICATION.md` - Phase 24 route evidence and verified truths. [VERIFIED: repo grep]
- `crates/loom-core/src/runtime_abi.rs` and runtime tests - cache key, policy, diagnostics, projection/predicate/split planning. [VERIFIED: repo grep]
- `crates/loom-ffi/src/duckdb_runtime.rs` and `crates/loom-ffi/tests/*` - DuckDB runtime bridge, native prepare/JIT comparison, internal C ABI, diagnostics. [VERIFIED: repo grep]
- `duckdb-ext/loom_extension.cpp` and `scripts/duckdb-native-integration-test.sh` - Bind/init/scan adapter, route reports, direct DataChunk output, public SQL gate. [VERIFIED: repo grep]
- `crates/loom-native-melior/src/backend.rs`, `pipeline.rs`, `jit.rs` and tests - backend identity, toolchain/cancel/mismatch diagnostics, accepted artifact reports. [VERIFIED: repo grep]
- `crates/loom-fixtures/src/oracle.rs` and `crates/loom-vortex-ingress/tests/*` - existing Vortex/fixture oracle evidence. [VERIFIED: repo grep]

### Secondary (MEDIUM confidence)
- DuckDB extension overview - extension concepts and host extension boundary. [CITED: https://duckdb.org/docs/stable/extensions/overview.html]
- OWASP ASVS Developer Guide - ASVS category names and security verification framing. [CITED: https://devguide.owasp.org/en/03-requirements/05-asvs/]

### Tertiary (LOW confidence)
- None. [VERIFIED: repo grep]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new packages; versions and local crates verified from `Cargo.toml` and implementation files. [VERIFIED: repo grep]
- Architecture: HIGH - Phase 24 verification and implementation files directly show the runtime/backend/DuckDB boundaries. [VERIFIED: repo grep]
- Pitfalls: HIGH - each pitfall maps to an existing diagnostic, route, or deferred scope item. [VERIFIED: repo grep]
- Cache implementation detail: MEDIUM - identity and desired contract are verified, but storage lifetime and exact cache entry type remain planner/implementation choices. [VERIFIED: `25-CONTEXT.md`; VERIFIED: `runtime_abi.rs`]

**Research date:** 2026-06-09
**Valid until:** 2026-06-16 for local implementation planning because the phase is tightly coupled to active repo state and recent Phase 24 artifacts. [VERIFIED: `.planning/STATE.md`]
