# Roadmap: Loom MVP1 (post-MVP0 distribution/verification track)

## Overview

The original Loom MVP0 proved one narrow chain end-to-end: Vortex-style encoded payloads are decoded
by a pure-Rust interpreter through L1 declarative encodings and L2 kernels, producing well-formed
Apache Arrow that crosses a C ABI seam into a C++ DuckDB table function and is queried with SQL.
Phases 1-10 complete that MVP0/v2 proof chain. The project is now in MVP1/v3, focused on
distribution containers, verifier-backed safety, native-lowering preparation, and narrow real Vortex
file ingress. Phase 16 completed optional verifier-gated `melior`/LLVM/JIT backend evidence for the bounded Int32 slice. Phase 17 closed
the largest verifier gap by unifying the current payload verifier and future `L2Core` verifier foundation into one artifact verification
pipeline. Phase 18 is next and reserves a complete Vortex reader beyond the narrow Phase 15 ingress slice. Phase 19
is reserved for the solver-backed full artifact verifier that upgrades collected obligations into discharged verifier evidence before
production native expansion. Phase 20 preserves the production MLIR decode dialect/native kernel expansion step. Phases 21-23 split the
formerly oversized engine-integrated native execution placeholder into a runtime ABI/policy phase, a DuckDB host-integration MVP, and an
equivalence/cache/fallback hardening phase. Phase 24 and Phase 25 reserve the table-format and multi-engine query surface that should follow: Iceberg ref/table
binding first, then StarRocks + DuckDB dual query surface.

## Phases

**Phase Numbering:**

- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Scaffold and FFI Boundary** - Establish Rust workspace invariants, Arrow version pinning, panic-safety contract, and cbindgen header generation
- [x] **Phase 2: DuckDB Extension Scaffold** - Build and load a stub DuckDB extension that links the Rust staticlib and proves the CMake + DuckDB ABI version chain (completed 2026-06-07)
- [x] **Phase 3: L1 Bitpack, FOR, and Arrow Builders** - Implement the core decode infrastructure (Arrow typed builders, vortex_reader, LayoutNode model) and the first two L1 decoders with null handling (completed 2026-06-07)
- [x] **Phase 4: L1 Dict, RLE, and L2 Escape Infrastructure** - Complete the remaining L1 decoders and wire the KernelEscape arm + L2KernelRegistry (FSST stub) so the full routing chain exists (completed 2026-06-07)
- [x] **Phase 5: FSST L2 Kernel and Full Verification** - Implement the FSST L2 kernel and run the row-for-row verification harness across all encodings — the MVP0 acceptance gate (completed 2026-06-08)
- [x] **Phase 6: MVP0 Hardening and Release Baseline** - Convert the completed MVP0 into a reproducible, documented baseline with one-command verification, stale planning cleanup, and explicit next-milestone boundaries (completed 2026-06-08)
- [x] **Phase 7: Human-Readable Layout Descriptor and CLI** - Make Loom's layout contract inspectable and decodable outside Rust tests by adding a recursive descriptor format, roundtrip parser/printer, CLI inspect/decode commands, and expanded fixture/timing support (completed 2026-06-08)
- [x] **Phase 8: Multi-Column Table Output and Arrow Stream Evaluation** - Promote the single-column MVP0 payload into table-shaped output with multiple named columns, mixed Arrow types, DuckDB SQL over real multi-column rows, and a documented ArrowArrayStream decision (completed 2026-06-08)
- [x] **Phase 9: Verifier and Safety Boundary MVP** - Add a first-pass verifier for layout/table payloads that rejects malformed or unsafe decode descriptions before execution and exposes a reviewer-visible verification command (completed 2026-06-08)
- [x] **Phase 10: Additional L2 Kernels and Numeric Compression Coverage** - Extend the L2 kernel path beyond FSST with ALP Float32/Float64 coverage for COV-01 (complete)
- [x] **Phase 11: Distribution Container v0** - Introduce a versioned `LMC1` container with feature flags and a section directory around existing `LMP1`/`LMT1` payloads (complete)
- [x] **Phase 12: Formal Verifier / Safety Proof MVP** - Turn the current verifier/container/decode boundary into a documented and executable safety-proof MVP (complete)
- [x] **Phase 13: Full Loom Verifier** - Build the verifier foundation for future Loom distribution IR and L2 total-function language using Rust abstract interpretation, SMT obligations, Lean/Rocq semantics, and TLA+ pipeline invariants (complete)
- [x] **Phase 14: MLIR/Native Lowering Spike** - Prove a verifier-gated textual MLIR/native lowering spike over a tiny `L2Core` slice (complete)
- [x] **Phase 15: Real Vortex File/Container Ingress** - Narrow real Vortex ingress boundary: isolated `vortex-file` use, Loom-owned facts/diagnostics, and one supported `.vortex` -> `LMC1` slice before production native backend work (complete)
- [x] **Phase 16: Full melior/LLVM/JIT Backend Integration** - Optional verifier-gated programmatic MLIR/LLVM/JIT backend evidence over the bounded Int32 copy slice, with skip-aware tooling and no production native-compiler claim (complete)
- [x] **Phase 17: Unified Artifact Verification Pipeline** - Fail-closed artifact verifier pipeline from `LMC1` container/schema/features/kernel manifest through L1 verification, L2Core verification, constraints/facts, and lowering-ready report (complete)
- [ ] **Phase 18: Complete Vortex Reader** - Placeholder for expanding Phase 15's narrow real-ingress slice into a complete, isolated, fail-closed Vortex file/container reader before engine-integrated native execution (not expanded)
- [ ] **Phase 19: Solver-backed Full Artifact Verifier** - Placeholder for real solver discharge over the unified artifact pipeline after complete-reader facts exist and before production native expansion (not expanded)
- [ ] **Phase 20: Production Decode Dialect and Native Kernel Expansion** - Placeholder for a custom Loom MLIR decode dialect, Arrow/raw-buffer builder lowering, vectorization, and native lowering beyond the tiny copy slice (not expanded)
- [ ] **Phase 21: Host Native Runtime ABI and Execution Policy** - Placeholder for the engine-independent ABI, artifact/facts contract, cache key, fail-closed policy, and interpreter fallback semantics that host engines will call (not expanded)
- [ ] **Phase 22: DuckDB Native Execution Integration MVP** - Placeholder for wiring verified native execution into the DuckDB table-function path over complete-reader artifacts with interpreter fallback (not expanded)
- [ ] **Phase 23: Native Equivalence, Cache, and Fallback Hardening** - Placeholder for oracle/equivalence gates, native artifact cache reuse/invalidation, negative coverage, and release-gate hardening before table-format binding (not expanded)
- [ ] **Phase 24: Iceberg Ref/Table Binding** - Placeholder for binding Loom distribution artifacts into Iceberg table/reference metadata after the native execution path and full reader boundary are credible (not expanded)
- [ ] **Phase 25: StarRocks + DuckDB Dual Query Surface** - Placeholder for proving the same Loom/Iceberg-bound artifacts can be queried through both StarRocks and DuckDB surfaces (not expanded)

