# Feature Research

**Domain:** Rust decoder prototype — Vortex-encoded column → L1/L2 decode → Arrow → DuckDB
**Researched:** 2026-06-07
**Confidence:** HIGH (requirements derived from locked design.md + PROJECT.md; stack confirmed in STACK.md)

---

## Feature Landscape

### Table Stakes (Demo Cannot Run Without These)

| Feature | Why Required | Complexity | Notes |
|---------|--------------|------------|-------|
| **Vortex array deserializer** | Every other building block needs the encoded bytes in memory as a structured object before it can decode anything | LOW | Use `vortex-array` `ArrayRef` + encoding-specific `try_from`. The MVP0 input is a single in-memory column, not a full `.vortex` file; no footer/chunk parsing needed. Produces an opaque `ArrayRef` that knows its encoding ID. |
| **L1 bitpack decoder** | BitPacked is the primary Vortex numeric encoding; without it the demo has no integer column to query | MEDIUM | Input bytes: a packed bit-stream of `N` values at fixed `width` bits each (e.g. 11 bits/value), plus an `offset` (FOR base) baked in or separate. Output Arrow type: `Int32Array` / `Int64Array` (or `UInt*` depending on declared type). Algorithm: shift+mask per lane, width is a compile-time or runtime constant from the L1 descriptor. Complexity driver: bit-boundary addressing when `width` is not byte-aligned, and correctly reconstructing negative values after adding FOR base. Depends on: Vortex array deserializer (to extract the packed buffers). |
| **L1 FOR (Frame-of-Reference) decoder** | FOR is always composed with bitpack in Vortex; it reconstructs absolute values from delta-coded bitpacked integers | LOW | Input bytes: same packed bit-stream as bitpack; additionally a scalar `reference` (the frame origin, same dtype as output). Output: same Arrow integer type. Algorithm: add `reference` to each unpacked value. In Vortex, `FoRArray` wraps a `BitPackedArray`; decoding is bitpack-first then broadcast-add the reference scalar. Depends on: L1 bitpack decoder. |
| **L1 dict decoder** | Dictionary encoding is ubiquitous for low-cardinality string or integer columns | MEDIUM | Input bytes: two sub-arrays — a `codes` array (small integer type, e.g. `UInt8`/`UInt16`) and a `values` array (the dictionary entries, any type). Output Arrow type: matches the values array type (e.g. `StringArray`, `Int32Array`); for strings Arrow uses `StringArray` (offsets buffer + values buffer). Algorithm: for each code, index into the decoded values array. Depends on: Vortex array deserializer for both sub-arrays; the values sub-array may itself be L1-encoded (e.g. FSST-encoded strings in the dict values — the L1→L2 escape is exercised here). |
| **L1 RLE decoder** | Run-length encoding handles monotone / low-variance integer and boolean columns | MEDIUM | Input bytes: Vortex uses "run-end encoding" — two parallel sub-arrays: `run_ends` (sorted integer offsets marking where each run ends) and `values` (one value per run). Output Arrow type: whatever the `values` element type is — `Int32Array`, `BooleanArray`, etc. Algorithm: binary-search or linear scan of `run_ends` to expand each run into the output. Depends on: Vortex array deserializer for both sub-arrays; `RunEndArray` in `vortex-array`. |
| **L1→L2 escape / kernel reference mechanism** | Without this there is no L2; it is the joint between the two layers | LOW | Design: the L1 layout descriptor holds a `codec = kernel#N` field on the segment that cannot be handled declaratively. The interpreter sees this tag, looks up kernel N in the kernel table, and dispatches. In MVP0 there is exactly one kernel (FSST, id=0). Implementation: a match arm in the interpreter dispatch loop. Depends on: Vortex array deserializer (to identify `FsstArray`); L1 descriptor parsing. |
| **FSST L2 kernel** | FSST is the one total-function kernel MVP0 proves; it is the demo's proof that the L2 escape works end-to-end | HIGH | Input bytes: (a) an FSST symbol table (up to 255 symbols of up to 8 bytes each, stored as a header in the encoded array); (b) a stream of 8-bit codes where codes 0–254 index into the symbol table and code 255 (escape) means the next byte is a literal. Output Arrow type: `StringArray` (variable-length UTF-8; Arrow layout = offsets `Buffer<i32>` + values `Buffer<u8>`). Algorithm: iterate codes; for each non-escape code concatenate `symbol_table[code]` to the current string accumulator; for escape, emit the following literal byte; emit one Arrow string value per logical row boundary. Complexity driver: correctly reconstructing row boundaries (a separate `string_lengths` or `offsets` sub-array in `FsstArray` marks where each row ends in the code stream). Use `fsst-rs` `Decompressor` via `vortex-fsst`; do NOT reimplement. Depends on: L1→L2 escape mechanism; typed Arrow string builder (before FSST can emit its output). |
| **Typed Arrow builders → ArrowArray/ArrowSchema** | Every L1 decoder and FSST need somewhere to write output; builders are the only legal output primitive (design §6) | MEDIUM | Use `arrow-rs` 58.x typed builders: `Int32Builder`, `Int64Builder`, `StringBuilder`, `BooleanBuilder`, etc. Each builder accumulates values and nulls, then `.finish()` materializes an `ArrayRef`. From `ArrayRef` extract `ArrayData` and call `arrow::ffi::to_ffi` to get `FFI_ArrowArray` + `FFI_ArrowSchema`. The builder abstraction guarantees offset/null-bitmap consistency — validation is free. Depends on: nothing; this is the foundation all decoders write into. Must come before any decoder that emits output. |
| **Arrow C Data Interface FFI export** | The Rust core and the C++ DuckDB extension speak different languages; Arrow C Data Interface is the zero-copy bridge | MEDIUM | Use `arrow::ffi::to_ffi` (feature `ffi` on the `arrow` crate). Expose `extern "C" fn loom_decode(out_array: *mut FFI_ArrowArray, out_schema: *mut FFI_ArrowSchema)` from the Rust staticlib. Memory model: `FFI_ArrowArray.release` callback handles deallocation; ownership transfers to C++ caller. cbindgen generates `loom.h` automatically. Depends on: typed Arrow builders (need a finished `ArrayData` to convert); Rust staticlib build configuration. |
| **DuckDB C++ table function** | The host engine that runs the SQL query; without it the demo chain is incomplete | MEDIUM | Implement the bind/init/scan callback pattern in C++. Bind: call into Rust to get `FFI_ArrowSchema`, declare `LogicalType` to DuckDB. Scan: call `loom_decode`, receive `FFI_ArrowArray`, convert to `DataChunk` via `ArrowToDuckDB()` or `arrow_scan` built-in. Register as `loom_scan(path)`. Depends on: Arrow C Data Interface FFI export (Rust side must be linked before C++ extension builds). |
| **Verification harness** | Acceptance criterion is "DuckDB SQL results match Vortex's decoder row-for-row" — without the harness the demo has no falsifiable success bar | MEDIUM | Two-path comparison: (1) decode the same serialized column through Vortex's own `into_canonical().into_arrow()` path; (2) decode through the Loom interpreter. Run `SELECT * FROM loom_scan(...)` and compare against the Vortex reference row-for-row (checksum or full value equality). Implement as a Rust test + a small shell script that drives DuckDB with `.sql`. Depends on: all decode paths; DuckDB table function; Vortex crate available as a dev-dependency. |

