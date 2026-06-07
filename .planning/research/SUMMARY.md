# Project Research Summary

**Project:** Loom MVP0 (DuckDB demo)
**Domain:** Rust decoder core + Arrow C Data Interface FFI + C++ DuckDB extension
**Researched:** 2026-06-07
**Confidence:** HIGH

## Executive Summary

Loom MVP0 proves a single, narrow chain: one Vortex-encoded column is decoded by a pure-Rust interpreter (no MLIR, no codegen) through four L1 declarative encodings (bitpack, FOR, dict, RLE) and one L2 total-function kernel (FSST), producing well-formed Apache Arrow that crosses a C ABI boundary into a C++ DuckDB table function and is queried with SQL. The acceptance bar is concrete and falsifiable: DuckDB SELECT results must match Vortex's own decoder row-for-row. Experts building this class of system use a Rust `staticlib` linked into a C++ DuckDB extension, bridged zero-copy via the Arrow C Data Interface — exactly what this stack recommends. The `vortex-array` / `vortex-fastlanes` / `vortex-dict` / `vortex-fsst` crates (all at 0.74.0) plus `arrow-rs` 58.3.0 and DuckDB 1.5.3 are the pinned versions; any mismatch in the arrow-rs sub-crate family or the DuckDB git hash produces hard, silent failures.

The recommended architecture has two language-domain components connected by one ABI seam. On the Rust side: `vortex_reader` (encoding identification), `l1_model` + `synthesized_read_loop` (recursive interpreter over a `LayoutNode` enum), `l2_kernel_registry` (trait-object dispatch), `fsst_kernel` (wraps `fsst-rs` Decompressor), `arrow_builder_output` (typed arrow-rs builders), and `ffi_export_shim` (the single `extern "C"` surface). On the C++ side: a minimal DuckDB table function (`loom_scan`) that calls `loom_decode`, receives `FFI_ArrowArray` + `FFI_ArrowSchema`, converts via `ArrowToDuckDB`, and calls `release` on teardown. The Vortex crate is used only in `vortex_reader` (layout identification) and in the independent reference decoder binary (the oracle for verification); the Loom L1 loop must implement decoding independently or the proof collapses.

The three must-not-forget risks are: (1) FFI ownership — `FFI_ArrowArray` must be transferred via `std::ptr::write` in Rust and its `release` callback called in every C++ exit path; (2) panic-across-FFI — every `extern "C"` entry point requires a `catch_unwind` wrapper and `panic = "abort"` in the release profile; and (3) arrow-rs version skew — all `arrow-*` sub-crates must resolve to the same 58.3.0 patch across the entire workspace. Scope discipline is the fourth risk: MLIR, the formal verifier, `.vortex` file parsing, and multi-column support are all explicitly out of scope and must stay out.

---

## Key Findings

### Recommended Stack

The Rust decoder crate compiles as a `staticlib` (not `cdylib`), linked at compile time into the C++ DuckDB extension to avoid runtime RPATH complexity. `cbindgen` 0.29 generates `loom.h` automatically from `extern "C"` symbols in `build.rs`. The C++ side uses DuckDB's `extension-template` (not the experimental `extension-template-rs`), pinned to `DUCKDB_GIT_VERSION=v1.5.3` — the git hash in the extension footer must match byte-for-byte or the load fails. Arrow C Data Interface FFI (`arrow::ffi::to_ffi`, feature `ffi` on the `arrow` crate) is the zero-copy Rust-to-C++ bridge; `arrow2` must not appear anywhere in the workspace.

**Core technologies:**

| Technology | Version | Purpose |
|------------|---------|---------|
| `vortex-array` | 0.74.0 | Core in-memory array model, encoding dispatch, no-file-container `ArrayRef` construction |
| `vortex-fastlanes` | 0.74.0 | `BitPackedEncoding` + `FoREncoding` L1 implementations |
| `vortex-dict` | 0.74.0 | `DictEncoding` L1 implementation |
| `vortex-fsst` | 0.74.0 | `FsstEncoding` + `fsst-rs` FSST L2 kernel |
| `arrow` (arrow-rs) | 58.3.0 | Arrow typed builders, `ArrayData`, FFI export (feature `ffi`) |
| `cbindgen` | 0.29.3 | C header generation from `extern "C"` symbols in `build.rs` |
| DuckDB C++ API | 1.5.3 | `TableFunction` bind/init/scan callbacks, `DataChunk`, `ArrowToDuckDB` |