## Phase Details

### Phase 1: Scaffold and FFI Boundary

**Goal**: The Rust workspace compiles as a sound FFI staticlib with all Arrow sub-crates at a single version, panic-abort enforced, a System allocator installed, and cbindgen producing loom.h
**Depends on**: Nothing (first phase)
**Requirements**: CORE-01, CORE-02, CORE-03, ARROW-03, DUCK-04
**Success Criteria** (what must be TRUE):

  1. `cargo tree -d | grep arrow` returns zero duplicate entries
  2. `grep vortex-file Cargo.lock` returns nothing
  3. `loom.h` is generated by cbindgen and contains the `loom_decode` signature
  4. A stub `extern "C" fn loom_decode(...)` wrapped in `catch_unwind` compiles and links into `libloom_decoder.a` without warnings
  5. `[profile.release] panic = "unwind"` (revised from `abort` per 01-REVIEW.md CR-01, so the boundary `catch_unwind` is live) and `#[global_allocator] static A: System = System;` are present and verified in CI

**Plans**: 2 plans

**Wave 1**

- [x] 01-01-PLAN.md - Cargo workspace, 3-crate boundary, arrow version pinning, panic=abort + System allocator (CORE-01, CORE-02)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 01-02-PLAN.md - extern "C" loom_decode + catch_unwind, real Arrow to_ffi roundtrip, cbindgen loom.h, CORE-invariant CI (CORE-03, ARROW-03, DUCK-04)

### Phase 2: DuckDB Extension Scaffold

**Goal**: A stub DuckDB extension pinned to v1.5.3 builds, loads into DuckDB, and invokes the Rust stub decoder without crashing -- proving the full CMake + Rust staticlib + DuckDB ABI chain
**Depends on**: Phase 1
**Requirements**: DUCK-01, DUCK-02, DUCK-03
**Success Criteria** (what must be TRUE):

  1. `cmake --build` produces `loom_extension.duckdb_extension` without errors
  2. `LOAD 'loom_extension'; SELECT * FROM loom_scan('test.bin');` executes and returns without crashing or failing with an ABI version error
  3. The `LoomScanState` destructor calls `array.release(&array)` on every exit path (verified by manual test outside DuckDB)
  4. `nm -g libloom_decoder.a | grep -i malloc` returns nothing unexpected (no rogue allocator symbols)

**Plans**: 2 plans

**Wave 1**

- [x] 02-01-PLAN.md - Vendor DuckDB v1.5.3 inputs, run Wave-0 checks, author loom_extension.cpp (entry point, loom_scan, OneShotStream factory → arrow_scan, release-safe teardown) (DUCK-01, DUCK-02, DUCK-03)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 02-02-PLAN.md - Hand-rolled CMake (cargo build + link + footer-stamp POST_BUILD), duckdb -unsigned load smoke-test, allocator-symbol guard, CI wiring (DUCK-01, DUCK-03)

### Phase 3: L1 Bitpack, FOR, and Arrow Builders

**Goal**: The synthesized read loop decodes BitPack and FrameOfReference Vortex columns, correctly preserving nulls, and emits well-formed Arrow arrays through typed builders
**Depends on**: Phase 2
**Requirements**: INPUT-01, INPUT-02, L1-01, L1-02, L1-03, L1-04, L1-07, ARROW-01, ARROW-02
**Success Criteria** (what must be TRUE):

  1. A programmatically constructed BitPacked Vortex array (non-byte-aligned width, e.g. 11-bit) decodes to an Arrow Int32Array whose values match the original input row-for-row
  2. A FrameOfReference column layered on bitpacking decodes correctly, with the reference scalar added to every unpacked value
  3. At least one nullable column per encoding (bitpack, FOR) round-trips with nulls intact -- `COUNT(col)` vs `COUNT(*)` in the Arrow output matches expected null count
  4. `arrow_builder_output::finish()` produces `ArrayData` that can be exported via `to_ffi` without compile errors (arrow-rs version conflict would surface here)
  5. No `.vortex` file is read or written; all test inputs are constructed via `vortex-array` builder APIs in Rust

**Plans**: 2 plans

**Wave 1**

- [x] 03-01-PLAN.md — loom-core L1 core: full LayoutNode enum, FastLanes unpack, OutputBuilder, read loop (Raw/BitPack/FOR + validity) (L1-01, L1-02, L1-03, L1-04, L1-07, ARROW-01, ARROW-02)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 03-02-PLAN.md — vortex_reader + oracle + 4 Wave-0 BLOCKING checks + bitpack/FOR/nullable roundtrip vs Vortex oracle (INPUT-01, INPUT-02, L1-03, L1-04, L1-07)

### Phase 4: L1 Dict, RLE, and L2 Escape Infrastructure

**Goal**: Dictionary and run-length-encoded columns decode correctly, and the KernelEscape arm routes through a wired L2KernelRegistry (with a stub FSST kernel) without panicking
**Depends on**: Phase 3
**Requirements**: L1-05, L1-06, L2-01
**Success Criteria** (what must be TRUE):

  1. A dictionary-encoded integer column (codes -> values lookup with recursive sub-array dispatch) decodes to an Arrow array matching the expected values row-for-row, including nullable variants
  2. A run-length-encoded boolean column and a run-length-encoded integer column both expand correctly via run-end expansion, with nulls preserved
  3. A `LayoutNode::KernelEscape { kernel_id: 0, ... }` node routes to `L2KernelRegistry::get(0)` without panicking (stub kernel returns empty output; the routing path is the deliverable)

**Plans**: 2 plans

**Wave 1**

