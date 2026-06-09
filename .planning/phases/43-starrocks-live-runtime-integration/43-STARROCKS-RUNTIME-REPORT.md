# Phase 43 StarRocks Runtime Report

## Executive Summary

Phase 43 introduces a typed StarRocks runtime evidence contract and a focused
gate for live StarRocks runtime checks. The local deterministic gate validates
the contract, descriptor identity binding, DuckDB/oracle matrix inheritance,
and fail-closed behavior. A genuine live StarRocks runtime query is accepted
only when explicit runtime/client environment inputs are provided and the
operator-provided table is bound to the accepted Loom artifact SHA-256.

## Live Runtime Evidence Status

No live StarRocks runtime evidence has been collected on this workstation in
the default run. The local environment currently has no `docker`, `mysql`, or
`mariadb` command in PATH. Missing runtime is not accepted evidence.

When a live StarRocks runtime is available, the focused gate requires all of:

- `STARROCKS_MYSQL`
- `STARROCKS_HOST`
- `STARROCKS_PORT`
- `STARROCKS_USER`
- `STARROCKS_PASSWORD`
- `STARROCKS_DATABASE`
- `STARROCKS_TABLE`
- `STARROCKS_LOOM_ARTIFACT_SHA256`

`STARROCKS_LOOM_ARTIFACT_SHA256` must match the accepted descriptor identity
for the generated Loom-bound artifact. This prevents an unrelated StarRocks
table with matching values from becoming accepted Loom runtime evidence.

## Artifact Identity Binding

Runtime evidence is bound to the Phase 29 accepted binding identity:

- table UUID;
- table name;
- schema ID;
- snapshot ID;
- artifact SHA-256;
- row count;
- query kind and projection;
- expected result digest.

The runtime result is accepted only if the observed rows or scalar reproduce the
same digest and concrete expected values.

## Query Matrix

The bounded matrix remains the Phase 30 matrix:

| Query | Expected |
|---|---|
| `SELECT id ... ORDER BY id` | `-1`, `7`, `42` |
| `SELECT id ... WHERE id >= 0 ORDER BY id` | `7`, `42` |
| `SELECT COUNT(*) ...` | `3` |
| `SELECT SUM(id) ...` | `48` |

DuckDB evidence continues to run through public `loom_scan(path)`. StarRocks
runtime evidence runs through the operator-provided live table only after
artifact identity binding is supplied.

## Strict Live Mode

Default local contract mode passes without a live StarRocks runtime, but it
prints that live runtime evidence is missing and not accepted.

Strict live mode is available with:

```bash
LOOM_REQUIRE_STARROCKS_LIVE=1 bash scripts/starrocks-live-runtime-test.sh
```

If any required live runtime input is missing, strict mode fails closed.

## Fail-Closed Matrix

| Case | Result |
|---|---|
| Missing runtime env/client | `missing-runtime`, not accepted |
| Strict live mode with missing env/client | script failure |
| Artifact SHA mismatch | script failure before query evidence |
| Descriptor identity drift | rejected before runtime digest |
| Result digest/value mismatch | mismatch, not accepted |
| Unsupported feature/shape | unsupported, not accepted |
| Descriptor status not accepted | rejected, not accepted |

## Non-Claims

- Local contract mode is not live StarRocks evidence.
- Phase 43 does not add a public StarRocks scan function, catalog route,
  credential route, object-store route, Docker orchestration, JDBC/ODBC client,
  or ABI freeze.
- A live StarRocks table must be prepared by the operator from the accepted
  artifact; this phase does not implement StarRocks data-loading productization.
