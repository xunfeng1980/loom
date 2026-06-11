# Roadmap: Loom — MVP2 (post-MVP1.5 coverage, native codegen, repositioning)

## Overview

The original Loom MVP0 proved one narrow chain end-to-end: Vortex-style encoded payloads are decoded
by a pure-Rust interpreter through L1 declarative encodings and L2 kernels, producing well-formed
Apache Arrow that crosses a C ABI seam into a C++ DuckDB table function and is queried with SQL.
Phases 1-5 complete **MVP0** (DuckDB demo over FSST/bitpack/FOR/dict/RLE). Phases 6-10 complete the MVP0/v2 hardening, DX, table, verifier, and ALP coverage path. Phases 11-35 complete **MVP1 / v3** (distribution containers, verifier foundation, native lowering, real ingress, source/table/query surface, `LMC2` Arrow semantic distribution). Phases 36-41 complete **MVP1.5** (verified lineage with Lean soundness theorem and model↔native validation). The project is now in **MVP2**, with **Repositioning** (整理稿) phases 48-50 inserted out-of-order to deliver the independent L2Core IR identity and sidecar overlay model.

Key phase highlights: Phase 16 delivered verifier-gated melior/LLVM/JIT backend evidence.
Phases 17-21 unified artifact verification, completed the Vortex reader, added solver-backed
Bitwuzla discharge, production lowering seed, and expanded Vortex encoding coverage. Phases 22-25
established the host runtime ABI, production native backend, DuckDB native integration MVP, and
equivalence/cache/fallback hardening. Phases 26-31 delivered external source ingress, Lance/Parquet
archival readability, semantic compatibility matrices, Iceberg ref binding, StarRocks+DuckDB dual
query surface, and full Arrow semantic source compatibility. Phase 32 completed an MVP1 architecture
and code review. Phases 33-35 delivered `LMC2(LMA1)`, DuckDB Arrow semantic SQL surface, and
engine-neutral native Arrow semantic execution. Phase 48 delivered the kloom v4 spec-oracle differential
gate. Phase 49 delivered the independent L2Core IR codec and content-hash identity. Phase 50.1 (container demotion + thin adapters) and Phase 50 (sidecar overlay) are the next repositioning slices. ABI Freeze was moved
from Phase 44 to Phase 51; Phase 44 is now an MVP1.5 closeout placeholder.

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
- [x] **Phase 13: Full Loom Verifier** - Build the verifier foundation for future Loom distribution IR and L2 total-function language using Rust abstract interpretation, SMT obligations, and Lean/Rocq semantics (complete)
- [x] **Phase 14: MLIR/Native Lowering Spike** - Prove a verifier-gated textual MLIR/native lowering spike over a tiny `L2Core` slice (complete)
- [x] **Phase 15: Real Vortex File/Container Ingress** - Narrow real Vortex ingress boundary: isolated `vortex-file` use, Loom-owned facts/diagnostics, and one supported `.vortex` -> `LMC1` slice before production native backend work (complete)
- [x] **Phase 16: Full melior/LLVM/JIT Backend Integration** - Optional verifier-gated programmatic MLIR/LLVM/JIT backend evidence over the bounded Int32 copy slice, with managed LLVM/MLIR tooling and no production native-compiler claim (complete)
- [x] **Phase 17: Unified Artifact Verification Pipeline** - Fail-closed artifact verifier pipeline from `LMC1` container/schema/features/kernel manifest through L1 verification, L2Core verification, constraints/facts, and lowering-ready report (complete)
- [x] **Phase 18: Complete Vortex Reader** - Complete expansion from Phase 15's narrow real-ingress slice into an isolated, fail-closed Vortex reader boundary with recursive facts, supported artifact emission, CLI visibility, and release-gate evidence
- [x] **Phase 19: Solver-backed Full Artifact Verifier** - Real solver discharge over the unified artifact pipeline after complete-reader facts exist and before production native expansion (complete)
- [x] **Phase 20: Production Decode Dialect Seed and Raw Primitive Native Lowering** - First verifier-gated production native-lowering surface seed with `loom.decode` textual contract, primitive Arrow/raw-buffer builder lowering, raw primitive multi-column matrix, and strict MLIR 22 validation evidence, without claiming a complete compiled dialect or production JIT backend (complete)
- [x] **Phase 21: Expanded Vortex Encoding Coverage** - Widen supported Vortex encoding/layout coverage beyond Phase 18's accepted matrix after solver-backed verifier evidence and the Phase 20 lowering seed exist, with a paired lowering disposition for each new encoding/layout (complete)
- [x] **Phase 22: Host Native Runtime ABI and Execution Policy** - Engine-independent ABI, artifact/facts contract, cache key, fail-closed policy, projection/predicate/split planning, concurrency, and interpreter fallback semantics that host engines will call (complete)
- [x] **Phase 23: Production Native Backend Implementation** - Backend implementation seed over Phase 22 `RuntimePlan`/`RuntimeCacheKey`: compiled `loom.decode` ODS evidence, melior/LLVM pipeline reports, backend identity, cancellation, verifier-gated JIT seed, and release-gate coverage (complete)
- [x] **Phase 24: DuckDB Native Execution Integration MVP** - Ready for research/planning: wire verified native execution into the DuckDB table-function path over complete-reader artifacts with interpreter fallback (completed 2026-06-08)
- [x] **Phase 25: Native Equivalence, Cache, and Fallback Hardening** - Bounded oracle/equivalence gates, in-process native preparation cache reuse/invalidation, negative coverage, and release-gate hardening before table-format binding (completed 2026-06-09)
- [x] **Phase 26: External Source Ingress Contract** - Next active focus for abstracting the Vortex ingress facts, diagnostics, support classification, emission disposition, and verifier-routed emission pattern into a generic source-ingress contract before Lance/MCAP/Zarr/LeRobot-style integrations duplicate it (completed 2026-06-09)
- [x] **Phase 27: Lance + Parquet Archival Readability / Dataset Ingress** - Verifier-backed Loom artifacts for supported Lance datasets and Parquet files so supported schema, fragment/row-group, and column data remain readable and rewritable across source-reader version drift, with current-version read/write/verify and actual older-version fixture compatibility as the value proof (completed 2026-06-09)
- [x] **Phase 28: Full Lance + Parquet + Vortex Semantic Compatibility** - Bounded compatibility gate after source ingress/readability evidence and before Iceberg/query-surface work; records accepted, unsupported, rejected, canonicalized, and native-disposition rows without overclaiming full structured support (completed 2026-06-09)
- [x] **Phase 29: Iceberg Ref/Table Binding** - Bind verifier-backed Loom artifacts to local Iceberg table/reference metadata with schema/snapshot identity, sidecar/reference evidence, and fail-closed mismatch handling before query-surface work (completed 2026-06-09)
- [x] **Phase 30: StarRocks + DuckDB Dual Query Surface** - Bounded dual query-surface proof: DuckDB executable `loom_scan(path)` SQL and StarRocks-compatible offline descriptor evidence over Phase 29 accepted bytes, with optional non-canonical runtime smoke (completed 2026-06-09)
- [x] **Phase 31: Full Arrow Semantic Source Compatibility** - Replace the bounded source-ingress raw/table slice with verifier-backed Arrow semantic artifacts for arbitrary Lance/Parquet schemas and materialized Vortex dtypes (completed 2026-06-09)
- [x] **Phase 32: MVP1 Architecture and Code Review** - Audit the full MVP1 design and implementation for architectural consistency, true execution evidence, ABI/FFI safety, release-gate fidelity, dependency boundaries, code quality, and overclaim cleanup before further feature expansion (completed 2026-06-09)
- [x] **Phase 33: LMC2 Arrow Semantic Container Wrapper** - Implement the `LMC2` distribution wrapper around verifier-backed `LMA1` Arrow semantic payloads before expanding query or native claims (completed 2026-06-09)
- [x] **Phase 34: DuckDB Arrow Semantic SQL Surface for LMC2(LMA1)** - Broaden DuckDB `loom_scan(path)` support by accepting default `LMC2(LMA1)` artifacts, unwrapping to inner `LMA1`, and staging SQL support from multi-column primitive/nullable through logical and nested Arrow semantic payloads (completed 2026-06-09)
- [x] **Phase 35: Native Arrow Semantic Execution** - Add true verifier-gated, engine-neutral native execution for Arrow semantic payloads with equivalence evidence, rather than relying on interpreter fallback (completed 2026-06-09)

**MVP1.5 — Verified Lineage** *(active as of Phase 36; supersedes the earlier parked Phase 36/37. Stages 0/B/C/D do not depend on Phase 35; only Phase 40 depends on Phase 35. Standing red line: Loom guarantees safety + well-formedness, never correctness — every "verified" claim maps to one named evidence layer or to the explicit TCB.)*

- [x] **Phase 36: Verified-Lineage Contract and TCB Declaration** - Define what "verified" means at MVP1.5 exit, the obligation matrix, and the explicit Trusted Computing Base (Rust compiler, LLVM/MLIR, C ABI seam, DuckDB host, Arrow C Data Interface) (completed 2026-06-09)
- [x] **Phase 37: Lean Stage B — Lean ↔ Rust Verifier Correspondence** - Enrich the Lean AST (ScalarExpr/LetScalar) so `builder_events_typed` derives value types from expressions like the Rust verifier; wire Lean↔Rust differential testing into the release gate (supersedes parked Phase 36)
- [x] **Phase 38: Lean Stage C — Operational Semantics and Soundness Theorem** - Define small-step operational semantics over L2Core and prove the load-bearing safety theorem so `accepted_program_safe` is a semantic theorem, scoped explicitly to the modeled executor (supersedes parked Phase 37)
- [x] **Phase 39: Model ↔ Rust Interpreter Consistency** - Validate that the real Rust interpreter (the actual safety path) matches a faithful transcription of the Lean operational semantics, event-for-event, across the supported matrix plus a fuzz corpus
- [x] **Phase 40: Native ↔ Model Validation** - Re-anchor Phase 35 native equivalence against the faithful model reference (not just the interpreter), as per-run translation validation; record the MLIR/LLVM pipeline as a permanent TCB trust assumption (completed 2026-06-09)
- [x] **Phase 41: Verified-Lineage Closeout** - One combined `verified-lineage-test.sh` gate over all stages, plus a per-artifact verified-lineage record that names the evidence layers backing its safety (completed 2026-06-09)

**MVP2 — Coverage, Second Engine, Productization** *(active as of Phase 44. Widen coverage, realize and stabilize true native codegen, track the suspended second-engine runtime proof, then freeze the ABI (Phase 51). Distribution, signing, remote fetch, and GA hardening are deferred to later phases.)*

- [x] **Phase 42: Verified + Native Coverage Expansion** - Widen the accepted Vortex/Lance/Parquet encoding/layout/shape matrix, each new shape carrying a paired verified-lineage + native-execution + interpreter-fallback disposition
- [ ] **Phase 43: StarRocks Live Runtime Integration** - Suspended pending live StarRocks runtime/client availability; completed contract/gate/ABI-findings work remains retained, while `ENGINE-01` moves to a pre-GA reactivation gate
- [x] **Phase 43.1: Production Native Codegen Realization** - 44-pre / 44A inserted phase: replace the current Rust/Arrow native-copy placeholder with true MLIR/LLVM/JIT/native backend output for the Phase 35 Arrow semantic matrix, validated by Phase 40 before any runtime/cache admission
- [x] **Phase 43.2: Production Native Codegen Stabilization and Production Readiness** - 44-pre stabilization phase: harden the real Phase 43.1 native-codegen path with deterministic replay, production-route evidence, adversarial validation, perf/soak/resource checks, and an ABI-freeze dossier
- [ ] **Phase 44: MVP1.5 Closeout and Milestone Archive** - Placeholder — spec via `/gsd-spec-phase 44`
- [ ] **Phase 51: ABI Freeze and Compatibility Contract** - Freeze `loom_runtime.h` / the C ABI with a versioned compatibility policy, informed by the full coverage matrix, DuckDB evidence, Phase 43's recorded ABI findings, Phase 43.1's real native-codegen requirements, and Phase 43.2's production-readiness evidence; live second-engine runtime evidence is deferred to pre-GA reactivation

**Repositioning (整理稿) — Decode-IR Sidecar** *(active; phases 48-50 inserted out-of-order. Decision One: separate decode IR from container. Decision Two: sidecar overlay + host-native reader fallback.)*

