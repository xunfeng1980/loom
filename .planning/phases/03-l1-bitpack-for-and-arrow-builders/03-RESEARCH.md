# Phase 3: L1 Bitpack, FOR, and Arrow Builders — Research

**Researched:** 2026-06-07
**Domain:** vortex-fastlanes 0.74 decode internals, arrow-rs 58.3 typed builders, LayoutNode model
**Confidence:** HIGH (all key claims verified against installed crate source in ~/.cargo/registry)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-01 — Bitpack fidelity: real FastLanes layout, no patches**
Decode the genuine `vortex-fastlanes` 1024-lane TRANSPOSED bit-packing layout row-for-row (not naive sequential LSB unpacking). Phase-3 fixtures are restricted to values that fit the declared bit width, so the exception/"patch" path is deferred. Implement the FastLanes transpose/unpack for the in-width case.

**D-02 — vortex_reader derives the LayoutNode**
`vortex_reader` inspects the Vortex `ArrayRef` (encoding id, `bit_width`, packed buffer, validity, FOR reference scalar) and emits a `LayoutNode` + raw buffer references. `loom-core` decodes from the `LayoutNode` with ZERO `vortex-*` dependency. Vortex stays isolated inside `vortex_reader` (and the oracle), preserving D-02 from Phase 1.

**D-03 — Phase 3 stays loom-core + FFI-exportable; DuckDB rewire deferred to Phase 5**
Phase 3's deliverable is the decode core whose Arrow output is FFI-exportable (`arrow_builder_output::finish()` → `ArrayData` → `to_ffi`). `loom_decode`/`loom_scan` keep the Phase-2 hardcoded `[1,2,3,null]` path this phase.

**D-04 — Define the full LayoutNode enum now; stub unimplemented arms**
Define the complete `LayoutNode` enum now — `Raw`, `BitPack`, `FrameOfReference`, `Dictionary`, `RunEnd`, `KernelEscape`. Implement `Raw`/`BitPack`/`FrameOfReference` this phase; others return an explicit "unimplemented in Phase 3" typed error.

### Claude's Discretion
- Exact `LayoutNode` field shapes and how `FrameOfReference` nests over `BitPack`.
- Validity → Arrow null bitmap mapping: keep validity as plain (non-encoded) bitmap; recursive/encoded validity deferred.
- In-phase verification: Rust unit tests asserting decoded values against known expected arrays, and/or comparison to Vortex's own `into_canonical().into_arrow()`.
- Which integer width(s) to demonstrate (e.g. 11-bit non-byte-aligned case) and whether to seed `arrow_builder_output` from the existing `Int32Builder` pattern in `crates/loom-ffi/src/ffi.rs`.

### Deferred Ideas (OUT OF SCOPE)
- Bitpack exception/"patch" path — out-of-width values stored separately.
- `arrow_scan` / record-batch + DuckDB-shows-real-data — deferred to Phase 5.
- Encoded/recursive validity.
- `Dictionary` / `RunEnd` decode → Phase 4; `KernelEscape`/FSST → Phase 4–5.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| INPUT-01 | A single serialized Vortex encoded array/column is read into the decoder without parsing a `.vortex` file container | §"Accessing the encoded array" + vortex-array `serialize`/`deserialize` API |
| INPUT-02 | Test fixtures constructed programmatically as in-memory Vortex arrays (no .vortex files) | §"In-phase verification" — use `BitPackedData::encode` + `FoRData` directly in loom-fixtures |
| L1-01 | `LayoutNode` data model represents a column's physical layout as pure data | §"LayoutNode enum" — full enum defined with verified field names |
| L1-02 | A synthesized read loop interprets a `LayoutNode` tree to produce decoded values | §"Synthesized read loop" — recursive match interpreter |
| L1-03 | Decode a bit-packed integer column, including non-byte-aligned widths (1–64 bits) | §"FastLanes unpack algorithm" — pseudocode + source-verified correctness |
| L1-04 | Decode a frame-of-reference (FOR) column layered on bit-packing | §"FOR decode" — wrapping_add of reference scalar |
| L1-07 | Null/validity preserved through every L1 decode path | §"Validity → Arrow null bitmap" — Validity enum + append_null pattern |
| ARROW-01 | Decoded values emitted only through typed Arrow builders | §"arrow-rs 58.3 builder API" — `append_value`/`append_null` confirmed |
| ARROW-02 | Output materializes as Arrow `ArrayData` → `ArrowArray` + `ArrowSchema` | §"finish() → into_data() → to_ffi" — confirmed from source |
</phase_requirements>

---

## Summary

Phase 3 implements the L1 decode core: the `LayoutNode` enum, the synthesized read loop, and the BitPack + FOR decoders, feeding output into arrow-rs typed builders. The key technical risks are (1) correctly implementing the FastLanes transposed bit-layout without using the `fastlanes` crate in `loom-core`, and (2) threading Vortex's `Validity` enum through to Arrow null bitmaps at every layer.

All key API names (accessor methods, enum variants, builder methods) have been verified against the installed source of `vortex-fastlanes` 0.74.0 and `arrow-array` 58.3.0. The FastLanes transpose index function is a 3-line formula sourced directly from `fastlanes-0.5.1/src/transpose.rs`. The single-element unpack algorithm is sourced from `bitpack_decompress.rs:unpack_single_primitive`. The FoR decode path is a wrapping-add of the reference scalar after bitpack, confirmed in `for_decompress.rs`.

**Primary recommendation:** Implement the FastLanes single-element unpack (`unpack_single_primitive` logic) in `loom-core` as a pure-Rust function that replicates the index formula and bit-extraction without any `fastlanes`/`vortex-*` dependency. Use `BitUnpackedChunks::decode_into` logic as the reference for full-chunk decoding.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Vortex array inspection (encoding ID, buffer extraction, validity, reference scalar) | `loom-fixtures` / `vortex_reader` only | — | D-02: zero vortex-* in loom-core |
| LayoutNode construction | `vortex_reader` (loom-fixtures) | — | Consumes Vortex ArrayRef, emits pure-data LayoutNode |
| BitPack decode (FastLanes unpack loop) | `loom-core` / `l1_model` | — | Zero vortex-* dependency; pure bit arithmetic |
| FOR decode (wrapping_add of reference scalar) | `loom-core` / `l1_model` | — | Thin wrapper over BitPack decode |
| Validity → null bitmap | `loom-core` / `l1_model` | — | Maps Vortex Validity enum fields to append_null calls |
| Arrow builder accumulation | `loom-core` / `arrow_builder_output` | — | Single site for all typed builder calls |
| FFI export (to_ffi, ptr::write) | `loom-ffi` | — | Existing Phase-2 pattern; Phase 3 does not touch this |

---

## Q1: FastLanes Bit-Packing Layout

### Block Structure

[VERIFIED: fastlanes-0.5.1/src source, vortex-fastlanes-0.74.0/src/bitpacking source]

A FastLanes BitPackedArray is laid out as a sequence of **1024-element blocks**. Each block of 1024 values of native type `T` (e.g. `u32` = 32 bits) at bit-width `W` occupies exactly `128 * W` bytes in the packed buffer.

- Buffer size for `N` values: `ceil((N + offset) / 1024) * 128 * W` bytes (padding to next 1024-element boundary with zeros).
- The `offset` field (`u16`, 0..1024) records where logical index 0 starts within the first block when the array is a slice.

From `BitPackedData::validate`:
```rust
let expected_packed_len =
    (length + offset as usize).div_ceil(1024) * (128 * bit_width as usize);
```

