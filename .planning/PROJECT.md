# Loom — MVP2 / post-MVP1.5 coverage, native codegen, repositioning track

## What This Is

Loom is a distribution-oriented decoder IR: a deliberately non-Turing-complete,
total-function language whose only possible output is well-formed Apache Arrow
(full design in `design.md`). The original **MVP0** DuckDB demo is complete. **MVP1**
and **MVP1.5** are complete. The project is now in **MVP2**, focused on coverage
expansion, production native codegen stabilization, ABI freeze preparation,
distribution/security, and repositioning toward a decode-IR sidecar model.

The default Arrow semantic source-distribution artifact is `LMC2(LMA1)`. The
L2Core decode IR now has an independent content-hash identity (`l2ir:<hex>`)
decoupled from any container format (Phase 49). The next structural milestone
is the ABI freeze (Phase 51, moved from original Phase 44).

## Core Value

A user can run a SQL query in DuckDB over Loom-decoded artifacts, including
mixed-column table payloads and default source-backed `LMC2(LMA1)` Arrow
semantic artifacts, and get row/aggregate results that match the expected
decoded values. Parquet, Lance, and Vortex sources that materialize as Arrow
emit verifier-accepted `LMC2(LMA1)` distribution artifacts by default. Real Vortex
files can enter Loom through the complete-reader boundary. Full-projection,
unfiltered, one-batch nullable fixed-width primitive `LMC2(LMA1)` / direct-`LMA1`
artifacts route through production native codegen by default when the MLIR/JIT
backend is available. Every emitted artifact is verifier-gated and fail-closed.
The L2Core IR now has an independent content-hash identity decoupled from any
container (Phase 49), anchoring the project's repositioning toward a decode-IR
sidecar model.

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

