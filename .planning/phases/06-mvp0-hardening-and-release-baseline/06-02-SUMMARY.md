---
phase: 06-mvp0-hardening-and-release-baseline
plan: "02"
subsystem: scripts
tags: [verification, build, duckdb, ci]
requirements_completed: [VERIFY-04, BUILD-01]
completed: 2026-06-08
---

# Phase 06-02: One-Command MVP0 Verification Gate Summary

Phase 06-02 added a single release-gate script for the completed MVP0 baseline.

## Accomplishments

- Added executable `scripts/mvp0-verify.sh`.
- The script runs:
  - `cargo test --workspace`
  - `cargo tree -p loom-core` dependency guard for Vortex/FastLanes isolation
  - fixture hygiene grep for file-backed Vortex APIs
  - `bash scripts/duckdb-smoke-test.sh`
- The script resolves the repository root with `git rev-parse --show-toplevel`, so it works from subdirectories.
- The release gate relies on the existing smoke-test stale-artifact protection: explicit `cargo build -p loom-ffi --release` plus removal of the old DuckDB extension before relink.

## Verification

- `bash scripts/mvp0-verify.sh` - PASS.
- `cd crates/loom-core && bash ../../scripts/mvp0-verify.sh` - PASS.

Both runs passed all workspace tests, dependency hygiene checks, fixture hygiene checks, and DuckDB SQL smoke tests for bitpack-i32, for-i32, dict-i32, rle-i32, fsst-utf8, and dict-fsst-utf8.

## Notes

No CMake change was required for Phase 6. The release gate is the supported acceptance path, and it already forces the Rust staticlib rebuild before relinking the DuckDB extension.
