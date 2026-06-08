# Phase 13 Discussion Log

**Date:** 2026-06-08
**Phase:** 13 — Full Loom Verifier

## User Direction

The user asked to begin Phase 13 as the full Loom verifier and explicitly selected the layered approach:

```text
Lean/Rocq 定义核心语义和 soundness，
Rust 抽象解释 verifier 负责实际执行，
SMT 自动 discharge 边界/算术 obligation，
TLA+ 管 lifecycle/pipeline invariants
```

## Clarifications Resolved

### Why not one formal system?

The full Loom verifier spans different problem classes:

- lifecycle and pipeline state,
- language soundness,
- local arithmetic/range facts,
- practical executable diagnostics,
- future lowering refinement.

One formal system would either be too weak for some parts or too heavy for day-to-day verifier execution.

### What is complete language soundness meta-theory?

It means proving the verifier rules themselves are sound:

```text
If a Loom program/artifact is accepted by the verifier,
then it cannot violate memory, capability, output-well-formedness,
termination, or fail-closed safety rules.
```

This is stronger than proving one concrete artifact safe.

## Captured Decision

Phase 13 should start from a tiny but representative `L2Core` vertical slice and build:

- a normative spec,
- Rust verifier prototype,
- SMT obligation model,
- Lean/Rocq theorem scaffold,
- TLA+ pipeline invariant model.

The full future verifier remains the long-term target, but Phase 13 should deliver a concrete foundation that later phases can extend.

