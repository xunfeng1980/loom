---
phase: 51-sidecar-duckdb-decoupling-and-loom-self-ingress
plan: 02
subsystem: cli
tags: [rust, cargo-features, feature-gating, self-ingress, loom-container, cli, decoupling]

# Dependency graph
requires:
  - phase: 51-sidecar-duckdb-decoupling-and-loom-self-ingress
    plan: 01
    provides: "Lean loom-sidecar-ffi staticlib, container-free loom-parquet-ingress"
provides:
  - "loom-self-ingress crate wrapping loom-container as single .loom file IO boundary"
  - "Feature-gated loom-cli binary with lean compilation mode (zero container deps)"
affects: [51-sidecar-duckdb-decoupling-and-loom-self-ingress]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Single IO boundary crate — loom-self-ingress wraps loom-container, all .loom file access routes through it"
    - "Cargo feature-gated CLI — default=full enables all commands, --no-default-features enables only sidecar/verify-l2core"

key-files:
  created:
    - "crates/loom-self-ingress/Cargo.toml — library crate with sole dependency on loom-container"
    - "crates/loom-self-ingress/src/lib.rs — read_loom_file, write_loom_file, verify_loom_file public API + SelfIngressError"
  modified:
    - "Cargo.toml — added crates/loom-self-ingress to workspace members"
    - "crates/loom-cli/Cargo.toml — added [features] with full/default, made container deps optional"
    - "crates/loom-cli/src/main.rs — split into cfg-gated imports, run_full_commands dispatch"

key-decisions:
  - "loom-self-ingress depends on loom-container (not loom-core) — avoids Cargo feature unification and keeps dependency graph explicit"
  - "loom-cli run_full_commands uses impl Iterator<Item = String> to accept both Args and Skip<Args> without type mismatch"
  - "sidecar embed and verify-l2core use direct loom-ir-core imports (not via loom-core) — lean mode stays container-free"
  - "Lean-mode error message: 'command '{cmd}' requires full feature (rebuild without --no-default-features)' — matches RESEARCH.md Pattern 1"

requirements-completed: [SC-3, SC-4]

# Metrics
duration: 7 min
completed: 2026-06-11
status: complete
---

# Phase 51 Plan 02: Loom Self-Ingress Crate and CLI Feature Gate Summary

**Created the `loom-self-ingress` crate as the single IO boundary for `.loom` files (wrapping `loom-container` codecs), and feature-gated `loom-cli` so the `sidecar embed` and `verify-l2core` commands compile without `loom-container` in the dependency tree, while `inspect`/`decode`/`verify-artifact`/`ingest-vortex` require the `full` feature.**

## Performance

- **Duration:** 7 min
- **Started:** 2026-06-11T10:36:38Z
- **Completed:** 2026-06-11T10:43:19Z
- **Tasks:** 2
- **Files modified:** 5 (2 created, 3 modified)

## Accomplishments

- `crates/loom-self-ingress/` library crate with three public functions: `read_loom_file`, `write_loom_file`, `verify_loom_file`
- `SelfIngressError` enum (Io, NotALoomFile, InvalidContainer) implementing Display and Error
- Atomic temp-file + rename pattern in `write_loom_file` for safe `.loom` file writing
- `loom-cli` feature split: `default = ["full"]` enables all commands; `--no-default-features` builds only sidecar + verify-l2core
- Lean build verified: `cargo tree -p loom-cli --no-default-features | grep loom-container` returns zero lines
- Full-feature commands (`inspect`, `decode`, `verify-artifact`, `ingest-vortex`) emit clear `requires full feature` error in lean mode

## Task Commits

Each task was committed atomically:

1. **Task 1: Create loom-self-ingress crate wrapping loom-container for .loom file IO** — `c9e07ba` (feat)
2. **Task 2: Feature-gate loom-cli for lean sidecar compilation without container** — `8538dce` (feat)

## Files Created/Modified

- `crates/loom-self-ingress/Cargo.toml` — library crate with sole dependency on loom-container, workspace arrow dep
- `crates/loom-self-ingress/src/lib.rs` — read_loom_file, write_loom_file, verify_loom_file public API + SelfIngressError
- `Cargo.toml` — added crates/loom-self-ingress to workspace members
- `crates/loom-cli/Cargo.toml` — added [features] section, made container deps optional, added loom-ir-core as required
- `crates/loom-cli/src/main.rs` — split imports into always-available (ir-core + parquet-ingress) and cfg-gated (full), restructured run() dispatch

## Decisions Made

- **loom-self-ingress depends on loom-container directly** (not via loom-core) — keeps the dependency boundary explicit and avoids Cargo feature unification pulling in more than needed
- **`impl Iterator<Item = String>` in run_full_commands** — accepts both `env::Args` and `Skip<Args>` without type mismatch, avoiding the need to collect or retokenize arguments
- **Direct loom-ir-core imports for sidecar path** — `loom_ir_core::sidecar::SidecarOverlay`, `loom_ir_core::l2core_codec::{...}`, `loom_ir_core::full_verifier::verify_l2_core` are always available, never gated
- **Lean error message format** — `command '{cmd}' requires full feature (rebuild without --no-default-features)` matches RESEARCH.md Pattern 1, providing a clear, actionable error

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

- Pre-existing unreachable-pattern warnings in `loom-ir-core` (l2core_codec.rs, full_verifier.rs) — unrelated to this plan

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Ready for Plan 51-03 (DuckDB extension lean sidecar path via CMake build option)
- `loom-self-ingress` crate available at `crates/loom-self-ingress/`
- `loom-cli` lean binary available via `cargo build -p loom-cli --no-default-features`
- All success criteria verified: zero container in lean tree, sidecar embed works, full build compiles all commands, inspect/decode/verify-artifact produce clear errors in lean mode

## Self-Check: PASSED

- All 5 key files exist on disk
- Both commits (c9e07ba, 8538dce) found in git history
- All 5 plan success criteria verified