- [x] 04-01-PLAN.md - loom-core implementation: Boolean OutputBuilder, dictionary lookup, RunEnd expansion, L2KernelRegistry + FSST stub, KernelEscape routing, and FOR-over-Raw fix (L1-05, L1-06, L2-01)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 04-02-PLAN.md - loom-fixtures bridge + oracle tests for dict/RLE and public KernelEscape routing verification (L1-05, L1-06, L2-01)

### Phase 5: FSST L2 Kernel and Full Verification

**Goal**: The FSST L2 kernel decompresses FSST-encoded strings and dict-over-FSST columns correctly, and a DuckDB SQL query over a Loom-decoded column matches Vortex's own decoder row-for-row -- the MVP0 acceptance gate
**Depends on**: Phase 4
**Requirements**: L2-02, L2-03, VERIFY-01, VERIFY-02, VERIFY-03
**Success Criteria** (what must be TRUE):

  1. The Vortex reference decoder (`into_canonical().into_arrow()`) and the Loom L1 loop produce byte-for-byte identical output for an FSST-encoded string column, including empty strings, all-escape-sequence strings, and max-length (8-byte) symbol strings
  2. A dict-over-FSST column decodes end-to-end through the dict L1 arm -> FSST L2 kernel path and matches the Vortex reference row-for-row
  3. `SELECT * FROM loom_scan(...)` in DuckDB over every supported encoding (bitpack, FOR, dict, RLE, FSST, dict-over-FSST) returns results matching the Vortex reference decoder with zero row mismatches
  4. `VERIFY-03` passes: an aggregate SQL query (e.g. `SELECT COUNT(*), SUM(col)`) over the Loom-decoded column returns the same result as the same query over the Vortex-decoded oracle

**Plans**: 4 plans across 3 waves

- **Wave 1**: `05-01` Core FSST L2 kernel and Utf8 dict-over-FSST integration
- **Wave 2** *(blocked on Wave 1 completion)*: `05-02` Vortex FSST fixture/oracle row-match, `05-03` Loom layout payload codec and FFI decode path
- **Wave 3** *(blocked on Waves 1-2 completion)*: `05-04` DuckDB SQL MVP0 acceptance gate

- [x] 05-01-PLAN.md - Core FSST L2 kernel, validated params, Utf8 builder/materialization, dict-over-FSST integration (L2-02, L2-03)
- [x] 05-02-PLAN.md - Vortex FSST bridge, Utf8 oracle, FSST and dict-over-FSST fixture row matching (L2-02, L2-03, VERIFY-01, VERIFY-02)
- [x] 05-03-PLAN.md - MVP0 layout payload codec, non-empty FFI input decode, Utf8 Arrow C buffer coverage (VERIFY-02, VERIFY-03)
- [x] 05-04-PLAN.md - DuckDB payload emitter, payload-aware loom_scan, and final SQL acceptance gate (VERIFY-01, VERIFY-02, VERIFY-03)

### Phase 6: MVP0 Hardening and Release Baseline

**Goal**: The completed MVP0 is reproducible and reviewable from a clean checkout: documentation reflects the actual implementation, stale planning state is resolved, one command runs the acceptance gate, and build hygiene prevents stale Rust/C++ artifacts from masking regressions
**Depends on**: Phase 5
**Requirements**: BASE-01, DOC-01, DOC-02, VERIFY-04, BUILD-01
**Success Criteria** (what must be TRUE):

  1. `.planning/PROJECT.md`, `.planning/STATE.md`, `.planning/ROADMAP.md`, and `.planning/REQUIREMENTS.md` agree that MVP0 is complete and Phase 6 is the active hardening phase
  2. README documents the current MVP0 implementation status, exact verification commands, and links the Vortex/AnyBlox/F3 positioning note
  3. A single script, `scripts/mvp0-verify.sh`, runs the full release gate: workspace tests, core dependency guard, fixture hygiene grep, and DuckDB SQL smoke test
  4. The DuckDB extension build path cannot silently reuse a stale Rust staticlib during the release gate
  5. Phase 6 final verification passes from the repository root without requiring manual cleanup of generated fixture or extension outputs

**Plans**: 3 plans across 3 waves

- **Wave 1**: `06-01` Planning-state and README consistency cleanup
- **Wave 2** *(blocked on Wave 1 completion)*: `06-02` One-command MVP0 verification gate and build hygiene
- **Wave 3** *(blocked on Wave 2 completion)*: `06-03` Baseline audit, final docs, and Phase 7 readiness notes

- [x] 06-01-PLAN.md - Update state/project/requirements/README, resolve stale blockers, and fix roadmap mojibake (BASE-01, DOC-01, DOC-02)
- [x] 06-02-PLAN.md - Add `scripts/mvp0-verify.sh`, strengthen Rust staticlib rebuild behavior, and verify the full gate (VERIFY-04, BUILD-01)
- [x] 06-03-PLAN.md - Run final baseline audit, record verification output, and prepare Phase 7 descriptor/CLI handoff notes (BASE-01, VERIFY-04)

### Phase 7: Human-Readable Layout Descriptor and CLI

**Goal**: A reviewer can inspect and decode Loom layout payloads without reading Rust tests or Vortex bridge code; the human-readable descriptor roundtrips to the existing `LayoutDescription`, and a CLI exposes inspect/decode workflows while keeping `loom-core` Vortex-free
**Depends on**: Phase 6
**Requirements**: DX-01, DX-02, DX-03, DX-04
**Success Criteria** (what must be TRUE):

  1. A recursive human-readable descriptor format represents all MVP0 layout nodes: Raw, BitPack, FrameOfReference, Dictionary, RunEnd, and KernelEscape with FSST params
  2. Descriptor text parses into `LayoutDescription` and prints back deterministically; binary `.loom` payloads can be inspected as descriptor text
  3. `loom inspect <input>` prints schema, dtype, layout tree, row count, and kernel references for generated MVP0 payloads
  4. `loom decode <input>` prints values/nulls for int, bool, and UTF-8 payloads without requiring Rust test code
  5. Fixture coverage includes multiple samples per supported encoding, and optional timing output compares Loom interpreter and Vortex oracle decode as illustrative wall-clock numbers
  6. `scripts/mvp0-verify.sh` remains green, and `loom-core` still has zero Vortex/FastLanes dependencies

**Plans**: 4 plans across 3 waves