- [x] **Phase 48: K Spec-Oracle Differential Gate Completion (方案 A)** - Close kloom v4 spec-oracle gaps: typed KOracleOutcome, krun-absent skip, garbled-output hard-fail, per-shape native-route disable, strict skip convention, LLVM-backend feasibility evidence (completed 2026-06-10)
- [x] **Phase 49: Independent L2Core Decode IR Codec and Content-Hash Identity** - Decision One: standalone L2Core IR codec with `L2IR` magic/version, deterministic wire format, content-hash identity via FNV-1a over canonical bytes, fail-closed parse-and-verify (completed 2026-06-11)
- [x] **Phase 50.1: Container Demotion and Thin Host Adapters** - First slice of Decision Two: demote LMC2/LMA1 from top-level format to optional lineage section + reference packaging; degrade existing ingress crates into thin host adapters (one IR + three adapters) (completed 2026-06-11)
- [ ] **Phase 50: Sidecar Overlay Model and Host-Native Reader Fallback** - Second slice of Decision Two: sidecar overlay contract, content-hash binding at column-chunk/fragment granularity, fail-closed routing to verifiable-native track or host's own native reader

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
**Goal:** Establish the full Loom verifier foundation for the future distribution IR and L2 total-function language, with a tiny `L2Core` vertical slice that combines an executable Rust verifier, local SMT obligations, and mechanized Lean/Rocq soundness scaffolding.
**Proof status note:** Phase 13 Lean is a scaffold only: its current semantic predicates are `True` placeholders, so theorem names such as `accepted_program_safe` are not load-bearing proof evidence. Current load-bearing verifier evidence comes from the Rust verifier and, after Phase 19, Bitwuzla-backed SMT discharge.
**Depends on:** Phase 12
**Requirements:** VERIFIER-01, VERIFIER-02, VERIFIER-03, VERIFIER-04, VERIFIER-05, VERIFIER-06, VERIFIER-07, VERIFIER-08, VERIFIER-09, VERIFIER-10
**Research:** `.planning/phases/13-full-loom-verifier/13-RESEARCH.md`
**Context:** `.planning/phases/13-full-loom-verifier/13-CONTEXT.md`
**Success Criteria** (what must be TRUE):

  1. A normative verifier/spec document defines the Phase 13 `L2Core` subset, capability model, resource model, and Arrow builder event semantics.
  2. A Rust verifier prototype or architecture in `loom-core` uses type/effect checking plus abstract interpretation to reject unsafe `L2Core` artifacts.
  3. Local arithmetic/range/loop/resource obligations are represented as explicit verifier constraints, with an SMT-ready path.
  4. A Lean or Rocq proof scaffold defines core syntax/static semantics/dynamic semantics and states an accepted-program safety theorem target; current predicates are placeholders, so this is not load-bearing proof evidence.
  5. The artifact verifier pipeline enforces parse-before-verify and verify-before-lower ordering; lowering readiness is a fail-closed decision over the unified verification report.
  6. Phase 13 emits verifier facts/proof obligations that Phase 14 can consume as native-lowering preconditions.

**Plans:** 5 plans across 4 waves

**Wave 1**

- [x] 13-01-PLAN.md - Define the normative `L2Core` verifier spec and Phase 13 proof-obligation matrix (VERIFIER-01, VERIFIER-02, VERIFIER-03, VERIFIER-04, VERIFIER-05, VERIFIER-10)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 13-02-PLAN.md - Add the Rust `L2Core` syntax/fact model and SMT-ready constraint IR (VERIFIER-03, VERIFIER-04, VERIFIER-05, VERIFIER-07, VERIFIER-10)

**Wave 3** *(blocked on required prior artifacts)*

- [x] 13-03-PLAN.md - Implement the Rust abstract-interpreting `L2Core` verifier with diagnostics, facts, tests, and optional CLI visibility (VERIFIER-04, VERIFIER-06, VERIFIER-07, VERIFIER-08, VERIFIER-10)
- [x] 13-04-PLAN.md - Add Lean soundness scaffold and full-verifier gate script (VERIFIER-01, VERIFIER-03, VERIFIER-04, VERIFIER-05, VERIFIER-09, VERIFIER-10)

**Wave 4** *(blocked on Waves 1-3 completion)*

- [x] 13-05-PLAN.md - Write final verifier report, update public/planning docs, wire release gate, run final verification, and close VERIFIER requirements (VERIFIER-01, VERIFIER-02, VERIFIER-03, VERIFIER-04, VERIFIER-05, VERIFIER-06, VERIFIER-07, VERIFIER-08, VERIFIER-09, VERIFIER-10)

### Phase 14: MLIR/Native Lowering Spike

**Goal**: Prove the first verifier-gated native-lowering boundary by accepting only a tiny Phase 13 `L2Core` bounded Int32 copy slice, rejecting unsupported programs fail-closed, emitting deterministic textual MLIR, and validating it with managed MLIR/LLVM tooling while keeping MLIR/LLVM/JIT out of `loom-core`/`loom-ffi` dependencies
**Depends on:** Phase 13
**Requirements:** LOWER-01, LOWER-02, LOWER-03, LOWER-04, LOWER-05
**Success Criteria** (what must be TRUE):

  1. A lowering contract and support predicate require an accepted `verify_l2_core` report plus present `VerifiedArtifactFacts`; standalone facts or rejected reports cannot lower
  2. Unsupported accepted programs fail closed with stable lowering diagnostics before any MLIR/native artifact is emitted
  3. The bounded Int32 copy sample emits deterministic textual MLIR using standard `func`, `arith`, `scf`, and `memref` dialect operations without adding mandatory `melior`, LLVM, or Cranelift dependencies
  4. Focused tests compare the supported slice against typed primitive reference output and cover negative range/capacity cases
  5. `scripts/native-lowering-test.sh` runs the focused gate and requires managed LLVM/MLIR validation by default; skip is allowed only by explicit `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`
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
**Ordering decision:** Promote the Phase 14 verifier-gated textual MLIR spike into a programmatic backend only after real Vortex artifact shapes are visible. Scope should remain verifier-gated and fail-closed, with `melior` kept isolated from `loom-core`/`loom-ffi` while LLVM/MLIR tools are managed externally.

**Research:** `.planning/phases/16-full-melior-llvm-jit-backend-integration/16-RESEARCH.md`

**Plans:** 5 plans across 5 waves

**Wave 1**

- [x] 16-01-PLAN.md - Toolchain contract and isolated backend crate boundary

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 16-02-PLAN.md - Programmatic melior module construction for bounded Int32 copy

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 16-03-PLAN.md - MLIR validation pipeline and managed backend gate

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 16-04-PLAN.md - MLIR ExecutionEngine/JIT execution and Rust reference equivalence

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 16-05-PLAN.md - Final docs, release-gate wiring, and roadmap/state closeout

**Cross-cutting constraints:**

- The backend remains verifier-gated and accepts only the Phase 14 bounded Int32 copy slice.
- `loom-core` and `loom-ffi` remain free of mandatory MLIR/LLVM/JIT dependencies.
- Missing or incompatible MLIR/LLVM fails gates by default; skip is permitted only by explicit `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`.
- Phase 16 must not claim custom Loom dialect, vectorization, DuckDB native execution, or complete Vortex reader support.
- Local Phase 16 evidence now uses managed LLVM/MLIR 22.1.7; compatible MLIR 22 environments are required for feature-enabled JIT evidence unless the explicit skip configuration is set.

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

**Status:** Complete (2026-06-08). See `.planning/phases/18-complete-vortex-reader/18-RESEARCH.md`, `18-CONTEXT.md`, `18-READER-CONTRACT.md`, `18-READER-REPORT.md`, `18-SUMMARY.md`, and plans `18-01` through `18-05`.
**Depends on:** Phase 15 and Phase 17; may consume constraints discovered in Phase 16.
**Ordering decision:** Expand from the supported non-null Int32 `.vortex` -> `LMC1` evidence slice to a complete Vortex reader boundary before engine-integrated native execution. Engine integration needs stable real artifact/fact/schema semantics; those should come from the full reader boundary rather than from the Phase 15 narrow ingress slice. Scope should include real file/container layout coverage, chunk/schema handling, representative encoding coverage, projection/statistics decisions, stable Loom-owned facts/diagnostics, and fail-closed behavior. It must not become a new query-engine integration phase.

**Research recommendation:** Complete reader means complete reader boundary, not arbitrary native decode of every Vortex encoding. Phase 18 should deliver recursive dtype/layout/segment/statistics facts, explicit accepted/unsupported/rejected support classification, `LMC1`/`LMT1` emission only for supported shapes, Phase 17 artifact verifier checks on emitted artifacts, and Vortex scan oracle evidence for every emitted fixture. `vortex-file` / `vortex-layout` APIs remain isolated to `loom-vortex-ingress`; `loom-core` and `loom-ffi` remain Vortex-free.

**Suggested plan split:**

- [x] 18-01-PLAN.md - Reader facts contract and dependency boundary
- [x] 18-02-PLAN.md - Recursive layout/dtype/segment inspection
- [x] 18-03-PLAN.md - Supported single-column conversion matrix
- [x] 18-04-PLAN.md - Supported struct/table conversion
- [x] 18-05-PLAN.md - CLI/report/release-gate closeout

### Phase 19: Solver-backed Full Artifact Verifier

**Status:** Complete (2026-06-08). See `.planning/phases/19-solver-backed-full-artifact-verifier/19-SOLVER-REPORT.md`, `19-SUMMARY.md`, and plans `19-01` through `19-05`.
**Depends on:** Phase 16, Phase 17, and Phase 18.
**Ordering decision:** Upgrade the Phase 17 unified artifact pipeline from collected obligations to solver-backed verifier evidence before production native expansion. Phase 18 must come first so the verifier targets real complete-reader facts instead of only the synthetic or narrow Phase 13/14/16 bounded copy slice. Scope should include a Z3/CVC5 or SMT-LIB strategy, symbolic offset/range/overflow obligation discharge, fail-closed unknown/unsupported obligations, stable external `L2Core` artifact codec/parser planning or implementation, solver-backed artifact reports, and `VerifiedArtifactFacts` that can be trusted by later native lowering only when obligations are discharged. It must not become production MLIR dialect work, native kernel expansion, or host-engine execution.

**Research recommendation:** Keep `loom-core` solver-neutral, emit deterministic SMT-LIB v2.7 scripts from Loom-owned obligation/report types, and add an optional `loom-solver-smt` backend crate whose command-line backend trait declares `z3`, `cvc5`, and `bitwuzla` from day one. Phase 19 implements Bitwuzla as the primary backend with a Bitwuzla-supported `QF_BV` required path; Z3/cvc5 remain optional adapters or strict cross-check paths, including possible `QF_LIA` alternate scripts. Treat `unsat` on negated bad-state queries as discharged evidence; treat `sat`, `unknown`, timeout, parse error, solver crash, missing strict solver, or cross-check disagreement as fail-closed. Phase 20+ must consume discharged facts, not `CollectedOnly` obligations.

**Suggested plan split:**

- [x] 19-01-PLAN.md - Solver contract and obligation report model
- [x] 19-02-PLAN.md - Deterministic Bitwuzla-primary SMT-LIB emitter
- [x] 19-03-PLAN.md - Optional `loom-solver-smt` crate with Bitwuzla backend
- [x] 19-04-PLAN.md - Artifact verifier solver-discharge integration
- [x] 19-05-PLAN.md - CLI, release gate, and solver verifier closeout

### Phase 20: Production Decode Dialect Seed and Raw Primitive Native Lowering

**Status:** Complete.
**Depends on:** Phase 16, Phase 17, Phase 18, and Phase 19.
**Ordering decision:** Preserve the first production lowering seed after verifier unification, complete-reader evidence, and solver-backed artifact verification. This phase introduced the `loom.decode` textual contract, Arrow/raw-buffer builder lowering, raw primitive multi-column lowering, and strict MLIR 22 validation evidence over a narrow primitive matrix. It is complete only as a seed: it does not claim a compiled C++/ODS dialect, a production `melior` pass pipeline, LLVM/JIT execution, host-engine integration, complete-reader work, or solver work.
**Research:** `.planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-RESEARCH.md`
**Context:** `.planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-CONTEXT.md`
**Report:** `.planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-NATIVE-LOWERING-REPORT.md`
**Summary:** `.planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-SUMMARY.md`

**Research recommendation:** Treat Phase 20 as the first production native-lowering surface seed, not a host-engine execution phase and not the full production backend. Define a `loom.decode` dialect contract and deterministic textual surface over a narrow primitive seed, require accepted artifact verification plus solver-backed `Discharged` facts before emission, lower to standard MLIR dialects for validation, and expand first through primitive Arrow/raw-buffer builders and multi-column primitive table slices. Default workspace builds must remain MLIR-free; strict native-lowering gates may require LLVM/MLIR 22. Move the real compiled C++/ODS dialect registration, `melior` pass pipeline, LLVM lowering, and LLVM/JIT execution backend to Phase 23 after Phase 22 locks the host runtime ABI/policy. This is not a claim that Phase 21 can widen encodings without touching lowering; each new encoding must declare whether it is interpreter-only, lowering-supported, or intentionally deferred with diagnostics.

**Suggested plan split:**

- [x] 20-01-PLAN.md - Production lowering contract and discharged-facts gate
- [x] 20-02-PLAN.md - `loom.decode` dialect contract and textual surface
- [x] 20-03-PLAN.md - Arrow raw-buffer builder lowering
- [x] 20-04-PLAN.md - Native kernel expansion for primitive multi-column slices
- [x] 20-05-PLAN.md - MLIR validation gate, report, and closeout

### Phase 21: Expanded Vortex Encoding Coverage

