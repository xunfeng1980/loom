# Architecture Patterns

**Project:** Loom MVP0 (DuckDB demo)
**Researched:** 2026-06-07
**Confidence:** HIGH — derived from design.md, STACK.md, official Arrow C Data Interface spec, and vortex-data source docs.

---

## Recommended Architecture

The system has two language-domain components connected by a single ABI seam (Arrow C Data Interface), plus a verification harness.

```
┌───────────────────────────────────────────────────────────────┐
│  Rust decoder crate  (staticlib: libloom_decoder.a)           │
│                                                               │
│  ┌──────────────────┐   ┌────────────────────────────────┐   │
│  │  vortex_reader   │   │  l1_model                      │   │
│  │  (mod)           │──▶│  (mod)                         │   │
│  │                  │   │  LayoutNode enum               │   │
│  │  Deserialize a   │   │  bitpack / FOR / dict / RLE /  │   │
│  │  single Vortex   │   │  KernelEscape(kernel_id)       │   │
│  │  ArrayRef from   │   │                                │   │
│  │  bytes (no file  │   │  synthesized_read_loop()       │   │
│  │  container).     │   │  walks LayoutNode tree,        │   │
│  │  Hands off a     │   │  dispatches to L1 decoders     │   │
│  │  typed Layout    │   │  or calls l2_kernel_registry   │   │
│  │  description.    │   │  on KernelEscape nodes.        │   │
│  └──────────────────┘   └─────────────┬──────────────────┘   │
│                                       │                       │
│                         ┌─────────────▼──────────────────┐   │
│                         │  l2_kernel_registry             │   │
│                         │  (mod)                          │   │
│                         │                                 │   │
│                         │  trait L2Kernel { decode() }    │   │
│                         │  registry: Vec<Box<dyn L2Kernel>>│  │
│                         │                                 │   │
│                         │  ┌──────────────────────────┐  │   │
│                         │  │  fsst_kernel             │  │   │
│                         │  │  (struct)                │  │   │
│                         │  │                          │  │   │
│                         │  │  Holds Decompressor      │  │   │
│                         │  │  (fsst-rs).              │  │   │
│                         │  │  Input: codes &[u8] +    │  │   │
│                         │  │    symbol_table.         │  │   │
│                         │  │  Output: decoded bytes   │  │   │
│                         │  │    appended to Arrow     │  │   │
│                         │  │    StringBuilder.        │  │   │
│                         │  └──────────────────────────┘  │   │
│                         └─────────────┬──────────────────┘   │
│                                       │                       │
│                         ┌─────────────▼──────────────────┐   │
│                         │  arrow_builder_output           │   │
│                         │  (mod)                          │   │
│                         │                                 │   │
│                         │  Holds typed arrow-rs builders  │   │
│                         │  (Int32Builder, StringBuilder,  │   │
│                         │  etc.).  Only append_value /   │   │
│                         │  append_null surface exposed.   │   │
│                         │  finish() → ArrayData → to_ffi  │   │
│                         └─────────────┬──────────────────┘   │
│                                       │                       │
│                         ┌─────────────▼──────────────────┐   │
│                         │  ffi_export_shim                │   │
│                         │  (mod, #[no_mangle] extern "C") │   │
│                         │                                 │   │
│                         │  loom_decode(                   │   │
│                         │    input: *const u8, len: usize,│   │
│                         │    out_array: *mut FFI_ArrowArray│  │
│                         │    out_schema: *mut FFI_ArrowSchema│ │
│                         │  )                              │   │
│                         │                                 │   │
│                         │  Calls builder_output.finish(), │   │
│                         │  calls arrow::ffi::to_ffi(),    │   │
│                         │  writes structs to caller-owned │   │
│                         │  stack slots via ptr::write().  │   │
│                         └────────────────────────────────┘   │
└──────────────────────────────────│────────────────────────────┘
                     Arrow C Data Interface (C ABI)
                     FFI_ArrowArray + FFI_ArrowSchema
                     ownership transfers here
                                   │
┌──────────────────────────────────▼────────────────────────────┐
│  C++ DuckDB Extension  (loom_extension.duckdb_extension)       │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  loom_extension.cpp                                  │    │
│  │                                                       │    │
│  │  ExtensionLoad() {                                    │    │
│  │    TableFunction fn("loom_scan", …, Scan, Bind, Init)│    │
│  │    ExtensionUtil::RegisterFunction(*db, fn);          │    │
│  │  }                                                    │    │
│  │                                                       │    │
│  │  Bind(): declares output LogicalType(s) from schema   │    │
│  │  Init(): calls loom_decode(), stores                  │    │
│  │          FFI_ArrowArray + FFI_ArrowSchema in state.   │    │
│  │  Scan(): calls ArrowToDuckDB() to fill DataChunk;     │    │
│  │          after last chunk, calls array.release(&array)│    │
│  └──────────────────────────────────────────────────────┘    │
└───────────────────────────────────────────────────────────────┘
                                   │
                             DuckDB SQL engine
                    SELECT / aggregate over loom_scan(...)
```

