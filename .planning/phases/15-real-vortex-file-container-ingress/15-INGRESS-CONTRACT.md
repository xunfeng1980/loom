# Phase 15 Ingress Contract

## Scope

Phase 15 introduces a narrow real Vortex file/container ingress boundary. The
boundary may inspect real `.vortex` buffers and local paths, produce stable
Loom-owned facts, and emit an existing `LMC1` payload only for explicitly
supported shapes.

The phase is not a complete Vortex reader. Unsupported real files must still be
inspectable enough to explain why they are unsupported.

## Dependency Boundary

`vortex-file` is allowed only in `ingress/loom-vortex-ingress`. The ingress crate
is an adapter that translates Vortex APIs into Loom-owned data. It may depend on
`loom-core`, but `loom-core` must never depend on it.

Required guard policy:

- `loom-core` has zero `vortex-*` or `fastlanes` dependency entries.
- `loom-ffi` has zero `vortex-*` or `fastlanes` dependency entries.
- only `ingress/loom-vortex-ingress/Cargo.toml` may directly name
  `vortex-file`.
- file-backed Vortex APIs are forbidden outside the ingress crate, except for
  narrow CLI command wiring that calls the ingress crate.

## Ingress Report Schema

`VortexIngressReport` is the stable public report type. It contains:

- `status`: `accepted`, `unsupported`, or `rejected`,
- `facts`: optional `VortexFileFacts`,
- `diagnostics`: stable code/path/message triples.

`VortexFileFacts` records:

- source kind,
- Vortex file version,
- row count,
- dtype summary,
- layout summary,
- segment count,
- segment byte ranges,
- alignment summary,
- statistics presence,
- approximate footer byte size,
- whether a supported Loom payload can be emitted.

No public report field may expose a Vortex type.

## Stable Diagnostics

Diagnostics are stable strings. Initial codes include:

- `INGRESS_NOT_YET_INSPECTED`
- `INGRESS_OPEN_FAILED`
- `INGRESS_UNSUPPORTED_LAYOUT`
- `INGRESS_UNSUPPORTED_DTYPE`
- `INGRESS_UNSUPPORTED_CONVERSION`

Malformed, truncated, or unsupported input returns diagnostics instead of
panicking or emitting partial `.loom` output.

## Supported Conversion Slice

The first conversion slice is intentionally small: one generated real `.vortex`
fixture whose shape can be translated into the existing `LMP1` or `LMT1` payload
surface without adding new `loom-core` layout variants.

Unknown layouts, unsupported dtypes, unsupported validity, unsupported segments,
or unsupported multi-column relationships fail closed with an unsupported
diagnostic.

## Verifier Routing

When a supported real Vortex file emits a Loom payload, that payload must use
the existing `LMC1` container boundary and pass `verify_container` before Loom
decode. The ingress bridge cannot bypass the existing verifier.

## Oracle Use

Vortex scan may be used as oracle evidence for generated real fixtures. It must
not become the implementation path for Loom decode. The supported output path is
real `.vortex` -> ingress facts -> `LMC1` -> Loom verifier -> Loom decode.

## Non-Goals

- No arbitrary Vortex layout lowering.
- No object-store ingress.
- No forward-compatibility WASM.
- No encryption/signature/attestation support.
- No zstd or unstable encoding support unless explicitly selected later.
- No MLIR lowering, `melior`, LLVM, JIT, vectorization, or native-speed claim.
- No replacement of `LMC1`.
- No correctness claim beyond fail-closed behavior and oracle equality for
  supported fixtures.
