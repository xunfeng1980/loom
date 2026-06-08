# Phase 11: Distribution Container v0 - Discussion Log

**Date:** 2026-06-08
**Mode:** Default-mode continuation using recommended research path

## Context

After Phase 10, Loom has a working MVP0/v2 proof chain across L1 layouts, FSST and ALP L2 kernels, CLI inspection, structural verifier, FFI, DuckDB SQL, and the release gate. The remaining gap against the final Loom goal is that generated `.loom` files are still raw internal `LMP1` or `LMT1` fixture payloads, not a versioned distribution artifact.

The user selected the future sequence:

1. Phase 11: Distribution Container v0
2. Phase 12: Formal verifier / safety proof MVP
3. Phase 13: MLIR/native lowering spike
4. Phase 14: Real Vortex file/container ingress

The user also requested Phase 12/13/14 stay as roadmap placeholders only.

## Recommended Decisions Applied

| Decision | Choice | Rationale |
|---|---|---|
| Container magic | `LMC1` | Avoid overloading `LMP1`/`LMT1`; make the top-level distribution artifact visible. |
| Compatibility | Keep raw `LMP1`/`LMT1` accepted | Phase 11 should not break the completed MVP0 gate or existing descriptor workflows. |
| v0 payload strategy | Wrap existing `LMP1`/`LMT1` as sections | Gives a real container boundary without rewriting all payload codecs. |
| Feature handling | Required vs optional bitsets | Unknown required features fail closed; optional/debug sections can be skipped when unreferenced. |
| Section directory | Checked offsets/lengths | Takes the Arrow/Parquet/Vortex/Wasm lesson: metadata should guide parsing, not ad hoc scanning. |
| Trailer | Reserve optional trailing `LMC1` magic | Keeps future random-access/file-shape evolution open while allowing a simple v0 front directory. |
| Scope fence | No formal proof, MLIR, real Vortex file ingress, URI/signature/attestation | Those are Phase 12-14 or later, not Phase 11. |

## Planning Implication

Phase 11 should be executable in four plans:

1. Core `LMC1` container codec and feature model.
2. Rust decode/verifier/FFI routing for container-wrapped payloads.
3. CLI, fixture emitter, DuckDB extension, and SQL smoke support for container-wrapped fixtures.
4. Documentation, negative container gate, final release verification, and Phase 11 closure.

