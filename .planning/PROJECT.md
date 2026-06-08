# Loom — MVP1 (post-MVP0 distribution/verification track)

## What This Is

Loom is a distribution-oriented decoder IR: a deliberately non-Turing-complete,
total-function language whose only possible output is well-formed Apache Arrow
(full design in `design.md`). The original MVP0 DuckDB demo is complete. The
project is now in MVP1 / v3, focused on distribution containers, verifier-backed
safety, native-lowering preparation, complete-reader Vortex ingress, and the
post-native table/query-surface path.

## Core Value

A user can run a SQL query in DuckDB over Loom-decoded Vortex-style payloads,
including mixed-column table payloads, and get row/aggregate results that match
the expected decoded values. Real Vortex files can enter Loom through the Phase
18 complete-reader boundary, and later phases should preserve the verifier-gated,
fail-closed boundary as Loom grows toward native execution and table bindings.

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
- ✓ Full Loom Verifier foundation: a tiny `L2Core` spec, Rust executable verifier with stable diagnostics/facts, SMT-ready constraint IR, Lean/Rocq scaffold, TLA+ lifecycle invariant, and release-gated `scripts/full-verifier-test.sh` evidence without claiming complete production verification, native lowering safety, or real Vortex ingress — Phase 13
- ✓ MLIR/native lowering spike: `loom_core::native_lowering` requires accepted `verify_l2_core` reports plus `VerifiedArtifactFacts`, rejects unsupported programs fail-closed, emits deterministic textual MLIR for bounded Int32 copy, and gates typed primitive equivalence evidence through `scripts/native-lowering-test.sh` without mandatory MLIR/LLVM/JIT dependencies — Phase 14
- ✓ Real Vortex file/container ingress: isolated `loom-vortex-ingress` owns `vortex-file` usage, emits stable Loom-owned `VortexIngressReport` / `VortexFileFacts`, inspects real buffers/paths fail-closed, supports one generated non-null Int32 `.vortex` -> `LMC1` slice, exposes CLI inspection/emission, and gates the evidence through `scripts/vortex-ingress-test.sh` — Phase 15
- ✓ Full melior/LLVM/JIT backend boundary: optional `loom-native-melior` crate, toolchain facts, verifier-gated builder, MLIR validation pipeline, JIT boundary diagnostics, and skip-aware `scripts/melior-jit-test.sh` evidence for the bounded Int32 copy slice without claiming a production native compiler or host-engine native execution — Phase 16
- ✓ Unified artifact verification pipeline: `loom_core::artifact_verifier` verifies `LMC1` artifacts through container/manifest/L1 structural checks, optionally fuses accepted `L2Core` `VerifiedArtifactFacts`, records constraint status, reports lowering readiness, exposes `loom verify-artifact`, and gates the evidence through `scripts/artifact-verifier-test.sh` — Phase 17
- ✓ Complete Vortex reader boundary: isolated `loom-vortex-ingress` now emits recursive Loom-owned reader dtype/layout/segment/split facts, classifies accepted/unsupported/rejected inputs fail-closed, supports non-null Int32/Int64/Float32/Float64 single-column emission plus non-null primitive struct/table emission to verifier-accepted `LMC1`/`LMT1`, exposes CLI reader/artifact-verifier status, and gates the evidence through `scripts/complete-vortex-reader-test.sh` — Phase 18

### Active

<!-- Current scope. Building toward these. MVP1 hypotheses until shipped. -->

- [ ] Phase 19 research started: solver-backed full artifact verifier after complete-reader facts exist.
- [ ] Phase 20 remains a roadmap placeholder only: production MLIR decode dialect and native kernel expansion.
- [ ] Phase 21 remains a roadmap placeholder only: host native runtime ABI and execution policy over complete-reader and solver-backed verifier artifacts.
- [ ] Phase 22 remains a roadmap placeholder only: DuckDB native execution integration MVP over the Phase 21 runtime contract.
- [ ] Phase 23 remains a roadmap placeholder only: native equivalence, cache, and fallback hardening before table-format binding.
- [ ] Phase 24 remains a roadmap placeholder only: Iceberg ref/table binding after the hardened native execution contract is credible.
- [ ] Phase 25 remains a roadmap placeholder only: StarRocks + DuckDB dual query surface after Iceberg binding exists.

