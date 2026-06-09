# Phase 30 Dual Query-Surface Report

## Executive Summary

Phase 30 is complete as a bounded dual query-surface proof.

The shared trust root is one Phase 29 accepted Iceberg-bound, verifier-accepted
Loom table artifact. DuckDB evidence is executable through the existing public
`loom_scan(path)` table function. StarRocks-compatible evidence is an offline
descriptor/query contract by default, not a live StarRocks runtime claim.

Optional StarRocks runtime smoke is env-gated and supplemental only. A skipped
runtime smoke is not accepted StarRocks runtime evidence.

## Implemented Artifacts

- `crates/loom-dual-query-surface`: adapter-local Phase 30 evidence crate.
- `crates/loom-dual-query-surface/src/fixture_bundle.rs`: deterministic Phase
  29 accepted binding fixture.
- `crates/loom-dual-query-surface/src/query_surface.rs`: canonical query
  matrix, binding identity evidence, StarRocks-compatible descriptors, and
  typed unsupported query-feature handling.
- `crates/loom-dual-query-surface/src/duckdb_evidence.rs`: DuckDB SQL cases
  over the same accepted Loom artifact.
- `scripts/dual-query-surface-test.sh`: focused Phase 30 gate.
- `scripts/mvp0-verify.sh`: main release gate wiring after Phase 29 and before
  DuckDB SQL smoke.

## Shared Phase 29 Binding Input

The fixture generator constructs a local Iceberg metadata/sidecar/evidence
bundle and accepts it only through `bind_iceberg_ref_from_paths`.

| Field | Value |
|---|---|
| Table | `demo.events` |
| Table UUID | `9f1a03d0-61f7-4f6d-a7a4-3d8b983cbe30` |
| Schema ID | `7` |
| Snapshot ID | `314159` |
| Loom artifact kind | verifier-accepted `LMC1` / `LMT1` table |
| Rows | `7`, `-1`, `42` |

The sidecar flags are descriptive inputs only. Acceptance requires local
artifact bytes, recomputed SHA-256, artifact verifier acceptance, source
evidence, and decoded-row oracle evidence.

## Query Matrix

| Query | Expected Evidence |
|---|---|
| Ordered rows | `-1`, `7`, `42` |
| Projection | `id` only |
| Predicate | `id >= 0` -> `7`, `42` |
| Count | `3` |
| Sum | `48` |

This is a small seam proof, not full SQL dialect compatibility.

## DuckDB Executable Evidence

DuckDB runs the matrix through the existing extension and public
`loom_scan(path)` surface:

- `SELECT id FROM loom_scan(path) ORDER BY id` -> `-1`, `7`, `42`
- `SELECT id FROM loom_scan(path) WHERE id >= 0 ORDER BY id` -> `7`, `42`
- `SELECT COUNT(*) FROM loom_scan(path)` -> `3`
- `SELECT SUM(id) FROM loom_scan(path)` -> `48`

This closes the native-query zero-buffer concern for Phase 30's DuckDB slice:
the gate asserts concrete row and aggregate values from the accepted artifact,
not route labels or zero-filled native helper buffers.

## StarRocks-Compatible Descriptor Evidence

The StarRocks-compatible proof emits deterministic descriptors with:

- accepted Phase 29 binding identity;
- query kind and projected column list;
- StarRocks-compatible SQL text such as
  `SELECT id FROM \`demo\`.\`events\` ORDER BY id`;
- expected row/scalar evidence and stable digest;
- no runtime endpoint, credential, catalog, external table DDL, or separate
  artifact format.

Descriptor validation fails closed if table UUID, schema ID, snapshot ID,
artifact SHA-256, row count, projection, status, or expected result evidence
drifts from the accepted Loom artifact.

## Descriptor Schema

Each `StarRocksQueryDescriptor` contains:

| Field | Meaning |
|---|---|
| `status` | `accepted`, `unsupported`, or `rejected` |
| `identity` | table UUID/name, schema ID, snapshot ID, artifact SHA-256, row count |
| `query_kind` | one of the bounded query matrix entries |
| `projection` | currently `["id"]` |
| `sql` | StarRocks-compatible SQL text |
| `expected_result_digest` | stable digest over expected evidence |
| `expected_values` / `expected_scalar` | deterministic row or aggregate evidence |
| `diagnostics` | populated for non-accepted descriptors |

## Mismatch Fail-Closed Matrix

