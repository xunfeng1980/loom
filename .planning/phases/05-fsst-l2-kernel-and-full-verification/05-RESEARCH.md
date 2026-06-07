# Phase 05: FSST L2 Kernel and Full Verification - Research

**Date:** 2026-06-08
**Status:** Ready for planning
**Mode:** Inline research in Codex runtime

## Research Complete

Phase 05 should be implemented as three connected slices:

1. `loom-core`: add a Loom-owned FSST params format, real FSST L2 decode, and
   Utf8-aware dictionary gather.
2. `loom-fixtures`: extract `vortex-fsst` arrays into plain params, add Utf8
   oracle helpers, and prove FSST plus dict-over-FSST row-for-row.
3. `loom-ffi` / `duckdb-ext`: pass encoded input bytes through `loom_decode`,
   populate DuckDB vectors for the required Arrow types, and run the MVP0 SQL
   gate.

This split preserves the Phase 04 dependency boundary: `loom-core` remains free
of `vortex-*` and FastLanes crates.

## Phase Requirement Mapping

| Requirement | Research conclusion |
|-------------|---------------------|
| `L2-02` | Feasible in `loom-core` with `fsst-rs::Decompressor` and a Loom-owned params parser. |
| `L2-03` | Feasible by extending `DecodedArray` / builder paths to Utf8 and letting Dictionary gather from a `KernelEscape(FSST)` values child. |
| `VERIFY-01` | Existing `loom-fixtures::oracle` pattern can be extended with Utf8/String decode through Vortex execution. |
| `VERIFY-02` | Existing fixture tests already compare values/nulls for L1 encodings; add FSST and dict-over-FSST tests using the same shape. |
| `VERIFY-03` | Existing direct DuckDB `DataChunk` path should be extended rather than replaced by ArrowArrayStream in this phase. |

## FSST API Findings

### `vortex-fsst` physical layout

Relevant source: `~/.cargo/registry/src/index.crates.io-*/vortex-fsst-0.74.0/src/array.rs`.

`vortex-fsst` stores FSST arrays as:

- buffer 0: `symbols`
- buffer 1: `symbol_lengths`
- buffer 2: `compressed_codes_bytes`
- child slot 0: `uncompressed_lengths`
- child slot 1: `codes_offsets` with length `rows + 1`
- child slot 2: optional `codes_validity`

Useful public APIs:

- `FSSTData::symbols() -> &Buffer<Symbol>`
- `FSSTData::symbol_lengths() -> &Buffer<u8>`
- `FSSTData::codes_bytes() -> &ByteBuffer`
- `FSSTData::decompressor() -> fsst::Decompressor`
- `FSSTArrayExt::uncompressed_lengths() -> &ArrayRef`
- `FSSTArrayExt::codes() -> VarBinArray`

The fixture bridge can downcast `ArrayRef` with `as_opt::<vortex_fsst::FSST>()`,
then use `FSSTArrayExt` and `FSSTData` accessors to emit a Loom-owned params
payload. Vortex types must not cross into `loom-core`.

### Vortex canonical decode strategy

Relevant source: `vortex-fsst-0.74.0/src/canonical.rs`.

Vortex canonicalization bulk-decompresses the entire compressed string heap:

1. `fsst_array.codes().sliced_bytes()` gets all compressed bytes in logical slice
   order.
2. `uncompressed_lengths()` is executed to a primitive array.
3. A `Decompressor` decodes the heap into a single uncompressed bytes buffer.
4. Vortex builds string/binary views from uncompressed lengths and validity.

Loom can choose either per-row decode from offsets or bulk decode from the whole
heap. For MVP0, per-row decode is simpler to validate because each row can use
`codes_offsets[i]..codes_offsets[i + 1]`, apply validity, and append one string.
The plan should still carry `uncompressed_lengths` in params for validation and
capacity sizing.

### `fsst-rs` decoder API

Relevant source: `~/.cargo/registry/src/index.crates.io-*/fsst-rs-0.5.11/src/lib.rs`.

Useful APIs:

- `fsst::Symbol` is 8 bytes and exposes `Symbol::from_slice(&[u8; 8])`.
- `fsst::Decompressor::new(symbols: &[Symbol], lengths: &[u8])`.
- `Decompressor::decompress(compressed: &[u8]) -> Vec<u8>`.
- `Decompressor::decompress_into(compressed, spare_capacity_mut) -> usize`.
- `ESCAPE_CODE` is `255`; truncated escape input currently asserts in
  `fsst-rs`.

