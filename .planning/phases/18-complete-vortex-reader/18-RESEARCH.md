# Phase 18 Research: Complete Vortex Reader

**Status:** Research started
**Date:** 2026-06-08
**Phase:** 18 - Complete Vortex Reader

## Executive Recommendation

Phase 18 should expand Phase 15's narrow real-ingress slice into a complete,
isolated, fail-closed Vortex reader boundary before solver-backed verifier work
and production native expansion:

```text
real .vortex file/buffer/path
  -> isolated loom-vortex-ingress reader
  -> complete Loom-owned VortexReaderFacts
  -> explicit support predicate per dtype/layout/encoding shape
  -> LMC1/LMT1 emission only for supported shapes
  -> artifact verifier pipeline from Phase 17
  -> oracle/equivalence evidence against Vortex scan
```

The recommended Phase 18 goal is not to make `loom-core` depend on Vortex and
not to hand all decode work to Vortex scan. The goal is to define and implement
the complete reader boundary: real file facts, schema/layout/segment traversal,
projection and chunk decisions, supported conversion matrix, stable diagnostics,
and fail-closed unsupported cases.

## Why Phase 18 Comes Before Phase 19

Phase 19 is now reserved for the solver-backed full artifact verifier. That
verifier should target real complete-reader facts, not only the synthetic
Phase 13/14/16 bounded Int32 copy slice or the Phase 15 one-file ingress slice.

Phase 18 must therefore provide:

- stable dtype/schema facts;
- stable layout-tree facts;
- stable segment/range/alignment facts;
- complete-reader unsupported diagnostics;
- a supported conversion matrix that can be promoted into proof obligations;
- `LMC1` / `LMT1` artifacts that the Phase 17 artifact verifier can accept.

Without these, Phase 19 would discharge obligations for a toy reader rather than
for real Vortex file/container structure.

## Upstream Findings

### File format stability and shape

Vortex documents the file format as stable since version `0.36.0`. The file
has a small envelope with leading/trailing `VTXF` magic, binary segments,
postscript data, a version tag, and postscript length. The postscript points to
logical regions for root dtype, root layout, file statistics, and footer.

The footer contains dictionary-encoded array specs, layout specs, segment specs,
compression specs, and encryption specs. Segment specs include offset, length,
and alignment exponent.

Sources:

- Vortex file format: https://docs.vortex.dev/specs/file-format
- Vortex serialization internals: https://docs.vortex.dev/developer-guide/internals/serialization.html

### Layouts carry most of the complexity

The Vortex docs describe layouts as the out-of-memory equivalent of arrays:
hierarchical objects with metadata, dtype, children, and lazy buffers called
segments. A Vortex file is essentially a serialized layout tree plus data
segments in the same file.

Built-in layouts include flat, struct, chunked, dictionary, and zoned layout
families. Local `vortex-layout 0.74.0` source exposes traversal-oriented APIs:

- `Layout::encoding()`
- `Layout::encoding_id()`
- `Layout::row_count()`
- `Layout::dtype()`
- `Layout::nchildren()`
- `Layout::child(idx)`
- `Layout::child_type(idx)`
- `Layout::segment_ids()`
- `Layout::depth_first_traversal()`
- `Layout::display_tree()`

Sources:

- Vortex layouts concept page: https://docs.vortex.dev/concepts/layouts
- Local source:
  `~/.cargo/registry/src/.../vortex-layout-0.74.0/src/layout.rs`

### `vortex-file 0.74.0` reader APIs

Local source confirms the existing ingress crate can keep using the current
Vortex APIs:

- `VortexOpenOptions::open_buffer`
- `VortexOpenOptions::open_path`
- `VortexOpenOptions::with_initial_read_size`
- `VortexOpenOptions::with_file_size`
- `VortexOpenOptions::with_dtype`
- `VortexOpenOptions::with_footer`
- `VortexFile::footer`
- `VortexFile::row_count`
- `VortexFile::dtype`
- `VortexFile::file_stats`
- `VortexFile::layout_reader`
- `VortexFile::scan`
- `VortexFile::can_prune`
- `VortexFile::splits`

`Footer` exposes:

- `layout()`
- `segment_map()`
- `statistics()`
- `dtype()`
- `approx_byte_size()`
- `row_count()`

`SegmentSpec` exposes offset, length, alignment, and `byte_range()`.

Sources:

- `vortex_file` crate docs: https://docs.rs/vortex-file/latest/vortex_file/
- Local source:
  `~/.cargo/registry/src/.../vortex-file-0.74.0/src/file.rs`
