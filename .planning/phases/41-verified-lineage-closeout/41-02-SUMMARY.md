# Phase 41-02 Summary: Verified-Lineage Record and Docs

**Status:** Complete
**Date:** 2026-06-09
**Plan:** `41-02-PLAN.md`

## What Changed

- Added `loom_core::verified_lineage`.
- Added `VerifiedLineageRecord`, evidence-layer/status rows, TCB assumption
  rows, non-claim rows, and stable diagnostics.
- Positive records are produced only from accepted artifact verifier reports.
- Artifacts with collected constraints require discharged solver evidence
  before a positive lineage record.
- Optional native/model validation is recorded as positive lineage only when
  validation succeeds; divergent validation produces a stable fail-closed
  lineage diagnostic.
- Added `crates/loom-core/tests/verified_lineage.rs`.
- Wired verified-lineage record tests and marker checks into
  `scripts/verified-lineage-test.sh`.
- Updated English and Chinese README docs.
- Closed LINEAGE-12 and marked Phase 41 complete.

## Claim Boundary

The record is safety provenance. It names evidence and assumptions; it is not a
correctness certificate.

Non-claims preserved in code and docs:

- no source-data correctness claim;
- no upstream format semantic correctness claim;
- no end-to-end toolchain verification claim;
- no verified compilation claim;
- no production-readiness or performance claim.

## Verification

Passed:

```sh
cargo test -p loom-core --test verified_lineage
bash scripts/verified-lineage-test.sh
```

Final broad verification is run before commit:

```sh
bash scripts/full-verifier-test.sh
git diff --check
```

## Handoff To Phase 42

Phase 42 should consume `VerifiedLineageRecord` as the per-shape provenance
surface while widening coverage. New accepted shapes should carry explicit
verified-lineage, native-execution, and interpreter-fallback disposition.

Self-Check: PASSED