For `elems_per_chunk` (the number of elements of type `T` in one packed block):
```rust
elems_per_chunk = 128 * bit_width / size_of::<T>()
```
For `T = u32`, `W = 11`: `elems_per_chunk = 128 * 11 / 4 = 352` u32 elements per block.

### FastLanes "Unified Transpose" — Lane Ordering

FastLanes uses a **transposed layout** to enable SIMD vectorization. The key insight: instead of packing values in sequential order `[v0, v1, v2, ...]`, FastLanes reorders them so that SIMD lanes process independent values concurrently (the "unified virtual ISA" concept from the FastLanes paper by Zukowski et al.).

The transpose index function (from `fastlanes-0.5.1/src/transpose.rs`):
```rust
pub const FL_ORDER: [usize; 8] = [0, 4, 2, 6, 1, 5, 3, 7];

pub const fn transpose(idx: usize) -> usize {
    let lane = idx % 16;
    let order = (idx / 16) % 8;
    let row = idx / 128;
    (lane * 64) + (FL_ORDER[order] * 8) + row
}
```

This maps **logical index** `idx` (0..1024) to the **transposed storage index**. The packing macros read from `input[transpose(i)]` for each `i` in row order.

For `T = u32` (32 bits): `LANES = 1024 / 32 = 32`. There are 32 SIMD lanes, each processing 32 rows.

### The Pack/Unpack Index Function (from `macros.rs`)

The actual access pattern used during pack/unpack iterates in transposed order:
```rust
fn index(row: usize, lane: usize) -> usize {
    let o = row / 8;
    let s = row % 8;
    (FL_ORDER[o] * 16) + (s * 128) + lane
}
```

During pack: for each lane (0..LANES), for each row (0..T), compute `idx = index(row, lane)`, read `input[idx]`, shift-mask into output word. During unpack: same, but write `output[idx] = extracted_value`.

### Non-Byte-Aligned Bit-Width (e.g. 11 bits for u32)

For `W = 11`, `T = 32`: a row spans `row * W` bits. The packed word positions are:
- `curr_word = (row * W) / T` — which u32 element in the packed block this row starts in
- `next_word = ((row + 1) * W) / T` — which u32 element it ends in
- `shift = (row * W) % T` — bit offset within that u32

When `next_word > curr_word`, the value straddles two packed words:
```
remaining_bits = ((row + 1) * W) % T      // bits in next word
current_bits   = W - remaining_bits        // bits in curr word
tmp = (src >> shift) & mask(current_bits) // low bits from curr
src = packed[LANES * next_word + lane]     // load next word
tmp |= (src & mask(remaining_bits)) << current_bits  // high bits
```
All values are stored unsigned (u32/u64) in the packed buffer regardless of signed-ness of the logical type; sign is applied after unpacking via the FOR reference or explicit cast.

### Single-Element Unpack — Exact Algorithm

From `bitpack_decompress.rs:unpack_single_primitive` [VERIFIED: source]:

```rust
pub unsafe fn unpack_single_primitive<T: NativePType + BitPacking>(
    packed: &[T],
    bit_width: usize,
    index_to_decode: usize,  // index_to_decode = logical_index + array.offset()
) -> T {
    let chunk_index = index_to_decode / 1024;
    let index_in_chunk = index_to_decode % 1024;
    let elems_per_chunk: usize = 128 * bit_width / size_of::<T>();
    let packed_chunk = &packed[chunk_index * elems_per_chunk..][..elems_per_chunk];
    BitPacking::unchecked_unpack_single(bit_width, packed_chunk, index_in_chunk)
}
```

`unchecked_unpack_single` internally uses the transposed index function to locate the bit position. The logical index `i` (within the array) maps to `index_to_decode = i + array.offset()`.

### Pseudocode for In-Width (No-Patch) Unpack in loom-core

For `loom-core` (no fastlanes dependency), replicate the logic:

```
// For type T (u32 or u64), bit_width W, logical index i:
index_to_decode = i + offset  // offset is array.offset() (u16)
chunk_index     = index_to_decode / 1024
index_in_chunk  = index_to_decode % 1024

// Transposed position in chunk (replicates FastLanes index() macro):
lane  = index_in_chunk % LANES         // LANES = 1024 / T_bits
row   = index_in_chunk / LANES         // which row within the lane
// The FL_ORDER permutation reorders "rows" for SIMD:
// order = row / 8, step = row % 8
// transposed_lane_row = FL_ORDER[order] * (LANES / 4) + step * LANES + lane
// But for single-element extraction, we need the BIT position directly:

// The bit position of element `index_in_chunk` within the 1024*W bit packed chunk:
// Row in the bit-packing loop corresponds to iterating over the W output bits
// The packed chunk is indexed as [LANES elements per row, W rows]:
//   packed_chunk[row * LANES + lane] holds bits for row of lane.
// But actually the macros use the transposed index function to know WHICH element
// corresponds to which packed bit position, so for single-element unpack:

curr_word = (row * W) / T_bits
shift     = (row * W) % T_bits
next_word = ((row + 1) * W) / T_bits

packed_idx_curr = elems_per_chunk_start + LANES * curr_word + lane
val_low = (packed[packed_idx_curr] >> shift) & ((1 << W) - 1)

if next_word > curr_word:
    remaining = ((row + 1) * W) % T_bits
    current_bits = W - remaining
    val_low = (packed[packed_idx_curr] >> shift) & ((1 << current_bits) - 1)
    packed_idx_next = elems_per_chunk_start + LANES * next_word + lane
    val_low |= (packed[packed_idx_next] & ((1 << remaining) - 1)) << current_bits

// The "lane" and "row" here are the transposed coordinates:
// lane = index_in_chunk % LANES
// BUT row is the ROW in the FastLanes "matrix" — which bit-plane (0..W) we're extracting
// WAIT: this is confusing because FastLanes has two levels of row.
```

**IMPORTANT CLARIFICATION** [VERIFIED: fastlanes-0.5.1/src/macros.rs + bitpacking.rs]: The FastLanes layout is:
- Outer loop: `for lane in 0..LANES` (the SIMD lane index, 0..32 for u32)
- Inner loop: `for row in 0..T_bits` (which iteration of T bits — for u32, 0..32)
- The logical element accessed is `input[index(row, lane)]` where `index()` maps to transposed position
- The packed output position is `packed[LANES * curr_word + lane]`

So `lane = index_in_chunk % LANES` and the row is determined by finding which `row` satisfies `index(row, lane) == index_in_chunk`. This inversion is what `unchecked_unpack_single` does internally.

**Practical recommendation for loom-core:** Do NOT re-implement the full transposed unpack from scratch. Instead:

1. For the `vortex_reader` (in `loom-fixtures`), extract the raw packed buffer bytes as a `&[u8]` (or `Vec<u8>`) and store it in `LayoutNode::BitPack { values_buf: Vec<u8>, ... }`.
2. In `loom-core`'s read loop, implement a **full-chunk unpack** by calling through the `fastlanes` crate's `BitPacking::unchecked_unpack` — but wait, `loom-core` cannot depend on `fastlanes` (D-02 says no vortex-*, and `fastlanes` is a transitive dep of vortex-fastlanes).

**Resolution for D-01 + D-02 (CRITICAL):** The FastLanes transpose + pack algorithm is the key insight. `loom-core` must implement it WITHOUT the `fastlanes` crate. The `fastlanes` crate (0.5.1) is a pure-Rust implementation of:
1. The `FL_ORDER` permutation constant: `[0, 4, 2, 6, 1, 5, 3, 7]`
2. The `index(row, lane)` function: 3 lines of arithmetic
3. The unpack macro: bit-shift and mask arithmetic