---

### Differentiators / Nice-to-Have (Post-Demo)

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Human-readable L1 layout descriptor (text format)** | Makes the "L1 is data, not code" thesis visible to a reviewer; aids debugging and exposition | LOW | A simple TOML or S-expression representation of the layout tree (encoding type, width, reference scalar, kernel reference id). Not required for correctness — the interpreter can be wired directly from Vortex's in-memory representation. Add after the decode chain runs. |
| **Multiple sample columns** | Demonstrates breadth; a single column is enough to prove the chain but multiple encodings shown together are more convincing | LOW | Extend the verification harness to run against several pre-generated `.vortex` column files: one bitpack/FOR integer column, one dict-of-strings column, one FSST column, one RLE boolean column. Depends on: all four L1 decoders + FSST working individually. |
| **CLI driver (`loom decode <file> <column>`)** | Lets a reviewer try the decoder without writing Rust test code; lowers the bar for live demo | LOW | A thin Rust binary (separate from the library) that reads a serialized Vortex column, runs the Loom interpreter, and prints results to stdout or a CSV. Depends on: the library decode path being complete. |
| **Wall-clock timing output** | Concretely shows the overhead of the interpreter vs Vortex's native decode path; a talking point in the demo | LOW | Wrap the decode call in `std::time::Instant` in the harness. Print both paths' elapsed time. No optimization required — MVP0 is a correctness prototype, timing is illustrative. |

