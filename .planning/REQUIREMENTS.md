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

- [x] **PROOF-01**: A reviewer-readable safety contract and proof-obligation matrix define the implemented boundary, stable diagnostic/error categories, source evidence, executable evidence, and explicit exclusions
- [x] **PROOF-02**: Focused executable tests prove curated malformed `LMC1`/`LMP1`/`LMT1`/descriptor inputs fail closed through typed errors or verifier diagnostics rather than panicking
- [x] **PROOF-03**: A written safety proof explains no-unsafe-core, FFI panic containment, decode-before-Arrow behavior, and bounded parser/interpreter/kernel loops for the current implementation
- [x] **PROOF-04**: A dedicated `scripts/safety-proof-test.sh` gate checks proof docs, obligation IDs, static safety invariants, focused tests, and existing negative gates, and is invoked by `scripts/mvp0-verify.sh`
- [x] **PROOF-05**: Public and planning docs state the narrow Phase 12 proof scope and do not claim future Loom IR, future L2 language, MLIR/native lowering, real Vortex ingress, signature, attestation, or correctness proofs

### Full Loom Verifier

- [x] **VERIFIER-01**: A normative Phase 13 verifier/spec document defines the tiny `L2Core` subset, artifact assumptions, and safety theorem target
- [x] **VERIFIER-02**: L1 declarative layout semantics are specified as finite, pure data descriptions that compose with `L2Core`
- [x] **VERIFIER-03**: `L2Core` syntax, static semantics, dynamic semantics, and allowed loop forms are defined
- [x] **VERIFIER-04**: The capability/resource model covers input ranges, scratch bounds, output builders, no ambient authority, and fail-closed errors
- [x] **VERIFIER-05**: Arrow builder event semantics are specified so output well-formedness can be checked or proved by construction
- [x] **VERIFIER-06**: A Rust verifier prototype or architecture uses type/effect checking plus abstract interpretation for `L2Core`
- [x] **VERIFIER-07**: Local arithmetic, range, overflow, loop-variant, and resource-bound obligations are represented as SMT-ready constraints
- [x] **VERIFIER-08**: Verifier diagnostics and proof-obligation traces are stable enough for reviewer-facing rejection reports
- [x] **VERIFIER-09**: A Lean or Rocq proof scaffold defines core semantics and states or proves an accepted-program safety theorem
- [x] **VERIFIER-10**: Phase 13 emits verifier facts/proof obligations that Phase 14 can use as native-lowering preconditions

### MLIR/Native Lowering Spike

- [x] **LOWER-01**: A lowering contract and Rust support predicate require an accepted `verify_l2_core` report plus present `VerifiedArtifactFacts`, and unsupported shapes fail closed with stable diagnostics before artifact emission
- [x] **LOWER-02**: The supported bounded Int32 copy `L2Core` slice emits deterministic textual MLIR using standard `func`, `arith`, `scf`, and `memref` dialect operations without mandatory `melior`, LLVM, or Cranelift dependencies
- [x] **LOWER-03**: Focused tests compare the supported slice against typed primitive reference output and cover negative range/capacity and unsupported-shape rejection cases
- [x] **LOWER-04**: A `scripts/native-lowering-test.sh` gate runs focused native-lowering tests and treats `mlir-opt`/native toolchain validation as explicit optional evidence when unavailable
- [x] **LOWER-05**: Public and planning docs state the narrow Phase 14 spike scope and do not claim production native compiler completion, custom Loom dialect completion, vectorization, mandatory JIT, or compiler correctness proof

### Real Vortex File/Container Ingress

- [x] **INGEST-01**: Real Vortex file APIs are isolated to a dedicated ingress boundary; `loom-core` and `loom-ffi` remain free of `vortex-*` dependencies, and scoped dependency/API guards enforce the allowlist
- [x] **INGEST-02**: A stable Loom-owned `VortexIngressReport` / `VortexFileFacts` model records file facts, supported/unsupported status, and stable diagnostics without exposing Vortex types
- [x] **INGEST-03**: Real Vortex buffers and local paths can be inspected, while malformed/truncated/unsupported inputs fail closed with diagnostics rather than panics or partial output
- [x] **INGEST-04**: At least one generated real `.vortex` fixture emits an existing `LMC1` payload, passes the existing verifier/decode path, and matches Vortex oracle rows
- [x] **INGEST-05**: CLI, documentation, and release gates expose the narrow real-ingress behavior without claiming arbitrary Vortex layout support, remote/object-store ingress, native lowering, or production speed