**Do not use:** `arrow2`, `vortex-file`, `vortex-serde`, `vortex-ipc`, `extension-template-rs`, `cxx`, nested `cdylib`, the deprecated DuckDB `arrow` community extension.

### Expected Features

**Must have (table stakes — demo cannot run without these):**
- Vortex array deserializer: identifies encoding from in-memory `ArrayRef` without file container parsing
- Typed Arrow builders: `Int32Builder`, `Int64Builder`, `StringBuilder`, `BooleanBuilder` — foundation all decoders write into
- L1 bitpack decoder: shift+mask bit-unpacking, handles non-byte-aligned widths (1–64 bits), ~2–3 days
- L1 FOR decoder: scalar broadcast-add over bitpack output; trivial once bitpack works (~0.5 days)
- L1 dict decoder: codes-to-values lookup with recursive sub-array dispatch; FSST-valued dicts require FSST first (~2 days)
- L1 RLE decoder: run-end expansion via binary search over `run_ends` sub-array (~1–2 days)
- L1→L2 escape / kernel dispatch: `KernelEscape` match arm in the synthesized read loop; routes to `L2KernelRegistry`
- FSST L2 kernel: wraps `fsst-rs` `Decompressor`; extracts symbol table + codes + offsets from `FsstArray`; appends to `StringBuilder` (~3–4 days)
- Arrow C Data Interface FFI export: `extern "C" fn loom_decode(...)`, `ptr::write` into caller slots, `catch_unwind` wrapper
- DuckDB C++ table function: `loom_scan` registered via `ExtensionUtil::RegisterFunction`, `ArrowToDuckDB` in scan callback
- Verification harness: side-by-side comparison of Vortex `into_canonical().into_arrow()` vs Loom L1 loop output

**Should have (add after MVP0 validates the chain):**
- Human-readable L1 layout descriptor (TOML/S-expr) for reviewer exposition
- Multiple sample columns (one per encoding) in the verification harness
- CLI driver (`loom decode <file> <column>`) for non-Rust reviewers
- Wall-clock timing comparison (Loom interpreter vs Vortex native decode)

**Defer (later milestones only):**
- Additional L2 kernels (ALP float, delta-of-delta)
- Full `.vortex` file parsing (footer/layout tree/multi-chunk)
- Multi-column table function and schema assembly
- `statistics()` / `projection_mask` / `range` ABI
- Formal verifier / totality proofs
- MLIR `decode` dialect / native codegen
- Distribution container / feature flags / content-hash URI

### Architecture Approach

Two language-domain components connected by a single Arrow C Data Interface seam. The Rust staticlib is composed of six modules with a strict dependency order: `vortex_reader` feeds `l1_model`, which drives the `l2_kernel_registry` on escape nodes and writes to `arrow_builder_output`, which is finalized by `ffi_export_shim`. The C++ extension is a thin caller: Init invokes `loom_decode`, Scan calls `ArrowToDuckDB`, teardown calls `array.release`. Single-array transfer (`FFI_ArrowArray` + `FFI_ArrowSchema` via `ptr::write`) is preferred over `FFI_ArrowArrayStream` for MVP0 — one column, one batch, no pull loop needed.

**Major components:**