---

### Anti-Features (Explicitly Out of Scope for MVP0)

| Anti-Feature | Why It Seems Appealing | Why It Must Stay Out | What to Do Instead |
|--------------|------------------------|---------------------|--------------------|
| **MLIR / native codegen** | "Real" Loom eventually lowers to native; including it would make the demo faster | Adds weeks of complexity; MVP0 proves correctness via interpretation, not speed; the design (§8) places MLIR firmly after the distribution/safety layer is proven | Interpret directly in Rust; annotate the interpreter hotpaths as future MLIR lowering targets |
| **Formal verifier / totality proofs** | Loom's safety story requires a verifier; without one the "total function" claim looks hollow | The acceptance bar is correct SQL results, not sandbox safety; building a verifier before the decode chain exists is premature and would consume all time | Defer to a later milestone; document where termination checks would be inserted in the interpreter dispatch loop |
| **Full `.vortex` file layout (footer/chunk/layout tree)** | A real file reader would be more impressive than a single in-memory column | Multi-chunk file parsing is a separate, large problem; it adds a file-format parsing layer that is orthogonal to the L1/L2 decode chain | Serialize a single column to bytes using Vortex's own serialization helpers and feed those bytes directly to the Loom decoder |
| **Multi-column tables / schema assembly** | SQL queries over real tables use multiple columns | Single column is sufficient to prove the chain; multi-column requires schema merging, projection masks, and multi-array FFI — all out of scope | Use a single column; the DuckDB table function returns a one-column result table; SQL can still aggregate over it |
| **`statistics()` / `projection_mask` / `range` random-access ABI** | These are part of the full decoder ABI (design §9) and enable predicate pushdown and I/O skipping | None of these are needed for a full sequential decode of one column; they add surface area without proving anything new | Implement only `schema()` and `decode_batch()` with a trivial full-range argument |
| **Distribution container / feature flags / content-hash URI** | The design (§10–11) describes a versioned container for shipping decoders with data | Distribution concerns are irrelevant until the decode chain is correct and the design is validated | The demo loads the decoder as a compiled DuckDB extension; distribution is a future milestone |
| **Additional L2 kernels (ALP, delta-of-delta, etc.)** | More kernels demonstrate greater generality | One kernel (FSST) is sufficient to prove the L2 escape mechanism; additional kernels require the same mechanism and add no new insight for MVP0 | Design the kernel dispatch table to be extensible; leave slots commented for future kernels |

---

## Per-Encoding Technical Details

### Bitpack (`FoRArray` wrapping `BitPackedArray`)

