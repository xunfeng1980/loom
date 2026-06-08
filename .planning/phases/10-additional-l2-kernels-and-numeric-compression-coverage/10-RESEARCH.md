# Phase 10 Research: Additional L2 Kernels and Numeric Compression Coverage

**Date:** 2026-06-08
**Status:** Ready for planning
**Requirement:** COV-01

## Executive Summary

Phase 10 should proceed with ALP-style float L2 coverage, but not by adding a new Vortex dependency to `loom-core` or waiting for a `vortex-alp` crate. Local source inspection of the pinned Vortex 0.74.0 crates found no public `vortex-alp` package and no ALP array/encoding API. The only local ALP-related source mention is Vortex's `PatchedArray` documentation citing G-ALP as background for patching.

The recommended implementation is therefore:

- Add Loom-owned `AlpParams` with a stable binary format and a pure `loom-core` decode kernel at kernel id `1`.
- Extend the existing typed Arrow path to `Float32` and `Float64`.
- Use Vortex primitive float arrays as the native oracle for row values, plus synthetic ALP known-value fixtures that exercise params, nulls, repeats, negatives, zero, and decimal scaling.
- Document that Vortex 0.74.0 does not expose an ALP extraction bridge in this repository's pinned stack; delta-of-delta remains a fallback only if the Loom ALP kernel itself hits a hard compile/API blocker.

This keeps the Phase 10 user decision intact: the phase targets ALP first, not a toy kernel and not delta-of-delta by convenience.

## Local Evidence

### Vortex ALP Surface

Commands inspected local pinned Vortex sources under:

- `$HOME/.cargo/registry/src/index.crates.io-*/vortex-array-0.74.0`
- `$HOME/.cargo/registry/src/index.crates.io-*/vortex-fastlanes-0.74.0`
- `$HOME/.cargo/registry/src/index.crates.io-*/vortex-fsst-0.74.0`

Findings:

- `find ~/.cargo/registry/src -maxdepth 2 -type d -name '*alp*'` found no ALP crate.
- `rg -n 'Alp|ALP|alp' .../vortex-*-0.74.0` found no ALP array or kernel API.
- The only relevant mention is `vortex-array-0.74.0/src/arrays/patched/mod.rs`, whose module docs say patched arrays are inspired by G-ALP, but that API is a general primitive patching mechanism, not an ALP float compression format.

Planning implication: do not spend Phase 10 trying to extract `AlpArray` internals that are not present in the pinned dependency set. Keep Vortex in `loom-fixtures` as an oracle producer, not as an implementation dependency.

### Existing Loom L2 Surface

Reusable pieces:

- `crates/loom-core/src/l2_kernel_registry.rs` already has `L2Kernel::decode(params, count) -> ArrayData` and FSST at id `0`.
- `crates/loom-core/src/fsst_params.rs` is the model for stable params encode/decode and structural validation.
- `LayoutNode::KernelEscape { kernel_id, params, count }` is already stable in the binary payload and descriptor formats.
- `decode_node_to_array_data_with_registry` already lets nested dictionary/RLE values route into kernel escapes.
- Phase 9 verifier already rejects unknown kernels and malformed FSST params before decode.

Missing pieces for ALP:

- `OutputBuilder`, `DecodedArray`, `descriptor`, `layout_codec`, `verifier`, `loom-cli`, and the DuckDB extension currently know only Boolean, Int32, Int64, and Utf8.
- The DuckDB extension's header parser maps LMP1 dtype tags `1..=4` only. Float32/Float64 need append-only tags and C++ scan support.
- `loom-core` currently has `fsst-rs` as an L2 implementation dependency; adding ALP should not add Vortex/FastLanes dependencies to `loom-core`.

## Recommended ALP Params Contract

Use a compact, explicit Loom-owned format:

- Magic: `LAP1`
- Version: `1`
- Flags: validity present
- Output type tag: `Float32` or `Float64`
- Row count
- Base decimal exponent as signed integer
- Encoded mantissas as signed 64-bit values
- Optional per-row validity bytes
- Optional exception table is deferred unless implementation needs it for the chosen fixture set

Decode rule for Phase 10:

```text
decoded = mantissa * 10^decimal_exponent
```

The exact internal representation can be adjusted during execution, but must remain deterministic, bounds-checked, and verifier-readable. Exact bit equality is preferred for fixture values chosen to be exactly representable after scaling; if any row requires tolerance, the test must name that row/class and keep tolerance fixed.

## Fixture and Oracle Strategy

Use two complementary fixture classes:

- Synthetic known-value ALP fixtures: hand-encoded `AlpParams` for Float32 and Float64 with small finite values, negatives, zero, repeats, and nulls.
- Vortex-native oracle fixtures: build Vortex `PrimitiveArray` values for the same rows and decode through Vortex's execution path to obtain the reference float rows/null flags.

Because Vortex 0.74.0 lacks an exposed ALP encoded-array API, the Vortex oracle verifies the semantic row values, while the synthetic ALP params verify Loom's kernel and wire format. This matches the MVP0 pattern that Vortex is an input/oracle bridge and `loom-core` owns independent decode logic.

## Risks

| Risk | Impact | Mitigation |
|---|---:|---|
| Vortex 0.74.0 has no ALP API | High | Do not block on nonexistent extraction. Record this evidence and use Vortex primitive oracle plus synthetic ALP params. |
| Float values introduce equality ambiguity | Medium | Choose exact-representable fixtures first; allow fixed tolerance only with a documented reason. |
| Float support touches many surfaces | Medium | Split Float dtype plumbing before ALP fixture/DuckDB gates. |
| `loom-core` accidentally imports Vortex | High | Keep dependency guard in every plan and in final verification. |
| DuckDB direct Arrow scan may mishandle float buffers | Medium | Add explicit C++ `FLOAT`/`DOUBLE` bind and `FillFixedWidthVector<float/double>` coverage. |

## Fallback Boundary

Delta-of-delta is allowed only if the Loom-owned ALP params/kernel cannot be made to compile or cannot produce stable finite Float32/Float64 Arrow arrays under the current dependency set. Lack of a Vortex ALP extractor is not by itself a fallback trigger because Phase 10 can still use Vortex native primitive arrays as the oracle for row semantics.