### Out of Scope

<!-- Explicit boundaries. Includes reasoning to prevent re-adding. -->

- Production MLIR `decode` dialect / arbitrary lowering / native-speed host execution — Phase 14 is only a verifier-gated textual lowering spike, Phase 16 is only optional bounded Int32 backend evidence, Phase 17 only unifies the artifact verifier report/facts pipeline, Phase 18 only establishes complete-reader facts and a finite emission matrix, and Phase 19 solver-backed verification should precede later production native phases (`design.md` §8)
- MLIR/native lowering correctness proof and arbitrary real Vortex ingress proof — Phase 14 is only a verifier-gated textual lowering spike; Phase 16 is only bounded backend evidence; Phase 17 does not add full SMT discharge or a stable external `L2Core` codec; Phase 18 provides a complete reader boundary but not a proof for arbitrary Vortex semantics; Phase 19 is reserved for solver-backed full artifact verifier work (`design.md` §5, §7, §13)
- Full arbitrary `.vortex` decode support for every encoding/layout/storage mode — Phase 18 records complete reader facts but emits Loom artifacts only for the explicit accepted matrix
- `statistics()` and `projection_mask` / `range` random-access parts of the ABI (`design.md` §9) — current implementation focuses on schema/decode and SQL smoke paths
- Content-hash URI, signatures, remote fetch, attestation, encryption, and native fast-path (`design.md` §10–11) — Phase 11 only starts the local versioned container boundary
- Correctness guarantees beyond matching the reference decoder — Loom guarantees safety + well-formedness, never correctness (`design.md` §7)

## Context

- **Origin doc:** `design.md` (Chinese) is the authoritative full design. MVP0 was the smallest slice that exercised the L1→L2-escape→Arrow→engine chain on real data; MVP1 is widening that proof toward distribution, verification, native lowering, real ingress, and table/query-surface integration.
- **Vortex is Rust-native** (SpiralDB). Choosing Rust for the decoder core lets Loom use Vortex crates in oracle/fixture/ingress boundaries while keeping `loom-core` and `loom-ffi` Vortex-free.
- **DuckDB extension path:** DuckDB is C++. The decoder is Rust. The seam between them is the Arrow C Data Interface — Rust produces an `ArrowArray`/`ArrowSchema`, the C++ table function adopts it zero-copy.
- **Design philosophy carried into MVP1:** "Anything that can be declared shouldn't be code." ~90% of a decoder is structural layout (L1, pure data, zero verification); only the genuinely computational ~10% drops into L2. The current work keeps shrinking and verifying the executable surface before widening backend and engine integration.
- **What MVP1 is *not* trying to prove yet:** native speed, arbitrary Vortex decode semantics, complete production verification of all future Loom artifacts, Iceberg table binding, or multi-engine query execution. Phase 12 covers only the current implemented byte-to-Arrow safety boundary; Phase 13 adds the future-verifier foundation; Phase 14 starts only a narrow verifier-gated textual lowering spike; Phase 16 adds optional bounded backend evidence only; Phase 17 unifies artifact verification but does not add real SMT discharge or a stable external `L2Core` codec; Phase 18 establishes complete-reader facts and a finite accepted emission matrix, not arbitrary Vortex support. Phase 19 is now the reserved slot for solver-backed full artifact verification after Phase 18 complete-reader facts exist.

## Constraints

