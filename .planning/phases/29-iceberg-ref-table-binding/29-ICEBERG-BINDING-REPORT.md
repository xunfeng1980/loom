# Phase 29 Iceberg Binding Report

## Executive Summary

Phase 29 defines a narrow local Iceberg table/ref binding for verifier-backed Loom artifacts. The binding is a bounded evidence claim over local Iceberg-style metadata, a standalone Loom sidecar/reference JSON file, a local `.loom` artifact, live `verify_artifact` acceptance, and sidecar-referenced source/oracle evidence. Iceberg metadata and sidecar accepted flags are descriptive only; they are not trust tokens and cannot return bytes by themselves.

The default implementation intentionally does not add the official `iceberg` crate. Phase research found the current Rust SDK useful for later catalog work but mismatched with this workspace's Arrow/Parquet 58.3 pin, so Plan 28 keeps a typed `serde_json` local fixture parser in `loom-iceberg-binding`.

## Implemented Artifacts

| Artifact | Role |
|---|---|
| `crates/loom-iceberg-binding` | Adapter-local crate that owns Iceberg binding vocabulary and keeps it out of core, FFI, source-neutral, DuckDB, CLI, and public-header surfaces. |
| `crates/loom-iceberg-binding/src/binding_contract.rs` | Typed local metadata/sidecar parser, accepted/unsupported/rejected report model, verifier-backed binding handoff, SHA-256 check, and source/oracle evidence validation. |
| `crates/loom-iceberg-binding/tests/binding_contract.rs` | Report invariants and parser classification coverage. |
| `crates/loom-iceberg-binding/tests/binding_handoff.rs` | Accepted binding handoff and existing fail-closed hash/evidence coverage. |
| `crates/loom-iceberg-binding/tests/mismatch_fail_closed.rs` | D-08/D-15 mismatch matrix for schema, snapshot, table identity, hash, verifier status, malformed bytes, missing evidence, stale evidence, forged oracle flags, public-scope creep, and manifest-only claims that must not be accepted as evidence. |
| `crates/loom-iceberg-binding/tests/fixtures/local/*.json` | Local metadata, sidecar, accepted evidence, unsupported/rejected metadata, and mismatch/static stale evidence fixtures. |
| `scripts/iceberg-binding-test.sh` | Focused Phase 29 gate; Plan 29-04 expands it to cover mismatch tests and report markers. |

## Binding Schema

The implemented binding schema is a local sidecar/reference model. Iceberg table metadata supplies bounded identity fields:

| Field | Source | Trust role |
|---|---|---|
| `table-uuid` | Iceberg-style metadata JSON | Descriptive table identity until matched against sidecar and evidence. |
| `loom.table.name` | Metadata properties | Adapter-local table label for reports and decoded-row evidence identity. |
| `current-schema-id` | Metadata JSON | Required schema identity; stale sidecars fail closed. |
| `current-snapshot-id` and `refs` | Metadata JSON | Required snapshot/ref identity; stale refs fail closed. |
| `snapshots[].manifest-list` | Metadata JSON | Provenance only; manifest-list records must not be accepted without artifact bytes and evidence. |
| `loom_artifact_path` | Loom sidecar JSON | Local artifact reference; explicit function argument must match it. |
| `loom_artifact_sha256` | Loom sidecar JSON | Descriptive hash claim; binder recomputes SHA-256 before acceptance. |
| `source_oracle_evidence_path` | Loom sidecar JSON | Local evidence artifact reference; binder parses and validates it independently. |

Accepted binding requires D-05/D-08 conditions: metadata and sidecar identity match, local artifact path matches, SHA-256 is recomputed from bytes, `verify_artifact` accepts the bytes, source evidence is accepted, decoded-row oracle evidence is accepted, evidence row count matches the verified Loom artifact row count, and all table UUID/schema/snapshot/hash fields match.

## Accepted Unsupported Rejected Matrix

