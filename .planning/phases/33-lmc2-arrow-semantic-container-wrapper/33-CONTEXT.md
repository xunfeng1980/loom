# Phase 33: LMC2 Arrow Semantic Container Wrapper - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning
**Mode:** Text-mode discuss; user selected envelope, verifier facts, and source cutover, then accepted recommended defaults

<domain>
## Phase Boundary

Phase 33 implements the deferred `LMC2` distribution wrapper around verifier-backed
`LMA1` Arrow semantic payloads. It resolves the artifact-contract gap identified
by Phase 32: direct `LMA1` is implemented today, but `LMC2` is only a marker and
future wrapper.

The phase should make `LMC2(LMA1)` a real verifier-accepted artifact with
versioning, minimal wrapper metadata, source-ingress emission support, and
release-gated negative coverage. It must preserve direct `LMA1` compatibility as
an explicit legacy/current bridge, but new full Arrow semantic distribution
claims should point at `LMC2`.

This phase must not broaden DuckDB SQL shape support, claim native `LMA1` or
`LMC2` execution, add live StarRocks runtime integration, or redesign Arrow
semantic compatibility beyond wrapping and verifying the existing semantic
payload.

</domain>

<decisions>
## Implementation Decisions

### LMC2 Envelope Shape

- **D-33-01:** Implement `LMC2` as a semantic-specific wrapper, not a broad new
  universal container. The wrapper should carry a version and one required
  `LMA1` Arrow semantic payload section.
- **D-33-02:** Keep the wrapper minimal but distribution-oriented: enough
  feature/section metadata for fail-closed version and required-feature checks,
  without cloning every `LMC1` section kind or inventing future distribution
  machinery in this phase.
- **D-33-03:** Direct `LMA1` payloads remain accepted by the verifier and decode
  path as an explicit legacy/current bridge. New Phase 33 source-distribution
  evidence should prefer `LMC2(LMA1)`.

### Verifier Facts and Diagnostics

- **D-33-04:** `verify_artifact` must route both direct `LMA1` and wrapped
  `LMC2(LMA1)` fail-closed. `LMC2` should not fall through the `LMC1` container
  branch as an unknown malformed container.
- **D-33-05:** Accepted `LMC2` facts should expose at minimum artifact kind
  `LMC2`, wrapper version, required/optional feature names if present, payload
  kind `Arrow semantic payload`, schema presence, row-count bound, batch count
  when practical, and an inner `LMA1` acceptance summary.
- **D-33-06:** Lowering readiness remains not ready for Arrow semantic artifacts.
  `LMC2` verifier facts should preserve the same native-lowering deferral
  message as direct `LMA1`, rather than suggesting Phase 35 support exists.
- **D-33-07:** Negative diagnostics should distinguish malformed wrapper shape,
  unsupported wrapper version, unknown required feature, missing required
  `LMA1` payload, malformed inner payload, trailing bytes, and section
  offset/length overflow or truncation.

### Source-Ingress Cutover

- **D-33-08:** Parquet, Lance, and Vortex source-ingress accepted emission should
  produce verifier-accepted `LMC2(LMA1)` by default in Phase 33.
- **D-33-09:** Keep direct `LMA1` helper functions or compatibility shims where
  they are already used by tests, DuckDB e2e, or legacy evidence. Rename or add
  wrapper-emission entry points only where that makes the new default clear.
- **D-33-10:** Source reports should state that accepted bytes are `LMC2` wrapping
  an Arrow semantic `LMA1` payload. They should continue to describe source
  oracle evidence separately from artifact verifier acceptance.
- **D-33-11:** Existing full Arrow semantic equality tests remain source-to-Arrow
  semantic tests. Phase 33 adds wrapper acceptance and wrapper negative coverage;
  it does not expand the semantic compatibility matrix itself.

### the agent's Discretion

- Choose exact Rust type and function names for the `LMC2` codec, provided the
  public behavior above is clear and old direct `LMA1` helpers remain available.
- Choose whether `LMC2` lives in `arrow_semantic_codec.rs` or a neighboring
  module, provided `loom-core` stays source-reader-free.
- Choose exact CLI, report, fixture, and script updates needed to make the new
  wrapper visible. The selected discussion areas did not require a separate
  user decision on these surfaces.
- Choose focused release-gate names and wiring order, provided Phase 33 gets a
  focused gate before broader `mvp0`/`mvp1` wiring.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project Scope and Prior Decisions

- `.planning/ROADMAP.md` — Phase 33 goal, ordering decision, explicit non-goals,
  and downstream Phase 34/35 dependencies.
- `.planning/STATE.md` — Current accumulated decisions, especially Phase 31
  direct `LMA1` and Phase 32 `LMC2` deferral notes.
- `.planning/PROJECT.md` — Project value statement, dependency constraints, and
  no-overclaim posture.
- `.planning/REQUIREMENTS.md` — v3 full Arrow semantic compatibility requirement
  context and source compatibility boundaries.

### Recent Phase Context

- `.planning/phases/31-full-arrow-semantic-source-compatibility/31-CONTEXT.md`
  — Locks Arrow `Schema`/`ArrayData` as the semantic contract and introduces
  `LMC2` plus `LMA1`.
- `.planning/phases/31-full-arrow-semantic-source-compatibility/31-FULL-COMPATIBILITY-REPORT.md`
  — Phase 31 evidence and tradeoff record for direct `LMA1`.
