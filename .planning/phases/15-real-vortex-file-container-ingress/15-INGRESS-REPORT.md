# Phase 15 Ingress Report

## Scope

Phase 15 adds a narrow real Vortex file/container ingress boundary. It is not a complete Vortex reader.

Implemented scope:

- `loom-vortex-ingress` is the isolated crate that may directly depend on `vortex-file`.
- `loom-core` and `loom-ffi` remain free of `vortex-*` and FastLanes dependencies.
- Real Vortex buffers and local paths can be inspected into Loom-owned facts.
- Malformed input fails closed with stable diagnostics and no facts.
- One supported slice emits `LMC1`: generated real Vortex files that scan to non-null `Int32` rows.
- Unsupported real files return `VortexIngressReport` diagnostics and no partial `.loom` bytes.
- `loom ingest-vortex --inspect` and `loom ingest-vortex --emit-loom` expose the behavior without adding a direct CLI dependency on `vortex-file`.

Deferred scope:

- Arbitrary Vortex layouts, nullable real-file conversion, multi-column real-file conversion, object-store ingress, remote credentials, native lowering, and production-speed claims.
- Full `melior`/LLVM/JIT integration remains Phase 16.

## Dependency Boundary

Direct `vortex-file` usage is allowed only in `crates/loom-vortex-ingress`. The release guards now check:

- `cargo tree -p loom-core` has zero Vortex/FastLanes dependencies.
- `cargo tree -p loom-ffi` has zero Vortex/FastLanes dependencies.
- Direct `vortex-file` references are allowlisted to `crates/loom-vortex-ingress/Cargo.toml`.
- `crates/loom-fixtures` still cannot use file-backed Vortex APIs.

## Report Fields

`VortexIngressReport` records:

- `status`: `accepted`, `unsupported`, or `rejected`
- `facts`: optional `VortexFileFacts`
- `diagnostics`: stable code/path/message records

`VortexFileFacts` records source kind, Vortex file version, row count, dtype summary, layout summary, segment count, segment byte ranges, alignment summary, statistics presence, approximate footer size, and whether a supported Loom payload can be emitted.

## Stable Diagnostics

Current stable codes:

- `INGRESS_OPEN_FAILED`
- `INGRESS_UNSUPPORTED_LAYOUT`
- `INGRESS_UNSUPPORTED_DTYPE`
- `INGRESS_UNSUPPORTED_CONVERSION`
- `INGRESS_NOT_YET_INSPECTED` remains reserved for compatibility with the initial 15-01 skeleton shape.

## Fixture Evidence

`cargo run -p loom-vortex-ingress --bin emit_vortex_ingress_fixtures` generates:

- `fixtures/vortex/int32-flat.vortex`
- `fixtures/loom/int32-flat.loom`

The generated real Vortex fixture currently reports:

- status: `accepted`
- row_count: `4`
- dtype: `Primitive(I32, NonNullable)`
- layout: `vortex.stats`
- segments: `2`
- supported_loom_payload: `true`

## Oracle Comparison

`crates/loom-vortex-ingress/tests/real_file_to_loom.rs` proves the supported slice:

1. Generate a real Vortex file in memory.
2. Scan it through Vortex to Loom-owned `Vec<i32>` oracle rows.
3. Emit `LMC1` through `emit_supported_lmc1_from_vortex_buffer`.
4. Verify the emitted container with `verify_container`.
5. Decode the container through `loom-core`.
6. Compare Loom decoded rows against Vortex scan rows.

The same test file confirms an unsupported real `Int64` Vortex file fails closed with `INGRESS_UNSUPPORTED_CONVERSION` and no emitted bytes.

## CLI Behavior

Supported commands:

```bash
cargo run --bin loom -- ingest-vortex --inspect fixtures/vortex/int32-flat.vortex
cargo run --bin loom -- ingest-vortex --emit-loom fixtures/vortex/int32-flat.vortex /tmp/int32-flat.loom
```

The CLI prints status, diagnostics, row count, dtype summary, layout summary, segment ranges, statistics presence, and `supported_loom_payload`.

## Verification

Focused verification:

```bash
cargo test -p loom-vortex-ingress
bash scripts/vortex-ingress-test.sh
cargo run -p loom-cli -- ingest-vortex --inspect fixtures/vortex/int32-flat.vortex
```

Final release verification:

```bash
cargo test --workspace
bash scripts/vortex-ingress-test.sh
bash scripts/check-core-invariants.sh
bash scripts/mvp0-verify.sh
git diff --check
```

## Follow-Ups

- Phase 16 should consume the real-ingress evidence but keep the backend verifier-gated and optional.
- Future real-ingress expansion should add explicit support predicates before emitting additional `.loom` payload shapes.
- Nullable and multi-column real-file ingress should remain fail-closed until they have oracle equality tests and verifier/decode coverage.
