# Phase 3: L1 Bitpack, FOR, and Arrow Builders - Discussion Log

> **Audit trail only.** Not consumed by downstream agents — decisions are in CONTEXT.md.

**Date:** 2026-06-07
**Phase:** 03-l1-bitpack-for-and-arrow-builders
**Areas discussed:** Bitpack fidelity, vortex_reader→LayoutNode boundary, DuckDB wiring timing, LayoutNode enum scope (all four selected)

---

## Bitpack fidelity

| Option | Selected |
|--------|----------|
| Real FastLanes 1024-lane transposed layout, no patches (fixtures fit bit width) | ✓ |
| Full FastLanes incl. exception patches | |
| Scalar/contiguous (Loom-native) bitpacking | |

**Choice:** Real FastLanes layout, exception/patch path deferred. Faithful to a real Vortex BitPackedArray while bounding Phase-3 scope.

## vortex_reader → LayoutNode boundary

| Option | Selected |
|--------|----------|
| vortex_reader derives LayoutNode from the Vortex ArrayRef; loom-core decodes from it (no vortex dep) | ✓ |
| Fixtures hand-author LayoutNodes; Vortex = byte source + oracle only | |

**Choice:** vortex_reader derives it — truest to INPUT-01 and the design's "identify layout" role; Vortex stays isolated (D-02).

## DuckDB wiring timing

| Option | Selected |
|--------|----------|
| Stay loom-core + FFI-export only; loom_scan keeps hardcoded; DuckDB-real-data + arrow_scan revisit → Phase 5 | ✓ |
| Rewire loom_decode now (forces arrow_scan revisit in Phase 3) | |

**Choice:** Stay loom-core + FFI. Matches the Rust+to_ffi success criteria; keeps the deferred arrow_scan decision scheduled for Phase 5.

## LayoutNode enum scope

| Option | Selected |
|--------|----------|
| Full enum now (Raw/BitPack/FOR/Dictionary/RunEnd/KernelEscape); implement BitPack/FOR/Raw; others stubbed with explicit error | ✓ |
| Only Raw/BitPack/FOR now; grow later | |

**Choice:** Full enum now, unimplemented arms return a typed "unimplemented in Phase 3" error.

---

## Claude's Discretion
LayoutNode field shapes; FOR-over-BitPack nesting; validity→Arrow bitmap mapping (plain bitmap this phase); in-phase verification (Rust unit asserts ± in-test Vortex oracle); demonstrated bit width(s); seeding arrow_builder_output from the existing Int32Builder pattern.

## Deferred Ideas
Bitpack exception/patch path → Phase 4/5; arrow_scan/record-batch + DuckDB real data → Phase 5; encoded/recursive validity → later; Dictionary/RunEnd/KernelEscape decode → Phase 4–5.
