# Loom — MVP0 (DuckDB demo)

## What This Is

Loom is a distribution-oriented decoder IR: a deliberately non-Turing-complete,
total-function language whose only possible output is well-formed Apache Arrow
(full design in `design.md`). **This project is MVP0** — a runnable prototype that
proves the core chain end-to-end on a real engine: Vortex-style encoded payloads
are decoded through Loom's declarative **L1 layout layer** plus one total-function
**L2 kernel (FSST)** into legal Arrow, handed to **DuckDB** via the Arrow C Data
Interface, and queried with SQL, including a small mixed-column table payload and ALP-style Float32/Float64 L2 coverage. It is for the author/systems audience evaluating
whether the L1/L2 + "output-as-typed-Arrow" idea actually works in practice.

## Core Value

A user can run a SQL query in DuckDB over Loom-decoded Vortex-style payloads,
including a mixed-column table payload, and get row/aggregate results that match
the expected decoded values.
If only one thing works, it is this end-to-end chain.

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

### Active

<!-- Current scope. Building toward these. MVP0 hypotheses until shipped. -->

- [ ] Phase 11: Distribution Container v0 (`DIST-01` through `DIST-05`)

### Out of Scope

<!-- Explicit boundaries. Includes reasoning to prevent re-adding. -->

- MLIR `decode` dialect / lowering to LLVM / native-speed codegen — MVP0 interprets directly; speed layer is the design's later act (`design.md` §8)
- Formal verifier and totality/termination proofs — Phase 9 is only a first-pass structural verifier; full formal proof remains later work (`design.md` §5, §7, §13)
- Full `.vortex` file layout (footer / layout tree / multi-chunk) — MVP0 decodes a single column, not a file container
- `statistics()` and `projection_mask` / `range` random-access parts of the ABI (`design.md` §9) — MVP0 implements only schema() + decode of the column
- Content-hash URI, signatures, remote fetch, attestation, encryption, and native fast-path (`design.md` §10–11) — Phase 11 only starts the local versioned container boundary
- Correctness guarantees beyond matching the reference decoder — Loom guarantees safety + well-formedness, never correctness (`design.md` §7)

## Context

- **Origin doc:** `design.md` (Chinese) is the authoritative full design. MVP0 is the smallest slice that exercises the L1→L2-escape→Arrow→engine chain on real data.
- **Vortex is Rust-native** (SpiralDB). Choosing Rust for the decoder core lets MVP0 reference Vortex's encoding definitions / crates directly rather than reverse-engineering a wire format.
- **DuckDB extension path:** DuckDB is C++. The decoder is Rust. The seam between them is the Arrow C Data Interface — Rust produces an `ArrowArray`/`ArrowSchema`, the C++ table function adopts it zero-copy.
- **Design philosophy carried into MVP0:** "Anything that can be declared shouldn't be code." ~90% of a decoder is structural layout (L1, pure data, zero verification); only the genuinely computational ~10% (here, FSST) drops into L2. MVP0 should make that split visible.
- **What MVP0 is *not* trying to prove:** sandbox safety, native speed, decades-long version stability. Those are the hard bones the design itself flags (`design.md` §13) and belong to later milestones.

## Constraints

- **Tech stack**: Rust decoder core (Arrow via arrow-rs) — chosen for Vortex-ecosystem alignment and a path toward the eventual safety/memory model.
- **Tech stack**: C++ DuckDB extension (table function) — same language as DuckDB; thinnest possible wrapper over the Rust core.
- **Interop**: Arrow C Data Interface as the Rust↔C++ FFI boundary — zero-copy, language-neutral, matches the design's "output is Arrow" contract.
- **Dependencies**: Vortex (as reference decoder for verification and as the source of the encoding to decode); DuckDB (host engine + extension API); Apache Arrow (C Data Interface, arrow-rs).
- **Scope discipline**: MVP0 is a feasibility prototype, not production. Prefer the narrowest path that produces a correct, demonstrable SQL result over generality.

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
| Defer the formal verifier / totality proof | MVP0 proves the decode chain, not the full sandbox proof; Phase 9 starts structural verification only | Formal proof still deferred |
| Phase 6 before descriptor/CLI | A clean baseline prevents v2 work from inheriting stale docs or fragile verification steps | Complete — Phase 6 |
| Phase 7 should prioritize descriptor/CLI before more kernels | Loom's next proof point is an independent, inspectable decoder contract rather than broader kernel coverage | Complete — Phase 7 |
| Descriptor format = RON for MVP0 | Recursive enum trees are clearer in RON than TOML; descriptor remains MVP0-scoped and unstable | Complete — Phase 7 |
| Phase 8 should prioritize table output before more kernels | Multi-column schema/row semantics are more load-bearing for Loom's engine story than adding another scalar kernel | Complete — Phase 8 |
| Keep direct DataChunk population for Phase 8 | Current FFI emits bare column arrays; `LMT1` can compose them into table output without introducing a new stream ABI | Complete — Phase 8 |
| Phase 9 should prioritize verifier MVP before more decode coverage | Safety is Loom's core claim; after SQL and table output work, the next missing proof point is fail-closed validation of untrusted payload descriptions | Complete — Phase 9 |
| Phase 10 should return to L2 numeric compression coverage | COV-01 was the remaining explicit v2 decode coverage item; ALP Float32/Float64 exercised the L2 path without jumping to MLIR or formal verification scope | Complete — Phase 10 |
| Phase 11 should introduce a distribution container before formal proof or lowering | The final Loom goal needs a stable artifact/trust boundary; formal verification, MLIR lowering, and real Vortex file ingress should target that boundary rather than raw MVP0 fixture payloads | Active — Phase 11 |

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
*Last updated: 2026-06-08 after Phase 11 research — active scope is Distribution Container v0. Formal verifier, MLIR/native lowering, and real Vortex file/container ingress are roadmap placeholders.*
