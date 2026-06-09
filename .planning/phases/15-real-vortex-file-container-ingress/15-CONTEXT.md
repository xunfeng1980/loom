# Phase 15 Context: Real Vortex File/Container Ingress

**Status:** Ready for planning
**Date:** 2026-06-08
**Inputs:** `15-RESEARCH.md`, Phase 11 `LMC1` container, Phase 13 verifier
facts boundary, Phase 14 lowering handoff

## Phase Intent

Phase 15 makes Loom touch real Vortex file/container structure for the first
time. The goal is not a complete Vortex reader. The goal is a narrow,
reviewable ingress boundary:

```text
real .vortex file/buffer
  -> isolated Vortex ingress bridge
  -> Loom-owned ingress facts and diagnostics
  -> existing LMC1/LMP1/LMT1 verifier/decode boundary where supported
```

This phase exists before full `melior`/LLVM/JIT work so Phase 16 has real
artifact/layout evidence instead of only the Phase 14 synthetic Int32 copy
slice.

## Locked Research Decisions

### D-15-01: Isolate real Vortex file APIs

`vortex-file` may be introduced only in an ingress layer, preferably
`ingress/loom-vortex-ingress`. `loom-core` and `loom-ffi` must remain free of
`vortex-*` dependencies. `loom-cli` may call the ingress crate but must not
depend on `vortex-file` directly.

### D-15-02: Replace the old global no-vortex-file guard

The old Phase 1 guard that rejects any `vortex-file` entry in `Cargo.lock`
conflicts with Phase 15. Replace it with a scoped guard:

- `loom-core` has no Vortex/FastLanes dependency.
- `loom-ffi` has no Vortex/FastLanes dependency.
- only `ingress/loom-vortex-ingress/Cargo.toml` may directly name
  `vortex-file`.
- file-backed Vortex API tokens are forbidden outside the ingress layer and
  CLI command wiring.

### D-15-03: Facts first, conversion second

The first stable output is a Loom-owned `VortexIngressReport` /
`VortexFileFacts` structure with stable diagnostics. A file can be inspected and
classified even when it cannot be converted to `.loom`.

### D-15-04: Supported conversion slice is intentionally small

The first supported `.vortex` -> `LMC1` path should cover one deterministic
local real-file fixture, likely a primitive Int32 or tiny struct shape that can
be translated into existing Loom layout/table payloads. Unknown layouts,
unsupported dtypes, unsupported validity, and unsupported segment/layout
relationships fail closed.

### D-15-05: Vortex scan is oracle evidence, not the implementation

Vortex scan may be used to create or compare oracle rows. It must not become a
silent replacement for Loom's verifier/decode path. Supported conversion must
emit an existing Loom-owned payload and pass the existing verifier before Loom
decode.

### D-15-06: No native backend scope

Phase 15 does not add MLIR lowering, `melior`, LLVM, JIT, vectorization,
native-speed claims, object-store ingress, encryption, signatures, attestation,
or arbitrary Vortex layout support.

## Required Phase Outputs

- `15-INGRESS-CONTRACT.md` describing scope, dependency boundary, report schema,
  diagnostics, supported conversion slice, and exclusions.
- Isolated ingress crate or module with stable report/diagnostic types.
- Scoped dependency/file API guard replacing the old global `vortex-file`
  absence check.
- Real Vortex open/inspect support for in-memory buffers and local paths.
- Fail-closed negative tests for malformed/truncated real file inputs.
- At least one generated real `.vortex` fixture that can be converted into an
  existing `LMC1` payload and decoded by Loom.
- Vortex oracle comparison for the supported real fixture.
- CLI inspection for real Vortex ingress.
- Focused `scripts/vortex-ingress-test.sh` gate wired into
  `scripts/mvp0-verify.sh`.

## Non-Goals

- No complete `.vortex` file reader.
- No arbitrary layout lowering.
- No object-store reads.
- No forward-compatibility WASM.
- No encrypted/compressed/zstd/unstable encoding support unless explicitly
  chosen for a supported fixture.
- No native lowering/JIT.
- No replacement of the `LMC1` container boundary.
- No correctness claim beyond safety/fail-closed behavior and oracle equality
  for supported fixtures.

## Requirement Mapping

- `INGEST-01`: isolated real Vortex ingress dependency boundary.
- `INGEST-02`: stable ingress report/facts and diagnostics.
- `INGEST-03`: real `.vortex` open/inspect with malformed-input rejection.
- `INGEST-04`: one supported real `.vortex` -> `LMC1` conversion slice with
  verifier and oracle evidence.
- `INGEST-05`: CLI/docs/release gate for the narrow ingress scope.
