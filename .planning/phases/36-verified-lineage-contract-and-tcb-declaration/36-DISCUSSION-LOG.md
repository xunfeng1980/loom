# Phase 36: Verified-Lineage Contract and TCB Declaration - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-09
**Phase:** 36-Verified-Lineage Contract and TCB Declaration
**Areas discussed:** Meaning of verified, TCB clause, Obligation matrix, Deliverable and verification shape
**Mode:** Autonomous discuss; selected recommended defaults because the user previously approved recommended defaults and the active autonomous workflow permits safe defaulting.

---

## Meaning Of Verified

| Option | Description | Selected |
|--------|-------------|----------|
| Safety/well-formedness lineage | "Verified" means a named evidence layer backs a safety or Arrow well-formedness claim, never source correctness. | ✓ |
| Broad correctness | "Verified" could imply source-data correctness or semantic equivalence to upstream systems. | |
| Agent discretion | Let planning infer the meaning from prior phases. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** This preserves the MVP1.5 standing red line in ROADMAP.md and the Phase 32 claim-ledger posture.

---

## TCB Clause

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit per-component assumptions | List each TCB item with what is assumed and why it is not proven here. | ✓ |
| Blanket TCB paragraph | Use one short paragraph naming trusted infrastructure without per-item assumptions. | |
| Defer TCB detail | Let later proof phases decide their own TCB wording. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** Phase 36 success criteria explicitly require Rust compiler/std, LLVM + MLIR, Rust<->C ABI, DuckDB host, and Arrow C Data Interface.

---

## Obligation Matrix

| Option | Description | Selected |
|--------|-------------|----------|
| Assign every seam | Lean<->Rust verifier, static<->dynamic, and modeled-executor<->real-executor each map to a later phase or TCB. | ✓ |
| List seams only | Document the seams without assigning phase ownership. | |
| Focus only on Lean | Limit the matrix to Lean proof obligations. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** Unowned seams would recreate the overclaim risk Phase 32 was designed to prevent.

---

## Deliverable And Verification Shape

| Option | Description | Selected |
|--------|-------------|----------|
| One normative contract plus marker checks | Produce a single canonical contract, summary, and documentation/marker verification only. | ✓ |
| Multiple independent docs | Split evidence layers, TCB, and matrix into separate peer documents. | |
| Add proof or execution code | Start implementing proof, code, or gate scripts in Phase 36. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** ROADMAP Phase 36 non-goals say no proofs and no code. Verification should remain docs/marker based.

---

## the agent's Discretion

- Choose exact contract filename and table layouts.
- Choose where LINEAGE-01 and LINEAGE-02 are represented.
- Keep public/planning doc changes concise and non-overclaiming.

## Deferred Ideas

- Phase 37: Lean AST enrichment and Rust verifier correspondence.
- Phase 38: Operational semantics and soundness theorem.
- Phase 39: Model-to-Rust interpreter consistency.
- Phase 40: Native-to-model validation.