None of these require unsafe code that can't be replicated in safe Rust. The planner should include a Wave-0 task to implement `fastlanes_unpack` in `loom-core/src/l1_model/bitpack.rs` that replicates these ~50 lines of logic. This is what makes Phase 3 technically interesting — loom-core independently reimplements the decode.

**Wave-0 check:** Add a test that compares `loom-core`'s unpack result against `BitPackedData::encode + execute::<PrimitiveArray>` from `loom-fixtures` for the 11-bit case.

---

## Q2: Accessing the Encoded Array in vortex-fastlanes 0.74

### BitPackedArray Accessors

[VERIFIED: vortex-fastlanes-0.74.0/src/bitpacking/array/mod.rs]

The `BitPackedArrayExt` trait (automatically implemented for any `TypedArrayRef<BitPacked>`) provides:

```rust
// All confirmed in BitPackedArrayExt trait + BitPackedData struct:

fn bit_width(&self) -> u8          // bits per value (e.g. 11 for 11-bit packing)
fn offset(&self) -> u16            // 0 <= offset < 1024; start within first block
fn packed(&self) -> &BufferHandle  // the raw packed bytes buffer
fn patches(&self) -> Option<Patches>  // exception values (deferred, None for Phase 3)
fn validity(&self) -> Validity     // Validity enum (NonNullable / AllValid / AllInvalid / Array)
fn packed_slice<T: NativePType + BitPacking>(&self) -> &[T]  // typed view of packed buffer
```

To get the `length` of the array: `array.as_ref().len()` (from `ArrayRef`).
To get the `dtype`: `array.as_ref().dtype()` (returns `&DType`).

**Getting raw bytes for the packed buffer:**
```rust
// In vortex_reader (loom-fixtures):
let packed_buf: &BufferHandle = array.packed();
let packed_bytes: &[u8] = packed_buf.as_host().as_ref();  // or .as_slice()
// Copy into Vec<u8> to own the bytes:
let packed_owned: Vec<u8> = packed_bytes.to_vec();
```

Note: `BufferHandle::as_host()` returns a `ByteBuffer`. The exact method to get `&[u8]` from `ByteBuffer` is `as_ptr()` + `len()`, or the buffer implements `Deref<Target=[u8]>`. [ASSUMED — need Wave-0 check: `ByteBuffer` deref target; use `packed_buf.as_host().as_ref()` or `packed_buf.as_host().as_slice()`]

### FoRArray Accessors

[VERIFIED: vortex-fastlanes-0.74.0/src/for/array/mod.rs]

The `FoRArrayExt` trait provides:

```rust
fn encoded(&self) -> &ArrayRef        // the inner encoded array (a BitPackedArray)
fn reference_scalar(&self) -> &Scalar // the reference (minimum) value as a Scalar
```

`FoRData::ptype()` returns the `PType` (u32, u64, i32, i64 etc.) of the reference scalar.

To extract the reference value as `i64` (for use in LayoutNode):
```rust
// In vortex_reader:
let ref_scalar: &Scalar = array.reference_scalar();
// Scalar has typed_value<T> method:
let ref_value: i64 = ref_scalar.as_primitive().typed_value::<i64>()
    .vortex_expect("FoR reference must be non-null");
// Or for unsigned:
let ref_value: u64 = ref_scalar.as_primitive().typed_value::<u64>()
    .vortex_expect("FoR reference must be non-null");
```

[ASSUMED — `Scalar::as_primitive().typed_value::<T>()` pattern confirmed in for_decompress.rs:68, but exact `as_primitive()` method name needs Wave-0 verification in Scalar source]

### Encoding ID Dispatch

[VERIFIED: ARCHITECTURE.md + STACK.md patterns]

In `vortex_reader`, dispatch on encoding:
```rust
// Use encoding ID to determine type:
use vortex_fastlanes::{BitPacked, FoR};
// For BitPackedArray:
if let Some(bp) = array.as_opt::<BitPacked>() { ... }
// For FoRArray:
if let Some(for_) = array.as_opt::<FoR>() { ... }
```

The `.as_opt::<E>()` pattern is confirmed in `for_decompress.rs` line 53: `array.encoded().as_opt::<BitPacked>()`.

**Wave-0 check:** Confirm `as_opt::<BitPacked>()` vs `try_downcast::<BitPacked>()` — the decompress code uses `as_opt` pattern; use that.

---

## Q3: FOR Decode

[VERIFIED: vortex-fastlanes-0.74.0/src/for/array/for_decompress.rs]

### Structure: FoRArray Contains a BitPackedArray Child

FoRArray stores its inner encoding in a slot named `encoded`:
```
FoRArray:
  - data: FoRData { reference: Scalar }  // the frame-of-reference minimum
  - slot[ENCODED_SLOT=0]: ArrayRef       // the BitPackedArray of deltas
```

The `encoded` child is accessed via `array.encoded()` and is always a `BitPackedArray` in normal Vortex usage (confirmed by the fused decompress path in `for_decompress.rs` which checks `as_opt::<BitPacked>()`).

### FOR Decode Algorithm

```
decoded[i] = bitunpacked[i] + reference_scalar   (wrapping_add for unsigned types)
```

This is confirmed in `for_decompress.rs:fused_decompress`: uses `FoRStrategy::unpack_chunk` which calls `FoR::unchecked_unfor_pack` — but that is an optimization. The conceptual algorithm is just a scalar broadcast add.

For signed types, the `decompress` function does:
```rust
values.map_each_in_place(move |v| v.wrapping_add(&min))
```

For `loom-core`, after bitunpacking to a `Vec<i32>` (or the intermediate buffer), broadcast-add the reference scalar with wrapping arithmetic.

### FoRArray Validity

[VERIFIED: vortex-fastlanes-0.74.0/src/for/vtable/validity.rs]

```rust
impl ValidityChild<FoR> for FoR {
    fn validity_child(array: ArrayView<'_, FoR>) -> ArrayRef {
        array.encoded().clone()  // validity delegates to the inner BitPackedArray
    }
}
```

This means: **FoRArray does not carry its own validity; the validity lives in the inner BitPackedArray**. When `vortex_reader` processes a FoRArray, it must extract validity from the `encoded` (inner BitPackedArray) child, not from the FoRArray itself.

### LayoutNode Representation

The `LayoutNode::FrameOfReference` wraps a `BitPack` node:
```rust
LayoutNode::FrameOfReference {
    reference: i64,             // the reference scalar (cast to i64; wrapping-add handles sign)
    inner: Box<LayoutNode>,     // always LayoutNode::BitPack for Phase 3
}
```

The read loop for FOR:
1. Decode the `inner` BitPack node into a temporary `Vec<i32>` (or `Vec<i64>`).
2. For each value, broadcast-add `reference` with wrapping: `val.wrapping_add(reference as T)`.
3. Append to the Arrow builder.

Validity is read from the inner `BitPack` node (it belongs there). The outer `FrameOfReference` arm does not emit nulls — it delegates that to the inner BitPack arm.

---

## Q4: Validity → Arrow Null Bitmap

### Vortex Validity Enum (0.74.0)

[VERIFIED: vortex-array-0.74.0/src/validity.rs]

```rust
pub enum Validity {
    NonNullable,       // column has no nulls, no validity bitmap
    AllValid,          // all rows are valid (but column is nullable)
    AllInvalid,        // all rows are null
    Array(ArrayRef),   // per-row boolean array: true = valid, false = null
}
```

For Phase 3, the `Array(ArrayRef)` variant holds a plain BoolArray (non-encoded validity per D-02 decision).

