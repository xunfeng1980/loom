# Phase 19 Plan 03 Summary: Optional loom-solver-smt Crate with Bitwuzla Backend

**Status:** Complete
**Date:** 2026-06-08
**Self-Check:** PASSED

## Shipped

- Added optional workspace crate `loom-solver-smt` for command-line solver execution outside `loom-core`.
- Declared backend discovery/execution boundaries for `z3`, `cvc5`, and `bitwuzla`.
- Implemented Bitwuzla as the first full backend over deterministic `SmtLibScript` input.
- Added subprocess timeout handling, stdout/stderr excerpts, decisive `unsat` / `sat` / `unknown` parsing, and fail-closed malformed/non-zero/timeout/missing handling.
- Preserved normal-mode missing-Bitwuzla behavior as explicit `Skipped` evidence and strict-mode missing-Bitwuzla behavior as `Error`.
- Added `scripts/solver-verifier-test.sh` to run core solver tests, SMT-LIB emitter tests, backend crate tests, Bitwuzla discovery, and strict solver evidence where available.

## Commands

- `cargo test -p loom-solver-smt`
- `cargo test -p loom-core --test smtlib_emitter`
- `cargo test -p loom-core --test solver_contract`
- `bash scripts/solver-verifier-test.sh`
- `LOOM_REQUIRE_SOLVER=1 bash scripts/solver-verifier-test.sh`

## Verification Notes

- Bitwuzla was installed locally via Homebrew and detected at `/opt/homebrew/bin/bitwuzla`.
- `bitwuzla --version` reports `0.9.1`.
- Default and strict solver gates both pass with real Bitwuzla execution evidence.
- Unit tests cover backend declarations, decisive stdout parsing, missing normal-mode skip, missing strict-mode error, installed-Bitwuzla `unsat` discharge, and installed-Bitwuzla `sat` failed evidence.

## Deviations

- Z3 and cvc5 are discoverable/declarative adapters only in 19-03; their execution paths return deferred diagnostics. This follows the Phase 19 decision to implement Bitwuzla first while keeping the backend trait open to all three command-line solvers.

## Residual Risks

- Artifact verifier reports do not consume solver discharge yet; that is 19-04.
- Current local strict evidence depends on Homebrew Bitwuzla `0.9.1` being available on `PATH`.
- The current execution path maps one script result across all script obligation IDs; finer per-obligation replay can be added when 19-04 wires real artifact obligations.
