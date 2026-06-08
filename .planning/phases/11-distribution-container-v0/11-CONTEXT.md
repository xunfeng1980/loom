# Phase 11 Context: Distribution Container v0

## Objective

Introduce the first explicit Loom distribution container boundary so the project moves beyond internal `LMP1`/`LMT1` fixture payloads toward a versioned artifact that can travel with data.

## Recommended Scope

- Add `LMC1` container v0 with magic/version/features/section directory.
- Wrap existing `LMP1` and `LMT1` payloads as container sections.
- Add feature negotiation with fail-closed unknown required features.
- Extend verifier and CLI to understand the container boundary.
- Keep raw payload compatibility.
- Update fixtures, docs, and release gate.

## Deferred By Design

- Phase 12: formal verifier / safety proof MVP.
- Phase 13: MLIR/native lowering spike.
- Phase 14: real Vortex file/container ingress.
- Remote URI fetch, signatures, encryption, attestation, and content-addressed distribution.
- New kernels or performance claims.

## Research Inputs

- Arrow IPC file format: magic at both ends, footer offsets/sizes for random access.
- Parquet file format: footer metadata and `PAR1` identity.
- Vortex file format: `VTXF`, postscript, segment directory, compatibility story.
- WebAssembly binary format: length-delimited sections and skippable custom sections.
- Substrait serialization: binary transport plus text debugging surface.

See `11-RESEARCH.md` for analysis and source links.