| Case | Disposition | Byte rule |
|---|---|---|
| Local metadata + sidecar + local Loom artifact + matching source/oracle evidence + verifier accepted | Accepted | Returns artifact bytes with accepted binding report. |
| Valid local metadata without complete verifier/source/oracle evidence | Unsupported | Facts may be present; no artifact bytes. |
| Remote metadata location, REST/catalog marker, warehouse/object-store/credential marker | Unsupported or rejected fail-closed | No artifact bytes and no accepted evidence. |
| Malformed JSON, missing table UUID, missing schema/snapshot/ref identity | Rejected | Diagnostics only; no trusted facts. |
| Schema/snapshot/table UUID/ref mismatch between metadata and sidecar | Rejected or unsupported fail-closed | No accepted binding and no bytes. |
| Artifact hash mismatch, verifier-rejected bytes, missing evidence file, stale source evidence, forged decoded-row oracle flags | Unsupported fail-closed | No accepted binding and no bytes. |
| Manifest-only, sidecar-only, metadata-only, verifier-status-only, source-evidence-only, or oracle-accepted-flag-only claim | Fail-closed | Must not be accepted as proof or evidence without the full independent checks. |

## Mismatch Fail-Closed Matrix

| Dimension | Fixture/Test | Expected result |
|---|---|---|
| Stale schema | `mismatch-schema-sidecar.json`; `schema_snapshot_table_and_artifact_mismatches_return_no_bytes` | Sidecar schema ID mismatch returns no accepted bytes. |
| Stale snapshot/ref target | `mismatch-snapshot-sidecar.json`; `schema_snapshot_table_and_artifact_mismatches_return_no_bytes` | Snapshot ID mismatch returns no accepted bytes. |
| Table UUID/name mismatch | Dynamic sidecar mutation in `mismatch_fail_closed.rs` | Table identity mismatch returns no accepted bytes. |
| Artifact SHA-256 mismatch | Dynamic sidecar mutation in `mismatch_fail_closed.rs` | Recomputed hash mismatch returns no accepted bytes. |
| Sidecar verifier status mismatch | `verifier_status_rejected_bytes_and_missing_evidence_return_no_bytes` | Rejected status in sidecar cannot force acceptance. |
| Live verifier rejection | Malformed local artifact bytes in `mismatch_fail_closed.rs` | `verify_artifact` rejection returns no accepted bytes. |
| Missing source evidence | Dynamic sidecar with missing `source_evidence` | No accepted binding. |
| Missing oracle evidence | Dynamic sidecar with missing `oracle_evidence` | No accepted binding. |
| Stale source evidence | `stale-source-evidence.json` | Row-count evidence stale against the verified artifact returns no accepted bytes. |
| Forged decoded-row/oracle evidence | `forged-oracle-evidence.json` | Accepted flags alone return no accepted bytes when independent row-count evidence is stale. |
| Manifest-only sidecar | `manifest-only-sidecar.json` | Manifest-only and verifier-status-only claims must not be accepted. |
| Remote/catalog/object-store/credential creep | `unsupported-remote-metadata.json` and public-surface scans | Unsupported or rejected; no public route, catalog, credential, pushdown, split, or native-kernel surface. |

## Source Evidence

The accepted evidence fixture is `accepted-table-source-evidence.json`. The binder checks these fields before acceptance:

- `row_count`
- `table_uuid`
- `schema_id`
- `snapshot_id`
- `artifact_sha256`
- `source.accepted`
- `source.status`
- decoded-row fixture row count and identity

The source evidence artifact is descriptive until it is read from the sidecar-referenced path and matched against Iceberg table/ref identity, recomputed artifact SHA-256, and verified artifact row count. A source accepted flag alone is not proof.

## Verifier Evidence

The accepted path reads the referenced `.loom` bytes and calls `loom_core::artifact_verifier::verify_artifact` with the current MVP0 registry. Accepted binding reports use `SourceArtifactVerificationSummary::accepted` only after the live verifier accepts the artifact and the byte length is non-zero. Sidecar verifier status is required as a descriptive cross-check but must not be accepted by itself.

## Oracle Evidence

Phase 29 uses `SourceOracleStrategy::DecodedRowFixture`. The decoded-row evidence is stored in `accepted-table-source-evidence.json` and must match the table UUID, schema ID, snapshot ID, artifact SHA-256, expected identity string, oracle strategy, row count, accepted status, and oracle accepted status. Forged decoded-row/oracle evidence with accepted flags is rejected if independent row-count, identity, hash, or status checks do not match.

