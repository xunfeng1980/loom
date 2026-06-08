---
gsd_state_version: 1.0
milestone: v1.5.3
milestone_name: milestone
status: executing
stopped_at: Phase 23 23-01 complete; execute 23-02 compiled loom.decode ODS dialect evidence next
last_updated: "2026-06-08T14:58:03.000Z"
last_activity: 2026-06-08 -- Phase 23 23-01 backend contract/runtime-plan bridge complete
progress:
  total_phases: 28
  completed_phases: 22
  total_plans: 92
  completed_plans: 88
  percent: 80
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-06-08)

**Core value:** A user can run a SQL query in DuckDB over Loom-decoded Vortex-style payloads, including a mixed-column table payload, and get expected row/aggregate results.
**Current focus:** Phase 23 — Production Native Backend Implementation (23-02 next)

## Current Position

Phase: 23 (Production Native Backend Implementation) — EXECUTING
Plan: 2 of 5
Status: 23-01 complete; execute 23-02 compiled `loom.decode` ODS dialect evidence next
Last activity: 2026-06-08 -- Phase 23 23-01 backend contract/runtime-plan bridge complete

Progress: 80%

## Progress Snapshot

- Completed phases: 22 / 28
- Completed executable plans: 88 / 92
- Current milestone stage: MVP1 / v3 distribution and verification track
- Current position: Phase 23 production native backend implementation is executing; 23-02 is next
- Last verified gate: Phase 22 focused gate passed; `scripts/runtime-abi-test.sh` is wired into `scripts/mvp0-verify.sh`

**Completed phase plan counts:**

| Phase range | Scope | Plans complete |
|-------------|-------|----------------|
| 1-5 | Original MVP0 DuckDB demo path | 12/12 |
| 6-10 | MVP0 hardening, DX, tables, verifier, ALP coverage | 19/19 |
| 11-15 | MVP1/v3 distribution, safety proof, full verifier foundation, native-lowering spike, real ingress | 21/21 |
| 16 | Optional verifier-gated melior/LLVM/JIT backend evidence | 5/5 |
| 17 | Unified artifact verification pipeline | 5/5 |
| 18 | Complete Vortex reader | 5/5 |
| 19 | Solver-backed full artifact verifier | 5/5 complete |
| 20 | Production decode dialect/native kernel expansion | 5/5 complete |
| 21 | Expanded Vortex encoding coverage | 5/5 complete |
| 22 | Host native runtime ABI and execution policy | 5/5 complete |
| 23 | Production native backend implementation | 1/5 executing |

