---
gsd_state_version: 1.0
milestone: v1.5.3
milestone_name: milestone
status: executing
stopped_at: Phase 32 plan 32-03 complete
last_updated: "2026-06-09T04:10:00Z"
last_activity: 2026-06-09 -- Completed Phase 32 architecture boundary review
progress:
  total_phases: 32
  completed_phases: 30
  total_plans: 138
  completed_plans: 134
  percent: 97
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-06-08)

**Core value:** A user can run a SQL query in DuckDB over Loom-decoded artifacts, including mixed-column table payloads and the current Parquet/Lance/Vortex source-backed single-column `LMA1` e2e artifacts, and get expected row/aggregate results.
**Current focus:** Execute Phase 32 overall MVP1 architecture and code review; Phase 30 deferred StarRocks/full dual-surface work remains explicitly incomplete

## Current Position

Phase: 32 executing
Plan: 32-04 ready
Status: Phase 32 executing code-quality review and narrow remediation; Phase 30 dual-query remains partial/deferred
Last activity: 2026-06-09 -- Completed Phase 32 architecture boundary review

Progress: 97%

## Progress Snapshot

- Completed phases: 30 / 32
- Completed executable plans: 134 / 138
- Current milestone stage: MVP1 / v3 distribution and verification track
- Current position: Phase 32 executing: claim/evidence/boundary reviews complete; code-quality review and narrow remediation are next
- Last verified gate: `RUSTC_WRAPPER= bash scripts/duckdb-source-e2e-test.sh` passed; `scripts/mvp1-verify.sh` now wraps `scripts/mvp0-verify.sh` plus the DuckDB source e2e gate

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
| 23 | Production native backend implementation | 5/5 complete |
| 24 | DuckDB native execution integration MVP | 5/5 complete |
| 25 | Native equivalence, cache, and fallback hardening | 5/5 complete |
| 26 | External source ingress contract | 5/5 complete |
| 27 | Lance + Parquet archival readability / dataset ingress | 5/5 complete |
| 28 | Full Lance + Parquet + Vortex semantic compatibility | 5/5 complete |
| 29 | Iceberg Ref/Table Binding | 5/5 complete |
| 30 | StarRocks + DuckDB Dual Query Surface | 3/5 complete; DuckDB executable slice complete, full dual-surface pending |
| 31 | Full Arrow Semantic Source Compatibility | 6/6 complete |
| 32 | MVP1 Architecture and Code Review | 3/5 executing |

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
- [Phase 24 P01]: DuckDB route planning remains inside Rust runtime policy; C++ should consume reports rather than duplicate native/fallback switches.
- [Phase 24 P01]: Native buffers are exposed only after backend prepare/JIT output comparison succeeds; mismatch and cancellation return no buffers.
- [Phase 3 P01]: Array trait must be explicitly imported in arrow-rs 58.3 for .into_data() and .is_null() on PrimitiveArray<T>
- [Phase ?]: BufferHandle .as_host().as_ref() (option A) confirmed for packed bytes access
- [Phase ?]: FoR+BitPack: use FoR::try_new(bp.into_array(), ref) with manual deltas, not FoRData::encode
- [Phase ?]: BitPackedArrayExt::validity explicit UFCS avoids ArrayRef::validity ambiguity
- [Phase 24]: DuckDB route controls are exposed only through loom_duckdb_internal.h; generated public loom.h excludes every loom_duckdb_* symbol and LoomDuckDb* type. — Preserves Phase 24 internal adapter boundary while preventing public ABI freeze creep.
- [Phase 24]: [Phase 24 P03]: DuckDB C++ consumes internal Rust route decisions instead of duplicating native eligibility policy. — Plan 24-03 lifecycle adapter uses loom_duckdb_* route reports from Rust.
- [Phase 24]: [Phase 24 P03]: Projection pushdown is enabled internally through TableFunctionInitInput::column_ids while public SQL remains loom_scan(path). — Preserves D-10 and D-13 without adding public SQL mode parameters.
- [Phase 24]: [Phase 24 P03]: Phase 24 keeps single-worker, single-batch direct DataChunk output; local worker state and stream APIs remain deferred. — Matches D-03/D-11 and keeps Phase 25 responsible for concurrency/cache hardening.
- [Phase 24]: [Phase 24 P04]: Native DuckDB output remains an internal direct DataChunk fill path, not a public ArrowArrayStream or record-batch ABI. — Preserves D-05/D-13 while letting DuckDB consume prepared native primitive buffers.
- [Phase 24]: [Phase 24 P04]: Native primitive buffers must match pointer, exact byte length, Arrow type, projected Loom kind, and DuckDB vector type before row emission. — Enforces T-24-04-01 and D-08 at the adapter boundary.
- [Phase 24]: [Phase 24 P04]: LoomScan sets positive cardinality only after all selected native or interpreter columns fill successfully. — Keeps mismatch/cancel/fail-closed routes from emitting partial rows.
- [Phase 24]: [Phase 24 P05]: Phase 24 route evidence is tested through public loom_scan(path) SQL plus internal LOOM_DUCKDB_TEST diagnostics, not public route-specific SQL.
- [Phase 24]: [Phase 24 P05]: The native primitive DuckDB fixture is all-zero non-null Int32/Int64/Float32/Float64 raw table data to avoid broader native semantics claims.
- [Phase 24]: [Phase 24 P05]: The main release gate now runs Phase 24 after the Phase 23 backend gate and before the existing DuckDB SQL smoke gate.
- [Phase 25-native-equivalence-cache-and-fallback-hardening]: 25-02: Cache evidence remains on existing internal DuckDB diagnostics and public loom.h rejects cache/API creep markers.
- [Phase 25-native-equivalence-cache-and-fallback-hardening]: 25-02: Cache hits reuse only accepted NativeBackendReport preparation evidence; native buffers are regenerated and compared before every native return.
- [Phase 25-native-equivalence-cache-and-fallback-hardening]: 25-02: Kept DuckDB native preparation cache in-process and Rust-owned with no persistent format, public C API, SQL flags, path/mtime semantics, eviction policy, or package additions.
- [Phase 25-native-equivalence-cache-and-fallback-hardening]: 25-03: Interpreter/reference bytes remain the oracle for supported non-null primitive native helper routes; unsupported strings, nullability, compression, predicates, splits, malformed artifacts, mismatch, and cancellation are negative/fallback evidence.
- [Phase 25-native-equivalence-cache-and-fallback-hardening]: 25-04: DuckDB SQL hardening uses public `loom_scan(path)` plus internal route reports for cache smoke, projection drift, fallback, strict fail-closed, cancellation, and public API creep gates.
- [Phase 25-native-equivalence-cache-and-fallback-hardening]: 25-05: Main release gate runs Phase 25 native hardening after Phase 24 and before DuckDB smoke; final report records bounded equivalence/cache/fallback evidence and Phase 26 handoff assumptions.
- [Phase 27]: [Phase 27 P01]: Lance and Parquet SDK dependencies stay isolated to adapter crates with exact workspace pins; generic/core/ffi/public surfaces remain SDK-free.
- [Phase 27]: [Phase 27 P01]: SourceIngressAcceptedArtifact is a source-neutral bytes-plus-report handoff; existing Vortex adapter-local handoff remains stable for now.
- [Phase 27]: [Phase 27 P01]: Phase 27 guard script remains unwired from mvp0-verify until Plan 27-05.
- [Phase 27]: [Phase 27 P02]: Supported Parquet shapes are classified in SourceCoverage only; accepted reports and artifact bytes remain deferred to the emission plan.
- [Phase 27]: [Phase 27 P02]: Parquet SDK metadata stays adapter-private and maps to source-neutral strings, counts, booleans, layout facts, and split facts.
- [Phase 27]: [Phase 27 P03]: Lance facts classify supported shapes in SourceCoverage only; accepted reports and artifact bytes remain deferred to Plan 27-04.
- [Phase 27]: [Phase 27 P03]: Lance SDK objects and object-store state remain private to loom-lance-ingress; generic/core/ffi crates receive only source-neutral facts and diagnostics.
- [Phase 27]: [Phase 27 P03]: Arrow extension metadata is treated as unsupported schema even when the physical storage type is a supported primitive.
- [Phase 27]: [Phase 27 P04]: Accepted Lance and Parquet source reports are constructed only after verify_artifact accepts emitted LMC1 bytes and source oracle evidence is accepted.
- [Phase 27]: [Phase 27 P04]: Parquet uses ArrowScan evidence and Lance uses SourceNativeScan evidence; both remain evidence paths rather than Loom decode bypasses.
- [Phase 27]: [Phase 27 P04]: Legacy readability uses actual older writer outputs from parquet 57.0.0 and lance 6.0.0 paired with sibling verifier-accepted Loom artifacts.
- [Phase 27]: Phase 27 legacy proof remains hard-gated on actual older-version Lance and Parquet fixture paths plus paired verifier-accepted Loom artifacts.
- [Phase 27]: The main release verifier runs Phase 27 after Phase 26 source ingress and before DuckDB SQL smoke.
- [Phase 28]: Phase 28 was reordered ahead of Iceberg/query-surface work to make Lance, Parquet, and Vortex semantic compatibility claims explicit before table/ref binding consumes them.
- [Phase 28]: Current semantic gate records accepted, unsupported, rejected, canonicalized, and native-disposition rows; it must not overclaim canonical raw rows as full structured semantics.
- [Phase 29]: 29-03: Accepted Iceberg bindings require local artifact bytes, recomputed SHA-256, live verify_artifact acceptance, and a sidecar-referenced evidence JSON artifact before bytes are returned.
- [Phase 29]: 29-03: Sidecar verifier/source/oracle accepted flags are required descriptive inputs only; they are never sufficient to construct accepted binding evidence.
- [Phase 29]: 29-04: Stale source/oracle evidence row count must match verified Loom artifact rows; sidecar flags remain descriptive only.
- [Phase 29]: 29-04: Manifest-only sidecars fail before binding facts are considered complete; no official iceberg crate is added by default.
- [Phase 29]: 29-05: The focused Iceberg binding gate is wired into `scripts/mvp0-verify.sh` after Phase 28 semantic compatibility and before DuckDB smoke.
- [Phase 29]: 29-05: Phase 29 remains binding evidence only; no DuckDB/CLI SQL route, StarRocks route, public C ABI, catalog, credential, branch/tag mutation, or default `iceberg` SDK scope was added.
- [Phase 31]: Source compatibility target reset from bounded/core-80 coverage to full Arrow semantic compatibility for arbitrary Lance/Parquet schemas and materialized Vortex dtypes.
- [Phase 31]: New source compatibility artifacts should use `LMC2`/`LMA1`; old `LMC1(LMP1/LMT1)` remains legacy narrow evidence and must not carry new full-schema claims.
- [Phase 31]: Native MLIR and query-engine coverage are optimization/surface layers, not prerequisites for full source semantic compatibility.
- [Phase 31]: 31-01 added the `LMC2`/`LMA1` Arrow semantic contract and core module scaffolds; `NullableRaw` WIP is not present in `loom-core`.
- [Phase 31]: 31-02 added an IPC-backed `LMA1` codec that verifies Arrow semantic payloads before encode and after decode; Arrow IPC is the carrier, not the trust boundary.
- [Phase 31]: 31-03 and 31-04 changed Parquet and Lance accepted emission from narrow `LMC1(LMP1/LMT1)` raw/table artifacts to verifier-accepted `LMA1` Arrow semantic artifacts with source Arrow equality tests.
- [Phase 31]: Current tradeoff: `LMA1` direct verifier acceptance is implemented; `LMC2` container wrapping remains a Phase 31 finalization item rather than a blocker for Parquet/Lance semantic e2e evidence.
- [Phase 31]: 31-05 added Vortex Arrow executor materialization into verifier-accepted `LMA1` semantic artifacts; legacy Vortex `LMC1` helper remains for older narrow raw/table tests while the Phase 31 path uses `emit_source_ingress_lma1_from_vortex_buffer`.
- [Phase 31]: 31-06 wired `scripts/full-arrow-semantic-compatibility-test.sh` into `scripts/mvp0-verify.sh` after Phase 28 and before Phase 29, added the final compatibility report, and updated public docs to avoid DuckDB/native overclaims.
- [Quick 260609-eip]: MVP1 verify is the broad check entry point; it runs `scripts/mvp0-verify.sh` first, then DuckDB e2e over Parquet, Lance, and Vortex source-backed single-column `LMA1` artifacts.
- [Quick 260609-eip]: DuckDB `loom_scan` may execute single-column `LMA1` through interpreter fallback; native lowering remains unsupported for `Arrow semantic payload` and continues to report lowering diagnostics.
- [Phase 32]: Added as a review-first phase to audit MVP1 architecture, code quality, ABI/FFI boundaries, true execution evidence, release gates, dependency isolation, and documentation claims before further feature expansion.
- [Phase 32]: Planned as five review-first plans: claim ledger, execution evidence audit, architecture/ABI/dependency audit, code-quality review with narrow remediation, and MVP1 go/no-go readiness closeout.
- [Phase 32 P01]: Claim ledger classifies source compatibility as proven at the Arrow semantic layer, DuckDB `LMA1` SQL as bounded to the single-column e2e slice, native `LMA1` execution as unsupported/fallback, `LMC2` as deferred, and Phase 30 dual-surface as partial.
- [Phase 32 P02]: Execution evidence matrix records what each major gate proves and does not prove; `scripts/mvp1-review-audit-test.sh` is a marker/report audit seed, not a runtime semantics gate.
- [Phase 32 P03]: Architecture boundary review found source SDK isolation and public/internal FFI separation intact; direct `LMA1` is implemented, `LMC2` remains future wrapper, and native lowering rejects `Arrow semantic payload`.

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
- Phase 23 executing: 23-02 complete with `loom.decode` ODS/TableGen sources, Rust manifest drift checks against the Phase 20 textual surface, default MLIR-free manifest tests, and `scripts/production-backend-test.sh` strict `mlir-tblgen` validation.
- Phase 23 executing: 23-03 complete with validated `NativeBackendRequest` to production MLIR/LLVM pipeline bridging, pipeline/toolchain identity in `NativeBackendReport`, skip-aware strict toolchain handling, production LLVM translation validation, and focused negative `production_backend_pipeline` tests.
- Phase 23 executing: 23-04 complete with production JIT seed entry points over accepted `NativeBackendReport` artifacts, deterministic primitive reference-output comparison, cancellation checks, unsupported-shape rejection before toolchain probing, and focused `production_backend_jit` tests.
- Phase 23 complete: `scripts/production-backend-test.sh` gates backend contract, ODS manifest, production pipeline, JIT seed, and strict ODS validation when local LLVM/MLIR tooling is available; it is wired into `scripts/mvp0-verify.sh`. Final report and summary document supported non-null primitive native evidence, deferred paths, Backend Identity, Cancellation, Unfrozen `loom_runtime.h`, and the Phase 24 DuckDB handoff.
- Phase 24 ready for research/planning: DuckDB native execution integration MVP over the Phase 22 runtime contract and Phase 23 production backend. Phase 24 must keep DuckDB as a natural adapter over the runtime/backend contract, mapping bind/init/local-init/function to plan/scan/worker/next-batch and testing projection/threading plus Arrow release/error/cancel paths.
- Phase 24 executing: 24-01 complete with `loom_ffi::duckdb_runtime`, verifier-backed runtime planning, projection/no-predicate/full-scan/single-worker route evidence, backend prepare/JIT comparison routing, and fail-closed mismatch/cancellation diagnostics.
- Phase 24 executing: 24-02 complete with internal `loom_duckdb_*` FFI handles, `loom_duckdb_internal.h`, panic-safe route/diagnostic/native-buffer accessors, and public-header leakage gates.
- Phase 25 complete: native equivalence/cache/fallback hardening is release-gated through `scripts/native-hardening-test.sh` and the main `scripts/mvp0-verify.sh` gate. The final report is `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-NATIVE-HARDENING-REPORT.md`.
- Phase 26 complete: external source ingress contract is release-gated through `scripts/source-ingress-contract-test.sh` and wired into the main `scripts/mvp0-verify.sh` gate after Phase 25 native hardening and before DuckDB smoke. The generic `loom-source-ingress` contract preserves source-neutral facts/diagnostics/support/emission/oracle/verifier handoff rules, with Vortex as the reference adapter.
- Phase 27 complete: Lance + Parquet archival readability through the external source ingress contract is release-gated with current-version and actual older-version Parquet 57.0.0 / Lance 6.0.0 read/write proofs.
- Phase 28 complete: Full Lance + Parquet + Vortex semantic compatibility now has a bounded matrix, focused no-overclaim gate, nullable and structured encoding deferral tests, final report, and main release-gate wiring before Iceberg binding.
- Phase 29 executing: 29-01 established the adapter-local `loom-iceberg-binding` crate, binding report contract, exact `serde_json` pin, and dependency/public-surface guards.
- Phase 29 executing: 29-02 added typed local Iceberg metadata and Loom sidecar parsing into descriptive facts, byte-free unsupported source reports, rejected diagnostics for malformed/missing identity, and parser fixture coverage in the focused gate.
- Phase 29 executing: 29-04 added the fail-closed mismatch matrix, stale source and forged decoded-row/oracle evidence fixtures, the final binding evidence report, and focused gate checks for report markers plus metadata-only trust wording.
- Phase 29 complete: 29-05 finalized and wired `scripts/iceberg-binding-test.sh` into the main release verifier after Phase 28 semantic compatibility and before DuckDB smoke, recorded closeout evidence, and kept Iceberg binding out of public query/API/catalog/credential surfaces.
- Phase 30 partial completion on 2026-06-09: DuckDB executable evidence over Phase 29 accepted bytes is implemented and verified through `scripts/dual-query-surface-test.sh`.
- Phase 30 remaining work: StarRocks runtime-smoke semantics, fail-closed negative matrix expansion, main release-gate wiring, and final dual-surface report are not complete and must not be cited as completed dual-engine evidence.
- Phase 32 added: MVP1 Architecture and Code Review, focused on whole-system design/code audit and remediation planning before further feature expansion.
- Phase 32 executing: 32-01 through 32-03 completed claim/evidence/boundary review artifacts; 32-04 will perform code-quality review and narrow remediation.

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 260609-eip | Add DuckDB e2e tests for Lance, Parquet, and Vortex semantic sources; create mvp1-verify that includes mvp0-verify plus these e2e checks | 2026-06-09 | 6bc4638 | [260609-eip-add-duckdb-e2e-tests-for-lance-parquet-a](./quick/260609-eip-add-duckdb-e2e-tests-for-lance-parquet-a/) |
| 260608-wy8 | Extend Phase 27 archival readability target to Lance and Parquet | 2026-06-08 | none | [260608-wy8-extend-phase-27-archival-readability-tar](./quick/260608-wy8-extend-phase-27-archival-readability-tar/) |
| 260608-wwx | Refine Phase 27 Lance archival readability roadmap target | 2026-06-08 | none | [260608-wwx-refine-phase-27-lance-archival-readabili](./quick/260608-wwx-refine-phase-27-lance-archival-readabili/) |
| 260608-waw | Add external source ingress and Lance phases to roadmap | 2026-06-08 | this commit | [260608-waw-add-external-source-ingress-and-lance-ph](./quick/260608-waw-add-external-source-ingress-and-lance-ph/) |
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
| v3 native | Production native backend implementation | Complete | Phase 23 |
| v3 engine | DuckDB native execution integration MVP | Complete | Phase 24 |
| v3 engine | Native equivalence, cache, and fallback hardening | Complete | Phase 25 |
| v3 ingress | External source ingress contract | Complete | Phase 26 |
| v3 ingress | Lance + Parquet archival readability / dataset ingress | Complete | Phase 27 |
| v3 compatibility | Full Lance + Parquet + Vortex semantic compatibility | Active | Phase 28 |
| v3 table | Iceberg ref/table binding | Complete | Phase 29 |
| v3 engine | StarRocks + DuckDB dual query surface | DuckDB executable slice complete; full dual-surface pending/deferred | Phase 30 |

