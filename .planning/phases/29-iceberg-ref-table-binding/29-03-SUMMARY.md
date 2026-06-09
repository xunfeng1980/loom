---
phase: 29-iceberg-ref-table-binding
plan: 03
subsystem: source-adapter
tags: [rust, iceberg, artifact-verifier, source-ingress, serde_json, shasum]
requires:
  - phase: 29-02
    provides: typed local Iceberg metadata and Loom sidecar parsing into descriptive facts
provides:
  - Verifier, SHA-256, source-evidence, and decoded-row oracle gated accepted Iceberg binding
  - Concrete source/oracle evidence fixture referenced by the accepted sidecar
  - Accepted handoff tests proving sidecar claims alone cannot return artifact bytes
  - Focused gate coverage for accepted binding and artifact verifier tests
affects: [phase-28, phase-29, iceberg-binding, source-ingress, release-gates]
tech-stack:
  added: []
  patterns: [independent evidence artifact validation, adapter-local shasum helper, verifier-backed accepted handoff]
key-files:
  created:
    - crates/loom-iceberg-binding/tests/binding_handoff.rs
    - crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-source-evidence.json
  modified:
    - Cargo.lock
    - crates/loom-iceberg-binding/Cargo.toml
    - crates/loom-iceberg-binding/src/binding_contract.rs
    - crates/loom-iceberg-binding/src/lib.rs
    - crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-loom-binding.json
    - scripts/iceberg-binding-test.sh
key-decisions:
  - "Accepted Iceberg bindings require local artifact bytes, recomputed SHA-256, live verify_artifact acceptance, and a sidecar-referenced evidence JSON artifact."
  - "Sidecar verifier/source/oracle accepted flags are necessary descriptive inputs but never sufficient to construct accepted evidence."
  - "The focused Phase 29 gate runs accepted handoff and artifact verifier tests but remains unwired from mvp0-verify until Plan 29-05."
patterns-established:
  - "SourceOracleEvidence::accepted(SourceOracleStrategy::DecodedRowFixture, row_count) is constructed only after the evidence JSON row count, table UUID, schema ID, snapshot ID, artifact SHA-256, source accepted status, and decoded-row oracle status match."
  - "Unsupported/rejected binding paths return IcebergBindingReport without artifact bytes or accepted evidence."
requirements-completed: [PHASE-29]
duration: 5m37s
completed: 2026-06-08T22:39:18Z
---

# Phase 29 Plan 03: Accepted Binding Validation Summary

**Verifier-backed Iceberg binding that accepts only matched local artifact bytes, recomputed SHA-256, and concrete source/oracle evidence JSON**

## Performance

- **Duration:** 5m37s
- **Started:** 2026-06-08T22:33:41Z
- **Completed:** 2026-06-08T22:39:18Z
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments

- Added accepted binding handoff coverage that generates real local `LMC1(LMT1)` bytes, computes the expected SHA-256 with `shasum`, and proves stale sidecar hashes, mutated artifact bytes, missing evidence, and stale/forged evidence fail closed.
- Implemented `bind_iceberg_ref_from_paths`, which reads artifact bytes, recomputes SHA-256, calls `verify_artifact`, parses the sidecar-referenced evidence JSON, and validates row count, table UUID, schema ID, snapshot ID, artifact SHA-256, source accepted status, and decoded-row oracle accepted status before returning bytes.
- Extended `scripts/iceberg-binding-test.sh` to run binding handoff tests and `loom-core` artifact verifier tests while preserving the no-SDK, no-public-route, and unwired-main-gate boundaries.

## Task Commits

1. **Task 1: Add accepted binding handoff tests** - `3f69bb5` (`test`)
2. **Task 2: Implement verifier and SHA-256 backed accepted binding** - `52fb6e1` (`feat`)
3. **Task 3: Add unsupported and rejected binding validation coverage to the focused gate** - `ee1ab31` (`test`)

## Files Created/Modified

