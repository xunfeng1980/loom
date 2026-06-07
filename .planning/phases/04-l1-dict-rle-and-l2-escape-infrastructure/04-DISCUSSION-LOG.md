# Phase 4: L1 Dict, RLE, and L2 Escape Infrastructure - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-07
**Phase:** 4-l1-dict-rle-and-l2-escape-infrastructure
**Areas discussed:** L2 escape contract, Builder type expansion, Verification rigor, Recursion & CR-02

---

## L2 Escape Contract

### Kernel output contract

| Option | Description | Selected |
|--------|-------------|----------|
| Return Arrow ArrayData | Kernel is a self-contained total function producing its own Arrow array; read loop adopts it. Cleanest L1/L2 boundary; decouples kernel output type from Int-only OutputBuilder. | ✓ |
| Write into OutputBuilder | Kernel gets &mut OutputBuilder and appends; forces a String variant and couples output type. | |
| Return raw values | Kernel returns Vec<Option<Vec<u8>>>; read loop materializes Arrow. | |

### Stub output

| Option | Description | Selected |
|--------|-------------|----------|
| Empty StringArray | Zero-length Utf8 array; stub type already matches Phase 5 FSST output. | ✓ |
| Empty Int32Array | Zero-length array of existing supported type; type won't match eventual FSST output. | |

### Registry shape & lookup

| Option | Description | Selected |
|--------|-------------|----------|
| Vec, typed error on miss | Vec<Box<dyn L2Kernel>>, FSST at index 0, default_for_mvp0(), get(id)→Option, typed LoomDecodeError on miss. | ✓ |
| HashMap, typed error on miss | HashMap<u32,…> keyed by kernel_id; more flexible for sparse ids, more ceremony. | |

**User's choice:** ArrayData-returning kernel; empty StringArray stub; Vec registry with typed error.
**Notes:** Locks the Phase-5 FSST plug-in shape. Surfaced wrinkle: a top-level KernelEscape returns its own array rather than appending into OutputBuilder — read-loop output shape left to planning (CONTEXT.md Claude's Discretion).

---

## Builder Type Expansion

### Boolean output

| Option | Description | Selected |
|--------|-------------|----------|
| Add Boolean variant | OutputBuilder::Boolean(BooleanBuilder) with append_bool/append_null; required by RLE-boolean criterion. | ✓ |
| Kernel-style ArrayData | Decode RLE-boolean to its own BooleanArray outside OutputBuilder. | |

### String/Utf8 variant

| Option | Description | Selected |
|--------|-------------|----------|
| No — defer to Phase 5 | Phase-4 dict is integer-only; FSST kernel owns its StringBuilder; add string support with dict-over-FSST. | ✓ |
| Add String now | Add Utf8 proactively for symmetry; unused in Phase 4. | |

**User's choice:** Add Boolean; defer String.
**Notes:** Consistent with the Area-1 decision that the kernel owns its string output.

---

## Verification Rigor

### Dict/RLE verification

| Option | Description | Selected |
|--------|-------------|----------|
| Vortex oracle, row-for-row | Reuse Phase-3 harness; compare loom-core output to into_canonical().into_arrow() element-by-element; hand-written fallback only where Vortex 0.74 can't build a fixture. | ✓ |
| Hand-written expected arrays | Unit tests with hardcoded expected output; simpler, lower fidelity. | |
| Both | Hand-written + oracle; most thorough, most fixture work. | |

### KernelEscape (L2-01) verification

| Option | Description | Selected |
|--------|-------------|----------|
| Routing-only unit tests | Two tests: route to registry returns empty StringArray no panic; unknown id returns typed error no panic. | ✓ |
| Defer assertion to Phase 5 | Only assert 'does not panic' now; full output correctness later. | |

**User's choice:** Vortex oracle row-for-row for dict/RLE; routing-only unit tests for KernelEscape.
**Notes:** Researcher must confirm Vortex 0.74 can construct dict + RunEnd fixtures; RLE falls back to hand-written if not.

---

## Recursion & CR-02

### Fixture sub-encodings

| Option | Description | Selected |
|--------|-------------|----------|
| Realistic Vortex layout | Build fixtures as Vortex naturally encodes (dict codes=BitPack/values=Raw, etc.); exercises recursion through proven arms. | ✓ |
| Minimal (Raw sub-arrays) | Force Raw sub-arrays; isolates lookup/expansion, near-trivial recursion. | |
| Deep nesting stress | Nest FOR/extra layers to stress recursion and force CR-02 path; max coverage, more effort. | |

### CR-02 handling

| Option | Description | Selected |
|--------|-------------|----------|
| Fix now + oracle test | Apply reference after inner decode for non-BitPack path; add FOR-over-Raw oracle test; folds todo. | ✓ |
| Guard with typed error | Return typed error for FOR-over-non-BitPack; safe minimum, defers real fix. | |
| Defer | Leave as-is if no fixture nests FOR over non-BitPack. | |

### Validity layering

| Option | Description | Selected |
|--------|-------------|----------|
| Delegate to child (Pitfall 3) | Read validity from the sub-array Vortex carries it on; consistent with Phase-3 FOR→BitPack. | ✓ |
| Top-level on the node | Validity bitmap directly on Dictionary/RunEnd node. | |

**User's choice:** Realistic Vortex fixtures; fix CR-02 now + oracle test; validity delegates to child.
**Notes:** CR-02 todo folded into scope (D-09) — recursive dispatch is the first place FOR-over-non-BitPack becomes reachable.

---

## Claude's Discretion

- RLE run-end expansion algorithm (linear scan vs binary search).
- Read-loop output shape for a top-level KernelEscape (kernel-array vs builder-backed).
- How dict/RLE materialize decoded sub-arrays before lookup/expansion.
- Exact integer widths and run/dict cardinalities in fixtures.

## Deferred Ideas

- Real FSST decompression — Phase 5 (L2-02).
- dict-over-FSST end-to-end + string-valued dict output / OutputBuilder string support — Phase 5 (L2-03).
- Full verification harness + DuckDB SQL over real data (arrow_scan/record-batch rewire) — Phase 5.
- Bitpack exception/patch path — still deferred (fixtures stay in-width).
- Encoded/recursive validity — still assuming plain validity bitmaps.
