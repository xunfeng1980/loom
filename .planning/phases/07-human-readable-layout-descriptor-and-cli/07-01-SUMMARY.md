---
phase: 07-human-readable-layout-descriptor-and-cli
plan: "01"
subsystem: loom-core
tags: [descriptor, parser, layout]
requirements_completed: [DX-01]
completed: 2026-06-08
---

# Phase 07-01: Descriptor Format and Core Roundtrip Summary

Phase 07-01 added a human-readable MVP0 descriptor codec.

## Accomplishments

- Chose RON for the descriptor format because MVP0 layouts are recursive enum trees.
- Added `loom_core::descriptor`.
- Implemented:
  - `to_descriptor_text`
  - `from_descriptor_text`
  - `payload_to_descriptor_text`
  - `descriptor_text_to_payload`
- Covered Raw, BitPack, FrameOfReference, Dictionary, RunEnd, and KernelEscape.
- Represented FOR `i128` references as strings in descriptor text because RON 0.10 does not serialize `i128` directly.
- Added deterministic parse/print tests and malformed descriptor error coverage.

## Verification

- `cargo test -p loom-core descriptor` - PASS.