Historical per-plan timing estimates were removed because they had drifted from the frontmatter and were no longer a reliable planning signal.

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- Roadmap: 5-phase structure adopted — dependency chain (FFI → DuckDB scaffold → L1 core → L1 remainder + L2 escape → FSST + verify) is load-bearing and cannot be reordered
- Roadmap: DUCK-04 (catch_unwind) assigned to Phase 1 alongside CORE-02 (panic=abort) — both are FFI panic-safety invariants that must precede any C++ calls
- [Phase ?]: Toolchain pinned to 1.92.0 not 1.87.0 — vortex-array 0.74.0 requires MSRV 1.91.0
- [Phase ?]: vortex-dict removed from deps — crate does not exist at 0.74.0; dict encoding via vortex-array 0.74.0
- [Phase ?]: [patch.crates-io] removed — exact version pins achieve arrow unification without invalid self-patch
- [Phase 1 P02]: loom_decode signature locked — i32 return code, no loom_free, Arrow release callback owns teardown
- [Phase 1 P02]: LoomError codes: NullPointer=1, DecodeFailed=2, Panicked=3
- [Phase 1 P02]: cbindgen excludes FFI_ArrowArray/FFI_ArrowSchema — incomplete-type pointer in loom.h prevents ABI struct mismatch
- [Phase 1 P02]: panic sentinel uses thread_local Cell<bool> for test isolation (not global AtomicBool)
- [Phase ?]: macro path used, no manual fallback
- [Phase ?]: D-01 honored: OneShotStream + produce-callback factory delegating to arrow_scan
- [Phase ?]: n_buffers==2, buffers[0]=validity, buffers[1]=int32 values confirmed by Rust test
- [Phase 2 P02]: Direct DataChunk population used in Phase 2 LoomScan — loom_decode returns bare Int32 schema (format=i), not struct schema arrow_scan requires; D-01 arrow_scan delegation is Phase 3+ work
- [Phase 2 P02]: ArrowStreamParameters forward-declared in duckdb namespace — internal type not in amalgamated header
- [Phase 2 P02]: Footer fields confirmed: duckdb_version=v1.5.3, platform=osx_arm64, abi_type=CPP; correct null.txt path used
- [Phase 3 P01]: FrameOfReference.reference stored as i128 (not i64) to handle u64 columns without truncation
- [Phase 3 P01]: unpack_all returns Vec<u64> (unsigned); callers apply wrapping_add of FOR reference after (Pitfall 4)
- [Phase 3 P01]: OutputBuilder::t_bits() drives both unpack_all t_bits and emit-width — builder is single authority for type width
- [Phase 3 P01]: Array trait must be explicitly imported in arrow-rs 58.3 for .into_data() and .is_null() on PrimitiveArray<T>
- [Phase ?]: BufferHandle .as_host().as_ref() (option A) confirmed for packed bytes access
- [Phase ?]: FoR+BitPack: use FoR::try_new(bp.into_array(), ref) with manual deltas, not FoRData::encode
- [Phase ?]: BitPackedArrayExt::validity explicit UFCS avoids ArrayRef::validity ambiguity

### Pending Todos

None yet.

### Blockers/Concerns

- ArrowArrayStream remains deferred after Phase 8. Current evidence favors direct DataChunk population because the existing FFI emits bare Arrow column arrays and the direct path supports mixed table payloads without widening the ABI.

### Roadmap Evolution

