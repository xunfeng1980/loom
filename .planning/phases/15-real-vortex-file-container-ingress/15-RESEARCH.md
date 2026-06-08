# Phase 15 Research: Real Vortex File/Container Ingress

**Status:** Research complete enough to plan
**Date:** 2026-06-08
**Phase:** 15 - Real Vortex File/Container Ingress

## Executive Recommendation

Phase 15 should implement a narrow, fail-closed real Vortex ingress slice before
full `melior`/LLVM/JIT work:

```text
real .vortex file/buffer
  -> isolated Vortex ingress crate/bridge
  -> Loom-owned VortexFileFacts + supported layout/payload facts
  -> existing LMC1/LMP1/LMT1 boundary where supported
  -> existing verifier/decode/oracle gates
```

The first slice should not attempt a complete Vortex file reader, arbitrary
layout lowering, object-store reads, forward-compatibility WASM, native lowering,
or native-speed claims. The purpose is to make Loom face real Vortex
file/container metadata and segment/layout shapes while preserving the core
project invariant: `loom-core` remains Vortex-free.

Recommended implementation shape:

- Add or isolate an ingress-only crate/module, preferably
  `crates/loom-vortex-ingress`, as the only place where `vortex-file` is
  allowed.
- Keep `loom-core`, `loom-ffi`, and the existing verifier/lowering code free of
  `vortex-*` dependencies.
- Replace the old global "`vortex-file` absent from Cargo.lock" invariant with a
  scoped guard: `vortex-file` is allowed only in the ingress/fixture layer.
- First produce stable Loom-owned metadata reports before trying to emit full
  `.loom` payloads from real `.vortex` files.
- Where a real file is supported, produce a container-wrapped `.loom` fixture
  through the existing `LMC1` boundary and verify it using the existing verifier.

## Why Phase 15 Comes Before Full JIT

Phase 14 proved only a tiny verifier-gated textual MLIR lowering slice:
bounded Int32 copy with `VerifiedArtifactFacts`. That is useful, but synthetic.
Real Vortex files introduce the shapes that the production backend must later
consume: file footer, postscript, segment map, root dtype, layout tree, layout
encoding ids, row counts, statistics presence, segment alignment, and lazy scan
behavior.

If full `melior`/LLVM/JIT integration happens before real ingress, the backend
risks optimizing a toy artifact rather than the real boundary. Phase 15 should
therefore stabilize the real artifact evidence that Phase 16 consumes.

## Current Repo Baseline

Existing project invariants:

- `loom-core` has zero Vortex dependencies.
- Vortex ecosystem calls currently live in `crates/loom-fixtures/src/vortex_reader.rs`.
- Existing fixture bridge translates in-memory `vortex-array` objects into
  plain Loom data (`LayoutNode`, `Vec<u8>`, `Option<Vec<bool>>`, Loom-owned FSST
  params).
- Existing release gates still treat `vortex-file` as out of scope:
  `scripts/check-core-invariants.sh` checks `Cargo.lock`, and
  `scripts/mvp0-verify.sh` greps `crates/loom-fixtures` for file-backed Vortex
  tokens.

Phase 15 must update that invariant intentionally. Keeping the old global check
would make real file ingress impossible; relaxing it without a replacement would
weaken the proof boundary. The right replacement is a scoped dependency guard.

## Upstream Findings

### Vortex file format is stable, APIs are not the same kind of stable

Vortex documents the file format as stable since version 0.36.0, with newer
libraries expected to read files written by version 0.36.0 or later. The Rust
library/API is still versioned rapidly; the current repo pins Vortex crates at
0.74.0, and `vortex-file` 0.74.0 declares `rust-version = 1.91.0`.

Planning implication:

- Treat the on-disk format as a real target.
- Treat Rust `vortex-file` APIs as an isolated adapter dependency, not a Loom
  core dependency or long-lived public contract.
- Record exact Vortex crate version and Rust toolchain implications in Phase 15
  plans before implementation.

