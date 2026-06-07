# Phase 4: L1 Dict, RLE, and L2 Escape Infrastructure - Context

**Gathered:** 2026-06-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Complete the L1 declarative layer and stand up the L2 escape infrastructure. Two halves:

1. **Finish L1.** Fill the `LayoutNode::Dictionary` and `LayoutNode::RunEnd` arms in `synthesized_read_loop` — dictionary codes→values lookup and run-end expansion, both via **recursive sub-array dispatch** and with nulls preserved. The enum arms already exist (Phase 3, D-04); Phase 4 implements them with no schema churn.
2. **Wire L2 routing.** Implement the `LayoutNode::KernelEscape` arm so it dispatches through a new `L2KernelRegistry`. The registry holds one **stub** FSST kernel (index 0) that returns empty output. The routing path — not FSST decompression — is the deliverable.

Requirements: **L1-05** (dictionary decode, codes→values, recursive sub-array dispatch, nullable), **L1-06** (RLE decode via run-end expansion, boolean + integer, nulls preserved), **L2-01** (`KernelEscape` routes through `L2KernelRegistry` without panicking).

**Not this phase:** The real FSST kernel body (L2-02), dict-over-FSST end-to-end (L2-03), the full standalone row-for-row verification across all encodings, and the DuckDB-SQL-shows-real-data rewire (`arrow_scan`/record-batch) — all Phase 5. Per D-03 (Phase 3), `loom_decode`/`loom_scan` keep their hardcoded path; Phase 4 verifies at the **Rust + Vortex-oracle** level.

</domain>

<decisions>
## Implementation Decisions

### L2 Escape Contract (the L1↔L2 seam — locks how Phase 5's FSST plugs in)
- **D-01:** An `L2Kernel` is a **self-contained total function that returns its own Arrow `ArrayData`** (e.g. FSST owns a `StringBuilder` internally, finishes it, returns the array). The read loop adopts the returned array. This keeps the L1/L2 boundary clean and decouples kernel output type from the integer-only `OutputBuilder`. (Chosen over: writing into a passed-in `OutputBuilder`; or returning raw `Vec<Option<Vec<u8>>>`.)
- **D-02:** The Phase-4 **stub FSST kernel returns an empty (zero-length) `StringArray`/Utf8 `ArrayData`**. Its type signature already matches Phase 5's real FSST output, so Phase 5 fills only the body — no contract change. (Chosen over: empty `Int32Array`.)
- **D-03:** `L2KernelRegistry` wraps **`Vec<Box<dyn L2Kernel>>`**, FSST at **index 0**, constructed via `default_for_mvp0()`. `get(id)` returns `Option`; a miss surfaces a **typed `LoomDecodeError`** (e.g. `UnknownKernel`) — never a panic. Matches research/ARCHITECTURE.md and the success-criterion `get(0)` shape. (Chosen over: `HashMap<u32, …>`.)

### Builder Type Expansion
- **D-04:** Add **`OutputBuilder::Boolean(BooleanBuilder)`** alongside `Int32`/`Int64`, with `append_bool`/`append_null`, mirroring the existing typed-builder pattern. Required by the RLE-boolean success criterion.
- **D-05:** **No string/Utf8 variant in `OutputBuilder` this phase.** Phase-4 dict is integer-only, and the FSST kernel owns its own `StringBuilder` (D-01). Add string support when dict-over-FSST lands in Phase 5 — avoid building the interface before its real consumer exists.

### Verification Rigor
- **D-06:** Verify dict and RLE decode **row-for-row against the live Vortex oracle** (reuse the Phase-3 `vortex_reader` + `oracle` harness: `into_canonical().into_arrow()` vs loom-core output, element-by-element). **Hand-written expected-array fallback only** for any encoding Vortex 0.74 cannot easily construct as a fixture (see researcher confirm item in canonical_refs). Nullable variants included for both dict and RLE.
- **D-07:** `KernelEscape` routing (L2-01) is proven by **two routing-only Rust unit tests**: (1) `KernelEscape { kernel_id: 0, … }` routes to the registry, returns the empty `StringArray`, no panic; (2) an unknown `kernel_id` returns the typed `LoomDecodeError`, no panic. No oracle comparison — stub output is empty by contract.

