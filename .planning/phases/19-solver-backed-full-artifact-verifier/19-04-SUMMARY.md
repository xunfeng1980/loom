# Phase 19 Plan 04 Summary: Artifact Verifier Solver-Discharge Integration

**Status:** Complete
**Date:** 2026-06-08
**Self-Check:** PASSED

## Shipped

- Added `apply_solver_discharge` in `loom-core` to apply solver reports only to structurally accepted artifact reports.
- Added `FullVerificationReport::constraints()` so solver execution can use the verifier-owned `ConstraintSet` instead of parsing comments.
- Added artifact constraint diagnostics for obligation mismatch, required-count mismatch, failed, unknown, timed-out, errored, and skipped solver outcomes.
- Gated artifact-level lowering readiness: non-empty `constraint_ids` remain not ready while status is `CollectedOnly`, `Failed`, `Unknown`, or `Skipped`.
- Promoted solver-blocked lowering readiness only when all required obligations match and discharge successfully.
- Added `loom-solver-smt::verify_artifact_with_l2_core_and_bitwuzla` to verify an artifact/L2Core pair, emit SMT-LIB, run Bitwuzla, and apply solver evidence to artifact facts.
- Added focused core and solver crate tests for discharged evidence, non-`unsat` fail-closed evidence, mismatch rejection, rejected/unsupported preservation, collected-only lowering blocking, and real Bitwuzla artifact helper discharge.

## Commands

- `cargo test -p loom-core --test artifact_solver_discharge`
- `cargo test -p loom-solver-smt artifact_solver`
- `cargo test -p loom-core --test artifact_verifier`
- `cargo test -p loom-solver-smt`
- `cargo test -p loom-core native_lowering`
- `LOOM_REQUIRE_SOLVER=1 bash scripts/solver-verifier-test.sh`
- `git diff --check`

## Verification Notes

- Strict solver gate passed with Bitwuzla 0.9.1 at `/opt/homebrew/bin/bitwuzla`.
- `CollectedOnly` artifact facts no longer claim artifact-level lowering readiness when constraints exist.
- Existing native lowering unit tests remain green because their direct verifier-gated path is still separate from artifact-level production readiness.

## Deviations

- `loom-solver-smt` re-runs `verify_l2_core` in the helper to obtain `ConstraintSet`; this keeps `loom-core` solver-neutral and avoids parsing comments, at the cost of duplicate verification work for this phase.

## Residual Risks

- 19-05 still needs CLI/release-gate closeout so humans can request solver-backed artifact verification outside tests.
- The current solver script applies one aggregate script result over all obligations; finer per-obligation replay remains a later precision improvement.
