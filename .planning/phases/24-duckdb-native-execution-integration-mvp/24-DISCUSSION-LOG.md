# Phase 24: DuckDB Native Execution Integration MVP - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-08T15:28:01Z
**Phase:** 24-DuckDB Native Execution Integration MVP
**Areas discussed:** DuckDB lifecycle mapping, output delivery, native failure SQL behavior, projection/predicate/threading MVP, SQL API shape

---

## DuckDB Lifecycle Mapping

| Option | Description | Selected |
|--------|-------------|----------|
| Bind plans shape | Bind reads payload/schema and constructs runtime planning inputs; init consumes planned shape. | ✓ |
| Init plans all | Bind declares schema only; global init performs runtime plan/backend prepare. | |
| Scan lazily | First scan call performs planning/backend work. | |

**User's choice:** 1, recommended path.
**Notes:** Runtime planning belongs in `Bind` because schema and projection are bind-time facts.

| Option | Description | Selected |
|--------|-------------|----------|
| Global init prepare | Bind builds runtime plan/cache; global init prepares backend/JIT seed. | ✓ |
| Bind prepare too | Bind also performs backend/toolchain work. | |
| Local init per worker | Each worker prepares its own backend artifact. | |

**User's choice:** 1, recommended path.
**Notes:** Keeps LLVM/toolchain work out of bind without delaying errors until row production.

| Option | Description | Selected |
|--------|-------------|----------|
| Single worker MVP | Preserve serialized scan for Phase 24. | ✓ |
| Local state only | Add local state scaffolding without parallel execution. | |
| Parallel splits now | Implement row-range split execution now. | |

**User's choice:** 1, recommended path.
**Notes:** Parallel split/cache behavior is deferred to hardening.

| Option | Description | Selected |
|--------|-------------|----------|
| Single batch MVP | Preserve current one-batch output model. | ✓ |
| Chunked batches | Emit multiple DuckDB-sized chunks. | |
| Native only single batch, interpreter unchanged | Let native and interpreter output models diverge. | |

**User's choice:** 1, recommended path.
**Notes:** Single-batch output keeps this phase centered on adapter integration.

---

## Output Delivery

| Option | Description | Selected |
|--------|-------------|----------|
| Direct DataChunk population | Reuse existing DuckDB vector fill path. | ✓ |
| ArrowArrayStream/record batch | Introduce a table/stream ABI now. | |
| Hybrid public stream | Keep direct path but expose stream-shaped public ABI. | |

**User's choice:** User requested all remaining areas follow recommendations.
**Notes:** Tradeoff recorded in CONTEXT.md: direct `DataChunk` is smaller and already proven, but does not validate future stream ABI.

---

## Native Failure SQL Behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Policy-controlled fallback | Use interpreter fallback only when runtime policy allows; fail closed otherwise. | ✓ |
| Always throw | Any native failure becomes a DuckDB error. | |
| Always fallback | Any native failure silently runs the interpreter. | |

**User's choice:** User requested all remaining areas follow recommendations.
**Notes:** Native output mismatch is stricter than ordinary skip and should fail closed.

---

## Projection / Predicate / Threading MVP

| Option | Description | Selected |
|--------|-------------|----------|
| Projection + single worker | Prove projection/schema mapping and serialized full scan. | ✓ |
| Projection + predicates | Add predicate envelope/pushdown behavior now. | |
| Parallel splits now | Add row-range split and multi-worker behavior now. | |

**User's choice:** User requested all remaining areas follow recommendations.
**Notes:** Predicate pushdown and parallel split execution are deferred.

---

## SQL API Shape

| Option | Description | Selected |
|--------|-------------|----------|
| Keep loom_scan(path) | Native/interpreter routing is internal and policy-driven. | ✓ |
| Add mode parameter | Expose native/interpreter/strict as SQL argument. | |
| Add separate functions | Add `loom_scan_native` or similar. | |

**User's choice:** User requested all remaining areas follow recommendations.
**Notes:** Test-only controls are allowed if not documented as stable public API.

## the agent's Discretion

- Exact helper names, adapter struct layout, and test fixture organization.
- Whether route diagnostics are exposed through internal structs, debug strings,
  or test-only hooks, as long as public SQL remains `loom_scan(path)`.

## Deferred Ideas

- Chunked DataChunk output and ArrowArrayStream/record-batch public ABI.
- Parallel split execution and per-worker native cache.
- Predicate pushdown into runtime/native backend.
- Public SQL mode knobs.
- Persistent native artifact cache hardening and broad equivalence matrices.
