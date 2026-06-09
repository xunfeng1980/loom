# Phase 37-01 Summary: Lean AST Enrichment and Typing Parity

**Status:** Complete
**Date:** 2026-06-09
**Plan:** `37-01-PLAN.md`

## What Changed

- Reworked `formal/lean/LoomCore.lean` from the older flattened `Nat` projection
  into a checker surface with Lean `ScalarValue`, `ScalarExpr`, and
  `Stmt.letScalar`.
- Updated `readInput`, `appendValue`, `forRange`, and `cursorLoop` to carry
  scalar expressions where the Rust verifier does.
- Added `ScalarEnv`, `typeOfExpr?`, `exprVarsKnown`, and `exprWellTyped` so
  `appendValue` derives value type from expressions instead of explicit
  statement type fields.
- Modeled `letScalar` insertion and unknown-variable rejection in the typed,
  authority, and bounds checkers.
- Kept overflow and non-concrete range obligations explicitly delegated to the
  existing Rust/Bitwuzla SMT evidence; Phase 37-01 did not claim bitvector
  arithmetic soundness.
- Added small Lean examples for valid `letScalar` append typing, unknown-variable
  rejection, and cursor-loop finite-bounds checking.
- Updated planning state so Phase 37 is In Progress with plan 1 of 2 complete
  and LINEAGE-03/LINEAGE-04 are visible for closeout in 37-02.

## Verification

All checks passed:

```sh
lean formal/lean/LoomCore.lean
rg -n "inductive ScalarValue|inductive ScalarExpr|letScalar|appendValue .*ScalarExpr|readInput .*ScalarExpr" formal/lean/LoomCore.lean
rg -n "typeOfExpr|ScalarEnv|UnknownVariable|builder.*expected|letScalar" formal/lean/LoomCore.lean
rg -n "NonMonotoneCursorLoop|InvalidLoopBounds|ResourceBudgetExceeded|MissingInputCapability|MissingOutputBuilder|OutputTypeMismatch|OutputNullabilityMismatch|UnknownVariable" formal/lean/LoomCore.lean
rg -n "Bitwuzla|SMT|overflow|delegated" formal/lean/LoomCore.lean
git diff --check
```

## Residual Risks

- Lean now mirrors the covered static checker shape more closely, but this is
  still correspondence evidence only. It is not an operational semantics or
  soundness theorem.
- The exact Lean/Rust reject-code comparison is not complete until 37-02 adds
  the differential harness and release-gate wiring.
- Non-row budgets and SMT-only arithmetic/range obligations remain Rust/solver
  evidence, as intended by the Phase 36 contract.

## Handoff To 37-02

37-02 should build the deterministic Lean/Rust differential corpus, compare
accept/reject plus reject code, cover the required roadmap diagnostics, wire the
gate into the verifier/release path, and then close LINEAGE-03/LINEAGE-04.

Self-Check: PASSED
