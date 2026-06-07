# Phase 3: L1 Bitpack, FOR, and Arrow Builders - Context

**Gathered:** 2026-06-07
**Status:** Ready for planning

<domain>
## Phase Boundary

The first phase with **real decode logic**. Build loom-core's L1 declarative layer: a `LayoutNode` data model, a synthesized read-loop interpreter, the BitPack and FrameOfReference decoders (nulls preserved), and the typed Arrow builder output. `vortex_reader` turns a single in-memory Vortex encoded array into a `LayoutNode` + buffer refs; the read loop decodes it; `arrow_builder_output` emits a well-formed Arrow array exportable via `to_ffi`.

Requirements: INPUT-01 (read a single serialized Vortex encoded column, no `.vortex` file), INPUT-02 (programmatic in-memory fixtures), L1-01 (`LayoutNode` pure-data model), L1-02 (synthesized read loop), L1-03 (bit-packed decode), L1-04 (FOR decode), L1-07 (null/validity preserved), ARROW-01 (typed builders only), ARROW-02 (`ArrayData` → ArrowArray/ArrowSchema).

**Not this phase:** Dictionary / RLE / L2 escape (Phase 4); FSST + the row-for-row verification harness + DuckDB-SQL-shows-real-data (Phase 5). Phase 3 verifies at the **Rust + `to_ffi`** level; `loom_decode`/`loom_scan` keep their Phase-2 hardcoded output (see D-03 below).

</domain>

<decisions>
## Implementation Decisions

### D-01 — Bitpack fidelity: real FastLanes layout, no patches
Decode the **genuine `vortex-fastlanes` 1024-lane TRANSPOSED bit-packing layout** row-for-row (not naive sequential LSB unpacking) — so loom-core decodes a real Vortex `BitPackedArray` and matches Vortex's own decode (honors INPUT-01). **Scope bound:** Phase-3 fixtures are restricted to values that fit the declared bit width, so the **exception/"patch" path** (out-of-width values stored separately) is **deferred** (see Deferred). Implement the FastLanes transpose/unpack for the in-width case.

### D-02 — vortex_reader derives the LayoutNode
`vortex_reader` inspects the Vortex `ArrayRef` (encoding id, `bit_width`, packed buffer, validity, FOR reference scalar) and emits a `LayoutNode` + raw buffer references. **loom-core decodes from the `LayoutNode` with ZERO `vortex-*` dependency** — Vortex stays isolated inside `vortex_reader` (and the oracle), preserving D-02 from Phase 1. This is the honest reading of INPUT-01 ("a Vortex encoded array is read into the decoder").

### D-03 — Phase 3 stays loom-core + FFI-exportable; DuckDB rewire deferred to Phase 5
Phase 3's deliverable is the decode core whose Arrow output is FFI-exportable (`arrow_builder_output::finish()` → `ArrayData` → `to_ffi`, success criterion 4). **`loom_decode`/`loom_scan` keep the Phase-2 hardcoded `[1,2,3,null]` path** this phase. The DuckDB-SQL-shows-real-decoded-data work AND the **deferred arrow_scan/record-batch revisit** (from 02-CONTEXT.md "D-01 REVISED") both land in **Phase 5** (VERIFY-03), where they naturally belong. This keeps a clean phase boundary and avoids reopening arrow_scan prematurely.

### D-04 — Define the full LayoutNode enum now; stub unimplemented arms
Define the complete `LayoutNode` enum now — `Raw`, `BitPack`, `FrameOfReference`, `Dictionary`, `RunEnd`, `KernelEscape` (per research/ARCHITECTURE.md). **Implement `Raw` / `BitPack` / `FrameOfReference`** this phase; `Dictionary` / `RunEnd` / `KernelEscape` arms return an explicit "unimplemented in Phase 3" error (not silent/`todo!()` panic — a typed error the read loop surfaces). It's pure data, cheap to define, makes L1-01 genuinely complete, and lets Phases 4–5 just fill arms without churning the model.

### Claude's Discretion
- Exact `LayoutNode` field shapes and how `FrameOfReference` nests over `BitPack` (FOR = unpack bit-packed deltas, then broadcast-add the reference scalar — research/FEATURES.md). The read loop is a recursive `match` interpreter over the tree (research/ARCHITECTURE.md).
- Validity → Arrow null bitmap mapping: for Phase 3 keep validity as a **plain (non-encoded) bitmap** read by `vortex_reader`; recursive/encoded validity is deferred.
- In-phase verification approach: Rust unit tests asserting decoded values against known expected arrays, and/or an in-test comparison to Vortex's own `into_canonical().into_arrow()`. The full standalone oracle harness is Phase 5 (VERIFY-01/02).
- Which integer width(s) to demonstrate (e.g. the 11-bit non-byte-aligned case from success criterion 1) and whether to seed `arrow_builder_output` from the existing `Int32Builder` pattern in `crates/loom-ffi/src/ffi.rs`.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design (authoritative L1 model)
- `design.md` §4 (L1 declarative layout layer — the physical layout tree, declarative encodings incl. bit-packing/FOR), §5.2 (parallelism as structure / FastLanes "unified virtual ISA" insight behind the transposed layout), §6 (typed builder output → construction-is-valid)
- `.planning/REQUIREMENTS.md` — INPUT-01/02, L1-01/02/03/04/07, ARROW-01/02
- `.planning/ROADMAP.md` Phase 3 — goal + 5 success criteria

