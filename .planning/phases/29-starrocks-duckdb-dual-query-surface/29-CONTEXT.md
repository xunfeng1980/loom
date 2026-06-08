# Phase 29: StarRocks + DuckDB Dual Query Surface - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning
**Mode:** Autonomous smart discuss; recommended defaults accepted per user preference

<domain>
## Phase Boundary

Phase 29 proves that the same Phase 28 Iceberg-bound, verifier-backed Loom
table artifacts can feed two query surfaces: the existing DuckDB `loom_scan`
surface and a StarRocks-compatible query surface contract. The phase should
compare integration seams and query behavior across the two engines while
preserving one Loom artifact/binding contract. It should not invent a second
artifact format, broaden Iceberg semantics, add remote catalogs, or require a
production StarRocks cluster.

</domain>

<decisions>
## Implementation Decisions

### Query Surface Shape

- Keep DuckDB as the real executable host surface: reuse `loom_scan(path)` and
  the existing DuckDB smoke/native gates instead of adding a new public SQL
  function.
- Add a narrow adapter-local StarRocks query-surface proof that emits and
  validates StarRocks-compatible scan/query descriptors over Phase 28 binding
  facts. Do not add a networked StarRocks FE/BE deployment in this phase.
- The shared input must be the Phase 28 local Iceberg binding plus a
  verifier-accepted Loom artifact. DuckDB and StarRocks surfaces must not fork
  the artifact identity, schema identity, snapshot identity, or source/oracle
  evidence model.
- The first query matrix should stay intentionally small: projection,
  filter-like predicate expression, count, sum, and deterministic row material
  checks over the current non-null primitive/table slice.

### Evidence and Trust Model

- Both surfaces must start from accepted Phase 28 binding evidence; sidecar
  claims alone are not sufficient.
- StarRocks-compatible surface output is accepted only when the binding is
  accepted and the generated query/descriptor records the same table UUID,
  schema ID, snapshot ID, artifact hash, row count, and projected columns.
- DuckDB evidence must remain executable through the existing extension path;
  StarRocks evidence may be a local contract/descriptor proof with syntax and
  semantic checks rather than a running cluster.
- Fail closed on stale Phase 28 binding evidence, schema/snapshot mismatch,
  artifact hash mismatch, unsupported remote/catalog/credential routes, and
  unsupported query features.

### Scope and Dependency Boundaries

- Do not add StarRocks server, client, JDBC/ODBC, REST, credential, catalog, or
  object-store dependencies unless research finds a small offline official
  parser/contract surface that does not create service coupling.
- Keep `loom-core`, `loom-ffi`, `loom-source-ingress`, and Phase 28 binding
  semantics source/query-engine neutral. StarRocks-specific vocabulary belongs
  in a Phase 29 adapter/report boundary.
- Do not change the public C ABI or DuckDB public SQL surface unless a plan
  explicitly proves backwards compatibility.
- Do not broaden full Vortex semantic compatibility, nested type coverage,
  distributed execution, predicate pushdown into native kernels, or Iceberg
  catalog commit semantics in this phase.

### Verification and Release Gate

- Produce a Phase 29 report, tentatively
  `29-DUAL-QUERY-SURFACE-REPORT.md`, that records the shared binding input,
  DuckDB executable evidence, StarRocks-compatible contract evidence, mismatch
  matrix, non-goals, and current-phase tradeoffs.
- Add a focused gate, tentatively `scripts/dual-query-surface-test.sh`, and
  wire it into `scripts/mvp0-verify.sh` after Phase 28's Iceberg binding gate
  and before the existing DuckDB smoke closeout.
- Include negative tests for query-surface creep, separate artifact formats,
  StarRocks credential/network routes, unsupported query features, unchecked
  Phase 28 sidecar success, and stale schema/snapshot/artifact identity.
- Keep the release proof deterministic on a clean checkout.

### Current-Phase Tradeoffs

- Prefer a StarRocks-compatible offline contract over a real StarRocks cluster.
  This is less end-to-end than a deployment, but keeps the phase reproducible
  without service orchestration, credentials, object stores, or catalog drift.
- Prefer the existing DuckDB `loom_scan(path)` executable proof over adding a
  new DuckDB table function for Iceberg-bound refs. This preserves the stable
  public surface and keeps Phase 29 focused on shared query behavior.
- Prefer a small query matrix over broad SQL compatibility. This proves the
  dual-surface seam before taking on StarRocks planner/executor semantics.
- Prefer adapter-local duplication of query-surface descriptors over changing
  Phase 28 binding types into a generic engine framework. This avoids freezing
  abstractions before the StarRocks surface shape is validated.

### the agent's Discretion

- Choose exact crate/module names, descriptor JSON shape, and report section
  names during planning.
- Decide whether the StarRocks-compatible proof is a Rust crate, a fixture
  generator, or a test-only module, provided it remains local, deterministic,
  and dependency-contained.
- Decide the exact query matrix, but include at minimum projection, aggregate,
  and fail-closed unsupported query cases.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets

- `crates/loom-iceberg-binding` provides the accepted local Iceberg binding
  contract and evidence source from Phase 28.
- `crates/loom-source-ingress` provides source-neutral accepted/unsupported/
  rejected report vocabulary.
- `loom-core::artifact_verifier::verify_artifact` remains the artifact trust
  gate.
- `duckdb-ext` and `scripts/duckdb-smoke-test.sh` are the executable DuckDB
  surface.
- `scripts/iceberg-binding-test.sh` is the closest focused-gate analog for
  Phase 29 ordering and scope-creep guards.

### Established Patterns

- New source/query adapters stay isolated in their own crate/module and carry
  dependency-boundary tests.
- Accepted reports require verifier acceptance and evidence; unsupported valid
  inputs may expose facts but no accepted bytes/query execution.
- Main release gates are added only after focused gates pass.
- Planning reports include non-goals and current-phase tradeoffs when a phase
  intentionally stops short of a larger integration.

### Integration Points

- Workspace manifests: root `Cargo.toml` and any Phase 29 adapter crate
  manifest.
- Phase 28 binding input: `crates/loom-iceberg-binding`.
- Existing DuckDB public surface: `duckdb-ext/src/loom_extension.cpp`,
  `crates/loom-ffi`, and `scripts/duckdb-smoke-test.sh`.
- Main release gate: `scripts/mvp0-verify.sh`.

</code_context>

<specifics>
## Specific Ideas

- Reuse the Phase 28 accepted fixture as the shared table binding input.
- Generate a deterministic StarRocks-compatible descriptor such as
  `table_ref`, `snapshot_id`, `schema_id`, `artifact_sha256`, `projection`,
  `predicate`, `expected_rows`, and `expected_aggregates`.
- Compare DuckDB query results against the same expected row/aggregate evidence
  used by the StarRocks-compatible descriptor.
- Make the focused gate assert that no StarRocks network, credential, or
  cluster-launch surface was introduced.

</specifics>

<deferred>
## Deferred Ideas

- Running a real StarRocks cluster, FE/BE lifecycle, JDBC/ODBC integration,
  external table registration, credential management, and object-store access.
- Production Iceberg REST catalog integration, branch/tag mutation, table
  commit semantics, and remote warehouse behavior.
- Full SQL dialect compatibility, distributed query execution, predicate
  pushdown into native kernels, split planning across engines, and nested/
  nullable type expansion.
- Full Vortex semantic compatibility remains Phase 30.

</deferred>

---

*Phase: 29-starrocks-duckdb-dual-query-surface*
*Context gathered: 2026-06-09 via autonomous smart discuss*
