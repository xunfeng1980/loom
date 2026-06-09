---
status: in_progress
created: 2026-06-09
---

# Fix DuckDB CMake Fail-Closed Gates

## Goal

Audit skip/fake-pass risks in the current DuckDB execution flow and fix any gate
that can mask a failed build/configure step.

## Scope

- Make DuckDB extension CMake configure fail closed in focused gates.
- Re-run DuckDB focused gates after the change.
- Record that explicit `LOOM_ALLOW_*_SKIP=1` paths remain documented tradeoffs,
  not default success paths.
