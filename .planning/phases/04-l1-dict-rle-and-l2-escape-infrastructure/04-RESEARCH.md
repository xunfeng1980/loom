# Phase 04 Research: L1 Dict, RLE, and L2 Escape Infrastructure

**Phase:** 04-l1-dict-rle-and-l2-escape-infrastructure  
**Researched:** 2026-06-07  
**Status:** Complete

## Research Complete

Phase 4 can be planned against local source rather than guesses. The existing `loom-core` model already has `LayoutNode::Dictionary`, `LayoutNode::RunEnd`, and `LayoutNode::KernelEscape`, but all three still return `LoomDecodeError::UnimplementedEncoding`. `crates/loom-core/src/lib.rs` also exposes an empty inline `pub mod l2_kernel_registry {}` placeholder that should become a real module.

## Key Findings

### Existing Loom Core Shape

- `crates/loom-core/src/l1_model.rs` implements `Raw`, `BitPack`, and `FrameOfReference`.
- `decode_for` applies the reference only for an inner `BitPack`; the non-`BitPack` fallback currently calls `synthesized_read_loop(inner, builder)` and returns without applying `reference`. This is the Phase 4 folded CR-02 fix.
- `OutputBuilder` currently supports `Int32` and `Int64` only. RLE boolean success criteria require `OutputBuilder::Boolean(BooleanBuilder)`, `append_bool`, `finish()`, and a clear typed-width behavior for non-integer builders.
- `loom-core` has no `vortex-*` dependencies and must stay that way. Dictionary/RLE Vortex inspection belongs in `loom-fixtures`.

### Vortex Dict API Confirmed

Source: `~/.cargo/registry/src/.../vortex-array-0.74.0/src/arrays/dict`.

- Dictionary lives in `vortex-array`, not a separate `vortex-dict` crate.
- Public type: `vortex_array::arrays::DictArray`.
- Extension traits expose slots:
  - `DictArraySlotsExt::codes() -> &ArrayRef`
  - `DictArraySlotsExt::values() -> &ArrayRef`
- Constructor: `DictArray::try_new(codes.into_array(), values.into_array())`.
- Validity is the combination of code validity and value validity:
  - null codes produce null output rows;
  - nullable values can produce null output rows after lookup;
  - both-nullable case builds a validity mask by dictionary-taking `values_validity` through `codes`.
- Vortex execution expands dict by taking canonical `values` using primitive `codes`.

Planning consequence: `vortex_reader` should add `from_dict_array`, recurse through `codes()` and `values()`, and `loom-core` should implement dictionary lookup from child decoded arrays without depending on Vortex.

### Vortex RLE API Confirmed

Source: `~/.cargo/registry/src/.../vortex-fastlanes-0.74.0/src/rle`.

- Public type: `vortex_fastlanes::RLEArray`.
- Extension trait: `RLEArrayExt`.
- Slots:
  - `values() -> &ArrayRef`
  - `indices() -> &ArrayRef`
  - `values_idx_offsets() -> &ArrayRef`
  - `offset() -> usize`
- Constructor: `RLE::try_new(values, indices, values_idx_offsets, offset, length)`.
- Encoder: `RLEData::encode(array.as_view(), ctx)` or `RLE::encode(...)`.
- Validity delegates to the sliced `indices()` array: `array.indices().slice(start..stop)?.validity()`.
- Vortex FastLanes RLE is chunk-index based, not the simple Arrow RunEndArray model. Its decompressor uses chunk-local `indices` into chunk-local value dictionaries with `values_idx_offsets`.

Planning consequence: preserve the existing Loom `LayoutNode::RunEnd { run_ends, values, count }` contract for the interpreter, but the fixture bridge must translate Vortex RLE into the Loom run-end model or use a hand-written run-end fallback where exact Vortex run-end extraction is not direct. The plan should require Vortex oracle comparison where feasible and explicit fallback tests if bridge complexity would exceed Phase 4.

### L2 Registry Contract

The Phase 4 context is internally consistent:

- `L2Kernel` should be a total function returning its own `arrow_data::ArrayData`.
- `L2KernelRegistry` should wrap `Vec<Box<dyn L2Kernel>>`.
- `default_for_mvp0()` registers FSST at index `0`.
- `get(id)` returns `Option<&dyn L2Kernel>` or equivalent; missing IDs become a typed `LoomDecodeError::UnknownKernel(u32)`, not a panic.
- Stub FSST returns an empty `arrow::array::StringArray`/Utf8 `ArrayData`.

Planning consequence: because `synthesized_read_loop` currently appends into a supplied `OutputBuilder`, Phase 4 needs a top-level helper such as `decode_layout_to_array_data(&LayoutDescription, &L2KernelRegistry) -> Result<ArrayData, LoomDecodeError>` or an internal decode result enum. Do not contort `OutputBuilder` to own strings this phase.

## Recommended Plan Split

### Plan 04-01: loom-core implementation

Implement the safe core mechanics:

- `OutputBuilder::Boolean` and `append_bool`.
- typed errors for invalid dictionary code, unsupported builder/type mismatch, and unknown kernel.
- child materialization helpers for integer and boolean arrays from `ArrayData`.
- dictionary lookup for integer-valued dictionaries with null propagation.
- RunEnd expansion for integer and boolean values with monotonic run-end validation.
- `l2_kernel_registry.rs` with stub `FsstKernel`.
- `KernelEscape` routing via registry and a decode entry that can return kernel-owned `ArrayData`.
- CR-02 `decode_for` non-BitPack fallback by materializing child values, applying `reference`, and preserving nulls.

### Plan 04-02: Vortex bridge and oracle tests

Extend `loom-fixtures`:

- `from_dict_array` using `DictArraySlotsExt::codes()` and `values()`.
- `from_rle_array` using `RLEArrayExt` and either a faithful run-end conversion or an explicit fallback for hand-built Loom RunEnd fixtures plus Vortex oracle coverage of RLE source arrays.
- oracle helpers for bool and signed/unsigned primitive arrays.
- tests for dict integer, nullable dict, RLE integer, RLE boolean, nullable RLE, FOR-over-Raw CR-02, and KernelEscape routing.

## Risks

- The existing `LayoutNode::RunEnd` is simpler than Vortex FastLanes RLE. The executor must make this mismatch explicit in code comments and tests.
- `OutputBuilder::t_bits()` is integer-specific. Boolean support must not make bitpack/FOR paths accidentally treat booleans as 1-bit integer builders.
- KernelEscape cannot be fully represented by a mutable `OutputBuilder` append API. A top-level `ArrayData` decode helper is the clean route.

## Verification Architecture

- `cargo test -p loom-core` for builder, read-loop, typed-error, KernelEscape, and CR-02 tests.
- `cargo test -p loom-fixtures --test dict_roundtrip`.
- `cargo test -p loom-fixtures --test rle_roundtrip`.
- `cargo test --workspace`.
- `cargo tree -p loom-core | grep -c -E 'vortex|fastlanes'` must remain `0`.
- `grep -rn 'todo!|unimplemented!|panic!' crates/loom-core/src/l1_model.rs crates/loom-core/src/l2_kernel_registry.rs` should not find newly introduced production panic stubs.