- ✓ Sound FFI foundation — multi-crate Rust workspace (loom-core / loom-ffi / loom-fixtures), single unified arrow-rs version, `panic="unwind"` + boundary `catch_unwind` (live panic safety), System allocator, cbindgen-generated `loom.h` — Phase 1
- ✓ Rust core exports a real Arrow array across FFI via the Arrow C Data Interface (`to_ffi` + `ptr::write`, correct release ownership), verified by an outside-DuckDB roundtrip + release test — Phase 1
- ✓ Thin C++ DuckDB v1.5.3 extension (`loom_scan` table function) links `libloom_ffi.a`, calls `loom_decode`, and exposes the decoded column as a DuckDB-queryable table — `SELECT * FROM loom_scan('test.bin')` returns the decoded rows via an unsigned, footer-stamped extension — Phase 2 (Arrow→DuckDB import via direct DataChunk population; arrow_scan/stream path deferred to Phase 3 — see 02-CONTEXT.md D-01 REVISED)
- ✓ L1 decode core: `LayoutNode` model + `synthesized_read_loop` interpreter decoding Raw / BitPack / FrameOfReference with per-row validity routing, a from-scratch FastLanes transposed bit-unpack (zero vortex/fastlanes dependency — D-02), and typed Arrow `OutputBuilder` (Int32/Int64). `loom-fixtures` `vortex_reader`/`oracle` prove loom-core matches Vortex's own decoder row-for-row for bitpack + FOR (incl. nullable); no arm panics on malformed input — Phase 3
- ✓ Remaining L1 encodings and L2 escape: dictionary lookup, run-end expansion, Boolean builder support, `KernelEscape`, `L2KernelRegistry`, and the FOR-over-Raw reference fix are implemented and verified against Vortex fixtures — Phase 4
- ✓ FSST L2 kernel and dict-over-FSST path: Loom-owned FSST params decode UTF-8 strings through typed Arrow builders, with row-for-row Vortex oracle coverage — Phase 5
- ✓ MVP0 DuckDB acceptance gate: generated `.loom` payloads for bitpack, FOR, dict, RLE, FSST, and dict-over-FSST all pass exact SQL row and aggregate checks through `loom_scan` — Phase 5
- ✓ MVP0 release baseline: README and planning state reflect the completed MVP0 surface, `scripts/mvp0-verify.sh` runs the full release gate, and Phase 7 descriptor/CLI handoff notes are recorded — Phase 6
- ✓ Human-readable descriptor and CLI: RON descriptor text roundtrips through `LayoutDescription`, binary payloads can be inspected, `loom inspect`/`loom decode` expose reviewer workflows, fixture samples expanded, and illustrative Loom-vs-Vortex timing output is available — Phase 7
- ✓ Multi-column table output: `LMT1` table payloads wrap named `LMP1` column payloads, Rust and CLI can decode row-wise table output, DuckDB `loom_scan` returns mixed Int32/Boolean/Utf8 columns, and SQL row/projection/filter/aggregate checks are part of the release gate — Phase 8
- ✓ ArrowArrayStream decision: direct DuckDB DataChunk population remains the Phase 8 path; ArrowArrayStream is deferred until a later table/record-batch FFI ABI is introduced — Phase 8
- ✓ Verifier and safety boundary MVP: `loom_core::verifier` checks MVP0 layout/table descriptions with typed code/path/message diagnostics, Rust decode helpers and FFI ingress fail closed before Arrow output, `loom inspect` prints `verification: pass|fail`, and `scripts/mvp0-verify.sh` includes curated negative verifier coverage — Phase 9
- ✓ ALP Float32/Float64 L2 coverage: Loom-owned `AlpParams`, kernel id `1`, verifier checks, synthetic fixtures with Vortex primitive float oracle comparison, FFI roundtrips, CLI inspect/decode output, and DuckDB SQL smoke checks are complete — Phase 10
- ✓ Distribution Container v0: `LMC1` wraps existing `LMP1`/`LMT1` payloads with versioning, required/optional feature flags, checked sections, CLI visibility, generated fixture coverage, DuckDB SQL smoke coverage, and malformed-container release-gate coverage — Phase 11
- ✓ Formal verifier / Safety Proof MVP: the current `LMC1`/`LMP1`/`LMT1` byte-to-Arrow boundary has a safety contract, proof-obligation matrix, focused no-panic/fail-closed tests, final proof narrative, and release-gated `scripts/safety-proof-test.sh` evidence without claiming the future full Loom verifier — Phase 12
- ✓ Full Loom Verifier foundation: a tiny `L2Core` spec, Rust executable verifier with stable diagnostics/facts, SMT-ready constraint IR, Lean/Rocq scaffold, and release-gated `scripts/full-verifier-test.sh` evidence without claiming complete production verification, native lowering safety, real Vortex ingress, or load-bearing Lean soundness proof — Phase 13. The current Lean predicates `builder_events_typed` and `no_ambient_authority` are `True` placeholders; real verifier evidence is Rust + Phase 19 Bitwuzla discharge.
- ✓ MLIR/native lowering spike: `loom_core::native_lowering` requires accepted `verify_l2_core` reports plus `VerifiedArtifactFacts`, rejects unsupported programs fail-closed, emits deterministic textual MLIR for bounded Int32 copy, and gates typed primitive equivalence plus managed LLVM/MLIR validation through `scripts/native-lowering-test.sh` — Phase 14
- ✓ Real Vortex file/container ingress: isolated `loom-vortex-ingress` owns `vortex-file` usage, emits stable Loom-owned `VortexIngressReport` / `VortexFileFacts`, inspects real buffers/paths fail-closed, supports one generated non-null Int32 `.vortex` -> `LMC1` slice, exposes CLI inspection/emission, and gates the evidence through `scripts/vortex-ingress-test.sh` — Phase 15
- ✓ Full melior/LLVM/JIT backend boundary: optional `loom-native-melior` crate, toolchain facts, verifier-gated builder, MLIR validation pipeline, JIT boundary diagnostics, and managed LLVM/MLIR evidence for the bounded Int32 copy slice without claiming a production native compiler or host-engine native execution — Phase 16
- ✓ Unified artifact verification pipeline: `loom_core::artifact_verifier` verifies `LMC1` artifacts through container/manifest/L1 structural checks, optionally fuses accepted `L2Core` `VerifiedArtifactFacts`, records constraint status, reports lowering readiness, exposes `loom verify-artifact`, and gates the evidence through `scripts/artifact-verifier-test.sh` — Phase 17
- ✓ Complete Vortex reader boundary: isolated `loom-vortex-ingress` now emits recursive Loom-owned reader dtype/layout/segment/split facts, classifies accepted/unsupported/rejected inputs fail-closed, supports non-null Int32/Int64/Float32/Float64 single-column emission plus non-null primitive struct/table emission to verifier-accepted `LMC1`/`LMT1`, exposes CLI reader/artifact-verifier status, and gates the evidence through `scripts/complete-vortex-reader-test.sh` — Phase 18
- ✓ Solver-backed full artifact verifier: solver-neutral obligation/report types, deterministic Bitwuzla-primary `QF_BV` SMT-LIB emission, optional `loom-solver-smt` backend declarations for `z3`/`cvc5`/`bitwuzla`, managed Bitwuzla subprocess discharge, artifact-verifier solver facts, CLI visibility, and release-gated solver evidence are complete without claiming production native execution — Phase 19
- ✓ Production decode dialect and native kernel expansion seed: production lowering starts from accepted artifact reports with `Discharged`/`NotRequired` facts, emits deterministic `loom.decode` textual artifacts, plans primitive Arrow/raw-buffer builders, supports raw non-null Int32/Int64/Float32/Float64 single/table slices, validates standard MLIR text, and gates evidence through `scripts/production-native-lowering-test.sh` without claiming host execution or arbitrary encoding coverage — Phase 20
- ✓ Expanded Vortex encoding coverage: real Vortex reader facts now include coverage/emission/lowering disposition, nullable primitives fail closed with null-preserving oracle evidence, chunked/dictionary/RLE/bitpack/FOR fixtures have row oracle evidence and canonical raw emission where safe, string/compression cases remain explicit deferrals, and `scripts/vortex-encoding-coverage-test.sh` gates the matrix without claiming arbitrary Vortex support — Phase 21
- ✓ Host native runtime ABI and execution policy: host-neutral runtime model, native/interpreter/fail-closed decision policy, projection/predicate/split/concurrency planning, cache identity, diagnostics, and C ABI sketch are complete and gated through `scripts/runtime-abi-test.sh` — Phase 22
- ✓ Production native backend implementation: `loom-native-melior` consumes Phase 22 runtime plans/cache identity, validates `loom.decode` dialect artifacts, runs production MLIR/LLVM preparation, seeds verifier-gated JIT execution for supported primitive kernels, and gates evidence through `scripts/production-backend-test.sh` — Phase 23
- ✓ DuckDB native execution integration MVP: public `loom_scan(path)` routes eligible complete-reader artifacts through Phase 22 runtime policy and Phase 23 backend, preserves interpreter fallback/fail-closed diagnostics/direct DataChunk output, passes projected column ids into runtime projection/cache input, and gates evidence through `scripts/duckdb-native-integration-test.sh` plus `scripts/mvp0-verify.sh` — Phase 24
- ✓ Native equivalence, cache, and fallback hardening: public `loom_scan(path)` now has release-gated evidence for supported non-null primitive native equivalence, same-process in-process cache miss/insert/hit smoke behavior, key-driven invalidation, unsupported-route fallback/strict fail-closed diagnostics, malformed/cancel/mismatch recovery, and a final bounded report at `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-NATIVE-HARDENING-REPORT.md` — Phase 25
- ✓ DuckDB Arrow semantic SQL surface for `LMC2(LMA1)`: public `loom_scan(path)` now binds and scans default wrapped Arrow semantic artifacts directly, supports one-batch multi-column primitive/Utf8/Boolean nullable SQL with projection/filter/aggregate/null evidence, keeps direct `LMA1` as regression-only bridge input, and fails closed with explicit unsupported diagnostics for Date32 logical and Struct nested fixtures — Phase 34
- ✓ Native Arrow semantic execution: `loom_core::native_arrow_semantic` verifier-gates default `LMC2(LMA1)` and explicit direct `LMA1`, copies one-batch nullable fixed-width primitive Boolean/Int32/Int64/Float32/Float64 columns through typed builders into a new Arrow `RecordBatch`, exposes explicit native/reference equivalence and mismatch diagnostics, records engine-neutral runtime/cache identity, and gates fail-closed unsupported Utf8/logical/nested/multi-batch behavior through `scripts/native-arrow-semantic-execution-test.sh` and `scripts/mvp1-verify.sh` — Phase 35
- ✓ Verified-lineage contract: MVP1.5 now has a normative contract that defines "verified" as safety + Arrow well-formedness evidence lineage only, maps each safety claim to exactly one layer (Rust verifier structural check, Bitwuzla SMT discharge, Lean soundness theorem, differential validation, or explicit TCB trust assumption), declares the Rust/std, LLVM/MLIR, Rust↔C ABI, DuckDB host, and Arrow C Data Interface TCB, and assigns Lean↔Rust verifier, static↔dynamic, modeled-executor↔real-executor, and native↔model seams to Phase 37-40 or TCB — Phase 36
- ✓ Lean Rust verifier correspondence: `formal/lean/LoomCore.lean` now mirrors the Rust verifier's current static L2Core surface for `ScalarExpr` / `LetScalar`, scalar environment typing, expression-derived append value typing, and unknown-variable rejection; `scripts/lean-rust-correspondence-test.sh` diffs Lean and Rust accept/reject plus reject-code classifications over the current verifier matrix plus deterministic fuzz cases and is wired into `scripts/full-verifier-test.sh` — Phase 37
- ✓ Lean modeled operational semantics and soundness theorem: `formal/lean/LoomCore.lean` now contains a proof-friendly modeled executor, fail-closed terminal semantics, and modeled safety predicates over `execProgram p`. Out-of-bounds reads are representable as `inBounds := false` and fail-close rejected/unverified modeled runs. The no-`sorry` semantic `accepted_program_safe : Verified p -> ModeledExecutionSafe p` theorem now uses `verified_program_finishes` / `verified_program_reads_in_bounds` to prove `Verified p` reaches `.finished` and all recorded reads are in bounds, instead of accepting the fail-closed read-safety disjunction as the dynamic result; `scripts/full-verifier-test.sh` checks the theorem marker, bridge markers, modeled-only scope note, no `_state`/discarded-premise/direct-readSafety/all-reads-in-bounds invariant regression, and no-sorry policy — Phase 38
- ✓ Model/Rust interpreter consistency: `loom_core::l2_core_reference_executor` provides a separate Rust transcription of the Lean modeled executor, and `scripts/model-rust-interpreter-consistency-test.sh` compares reference and observer-only production trace-subject builder-event/fail-closed traces over a deterministic matrix; this is per-run differential validation, not an all-program proof or native/model validation — Phase 39
- ✓ Native/model validation: `loom_core::native_arrow_semantic` now validates Phase 35 native Arrow semantic output against Phase 39 reference-executor builder-event traces and decoded Arrow value equivalence for default `LMC2(LMA1)` plus explicit direct `LMA1` nullable Boolean/Int32/Int64/Float32/Float64 one-batch primitives. Validation-aware runtime/cache helpers require successful native/model validation, divergent traces fail closed and cannot seed native cache identity, and `scripts/native-model-validation-test.sh` is wired into `scripts/full-verifier-test.sh`; MLIR/LLVM/native lowering remains a permanent TCB assumption and this is per-run translation validation, not verified compilation — Phase 40
- ✓ Verified-lineage closeout: `scripts/verified-lineage-test.sh` runs the MVP1.5 lineage matrix, and `loom_core::verified_lineage` can produce accepted-artifact safety provenance records naming verifier, solver, Lean, differential-validation evidence, and explicit TCB assumptions without claiming correctness, verified compilation, production readiness, or signed attestation transport — Phase 41

