# Phase 38: Lean Stage C - Operational Semantics and Soundness Theorem - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-06-09
**Phase:** 38-Lean Stage C - Operational Semantics and Soundness Theorem
**Areas discussed:** Semantics shape, theorem scope, gate wiring, non-claims
**Mode:** Autonomous discuss; selected recommended defaults because the user previously approved recommended defaults and the active autonomous workflow permits safe defaulting.

---

## Semantics Shape

| Option | Description | Selected |
|--------|-------------|----------|
| Proof-friendly modeled executor | Define abstract inputs, typed builder events, bounded execution, and fail-closed terminal semantics in Lean. | yes |
| Byte-accurate interpreter | Model concrete bytes and Arrow buffers in Lean. | |
| Rust/native semantics | Try to prove behavior of the real Rust interpreter or native backend. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** Phase 38 owns the modeled `static<->dynamic` seam only.

---

## Soundness Theorem Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Modeled executor safety | Prove verifier acceptance implies modeled execution safety and Arrow well-formedness. | yes |
| Broad product correctness | Claim source correctness, Rust/native correctness, or performance. | |
| Structural projection only | Keep `accepted_program_safe` as `Verified -> Safe` without execution semantics. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** The roadmap requires the previous structural theorem to become semantic.

---

## Gate Wiring

| Option | Description | Selected |
|--------|-------------|----------|
| Lean compile gate plus focused markers | Keep `lean formal/lean/LoomCore.lean` load-bearing and add markers if useful. | yes |
| Advisory proof docs | Write theorem notes without a machine-checked Lean proof. | |
| Separate unwired proof artifact | Add a proof file that release gates do not execute. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** Phase 38 must compile with 0 `sorry`.

---

## Non-Claims

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit modeled-only scope note | State that Rust interpreter consistency is Phase 39 and native/model validation is Phase 40. | yes |
| Let readers infer scope | Omit scope notes and rely on roadmap order. | |
| Collapse later seams | Treat the modeled theorem as proof of real executor/native behavior. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** Phase 36 requires each trust seam to remain owned and named.

---

## the agent's Discretion

- Choose small-step, big-step/fueled, or hybrid Lean semantics.
- Choose exact helper names for state, input model, and safety predicates.
- Choose whether to keep semantics in `LoomCore.lean` or add a gated companion
  Lean file.

## Deferred Ideas

- Phase 39: modeled-executor to Rust interpreter consistency.
- Phase 40: native-to-model validation.
- TCB/proof-object/compiler correctness work remains outside this phase.
