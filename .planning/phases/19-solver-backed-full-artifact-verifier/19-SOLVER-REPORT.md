# Phase 19 Solver Report: Solver-backed Full Artifact Verifier

## Scope

Phase 19 upgrades the unified artifact verifier from collected local obligations to solver-backed evidence for the current `L2Core` artifact slice. It does not introduce production native execution, arbitrary Vortex semantic proof, checked proof objects, or a stable external `L2Core` artifact codec.

## Solver Architecture

`loom-core` owns solver-neutral data structures: obligations, SMT-LIB script metadata, raw solver outcomes, per-obligation statuses, discharge summaries, and artifact facts. Subprocess execution lives in the optional `loom-solver-smt` crate so `loom-core` remains free of solver runtime dependencies.

## Backend Contract

The backend contract declares `z3`, `cvc5`, and `bitwuzla` command-line backend kinds from day one. Phase 19 implements Bitwuzla as the primary backend. Z3 and cvc5 remain declared adapters and future optional cross-check paths.

## Bitwuzla Evidence

Bitwuzla is the implemented Phase 19 backend. Managed local evidence passed with
Homebrew Bitwuzla `0.9.1` at `/opt/homebrew/bin/bitwuzla`. Missing Bitwuzla
fails the release gate by default. Skip is permitted only by explicit
`LOOM_ALLOW_SOLVER_SKIP=1`, and skipped evidence never marks constraints as
discharged.

## SMT-LIB Emission

The required script family emits deterministic SMT-LIB v2.7 `QF_BV` scripts. Current scripts encode Loom constraints as named bad-state assertions and treat `unsat` as discharged evidence. `sat`, `unknown`, timeout, parse error, solver crash, and missing strict solver fail closed.

## Artifact Verifier Integration

`apply_solver_discharge` applies solver reports only to structurally accepted artifact reports. `ConstraintDischargeStatus::Discharged` requires exact obligation ID coverage and successful discharge of every required obligation. `CollectedOnly`, failed, unknown, timed-out, errored, or skipped evidence does not mark artifact facts production-discharge-ready.

## CLI and Release Gate

`loom verify-artifact --solver-bitwuzla --l2core-sample <artifact.loom>` exposes solver-backed status, backend, script logic, required obligation count, discharged/failed/unknown/skipped counts, and production discharge readiness. `scripts/solver-verifier-test.sh` is wired into `scripts/mvp0-verify.sh`.

## Commands Run

- `cargo test -p loom-cli`
- `cargo run --bin loom -- verify-artifact --help | rg -n "solver|bitwuzla|discharge"`
- `mise run external-tools`
- `LOOM_REQUIRE_SOLVER=1 cargo run -q --bin loom -- verify-artifact --solver-bitwuzla --l2core-sample target/loom-duckdb-fixtures/bitpack-i32.loom`
- `cargo test -p loom-core --test solver_contract`
- `cargo test -p loom-core --test smtlib_emitter`
- `cargo test -p loom-core --test artifact_solver_discharge`
- `cargo test -p loom-solver-smt`
- `bash scripts/solver-verifier-test.sh`
- `bash scripts/mvp0-verify.sh`
- `git diff --check`

## Deferred Work

- Stable external `L2Core` artifact codec/parser.
- Finer per-obligation solver replay instead of one aggregate script result.
- Optional Z3/cvc5 adapters and cross-check policy.
- Checked proof objects or independently checkable solver certificates.
- Arbitrary Vortex encoding/layout semantic proof.

## Phase 20 Handoff

Phase 20 may consume artifact facts only when solver-backed constraints are `Discharged`. It must not treat `CollectedOnly` obligations as production native-lowering proof.
