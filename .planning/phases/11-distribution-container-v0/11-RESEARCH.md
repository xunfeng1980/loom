# Phase 11 Research: Distribution Container v0

**Date:** 2026-06-08
**Status:** Ready for discuss/plan
**Primary scope:** Versioned Loom distribution container v0
**Related future placeholders:** Phase 12 formal verifier, Phase 13 MLIR/native lowering, Phase 14 real Vortex file/container ingress

## Executive Summary

Phase 11 should introduce a Loom-owned distribution container boundary around the already-working `LMP1` single-column and `LMT1` table payloads. The goal is not to replace the current payload codecs immediately, and not to implement a full file format. The goal is to create the first explicit artifact that can plausibly "travel with data": versioned header, feature flags, schema/layout/kernel sections, section directory, strict fail-closed behavior for unsupported required features, and CLI/release-gate visibility.

Recommended direction:

- Add a new container magic, tentatively `LMC1`, whose payload can wrap existing `LMP1` or `LMT1` sections during v0.
- Keep the existing `LMP1` and `LMT1` codecs as compatibility sections rather than rewriting all layout/table codecs in Phase 11.
- Introduce a checked section directory with offset/length/kind/flags so readers can inspect, skip, or reject sections without ad hoc scanning.
- Split feature flags into required and optional sets. Unknown required features must fail closed; unknown optional sections may be ignored if they are not referenced by required sections.
- Make `loom inspect` display the container header, version, features, sections, schema summary, and wrapped payload kind before decode.
- Extend the release gate with both success fixtures and negative fixtures: unknown required feature, duplicate required section, truncated section, section offset overflow, and unsupported container version.

This is the right Phase 11 because it turns the project from "demo payloads that happen to work" into "a bounded, inspectable distribution artifact." Formal verification, MLIR lowering, and real `.vortex` file ingress should wait until this trust boundary exists.

## External Evidence

### Arrow IPC: Magic + Footer Directory for Random Access

Apache Arrow's IPC file format starts and ends with `ARROW1`, and its footer stores schema plus offsets and sizes for record batches, enabling random access to batches. Source: https://arrow.apache.org/docs/format/Columnar.html#ipc-file-format

Implication for Loom:

- A container should have both an early identity check and a late/directory entry point.
- A section table with offsets/sizes is more durable than positional parsing once sections begin to evolve.
- Schema metadata belongs near the container boundary, not buried inside decoder params.

### Parquet: Footer Metadata Separates Data from Navigation

Parquet writes `PAR1` magic at both ends and keeps file metadata at the end, including column chunk locations; readers first read metadata to find columns of interest. Source: https://parquet.apache.org/docs/file-format/

Implication for Loom:

- Metadata should point to data/kernel/layout sections, not require reading all data first.
- Even if Loom v0 only wraps small fixtures, the format should not preclude later column or range skipping.
- Phase 11 can stay small while preserving a future footer/directory shape.

### Vortex: Minimal File Wrapper + Postscript + Segment Directory

Vortex files use `VTXF` at both ends, store version and postscript length immediately before the trailing magic, and use the postscript to locate dtype, layout, statistics, and footer segments. Vortex documentation also emphasizes backward compatibility from version 0.36.0 and planned forward compatibility via minimum reader version and embedded decompression logic. Sources:

- https://docs.vortex.dev/specs/file-format
- https://docs.vortex.dev/developer-guide/internals/serialization

Implication for Loom:

- The distribution container should separate logical schema/type, layout, kernel/module sections, and optional statistics.
- A small end-of-file entry point is useful for object storage and partial reads later.
- Loom should capture `min_reader_version` or equivalent sooner rather than later, even if only v0/v1 exists initially.
- Real Vortex file support belongs in Phase 14; Phase 11 can borrow the structural lessons without depending on `vortex-file`.

### WebAssembly: Section Lengths and Skippable Custom Sections

The WebAssembly binary format organizes modules into sections. Each section carries an id and byte length; section sizes can be used to skip sections, and custom sections are ignored by the semantics. Source: https://webassembly.github.io/spec/core/binary/modules.html

Implication for Loom:

- Section length is a load-bearing verifier/safety primitive.
- Optional metadata/debug sections should be skippable.
- Required executable/semantic sections must be different from optional custom sections; "unknown optional" and "unknown required" need different behavior.

### Substrait: Binary for Transport, Text for Debugging

Substrait explicitly supports binary serialization for program-to-program transport and text serialization for debugging/human readability. Source: https://substrait.io/serialization/basics/

Implication for Loom:

- Keep the machine distribution container binary.
- Preserve human-readable descriptor/inspect output as a reviewer/debug surface, not as the canonical distribution format.
- CLI output should be the bridge between binary container and human review.

## Local Evidence

The repository already has useful building blocks:

- `LMP1` single-column payload codec in `crates/loom-core/src/layout_codec.rs`
- `LMT1` table payload codec in `crates/loom-core/src/table_codec.rs`
- RON descriptor text for human review in `crates/loom-core/src/descriptor.rs`
- verifier routing for layout/table payloads in `crates/loom-core/src/verifier.rs`
- CLI inspect/decode in `crates/loom-cli/src/main.rs`
- DuckDB smoke payload emitter in `crates/loom-fixtures/src/bin/emit_duckdb_payloads.rs`