- `Cargo.lock` - Recorded test-only Arrow crate references for the Iceberg binding test target.
- `crates/loom-iceberg-binding/Cargo.toml` - Added workspace-pinned `arrow-array` and `arrow-schema` dev dependencies for real artifact generation in tests.
- `crates/loom-iceberg-binding/src/binding_contract.rs` - Added accepted binding validation, adapter-local SHA-256 helper, live verifier call, typed evidence JSON parsing, and accepted source/oracle evidence construction.
- `crates/loom-iceberg-binding/src/lib.rs` - Exported `bind_iceberg_ref_from_paths`.
- `crates/loom-iceberg-binding/tests/binding_handoff.rs` - Added TDD handoff tests for accepted and fail-closed binding behavior.
- `crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-loom-binding.json` - Added accepted flags and a concrete source/oracle evidence artifact path.
- `crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-source-evidence.json` - Added concrete decoded-row/source evidence fixture.
- `scripts/iceberg-binding-test.sh` - Added accepted handoff, artifact verifier, evidence marker, and fixture checks.

## Decisions Made

- Kept the hash helper adapter-local and shell-backed with `shasum -a 256`, matching the Phase 27 fixture pattern.
- Treated sidecar `accepted`/`status` fields as required claims but not proof; accepted report construction uses live verifier facts and the independently parsed evidence artifact.
- Preserved Plan 29-03 scope by leaving `scripts/mvp0-verify.sh` unwired and adding no Iceberg SDK, catalog, object-store, SQL, CLI, C ABI, DuckDB, or StarRocks route surface.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added test-only Arrow dev dependencies**
- **Found during:** Task 1
- **Issue:** The new handoff test needed to build and decode real `LMC1(LMT1)` bytes, but `loom-iceberg-binding` did not expose `arrow-array` or `arrow-schema` to its test target.
- **Fix:** Added workspace-pinned `arrow-array` and `arrow-schema` under `[dev-dependencies]`; no new package names or runtime dependencies were introduced.
- **Files modified:** `crates/loom-iceberg-binding/Cargo.toml`, `Cargo.lock`
- **Verification:** `cargo test -p loom-iceberg-binding --test binding_handoff` failed only on the missing production function during RED, then passed after Task 2.
- **Committed in:** `3f69bb5`

---

**Total deviations:** 1 auto-fixed (Rule 3 blocking)
**Impact on plan:** Test infrastructure only. Runtime binding scope and dependency boundaries remain unchanged.

## Issues Encountered

- `gsd-tools` was not available on PATH in the shell, so the local `node $HOME/.codex/gsd-core/bin/gsd-tools.cjs` shim is used for GSD state operations.

## Verification

- `cargo test -p loom-iceberg-binding --test binding_handoff`
- `cargo test -p loom-iceberg-binding --test binding_contract`
- `cargo test -p loom-iceberg-binding --test dependency_boundary`
- `cargo test -p loom-core --test artifact_verifier`
- `bash -n scripts/iceberg-binding-test.sh && bash scripts/iceberg-binding-test.sh`
- `! rg -q "iceberg-binding-test\\.sh" scripts/mvp0-verify.sh`
- `rg -n "Command::new\\(\"shasum\"\\)|verify_artifact|SourceOracleStrategy::DecodedRowFixture|serde_json::" crates/loom-iceberg-binding/src/binding_contract.rs`

## Known Stubs

None. Empty terminal color variables in `scripts/iceberg-binding-test.sh` are intentional non-TTY fallback values.

## User Setup Required

None.

## Next Phase Readiness

Plan 29-04 can build on a fail-closed accepted binding function and focused gate evidence. Accepted bindings now require concrete local artifact bytes, live verifier acceptance, recomputed SHA-256, and matching sidecar-referenced source/oracle evidence before bytes are returned.

## Self-Check: PASSED

- Created files exist: `crates/loom-iceberg-binding/tests/binding_handoff.rs` and `crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-source-evidence.json`.
- Modified key files exist: `crates/loom-iceberg-binding/src/binding_contract.rs`, `crates/loom-iceberg-binding/src/lib.rs`, and `scripts/iceberg-binding-test.sh`.
- Task commits exist: `3f69bb5`, `52fb6e1`, and `ee1ab31`.
- Overall verification passed.
- No accidental tracked-file deletions were reported after task commits.

---
*Phase: 29-iceberg-ref-table-binding*
*Completed: 2026-06-08T22:39:18Z*