**Status:** Complete (2026-06-08). See `21-COVERAGE-MATRIX.md`,
`21-COVERAGE-REPORT.md`, and `21-SUMMARY.md`.
**Depends on:** Phase 18, Phase 19, and Phase 20.
**Ordering decision:** Widen real Vortex coverage after the verifier and first production lowering seed exist, not because that seed is permanently complete. This phase should add representative Vortex encodings, layouts, and storage modes beyond the Phase 18 accepted matrix, preserve Loom-owned facts and diagnostics, emit artifacts only when verifier/lowering facts accept them, and add Vortex oracle/equivalence gates. For every new encoding/layout, Phase 21 must record a paired decision: interpreter-only for now, production-lowering-supported with dialect/native delta, or fail-closed/deferred with stable diagnostics. It must not become solver work, host-runtime ABI work, production backend implementation, or DuckDB/Iceberg integration.
**Research:** `.planning/phases/21-expanded-vortex-encoding-coverage/21-RESEARCH.md`
**Context:** `.planning/phases/21-expanded-vortex-encoding-coverage/21-CONTEXT.md`

**Delivered plan split:**

- [x] 21-01-PLAN.md - Coverage matrix and reader fact contract
- [x] 21-02-PLAN.md - Nullable primitive and chunked primitive coverage
- [x] 21-03-PLAN.md - Dictionary, RunEnd, and sequence coverage
- [x] 21-04-PLAN.md - Bitpack, FOR, and numeric compression coverage
- [x] 21-05-PLAN.md - Report, release gate, and Phase 22/23 handoff

**Closeout:** Phase 21 added `VortexEncodingCoverage`,
`VortexEmissionDisposition`, and `VortexLoweringDisposition`; focused
nullable/chunked/dictionary/RLE/bitpack/FOR real Vortex tests; canonical raw
emission evidence where safe; fail-closed nullable/string/compression
deferrals; and `scripts/vortex-encoding-coverage-test.sh` wired into the
release gate. Structured native support for dictionary/run-end/bitpack/FOR
remains a Phase 23 backend delta.

### Phase 22: Host Native Runtime ABI and Execution Policy

**Status:** Complete (2026-06-08). See `22-RUNTIME-ABI-CONTRACT.md`,
`22-RUNTIME-ABI-REPORT.md`, and `22-SUMMARY.md`.
**Depends on:** Phase 17, Phase 18, Phase 19, Phase 20, and Phase 21.
**Ordering decision:** Define the engine-independent boundary before touching a host engine or committing to the production backend mechanics, while treating engine independence as a design claim until a second consumer proves it. This phase should lock the native callable ABI, artifact identity, verified-facts handoff, cache key, diagnostics, memory ownership, Arrow/raw-buffer output contract, predicate/projection pushdown contract, concurrency/reentrancy/thread-ownership model, fail-closed policy, and interpreter fallback semantics over complete-reader artifacts. It should not become a DuckDB, Iceberg, StarRocks, compiled dialect, or JIT implementation phase, and it should document which ABI choices remain DuckDB-shaped assumptions pending Phase 29 validation.

**Split research:** `.planning/research/ENGINE-INTEGRATION-SPLIT.md`
**Research:** `.planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RESEARCH.md`
**Context:** `.planning/phases/22-host-native-runtime-abi-and-execution-policy/22-CONTEXT.md`

**Delivered split:**

- [x] 22-01-PLAN.md - Runtime ABI contract and lifecycle model
- [x] 22-02-PLAN.md - Verified facts handoff and execution decision policy
- [x] 22-03-PLAN.md - Projection, predicate, and split planning envelope
- [x] 22-04-PLAN.md - Cache key, diagnostics, and C ABI sketch
- [x] 22-05-PLAN.md - Report, release gate, and backend handoff

**Closeout:** Phase 22 added `loom_core::runtime_abi`, a host-neutral runtime
contract, deterministic native/interpreter/fail-closed decision policy,
projection/predicate/split/concurrency planning, deterministic cache identity,
stable diagnostics, a non-frozen `loom_runtime.h` ABI sketch, and
`scripts/runtime-abi-test.sh` wired into the release gate. It does not implement
DuckDB native execution, StarRocks integration, Iceberg binding, production JIT,
or arbitrary Vortex semantic compatibility.

### Phase 23: Production Native Backend Implementation

**Status:** Complete (2026-06-08).
**Depends on:** Phase 22.
**Ordering decision:** Implement the real production backend after the ABI/policy is explicit and before any host engine depends on it. This phase moved beyond the Phase 20 textual seed by adding compiled `loom.decode` ODS evidence, a `melior`/LLVM validation pipeline, backend identity, cancellation, verifier-gated JIT seed evidence, strict toolchain/release gates, and primitive equivalence checks for the supported slice. It consumes the Phase 22 `RuntimePlan` and `RuntimeCacheKey` as mandatory backend inputs, keeps public `loom_runtime.h` unfrozen, and does not become DuckDB integration, cache hardening, Iceberg binding, or StarRocks comparison.
**Research:** `.planning/phases/23-production-native-backend-implementation/23-RESEARCH.md`
**Context:** `.planning/phases/23-production-native-backend-implementation/23-CONTEXT.md`
**Report:** `.planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md`
**Summary:** `.planning/phases/23-production-native-backend-implementation/23-SUMMARY.md`

**Success Criteria** (what must be TRUE):

  1. `loom-native-melior` exposes a host-neutral backend request/report model that requires Phase 22 `RuntimePlan` and `RuntimeCacheKey`.
  2. Backend identity records Loom ABI version, backend version, LLVM/MLIR toolchain identity, pass pipeline identity, capabilities, and target/layout evidence where available.
  3. The Phase 20 textual `loom.decode` surface has matching ODS/TableGen source evidence and drift checks without making LLVM mandatory for default builds.
  4. Production MLIR validation, LLVM lowering, and optional JIT preparation fail closed with stable backend diagnostics.
  5. Supported primitive native/JIT output has focused interpreter-equivalence evidence, while deferred encodings remain explicit.
  6. `scripts/production-backend-test.sh` is wired into the release gate and Phase 24 receives a DuckDB-adapter-shaped handoff.

**Plans:** 5 plans across 5 waves

**Wave 1**

- [x] 23-01-PLAN.md - Backend contract and runtime-plan bridge

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 23-02-PLAN.md - Compiled `loom.decode` ODS dialect evidence

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 23-03-PLAN.md - Production melior and LLVM lowering pipeline

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 23-04-PLAN.md - Verifier-gated JIT execution seed and interpreter equivalence

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 23-05-PLAN.md - Backend release gate, report, docs, and Phase 24 handoff

**Closeout:** Phase 23 complete. `scripts/production-backend-test.sh` now gates
the backend contract, ODS manifest, production pipeline, JIT seed, and strict
ODS validation when managed LLVM/MLIR tooling is available; it is wired into
`scripts/mvp0-verify.sh`. Supported native evidence is limited to verified
non-null primitive raw-buffer/table shapes and deterministic primitive JIT seed
output. Bitpack/FOR native execution, nullable validity copy, complex encodings,
persistent cache hardening, DuckDB execution integration, and arbitrary Vortex
semantics remain deferred.

### Phase 24: DuckDB Native Execution Integration MVP

**Status:** Complete (2026-06-08).
**Depends on:** Phase 23.
**Ordering decision:** Prove one concrete host integration before broadening the table story. DuckDB is the first host because the project already has a C++ table-function path and SQL smoke gates. This phase should consume `23-BACKEND-REPORT.md`, `23-BACKEND-CONTRACT.md`, and the Phase 22 runtime ABI report, then wire the Phase 22 runtime and Phase 23 production backend into `loom_scan`/DuckDB table-function execution over complete-reader artifacts. Select native only when verifier/native facts accept the program, fall back to the interpreter where policy allows, and preserve fail-closed diagnostics. The DuckDB layer must be a natural adapter over the Phase 22 runtime contract, not a second ABI: map DuckDB bind/init/local-init/function lifecycle to runtime/backend plan/scan/worker/next-batch, derive projection pushdown and max-thread behavior from runtime planning, and test Arrow C Data release plus error/cancel paths. It must not absorb Iceberg binding or StarRocks comparison.
**Goal:** DuckDB `loom_scan(path)` routes eligible complete-reader artifacts through the Phase 22 runtime policy and Phase 23 production backend while preserving interpreter fallback, fail-closed diagnostics, direct DataChunk output, projection evidence, and the existing public SQL surface.
**Requirements:** PHASE-24
**Plans:** 5/5 plans complete
Plans:
**Wave 1**

- [x] 24-01-PLAN.md - Internal Rust DuckDB runtime bridge and route policy tests

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 24-02-PLAN.md - Internal non-public DuckDB C ABI over opaque runtime/prepared handles

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 24-03-PLAN.md - DuckDB bind/global-init lifecycle routing over runtime/backend contracts

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 24-04-PLAN.md - Direct native/interpreter DataChunk output and single-batch scan behavior

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 24-05-PLAN.md - Route-aware DuckDB integration gate, release wiring, and final report

### Phase 25: Native Equivalence, Cache, and Fallback Hardening

**Status:** Complete (2026-06-09). See `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-NATIVE-HARDENING-REPORT.md` and `25-05-SUMMARY.md`.
**Depends on:** Phase 24.
**Requirements:** PHASE-25
**Ordering decision:** Harden the native execution path before making it table-format-visible. This phase should add oracle/equivalence matrices against interpreter/Vortex rows, native artifact cache reuse and invalidation semantics, unsupported-program negative coverage, deterministic diagnostics, performance smoke evidence, and release-gate wiring. It is the closeout for the engine-integrated native execution story, not a new query surface.
**Plans:** 5/5 plans complete

**Wave 1**

- [x] 25-01-PLAN.md - Runtime cache compatibility contract and stable policy diagnostics (PHASE-25)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 25-02-PLAN.md - Rust-owned in-process native preparation cache and internal cache diagnostics (PHASE-25)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 25-03-PLAN.md - Rust helper equivalence and unsupported-route negative matrices (PHASE-25)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 25-04-PLAN.md - DuckDB SQL native-hardening gate with cache smoke and fallback evidence (PHASE-25)

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 25-05-PLAN.md - Release-gate wiring, final native hardening report, and planning docs closeout (PHASE-25)

### Phase 26: External Source Ingress Contract

**Status:** Next active focus. Phase 25 has hardened the native execution contract; expand through research/planning before implementing source-specific bindings.
**Depends on:** Phase 18, Phase 21, and Phase 25.
**Ordering decision:** Abstract the proven Vortex ingress boundary before adding more source formats. This phase should define a source-neutral ingress contract for source facts, diagnostics, support classification, emission disposition, dependency isolation, verifier-routed `LMC1`/`LMT1` emission, oracle/equivalence evidence, and fail-closed unsupported/rejected behavior. It should reuse lessons from `loom-vortex-ingress` without exposing Vortex-specific types in the generic contract. It must not become Lance implementation, MCAP/Zarr/LeRobot support, Iceberg binding, or host-engine integration.

### Phase 27: Lance + Parquet Archival Readability / Dataset Ingress

**Status:** Complete. Phase 26 established the external source ingress contract; Phase 27 applied it to Lance and Parquet with verifier-backed current and actual older-version archival-readability evidence.
**Depends on:** Phase 26.
**Goal:** Supported local Lance datasets and Parquet files produce source-neutral facts, verifier-backed Loom artifacts, oracle/equivalence evidence, and current plus legacy archival-readability proof for the narrow non-null primitive/table slice.
**Requirements:** PHASE-27
**Ordering decision:** Make Lance and Parquet the first non-Vortex archival-readability targets because both are Arrow-adjacent columnar data sources and are close to Loom's successful output contract, but define the value as long-term readable artifacts rather than broad source-format compatibility. This phase should generate verifier-backed, long-lived Loom artifacts for supported Lance datasets and Parquet files so a platform can still read describable schema, Lance fragment metadata, Parquet row-group/page-adjacent metadata where supported, and column data years later without strongly depending on the original source reader version. The first slice has two required value proofs: current-version Lance and Parquet read/write plus Loom artifact verification, and older Lance/Parquet-version files carrying or paired with Loom artifacts that remain readable and rewritable for the supported schema/fragment-or-row-group/column subset. The isolated Lance and Parquet boundaries should extract source facts and diagnostics through the Phase 26 contract, emit verified `LMC1`/`LMT1` artifacts for supported Arrow-compatible primitive/table shapes, and record oracle/equivalence evidence against current source-reader and Arrow scan output. Deeper binding of Loom artifacts into Lance manifests, Lance indices, Parquet writer internals, object-store semantics, nested/extension types, or arbitrary source encodings should remain deferred until the archival-readability slice is proven. It must not add Iceberg binding, StarRocks/DuckDB dual-query work, MCAP/Zarr/LeRobot support, or arbitrary Lance/Parquet semantic compatibility.
**Plans:** 5/5 plans complete
Plans:

**Wave 1**

- [x] 27-01-PLAN.md - Adapter crate scaffolding and dependency/scope guards (PHASE-27)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 27-02-PLAN.md - Parquet fact extraction and source-ingress mapping (PHASE-27)
- [x] 27-03-PLAN.md - Lance fact extraction and source-ingress mapping (PHASE-27)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 27-04-PLAN.md - Verifier-routed Loom emission, oracle equivalence, and legacy readability fixtures (PHASE-27)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 27-05-PLAN.md - Release gate wiring, archival readability report, and closeout verification (PHASE-27)

