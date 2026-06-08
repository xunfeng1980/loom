# Phase 19 Solver Contract

## Scope

Phase 19 upgrades artifact verification from collected constraints to solver-discharged evidence. This contract defines the stable Loom-owned vocabulary for obligations, SMT-LIB scripts, backend declarations, and discharge reports.

`loom-core` owns the data model and trust rules. It does not execute solver binaries and must not depend on Z3, cvc5, Bitwuzla, or solver process-wrapper crates.

## Backend Declarations

`loom-solver-smt` must declare command-line backend kinds for:

- `z3`
- `cvc5`
- `bitwuzla`

The declarations are part of the public contract from day one so future backend adapters do not require changing the artifact verifier's report model.

## Bitwuzla Primary Path

Phase 19 implements Bitwuzla as the primary solver backend. The required SMT-LIB path is Bitwuzla-supported `QF_BV`.

Z3 and cvc5 remain optional adapter or strict cross-check paths. They may use alternate `QF_LIA` scripts where useful, but `QF_LIA` is not the required Phase 19 proof path.

## Obligation Semantics

Safety obligations are encoded as bad-state queries:

```text
artifact facts + verifier facts + negated safety property
```

For required safety obligations, success means the bad-state query returns `unsat`.

## SMT-LIB Contract

SMT-LIB scripts must be deterministic, replayable, and free of timestamps or local absolute paths. Required scripts use `QF_BV`, stable symbol names, named assertions, and explicit bad-state assertions.

## Discharge Report Invariants

A discharge report is successful only when every required obligation is discharged. `sat`, `unknown`, timeout, malformed output, solver crash, skipped strict evidence, missing strict solver, and cross-check disagreement are not successful discharge.

## Artifact Facts Trust Rule

`ArtifactVerificationFacts.constraint_status` may be set to `Discharged` only when a solver discharge report covers all required obligations and every required obligation is discharged.

`CollectedOnly` facts remain useful diagnostics, but later production native phases must not treat them as proof.

## Normal vs Strict Mode

Normal mode may record missing Bitwuzla as explicit `Skipped` evidence so local development remains usable.

Strict mode must fail closed if Bitwuzla is unavailable or if any required obligation does not discharge.

## Non-Goals

- Production MLIR decode dialect.
- Native kernel expansion.
- Host-engine native execution.
- Expanded Vortex encoding coverage.
- Checked proof objects.
- Direct solver FFI dependencies in `loom-core`.

