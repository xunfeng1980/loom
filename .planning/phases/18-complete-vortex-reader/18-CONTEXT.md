# Phase 18 Context: Complete Vortex Reader

**Status:** Research context
**Date:** 2026-06-08

## Problem Statement

Phase 15 proved Loom can touch real Vortex files, inspect them into Loom-owned
facts, and emit one non-null Int32 `.vortex` -> `LMC1` slice. That is not a
complete Vortex reader.

Phase 18 should expand that boundary so later phases can rely on real artifact
semantics:

```text
complete local Vortex file reader boundary
  -> stable reader facts
  -> explicit support matrix
  -> LMC1/LMT1 emission where supported
  -> Phase 17 artifact verifier acceptance
  -> Vortex scan oracle/equivalence evidence
```

## Key Decisions

### D-18-01: Complete reader means complete boundary, not arbitrary native decode

Phase 18 should define complete file/dtype/layout/segment/statistics facts and
support classification. It does not have to support every Vortex encoding as a
Loom emission target in one pass.

### D-18-02: Vortex APIs remain isolated

Direct `vortex-file` / `vortex-layout` use remains isolated to
`crates/loom-vortex-ingress`. `loom-core` and `loom-ffi` remain Vortex-free.

### D-18-03: Vortex scan is oracle evidence

`VortexFile::scan()` may be used for semantic oracle rows. It must not become
the implementation path for emitted Loom artifacts.

### D-18-04: Support matrix is explicit and fail-closed

Every real reader shape is classified as accepted, unsupported, or rejected.
Unsupported files may expose facts but must not emit partial `.loom` bytes.

### D-18-05: Phase 19 depends on Phase 18 facts

The solver-backed verifier should discharge obligations over real complete-reader
facts. Phase 18 should therefore record enough dtype/layout/segment structure to
be useful as solver input.

## Expected Deliverables

- `VortexReaderFacts` or an equivalent evolution of `VortexFileFacts`.
- Recursive dtype/layout/segment facts.
- Stable reader diagnostics and support status.
- Expanded single-column `.vortex` -> `LMC1` support.
- Real struct/table `.vortex` -> `LMT1` support for supported field shapes.
- Phase 17 artifact verifier checks for emitted artifacts.
- Vortex scan oracle comparisons for every emitted fixture.
- CLI complete-reader report.
- Phase 18 release-gate script wired into `scripts/mvp0-verify.sh`.

## Explicit Non-Goals

- No solver discharge.
- No production MLIR/native dialect work.
- No host-engine native execution.
- No object-store credential handling.
- No encrypted or forward-compatible WASM Vortex files.
- No weakening of the `loom-core` / `loom-ffi` dependency boundary.

## Handoff to Planning

Recommended plan split:

1. `18-01` reader facts contract and dependency boundary.
2. `18-02` recursive layout/dtype/segment inspection.
3. `18-03` supported single-column conversion matrix.
4. `18-04` supported struct/table conversion.
5. `18-05` CLI/report/release-gate closeout.
