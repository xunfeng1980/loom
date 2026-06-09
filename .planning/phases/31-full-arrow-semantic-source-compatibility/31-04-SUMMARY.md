---
phase: 31-full-arrow-semantic-source-compatibility
plan: 04
type: summary
status: complete
completed_at: 2026-06-09
requirements:
  - PHASE-31
---

# 31-04 Summary: Lance Arrow Semantic Emission

## Completed

- Replaced Lance accepted emission with `LMA1` Arrow semantic payload emission.
- Preserved Lance source facts, version, fragment, split, and schema summaries while changing artifact bytes to Arrow semantic payloads.
- Routed Lance scanner `RecordBatch` output through `ArrowSemanticPayload` and verifier-backed accepted reports.
- Updated Lance handoff tests from `LMP1`/`LMT1` raw/table assertions to Arrow semantic equality.
- Added focused full-schema Lance e2e coverage for nullable scalar, bool, UTF-8, struct, and list Arrow batches.
- Updated legacy readability so older Lance datasets remain readable while current rewrites produce verifier-accepted `LMA1` semantic artifacts.

## Verification

- `cargo test -p loom-lance-ingress --test full_arrow_schema_compatibility`
- `cargo test -p loom-lance-ingress --test source_ingress_handoff`
- `cargo test -p loom-lance-ingress --test legacy_readability`
- `scripts/full-arrow-semantic-compatibility-test.sh`

## Tradeoffs

- The public Lance emission helper was renamed from `lmc1` to `lma1`; Phase 31 intentionally has no compatibility obligation to keep the old helper name.
- The current full-schema test is representative, not exhaustive across every Lance-supported Arrow logical family. The source path now accepts everything Lance can materialize as Arrow; later gate expansion should add decimal where supported, temporal, binary, fixed-size list/vector-like, map where supported, dictionary, and empty/all-null fixtures.
