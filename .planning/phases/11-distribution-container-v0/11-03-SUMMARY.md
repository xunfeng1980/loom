---
phase: 11-distribution-container-v0
plan: "03"
status: complete
completed_at: "2026-06-08T03:20:00Z"
commit: 1bec8e7
requirements: [DIST-01, DIST-04, DIST-05]
---

# 11-03 Summary: CLI, Fixtures, and DuckDB LMC1 Exposure

## What Changed

- `loom inspect` detects `LMC1` containers before raw payloads.
  - Prints container version, required/optional feature names, section count, section summaries, trailer status, verifier status, and wrapped payload kind.
  - Then prints the existing layout/table summaries for the wrapped `LMP1`/`LMT1` payload.
- `loom decode` unwraps supported container payloads while preserving raw `LMP1`, raw `LMT1`, and descriptor-text behavior.
- `emit_duckdb_payloads` now writes deterministic `.loom` smoke fixtures as `LMC1` containers by default.
  - Existing fixture filenames remain stable.
  - Manifest rows now identify the wrapped payload kind and container status.
- DuckDB bind now recognizes `LMC1`, validates the header/directory shape needed for bind, extracts the wrapped payload kind, and reuses the existing schema inference.
  - Single-column containers are passed through to `loom_decode`, exercising the FFI container path.
  - Table containers are unwrapped for bind-time column discovery; per-column scan still uses existing raw `LMP1` column payloads.
- `scripts/duckdb-smoke-test.sh` now fails if generated smoke fixtures are not `LMC1`.
- `scripts/verifier-negative-test.sh` now mutates table row counts inside either raw `LMT1` or container-wrapped `LMT1` fixtures.

## Acceptance Criteria

- [x] `loom inspect <container>` prints `container: LMC1`.
- [x] Inspect output includes feature sets and concise section summaries.
- [x] `loom decode <container>` prints the same table rows as the wrapped raw payload.
- [x] Raw `LMP1`, raw `LMT1`, and descriptor workflows remain supported through existing Rust tests.
- [x] Generated fixture files begin with `LMC1`.
- [x] Existing fixture names remain stable for scripts and docs.
- [x] DuckDB `loom_scan` works on container-wrapped single-column fixtures.
- [x] DuckDB `loom_scan` works on the container-wrapped mixed-table fixture.
- [x] Smoke test fails closed if generated fixtures stop being `LMC1`.

## Verification

- `cargo test -p loom-cli`
- `cargo run -p loom-fixtures --bin emit_duckdb_payloads`
- `cargo run -p loom-cli --bin loom -- inspect target/loom-duckdb-fixtures/bitpack-i32.loom`
- `cargo run -p loom-cli --bin loom -- decode target/loom-duckdb-fixtures/mixed-table.loom`
- `bash scripts/duckdb-smoke-test.sh`
- `bash scripts/verifier-negative-test.sh`
- `git diff --check`

## Notes

- The C++ bind parser remains intentionally shallow: it parses enough `LMC1` structure to derive schema and payload kind, while Rust remains the scan-time authority for single-column container validation.
- Full table-container Rust validation at the SQL boundary would require a table-aware FFI/record-batch surface and remains outside Phase 11.