Risk: `fsst-rs` decode uses `assert!` for malformed compressed streams such as a
trailing escape byte. `loom-core` should validate enough params before calling
the decoder, and where malformed streams cannot be prevalidated cheaply, wrap the
kernel body in `catch_unwind` inside `FsstKernel::decode` and convert panics into
a typed `LoomDecodeError` variant. This is safe because `loom-core` currently
forbids unsafe code, and `catch_unwind` itself is safe.

## Loom-Owned FSST Params Recommendation

Add a small module in `loom-core`, for example `fsst_params.rs`, with:

- `FsstParams`
- `FsstParams::encode(...) -> Vec<u8>`
- `FsstParams::decode(params: &[u8], expected_count: usize) -> Result<Self, LoomDecodeError>`

Recommended binary format, little-endian:

| Field | Type | Notes |
|-------|------|-------|
| magic | `[u8; 4]` | Suggested `b"LFS1"` |
| version | `u16` | Start at `1` |
| flags | `u16` | bit 0 = validity present |
| row_count | `u64` | Must equal `KernelEscape.count` |
| symbol_count | `u16` | Must be `< 256` |
| reserved | `u16` | zero for now |
| symbols | `symbol_count * u64` | FSST symbols as little-endian `u64` |
| symbol_lengths | `symbol_count * u8` | each in `1..=8` |
| codes_offsets_len | `u64` | Must equal `row_count + 1` |
| codes_offsets | `codes_offsets_len * u64` | monotonic, last <= `codes_bytes.len()` |
| uncompressed_lengths | `row_count * u64` | expected decoded byte length per row |
| validity_len + bytes | optional | Arrow-style bits or simple bytes; planner may choose, but parser must be exact |
| codes_bytes_len | `u64` | compressed heap length |
| codes_bytes | bytes | concatenated compressed row payloads |

The exact format can be adjusted during implementation, but the parser must
validate:

- params have correct magic/version.
- `row_count == count`.
- `symbol_count < 256`.
- `symbol_lengths.len() == symbol_count`.
- every symbol length is `1..=8`.
- `codes_offsets.len() == row_count + 1`.
- offsets are monotonic and last offset is at most `codes_bytes.len()`.
- `uncompressed_lengths.len() == row_count`.
- validity, if present, has at least `row_count` bits/entries.
- each valid row's decoded byte length equals `uncompressed_lengths[row]`.
- output bytes are valid Utf8 before appending to `StringBuilder`.

## Utf8 Dictionary Gather Findings

Current state:

- `OutputBuilder` supports Boolean, Int32, and Int64.
- `DecodedArray` supports Boolean, Int32, and Int64.
- `Dictionary` decodes values with `builder.data_type()` and appends through
  `DecodedArray::append_value_to_builder`.
- top-level `KernelEscape` already returns `ArrayData`, but direct
  `synthesized_read_loop` on `KernelEscape` still returns
  `UnimplementedEncoding("KernelEscape")`.

Plan impact:

- Extend `OutputBuilder` with `String(arrow::array::StringBuilder)`,
  `append_string(&str)`, null handling, and Utf8 `finish()`.
- Extend `DecodedArray` with `Utf8(StringArray)`.
- Extend `decode_node_to_array_data` so nested `KernelEscape` can dispatch
  through `L2KernelRegistry` when Dictionary values are `KernelEscape(FSST)`.
  A conservative approach is to add a registry-aware helper and thread the
  registry through dictionary/run-end child materialization. Preserve the direct
  read-loop typed error for callers that do not provide a registry.
- For dict-over-FSST, codes null => output null; value null => output null;
  otherwise append the string value.

## Fixture and Oracle Findings

Existing useful patterns:

- `loom-fixtures/src/vortex_reader.rs` is the only crate that uses Vortex APIs.
- `from_array_ref` already downcasts to FastLanes BitPacked/FoR, Dict,
  Primitive, and Bool.
- Existing tests compare Arrow values/nulls with oracle values/nulls.
- RLE canonicalization is allowed when Vortex's physical representation does not
  directly match Loom's simple model, but the live Vortex oracle remains the
  source of truth.

Needed additions:

- Add FSST downcast in `from_array_ref`:
  `array.as_opt::<vortex_fsst::FSST>()`.
- Add `from_fsst_view` / `from_fsst_array` that emits
  `LayoutNode::KernelEscape { kernel_id: 0, params, count }`.
- Add `decode_utf8_oracle(array: &ArrayRef) -> (Vec<Option<String>>, Vec<bool>)`
  or equivalent. Prefer Vortex execution to canonical Utf8/Binary view arrays
  rather than copied literals.
