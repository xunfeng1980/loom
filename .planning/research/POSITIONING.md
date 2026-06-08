# Loom Positioning Against Vortex, AnyBlox, and F3

**Status:** Research note
**Scope:** Conceptual positioning for Loom after MVP0
**Last updated:** 2026-06-08

## Summary

Loom is not trying to replace Vortex, AnyBlox, or F3 directly. It sits at a narrower layer: a portable, verifiable decoder contract whose only output is well-formed Arrow.

- **Vortex** is the concrete columnar format and compressed-array ecosystem used by MVP0 as the target input and oracle.
- **AnyBlox** is the closest conceptual predecessor for "decoder travels with the data", but uses general WebAssembly decoders.
- **F3** is a future-proof file format proposal that embeds WebAssembly decoders in files to preserve compatibility and extensibility.
- **Loom** takes the self-decoding idea but deliberately rejects general computation: most decoding is L1 declarative layout data, and the remaining compute is L2 total-function kernels.

The core distinction is therefore not "who can ship a decoder with data"; AnyBlox and F3 already do that. The distinction is that Loom tries to make the shipped decoder small enough, structured enough, and output-constrained enough to verify cheaply and preserve for decades.

## Comparison Matrix

| System | Primary abstraction | Decoder distribution | Safety boundary | Termination model | Output contract | Main tradeoff |
|---|---|---|---|---|---|---|
| Vortex | Extensible columnar file format plus compressed array model | Native library / format implementation | Trust the native Vortex implementation in the host process | Native implementation discipline | Vortex canonical arrays / Arrow conversion | Excellent concrete format, but not a target-neutral decoder IR |
| AnyBlox | Self-decoding datasets | Lightweight WebAssembly decoders bundled with or referenced by the data | Wasm sandbox, runtime limits, decoder hash/URI mechanism | General Wasm, typically controlled by fuel or runtime limits | Decoder-defined integration API; not inherently Arrow-only | Very flexible, but inherits the verification burden of general computation |
| F3 | Future-proof open-source file format | File includes data, metadata, and Wasm decoder binaries, with native decoders when available | Wasm/native decoder boundary | General Wasm for portable fallback | F3 file-format API | Strong file-format story, but still centered on a new format and general decoder binaries |
| Loom | Distribution-oriented decoder IR | Versioned decoder artifact travels with data or is referenced by content hash | Loom verifier plus minimal host capabilities | Total-function L2 kernels; L1 is declarative data | Mandatory well-formed Arrow via typed builders | Narrower and less expressive by design, in exchange for static verifiability and stable semantics |

## Relationship to Vortex

Vortex is the concrete data-format ecosystem MVP0 uses to make Loom falsifiable. The project builds real in-memory Vortex arrays, extracts Loom-owned layout descriptions, decodes them through `loom-core`, and compares the results against Vortex's own canonical decode path.

That makes Vortex a target and an oracle, not a competitor in MVP0.

Where Vortex owns the full file and array model, Loom is trying to own a different boundary: a durable description of how bytes become Arrow that could, in principle, describe Vortex, Parquet, ROOT, or custom encodings without requiring every engine to embed each format's native reader.

Practical implication:

- MVP0 should continue using Vortex as the reference source for correctness checks.
- Future Loom work should avoid depending on Vortex file/container internals unless the milestone explicitly targets full `.vortex` files.
- A stronger v2 milestone would define a human-readable Loom layout descriptor independent of Vortex APIs, then keep the Vortex bridge as one producer of that descriptor.

## Relationship to AnyBlox

AnyBlox and Loom share the same pressure point: storage formats evolve faster than engines adopt new readers. Both answer by moving decoder logic closer to the data.

The difference is the execution model.

AnyBlox uses WebAssembly decoders. This gives it broad expressiveness and a practical sandbox story. Loom intentionally gives up that expressiveness. It splits decoding into:

- **L1 layout:** declarative data such as bit-packing, FOR, dictionary, RLE, offsets, counts, and validity routing.
- **L2 kernels:** total functions for the parts that genuinely require computation, such as FSST or future ALP-style kernels.

This is the main argument Loom needs to defend: if most decoder logic is structural, a general Wasm decoder is more expressive than necessary. Loom's bet is that a constrained decoder IR can be safer, more stable, and easier to optimize once inside the engine.

Practical implication:

- Loom should explicitly acknowledge AnyBlox as prior art for self-decoding datasets.
- Loom's positioning should not claim novelty for "decoder travels with data" alone.
- The novelty claim should be: self-decoding with a non-general, Arrow-only, statically verifiable decoder representation.

## Relationship to F3

F3 is closer to a complete next-generation file format. It packages data, metadata, and decoder logic together, with WebAssembly decoders available as the compatibility path when native decoders are unavailable.

Loom is intentionally less of a file format and more of a decoder contract. A future Loom distribution container may include schema, L1 layout, L2 kernel modules, feature flags, and optional optimized tiers, but the current design does not require Loom to own the entire physical file organization.

The distinction:

- F3 asks: what should a future-proof open-source file format look like?
- Loom asks: what is the smallest durable representation of decoding logic that an engine can safely accept from data?

Practical implication:

- If the project evolves toward a full storage format, F3 becomes a direct architectural reference and comparison target.
- If the project remains a decoder IR, F3 should be treated as adjacent prior art rather than a direct competitor.
- Loom should avoid prematurely designing a whole file format until the decoder contract, verifier boundary, and Arrow output semantics are convincing.

## Design Claims Loom Should Keep

1. **Do not claim self-decoding as unique.** AnyBlox and F3 already establish that direction.
2. **Claim narrower semantics as the differentiator.** Loom is deliberately not general Wasm.
3. **Keep Arrow mandatory.** This is a major simplification: output validity can be enforced by typed builder events rather than arbitrary memory writes.
4. **Keep Vortex isolated.** Vortex should remain an input/oracle bridge for MVP0 and early v2, not leak into `loom-core`.
5. **Delay full file-format work.** A distribution container is useful later, but designing a full storage format now would dilute the current thesis.

## Recommended Next Documentation Updates

- Add a short link from `README.md` section 12 to this note.
- Add a Chinese version or a bilingual summary if the README-zh track remains first-class.
- When v2 planning starts, convert the "Practical implication" bullets into concrete requirements:
  - human-readable L1 descriptor;
  - CLI decoder/inspector;
  - multi-column Arrow output;
  - explicit verifier boundary;
  - optional timing comparison against Vortex native decode.

## References

- Vortex project page: <https://spiraldb.com/vortex>
- Vortex documentation references: <https://docs.vortex.dev/references>
- AnyBlox: A Framework for Self-Decoding Datasets: <https://www.vldb.org/pvldb/vol18/p4017-gienieczko.pdf>
- AnyBlox publication record: <https://portal.fis.tum.de/en/publications/anyblox-a-framework-for-self-decoding-datasets/>
- F3: The Open-Source Data File Format for the Future: <https://dl.acm.org/doi/10.1145/3749163>
