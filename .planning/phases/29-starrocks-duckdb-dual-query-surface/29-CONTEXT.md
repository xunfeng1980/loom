# Phase 29: StarRocks + DuckDB Dual Query Surface - Context

**Gathered:** 2026-06-09
**Status:** Ready for research and planning

<domain>
## Phase Boundary

Phase 29 proves that the same Phase 28 Loom/Iceberg-bound table artifact can be consumed through two query surfaces: the existing DuckDB `loom_scan(path)` path and a StarRocks surface. The value proof is query-result equivalence over the same table identity, schema/snapshot binding, verifier-accepted Loom bytes, and source/oracle evidence.

This is a bounded dual-query proof, not a production StarRocks connector, not a public ABI expansion, and not a new artifact format.

</domain>

<decisions>
## Implementation Decisions

### Query Surface Shape

- **D-01:** Use the Phase 28 binding as the shared source of truth. The phase must not create a second artifact format, second source-ingress framework, or StarRocks-specific sidecar.
- **D-02:** DuckDB remains the existing public `loom_scan(path)` surface. Do not add a route-specific public SQL function such as `loom_scan_iceberg` or `loom_scan_starrocks`.
- **D-03:** StarRocks proof should use the most natural narrow surface available for this milestone: materialize/query the verifier-accepted Loom table through a StarRocks table/load path and compare SQL results. Avoid a first-pass custom StarRocks BE scanner or C++ connector unless research proves it is smaller and more reliable than the load/query proof.
- **D-04:** The minimum accepted query equivalence is row count plus deterministic aggregate/projection checks over the current non-null primitive/table slice. A broader SQL conformance matrix is deferred.

### Evidence and Trust

- **D-05:** Both engines must consume evidence derived from the same accepted Iceberg binding facts: table UUID, table name, schema ID, snapshot ID, artifact SHA-256, source evidence, oracle evidence, and verifier facts.
- **D-06:** StarRocks-loaded rows are evidence of query-surface compatibility, not a new trusted source. The verifier-accepted Loom artifact remains the trust anchor.
- **D-07:** Any adapter-generated CSV/fixture used for StarRocks loading must be derived from verified Loom bytes after Phase 28 binding acceptance, and its hash/provenance must be reported.
- **D-08:** Query-result comparison must fail closed on schema mismatch, snapshot mismatch, artifact hash mismatch, row mismatch, unsupported StarRocks environment, or skipped StarRocks runtime without explicit skip configuration.

### Environment and Release Gate

- **D-09:** Add a focused Phase 29 gate script that always verifies the binding/query-surface contract and DuckDB side. StarRocks runtime smoke may be skip-aware only when an explicit environment flag is set and Docker/StarRocks is unavailable; skipped runtime evidence must not be reported as a StarRocks pass.
- **D-10:** Docker is an optional runtime dependency for the StarRocks smoke, not a required crate/workspace dependency. If used, the script must make daemon/image availability explicit.
- **D-11:** The main release verifier should run the Phase 29 gate after Phase 28 Iceberg binding and before the final DuckDB smoke.

### Boundaries

- **D-12:** Do not add official Iceberg SDK, object-store credentials, remote catalogs, warehouse mutation, branch/tag mutation, or production StarRocks catalog controls by default.
- **D-13:** Do not expose StarRocks names through `loom-core`, `loom-source-ingress`, public FFI headers, or generic runtime ABI types. StarRocks-specific code, if any, must stay in an adapter/test boundary.
- **D-14:** Do not claim full engine independence. This phase should record exactly which runtime ABI assumptions remain DuckDB-shaped after the StarRocks comparison.

### the agent's Discretion

The user approved recommended defaults. Downstream agents may choose the narrowest implementation that proves D-01 through D-11 without pulling in a production StarRocks connector. Record current-phase tradeoffs in the report.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Upstream Phase Artifacts