### Stack / Architecture / Pitfalls (project research)
- `.planning/research/ARCHITECTURE.md` — `LayoutNode` enum + `synthesized_read_loop` recursive interpreter, `arrow_builder_output`, `vortex_reader` module boundaries and the build-order graph (THE structural blueprint for this phase)
- `.planning/research/STACK.md` — `vortex-fastlanes` 0.74 = `BitPackedEncoding` + `FoREncoding`; arrow-rs 58.3 typed builders (`Int32Builder` etc.)
- `.planning/research/FEATURES.md` — per-encoding detail: bitpack (non-byte-aligned widths), FOR as scalar broadcast-add over bitpack output; dependency ordering
- `.planning/research/PITFALLS.md` — **P7 validity/null handling** (Vortex carries validity separately; the read loop must map it to the Arrow null bitmap at every layer), arrow-version skew

### Phase 1/2 decisions + the FFI seam this phase's output flows through
- `.planning/phases/01-scaffold-and-ffi-boundary/01-CONTEXT.md` — multi-crate workspace; **Vortex isolated to vortex_reader + oracle (D-02)** — reaffirmed here
- `.planning/phases/02-duckdb-extension-scaffold/02-CONTEXT.md` — **"D-01 REVISED"**: arrow_scan/record-batch path deferred to "when real columnar decode produces record-batch output"; D-03 above schedules that for Phase 5
- `crates/loom-ffi/src/ffi.rs` — the existing `to_ffi` + `ptr::write` export path (and the `Int32Builder` seed pattern) the Phase-3 Arrow output must remain compatible with

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/loom-ffi/src/ffi.rs` already builds an `Int32Array [1,2,3,null]` via `Int32Builder` and exports it via `to_ffi` — this is the seed for `arrow_builder_output` and proves the Arrow→FFI export (success criterion 4) already works end-to-end.
- `crates/loom-ffi/tests/buffer_layout.rs` (Phase 2 Wave-0) pins the Arrow buffer layout the output must produce.
- `loom-core` exists as a pure-Rust crate skeleton (zero FFI, zero vortex) — Phase 3 fills it with the L1 model + read loop + builders. `loom-fixtures` is where `vortex-*` lives.

### Established Patterns
- Vortex isolation (D-02): `vortex-*` only in `vortex_reader`/fixtures/oracle; `loom-core` decode logic must not depend on `vortex-*`.
- arrow-rs single-version pin (CORE-01) — `arrow_builder_output` uses the same workspace `arrow` dep; a version conflict would surface at `to_ffi` (success criterion 4 is partly a version-skew tripwire).

### Integration Points
- `arrow_builder_output::finish()` → `ArrayData` → consumed by the existing `to_ffi` export in `loom-ffi`. Phase 3 does NOT rewire `loom_decode` (D-03) — it ensures the produced `ArrayData` is export-compatible.

</code_context>

<specifics>
## Specific Ideas

- Demonstrate the non-byte-aligned width explicitly (e.g. 11-bit packing, success criterion 1) — it's the case that proves real bit-unpacking rather than byte copying.
- Make the L1/L2 split visible in the code structure even though L2 isn't implemented yet (the `KernelEscape` arm exists as the seam).

</specifics>

<deferred>
## Deferred Ideas

- **Bitpack exception/"patch" path** (out-of-width values stored separately by vortex-fastlanes) — deferred; Phase-3 fixtures stay in-width. Revisit in Phase 4/5 when broader real data is decoded. [TRACKED]
- **arrow_scan / record-batch + DuckDB-shows-real-data** — deferred to Phase 5 per D-03 (and 02-CONTEXT.md "D-01 REVISED"). [TRACKED in STATE.md]
- **Encoded/recursive validity** (validity that is itself an encoded array) — Phase 3 assumes a plain validity bitmap; recursive validity is later.
- `Dictionary` / `RunEnd` decode → Phase 4; `KernelEscape` / FSST → Phase 4–5 (the enum arms exist now per D-04, but unimplemented).

</deferred>

---

*Phase: 03-l1-bitpack-for-and-arrow-builders*
*Context gathered: 2026-06-07*
