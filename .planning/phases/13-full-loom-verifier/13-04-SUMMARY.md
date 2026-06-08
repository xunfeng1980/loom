# Phase 13-04 Summary

**Plan:** `13-04-PLAN.md`
**Status:** Complete
**Date:** 2026-06-08
**Commit:** `fe4bba1 test(13-04): add formal verifier gate`

## Completed

- Added `formal/lean/LoomCore.lean` with the Phase 13 `L2Core` Lean scaffold,
  including `L2Ty`, `Capability`, `ArrowEvent`, `Stmt`, `Program`, `Verified`,
  `Safe`, `builder_events_well_formed`, and `accepted_program_safe`.
- Added `specs/tla/LoomVerifierPipeline.tla` and `.cfg` with lifecycle states
  and the `LoweredImpliesVerified` invariant.
- Added `scripts/full-verifier-test.sh`, a repeatable Phase 13 gate that checks
  docs, obligation IDs, Rust model/verifier tests, CLI visibility, Lean when
  installed, and TLC when installed.
- Updated the proof-obligation matrix with Lean/Rocq and TLA+ evidence for
  `VERIFIER-09` and `VERIFIER-10`.

## Verification

```bash
bash scripts/full-verifier-test.sh
rg -n "inductive L2Ty|inductive Capability|inductive ArrowEvent|inductive Stmt|structure Program|def Verified|def Safe|builder_events_well_formed|accepted_program_safe" formal/lean/LoomCore.lean
rg -n "Raw|Parsed|Verified|Rejected|Lowerable|Lowered|Invalidated|requiredFeaturesAccepted|resourceBounded|verifiedFactsPresent|LoweredImpliesVerified" specs/tla/LoomVerifierPipeline.tla specs/tla/LoomVerifierPipeline.cfg
rg -n "VERIFIER-09|VERIFIER-10|accepted_program_safe|LoweredImpliesVerified|full-verifier" .planning/phases/13-full-loom-verifier/13-PROOF-OBLIGATIONS.md scripts/full-verifier-test.sh
git diff --check
```

Lean and TLC were not installed in this environment, so the gate reported both
optional checks as skipped.

## Result

Wave 3 is complete. Phase 13 can proceed to `13-05`: final verifier report,
public/planning docs, release-gate wiring, final verification, and requirement
closure.

## Closed Requirements

- `VERIFIER-01` formal scaffold alignment
- `VERIFIER-03` Lean scaffold portion
- `VERIFIER-04` lifecycle/resource invariant portion
- `VERIFIER-05` Lean builder theorem scaffold portion
- `VERIFIER-09`
- `VERIFIER-10` TLA lifecycle portion
