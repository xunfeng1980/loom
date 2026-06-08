# Phase 13 Full Loom Verifier Report

**Status:** Complete verifier foundation
**Date:** 2026-06-08
**Gate:** `scripts/full-verifier-test.sh`

## Summary

Phase 13 completed a full-verifier foundation for the future Loom distribution
IR. It did not complete the final production verifier for all future artifacts.

The delivered foundation is deliberately layered:

- `L2Core` normative spec and proof-obligation matrix.
- Rust executable verifier with type/effect checks and abstract-state walking.
- SMT-ready local constraints for arithmetic, ranges, overflow, progress, and
  resource obligations.
- Lean scaffold for core semantics and accepted-program safety theorem names.
- TLA+ lifecycle model for verify-before-lower invariants.
- `VerifiedArtifactFacts` as the Phase 14 lowering-precondition handoff.

## What Is Complete

### L2Core

`13-VERIFIER-SPEC.md` defines the tiny Phase 13 `L2Core` subset: finite input
slices, bounded `ForRange`, monotone `CursorLoop`, scalar checked arithmetic,
typed Arrow builder events, explicit capabilities, resource budgets, and
fail-closed outcomes.

The Rust model in `crates/loom-core/src/l2_core.rs` mirrors that slice with:

- `L2CoreProgram`
- `Capability`
- `ResourceBudget`
- `ArrowEventType`
- `VerifiedArtifactFacts`

### Rust executable verifier

`crates/loom-core/src/full_verifier.rs` adds `verify_l2_core`, returning a
`FullVerificationReport` rather than a bare boolean. It emits stable diagnostic
codes, path-addressed diagnostics, proof-obligation traces, constraint comments,
and facts only for accepted programs.

Coverage in `crates/loom-core/tests/full_verifier.rs` includes:

- valid bounded copy program,
- missing input capability rejection,
- output type mismatch rejection,
- non-monotone cursor-loop rejection,
- overflow/range/progress constraint emission,
- `VerifiedArtifactFacts` presence for accepted programs,
- facts absence for rejected programs.

### SMT

`crates/loom-core/src/l2_core/constraints.rs` defines a solver-neutral local IR:

- `Le`
- `Lt`
- `Eq`
- `AddNoOverflow`
- `MulNoOverflow`
- `InRange`
- `Decreases`
- `NonNegative`
- `FeatureImplies`

The current output is deterministic diagnostic text via
`ConstraintSet::to_smtlib_comments`. Phase 13 intentionally does not add a Z3 or
SMT-LIB runtime dependency.

### Lean

`formal/lean/LoomCore.lean` is the Phase 13 Lean scaffold. It defines minimal
core terms for `L2Ty`, `Capability`, `ArrowEvent`, `Stmt`, `Program`,
`Verified`, and `Safe`, plus theorem names:

- `builder_events_well_formed`
- `accepted_program_safe`

This is a scaffold, not the complete final Loom soundness proof. Rocq remains a
fallback if later extraction or verified-checker lineage becomes mandatory.

Current limitation: the scaffold compiles without `sorry`, but the load-bearing
semantic predicates are placeholders. In `formal/lean/LoomCore.lean`,
`builder_events_typed` and `no_ambient_authority` are defined as `True`, so
`accepted_program_safe` proves the theorem target shape rather than a substantive
language soundness result. This is intentional Phase 13 scaffolding and must not
be counted as checked proof evidence for artifact safety.

The current load-bearing evidence is:

- the Rust executable verifier and artifact verifier diagnostics/facts, and
- Phase 19 Bitwuzla-backed discharge of solver obligations.

### TLA+

`specs/tla/LoomVerifierPipeline.tla` models lifecycle states and the
`LoweredImpliesVerified` invariant. It is scoped to workflow invariants:

- raw artifact must be parsed before verification,
- lowering must require verifier acceptance,
- required features must be accepted,
- resources must be bounded,
- verifier facts must be present.

TLA+ is not used as the L2 type-soundness proof.

### VerifiedArtifactFacts

`VerifiedArtifactFacts` is the Phase 14 handoff. It records artifact version,
features, input ranges, output schema, row-count bounds, loop bounds, resource
bounds, builder event types, capabilities, constraint IDs, and proof-obligation
IDs.

Phase 14 must consume these facts only as verifier-tied lowering preconditions.
The facts are not independent trust tokens.

## Phase 14 Lowering Preconditions

Phase 14 MLIR/native lowering should require:

- a successful `verify_l2_core` report,
- no unknown required features,
- finite input capabilities,
- finite loop bounds or discharged monotone progress obligations,
- represented arithmetic/range/resource constraints,
- typed Arrow builder events only,
- present `VerifiedArtifactFacts`,
- lifecycle invariant that lowering cannot occur before verifier acceptance.

## Deferred

Deferred beyond Phase 13:

- complete final production verifier for all future Loom IR features,
- complete Lean/Rocq soundness metatheory,
- real SMT solver integration,
- MLIR/native lowering implementation and correctness proof,
- real Vortex file/container ingress,
- signatures, attestation, content-addressed remote lookup, encryption, and
  remote fetch,
- semantic correctness proof of arbitrary producer intent.

These deferrals do not undermine Phase 13 because the phase goal was the
verifier foundation and evidence chain, not the complete final runtime.

## Gate Evidence

Phase 13 final verification:

```bash
cargo test --workspace
bash scripts/full-verifier-test.sh
bash scripts/safety-proof-test.sh
bash scripts/mvp0-verify.sh
git diff --check
```

`scripts/full-verifier-test.sh` checks Phase 13 documents, `VERIFIER-01` through
`VERIFIER-10`, Rust model/verifier tests, CLI visibility, managed Lean, and
managed TLC. Install the required formal tools with `mise install && mise run
formal-tools`; missing Lean or TLC is a gate failure, not skipped evidence.

## Requirement Closure

- `VERIFIER-01`: Complete. Normative `L2Core` verifier/spec document exists.
- `VERIFIER-02`: Complete. L1 declarative semantics composition boundary is
  specified through finite capabilities and facts.
- `VERIFIER-03`: Complete. Syntax/static/dynamic semantics and loop forms are
  specified and modeled in Rust.
- `VERIFIER-04`: Complete. Capability/resource model and executable verifier
  checks exist.
- `VERIFIER-05`: Complete. Arrow builder event semantics and Rust event facts
  exist.
- `VERIFIER-06`: Complete. Rust abstract-state verifier exists.
- `VERIFIER-07`: Complete. SMT-ready constraint IR and verifier emission exist.
- `VERIFIER-08`: Complete. Stable diagnostics and proof traces exist.
- `VERIFIER-09`: Complete as scaffold only. Lean theorem names compile, but
  substantive semantic predicates are `True` placeholders; this is not
  load-bearing proof evidence.
- `VERIFIER-10`: Complete. `VerifiedArtifactFacts` and TLA lowering lifecycle
  evidence exist for Phase 14 preconditions.