- **Input consumed:** A contiguous buffer of `ceil(N * width / 8)` bytes. The `width` parameter (1–64 bits) is stored in the `BitPackedArray` metadata. Optionally a scalar `offset` (the FOR reference) lives in the `FoRArray` wrapper.
- **Output Arrow type:** `Int32Array` or `Int64Array` (signed, after adding the FOR reference). Raw bitpacked arrays without FOR are `UInt*`.
- **Algorithm:** For each output index `i`, read bits `[i*width, (i+1)*width)` from the packed buffer (crossing byte boundaries with shift+mask), cast to the output integer type, add the FOR reference scalar.
- **Rough complexity:** 2–3 days. Bit-boundary arithmetic is fiddly but well-understood. Use `vortex-fastlanes` `BitPackedArray::try_from` + `FoRArray::try_from` to extract the buffers and metadata rather than parsing raw bytes manually.
- **Dependencies:** Vortex array deserializer, typed Arrow integer builders.

### FOR (Frame-of-Reference)

- **Input consumed:** No independent byte stream — FOR is always a wrapper around another encoded array (in practice always bitpack in Vortex). The scalar `reference` value is metadata in `FoRArray`.
- **Output Arrow type:** Same as the inner bitpack array's output type (signed integer).
- **Algorithm:** Decode the inner bitpack array first, then broadcast-add the `reference` scalar to each element.
- **Rough complexity:** 0.5 days (trivial once bitpack works; it is a scalar broadcast).
- **Dependencies:** L1 bitpack decoder.

### Dictionary (`DictArray`)

- **Input consumed:** Two sub-arrays: a `codes` array (typically `UInt8` or `UInt16`, one code per output row) and a `values` array (the dictionary entries, any type).
- **Output Arrow type:** Matches `values` element type. For strings: `StringArray`. For integers: `Int32Array` etc. Arrow has a native `DictionaryArray` type but for MVP0 it is simpler to expand (decode each code to its value) — this avoids teaching DuckDB about dictionary-typed Arrow.
- **Algorithm:** Decode the `codes` sub-array (may itself be L1-encoded, e.g. bitpacked); decode the `values` sub-array (may be FSST-encoded — this is where the L1→L2 escape fires); for each code index, copy `values[code]` into the output builder.
- **Rough complexity:** MEDIUM — 2 days. The indirection through two sub-arrays each of which may be independently encoded is the complexity driver. Must handle FSST values correctly.
- **Dependencies:** Vortex array deserializer, L1→L2 escape mechanism (for FSST values), typed Arrow builders. Dict over FSST strings depends on FSST kernel.

### RLE (`RunEndArray`)

- **Input consumed:** Two sub-arrays: `run_ends` (sorted integer array, e.g. `UInt32`, one end-offset per run) and `values` (one value per run, any type).
- **Output Arrow type:** Same as `values` element type.
- **Algorithm:** Walk output row index `i`; binary-search `run_ends` for the run containing `i`; emit `values[run_index]`. Or linear scan if runs are short. The Vortex `RunEndArray` stores 1-indexed run-end positions in the Arrow convention.
- **Rough complexity:** LOW-MEDIUM — 1–2 days. The binary-search expansion is straightforward. The only subtlety is correctly handling the Arrow run-end convention (end is exclusive, 1-indexed from row 0).
- **Dependencies:** Vortex array deserializer, typed Arrow builders.

### FSST (`FsstArray`)

- **Input consumed:** (a) Symbol table header: up to 255 entries, each up to 8 bytes (stored in `FsstArray` metadata). (b) A code byte stream: one byte per code; code < 255 indexes into the symbol table; code == 255 means next byte is a literal. (c) A `string_lengths` or cumulative `offsets` sub-array that marks row boundaries in the expanded byte stream.
- **Output Arrow type:** `StringArray` (Arrow layout: `i32` offsets buffer + `u8` values buffer + validity bitmap).
- **Algorithm:** Use `fsst_rs::Decompressor` (via `vortex-fsst`): (1) construct `Decompressor` from the symbol table; (2) call `decompressor.decompress(code_stream)` to get the raw concatenated string bytes; (3) use the `offsets` sub-array from `FsstArray` to split the flat byte output into individual `StringArray` values; (4) feed each string slice to `StringBuilder::append_value`.
- **Rough complexity:** HIGH — 3–4 days. The complexity is not the FSST algorithm itself (handled by `fsst-rs`) but correctly extracting `FsstArray`'s internal structure (symbol table bytes, code buffer, offsets/lengths sub-array) from the Vortex in-memory representation, and correctly feeding the result into an Arrow `StringBuilder`.
- **Dependencies:** L1→L2 escape mechanism (FSST is only reached via the escape); typed Arrow string builder (must exist before FSST emits output); `vortex-fsst` crate.