## Dependency and API Boundary

`loom-iceberg-binding` depends on `loom-core`, `loom-source-ingress`, `serde`, and `serde_json`. It has test-only Arrow dev dependencies for fixture artifact generation. It does not add the `iceberg` crate by default.

The following surfaces remain Iceberg-SDK-free and query-surface-free in Phase 29:

- `loom-core`
- `loom-ffi`
- `loom-source-ingress`
- public headers
- DuckDB extension code
- CLI public routes

The focused gate scans for official SDK default dependency creep, public SQL/API route markers, REST/catalog/warehouse/object-store credential controls, branch/tag mutation controls, pushdown/split execution controls, and new native-kernel markers.

## Current-Phase Tradeoffs

Phase 29 uses a sidecar/reference binding instead of embedding Loom bytes in Iceberg manifests or Parquet footers. This is less integrated, but it avoids freezing writer internals or manifest mutation semantics before the binding trust model is proven.

Phase 29 uses local JSON metadata fixtures instead of production catalog operations. This keeps tests deterministic and reviewable, but it defers REST catalog auth, object-store credentials, table commits, and snapshot lifecycle management.

Phase 29 keeps the accepted shape narrow and inherited from verifier-backed Loom artifacts. This avoids broad Iceberg type-coverage claims and keeps the binding tied to current artifact-verifier evidence.

Phase 29 duplicates a small amount of metadata mapping logic in an adapter-local crate. That duplication is intentional because the generic source-ingress contract must remain source-neutral.

Phase 29 does not add the official `iceberg` crate by default. The tradeoff is less SDK coverage today in exchange for preserving Arrow/Parquet version unification and avoiding catalog/API churn in a local binding proof.

## Non-Goals

- No StarRocks or DuckDB query surfaces.
- No public `loom_scan_iceberg`, CLI ingest route, C ABI symbol, or host-engine adapter.
- No production Iceberg catalog, REST auth, warehouse config, table commit, branch/tag mutation, or snapshot lifecycle management.
- No object-store credentials, storage options, cloud credentials, or remote fetch.
- No embedding Loom bytes into Iceberg manifests, Parquet footers, or source metadata as an accepted artifact proof.
- No broad Iceberg type coverage, nested/null semantic claims, predicate pushdown, split execution, or new native kernels.
- No full Vortex compatibility claim.

## Release Gate Evidence

Final Plan 29-05 closeout evidence:

| Command | Status |
|---|---|
| `bash -n scripts/iceberg-binding-test.sh` | passed during Plan 29-05 closeout |
| `bash scripts/iceberg-binding-test.sh` | passed during Plan 29-05 closeout and again through `scripts/mvp0-verify.sh` |
| `bash -n scripts/mvp0-verify.sh` | passed during Plan 29-05 closeout |
| `python3 -c 'from pathlib import Path; text=Path("scripts/mvp0-verify.sh").read_text(); order=["scripts/source-ingress-contract-test.sh","scripts/lance-parquet-ingress-test.sh","scripts/iceberg-binding-test.sh","scripts/duckdb-smoke-test.sh"]; pos=[text.index(x) for x in order]; assert pos == sorted(pos), pos'` | passed; confirms Phase 29 runs after Phase 27 and before DuckDB smoke |
| `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp0-verify.sh` | passed during Plan 29-05 closeout |

The main release verifier now invokes `scripts/iceberg-binding-test.sh` after `scripts/lance-parquet-ingress-test.sh` and before `scripts/duckdb-smoke-test.sh`. This preserves the Phase 29 boundary: no Iceberg SQL route, DuckDB route, CLI route, public C ABI symbol, StarRocks surface, production catalog control, object-store credential handling, branch/tag mutation, or official `iceberg` SDK default dependency was added.

## Phase 29 Handoff

Phase 29 may consume this report as the table/ref binding contract for later dual query-surface planning. The safe handoff is: an Iceberg-bound Loom artifact is accepted only when the local metadata, sidecar reference, actual artifact bytes, recomputed hash, live verifier result, source evidence, and decoded-row oracle evidence all match. Phase 29 should treat Iceberg metadata as table/ref identity and planning context, not as a bypass around Loom artifact verification.