- Add tests for:
  - top-level FSST edge cases: empty string, escape-heavy strings, 8-byte symbol
    strings, nulls.
  - dict-over-FSST: DictArray codes over FSST values, including nullable codes
    if straightforward.
  - regression that `loom-core` still has no Vortex/FastLanes dependencies.

Potential API wrinkle: Vortex canonical string output may be `VarBinViewArray`
with Utf8 dtype, not Arrow `StringArray` directly. The oracle helper should
execute through Vortex and collect scalar/string values using Vortex accessor
APIs if direct Arrow conversion is not exposed in the already-imported crates.

## DuckDB SQL Gate Findings

Current state:

- `duckdb-ext/loom_extension.cpp` registers `loom_scan(VARCHAR)`.
- `LoomInit` calls `loom_decode(nullptr, 0, ...)`.
- `LoomBind` hardcodes one nullable `INTEGER value` column.
- `LoomScan` reads Arrow C buffers for a flat Int32 array:
  - `buffers[0]` validity bitmap
  - `buffers[1]` values
- `scripts/duckdb-smoke-test.sh` builds the extension, loads it with DuckDB
  v1.5.3 `-unsigned`, and checks `SELECT count(*)` plus `SELECT *`.

Plan impact:

- `loom_decode_inner` must stop ignoring input bytes. It should parse a simple
  test/demo input format sufficient to select or carry a serialized
  `LayoutDescription` / fixture payload. The executor can choose a minimal
  internal format for MVP0, but the DuckDB path must no longer be hardcoded to
  `[1, 2, 3, null]`.
- `loom_scan` should pass the `VARCHAR` argument bytes to `loom_decode`.
- `LoomBind` currently cannot know output type from the path unless it parses the
  same input or uses a file/fixture naming convention. For Phase 05, the simplest
  safe path is to keep one `value` column and infer type in bind from the
  argument string using the same tiny fixture descriptor format as FFI.
- Extend direct `DataChunk` population for:
  - Int32 (already present)
  - Boolean if included in SQL all-encoding checks
  - Utf8 strings for FSST and dict-over-FSST
- DuckDB string vector APIs should be confirmed in `duckdb.hpp` during execution.
  Search targets: `StringVector::AddString`, `FlatVector::GetData<string_t>`,
  `string_t`.

Verification should end with:

- `cargo test --workspace`
- `cargo tree -p loom-core | grep -c -E 'vortex|fastlanes'` prints `0`
- `bash scripts/duckdb-smoke-test.sh` updated to cover `SELECT *` and aggregate
  queries over supported encodings, including FSST/dict-over-FSST.

## Validation Architecture

Plan checker should require these validation dimensions:

1. **Core correctness:** `loom-core` unit tests for params parsing, FSST decode,
   malformed params, Utf8 builder, nested `KernelEscape`, and dict-over-FSST
   gather.
2. **Boundary preservation:** `cargo tree -p loom-core | grep -c -E 'vortex|fastlanes'`
   prints `0`.
3. **Oracle row match:** `loom-fixtures` tests compare values and null flags to
   Vortex oracle for FSST and dict-over-FSST.
4. **Workspace regression:** `cargo test --workspace` passes.
5. **DuckDB acceptance:** smoke script loads the extension and runs real DuckDB
   `SELECT` and aggregate SQL over Loom-decoded data.

## Risks and Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| `fsst-rs` panics on malformed compressed streams | High | Validate params and convert `catch_unwind` failures in `FsstKernel::decode` into typed decode errors. |
| Nested `KernelEscape` cannot currently dispatch in Dictionary values | High | Add a registry-aware child materialization helper; preserve direct read-loop error for no-registry callers. |
| Vortex Utf8 oracle collection API differs from primitive helpers | Medium | Research during implementation in `loom-fixtures`; collect through Vortex canonical/scalar APIs rather than adding Vortex deps to core. |
| DuckDB bind needs output type before scan | Medium | Use a minimal descriptor/fixture input format parsed in bind and passed to FFI; keep one-column schema. |
| Phase 05 becomes too large | Medium | Split into three plans/waves: core, fixtures, DuckDB gate. |

## Recommended Plan Split

1. **05-01 Core FSST and Utf8 L1/L2 integration** - `loom-core`, `L2-02`,
   `L2-03`.
2. **05-02 Vortex FSST fixtures and oracle row-match** - `loom-fixtures`,
   `VERIFY-01`, `VERIFY-02`.
3. **05-03 FFI/DuckDB SQL acceptance gate** - `loom-ffi`, `duckdb-ext`,
   `VERIFY-03`, with final full verification.

---

*Research complete: 2026-06-08*