### Full Arrow Semantic Source Compatibility

- [x] **PHASE-31**: Any Lance or Parquet source that the upstream reader can materialize as Arrow, and any Vortex source/dtype that Vortex can materialize as Arrow, can be encoded into a verifier-backed Loom Arrow semantic artifact, decoded back to Arrow, and compared for schema/value/null/metadata equality without relying on narrow `LMP1`/`LMT1` raw-layout coverage

### LMC2 Arrow Semantic Container Wrapper

- [x] **PHASE-33**: The default Arrow semantic source-distribution artifact is a verifier-accepted `LMC2(LMA1)` wrapper with version/features/section checks, artifact-verifier routing and CLI facts, source-ingress emission cutover, focused and broad release-gate coverage, and explicit non-claims for broad DuckDB SQL and native Arrow semantic execution

### DuckDB Arrow Semantic SQL Surface for LMC2(LMA1)

- [x] **PHASE-34**: DuckDB `loom_scan(path)` accepts default verifier-backed `LMC2(LMA1)` artifacts directly, preserves Arrow field names, supports one-batch multi-column primitive/Utf8/Boolean nullable SQL with projection/filter/aggregate/null evidence, keeps direct `LMA1` as regression-only bridge input, rejects unsupported logical/nested SQL shapes with stable diagnostics, and does not claim native Arrow semantic execution

### Native Arrow Semantic Execution

- [x] **PHASE-35**: Verifier-accepted default `LMC2(LMA1)` and explicit direct `LMA1` Arrow semantic artifacts can execute through an engine-neutral native backend for one-batch nullable fixed-width primitive Boolean/Int32/Int64/Float32/Float64 columns, producing a new Arrow `RecordBatch` with explicit native/reference equivalence, runtime/cache identity, fail-closed unsupported Utf8/logical/nested/multi-batch diagnostics, focused/broad gate coverage, and no claim that DuckDB consumes the native route

### Verified Lineage Contract

- [x] **LINEAGE-01**: The MVP1.5 verified-lineage contract defines "verified" as safety + Arrow well-formedness evidence lineage only, keeps source correctness/performance/production readiness as non-claims, and maps each in-scope safety claim to exactly one backing evidence layer: Rust verifier structural check, Bitwuzla SMT discharge, Lean soundness theorem, differential validation, or explicit TCB trust assumption
- [x] **LINEAGE-02**: The MVP1.5 verified-lineage contract declares the TCB for Rust compiler/std, LLVM + MLIR toolchain, Rust↔C ABI, DuckDB host process, and Arrow C Data Interface, and assigns the Lean↔Rust verifier, static↔dynamic, modeled-executor↔real-executor, and native↔model seams to later MVP1.5 phases or to the TCB

### Lean Rust Verifier Correspondence

- [x] **LINEAGE-03**: The Lean checker mirrors the executable Rust verifier's current static L2Core surface for `ScalarExpr` / `LetScalar`, scalar type environment threading, expression-derived append value typing, and `UnknownVariable` rejection without expanding Rust L2Core beyond what the verifier already accepts
- [x] **LINEAGE-04**: A deterministic Lean/Rust verifier differential harness runs the current full verifier fixture matrix plus bounded generated cases, compares accept/reject and stable reject classification, covers required reject codes, and is wired into the release gate as a fail-closed correspondence check

### Lean Modeled Operational Semantics