---

## Component Boundaries

| Component | Module / File | Responsibility | Communicates With |
|-----------|--------------|----------------|-------------------|
| `vortex_reader` | `src/vortex_reader.rs` | Deserialize raw Vortex bytes into a typed `LayoutDescription` (the L1 model). Wraps `vortex-array` encoding dispatch to identify what encoding is present without decoding it. | `l1_model` (produces `LayoutNode` tree) |
| `l1_model` + synthesized read loop | `src/l1_model.rs` | Owns the `LayoutNode` enum (bitpack/FOR/dict/RLE/KernelEscape). Implements `synthesized_read_loop()`, which recursively interprets the node tree: L1 nodes call their own decode logic; KernelEscape nodes forward to the L2 registry. Appends to `OutputBuilder`. | `l2_kernel_registry` (for escape nodes), `arrow_builder_output` (append values) |
| `l2_kernel_registry` | `src/l2_kernel_registry.rs` | Registry of `L2Kernel` trait objects indexed by kernel ID. In MVP0 has exactly one entry: `FsstKernel` at index 0. The `L2Kernel` trait: `fn decode(&self, input: &[u8], builder: &mut OutputBuilder)`. | `arrow_builder_output` (appends decoded values) |
| `fsst_kernel` | `src/fsst_kernel.rs` | Implements `L2Kernel`. Holds an `fsst_rs::Decompressor` built from the symbol table embedded in the Vortex FSST array. On `decode()`: calls `decompressor.decompress(codes_slice)` per string, then calls `builder.append_string(bytes)`. | `l2_kernel_registry` (registered into), `arrow_builder_output` |
| `arrow_builder_output` | `src/arrow_builder_output.rs` | Wraps `arrow-rs` typed builders. Exposes `append_value(v)`, `append_null()`. Hides builder internals. `finish() -> ArrayData`. The only place where `arrow-array` 58.x builders are used. | `l1_model`, `fsst_kernel`, `ffi_export_shim` |
| `ffi_export_shim` | `src/ffi.rs` | The `extern "C"` surface. Exposes `loom_decode(…)`. Calls `synthesized_read_loop`, calls `builder.finish()`, calls `arrow::ffi::to_ffi(&data)`, writes `FFI_ArrowArray` + `FFI_ArrowSchema` into caller-provided slots. | C++ DuckDB extension (sole consumer) |
| C++ DuckDB extension | `cpp/loom_extension.cpp` | Registers `loom_scan` table function. On `Init`, calls `loom_decode()`. Stores FFI structs in scan state. On `Scan`, calls `ArrowToDuckDB()` to fill `DataChunk`. Calls `array.release()` on teardown. | DuckDB internals, `loom_decoder.a` via C ABI |
| Verification harness | `tests/` or separate binary | Decodes the same Vortex column via Vortex's own API, decodes via `loom_scan`, compares row-for-row. | Rust decoder (via Vortex), DuckDB C client / `duckdb-rs` in-process |

---

## The Arrow C Data Interface Seam (Rust → C++)

### Option Comparison