- **Wave 1**: `07-01` Descriptor format decision and core parser/printer
- **Wave 2** *(blocked on Wave 1 completion)*: `07-02` Payload inspection bridge and descriptor roundtrip fixtures, `07-03` CLI inspect/decode surface
- **Wave 3** *(blocked on Wave 2 completion)*: `07-04` Expanded fixture matrix, timing output, docs, and final gate

- [x] 07-01-PLAN.md - Define tree-friendly descriptor format and implement deterministic parse/print roundtrip for all MVP0 layout nodes (DX-01)
- [x] 07-02-PLAN.md - Bridge binary payloads to descriptor inspection and add descriptor roundtrip tests against generated fixtures (DX-01, DX-02)
- [x] 07-03-PLAN.md - Add CLI commands `loom inspect` and `loom decode` for payload/descriptor inputs (DX-03)
- [x] 07-04-PLAN.md - Expand fixture matrix, add optional timing output, update docs, and run the full release gate (DX-02, DX-04)

### Phase 8: Multi-Column Table Output and Arrow Stream Evaluation

**Goal**: Loom can describe, decode, inspect, and query a table-shaped payload with multiple named columns of mixed MVP0 types, while preserving the single-column release gate and making an explicit decision on whether to keep direct DuckDB DataChunk population or move to ArrowArrayStream
**Depends on**: Phase 7
**Requirements**: COV-02, TABLE-01, TABLE-02, TABLE-03, DUCK-05, STREAM-01, VERIFY-05
**Success Criteria** (what must be TRUE):

  1. A `TableDescription` or equivalent model represents multiple named columns, each carrying a `LayoutDescription`, dtype, row count, and nullable metadata
  2. A checked table payload format can encode/decode mixed Int32, Boolean, and Utf8 columns without breaking existing single-column `.loom` payloads
  3. Rust-side multi-column decode returns column arrays with a shared row count and typed errors on schema/length mismatch
  4. `loom inspect` and `loom decode` can display table payloads, including column names and row-wise output
  5. `loom_scan(<table-payload>)` in DuckDB returns multiple columns and supports SQL projections, filters, and aggregates over mixed column types
  6. The phase records a concrete ArrowArrayStream decision: implement it if the DuckDB API path is stable in this repo, otherwise document why direct DataChunk population remains the Phase 8 path
  7. `scripts/mvp0-verify.sh` remains green and a new multi-column SQL acceptance check passes

**Plans**: 4 plans across 3 waves

- **Wave 1**: `08-01` Table model and table payload codec
- **Wave 2** *(blocked on Wave 1 completion)*: `08-02` Multi-column fixture emitter, CLI inspect/decode, and Rust table decode tests; `08-03` DuckDB multi-column bind/init/scan and ArrowArrayStream decision
- **Wave 3** *(blocked on Wave 2 completion)*: `08-04` SQL acceptance gate, docs, requirements closure, and release verification

- [x] 08-01-PLAN.md - Add table-shaped descriptor/payload model with checked mixed-column encode/decode while preserving single-column payload compatibility (COV-02, TABLE-01, TABLE-02)
- [x] 08-02-PLAN.md - Generate multi-column fixtures, decode them in Rust, and extend CLI inspect/decode for row-wise table output (TABLE-02, TABLE-03)
- [x] 08-03-PLAN.md - Extend DuckDB `loom_scan` for multiple columns and decide ArrowArrayStream vs direct DataChunk population based on working API evidence (DUCK-05, STREAM-01)
- [x] 08-04-PLAN.md - Add multi-column SQL acceptance checks, update docs, close requirements, and run full release gates (VERIFY-05)

### Phase 9: Verifier and Safety Boundary MVP

**Goal**: Loom rejects malformed or unsafe MVP0 layout/table payloads before decode, with typed verifier diagnostics, CLI visibility, and regression cases that prove the safety boundary exists independently of successful SQL output
**Depends on**: Phase 8
**Requirements**: SAFE-01, SAFE-02, SAFE-03, SAFE-04, VERIFY-06
**Success Criteria** (what must be TRUE):

  1. A verifier module walks `LayoutDescription` and `TableDescription` before decode and returns typed diagnostics instead of panicking
  2. The verifier rejects at least truncated buffers, invalid row/count relationships, invalid dictionary codes domain assumptions, non-monotonic run ends, unknown kernels, unsupported data-type/layout combinations, and table column mismatches
  3. Decode entry points call the verifier or document why an existing decode-time check is the authoritative verifier for that invariant
  4. `loom inspect` exposes verifier pass/fail status for payloads and descriptors
  5. A negative fixture/test suite proves malformed payloads fail closed without crossing into DuckDB execution
  6. `scripts/mvp0-verify.sh` remains green and includes the verifier regression suite

**Plans**: 4 plans across 3 waves

**Wave 1**

- [x] 09-01-PLAN.md - Add `loom_core::verifier` report/diagnostic model and structural layout/table verification (SAFE-01, SAFE-02)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 09-02-PLAN.md - Route verifier through Rust decode helpers and FFI ingress while preserving decode-time authoritative checks (SAFE-03)
- [x] 09-03-PLAN.md - Expose verifier status in `loom inspect` and add curated negative verifier gate (SAFE-04, VERIFY-06)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 09-04-PLAN.md - Document verifier MVP behavior, audit stale FOR todo, run final gates, and close requirements (SAFE-01, SAFE-02, SAFE-03, SAFE-04, VERIFY-06)

### Phase 10: Additional L2 Kernels and Numeric Compression Coverage

**Goal:** Extend Loom's L2 kernel path beyond FSST with ALP-style Float32/Float64 numeric compression coverage, proving the registry/params/verifier/fixture/CLI/DuckDB surfaces generalize to a second real kernel while keeping `loom-core` Vortex-free.
**Requirements**: COV-01
**Depends on:** Phase 9
**Success Criteria** (what must be TRUE):

  1. Float32 and Float64 are supported by the core descriptor/payload/materialization surfaces needed for L2 kernel output
  2. `AlpParams` has a stable checked binary format carrying decoded output type, row count, decimal exponent, mantissas, and validity
  3. ALP is registered as append-only kernel id `1`, FSST remains id `0`, and verifier rejects malformed ALP params or output-type mismatches
  4. ALP Float32 and Float64 fixtures decode correctly against synthetic known values and Vortex primitive float oracle rows
  5. `loom inspect` and `loom decode` expose concise ALP/float output for reviewers
  6. DuckDB SQL smoke tests include ALP Float32 and Float64 row and aggregate checks, and `scripts/mvp0-verify.sh` remains green