- [x] **LINEAGE-05**: Lean defines a proof-friendly modeled operational semantics for the current L2Core checker slice, including abstract input assumptions, typed builder events, bounded loop/cursor execution, fail-closed terminal behavior, and modeled safety predicates for safe reads, well-typed events, maxRows termination, and Arrow well-formedness by construction
- [x] **LINEAGE-06**: Lean proves `accepted_program_safe` as a no-`sorry` semantic theorem over the modeled executor, with a program-level induction bridge (`verified_program_finishes` / `verified_program_reads_in_bounds`) showing `Verified p` implies `execProgram p` finishes and every recorded read is in bounds; out-of-bounds reads are representable as `inBounds := false` and fail close unverified modeled runs; `checked_readInput_concrete_in_range` connects static `ReadInput` authority acceptance to the modeled executor's concrete read-range predicate; and the theorem remains explicitly modeled-only, with Rust interpreter consistency delegated to Phase 39 and native/model validation delegated to Phase 40

### Model Rust Interpreter Consistency

- [x] **LINEAGE-07**: A Rust reference executor transcribes the Phase 38 Lean modeled operational semantics as a separate differential oracle, emits stable read/append/fail-closed/terminal trace records, and is not used as a production fallback or behavior fixup
- [x] **LINEAGE-08**: A deterministic trace-level differential gate compares the production Rust interpreter surface against the reference executor over the supported matrix plus generated cases, fails closed on builder-event/fail-closed divergence, and records that this is per-run validation rather than all-program proof

### Native Model Validation

- [x] **LINEAGE-09**: Native Arrow semantic output for every Phase 35 supported shape (`LMC2(LMA1)` and explicit direct `LMA1`, nullable Boolean/Int32/Int64/Float32/Float64 one-batch primitives) is validated against a Phase 39 reference-executor builder-event trace, with injected native/model trace divergence producing stable fail-closed diagnostics
- [x] **LINEAGE-10**: Runtime route/cache eligibility requires successful native/model validation, divergent or unsupported validation cannot seed native cache identity, and Phase 40 records MLIR/LLVM/native lowering as permanent TCB per-run translation validation rather than verified compilation

### Verified Lineage Closeout

- [x] **LINEAGE-11**: A single `scripts/verified-lineage-test.sh` closeout gate runs the full MVP1.5 evidence matrix fail-closed: Lean build with zero `sorry`, Lean/Rust verifier differential, model/Rust interpreter trace consistency, native/model validation, and stable non-claim/TCB markers
- [x] **LINEAGE-12**: Accepted artifacts can produce a verified-lineage record naming structural verifier evidence, Bitwuzla/solver discharge status, Lean modeled soundness evidence, differential-validation gates, and explicit TCB assumptions, while rejected/unsupported artifacts cannot produce a positive lineage claim

### Verified + Native Coverage Expansion

- [x] **COV2-01**: Vortex coverage rows are widened into a Phase 42 living matrix with explicit source shape, emitted Loom shape, oracle/verifier/verified-lineage evidence, native disposition, interpreter fallback, and fail-closed/deferred rows; canonical raw Vortex rows do not claim native support for original dictionary/RLE/bitpack/FOR encodings
- [x] **COV2-02**: Lance and Parquet schema rows record accepted `LMC2(LMA1)` source semantic coverage, native-supported fixed-width primitive subsets, interpreter-only Utf8/nested rows, and fail-closed/deferred unsupported shapes
- [x] **COV2-03**: A focused Phase 42 gate validates the living matrix and is wired into the broad verifier before closeout

### StarRocks Live Runtime Integration

