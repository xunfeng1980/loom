---
phase: 31-full-arrow-semantic-source-compatibility
plan: 01
status: complete
completed: 2026-06-09
---

# 31-01 Summary

## Completed

- Added `31-ARROW-SEMANTIC-CONTRACT.md` defining the `LMC2`/`LMA1` substrate,
  acceptance rules, verification requirements, and non-goals.
- Confirmed the abandoned `NullableRaw` WIP is not present in `loom-core`.
- Added `loom_core::arrow_semantic`, `arrow_semantic_codec`, and
  `arrow_semantic_verifier` scaffolds.
- Added focused tests for stable markers, schema/column count checks, row-count
  checks, Arrow `validate_full` verifier path, and source-reader dependency
  isolation in `loom-core`.

## Verification

- `cargo fmt`
- `cargo test -p loom-core arrow_semantic`

## Tradeoff

This plan intentionally implements only the contract and core scaffold. The
deterministic full `LMA1` codec and broad Arrow type matrix remain in 31-02.
