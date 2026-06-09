# Phase 38-02 Summary: Modeled Soundness Theorem and Closeout

**Status:** Complete
**Date:** 2026-06-09
**Plan:** `38-02-PLAN.md`

## What Changed

- Replaced the primary `accepted_program_safe` theorem in
  `formal/lean/LoomCore.lean` with a semantic theorem:
  `Verified p -> ModeledExecutionSafe p`.
- Preserved the old structural projection as `structural_safe_projection`.
- Added a visible modeled-executor-only theorem scope note in Lean.
- Strengthened `scripts/full-verifier-test.sh` to check:
  - `ModeledExecutionSafe` exists;
  - the modeled-executor-only scope note exists;
  - no `sorry` appears in `formal/lean/LoomCore.lean`.
- Completed `LINEAGE-05` and `LINEAGE-06`, marked Phase 38 complete, and moved
  planning state to Phase 39 ready.

## Theorem Scope

The load-bearing theorem is scoped to the Lean modeled executor:

```lean
theorem accepted_program_safe (p : Program) :
    Verified p -> ModeledExecutionSafe p
```

This proves modeled execution safety for verifier-accepted programs in the Lean
model. It does not prove Rust interpreter behavior, native behavior, source
correctness, performance, compiler correctness, ABI correctness, or host engine
correctness.

## Verification

All checks passed:

```sh
lean formal/lean/LoomCore.lean
! rg -n "\\bsorry\\b" formal/lean/LoomCore.lean
rg -n "accepted_program_safe|ModeledExecutionSafe|modeled executor" formal/lean/LoomCore.lean
bash scripts/full-verifier-test.sh
git diff --check
```

## Residual Risks

- The modeled semantics are intentionally abstract and proof-friendly. The
  modeled-to-real Rust interpreter seam is not closed here.
- Phase 38 does not validate native execution, compiler/toolchain correctness,
  source-data correctness, or performance.

## Handoff To Phase 39

Phase 39 should validate real Rust interpreter behavior against the modeled
executor at builder-event trace granularity. Phase 40 remains responsible for
native-to-model validation.

Self-Check: PASSED