| Criterion | Single array (`FFI_ArrowArray` + `FFI_ArrowSchema`) | `FFI_ArrowArrayStream` |
|-----------|-----------------------------------------------------|------------------------|
| Structure | Two flat C structs; one schema + one data array | One struct wrapping `get_schema`, `get_next`, `release` callbacks |
| Batch model | One call produces one batch (the whole column) | Pull model; consumer calls `get_next` repeatedly until NULL |
| Release rule | Consumer calls `array.release(&array)` once; producer's callback recursively frees children | Consumer calls `stream.release(&stream)` once; each batch returned by `get_next` must also be individually released |
| Ownership transfer | Producer writes to caller-owned slots via `ptr::write`; after that, producer has no obligation | Stream object lives until consumer calls its `release`; more complex lifetime |
| Fit for MVP0 | Perfect — one column, one batch, single decode call | Overkill — stream protocol is for multi-batch or multi-column scenarios |
| DuckDB ingestion | `ArrowToDuckDB()` internal helper converts directly; or call `arrow_scan` with a trivially wrapped stream | `arrow_scan` built-in accepts `uintptr_t`-cast stream pointer natively |
| Complexity | Minimal | Requires implementing `get_schema`, `get_next`, `release` callbacks |

**Recommendation: single-array transfer (`FFI_ArrowArray` + `FFI_ArrowSchema`).**

MVP0 decodes exactly one column, producing exactly one Arrow array. The stream protocol adds two extra callbacks and a pull loop with no benefit. Single-array is the simplest correct option.

### Ownership and Release Semantics (Explicit)

**Who allocates:** Rust (`arrow::ffi::to_ffi()`) allocates the backing buffers inside the `FFI_ArrowArray`. The caller (C++) allocates the `FFI_ArrowArray` struct shell on the stack or as a member of the scan state struct.

**Who writes:** The Rust `ffi_export_shim` receives `*mut FFI_ArrowArray` and `*mut FFI_ArrowSchema` pointing to the C++ caller's stack/heap slots and calls `std::ptr::write(out_array, ffi_array)` — this moves the Rust value (including its `release` callback and `private_data` pointer back into Rust heap memory) into the C++ slot.

**After `ptr::write`:** Rust has relinquished ownership. The Rust stack frame no longer holds the value. The C++ owns the struct.

**Who calls `release`:** The C++ extension. Specifically: after `ArrowToDuckDB()` has populated the last `DataChunk`, the scan `Scan()` callback (or the `GlobalTableFunctionState` destructor) calls `array.release(&array)`. This invokes the `release` function pointer stored inside `FFI_ArrowArray`, which Rust installed during `to_ffi()`.

**What `release` does:** The Rust-installed callback recursively releases all children (none for a flat column), frees the buffer backing allocations (e.g., the `Int32` values buffer), frees the `private_data` box, then sets `release = NULL` (marking the struct as released). The C++ does not free children directly — the spec prohibits it.