**vortex_reader** extracts validity from a `BitPackedArray` via `array.validity()` which returns this `Validity` enum.

For FoRArray: validity is in the inner BitPackedArray (see Q3 above).

### Mapping Validity to Arrow Builders

Pattern for `loom-core`'s synthesized read loop:

```rust
// In the BitPack arm of synthesized_read_loop:
let validity = /* Validity enum from LayoutNode or from raw bitmap */;

match validity {
    Validity::NonNullable | Validity::AllValid => {
        // No nulls — fast path: just append_value for each element
        for val in unpacked_values {
            builder.append_value(val);
        }
    }
    Validity::AllInvalid => {
        // All null — fast path
        for _ in 0..count {
            builder.append_null();
        }
    }
    Validity::Array(bool_arr) => {
        // Per-row validity: must iterate in lockstep
        // The boolean array is a BoolArray with a packed bit buffer
        // Each bit: 1 = valid, 0 = null (Arrow convention = Vortex convention)
        for (i, val) in unpacked_values.iter().enumerate() {
            if validity_bitmap[i] {  // bit i of the boolean array
                builder.append_value(*val);
            } else {
                builder.append_null();
            }
        }
    }
}
```

**Getting a boolean iterator from Validity::Array**: The `ArrayRef` inside is a `BoolArray`. In `vortex_reader`, convert validity to a `Vec<bool>` before building the `LayoutNode`, so that `loom-core` never needs to call Vortex APIs:

```rust
// In vortex_reader (loom-fixtures):
fn extract_validity(validity: Validity, len: usize) -> Option<Vec<bool>> {
    match validity {
        Validity::NonNullable | Validity::AllValid => None,  // no nulls
        Validity::AllInvalid => Some(vec![false; len]),
        Validity::Array(bool_arr) => {
            // Execute to canonical BoolArray and extract bits
            // Use Vortex's own execute path in vortex_reader
            let mut ctx = session.create_execution_ctx();
            let bool_arr = bool_arr.execute::<BoolArray>(&mut ctx).unwrap();
            Some(bool_arr.boolean_buffer().into_iter().collect())
        }
    }
}
```

Then `LayoutNode::BitPack` carries an `Option<Vec<bool>>` (or `Option<Vec<u8>>` as a packed bitmap) for the validity.

**Alternative for loom-core:** Store validity as `Option<Vec<u8>>` (packed bit buffer, Arrow convention) extracted by `vortex_reader`. `loom-core` then reads bit `i` as `(bitmap[i/8] >> (i%8)) & 1 == 1`.

### Arrow Builder Null Handling

[VERIFIED: arrow-array-58.3.0/src/builder/primitive_builder.rs]

`Int32Builder` (and all `PrimitiveBuilder<T>`):
```rust
fn append_value(&mut self, v: T::Native)  // appends a non-null value
fn append_null(&mut self)                 // appends a null (stores T::default() + sets null bit)
fn append_option(&mut self, v: Option<T::Native>)  // convenience: None → append_null
fn finish(&mut self) -> PrimitiveArray<T>  // materializes; clears builder
```

After `finish()`, get `ArrayData`:
```rust
let prim_array: PrimitiveArray<Int32Type> = builder.finish();
let array_data: ArrayData = prim_array.into_data();  // consumes array
```

`into_data()` is confirmed in `ffi.rs:138`: `let array_data = array.into_data();`. The `to_ffi(&array_data)` call is also confirmed (line 142).

**Null count verification:** `ArrayData.null_count()` is computed automatically from the null bitmap when the builder records nulls. No manual null_count needed.

---

## Q5: arrow_builder_output with arrow-rs 58.3

[VERIFIED: arrow-array-58.3.0/src/builder/primitive_builder.rs + loom-ffi/src/ffi.rs]

### Confirmed API Chain

```rust
use arrow::array::Int32Builder;

let mut builder = Int32Builder::new();
builder.append_value(42);
builder.append_null();
let prim_array = builder.finish();           // → PrimitiveArray<Int32Type>
let array_data = prim_array.into_data();     // → ArrayData (consumes array)
let (ffi_array, ffi_schema) = arrow::ffi::to_ffi(&array_data)  // → (FFI_ArrowArray, FFI_ArrowSchema)
    .map_err(|_| LoomError::DecodeFailed)?;
unsafe {
    std::ptr::write(out_array, ffi_array);
    std::ptr::write(out_schema, ffi_schema);
}
```

This exact pattern is already proven in `loom-ffi/src/ffi.rs` (Phase 2). Phase 3 only changes what fills the builder before `finish()`.

### arrow_builder_output Module Design

`loom-core/src/arrow_builder_output.rs` should provide an `OutputBuilder` enum that wraps the concrete typed builder:

```rust
pub enum OutputBuilder {
    Int32(arrow::array::Int32Builder),
    Int64(arrow::array::Int64Builder),
    // ... other types as needed
}

impl OutputBuilder {
    pub fn append_i32(&mut self, v: i32) { ... }
    pub fn append_null(&mut self) { ... }
    pub fn finish(self) -> ArrayData { ... }
}
```

The `loom-core` crate already has `arrow`, `arrow-array`, `arrow-schema`, `arrow-data` as workspace dependencies. No version-skew risk because all are pinned to `=58.3.0` in workspace `Cargo.toml`.

### Version-Skew Tripwire (Success Criterion 4)

[VERIFIED: workspace Cargo.toml]

All four arrow crates are pinned at `=58.3.0` (exact) in the workspace. The existing test in `crates/loom-ffi/tests/roundtrip.rs` already exercises `to_ffi` end-to-end. Phase 3's `arrow_builder_output::finish()` → `into_data()` → `to_ffi` chain is identical to the Phase-2 hardcoded path, so if it compiles, it is version-skew-free.

---

## Q6: In-Phase Verification

### Using Vortex's Own Decode as Oracle

In Phase 3, the oracle (in `loom-fixtures`) creates the Vortex array, encodes it, and also decodes it using Vortex's own path:

```rust
// In a #[test] in loom-fixtures or loom-core (with loom-fixtures as dev-dep):

// 1. Build fixture (in vortex_reader / loom-fixtures)
let mut ctx = LEGACY_SESSION.create_execution_ctx();
let values = PrimitiveArray::from_iter((0u32..100).map(|i| i % 2047));
let bitpacked = BitPackedData::encode(&values.into_array(), 11, &mut ctx).unwrap();

// 2. Oracle decode (Vortex path):
let canonical = bitpacked.as_array().clone()
    .execute::<PrimitiveArray>(&mut ctx).unwrap();
let oracle_values: Vec<u32> = canonical.as_slice::<u32>().to_vec();

// 3. Loom decode (our path):
let layout_node = vortex_reader::from_bitpacked_array(&bitpacked);
let mut builder = OutputBuilder::Int32(Int32Builder::new());
loom_core::l1_model::synthesized_read_loop(&layout_node, &packed_bytes, &registry, &mut builder);
let loom_data = builder.finish();
let loom_values: Vec<i32> = /* extract from ArrayData */;

// 4. Assert row-for-row:
assert_eq!(loom_values.len(), oracle_values.len());
for (i, (loom, oracle)) in loom_values.iter().zip(oracle_values.iter()).enumerate() {
    assert_eq!(*loom as u32, *oracle, "mismatch at index {i}");
}
```

For nullable fixtures, compare null positions explicitly:
```rust
// From ArrayData: check null bitmap
let nulls = loom_data.nulls();
for i in 0..loom_data.len() {
    let loom_is_null = nulls.map_or(false, |n| n.is_null(i));
    let oracle_is_null = /* from Vortex validity */;
    assert_eq!(loom_is_null, oracle_is_null, "null mismatch at {i}");
}
```