---

## Feature Dependencies

```
[Typed Arrow builders]
    └──required-by──> [L1 bitpack decoder]
    └──required-by──> [L1 FOR decoder]
    └──required-by──> [L1 dict decoder]
    └──required-by──> [L1 RLE decoder]
    └──required-by──> [FSST L2 kernel]

[Vortex array deserializer]
    └──required-by──> [L1 bitpack decoder]
    └──required-by──> [L1 dict decoder]
    └──required-by──> [L1 RLE decoder]
    └──required-by──> [L1→L2 escape mechanism]

[L1 bitpack decoder]
    └──required-by──> [L1 FOR decoder]
    └──required-by──> [L1 dict decoder]  (codes sub-array is often bitpacked)

[L1→L2 escape mechanism]
    └──required-by──> [FSST L2 kernel]

[FSST L2 kernel]
    └──required-by──> [L1 dict decoder]  (when dict values are FSST-encoded strings)

[Arrow C Data Interface FFI export]
    └──required-by──> [DuckDB C++ table function]
    └──requires──> [Typed Arrow builders]

[DuckDB C++ table function]
    └──required-by──> [Verification harness]

[All L1 decoders + FSST]
    └──required-by──> [Verification harness]
```

### Dependency Notes

- **Typed Arrow builders before any decoder:** All decoders write into builders; builders must exist and be understood before implementing any decode path. This is the foundation layer — implement and test builders in isolation first.
- **Vortex array deserializer before L1 decoders:** Decoders must extract encoding-specific metadata (width, reference scalar, sub-arrays) from the Vortex `ArrayRef`. The deserializer provides this. In practice "deserializer" means understanding `BitPackedArray::try_from`, `FoRArray::try_from`, `DictArray::try_from`, `RunEndArray::try_from` — one or two days of reading Vortex source.
- **Bitpack before FOR:** FOR adds a scalar to the bitpack output; it has no independent byte stream. Bitpack must be correct before FOR can be tested.
- **L1→L2 escape before FSST:** FSST is only invoked via the escape mechanism in the interpreter dispatch loop. The escape routing must exist before the FSST kernel is wired up.
- **FSST before dict-over-FSST:** The dict decoder, when its values sub-array is `FsstArray`, dispatches through the escape to FSST. Full dict support requires FSST to be complete.
- **Arrow FFI export before DuckDB extension:** The C++ table function calls into the Rust staticlib via the `loom_decode` extern-C function. The Rust side must compile successfully and expose that symbol before the C++ extension can link.
- **Everything before verification harness:** The harness exercises the full end-to-end chain; it is the last thing to integrate.

---

## MVP Definition

### Launch With (MVP0 — all required)

- [ ] Vortex array deserializer — entry point for all data
- [ ] Typed Arrow builders (Int32, Int64, String, Boolean) — foundation layer
- [ ] L1 bitpack decoder — primary numeric encoding
- [ ] L1 FOR decoder — composes with bitpack; adds scalar reference
- [ ] L1 dict decoder — low-cardinality columns; exercises sub-array dispatch
- [ ] L1 RLE decoder — monotone / run-length columns
- [ ] L1→L2 escape / kernel dispatch mechanism — proves the layer boundary
- [ ] FSST L2 kernel — the one total-function compute kernel; proves L2 escape
- [ ] Arrow C Data Interface FFI export (Rust staticlib + cbindgen header)
- [ ] DuckDB C++ table function (bind/init/scan; adopts Arrow array)
- [ ] Verification harness (Vortex reference decode vs Loom decode, row-for-row)