- Phase 9 complete: Verifier and Safety Boundary MVP implemented with typed diagnostics, decode/FFI/CLI routing, negative release gate coverage, and docs.
- Phase 10 complete: ALP Float32/Float64 has core kernel support, fixtures and Vortex primitive oracle comparisons, FFI roundtrips, CLI inspect/decode output, DuckDB SQL smoke coverage, documentation, and full release-gate verification.
- Phase 11 research complete: recommended `LMC1` container v0 with magic/version, required/optional feature flags, checked section directory, `LMP1`/`LMT1` compatibility wrappers, verifier/CLI visibility, and negative release-gate coverage.
- Phase 11 planned: 4 plans across core container codec, Rust verifier/decode/FFI routing, CLI/fixtures/DuckDB smoke support, and docs/final gates.
- Phase 11 executing: 11-01 complete with `LMC1` encode/decode, feature bitsets, checked section directory, and `LMP1`/`LMT1` wrapper helpers.
- Phase 11 executing: 11-02 complete with `verify_container`, container-aware Rust decode helpers, and `loom_decode` support for single-column `LMC1` containers without C ABI changes.
- Phase 11 executing: 11-03 complete with CLI inspect/decode support, generated `LMC1` smoke fixtures, DuckDB bind support, and container-aware negative verifier scripting.
- Phase 11 complete: `LMC1` Distribution Container v0 now has docs, generated fixture coverage, DuckDB SQL smoke coverage, malformed-container release-gate coverage, and DIST-01 through DIST-05 closed.
- Phase 12 complete: current-boundary Safety Proof MVP has a safety contract, proof-obligation matrix, focused no-panic/fail-closed tests, final proof docs, and a release-gated safety proof script for the implemented `LMC1`/`LMP1`/`LMT1` byte-to-Arrow boundary only.
- Phase 13 entered: selected full-verifier architecture combines Rust abstract interpretation/type-effect checking, SMT local obligations, Lean/Rocq semantics scaffolding, and TLA+ lifecycle/pipeline invariants.
- Phase 13 executing: 13-01 complete with normative `L2Core` verifier spec and proof-obligation matrix.
- Phase 13 executing: 13-02 complete with Rust `L2Core` model, SMT-ready constraint IR, `VerifiedArtifactFacts`, and focused model tests.
- Phase 13 executing: 13-03 complete with executable Rust `verify_l2_core`, stable diagnostics, proof-obligation traces, facts emission, tests, and `loom verify-l2core --sample`.
- Phase 13 executing: 13-04 complete with Lean `LoomCore.lean` scaffold, TLA+ `LoweredImpliesVerified` lifecycle model, and `scripts/full-verifier-test.sh`.
- Phase 13 evidence caveat: the current Lean file compiles as a scaffold, but `builder_events_typed` and `no_ambient_authority` are `True` placeholders; `accepted_program_safe` is not load-bearing safety evidence. Current load-bearing verifier evidence is Rust executable verification plus Phase 19 Bitwuzla-backed SMT discharge.
- Phase 13 complete: verifier foundation closed with final report, public/planning docs, `scripts/full-verifier-test.sh` wired into `scripts/mvp0-verify.sh`, and VERIFIER-01 through VERIFIER-10 marked complete. Phase 14 consumed the verifier handoff for lowering planning.
- Phase 14 research complete: recommended verifier-gated textual MLIR first, MLIR toolchain validation second, and no mandatory MLIR/LLVM workspace dependency during the initial spike.
- Phase 14 planned: 4 plans across lowering contract/support predicate, textual MLIR emission, supported-slice equivalence gate, and final docs/release-gate closeout.
- Phase 14 complete: verifier-gated support predicate, deterministic textual MLIR for bounded Int32 copy, typed primitive equivalence evidence, managed `mlir-opt` gate, final report, docs, and release-gate integration are complete.
- Phase 15 moved from roadmap placeholder to research: real Vortex file/container ingress before production native backend work.
- Phase 15 research started: recommended an isolated real Vortex ingress bridge, scoped `vortex-file` allowlist, Loom-owned `VortexFileFacts`, fail-closed diagnostics, and one narrow supported `.vortex` -> `LMC1` slice before Phase 16 JIT work.
- Phase 15 planned: 4 plans across ingress contract/dependency boundary, real Vortex metadata facts, supported real `.vortex` -> `LMC1` conversion, and CLI/docs/release-gate closeout.
- Phase 15 complete: `loom-vortex-ingress` isolates real `vortex-file` use, emits stable `VortexIngressReport` / `VortexFileFacts`, inspects real buffers/paths fail-closed, emits one non-null Int32 `.vortex` -> `LMC1` slice, exposes `loom ingest-vortex`, and wires `scripts/vortex-ingress-test.sh` into the release gate.
- Phase 16 research started after Phase 15 real ingress evidence: full `melior`/LLVM/JIT backend integration remains optional, verifier-gated, and fail-closed.
- Phase 16 research started: recommended an optional `loom-native-melior` backend crate, explicit LLVM/MLIR toolchain probing, verifier-gated programmatic MLIR construction, optional MLIR ExecutionEngine/JIT evidence, and fail-closed rejection before native artifact creation.
- Phase 16 planned: 5 sequential plans across optional backend crate/toolchain contract, programmatic `melior` module construction, MLIR validation gate, ExecutionEngine/JIT equivalence, and docs/release-gate closeout.
- Phase 16 complete: `loom-native-melior` provides optional backend/toolchain facts, verifier-gated artifact construction, MLIR validation, JIT boundary diagnostics, managed `scripts/melior-jit-test.sh`, and release-gate integration for bounded Int32 copy evidence only. Local LLVM/MLIR is managed at 22.1.7; skip is only allowed by explicit `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`.
- Phase 17 complete: `loom_core::artifact_verifier` now exposes `verify_artifact` and `verify_artifact_with_l2_core`, unifying `LMC1` container/manifest/L1 structural checks, optional accepted `L2Core` `VerifiedArtifactFacts`, constraint status, lowering readiness, `loom verify-artifact`, and `scripts/artifact-verifier-test.sh` release-gate evidence.
- Phase 18 planned: 5 plans across reader facts contract/dependency boundary, recursive layout/dtype/segment inspection, supported single-column conversion matrix, supported struct/table conversion, and CLI/report/release-gate closeout.
- Phase 18 executing: 18-01 complete with `18-READER-CONTRACT.md`, Loom-owned `VortexReaderFacts` / layout / dtype / segment facts, buffer/path reader fact extraction, support/emission classification, reader contract tests, and strengthened `vortex-file` / `vortex-layout` dependency gates.
- Phase 18 executing: 18-02 complete with recursive layout child paths, structured `DType` classification including struct field names/counts, segment overlap/order facts, split range facts or non-fatal diagnostics, and guard coverage for unsupported struct files without primitive-scan panics.
- Phase 18 executing: 18-03 complete with an explicit non-null single-column primitive matrix for Int32, Int64, Float32, and Float64, verifier-backed `LMC1` emission, typed Vortex scan oracle helpers, and UTF-8 fail-closed negative coverage.
- Phase 18 executing: 18-04 complete with real Vortex struct/table support for non-null primitive fields, `LMT1` table emission wrapped in `LMC1`, artifact-verifier/table-decode oracle tests, unsupported string-field table fail-closed coverage, and CLI reader support/emission output.
- Phase 18 complete: 18-05 closed with CLI reader facts and artifact-verifier status, `scripts/complete-vortex-reader-test.sh`, release-gate wiring, final report, public/planning docs, and Phase 19 solver-backed verifier handoff.
- Phase 19 research refreshed: recommended solver-neutral obligation/report types in `loom-core`, deterministic SMT-LIB v2.7 emission, optional `loom-solver-smt` subprocess backend with `z3`/`cvc5`/`bitwuzla` backend declarations from day one, Bitwuzla as the primary implemented backend, a Bitwuzla-supported `QF_BV` required path, Z3/cvc5 as optional adapters or strict cross-check paths, and fail-closed handling for `sat`, `unknown`, timeout, parse error, solver crash, missing strict solver, and cross-check disagreement.
- Phase 19 planned: 5 plans across solver contract/report model, deterministic Bitwuzla-primary `QF_BV` SMT-LIB emission, optional `loom-solver-smt` Bitwuzla backend, artifact verifier solver-discharge integration, and CLI/release-gate closeout.
- Phase 19 executing: 19-01 complete with `19-SOLVER-CONTRACT.md`, `loom_core::solver`, `ArtifactVerificationFacts.solver_report`, backend declarations for `z3`/`cvc5`/`bitwuzla`, Bitwuzla primary metadata, and focused solver contract tests.
- Phase 19 executing: 19-02 complete with deterministic Bitwuzla-primary `QF_BV` SMT-LIB script emission, required/cross-check script family metadata, named bad-state assertions, stable FNV-style script IDs, and focused `smtlib_emitter` tests.
- Phase 19 executing: 19-03 complete with optional `loom-solver-smt`, backend discovery/declarations for `z3`/`cvc5`/`bitwuzla`, Bitwuzla subprocess execution/parsing, managed Bitwuzla gate requirements, explicit config-only skip diagnostics, and `scripts/solver-verifier-test.sh`.
- Phase 19 executing: 19-04 complete with `apply_solver_discharge`, artifact verifier facts carrying trusted solver reports only after matching/discharged obligations, artifact lowering readiness blocked for `CollectedOnly` constraints, and `loom-solver-smt` artifact-level Bitwuzla helper tests.
- Phase 19 complete: 19-05 closed with solver-backed `loom verify-artifact --solver-bitwuzla --l2core-sample`, release-gate wiring through `scripts/solver-verifier-test.sh` and `scripts/mvp0-verify.sh`, final solver report, public/planning docs, and Phase 20 handoff requiring discharged facts.
- Phase 20 planned: 20-01 through 20-05 cover the production lowering contract/discharged-facts gate, `loom.decode` dialect contract and textual surface, Arrow raw-buffer builder lowering, primitive multi-column native kernel expansion, and MLIR validation/report/closeout.
- Phase 20/21 roadmap caveat added: production lowering and expanded encoding coverage are coupled axes, not a one-way sequence; Phase 21 must classify each new encoding as interpreter-only, lowering-supported with a dialect/native delta, or fail-closed/deferred.
- Phase 22 ABI caveat added: engine independence is a design claim until a second consumer validates it; predicate/projection pushdown plus concurrency/reentrancy/thread ownership must be decided in Phase 22.
- Phase 20 complete: production native-lowering seed, `loom.decode` textual surface, primitive Arrow/raw-buffer builder lowering, raw primitive multi-column matrix, explicit bitpack/FOR/complex-encoding deferral, production MLIR validation hook, `scripts/production-native-lowering-test.sh`, release-gate wiring, and final report are complete; compiled ODS dialect, production `melior` pass pipeline, LLVM lowering, and LLVM/JIT execution are explicitly deferred to Phase 23.
- Phase 21 reserved as a roadmap placeholder only: expanded Vortex encoding/layout/storage coverage beyond the Phase 18 accepted matrix after solver-backed verifier evidence and the Phase 20 lowering seed exist, with a paired lowering disposition for each new encoding/layout.
- Phase 21 research started: recommended a finite coverage matrix over dtype/nullability/array encoding/layout/statistics/emission/lowering disposition, with priority on nullable primitives, chunked primitives, dictionary/run-end/sequence, bitpack/FOR integer facts, and explicit Phase 22 ABI plus Phase 23 backend handoff.
- Phase 21 planned: 21-01 through 21-05 cover the coverage/disposition matrix, nullable and chunked primitive facts, dictionary/run-end/sequence coverage, bitpack/FOR/numeric compression coverage, and release-gate/report handoff to Phase 22/23.
- Phase 21 complete: expanded real Vortex coverage now records reader support, artifact emission, oracle evidence, and native-lowering disposition separately. Nullable primitives fail closed with null-preserving facts; chunked/dictionary/RLE/bitpack/FOR fixtures have row oracle evidence and canonical raw emission where safe; string/ALP/PCodec-style compression remains deferred until Loom-owned params are extracted and verified. `scripts/vortex-encoding-coverage-test.sh` is wired into the release gate.
- Phase 22-25 split research complete: the former engine-integrated native execution MVP placeholder is now four placeholders covering host native runtime ABI/policy, production native backend implementation, DuckDB native integration MVP, and native equivalence/cache/fallback hardening.
- Phase 22 research started: recommended a host-neutral runtime ABI and execution policy over verified artifact facts, Bitwuzla discharge, production-lowering facts, projection/predicate/split planning, cache identity, diagnostics, concurrency, and native/interpreter/fail-closed decisions before Phase 23 backend or Phase 24 DuckDB integration.
- Phase 22 planned: 22-01 through 22-05 cover runtime ABI contract/lifecycle, verified-facts handoff and execution decisions, projection/predicate/split/concurrency planning, cache key/diagnostics/C ABI sketch, and final release-gate/backend handoff.
- Phase 22 complete: `loom_core::runtime_abi` now provides host-neutral runtime ABI model types, deterministic native/interpreter/fail-closed decision policy, projection/predicate/split/concurrency planning, deterministic cache identity, stable runtime diagnostics, a non-frozen `loom_runtime.h` C ABI sketch, final report, and `scripts/runtime-abi-test.sh` wired into the release gate.
- Phase 23 research/planning started: recommended a host-neutral production backend boundary in `loom-native-melior` that consumes Phase 22 `RuntimePlan`/`RuntimeCacheKey`, keeps public `loom_runtime.h` unfrozen, records backend/toolchain/pipeline/target identity, adds ODS/TableGen evidence for `loom.decode`, promotes melior/LLVM lowering into a production pipeline, seeds verifier-gated JIT execution over supported primitive kernels, and release-gates the backend before Phase 24 DuckDB integration.
- Phase 23 planned: 23-01 through 23-05 cover the backend contract/runtime-plan bridge, compiled `loom.decode` ODS dialect evidence, production melior/LLVM lowering pipeline, verifier-gated JIT execution plus interpreter equivalence, and backend release-gate/report/Phase 24 handoff.
- Phase 23 executing: 23-01 complete with `23-BACKEND-CONTRACT.md`, `loom_native_melior::backend`, stable backend identity/diagnostics/cancellation modeling, runtime-plan/cache-key preflight validation, and focused `production_backend_contract` tests.
- Phase 24 reserved as a roadmap placeholder only: DuckDB native execution integration MVP over the Phase 22 runtime contract and Phase 23 production backend. Phase 24 must keep DuckDB as a natural adapter over the runtime ABI, mapping bind/init/local-init to plan/scan/worker and testing projection/threading plus Arrow release/error/cancel paths.
- Phase 25 reserved as a roadmap placeholder only: native equivalence, cache, and fallback hardening before table-format binding.
- Phase 26 reserved as a roadmap placeholder only: Iceberg ref/table binding after the hardened native execution contract is credible.
- Phase 27 reserved as a roadmap placeholder only: StarRocks + DuckDB dual query surface after Iceberg binding exists.
- Phase 28 reserved as a roadmap placeholder only: full arbitrary Vortex semantic compatibility after ABI/backend/hardening/table-binding and dual-query-surface evidence exists.

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 260608-vni | Update Phase 23 and 24 roadmap handoff from Phase 22 ABI deep research | 2026-06-08 | 3ed57a5 | [260608-vni-update-phase-23-and-24-roadmap-handoff-f](./quick/260608-vni-update-phase-23-and-24-roadmap-handoff-f/) |
| 260608-v17 | Polish English and Chinese READMEs with final minimal logo, DuckDB data flow, and quickstart | 2026-06-08 | none | [260608-v17-polish-readme-with-logo-duckdb-data-flow](./quick/260608-v17-polish-readme-with-logo-duckdb-data-flow/) |
| 260608-vfc | Extend Phase 22 deep research with C API, Node-API/N-API, and natural API design lessons | 2026-06-08 | 58f9cd4 | [260608-vfc-extend-phase-22-deep-research-with-c-api](./quick/260608-vfc-extend-phase-22-deep-research-with-c-api/) |
| 260608-va8 | Deepen Phase 22 research with papers, related projects, and ABI best practices | 2026-06-08 | 7b832ef | [260608-va8-deepen-phase-22-research-with-papers-rel](./quick/260608-va8-deepen-phase-22-research-with-papers-rel/) |
| 260607-taf | Translate design.md (Chinese) into English README.md and create README-zh.md as the consistent Chinese version | 2026-06-07 | 5f8b8e7 | [260607-taf-translate-design-md-chinese-into-english](./quick/260607-taf-translate-design-md-chinese-into-english/) |