**Plans:** 4 plans across 3 waves

**Wave 1**

- [x] 10-01-PLAN.md - Add Float32/Float64 core support, `AlpParams`, ALP kernel id 1, and verifier checks (COV-01)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 10-02-PLAN.md - Add ALP synthetic fixtures, Vortex primitive float oracle comparisons, and FFI float roundtrips (COV-01)
- [x] 10-03-PLAN.md - Add CLI ALP/float output plus DuckDB Float32/Float64 SQL smoke coverage (COV-01)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 10-04-PLAN.md - Document ALP coverage, run final gates, close COV-01, and write Phase 10 summaries (COV-01)

### Phase 11: Distribution Container v0

**Goal:** Introduce the first explicit Loom distribution artifact boundary by wrapping existing single-column `LMP1` and table-shaped `LMT1` payloads in a versioned `LMC1` container with magic/version, required and optional feature flags, a checked section directory, verifier routing, CLI visibility, and release-gate coverage.
**Requirements:** DIST-01, DIST-02, DIST-03, DIST-04, DIST-05
**Depends on:** Phase 10
**Research:** `.planning/phases/11-distribution-container-v0/11-RESEARCH.md`
**Success Criteria** (what must be TRUE):

  1. `LMC1` container encode/decode roundtrips both single-column and table payloads while preserving raw `LMP1`/`LMT1` compatibility
  2. The container header exposes version, required features, optional features, and a checked section directory with offset/length validation
  3. Unknown required features, unsupported versions, duplicate required sections, truncated sections, and offset overflows fail closed with typed diagnostics
  4. `loom inspect` shows container version, feature sets, section summary, schema/payload kind, and verifier pass/fail status
  5. Generated fixtures and `scripts/mvp0-verify.sh` cover container-wrapped payload success plus negative container rejection cases

**Plans:** 4 plans across 3 waves

**Wave 1**

- [x] 11-01-PLAN.md - Add core `LMC1` container codec, feature bitsets, checked section directory, and `LMP1`/`LMT1` wrapper helpers (DIST-01, DIST-02, DIST-03)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 11-02-PLAN.md - Route `LMC1` through Rust verifier/decode/FFI while preserving raw payload compatibility (DIST-01, DIST-02, DIST-03)
- [x] 11-03-PLAN.md - Expose `LMC1` through CLI, generated fixtures, DuckDB bind, and SQL smoke coverage (DIST-01, DIST-04, DIST-05)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 11-04-PLAN.md - Document container v0, add negative container gate, run final verification, and close DIST requirements (DIST-01, DIST-02, DIST-03, DIST-04, DIST-05)

### Phase 12: Formal Verifier / Safety Proof MVP

**Goal:** Make Loom's implemented `LMC1`/`LMP1`/`LMT1` byte-to-Arrow safety boundary reviewable and mechanically guarded through a safety contract, proof-obligation matrix, focused no-panic/fail-closed tests, a bounded-loop safety argument, and a dedicated release gate.
**Requirements:** PROOF-01, PROOF-02, PROOF-03, PROOF-04, PROOF-05
**Depends on:** Phase 11
**Research:** `.planning/phases/12-formal-verifier-safety-proof-mvp/12-RESEARCH.md`
**Success Criteria** (what must be TRUE):

  1. A safety contract and proof-obligation matrix define the current implemented boundary, source evidence, executable evidence, release-gate evidence, and explicit exclusions
  2. Curated malformed `LMC1`/`LMP1`/`LMT1`/descriptor inputs fail closed through typed errors or verifier diagnostics and do not panic in focused tests
  3. The written safety proof explains no-unsafe-core, FFI panic containment, decode-before-Arrow behavior, and bounded parser/interpreter/kernel loops for the current implementation
  4. `scripts/safety-proof-test.sh` checks proof docs, obligation IDs, static invariants, focused tests, and existing negative gates
  5. `scripts/mvp0-verify.sh` invokes the safety proof gate, and docs do not claim future L2 language, MLIR/native lowering, real Vortex ingress, or correctness proofs

**Plans:** 4 plans across 3 waves

**Wave 1**

- [x] 12-01-PLAN.md - Define the safety contract, proof-obligation matrix, loop-bound audit, and unsafe-boundary argument (PROOF-01, PROOF-03, PROOF-05)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 12-02-PLAN.md - Add focused core/FFI no-panic and fail-closed safety contract tests (PROOF-02, PROOF-03)
- [x] 12-03-PLAN.md - Add `scripts/safety-proof-test.sh`, wire it into `mvp0-verify.sh`, and map gate evidence (PROOF-02, PROOF-04)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 12-04-PLAN.md - Write final safety proof, update public/planning docs, run final gates, and close PROOF requirements (PROOF-01, PROOF-02, PROOF-03, PROOF-04, PROOF-05)

### Phase 13: Full Loom Verifier

**Status:** Complete.
**Goal:** Establish the full Loom verifier foundation for the future distribution IR and L2 total-function language, with a tiny `L2Core` vertical slice that combines an executable Rust verifier, local SMT obligations, mechanized Lean/Rocq soundness scaffolding, and TLA+ lifecycle/pipeline invariants.
**Depends on:** Phase 12
**Requirements:** VERIFIER-01, VERIFIER-02, VERIFIER-03, VERIFIER-04, VERIFIER-05, VERIFIER-06, VERIFIER-07, VERIFIER-08, VERIFIER-09, VERIFIER-10
**Research:** `.planning/phases/13-full-loom-verifier/13-RESEARCH.md`
**Context:** `.planning/phases/13-full-loom-verifier/13-CONTEXT.md`
**Success Criteria** (what must be TRUE):

  1. A normative verifier/spec document defines the Phase 13 `L2Core` subset, capability model, resource model, and Arrow builder event semantics.
  2. A Rust verifier prototype or architecture in `loom-core` uses type/effect checking plus abstract interpretation to reject unsafe `L2Core` artifacts.
  3. Local arithmetic/range/loop/resource obligations are represented as explicit verifier constraints, with an SMT-ready path.
  4. A Lean or Rocq proof scaffold defines core syntax/static semantics/dynamic semantics and states or proves an accepted-program safety theorem.
  5. A TLA+ lifecycle model captures parse/verify/lower/cache transitions and the invariant that lowering cannot occur before verifier acceptance.
  6. Phase 13 emits verifier facts/proof obligations that Phase 14 can consume as native-lowering preconditions.