| Component | File | Responsibility |
|-----------|------|----------------|
| `vortex_reader` | `src/vortex_reader.rs` | Deserialize in-memory Vortex `ArrayRef` → typed `LayoutDescription` (LayoutNode tree) |
| `l1_model` + read loop | `src/l1_model.rs` | `LayoutNode` enum (BitPack/FrameOfReference/Dictionary/RunEnd/KernelEscape); `synthesized_read_loop()` recursive interpreter |
| `l2_kernel_registry` | `src/l2_kernel_registry.rs` | `L2Kernel` trait; `Vec<Box<dyn L2Kernel>>` indexed by `kernel_id`; MVP0 has one entry (FSST at index 0) |
| `fsst_kernel` | `src/fsst_kernel.rs` | Deserializes FSST params, builds `fsst_rs::Decompressor`, calls `decompressor.decompress()` per string, appends to `StringBuilder` |
| `arrow_builder_output` | `src/arrow_builder_output.rs` | Typed arrow-rs builders; `append_value/append_null`; `finish() → ArrayData`; only location where arrow-array 58.x is used |
| `ffi_export_shim` | `src/ffi.rs` | `#[no_mangle] extern "C" fn loom_decode(...)`; `catch_unwind` wrapper; `to_ffi()` + `ptr::write` |
| C++ extension | `cpp/loom_extension.cpp` | `loom_scan` table function; Bind/Init/Scan; `ArrowToDuckDB`; `release` on teardown |
| Verification harness | `tests/` | Dual decode (Vortex oracle vs Loom interpreter); row-for-row comparison |

**Build order / critical path:**
```
[1] Data fixture (in-memory Vortex ArrayRef, no .vortex file)
[2] Rust cargo build → libloom_decoder.a + loom.h
[3] cmake → loom_extension.duckdb_extension  (depends on [2])
[4] DuckDB smoke test: LOAD + SELECT  (depends on [1], [3])
[5] Vortex reference binary (parallel to [2])
[6] Row-for-row verification  (depends on [4], [5])
```

### Critical Pitfalls

1. **FFI release-callback double-free or leak** — Use exactly one `std::ptr::write(out_array, ffi_array)` per produced `FFI_ArrowArray`; the write moves the value out of Rust's drop graph. On the C++ side, call `array.release(&array)` in every exit path (happy, error, cancel) and set `release = nullptr` immediately after. Zero-initialize the scan state struct before calling `loom_decode`. Unit-test the release path outside DuckDB first. (RUSTSEC-2022-0012, Arrow C Data Interface spec, DuckDB PR #15632)

2. **Panic across `extern "C"` kills the DuckDB process** — Wrap every `extern "C"` entry point in `std::panic::catch_unwind`; convert caught panics to an error-code return that C++ checks before calling DuckDB error reporting. Set `[profile.release] panic = "abort"`. Never use `.unwrap()` or `.expect()` reachable from an `extern "C"` fn. (Rust RFC 2945)

