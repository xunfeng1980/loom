# Loom — MVP0 (DuckDB demo)

## What This Is

Loom is a distribution-oriented decoder IR: a deliberately non-Turing-complete,
total-function language whose only possible output is well-formed Apache Arrow
(full design in `design.md`). **This project is MVP0** — a runnable prototype that
proves the core chain end-to-end on a real engine: a single Vortex-encoded column
is decoded through Loom's declarative **L1 layout layer** plus one total-function
**L2 kernel (FSST)** into legal Arrow, handed to **DuckDB** via the Arrow C Data
Interface, and queried with SQL. It is for the author/systems audience evaluating
whether the L1/L2 + "output-as-typed-Arrow" idea actually works in practice.

## Core Value

A user can run a SQL query in DuckDB over a Vortex-encoded column that was decoded
by the Loom interpreter, and get results that match Vortex's own decoder row-for-row.
If only one thing works, it is this end-to-end chain.

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

- ✓ Sound FFI foundation — multi-crate Rust workspace (loom-core / loom-ffi / loom-fixtures), single unified arrow-rs version, `panic="unwind"` + boundary `catch_unwind` (live panic safety), System allocator, cbindgen-generated `loom.h` — Phase 1
- ✓ Rust core exports a real Arrow array across FFI via the Arrow C Data Interface (`to_ffi` + `ptr::write`, correct release ownership), verified by an outside-DuckDB roundtrip + release test — Phase 1

### Active

<!-- Current scope. Building toward these. MVP0 hypotheses until shipped. -->

- [ ] Rust decoder core that interprets an L1 declarative layout description (no codegen, no MLIR)
- [ ] L1 built-in declarative encodings: bit-packing, FOR (frame-of-reference), dictionary, RLE — pure data, decoded by a synthesized read loop
- [ ] L2 escape mechanism: an L1 segment can reference an L2 kernel by id
- [ ] One L2 total-function kernel: FSST string decompression
- [ ] Decoder produces well-formed Arrow via typed builder operations (append_value / append_null / list / struct), materialized as ArrowArray + ArrowSchema
- [ ] Thin C++ DuckDB extension (table function) that invokes the Rust decoder and exposes the decoded column as a DuckDB-queryable table
- [ ] Input: a single serialized Vortex encoded array/column (one of the L1-expressible encodings, or FSST-encoded strings)
- [ ] Verification harness: DuckDB SELECT/aggregate over the Loom-decoded column matches Vortex's official decoder row-for-row

### Out of Scope

<!-- Explicit boundaries. Includes reasoning to prevent re-adding. -->

- MLIR `decode` dialect / lowering to LLVM / native-speed codegen — MVP0 interprets directly; speed layer is the design's later act (`design.md` §8)
- Formal verifier, totality/termination proofs, the safety-boundary demo (rejecting out-of-bounds / non-terminating L1/L2 input) — the chosen acceptance bar is "correct query results"; the verifier is a later phase (`design.md` §5, §7, §13)
- Full `.vortex` file layout (footer / layout tree / multi-chunk) — MVP0 decodes a single column, not a file container
- Multi-column tables and schema assembly across columns — single column first
- Additional L2 kernels (ALP float decode, decompression blocks, etc.) — one kernel (FSST) is enough to prove the L2 escape
- `statistics()` and `projection_mask` / `range` random-access parts of the ABI (`design.md` §9) — MVP0 implements only schema() + decode of the column
- Versioned distribution container, feature flags, content-hash URI, native fast-path (`design.md` §10–11) — distribution concerns come after the decode chain works
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
| Build MVP0 on DuckDB as the host engine | Real engine to prove the chain end-to-end; "runnable prototype first" | — Pending |
| Decoder core in Rust | Vortex is Rust-native; path to eventual memory/safety model | — Pending |
| Integrate via a C++ DuckDB extension (table function) | Most engine-native interface; truest to "code travels with data" | — Pending |
| Rust↔C++ bridge = Arrow C Data Interface | Zero-copy, language-neutral, matches Loom's Arrow-only output contract | — Pending |
| Target format = Vortex, single encoded column | Real-world encodings; bounded scope; closest to the design's worked example | — Pending |
| Scope = L1 (bitpack/FOR/dict/RLE) + one L2 kernel | Smallest set that demonstrates the declarative layer *and* the L2 escape | — Pending |
| L2 kernel = FSST string decompression | Canonical "can't be declared, must compute" case in the Vortex world | — Pending |
| Interpret directly; no MLIR in MVP0 | Prove correctness/feasibility now; native speed is a later act | — Pending |
| Acceptance = DuckDB SQL results match Vortex's decoder row-for-row | Concrete, end-to-end, falsifiable success bar | — Pending |
| Defer the verifier / safety-boundary demo | MVP0 proves the decode chain, not the sandbox; safety is a later milestone | — Pending |

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
*Last updated: 2026-06-07 after Phase 1 (Scaffold and FFI Boundary) — verification passed; one code-review BLOCKER (CR-01: panic strategy) found and fixed before completion.*