## Deferred Items

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| v2 | COV-01: Additional L2 kernels (ALP Float32/Float64) | Complete | Phase 10 |
| v2 | COV-02: Multi-column table function | Complete | Phase 8 |
| v2 foundation | BASE-01: MVP0 planning/docs baseline cleanup | Complete | Phase 6 |
| v2 foundation | DOC-01/DOC-02: README and positioning documentation cleanup | Complete | Phase 6 |
| v2 foundation | VERIFY-04: One-command MVP0 release gate | Complete | Phase 6 |
| v2 foundation | BUILD-01: Rust/DuckDB stale-artifact build hygiene | Complete | Phase 6 |
| v2 | DX-01: Human-readable L1 layout descriptor | Complete | Phase 7 |
| v2 | DX-02: Multiple sample columns per encoding | Complete | Phase 7 |
| v2 | DX-03: CLI inspect/decode driver | Complete | Phase 7 |
| v2 | DX-04: Illustrative timing comparison | Complete | Phase 7 |
| v2 table | TABLE-01: Table description model | Complete | Phase 8 |
| v2 table | TABLE-02: Mixed-column table payload codec | Complete | Phase 8 |
| v2 table | TABLE-03: Rust multi-column decode | Complete | Phase 8 |
| v2 table | DUCK-05: DuckDB multi-column loom_scan | Complete | Phase 8 |
| v2 table | STREAM-01: ArrowArrayStream decision | Complete | Phase 8 |
| v2 table | VERIFY-05: Multi-column SQL acceptance | Complete | Phase 8 |
| v2 safety | SAFE-01: Verifier module with typed diagnostics | Complete | Phase 9 |
| v2 safety | SAFE-02: Structural invariant rejection coverage | Complete | Phase 9 |
| v2 safety | SAFE-03: Decode entry verifier routing | Complete | Phase 9 |
| v2 safety | SAFE-04: CLI verifier visibility | Complete | Phase 9 |
| v2 safety | VERIFY-06: Negative verifier release gate | Complete | Phase 9 |
| v3 distribution | DIST-01..DIST-05: Distribution Container v0 | Complete | Phase 11 |
| v3 safety | Formal verifier / safety proof MVP | Complete | Phase 12 |
| v3 safety | Full Loom verifier | Complete | Phase 13 |
| v3 native | MLIR/native lowering spike | Complete | Phase 14 |
| v3 ingress | Real Vortex file/container ingress | Complete | Phase 15 |
| v3 native | Full melior/LLVM/JIT backend integration | Complete | Phase 16 |
| v3 verifier | Unified artifact verification pipeline | Complete | Phase 17 |
| v3 ingress | Complete Vortex reader | Complete | Phase 18 |
| v3 verifier | Solver-backed full artifact verifier | Complete | Phase 19 |
| v3 native | Production decode dialect seed and raw primitive native lowering | Complete | Phase 20 |
| v3 ingress | Expanded Vortex encoding coverage | Complete | Phase 21 |
| v3 engine | Host native runtime ABI and execution policy | Complete | Phase 22 |
| v3 native | Production native backend implementation | Planned | Phase 23 |
| v3 engine | DuckDB native execution integration MVP | Placeholder | Phase 24 |
| v3 engine | Native equivalence, cache, and fallback hardening | Placeholder | Phase 25 |
| v3 table | Iceberg ref/table binding | Placeholder | Phase 26 |
| v3 engine | StarRocks + DuckDB dual query surface | Placeholder | Phase 27 |
| v3 compatibility | Full Vortex semantic compatibility | Placeholder | Phase 28 |