Sources:

- Vortex file format stability and file layout:
  https://docs.vortex.dev/specs/file-format
- Vortex 0.74.0 crate metadata:
  https://docs.rs/crate/vortex/latest
- `cargo info vortex-file@0.74.0` in this repo: `vortex-file 0.74.0`,
  `rust-version: 1.91.0`, features `object_store`, `tokio`,
  `unstable_encodings`, `zstd`.

### File format structure

The real Vortex file envelope is small:

```text
VTXF magic
segments / padding
postscript data
u16 version
u16 postscript length
VTXF magic
```

The postscript points to four logical regions:

- root `DType`,
- root `Layout`,
- file-level `Statistics`,
- `Footer`.

The footer carries dictionary-encoded array/layout specs, segment specs,
compression specs, and encryption specs. In `vortex-file 0.74.0`, `Footer`
exposes:

- `layout() -> &LayoutRef`,
- `segment_map() -> &Arc<[SegmentSpec]>`,
- `statistics()`,
- `dtype()`,
- `row_count()`,
- `approx_byte_size()`.

`SegmentSpec` has:

- `offset: u64`,
- `length: u32`,
- `alignment: Alignment`,
- `byte_range()`.

Planning implication:

- A useful first Phase 15 deliverable is a deterministic
  `VortexFileFacts` report, independent of whether Loom can yet decode every
  file.
- Facts should include magic/version validation result, row count, root dtype
  summary, root layout summary, segment count/ranges/alignment, stats presence,
  and supported/unsupported classification.

Sources:

- Vortex file format spec:
  https://docs.vortex.dev/specs/file-format
- Vortex serialization internals:
  https://docs.vortex.dev/developer-guide/internals/serialization
- Local inspected source:
  `~/.cargo/registry/src/.../vortex-file-0.74.0/src/footer/mod.rs`
  and `src/footer/segment.rs`.

### Layouts are the real complexity

Vortex docs state that most complexity is encapsulated in layouts. Layouts are
hierarchical, can be serialized/persisted, and are bound to segment sources for
lazy buffer fetch. Built-in layouts include:

- `FlatLayout`,
- `StructLayout`,
- `ChunkedLayout`,
- `DictionaryLayout`,
- `ZonedLayout`.

`vortex-file` documentation notes that a layout alone is not a standalone file:
it is not self-describing without dtype and layout kind metadata. A real file is
the container that combines data segments, dtype/schema, layout flatbuffer,
postscript, footer, and EOF marker.

Planning implication:

- Phase 15 should not promise arbitrary Vortex layout -> Loom layout conversion.
- First supported conversion should be a very small vertical slice:
  one local file, one root dtype shape, one or two layout shapes, and one output
  mode.
- Unknown layout encodings must fail closed with stable diagnostics.

Sources:

- Vortex layouts concept page:
  https://docs.vortex.dev/concepts/layouts
- `vortex-file 0.74.0` crate docs/source inspected locally.

### Vortex file open/scan APIs

`vortex-file 0.74.0` provides:

- `VortexOpenOptions::open(source: Arc<dyn VortexReadAt>)`,
- `VortexOpenOptions::open_path(path)` behind non-wasm path support,
- `VortexOpenOptions::open_buffer(buffer)`,
- `VortexOpenOptions::with_initial_read_size`,
- `VortexOpenOptions::with_file_size`,
- `VortexOpenOptions::with_dtype`,
- `VortexOpenOptions::with_footer`,
- `VortexFile::footer()`,
- `VortexFile::row_count()`,
- `VortexFile::dtype()`,
- `VortexFile::file_stats()`,
- `VortexFile::layout_reader()`,
- `VortexFile::scan()`.

Open reads at least enough bytes to cover `MAX_POSTSCRIPT_SIZE + EOF_SIZE`,
parses magic/version/postscript length, and may request more bytes if dtype,
layout, statistics, or footer segments are not covered by the initial read.

Planning implication:

- `open_buffer` is a good first target for deterministic tests because it avoids
  object-store and async file-system concerns.
- `open_path` can be a CLI-level follow-up once local buffer tests are stable.
- `scan()` can provide an oracle path, but plans must verify whether scan output
  preserves encoded arrays or canonicalizes them before feeding existing
  `vortex_reader::from_array_ref`.

Source:

- Local inspected source:
  `~/.cargo/registry/src/.../vortex-file-0.74.0/src/open.rs`
  and `src/file.rs`.

## Design Options

### Option A: Use Vortex scan as oracle only

Read real `.vortex`, inspect file facts, then use Vortex scan to compare
semantic rows against Loom-generated fixtures.

Pros:

- Fastest path to real file evidence.
- Keeps conversion scope small.
- Good for negative/file-format tests.

Cons:

- Does not prove Loom can ingest real layout structure into its own artifact.
- Easy to accidentally outsource decode semantics to Vortex.

Verdict: Useful as an oracle, not sufficient alone.

### Option B: File facts first, limited `.loom` emission second

Read real `.vortex`, emit stable `VortexFileFacts`, then only for a tightly
supported shape emit `LMC1` with a Loom-owned payload.

Pros:

- Preserves Loom boundary.
- Makes unsupported layouts explicit.
- Produces real evidence for Phase 16 without forcing arbitrary layout support.

Cons:

- Requires careful dependency gate rewrite.
- May need a fixture writer/generator path to create the exact supported real
  files.

Verdict: Recommended.

### Option C: Implement direct parser of Vortex file format in Loom

Parse magic/postscript/footer/layout flatbuffers directly without `vortex-file`.

Pros:

- Strongest independence story.
- Could become long-term reader foundation.

Cons:

- Duplicates upstream file parsing too early.
- Must track FlatBuffer schema and registry behavior.
- High risk for Phase 15 and not needed to validate the boundary.

Verdict: Defer. Consider only after the ingress surface is understood.

### Option D: Add `vortex-file` directly to `loom-core`

Pros:

- Lowest integration friction.

Cons:

- Breaks the most important architectural invariant.
- Pulls unstable upstream Rust API into the verifier/decode core.
- Weakens the final Loom claim that the distribution artifact is Loom-owned and
  independently verifiable.

Verdict: Reject.

## Recommended Phase 15 Scope

### In scope

- A new or clearly isolated ingress bridge that may depend on `vortex-file`.
- Real `.vortex` file or buffer open using Vortex 0.74.0.
- A stable Loom-owned `VortexFileFacts`/`VortexIngressReport` structure.
- Fail-closed diagnostics for:
  - invalid magic,
  - unsupported version,
  - truncated EOF/postscript/footer,
  - missing dtype when required,
  - segment range overflow/out-of-order,
  - unknown or unsupported layout encoding,
  - unsupported root dtype/layout combinations.
- One or two generated real `.vortex` fixtures.
- A narrow supported conversion path into existing `LMC1` where evidence proves
  the scanned/extracted array shape maps to existing Loom layout/table payloads.
- CLI inspection for real Vortex ingress, likely `loom ingest-vortex --inspect`
  or a similarly scoped command.
- Updated invariant scripts:
  - `loom-core` remains Vortex-free,
  - `loom-ffi` remains Vortex-free unless explicitly justified,
  - `vortex-file` allowed only in the ingress/fixture crate,
  - real file API tokens forbidden elsewhere.

### Out of scope

- Full arbitrary Vortex layout -> Loom conversion.
- Remote object store ingress.
- Forward-compatibility WASM decompression.
- Encryption support.
- zstd/unstable encodings unless chosen explicitly for a fixture.
- Production performance claims.
- Native lowering or JIT.
- Compiler/backend correctness proof.
- Replacing the existing `LMC1` container.

## Proposed Artifacts

