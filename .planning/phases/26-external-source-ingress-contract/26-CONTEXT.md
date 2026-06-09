# Phase 26: External Source Ingress Contract - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning
**Mode:** Autonomous smart discuss; recommended answers accepted per user preference to follow recommendations while recording tradeoffs.

<domain>
## Phase Boundary

Phase 26 defines a source-neutral ingress contract before adding Lance, Parquet,
Iceberg, MCAP, Zarr, LeRobot, or other source-specific integrations. It should
abstract the proven `loom-vortex-ingress` boundary into stable Loom-owned
concepts for source facts, diagnostics, support classification, emission
disposition, dependency isolation, verifier-routed `LMC1`/`LMT1` emission,
oracle/equivalence evidence, and fail-closed unsupported/rejected behavior.

This phase is a contract and scaffolding phase, not a new source reader phase.
It must not implement Lance/Parquet ingestion, Iceberg binding, host-engine
integration, object-store credential handling, predicate pushdown, parallel
split execution, or arbitrary Vortex semantic compatibility. Source-specific
readers may be represented only through examples, fixtures, mock adapters, or
contract tests that prove the generic shape.

</domain>

<decisions>
## Implementation Decisions

### Contract Shape
- Recommended: define a generic source-ingress vocabulary in Loom-owned terms:
  source identity, source kind, version/fingerprint, schema/facts, layout or
  segment facts where available, diagnostics, support status, emission kind,
  emission disposition, oracle evidence, and lowering disposition. Tradeoff:
  this gives downstream source phases a shared contract without forcing every
  source to expose Vortex-like internals.
- Recommended: keep the contract narrow enough to map cleanly from
  `VortexReaderFacts`, `VortexEncodingCoverage`, and `VortexIngressReport`, but
  name generic types without `Vortex` in their public identity. Tradeoff:
  Vortex remains the reference implementation while the contract does not leak
  source-specific terminology.
- Recommended: stable status vocabulary should preserve the accepted /
  unsupported / rejected triad. Unsupported valid sources may expose facts but
  must not emit partial `.loom` bytes. Tradeoff: useful diagnostics remain
  available without weakening fail-closed emission.

### Dependency Boundary
- Recommended: source-specific crates own source SDK dependencies. The generic
  contract should not add Lance, Parquet, Iceberg, MCAP, Zarr, LeRobot, or
  object-store dependencies to `loom-core`, `loom-ffi`, or DuckDB extension
  code. Tradeoff: a small amount of adapter boilerplate is preferable to
  coupling Loom's core artifact verifier to external source APIs.
- Recommended: reuse `loom-vortex-ingress` as the first adapter proving the
  contract, either by mapping existing Vortex facts into generic facts or by
  adding a source-neutral wrapper layer beside the current Vortex-specific API.
  Tradeoff: avoids rewriting the working reader while giving Phase 27 a stable
  target.

### Emission And Verification
- Recommended: contract-level emission is limited to verifier-routed `LMC1`
  wrapping `LMP1`/`LMT1` payloads for supported shapes. Every emitted artifact
  must pass the existing artifact verifier before being considered accepted.
  Tradeoff: the contract focuses on safe Loom artifact creation rather than
  broad source-format translation.
- Recommended: record emission disposition separately from source support:
  `none`, `canonical-raw`, `canonical-table`, and `structured-layout` style
  outcomes. Tradeoff: downstream phases can distinguish fact-bearing unsupported
  inputs from supported canonical emission and from future structured layouts.
- Recommended: lowering/native disposition remains descriptive metadata, not a
  trigger for new native kernels. Use `interpreter-only`,
  `production-lowering-supported`, and `fail-closed/deferred` style language.
  Tradeoff: Phase 26 can hand native execution facts to Phase 25 contracts
  without expanding native semantics.

### Oracle And Equivalence Evidence
- Recommended: require each source adapter to declare an oracle strategy:
  source-native scan, Arrow scan, decoded row fixture, or explicit unsupported
  reason. Tradeoff: this keeps equivalence auditable without requiring every
  source to expose the same scan API.
- Recommended: Phase 26 should add contract tests using Vortex as the real
  reference adapter and minimal mock/source-neutral fixtures for edge cases.
  Tradeoff: real evidence proves the mapping, mocks keep the contract from
  becoming Vortex-shaped.

### Diagnostics And Reports
- Recommended: diagnostics should retain stable code/path/message fields and
  source-neutral code families such as open/read/schema/layout/support/
  conversion/verification/oracle failures. Tradeoff: generic codes improve
  downstream consistency while adapters may still include source-specific detail
  in messages.
- Recommended: write a final `26-SOURCE-INGRESS-CONTRACT.md` or equivalent
  report that lists the generic model, Vortex mapping table, required adapter
  obligations, non-goals, and Phase 27 handoff assumptions.

### the agent's Discretion
- Choose whether generic contract types live in a new crate or an existing crate
  based on dependency hygiene and existing workspace patterns discovered during
  research.
