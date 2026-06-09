---
phase: 29-iceberg-ref-table-binding
plan: 01
subsystem: source-adapter
tags: [rust, iceberg, source-ingress, serde_json, dependency-boundary]
requires:
  - phase: 26-external-source-ingress-contract
    provides: source-neutral verifier/source/oracle evidence vocabulary
  - phase: 27-lance-parquet-archival-readability-dataset-ingress
    provides: adapter-local dependency boundary and focused gate patterns
provides:
  - Adapter-local `loom-iceberg-binding` crate scaffold
  - Iceberg table/ref binding report model with accepted/unsupported/rejected semantics
  - Focused dependency and public-surface guard script
affects: [phase-28, phase-29, source-ingress, release-gates]
tech-stack:
  added: [serde_json = =1.0.150]
  patterns: [adapter-local binding crate, Loom-owned report model, focused unwired gate]
key-files:
  created:
    - crates/loom-iceberg-binding/Cargo.toml
    - crates/loom-iceberg-binding/src/lib.rs
    - crates/loom-iceberg-binding/src/binding_contract.rs
    - crates/loom-iceberg-binding/tests/binding_contract.rs
    - crates/loom-iceberg-binding/tests/dependency_boundary.rs
    - scripts/iceberg-binding-test.sh
  modified:
    - Cargo.toml
    - Cargo.lock
key-decisions:
  - "Keep Phase 29 Iceberg vocabulary adapter-local and out of core/FFI/source-ingress/public surfaces."
  - "Pin only serde_json = =1.0.150 for local JSON parsing; do not add the official iceberg SDK by default."
  - "Create the focused Iceberg binding gate but leave it unwired from mvp0-verify until later Phase 29 closeout."
patterns-established:
  - "Accepted Iceberg binding reports require facts, verifier acceptance, accepted source evidence, accepted oracle evidence, identity match, snapshot match, schema match, and artifact hash match."
  - "Unsupported and rejected binding reports carry no accepted verifier/oracle evidence and no artifact bytes."
requirements-completed: [PHASE-29]
duration: 4min
completed: 2026-06-08T22:24:29Z
---

# Phase 29 Plan 01: Iceberg Binding Scaffold Summary

**Adapter-local Iceberg table/ref binding scaffold with Loom-owned report semantics, exact local JSON dependency pin, and dependency/public-surface guards**

## Performance

- **Duration:** 4 min
- **Started:** 2026-06-08T22:20:32Z
- **Completed:** 2026-06-08T22:24:29Z
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments

- Registered `loom-iceberg-binding` as a workspace crate and pinned `serde_json = "=1.0.150"` for later local metadata parsing.
- Added `IcebergBindingStatus`, table/ref identity, binding facts, evidence, report, accepted artifact handoff, and fail-closed constructor errors.
- Added dependency-boundary tests and `scripts/iceberg-binding-test.sh` to prove no default Iceberg SDK dependency, no source-neutral/public surface creep, and no premature main-gate wiring.

## Task Commits

1. **Task 1: Add adapter crate and exact local JSON dependency** - `3bd8c80` (`feat`)
2. **Task 2: Define binding report model and accepted/unsupported/rejected constructors** - `2834c37` (`feat`)
3. **Task 3: Add dependency and public-surface guard tests plus initial focused gate** - `88e9f12` (`test`)

## Files Created/Modified

- `Cargo.toml` - Added the adapter workspace member and exact `serde_json` workspace pin.
- `Cargo.lock` - Recorded the new workspace package.
- `crates/loom-iceberg-binding/Cargo.toml` - Declared the adapter-local dependency boundary.
- `crates/loom-iceberg-binding/src/lib.rs` - Documented the local-only adapter boundary and exported Loom-owned contract types.
- `crates/loom-iceberg-binding/src/binding_contract.rs` - Defined binding identity, facts, evidence, report, errors, constructors, and accepted-artifact handoff.
- `crates/loom-iceberg-binding/tests/binding_contract.rs` - Covered accepted/unsupported/rejected semantics and fail-closed accepted-constructor requirements.
- `crates/loom-iceberg-binding/tests/dependency_boundary.rs` - Guarded SDK absence, `serde_json` placement, source neutrality, public surfaces, and unwired focused gate state.
- `scripts/iceberg-binding-test.sh` - Added the focused Phase 29 dependency/scope guard.

## Decisions Made

- Followed the Phase 29 default: no official `iceberg` crate in the default graph.
- Kept Iceberg binding evidence as Loom-owned structs plus `loom-source-ingress` evidence types rather than SDK or catalog types.
- Left `scripts/mvp0-verify.sh` unchanged because Plan 29-01 explicitly creates but does not wire the focused gate.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `gsd-tools` was not available on the shell PATH, so STATE/ROADMAP updates were applied directly rather than through SDK handlers.
- `.planning/STATE.md` already contained a Phase 29 execution-start edit before this plan completed; the update preserves that direction and advances only the current plan/session fields.

## Verification

- `cargo check -p loom-iceberg-binding`
- `cargo test -p loom-iceberg-binding`
- `cargo test -p loom-iceberg-binding --test dependency_boundary`
- `bash -n scripts/iceberg-binding-test.sh`
- `bash scripts/iceberg-binding-test.sh`
- `! rg -q "iceberg-binding-test\\.sh" scripts/mvp0-verify.sh`

## Known Stubs

None. The empty terminal color variables in `scripts/iceberg-binding-test.sh` are intentional non-TTY fallback values, not UI/data stubs.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: local-test-filesystem-scan | `crates/loom-iceberg-binding/tests/dependency_boundary.rs` | Test-only filesystem reads scan manifests and guarded public/source files for dependency and API boundary assertions. |

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 29-02 can add local Iceberg metadata and sidecar fixture parsing on top of the established crate, report types, and dependency guards. The focused gate is intentionally available but not yet part of the main release verifier.

## Self-Check: PASSED

- Created files exist: summary, adapter manifest, binding contract, contract tests, dependency-boundary tests, and focused gate script.
- Task commits exist: `3bd8c80`, `2834c37`, and `88e9f12`.
- No accidental tracked-file deletions were reported after task commits.

---
*Phase: 29-iceberg-ref-table-binding*
*Completed: 2026-06-08T22:24:29Z*
