---
phase: 09-verifier-and-safety-boundary-mvp
status: complete
created: 2026-06-08
requirements: [SAFE-01, SAFE-02, SAFE-03, SAFE-04, VERIFY-06]
---

# Phase 09 Research: Verifier and Safety Boundary MVP

## Research Goal

Determine how to plan a first-pass structural verifier for the existing MVP0 Loom implementation without overstating it as the formal Loom verifier.

## Current Implementation Surface

### Layout and Payload Parsing

- `crates/loom-core/src/layout_codec.rs` decodes `LMP1` single-column payloads into `LayoutDescription`.
- `crates/loom-core/src/table_codec.rs` decodes `LMT1` table payloads into `TableDescription` and already validates table column names and row counts.
- `crates/loom-core/src/descriptor.rs` parses human-readable RON descriptors into `LayoutDescription`.

### Decode and Error Handling

- `crates/loom-core/src/l1_model.rs` owns `LayoutNode`, `LayoutDescription`, and decode helpers.
- Decode-time malformed input already returns `LoomDecodeError` variants for many unsafe or invalid states:
  - `BufferTooShort`
  - `UnsupportedWidth`
  - `BitWidthExceedsType`
  - `InvalidDictionaryCode`
  - `NonMonotonicRunEnd`
  - `RunEndOutOfBounds`
  - `RunEndTooShort`
  - `InsufficientRunValues`
  - `UnsupportedBuilderType`
  - `UnknownKernel`
  - FSST parameter and UTF-8 errors
- `crates/loom-ffi/src/ffi.rs` maps all decode errors to `LoomError::DecodeFailed`, while panic safety stays behind `catch_unwind`.

### CLI Surface

- `crates/loom-cli/src/main.rs` supports `loom inspect <input>` and `loom decode <input>`.
- `inspect` already differentiates `LMT1` table payloads, `LMP1` single-column payloads, and descriptor text.
- Phase 9 should extend `inspect` rather than add a new CLI command.

## Verifier Scope Recommendation

Phase 9 should introduce `loom_core::verifier` with a small public API:

- `verify_layout(desc: &LayoutDescription, registry: &L2KernelRegistry) -> VerificationReport`
- `verify_table(table: &TableDescription, registry: &L2KernelRegistry) -> VerificationReport`
- `VerificationReport::is_ok() -> bool`
- diagnostics with `code`, `path`, and `message`

The verifier should be structural and cheap:

- Validate shape, counts, supported type/layout combinations, recursive paths, and known kernel ids.
- Reuse `TableDescription::validate` for table-level invariants.
- Reuse FSST parameter decoding for kernel id 0 where practical.
- Avoid duplicating all decode-time semantics. For data-dependent checks such as dictionary code bounds or run-end expansion, route through existing decode-time typed errors and document those as authoritative checks.

## Suggested Diagnostic Codes

Exact names can change during implementation, but the plan should ensure stable categories:

- `layout.raw.count_bytes`
- `layout.raw.unsupported_type`
- `layout.bitpack.width`
- `layout.bitpack.validity_len`
- `layout.for.unsupported_type`
- `layout.dictionary.codes_type`
- `layout.run_end.count`
- `layout.kernel.unknown`
- `layout.kernel.params`
- `table.empty`
- `table.column_name`
- `table.row_count`

Paths should use recursive names such as:

- `root`
- `root.inner`
- `root.codes`
- `root.values`
- `root.run_ends`
- `columns[id].root`
- `columns[label].root`

## Negative Coverage Strategy

Use curated tests, not fuzzing. Required cases:

1. Truncated binary payload parsing fails closed.
2. `Raw` count/byte mismatch is detected.
3. `BitPack` bit width exceeds target type.
4. `BitPack` validity length mismatch is detected.
5. `FrameOfReference` over Boolean/Utf8 is rejected.
6. `Dictionary` with non-integer codes is rejected structurally.
7. Run-end invalidity is covered by authoritative decode-time errors.
8. Unknown kernel ids are rejected.
9. FSST malformed params are rejected.
10. Table duplicate/empty names and row-count mismatches are rejected.
11. The stale FOR-over-non-BitPack todo is audited and closed or updated with evidence.

## Release Gate Integration

`scripts/mvp0-verify.sh` already runs `cargo test --workspace`, which will cover Rust verifier tests. Add a dedicated script only if CLI-level negative fixture behavior needs shell checks. A small `scripts/verifier-negative-test.sh` can be useful if it verifies exact `loom inspect` failure output, but it should be invoked by `mvp0-verify.sh` to avoid a hidden gate.

## Risks and Constraints

- Do not introduce Vortex/FastLanes dependencies into `loom-core`.
- Do not claim formal totality or non-termination proof.
- Avoid verifier/decode drift by delegating data-dependent invariants to existing typed decode-time checks.
- Keep CLI output stable and concise; JSON is out of scope.

## Validation Architecture

Phase 9 validation should prove:

- Unit tests cover verifier diagnostics and paths.
- Integration tests prove decode/FFI fail closed on malformed payloads.
- CLI smoke verifies `loom inspect` reports `verification: pass` for valid payloads and prints diagnostics for invalid descriptors/payloads.
- `bash scripts/mvp0-verify.sh` remains the one-command gate and includes verifier coverage.

