# Phase 37-02 Summary: Lean Rust Correspondence Harness and Gate

**Status:** Complete
**Date:** 2026-06-09
**Plan:** `37-02-PLAN.md`

## What Changed

- Added a deterministic Rust correspondence corpus in
  `crates/loom-core/tests/full_verifier.rs`.
- Added Lean-side classification and matching correspondence report rows in
  `formal/lean/LoomCore.lean`.
- Added `scripts/lean-rust-correspondence-test.sh`, which captures Rust and Lean
  `correspondence:<case>:<classification>` rows and fails closed with `diff -u`
  on any accept/reject or reject-code divergence.
- Wired the new correspondence gate into `scripts/full-verifier-test.sh`.
- Closed `LINEAGE-03` and `LINEAGE-04`, marked Phase 37 complete, and moved
  planning state to Phase 38 ready.

## Corpus Coverage

The release-gated correspondence corpus includes the current full verifier
matrix plus deterministic fuzz cases:

| Case | Expected Classification |
|---|---|
| `matrix-accepted-copy` | `accepted` |
| `matrix-missing-input-capability` | `missing-input-capability` |
| `matrix-missing-output-builder` | `missing-output-builder` |
| `matrix-invalid-loop-bounds` | `invalid-loop-bounds` |
| `matrix-non-monotone-cursor-loop` | `non-monotone-cursor-loop` |
| `matrix-resource-budget-exceeded` | `resource-budget-exceeded` |
| `matrix-unknown-variable` | `unknown-variable` |
| `matrix-output-type-mismatch` | `output-type-mismatch` |
| `matrix-output-nullability-mismatch` | `output-nullability-mismatch` |
| `fuzz-000-let-add-int32` | `accepted` |
| `fuzz-001-eq-bool` | `accepted` |
| `fuzz-002-read-width-bytes-mismatch` | `output-type-mismatch` |

Required roadmap reject codes are covered:
`MissingInputCapability`, `MissingOutputBuilder`, `InvalidLoopBounds`,
`NonMonotoneCursorLoop`, and `ResourceBudgetExceeded`.

Additional covered codes:
`UnknownVariable`, `OutputTypeMismatch`, and
`OutputNullabilityMismatch`.

`ConstraintBudgetExceeded` remains Rust-only coverage for now because the Lean
program model does not carry `max_constraint_count`; this is outside the
roadmap-required reject-code floor and remains tied to Rust/SMT obligation
accounting.

## Verification

All checks passed:

```sh
cargo test -p loom-core --test full_verifier
cargo test -p loom-core --test l2_core_model
lean formal/lean/LoomCore.lean
bash scripts/lean-rust-correspondence-test.sh
bash scripts/full-verifier-test.sh
git diff --check
```

## Non-Claims

Phase 37 establishes Lean/Rust verifier correspondence evidence only. It does
not prove operational semantics, modeled-executor soundness, Rust interpreter
correctness, native correctness, source correctness, or performance.

## Handoff To Phase 38

Phase 38 should consume the now-corresponding Lean static checker and define the
modeled operational semantics plus a scoped `accepted_program_safe` soundness
theorem over the modeled executor. Modeled-to-real executor validation remains
Phase 39/40, not Phase 38.

Self-Check: PASSED
