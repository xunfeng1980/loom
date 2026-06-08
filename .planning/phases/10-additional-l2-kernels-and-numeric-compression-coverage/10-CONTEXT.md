# Phase 10: Additional L2 Kernels and Numeric Compression Coverage - Context

**Gathered:** 2026-06-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 10 extends Loom's L2 kernel path beyond FSST by adding numeric compression coverage for `COV-01`. The primary target is an ALP float L2 kernel covering both Float32 and Float64. Delta-of-delta integers are a fallback only if ALP hits a hard compile/API blocker.

This phase should prove the L2 registry/params/verifier/fixture/CLI/DuckDB path generalizes to a second real kernel. It must not expand into MLIR/native lowering, formal verification, distribution containers, random-access ABI work, or full `.vortex` file container support.

</domain>

<decisions>
## Implementation Decisions

### Kernel Target Selection
- **D-01:** Phase 10 primary kernel is ALP float, not a synthetic toy kernel.
- **D-02:** ALP must cover both Float32 and Float64.
- **D-03:** Delta-of-delta integers are the fallback kernel only if ALP has a hard compile/API blocker.
- **D-04:** Testing/oracle complexity alone is not a fallback trigger. If ALP compiles and stable fixtures can be constructed, planning should continue with ALP.

### Kernel ABI and Params Shape
- **D-05:** Add a dedicated `AlpParams` structure, following the role of `FsstParams`: stable encode/decode, verifier-readable fields, and negative tests.
- **D-06:** Kernel IDs are append-only: `FSST=0`, `ALP=1`, `delta fallback=2`. Existing IDs and fixtures must not be renumbered.
- **D-07:** `AlpParams` carries the decoded output type (`Float32` or `Float64`).
- **D-08:** The verifier must check that `AlpParams` output type matches `LayoutDescription.data_type`.
- **D-09:** Keep the current `L2Kernel` trait shape: `decode(params, count) -> ArrayData`. Do not add expected dtype to the trait in Phase 10.

### Oracle and Fixture Strategy
- **D-10:** Use a dual oracle strategy: Vortex-native oracle plus synthetic known-value/edge fixtures.
- **D-11:** Prefer exact bit equality for decoded floats where possible. If Vortex ALP introduces unavoidable rounding differences, use a documented fixed tolerance fallback for those cases only.
- **D-12:** ALP Float32 and Float64 fixtures must enter the DuckDB SQL gate, not just Rust tests or CLI checks.
- **D-13:** Fixture matrix should be small and representative: normal decimals, negative values, zero, repeated values, and nulls for both Float32 and Float64.
- **D-14:** NaN, infinities, subnormal values, and an exhaustive floating-point edge suite are not required for the first ALP pass.

### User-Visible Surface
- **D-15:** `loom inspect` should display kernel name, kernel id, output type, count, and a concise params summary for ALP. It should not dump full params bytes by default.
- **D-16:** README and README-zh should get a concise Phase 10 section and verification commands when ALP lands.
- **D-17:** `loom decode` should support finite Float32/Float64 values with stable plain decimal output and keep `NULL` for nulls.
- **D-18:** Do not extend illustrative timing output to ALP in Phase 10. The phase is about functional/kernel coverage, not benchmark messaging.

### the agent's Discretion

The agent may choose exact module names, test file layout, ALP params field encoding, and verifier diagnostic code names, provided the public decisions above hold and `loom-core` remains Vortex/FastLanes-free.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` — Phase 10 goal, dependency, and not-planned status.
- `.planning/REQUIREMENTS.md` — `COV-01` decode coverage requirement.
- `.planning/PROJECT.md` — active Phase 10 scope and remaining out-of-scope boundaries.
- `.planning/STATE.md` — current phase status and continuity.

### Prior Phase Context
- `.planning/phases/09-verifier-and-safety-boundary-mvp/09-CONTEXT.md` — verifier expectations for new kernel params, fail-closed decode routing, and diagnostic visibility.
- `.planning/phases/08-multi-column-table-output-and-arrow-stream-evaluation/08-CONTEXT.md` — table payload, DuckDB SQL gate, and release-gate expectations.
- `.planning/phases/07-human-readable-layout-descriptor-and-cli/07-CONTEXT.md` — descriptor and CLI inspect/decode constraints.

### Research Baseline
- `.planning/research/FEATURES.md` — original `COV-01` deferred item and ALP/delta positioning.
- `.planning/research/SUMMARY.md` — L2 kernel registry architecture and dependency boundary summary.
- `.planning/research/ARCHITECTURE.md` — `KernelEscape`, `L2KernelRegistry`, and multiple-kernel extension notes.
- `.planning/research/PITFALLS.md` — scope guardrails and Vortex fixture/oracle pitfalls.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/loom-core/src/l2_kernel_registry.rs` — current `L2Kernel` trait, registry, and FSST kernel implementation; Phase 10 adds ALP here or in a sibling module.
- `crates/loom-core/src/fsst_params.rs` — model for dedicated kernel params encode/decode tests; `AlpParams` should follow this pattern.
- `crates/loom-core/src/verifier.rs` — verifier must learn ALP kernel id/type/params validation and preserve typed diagnostics.
- `crates/loom-core/src/l1_model.rs` — `KernelEscape` decode routing and `LayoutDescription.data_type` integration point.
- `crates/loom-core/src/descriptor.rs` and `crates/loom-core/src/layout_codec.rs` — descriptor/payload roundtrip must support Float32/Float64 and ALP params.
- `crates/loom-cli/src/main.rs` — inspect/decode user-visible output surface for kernel names and floats.
- `crates/loom-fixtures/src/vortex_reader.rs` and `crates/loom-fixtures/src/oracle.rs` — Vortex fixture/oracle bridge for ALP if Vortex 0.74 exposes stable APIs.
- `scripts/duckdb-smoke-test.sh` and `scripts/mvp0-verify.sh` — release-gate integration points for ALP SQL coverage.

### Established Patterns
- `loom-core` must stay independent of Vortex/FastLanes dependencies.
- Kernel params are parsed and validated in `loom-core`, while Vortex-specific extraction stays in `loom-fixtures`.
- Public decode helpers and FFI ingress already route through verifier checks before Arrow output.
- `loom inspect` should stay concise and reviewer-facing.
- DuckDB acceptance is the strongest end-to-end proof for newly supported payloads.

### Integration Points
- Add Float32/Float64 support to Arrow builder/materialization paths where missing.
- Register ALP as kernel id `1` in the default MVP0 registry.
- Extend verifier, descriptor, layout codec, CLI inspect/decode, fixtures, and DuckDB smoke tests consistently.
- Preserve existing FSST, single-column, table, descriptor, verifier-negative, and DuckDB gates.

</code_context>

<specifics>
## Specific Ideas

- ALP is preferred because it better represents Loom's intended L2 role: a real compute kernel that L1 declarative layout cannot express.
- Delta-of-delta remains a fallback but should not be used merely to avoid oracle/test complexity.
- Phase 10 should demonstrate generality without prematurely introducing a generic L2 params envelope or changing the `L2Kernel` trait.

</specifics>

<deferred>
## Deferred Ideas

- Generic L2 params envelope.
- Named registry with numeric wire IDs.
- ALP timing output.
- NaN/Infinity/subnormal floating-point edge suite.
- MLIR/native lowering.
- Formal totality/termination verifier.
- Distribution container and full `.vortex` file support.

</deferred>

---

*Phase: 10-additional-l2-kernels-and-numeric-compression-coverage*
*Context gathered: 2026-06-08*
