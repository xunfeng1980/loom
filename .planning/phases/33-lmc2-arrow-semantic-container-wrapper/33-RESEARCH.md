# Phase 33: LMC2 Arrow Semantic Container Wrapper - Research

**Gathered:** 2026-06-09
**Status:** Complete

## Research Question

What needs to be known to plan the `LMC2` wrapper phase well?

## Summary Recommendation

Implement `LMC2` as a narrow Arrow-semantic wrapper codec in `loom-core`, route it
through `verify_artifact` before the existing `LMC1` branch, and update source
adapters to emit `LMC2(LMA1)` by default while retaining direct `LMA1` helper
compatibility.

The safest implementation path is:

1. Add `LMC2` encode/decode helpers near the existing direct `LMA1` codec.
2. Add artifact-verifier routing and facts for `LMC2`, preserving direct `LMA1`.
3. Shift Parquet/Lance/Vortex accepted source emission to the wrapper.
4. Update focused gates, DuckDB source e2e compatibility checks, CLI/report
   visibility, and docs without broadening SQL/native claims.

## Current Code Findings

### Core Codec

- `crates/loom-core/src/arrow_semantic.rs` already defines `LMA1_MAGIC` and
  `LMC2_MAGIC`.
- `crates/loom-core/src/arrow_semantic_codec.rs` currently encodes and decodes
  only direct `LMA1`.
- `is_arrow_semantic_container(bytes)` exists but is marker-only; no wrapper
  encode/decode path consumes `LMC2`.
- The existing codec has a local `Reader` that already handles checked
  little-endian reads, length overflow, truncation, and trailing bytes. It can
  be reused or mirrored for wrapper parsing.

### Artifact Verifier

- `crates/loom-core/src/artifact_verifier.rs` first checks direct `LMA1` via
  `is_arrow_semantic_payload(bytes)`, then falls through to `decode_container`
  for `LMC1`.
- Without Phase 33 routing, `LMC2` bytes would be reported as malformed `LMC1`
  container bytes rather than Arrow semantic wrapper bytes.
- Direct `LMA1` accepted facts currently include artifact kind `LMA1`, payload
  kind `Arrow semantic payload`, schema presence, row count, and lowering
  readiness deferral when requested.

### LMC1 Container Pattern

- `crates/loom-core/src/container_codec.rs` provides useful patterns:
  versioned magic, required/optional feature bitsets, section flags, checked
  header/section lengths, offset overflow rejection, and unknown required
  feature rejection.
- Phase 33 should borrow the fail-closed style but avoid copying legacy
  `LMP1`/`LMT1` payload semantics or all `LMC1` section kinds into `LMC2`.

### Source Ingress

- Parquet and Lance source adapters use private `loom_artifact_from_batches`
  helpers that call `encode_arrow_semantic_payload(&payload)`.
- Vortex source ingress performs the same direct `LMA1` encode inline in
  `emit_source_ingress_lma1_from_vortex_buffer`.
- Each source adapter immediately calls `verify_artifact` on emitted bytes.
  That makes the source cutover natural: wrap after direct `LMA1` encode, then
  verify the wrapped bytes.
- Existing source report summary strings name `LMA1`; they should name `LMC2`
  wrapping `LMA1` after the cutover.

### Gates and DuckDB Bridge

- `scripts/full-arrow-semantic-compatibility-test.sh` checks for direct `LMA1`
  markers and runs core/source semantic tests.
- `scripts/duckdb-source-e2e-test.sh` generates source-backed fixtures and
  currently asserts the first four bytes are `LMA1`.
- Phase 33 must update this e2e gate carefully: DuckDB SQL broadening is not in
  scope, but source fixtures may now be wrapped. The e2e path must either teach
  the existing bridge to unwrap `LMC2(LMA1)` for the same bounded single-column
  path or keep a documented direct-`LMA1` compatibility fixture while separately
  gate-testing wrapper emission.

## Planning Implications

### Plan 1: Core Wrapper Codec

The first plan should define the wrapper grammar and tests in `loom-core`.
Likely artifacts:

- `ArrowSemanticContainerDescription` or equivalent.
- `encode_arrow_semantic_container_payload`.
- `decode_arrow_semantic_container_payload`.
- `wrap_arrow_semantic_payload`.
- `unwrap_arrow_semantic_payload`.
- `is_arrow_semantic_container`.

Required negative cases:

- wrong magic,
- unsupported version,
- header too short or truncated,
- unknown required feature,
- missing required `LMA1` payload section,
- malformed inner `LMA1`,
- trailing bytes,
- offset/length overflow or section outside container.

### Plan 2: Artifact Verifier Routing

The verifier should check `LMC2` before the `LMC1` container branch and should
retain direct `LMA1` support. Facts should identify `LMC2` while carrying the
Arrow semantic payload summary downstream.

Lowering readiness remains not ready with the Arrow semantic deferral diagnostic.

### Plan 3: Source Cutover

Adapters should emit `LMC2(LMA1)` by default. Direct `LMA1` helpers remain
available or are renamed only with compatibility shims.

Tests should assert:

- emitted source bytes start with `LMC2`,
- `verify_artifact` accepts them,
- unwrapped/decoded Arrow semantic payload still equals source/oracle Arrow
  batches,
- report summaries name `LMC2`.

### Plan 4: Visible Surfaces and Focused Gate

CLI/report/gate updates should make `LMC2` visible without making new query or
native claims. The gate should be focused before broad release wiring.

Likely script: `scripts/lmc2-arrow-semantic-container-test.sh`.

### Plan 5: Release Wiring and Closeout

Final closeout should wire the focused gate into the broad verifier, update
public/planning docs, and produce a Phase 33 report that states what is proven
and not proven.

## Risks and Guardrails

- **Overclaim risk:** `LMC2` acceptance is not DuckDB nested/logical SQL support
  and not native execution.
- **Compatibility risk:** direct `LMA1` helpers and tests should remain so Phase
  31 evidence is not erased accidentally.
- **Verifier-routing risk:** `LMC2` must not fall through to the legacy `LMC1`
  branch.
- **Gate-drift risk:** scripts that assert `LMA1` magic need deliberate updates,
  not accidental weakening.
- **Dependency risk:** all wrapper logic belongs in `loom-core` and must remain
  source-reader-free.

## Verification Architecture

Use layered verification:

1. Core codec unit tests for positive and malformed `LMC2`.
2. Artifact verifier tests for direct `LMA1`, wrapped `LMC2(LMA1)`, facts, and
   lowering deferral.
3. Source adapter tests for default `LMC2` emission and Arrow equality.
4. Focused shell gate asserting marker strings, running core/source tests, and
   checking public docs do not claim Phase 34/35 behavior.
5. Broad release gate wiring after the focused gate passes.

## RESEARCH COMPLETE