### Recursion & Validity
- **D-08:** Fixtures use **realistic Vortex layouts** — build them the way `vortex-array` naturally encodes (e.g. dict codes=`BitPack` over values=`Raw`; RLE `run_ends` + `values` as Vortex emits them). Exercises recursive dispatch through the already-proven `BitPack`/`Raw`/`FOR` arms and reflects real-world layouts.
- **D-09:** **Fix CR-02 now** (folded todo). `decode_for`'s non-`BitPack` fallback currently emits `unpacked[i]` instead of `unpacked[i] + reference`. Apply the reference scalar after the inner decode for the non-`BitPack` path, and add a **FOR-over-Raw roundtrip test vs the Vortex oracle**. Recursive sub-array dispatch makes this path reachable (FOR nested inside a dict/RLE sub-array), so the landmine is closed before it can fire. (Chosen over: typed-error guard; or deferring.)
- **D-10:** Nullable dict/RLE validity **delegates to the child sub-array** Vortex carries it on (Pitfall 3 — same pattern as Phase 3, where `FrameOfReference` delegates to its inner `BitPack`'s validity), routed through `append_null`. Matches Vortex's own representation. (Chosen over: a top-level validity bitmap on the `Dictionary`/`RunEnd` node.)

### Claude's Discretion
- RLE run-end **expansion algorithm** (linear scan vs binary search over decoded `run_ends`) — research/ARCHITECTURE.md suggests binary search; planner/executor may choose based on fixture sizes.
- **Read-loop output shape for a top-level `KernelEscape`**: since a kernel returns its own `ArrayData` (D-01) rather than appending into the shared `OutputBuilder`, the read loop / `loom-core` decode entry needs a way to surface a kernel-produced array as the column output. The exact mechanism (e.g. read loop returns an enum of `builder-backed` vs `kernel-array`, or sub-arrays are decoded to `ArrayData` then read back) is left to research/planning.
- How dict/RLE **materialize their decoded sub-arrays** before lookup/expansion (decode child → `ArrayData`/temp builder → read values out → emit into parent) — implementation detail for the planner.
- Exact integer widths and run/dict cardinalities used in fixtures.

### Folded Todos
- **CR-02 — decode_for non-BitPack fallback silently drops the FOR reference** (`.planning/.../todos` `cr-02-decode-for-non-bitpack-reference.md`, `resolves_phase: 4`, severity warning). Original problem: `decode_for` delegates a non-`BitPack` inner to `synthesized_read_loop` without applying the FOR `reference` scalar, emitting wrong values. Fits this phase's scope because Phase-4 recursive sub-array dispatch is the first place FOR-over-non-BitPack layouts become constructible. Resolution: D-09 (fix + FOR-over-Raw oracle test).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design (authoritative L1/L2 model)
- `design.md` §4 (L1 declarative layout layer — dictionary and run-length as declarative encodings; recursive physical layout tree), §4/§7 (L2 escape: "anything that can't be declared drops into a total-function L2 kernel"; the L1→L2 boundary), §6 (typed builder output → construction-is-valid)
- `.planning/REQUIREMENTS.md` — L1-05, L1-06, L2-01 (and L2-02/L2-03 for Phase-5 awareness so the L2 contract doesn't paint Phase 5 into a corner)
- `.planning/ROADMAP.md` Phase 4 — goal + 3 success criteria

### Stack / Architecture / Pitfalls (project research)
- `.planning/research/ARCHITECTURE.md` — `l2_kernel_registry` (`L2Kernel` trait, `Vec<Box<dyn L2Kernel>>` indexed by `kernel_id`, FSST at index 0), `Dictionary`/`RunEnd` arms with recursive sub-array dispatch, RLE run-end expansion via binary search, build-order graph (THE structural blueprint)
- `.planning/research/FEATURES.md` — dict codes→values lookup; RLE run-end expansion; dependency ordering (dict/RLE depend on Phase-3 infra)
- `.planning/research/STACK.md` — dict via `vortex-array` 0.74 (**`vortex-dict` does not exist at 0.74** — see STATE blocker); `fsst-rs` / `vortex-fsst` for Phase 5
- `.planning/research/PITFALLS.md` — **P7 validity/null handling** (validity delegates to child arrays; map to Arrow null bitmap at every layer — basis for D-10)
- `.planning/research/SUMMARY.md` — Phase-4 deliverables list; `fsst_kernel` appends to `StringBuilder` (informs the D-01/D-02 string-typed kernel contract)

### Prior-phase decisions this phase builds on
- `.planning/phases/03-l1-bitpack-for-and-arrow-builders/03-CONTEXT.md` — **D-02 Vortex isolation** (loom-core zero `vortex-*`; vortex only in `loom-fixtures`), **D-04** (full `LayoutNode` enum defined, deferred arms return typed `UnimplementedEncoding`), **D-03** (DuckDB rewire deferred), validity-delegation precedent (FOR→inner BitPack, Pitfall 3)
- `.planning/phases/02-duckdb-extension-scaffold/02-CONTEXT.md` — "D-01 REVISED": `arrow_scan`/record-batch path deferred (lands Phase 5, not here)

### Code the implementation touches
- `crates/loom-core/src/l1_model.rs` — `synthesized_read_loop` match interpreter; the `Dictionary`/`RunEnd`/`KernelEscape` stub arms to fill (~L243–251); `decode_for` (~L389–392) is the CR-02 fix site
- `crates/loom-core/src/arrow_builder_output.rs` — `OutputBuilder` enum (Int32/Int64) to extend with `Boolean` (D-04)
- `crates/loom-core/src/error.rs` — `LoomDecodeError`; add the registry-miss variant (e.g. `UnknownKernel`, D-03)
- `crates/loom-fixtures/src/vortex_reader.rs` + `oracle.rs` — extend to construct/identify dict & RLE arrays and provide the oracle comparison (D-06)
- New module: `crates/loom-core/src/l2_kernel_registry.rs` (per research/ARCHITECTURE.md) — `L2Kernel` trait + `L2KernelRegistry` + `default_for_mvp0()` + stub `FsstKernel`

### Researcher confirm items (flagged in STATE.md blockers)
- Confirm **`DictArray` sub-array accessor names** in `vortex-array` 0.74 (codes / values getters) before planning — STATE notes this; reconcile with the fact that `vortex-dict` is not a separate crate at 0.74.
- Confirm **Vortex 0.74 can construct a RunEnd/RLE fixture** (which crate/encoding); if not, D-06's hand-written fallback applies to RLE.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/loom-core/src/l1_model.rs` — `synthesized_read_loop` recursive `match` interpreter and the already-defined `Dictionary`/`RunEnd`/`KernelEscape` enum arms (currently returning `UnimplementedEncoding`). Phase 4 fills the arms; the `Raw`/`BitPack`/`FrameOfReference` arms are reusable as sub-array decoders during recursive dispatch.
- `crates/loom-core/src/arrow_builder_output.rs` — `OutputBuilder` (Int32/Int64) + `append_*`/`finish()→ArrayData` pattern; extend with `Boolean` following the same shape.
- `crates/loom-fixtures/src/vortex_reader.rs` + `oracle.rs` — the Phase-3 fixture-builder + oracle harness proving row-for-row Vortex equality; extend for dict/RLE (D-06).
- `decode_for` already broadcasts the FOR reference correctly for the `BitPack` inner — the CR-02 fix (D-09) generalizes that to the non-`BitPack` path.

### Established Patterns
- **Vortex isolation (D-02 from Phase 1/3):** `loom-core` decode logic stays zero-`vortex-*`; dict/RLE decode works off `LayoutNode`, vortex only in `loom-fixtures`.
- **Typed errors, never panic** (D-04 from Phase 3): deferred/invalid arms return `LoomDecodeError`; the registry miss path (D-03) and CR-02 (D-09) follow this.
- **Validity delegates to the child** (Pitfall 3): the precedent set by FOR→BitPack in Phase 3 is the model for dict/RLE nullable handling (D-10).
- **arrow-rs single-version pin** (CORE-01): `OutputBuilder::Boolean` and any kernel-returned `ArrayData` use the same workspace `arrow` dep — version skew would surface at `to_ffi`.

### Integration Points
- A kernel-returned `ArrayData` (D-01) must remain export-compatible with the existing `to_ffi` path in `crates/loom-ffi/src/ffi.rs` (same as the L1 builder output).
- The read loop's handling of a top-level `KernelEscape` (kernel returns its own array vs L1 arms appending into `OutputBuilder`) is the one structural wrinkle — see Claude's Discretion.

</code_context>

<specifics>
## Specific Ideas

- Keep the L1/L2 split **visible in the code structure**: `KernelEscape` is the only place L2 code runs, and `l2_kernel_registry.rs` is a distinct module — the seam should read as a seam (carried from Phase 3's "make the split visible" intent).
- The stub FSST kernel is deliberately type-accurate (empty `StringArray`) so Phase 5 is a body-fill, not a contract change.

</specifics>

<deferred>
## Deferred Ideas

- **Real FSST decompression** (symbol table + code stream → strings) — Phase 5 (L2-02); the stub kernel's contract is fixed here.
- **dict-over-FSST end-to-end** (dict values sub-array is a `KernelEscape` → `StringArray`, codes index into it) — Phase 5 (L2-03); this is also when `OutputBuilder`/output handling for string-valued dicts gets built (D-05).
- **Standalone full verification harness + DuckDB SQL over real decoded data** (`arrow_scan`/record-batch rewire) — Phase 5 (VERIFY-01/02/03, and 02-CONTEXT.md "D-01 REVISED").
- **Bitpack exception/"patch" path** (out-of-width values) — still deferred from Phase 3; fixtures stay in-width.
- **Encoded/recursive validity** (validity that is itself an encoded array) — still assuming plain validity bitmaps read from the child.

</deferred>

---

*Phase: 04-l1-dict-rle-and-l2-escape-infrastructure*
*Context gathered: 2026-06-07*
