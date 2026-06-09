# Phase 43 ABI Findings

## Summary

Phase 43 tested the current runtime/query surface against a second-consumer
shape. The StarRocks consumer can validate descriptor-bound runtime output, but
the current runtime ABI is still visibly shaped by the first DuckDB host.

No ABI freeze should happen until Phase 44 consumes these findings.

## Findings

| Area | Finding | Classification | Phase 44 Action |
|---|---|---|---|
| Scan identity | The public executable surface is still path-oriented (`loom_scan(path)` in DuckDB; operator-provided table name in StarRocks). | Phase 44 input | Define a host-neutral artifact handle or descriptor binding rule. |
| Artifact binding | StarRocks evidence needs explicit artifact SHA binding because a table name alone cannot prove Loom provenance. | Fixed now | Phase 43 gate requires `STARROCKS_LOOM_ARTIFACT_SHA256`. |
| Result evidence | DuckDB evidence is executable through the extension; StarRocks evidence is validated from runtime rows/scalars plus descriptor identity. | Accepted asymmetry | Freeze the evidence model as host-neutral, not the invocation mechanics. |
| Projection | Current query matrix supports `id` projection only. DuckDB SQL can express more, but accepted descriptors reject projection drift. | Phase 44 input | Version projection semantics before broader SQL. |
| Predicate | Current matrix supports `id >= 0` as a surface query, not native predicate pushdown. | Accepted asymmetry | Keep predicate pushdown out of ABI v1 unless cross-engine evidence expands. |
| Lifecycle | DuckDB maps naturally to bind/init/scan/release. StarRocks runtime evidence is query-result validation over an externally prepared table. | Phase 44 input | Separate runtime lifecycle ABI from query evidence collection. |
| Memory ownership | DuckDB consumes Arrow C Data release callbacks. StarRocks runtime evidence observes SQL rows/scalars and does not adopt Arrow buffers. | Phase 44 input | Freeze Arrow/raw-buffer ownership for host adapters while allowing non-Arrow row-result evidence. |
| Concurrency | Current native runtime remains single-worker/single-batch for supported evidence. StarRocks distributed execution is unsupported. | Accepted asymmetry | Keep distributed execution out of ABI v1. |
| Diagnostics | DuckDB exposes route diagnostics through extension errors. StarRocks evidence uses typed runtime evidence statuses and script failures. | Phase 44 input | Normalize host-neutral diagnostic codes and leave host formatting adapter-local. |
| Credentials/catalog | StarRocks live checks need env-provided client/connection data, but no public Loom credential/catalog route exists. | Accepted asymmetry | Keep credentials outside ABI v1; document operator-provided runtime setup. |

## Fixed Now

- Added typed runtime evidence statuses:
  - `accepted`
  - `missing-runtime`
  - `unsupported`
  - `rejected`
  - `mismatch`
- Added artifact SHA binding before accepting live StarRocks runtime rows.
- Added strict live mode so missing runtime configuration fails closed when live
  evidence is required.
- Preserved dependency boundaries: no default StarRocks/MySQL/Docker/JDBC/ODBC
  client dependency was added.

## Accepted Asymmetries

- DuckDB remains the only local executable host that consumes Loom bytes
  directly through `loom_scan(path)`.
- StarRocks live evidence requires an operator-prepared table plus artifact SHA
  binding, because this phase does not productize StarRocks data loading.
- Distributed execution, external table DDL, remote catalog, object-store
  credentials, and predicate pushdown remain unsupported.

## Phase 44 Inputs

Phase 44 should freeze only the host-neutral pieces that survived this
second-consumer check:

- artifact identity and accepted binding facts;
- result-evidence records;
- typed fail-closed statuses and diagnostics;
- projection/predicate subset semantics;
- explicit unsupported distributed/runtime-loading features;
- a versioned boundary between runtime invocation and evidence validation.

Phase 44 should not freeze `loom_scan(path)` as the universal ABI shape.
