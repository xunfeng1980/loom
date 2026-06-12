# Phase: Production decode chain (sidecar decode → real Arrow IPC, full type coverage)

> Status: executing (produced directly, not in .planning/PLAN.md format)
> Date: 2026-06-12 (revised: Plan 1 upgraded to a general interpreter; Plan 3 became a tier ladder; goal = full coverage)
>
> **Implementation progress (2026-06-12):**
> - **Plan 1 ✅ done**: general L2Core interpreter [`l2core_interp.rs`](../crates/loom-ffi/src/interp/l2core_interp.rs) wired into the `LoomNative` branch of `loom_sidecar_decode`; subsumes the i32 shortcut (equivalence regression passes); the LMA1 path is annotated as an offline oracle + an interp-vs-LMA1 differential test. 109 lib tests + integration all green.
> - **Plan 2 ✅ complete (incl. typed-row materialization, verified via end-to-end SQL)**:
>   - `loom_sidecar_decode` emits **real bare Arrow IPC** (`StreamWriter`); new `loom_sidecar_decode_carray` exports a struct array over the **Arrow C Data Interface** (`arrow::ffi::to_ffi`) for zero-copy hand-off. E2E FFI test [`sidecar_decode_ffi.rs`](../crates/loom-ffi/tests/sidecar_decode_ffi.rs): IPC reads back via `StreamReader`, the C-array round-trips via `from_ffi`, `free_bytes` releases. loom.h contract updated (bare IPC).
>   - **DuckDB extension runs end-to-end**: the JIT is behind a cargo feature (`--no-default-features`), so the extension carries no LLVM symbols and loads in the bundled `vendor/duckdb-cli/duckdb` (v1.5.3). `loom_scan` materializes decoded columns into **typed DuckDB rows** (generic `FillVector` for i32/i64/f32/f64/bool/utf8; unknown types fail-soft to a diagnostic column). Verified: `SELECT * FROM loom_scan('<fixture>')` returns 10 rows of int32=42; `SELECT COUNT/SUM/MIN` → 10/420/42. Fixture generator [`examples/make_fixture.rs`](../crates/loom-ffi/examples/make_fixture.rs).
>   - **DoD#2 met**: SQL returns real decoded values, end-to-end DuckDB SQL → interpreter → Arrow C interface → typed result rows.
> - **Plan 3 ✅ Tiers 1–4 green** (E2E test [`decode_ir_gen_tier1.rs`](../crates/loom-ffi/tests/decode_ir_gen_tier1.rs): parquet → auto IR → full verifier → interpreter → correct values/null positions):
>   - **Tier 1a (non-null i32/i64)**: `generate_decode_ir_from_parquet` emits a real `body` (ForRange+ReadInput+AppendValue); `parquet_to_raw_host` packs a column-major LE buffer.
>   - **Tier 1b (f32/f64/bool)**: new `ScalarExpr::Bitcast { target, value }` (codec tag 10 + full_verifier + interpreter; kloom skips it) resolves the "width→type" ambiguity. Float/bool AppendValue wraps the read in a Bitcast.
>   - **Tier 2 (nullable)**: new `L2CoreStmt::If { cond, then_body, else_body }` (codec tag 7 + verifier two-branch + interpreter + vortex corpus counter); a nullable column = a validity slice + `If(bitcast bool validity){AppendValue} else {AppendNull}`.
>   - **Tier 3 (non-null Utf8)**: reuses existing IR — offsets+data slices; per row read lo/hi offsets, Bitcast to Int32, dynamic-width `ReadInput data[lo..hi]`, Bytes→Utf8 append. The generator reads batches to size the data slice.
>   - **Tier 4 (dictionary)**: Parquet dictionary is a **physical encoding**; the Arrow reader materializes it to a plain column, so dictionary-encoded input decodes transparently through Tiers 1–3 (test forces dictionary encoding). Producing a dictionary-**typed** Arrow output (DictionaryArray) is a representation optimization left as future work (needs OutputBuilder::Dictionary + DuckDB dictionary materialization).
> - **Plan 5 ✅**: the README / README-zh "correctness model / production runtime" narrative is corrected to match the code — the production decode runtime is the L2Core **interpreter** (wired into `loom_sidecar_decode`, emits real Arrow, materialized into typed rows by DuckDB `loom_scan`); the JIT is **offline-verified and not yet wired to the production FFI** (the extension is built `--no-default-features`, excluding LLVM).
> - **Plan 4 ⏳ building block done / zero-transcode direct read remaining**:
>   - **Done**: `read_column_chunk_physical_bytes` (`File::seek` + `byte_range` reads the raw physical column-chunk bytes directly, no Arrow materialization) + `parquet_column_chunk_hash` (BLAKE3 over the physical bytes, usable for sidecar binding verification). Test [`physical_bytes.rs`](../crates/loom-parquet-ingress/tests/physical_bytes.rs): deterministic reads, distinct bytes/hashes across columns, out-of-range fails closed. This closes the original gap where `bind_content_hash_to_parquet_data` was a no-op and physical ranges were used only for diagnostics.
>   - **Remaining frontier**: have the auto-generated IR **decode the physical bytes directly** (page-header parse + per-encoding decompress: PLAIN/dict/RLE) to drop the current raw transcode (`parquet_to_raw_host` materializes via Arrow then repacks). That amounts to rebuilding Parquet page decoding inside the L2Core IR — the last large step toward zero-transcode direct read in production.
>
> Scope: collapse the five outstanding items from the prior analysis into one phase, split into 5 dependency-ordered plans.
> **End goal: full type coverage** — i32/i64/f32/f64/bool + nullable + Utf8 + dictionary, decoded end-to-end through the sidecar path into real Arrow. i32 is the first vertical slice, not the finish line.

