# Phase 29: Iceberg Ref/Table Binding - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning
**Mode:** Autonomous smart discuss; recommended defaults accepted per user preference

<domain>
## Phase Boundary

Phase 29 defines and proves a narrow Iceberg ref/table binding for verifier-backed Loom distribution artifacts after Phase 27's Lance + Parquet archival-readability slice. The phase should establish how an Iceberg table/ref points at or carries Loom artifact references, schema/snapshot identity, source-ingress facts, verifier acceptance, oracle evidence, and fail-closed diagnostics. It should make a local, testable binding contract and a minimal artifact/metadata proof, not a production catalog, query-engine integration, object-store deployment, or broad Iceberg semantic compatibility layer.

</domain>

<decisions>
## Implementation Decisions

### Binding Surface
- Use a minimal adapter-local Iceberg binding crate or module as the first surface; do not add public SQL functions, public C ABI symbols, DuckDB routes, or StarRocks integration in Phase 29.
- Bind Loom artifacts as explicit sidecar/reference metadata associated with Iceberg table/ref identity rather than embedding Loom bytes into Iceberg manifests or Parquet footers.
- Represent Iceberg identity with bounded, source-neutral fields such as table UUID/name, snapshot ID, schema ID, manifest or metadata file location, and content/hash references where available.
- Keep the generic `loom-source-ingress` contract source-neutral; Iceberg-specific vocabulary belongs only in the Phase 29 adapter/reporting boundary.

### Evidence and Trust Model
- Accepted Iceberg-bound entries require an existing verifier-accepted Loom artifact and must carry the verifier summary forward as evidence, not as an unchecked trust token.
- Reuse Phase 26/27 semantics: accepted bindings require facts plus verifier acceptance plus oracle/equivalence evidence; unsupported valid Iceberg metadata may expose facts but no accepted Loom binding; malformed metadata must expose diagnostics only.
- Treat Iceberg schema/snapshot facts as descriptive until they are matched to verifier-accepted Loom artifact identity and source evidence.
- The binding should be fail-closed on stale snapshot/schema/artifact hash mismatches and on any attempt to treat manifest-only metadata as an accepted artifact proof.

### Scope and Dependency Boundaries
- Research current Iceberg Rust/project APIs before planning; choose dependencies only after primary-source verification.
- Isolate any Iceberg SDK dependency in a source-specific adapter crate; `loom-core`, `loom-ffi`, `loom-source-ingress`, public headers, CLI public route surfaces, and DuckDB host code must remain Iceberg-SDK-free unless explicitly planned as a private test-only guard.
- Local-file metadata/fixture handling is in scope. Remote catalogs, REST catalog auth, object-store credentials, warehouse configuration, production table commits, branch/tag mutation, and snapshot lifecycle management are out of scope.
- Do not broaden Lance/Parquet source compatibility, native kernels, predicate pushdown, split execution, nullable/nested semantic coverage, or full Vortex compatibility in this phase.

### Verification and Release Gate
- Produce a Phase 29 binding report, tentatively `29-ICEBERG-BINDING-REPORT.md`, that records binding schema, accepted/unsupported/rejected matrix, source evidence, verifier evidence, oracle/equivalence evidence, non-goals, and current-phase tradeoffs.
- Add a focused gate, tentatively `scripts/iceberg-binding-test.sh`, and wire it into `scripts/mvp0-verify.sh` only after the focused gate passes.
- The release gate should prove ordering after Phase 27 Lance/Parquet ingress and before any Phase 29 dual-query surface.
- Include negative tests for public-surface creep, object-store/catalog credential creep, unchecked manifest-only success, stale snapshot/schema/hash mismatch, and source SDK leakage.

### Current-Phase Tradeoffs
- Prefer sidecar/reference binding over embedding Loom bytes into Iceberg manifests. This is less integrated but avoids freezing writer internals or depending on manifest mutation semantics before the binding contract is proven.
- Prefer a local fixture and metadata proof over real catalog operations. This keeps Phase 29 deterministic and reviewable while deferring production catalog commit semantics.
- Prefer a narrow accepted primitive/table slice inherited from Phase 27 over broad Iceberg type coverage. This keeps the binding tied to verifier-backed Loom artifacts rather than source-format compatibility claims.
- Prefer adapter-local dependency isolation even if it duplicates some metadata mapping logic. This preserves the core/source-ingress neutrality established in Phase 26.

### the agent's Discretion
- Choose exact crate/module names, fixture formats, and report section names during planning, provided the binding remains narrow, local, verifier-backed, and dependency-isolated.
- Decide whether the first proof uses hand-authored Iceberg metadata fixtures, SDK-generated local metadata fixtures, or a combination, based on current primary-source API research and build reliability.
- Decide the exact mismatch dimensions to test, but include at minimum schema identity, snapshot identity, artifact hash/content identity, and verifier status mismatch.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `loom-source-ingress` provides source-neutral `SourceFacts`, diagnostics, support classification, emission disposition, oracle evidence, and accepted artifact handoff vocabulary.
- `loom-core::artifact_verifier::verify_artifact` is the trust gate for `LMC1` artifacts.
- `loom-parquet-ingress` and `loom-lance-ingress` show the current adapter-local pattern for source facts, accepted/unsupported/rejected classification, verifier-routed emission, oracle evidence, legacy/readability fixtures, and dependency boundary tests.
- `scripts/lance-parquet-ingress-test.sh` and `scripts/source-ingress-contract-test.sh` are the closest gate analogs.

### Established Patterns
- Source adapters are isolated crates with exact workspace dependency pins and focused dependency-boundary tests.
- Accepted reports are constructed only after verifier acceptance and oracle/equivalence evidence.
- Unsupported valid inputs may expose facts but no artifact bytes; rejected malformed inputs expose diagnostics only.
- Release gates are focused scripts first, then wired into `scripts/mvp0-verify.sh` after passing.

### Integration Points
- Workspace manifests: root `Cargo.toml` and adapter crate manifests.
- Source-neutral contract: `ingress/loom-source-ingress/src/lib.rs`.
- Artifact verifier: `crates/loom-core/src/artifact_verifier.rs`.
- Existing source adapter analogs: `ingress/loom-parquet-ingress`, `ingress/loom-lance-ingress`, and `ingress/loom-vortex-ingress`.
- Main release gate: `scripts/mvp0-verify.sh`.

</code_context>

<specifics>
## Specific Ideas

- Use Phase 27's paired Loom artifact pattern as the default archival package model: source/table metadata references a verifier-accepted Loom artifact rather than embedding Loom bytes into the source format.
- Treat Phase 29 as the handoff to Phase 29: define table/ref metadata well enough that a later dual-query phase can consume the same Loom-bound table artifact from StarRocks and DuckDB surfaces.
- Keep a visible `Current-Phase Tradeoffs` section in the final Phase 29 report.

</specifics>

<deferred>
## Deferred Ideas

- StarRocks and DuckDB dual query surfaces remain Phase 29.
- Full Vortex semantic compatibility remains Phase 30.
- Production Iceberg catalog commits, REST catalog auth, object-store credentials, branch/tag mutation, and remote warehouse semantics are deferred.
- Embedding Loom bytes into Iceberg manifests or Parquet footers is deferred until sidecar/reference binding is proven.
- Broad Iceberg type coverage, nested/nullable semantics, predicate pushdown, split execution, and new native kernels are deferred.

</deferred>

---

*Phase: 29-iceberg-ref-table-binding*
*Context gathered: 2026-06-09 via autonomous smart discuss*