**Plans:** 5 plans across 4 waves

**Wave 1**

- [x] 13-01-PLAN.md - Define the normative `L2Core` verifier spec and Phase 13 proof-obligation matrix (VERIFIER-01, VERIFIER-02, VERIFIER-03, VERIFIER-04, VERIFIER-05, VERIFIER-10)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 13-02-PLAN.md - Add the Rust `L2Core` syntax/fact model and SMT-ready constraint IR (VERIFIER-03, VERIFIER-04, VERIFIER-05, VERIFIER-07, VERIFIER-10)

**Wave 3** *(blocked on required prior artifacts)*

- [x] 13-03-PLAN.md - Implement the Rust abstract-interpreting `L2Core` verifier with diagnostics, facts, tests, and optional CLI visibility (VERIFIER-04, VERIFIER-06, VERIFIER-07, VERIFIER-08, VERIFIER-10)
- [x] 13-04-PLAN.md - Add Lean soundness scaffold, TLA+ lifecycle model, and full-verifier gate script (VERIFIER-01, VERIFIER-03, VERIFIER-04, VERIFIER-05, VERIFIER-09, VERIFIER-10)

**Wave 4** *(blocked on Waves 1-3 completion)*

- [x] 13-05-PLAN.md - Write final verifier report, update public/planning docs, wire release gate, run final verification, and close VERIFIER requirements (VERIFIER-01, VERIFIER-02, VERIFIER-03, VERIFIER-04, VERIFIER-05, VERIFIER-06, VERIFIER-07, VERIFIER-08, VERIFIER-09, VERIFIER-10)

### Phase 14: MLIR/Native Lowering Spike

**Goal**: Prove the first verifier-gated native-lowering boundary by accepting only a tiny Phase 13 `L2Core` bounded Int32 copy slice, rejecting unsupported programs fail-closed, emitting deterministic textual MLIR, and recording optional MLIR/native toolchain evidence without making MLIR/LLVM/JIT mandatory dependencies
**Depends on:** Phase 13
**Requirements:** LOWER-01, LOWER-02, LOWER-03, LOWER-04, LOWER-05
**Success Criteria** (what must be TRUE):

  1. A lowering contract and support predicate require an accepted `verify_l2_core` report plus present `VerifiedArtifactFacts`; standalone facts or rejected reports cannot lower
  2. Unsupported accepted programs fail closed with stable lowering diagnostics before any MLIR/native artifact is emitted
  3. The bounded Int32 copy sample emits deterministic textual MLIR using standard `func`, `arith`, `scf`, and `memref` dialect operations without adding mandatory `melior`, LLVM, or Cranelift dependencies
  4. Focused tests compare the supported slice against typed primitive reference output and cover negative range/capacity cases
  5. `scripts/native-lowering-test.sh` runs the focused gate and reports optional `mlir-opt`/toolchain validation as skipped when unavailable rather than failing the release gate
  6. Public and planning docs state that Phase 14 is a lowering spike, not production native compiler completion, custom Loom dialect, vectorization, or compiler correctness proof

**Plans:** 4 plans across 4 waves

**Wave 1**

- [x] 14-01-PLAN.md - Define the lowering contract and implement verifier-gated support predicate with fail-closed diagnostics (LOWER-01, VERIFIER-10)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 14-02-PLAN.md - Emit deterministic textual MLIR for the bounded Int32 copy subset without mandatory MLIR/LLVM dependencies (LOWER-02)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 14-03-PLAN.md - Add supported-slice reference/equivalence tests and optional native-lowering gate script (LOWER-03, LOWER-04)

**Wave 4** *(blocked on Waves 1-3 completion)*

- [x] 14-04-PLAN.md - Write final lowering report, update public/planning docs, wire release gate, run final verification, and close LOWER requirements (LOWER-01, LOWER-02, LOWER-03, LOWER-04, LOWER-05)

### Phase 15: Real Vortex File/Container Ingress

**Goal:** Add a narrow real Vortex file/container ingress boundary that keeps `vortex-file` isolated outside `loom-core`, emits Loom-owned file/layout/segment facts with stable diagnostics, opens real `.vortex` buffers/paths fail-closed, and proves one supported real `.vortex` fixture can enter the existing `LMC1` verifier/decode path with Vortex oracle equality.
**Depends on:** Phase 11, Phase 13, and Phase 14.
**Requirements:** INGEST-01, INGEST-02, INGEST-03, INGEST-04, INGEST-05
**Research:** `.planning/phases/15-real-vortex-file-container-ingress/15-RESEARCH.md`
**Report:** `.planning/phases/15-real-vortex-file-container-ingress/15-INGRESS-REPORT.md`
**Ordering decision:** Keep Phase 15 before full `melior`/LLVM/JIT integration. Real Vortex ingress should stabilize the file/container/layout evidence that Phase 16 consumes; Phase 15 must not add native lowering or native-speed claims.
**Success Criteria** (what must be TRUE):

  1. `vortex-file` is introduced only through an isolated ingress boundary; `loom-core` and `loom-ffi` remain free of `vortex-*` dependencies, and scoped guard scripts enforce that boundary
  2. A stable `VortexIngressReport` / `VortexFileFacts` model records row count, dtype/layout summaries, segment ranges/alignment, statistics presence, supported/unsupported status, and stable diagnostic codes without exposing Vortex types
  3. Real Vortex buffers and local paths can be inspected; malformed/truncated/unsupported files fail closed with diagnostics instead of panics or partial `.loom` output
  4. At least one generated real `.vortex` fixture emits an existing `LMC1` payload, passes `verify_container`, decodes through Loom, and matches Vortex oracle rows
  5. CLI/docs/release gates expose the narrow ingress behavior without claiming arbitrary Vortex layout support, remote/object-store ingress, native lowering, or production speed

**Plans:** 4 plans across 4 waves

**Wave 1**

