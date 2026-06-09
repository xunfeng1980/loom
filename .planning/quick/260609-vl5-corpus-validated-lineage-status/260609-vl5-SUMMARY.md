---
phase: quick-260609-vl5
plan: 01
status: complete
subsystem: verified-lineage
tags: [mvp1.5, verified-lineage, audit-fix, claim-boundary]
provides:
  - "VerifiedLineageEvidenceStatus::CorpusValidated status word"
  - "Per-artifact lineage records distinguish corpus-level trust-by-reference from per-artifact verification"
affects:
  - "scripts/verified-lineage-test.sh (still PASS; no marker removed)"
key-files:
  modified:
    - "crates/loom-core/src/verified_lineage.rs"
    - "crates/loom-core/tests/verified_lineage.rs"
    - ".planning/phases/36-verified-lineage-contract-and-tcb-declaration/36-VERIFIED-LINEAGE-CONTRACT.md"
metrics:
  duration: ~5min
  completed: 2026-06-09
---

# Quick Task vl5: Corpus-Validated Lineage Status Summary

Resolved the MVP1.5 — Verified Lineage audit finding (medium, non-blocking): the
per-artifact `VerifiedLineageRecord` stamped its three milestone-level evidence
layers with `Passed`, which reads as if each artifact was individually
re-validated. In fact those layers are corpus/gate-level properties trusted
because the release gate `scripts/verified-lineage-test.sh` ran them over the
full fixture+fuzz corpus — not facts re-checked when an individual record is
built.

## What Changed

- Added `VerifiedLineageEvidenceStatus::CorpusValidated` (`as_str` →
  `"corpus-validated"`) with a doc comment naming it trust-by-reference at
  artifact granularity.
- `build_verified_lineage_record` now stamps `CorpusValidated` (was `Passed`) for:
  - `LeanModeledSoundnessTheorem` (when `l2_core` facts present),
  - `LeanRustVerifierDifferential`,
  - `ModelRustInterpreterDifferential`.
- Unchanged, by design:
  - `RustVerifierStructuralCheck` → `Passed` (genuinely per-artifact).
  - `BitwuzlaSmtDischarge` status logic (tracks `constraint_status`).
  - `NativeModelValidation` per-run logic (`PerRunValidated` / `NotRun` / fail-closed).
  - `LeanModeledSoundnessTheorem` `NotApplicable` branch (no `l2_core` facts).
- Updated `tests/verified_lineage.rs` to assert `CorpusValidated` for the three
  layers.
- Added a `corpus-validated` row to the Phase 36 contract evidence-status table.

## Verification

- `cargo test -p loom-core --test verified_lineage` → 4 passed.
- `bash scripts/verified-lineage-test.sh` → full gate PASSED end-to-end (Lean
  no-sorry, Lean↔Rust differential, model↔interpreter trace, native↔model
  validation, record tests, all non-claim/TCB markers intact).
- `git diff --check` clean.

## Claim Boundary

No evidence layer's underlying check changed. This is a claim-granularity
wording fix that makes the record honest about *where* each guarantee was
established (full release corpus vs. this artifact), preserving the milestone
red line: safety + well-formedness, never correctness; every layer maps to a
named evidence source or the TCB.

## Deviations from Plan

None.

## Self-Check: PASSED

- crates/loom-core/src/verified_lineage.rs: FOUND (CorpusValidated added + applied)
- crates/loom-core/tests/verified_lineage.rs: FOUND (assertions updated, tests pass)
- 36-VERIFIED-LINEAGE-CONTRACT.md: FOUND (status row added)
- Commit d4d6b03: FOUND
