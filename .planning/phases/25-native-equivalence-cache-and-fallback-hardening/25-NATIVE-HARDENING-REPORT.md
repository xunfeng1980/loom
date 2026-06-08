# Phase 25 Native Hardening Report

**Status:** Complete and release-gated.
**Scope:** Native equivalence, in-process cache reuse/invalidation, fallback/fail-closed behavior, and bounded handoff to Phase 26.
**Gate:** `scripts/native-hardening-test.sh`, wired into `scripts/mvp0-verify.sh` after `scripts/duckdb-native-integration-test.sh` and before `scripts/duckdb-smoke-test.sh`.

## Executive Summary

Phase 25 hardens the existing DuckDB native execution path without widening the public query surface. Public SQL remains `loom_scan(path)`. Cache controls remain internal diagnostics/test hooks. No new external packages were added.

The supported positive native evidence is limited to verifier-gated, raw, non-null primitive shapes that the Phase 23 backend and Phase 24 DuckDB adapter already support. Unsupported strings, nullable primitive native execution, compressed layouts, predicates, splits, malformed artifacts, cancellation, and native-output mismatch are fallback or fail-closed evidence, not new native-kernel support.

## Supported Equivalence Matrix

| Case | Shape | Oracle | Evidence | Result |
|------|-------|--------|----------|--------|
| Native primitive table aggregate | `LMC1`/`LMT1` table with non-null `Int32`, `Int64`, `Float32`, and `Float64` raw columns | Interpreter/reference output | `scripts/native-hardening-test.sh` runs `COUNT`, `SUM`, `MIN`, and `MAX` over `loom_scan(path)` | Supported native-candidate route when backend/toolchain accepts; SQL rows remain correct when explicit toolchain skip is allowed |
| Repeated identical native scan | Same native primitive table, same projection and policy | First accepted prepare plus interpreter/reference comparison | Same DuckDB CLI process runs the query twice and route diagnostics report `cache-miss`, `cache-inserted`, then `cache-hit` | Supported cache smoke evidence |
| Reordered projection | `f64_col, i32_col` projected from the native primitive table | Interpreter/reference output | Public SQL row equality plus route report projection text `projection=columns:3>0,0>1` | Supported projection-order equality; different projection produces a cache miss |
| Rust helper primitive buffer equality | Raw primitive single/table helper routes | Interpreter/reference bytes | `loom-ffi` helper tests compare builder id, Arrow type, and bytes before native route acceptance | Supported helper-level equivalence |
| Cache replay | Accepted native preparation followed by hit | Recomputed native/reference comparison after hit | `loom-ffi` cache tests verify hits do not bypass output comparison | Supported in-process reuse semantics |

## Interpreter Oracle Scope

The interpreter/reference output is the primary oracle for Phase 25 native evidence. Native buffers are accepted only after the Rust-owned runtime/backend path compares the produced native output with the reference output for the supported primitive shape.

This does not claim arbitrary Vortex semantic compatibility. Vortex-backed evidence remains bounded to existing fixture, reader, and ingress boundaries. Phase 25 consumes those artifacts as test inputs and row-oracle history, but it does not move Vortex dependencies into `loom-core`, `loom-ffi`, or the DuckDB extension.

## Existing Vortex And Fixture Evidence Used

- `loom-fixtures` emits deterministic DuckDB payloads, including `native-primitives-table.loom`, `fsst-utf8.loom`, `bitpack-i32.loom`, and `bitpack-nullable-i32.loom`.
- Earlier reader/coverage phases established Vortex-backed row evidence for supported emitted fixture classes.
- Phase 25 uses these fixtures to prove DuckDB row behavior through `loom_scan(path)` and to classify unsupported native shapes as fallback or fail-closed cases.
- The report does not upgrade that evidence into native support for arbitrary Vortex encodings, storage modes, nested layouts, nullability, or source/table-format binding.

## In-Process Cache Design

The native preparation cache is process-local, Rust-owned, and internal to the DuckDB runtime bridge. It is keyed by `RuntimeCacheKey.stable_id` and validated with exact canonical input compatibility before reuse.

Cached entries store accepted preparation evidence, not unchecked DuckDB vectors or persistent native artifacts. A cache hit reuses only preparation evidence; native output is still regenerated and compared with interpreter/reference output before any native route returns buffers.

The C++ DuckDB adapter consumes route and diagnostic reports. It does not own cache eligibility, cache identity, invalidation, fallback policy, or native support decisions.

## Cache Invalidation Rules

Cache reuse requires exact `RuntimeCacheKey` compatibility. The key material covers artifact identity, artifact facts, solver/lowering/backend identity, projection, predicate, split, concurrency, and safety policy inputs.

Invalidation is key-driven, not path/mtime-driven:

- Different stable id: normal cache miss.
- Same stable id with canonical input drift: `cache-key-mismatch`, stale entry removed, and route recomputed.
- Projection drift: cache miss, proven through reordered projection SQL evidence.
- Policy, backend identity, lowering facts, artifact facts, predicate, split, or concurrency drift: cache miss or key mismatch according to the runtime compatibility contract.

