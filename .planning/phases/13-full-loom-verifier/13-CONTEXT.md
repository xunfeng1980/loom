# Phase 13 Context: Full Loom Verifier

**Status:** Context captured
**Date:** 2026-06-08
**Inputs:** `13-RESEARCH.md`, Phase 12 safety contract/proof, user-selected verifier architecture

## Phase Intent

Phase 13 moves beyond Phase 12's current-boundary Safety Proof MVP. The target is the complete Loom verifier architecture for the future distribution IR and L2 total-function language.

Phase 13 should produce a practical and partially mechanized verifier foundation, not a paper-only proof and not a complete final production verifier in one jump.

## Locked Decisions

### D-13-01: Use a layered verification stack

The Phase 13 direction is:

```text
Lean/Rocq core semantics + soundness
Rust abstract-interpretation verifier
SMT arithmetic/range obligations
TLA+ lifecycle/pipeline invariants
```

No single formal method is expected to carry the entire verifier.

### D-13-02: Rust verifier is the executable boundary

The implementation path remains a Rust verifier in `loom-core`. It should reject unsafe artifacts before decode or lowering and produce stable diagnostics.

The Rust verifier should be based on:

- type/effect checking,
- abstract interpretation,
- resource-bound computation,
- explicit proof-obligation traces.

### D-13-03: SMT is an automation backend, not the foundation

SMT should discharge local obligations:

- offset/range checks,
- overflow checks,
- monotone progress checks,
- loop variant/ranking checks,
- resource-bound inequalities.

The verifier should keep an internal constraint IR so Z3/SMT-LIB integration does not leak into the Loom distribution spec.

### D-13-04: Lean/Rocq handles language semantics and soundness

The mechanized proof target is a small Loom core language:

- syntax,
- static semantics,
- dynamic semantics,
- accepted-program safety theorem,
- Arrow builder well-formedness theorem or sub-theorem.

Lean is preferred for clean mathematical specification and algebraic ecosystem. Rocq remains the fallback if extraction or verified-checker lineage becomes the dominant requirement.

### D-13-05: TLA+ is for workflow invariants only

TLA+ should model lifecycle/pipeline state:

- parse/verify/lower transitions,
- feature negotiation,
- artifact cache/trust state,
- invariant that lowering cannot occur before verification,
- invalidation and version/feature compatibility.

TLA+ should not be the core L2 type-soundness proof.

### D-13-06: Geometric algebra is not the verifier core

Geometric/Clifford algebra formalization is useful evidence that Lean/Rocq can support rich algebraic proofs and may matter for future algebraic kernels. It is not the basis for Loom verifier safety, termination, or capability discipline.

### D-13-07: Phase 13 MVP is a tiny vertical slice

The first implementable slice should define and verify a deliberately small `L2Core` subset:

- finite input slices,
- bounded `for i in 0..N`,
- monotone cursor loop,
- scalar bounded arithmetic,
- typed Arrow builder events,
- resource bounds and diagnostics.

This avoids trying to build the full future Loom language and complete mechanized proof all at once.

## Required Phase Outputs

- A normative Loom verifier/spec document for the Phase 13 core subset.
- A Rust verifier architecture or prototype for `L2Core`.
- A small mechanized Lean/Rocq proof artifact or theorem scaffold.
- An SMT constraint model for local arithmetic/range obligations.
- A TLA+ lifecycle/pipeline model.
- A final Phase 13 proof-obligation matrix linking all layers.

## Non-Goals

- No MLIR/native lowering implementation in Phase 13.
- No real Vortex file/container ingress in Phase 13.
- No semantic correctness proof of arbitrary producer intent.
- No attempt to mechanize all future Loom IR features at once.
- No geometric-algebra-based verifier design.

## Open Choices For Planning

- Whether the mechanized artifact starts in Lean or Rocq.
- Whether the first Rust verifier prototype is purely internal or exposed through `loom inspect`.
- Whether SMT integration is direct Z3, SMT-LIB text output, or an internal-only constraint IR first.
- Whether TLA+ is checked in as runnable TLC/Apalache spec in Phase 13 or documented as a model first.