## Session Continuity

Last session: 2026-06-09T02:50:45.638Z
Stopped at: Phase 32 context gathered

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
Phase 23 backend contract: .planning/phases/23-production-native-backend-implementation/23-BACKEND-CONTRACT.md
Phase 23 backend report: .planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md
Phase 23 summary: .planning/phases/23-production-native-backend-implementation/23-SUMMARY.md
Phase 24 next: consume Phase 22 runtime ABI report plus Phase 23 backend report; map DuckDB bind/init/local-init/function to runtime/backend plan/scan/worker/next-batch before editing host code.
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
Resume file: .planning/phases/32-mvp1-architecture-and-code-review/32-CONTEXT.md

## Performance Metrics

| Phase | Plan | Duration | Notes |
|-------|------|----------|-------|
| Phase 24 P02 | 5min | 2 tasks | 5 files |
| Phase 24-duckdb-native-execution-integration-mvp P03 | 5min | 2 tasks | 1 files |
| Phase 24-duckdb-native-execution-integration-mvp P04 | 8min | 2 tasks | 1 files |
| Phase 24-duckdb-native-execution-integration-mvp P05 | 8min | 3 tasks | 7 files |
| Phase 25-native-equivalence-cache-and-fallback-hardening P02 | 10m32s | 3 tasks | 4 files |
| Phase 25-native-equivalence-cache-and-fallback-hardening P05 | ~15min | 3 tasks | 6 files |
| Phase 27-lance-parquet-archival-readability-dataset-ingress P01 | 13m | 3 tasks | 11 files |
| Phase 27-lance-parquet-archival-readability-dataset-ingress P02 | 6m | 3 tasks | 6 files |
| Phase 27-lance-parquet-archival-readability-dataset-ingress P03 | 5m23s | 3 tasks | 5 files |
| Phase 27-lance-parquet-archival-readability-dataset-ingress P04 | 62m | 3 tasks | 20 files |
| Phase 27-lance-parquet-archival-readability-dataset-ingress P05 | 57m | 3 tasks | 3 files |
| Phase 29-iceberg-ref-table-binding P01 | 4m | 3 tasks | 8 files |
| Phase 29-iceberg-ref-table-binding P03 | 5m37s | 3 tasks | 8 files |
| Phase 29-iceberg-ref-table-binding P04 | 9min | 3 tasks | 9 files |
| Phase 29-iceberg-ref-table-binding P05 | ~30min | 3 tasks | 5 files |