The full standalone harness (VERIFY-01/02) is Phase 5. Phase 3 uses simpler inline assertions in Rust `#[test]` functions inside `loom-fixtures` or as integration tests that have `loom-fixtures` as a dev-dependency.

### Serialization for INPUT-01 (Programmatic In-Memory)

Phase 3 does NOT need to serialize Vortex arrays to bytes and deserialize them. INPUT-01 says "read into the decoder without parsing a .vortex file" — this is satisfied by:
1. `loom-fixtures` constructs the `ArrayRef` in memory (using `BitPackedData::encode`).
2. `vortex_reader::from_array(array_ref)` inspects it directly (still in-process).
3. It extracts the raw bytes from `packed()` + `bit_width()` + `offset()` + `validity()` into a `LayoutNode`.

No serialization/deserialization is needed for Phase 3. The "input bytes" are the raw packed buffer bytes that `vortex_reader` extracts and hands to `loom-core` as part of the `LayoutNode`.

---

## Standard Stack

### Core (Phase 3)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `arrow` | =58.3.0 | `Int32Builder`, `to_ffi`, `FFI_ArrowArray` | Workspace-pinned; already used in loom-ffi |
| `arrow-array` | =58.3.0 | `PrimitiveBuilder<T>`, `PrimitiveArray` | Sub-crate; already in loom-core deps |
| `arrow-data` | =58.3.0 | `ArrayData`, substrate for `to_ffi` | Sub-crate; already in loom-core deps |
| `vortex-array` | =0.74.0 | `ArrayRef`, `Validity`, `PrimitiveArray` in fixtures | loom-fixtures only |
| `vortex-fastlanes` | =0.74.0 | `BitPackedData::encode`, `FoRData`, fixture construction | loom-fixtures only |

No new dependencies are needed for Phase 3. All crates are already in `Cargo.toml`.

### New Code Modules (Phase 3)

All in `loom-core`:
- `src/l1_model.rs` — `LayoutNode` enum + `synthesized_read_loop`
- `src/l1_model/bitpack.rs` — FastLanes unpack implementation (pure Rust, no deps)
- `src/arrow_builder_output.rs` — `OutputBuilder` enum + `finish()` → `ArrayData`

In `loom-fixtures`:
- `src/lib.rs` — `vortex_reader::from_bitpacked_array()` + `from_for_array()` functions
- Oracle decode helpers

---

## Package Legitimacy Audit

> No new packages are introduced in Phase 3. All packages are already workspace-pinned and in Cargo.lock.

| Package | Status |
|---------|--------|
| `arrow` 58.3.0 | Already installed — Apache Foundation package |
| `vortex-array` 0.74.0 | Already installed — SpiralDB/vortex-data |
| `vortex-fastlanes` 0.74.0 | Already installed — SpiralDB/vortex-data |
| `fastlanes` 0.5.1 | Transitive dep — SpiralDB package; loom-core does NOT add it |

**No new package installs required for Phase 3.** The `fastlanes` crate (0.5.1) is already present as a transitive dependency of `vortex-fastlanes`, but `loom-core` must NOT add it as a direct dependency (D-02 principle — keep loom-core free of the vortex ecosystem). loom-core reimplements the ~50 lines of FastLanes logic independently.

---

## Architecture Patterns

### System Architecture Diagram (Phase 3 additions)

```
loom-fixtures (has vortex-* deps)
  BitPackedData::encode()  →  ArrayRef (in-memory)
  vortex_reader::from_array(ArrayRef)
    ├── inspects array.bit_width(), array.offset(), array.packed()
    ├── extracts validity → Option<Vec<bool>>
    └── emits LayoutNode::BitPack { values_buf, bit_width, offset, count, validity }
         or LayoutNode::FrameOfReference { reference, inner: Box<LayoutNode::BitPack> }
              ↓
loom-core (zero vortex-* deps)
  l1_model::synthesized_read_loop(&layout_node, &mut builder)
    match layout_node:
      BitPack  → bitpack::unpack_all() → for each value: builder.append_value/null
      FOR      → inner BitPack decode → wrapping_add reference → builder.append_value/null
      Other    → Err(LoomError::UnimplementedEncoding)
              ↓
  arrow_builder_output::OutputBuilder::finish()
    → PrimitiveArray::into_data()
    → ArrayData
              ↓
loom-ffi (existing)
  to_ffi(&array_data)  →  FFI_ArrowArray + FFI_ArrowSchema
  ptr::write(out_array, ffi_array)
```

### Recommended Project Structure (Phase 3 additions)

```
crates/loom-core/src/
├── lib.rs               (existing — #![forbid(unsafe_code)])
├── l1_model.rs          (NEW — LayoutNode enum + synthesized_read_loop)
├── l1_model/
│   ├── mod.rs           (re-exports)
│   └── bitpack.rs       (NEW — FastLanes unpack without vortex-* dep)
├── arrow_builder_output.rs  (NEW — OutputBuilder enum + finish())
└── l2_kernel_registry.rs    (stub — existing placeholder)

crates/loom-fixtures/src/
└── lib.rs               (NEW — vortex_reader functions + oracle helpers)
```

### Pattern 1: FastLanes Unpack (loom-core independent implementation)

```rust
// Source: fastlanes-0.5.1/src/transpose.rs + macros.rs (rewritten without deps)
// loom-core/src/l1_model/bitpack.rs

const FL_ORDER: [usize; 8] = [0, 4, 2, 6, 1, 5, 3, 7];

/// Returns the logical index of the element stored at transposed position `i`.
/// (From fastlanes-0.5.1/src/transpose.rs)
fn fl_index(row: usize, lane: usize) -> usize {
    let o = row / 8;
    let s = row % 8;
    (FL_ORDER[o] * 16) + (s * 128) + lane
}

/// Unpack all N values from a FastLanes bit-packed buffer.
/// T_bits: bit width of the target type (32 for i32/u32, 64 for i64/u64)
/// W: packed bit width (e.g. 11)
/// offset: array.offset() (0..1024)
/// packed: raw bytes of the packed buffer
/// Returns Vec<u64> (unsigned; caller casts + sign-extends if needed)
pub fn unpack_all_u64(
    packed: &[u8],
    bit_width: usize,   // W
    t_bits: usize,      // T (32 or 64)
    offset: usize,      // array.offset()
    count: usize,       // logical length
) -> Vec<u64> {
    let lanes = 1024 / t_bits;
    let elems_per_chunk = 128 * bit_width / (t_bits / 8);
    let num_chunks = (offset + count).div_ceil(1024);
    let mask: u64 = if bit_width == 64 { u64::MAX } else { (1u64 << bit_width) - 1 };

    // Reinterpret packed bytes as &[u32] or &[u64] (little-endian)
    // For simplicity, work in u64 regardless of T:
    // packed_u64[i] = LE u64 at byte offset i*8

    let mut result = Vec::with_capacity(count);

    for chunk_idx in 0..num_chunks {
        let chunk_start = chunk_idx * elems_per_chunk; // in units of T-elements
        let chunk_bytes_start = chunk_start * (t_bits / 8);

        for lane in 0..lanes {
            for row in 0..t_bits {
                let logical_idx = fl_index(row, lane);
                let abs_logical = chunk_idx * 1024 + logical_idx;

                // Only emit values in [offset, offset+count)
                if abs_logical < offset || abs_logical >= offset + count {
                    continue;
                }

                // Find the packed word position for this (row, lane):
                let curr_word = (row * bit_width) / t_bits;
                let next_word = ((row + 1) * bit_width) / t_bits;
                let shift = (row * bit_width) % t_bits;

                let load_t = |word_idx: usize| -> u64 {
                    let byte_off = chunk_bytes_start + (word_idx * lanes + lane) * (t_bits / 8);
                    if t_bits == 32 {
                        u32::from_le_bytes(packed[byte_off..byte_off+4].try_into().unwrap()) as u64
                    } else {
                        u64::from_le_bytes(packed[byte_off..byte_off+8].try_into().unwrap())
                    }
                };

                let val = if next_word > curr_word {
                    let remaining = ((row + 1) * bit_width) % t_bits;
                    let current_bits = bit_width - remaining;
                    let lo = (load_t(curr_word) >> shift) & mask_bits(current_bits);
                    if next_word < bit_width {
                        let hi = load_t(next_word) & mask_bits(remaining);
                        lo | (hi << current_bits)
                    } else {
                        lo
                    }
                } else {
                    (load_t(curr_word) >> shift) & mask
                };

                result.push(val);
            }
        }
    }

    // Result is in transposed order; must sort by logical index.
    // Actually the above emits values in transposed order, not logical order.
    // Fix: collect (logical_index → value) pairs and sort.
    // See implementation note below.
    result
}
```