### Phase 28: Full Lance + Parquet + Vortex Semantic Compatibility

**Status:** Complete. Reordered ahead of Iceberg and dual-query work so source-family semantic claims are bounded before table/ref binding and query-surface evidence consume them.
**Depends on:** Phase 21, Phase 23, Phase 25, Phase 26, and Phase 27.
**Ordering decision:** Semantic compatibility across Lance, Parquet, and Vortex should be explicit before Iceberg binding or multi-engine query claims. This phase records accepted, unsupported, rejected, canonicalized, and native-disposition rows across the existing source-ingress/readability evidence. It must distinguish original structured semantics from canonical raw/table emission, require oracle/verifier/native evidence before accepted claims, and fail closed on unsupported nullability, nested/logical/string/encoding cases. It must not become an Iceberg binding phase, StarRocks/DuckDB query-surface phase, or a second ABI design phase.

**Plans:** 5/5 plans complete

Plans:

- [x] 28-01-PLAN.md - Semantic compatibility matrix contract and row validation (PHASE-28)
- [x] 28-02-PLAN.md - Executable matrix drift/no-overclaim gate seed (PHASE-28)
- [x] 28-03-PLAN.md - Nullable primitive semantic compatibility closure or explicit deferral (PHASE-28)
- [x] 28-04-PLAN.md - Structured encoding semantics versus canonical raw evidence (PHASE-28)
- [x] 28-05-PLAN.md - Focused gate wiring, final report, release evidence, and milestone handoff (PHASE-28)

### Phase 29: Iceberg Ref/Table Binding

**Status:** Complete. Phase 29 established the adapter-local Iceberg binding crate, local metadata/sidecar fact parser, verifier/hash/source/oracle accepted binding path, fail-closed mismatch matrix, focused gate, main verifier wiring, and final evidence report without adding query or public API surfaces.
**Depends on:** Phase 18, Phase 21, Phase 25, Phase 26, Phase 27, and Phase 28.
**Goal:** Local Iceberg table/ref metadata can be bound to verifier-backed Loom artifacts through sidecar/reference evidence, preserving schema/snapshot identity, source/oracle evidence, and fail-closed verifier facts without adding query surfaces or a second source-ingress framework.
**Requirements:** PHASE-29
**Ordering decision:** Bind Loom artifacts to Iceberg reference/table metadata after source-family semantic compatibility is explicitly bounded and before adding more query surfaces. This phase should define how an Iceberg table/ref points at or carries Loom distribution artifacts, how schema/snapshot identity is represented, and how fail-closed verifier facts travel with table metadata. It must not become a StarRocks/DuckDB integration phase or a second source-ingress framework.
**Plans:** 5/5 plans complete
Plans:

**Wave 1**

- [x] 29-01-PLAN.md - Crate scaffold, binding data model, and dependency/scope guards (PHASE-29)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 29-02-PLAN.md - Local Iceberg metadata and sidecar fixture parsing with identity/facts extraction (PHASE-29)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 29-03-PLAN.md - Accepted binding validation with verifier, hash, source, and oracle evidence (PHASE-29)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 29-04-PLAN.md - Mismatch fail-closed matrix, fixture evidence, and binding report (PHASE-29)

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 29-05-PLAN.md - Focused gate wiring, main verifier order, and closeout evidence (PHASE-29)

### Phase 30: StarRocks + DuckDB Dual Query Surface

**Status:** Complete. Phase 30 is complete as a bounded dual query-surface proof: the adapter-local crate generates a Phase 29 accepted binding fixture, derives canonical rows/count/sum from verifier-accepted bytes, executes real DuckDB `loom_scan(path)` SQL for ordered rows, predicate, count, and sum, emits StarRocks-compatible offline descriptors over the same identity/evidence, fails closed on drift/unsupported features, and is wired into `scripts/mvp0-verify.sh`. Live StarRocks runtime smoke remains optional and non-canonical.
**Depends on:** Phase 29.
**Ordering decision:** After Iceberg binding exists, prove the same Loom-bound table artifacts can be consumed from both StarRocks and DuckDB query surfaces. This phase should compare integration seams and query behavior across the two engines, rather than inventing a second artifact format.
**Plans:** 5/5 plans complete.

Plans:

- [x] 30-01-PLAN.md - Adapter-local dual query-surface crate and boundary contract (PHASE-30)
- [x] 30-02-PLAN.md - Accepted binding query matrix and StarRocks-compatible descriptor validation (PHASE-30)
- [x] 30-03-PLAN.md - Executable DuckDB `loom_scan(path)` evidence and focused gate seed (PHASE-30)
- [x] 30-04-PLAN.md - Mismatch fail-closed matrix, scope guards, and optional StarRocks runtime-smoke handling (PHASE-30)
- [x] 30-05-PLAN.md - Focused gate wiring, final report, release evidence, and milestone handoff (PHASE-30)

Current tradeoff:

- DuckDB real execution is strong evidence for the shared Phase 29 accepted artifact and existing public `loom_scan(path)` surface.
- StarRocks-compatible evidence is deterministic offline descriptor/query evidence, not a default live runtime connector.
- Optional StarRocks runtime smoke can be run with explicit `STARROCKS_*` env inputs, but skipped runtime smoke is not accepted runtime evidence.

### Phase 31: Full Arrow Semantic Source Compatibility

**Status:** Complete. This phase supersedes the abandoned "core 80" nullable scalar increment and raises the target to full Arrow semantic compatibility for source schemas.
**Depends on:** Phase 26, Phase 27, and Phase 28. Phase 30 was not a blocker for this source-compatibility reset and is now complete as a bounded offline-descriptor dual-surface proof.
**Goal:** Any Lance or Parquet source that the upstream reader can materialize as Arrow, and any Vortex source/dtype that Vortex can materialize as Arrow, can be encoded into a verifier-backed Loom artifact, decoded back to Arrow, and compared for schema/value/null/metadata equality without relying on narrow `LMP1`/`LMT1` raw-layout coverage.
**Requirements:** PHASE-31
**Ordering decision:** Full source compatibility should be solved at the Arrow semantic layer before further query-engine or native-lowering claims. This phase introduces new `LMC2`/`LMA1` Arrow semantic artifacts and treats old `LMC1(LMP1/LMT1)` artifacts as legacy narrow payloads. It must not become a native-compiler phase, StarRocks runtime phase, direct Parquet Dremel decoder, direct Lance page decoder, or direct all-encoding Vortex physical decoder inside `loom-core`.
**Plans:** 6/6 plans complete.

Plans:

- [x] 31-01-PLAN.md - Contract reset, abandoned `NullableRaw` WIP cleanup, and Arrow semantic module scaffolding (PHASE-31)
- [x] 31-02-PLAN.md - `LMA1` Arrow semantic payload model, codec, verifier, and core roundtrip matrix (PHASE-31)
- [x] 31-03-PLAN.md - Parquet arbitrary Arrow schema semantic emission and equality tests (PHASE-31)
- [x] 31-04-PLAN.md - Lance arbitrary Arrow schema semantic emission, field metadata preservation, and equality tests (PHASE-31)
- [x] 31-05-PLAN.md - Vortex arbitrary DType semantic materialization and Arrow equality tests (PHASE-31)
- [x] 31-06-PLAN.md - Full compatibility gate, final report, docs, and no-overclaim release wiring (PHASE-31)

### Phase 32: MVP1 Architecture and Code Review

**Status:** Complete. Plans 32-01 through 32-05 completed claim, evidence, boundary, code-quality, and MVP1 readiness review artifacts. Phase 32 closed with a bounded MVP1 GO decision while preserving `LMC2`, arbitrary DuckDB `LMA1`, native `LMA1` execution, and live StarRocks runtime integration as explicit non-claims at that point; Phase 33 later implemented the `LMC2` wrapper, while arbitrary DuckDB Arrow semantic SQL and native Arrow semantic execution remain Phase 34/35 scope. Phase 30 later closed as a bounded offline-descriptor dual-surface proof.
**Depends on:** Phase 31. Phase 30 was reviewed as partial during Phase 32 and later completed as bounded DuckDB executable plus StarRocks-compatible descriptor evidence, not as a default live StarRocks connector.
**Goal:** Produce an end-to-end design and code review of the MVP1 implementation, covering artifact contracts, source-ingress semantics, DuckDB execution evidence, native/runtime claims, ABI/FFI boundaries, dependency isolation, release gates, documentation truthfulness, and concrete remediation items.
**Requirements:** PHASE-32
**Ordering decision:** Pause feature expansion after the MVP1 source compatibility and DuckDB source e2e gate so the project can separate proven value from scaffolding, fallback-only paths, and deferred claims. This phase should read the code and executable gates directly, classify findings by severity, update design/code review reports, and propose or apply narrow fixes only when a defect is unambiguous. It must not resume StarRocks integration, broaden DuckDB SQL support, redesign `LMA1`/`LMC2`, or expand native MLIR execution scope.
**Plans:** 5 plans

Plans:

- [x] 32-01-PLAN.md - Claim ledger and documentation truth audit (PHASE-32)
- [x] 32-02-PLAN.md - Execution evidence matrix and focused review audit gate seed (PHASE-32)
- [x] 32-03-PLAN.md - Architecture, ABI/FFI, and dependency-boundary audit (PHASE-32)
- [x] 32-04-PLAN.md - Code-quality review and narrow remediation (PHASE-32)
- [x] 32-05-PLAN.md - MVP1 go/no-go readiness report, audit gate finalization, and closeout (PHASE-32)

### Phase 33: LMC2 Arrow Semantic Container Wrapper

**Status:** Complete. Phase 33 implemented verifier-accepted `LMC2(LMA1)` wrapping, artifact-verifier/CLI visibility, source-ingress cutover, and release-gate coverage.
**Depends on:** Phase 31 and Phase 32.
**Goal:** Implement a versioned `LMC2` distribution wrapper for Arrow semantic `LMA1` payloads, with verifier routing, feature flags/section metadata as needed, CLI/report visibility, source-ingress emission updates, and release-gate coverage. Default source reports and new `lmc2` source entry points now produce `LMC2(LMA1)`; historical `lma1` source entry points remain explicit direct `LMA1` bridge evidence for legacy readability and regression checks.
**Requirements:** PHASE-33
**Ordering decision:** Resolve the artifact contract before broadening query-engine or native-execution claims. This phase should make `LMC2` a real verifier-accepted wrapper around Arrow semantic payloads, not a documentation-only future direction. It must not broaden DuckDB SQL shape support, claim native `LMA1` execution, or add live StarRocks runtime integration.
**Plans:** 5/5 plans executed

### Phase 34: DuckDB Arrow Semantic SQL Surface for LMC2(LMA1)

**Status:** Complete. Phase 34 adds DuckDB SQL over default `LMC2(LMA1)` artifacts for the staged primitive/nullable surface: one record batch, multiple named columns, Bool/Int32/Int64/Utf8/Float32/Float64, projection/filter/aggregate/null evidence, and direct `LMA1` regression bridge support. Date32 logical and Struct nested artifacts are verifier-encoded but rejected by DuckDB with stable unsupported diagnostics; native execution remains Phase 35 scope.
**Depends on:** Phase 33, plus Phase 31 and Phase 32 evidence.
**Goal:** Broaden DuckDB `loom_scan(path)` over default `LMC2(LMA1)` Arrow semantic artifacts: recognize the `LMC2` distribution wrapper, unwrap to the inner verifier-accepted `LMA1` payload, and scan Arrow semantic data through a staged surface.
**Requirements:** PHASE-34
**Ordering decision:** Query semantics should expand after the artifact wrapper decision is settled. This phase should redesign or extend the FFI/DuckDB adapter surface as needed instead of stretching the current one-column bind path. It must not claim native execution; interpreter-backed DuckDB correctness is sufficient unless Phase 35 has already supplied native Arrow semantic evidence.
**Scope split:** Completed positive SQL support for multi-column primitive and nullable Arrow semantic payloads. Logical Date32 and nested Struct are covered as fail-closed unsupported diagnostics and remain follow-up positive-support candidates.
**Plans:** 5/5 plans executed.

Plans:

- [x] 34-01-PLAN.md - Internal Arrow semantic DuckDB FFI contract for `LMC2(LMA1)` and direct `LMA1`
- [x] 34-02-PLAN.md - DuckDB adapter bind/init/scan support for default `LMC2(LMA1)`
- [x] 34-03-PLAN.md - Focused DuckDB LMC2 SQL gate and source e2e cutover to default artifacts
- [x] 34-04-PLAN.md - Logical/nested scope diagnostics for unsupported Arrow semantic SQL shapes
- [x] 34-05-PLAN.md - Release gate wiring, docs, final report, and closeout

### Phase 35: Native Arrow Semantic Execution

