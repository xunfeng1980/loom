# Phase 35 Native Arrow Semantic Execution Report

**Date:** 2026-06-09
**Status:** In progress, gate wired in 35-04

## Positive Evidence

- `loom-core::native_arrow_semantic` is an engine-neutral native backend with
  backend identity `loom-native-arrow-semantic`.
- Accepted default `LMC2(LMA1)` artifacts and explicit direct `LMA1` bridge
  artifacts can execute natively for one-record-batch fixed-width primitive
  nullable columns.
- Supported native types: `Boolean`, `Int32`, `Int64`, `Float32`, and
  `Float64`.
- Execution copies real values and nulls through typed Arrow builders into a
  new Arrow `RecordBatch`.
- Native/reference equivalence is explicit and can report
  `native-output-mismatch`.
- Runtime/cache identity is host-neutral and includes backend identity,
  artifact digest, facts fingerprint, projection, split, and policy.

## Fail-Closed Evidence

- Utf8, Date32 logical, Struct/List-style nested data, multi-batch payloads,
  malformed bytes, and verifier-rejected artifacts do not produce native output.
- Unsupported-but-accepted Arrow semantic shapes can follow runtime fallback
  policy, but cannot seed native cache keys.
- Native cache keys are generated only for accepted, diagnostic-free native
  Arrow semantic execution reports.

## Gate Evidence

- Focused gate: `bash scripts/native-arrow-semantic-execution-test.sh`.
- Broad gate wiring: `scripts/mvp1-verify.sh` runs the Phase 35 gate after the
  MVP1 DuckDB source e2e gate.

## Non-Claims

- DuckDB does not yet consume the native Arrow semantic route.
- Utf8, logical, and nested native execution are explicitly unsupported.
- This is not live StarRocks runtime evidence.