---

## 0. Root cause, merge direction, and the single lever

The five items are not five separate gaps — they are facets of one broken chain:

> **The `LoomNative` branch of the production FFI entry [`loom_sidecar_decode`](../crates/loom-ffi/src/ffi.rs#L503) neither executed nor output anything** —
> it decoded the IR, verified hashes, and ran the 4-gate route, then at the decode step it returned [`let ipc_output: Vec<u8> = Vec::new();`](../crates/loom-ffi/src/ffi.rs#L580), and the function had **no caller at all** (only declared in [loom.h:156](../crates/loom-ffi/include/loom.h#L156)).

Fix this one point and R1/R2/R4 gain real support; R3/R5 are the depth and alignment work on top.

### Key architectural truth (must be understood first, or you get a "fake merge")

The codebase has **two parallel decode machines**, and the code already states their fate:

| Layer | File | Supported | Nature |
|---|---|---|---|
| low-level builder | [arrow_builder_output.rs:78-83](../crates/loom-ffi/src/interp/arrow_builder_output.rs#L78) | Bool/i32/i64/f32/f64/**Utf8** + **AppendNull** | primitive; 6 types + null ready |
| L2 kernel | [l2_kernel_registry.rs](../crates/loom-ffi/src/interp/l2_kernel_registry.rs) | **FSST** (string), **ALP** (float) | encoding-specific decoders, present |
| L1 model | [l1_model.rs](../crates/loom-ffi/src/interp/l1_model.rs) | **bitpack** ([l1_model/bitpack.rs](../crates/loom-ffi/src/interp/l1_model/bitpack.rs)) | physical L1 decode primitive |
| **LMA1 "Arrow semantic" machine** | [native_arrow_semantic.rs:368](../crates/loom-ffi/src/interp/native_arrow_semantic.rs#L368) | Bool/i32/i64/f32/f64 | **not a decoder** — see below |
| **L2Core IR machine** | [native_lowering.rs:168](../crates/loom-ffi/src/interp/native_lowering.rs#L168) `execute_supported_copy_i32` | **i32 non-null only** | comment says "intentionally **not** a general interpreter" |

**Two facts you cannot get wrong:**

1. **`execute_native_arrow_semantic` is not a decoder — it is a replay/validation machine.** The LMA1 it receives **already embeds the answer (Arrow IPC)**: `decode_reference_batch` produces a reference batch, then `copy_supported_column` re-materializes each column. It proves "can the native model reproduce the embedded Arrow", and does not read physical bytes. Wiring the sidecar to it would be a **fake merge** (it only "works" because LMA1 already contains the answer).

2. **The merge direction is already written in the code.** [native_arrow_semantic.rs:401-403](../crates/loom-ffi/src/interp/native_arrow_semantic.rs#L401):
   > Phase 50 will **re-anchor native execution to sidecar overlay**. LMC2/LMA1 kept for backward compat with **test fixtures**. DO NOT remove until sidecar-native track is production-ready.

### Merge decision (this phase executes accordingly)

- **Write a general L2Core body interpreter** as the **single production decoder** wired into the sidecar FFI. It **subsumes** `execute_supported_copy_i32`, walks `ForRange/ReadInput/AppendValue/AppendNull`, dispatches appends to `arrow_builder_output` (6 types + null ready) and encoding ops to FSST/ALP/bitcast L1/L2 primitives.
- **Demote the LMA1 path to an offline differential oracle** (the test asks "can the IR interpreter reproduce the reference LMA1?"), not wired to the production FFI. This is consistent with the README "interpreter offline / production runs solo" narrative.
- **Type coverage is not "add a match arm" — it first needs the general interpreter skeleton** (today's i32 is a hardcoded shortcut, deliberately not a general loop); once the skeleton exists the low-level building blocks are mostly ready and adding types is incremental wiring.

---

## 1. Requirement mapping

| ID | Outstanding item | Owning plan |
|---|---|---|
| R1 | `loom_sidecar_decode` real Arrow IPC output | Plan 2 (depends on Plan 1) |
| R2 | L2Core interpreter/JIT wired to the sidecar FFI | Plan 1 |
| R3 | Parquet raw physical byte binding | Plan 4 |
| R4 | README production JIT/online-decode narrative aligned with code | Plan 5 |
| R5 | auto IR gen produces a real decode program (**full type coverage**) | Plan 3 |

## 2. Dependency order (execution order)

```
Plan 1 (R2: general L2Core interpreter + LMA1 demoted to oracle, Tier 1 engine green)
   └─> Plan 2 (R1: backfill real Arrow IPC + wire the caller)
          ├─> Plan 3 (R5: tier ladder — full type coverage, each tier end-to-end)
          │       Tier 1 → Tier 2 → Tier 3 → Tier 4
          └─> Plan 5 (R4: correct the README/correctness model to match code)
Plan 4 (R3: direct Parquet physical-byte read, incl. variable-length/dict chunks) — parallel to Plan 3
```

Plan 5 must be written **after** Plan 1/2 land. Plan 3 is the bulk of this phase (climbing the tier ladder = full coverage).

---

## Plan 1 — wire the general L2Core interpreter into the `LoomNative` branch (R2)

**Goal**: write a **general** `L2CoreProgram` body interpreter as the single production decoder, wire it into `loom_sidecar_decode`, subsume the i32 shortcut, and demote the LMA1 path to an offline oracle. This plan delivers the **engine skeleton + Tier 1 (fixed-width non-null) green**, with dispatch points reserved for later tiers from the start.

**Depends**: none.

**Files**
- New: `crates/loom-ffi/src/interp/l2core_interp.rs` (general interpreter; `interpret_l2core(program, inputs) -> Result<Vec<ArrayData>>`)
- Edit: [crates/loom-ffi/src/ffi.rs:566-600](../crates/loom-ffi/src/ffi.rs#L566) (`LoomNative` branch calls the general interpreter)
- Reuse: [arrow_builder_output.rs](../crates/loom-ffi/src/interp/arrow_builder_output.rs) (append dispatch, 6 types + null ready)
- Subsume/reuse: [native_lowering.rs:168](../crates/loom-ffi/src/interp/native_lowering.rs#L168) `execute_supported_copy_i32` (its i32 semantics fold into the general interpreter; kept as a thin wrapper or moved to tests)
- Demotion note: [native_arrow_semantic.rs:401](../crates/loom-ffi/src/interp/native_arrow_semantic.rs#L401) (comment update: LMA1 = offline oracle)

**Tasks**
1. **A (engine skeleton)**: implement `interpret_l2core`, executing `body` in order: `ForRange`/`CursorLoop` drive a row cursor, `ReadInput` reads from the host byte slice per `InputSlice.offset/length`, `LetScalar` binds, `AppendValue`/`AppendNull` dispatch to `OutputBuilder`, `FailClosed` fails closed immediately. **Dispatch covers all `L2DataType` via `match`; unimplemented arms return a typed `Unsupported` error (reserved for Tiers 2-4).**
   **AC**: the interpreter is general — adding a type is filling a dispatch arm, not changing control flow; the i32 case of `execute_supported_copy_i32` is reproduced by `interpret_l2core` and all old tests pass.
2. **B (FFI wiring)**: the `LoomNative` branch takes the `program` + host slices, calls `interpret_l2core`, builds the batch and stores it for Plan 2 serialization.
   **AC**: an i32 non-null program → correct `Int32Array`; out-of-bounds/unsupported → fail closed to `host-native`, no panic.
3. **C (LMA1 demotion)**: annotate/move `execute_native_arrow_semantic` to an **offline differential oracle** (test-only); add an "interp vs LMA1 reference" differential test scaffold.
   **AC**: the production FFI path no longer references LMA1 execution; LMA1 appears only in test/oracle modules.

**Verification**: unit tests; regression of `execute_supported_copy_i32`; negative paths fail closed without panic; interp output == LMA1 oracle reference (i32 fixture).

**must-have**: the interpreter **must be a general skeleton** — no second i32-specific shortcut. Fail-closed is a hard constraint (CLAUDE.md).

**Risks**: do not call `execute_native_arrow_semantic` from the sidecar (fake merge); the `verified_bindings`↔capability column mapping (granule_id ↔ capability id) must be checked explicitly.

---

## Plan 2 — backfill real Arrow IPC output + wire the caller (R1)

**Goal**: serialize Plan 1's output to **real Arrow IPC bytes** into `out_ipc_bytes`, and have the DuckDB extension actually consume it.

**Depends**: Plan 1.

**Files**
- Edit: [crates/loom-ffi/src/ffi.rs:579-598](../crates/loom-ffi/src/ffi.rs#L579) (remove the `Vec::new()` empty buffer)
- Edit: [contrib/duckdb-ext/loom_extension.cpp](../contrib/duckdb-ext/loom_extension.cpp) (call decode + ingest IPC)
- Edit: [crates/loom-ffi/include/loom.h](../crates/loom-ffi/include/loom.h) (contract alignment)

**Tasks**
1. **A**: on success, serialize the batch to a bare Arrow IPC stream into `out_ipc_bytes/out_ipc_len`, with real `row_count/column_count`. **AC**: `ipc_len > 0`; arrow-rs `StreamReader` reads it back.
2. **B**: document the bare-IPC contract in `loom.h`. **AC**: header doc matches the returned bytes.
3. **C**: the DuckDB extension calls `loom_sidecar_decode` on `route=="loom-native"` and materializes the columns; non-loom-native falls back. **AC**: one SQL query returns real column values end-to-end.
4. **D**: `loom_sidecar_free_bytes` correctly frees the non-empty buffer. **AC**: no leak / double free.

**must-have**: the non-loom-native path is byte-for-byte unchanged.

---

## Plan 3 — `decode_ir_gen` full type coverage (tier ladder) (R5)

**Goal**: have [`generate_decode_ir_from_parquet`](../crates/loom-parquet-ingress/src/decode_ir_gen.rs) emit a real executable `body`, and **climb the tier ladder to full type coverage**. Each tier is a vertical slice: (a) interpreter dispatch + (b) decode_ir_gen body + (c) a parquet→sidecar→IPC→correct-values E2E test.

**Depends**: Plan 1 (engine skeleton), Plan 2 (serializable/verifiable downstream).

### Tier 1 — fixed-width non-null (i32/i64/f32/f64/bool)
Integers append the width-typed read directly; floats/bool wrap it in `Bitcast` (Tier 1b).

### Tier 2 — nullable
A validity slice + `If` per row drives AppendValue/AppendNull.

### Tier 3 — Utf8 (variable-length)
offsets+data slices; dynamic-width `ReadInput data[lo..hi]` → Utf8 builder. (FSST kernel dispatch remains available for FSST-compressed fixtures; the default Arrow read path materializes strings.)

### Tier 4 — dictionary
Dictionary-encoded input decodes transparently (Arrow materializes it). Producing a dictionary-typed Arrow output is a representation optimization left as future work.

**must-have**: any type/encoding not covered by a tier must fail closed — no placeholder output.

---

## Plan 4 — Parquet raw physical byte binding (R3)

**Goal**: bypass Arrow materialization, `File::seek` to read raw column-chunk bytes via footer metadata, and bind the content hash to real physical bytes.

**Files**
- [source_contract.rs](../crates/loom-parquet-ingress/src/source_contract.rs) (`read_column_chunk_physical_bytes`, `parquet_column_chunk_hash`)
- Test [physical_bytes.rs](../crates/loom-parquet-ingress/tests/physical_bytes.rs)

**Status**: the direct-read + hash building block is done (see progress note). Decoding physical bytes in-IR (page-header parse + per-encoding decompress) to drop the raw transcode is the remaining frontier.

**must-have**: page-header parse and page-level decompress are explicitly **not** in the building-block step — record the gap rather than imply full physical coverage.

---

## Plan 5 — align README / correctness model with code (R4)

**Goal**: correct the README/README-zh narrative that ran ahead of the code — **after** Plan 1/2 land, write what the code actually does.

**Files**: [README.md](../README.md), [README-zh.md](../README-zh.md)

**Tasks**: change "JIT is the sole production runtime / online decode" to match code — the production path uses the **general L2Core interpreter** via `loom_sidecar_decode`; the JIT remains **offline differential verification** and is not wired to the production FFI; LMA1 is the offline oracle.

**must-have**: do not present "planned but not built" capability as fact; mark aspirational items as roadmap.

---

## 3. Definition of Done — full coverage

This phase is done iff:

1. **R2**: the general L2Core interpreter is the single production decoder wired into the sidecar FFI, subsumes the i32 shortcut; LMA1 demoted to an offline oracle with an interp-vs-LMA1 differential test.
2. **R1**: the DuckDB extension consumes non-empty real Arrow IPC via the sidecar `loom-native` path.
3. **R5 (full type coverage)**: `generate_decode_ir_from_parquet` emits a real executable body, **Tiers 1-4 green** — i32/i64/f32/f64/bool + nullable + Utf8 + dictionary, each with a parquet→sidecar→IPC→correct-values E2E test.
4. **R3**: parquet column-chunk physical bytes can be read directly, with hash binding failing closed.
5. **R4**: README strong claims each have `file:line` support, no overpromising.
6. **Fail-closed throughout**: any uncovered type/encoding/out-of-bounds/hash mismatch routes to `host-native` — never a half-built IPC.
7. core/FFI remain **Vortex-free**; parquet direct-read lives only in `loom-parquet-ingress`.

## 4. Explicitly out of scope for this phase

- **Wiring the JIT into the production FFI** (the JIT stays offline-verified here; wiring it is a separate phase).
- Parquet **page-header parse and page-level decompress** (Plan 4 records the gap).
- Nested/composite types (Struct/List/Map), Decimal, timestamps, and other types beyond Tiers 1-4.
- Advanced encodings beyond dictionary (RLE/FOR direct execution — unless a tier covers them incidentally; bitpack already has an L1 primitive, wired as needed).
- Parity wiring for Lance / Vortex ingress.