- Local source:
  `~/.cargo/registry/src/.../vortex-file-0.74.0/src/open.rs`
- Local source:
  `~/.cargo/registry/src/.../vortex-file-0.74.0/src/footer/mod.rs`
- Local source:
  `~/.cargo/registry/src/.../vortex-file-0.74.0/src/footer/segment.rs`

## Current Repository Baseline

Phase 15 already shipped:

- isolated `crates/loom-vortex-ingress`;
- scoped `vortex-file` allowlist;
- `VortexIngressReport` and `VortexFileFacts`;
- buffer/path inspection;
- malformed input rejection;
- one supported non-null Int32 `.vortex` -> `LMC1` conversion;
- Vortex scan oracle comparison for that one supported slice;
- CLI `loom ingest-vortex --inspect` and `loom ingest-vortex --emit-loom`;
- `scripts/vortex-ingress-test.sh` in the release gate.

Phase 17 already shipped:

- unified `verify_artifact`;
- optional `verify_artifact_with_l2_core`;
- facts fusion;
- lowering-readiness reporting;
- artifact verifier CLI/gate.

Phase 18 should build on both: complete the reader boundary in
`loom-vortex-ingress`, then route emitted artifacts through the Phase 17
artifact verifier.

## Definition of "Complete Vortex Reader" for Phase 18

For this project, "complete reader" should mean complete reader boundary, not
complete native-speed execution of every Vortex encoding.

Phase 18 should deliver:

1. Complete file/container fact extraction for real buffers and paths.
2. Recursive layout-tree facts for all layout nodes the upstream reader can
   open.
3. Segment-range, alignment, and overlap checks owned by Loom.
4. Stable dtype/schema facts including primitive, bool, utf8/binary, struct,
   list, fixed-size-list, decimal, extension, and nullable flags where visible.
5. A support matrix that classifies each root shape as accepted, unsupported,
   or rejected.
6. `LMC1` single-column and `LMT1` multi-column emission for explicitly
   supported reader shapes.
7. Vortex scan oracle evidence for every emitted shape.
8. Fail-closed diagnostics for every unsupported or malformed case.
9. Dependency guards proving `loom-core` and `loom-ffi` remain Vortex-free.

This is enough for Phase 19 to consume complete-reader facts as solver inputs.

## Recommended Reader Facts

Introduce or evolve the Phase 15 facts into `VortexReaderFacts`:

- source kind: buffer/path;
- Vortex file version;
- file size where known;
- row count;
- root dtype tree summary;
- root layout encoding id;
- recursive layout tree:
  - node path;
  - encoding id;
  - dtype summary;
  - row count;
  - child count;
  - child type/name;
  - child row offset;
  - segment IDs;
  - metadata byte length;
- segment map:
  - segment id;
  - byte range;
  - length;
  - alignment;
  - overlap/order classification;
- statistics presence and field-stat summary;
- split ranges from `VortexFile::splits()` where available;
- scan oracle availability;
- supported Loom emission kind: none, `LMP1`, `LMT1`;
- artifact verifier status for emitted bytes.

These facts should be plain Loom-owned Rust structs with stable CLI rendering,
not Vortex types crossing into `loom-core`.

## Recommended Support Matrix

Phase 18 should not try to support every Vortex encoding in one step. It should
make support explicit:

| Shape | Recommended status | Reason |
|---|---|---|
| Root non-null primitive Int32 flat/chunked | Accept | Extends Phase 15 and matches existing `LMP1` decode |
| Root nullable primitive Int32 | Accept after validity extraction support | Required to escape the non-null-only limitation |
| Root primitive Int64 | Accept if existing Raw Int64 path can be emitted and verified | Existing Loom builder can decode Int64 |
| Root Bool | Accept if existing Boolean table/layout path can be emitted and verified | Existing Phase 4/8 support exists |
| Root Float32/Float64 | Accept if emitted through existing ALP/raw-compatible path | Existing Phase 10 support exists, but real Vortex encoding mapping must be explicit |
| Struct of supported primitive columns | Accept to `LMT1` | Required for real table-shaped reader evidence |
| Utf8/Binary | Unsupported unless FSST/raw string extraction is explicit | Avoid silently outsourcing string decode to Vortex scan |
| Dictionary/RLE/FOR/Bitpack encoded layouts | Accept only when mapped to existing Loom `LayoutNode` variants | Must prove physical-layout extraction, not just semantic scan |
| Chunked layouts | Accept only if each chunk has compatible dtype/layout and row ranges compose | Needed for realistic files |
| Zoned layouts/statistics pruning | Inspect facts first, unsupported for emission initially | Useful for Phase 19/engine facts, not first emission target |
| Extension/Decimal/List/Struct nesting beyond table columns | Unsupported initially | Avoid widening into arbitrary Arrow semantics too early |
| Object-store/remote reads | Defer | Phase 18 should stabilize local reader facts first |
| Encrypted/forward-compatible WASM files | Defer | Outside current trust model |

