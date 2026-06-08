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

- [x] **INPUT-01**: A single serialized Vortex encoded array/column is read into the decoder without parsing a `.vortex` file container
- [x] **INPUT-02**: Test fixtures are constructed programmatically as in-memory Vortex arrays (no `.vortex` files, no `vortex-file`/`vortex-serde`/`vortex-ipc`)

### L1 — Declarative Layout Layer

- [x] **L1-01**: A `LayoutNode` data model represents a column's physical layout as pure data (no code)
- [x] **L1-02**: A synthesized read loop interprets a `LayoutNode` tree to produce decoded values
- [x] **L1-03**: Decode a bit-packed integer column, including non-byte-aligned widths (1–64 bits)
- [x] **L1-04**: Decode a frame-of-reference (FOR) column layered on bit-packing
- [x] **L1-05**: Decode a dictionary-encoded column via codes→values lookup with recursive sub-array dispatch
- [x] **L1-06**: Decode a run-length-encoded (RLE) column via run-end expansion
- [x] **L1-07**: Null/validity is preserved through every L1 decode path

### L2 — Total-Function Kernel Layer

- [x] **L2-01**: A `LayoutNode` can escape to an L2 kernel by id (`KernelEscape`), dispatched through a kernel registry
- [x] **L2-02**: An FSST L2 kernel decompresses FSST-encoded strings (symbol table + code stream) into string values
- [x] **L2-03**: A dictionary whose values are FSST-encoded decodes end-to-end (dict-over-FSST exercises the L1→L2 boundary)

### Arrow — Output Contract

- [x] **ARROW-01**: Decoded values are emitted only through typed Arrow builders (`append_value`/`append_null`/list/struct), never raw writes
- [x] **ARROW-02**: Output materializes as Arrow `ArrayData` → `ArrowArray` + `ArrowSchema`
- [x] **ARROW-03**: The Arrow array is exported across FFI via the Arrow C Data Interface (`to_ffi` + `ptr::write`) with correct release-callback ownership

### DuckDB — Engine Integration

- [x] **DUCK-01**: A C++ DuckDB extension pinned to DuckDB v1.5.3 builds and loads
- [x] **DUCK-02**: A `loom_scan` table function invokes the Rust decoder and adopts the imported Arrow array zero-copy
- [x] **DUCK-03**: The extension releases the imported Arrow array on every teardown path (no leak, no double-free)
- [x] **DUCK-04**: Every `extern "C"` entry point is wrapped in `catch_unwind` so a decoder panic cannot abort the DuckDB process

### Verify — Verification & Acceptance

- [x] **VERIFY-01**: An independent Vortex reference decoder produces oracle output for each fixture
- [x] **VERIFY-02**: Loom-decoded Arrow matches the Vortex reference row-for-row (values + nulls) for every L1 encoding and for FSST
- [x] **VERIFY-03**: A SQL `SELECT`/aggregate in DuckDB over a Loom-decoded Vortex column returns results matching the reference — the MVP0 acceptance gate

## v2 Requirements

Tracked for post-MVP0 work. Phase 6 starts the v2 foundation by hardening the completed MVP0 baseline before descriptor/CLI or multi-column work begins.

### Baseline Hardening

- [x] **BASE-01**: Planning state and project documentation consistently mark MVP0 complete and Phase 6 active, with stale blockers either removed or marked resolved
- [x] **DOC-01**: README documents the implemented MVP0 surface, exact verification commands, and the distinction between current prototype and full Loom distribution IR design
- [x] **DOC-02**: Vortex / AnyBlox / F3 positioning is linked from public docs and kept as a concrete reference for future v2 planning
- [x] **VERIFY-04**: A single release-gate script runs the full MVP0 verification suite from the repository root
- [x] **BUILD-01**: The release gate rebuilds Rust and DuckDB extension artifacts in a way that prevents stale `libloom_ffi.a` or extension binaries from masking regressions

### Developer Experience

