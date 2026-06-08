# Phase 19 Summary: Solver-backed Full Artifact Verifier

**Status:** Complete
**Date:** 2026-06-08
**Self-Check:** PASSED

## Shipped

- `loom_core::solver` report model and deterministic Bitwuzla-primary `QF_BV` SMT-LIB emission.
- Optional `loom-solver-smt` crate with `z3`/`cvc5`/`bitwuzla` backend declarations and full Bitwuzla subprocess execution.
- `apply_solver_discharge` integration in the artifact verifier.
- Artifact-level lowering readiness gated on `Discharged` constraints instead of `CollectedOnly` obligations.
- Solver-backed CLI mode: `loom verify-artifact --solver-bitwuzla --l2core-sample <artifact.loom>`.
- `scripts/solver-verifier-test.sh` wired into `scripts/mvp0-verify.sh`.
- Final public and planning docs clarifying that Phase 19 is solver-backed verifier evidence, not production native execution.

## Commands

- `cargo test -p loom-cli`
- `cargo test -p loom-core --test solver_contract`
- `cargo test -p loom-core --test smtlib_emitter`
- `cargo test -p loom-core --test artifact_solver_discharge`
- `cargo test -p loom-solver-smt`
- `bash scripts/solver-verifier-test.sh`
- `LOOM_REQUIRE_SOLVER=1 bash scripts/solver-verifier-test.sh`
- `bash scripts/mvp0-verify.sh`
- `git diff --check`

## Deviations

- `loom-solver-smt` re-runs `verify_l2_core` in the artifact helper to obtain `ConstraintSet`; this avoids parsing comments and keeps `loom-core` solver-neutral.
- Z3 and cvc5 are declared but not implemented as execution adapters in Phase 19.

## Residual Risks

- No stable external `L2Core` artifact codec/parser yet.
- No checked solver certificates or proof-object validation.
- Current solver evidence covers the current bounded artifact/L2Core slice, not arbitrary Vortex semantics.
- Production MLIR decode dialect and broader native kernel expansion remain Phase 20 work.
