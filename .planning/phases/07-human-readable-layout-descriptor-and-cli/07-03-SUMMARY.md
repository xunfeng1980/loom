---
phase: 07-human-readable-layout-descriptor-and-cli
plan: "03"
subsystem: cli
tags: [cli, inspect, decode]
requirements_completed: [DX-03]
completed: 2026-06-08
---

# Phase 07-03: CLI Inspect and Decode Summary

Phase 07-03 added the reviewer-facing Loom CLI.

## Accomplishments

- Added `crates/loom-cli` as a Vortex-free workspace crate.
- Added binary target `loom`.
- Implemented `loom inspect <payload-or-descriptor>`.
- Implemented `loom decode <payload-or-descriptor>`.
- CLI accepts binary LMP1 `.loom` payloads and descriptor text inputs.
- CLI prints `NULL` explicitly for null rows.

## Verification

- `cargo run --bin loom -- inspect target/loom-duckdb-fixtures/bitpack-i32.loom` - PASS.
- `cargo run --bin loom -- decode target/loom-duckdb-fixtures/fsst-utf8.loom` - PASS, printed `alpha`, `NULL`, `beta`.