### Add After Validation (post-MVP0)

- [ ] Human-readable L1 layout descriptor format — when explaining the design to collaborators
- [ ] Multiple sample columns (one per encoding) — when preparing a broader demo
- [ ] CLI driver — when non-Rust reviewers need to run the demo
- [ ] Wall-clock timing comparison — when making the performance story concrete

### Future Consideration (later milestones)

- [ ] Additional L2 kernels (ALP float, delta-of-delta) — after L2 escape pattern is validated
- [ ] Full `.vortex` file format parsing — when multi-chunk columnar files are needed
- [ ] Multi-column table function — when multi-column SQL queries are needed
- [ ] `statistics()` / `projection_mask` / `range` ABI — when predicate pushdown is a goal
- [ ] Formal verifier / totality proofs — the safety milestone
- [ ] MLIR `decode` dialect / native codegen — the performance milestone
- [ ] Distribution container / feature flags — the portability milestone

---

## Feature Prioritization Matrix

| Feature | Demo Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Vortex array deserializer | HIGH | LOW | P1 |
| Typed Arrow builders | HIGH | LOW | P1 |
| L1 bitpack decoder | HIGH | MEDIUM | P1 |
| L1 FOR decoder | HIGH | LOW | P1 |
| L1 dict decoder | HIGH | MEDIUM | P1 |
| L1 RLE decoder | MEDIUM | MEDIUM | P1 |
| L1→L2 escape mechanism | HIGH | LOW | P1 |
| FSST L2 kernel | HIGH | HIGH | P1 |
| Arrow C Data Interface FFI export | HIGH | MEDIUM | P1 |
| DuckDB C++ table function | HIGH | MEDIUM | P1 |
| Verification harness | HIGH | MEDIUM | P1 |
| Human-readable L1 descriptor | MEDIUM | LOW | P2 |
| Multiple sample columns | MEDIUM | LOW | P2 |
| CLI driver | LOW | LOW | P2 |
| Timing numbers | LOW | LOW | P2 |
| Additional L2 kernels | LOW | HIGH | P3 |
| Full file format | LOW | HIGH | P3 |
| Multi-column support | LOW | HIGH | P3 |
| statistics() / range ABI | LOW | MEDIUM | P3 |
| Formal verifier | LOW | VERY HIGH | P3 |
| MLIR / native codegen | LOW | VERY HIGH | P3 |

**Priority key:**
- P1: Required for MVP0 — demo cannot run without it
- P2: Add after MVP0 validates the chain
- P3: Future milestone

---

## Sources

- `design.md` — Loom full design (Chinese); §3 L1/L2 split, §4 L1 encodings, §5 L2 total-function kernels, §6 output contract, §9 ABI, §10 distribution container, §13 hard bones
- `.planning/PROJECT.md` — MVP0 requirements (Active), out-of-scope list, key decisions
- `.planning/research/STACK.md` — Vortex crate structure, arrow-rs 58.x, fsst-rs 0.5.11, DuckDB 1.5.3 extension pattern
- [vortex-data/vortex](https://github.com/vortex-data/vortex) — FsstArray, BitPackedArray, FoRArray, DictArray, RunEndArray internal structure
- [fsst-rs docs.rs](https://docs.rs/fsst-rs/latest/fsst/) — `Decompressor` API
- [arrow-rs ffi module](https://docs.rs/arrow/latest/arrow/ffi/index.html) — `to_ffi`, `FFI_ArrowArray`, `FFI_ArrowSchema`

---
*Feature research for: Loom MVP0 — Vortex-encoded column → L1/L2 decode → Arrow → DuckDB*
*Researched: 2026-06-07*