3. **DuckDB extension ABI version mismatch** — Pin `DUCKDB_GIT_VERSION=v1.5.3` in the Makefile. Run `make update_duckdb_headers` before first build. Never use a Homebrew/system DuckDB binary alongside a source-built extension. The git hash in the extension footer must match byte-for-byte or load is rejected. (DuckDB Issue #16337)

4. **arrow-rs version skew — duplicate types across FFI** — Pin all `arrow-*` sub-crates to exactly 58.3.0 via `[patch.crates-io]` in workspace `Cargo.toml`. Run `cargo tree -d | grep arrow` and verify zero duplicates before any FFI integration test. Match vortex-array 0.74's exact 58.x pin.

5. **Vortex null/validity handling silently dropped** — Every L1 decode loop arm must check `array.validity()` before emitting values. Test at least one nullable column per L1 encoding; the row-for-row comparison must check `IS NULL` rows.

6. **Scope creep into `.vortex` file parsing** — Construct test inputs programmatically using `vortex-array` builder APIs. `vortex-file`, `vortex-serde`, and `vortex-ipc` must not appear in `Cargo.toml` or `Cargo.lock`.

7. **FSST correctness edge cases** — `fsst-rs` is "not production ready" and little-endian-only. Build the oracle comparison before writing decode logic. Test: empty string, all-escape-sequence string, max-length (8-byte) symbol.

8. **Rust staticlib allocator mismatch** — DuckDB's jemalloc extension overrides `malloc` as a strong symbol on some platforms; Rust's default allocator can clash. Fix: `#[global_allocator] static A: System = System;`.

---

## Implications for Roadmap

### Phase 1: Project Scaffold and FFI Boundary

**Rationale:** FFI ownership protocol and arrow-rs version pinning are load-bearing for every downstream phase. Establishing them first as mandatory invariants prevents every other phase from inheriting subtle memory bugs.

**Delivers:**
- Workspace `Cargo.toml` with pinned versions; `[patch.crates-io]` for all arrow-* sub-crates
- `[lib] crate-type = ["staticlib"]` and `[profile.release] panic = "abort"`
- Stub `extern "C" fn loom_decode(...)` with `catch_unwind` wrapper; `cbindgen` generating `loom.h`
- `cargo tree -d | grep arrow` returning zero duplicates
- `grep vortex-file Cargo.lock` returning nothing
- `#[global_allocator] static A: System = System;`

**Avoids:** Pitfalls 1, 2, 3, 4, 6, 8

### Phase 2: DuckDB Extension Scaffold

**Rationale:** DuckDB ABI version lock must be established before any C++ code is written. The build system linkage must be proven working before decode logic is written so integration failures are attributed to the scaffold, not the decoder.

**Delivers:**
- CMakeLists.txt with `cargo build --release` as `add_custom_command`, `target_link_libraries`, `target_include_directories`
- `loom_extension.cpp`: stub `loom_scan` table function; `LoomScanState` destructor calls `array.release`
- `LOAD 'loom_extension'; SELECT * FROM loom_scan('test.bin');` returning without crashing
- `DUCKDB_GIT_VERSION=v1.5.3` locked in Makefile

**Avoids:** Pitfalls 3, 8

### Phase 3: L1 Decode Loop — Bitpack, FOR, and Arrow Builders

**Rationale:** Bitpack is a dependency of FOR, dict (codes sub-array), and indirectly FSST. Arrow builders are the write surface for all decoders and must be exercised first.

**Delivers:**
- `arrow_builder_output`, `vortex_reader`, `l1_model` modules
- `LayoutNode::BitPack` and `LayoutNode::FrameOfReference` arms in `synthesized_read_loop`
- `loom_decode` wired to the real read loop
- Unit tests for bitpack and FOR including nullable variants

**Avoids:** Pitfall 5 (null handling established here)

### Phase 4: L1 Dict and RLE Decoders + L2 Escape Infrastructure

**Rationale:** Dict and RLE depend on Phase 3 infrastructure. The `KernelEscape` arm and `L2KernelRegistry` must be established here (with FSST stubbed) so Phase 5 can just add the kernel implementation.

**Delivers:**
- `LayoutNode::Dictionary` and `LayoutNode::RunEnd` arms; recursive sub-array dispatch
- `LayoutNode::KernelEscape` arm routing to `L2KernelRegistry`
- `L2KernelRegistry::default_for_mvp0()` with stub `FsstKernel`
- Unit tests for dict-of-integers, RLE-of-booleans, RLE-of-integers (nullable variants included)

### Phase 5: FSST L2 Kernel and Full Verification

**Rationale:** FSST is the most complex piece and depends on all prior phases. The oracle-first approach (build comparison before decode logic) is mandatory given `fsst-rs`'s "not production ready" caveat.

**Delivers:**
- `fsst_kernel` module implementing `L2Kernel` using `fsst_rs::Decompressor`
- Dict-over-FSST path exercised end-to-end
- Verification harness: Vortex reference binary vs Loom L1 loop, row-for-row, all encodings + FSST
- FSST edge-case test vectors (empty string, all-escape, max-length symbol)
- `SELECT * FROM loom_scan(...)` matches Vortex reference output row-for-row

**Avoids:** Pitfalls 1, 6, 7

### Phase Ordering Rationale

- Phase 1 before everything: FFI invariants cannot be retrofitted.
- Phase 2 before Phase 3: Build system linkage must be proven before decode logic, so failures are attributed correctly.
- Phase 3 before Phase 4: Bitpack and Arrow builders are dependencies of dict/RLE.
- Phase 4 before Phase 5: `KernelEscape` arm and `L2KernelRegistry` infrastructure must exist before FSST can be wired.
- Phase 5 last: Highest complexity; verification harness requires all decoders working.

### Research Flags

Phases with well-documented patterns (skip research-phase): Phase 1 (Cargo workspace + arrow-rs FFI), Phase 2 (DuckDB extension-template + CMake), Phase 3 (Arrow builder API + bitpack).

Phases that may need targeted source-level investigation during planning: Phase 4 (`DictArray` accessor names in vortex-dict 0.74), Phase 5 (`FsstArray` field names in vortex-fsst 0.74; `ArrowToDuckDB()` include path/signature).

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | MEDIUM-HIGH | Versions verified against crates.io/docs.rs/GitHub as of 2026-06-07. Internal Vortex API field names are from source-level docs, not stable user guides. |
| Features | HIGH | Requirements derived from locked design.md + PROJECT.md; acceptance criterion is concrete and binary. |
| Architecture | HIGH | Derived from design.md (authoritative), Arrow C Data Interface spec, and vortex-data source confirmation. |
| Pitfalls | HIGH | All critical pitfalls sourced from official specs, confirmed CVEs, and confirmed DuckDB GitHub issues. |

**Overall confidence:** HIGH

### Gaps to Address

- **`FsstArray` internal field names in vortex-fsst 0.74:** confirm against 0.74 source before Phase 5.
- **`DictArray` sub-array accessor API in vortex-dict 0.74:** confirm against vortex-array 0.74 source before Phase 4.
- **`ArrowToDuckDB` helper availability and signature:** confirm include path and signature from extension-template examples before Phase 2.
- **`fsst-rs` `Decompressor::new` exact argument types:** confirm `Symbol` type definition and constructor signature against fsst-rs 0.5.11 docs.rs before Phase 5.

---

## Sources

### Primary (HIGH confidence)
- `design.md` (Loom full design — authoritative)
- `.planning/PROJECT.md` — MVP0 requirements, out-of-scope, key decisions
- [Apache Arrow C Data Interface spec](https://arrow.apache.org/docs/format/CDataInterface.html)
- [RUSTSEC-2022-0012](https://rustsec.org/advisories/RUSTSEC-2022-0012.html) — double-free on FFI Arrow struct
- [DuckDB PR #15632](https://github.com/duckdb/duckdb/pull/15632) — ArrowArray lifetime bug fix
- [DuckDB Issue #16337](https://github.com/duckdb/duckdb/issues/16337) — ABI version mismatch
- [Rust RFC 2945: C-unwind ABI](https://rust-lang.github.io/rfcs/2945-c-unwind-abi.html)
- [arrow-rs ffi module docs](https://docs.rs/arrow/latest/arrow/ffi/index.html)
- [fsst-rs docs.rs](https://docs.rs/fsst-rs/latest/fsst/)
- [vortex-data/vortex GitHub](https://github.com/vortex-data/vortex)

### Secondary (MEDIUM confidence)
- [vortex-data/duckdb-vortex GitHub](https://github.com/vortex-data/duckdb-vortex)
- [SpiralDB FSST blog post](https://spiraldb.com/post/compressing-strings-with-fsst)
- [DuckDB Arrow IPC blog (2025-05-23)](https://duckdb.org/2025/05/23/arrow-ipc-support-in-duckdb)
- [DuckDB jemalloc extension docs](https://duckdb.org/docs/current/internals/jemalloc)

### Tertiary (confirm during implementation)
- `ArrowToDuckDB()` include path — confirm before Phase 2
- `DictArray` accessor names in vortex-dict 0.74 — confirm before Phase 4
- `FsstArray` internal field names in vortex-fsst 0.74 — confirm before Phase 5

---
*Research completed: 2026-06-07*
*Ready for roadmap: yes*
