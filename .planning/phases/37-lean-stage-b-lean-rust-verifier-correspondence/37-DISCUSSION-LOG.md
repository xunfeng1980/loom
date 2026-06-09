# Phase 37: Lean Stage B - Lean Rust Verifier Correspondence - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-06-09
**Phase:** 37-Lean Stage B - Lean Rust Verifier Correspondence
**Areas discussed:** Lean AST enrichment, differential harness, scope boundary, gate wiring
**Mode:** Autonomous discuss; selected recommended defaults because the user previously approved recommended defaults and the active autonomous workflow permits safe defaulting.

---

## Lean AST Enrichment

| Option | Description | Selected |
|--------|-------------|----------|
| Mirror Rust `ScalarExpr` / `LetScalar` | Enrich Lean so the covered static checker derives types from scalar expressions and models unknown variables. | yes |
| Keep `Nat` projection | Leave Lean as a flattened approximation with concrete offsets, widths, and append types. | |
| Extend beyond Rust | Design a richer Lean IR and then adjust Rust to match it. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** The roadmap success criteria require `ScalarExpr` / `LetScalar` and expression-derived builder typing.

---

## Differential Harness Shape

| Option | Description | Selected |
|--------|-------------|----------|
| Deterministic shared corpus | Compare Lean and Rust over the current full fixture matrix plus bounded seeded fuzz cases. | yes |
| Hand-written examples only | Cover a few positive and negative examples without generated cases. | |
| Live random fuzz only | Generate fresh random cases every run. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** Phase 37 needs repeatable zero-divergence evidence. Live random fuzz would make failures harder to reproduce.

---

## Scope Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Correspondence only | Prove that Lean and Rust classify the same current verifier slice the same way. | yes |
| Soundness theorem | Add operational semantics and prove accepted programs safe in this phase. | |
| Native/executor validation | Compare native or interpreter execution behavior against the model. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** Phase 36 assigns the later proof/executor seams to Phases 38-40.

---

## Gate Wiring

| Option | Description | Selected |
|--------|-------------|----------|
| Focused fail-closed gate | Add a correspondence gate and wire it into the verifier/release path. | yes |
| Advisory report | Produce a comparison report but do not fail the release path on divergence. | |
| Manual-only check | Keep correspondence evidence outside scripts. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** The roadmap explicitly requires the differential harness to fail closed on divergence or differently classified inputs.

---

## the agent's Discretion

- Choose exact shared-fixture encoding.
- Choose Lean helper names and simple environment representation.
- Choose exact script names, provided the focused gate is wired into the existing release/verifier path.
- Include additional reject codes beyond the required roadmap floor when practical.

## Deferred Ideas

- Phase 38: Operational semantics and soundness theorem.
- Phase 39: Static-to-dynamic model/interpreter consistency.
- Phase 40: Native-to-model validation.