- **Tech stack**: Rust decoder core (Arrow via arrow-rs) — chosen for Vortex-ecosystem alignment and a path toward the eventual safety/memory model.
- **Tech stack**: C++ DuckDB extension (table function) — same language as DuckDB; thinnest possible wrapper over the Rust core.
- **Interop**: Arrow C Data Interface as the Rust↔C++ FFI boundary — zero-copy, language-neutral, matches the design's "output is Arrow" contract.
- **Dependencies**: DuckDB (host engine + extension API); Apache Arrow (C Data Interface, arrow-rs); Vortex crates only in oracle/fixture/ingress boundaries, not in the core decode path.
- **Scope discipline**: MVP1 remains pre-production. Prefer narrow, verifier-gated vertical slices over broad format coverage or unverified execution paths.

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
| Phase 13 should use a layered full-verifier stack | The full verifier spans different problem classes: Rust executable diagnostics, local arithmetic/range proof, language soundness, and lifecycle invariants. Use Rust abstract interpretation + SMT + Lean/Rocq + TLA+ rather than betting on one formalism. | Complete — Phase 13 |
| Phase 14 should start with verifier-gated textual MLIR | The first native-lowering proof point must preserve the Phase 13 verifier boundary before taking on `melior`/LLVM/JIT/toolchain complexity. | Complete — Phase 14 |
| Phase 15 should remain before full `melior`/LLVM/JIT | Real Vortex file/container ingress should stabilize the artifact/layout evidence that later native lowering consumes; otherwise the backend risks overfitting the Phase 14 synthetic copy slice. | Complete — Phase 15 |
| Phase 16 should be the full `melior`/LLVM/JIT integration step | Programmatic MLIR, LLVM lowering, and JIT execution are the next backend step only after Phase 15 provides real-ingress shapes and Phase 14 preserves the verifier-gated handoff. Keep it optional and bounded to Int32 copy evidence. | Complete — Phase 16 |
| Phase 17 should unify artifact verification before production native expansion | The current payload structural verifier and future `L2Core` verifier foundation were parallel lines; lowering and engine work now have one fail-closed artifact report from container/schema/features through L1/L2 verification, facts, and lowering readiness. | Complete — Phase 17 |
| Phase 18 should complete the Vortex reader before solver-backed verifier and engine integration | Solver discharge and engine-integrated native execution need stable real artifact/fact/schema semantics; those should come from a complete reader boundary, not the Phase 15 narrow ingress slice. | Complete — Phase 18 |
| Phase 19 should add solver-backed full artifact verification before production native expansion | Complete-reader facts should exist first, and production native lowering should consume discharged verifier evidence rather than `CollectedOnly` obligations. | Research — Phase 19 |
| Phase 20 preserves production decode dialect/native kernel expansion | The unified and solver-backed verifier pipeline should feed a real production MLIR/native surface instead of skipping straight to host integration. | Placeholder — Phase 20 |
| Phase 21 should define host native runtime ABI before DuckDB integration | DuckDB should call a stable verifier-gated runtime contract instead of becoming the place where artifact identity, cache keys, fallback policy, and output ownership are first invented. | Placeholder — Phase 21 |
| Phase 22 should prove DuckDB native execution before broader table binding | DuckDB is the existing host seam and SQL gate, so it is the lowest-risk first native host integration over complete-reader artifacts. | Placeholder — Phase 22 |
| Phase 23 should harden equivalence, cache, and fallback before Iceberg | Iceberg metadata should point at a credible execution/artifact contract, not an experimental native path without oracle and negative evidence. | Placeholder — Phase 23 |
| Phase 24 should bind Iceberg refs/tables before adding dual query surfaces | Table metadata identity and verifier facts need one stable contract before StarRocks and DuckDB are compared as host query surfaces. | Placeholder — Phase 24 |
| Phase 25 should prove StarRocks + DuckDB over the same Loom/Iceberg-bound artifacts | The next engine story should avoid inventing a second artifact format and instead compare two query surfaces over one table binding. | Placeholder — Phase 25 |

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
*Last updated: 2026-06-08 after Phase 19 research start — complete Vortex reader boundary is release-gated; Phase 19 is now researching solver-backed full artifact verification before production native expansion.*
