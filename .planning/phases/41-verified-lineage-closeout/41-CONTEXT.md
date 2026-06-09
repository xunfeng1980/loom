# Phase 41 Context: Verified-Lineage Closeout

**Status:** Context captured
**Date:** 2026-06-09
**Roadmap phase:** 41 - Verified-Lineage Closeout

## Roadmap Intent

Phase 41 closes MVP1.5 Verified Lineage by composing the prior lineage phases
into one visible gate and one artifact-facing provenance record.

Success criteria:

1. `scripts/verified-lineage-test.sh` runs the full lineage matrix: Lean build
   with zero `sorry`, Lean/Rust verifier differential, model/Rust interpreter
   trace consistency, and native/model validation.
2. Each emitted artifact can carry or produce a verified-lineage record naming
   the evidence layers establishing safety and the TCB assumptions it relies on.
3. Public and planning docs state exactly what Verified Lineage does and does
   not assert.

## Inputs From Prior Phases

- Phase 36 defines the verified-lineage contract, evidence-layer vocabulary,
  and TCB rows.
- Phase 37 provides `scripts/lean-rust-correspondence-test.sh`.
- Phase 38 provides the Lean modeled executor and no-`sorry`
  `accepted_program_safe` theorem scoped to modeled execution.
- Phase 39 provides `scripts/model-rust-interpreter-consistency-test.sh`.
- Phase 40 provides `scripts/native-model-validation-test.sh` and validation
  aware native runtime/cache eligibility.

## Current Code Anchors

- `crates/loom-core/src/artifact_verifier.rs` exposes accepted artifact facts.
- `crates/loom-core/src/native_arrow_semantic.rs` exposes native/model
  validation reports and validation-aware routing/cache helpers.
- `scripts/full-verifier-test.sh` already runs the lineage constituent gates,
  but it is still named as a broad verifier gate rather than the closeout
  lineage contract.
- `README.md` and `README-zh.md` document Phase 35/40 native execution but do
  not yet expose a concise Verified Lineage artifact record contract.

## Key Design Constraint

Verified Lineage is safety provenance, not source correctness, production
readiness, end-to-end toolchain verification, or verified compilation.

The lineage record should name both evidence and assumptions. It must not imply
that differential validation is a proof for all programs, or that the MLIR/LLVM,
Rust compiler/std, ABI, Arrow C Data Interface, or host-engine seams have been
verified.

## Proposed Plan Split

- `41-01`: Add the combined `verified-lineage-test.sh` gate and marker checks.
- `41-02`: Add a per-artifact verified-lineage record API plus tests and docs.

Self-Check: READY