- [x] 15-RESEARCH.md - Research real Vortex file/container ingress, `vortex-file` 0.74.0 APIs, file-format shape, dependency boundary, and recommended plan split
- [x] 15-01-PLAN.md - Define ingress contract, isolated ingress crate, and scoped `vortex-file` dependency/API guards (INGEST-01, INGEST-02)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 15-02-PLAN.md - Open real Vortex buffers/paths and emit deterministic metadata facts with malformed-input diagnostics (INGEST-02, INGEST-03)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 15-03-PLAN.md - Generate a supported real `.vortex` fixture, emit `LMC1`, verify/decode it through Loom, and compare against Vortex oracle rows (INGEST-04)

**Wave 4** *(blocked on Waves 1-3 completion)*

- [x] 15-04-PLAN.md - Add CLI inspection, wire the ingress gate, update docs/planning state, write final report, and run final verification (INGEST-01, INGEST-02, INGEST-03, INGEST-04, INGEST-05)

### Phase 16: Full melior/LLVM/JIT Backend Integration

**Status:** Complete (2026-06-08).
**Depends on:** Phase 14 and Phase 15.
**Ordering decision:** Promote the Phase 14 verifier-gated textual MLIR spike into an optional programmatic backend only after real Vortex artifact shapes are visible. Scope should remain verifier-gated and fail-closed, with `melior`/LLVM/JIT kept behind optional tooling until the backend is stable.

**Research:** `.planning/phases/16-full-melior-llvm-jit-backend-integration/16-RESEARCH.md`

**Plans:** 5 plans across 5 waves

**Wave 1**

- [x] 16-01-PLAN.md - Toolchain contract and optional backend crate boundary

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 16-02-PLAN.md - Programmatic melior module construction for bounded Int32 copy

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 16-03-PLAN.md - MLIR validation pipeline and skip-aware backend gate

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 16-04-PLAN.md - MLIR ExecutionEngine/JIT execution and Rust reference equivalence

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 16-05-PLAN.md - Final docs, release-gate wiring, and roadmap/state closeout

**Cross-cutting constraints:**

- The backend remains verifier-gated and accepts only the Phase 14 bounded Int32 copy slice.
- `loom-core` and `loom-ffi` remain free of mandatory MLIR/LLVM/JIT dependencies.
- Missing or incompatible MLIR/LLVM is skip-aware in normal gates and fail-closed in strict mode.
- Phase 16 must not claim custom Loom dialect, vectorization, DuckDB native execution, or complete Vortex reader support.
- Local Phase 16 evidence records LLVM/MLIR major 21 vs expected 22 as a normal-mode skip and strict-mode failure; compatible MLIR 22 environments are required for feature-enabled JIT evidence.

### Phase 17: Unified Artifact Verification Pipeline

**Status:** Complete. See `.planning/phases/17-unified-artifact-verification-pipeline/17-RESEARCH.md`, `17-ARTIFACT-VERIFIER-CONTRACT.md`, `17-ARTIFACT-VERIFIER-REPORT.md`, and `17-SUMMARY.md`.
**Depends on:** Phase 16.
**Ordering decision:** Before widening native lowering, unify the two current verifier lines into one artifact verifier pipeline. `verifier.rs` handles implemented `LMP1`/`LMT1`/`LMC1` structural payload verification, while `full_verifier.rs` handles the future `L2Core` verifier foundation. Phase 17 made those a single artifact-facing flow:

```text
LMC1 artifact
  -> schema/features/kernel manifest
  -> L1 structural verification
  -> L2Core verification
  -> constraint/facts collection
  -> VerifiedArtifactFacts
  -> lowering-ready verification report
```

**Verifier shortfall closed by Phase 17:**

- current payload verifier and future `L2Core` verifier now have one artifact-facing report/facts pipeline
- static structural verifier, runtime semantic guard, and oracle/equivalence evidence are explicitly layered in reports/docs
- lowering can consume one accepted artifact report with optional `L2Core` facts and lowering readiness

**Deferred beyond Phase 17:**

- real SMT discharge with Z3/CVC5 or equivalent solver strategy for symbolic offset/range/overflow obligations
- stable external `L2Core` artifact codec/parser instead of only Rust data model/sample CLI
- deeper value-dependent semantic checks beyond conservative static verification and runtime guards
- publishable Lean/Rocq/TLA proof depth beyond current scaffold
- production MLIR decode dialect, Arrow/raw-buffer native writes, vectorization, and broad native kernel expansion

**Plan Files:**
- [x] 17-01-PLAN.md - Artifact verifier contract and report model
- [x] 17-02-PLAN.md - Container and L1 structural artifact pipeline
- [x] 17-03-PLAN.md - L2Core adapter and verifier facts fusion
- [x] 17-04-PLAN.md - Lowering readiness, CLI visibility, and gate script
- [x] 17-05-PLAN.md - Final docs, verification report, and planning closeout

### Phase 18: Complete Vortex Reader

**Status:** Placeholder only. Do not expand until the narrow Phase 15 ingress boundary and Phase 17 unified verifier pipeline constraints are reviewed together.
**Depends on:** Phase 15 and Phase 17; may consume constraints discovered in Phase 16.
**Ordering decision:** Expand from the supported non-null Int32 `.vortex` -> `LMC1` evidence slice to a complete Vortex reader boundary before engine-integrated native execution. Engine integration needs stable real artifact/fact/schema semantics; those should come from the full reader boundary rather than from the Phase 15 narrow ingress slice. Scope should include real file/container layout coverage, chunk/schema handling, representative encoding coverage, projection/statistics decisions, stable Loom-owned facts/diagnostics, and fail-closed behavior. It must not become a new query-engine integration phase.

### Phase 19: Solver-backed Full Artifact Verifier

**Status:** Placeholder only. Do not expand until Phase 18 establishes the complete Vortex reader boundary.
**Depends on:** Phase 16, Phase 17, and Phase 18.
**Ordering decision:** Upgrade the Phase 17 unified artifact pipeline from collected obligations to solver-backed verifier evidence before production native expansion. Phase 18 must come first so the verifier targets real complete-reader facts instead of only the synthetic or narrow Phase 13/14/16 bounded copy slice. Scope should include a Z3/CVC5 or SMT-LIB strategy, symbolic offset/range/overflow obligation discharge, fail-closed unknown/unsupported obligations, stable external `L2Core` artifact codec/parser planning or implementation, solver-backed artifact reports, and `VerifiedArtifactFacts` that can be trusted by later native lowering only when obligations are discharged. It must not become production MLIR dialect work, native kernel expansion, or host-engine execution.