**Schema lifetime:** `FFI_ArrowSchema` follows identical rules. Its `release` callback is separate and must also be called by the C++ side (or will be called by DuckDB's `ArrowToDuckDB` helper, which handles schema internally).

**Invariant:** After `release` is called, `array.release == NULL`. Any subsequent call is a no-op (the callback checks for NULL). The C++ scan state struct can safely zero-initialize its FFI_ArrowArray members before the Rust call to make accidental double-release safe.

### Code Sketch (Rust side)

```rust
// ffi.rs
#[no_mangle]
pub unsafe extern "C" fn loom_decode(
    input_ptr: *const u8,
    input_len: usize,
    out_array:  *mut arrow::ffi::FFI_ArrowArray,
    out_schema: *mut arrow::ffi::FFI_ArrowSchema,
) {
    let input = std::slice::from_raw_parts(input_ptr, input_len);
    let layout = vortex_reader::read_layout(input);          // → LayoutDescription
    let mut builder = OutputBuilder::new(&layout.data_type());
    l1_model::synthesized_read_loop(&layout, input, &mut builder);
    let array_data = builder.finish();
    let (ffi_array, ffi_schema) = arrow::ffi::to_ffi(&array_data).unwrap();
    std::ptr::write(out_array,  ffi_array);
    std::ptr::write(out_schema, ffi_schema);
}
```

### Code Sketch (C++ side)

```cpp
// In Init():
struct LoomScanState : GlobalTableFunctionState {
    ArrowArray  arrow_array  = {};   // zero-init; release==NULL until populated
    ArrowSchema arrow_schema = {};
    bool done = false;
};

auto &state = *(LoomScanState*)init_input.global_state.get();
loom_decode(
    (const uint8_t*)file_bytes.data(), file_bytes.size(),
    &state.arrow_array, &state.arrow_schema);

// In Scan():
if (state.done) { output.SetCardinality(0); return; }
ArrowToDuckDB(state.arrow_array, output, /* offset= */ 0);
state.done = true;

// In ~LoomScanState() or at Scan() end:
if (state.arrow_array.release) state.arrow_array.release(&state.arrow_array);
if (state.arrow_schema.release) state.arrow_schema.release(&state.arrow_schema);
```

---

## The L1 Layout Model

### Data Structure

The L1 model is a recursive `LayoutNode` enum — pure data, no code. It describes *what the bytes look like*, not how to execute. The synthesized read loop separately interprets it.

```rust
// l1_model.rs

/// A complete layout description for one Vortex column.
pub struct LayoutDescription {
    pub data_type: DataType,    // Arrow DataType for the builder
    pub root: LayoutNode,       // The outermost encoding node
    pub row_count: usize,       // Total element count
}

pub enum LayoutNode {
    /// Raw unencoded values, width in bytes, little-endian.
    Raw {
        offset: usize,   // byte offset into input slice
        width: u8,       // 1, 2, 4, or 8
        count: usize,
    },

    /// Bit-packed integers (FastLanes/Vortex BitPackedEncoding).
    BitPack {
        values_offset: usize,     // byte offset of packed values buffer
        bit_width: u8,            // bits per value (e.g. 11)
        count: usize,
        patches: Option<Box<LayoutNode>>, // exception values, if any
    },

    /// Frame-of-Reference: decoded_value = packed_value + reference.
    FrameOfReference {
        reference: i64,
        inner: Box<LayoutNode>,   // the underlying packed representation
    },

    /// Dictionary encoding: codes → values lookup.
    Dictionary {
        codes: Box<LayoutNode>,       // integer codes array
        values: Box<LayoutNode>,      // the values array (any LayoutNode)
    },

    /// Run-End Encoding: runs of identical values.
    RunEnd {
        run_ends: Box<LayoutNode>,    // monotonically increasing end positions
        values: Box<LayoutNode>,      // one value per run
        count: usize,                 // total logical element count
    },

    /// Escape to an L2 kernel by stable integer ID.
    KernelEscape {
        kernel_id: u32,               // indexes into L2KernelRegistry
        /// Serialized kernel-specific parameters (e.g. FSST symbol table bytes,
        /// codes buffer offset/len).
        params: Vec<u8>,
        count: usize,
    },
}
```

### Synthesized Read Loop

The "synthesized read loop" is an interpreter, not a code generator. It is a recursive function over `LayoutNode`:

```rust
pub fn synthesized_read_loop(
    node: &LayoutNode,
    input: &[u8],
    registry: &L2KernelRegistry,
    builder: &mut OutputBuilder,
) {
    match node {
        LayoutNode::Raw { offset, width, count } => {
            // slice input, cast, append_value per element
        }
        LayoutNode::BitPack { values_offset, bit_width, count, patches } => {
            // unpack using FastLanes-style bit-unpacking; apply patches
        }
        LayoutNode::FrameOfReference { reference, inner } => {
            // decode inner into temp buffer, add reference to each element
        }
        LayoutNode::Dictionary { codes, values } => {
            // decode codes into Vec<usize>; decode values into Vec<V>;
            // emit values[code] for each code
        }
        LayoutNode::RunEnd { run_ends, values, count } => {
            // decode run_ends + values arrays; expand into output
        }
        LayoutNode::KernelEscape { kernel_id, params, count } => {
            // L1→L2 escape: delegate to registry
            registry.get(*kernel_id).decode(params, input, builder);
        }
    }
}
```

**Key properties:**
- The loop is bounded by `count` or the recursive structure depth — termination is trivially visible.
- No heap allocation inside the match arms except temporary decode buffers (FOR, dict) that are stack-scoped.
- The `KernelEscape` arm is the only place L2 code runs. Every other arm is a pure data transformation over `input`.

### How `vortex_reader` Populates the Model

`vortex_reader` uses `vortex-array` to deserialize the input bytes into a `vortex_array::ArrayRef`. It then pattern-matches on `array.encoding().id()` to build the corresponding `LayoutNode`:

- `BitPackedEncoding` → `LayoutNode::BitPack` (reads `bit_width`, `values` buffer offset, optional patches array)
- `FoREncoding` → `LayoutNode::FrameOfReference` (reads `reference`, wraps inner layout)
- `DictEncoding` → `LayoutNode::Dictionary` (recursively describes codes + values)
- `RunEndEncoding` → `LayoutNode::RunEnd` (run_ends + values)
- `FsstEncoding` → `LayoutNode::KernelEscape { kernel_id: 0, params: fsst_params_bytes }`

The `params` field for FSST contains the serialized symbol table (symbols buffer + lengths buffer + codes buffer offset/length). `vortex_reader` extracts these from the Vortex array's child arrays / metadata.

---

## The L2 Kernel Interface and FSST Invocation

### Trait

```rust
// l2_kernel_registry.rs

pub trait L2Kernel: Send + Sync {
    fn decode(
        &self,
        params: &[u8],      // kernel-specific serialized parameters
        input: &[u8],       // full input slice (for buffer-offset params)
        builder: &mut OutputBuilder,
    );
}

pub struct L2KernelRegistry {
    kernels: Vec<Box<dyn L2Kernel>>,
}

impl L2KernelRegistry {
    pub fn default_for_mvp0() -> Self {
        Self { kernels: vec![Box::new(FsstKernel::new())] }
    }

    pub fn get(&self, id: u32) -> &dyn L2Kernel {
        &*self.kernels[id as usize]
    }
}
```

### FSST Kernel: Inputs, Outputs, Position

```rust
// fsst_kernel.rs

use fsst_rs::{Decompressor, Symbol};

pub struct FsstKernel;

impl L2Kernel for FsstKernel {
    fn decode(&self, params: &[u8], input: &[u8], builder: &mut OutputBuilder) {
        // 1. Deserialize params → symbol table + codes buffer location
        let FsstParams { symbols, lengths, codes_offset, codes_len, offsets_offset, offsets_len }
            = FsstParams::deserialize(params);

        // 2. Build the Decompressor from symbol table
        //    Decompressor::new(&symbols, &lengths)
        let decompressor = Decompressor::new(&symbols, &lengths);

        // 3. For each string in the codes slice:
        //    - use offsets to find the per-string codes slice
        //    - call decompressor.decompress(codes_slice)  → Vec<u8>
        //    - call builder.append_string(&decompressed_bytes)
        let codes_buf = &input[codes_offset..codes_offset + codes_len];
        let offsets   = decode_offsets(&input[offsets_offset..offsets_offset + offsets_len]);

        for window in offsets.windows(2) {
            let (start, end) = (window[0], window[1]);
            let decompressed = decompressor.decompress(&codes_buf[start..end]);
            builder.append_string(std::str::from_utf8(&decompressed).unwrap());
        }
    }
}
```

**Position relative to Arrow builders:** FSST sits immediately above the `OutputBuilder`. It does not touch `FFI_ArrowArray` directly. The decode loop calls `builder.append_string()`, which routes to the `arrow-rs` `StringBuilder`. Only `arrow_builder_output::finish()` materializes the Arrow array.

**FSST inputs:**
- `symbols: &[Symbol]` — up to 255 entries, each up to 8 bytes packed in a `u64`
- `lengths: &[u8]` — parallel lengths array for symbol table
- `codes_buf: &[u8]` — the encoded byte stream (8-bit codes + escape bytes)
- `offsets: &[u64]` — per-string byte boundaries within `codes_buf`

**FSST outputs:** decoded UTF-8 bytes per string, appended to `StringBuilder` via `append_string`.

**The `Decompressor` API (`fsst-rs` 0.5.11):**
- `Decompressor::new(symbols: &[Symbol], lengths: &[u8]) -> Self`
- `decompressor.decompress(compressed: &[u8]) -> Vec<u8>` — simple allocation variant, fine for MVP0
- `decompressor.decompress_into(compressed: &[u8], buf: &mut [MaybeUninit<u8>]) -> usize` — zero-alloc variant (optional optimisation)

---

## Build and Link Topology

```
rust/
├── Cargo.toml            (crate-type = ["staticlib"])
├── build.rs              (cbindgen → include/loom.h)
├── src/
│   ├── lib.rs
│   ├── vortex_reader.rs
│   ├── l1_model.rs
│   ├── l2_kernel_registry.rs
│   ├── fsst_kernel.rs
│   ├── arrow_builder_output.rs
│   └── ffi.rs
└── include/
    └── loom.h            (generated by cbindgen)

cpp/
├── CMakeLists.txt
└── loom_extension.cpp

tests/
└── verify.rs             (or Python/shell harness using duckdb Python client)
```

### CMake Link Steps

```cmake
# Step 1: Build Rust staticlib as ExternalProject
add_custom_command(
    OUTPUT ${CMAKE_SOURCE_DIR}/rust/target/release/libloom_decoder.a
    COMMAND cargo build --release
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}/rust
)
add_custom_target(rust_decoder ALL
    DEPENDS ${CMAKE_SOURCE_DIR}/rust/target/release/libloom_decoder.a)

# Step 2: Link into DuckDB extension shared library
add_dependencies(loom_extension rust_decoder)
target_link_libraries(loom_extension PRIVATE
    ${CMAKE_SOURCE_DIR}/rust/target/release/libloom_decoder.a)
target_include_directories(loom_extension PRIVATE
    ${CMAKE_SOURCE_DIR}/rust/include)   # loom.h from cbindgen
```

### Why staticlib, not cdylib

The DuckDB extension is itself a shared library (`.duckdb_extension`). Nesting a `cdylib` inside another `cdylib` introduces runtime RPATH issues on macOS and requires coordinated dynamic linker configuration. `staticlib` links at compile time into the extension `.so`/`.dylib` — the extension binary is self-contained and the Rust standard library is included once, without dynamic resolution.

---

## Build Order / Dependency Graph

```
[1] Data fixture preparation (independent of build)
    └── Generate a single Vortex-encoded column (one of: BitPacked, FoR, Dict, RLE, FSST)
        using vortex-array + vortex-fastlanes/dict/fsst in a Rust binary.
        Write to disk as raw bytes (no .vortex file container).

[2] Rust decoder crate  (depends on: nothing external beyond cargo deps)
    ├── cargo build → runs build.rs → cbindgen generates include/loom.h
    └── cargo build --release → produces rust/target/release/libloom_decoder.a

[3] C++ DuckDB extension  (depends on: [2] libloom_decoder.a + loom.h, DuckDB headers)
    └── cmake --build → links loom_extension.duckdb_extension

[4] Load and smoke-test in DuckDB  (depends on: [1] fixture, [3] extension)
    └── LOAD 'loom_extension'; SELECT * FROM loom_scan('fixture.bin');

[5] Vortex reference decoder  (independent of [2]/[3]; can run in parallel with [2])
    └── Small Rust binary: read fixture.bin via vortex-array into_canonical()
        → print rows as CSV/JSON

[6] Verification harness  (depends on: [4] and [5])
    └── Compare [4] output vs [5] output row-for-row.
        Acceptance: zero mismatches.
```

**Dependency graph summary:**

```
[1] fixture ──────────────────────────────────────┐
                                                  ▼
[2] Rust cargo build ──▶ libloom_decoder.a ──▶ [3] cmake ──▶ extension ──▶ [4] DuckDB test
                                                                                    │
[5] vortex reference binary (parallel) ─────────────────────────────────────────────┤
                                                                                    ▼
                                                                            [6] row comparison
```

**Critical path for MVP0:** [1] → [2] → [3] → [4] → [6]. Steps [5] can proceed in parallel once the fixture is written.

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Calling Vortex's Own Decoder Inside the L1 Loop
**What:** Using `array.into_canonical()?.into_arrow()` (Vortex's decode path) inside `synthesized_read_loop`.
**Why bad:** Defeats the purpose of MVP0. The point is to demonstrate that Loom's interpreter produces matching output. If the L1 loop delegates to Vortex's decoder, there is nothing to compare.
**Instead:** Use Vortex crates only in `vortex_reader` (to parse the binary layout) and in the reference decoder binary (to generate comparison output). The read loop must implement decoding independently.

### Anti-Pattern 2: Mixing arrow2 with arrow-rs
**What:** Pulling `arrow2` for any reason alongside `vortex-array` (which uses `arrow-rs` 58.x).
**Why bad:** `FFI_ArrowArray` from `arrow2` and `arrow-rs` are ABI-identical C structs but Rust sees incompatible types; also `vortex-array` type aliases (ArrayRef, etc.) will fail to unify.
**Instead:** Use only `arrow` (arrow-rs) 58.x and its sub-crates throughout.

### Anti-Pattern 3: Putting Business Logic in the FFI Shim
**What:** Calling `synthesized_read_loop` or builder logic directly inside the `extern "C"` function.
**Why bad:** The `extern "C"` surface is not `#[catch_unwind]`-safe in Rust. Any panic crossing the FFI boundary is undefined behaviour.
**Instead:** All decode work happens in safe Rust inside the inner modules. The FFI shim calls a single entry function wrapped in `std::panic::catch_unwind`. On panic, return an error code rather than unwinding into C++.

### Anti-Pattern 4: Using ArrowArrayStream for a Single-Column Single-Batch Use Case
**What:** Implementing `FFI_ArrowArrayStream` with `get_schema`/`get_next`/`release` callbacks for MVP0.
**Why bad:** Three extra callback implementations, a pull loop on the C++ side, and per-batch release tracking — all for the single batch the column produces. Unnecessary complexity.
**Instead:** Use single-array transfer (`FFI_ArrowArray` + `FFI_ArrowSchema`) with `arrow::ffi::to_ffi`.

### Anti-Pattern 5: Allocating the FFI Structs Inside Rust and Returning Pointers
**What:** `Box`-allocating `FFI_ArrowArray` in Rust and returning a raw pointer to C++.
**Why bad:** C++ now owns a heap pointer allocated by Rust's allocator. Freeing it from C++ (via `delete` or `free`) is undefined behaviour unless the `release` callback handles it. The lifetime contract becomes unclear.
**Instead:** C++ allocates the struct (stack or member of scan state struct). Rust receives a `*mut FFI_ArrowArray` and writes into it with `ptr::write`. Ownership of the *struct shell* stays with C++; ownership of the *heap-allocated buffers inside it* transfers via the `release` callback.

---

## Scalability Considerations

This is MVP0 — scalability is not a goal. These notes exist to avoid architectural decisions that would force a rewrite later.

| Concern | MVP0 (single column, demo) | Later milestone |
|---------|---------------------------|-----------------|
| Multiple L2 kernels | `Vec<Box<dyn L2Kernel>>` registry, `kernel_id` index — adding a kernel is appending to a Vec | Add ALP, delta, etc. |
| Multiple columns | `loom_decode` signature takes a single column today — extend to take a `projection_mask` and return a `RecordBatch` | Matches design §9 ABI |
| Batch / streaming | Single call returns one array — extend to `FFI_ArrowArrayStream` with `get_next` loop | Natural extension of the seam |
| Verification | Row-by-row comparison binary — adequate for MVP0 | Need property tests / fuzz corpus later |

---

## Sources

- `design.md` (Loom full design — authoritative) — §3 L1/L2 split, §4 L1 layout model, §5 L2 memory model, §6 builder output, §9 ABI
- `.planning/PROJECT.md` — MVP0 requirements and constraints
- `.planning/research/STACK.md` — pinned crate versions, build topology, FFI patterns
- [Apache Arrow C Data Interface specification](https://arrow.apache.org/docs/format/CDataInterface.html) — `FFI_ArrowArray`/`FFI_ArrowSchema` struct layout, ownership/release rules
- [Apache Arrow C Stream Interface specification](https://arrow.apache.org/docs/format/CStreamInterface.html) — `ArrowArrayStream` release semantics (compared, not recommended for MVP0)
- [arrow::ffi module docs (arrow-rs)](https://docs.rs/arrow/latest/arrow/ffi/index.html) — `to_ffi`, `FFI_ArrowArray`, `FFI_ArrowSchema`
- [fsst-rs Decompressor docs](https://docs.rs/fsst-rs/latest/fsst/struct.Decompressor.html) — `new(symbols, lengths)`, `decompress()`, `decompress_into()` API (version 0.5.11)
- [SpiralDB FSST blog post](https://spiraldb.com/post/compressing-strings-with-fsst) — FSST array structure: symbols, codes, offsets
- [duckdb/arrow extension — arrow_scan_ipc.cpp](https://github.com/duckdb/arrow/blob/main/src/arrow_scan_ipc.cpp) — ArrowArrayStream consumption pattern in DuckDB C++
- [DuckDB Arrow IPC support blog](https://duckdb.org/2025/05/23/arrow-ipc-support-in-duckdb) — nanoarrow migration, 1.3 deprecation of arrow extension
- [vortex-data/vortex GitHub](https://github.com/vortex-data/vortex) — source of encoding structures confirmed

---

*Architecture research for: Loom MVP0 — Rust decoder core + Arrow C Data Interface + C++ DuckDB extension*
*Researched: 2026-06-07*
