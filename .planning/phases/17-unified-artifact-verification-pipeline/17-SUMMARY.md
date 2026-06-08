# Phase 17 Summary: Unified Artifact Verification Pipeline

**Status:** Complete
**Date:** 2026-06-08
**Self-Check:** PASSED

## Shipped

- Added `loom_core::artifact_verifier` as the unified artifact-facing verifier
  surface.
- Added `verify_artifact` for `LMC1` container, manifest, feature, payload-kind,
  and L1 structural verification reports.
- Added `verify_artifact_with_l2_core` to fuse accepted `verify_l2_core` facts,
  proof-obligation IDs, constraint status, and lowering readiness.
- Added `loom verify-artifact <payload>` for reviewer-visible artifact reports.
- Added `scripts/artifact-verifier-test.sh` and wired it into
  `scripts/mvp0-verify.sh`.
- Added Phase 17 contract and final report documentation.

## Commits

- `048af03 feat(17-01): add artifact verifier report model`
- `e3c0f49 feat(17-02): add artifact container verifier`
- `c8307b9 feat(17-03): fuse l2core verifier facts`
- `08b1aba feat(17-04): add artifact verifier CLI gate`
- final closeout commit: Phase 17 docs, report, state, and roadmap closure

## Commands

- `cargo test --workspace`
- `cargo test -p loom-core --test artifact_verifier`
- `cargo run --bin loom -- --help | rg -q "verify-artifact"`
- `bash scripts/artifact-verifier-test.sh`
- `bash scripts/mvp0-verify.sh`
- `git diff --check`

## Deviations

No scope expansion was added. Phase 17 stayed focused on the unified verifier
pipeline and did not add solver execution, a stable external `L2Core` codec,
complete Vortex reader support, production MLIR dialect work, or host-engine
native execution.

## Residual Risks

- Constraint obligations are collected but not discharged by a real solver.
- `L2Core` verification is still integrated through Rust data models rather than
  a stable artifact codec.
- Value-dependent semantic checks still require runtime guards and
  oracle/equivalence evidence.
- Phase 18 must expand the complete Vortex reader boundary before later engine
  integration consumes real artifact semantics.
- Phase 19 must preserve production decode dialect and native kernel expansion
  instead of treating the Phase 14/16 copy slice as a production compiler.

## Handoff

Phase 18 is the next roadmap placeholder: Complete Vortex Reader. Phase 19
remains the production decode dialect/native kernel expansion phase after the
reader boundary and unified verifier facts are available.
