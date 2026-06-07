# Phase 05: fsst-l2-kernel-and-full-verification - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-06-08
**Phase:** 05-fsst-l2-kernel-and-full-verification
**Areas discussed:** FSST payload / dependency boundary, dict-over-FSST / Utf8 integration, DuckDB SQL gate, verification corpus scope

---

## FSST Payload / Dependency Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Loom-owned params + fsst-rs | Keep `loom-core` zero Vortex dependency; add `fsst-rs`; params carry symbols, symbol lengths, compressed codes, offsets, validity, and metadata. | yes |
| Local hand-written decoder | Avoid dependency but replicate FSST escape/symbol decode semantics locally. | |
| `loom-core` directly uses Vortex | Fastest route to reference behavior but violates D-02 and Phase 4 architecture. | |

**User's choice:** Use the recommended option.
**Notes:** The codebase scout found `vortex-fsst` decomposes arrays into symbols,
symbol lengths, compressed codes bytes, offsets, uncompressed lengths, and
validity. `fsst-rs` exposes `fsst::Decompressor`, making it suitable for the
core kernel without importing Vortex.

---

## dict-over-FSST / Utf8 Integration

| Option | Description | Selected |
|--------|-------------|----------|
| General `ArrayData` gather | FSST L2 returns Utf8 `ArrayData`; Dictionary materializes values and gathers by decoded codes; extend decoded-child / builder support for Utf8. | yes |
| Top-level FSST only | Complete top-level `KernelEscape(FSST)` and defer dict-over-FSST. | |
| Dedicated dict-FSST kernel | Collapse dict-over-FSST into a special L2 kernel path. | |

**User's choice:** Use the recommended option.
**Notes:** This preserves the Phase 4 design where Dictionary is a real L1 arm
and L2 kernels own their output arrays.

---

## DuckDB SQL Gate

| Option | Description | Selected |
|--------|-------------|----------|
| Extend direct `DataChunk` filling | Continue current `loom_extension.cpp` path and add Arrow Utf8 / needed primitive vector population. | yes |
| Switch to Arrow stream | Emit record batches / ArrowArrayStream and delegate to DuckDB `arrow_scan`. | |
| Rust SQL substitute | Validate SQL-like behavior outside DuckDB and defer real DuckDB gate. | |

**User's choice:** Use the recommended option.
**Notes:** Phase 2 already revised the DuckDB route to direct DataChunk
population because the stub emitted a bare primitive array. For Phase 05, the
MVP0 gate must use real DuckDB SQL over `loom_scan(...)`.

---

## Verification Corpus Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Controlled MVP0 full gate | Cover FSST edge cases, dict-over-FSST, all supported L1 encodings, oracle row-match, and DuckDB SELECT + aggregate smoke. | yes |
| FSST-only | Focus on `L2-02` / `L2-03` and defer DuckDB plus all-L1 SQL coverage. | |
| Exhaustive matrix | Test every combination aggressively; highest confidence but phase may balloon. | |

**User's choice:** Use the recommended option.
**Notes:** The selected scope satisfies roadmap success criteria without turning
Phase 05 into a broad v2 test expansion.

---

## the agent's Discretion

- Exact binary params format and helper names.
- Fixture file organization and test module boundaries.
- Whether Utf8 support is implemented by extending `OutputBuilder` directly or
  by a narrow equivalent abstraction, provided Arrow builders remain the output
  path.

## Deferred Ideas

- ArrowArrayStream / `arrow_scan` integration unless direct `DataChunk`
  population becomes a hard blocker.
- Exhaustive nested-encoding verification matrix.
- v2 roadmap items: additional L2 kernels, multi-column table function,
  human-readable layout descriptor, and timing comparison.
