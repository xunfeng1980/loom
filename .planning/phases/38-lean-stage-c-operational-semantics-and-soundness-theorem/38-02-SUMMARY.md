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
- Remediation note 3, 2026-06-09: added
  `checked_readInput_concrete_in_range`, a theorem connecting the static
  `checkAuthorityStmt` read-input acceptance path to the same
  `concreteReadInRange` predicate used by the modeled executor. The
  `accepted_program_safe` proof now carries this read-boundary bridge inside
  `NoOutOfBoundsRead` instead of relying only on the executor's fail-closed
  read-safety invariant.
- Remediation note 4, 2026-06-09: added the program-level induction bridge:
  `classified_body_exec_finishes`, `classified_stmt_exec_progress`,
  `classified_program_finishes`, `verified_program_finishes`, and
  `verified_program_reads_in_bounds`. `NoOutOfBoundsRead` now requires
  `(execProgram p).status = .finished` and all recorded reads in bounds; the
  theorem derives those facts from `Verified p` instead of accepting the
  fail-closed/read-safe disjunction as the dynamic safety result.
- Remediation note 5, 2026-06-09: explicit `.failClosed` statements are now
  rejected by both the Lean/Rust verifier correspondence surface, and constant
  out-of-range reads are rejected before execution. The proof-friendly Lean
  executor validates append builder/type/nullability conditions fail-closed but
  keeps successful append events abstract; Rust builder-event trace behavior
  remains covered by the Phase 39 differential gate rather than this theorem.
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
state evidence plus a program-level induction bridge: `Verified p` implies
`(execProgram p).status = .finished`, which rules out the fail-closed arm of
the executor read-safety invariant and yields every recorded modeled read
in-bounds. The theorem also consumes the static `Verified p` premises for
authority, builder typing, and finite bounds. The authority premise is connected
to runtime read checks through `checked_readInput_concrete_in_range`: a statically
accepted `ReadInput` branch proves the concrete slice/range predicate that the
modeled executor uses before recording an in-bounds read.

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
! rg -n "And\\.intro \\(execProgram p\\)\\.readSafety" formal/lean/LoomCore.lean
rg -n "classified_program_finishes|verified_program_finishes|verified_program_reads_in_bounds|inBounds := false|appendModeledReadOutOfBoundsFailed|eventsTyped|rowsWithinMax" formal/lean/LoomCore.lean
rg -n "checked_readInput_concrete_in_range|exact checked_readInput_concrete_in_range" formal/lean/LoomCore.lean
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