There is no persistent cache format, no public eviction policy, and no cross-process cache claim.

## Non-Cacheable Routes

The cache does not insert entries for routes that are unsafe or incomplete:

- missing accepted verifier/lowering facts
- interpreter fallback
- fallback-disabled fail-closed routes
- unsupported strings, nullable native execution, or compressed-layout native expansion
- malformed artifacts
- cancellation
- native-output mismatch
- backend/toolchain skipped or failed preparation
- missing native buffers or failed reference comparison

`scripts/native-hardening-test.sh` records `cache-non-cacheable` helper evidence for unsupported and unsafe routes.

## Fallback And Strict Behavior

Fallback remains Rust policy owned. DuckDB/C++ surfaces the decision and emits rows only after the selected route succeeds.

- With fallback allowed, unsupported native shapes use interpreter output and still return correct public SQL rows.
- With fallback disabled, unsupported native routes fail closed with deterministic diagnostics.
- Cancellation, malformed artifacts, and native-output mismatch fail closed and emit no partial native rows.
- A malformed artifact failure does not poison a later valid scan in the same gate.

## Deterministic Diagnostics

Phase 25 evidence checks stable diagnostic vocabulary through route reports, stderr, and helper tests:

- `cache-miss`
- `cache-inserted`
- `cache-hit`
- `cache-key-mismatch`
- `cache-non-cacheable`
- `interpreter-fallback`
- `fail-closed`
- `fallback-disabled`
- `lowering-unsupported`
- `unsupported-type` / `unsupported-kernel`
- `missing-l2-facts`
- `native-output-mismatch`
- `cancelled`
- `toolchain-skipped` / `toolchain-failed`

The SQL-level strict checks require diagnostic `code` and `path` text so failures remain actionable and deterministic.

## Performance Smoke Evidence

Phase 25 records smoke evidence, not a benchmark. The repeated identical scan in one DuckDB CLI process must report cache miss/insert followed by cache hit while returning identical aggregate output.

This proves reuse/invalidation visibility for the current supported route. It does not claim native speedup, production throughput, amortized compile cost, or cross-process cache performance.

## Public SQL And API Non-Creep

Public SQL remains:

```sql
SELECT * FROM loom_scan(path);
```

No public route-specific SQL functions were added. No public cache/native/fallback SQL controls were added. Public `loom.h` remains free of cache controls and DuckDB internal route handles. Cache evidence uses internal diagnostics and test-only environment hooks.

The Phase 25 public-marker gate rejects route-specific function names, cache mode spellings, public Arrow stream exposure, public predicate pushdown controls, and public parallel split controls in the checked surfaces.

## Verification Evidence

Plan 25-05 verification:

- `bash -n scripts/native-hardening-test.sh && bash -n scripts/mvp0-verify.sh`
- explicit order assertion in `scripts/mvp0-verify.sh`: Phase 23 production backend gate, Phase 24 DuckDB native integration gate, Phase 25 native hardening gate, then DuckDB SQL smoke
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/native-hardening-test.sh`
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp0-verify.sh`

Earlier Phase 25 evidence:

- 25-01: runtime cache compatibility and stable policy diagnostics
- 25-02: Rust-owned in-process native preparation cache and internal cache diagnostics
- 25-03: helper equivalence, cache replay, and unsupported-route negative matrices
- 25-04: DuckDB SQL native-hardening gate with cache smoke and fallback evidence

## Phase 26 Handoff Assumptions

Phase 26 can assume that the current DuckDB native path has bounded equivalence, cache, fallback, and fail-closed evidence for supported primitive shapes. It should consume this as an execution-contract baseline while defining an external source ingress contract.

Phase 26 must not assume:

- persistent native cache
- native speedup
- public cache/native SQL controls
- source/table-format binding
- predicate pushdown
- parallel split execution
- new native kernels
- arbitrary Vortex semantic compatibility

## Explicit Tradeoffs

| Tradeoff | Decision | Reason |
|----------|----------|--------|
| In-process vs persistent cache | In-process only | Proves reuse and invalidation without freezing an on-disk format, security model, eviction policy, or cross-process compatibility contract. |
| Interpreter oracle vs broad Vortex semantic claims | Interpreter/reference oracle is primary | Validates the host native path against the current Loom semantics without claiming arbitrary Vortex compatibility. |
| Smoke evidence vs benchmark | Cache smoke evidence only | Route diagnostics prove cache behavior deterministically; benchmark claims would overstate the current MVP slice. |
| Rust-owned policy vs C++ duplication | Rust-owned policy | Keeps cache eligibility, fallback rules, and native support decisions in the host-neutral runtime/backend path while C++ remains a thin adapter. |

## Non-Goals

- No persistent cache.
- No native speedup claim.
- No public cache/native/fallback SQL controls.
- No source/table-format binding.
- No predicate pushdown.
- No parallel split execution.
- No new native kernels.
- No arbitrary Vortex semantic compatibility.
- No new external packages.