### Active

<!-- Current scope. Building toward these. -->

- [ ] MVP2 Phase 44 (MVP1.5 Closeout and Milestone Archive) — placeholder, spec before planning.
- [ ] MVP2 Phase 51 (ABI Freeze and Compatibility Contract) — freeze IR semantics and I/O contract with versioned compatibility policy.
- [ ] Repositioning Phase 50.1 (Container Demotion and Thin Host Adapters) — depends on Phase 49.
- [ ] Repositioning Phase 50 (Sidecar Overlay Model) — depends on Phase 50.1 and Phase 49.
- [ ] Distribution, signing, remote fetch, encryption, and GA hardening are deferred to later phases.

### Out of Scope

<!-- Explicit boundaries. Includes reasoning to prevent re-adding. -->

- Correctness guarantees beyond matching the reference decoder — Loom guarantees safety + well-formedness, never correctness (`design.md` §7)
- Verified compilation of the MLIR/LLVM toolchain — stays in the TCB permanently; per-run translation validation only (`design.md` §5)
- Live StarRocks runtime integration (`ENGINE-01`) — suspended pending external runtime/client availability; deferred to pre-GA reactivation
- Content-hash URI, signatures, remote fetch, attestation, encryption (`design.md` §10–11) — deferred to Phases 45–46 after ABI freeze
- `statistics()` and `projection_mask` / `range` random-access parts of the ABI (`design.md` §9)
- Wasm fallback track — rejected in repositioning (整理稿 §2.2); cross-arch solved by LLVM, sandbox contradicts verifiable safety
- PKI / key-management product — integrate an existing trust root; do not build one

