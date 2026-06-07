# Phase 05 Pattern Map

**Generated:** 2026-06-08
**Scope:** Files and patterns likely touched by Phase 05 plans.

## Core Decode Patterns

### `crates/loom-core/src/l2_kernel_registry.rs`

Role: L2 kernel dispatch and FSST implementation.

Closest analogs:

- `FsstKernel` stub already returns Utf8 `ArrayData`.
- `L2KernelRegistry::default_for_mvp0()` establishes id `0`.
- Existing tests assert id `0`, missing id, Utf8 type, and zero length.

Pattern to preserve:

- `L2Kernel::decode(&self, params: &[u8], count: usize) -> Result<ArrayData, LoomDecodeError>`.
- Unknown kernel ids are handled outside the kernel by `decode_layout_to_array_data`.
- Kernel output is owned `ArrayData`; it does not append through parent builders.

### `crates/loom-core/src/l1_model.rs`

Role: recursive L1 interpreter and child materialization.

Closest analogs:

- `decode_dictionary` decodes `codes` and `values`, then appends via
  `DecodedArray::append_value_to_builder`.
- `decode_run_end` expands a values child while preserving nulls.
- `decode_layout_to_array_data` is registry-aware for top-level
  `KernelEscape`; direct `synthesized_read_loop` remains registry-free.

Pattern to preserve:

- Parent encodings materialize child arrays to typed Arrow arrays before reading
  values/nulls.
- Malformed layout data returns `LoomDecodeError`, not a panic.
- Dictionary invalid code checks happen before indexing values.

### `crates/loom-core/src/arrow_builder_output.rs`

Role: one narrow Arrow builder abstraction.

Closest analogs:

- Boolean support added by extending the enum, constructor, append methods,
  `append_null`, `data_type`, and `finish`.

Pattern to preserve:

- Add Utf8/String support the same way: a concrete builder variant and explicit
  append method.
- Continue using Arrow builders; do not raw-write Arrow buffers in Rust.

## Fixture and Oracle Patterns

### `crates/loom-fixtures/src/vortex_reader.rs`

Role: the only Vortex-to-Loom bridge.

Closest analogs:

- `from_array_ref` downcasts Vortex encodings and emits plain `LayoutNode`.
- `from_dict_array` recursively bridges codes and values.
- `from_rle_array` is allowed to canonicalize when Vortex physical layout differs.

Pattern to preserve:

- Vortex types stay in this crate.
- Returned `LayoutNode` contains only plain Rust data (`Vec`, integers, bools).
- Any Vortex representation mismatch must be documented in code/tests.

### `crates/loom-fixtures/src/oracle.rs`

Role: Vortex reference decoder.

Closest analogs:

- `decode_i32_oracle`, `decode_u32_oracle`, and `decode_bool_oracle` execute the
  Vortex array to canonical arrays and return values plus null flags.

Pattern to preserve:

- Add Utf8 oracle helper returning string values and null flags.
- Use Vortex execution/canonical path as the reference source.
- Compare nulls row-for-row, not just values.

### Fixture tests

Role: row-for-row verification against Vortex.

Closest analogs:

- `dict_roundtrip.rs` builds real `DictArray`, decodes via Vortex oracle, then
  bridges to Loom and compares Arrow values/nulls.
- `kernel_escape_roundtrip.rs` exercises registry-backed `decode_layout_to_array_data`.
- `rle_roundtrip.rs` pairs live oracle coverage with documented canonicalization.

Pattern to preserve:

- Tests should construct arrays in memory.
- No file-backed Vortex IO/serde crates.
- Every fixture test should have an oracle side and a Loom side.

## FFI and DuckDB Patterns

### `crates/loom-ffi/src/ffi.rs`

Role: C ABI boundary and Arrow export.

Closest analogs:

- `loom_decode` validates pointers and wraps `loom_decode_inner` in
  `catch_unwind`.
- `loom_decode_inner` currently builds a hardcoded `Int32Array` and exports
  `ArrayData` via `arrow::ffi::to_ffi`.

Pattern to preserve:

- All FFI entry points remain panic-safe.
- Output ownership is still exactly one `ptr::write` each for `FFI_ArrowArray`
  and `FFI_ArrowSchema`.
- Decoder errors map to nonzero `LoomError`.

### `duckdb-ext/loom_extension.cpp`

Role: DuckDB table function and direct Arrow-to-DataChunk population.

Closest analogs:

- `LoomBind` declares one nullable INTEGER column.
- `LoomInit` calls `loom_decode`.
- `LoomScan` reads Arrow C buffers and fills a DuckDB flat vector.

Pattern to preserve:

- `LoomScanState` owns and releases Arrow array/schema on every teardown path.
- Return code is checked before reading outputs.
- For Utf8, confirm `duckdb.hpp` string APIs and use DuckDB-owned string storage
  (`StringVector::AddString` / `string_t`) rather than pointing into transient
  Arrow memory.

## Verification Patterns

- Rust: `cargo test --workspace`.
- Isolation: `cargo tree -p loom-core | grep -c -E 'vortex|fastlanes'` must print
  `0`.
- DuckDB: `bash scripts/duckdb-smoke-test.sh`.
- Fixture hygiene: grep for forbidden file-backed Vortex APIs in `crates/loom-fixtures`.

---

*Pattern map complete: 2026-06-08*