Current limitation:

- `LMP1` and `LMT1` are directly decoded as the top-level artifact.
- There is no stable container header, feature negotiation, section directory, or min-reader version.
- Unknown feature behavior exists only indirectly through individual payload parsers.
- The CLI can inspect payload internals, but cannot show a top-level distribution artifact boundary.

## Recommended Container v0 Shape

Tentative binary shape:

```text
<4 bytes>  magic "LMC1"
<2 bytes>  container_version = 1
<2 bytes>  header_len
<8 bytes>  required_features bitset
<8 bytes>  optional_features bitset
<4 bytes>  section_count
<section directory entries>
<section payload bytes>
<4 bytes>  magic "LMC1" optional trailer for v0 file fixtures
```

Tentative section directory entry:

```text
section_kind: u16
section_flags: u16
offset: u64
length: u64
checksum_or_reserved: u32
reserved: u32
```

Initial section kinds:

| Kind | Name | Required? | Notes |
|---:|---|---|---|
| 1 | `schema` | yes | MVP0 may store compact dtype/table schema summary or copy existing layout/table dtype metadata. |
| 2 | `layout_payload` | yes for single-column | Existing `LMP1` bytes; preserves compatibility. |
| 3 | `table_payload` | yes for table | Existing `LMT1` bytes; exactly one of layout/table payload required in v0. |
| 4 | `kernel_manifest` | optional in v0 | Kernel ids, names, params formats, and required feature bits. Can be derived at first, explicit later. |
| 5 | `stats` | optional | Reserved for later `statistics()` work. |
| 255 | `debug_descriptor` | optional | Human-readable descriptor text, ignored by decode. |

Recommended feature bits:

| Bit | Feature | Required when |
|---:|---|---|
| 0 | `single_column_lmp1` | wrapping a single-column payload |
| 1 | `table_lmt1` | wrapping a table payload |
| 2 | `kernel_fsst` | payload references FSST kernel id `0` |
| 3 | `kernel_alp_float` | payload references ALP kernel id `1` |
| 4 | `float32_float64` | payload exposes Float32/Float64 |
| 5 | `debug_sections` | optional only |
| 6 | `stats_section` | optional until statistics ABI exists |

## Recommended Phase 11 Boundaries

Phase 11 should include:

- Container encode/decode module in `loom-core`.
- Compatibility wrapping of current `LMP1` and `LMT1` payloads.
- Container-aware `decode` and verifier entry points.
- CLI inspect/decode support for both raw old payloads and new container payloads.
- Fixture emitter that writes container-wrapped `.loom` files while preserving old payload tests where useful.
- Negative tests for unsupported version/features and section corruption.
- Documentation that distinguishes `LMC1` distribution container v0 from MVP0 internal layout/table payloads.

Phase 11 should not include:

- Full formal totality/termination proof.
- MLIR/native lowering.
- Real `.vortex` footer/layout ingestion.
- Remote content-hash URI fetch, signatures, encryption, or attestation.
- New compression kernels.

## Key Design Decisions to Confirm During Discussion

1. **Top-level magic:** `LMC1` is recommended to avoid overloading `LMP1`/`LMT1`.
2. **Compatibility:** old `LMP1`/`LMT1` should continue to decode, but generated fixtures may switch to `LMC1` wrappers.
3. **Unknown features:** unknown required feature fails closed; unknown optional/debug section is skipped if unreferenced.
4. **Directory location:** v0 may use a simple front directory for implementation speed, but should reserve a trailer magic/EOF path to keep the Arrow/Parquet/Vortex random-access lesson alive.
5. **Schema section:** v0 can duplicate summary metadata even if `LMP1`/`LMT1` already contains dtype/row counts, because the container boundary needs inspectable schema identity.

## Risks

| Risk | Impact | Mitigation |
|---|---:|---|
| Container becomes a second full file format too early | High | v0 wraps existing payload codecs; defer stats, URI, signatures, and real file data segments. |
| Backward compatibility breaks the completed MVP0 gate | High | Keep raw `LMP1`/`LMT1` decode paths and add container paths alongside them. |
| Feature flags become decorative | Medium | Add negative tests for unknown required feature and unsupported version. |
| Section directory arithmetic introduces overflow bugs | Medium | Use checked offset/length math and verifier diagnostics before slicing. |
| CLI becomes noisy | Low | Show concise header/sections summary by default; keep full descriptor text in existing descriptor path. |

## Recommended Phase 11 Success Criteria

1. `LMC1` container encode/decode roundtrips both single-column and table payloads.
2. Existing raw `LMP1`/`LMT1` payloads remain accepted.
3. Unknown required features, unsupported versions, duplicate required sections, truncated sections, and offset overflows fail closed with typed diagnostics.
4. `loom inspect` shows container version, required/optional features, section directory, and wrapped payload kind.
5. DuckDB smoke and `scripts/mvp0-verify.sh` remain green using generated container-wrapped fixtures or an explicit mixed old/new fixture set.