## Context

- **Origin doc:** `design.md` (Chinese) is the authoritative full design. MVP0 was the smallest slice that exercised the L1→L2-escape→Arrow→engine chain on real data; MVP1 is widening that proof toward distribution, verification, native lowering, real ingress, and table/query-surface integration.
- **Vortex is Rust-native** (SpiralDB). Choosing Rust for the decoder core lets Loom use Vortex crates in oracle/fixture/ingress boundaries while keeping `loom-core` and `loom-ffi` Vortex-free.
- **DuckDB extension path:** DuckDB is C++. The decoder is Rust. The seam between them is the Arrow C Data Interface — Rust produces an `ArrowArray`/`ArrowSchema`, the C++ table function adopts it zero-copy.
- **Design philosophy carried into MVP2:** "Anything that can be declared shouldn't be code." ~90% of a decoder is structural layout (L1, pure data, zero verification); only the genuinely computational ~10% drops into L2. The current work keeps shrinking and verifying the executable surface while repositioning toward a decode-IR sidecar model.
- **What MVP2 is *not* trying to prove yet:** Live StarRocks runtime integration (suspended), Wasm fallback (rejected in repositioning), verified MLIR/LLVM compilation (permanent TCB), arbitrary Vortex encoding coverage, or GA production readiness.

