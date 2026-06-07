# Phase 1: Scaffold and FFI Boundary - Context

**Gathered:** 2026-06-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Establish the Rust workspace as a **sound FFI `staticlib`** — the load-bearing foundation every later phase depends on. Concretely: a Cargo workspace that compiles, with Arrow sub-crates pinned to a single version, `panic = "unwind"` (revised from `abort` per 01-REVIEW.md CR-01) and a `System` allocator enforced, `cbindgen` generating `loom.h`, and the Arrow C Data Interface export contract (`to_ffi` + `ptr::write` + correct release ownership) wired through a tiny `extern "C"` surface wrapped in `catch_unwind`.

Requirements: CORE-01 (single arrow version, staticlib), CORE-02 (panic=abort + System allocator), CORE-03 (cbindgen loom.h), ARROW-03 (Arrow C Data Interface export with release ownership), DUCK-04 (catch_unwind on every extern "C").

**Not this phase:** the C++ DuckDB extension (Phase 2), any real decode logic / L1 encodings (Phase 3+), the Vortex reader and FSST kernel (Phase 3–5). Phase 1 proves the *boundary*, not the decoder.

</domain>

<decisions>
## Implementation Decisions

### Workspace Layout
- **D-01:** Use a **multi-crate Cargo workspace**, not a single crate. Members: `loom-core` (pure-Rust decode library, zero FFI), `loom-ffi` (the `staticlib` carrying the `extern "C"` surface), and a fixtures/reference crate. Rationale: keeps the decoder testable in pure Rust and keeps the unsafe FFI surface tiny and isolated — pays off through Phases 3–5.
- **D-02:** **Isolate the Vortex dependency.** `loom-core`'s decode logic stays independent of the `vortex-*` crates. Vortex is used only in a `vortex_reader` module (encoding/layout identification) and in the reference/oracle binary. This keeps the "Loom decodes independently" proof honest — the verification oracle must not be the same code path as the thing it verifies.
- **D-03:** **Pin the Rust toolchain** via a committed `rust-toolchain.toml` (specific stable version). Reproducible builds across machines/CI; aligns with the project's long-term-stability ethos.

### Claude's Discretion
The user opted not to discuss these; the planner/researcher should choose sensible defaults grounded in the canonical refs:
- **FFI contract shape** — the exact `loom_decode` C signature (input ptr+len, output `ArrowArray*`/`ArrowSchema*`) and the error-reporting strategy (return code vs out-param error string + a `loom_free`). `research/ARCHITECTURE.md` already sketches a `loom_decode` shape; planner may adopt/refine. Whatever is chosen becomes the contract Phases 2–5 build against, so it should be documented in an "Artifacts this phase produces" section.
- **Phase 1 depth** — whether the stub `loom_decode` returns nothing or produces a minimal *real* Arrow array. **Recommended:** a minimal real Arrow roundtrip (hardcoded array → `to_ffi` → a Rust-side test that imports it back and calls `release`), because `research/PITFALLS.md` says to unit-test the release path outside DuckDB first. Planner has discretion on how minimal.
- **Verification automation** — whether the CORE checks (`cargo tree -d` zero arrow dupes, no `vortex-file` in lockfile, `panic=abort`/allocator present) run in GitHub Actions CI or a local Makefile/script. ROADMAP success criteria say "verified in CI" — lean toward a CI workflow but a committed script the CI calls is fine.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design & Scope
- `design.md` §5.3 (memory model: read-only input, bounded scratch, no raw output writes), §6 (output contract: typed Arrow builders → C Data Interface), §9 (decoder ABI: `schema()`, `decode_batch(...)`) — the conceptual contract the FFI surface realizes
- `.planning/PROJECT.md` — MVP0 scope, out-of-scope, key decisions
- `.planning/REQUIREMENTS.md` — CORE-01/02/03, ARROW-03, DUCK-04 (this phase's requirement IDs)
- `.planning/ROADMAP.md` Phase 1 — goal + success criteria

### Stack, Architecture & Pitfalls (project research)
- `.planning/research/STACK.md` — pinned versions (`arrow` 58.3.0, `vortex-*` 0.74.0, `cbindgen` 0.29.3); `staticlib` + cbindgen build path; explicit "do not use" list (`arrow2`, `vortex-file/-serde/-ipc`, `cxx`, `extension-template-rs`)
- `.planning/research/ARCHITECTURE.md` — module boundaries (`vortex_reader`, `l1_model`, `l2_kernel_registry`, `fsst_kernel`, `arrow_builder_output`, `ffi_export_shim`), the Arrow seam ownership/release semantics, and the build-order graph
- `.planning/research/PITFALLS.md` — FFI release-callback ownership (use `ptr::write`), panic-across-FFI = process abort (catch_unwind + panic=abort), arrow-rs version skew (`cargo tree -d`), staticlib allocator mismatch (`System` allocator)
- `.planning/research/SUMMARY.md` — Phase 1 deliverables checklist and the must-not-forget invariants

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- None — greenfield. No source tree exists yet (`design.md` + `.planning/` only). This phase creates the workspace from scratch.

### Established Patterns
- The module/crate decomposition in `research/ARCHITECTURE.md` is the de-facto pattern to scaffold toward, even though only the FFI shim + a stub are implemented this phase.

### Integration Points
- The `extern "C"` surface in `loom-ffi` + the generated `loom.h` are the integration point Phase 2's C++ DuckDB extension links against. Keep the header surface minimal and stable.

</code_context>

<specifics>
## Specific Ideas

- The workspace should make the "pure core, thin unsafe FFI, Vortex only at the edges" separation visually obvious in the directory layout — it's part of what makes the design legible to reviewers.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope. (Project-level v2 items remain tracked in STATE.md / REQUIREMENTS.md.)

</deferred>

---

*Phase: 01-scaffold-and-ffi-boundary*
*Context gathered: 2026-06-07*