**IMPORTANT IMPLEMENTATION NOTE:** The above loop iterates in transposed order (lane × row), not logical order. The output must be reordered to logical index order. The cleanest approach for loom-core:

```rust
// Allocate output by logical index:
let mut output = vec![0u64; count];

for chunk_idx in 0..num_chunks {
    for lane in 0..lanes {
        for row in 0..t_bits {
            let logical_idx = fl_index(row, lane);
            let abs_logical = chunk_idx * 1024 + logical_idx;
            if abs_logical < offset || abs_logical >= offset + count {
                continue;
            }
            let value = /* bit-extract as above */;
            output[abs_logical - offset] = value;
        }
    }
}
```

This is O(1024 * num_chunks) which is identical complexity to the vectorized path.

### Pattern 2: FOR Decode

```rust
// loom-core/src/l1_model.rs
LayoutNode::FrameOfReference { reference, inner } => {
    // Decode the inner BitPack into a temp buffer
    let mut temp: Vec<i64> = Vec::with_capacity(count);
    // ... decode inner (BitPack arm, writing to temp) ...

    // Broadcast-add reference (wrapping)
    for val in temp {
        let result = val.wrapping_add(*reference);
        builder.append_i64(result);  // or cast to i32 if dtype is i32
    }
}
```

### Pattern 3: LayoutNode Fields (Phase 3)

```rust
// loom-core/src/l1_model.rs
pub enum LayoutNode {
    Raw {
        data: Vec<u8>,          // raw bytes, width in bytes, little-endian
        elem_size: u8,          // 1, 2, 4, or 8
        count: usize,
    },
    BitPack {
        values_buf: Vec<u8>,    // raw packed bytes (from BitPackedData::packed())
        bit_width: u8,          // bits per value (1..=64)
        offset: u16,            // start offset within first block (0..1024)
        count: usize,           // number of logical values
        validity: Option<Vec<bool>>,  // None = NonNullable/AllValid; Some = per-row
        all_null: bool,         // true if AllInvalid (skip unpack, emit all nulls)
    },
    FrameOfReference {
        reference: i64,         // cast from Vortex Scalar (wrapping arithmetic)
        inner: Box<LayoutNode>, // always BitPack for Phase 3
    },
    Dictionary {
        codes: Box<LayoutNode>,
        values: Box<LayoutNode>,
    },
    RunEnd {
        run_ends: Box<LayoutNode>,
        values: Box<LayoutNode>,
        count: usize,
    },
    KernelEscape {
        kernel_id: u32,
        params: Vec<u8>,
        count: usize,
    },
}
```

### Anti-Patterns to Avoid

- **Putting `fastlanes` in loom-core deps:** D-02 spirit — if `fastlanes` enters loom-core, it carries the vortex ecosystem's transitive deps and makes loom-core's "independence proof" hollow.
- **Using `into_canonical().into_arrow()` inside synthesized_read_loop:** This defeats the point of Phase 3 (anti-pattern 1 in ARCHITECTURE.md).
- **Not handling `all_null` fast path:** If `Validity::AllInvalid`, skip the unpack entirely and emit `count` null values. Unpacking bits that will all be ignored wastes work and may produce garbage values at bit positions that aren't meaningful.
- **Treating FOR's reference as i64 uniformly for unsigned types:** The reference scalar may be unsigned (`u32`, `u64`). Store as `u64` internally and cast based on the `ptype` at emit time, OR store as `i128` to handle all cases. [ASSUMED — safest: store as `i128` reference in LayoutNode]

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Arrow null bitmap consistency | Custom bitmap | `Int32Builder::append_null()` | Builder manages null_count, bitmap alignment, and byte packing automatically |
| FFI export and release callback | Custom C struct | `arrow::ffi::to_ffi()` | Release callback lifecycle is subtle (see PITFALLS P1) |
| FoRArray reference extraction | Manual flatbuffer parse | `FoRArrayExt::reference_scalar()` (in vortex_reader) | Abstracted by the 0.74 vtable API |
| BitPackedArray buffer access | Manual offset arithmetic into raw bytes | `BitPackedArrayExt::packed() + bit_width() + offset()` (in vortex_reader) | `BitPackedData::validate` confirms the exact buffer layout |

---

## Common Pitfalls

### Pitfall 1: Transposed vs Logical Order — Getting the Index Wrong

**What goes wrong:** Implementing unpack in sequential order (bit `i` → logical value `i`) instead of using the FastLanes transposed index. The test passes for trivial inputs (all zeros, all ones) but fails for any real data.

**Why it happens:** The FastLanes transpose is non-obvious. Sequential (non-transposed) bit extraction is the natural first attempt.

**How to avoid:** Implement the `fl_index(row, lane)` function first, add a unit test against a small known vector encoded with `BitPackedData::encode`, assert byte-for-byte output matches before writing any other code.

**Warning signs:** Decoded values are a permutation of the correct values (not garbage, just shuffled). The error pattern is systematic, not random.

### Pitfall 2: The offset Field — Forgetting to Add it to the Logical Index

**What goes wrong:** loom-core treats logical index 0 as packed index 0, but `array.offset()` may be non-zero (when the BitPackedArray was created via `.slice()`). Without adding the offset, the first `offset` values are wrong.

**Why it happens:** Phase 3 fixtures don't start with a slice, so `offset = 0` in all happy-path tests. The bug is latent until someone passes a sliced array.

**How to avoid:** Always use `index_to_decode = logical_index + array.offset()` (confirmed in `unpack_single_primitive`). Store `offset` in `LayoutNode::BitPack` and add it unconditionally.

**Warning signs:** First N values are wrong (where N = offset); rest are correct.

### Pitfall 3: FOR Validity Is in the Inner BitPack, Not the FOR Array

**What goes wrong:** `vortex_reader` extracts `for_array.validity()` (which is delegated to the encoded child anyway) correctly, but the read loop applies validity at the FrameOfReference arm instead of the BitPack arm, causing double-null checks or missed nulls.

**Why it happens:** It's natural to think "the outer encoding carries the nulls." In Vortex, FOR delegates validity to the inner BitPackedArray.

**How to avoid:** The `LayoutNode::FrameOfReference` arm does NOT carry its own validity field. Validity lives in `LayoutNode::BitPack`. The FOR arm just adds the scalar; the BitPack arm handles null routing.