### Phase 20: Production Decode Dialect and Native Kernel Expansion

**Status:** Placeholder only. Do not expand until Phase 18 establishes the complete Vortex reader boundary and Phase 19 provides solver-backed artifact verifier evidence.
**Depends on:** Phase 16, Phase 17, Phase 18, and Phase 19.
**Ordering decision:** Preserve the production MLIR/native expansion step after verifier unification, complete-reader evidence, and solver-backed artifact verification. This phase should introduce the custom Loom MLIR decode dialect, Arrow/raw-buffer builder lowering, vectorization decisions, multi-column native lowering, and native kernels beyond the bounded Int32 copy slice. It must not become host-engine integration, complete-reader work, or solver work.

### Phase 21: Host Native Runtime ABI and Execution Policy

**Status:** Placeholder only. Do not expand until Phase 18 establishes the complete Vortex reader boundary, Phase 19 establishes solver-backed verifier evidence, and Phase 20 identifies the production native lowering surface.
**Depends on:** Phase 17, Phase 18, Phase 19, and Phase 20.
**Ordering decision:** Define the engine-independent boundary before touching a host engine. This phase should lock the native callable ABI, artifact identity, verified-facts handoff, cache key, diagnostics, memory ownership, Arrow/raw-buffer output contract, fail-closed policy, and interpreter fallback semantics over complete-reader artifacts. It should not become a DuckDB, Iceberg, or StarRocks integration phase.

**Split research:** `.planning/research/ENGINE-INTEGRATION-SPLIT.md`

### Phase 22: DuckDB Native Execution Integration MVP

**Status:** Placeholder only. Do not expand until Phase 21 defines the host native runtime ABI and execution policy.
**Depends on:** Phase 21.
**Ordering decision:** Prove one concrete host integration before broadening the table story. DuckDB is the first host because the project already has a C++ table-function path and SQL smoke gates. This phase should wire the Phase 21 runtime into `loom_scan`/DuckDB table-function execution over complete-reader artifacts, select native only when verifier/native facts accept the program, fall back to the interpreter where policy allows, and preserve fail-closed diagnostics. It must not absorb Iceberg binding or StarRocks comparison.

### Phase 23: Native Equivalence, Cache, and Fallback Hardening

**Status:** Placeholder only. Do not expand until Phase 22 proves the DuckDB native execution MVP.
**Depends on:** Phase 22.
**Ordering decision:** Harden the native execution path before making it table-format-visible. This phase should add oracle/equivalence matrices against interpreter/Vortex rows, native artifact cache reuse and invalidation semantics, unsupported-program negative coverage, deterministic diagnostics, performance smoke evidence, and release-gate wiring. It is the closeout for the engine-integrated native execution story, not a new query surface.

### Phase 24: Iceberg Ref/Table Binding

**Status:** Placeholder only. Do not expand until Phase 23 hardens the native execution contract.
**Depends on:** Phase 18 and Phase 23.
**Ordering decision:** Bind Loom artifacts to Iceberg reference/table metadata before adding more query surfaces. This phase should define how an Iceberg table/ref points at or carries Loom distribution artifacts, how schema/snapshot identity is represented, and how fail-closed verifier facts travel with table metadata. It must not become a StarRocks/DuckDB integration phase.

### Phase 25: StarRocks + DuckDB Dual Query Surface

**Status:** Placeholder only. Do not expand until Phase 24 establishes the Iceberg binding contract.
**Depends on:** Phase 24.
**Ordering decision:** After Iceberg binding exists, prove the same Loom-bound table artifacts can be consumed from both StarRocks and DuckDB query surfaces. This phase should compare integration seams and query behavior across the two engines, rather than inventing a second artifact format.

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8 -> 9 -> 10 -> 11 -> 12 -> 13 -> 14 -> 15 -> 16 -> 17 -> 18 -> 19 -> 20 -> 21 -> 22 -> 23 -> 24 -> 25

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Scaffold and FFI Boundary | 2/2 | Complete    | 2026-06-07 |
| 2. DuckDB Extension Scaffold | 2/2 | Complete    | 2026-06-07 |
| 3. L1 Bitpack, FOR, and Arrow Builders | 2/2 | Complete    | 2026-06-07 |
| 4. L1 Dict, RLE, and L2 Escape Infrastructure | 2/2 | Complete   | 2026-06-07 |
| 5. FSST L2 Kernel and Full Verification | 4/4 | Complete | 2026-06-08 |
| 6. MVP0 Hardening and Release Baseline | 3/3 | Complete | 2026-06-08 |
| 7. Human-Readable Layout Descriptor and CLI | 4/4 | Complete | 2026-06-08 |
| 8. Multi-Column Table Output and Arrow Stream Evaluation | 4/4 | Complete | 2026-06-08 |
| 9. Verifier and Safety Boundary MVP | 4/4 | Complete | 2026-06-08 |
| 10. Additional L2 Kernels and Numeric Compression Coverage | 4/4 | Complete | 2026-06-08 |
| 11. Distribution Container v0 | 4/4 | Complete | 2026-06-08 |
| 12. Formal Verifier / Safety Proof MVP | 4/4 | Complete | 2026-06-08 |
| 13. Full Loom Verifier | 5/5 | Complete | 2026-06-08 |
| 14. MLIR/Native Lowering Spike | 4/4 | Complete | 2026-06-08 |
| 15. Real Vortex File/Container Ingress | 4/4 | Complete | 2026-06-08 |
| 16. Full melior/LLVM/JIT Backend Integration | 5/5 | Complete | 2026-06-08 |
| 17. Unified Artifact Verification Pipeline | 5/5 | Complete | 2026-06-08 |
| 18. Complete Vortex Reader | 0/? | Placeholder | - |
| 19. Solver-backed Full Artifact Verifier | 0/? | Placeholder | - |
| 20. Production Decode Dialect and Native Kernel Expansion | 0/? | Placeholder | - |
| 21. Host Native Runtime ABI and Execution Policy | 0/? | Placeholder | - |
| 22. DuckDB Native Execution Integration MVP | 0/? | Placeholder | - |
| 23. Native Equivalence, Cache, and Fallback Hardening | 0/? | Placeholder | - |
| 24. Iceberg Ref/Table Binding | 0/? | Placeholder | - |
| 25. StarRocks + DuckDB Dual Query Surface | 0/? | Placeholder | - |
