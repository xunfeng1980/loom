---
phase: 36-verified-lineage-contract-and-tcb-declaration
plan: 01
subsystem: verified-lineage-contract
tags: [docs, verified-lineage, tcb, requirements, roadmap, state]
requires:
  - phase: 36-verified-lineage-contract-and-tcb-declaration
    provides: Phase 36 context and plan
provides:
  - Verified-lineage contract
  - TCB declaration
  - Obligation matrix
  - LINEAGE-01 and LINEAGE-02 requirements
  - Phase 37 handoff
affects: [phase-36, phase-37, requirements, roadmap, state]
tech-stack:
  added: []
  patterns:
    - Keep "verified" tied to safety and Arrow well-formedness evidence lineage only.
    - Assign every trust seam to a later MVP1.5 phase or to explicit TCB.
key-files:
  created:
    - .planning/phases/36-verified-lineage-contract-and-tcb-declaration/36-VERIFIED-LINEAGE-CONTRACT.md
    - .planning/phases/36-verified-lineage-contract-and-tcb-declaration/36-01-SUMMARY.md
  modified:
    - .planning/PROJECT.md
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
    - .planning/STATE.md
key-decisions:
  - Phase 36 defines "verified" as safety + Arrow well-formedness evidence lineage only.
  - Every safety claim must map to one named evidence layer or explicit TCB trust assumption.
  - Rust compiler/std, LLVM + MLIR, Rust<->C ABI, DuckDB host, and Arrow C Data Interface are named TCB.
  - Lean/Rust/model/native trust seams are assigned to Phase 37-40 or TCB.
requirements-completed: [LINEAGE-01, LINEAGE-02]
duration: 3 min
completed: 2026-06-09
---

# Phase 36 Plan 01: Verified-Lineage Contract Summary

Phase 36 is complete as a docs-only contract phase. It pins the word
"verified" before later proof work begins, declares the TCB, assigns trust
seams, and closes LINEAGE-01/LINEAGE-02 without adding proof or execution code.

No production code, proof code, or new execution gate was added in Phase 36.

## Execution

Start: 2026-06-09T09:00:26Z
End: 2026-06-09T09:03:38Z
Tasks: 3/3 complete
Files changed: 6

## Commits

| Task | Commit | Description |
|---|---|---|
| Task 1 | e6f4e56 | Created `36-VERIFIED-LINEAGE-CONTRACT.md` with scope, evidence layers, claim mapping, TCB, obligation matrix, non-claims, and downstream handoff. |
| Task 2 | 35ba930 | Added LINEAGE requirements, marked Phase 36 complete in roadmap/state, moved current focus to Phase 37, and updated project caveats. |

## Verification

- `rg -n "## Scope|## Evidence Layers|## Claim Mapping|## TCB|## Obligation Matrix|## Non-Claims|## Downstream Phase Handoff" .planning/phases/36-verified-lineage-contract-and-tcb-declaration/36-VERIFIED-LINEAGE-CONTRACT.md` - passed
- `rg -n "Rust verifier structural check|Bitwuzla SMT discharge|Lean soundness theorem|differential validation|explicit TCB trust assumption" .planning/phases/36-verified-lineage-contract-and-tcb-declaration/36-VERIFIED-LINEAGE-CONTRACT.md` - passed
- `rg -n "Rust compiler/std|LLVM \+ MLIR toolchain|Rust<->C ABI|DuckDB host process|Arrow C Data Interface" .planning/phases/36-verified-lineage-contract-and-tcb-declaration/36-VERIFIED-LINEAGE-CONTRACT.md` - passed
- `rg -n "Lean<->Rust verifier|static<->dynamic|modeled-executor<->real-executor" .planning/phases/36-verified-lineage-contract-and-tcb-declaration/36-VERIFIED-LINEAGE-CONTRACT.md` - passed
- `rg -n "LINEAGE-01|LINEAGE-02" .planning/REQUIREMENTS.md .planning/ROADMAP.md .planning/phases/36-verified-lineage-contract-and-tcb-declaration/36-VERIFIED-LINEAGE-CONTRACT.md` - passed
- `rg -n "Phase 36.*Complete|Phase 37.*READY|Verified-Lineage Contract" .planning/STATE.md .planning/ROADMAP.md` - passed
- `rg -n "safety \+ well-formedness|never correctness|verified-lineage" .planning/PROJECT.md .planning/REQUIREMENTS.md` - passed
- `git diff --check` - passed
- `node $HOME/.codex/gsd-core/bin/gsd-tools.cjs query roadmap.analyze` - passed; next phase is 37

## Deviations from Plan

None - plan executed exactly as written.

## Residual Risks

- Phase 36 does not prove any new safety theorem; it defines vocabulary and
  seam ownership for Phase 37-40.
- Later phases must cite this contract to avoid expanding "verified" into
  correctness, performance, production readiness, or compiler/host proof.

## Next Phase Readiness

Ready for Phase 37: Lean Stage B / Lean ↔ Rust Verifier Correspondence.

## Self-Check: PASSED

All plan acceptance criteria and verification commands passed.
