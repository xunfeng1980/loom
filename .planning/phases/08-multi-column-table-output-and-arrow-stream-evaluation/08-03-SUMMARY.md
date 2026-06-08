---
phase: 08-multi-column-table-output-and-arrow-stream-evaluation
plan: "03"
subsystem: duckdb
tags: [duckdb, ffi, table, arrow-stream]
requirements_completed: [DUCK-05, STREAM-01]
completed: 2026-06-08
---

# Phase 08-03: DuckDB Multi-Column Scan and ArrowArrayStream Decision Summary

Phase 08-03 extended `loom_scan` from a single `value` column to named multi-column table payloads.

## Accomplishments

- `loom_scan` now detects `LMT1` table payloads and binds named columns from the embedded payload metadata.
- Single-column `LMP1` payloads still bind as the existing `value` column.
- DuckDB init decodes each embedded column payload through the existing `loom_decode` FFI surface.
- DuckDB scan fills multiple `DataChunk` columns for Int32, Boolean, and Utf8 data.
- Added length mismatch checks before emitting a multi-column batch.

## ArrowArrayStream Decision

Direct `DataChunk` population remains the Phase 8 implementation path.

Repo-specific evidence: the current extension already has a stable direct path that can bind table-shaped schemas and populate mixed DuckDB vectors from Arrow C arrays. The earlier Arrow stream attempt was blocked by DuckDB's requirement for a top-level record-batch/struct shape while `loom_decode` intentionally exports bare column arrays. Phase 8 preserves the FFI signature and wraps multiple column payloads above it instead of widening the trust boundary with a new stream ABI.

ArrowArrayStream remains a future ABI decision for a later phase that can introduce a true table/record-batch FFI output contract.

## Verification

- `cmake --build duckdb-ext/build` - PASS.
- Manual SQL over `mixed-table.loom` - PASS.
- `bash scripts/duckdb-smoke-test.sh` - PASS.