## Architecture Options

### Option A: Scan-only complete reader

Use `VortexFile::scan()` to materialize arrays and convert scan results to Loom
payloads.

Pros:

- fastest route to many semantic row comparisons;
- good oracle evidence;
- avoids direct per-encoding extraction.

Cons:

- does not prove Loom owns decode semantics;
- risks turning Vortex scan into the implementation;
- weak input for Phase 19 solver obligations.

Verdict: keep scan as oracle only.

### Option B: Facts-first complete reader with explicit conversion matrix

Traverse file/footer/layout/dtype/segments into Loom-owned facts, then emit Loom
artifacts only for support-matrix shapes.

Pros:

- best match for Phase 19 solver inputs;
- preserves `loom-core` isolation;
- makes unsupported cases reviewer-visible;
- scales from narrow support to broader support.

Cons:

- more planning and diagnostics work;
- requires careful per-layout extraction.

Verdict: recommended.

### Option C: Direct Vortex flatbuffer parser in Loom

Parse Vortex file/postscript/footer/layout flatbuffers without upstream
`vortex-file`.

Pros:

- strongest independent-reader story.

Cons:

- duplicates upstream parsing too early;
- high maintenance risk while Rust APIs and schemas evolve;
- distracts from Loom's own distribution IR boundary.

Verdict: defer until the upstream-assisted boundary is fully understood.

## Suggested Phase 18 Plan Split

### 18-01: Reader facts contract and dependency boundary

Define `VortexReaderFacts`, recursive layout facts, support matrix status,
diagnostics, and release-gate invariants. Preserve `loom-core`/`loom-ffi`
Vortex-free boundaries.

### 18-02: Recursive layout/dtype/segment inspection

Extend `loom-vortex-ingress` inspection from flat summary strings to stable
recursive facts over dtype, layout, child types, segment IDs, ranges, alignment,
stats, and split ranges.

### 18-03: Supported single-column conversion matrix

Expand supported `.vortex` -> `LMC1` emission beyond non-null Int32. Recommended
first set: nullable Int32, Int64, Bool, and one Float32/Float64 shape where the
mapping is explicit and verifier-backed.

### 18-04: Supported struct/table conversion

Emit `LMT1` for real Vortex struct/table files whose fields are all supported
single-column shapes. Verify emitted artifacts through Phase 17 and compare
rows/aggregates against Vortex scan oracle.

### 18-05: CLI/report/release-gate closeout

Expose complete-reader facts and support status in CLI, add malformed and
unsupported fixture coverage, update README/ROADMAP/STATE/PROJECT, and wire a
Phase 18 gate into `scripts/mvp0-verify.sh`.

## Non-Goals

Phase 18 should not implement:

- solver discharge;
- production MLIR decode dialect;
- native kernel expansion;
- DuckDB native execution;
- Iceberg or StarRocks integration;
- object-store credential handling;
- encrypted Vortex files;
- forward-compatibility WASM execution;
- complete compiler correctness proof.

## Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Vortex scan becomes implementation | Weakens Loom proof | Use scan only as oracle; emitted `.loom` must be built from explicit supported facts |
| Layout support explodes | Phase stalls | Support matrix with accepted/unsupported/rejected; no implicit conversion |
| Upstream API churn | Maintenance risk | Keep calls isolated in `loom-vortex-ingress`; expose Loom-owned facts |
| `loom-core` gets Vortex dependency | Breaks architecture | Keep existing dependency guards and add Phase 18 checks |
| Segment/tree facts are too shallow for Phase 19 | Solver phase lacks inputs | Include recursive layout/dtype/segment facts now |
| Value-dependent semantic constraints remain runtime-only | Overclaims safety | Document static facts vs runtime guards vs oracle equivalence |

## Research Conclusion

Phase 18 should be a complete reader-boundary phase. It should convert real
Vortex file structure into stable Loom-owned facts, support explicit artifact
emission for a growing but finite matrix of dtype/layout shapes, and preserve
fail-closed behavior for everything else.

The phase should end with Phase 19 ready to consume real complete-reader facts
for solver-backed artifact verification.
