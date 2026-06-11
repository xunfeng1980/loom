---
phase: 50-sidecar-overlay-model-and-host-native-reader-fallback
plan: "02"
subsystem: ir
tags: [sidecar, parquet, codec, loom-ir-core, content-hash]
requires:
  - phase: 49-independent-l2core-decode-ir-codec-and-content-hash-identity
    provides: "L2Core IR codec (l2core_codec), content-hash identity (l2core_program_hash), deterministic binary wire format"
  - phase: 50.1-container-demotion-and-thin-host-adapters
    provides: "thin host adapter scaffold, sidecar stubs in source_contract.rs"
provides:
  - "SidecarOverlay and ChunkBinding types in loom-ir-core"
  - "Deterministic binary encode/decode for sidecar overlay"
  - "Parquet KeyValue metadata extract/embed for loom sidecar"
  - "Real extract_sidecar_bytes_from_parquet_path replacing Phase 50.1 stub"
affects: [50-03-sidecar-routing, vortex-sidecar, lance-sidecar]
tech-stack:
  added: [base64 0.22.1 (loom-parquet-ingress)]
  patterns: ["u8-length-prefixed strings for ChunkBinding fields (max 255 bytes per field)", "Base64-wrapped binary sidecar in Parquet KeyValue string metadata"]
key-files:
  created:
    - crates/loom-ir-core/src/sidecar.rs
    - ingress/loom-parquet-ingress/src/sidecar_parquet.rs
  modified:
    - crates/loom-ir-core/src/lib.rs
    - crates/loom-core/src/lib.rs
    - ingress/loom-parquet-ingress/src/lib.rs
    - ingress/loom-parquet-ingress/src/source_contract.rs
    - ingress/loom-parquet-ingress/Cargo.toml
key-decisions:
  - "Sidecar encode uses u8-length-prefixed strings for ChunkBinding fields (granule_id, content_hash, ir_identity) enforcing max 255 bytes per per-field (T-50-04 mitigation)"
  - "Parquet embed uses base64 encoding for lossless binary roundtrip through Option<String> KeyValue value field"
  - "Embed function takes &mut Vec<KeyValue> directly instead of &mut ParquetMetaData because the parquet crate does not expose mutable FileMetaData access through ParquetMetaData"
  - "l2ir:<hex> format validation (prefix + 16 hex chars) enforced on both content_hash and ir_identity during decode"
patterns-established:
  - "fail-closed decode: every byte read is bounds-checked before access; truncated/malformed input returns SidecarCodecError without panicking"
  - "sidecar embedding is additive-only: embed_sidecar_into_key_value_metadata only modifies KeyValue metadata list, never touches host data pages"
requirements-completed: []
duration: 9 min
completed: 2026-06-11
status: complete
---

# Phase 50 Plan 02: Core Sidecar Overlay Model and Parquet Sidecar Embedding Summary

**Host-neutral SidecarOverlay/ChunkBinding types in loom-ir-core with deterministic encode/decode, plus real Parquet sidecar extract/embed via KeyValue metadata replacing Phase 50.1 stubs**

## Performance

- **Duration:** 9 min
- **Started:** 2026-06-11T08:34:22Z
- **Completed:** 2026-06-11T08:44:09Z
- **Tasks:** 3
- **Files modified:** 7 (2 new, 5 modified)

## Accomplishments

- Created `crates/loom-ir-core/src/sidecar.rs` with `SidecarOverlay`, `ChunkBinding`, `SidecarCodecError`, and deterministic binary encode/decode with fail-closed bounds-checking
- Created `ingress/loom-parquet-ingress/src/sidecar_parquet.rs` with real Parquet sidecar extraction from `FileMetaData.key_value_metadata()` and embedding into `KeyValue` metadata entries
- Replaced stub `extract_sidecar_bytes_from_parquet_path` in `source_contract.rs` with real delegation to `sidecar_parquet::extract_sidecar_from_parquet_metadata`
- Wired `pub mod sidecar` into `loom-ir-core/lib.rs` and `pub mod sidecar_parquet` into `loom-parquet-ingress/lib.rs`
- All existing tests pass with zero regressions (29 loom-ir-core, 24 loom-parquet-ingress, 8+ loom-core)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create sidecar.rs — SidecarOverlay, ChunkBinding, and deterministic encode/decode** - `e9e288c` (feat)
2. **Task 2: Create sidecar_parquet.rs — real Parquet sidecar extract/embed** - `8380d32` (feat)
3. **Task 3: Wire sidecar module into lib.rs, add tests** - `7359b6f` (feat)