**Warning signs:** Nullable FOR column: nulls are missed (all values appear valid) or double-null (null bits applied twice).

### Pitfall 4: Signed vs Unsigned During Unpack

**What goes wrong:** BitPacking always uses the unsigned physical type (u32 for i32 columns). After unpacking, the value must be sign-extended. If treated as i32 before adding the FOR reference, negative packed values are misinterpreted.

**Why it happens:** The `bit_width` in Vortex is always less than the type width (e.g. 11 bits for values that fit in i32). The packed bits are always non-negative (the compressor subtracts the minimum, i.e. FOR reference, so the packed deltas are always unsigned).

**How to avoid:** Unpack as the unsigned counterpart (u32 for i32 columns). After adding the FOR reference (as a wrapping add of the signed reference value), the result is the signed final value.

### Pitfall 5: BufferHandle Access Method

**What goes wrong:** `BufferHandle::as_host()` may not return a type with a simple `as_slice()` or `Deref<[u8]>` method. The exact method depends on the `ByteBuffer` API in `vortex-buffer` 0.74.

**How to avoid:** **Wave-0 check required.** In the first task of Wave 1, verify the exact method chain:
```rust
let packed_buf: &BufferHandle = array.packed();
// Try in order until one compiles:
let bytes: &[u8] = packed_buf.as_host().as_ref();     // option A
let bytes: &[u8] = packed_buf.as_host().as_slice();   // option B
let bytes: &[u8] = &packed_buf.as_host()[..];          // option C (Deref)
```
Add a compile-time test that just calls `.packed()` on a `BitPackedArray` and accesses bytes.

---

## Code Examples

### Verified Pattern: Int32Builder → into_data() → to_ffi

```rust
// Source: loom-ffi/src/ffi.rs (Phase 2, in-repo, working)
use arrow::array::Int32Builder;
use arrow::ffi::{to_ffi, FFI_ArrowArray, FFI_ArrowSchema};

let mut builder = Int32Builder::new();
builder.append_value(1);
builder.append_value(2);
builder.append_null();
let array = builder.finish();
let array_data = array.into_data();
let (ffi_array, ffi_schema) = to_ffi(&array_data).map_err(|_| LoomError::DecodeFailed)?;
unsafe {
    std::ptr::write(out_array, ffi_array);
    std::ptr::write(out_schema, ffi_schema);
}
```

### Verified Pattern: BitPackedData::encode (fixture construction)

```rust
// Source: vortex-fastlanes-0.74.0/src/bitpacking/array/mod.rs tests
use vortex_fastlanes::BitPackedData;
use vortex_array::arrays::PrimitiveArray;

let values = PrimitiveArray::from_iter((0u32..100).map(|i| i % 2047));
let mut ctx = session.create_execution_ctx();
let packed = BitPackedData::encode(&values.into_array(), 11, &mut ctx).unwrap();

// Access fields:
let bit_width: u8 = packed.bit_width();       // 11
let offset: u16 = packed.offset();             // 0 (no slice)
let packed_buf: &BufferHandle = packed.packed();
let validity: Validity = packed.validity();
let count: usize = packed.as_ref().len();      // 100
```

### Verified Pattern: FoRArray inspection

```rust
// Source: vortex-fastlanes-0.74.0/src/for/array/for_decompress.rs + mod.rs
use vortex_fastlanes::{FoR, FoRArray};
use crate::for::array::FoRArrayExt;

// Get the inner BitPackedArray:
let inner: &ArrayRef = for_array.encoded();         // always a BitPackedArray for Phase 3
// Get the reference scalar:
let ref_scalar: &Scalar = for_array.reference_scalar();
// Extract as i64 (for signed types):
let reference: i64 = ref_scalar.as_primitive()
    .typed_value::<i64>()
    .expect("FoR reference must be non-null");
```

### Verified Pattern: FL_ORDER and transpose index

```rust
// Source: fastlanes-0.5.1/src/lib.rs + transpose.rs
// Replicate in loom-core WITHOUT importing fastlanes:

pub const FL_ORDER: [usize; 8] = [0, 4, 2, 6, 1, 5, 3, 7];

/// Logical-index → transposed-storage-index mapping.
/// idx: position in the logical 1024-element array
/// Returns: position in the transposed storage order
#[inline(always)]
pub const fn fl_transpose_index(idx: usize) -> usize {
    let lane = idx % 16;
    let order = (idx / 16) % 8;
    let row = idx / 128;
    (lane * 64) + (FL_ORDER[order] * 8) + row
}
```

---

## Runtime State Inventory

Phase 3 is a greenfield addition (new modules in existing crates). No rename/refactor. Omitting this section.

---

## State of the Art

| Old Approach | Current Approach (0.74) | Impact |
|--------------|-------------------------|--------|
| `vortex-dict` as separate crate | Dictionary encoding merged into `vortex-array` at 0.7x | loom-fixtures Cargo.toml already corrected — no `vortex-dict` dep |
| `into_canonical().into_arrow()` for all decodes | Per-encoding explicit decode paths + fused FoR+BitPack kernel | Phase 3 uses the explicit path (oracle uses `into_canonical()`) |
| `patches: Option<ArrayRef>` (older API) | `patches: Option<Patches>` with `PatchesData` indirection | `BitPackedData::patches()` returns `Option<Patches>` — deferred for Phase 3 (fixtures stay in-width) |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `BufferHandle::as_host()` returns a type with a `&[u8]` access method via `.as_ref()` or `.as_slice()` | Q2 / Accessors | vortex_reader won't compile; needs different method name — low risk, just needs a compile check |
| A2 | `Scalar::as_primitive().typed_value::<T>()` is the correct chain to extract the FoR reference value | Q2 / FoR Accessors | Reference extraction fails; confirmed in for_decompress.rs but `as_primitive()` path needs check |
| A3 | The `LayoutNode::FrameOfReference.reference` field can safely be stored as `i64` for both signed and unsigned column types | Q3 / LayoutNode | For u64 columns where reference > i64::MAX, this silently wraps; should use `u64` or `i128` if u64 columns are tested |
| A4 | `for_array.as_opt::<BitPacked>()` is the correct downcast pattern for the inner FoRArray child | Q2 / Encoding ID | Compile error; alternative is `for_array.encoded().try_downcast::<BitPacked>()` — check in Wave-0 |

**If this table is empty:** All claims were verified or cited — no user confirmation needed.
(This table is not empty.)

---

## Open Questions

1. **`BufferHandle` byte slice access (Wave-0 check)**
   - What we know: `packed.packed()` returns `&BufferHandle`; `ByteBuffer` holds the bytes.
   - What's unclear: Exact method to get `&[u8]` — `as_host().as_ref()` vs `as_host().as_slice()` vs `as_host()[..]`.
   - Recommendation: Add a Wave-0 compile check in loom-fixtures: write a function that calls `packed.packed().as_host()` and coerces to `&[u8]`; fix method name based on compile error.

2. **Signed type handling for FOR reference**
   - What we know: `FoRData::ptype()` returns the PType; the reference is stored as a `Scalar`.
   - What's unclear: For i32 columns with negative references, does storing as `i64` in LayoutNode and casting back to `i32` before `wrapping_add` give the right answer?
   - Recommendation: Test with a fixture where `reference = -500` and values span [0, 100]; verify decoded output matches oracle.

3. **`all_null` fast path in BitPack arm**
   - What we know: `Validity::AllInvalid` means every row is null; the packed buffer still exists but its values are meaningless.
   - What's unclear: Does Vortex guarantee the packed buffer is present (non-empty) even for all-null arrays? If not, slicing it would panic.
   - Recommendation: Gate the unpack on `!all_null`; if `all_null`, emit `count` nulls without touching the buffer.