- `crates/loom-vortex-ingress/` or an equivalent isolated module.
- `VortexIngressReport`:
  - `status`,
  - `diagnostics`,
  - `source_kind`,
  - `vortex_version`,
  - `row_count`,
  - `dtype_summary`,
  - `layout_summary`,
  - `segment_count`,
  - `segment_ranges`,
  - `statistics_present`,
  - `supported_loom_payload`,
  - `unsupported_reason`.
- `VortexIngressDiagnosticCode` with stable strings.
- Real Vortex fixture generator for a tiny supported file.
- CLI command to inspect real Vortex ingress.
- Negative fixture/test suite for malformed files.
- Updated docs and release gate.

## Suggested Plan Split

### 15-01: Ingress contract and dependency boundary

Define `VortexIngressReport`, stable diagnostics, dependency isolation rules,
and update invariant scripts so `vortex-file` is allowed only in the ingress
layer.

### 15-02: Real Vortex open + metadata facts

Use `vortex-file 0.74.0` to open real buffers/files and emit deterministic
metadata facts. Add negative tests for malformed/truncated files.

### 15-03: Supported real fixture + `.loom` emission

Generate at least one real `.vortex` file whose scanned/extracted shape can be
translated to existing Loom payloads. Emit `LMC1` where supported and compare
Loom output against Vortex oracle rows.

### 15-04: CLI/docs/release gate

Expose ingress inspection in CLI, document the narrow scope, wire the gate into
`scripts/mvp0-verify.sh`, and keep Phase 16+ placeholders clear.

## Key Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| `vortex-file` requires Rust 1.91.0 while repo toolchain may differ | Build failure | Verify `rust-toolchain.toml` and decide whether Phase 15 upgrades toolchain or uses a feature-gated/non-default crate |
| Old global no-`vortex-file` guard conflicts with real ingress | Gate failure | Replace with scoped allowlist guard |
| Vortex scan canonicalizes arrays before `vortex_reader::from_array_ref` can see encoded layout | Ingress becomes oracle-only | Make metadata facts the first deliverable; verify scan/extract behavior before promising `.loom` emission |
| Arbitrary layouts explode scope | Phase stalls | Support only explicit dtype/layout fixtures; unknown layouts fail closed |
| Upstream Rust API changes | Future maintenance risk | Keep API calls isolated in one ingress bridge; expose only Loom-owned facts |
| Pulling Vortex into core breaks proof story | Architectural regression | Guard `loom-core` and `loom-ffi` dependency trees in CI |

## Open Questions for Planning

1. Should Phase 15 introduce a new crate (`loom-vortex-ingress`) or extend
   `loom-fixtures`? Recommendation: new crate, because it makes the
   `vortex-file` allowlist clean.
2. Should `rust-toolchain.toml` move to Rust 1.91.0 for Vortex 0.74.0
   `vortex-file`, or should the ingress crate be optional/non-default until the
   toolchain decision is made?
3. Which first real fixture should be supported?
   Recommendation: a local in-memory/buffer file with a root primitive or small
   struct and a layout shape that can be classified deterministically.
4. Should CLI support path reads in Phase 15, or only inspect generated fixture
   buffers first?
   Recommendation: include path reads only after buffer tests pass.
5. Should `.loom` emission be required in Phase 15?
   Recommendation: yes, but only for one proven supported slice; all other real
   files should still get facts + unsupported diagnostics.

## Research Conclusion

Phase 15 should be a real-ingress boundary phase, not a broad Vortex reader
phase. The valuable proof is:

- Loom can safely touch real `.vortex` file/container structure.
- Vortex file/layout/segment facts can be turned into stable Loom-owned facts.
- Supported shapes can enter the existing `LMC1` verifier/decode path.
- Unsupported shapes fail closed with reviewer-visible diagnostics.
- The dependency boundary remains honest: only the ingress layer depends on
  Vortex file APIs; the core verifier/interpreter remains independent.

This creates the right evidence for Phase 16: a full native backend can target
real artifact shapes instead of only the Phase 14 synthetic copy slice.