**Status:** Complete. Phase 35 adds engine-neutral native Arrow semantic execution for verifier-accepted `LMC2(LMA1)` and explicit direct `LMA1` one-batch nullable fixed-width primitive artifacts, with explicit native/reference equivalence, runtime/cache identity, focused and broad gate coverage, and fail-closed unsupported-shape diagnostics.
**Depends on:** Phase 33 and the native/runtime foundations from Phases 22-25. Consumes Phase 34 only for DuckDB integration evidence; native correctness remains engine-neutral.
**Goal:** Add true verifier-gated native execution for Arrow semantic payloads, including support predicates, native buffer semantics, native/reference equivalence gates, fail-closed diagnostics, runtime/cache identity, fallback policy evidence, and release-gate evidence for representative bounded Arrow semantic shapes.
**Requirements:** PHASE-35
**Ordering decision:** Native Arrow semantic execution should remain separate from DuckDB SQL broadening so the project does not confuse "queryable" with "natively executed." If native support starts before Phase 34 completes, it must stay engine-neutral and avoid DuckDB SQL claims. This phase must not count route scaffolding, zero/reference buffers, toolchain skip, or interpreter fallback as positive native semantic evidence.
**Plans:**

- [x] 35-01-PLAN.md - Engine-neutral native Arrow semantic executor for primitive nullable `LMC2(LMA1)` / direct `LMA1`
- [x] 35-02-PLAN.md - Explicit native/reference equivalence report and mismatch diagnostics
- [x] 35-03-PLAN.md - Runtime/cache identity evidence for native Arrow semantic execution
- [x] 35-04-PLAN.md - Focused Phase 35 gate and broad MVP1 release-gate wiring
- [x] 35-05-PLAN.md - Documentation, requirements, roadmap/state closeout

## Milestone: MVP1.5 — Verified Lineage

> **Status: active.** Supersedes the earlier parked Phase 36/37 by wrapping Lean Stage B/C as the middle stages of a six-phase lineage milestone. Phase 36 has now defined the `LINEAGE-01`/`LINEAGE-02` contract requirements; later `LINEAGE-*` IDs remain placeholders until their phases start.

**Thesis:** Turn the current Lean *structural projection* into a load-bearing, machine-checked soundness story that is *connected to the executor users actually run*. This milestone does **not** add features, formats, engines, or speed; it makes "verifier accepts ⟹ safe" true about the running system within a stated TCB, and makes each artifact carry an inspectable record of *why* it is trusted.

**Standing red line:** Loom guarantees *safety + well-formedness*, never *correctness*. Every "verified" claim must map to one named evidence layer (Rust verifier / Bitwuzla SMT / Lean soundness / differential validation / explicit TCB trust assumption). The Rust+C+++MLIR/LLVM toolchain gap stays in the TCB permanently and must never be silently closed by a productization claim.

**Critical-path coupling:** Phases 36/37/38/39 do **not** depend on MVP1 Phase 35. Only **Phase 40** depends on Phase 35.

### Phase 36: Verified-Lineage Contract and TCB Declaration

**Status:** Complete. Phase 36 created `36-VERIFIED-LINEAGE-CONTRACT.md`, defining "verified" as safety + Arrow well-formedness evidence lineage only, mapping each safety claim to exactly one evidence layer, declaring the TCB, assigning trust seams to Phase 37-40 or TCB, and closing LINEAGE-01/LINEAGE-02 without adding proofs or code.
**Goal:** A normative document that fixes the meaning of "verified" at MVP1.5 exit, an obligation matrix, and a TCB clause that lists every component trusted but not proven.
**Depends on:** MVP1 Phase 32 (review audit), Phase 19 (solver), current Lean scaffold (quick task 260609-lb2).
**Requirements:** LINEAGE-01, LINEAGE-02
**Success Criteria** (what must be TRUE):

  1. Each safety claim in scope maps to exactly one backing evidence layer: Rust verifier structural check, Bitwuzla discharge, Lean soundness theorem, differential validation, or named TCB trust assumption.
  2. The TCB clause explicitly lists: Rust compiler/std, LLVM + MLIR toolchain, the Rust↔C ABI seam, the DuckDB host process, and the Arrow C Data Interface — each with a one-line statement of *what is assumed* and *why it is not proven here*.
  3. The obligation matrix enumerates the three trust seams (Lean↔Rust verifier, static↔dynamic, modeled-executor↔real-executor) and assigns each to a later MVP1.5 phase or to the TCB.

**Non-goals:** No proofs, no code; defines boundaries only. Must not redefine the MVP1 safety story or weaken the "never correctness" red line.
**Ordering decision:** The word "verified" must be pinned before any proof work, or MVP1.5 risks the exact overclaim Phase 32 was created to police.
**Plans:** 1/1 plan executed.

Plans:

- [x] 36-01-PLAN.md - Verified-lineage contract, obligation matrix, TCB clause, requirements, and closeout

### Phase 37: Lean Stage B — Lean ↔ Rust Verifier Correspondence

**Status:** Complete. Phase 37 enriched `formal/lean/LoomCore.lean` with `ScalarExpr` / `LetScalar`, expression-derived append typing, scalar environment checks, and unknown-variable rejection, then wired a Lean/Rust correspondence gate that diffs accept/reject plus reject-code classifications over the current verifier matrix plus deterministic fuzz cases. **Supersedes** parked Phase 36.
**Goal:** The Lean model's static checkers faithfully mirror the executable Rust verifier, and the two are continuously cross-checked.
**Depends on:** Phase 36.
**Requirements:** LINEAGE-03, LINEAGE-04
**Success Criteria** (what must be TRUE):

  1. The Lean AST gains `ScalarExpr`/`LetScalar` so `builder_events_typed` derives value types from expressions exactly as the Rust verifier does, not from a flattened approximation.
  2. A Lean↔Rust differential harness runs the *current full fixture matrix plus a generated fuzz corpus* and reports **zero** accept/reject divergence, including matching reject codes (MissingInputCapability, MissingOutputBuilder, InvalidLoopBounds, NonMonotoneCursorLoop, ResourceBudgetExceeded).
  3. The differential harness is wired into the release gate and fails closed on any divergence or on any input the two checkers classify differently.

**Non-goals:** No operational semantics yet; no soundness theorem yet. Must not extend L2Core beyond what the Rust verifier already accepts.
**Ordering decision:** Correspondence before soundness — a soundness theorem over a Lean model that does not match the real verifier proves nothing about the product.
**Plans:** 2 plans complete. Wave 1: [x] `37-01-PLAN.md` AST enrichment + typing parity. Wave 2: [x] `37-02-PLAN.md` differential harness + gate wiring.

### Phase 38: Lean Stage C — Operational Semantics and Soundness Theorem

**Status:** Complete. Phase 38 added a proof-friendly modeled executor in Lean and re-proved `accepted_program_safe` as a no-`sorry` semantic theorem `Verified p -> ModeledExecutionSafe p`, scoped explicitly to the modeled executor. The theorem now includes the program-level bridge `verified_program_finishes` / `verified_program_reads_in_bounds`: `Verified p` implies `execProgram p` finishes, which rules out fail-closed execution before deriving all recorded reads in bounds; out-of-bounds reads remain representable as `inBounds := false` and fail-close unverified modeled runs; `checked_readInput_concrete_in_range` connects static read-input acceptance to the executor's concrete read-range predicate; and the full-verifier gate rejects `_state`/discarded-premise/direct-readSafety/all-reads-in-bounds invariant/read-boundary regressions. **Supersedes** parked Phase 37.
**Goal:** A small-step operational semantics over L2Core and a machine-checked theorem that verifier acceptance implies execution safety, scoped to the modeled executor.
**Depends on:** Phase 37.
**Requirements:** LINEAGE-05, LINEAGE-06
**Success Criteria** (what must be TRUE):

  1. A small-step operational semantics for L2Core is defined in Lean (read-input, append-value/null, bounded loops, fail-closed), 0 `sorry`.
  2. `accepted_program_safe` is re-proved as a *semantic* theorem: `Verified p` implies, for every input satisfying declared capabilities, that execution (a) never reads outside a declared input slice, (b) emits only builder events well-typed for their builder, (c) terminates within the `maxRows` budget, and (d) yields well-formed Arrow by construction.
  3. The theorem statement carries an explicit scope note: it holds over the *modeled* executor; the modeled↔real gap is delegated to Phase 39/40 and the TCB.

**Non-goals:** Not a proof about the Rust interpreter or native codegen (that is Phase 39/40 validation, not Lean proof). No correctness theorem.
**Ordering decision:** This is the phase that retires the "structural projection" finding — but only as far as the model; the model↔executor seam is the next phase.
**Plans:** 2 plans complete. Wave 1: [x] `38-01-PLAN.md` operational semantics. Wave 2: [x] `38-02-PLAN.md` soundness theorem + scope note.

### Phase 39: Model ↔ Rust Interpreter Consistency

**Status:** Complete. Phase 39 added a separate Rust reference executor, observer-only production trace subject, deterministic trace-level diff gate, and Lean extraction deferral note. **(Highest-leverage new phase.)**
**Goal:** Continuously validate that the real Rust interpreter — the actual safety path users run — matches a faithful transcription of the Lean operational semantics.
**Depends on:** Phase 38.
**Requirements:** LINEAGE-07, LINEAGE-08
**Success Criteria** (what must be TRUE):

  1. A Rust *reference executor* transcribes the Lean operational semantics one-to-one and is documented as the differential oracle (transcription, not the production interpreter).
  2. Across the supported matrix plus a fuzz corpus, the production Rust interpreter and the reference executor agree on the **full builder-event trace** (not only final values) and on fail-closed behavior; divergences fail the gate.
  3. (Optional, additive) Lean code extraction is evaluated as a stronger oracle path and either adopted or recorded as deferred with reason.

**Non-goals:** Does not *prove* interpreter = model (that would be verified compilation); it *validates* the correspondence per-run. Must not modify the production interpreter to match the model — divergence is a finding, not a fixup.
**Ordering decision:** This is the step the parked 36/37 lacked. Without it the soundness theorem holds over a model nobody runs. It reuses the project's existing differential/oracle infrastructure, so cost is low relative to value.
**Plans:** 2 plans complete. Wave 1: [x] `39-01-PLAN.md` reference executor. Wave 2: [x] `39-02-PLAN.md` trace-level differential gate.

### Phase 40: Native ↔ Model Validation

**Status:** Complete. Phase 40 validates native Arrow semantic output against Phase 39 reference-executor builder-event traces and decoded Arrow value equivalence for the full Phase 35 supported primitive matrix, requires successful validation for Phase 40 native route/cache eligibility, fails closed on divergence, and records MLIR/LLVM/native lowering as permanent TCB per-run translation validation rather than verified compilation.
**Goal:** Anchor native execution equivalence against the faithful model reference, not merely against the (itself-only-validated) interpreter.
**Depends on:** **MVP1 Phase 35** and Phase 39.
**Requirements:** LINEAGE-09, LINEAGE-10
**Success Criteria** (what must be TRUE):

  1. For every supported shape, native output matches the **reference executor's builder-event trace** (native ↔ model), upgrading Phase 35's native ↔ interpreter ↔ oracle value equivalence.
  2. The MLIR/LLVM lowering pipeline is recorded in the TCB as a permanent trust assumption: this phase is per-run translation validation, explicitly **not** verified compilation.
  3. Any native/model divergence fails closed and disables the native route for that shape until resolved.

**Non-goals:** No verified compilation of MLIR/LLVM (stays in TCB forever). No new encodings or formats.
**Ordering decision:** Native equivalence is only meaningful once there is a model to be equivalent *to*; hence after Phase 38/39 and after Phase 35 ships native.
**Plans:** 2 plans complete. Wave 1: [x] `40-01-PLAN.md` native↔model trace check. Wave 2: [x] `40-02-PLAN.md` fail-closed routing + TCB record.

### Phase 41: Verified-Lineage Closeout

**Status:** Complete. Phase 41 added the combined verified-lineage gate, artifact-facing lineage record API, focused tests, and public/planning docs that preserve the safety-provenance-only claim boundary.
**Goal:** One combined gate over all lineage evidence, and a per-artifact record that makes each artifact's safety provenance inspectable.
**Depends on:** Phases 36–40.
**Requirements:** LINEAGE-11, LINEAGE-12
**Success Criteria** (what must be TRUE):

  1. `scripts/verified-lineage-test.sh` runs, over the full matrix: Lean build (0 `sorry`), Lean↔Rust differential, model↔interpreter trace consistency, and native↔model validation — all fail-closed.
  2. Each emitted artifact can carry/produce a **verified-lineage record** naming the evidence layers establishing its safety (structural verifier, Bitwuzla discharge, Lean soundness, differential validation) and the TCB assumptions it relies on.
  3. Public/planning docs state precisely what "Verified Lineage" does and does not assert; no claim of end-to-end toolchain verification or of correctness.

**Non-goals:** Not signing/attestation transport (deferred; this record is the substrate). Not productization.
**Ordering decision:** Closeout last; the lineage record is the deliverable the milestone name promises and the input MVP2 attestation will bind to.
**Plans:** 2/2 plans complete. Wave 1: [x] `41-01-PLAN.md` combined gate. Wave 2: [x] `41-02-PLAN.md` lineage record + docs.

## Milestone: MVP2 — Coverage, Second Engine, Productization