## Session Continuity

Last session: 2026-06-08T14:58:03.000Z
Stopped at: Phase 23 23-01 complete; execute 23-02 compiled loom.decode ODS dialect evidence next

Phase 17 handoff:

- Contract: `.planning/phases/17-unified-artifact-verification-pipeline/17-ARTIFACT-VERIFIER-CONTRACT.md`
- Final report: `.planning/phases/17-unified-artifact-verification-pipeline/17-ARTIFACT-VERIFIER-REPORT.md`
- Summary: `.planning/phases/17-unified-artifact-verification-pipeline/17-SUMMARY.md`
- Release gate: `scripts/artifact-verifier-test.sh` is wired into `scripts/mvp0-verify.sh`

Phase 15 research: .planning/phases/15-real-vortex-file-container-ingress/15-RESEARCH.md
Phase 15 context: .planning/phases/15-real-vortex-file-container-ingress/15-CONTEXT.md
Phase 15 report: .planning/phases/15-real-vortex-file-container-ingress/15-INGRESS-REPORT.md
Phase 16 research: .planning/phases/16-full-melior-llvm-jit-backend-integration/16-RESEARCH.md
Phase 16 report: .planning/phases/16-full-melior-llvm-jit-backend-integration/16-BACKEND-REPORT.md
Phase 16 summary: .planning/phases/16-full-melior-llvm-jit-backend-integration/16-SUMMARY.md
Phase 17 research: .planning/phases/17-unified-artifact-verification-pipeline/17-RESEARCH.md
Phase 17 plans: .planning/phases/17-unified-artifact-verification-pipeline/17-01-PLAN.md through 17-05-PLAN.md
Phase 17 report: .planning/phases/17-unified-artifact-verification-pipeline/17-ARTIFACT-VERIFIER-REPORT.md
Phase 17 summary: .planning/phases/17-unified-artifact-verification-pipeline/17-SUMMARY.md
Phase 18 research: .planning/phases/18-complete-vortex-reader/18-RESEARCH.md
Phase 18 context: .planning/phases/18-complete-vortex-reader/18-CONTEXT.md
Phase 18 plans: .planning/phases/18-complete-vortex-reader/18-01-PLAN.md through 18-05-PLAN.md
Phase 18 report: .planning/phases/18-complete-vortex-reader/18-READER-REPORT.md
Phase 18 summary: .planning/phases/18-complete-vortex-reader/18-SUMMARY.md
Phase 19 research: .planning/phases/19-solver-backed-full-artifact-verifier/19-RESEARCH.md
Phase 19 context: .planning/phases/19-solver-backed-full-artifact-verifier/19-CONTEXT.md
Phase 19 plans: .planning/phases/19-solver-backed-full-artifact-verifier/19-01-PLAN.md through 19-05-PLAN.md
Phase 19 19-01 summary: .planning/phases/19-solver-backed-full-artifact-verifier/19-01-SUMMARY.md
Phase 19 19-02 summary: .planning/phases/19-solver-backed-full-artifact-verifier/19-02-SUMMARY.md
Phase 19 19-03 summary: .planning/phases/19-solver-backed-full-artifact-verifier/19-03-SUMMARY.md
Phase 19 19-04 summary: .planning/phases/19-solver-backed-full-artifact-verifier/19-04-SUMMARY.md
Phase 19 solver report: .planning/phases/19-solver-backed-full-artifact-verifier/19-SOLVER-REPORT.md
Phase 19 summary: .planning/phases/19-solver-backed-full-artifact-verifier/19-SUMMARY.md
Phase 20 research: .planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-RESEARCH.md
Phase 20 context: .planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-CONTEXT.md
Phase 20 plans: .planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-01-PLAN.md through 20-05-PLAN.md
Phase 20 report: .planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-NATIVE-LOWERING-REPORT.md
Phase 20 summary: .planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-SUMMARY.md
Phase 21 research: .planning/phases/21-expanded-vortex-encoding-coverage/21-RESEARCH.md
Phase 21 context: .planning/phases/21-expanded-vortex-encoding-coverage/21-CONTEXT.md
Phase 21 plans: .planning/phases/21-expanded-vortex-encoding-coverage/21-01-PLAN.md through 21-05-PLAN.md
Phase 21 report: .planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-REPORT.md
Phase 21 summary: .planning/phases/21-expanded-vortex-encoding-coverage/21-SUMMARY.md
Resume file: .planning/ROADMAP.md