- `.planning/ROADMAP.md` — Phase 29 boundary and ordering decision.
- `.planning/PROJECT.md` — key decision that Phase 29 should prove StarRocks + DuckDB over the same Loom/Iceberg-bound artifacts.
- `.planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RUNTIME-ABI-REPORT.md` — host-neutral runtime ABI claims and known DuckDB-shaped assumptions.
- `.planning/phases/24-duckdb-native-execution-integration-mvp/24-DUCKDB-NATIVE-REPORT.md` — existing DuckDB native query integration surface and direct DataChunk route.
- `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-NATIVE-HARDENING-REPORT.md` — bounded equivalence/cache/fallback evidence before table-format visibility.
- `.planning/phases/28-iceberg-ref-table-binding/28-CONTEXT.md` — Phase 28 decisions and deferred Phase 29 boundary.
- `.planning/phases/28-iceberg-ref-table-binding/28-ICEBERG-BINDING-REPORT.md` — binding facts, accepted/unsupported/rejected matrix, release gate evidence, and Phase 29 handoff.
- `.planning/phases/28-iceberg-ref-table-binding/28-REVIEW-FIX.md` — hardened source/oracle evidence and sidecar path policy from code review.

### StarRocks Official Documentation

- `https://docs.starrocks.io/docs/loading/StreamLoad/` — Stream Load from local file system; useful for a bounded StarRocks table-load query proof.
- `https://docs.starrocks.io/docs/sql-reference/sql-statements/table_bucket_part_index/CREATE_TABLE/` — table creation syntax and key/distribution constraints.
- `https://docs.starrocks.io/docs/sql-reference/sql-functions/JAVA_UDF/` — Java UDF/UDTF constraints; UDTF returns multiple rows of one column, so it is not the recommended first table-scan proof.
- `https://docs.starrocks.io/docs/quick_start/shared-nothing/` — Docker quickstart reference for optional local StarRocks smoke.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `crates/loom-iceberg-binding` — accepted binding API and fail-closed report contract for the shared table/ref identity.
- `scripts/iceberg-binding-test.sh` — pattern for focused dependency/scope guards and release-gate wiring.
- `duckdb-ext/loom_extension.cpp` and `scripts/duckdb-smoke-test.sh` — existing DuckDB SQL surface and smoke-query proof.
- `crates/loom-core::artifact_verifier` — verifier gate that remains the trust anchor before any query-surface materialization.

### Established Patterns

- Source/table-format adapters are isolated crates or focused test boundaries; generic crates stay source/engine-neutral.
- Release gates can be skip-aware for heavyweight external tools only when the skip is explicit and recorded.
- Reports must distinguish accepted verifier evidence from descriptive metadata or runtime environment availability.

### Integration Points

- A Phase 29 query-surface contract can sit beside `loom-iceberg-binding` or in a new adapter/test crate if research shows a crate is justified.
- The focused gate should be wired into `scripts/mvp0-verify.sh` after `scripts/iceberg-binding-test.sh`.
- DuckDB result checks can reuse existing generated `.loom` fixtures or Phase 28 binding fixtures; StarRocks rows should be generated from the same verified binding/artifact path.

</code_context>

<specifics>
## Specific Ideas

- Preferred MVP: derive a canonical tabular fixture from a verifier-accepted Loom/Iceberg binding, run deterministic DuckDB SQL over `loom_scan(path)`, load/query equivalent rows in StarRocks, and compare result records in one report.
- Current local Docker daemon is not running. Planning must not assume a StarRocks container is already available.

</specifics>

<deferred>
## Deferred Ideas

- Production StarRocks BE/C++ scanner or connector.
- StarRocks Iceberg catalog integration, remote object store access, credential handling, or branch/tag mutation.
- Full SQL conformance across engines.
- Full arbitrary Vortex semantic compatibility, which remains Phase 30.

</deferred>

---

*Phase: 29-StarRocks + DuckDB Dual Query Surface*
*Context gathered: 2026-06-09*
