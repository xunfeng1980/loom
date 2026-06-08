# Phase 27: Lance + Parquet Archival Readability / Dataset Ingress - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning
**Mode:** Autonomous smart discuss; recommended answers accepted per user preference to follow recommendations while recording tradeoffs.

<domain>
## Phase Boundary

Phase 27 applies the Phase 26 source-ingress contract to the first non-Vortex
source families: Lance datasets and Parquet files. The value proof is archival
readability, not broad format compatibility: supported source shapes should
produce verifier-backed Loom artifacts that remain readable through Loom even if
the original source reader version changes.

This phase should implement narrow, isolated Lance and Parquet adapter slices
that extract source-neutral facts, diagnostics, support classification,
emission disposition, verifier-routed `LMC1`/`LMT1` or `LMC1`/`LMP1`
artifacts, and oracle/equivalence evidence through `loom-source-ingress`.

The first accepted slice should be deliberately small: Arrow-compatible,
non-null primitive single-column/table shapes that can be canonicalized into
existing Loom raw/table payloads and verified by the existing artifact verifier.
Adapters may expose additional fact-bearing metadata for unsupported valid
inputs, but unsupported inputs must emit no `.loom` bytes and rejected malformed
inputs must expose no trusted facts.

This phase must not become Iceberg binding, StarRocks/DuckDB dual-query work,
MCAP/Zarr/LeRobot support, arbitrary Lance/Parquet semantic compatibility,
object-store credential handling, remote IO policy, predicate pushdown,
parallel split execution, Lance index support, Parquet writer-internals support,
manifest embedding, public SQL/API expansion, or new native kernels.

</domain>

<decisions>
## Implementation Decisions

### Source Coverage
- Recommended: include both Lance and Parquet in the phase, but keep each to a
  minimal local-file adapter slice. Tradeoff: this proves the source-ingress
  contract generalizes across two Arrow-adjacent formats without turning the
  phase into a full source framework.
- Recommended: accepted emission should start with non-null primitive
  Int32/Int64/Float32/Float64 single-column and simple table shapes already
  representable by `LMP1`/`LMT1`. Tradeoff: this chooses durable Loom artifact
  evidence over broad nested/null/extension-type coverage.
- Recommended: facts may include Lance fragment/schema/version-style summaries
  and Parquet row-group/schema/page-adjacent summaries where available, but
  those facts are descriptive and source-neutral. Tradeoff: reviewers get
  archival evidence without freezing source SDK internals into Loom APIs.

### Adapter Boundary
- Recommended: create source-specific adapter crates or modules that own Lance
  and Parquet SDK dependencies and map into `loom-source-ingress`. Tradeoff:
  extra adapter boilerplate preserves the Phase 26 dependency boundary.
- Recommended: keep `loom-core`, `loom-ffi`, `loom-source-ingress`, DuckDB
  extension code, and public headers free of Lance/Parquet/source SDK deps.
  Tradeoff: host/query integration remains deferred, but the artifact contract
  stays portable.
- Recommended: do not expose Lance/Parquet SDK objects, dataset handles,
  readers, credentials, Arrow stream ownership handles, or object-store state in
  generic public types. Tradeoff: some useful metadata must be summarized as
  strings/facts rather than passed through directly.

### Archival Readability Proof
- Recommended: require two value proofs for each source family where feasible:
  current-version read/write/verify and legacy-file-with-Loom readability. The
  legacy proof may use checked-in or generated fixtures if research identifies a
  stable, license-safe fixture path. Tradeoff: this proves durability intent
  without designing a full archival container format.
- Recommended: emitted Loom artifacts should be paired with source facts and
  verifier/oracle evidence rather than embedded into Lance manifests or Parquet
  footers in this phase. Tradeoff: pairing is less integrated but avoids
  source-format writer internals and long-term compatibility traps.
- Recommended: if old-version source writer tooling is brittle or unavailable,
  prefer a small fixture compatibility matrix and explicitly record the gap in
  the report instead of widening dependencies. Tradeoff: avoids spending the
  phase on historical build archaeology while still capturing archival risk.

### Oracle And Equivalence
- Recommended: Parquet oracle should use an Arrow/Parquet scan path or decoded
  row fixture selected during research; Lance oracle should use Lance-native or
  Arrow-compatible scan output selected during research. Tradeoff: the oracle
  is source-specific evidence, not the Loom decode path.
- Recommended: accepted reports must require `SourceOracleEvidence` and
  `SourceArtifactVerificationSummary::accepted`, matching Phase 26. Tradeoff:
  no source adapter can claim accepted archival readability from facts alone.
- Recommended: equivalence should compare rows from verified Loom artifacts
  against source-native/Arrow oracle output for the supported primitive/table
  slice. Tradeoff: row-level equality is narrow but concrete and reviewable.

### Reports And Gates
- Recommended: write a final `27-ARCHIVAL-READABILITY-REPORT.md` describing
  supported Lance/Parquet slices, source fact mapping, accepted/unsupported/
  rejected matrices, oracle evidence, archival-readability proof, dependency
  guards, tradeoffs, non-goals, and Phase 28 handoff.
- Recommended: add `scripts/lance-parquet-ingress-test.sh` or equivalent and
  wire it into `scripts/mvp0-verify.sh` only after focused adapter tests pass.
  Tradeoff: release evidence becomes one-command reproducible, but the script
  must stay bounded and skip/fixture-aware if source SDK tooling is unavailable.