## Constraints

- **Tech stack**: Rust decoder core (Arrow via arrow-rs) — chosen for Vortex-ecosystem alignment and a path toward the eventual safety/memory model.
- **Tech stack**: C++ DuckDB extension (table function) — same language as DuckDB; thinnest possible wrapper over the Rust core.
- **Interop**: Arrow C Data Interface as the Rust↔C++ FFI boundary — zero-copy, language-neutral, matches the design's "output is Arrow" contract.
- **Dependencies**: DuckDB (host engine + extension API); Apache Arrow (C Data Interface, arrow-rs); Vortex crates only in oracle/fixture/ingress boundaries, not in the core decode path.
- **Scope discipline**: MVP2 remains pre-production. Prefer narrow, verifier-gated vertical slices over broad format coverage or unverified execution paths.

## Key Decisions

<!-- Decisions that constrain future work. -->

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Build MVP0 on DuckDB as the host engine | Real engine to prove the chain end-to-end; "runnable prototype first" | Complete — Phase 5 |
| Decoder core in Rust | Vortex is Rust-native; path to eventual memory/safety model | Complete — Phase 5 |
| Integrate via a C++ DuckDB extension (table function) | Most engine-native interface; truest to "code travels with data" | Complete — Phase 5 |
| Rust↔C++ bridge = Arrow C Data Interface | Zero-copy, language-neutral, matches Loom's Arrow-only output contract | Complete for single-column MVP0 |
| Target format = Vortex, single encoded column | Real-world encodings; bounded scope; closest to the design's worked example | Complete — Phase 5 |
| Scope = L1 (bitpack/FOR/dict/RLE) + one L2 kernel | Smallest set that demonstrates the declarative layer *and* the L2 escape | Complete — Phase 5 |
| L2 kernel = FSST string decompression | Canonical "can't be declared, must compute" case in the Vortex world | Complete — Phase 5 |
| Additional L2 kernel = ALP-style Float32/Float64 decode | Exercises a second real kernel family and numeric compression coverage while keeping `loom-core` Vortex-free | Complete — Phase 10 |
| Interpret directly; no MLIR in MVP0 | Prove correctness/feasibility now; native speed is a later act | Complete — Phase 5 |
| Acceptance = DuckDB SQL results match Vortex's decoder row-for-row | Concrete, end-to-end, falsifiable success bar | Complete — Phase 5 |
| Defer full future-IR formal proof, but complete current-boundary safety proof | MVP0 proves the decode chain; Phase 12 targets only the implemented `LMC1`/`LMP1`/`LMT1` byte-to-Arrow safety boundary with executable evidence | Complete — Phase 12 |
| Phase 6 before descriptor/CLI | A clean baseline prevents v2 work from inheriting stale docs or fragile verification steps | Complete — Phase 6 |
| Phase 7 should prioritize descriptor/CLI before more kernels | Loom's next proof point is an independent, inspectable decoder contract rather than broader kernel coverage | Complete — Phase 7 |
| Descriptor format = RON for MVP0 | Recursive enum trees are clearer in RON than TOML; descriptor remains MVP0-scoped and unstable | Complete — Phase 7 |
| Phase 8 should prioritize table output before more kernels | Multi-column schema/row semantics are more load-bearing for Loom's engine story than adding another scalar kernel | Complete — Phase 8 |
| Keep direct DataChunk population for Phase 8 | Current FFI emits bare column arrays; `LMT1` can compose them into table output without introducing a new stream ABI | Complete — Phase 8 |
| Phase 9 should prioritize verifier MVP before more decode coverage | Safety is Loom's core claim; after SQL and table output work, the next missing proof point is fail-closed validation of untrusted payload descriptions | Complete — Phase 9 |
| Phase 10 should return to L2 numeric compression coverage | COV-01 was the remaining explicit v2 decode coverage item; ALP Float32/Float64 exercised the L2 path without jumping to MLIR or formal verification scope | Complete — Phase 10 |
| Phase 11 should introduce a distribution container before formal proof or lowering | The final Loom goal needs a stable artifact/trust boundary; formal verification, MLIR lowering, and real Vortex file ingress should target that boundary rather than raw MVP0 fixture payloads | Complete — Phase 11 |
| Phase 12 should use obligation matrix + executable gates, not a theorem prover | Current code already has verifier diagnostics, fail-closed decode helpers, `LMC1`, negative gates, and FFI panic containment; a theorem prover would expand scope before the future IR exists | Complete — Phase 12 |
| Phase 13 should use a layered full-verifier stack | The full verifier spans different problem classes: Rust executable diagnostics, local arithmetic/range proof, language soundness, and lifecycle invariants. Use Rust abstract interpretation + SMT + Lean/Rocq rather than betting on one formalism. | Complete — Phase 13 |
| Phase 37 closes Lean/Rust verifier correspondence for the static checker slice | `formal/lean/LoomCore.lean` now models `ScalarExpr` / `LetScalar`, scalar environment typing, expression-derived append typing, and unknown-variable rejection, while `scripts/lean-rust-correspondence-test.sh` diffs Lean and Rust classifications. Overflow/range proof obligations and non-row budgets remain Rust/Bitwuzla evidence, and semantic soundness remains Phase 38. | Complete — Phase 37 |
| Phase 38 soundness is modeled-executor-only | The no-`sorry` `accepted_program_safe` theorem proves `Verified p -> ModeledExecutionSafe p` for the Lean modeled executor via `verified_program_finishes` / `verified_program_reads_in_bounds`: accepted programs finish and every recorded read is in bounds. The model can still record `inBounds := false` and fail close for rejected/unverified runs; it does not prove Rust interpreter behavior, native behavior, source correctness, performance, compiler correctness, or ABI/host correctness. | Complete — Phase 38 |
| Phase 39 is per-run differential validation | The reference executor and observer-only production trace subject compare traces across a deterministic corpus. Passing the gate validates the matrix and catches divergence; it is not a verified compilation proof or a native/model equivalence claim. | Complete — Phase 39 |
| Phase 14 should start with verifier-gated textual MLIR | The first native-lowering proof point must preserve the Phase 13 verifier boundary before taking on `melior`/LLVM/JIT/toolchain complexity. | Complete — Phase 14 |
| Phase 15 should remain before full `melior`/LLVM/JIT | Real Vortex file/container ingress should stabilize the artifact/layout evidence that later native lowering consumes; otherwise the backend risks overfitting the Phase 14 synthetic copy slice. | Complete — Phase 15 |
| Phase 16 should be the full `melior`/LLVM/JIT integration step | Programmatic MLIR, LLVM lowering, and JIT execution are the next backend step only after Phase 15 provides real-ingress shapes and Phase 14 preserves the verifier-gated handoff. Keep it optional and bounded to Int32 copy evidence. | Complete — Phase 16 |
| Phase 17 should unify artifact verification before production native expansion | The current payload structural verifier and future `L2Core` verifier foundation were parallel lines; lowering and engine work now have one fail-closed artifact report from container/schema/features through L1/L2 verification, facts, and lowering readiness. | Complete — Phase 17 |
| Phase 18 should complete the Vortex reader before solver-backed verifier and engine integration | Solver discharge and engine-integrated native execution need stable real artifact/fact/schema semantics; those should come from a complete reader boundary, not the Phase 15 narrow ingress slice. | Complete — Phase 18 |
| Phase 19 should add solver-backed full artifact verification before production native expansion | Complete-reader facts should exist first, and production native lowering should consume discharged verifier evidence rather than `CollectedOnly` obligations. Phase 19 implemented a Bitwuzla-primary `QF_BV` solver path with `z3`/`cvc5` backend declarations. | Complete — Phase 19 |
| Phase 20 is a production lowering seed, not the full production backend | The unified and solver-backed verifier pipeline needs a first verifier-gated `loom.decode`/standard-MLIR/native-lowering surface, but compiled ODS dialect registration, production `melior` pass pipeline, LLVM lowering, and LLVM/JIT execution should remain out of Phase 20. | Complete — Phase 20 |
| Phase 21 should expand Vortex encoding coverage with paired lowering disposition | Broader real Vortex support should consume solver-backed verifier evidence and the Phase 20 lowering seed; every new encoding/layout must be classified as interpreter-only, lowering-supported with a dialect/native delta, or fail-closed/deferred. | Complete — Phase 21 |
| Phase 22 should define host native runtime ABI before DuckDB integration | DuckDB should call a stable verifier-gated runtime contract instead of becoming the place where artifact identity, cache keys, fallback policy, and output ownership are first invented. | Complete — Phase 22 |
| Phase 23 should implement the production native backend before DuckDB integration | After ABI/policy is explicit, the real compiled `loom.decode` ODS dialect, `melior` pass pipeline, LLVM lowering, and verifier-gated LLVM/JIT execution backend should exist before a host engine depends on native execution. | Complete — Phase 23 |
| Phase 24 should prove DuckDB native execution before broader table binding | DuckDB is the existing host seam and SQL gate, so it is the lowest-risk first native host integration over complete-reader artifacts and the Phase 23 production backend. | Complete — Phase 24 |
| Phase 25 should harden equivalence, cache, and fallback before source/table binding | Downstream metadata should point at a credible execution/artifact contract, not an experimental native path without oracle and negative evidence. | Complete — Phase 25 |
| Phase 26 should define external source ingress before archival/table formats | Source identity and ingestion trust boundaries need one stable contract before Lance, Parquet, and Iceberg bindings build on them. | Next active focus — Phase 26 |
| Phase 27 should prove Lance + Parquet archival readability before Iceberg refs | Dataset/archive readability should be validated before introducing table metadata and ref semantics. | Placeholder — Phase 27 |
| Phase 28 should bind Iceberg refs/tables before adding dual query surfaces | Table metadata identity and verifier facts need one stable contract before StarRocks and DuckDB are compared as host query surfaces. | Placeholder — Phase 28 |
| Phase 29 should prove StarRocks + DuckDB over the same Loom/Iceberg-bound artifacts | The next engine story should avoid inventing a second artifact format and instead compare two query surfaces over one table binding. | Skipped/deferred by user request — Phase 29 |
| Phase 30 should own arbitrary Vortex semantic compatibility | Full Vortex coverage spans too many encoding families, layout wrappers, storage modes, null/nested semantics, pushdown interactions, and oracle matrices to hide inside Phase 21, Phase 23, or a host-engine integration phase. Because Phase 29 was skipped, Phase 30 must not rely on dual-query evidence. | Starting by user override — Phase 30 |
| Phase 33 should settle `LMC2(LMA1)` before broader query/native claims | The distribution contract must be explicit before DuckDB and native backends decide whether they consume direct `LMA1` or wrapped artifacts. | Complete — Phase 33 |
| Phase 34 should make DuckDB consume default `LMC2(LMA1)` before native Arrow semantic claims | Queryability and native execution are different evidence layers; DuckDB now unwraps/scans default artifacts through interpreter-backed Arrow C Data while Phase 35 remains engine-neutral native execution. | Complete — Phase 34 |
| Phase 35 should remain engine-neutral and separate from DuckDB SQL | Native correctness should be proven as verifier-gated Arrow semantic execution with explicit equivalence/runtime/cache evidence before any host consumes it. | Complete — Phase 35 |
| Phase 36 should pin "verified" before proof work | MVP1.5 must not let bounded/scaffolded/skipped evidence drift into broad correctness language; every verified claim now maps to one named evidence layer or explicit TCB trust assumption. | Complete — Phase 36 |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-06-11 after Phase 49 closeout — independent L2Core IR codec and content-hash identity complete; Phase 44 reorganized to MVP1.5 Closeout placeholder, ABI Freeze moved to Phase 51.*
