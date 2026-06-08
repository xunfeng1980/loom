# Phase 19: Solver-backed Full Artifact Verifier - Context

**Gathered:** 2026-06-08
**Status:** Ready for planning
**Source:** Phase 19 refreshed research

## Phase Boundary

Phase 19 upgrades the unified artifact verifier from collected constraints to solver-discharged evidence. It must produce replayable SMT-LIB scripts, a solver discharge report, and artifact facts whose `ConstraintDischargeStatus::Discharged` is earned only when all required bad-state queries are proven `unsat`.

## Locked Decisions

- `loom-core` stays solver-neutral. It may own obligation/report types and deterministic SMT-LIB emission, but it must not depend on Z3, cvc5, Bitwuzla, or process-wrapper crates.
- `loom-solver-smt` is the optional subprocess backend crate.
- The backend trait must declare command-line backend kinds for `z3`, `cvc5`, and `bitwuzla` from day one.
- Phase 19 implements Bitwuzla as the primary backend.
- The required Phase 19 SMT-LIB path is Bitwuzla-supported `QF_BV`; `QF_LIA` may exist only as an optional Z3/cvc5 cross-check path.
- Success for safety obligations means a negated bad-state query returns `unsat`.
- `sat`, `unknown`, timeout, malformed output, solver crash, missing strict solver, and cross-check disagreement fail closed.
- Normal mode may skip unavailable solver evidence with explicit `Skipped`; strict mode must fail if Bitwuzla is unavailable.
- Phase 20+ must consume discharged facts, not `CollectedOnly` facts.

## Non-Goals

- Production MLIR decode dialect or native kernel expansion.
- Host-engine native execution.
- Expanded Vortex encoding coverage.
- Full proof-object checking.
- Direct solver FFI/API dependencies inside `loom-core`.

## Required Handoff

The phase should leave the next phase with:

```text
ArtifactVerificationReport
  status = Accepted
  facts.constraint_status = Discharged
  facts.solver_report = Some(...)
```

only when structural verification, optional L2Core verification, and all required solver obligations have passed the fail-closed discharge policy.

