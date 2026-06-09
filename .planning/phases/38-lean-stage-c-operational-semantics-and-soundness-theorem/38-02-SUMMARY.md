# Phase 38-02 Summary: Modeled Soundness Theorem and Closeout

**Status:** Complete
**Date:** 2026-06-09
**Plan:** `38-02-PLAN.md`

## What Changed

- Replaced the primary `accepted_program_safe` theorem in
  `formal/lean/LoomCore.lean` with a semantic theorem:
  `Verified p -> ModeledExecutionSafe p`.
- Preserved the old structural projection as `structural_safe_projection`.
- Remediation note, 2026-06-09: strengthened the theorem target after review so
  modeled safety predicates no longer accept `_state` and discard it. `ModeledState`
- Remediation note 2, 2026-06-09: changed the read model so out-of-bounds reads
  are representable. `ModeledState` no longer carries an all-reads-in-bounds
  invariant. The `.readInput` branch appends `inBounds := false` and
  fail-closes when `concreteReadInRange` fails; `NoOutOfBoundsRead` now means
  the run is fail-closed or every recorded read is in bounds.
- Added a visible modeled-executor-only theorem scope note in Lean.
- Strengthened `scripts/full-verifier-test.sh` to check:
  - `ModeledExecutionSafe` exists;
  - the modeled-executor-only scope note exists;
  - modeled safety predicates reference state reads/events/rows/status rather
    than `_state`;
  - `accepted_program_safe` consumes the `Verified` premise;
  - the model can produce an `inBounds := false` read and fail close;
  - no `sorry` appears in `formal/lean/LoomCore.lean`.
- Completed `LINEAGE-05` and `LINEAGE-06`, marked Phase 38 complete, and moved
  planning state to Phase 39 ready.

## Theorem Scope

The load-bearing theorem is scoped to the Lean modeled executor:

```lean
theorem accepted_program_safe (p : Program) :
    Verified p -> ModeledExecutionSafe p
```

This now proves modeled execution safety by reading the actual `execProgram p`
state evidence: execution either fail-closes or every recorded modeled read is
in bounds, every recorded modeled builder event is well typed, modeled row use
is within the carried row bound, and finalization yields a terminal modeled
status. The theorem also consumes the static `Verified p` premises for
authority, builder typing, and finite bounds.

It does not prove Rust interpreter behavior, native behavior, source
correctness, performance, compiler correctness, ABI correctness, or host engine
correctness.

## Verification

All checks passed:

```sh
lean formal/lean/LoomCore.lean
! rg -n "\\bsorry\\b" formal/lean/LoomCore.lean
rg -n "accepted_program_safe|ModeledExecutionSafe|modeled executor" formal/lean/LoomCore.lean
! rg -n "_state : ModeledState|intro _h|readsInBounds|rowsUsed := min" formal/lean/LoomCore.lean
rg -n "readSafety|inBounds := false|appendModeledReadOutOfBoundsFailed|eventsTyped|rowsWithinMax|finalized_status_terminal" formal/lean/LoomCore.lean
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