- Recommended: all new source SDK package choices must be researched against
  current primary sources before implementation. Tradeoff: current crate/API
  reality is unstable enough that planning must verify versions rather than
  trusting memory.

### the agent's Discretion
- Choose exact crate names and module structure during research/planning, based
  on workspace patterns and dependency hygiene.
- Choose the smallest stable fixture strategy that proves current-version and
  legacy readability without adding remote services or credentials.
- Choose whether Lance and Parquet share helper functions only after a second
  adapter makes duplication real; do not introduce a broad plugin framework.
- Prefer local fixtures, deterministic generated data, and focused tests over
  external services.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope And Handoff
- `.planning/ROADMAP.md` - Phase 27 ordering, dependencies, and non-goals.
- `.planning/STATE.md` - Phase 27 current focus and Phase 26 closeout.
- `.planning/PROJECT.md` - project constraints and dependency-boundary decisions.
- `.planning/phases/26-external-source-ingress-contract/26-SOURCE-INGRESS-CONTRACT.md`
  - normative source-ingress contract.
- `.planning/phases/26-external-source-ingress-contract/26-SOURCE-INGRESS-REPORT.md`
  - Phase 27 handoff assumptions and source adapter obligations.
- `.planning/phases/26-external-source-ingress-contract/26-VERIFICATION.md`
  - verified Phase 26 must-haves and residual risks.

### Source Contract Code
- `crates/loom-source-ingress/src/lib.rs` - generic source facts, diagnostics,
  support/emission/lowering/oracle/verifier report contract.
- `crates/loom-source-ingress/tests/source_ingress_contract.rs` - stable
  vocabulary and fail-closed report invariant tests.
- `crates/loom-vortex-ingress/src/source_contract.rs` - reference source
  adapter mapping and verifier-routed accepted handoff.
- `crates/loom-vortex-ingress/tests/source_ingress_contract.rs` - reference
  mapping tests.
- `crates/loom-vortex-ingress/tests/source_ingress_handoff.rs` - accepted,
  unsupported, rejected, verifier, and oracle evidence tests.

### Artifact Targets And Gates
- `crates/loom-core/src/artifact_verifier.rs` - verifier accepted/rejected
  artifact report behavior.
- `crates/loom-core/src/container_codec.rs`, `crates/loom-core/src/layout_codec.rs`,
  and `crates/loom-core/src/table_codec.rs` - `LMC1`/`LMP1`/`LMT1` emission
  targets.
- `scripts/source-ingress-contract-test.sh` - Phase 26 source contract gate and
  dependency/API creep guard pattern.
- `scripts/mvp0-verify.sh` - main release-gate ordering.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `loom-source-ingress` already defines source-neutral reports and constructors
  that enforce accepted/unsupported/rejected invariants.
- `SourceIngressReport::accepted` requires accepted artifact verification and
  accepted oracle evidence; this should be reused directly.
- `SourceFacts`, `SourceSchemaFact`, `SourceLayoutFact`, `SourceSegmentFact`,
  and `SourceSplitFact` can describe Lance fragments and Parquet row groups as
  source-neutral facts.
- Vortex reference tests show the expected pattern: source-specific adapter
  extracts facts, emits `LMC1`, runs `verify_artifact`, then records oracle
  evidence before returning accepted bytes.
- `scripts/source-ingress-contract-test.sh` demonstrates dependency/API guard
  style for keeping source SDKs out of generic/core/ffi/DuckDB/public surfaces.

### Established Patterns
- Source SDK dependencies belong in isolated source-specific crates.
- Generic contract types must be plain Loom-owned data, not SDK handles.
- Valid-but-unsupported sources may expose facts but cannot emit partial bytes.
- Accepted artifact claims require verifier acceptance and oracle evidence.
- Release reports must state current-phase tradeoffs and non-goals explicitly.

### Integration Points
- New adapters should depend on `loom-source-ingress` and likely `loom-core`
  only for artifact encoding/verification, while keeping SDK dependencies out
  of `loom-core` itself.
- Emission should reuse `wrap_layout_payload`, `wrap_table_payload`,
  `encode_layout_payload`, `encode_table_payload`, and `verify_artifact`.
- CLI or public DuckDB integration is out of scope unless the planner proves an
  internal test-only helper is needed; public surfaces remain unchanged.

</code_context>

<specifics>
## Specific Ideas

- User preference from the autonomous workflow: follow recommended choices
  first, but record current-phase tradeoffs explicitly.
- Treat Lance and Parquet as archival-readability proof adapters, not broad
  query engines or format-compatibility promises.
- Prefer deterministic local fixtures and source-neutral reports that can be
  reviewed without source SDK knowledge.
- Research must verify current Lance and Parquet crate/API choices from
  primary sources before planning implementation details.

</specifics>

<deferred>
## Deferred Ideas

- Iceberg table/ref metadata binding: Phase 28.
- StarRocks + DuckDB dual query surface: Phase 29.
- Full arbitrary Vortex semantic compatibility: Phase 30.
- Embedding Loom artifacts into Lance manifests, Parquet footers, or source
  writer internals.
- Object-store credentials, remote IO policy, dataset catalog semantics,
  index semantics, predicate pushdown, projection pushdown, parallel split
  execution, nested/list/struct extension type coverage beyond the minimal
  primitive/table slice, public SQL/API changes, and new native kernels.

</deferred>

---

*Phase: 27-lance-parquet-archival-readability-dataset-ingress*
*Context gathered: 2026-06-09*