> **Status: Phase 51 (ABI Freeze) ready to start.** Phase 42 is complete. Phase 43 completed its contract/gate/ABI-findings work but is suspended until a live StarRocks runtime/client is available; `ENGINE-01` is deferred to a pre-GA reactivation gate rather than blocking ABI freeze planning. Phases 43.1 and 43.2 realized and stabilized real production native codegen, so Phase 51 can freeze the ABI from production-readiness evidence rather than placeholder or first-pass JIT tests.

**Thesis:** Take the now-load-bearing verified core and make it (1) cover a real breadth of shapes, (2) realize and stabilize true native codegen for the supported Arrow semantic matrix before freezing ABI, (3) freeze and productize the ABI with every live-engine gap explicitly tracked, and (4) revalidate engine independence before GA once a genuine second consumer is available.

**Ordering rationale:** The MVP1 ABI was deliberately left *unfrozen* because it had only one consumer (N=1). MVP2 first widened coverage and attempted the second-engine runtime proof. Because the live StarRocks runtime is externally blocked, Phase 43's contract and ABI findings are retained. Before ABI freeze, Phase 43.1 must turn the bounded native Arrow semantic path into real MLIR/LLVM/JIT/native backend output, and Phase 43.2 must harden that path enough that Phase 51 freezes an ABI shaped by stable production behavior rather than first-pass green tests. `ENGINE-01` must still be reactivated before GA rather than silently waived.

### Phase 42: Verified + Native Coverage Expansion