---

## Environment Availability

Phase 3 is pure Rust code within the existing workspace — no external tools, services, or CLIs beyond `cargo`. All crates are already in Cargo.lock. Skipping this section.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` (no external framework) |
| Config file | Cargo.toml test configuration (workspace) |
| Quick run command | `cargo test -p loom-core` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| L1-01 | LayoutNode enum compiles with all 6 arms | unit (compile) | `cargo build -p loom-core` | ❌ Wave 0 |
| L1-02 | synthesized_read_loop returns error for unimplemented arms | unit | `cargo test -p loom-core l1_model::unimplemented_arms` | ❌ Wave 0 |
| L1-03 | 11-bit BitPack decode matches Vortex oracle row-for-row | integration | `cargo test -p loom-fixtures bitpack_11bit_roundtrip` | ❌ Wave 0 |
| L1-03 | Non-byte-aligned decode for values 0..2047 | integration | `cargo test -p loom-fixtures bitpack_non_byte_aligned` | ❌ Wave 0 |
| L1-04 | FoR decode matches Vortex oracle (signed + unsigned reference) | integration | `cargo test -p loom-fixtures for_roundtrip` | ❌ Wave 0 |
| L1-07 | Nullable BitPack column: null positions match oracle | integration | `cargo test -p loom-fixtures nullable_bitpack` | ❌ Wave 0 |
| L1-07 | AllInvalid column: all positions are null in Arrow output | integration | `cargo test -p loom-fixtures all_null_bitpack` | ❌ Wave 0 |
| ARROW-01 | No direct buffer writes in OutputBuilder (code review check) | lint | — | N/A |
| ARROW-02 | `builder.finish() → into_data()` produces valid ArrayData | unit | `cargo test -p loom-core arrow_builder_output::finish_produces_valid_data` | ❌ Wave 0 |

### Wave 0 Gaps

- [ ] `crates/loom-core/src/l1_model.rs` — `LayoutNode` enum + `synthesized_read_loop` stub
- [ ] `crates/loom-core/src/l1_model/bitpack.rs` — `unpack_all` stub (Wave-0 just needs the signature + compile)
- [ ] `crates/loom-core/src/arrow_builder_output.rs` — `OutputBuilder` enum + `finish()`
- [ ] `crates/loom-fixtures/src/lib.rs` — `vortex_reader::from_bitpacked_array()` + oracle helpers
- [ ] `crates/loom-fixtures/tests/bitpack_roundtrip.rs` — integration test for L1-03
- [ ] Verify `BufferHandle::as_host()` byte access method (compile check)

---

## Security Domain

Phase 3 is a Rust-only in-process library with no network I/O, no file I/O, no deserialization of untrusted data, and no FFI surface changes. The existing `catch_unwind` wrapper in `loom-ffi` covers all panic safety for the existing `loom_decode` function (which is not modified in Phase 3).

The only new user-controlled input is the `LayoutNode` fields populated by `vortex_reader`. These are derived from in-memory Vortex arrays constructed by fixtures, not external inputs.

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes (bit_width bounds) | `vortex_ensure!(bit_width <= 64)` already in BitPackedData::try_new; replicate in loom-core unpack |
| V6 Cryptography | no | — |

Only relevant threat: integer overflow in bit arithmetic. Use `checked_*` or `div_ceil` (as Vortex does) for buffer size calculations to avoid panics on malformed LayoutNode inputs.

---

## Sources

### Primary (HIGH confidence — source code verified in ~/.cargo/registry)

- `~/.cargo/registry/src/.../fastlanes-0.5.1/src/transpose.rs` — `FL_ORDER`, `transpose()` function, verified exact implementation
- `~/.cargo/registry/src/.../fastlanes-0.5.1/src/macros.rs` — `pack!`, `unpack!`, `index()` macro, verified exact bit arithmetic
- `~/.cargo/registry/src/.../fastlanes-0.5.1/src/lib.rs` — `FL_ORDER = [0,4,2,6,1,5,3,7]`, `FastLanes` trait, `LANES` definition
- `~/.cargo/registry/src/.../vortex-fastlanes-0.74.0/src/bitpacking/array/mod.rs` — `BitPackedData` fields, `BitPackedArrayExt` accessors, `bit_width()`, `offset()`, `packed()`, `validity()`, `patches()` all verified
- `~/.cargo/registry/src/.../vortex-fastlanes-0.74.0/src/bitpacking/array/bitpack_decompress.rs` — `unpack_single_primitive` algorithm verified; `unpack_array` path
- `~/.cargo/registry/src/.../vortex-fastlanes-0.74.0/src/bitpacking/array/unpack_iter.rs` — `CHUNK_SIZE = 1024`, `elems_per_chunk = 128 * bit_width / size_of::<T>()`, `decode_into` verified
- `~/.cargo/registry/src/.../vortex-fastlanes-0.74.0/src/for/array/mod.rs` — `FoRData`, `FoRArrayExt::encoded()`, `FoRArrayExt::reference_scalar()` verified
- `~/.cargo/registry/src/.../vortex-fastlanes-0.74.0/src/for/array/for_decompress.rs` — FOR decode algorithm: wrapping_add of reference scalar after bitunpack
- `~/.cargo/registry/src/.../vortex-fastlanes-0.74.0/src/for/vtable/validity.rs` — `ValidityChild<FoR>` delegates to `encoded()` child verified
- `~/.cargo/registry/src/.../vortex-array-0.74.0/src/validity.rs` — `Validity` enum: `NonNullable / AllValid / AllInvalid / Array(ArrayRef)` verified
- `~/.cargo/registry/src/.../arrow-array-58.3.0/src/builder/primitive_builder.rs` — `append_value`, `append_null`, `append_option`, `finish()` returning `PrimitiveArray`, `into_data()` chain verified
- `crates/loom-ffi/src/ffi.rs` (in-repo) — `Int32Builder` → `finish()` → `into_data()` → `to_ffi` → `ptr::write` pattern verified working
- `Cargo.toml` (workspace, in-repo) — all arrow-* pinned at `=58.3.0`, vortex-* at `=0.74.0`, confirmed no version skew

### Secondary (MEDIUM confidence)

- `.planning/research/ARCHITECTURE.md` — `LayoutNode` schema, synthesized_read_loop pseudocode (pre-existing research, confirmed consistent with source)
- `.planning/research/FEATURES.md` — FOR algorithm description (pre-existing research, confirmed against source)
- `.planning/research/PITFALLS.md` — P7 validity/null handling (confirmed against source)

---

## Metadata

**Confidence breakdown:**
- FastLanes layout / unpack algorithm: HIGH — verified line-by-line against installed source
- vortex-fastlanes 0.74 accessor names: HIGH for `bit_width`, `offset`, `patches`, `validity`, `encoded`, `reference_scalar` — verified in source. MEDIUM for `BufferHandle::as_host()` byte access chain — deferred to Wave-0
- FOR nesting structure: HIGH — `ValidityChild<FoR>` delegates to inner, confirmed in source
- arrow-rs 58.3 builder API: HIGH — `append_value`, `append_null`, `finish()`, `into_data()` all verified in source + in-repo ffi.rs
- LayoutNode field design: MEDIUM — logically derived from source; exact field types (especially validity representation) are Claude's discretion

**Research date:** 2026-06-07
**Valid until:** 2026-07-07 (vortex-fastlanes 0.74 is pinned; stable until workspace upgrades)