- [x] **DX-01**: A human-readable L1 layout descriptor format for reviewer exposition, with deterministic parse/print roundtrips for all MVP0 layout nodes
- [x] **DX-02**: Multiple sample columns per supported encoding in the verification harness, including descriptor roundtrip coverage
- [x] **DX-03**: A CLI driver (`loom inspect`, `loom decode`) for non-Rust reviewers
- [x] **DX-04**: Wall-clock timing comparison (Loom interpreter vs Vortex native decode) reported as illustrative output, not a benchmark claim

### Decode Coverage

- [x] **COV-01**: Additional L2 kernel coverage with ALP-style Float32/Float64 decode, stable params, verifier checks, fixture oracle comparisons, FFI roundtrips, CLI output, and DuckDB SQL acceptance
- [x] **COV-02**: Multi-column table function and Arrow schema assembly across columns

### Table Output

- [x] **TABLE-01**: A table-shaped description model represents multiple named columns with per-column `LayoutDescription`, dtype, row count, and nullability metadata
- [x] **TABLE-02**: A checked table payload format encodes and decodes mixed MVP0 column types while preserving compatibility with existing single-column payloads
- [x] **TABLE-03**: Rust-side multi-column decode returns typed column arrays with a shared row count and typed errors for schema or length mismatch
- [x] **DUCK-05**: DuckDB `loom_scan` binds and scans multiple output columns from one payload, supporting projection, filters, and aggregates over mixed Int32/Boolean/Utf8 columns
- [x] **STREAM-01**: ArrowArrayStream is either implemented for table-shaped output or explicitly deferred with repo-specific API evidence and rationale
- [x] **VERIFY-05**: Multi-column SQL acceptance checks pass and the existing `scripts/mvp0-verify.sh` release gate remains green

### Safety Boundary

- [x] **SAFE-01**: A verifier module walks MVP0 layout and table descriptions before decode and returns typed diagnostics rather than panicking
- [x] **SAFE-02**: Verifier coverage rejects malformed structural invariants, including truncated buffers, invalid row/count relationships, non-monotonic run ends, unsupported type/layout combinations, unknown kernels, and table column mismatches
- [x] **SAFE-03**: Decode entry points either call the verifier or explicitly route through an existing authoritative decode-time check for each invariant
- [x] **SAFE-04**: `loom inspect` exposes verifier pass/fail status for binary payloads and descriptors
- [x] **VERIFY-06**: Negative verifier fixtures are included in the release gate and prove malformed inputs fail closed before DuckDB execution

## v3 Requirements

Tracked for work that moves Loom from a runnable MVP0/v2 prototype toward the final distribution-IR goal.

### Distribution Container

- [x] **DIST-01**: A versioned Loom distribution container (`LMC1`) wraps existing single-column `LMP1` and table `LMT1` payloads without breaking raw payload compatibility
- [x] **DIST-02**: The container records required and optional feature flags, and unknown required features fail closed before decode
- [x] **DIST-03**: A checked section directory records section kind, flags, offset, and length with overflow/truncation rejection
- [x] **DIST-04**: `loom inspect` exposes container version, features, sections, payload kind, schema summary, and verifier status
- [x] **DIST-05**: The release gate covers container-wrapped payload success and negative container rejection cases

### Formal Safety Proof

