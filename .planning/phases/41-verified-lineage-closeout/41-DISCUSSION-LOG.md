# Phase 41 Discussion Log

**Date:** 2026-06-09

## Decisions

- Keep Phase 41 as a closeout phase: compose existing evidence instead of
  expanding execution coverage.
- Make the combined gate explicit as `scripts/verified-lineage-test.sh`, even
  though `scripts/full-verifier-test.sh` already runs much of the matrix.
- Add the lineage record as a Loom-owned Rust data model derived from verifier
  reports and optional native/model validation, not as a new binary container
  section. Phase 45 can later bind/sign/transport the record.
- Preserve the Phase 36 red line: Loom guarantees safety and Arrow
  well-formedness provenance only, never source correctness.

## Review Notes To Preserve

- The Lean theorem is meaningful only over the modeled executor. Rust and native
  behavior are supported by differential validation gates, not by the Lean proof
  itself.
- Native/model validation is per-run translation validation. It does not prove
  MLIR/LLVM/native lowering correctness.
- The lineage record should expose TCB assumptions as first-class rows, not hide
  them behind a "verified" badge.

Self-Check: READY