**Status:** Complete. Phase 42 added a living verified/native coverage matrix for Vortex, Lance, and Parquet rows; native-supported rows require explicit native evidence, interpreter-only and canonicalized rows remain separate, and `scripts/verified-native-coverage-expansion-test.sh` plus `scripts/mvp2-verify.sh` gate the surface.
**Goal:** Widen the accepted matrix well beyond the MVP1 slice, with every new encoding/layout/format shape carrying an explicit paired disposition.
**Depends on:** MVP1.5 Phase 41 (so each new shape's safety is lineage-backed), MVP1 Phase 21/31.
**Requirements:** COV2-01, COV2-02, COV2-03
**Success Criteria** (what must be TRUE):

  1. The accepted matrix is widened with a target set of additional Vortex encodings/layouts and additional Lance/Parquet schema shapes; each addition records a paired disposition: verified-lineage-backed + native-supported, verified + interpreter-only, canonicalized, or fail-closed/deferred.
  2. Every newly accepted shape passes the MVP1.5 `verified-lineage` gate and the oracle row/aggregate equivalence gate; unsupported shapes still fail closed with typed diagnostics.
  3. Coverage growth is recorded as a living matrix with per-shape evidence, so the ABI freeze (Phase 51) can be taken against a known surface.

**Non-goals:** Not arbitrary "decode every Vortex encoding"; not engine work; not ABI freeze. New shapes that cannot be lineage-backed remain interpreter-only or deferred — never silently native.
**Ordering decision:** Coverage before ABI freeze, because new shapes stress ABI surface; a freeze taken before the matrix is known would freeze the wrong contract.
**Plans:** 3/3 complete. Wave 1: [x] `42-01-PLAN.md` Vortex coverage matrix. Wave 2: [x] `42-02-PLAN.md` Lance/Parquet coverage, [x] `42-03-PLAN.md` matrix closeout.

### Phase 43: StarRocks Live Runtime Integration

**Status:** Suspended / pending live runtime evidence. Plans 43-01 through 43-03 completed the typed runtime evidence contract, focused runtime gate, ABI findings report, and MVP2 gate wiring. Local contract mode passes, strict live mode fails closed without StarRocks env/client inputs, and no live StarRocks runtime query has been collected locally yet. `ENGINE-01` remains open and must be reactivated before GA, but it no longer blocks Phase 51 ABI-freeze planning.
**Goal:** Prove engine-independence empirically: a live, different query engine consumes the same accepted artifacts and returns equivalent results.
**Depends on:** Phase 42, MVP1 Phase 22 (ABI) and Phase 30 (StarRocks descriptor evidence).
**Requirements:** ENGINE-01, ENGINE-02, ENGINE-03
**Success Criteria** (what must be TRUE):

  1. A live StarRocks runtime queries Loom-bound/accepted artifacts (not just an offline descriptor) and returns row/aggregate results matching DuckDB and the oracle across the supported matrix.
  2. The integration surfaces every place the "engine-independent" ABI turned out to be DuckDB-shaped; each such finding is documented and either fixed in the still-unfrozen ABI or recorded as an accepted asymmetry before freeze.
  3. Fail-closed behavior holds in StarRocks: unsupported shapes are rejected, not silently mis-decoded.

**Non-goals:** Not freezing the ABI yet (this phase exists to *inform* the freeze). Not a third engine. Not productization.
**Ordering decision:** The second consumer remains the real falsifier for engine-independence, but it depends on external runtime availability. Rather than letting missing runtime tooling freeze the whole roadmap, Phase 43 is parked with fail-closed gates and ABI findings retained; the live runtime proof is a pre-GA reactivation requirement.
**Plans:** 3/3 complete, but phase completion is suspended pending live runtime evidence. Wave 1: [x] `43-01-PLAN.md` StarRocks runtime evidence contract. Wave 2: [x] `43-02-PLAN.md` cross-engine equivalence + fail-closed runtime gate, [x] `43-03-PLAN.md` ABI-shape findings + MVP2 wiring. See `43-SUSPENSION-NOTE.md`.

### Phase 43.1: Production Native Codegen Realization (INSERTED)

**Status:** Complete as of 2026-06-09.
**Goal:** 44-pre / 44A phase: for the full Phase 35 supported Arrow semantic matrix, produce Arrow buffers/RecordBatch through the real MLIR/LLVM/JIT/native backend path rather than the current Rust/Arrow copy placeholder; every execution must pass Phase 40 native/model validation, fail closed or fall back on divergence, and bind cache identity to backend/toolchain/pipeline/trace fingerprints.
**Requirements:** CODEGEN-01, CODEGEN-02, CODEGEN-03
**Depends on:** Phase 42 coverage, Phase 35 native Arrow semantic supported matrix, Phase 40 native/model validation, Phase 23 production backend/JIT seed, and Phase 43 ABI findings. Does not depend on closing suspended `ENGINE-01`.
**Success Criteria** (what must be TRUE):

  1. Supported `LMC2(LMA1)` and explicit direct `LMA1` one-batch nullable fixed-width primitive artifacts execute through real MLIR/LLVM/JIT/native backend codegen to produce Arrow buffers/RecordBatch output for the Phase 35 matrix, not through `reference_zeroed_value_bytes` or Rust/Arrow builder-copy stand-ins.
  2. Each native codegen execution is admitted only after Phase 40 native/model validation succeeds against reference executor trace and decoded Arrow value equivalence; validation failure, unsupported shape, toolchain mismatch, or output divergence fails closed or takes an explicit interpreter fallback path according to runtime policy.
  3. Runtime/cache identity includes artifact digest, verifier facts, backend identity, MLIR/LLVM toolchain identity, pass/pipeline identity, target/layout facts where available, and model/native trace fingerprints so Phase 51 can freeze an ABI informed by real native-codegen needs.

**Non-goals:** No verified compilation of MLIR/LLVM; those remain TCB. No broadening beyond the Phase 35 supported Arrow semantic matrix. No StarRocks live runtime claim. No ABI freeze; Phase 51 owns the frozen contract after this phase.
**Ordering decision:** This phase must precede ABI freeze. Freezing first would risk locking an ABI shaped by placeholders rather than by real native output buffers, validation admission, backend identity, and cache requirements.
**Plans:** 4 planned slices across 3 waves (Wave 1: codegen contract + supported-matrix lowering; Wave 2: JIT/native output + Phase 40 validation admission; Wave 3: cache/backend identity + release-gate closeout).

Plans:

- [x] 43.1-01-PLAN.md - Arrow semantic native-codegen contract, verifier-gated support classification, and real value/validity buffer extraction (CODEGEN-01, CODEGEN-03)
- [x] 43.1-02-PLAN.md - Production MLIR/LLVM/JIT output for typed primitive values, Boolean bitmaps, and nullable validity bitmaps (CODEGEN-01)
- [x] 43.1-03-PLAN.md - Phase 40 validation admission, fail-closed/fallback routing, and production codegen cache identity (CODEGEN-01, CODEGEN-02, CODEGEN-03)
- [x] 43.1-04-PLAN.md - Focused release gate, final native-codegen report, docs, and Phase 51 ABI-freeze handoff (CODEGEN-01, CODEGEN-02, CODEGEN-03)

### Phase 43.2: Production Native Codegen Stabilization and Production Readiness (INSERTED)

**Status:** Complete as of 2026-06-09. Plans 43.2-01 through 43.2-05 completed deterministic replay, shape-aware cache identity, production route closure, real JIT route evidence, fail-closed/fallback route negatives, adversarial output validation, bounded soak/resource/timing evidence, the focused stabilization gate, public docs, and the Phase 51 ABI-freeze dossier.
**Goal:** 44-pre stabilization phase: take the real Phase 43.1 MLIR/LLVM/JIT/native Arrow semantic path from "implemented and validated on the focused matrix" to "stable enough to freeze around." The phase must harden deterministic replay, production runtime routing, adversarial validation, resource/performance behavior, diagnostics, and ABI-pressure evidence before Phase 51 freezes any host/runtime contract.
**Requirements:** CODEGEN-STABLE-01, CODEGEN-STABLE-02, CODEGEN-STABLE-03, CODEGEN-STABLE-04, CODEGEN-STABLE-05
**Depends on:** Phase 43.1 production native codegen realization, Phase 40 native/model validation, Phase 35 supported Arrow semantic matrix, Phase 42 coverage surface, Phase 43 ABI findings, and MVP1 Phase 22 runtime ABI. Does not depend on closing suspended `ENGINE-01`.
**Success Criteria** (what must be TRUE):

  1. Real native-codegen executions are deterministic and replayable across repeated runs: generated MLIR/LLVM pipeline identity, toolchain identity, support-report fingerprints, output buffer digests, validation traces, and runtime/cache keys remain stable for unchanged inputs, while any artifact/schema/toolchain/pipeline/projection/predicate/split drift is detected before cache admission.
  2. The production runtime path, not only test-local helpers, exercises the Phase 43.1 JIT/native backend under the supported matrix with strict fail-closed behavior for unsupported shapes, cancellation, missing toolchains, validation divergence, cache mismatch, and output buffer corruption.
  3. Adversarial and boundary coverage stress the real native output contract: nullable/non-null columns, Boolean bitmaps, sliced buffers, zero-row and one-row batches, multi-column primitive matrices, invalid buffer lengths, wrong null counts, offset/bitmap drift, schema mismatch, trace divergence, and malformed `LMC2`/direct `LMA1` bridge inputs.
  4. Production readiness evidence records resource and performance behavior: repeated execution/soak, cache hit/miss/replay behavior, cancellation latency, memory ownership and release discipline, native-vs-interpreter timing on representative Phase 35 rows, and no accepted path that relies on `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`.
  5. A Phase 51 handoff dossier names the exact ABI pressures discovered by real native codegen: buffer ownership, alignment, lifetime, error/status codes, backend/toolchain identity fields, trace fingerprints, validation/fallback statuses, cache-key schema, observability fields, concurrency/reentrancy assumptions, and remaining TCB/non-claims.

**Non-goals:** No expansion beyond the Phase 35 supported Arrow semantic matrix except edge/boundary cases needed to stabilize that matrix. No ABI freeze; Phase 51 owns the frozen contract. No verified MLIR/LLVM compilation proof. No live StarRocks runtime claim. No performance promise beyond measured evidence for the bounded matrix.
**Ordering decision:** Phase 43.1 proved the path can produce real native buffers; Phase 43.2 must prove the path is stable, diagnosable, replayable, and production-routable before ABI freeze. Freezing immediately after first-pass codegen risks locking an ABI that has not yet absorbed failure modes, cache replay, lifecycle, observability, and resource behavior.
**Plans:** 5/5 complete. Five planned slices across 3 waves (Wave 1: determinism/replay + production route closure; Wave 2: adversarial validation + resource/performance hardening; Wave 3: production readiness gate + ABI-freeze dossier).

Plans:

- [x] 43.2-01-PLAN.md - Deterministic replay, toolchain/pipeline fingerprinting, and cache-key drift hardening (CODEGEN-STABLE-01, CODEGEN-STABLE-04)
- [x] 43.2-02-PLAN.md - Production runtime route closure for real JIT/native backend execution, fallback, cancellation, and cache admission (CODEGEN-STABLE-02, CODEGEN-STABLE-04)
- [x] 43.2-03-PLAN.md - Adversarial native output validation matrix for buffers, bitmaps, offsets, schemas, traces, and malformed artifacts (CODEGEN-STABLE-02, CODEGEN-STABLE-03)
- [x] 43.2-04-PLAN.md - Soak, resource ownership, memory/lifetime checks, and bounded native-vs-interpreter performance evidence (CODEGEN-STABLE-04)
- [x] 43.2-05-PLAN.md - Production readiness gate, diagnostics/observability contract, and Phase 51 ABI-freeze dossier (CODEGEN-STABLE-01, CODEGEN-STABLE-02, CODEGEN-STABLE-03, CODEGEN-STABLE-04, CODEGEN-STABLE-05)

### Phase 44: MVP1.5 Closeout and Milestone Archive

> **Status: PLACEHOLDER — not yet specced.** Stubbed to mark the MVP1.5 milestone closeout position. The MVP1.5 verified-lineage milestone (Phases 36-41) is complete; this phase archives its evidence and transitions fully into MVP2. Do not plan/execute until specced. Refine via `/gsd-spec-phase 44` → `/gsd-plan-phase 44`.

**Goal (provisional):** Archive MVP1.5 verified-lineage evidence, close the milestone formally, and transition to MVP2 ABI/distribution/GA work. (TBD — spec before planning)

**Depends on:** Phase 42 (verified + native coverage expansion), Phase 43.2 (production native codegen stabilization/readiness).

**Plans:** TBD (placeholder — spec before planning).

Plans:

- [ ] TBD (run /gsd-spec-phase 44, then /gsd-plan-phase 44 to break down)

### Phase 51: ABI Freeze and Compatibility Contract

**Status:** Not started. (Moved from original Phase 44.)
**Goal:** Freeze the host native runtime ABI with a versioned, documented compatibility policy.
**Depends on:** Phase 42 (coverage surface), Phase 43.1 (production native codegen realization), Phase 43.2 (production native codegen stabilization/readiness), MVP1 Phase 22 runtime ABI, and Phase 43's completed ABI-findings/contract work. Live StarRocks runtime evidence (`ENGINE-01`) is explicitly deferred to pre-GA reactivation and is not a Phase 51 entry blocker.
**Requirements:** ABI2-01, ABI2-02, ABI2-03
**Success Criteria** (what must be TRUE):

  1. `loom_runtime.h` / the C ABI is frozen at a v1: every symbol, struct, ownership rule, cache-key shape, native output buffer contract, backend/toolchain identity field, cache-key shape, projection/predicate contract, and concurrency/reentrancy guarantee is documented and versioned.
  2. A compatibility policy defines what changes are allowed within a major version (additive only) vs. require a major bump, with an ABI-version negotiation handshake at load time.
  3. An ABI conformance gate passes against the frozen header using DuckDB plus the Phase 43 StarRocks descriptor/runtime-evidence contract where live runtime is unavailable; ABI-drift guard fails closed on any unversioned change, and the live StarRocks conformance gap remains a named pre-GA blocker.

**Non-goals:** No new engines or formats. No distribution/security transport. The freeze does not claim toolchain verification (MLIR/LLVM stays in TCB).
**Ordering decision:** Freeze after the coverage matrix, Phase 43.1 real native codegen, Phase 43.2 production-readiness evidence, and Phase 43 ABI findings have shaped the contract, while preserving an explicit live-second-engine caveat. The freeze must not claim that StarRocks runtime evidence exists until `ENGINE-01` is reactivated and closed.
**Plans:** 2 plans across 2 waves (Wave 1: freeze + version handshake; Wave 2: conformance + drift gate).

## Out of Scope (deferred beyond MVP2 / permanent TCB)

- **Verified compilation of the MLIR/LLVM toolchain** — the modeled-executor ↔ real-native-codegen seam stays in the TCB permanently; MVP1.5/MVP2 use per-run translation validation, never a toolchain correctness proof. Closing it is CompCert-scale research, a future milestone at most.
- **Formal correctness** (values are the correct decoding of the input) — Loom proves safety + well-formedness only; correctness remains oracle-validated forever.
- **Third+ query engines, GUI, managed cloud service, multi-tenant SaaS** — adoption surface, not core proof; defer to MVP3+.
- **PKI / key-management product** — integrate an existing trust root; do not build one.

## Milestone dependency summary

```text
MVP1 (P1–35) ──> P35 native execution ─┐
                                        ├─> MVP1.5 P40 (native↔model)
MVP1.5 P36→37→38→39 (no P35 dep) ───────┘
MVP1.5 P36–41 ──> verified-lineage record ─┐
                                            ├─> MVP2 P42 (coverage, lineage-backed)
                                            └─> (attestation deferred)
MVP2: P42 coverage ──> P43.1 native codegen ──> P43.2 production stabilization ──> P51 ABI freeze
      P43 StarRocks ── suspended after contract/gate/ABI findings; ENGINE-01 reactivates before GA

Repositioning (整理稿): P48 kloom ──> P49 independent IR codec ──> P50.1 container demotion ──> P50 sidecar overlay
      (P48–50 run independent of MVP2 chain; P49 is the substrate for future artifact-level hash)
```

## Progress

**Execution Order:**
MVP1 phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8 -> 9 -> 10 -> 11 -> 12 -> 13 -> 14 -> 15 -> 16 -> 17 -> 18 -> 19 -> 20 -> 21 -> 22 -> 23 -> 24 -> 25 -> 26 -> 27 -> 28 -> 29 -> 30 -> 31 -> 32 -> 33 -> 34 -> 35

MVP1.5 (36–41) is complete. MVP2 (42–47 + 51) and Repositioning (48–50) are active with a non-linear dependency graph — see the "Milestone dependency summary" above.

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
| 18. Complete Vortex Reader | 5/5 | Complete | 2026-06-08 |
| 19. Solver-backed Full Artifact Verifier | 5/5 | Complete | 2026-06-08 |
| 20. Production Decode Dialect Seed and Raw Primitive Native Lowering | 5/5 | Complete | 2026-06-08 |
| 21. Expanded Vortex Encoding Coverage | 5/5 | Complete | 2026-06-08 |
| 22. Host Native Runtime ABI and Execution Policy | 5/5 | Complete | 2026-06-08 |
| 23. Production Native Backend Implementation | 5/5 | Complete | 2026-06-08 |
| 24. DuckDB Native Execution Integration MVP | 5/5 | Complete   | 2026-06-08 |
| 25. Native Equivalence, Cache, and Fallback Hardening | 5/5 | Complete    | 2026-06-09 |
| 26. External Source Ingress Contract | 5/5 | Complete    | 2026-06-09 |
| 27. Lance + Parquet Archival Readability / Dataset Ingress | 5/5 | Complete    | 2026-06-09 |
| 28. Full Lance + Parquet + Vortex Semantic Compatibility | 5/5 | Complete | 2026-06-09 |
| 29. Iceberg Ref/Table Binding | 5/5 | Complete    | 2026-06-09 |
| 30. StarRocks + DuckDB Dual Query Surface | 5/5 | Complete | 2026-06-09 |
| 31. Full Arrow Semantic Source Compatibility | 6/6 | Complete | 2026-06-09 |
| 32. MVP1 Architecture and Code Review | 5/5 | Complete | 2026-06-09 |
| 33. LMC2 Arrow Semantic Container Wrapper | 5/5 | Complete | 2026-06-09 |
| 34. DuckDB Arrow Semantic SQL Surface for LMC2(LMA1) | 5/5 | Complete | 2026-06-09 |
| 35. Native Arrow Semantic Execution | 5/5 | Complete | 2026-06-09 |
| 36. Verified-Lineage Contract and TCB Declaration | 1/1 | Complete | 2026-06-09 |
| 37. Lean Stage B — Lean↔Rust Verifier Correspondence | 2/2 | Complete | 2026-06-09 |
| 38. Lean Stage C — Operational Semantics + Soundness | 2/2 | Complete | 2026-06-09 |
| 39. Model↔Rust Interpreter Consistency | 2/2 | Complete | 2026-06-09 |
| 40. Native↔Model Validation | 2/2 | Complete | 2026-06-09 |
| 41. Verified-Lineage Closeout | 2/2 | Complete | 2026-06-09 |
| 42. Verified + Native Coverage Expansion | 3/3 | Complete (MVP2) | 42-03 complete |
| 43. StarRocks Live Runtime Integration | 3/3 | Suspended pending live evidence (MVP2; reactivates before GA) | - |
| 43.1. Production Native Codegen Realization | 4/4 | Complete (MVP2 44-pre) | 2026-06-09 |
| 43.2. Production Native Codegen Stabilization and Production Readiness | 5/5 | Complete (MVP2 44-pre) | 2026-06-09 |
| 44. MVP1.5 Closeout and Milestone Archive | 0/0 | Placeholder | - |
| 48. K Spec-Oracle Differential Gate Completion (方案 A) | 3/3 | Complete | 2026-06-10 |
| 49. Independent L2Core Decode IR Codec and Content-Hash Identity | 3/3 | Complete (Repositioning 决定一) | 2026-06-11 |
| 50.1. Container Demotion and Thin Host Adapters | 3/3 | Complete   | 2026-06-11 |
| 50. Sidecar Overlay Model and Host-Native Reader Fallback | 0/0 | Placeholder (Repositioning 决定二 slice 2) | - |
| 51. ABI Freeze and Compatibility Contract | 0/0 | Planned (MVP2) | - |

### Phase 48: K Spec-Oracle Differential Gate Completion (方案 A)

**Goal:** Under narrowed Plan-A scope, close the remaining kloom v4 spec-oracle differential gaps: typed `KOracleOutcome` with `ProducedTrace`/`SkippedRefereeAbsent`/`UnsupportedProgram` variants; krun-absent skip semantics (`LOOM_ALLOW_K_ORACLE_SKIP` env-gated, 30s timeout) that never block the production gate; garbled-output hard-fail; recursive unsupported-construct predicate for `Min`/`Max`/`Bytes`; per-shape native-route disable registry in `jit.rs` (pre-check fast-fallback, post-validation divergence hook gated by `oracle_skip_reason.is_none()`); strict skip convention in `kloom-diff.sh` and CI (no skip env var); skip-aware LLVM-backend feasibility script with findings doc; and `contrib/kloom` doc sync to v4 coverage and four-state taxonomy.
**Requirements**: TBD
**Depends on:** Phase 43.2 (production native codegen evidence) and the landed kloom v4 spec-oracle (commit 77d1bc4). Independent of Phases 44–47; may run before ABI freeze.
**Non-goals:** K never enters the production path or `loom-core`'s dependency graph beyond invoking external `krun` from test/CI harness code; native remains the only default execution route; no K reachability-logic proofs (future option); Lean `accepted_program_safe` is retained with minimal sync only; no correctness claims — safety/well-formedness and divergence detection only. Rust interpreter leg and extracting LLVM backend interpreter into production mode remain deferred indefinitely.
**Plans:** 3 plans

Plans:

- [x] 48-01-PLAN.md — Typed KOracleOutcome enum + ENOENT/timeout skip + garbled-output hard-fail + Min/Max/Bytes unsupported predicate in kloom_harness, threaded through native_arrow_semantic
- [x] 48-02-PLAN.md — Per-shape native-route disable registry in jit.rs (disable on K↔native divergence, fast-fallback pre-check, no cache seeding) + negative tests
- [x] 48-03-PLAN.md — Strict skip-convention wiring (kloom-diff.sh/CI/consistency script) + skip-aware LLVM-backend feasibility script & findings doc + contrib/kloom doc sync + STATE/ROADMAP closeout
- [x] 48-P1-PLAN.md — Real Min/Max K semantic rules: `EvalConst(min/max)` + `TypeOfMin/MaxCheck` in kloom.k v4; faithful Min/Max serialization in kloom_harness.rs; `test-013-min-max.kloom` + Rust skip-semantics tests
- [x] 48-P3-PLAN.md — Persistent cross-process disable store: `DisableStore` JSON with atomic temp-rename, `$XDG_CACHE_HOME` default, `LOOM_DISABLE_STORE_PATH` env override, load-on-init in `disabled_shapes_registry()`, unit tests in jit.rs
- [x] 48-P4-PLAN.md — Equivalence-class corpus generator: `loom-fixtures::corpus::CorpusBuilder` with schema shape × expr depth × stmt mix determinism, `include_min_max` toggle, valid `ResourceBudget`
- [x] 48-P5-PLAN.md — L2Core↔kloom.k↔Lean AST sync checklist: `scripts/l2core-sync-checklist.py` checking 22 mapped constructs across three artifacts + Phase 48 completion flags

### Phase 49: Independent L2Core Decode IR Codec and Content-Hash Identity

**Repositioning anchor:** This is the first slice of the Loom repositioning (整理稿) — **决定一: decode IR 与 container 分离**. The repositioning recasts Loom from a top-level distribution container into a *decode-IR sidecar* that parasitizes host formats (Parquet/Vortex/Lance). For that to be a physical fact rather than a concept, the L2Core decode IR must become an artifact that can be **independently serialized, independently hashed, independently verified, and independently distributed** — decoupled from any container. §8 item 1 names this "决定一的真正工作量" (the former Phase 17 deferred item). The complementary slice — **决定二: sidecar 叠加模型 + 回退宿主原生 reader** — is the announced follow-up (Phase 50) and depends on the IR identity landed here.

**Current-state gap (verified against codebase):** `L2CoreProgram` in `crates/loom-core/src/l2_core.rs` is a mature in-memory AST with **no `Serialize`/`Deserialize`, no dedicated codec, and no content-hash**. Containers (`LMC1`/`LMC2`/`LMP1`/`LMT1`/`LMA1`) bundle schema + payload + feature flags but **never the IR program**. The verifier (`full_verifier.rs` / `artifact_verifier.rs`) produces **ephemeral, in-memory `VerifiedArtifactFacts`** — the verified object has no serialized form and no stable identity. The IR's identity is therefore implicit and container-entangled. This phase makes it explicit and independent.

**Goal:** Give the L2Core decode IR a standalone, canonical, content-addressed identity that exists independently of every container format. Deliver (a) an **independent L2Core IR codec** — a deterministic, versioned, round-trippable serialization of `L2CoreProgram` (capabilities, `ResourceBudget`, body, feature sets) with its own magic/version, free of any `LMC*`/`LMP*`/`LMT*`/`LMA*` dependency; (b) a **content-hash identity** computed over the canonical codec bytes, so the same program always yields the same bytes and the same hash; and (c) **fail-closed independent verification** — the verifier accepts/rejects an IR program *parsed from its own codec bytes* (rejecting malformed/garbled/truncated input), so the verified object and the distributed object are byte-identical. After this phase, the formal-assurance assets (kloom v4 spec-oracle, Lean soundness model, Rust verifier) all anchor to one stable, hashable IR artifact regardless of how it is later packaged.

**Requirements**: `IRID-01` independent L2Core IR codec (magic/version, deterministic binary wire format, zero container dependency); `IRID-02` content-hash identity (FNV-1a over canonical bytes, stable across processes); `IRID-03` fail-closed parse-and-verify (`verify_l2_core_bytes` rejects malformed/truncated/bad-discriminant input before producing facts).

**Depends on:** Phase 48 (kloom v4 spec-oracle + the `scripts/l2core-sync-checklist.py` 22-construct L2Core↔kloom.k↔Lean sync — the codec must cover exactly that construct surface so the content-hash anchors the same object all three artifacts reason about). Also leans on Phase 36/41 verified-lineage, whose digest field currently holds an MD5 placeholder pending a real IR identity to bind. Independent of MVP2 phases; the IR-level content-hash here is the substrate that future artifact-level identity builds on (do not duplicate).

**Success Criteria** (what must be TRUE):

  1. `L2CoreProgram` round-trips through the independent codec byte-identically (encode → decode → re-encode is stable), the codec carries its own magic + version, and it links/serializes with **zero reference to any container codec** (`container_codec.rs`, `table_codec.rs`, `layout_codec.rs`, `arrow_semantic_codec.rs`) — proven by a dependency/visibility check, not just convention.
  2. Encoding is **canonical and deterministic**: identical programs produce identical bytes across processes/runs, so the content-hash over those bytes is a stable identity; differing programs produce differing hashes (no collisions across the Phase 48 equivalence-class corpus).
  3. Verification is **fail-closed on the wire form**: the verifier consumes IR *parsed from codec bytes* and rejects malformed/garbled/truncated/over-budget input with a typed error before any facts are produced; a valid program parsed from bytes yields the same `VerifiedArtifactFacts` as the in-memory AST path. The IR can be hashed, verified, and handed off as a freestanding artifact with no container present.

**Non-goals:** Not the sidecar overlay model or host-reader fallback — that is **决定二 / Phase 50** (mount on host, content-hash bind at column-chunk/fragment granularity, fail-closed → host native reader). No artifact-level signing/attestation/remote fetch (Phases 45/46); this phase delivers IR-level identity only. No new L2Core constructs and no expressiveness expansion — the codec covers exactly the existing verified surface (the 22 synced constructs); the core-bet "列式 decode 窄域里 total-function 够用" is validated elsewhere, not widened here. Containers (`LMC1`/`LMC2`/`LMA1`) are **not deleted** in this phase — their demotion to an optional out-of-TCB lineage section + a dev-time canonical reference packaging is the broader reframe (§9), staged after the IR is independently real. No Wasm track and no SMT (stay removed, per "安全来自限制"). No correctness claims — safety + well-formedness + stable identity only; correctness stays oracle-validated.

**Ordering decision:** First slice of the repositioning because every later piece depends on it: the sidecar overlay (Phase 50) binds a host data range to *an IR identity*, the three thin host adapters (§8 item 4) each bind their host's bytes to *the same one IR*, and the assurance stack must anchor to *one stable hashable artifact*. Without an independent codec + identity, "decode IR 与 container 分离" remains conceptual — so it must land before sidecar, adapters, or container demotion.

**Plans:** 3 plans

Plans:

- [x] 49-01 — Independent L2Core IR codec (`l2core_codec.rs`): custom binary format with `L2IR` magic + `u16` version, little-endian fixed-width primitives, length-prefixed strings/vectors, `u8` enum discriminants for `Capability`/`ScalarValue`/`ScalarExpr`/`L2CoreStmt`, narrow `DataType` subset encoding (Boolean/Int32/Int64/Float32/Float64/Utf8), zero import of `container_codec`/`table_codec`/`layout_codec`/`arrow_semantic_codec`
- [x] 49-02 — Content-hash identity: `L2CoreProgram::content_hash()` → `l2ir:<hex>` via FNV-1a over canonical codec bytes; deterministic encode-decode-reencode proven byte-identical; diverse-program corpus collision-free
- [x] 49-03 — Fail-closed parse-and-verify: `verify_l2_core_bytes` in `full_verifier.rs` decodes then verifies, returning `ExplicitFailClosed` diagnostic on any `L2CoreCodecError` (bad magic, unsupported version, truncated payload, bad discriminant); valid bytes yield identical `VerifiedArtifactFacts` to in-memory AST path

### Phase 50.1: Container Demotion and Thin Host Adapters

> **Status: PLACEHOLDER — not yet specced.** First slice of Decision Two: before building the sidecar overlay, the old top-level container artifacts must be demoted and the existing ingress crates must degrade into thin adapters. Refine via `/gsd-spec-phase 50.1` → `/gsd-plan-phase 50.1`.

**Repositioning anchor:** Part of **决定二** (§9 reconciliation): the repositioning doc mandates that `LMC2`/`LMA1` be demoted from top-level format to two things — (1) an optional out-of-TCB Loom verification + lineage section hung inside host packaging, and (2) a dev-time canonical reference packaging for tests. The three existing ingress crates (`loom-vortex-ingress`, `loom-parquet-ingress`, `loom-lance-ingress`) must degrade into **thin host adapters** that only mount/extract the sidecar overlay and bind the content-hash to host data — never a second IR, never a second decode path (§8 item 4: 一段 IR + 三个薄适配器).

**Goal (provisional):** (a) Demote `LMC2`/`LMA1` from public top-level artifact to an optional lineage section + dev-time reference packaging; the packaging layer is out-of-TCB. (b) Degrade the three ingress crates into thin host adapters: each adapter handles only "how to mount into / extract from the host + how to bind the hash to host data," never reimplementing decode logic. After this phase, the L2Core IR (Phase 49) is the sole in-TCB artifact; all packaging is out-of-TCB and swappable.

**Depends on:** Phase 49 (independent L2Core IR codec + content-hash identity — the IR must exist as an independent artifact before containers can be demoted and adapters can bind to it). Relates to Phases 26–31 ingress crates (which are the sources being degraded).

**Success Criteria** (to be firmed in spec):

  1. `LMC2`/`LMA1` are demoted: they are no longer the default top-level distribution artifact; they exist only as an optional lineage evidence section and a dev-time reference packaging for kloom/verifier tests.
  2. The three ingress crates are degraded to thin adapters: each adapter's public API is "mount host file → extract sidecar bytes + bind content-hash to host data range"; no decode logic lives in the adapter.
  3. All existing release gates and tests pass with the demoted containers (backward compat); the dev-time reference packaging round-trips through kloom differential and verifier tests.

**Non-goals:** No sidecar overlay contract yet (Phase 50). No hash-binding granularity decisions yet (Phase 50). No fallback routing yet (Phase 50). No Wasm track. No new host formats beyond the existing three (Parquet/Vortex/Lance). No correctness claims.

**Ordering decision:** Container demotion must precede the sidecar overlay (Phase 50) because the sidecar overlay replaces the old container as the distribution model — you cannot build a new packaging model on top of old containers that are still "top-level."

**Plans:** 3/3 plans complete

**Wave 1**

- [x] 50.1-01-PLAN.md — Container demotion in loom-core: LMC2/LMA1 codec docs marked out-of-TCB; ArtifactVerificationFacts gains tcb_status + artifact_role fields; deprecation annotations on SourceIngressAcceptedArtifact; backward-compat comments in native_arrow_semantic
- [x] 50.1-02-PLAN.md — Ingress crate degradation: remove emit_source_ingress_lmc2_from_* from all 3 adapters; gate oracle batch functions to #[cfg(test)]; add extract_sidecar_bytes_from_* + bind_content_hash_to_* stubs; update lib.rs re-exports

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 50.1-03-PLAN.md — Release gate and test adaptation: update source-ingress-contract-test.sh, lmc2-arrow-semantic-container-test.sh, complete-vortex-reader-test.sh; adapt 10 ingress test files to use cfg(test)-gated oracle + dev-time packaging; run full release gate for backward-compat verification

### Phase 50: Sidecar Overlay Model and Host-Native Reader Fallback

> **Status: PLACEHOLDER — not yet specced.** Second slice of Decision Two: the sidecar overlay contract itself, building on demoted containers (Phase 50.1) and the independent IR identity (Phase 49). Do not plan/execute until Phase 50.1 lands. Refine via `/gsd-spec-phase 50` → `/gsd-plan-phase 50`.

**Repositioning anchor:** Second slice of the Loom repositioning (整理稿) — **决定二: 参考 AnyBlox(思想参考、工程独立)、无 Wasm 回退、回退即宿主原生 reader**. Where Phase 49 makes the decode IR an independent, hashable artifact and Phase 50.1 demotes containers/degrades ingress to thin adapters, this phase makes Loom a *sidecar overlay* on host formats rather than a top-level format: a Loom-aware engine takes the verifiable native track; everything else falls back to the host's own native reader. Single execution track (Loom 原生 — 可验证 + 宽向量 + 64 位); **no Wasm fallback** (§2.2), **no second IR execution implementation**, **no equivalence-diff burden**.

**Goal (provisional):** Establish the sidecar contract so a Loom artifact rides *on top of* an unmodified host file (Parquet/Vortex/Lance) as a strippable overlay, with content-hash binding the host data at column-chunk/fragment granularity to the Phase 49 IR identity, and a fail-closed decision: **(integrated engine ∧ hash matches ∧ encoding supported) → Loom verifiable-native track; otherwise → host's own native reader** (§2.3, §3). Container demotion and thin-adapter degradation are Phase 50.1 prerequisites.

**Core discipline to hold (§2.3 前提):** Loom must be **叠加而非替换** — a host file carrying a Loom sidecar must still be readable as ordinary Parquet/Vortex/Lance by an engine with no Loom. The overlay is strippable; data is never re-encoded into a Loom-only form, or "fall back to host native reader" breaks.

**Candidate success criteria** (to be firmed in spec):

  1. A Loom sidecar mounts on an unmodified host file and is fully strippable — a Loom-unaware engine reads the host file unchanged (叠加-not-替换 proven by reading the same file through a vanilla host reader).
  2. Content-hash binds host data to the Phase 49 IR identity at column-chunk/fragment granularity; an independent rewrite of the host invalidates only the affected granule's sidecar, the rest still accelerates; verification cost does not cancel the native speedup (§8 item 3).
  3. Fail-closed routing is exhaustive and honest: integrated + hash-match + supported → verifiable-native; any miss (no Loom / hash mismatch / unsupported encoding) → host-native reader, zero risk to the host user (worst case: the sidecar is ignored).

**Depends on:** Phase 50.1 (container demotion + thin adapters — the sidecar needs the packaging layer cleared before the overlay contract is built) and Phase 49 (independent L2Core IR codec + content-hash identity — the sidecar binds host granules to *that* identity).

**Non-goals:** No Wasm track (rejected, §2.2 — browser is a pseudo-need; cross-arch is solved by LLVM; sandbox contradicts verifiable safety; no AnyBlox free-ride). No second IR execution / no equivalence differential. No new top-level user-facing container competing for adoption — `LMC2`/`LMA1` demote to an optional out-of-TCB lineage section + a dev-time canonical reference packaging (§9). No host PKI/key-management product. Core IR is designed server-side-optimal — degradation is the fallback side's responsibility, never a constraint pushed back onto the IR (§8 item 5). No correctness claims — verifiable safety + well-formedness + graceful degradation only.

**Host priority (§7, data-decided, not pre-bet):** Parquet first and deepest (oldest encodings → clearest incremental value), Vortex next (already-advanced encodings → Loom adds safety/native-execution, not compression), Lance a question mark (random-access vs sequential decode IR). "回退=宿主原生 reader" makes mounting any host zero-risk, so priority is decided by real usage, not a symmetric upfront bet.

**Plans:** TBD (placeholder — spec before planning).

Plans:

- [ ] TBD (run /gsd-spec-phase 50, then /gsd-plan-phase 50 to break down)
