# Phase 19 Plan 02 Summary: Deterministic Bitwuzla-primary SMT-LIB Emitter

**Status:** Complete
**Date:** 2026-06-08
**Self-Check:** PASSED

## Shipped

- Added `emit_required_qfbv_script` for deterministic Bitwuzla-compatible `QF_BV` SMT-LIB script generation.
- Added `SmtLibScriptFamily` so required scripts are distinct from optional cross-check scripts.
- Added `SmtLibScript::required_qfbv` and `SmtLibScript::cross_check_qflia` constructors.
- Encoded current `LoomConstraint` variants as named bad-state assertions.
- Added stable symbol sanitization, deterministic FNV-style script IDs, and stable 64-bit bit-vector constants.
- Added `smtlib_emitter` tests covering `QF_BV`, named bad states, overflow-aware read bounds, byte-stable output, and required-vs-cross-check script family separation.

## Commands

- `cargo test -p loom-core --test smtlib_emitter`
- `cargo test -p loom-core solver`
- `git diff --check`

## Deviations

The first emitter uses a stable 64-bit width policy for symbolic terms to avoid mixed-width declarations in Phase 19. Narrow native-width refinement is left for later backend-specific precision work.

## Residual Risks

- The emitter now produces replayable bad-state scripts, but no solver subprocess runs them yet.
- Bitwuzla discovery and strict/normal execution are pending for 19-03.
- Artifact facts are not updated from solver discharge until 19-04.

