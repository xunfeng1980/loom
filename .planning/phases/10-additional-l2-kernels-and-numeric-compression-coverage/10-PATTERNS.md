# Phase 10 Patterns: ALP Float L2 Coverage

**Date:** 2026-06-08
**Status:** Ready for planning

## Pattern Map

### Stable Params Modules

`crates/loom-core/src/fsst_params.rs` is the template for `AlpParams`:

- Private magic/version constants.
- Public params struct with owned vectors.
- `encode()` returns a deterministic byte vector.
- `decode(params, expected_count)` validates magic, version, flags, row count, lengths, offsets, validity bytes, and trailing bytes.
- Malformed inputs return typed `LoomDecodeError` values, never panics.

Apply the same shape for `crates/loom-core/src/alp_params.rs`.

### L2 Registry

`crates/loom-core/src/l2_kernel_registry.rs` owns:

- `L2Kernel` trait.
- Append-only `Vec<Box<dyn L2Kernel>>`.
- FSST id `0`.
- Panic-to-error wrapper around the kernel implementation.

Add ALP id `1` by appending to the registry. Do not renumber FSST. Keep `L2Kernel::decode(params, count) -> ArrayData`.

### Data Type Plumbing

Current type support is repeated in:

- `arrow_builder_output.rs`
- `l1_model.rs` `DecodedArray` and `data_type_name`
- `descriptor.rs` `DescriptorDataType`
- `layout_codec.rs` dtype tags
- `verifier.rs` `is_supported_data_type` and kernel/type checks
- `loom-cli/src/main.rs`
- `duckdb-ext/loom_extension.cpp`

Phase 10 should add `Float32` and `Float64` in all of these places before relying on ALP end-to-end gates.

### Verifier

Phase 9 established the pattern:

- Structural verifier runs before decode.
- Unknown kernel ids fail with `unknown-kernel`.
- Kernel-specific params are decoded during verification when the kernel id is known.
- Diagnostics include stable code/path/message.

For ALP:

- Kernel id `1` is known.
- `AlpParams::decode(params, count)` validates shape.
- `AlpParams.output_type` must equal `LayoutDescription.data_type`.
- FSST remains Utf8-only.
- ALP remains Float32/Float64-only.

### Fixture and Oracle Boundary

`crates/loom-fixtures/src/vortex_reader.rs` is the only Vortex layout bridge. `loom-core` remains Vortex-free.

For Phase 10, because Vortex 0.74.0 does not expose an ALP array, add helper functions in `loom-fixtures` that:

- Build Vortex primitive Float32/Float64 arrays for oracle row values.
- Build matching Loom `LayoutDescription { data_type: Float32/Float64, root: KernelEscape { kernel_id: 1, params, count } }`.
- Compare Loom output against Vortex primitive oracle and synthetic known-value expectations.

### DuckDB Gate

`scripts/duckdb-smoke-test.sh` is the strongest acceptance gate. `emit_duckdb_payloads.rs` emits deterministic payloads and manifest rows.

Add:

- `alp-f32.loom`
- `alp-f64.loom`
- SQL row checks with `COALESCE(CAST(value AS VARCHAR), 'NULL')`.
- Aggregate checks using `COUNT(*)`, `COUNT(value)`, and numeric aggregates that are stable for the chosen decimal values.

### Documentation and Closeout

Follow Phase 9's final-plan pattern:

- Public README docs after gates pass.
- Requirement closeout after final verification only.
- Phase summary artifacts.
- `scripts/mvp0-verify.sh` remains the release gate and includes DuckDB coverage transitively.