| Case | Result |
|---|---|
| Table UUID drift | rejected before descriptor acceptance |
| Schema ID drift | rejected before descriptor acceptance |
| Snapshot ID drift | rejected before descriptor acceptance |
| Artifact SHA-256 drift | rejected before descriptor acceptance |
| Row count drift | rejected before descriptor acceptance |
| Projection drift | unsupported before descriptor acceptance |
| Expected digest/value drift | rejected before descriptor acceptance |
| Sidecar-only / manifest-only claim | no accepted binding bytes |
| Stale source evidence | no accepted binding bytes |
| Forged oracle evidence | no accepted binding bytes |
| Malformed artifact path or bytes | no accepted binding bytes |
| Unsupported query feature | typed unsupported diagnostic |

Unsupported query features include joins, freeform SQL, external table DDL,
remote catalogs, credentials, nested fields, nullable expansion, distributed
execution, and predicate pushdown.

## Optional StarRocks Runtime Smoke

By default, `scripts/dual-query-surface-test.sh` prints that optional StarRocks
runtime smoke is skipped and that the skip is not accepted StarRocks runtime
evidence.

When `LOOM_STARROCKS_RUNTIME_SMOKE=1` is set, the script requires all of:

- `STARROCKS_MYSQL`
- `STARROCKS_HOST`
- `STARROCKS_PORT`
- `STARROCKS_USER`
- `STARROCKS_PASSWORD`
- `STARROCKS_DATABASE`
- `STARROCKS_TABLE`

If the env is complete, the script runs the bounded matrix against the
operator-provided table and compares rows/count/sum to the canonical evidence.
This is supplemental runtime smoke only; it does not replace the deterministic
offline descriptor proof.

## Dependency and API Boundary

Phase 30 adds no default StarRocks server/client/JDBC/ODBC/REST/catalog,
credential, object-store, Docker, or orchestration dependency.

Public surfaces remain unchanged:

- no new `loom_scan_starrocks` or `loom_scan_iceberg`;
- no new public C ABI symbol;
- no CLI StarRocks route;
- no external table DDL control;
- no credential, catalog, or object-store route.

`loom-core`, `loom-ffi`, `loom-source-ingress`, and `loom-iceberg-binding`
remain source/query-engine neutral.

## Current-Phase Tradeoffs

- Offline StarRocks-compatible contract over a required cluster: reproducible
  and deterministic, but not live runtime evidence by default.
- Existing DuckDB `loom_scan(path)` over a new DuckDB table function: preserves
  the stable public SQL surface.
- Small query matrix over broad SQL compatibility: proves the seam before
  taking on dialect/runtime scope.
- Adapter-local descriptors over a generic engine framework: avoids freezing a
  query-engine abstraction too early.

## Non-Goals

Phase 30 does not implement or claim:

- live StarRocks cluster lifecycle;
- FE/BE/CN service orchestration;
- JDBC, ODBC, REST, or MySQL client dependency by default;
- StarRocks external table creation;
- REST/catalog credentials or object-store access;
- production Iceberg catalog operations;
- distributed execution;
- predicate pushdown into native kernels;
- full SQL dialect compatibility;
- nested/nullable expansion;
- full Vortex semantic compatibility;
- a second Loom artifact format.

## Release Gate Evidence

| Command | Status | Notes |
|---|---|---|
| `bash -n scripts/dual-query-surface-test.sh` | PASS | shell syntax |
| `bash scripts/dual-query-surface-test.sh` | PASS | focused Phase 30 gate |
| `bash -n scripts/mvp0-verify.sh` | PASS | main gate syntax |
| `python3 -c 'from pathlib import Path; text=Path("scripts/mvp0-verify.sh").read_text(); order=["scripts/iceberg-binding-test.sh","scripts/dual-query-surface-test.sh","scripts/duckdb-smoke-test.sh"]; pos=[text.index(x) for x in order]; assert pos == sorted(pos), pos'` | PASS | Phase 29 -> Phase 30 -> DuckDB smoke order |
| `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp0-verify.sh` | PASS | full inherited release gate |

## Phase 30 Handoff

Phase 30 is ready to be cited as:

> One Phase 29 accepted Loom-bound table artifact drives executable DuckDB SQL
> evidence and deterministic StarRocks-compatible descriptor evidence over a
> bounded query matrix.

It must not be cited as a live StarRocks cluster integration unless optional
runtime smoke is run and reported separately.
