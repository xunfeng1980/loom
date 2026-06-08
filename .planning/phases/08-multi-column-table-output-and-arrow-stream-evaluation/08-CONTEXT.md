---
phase: 08-multi-column-table-output-and-arrow-stream-evaluation
status: planning
created: 2026-06-08
depends_on:
  - phase: 07-human-readable-layout-descriptor-and-cli
    provides: descriptor text, CLI inspect/decode, fixture expansion, MVP0 release gate
requirements: [COV-02, TABLE-01, TABLE-02, TABLE-03, DUCK-05, STREAM-01, VERIFY-05]
scope:
  - table-shaped descriptions
  - mixed-column table payload codec
  - Rust multi-column decode
  - CLI table inspect/decode
  - DuckDB multi-column loom_scan
  - ArrowArrayStream decision
  - multi-column SQL acceptance
out_of_scope:
  - additional L2 kernels
  - verifier and safety-boundary demo
  - MLIR/native backend
  - full .vortex file container support
  - projection/range/statistics ABI
---

# Phase 08 Context: Multi-Column Table Output and Arrow Stream Evaluation

## Current State

MVP0 and Phase 7 are complete. Loom can decode one-column `.loom` payloads, print RON descriptor text, expose CLI inspect/decode commands, and pass the release gate through DuckDB SQL.

The next structural gap is table shape. Real engines consume tables, not isolated columns. Phase 8 promotes the payload and DuckDB surface from "one unnamed value column" to multiple named columns with a shared row count.

## Design Direction

Add a table-level model that composes existing `LayoutDescription` values. The table model should not replace single-column payload compatibility; existing LMP1 payloads and `scripts/mvp0-verify.sh` must continue to pass.

Phase 8 should also evaluate the old ArrowArrayStream question with actual table-shaped output. If the vendored DuckDB API path is stable and practical, implement it. If not, keep direct `DataChunk` population and document the reason in the Phase 8 summary.

## Required Invariants

- `loom-core` remains Vortex/FastLanes-free.
- Existing single-column LMP1 payloads still decode and scan.
- Multi-column payload decode fails with typed errors on row-count/schema mismatches.
- DuckDB SQL sees named output columns, not a single generic `value`.
- Direct DataChunk population is acceptable only with a recorded ArrowArrayStream decision.

## Candidate Multi-Column Fixture

A deterministic table fixture should include at least:

- `id`: Int32, bitpack/FOR-backed numeric values.
- `flag`: Boolean, RLE-backed values.
- `label`: Utf8, FSST-backed strings with at least one null or edge value.

The SQL gate should check row output, `COUNT`, `SUM(id)`, `COUNT(label)`, and a filter such as `WHERE flag`.

## Phase 8 Waves

- **Wave 1:** table model and table payload codec.
- **Wave 2:** Rust/CLI table fixtures and DuckDB multi-column scan.
- **Wave 3:** SQL acceptance, docs, and requirement closure.
