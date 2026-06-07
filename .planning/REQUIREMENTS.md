# Requirements: Loom MVP0 (DuckDB demo)

**Defined:** 2026-06-07
**Core Value:** A user can run a SQL query in DuckDB over a Vortex-encoded column decoded by the Loom interpreter, and get results that match Vortex's own decoder row-for-row.

## v1 Requirements

Requirements for MVP0. Each maps to a roadmap phase.

### Core — Decoder Build & FFI Soundness

- [x] **CORE-01**: The decoder builds as a Rust `staticlib` with all `arrow-*` sub-crates resolved to a single version (`cargo tree -d` shows zero arrow duplicates)
- [x] **CORE-02**: The release profile enforces `panic = "unwind"` (revised from `abort` per 01-REVIEW.md CR-01 — so the boundary `catch_unwind` can actually catch panics and convert them to error codes per DUCK-04 rather than aborting the host) and a `System` global allocator, keeping the FFI boundary sound against unwind-across-FFI UB and allocator clashes
- [x] **CORE-03**: `cbindgen` generates the C header (`loom.h`) from the `extern "C"` surface during the build

### Input — Vortex Source

- [ ] **INPUT-01**: A single serialized Vortex encoded array/column is read into the decoder without parsing a `.vortex` file container
- [ ] **INPUT-02**: Test fixtures are constructed programmatically as in-memory Vortex arrays (no `.vortex` files, no `vortex-file`/`vortex-serde`/`vortex-ipc`)

### L1 — Declarative Layout Layer

- [ ] **L1-01**: A `LayoutNode` data model represents a column's physical layout as pure data (no code)
- [ ] **L1-02**: A synthesized read loop interprets a `LayoutNode` tree to produce decoded values
- [ ] **L1-03**: Decode a bit-packed integer column, including non-byte-aligned widths (1–64 bits)
- [ ] **L1-04**: Decode a frame-of-reference (FOR) column layered on bit-packing
- [ ] **L1-05**: Decode a dictionary-encoded column via codes→values lookup with recursive sub-array dispatch
- [ ] **L1-06**: Decode a run-length-encoded (RLE) column via run-end expansion
- [ ] **L1-07**: Null/validity is preserved through every L1 decode path

### L2 — Total-Function Kernel Layer

- [ ] **L2-01**: A `LayoutNode` can escape to an L2 kernel by id (`KernelEscape`), dispatched through a kernel registry
- [ ] **L2-02**: An FSST L2 kernel decompresses FSST-encoded strings (symbol table + code stream) into string values
- [ ] **L2-03**: A dictionary whose values are FSST-encoded decodes end-to-end (dict-over-FSST exercises the L1→L2 boundary)

### Arrow — Output Contract

- [ ] **ARROW-01**: Decoded values are emitted only through typed Arrow builders (`append_value`/`append_null`/list/struct), never raw writes
- [ ] **ARROW-02**: Output materializes as Arrow `ArrayData` → `ArrowArray` + `ArrowSchema`
- [x] **ARROW-03**: The Arrow array is exported across FFI via the Arrow C Data Interface (`to_ffi` + `ptr::write`) with correct release-callback ownership

### DuckDB — Engine Integration

- [x] **DUCK-01**: A C++ DuckDB extension pinned to DuckDB v1.5.3 builds and loads
- [x] **DUCK-02**: A `loom_scan` table function invokes the Rust decoder and adopts the imported Arrow array zero-copy
- [x] **DUCK-03**: The extension releases the imported Arrow array on every teardown path (no leak, no double-free)
- [x] **DUCK-04**: Every `extern "C"` entry point is wrapped in `catch_unwind` so a decoder panic cannot abort the DuckDB process

### Verify — Verification & Acceptance

- [ ] **VERIFY-01**: An independent Vortex reference decoder produces oracle output for each fixture
- [ ] **VERIFY-02**: Loom-decoded Arrow matches the Vortex reference row-for-row (values + nulls) for every L1 encoding and for FSST
- [ ] **VERIFY-03**: A SQL `SELECT`/aggregate in DuckDB over a Loom-decoded Vortex column returns results matching the reference — the MVP0 acceptance gate

## v2 Requirements

Deferred to a future milestone. Tracked, not in this roadmap.

### Developer Experience

- **DX-01**: A human-readable L1 layout descriptor format (TOML/S-expr) for reviewer exposition
- **DX-02**: Multiple sample columns (one per encoding) in the verification harness
- **DX-03**: A CLI driver (`loom decode <input> <column>`) for non-Rust reviewers
- **DX-04**: Wall-clock timing comparison (Loom interpreter vs Vortex native decode)

### Decode Coverage

- **COV-01**: Additional L2 kernels (ALP float decode, delta-of-delta)
- **COV-02**: Multi-column table function and Arrow schema assembly across columns

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| MLIR `decode` dialect / native codegen | MVP0 interprets directly; native speed is the design's later act (design.md §8) |
| Formal verifier, totality/termination proofs | Acceptance bar is "correct query results"; the verifier is a later milestone (design.md §5, §7, §13) |
| Safety-boundary demo (rejecting out-of-bounds / non-terminating input) | Belongs with the verifier; not part of the decode-chain proof |
| Full `.vortex` file layout (footer / layout tree / multi-chunk) | MVP0 decodes a single column, not a file container (design.md §10) |
| `statistics()` / `projection_mask` / `range` ABI | Single-column decode only; random access + stats come later (design.md §9) |
| Versioned distribution container, feature flags, content-hash URI, native fast-path | Distribution concerns follow the decode chain (design.md §10–11) |
| Correctness guarantees beyond matching the reference decoder | Loom guarantees safety + well-formedness, never correctness (design.md §7) |

## Traceability

Phase mapping finalized by roadmapper 2026-06-07.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CORE-01 | Phase 1 | Complete |
| CORE-02 | Phase 1 | Complete |
| CORE-03 | Phase 1 | Complete |
| ARROW-03 | Phase 1 | Complete |
| DUCK-04 | Phase 1 | Complete |
| DUCK-01 | Phase 2 | Complete |
| DUCK-02 | Phase 2 | Complete |
| DUCK-03 | Phase 2 | Complete |
| INPUT-01 | Phase 3 | Pending |
| INPUT-02 | Phase 3 | Pending |
| L1-01 | Phase 3 | Pending |
| L1-02 | Phase 3 | Pending |
| L1-03 | Phase 3 | Pending |
| L1-04 | Phase 3 | Pending |
| L1-07 | Phase 3 | Pending |
| ARROW-01 | Phase 3 | Pending |
| ARROW-02 | Phase 3 | Pending |
| L1-05 | Phase 4 | Pending |
| L1-06 | Phase 4 | Pending |
| L2-01 | Phase 4 | Pending |
| L2-02 | Phase 5 | Pending |
| L2-03 | Phase 5 | Pending |
| VERIFY-01 | Phase 5 | Pending |
| VERIFY-02 | Phase 5 | Pending |
| VERIFY-03 | Phase 5 | Pending |

**Coverage:**

- v1 requirements: 25 total
- Mapped to phases: 25
- Unmapped: 0 ✓

---
*Requirements defined: 2026-06-07*
*Last updated: 2026-06-07 — traceability finalized by roadmapper*