- [ ] **ENGINE-01**: StarRocks runtime evidence is collected only from a live runtime query over a Loom-bound/accepted artifact identity, and accepted rows/scalars match the oracle and DuckDB matrix for the supported query set
- [ ] **ENGINE-02**: Phase 43 records every observed DuckDB-shaped ABI assumption exposed by the StarRocks consumer and classifies each as fixed-now, accepted asymmetry, or Phase 44 freeze input
- [ ] **ENGINE-03**: Unsupported StarRocks runtime shapes, descriptor drift, missing runtime configuration, and output mismatches fail closed with typed diagnostics and cannot produce accepted runtime evidence

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| MLIR `decode` dialect / native codegen | MVP0 interprets directly; native speed is the design's later act (design.md §8) |
| MLIR/native lowering correctness proof and real Vortex file ingress proof | Phase 14 completes only a verifier-gated textual MLIR spike; production compiler lowering/proof and real file-ingress proofs remain later phases (design.md §5, §7, §13) |
| Non-terminating-input safety demo for future user-defined languages or native lowering | Phase 12 covers bounded loops in the current parser/interpreter/kernel implementation only |
| Full arbitrary `.vortex` file layout support (all layouts / multi-chunk / object-store / encrypted or compressed variants) | Phase 15 is planned as one narrow real-ingress slice, not a complete Vortex reader |
| `statistics()` / `projection_mask` / `range` ABI | Single-column decode only; random access + stats come later (design.md §9) |
| Content-hash URI, signatures, attestation, encryption, remote fetch, and native fast-path | Phase 11 completed only the local `LMC1` container boundary; remote trust/distribution features remain later work (design.md §10–11) |
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
| PROOF-01 | Phase 12 | Complete |
| PROOF-02 | Phase 12 | Complete |
| PROOF-03 | Phase 12 | Complete |
| PROOF-04 | Phase 12 | Complete |
| PROOF-05 | Phase 12 | Complete |
| VERIFIER-01 | Phase 13 | Complete |
| VERIFIER-02 | Phase 13 | Complete |
| VERIFIER-03 | Phase 13 | Complete |
| VERIFIER-04 | Phase 13 | Complete |
| VERIFIER-05 | Phase 13 | Complete |
| VERIFIER-06 | Phase 13 | Complete |
| VERIFIER-07 | Phase 13 | Complete |
| VERIFIER-08 | Phase 13 | Complete |
| VERIFIER-09 | Phase 13 | Complete |
| VERIFIER-10 | Phase 13 | Complete |
| LOWER-01 | Phase 14 | Complete |
| LOWER-02 | Phase 14 | Complete |
| LOWER-03 | Phase 14 | Complete |
| LOWER-04 | Phase 14 | Complete |
| LOWER-05 | Phase 14 | Complete |
| INGEST-01 | Phase 15 | Complete |
| INGEST-02 | Phase 15 | Complete |
| INGEST-03 | Phase 15 | Complete |
| INGEST-04 | Phase 15 | Complete |
| INGEST-05 | Phase 15 | Complete |
| PHASE-31 | Phase 31 | Complete |
| PHASE-33 | Phase 33 | Complete |
| PHASE-34 | Phase 34 | Complete |
| PHASE-35 | Phase 35 | Complete |
| LINEAGE-01 | Phase 36 | Complete |
| LINEAGE-02 | Phase 36 | Complete |
| LINEAGE-03 | Phase 37 | Complete |
| LINEAGE-04 | Phase 37 | Complete |
| LINEAGE-05 | Phase 38 | Complete |
| LINEAGE-06 | Phase 38 | Complete |
| LINEAGE-07 | Phase 39 | Complete |
| LINEAGE-08 | Phase 39 | Complete |

**Coverage:**

- v1 requirements: 25 total
- v2 foundation requirements: 5 total
- v2 developer-experience requirements: 4 total
- v2 decode-coverage requirements: 2 total
- v2 table-output requirements: 6 total
- v2 safety-boundary requirements: 5 total
- v3 distribution-container requirements: 5 total
- v3 formal-safety-proof requirements: 5 total
- v3 full-loom-verifier requirements: 10 total
- v3 mlir-native-lowering-spike requirements: 5 total
- v3 real-vortex-ingress requirements: 5 total
- v3 full-arrow-semantic-source-compatibility requirements: 1 total
- v3 lmc2-arrow-semantic-container-wrapper requirements: 1 total
- v3 duckdb-arrow-semantic-sql-surface requirements: 1 total
- v3 native-arrow-semantic-execution requirements: 1 total
- v3 verified-lineage requirements: 2 total
- Mapped to phases: 83
- Unmapped: 0 ✓

---
*Requirements defined: 2026-06-07*
*Last updated: 2026-06-09 — Phase 36 Verified-Lineage Contract complete*