## Files Created/Modified

- `crates/loom-ir-core/src/sidecar.rs` — SidecarOverlay (ir_bytes + bindings), ChunkBinding (granule_id, host_data_range, content_hash, ir_identity), SidecarCodecError (Malformed, Truncated, BadHashFormat), deterministic binary encode/decode with u32/u16/u8-length-prefixed format, 10 unit tests
- `crates/loom-ir-core/src/lib.rs` — Added `pub mod sidecar;`
- `crates/loom-core/src/lib.rs` — Added `pub use loom_ir_core::sidecar;` re-export
- `ingress/loom-parquet-ingress/src/sidecar_parquet.rs` — extract_sidecar_from_parquet_metadata (scans KeyValue for "loom.sidecar.v1", base64-decodes, calls SidecarOverlay::decode), embed_sidecar_into_key_value_metadata (encodes overlay, base64-wraps, adds "loom.sidecar.v1" + per-column "loom.hash.*" entries, idempotent re-embed), 9 unit tests using real Parquet I/O
- `ingress/loom-parquet-ingress/src/source_contract.rs` — Replaced stub extract_sidecar_bytes_from_parquet_path with real delegation using ParquetRecordBatchReaderBuilder → sidecar_parquet
- `ingress/loom-parquet-ingress/src/lib.rs` — Added `pub mod sidecar_parquet;`
- `ingress/loom-parquet-ingress/Cargo.toml` — Added `base64 = "0.22.1"` and `loom-ir-core` dependencies

## Decisions Made

- **Embed API adaptation:** Changed `embed_sidecar_into_parquet_metadata` from taking `&mut ParquetMetaData` to taking `&mut Vec<KeyValue>` directly. The parquet 58.3.0 crate does not expose mutable access to `FileMetaData` fields through `ParquetMetaData`. The embed function now operates on the KeyValue list directly; callers set it on `WriterProperties` or `FileMetaData::new()` at construction time.
- **Test approach:** Used real Parquet I/O (ArrowWriter + ParquetRecordBatchReaderBuilder) instead of constructing `ParquetMetaData` from scratch. The parquet schema types (`SchemaElement`, `PrimitiveType`, etc.) are private/hard to construct directly. Writing a real Parquet file is cleaner and tests the actual roundtrip.

## Deviations from Plan

None — plan executed as written with one API adaptation:

**1. [Rule 3 - Blocking] Changed embed function signature from `&mut ParquetMetaData` to `&mut Vec<KeyValue>`**
- **Found during:** Task 2 (Parquet sidecar embed)
- **Issue:** `parquet` crate 58.3.0 keeps `file_metadata` field private on `ParquetMetaData` with no `file_metadata_mut()` accessor. Cannot mutate `FileMetaData.key_value_metadata` through `ParquetMetaData`.
- **Fix:** Changed `embed_sidecar_into_key_value_metadata` to take `&mut Vec<KeyValue>` directly. The function still does everything the plan specifies (encode, base64-wrap, add KeyValue entries, idempotent re-embed). Callers pass the KV list to `WriterProperties::set_key_value_metadata()` or `FileMetaData::new()`. The extract function signature remains unchanged (`&ParquetMetaData`).
- **Files modified:** `ingress/loom-parquet-ingress/src/sidecar_parquet.rs`
- **Verification:** All 9 sidecar_parquet tests pass with the changed signature.

**Total deviations:** 1 auto-fixed (blocking API adaptation)
**Impact on plan:** Minimal — same functionality, slightly different call-site contract. The intent (additive metadata embedding) is fully preserved.

## Issues Encountered

None.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Ready for Plan 50-03 (Sidecar Routing Decision Logic): `SidecarOverlay` and `ChunkBinding` types are defined and tested, Parquet sidecar extract/embed is functional
- `extract_sidecar_bytes_from_parquet_path` now returns real decoded sidecar bytes instead of the Phase 50.1 `Ok(None)` stub
- Sidecar encode/decode is deterministic and fail-closed with comprehensive negative test coverage
- Per-column `"loom.hash.*"` KeyValue entries are embedded for routing verification in Plan 50-04

---

*Phase: 50-sidecar-overlay-model-and-host-native-reader-fallback*
*Completed: 2026-06-11*