- [ ] **PROOF-01**: A reviewer-readable safety contract and proof-obligation matrix define the implemented boundary, stable diagnostic/error categories, source evidence, executable evidence, and explicit exclusions
- [ ] **PROOF-02**: Focused executable tests prove curated malformed `LMC1`/`LMP1`/`LMT1`/descriptor inputs fail closed through typed errors or verifier diagnostics rather than panicking
- [ ] **PROOF-03**: A written safety proof explains no-unsafe-core, FFI panic containment, decode-before-Arrow behavior, and bounded parser/interpreter/kernel loops for the current implementation
- [ ] **PROOF-04**: A dedicated `scripts/safety-proof-test.sh` gate checks proof docs, obligation IDs, static safety invariants, focused tests, and existing negative gates, and is invoked by `scripts/mvp0-verify.sh`
- [ ] **PROOF-05**: Public and planning docs state the narrow Phase 12 proof scope and do not claim future Loom IR, future L2 language, MLIR/native lowering, real Vortex ingress, signature, attestation, or correctness proofs

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| MLIR `decode` dialect / native codegen | MVP0 interprets directly; native speed is the design's later act (design.md §8) |
| Full formal proof of future Loom IR, future L2 total-function language, MLIR/native lowering, or real Vortex file ingress | Phase 12 targets only the current implemented byte-to-Arrow safety boundary; future compiler/file-ingress proofs remain later work (design.md §5, §7, §13) |
| Non-terminating-input safety demo for future user-defined languages or native lowering | Phase 12 covers bounded loops in the current parser/interpreter/kernel implementation only |
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
| INPUT-01 | Phase 3 | Complete |
| INPUT-02 | Phase 3 | Complete |
| L1-01 | Phase 3 | Complete |
| L1-02 | Phase 3 | Complete |
| L1-03 | Phase 3 | Complete |
| L1-04 | Phase 3 | Complete |
| L1-07 | Phase 3 | Complete |
| ARROW-01 | Phase 3 | Complete |
| ARROW-02 | Phase 3 | Complete |
| L1-05 | Phase 4 | Complete |
| L1-06 | Phase 4 | Complete |
| L2-01 | Phase 4 | Complete |
| L2-02 | Phase 5 | Complete |
| L2-03 | Phase 5 | Complete |
| VERIFY-01 | Phase 5 | Complete |
| VERIFY-02 | Phase 5 | Complete |
| VERIFY-03 | Phase 5 | Complete |
| BASE-01 | Phase 6 | Complete |
| DOC-01 | Phase 6 | Complete |
| DOC-02 | Phase 6 | Complete |
| VERIFY-04 | Phase 6 | Complete |
| BUILD-01 | Phase 6 | Complete |
| DX-01 | Phase 7 | Complete |
| DX-02 | Phase 7 | Complete |
| DX-03 | Phase 7 | Complete |
| DX-04 | Phase 7 | Complete |
| COV-01 | Phase 10 | Complete |
| COV-02 | Phase 8 | Complete |
| TABLE-01 | Phase 8 | Complete |
| TABLE-02 | Phase 8 | Complete |
| TABLE-03 | Phase 8 | Complete |
| DUCK-05 | Phase 8 | Complete |
| STREAM-01 | Phase 8 | Complete |
| VERIFY-05 | Phase 8 | Complete |
| SAFE-01 | Phase 9 | Complete |
| SAFE-02 | Phase 9 | Complete |
| SAFE-03 | Phase 9 | Complete |
| SAFE-04 | Phase 9 | Complete |
| VERIFY-06 | Phase 9 | Complete |
| DIST-01 | Phase 11 | Complete |
| DIST-02 | Phase 11 | Complete |
| DIST-03 | Phase 11 | Complete |
| DIST-04 | Phase 11 | Complete |
| DIST-05 | Phase 11 | Complete |
| PROOF-01 | Phase 12 | Planned |
| PROOF-02 | Phase 12 | Planned |
| PROOF-03 | Phase 12 | Planned |
| PROOF-04 | Phase 12 | Planned |
| PROOF-05 | Phase 12 | Planned |

**Coverage:**

- v1 requirements: 25 total
- v2 foundation requirements: 5 total
- v2 developer-experience requirements: 4 total
- v2 decode-coverage requirements: 2 total
- v2 table-output requirements: 7 total
- v2 safety-boundary requirements: 5 total
- v3 distribution-container requirements: 5 total
- v3 formal-safety-proof requirements: 5 total
- Mapped to phases: 57
- Unmapped: 0 ✓

---
*Requirements defined: 2026-06-07*
*Last updated: 2026-06-08 — Phase 12 Formal Verifier / Safety Proof MVP planned; Phase 13-14 placeholders remain recorded*
