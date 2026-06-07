# Phase 04 Pattern Map

**Phase:** 04-l1-dict-rle-and-l2-escape-infrastructure  
**Created:** 2026-06-07

## Closest Existing Patterns

| Planned File | Role | Existing Analog | Pattern To Reuse |
|--------------|------|-----------------|------------------|
| `crates/loom-core/src/l1_model.rs` | recursive L1 interpreter | existing `Raw`, `BitPack`, `FrameOfReference` arms | match arm delegates to private decode helper; return `LoomDecodeError`, never panic for malformed input |
| `crates/loom-core/src/arrow_builder_output.rs` | typed Arrow output | existing `Int32`/`Int64` variants | enum wrapper over Arrow typed builders with narrow append API and `finish() -> ArrayData` |
| `crates/loom-core/src/error.rs` | typed decode errors | existing `UnimplementedEncoding`, `BufferTooShort`, `UnsupportedWidth` | add specific variants with Display text and unit tests |
| `crates/loom-core/src/l2_kernel_registry.rs` | L2 dispatch seam | placeholder module in `lib.rs`; Phase 4 context D-01/D-03 | separate module exposing `L2Kernel`, `L2KernelRegistry`, `default_for_mvp0`, and stub `FsstKernel` |
| `crates/loom-fixtures/src/vortex_reader.rs` | Vortex-to-LayoutNode bridge | existing `from_bitpacked_array`, `from_for_array`, `extract_validity` | all Vortex APIs stay in `loom-fixtures`; return plain `LayoutNode` trees to `loom-core` |
| `crates/loom-fixtures/src/oracle.rs` | Vortex reference decode | existing `decode_i32_oracle`, `decode_u32_oracle`, `extract_null_flags` | execute to canonical Primitive/Bool array, return values plus null flags |
| `crates/loom-fixtures/tests/*_roundtrip.rs` | oracle comparison | `bitpack_roundtrip.rs`, `for_roundtrip.rs` | build in-memory Vortex arrays, bridge to LayoutNode, decode with Loom, compare row-for-row |

## API Facts To Use

- `vortex_array::arrays::dict::DictArraySlotsExt` exposes `codes()` and `values()`.
- `vortex_array::arrays::DictArray::try_new(codes.into_array(), values.into_array())` constructs dict fixtures.
- `vortex_fastlanes::rle::RLEArrayExt` exposes `values()`, `indices()`, `values_idx_offsets()`, and `offset()`.
- `vortex_fastlanes::RLEData::encode(primitive.as_view(), ctx)` constructs real RLE fixtures.
- RLE validity is derived from `indices().slice(start..stop)?.validity()`.
- `loom-core` must continue to have zero `vortex-*` and zero `fastlanes` dependencies.

## Implementation Notes

- Keep the `KernelEscape` seam visible: L2 code should live only in `l2_kernel_registry.rs` and the `KernelEscape` match path.
- Prefer a top-level `decode_layout_to_array_data` helper over adding string support to `OutputBuilder` in Phase 4.
- Treat Vortex FastLanes RLE's chunk-index representation as a bridge concern. The Loom interpreter's `RunEnd` arm should remain the simple declarative run-end expansion described by the roadmap and context.