- Prefer adapters and conversion traits only where they reduce real duplication;
  do not introduce a broad plugin framework unless the codebase already points
  that way.
- Keep Phase 26 evidence bounded to contract behavior, mapping, verifier routing,
  and release-gate coverage.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope And Handoff
- `.planning/ROADMAP.md` - Phase 26 ordering, dependencies, and non-goals.
- `.planning/STATE.md` - Phase 26 current focus and recent Phase 25 decisions.
- `.planning/PROJECT.md` - project constraints and key decisions through Phase
  25.
- `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-NATIVE-HARDENING-REPORT.md`
  - execution-contract baseline and non-goals inherited by Phase 26.

### Prior Ingress Contracts
- `.planning/phases/18-complete-vortex-reader/18-CONTEXT.md` - complete-reader
  boundary decisions.
- `.planning/phases/18-complete-vortex-reader/18-READER-CONTRACT.md` - current
  Vortex reader fact contract.
- `.planning/phases/18-complete-vortex-reader/18-SUMMARY.md` - completed Vortex
  reader evidence and release gate.
- `.planning/phases/21-expanded-vortex-encoding-coverage/21-CONTEXT.md` -
  coverage, emission, lowering, and dependency-boundary decisions.
- `.planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-MATRIX.md`
  - covered source shapes and disposition vocabulary.

### Code
- `ingress/loom-vortex-ingress/src/lib.rs` - existing Vortex facts,
  diagnostics, support, emission, coverage, oracle helpers, and artifact
  emission.
- `ingress/loom-vortex-ingress/tests/reader_facts_contract.rs` - stable reader
  fact/status/emission/lowering contract tests.
- `ingress/loom-vortex-ingress/tests/single_column_to_loom.rs` - supported
  primitive source-to-`LMC1` evidence.
- `ingress/loom-vortex-ingress/tests/table_to_loom.rs` - supported struct/table
  source-to-`LMT1` evidence and Vortex scan oracle comparison.
- `crates/loom-core/src/artifact_verifier.rs` - artifact verifier that emitted
  source artifacts must pass.
- `crates/loom-core/src/container_codec.rs`, `crates/loom-core/src/layout_codec.rs`,
  and `crates/loom-core/src/table_codec.rs` - `LMC1`/`LMP1`/`LMT1` emission
  target model.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `VortexReaderFacts` already captures source kind, version, row count, dtype
  facts, layout facts, segment facts, split facts, statistics, support,
  emission kind, coverage, and diagnostics.
- `VortexEncodingCoverage` already separates reader support, artifact emission,
  emission disposition, and native-lowering disposition.
- `VortexIngressReport` already models accepted, unsupported, and rejected
  outcomes with stable diagnostics and optional facts.
- `emit_supported_lmc1_from_vortex_buffer` and table/single-column tests provide
  verifier-routed artifact emission patterns.
- `scripts/complete-vortex-reader-test.sh` and
  `scripts/vortex-encoding-coverage-test.sh` are existing release-gate patterns
  for source/reader evidence.

### Established Patterns
- Source SDK dependencies stay isolated to source-specific crates.
- Facts are Loom-owned summaries, not raw external SDK types.
- Valid-but-unsupported inputs may expose facts, but cannot emit partial Loom
  artifacts.
- Oracle evidence is explicit and source-specific; it does not become the
  implementation path.
- Planning reports must state non-goals and downstream handoff assumptions.

### Integration Points
- Generic contract planning should start from the `loom-vortex-ingress` fact and
  report shapes, then decide the narrowest stable home for shared types.
- Emitted artifacts should continue through `loom_core::artifact_verifier`.
- Phase 27 should consume the generic contract for Lance/Parquet rather than
  copying Vortex-specific APIs.
- Phase 25 native execution is a downstream consumer of accepted/verifier-backed
  artifacts, not part of source-ingress contract implementation.

</code_context>

<specifics>
## Specific Ideas

- User preference from the current autonomous workflow: follow recommended
  choices first, but record current-phase tradeoffs explicitly.
- Treat Vortex as the reference implementation for the contract, not as the
  generic contract's vocabulary.
- Phase 26 should end with a reviewer-readable source-ingress contract/report and
  a release gate proving the mapping.

</specifics>

<deferred>
## Deferred Ideas

- Lance and Parquet implementation: Phase 27.
- Iceberg ref/table binding: Phase 28.
- StarRocks + DuckDB dual query surface: Phase 29.
- Full Vortex semantic compatibility: Phase 30.
- Object-store credentials, remote IO policy, dataset catalog semantics,
  source-specific indexing, and long-term archival rewrite behavior.
- Public SQL/API changes, predicate pushdown, parallel split execution, and new
  native kernels.

</deferred>

---

*Phase: 26-external-source-ingress-contract*
*Context gathered: 2026-06-09*
