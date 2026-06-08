---
phase: 07-human-readable-layout-descriptor-and-cli
status: planning
created: 2026-06-08
depends_on:
  - phase: 06-mvp0-hardening-and-release-baseline
    provides: reproducible MVP0 release gate
requirements: [DX-01, DX-02, DX-03, DX-04]
scope:
  - human-readable descriptor format
  - descriptor parse/print roundtrip
  - CLI inspect/decode surface
  - expanded fixture samples
  - illustrative timing output
out_of_scope:
  - multi-column DuckDB output
  - ArrowArrayStream replacement
  - additional L2 kernels
  - verifier and safety-boundary demo
  - MLIR/native backend
  - full .vortex file container support
---

# Phase 07 Context: Human-Readable Layout Descriptor and CLI

## Current State

MVP0 is complete and reproducible through `scripts/mvp0-verify.sh`. The current `.loom` payload format is binary and internal. It proves the decode path but is not reviewer-friendly: users must read Rust tests, fixture emitter code, or the binary payload codec to understand what layout is being decoded.

Phase 7 makes the Loom layout contract visible.

## Design Direction

The descriptor should be tree-friendly because `LayoutNode` is recursive. TOML is possible but likely awkward for nested dictionary, run-end, FOR, and kernel-escape shapes. Phase 7 should evaluate and then commit to a simple format, with RON or S-expression as the likely options.

The descriptor is not a full distribution container. It is a readable representation of the existing MVP0 `LayoutDescription` and enough kernel parameters to inspect/decode existing fixtures.

## Required Invariants

- `loom-core` remains free of Vortex/FastLanes dependencies.
- Vortex remains isolated in `loom-fixtures` as a producer/oracle.
- Existing binary `.loom` payloads continue to decode.
- `scripts/mvp0-verify.sh` remains green after every plan.
- CLI commands are reviewer tools, not a production ABI.

## Expected User-Facing Commands

```bash
loom inspect target/loom-duckdb-fixtures/bitpack-i32.loom
loom decode target/loom-duckdb-fixtures/fsst-utf8.loom
```

The final binary name and package location are decided in 07-03, but commands should be exposed through Cargo without requiring custom shell setup.

## Phase 7 Waves

- **Wave 1:** descriptor format and core parse/print.
- **Wave 2:** payload inspection bridge and CLI.
- **Wave 3:** expanded fixtures, timing, docs, and full release gate.
