---
phase: 11-distribution-container-v0
plan: "01"
subsystem: loom-core
tags: [container, lmc1, distribution, codec]
requirements_completed: []
completed: 2026-06-08
commit: 9426e1c
---

# Phase 11-01: Core LMC1 Container Codec Summary

Phase 11-01 added the core `LMC1` distribution container v0 codec in `loom-core`.

## Accomplishments

- Added `container_codec` and exported it from `loom-core`.
- Added `ContainerDescription`, `ContainerSection`, `SectionKind`, `Feature`, and `PayloadKind`.
- Implemented deterministic `encode_container` / `decode_container` with magic/version/header/features/section directory/section bytes/trailer handling.
- Added `wrap_layout_payload` and `wrap_table_payload` helpers that preserve existing `LMP1` and `LMT1` payload bytes inside `LMC1` sections.
- Added payload-kind helpers for raw layout, raw table, container, and unknown bytes.
- Added fail-closed checks for unknown required features, malformed section directories, duplicate payload sections, wrong magic, unsupported version, offset overflow, sections outside the payload, and trailing corruption.
- Added `MalformedContainer` as a distinct typed decode error.

## Verification

- `cargo test -p loom-core container_codec` - PASS, 15 tests.
- `cargo test -p loom-core` - PASS, 113 tests.
- `git diff --check` - PASS.

## Notes

- `LMP1` and `LMT1` codecs were not rewritten; Phase 11-01 only adds the wrapping container boundary.
- `cargo fmt` temporarily touched an unrelated fixture timing helper; that formatting-only diff was reverted before the production commit.
