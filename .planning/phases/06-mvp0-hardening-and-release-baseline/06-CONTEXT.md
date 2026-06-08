---
phase: 06-mvp0-hardening-and-release-baseline
status: planning
created: 2026-06-08
depends_on:
  - phase: 05-fsst-l2-kernel-and-full-verification
    provides: MVP0 DuckDB SQL acceptance gate
scope:
  - planning/documentation consistency
  - one-command MVP0 verification
  - Rust/DuckDB build hygiene
  - Phase 7 readiness notes
out_of_scope:
  - human-readable descriptor implementation
  - CLI implementation
  - multi-column table output
  - additional L2 kernels
  - verifier or MLIR/native backend
---

# Phase 06 Context: MVP0 Hardening and Release Baseline

## Current State

MVP0 is complete. Phase 5 produced deterministic `.loom` payloads for bitpack, FOR, dict, RLE, FSST, and dict-over-FSST, and `scripts/duckdb-smoke-test.sh` verifies exact DuckDB SQL rows and aggregates across all supported encodings.

The final Phase 5 verification suite passed:

- `cargo test --workspace`
- `cargo tree -p loom-core | awk '/vortex|fastlanes/{c++} END{print c+0}'` -> `0`
- `rg -n 'vortex_file|vortex-file|\\.vortex|VortexFile|from_path|read_file' crates/loom-fixtures` -> no matches
- `bash scripts/duckdb-smoke-test.sh`

## Why Phase 6 Exists

The fast MVP0 execution left the implementation ahead of the project documentation. Several docs still describe Phase 3 or Phase 5 as active, and the full acceptance gate is spread across multiple commands. Phase 6 turns the completed prototype into a stable baseline for v2.

The next technical milestone should be descriptor/CLI work, but starting that before cleanup would create avoidable ambiguity about what is already guaranteed.

## Required Outcomes

1. Planning state consistently says MVP0 is complete and Phase 6 is active.
2. README explains the current implementation surface and how to run the MVP0 gate.
3. Vortex / AnyBlox / F3 positioning is linked from public docs.
4. A single script runs the whole MVP0 release gate from the repository root.
5. The release gate prevents stale Rust staticlib or DuckDB extension artifacts from hiding regressions.
6. Phase 7 handoff notes identify descriptor/CLI as the next recommended technical step.

## Constraints

- Do not change the binary `.loom` payload format except as required for verification script ergonomics.
- Do not add new runtime dependencies.
- Do not pull Vortex or FastLanes into `loom-core`.
- Do not replace direct DuckDB `DataChunk` population in Phase 6; revisit ArrowArrayStream only with multi-column output.
- Keep docs factual: current implementation is an MVP0 interpreter demo, not the full Loom distribution IR described in README.

## Phase 6 Plan Waves

- **Wave 1:** Documentation and planning-state consistency.
- **Wave 2:** One-command release gate and stale-artifact build hygiene.
- **Wave 3:** Final baseline audit and Phase 7 readiness notes.
