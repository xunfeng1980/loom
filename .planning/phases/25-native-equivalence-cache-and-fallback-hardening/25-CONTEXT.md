# Phase 25: Native Equivalence, Cache, and Fallback Hardening - Context

**Gathered:** 2026-06-08
**Status:** Ready for planning
**Mode:** Autonomous smart discuss; recommended answers accepted per user preference to follow recommendations while recording tradeoffs.

<domain>
## Phase Boundary

Phase 25 hardens the engine-integrated native execution story proven in Phase 24 before any source/table-format-visible work. It should add oracle/equivalence evidence, native cache reuse and invalidation semantics, unsupported-program negative coverage, deterministic diagnostics, performance smoke evidence, and release-gate wiring over the existing Phase 22 runtime policy, Phase 23 backend, and Phase 24 DuckDB adapter.

This phase is not a new query surface. Public SQL remains `loom_scan(path)`. It should not add external source formats, Iceberg/Lance/Parquet binding, predicate pushdown, parallel split execution, arbitrary Vortex semantic compatibility, native strings, nullable native execution, or a persistent cross-process cache unless planning discovers a smaller must-have cache contract that cannot be tested otherwise.

</domain>

<decisions>
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

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/loom-core/src/runtime_abi.rs` owns `RuntimeCacheKey`, `RuntimeCacheKeyInput`, projection/predicate/split planning, runtime diagnostics, and execution policy.
- `crates/loom-ffi/src/duckdb_runtime.rs` bridges DuckDB to runtime planning and Phase 23 backend prepare/JIT output comparison; it already exposes internal plan/prepared handles, route diagnostics, native buffers, cache stable id, and canonical cache input.
- `duckdb-ext/loom_extension.cpp` maps DuckDB bind/init/scan lifecycle to internal Rust route/prepared handles, direct `DataChunk` population, interpreter fallback through `loom_decode`, and internal route reports via `LOOM_DUCKDB_TEST_ROUTE_REPORT`.
- `crates/loom-native-melior` has backend identity, toolchain probing, production pipeline, JIT seed, and native output comparison helpers that can feed cache identity and invalidation tests.
- Existing gates include `scripts/runtime-abi-test.sh`, `scripts/production-backend-test.sh`, `scripts/duckdb-native-integration-test.sh`, `scripts/duckdb-smoke-test.sh`, and `scripts/mvp0-verify.sh`.

### Established Patterns
- Public surfaces stay narrow; route/cache/native controls are internal test hooks, not documented SQL/API.
- Unsupported or unsafe native paths fail closed or use interpreter fallback only through policy.
- Tests favor stable code/path/message diagnostics and deterministic fixture generation.
- Release gates are shell scripts that run focused Rust tests, build the DuckDB extension, exercise public SQL, and grep for API creep or route evidence.
- Planning/report artifacts explicitly record non-goals and tradeoffs after each phase.

### Integration Points
- Runtime cache identity should integrate at `RuntimeCacheKey::build` and `DuckDbRuntimePlanReport`.
- Native cache prepare/reuse likely belongs near `prepare_duckdb_runtime` and Phase 23 backend preparation, with the DuckDB adapter consuming reports rather than owning policy.
- DuckDB cache/equivalence visibility should extend `LOOM_DUCKDB_TEST_ROUTE_REPORT` or add similarly internal evidence hooks.
- Release wiring should extend `scripts/mvp0-verify.sh` after the Phase 24 DuckDB native integration gate.

</code_context>

<specifics>
## Specific Ideas

- User preference from current workflow: follow recommended choices first, but record the phase tradeoffs.
- Phase 25 should close the engine-integrated native execution story before Phase 26 starts external source ingress.
- Keep all evidence verifier-gated and fail-closed; no broad native-speed or arbitrary Vortex semantics claims.

</specifics>

<deferred>
## Deferred Ideas

- Persistent on-disk/native artifact cache format.
- Public cache/native/fallback SQL flags or functions.
- Predicate pushdown, parallel split execution, and worker-local scheduling.
- Native execution for strings, nullable primitives, bitpack/FOR expansion, dictionary/RLE, or arbitrary Vortex layouts.
- External source ingress, Lance/Parquet archival readability, Iceberg binding, StarRocks/DuckDB dual query surface, and full Vortex semantic compatibility.

</deferred>
