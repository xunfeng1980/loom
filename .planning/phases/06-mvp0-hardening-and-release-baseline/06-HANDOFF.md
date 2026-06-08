---
phase: 06-mvp0-hardening-and-release-baseline
status: handoff
created: 2026-06-08
---

# Phase 7 Handoff Notes

Phase 6 leaves MVP0 as a reproducible baseline. The recommended next technical phase is not another kernel; it is making Loom's layout contract independent and inspectable.

## Recommended Phase 7

**Name:** Human-Readable Layout Descriptor and CLI

**Primary requirements:** DX-01, DX-03, DX-02, optional DX-04

**Goal:** A reviewer can inspect and decode a Loom layout payload without reading Rust tests or Vortex bridge code.

## Suggested Scope

- Define a human-readable recursive layout descriptor.
- Prefer a tree-friendly format such as S-expression or RON over TOML unless readability tests prove otherwise.
- Add `LayoutDescription -> text` and `text -> LayoutDescription` roundtrips.
- Keep `loom-core` Vortex-free; Vortex remains a descriptor producer in `loom-fixtures`.
- Add a CLI surface:
  - `loom inspect <payload-or-descriptor>`
  - `loom decode <payload-or-descriptor>`
- Expand fixtures per encoding after the descriptor path is stable.
- Add timing output only as illustrative wall-clock comparison, not a benchmark claim.

## Explicit Non-Scope for Phase 7

- Multi-column DuckDB output.
- ArrowArrayStream replacement.
- Additional L2 kernels such as ALP or delta-of-delta.
- Formal verifier or MLIR/native lowering.
- Full `.vortex` file container support.

## Recommended Phase 8

**Name:** Multi-Column Table Output and Arrow Stream

Phase 8 should make Loom output table-shaped data: multi-column schema assembly, mixed Int/Boolean/Utf8 columns, and DuckDB SQL over multiple columns. Revisit ArrowArrayStream only there, when record-batch-shaped output exists.