- `.planning/phases/32-mvp1-architecture-and-code-review/32-CONTEXT.md`
  — Review boundary, claim-truth decisions, and `LMC2` deferred status.
- `.planning/phases/32-mvp1-architecture-and-code-review/32-MVP1-RELEASE-READINESS.md`
  — Current GO/no-go boundaries, including direct `LMA1` and deferred `LMC2`.

### Implementation Surfaces

- `crates/loom-core/src/arrow_semantic.rs` — `LMA1_MAGIC`, `LMC2_MAGIC`, and
  Arrow semantic payload model.
- `crates/loom-core/src/arrow_semantic_codec.rs` — direct `LMA1` codec and
  existing `LMC2` marker helper.
- `crates/loom-core/src/arrow_semantic_verifier.rs` — Arrow semantic verifier
  checks and diagnostics.
- `crates/loom-core/src/artifact_verifier.rs` — current direct `LMA1` verifier
  routing and fact construction.
- `crates/loom-core/src/container_codec.rs` — existing `LMC1` version,
  feature, section, and fail-closed container patterns to borrow sparingly.
- `crates/loom-core/tests/arrow_semantic.rs` — current direct `LMA1` roundtrip,
  nested schema, marker, and dependency-boundary tests.

### Source and Gate Surfaces

- `crates/loom-parquet-ingress/src/source_contract.rs` — Parquet source emission
  currently emits direct `LMA1`.
- `crates/loom-lance-ingress/src/source_contract.rs` — Lance source emission
  currently emits direct `LMA1`.
- `crates/loom-vortex-ingress/src/source_contract.rs` — Vortex source emission
  currently emits direct `LMA1`.
- `scripts/full-arrow-semantic-compatibility-test.sh` — Phase 31 focused gate
  that currently checks direct `LMA1` verifier routing.
- `scripts/duckdb-source-e2e-test.sh` — DuckDB source e2e gate currently expects
  generated source-backed fixtures to begin with `LMA1`; Phase 33 planning must
  decide the compatible update path without broadening SQL semantics.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `crates/loom-core/src/arrow_semantic_codec.rs` already defines stable direct
  `LMA1` encode/decode and `is_arrow_semantic_container` marker detection for
  `LMC2`; this is the natural starting point for a wrapper codec.
- `crates/loom-core/src/artifact_verifier.rs` already has a direct
  `verify_arrow_semantic_artifact` branch that constructs accepted facts for
  `LMA1`; Phase 33 should add a sibling or shared path for `LMC2`.
- `crates/loom-core/src/container_codec.rs` provides section directory,
  required-feature, unknown-feature, and truncation/overflow patterns. Reuse the
  fail-closed style without copying legacy `LMP1`/`LMT1` semantics into `LMC2`.
- Source adapters already funnel materialized Arrow batches through
  `encode_arrow_semantic_payload`, then `verify_artifact`; wrapping can be
  added near that handoff.

### Established Patterns

- Accepted artifacts are accepted only after `verify_artifact` succeeds; source
  sidecar/oracle facts alone are never sufficient.
- `loom-core` and `loom-ffi` stay free of Parquet, Lance, and Vortex reader
  dependencies.
- Unsupported or malformed artifact bytes fail closed with stable diagnostics
  and no partial accepted facts.
- New evidence gates should first be focused and deterministic, then wired into
  broader verification scripts.
- Documentation and reports must say what the gate proves and what it does not
  prove.

### Integration Points

- Core wrapper codec and tests: `crates/loom-core/src/arrow_semantic_codec.rs`,
  `crates/loom-core/src/artifact_verifier.rs`, and
  `crates/loom-core/tests/arrow_semantic.rs`.
- Source emission defaults: Parquet, Lance, and Vortex `source_contract.rs`
  files.
- DuckDB source fixtures and e2e scripts: generator bins plus
  `scripts/duckdb-source-e2e-test.sh`.
- Public docs and planning reports: `README.md`, `README-zh.md`,
  `.planning/ROADMAP.md`, `.planning/STATE.md`, and a Phase 33 closeout report.

</code_context>

<specifics>
## Specific Ideas

- Keep the first `LMC2` version intentionally boring: magic, version,
  required/optional feature bitsets if needed, one required Arrow semantic
  payload section, and checked section length/offset rules.
- Let direct `LMA1` continue to decode so existing Phase 31 tests and any
  current DuckDB source e2e bridge can be updated deliberately instead of
  broken accidentally.
- In source reports, make the distinction visible: source materialized to Arrow,
  Loom encoded Arrow semantics as `LMA1`, wrapper accepted as `LMC2`, and oracle
  equality remained separate evidence.

</specifics>

<deferred>
## Deferred Ideas

- Broad DuckDB multi-column, nested, logical, and metadata-preserving SQL support
  belongs to Phase 34.
- Native execution for Arrow semantic payloads belongs to Phase 35.
- Live StarRocks runtime integration and full dual query-surface completion are
  outside Phase 33.
- A universal future distribution container with signatures, remote fetch,
  attestation, encryption, or cache policy is outside this wrapper phase.

</deferred>

---

*Phase: 33-lmc2-arrow-semantic-container-wrapper*
*Context gathered: 2026-06-09*
