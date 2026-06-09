# Phase 30: StarRocks + DuckDB Dual Query Surface - Research

**Researched:** 2026-06-09
**Domain:** Local dual query-surface contract over verifier-accepted Loom/Iceberg binding evidence
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
## Implementation Decisions

### Query Surface Shape

- Keep DuckDB as the real executable host surface: reuse `loom_scan(path)` and
  the existing DuckDB smoke/native gates instead of adding a new public SQL
  function.
- Add a narrow adapter-local StarRocks query-surface proof that emits and
  validates StarRocks-compatible scan/query descriptors over Phase 29 binding
  facts. Do not add a networked StarRocks FE/BE deployment in this phase.
- The shared input must be the Phase 29 local Iceberg binding plus a
  verifier-accepted Loom artifact. DuckDB and StarRocks surfaces must not fork
  the artifact identity, schema identity, snapshot identity, or source/oracle
  evidence model.
- The first query matrix should stay intentionally small: projection,
  filter-like predicate expression, count, sum, and deterministic row material
  checks over the current non-null primitive/table slice.

### Evidence and Trust Model

- Both surfaces must start from accepted Phase 29 binding evidence; sidecar
  claims alone are not sufficient.
- StarRocks-compatible surface output is accepted only when the binding is
  accepted and the generated query/descriptor records the same table UUID,
  schema ID, snapshot ID, artifact hash, row count, and projected columns.
- DuckDB evidence must remain executable through the existing extension path;
  StarRocks evidence may be a local contract/descriptor proof with syntax and
  semantic checks rather than a running cluster.
- Fail closed on stale Phase 29 binding evidence, schema/snapshot mismatch,
  artifact hash mismatch, unsupported remote/catalog/credential routes, and
  unsupported query features.

### Scope and Dependency Boundaries

- Do not add StarRocks server, client, JDBC/ODBC, REST, credential, catalog, or
  object-store dependencies unless research finds a small offline official
  parser/contract surface that does not create service coupling.
- Keep `loom-core`, `loom-ffi`, `loom-source-ingress`, and Phase 29 binding
  semantics source/query-engine neutral. StarRocks-specific vocabulary belongs
  in a Phase 30 adapter/report boundary.
- Do not change the public C ABI or DuckDB public SQL surface unless a plan
  explicitly proves backwards compatibility.
- Do not broaden full Vortex semantic compatibility, nested type coverage,
  distributed execution, predicate pushdown into native kernels, or Iceberg
  catalog commit semantics in this phase.

### Verification and Release Gate

- Produce a Phase 30 report, tentatively
  `30-DUAL-QUERY-SURFACE-REPORT.md`, that records the shared binding input,
  DuckDB executable evidence, StarRocks-compatible contract evidence, mismatch
  matrix, non-goals, and current-phase tradeoffs.
- Add a focused gate, tentatively `scripts/dual-query-surface-test.sh`, and
  wire it into `scripts/mvp0-verify.sh` after Phase 29's Iceberg binding gate
  and before the existing DuckDB smoke closeout.
- Include negative tests for query-surface creep, separate artifact formats,
  StarRocks credential/network routes, unsupported query features, unchecked
  Phase 29 sidecar success, and stale schema/snapshot/artifact identity.
- Keep the release proof deterministic on a clean checkout.

### Current-Phase Tradeoffs

- Prefer a StarRocks-compatible offline contract over a real StarRocks cluster.
  This is less end-to-end than a deployment, but keeps the phase reproducible
  without service orchestration, credentials, object stores, or catalog drift.
- Prefer the existing DuckDB `loom_scan(path)` executable proof over adding a
  new DuckDB table function for Iceberg-bound refs. This preserves the stable
  public surface and keeps Phase 30 focused on shared query behavior.
- Prefer a small query matrix over broad SQL compatibility. This proves the
  dual-surface seam before taking on StarRocks planner/executor semantics.
- Prefer adapter-local duplication of query-surface descriptors over changing
  Phase 29 binding types into a generic engine framework. This avoids freezing
  abstractions before the StarRocks surface shape is validated.

### the agent's Discretion

- Choose exact crate/module names, descriptor JSON shape, and report section
  names during planning.
- Decide whether the StarRocks-compatible proof is a Rust crate, a fixture
  generator, or a test-only module, provided it remains local, deterministic,
  and dependency-contained.
- Decide the exact query matrix, but include at minimum projection, aggregate,
  and fail-closed unsupported query cases.

### Deferred Ideas (OUT OF SCOPE)
## Deferred Ideas

- Running a real StarRocks cluster, FE/BE lifecycle, JDBC/ODBC integration,
  external table registration, credential management, and object-store access.
- Production Iceberg REST catalog integration, branch/tag mutation, table
  commit semantics, and remote warehouse behavior.
- Full SQL dialect compatibility, distributed query execution, predicate
  pushdown into native kernels, split planning across engines, and nested/
  nullable type expansion.
- Full Lance + Parquet + Vortex semantic compatibility is Phase 28.
</user_constraints>

## Summary

Phase 30 should implement an adapter-local dual-surface evidence model, not a StarRocks connector. The shared trust root is the Phase 29 accepted binding path: local Iceberg-style metadata plus Loom sidecar, recomputed artifact SHA-256, live `verify_artifact`, source evidence, decoded-row evidence, and row-count/value checks all matching before artifact bytes are accepted. [VERIFIED: grep]

The default StarRocks proof should be a deterministic descriptor/query contract that records the StarRocks-compatible table reference, projected columns, supported predicate, aggregate intent, expected rows/aggregates, and every Phase 29 identity field. StarRocks official docs describe external Iceberg catalogs and Stream Load as cluster, metastore, storage, network, and credential surfaces, so they are intentionally out of the deterministic default path. [CITED: https://docs.starrocks.io/docs/data_source/catalog/iceberg/iceberg_catalog/] [CITED: https://docs.starrocks.io/docs/loading/StreamLoad/]

DuckDB remains the only required executable engine proof through `loom_scan(path)`, the existing extension build, and `COPY (SELECT ...) TO ...` CSV capture already used by `scripts/duckdb-smoke-test.sh`. [VERIFIED: grep] [CITED: https://duckdb.org/docs/current/sql/statements/copy.html]

**Primary recommendation:** Use `crates/loom-dual-query-surface` as an adapter-local, dependency-light Rust crate over `loom-iceberg-binding`; add `scripts/dual-query-surface-test.sh`; do not add StarRocks runtime/client/catalog dependencies by default. [VERIFIED: grep]

## Project Constraints (from AGENTS.md)

- Rust decoder core, C++ DuckDB extension, Arrow C Data Interface FFI boundary, DuckDB, Apache Arrow, and isolated Vortex boundaries are project constraints. [VERIFIED: grep]
- `loom-core` and `loom-ffi` must remain Vortex-free; Vortex file APIs remain isolated to ingress boundaries. [VERIFIED: grep]
- MVP1 should prefer narrow verifier-gated vertical slices over broad format coverage or unverified execution paths. [VERIFIED: grep]
- GSD workflow says file-changing work should happen through a GSD command; this research was explicitly requested as a GSD phase research artifact. [VERIFIED: grep]
- Project-defined skills were not present in `.codex/skills/` or `.agents/skills/`. [VERIFIED: find]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Phase 29 binding acceptance | Adapter crate / Rust library | `loom-core` verifier | `loom-iceberg-binding` already owns local table/ref binding and calls `verify_artifact`; Phase 30 consumes accepted reports only. [VERIFIED: grep] |
| DuckDB executable evidence | DuckDB extension / host adapter | Rust FFI/runtime | Existing public SQL surface is `loom_scan(path)` and smoke output is captured through DuckDB CLI. [VERIFIED: grep] |
| StarRocks-compatible evidence | Phase 30 adapter/report boundary | Optional external runtime smoke | StarRocks live catalog/load paths require cluster/network/credentials; default proof should be local descriptor validation. [CITED: https://docs.starrocks.io/docs/data_source/catalog/iceberg/iceberg_catalog/] |
| Query equivalence matrix | Phase 30 adapter crate | Scripts/report | The contract compares canonical rows/aggregates across surfaces without changing artifacts. [VERIFIED: grep] |
| Release gate ordering | Shell scripts | Cargo tests | Existing main gate runs Phase 29 before DuckDB smoke; Phase 30 should slot between them. [VERIFIED: grep] |

## Standard Stack

### Core

| Library / Tool | Version | Purpose | Why Standard |
|----------------|---------|---------|--------------|
| `loom-iceberg-binding` | 0.1.0 | Accepted binding facts and verifier-backed artifact bytes | Already implements the Phase 29 trust root and mismatch fail-closed matrix. [VERIFIED: cargo metadata] |
| `loom-source-ingress` | 0.1.0 | Accepted/unsupported/rejected report vocabulary | Existing source-neutral report model keeps adapter facts out of core surfaces. [VERIFIED: cargo metadata] |
| `loom-core` | 0.1.0 | Artifact verifier and decoded row checks | Phase 29 acceptance calls `verify_artifact` and decodes accepted values for evidence matching. [VERIFIED: cargo metadata] |
| `serde` / `serde_json` | `=1.0.228` / `=1.0.150` | Deterministic descriptor JSON parsing/emission | Already workspace-pinned and already used by the Phase 29 adapter. [VERIFIED: cargo metadata] |
| DuckDB CLI/extension | DuckDB `v1.5.3` in script | Required executable query proof | Existing smoke gate builds `loom.duckdb_extension`, loads it, and runs `loom_scan(path)`. [VERIFIED: grep] |

### Supporting

| Library / Tool | Version | Purpose | When to Use |
|----------------|---------|---------|-------------|
| StarRocks | Latest docs track `4.1`; release notes show `4.1.1` on 2026-05-29 | Optional live runtime smoke only | Use only when an operator provides FE/BE/CN, credentials, and a local test database; never require it for the deterministic gate. [CITED: https://docs.starrocks.io/releasenotes/release-4.1/] |
| Docker | 29.2.0 installed locally | Optional runtime smoke orchestration | Available, but Phase 30 should not launch StarRocks by default because context forbids required live deployment. [VERIFIED: command] |
| `curl` | available at `/opt/anaconda3/bin/curl` | Optional Stream Load smoke | Stream Load is HTTP PUT and requires network access to FE/BE. [VERIFIED: command] [CITED: https://docs.starrocks.io/docs/loading/StreamLoad/] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Offline StarRocks descriptor | Real StarRocks Iceberg external catalog | More realistic but requires StarRocks cluster, metastore, storage, and credentials. [CITED: https://docs.starrocks.io/docs/data_source/catalog/iceberg/iceberg_catalog/] |
| Offline StarRocks descriptor | Stream Load to StarRocks table | Useful runtime smoke, but HTTP PUT, INSERT privilege, FE/BE network, and table setup are service dependencies. [CITED: https://docs.starrocks.io/docs/loading/StreamLoad/] |
| Existing DuckDB `loom_scan(path)` | New `loom_scan_iceberg` or `loom_scan_starrocks` | Violates context and expands public SQL surface before equivalence is proven. [VERIFIED: grep] |
| Typed local query descriptor | General SQL parser/transpiler dependency | No small official offline StarRocks parser/contract surface was found in StarRocks docs during this research. [ASSUMED] |

**Installation:**
```bash
# No new external package install is recommended for the default Phase 30 path.
```

**Version verification:** `cargo metadata --no-deps --format-version 1` confirmed local package versions for `loom-core`, `loom-ffi`, `loom-source-ingress`, and `loom-iceberg-binding`. [VERIFIED: cargo metadata]

## Package Legitimacy Audit

No new external packages are recommended for the default implementation, so the package legitimacy gate is not applicable. Existing workspace dependencies are reused through workspace pins. [VERIFIED: cargo metadata]

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| none | n/a | n/a | n/a | n/a | n/a | No new package install |

**Packages removed due to slopcheck [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none.

## Architecture Patterns

### System Architecture Diagram

```text
Phase 29 local metadata + sidecar + artifact path
        |
        v
bind_iceberg_ref_from_paths(metadata, sidecar, artifact)
        |
        +--> reject/unsupported: no bytes, diagnostics only
        |
        v
accepted binding report + verifier-accepted Loom bytes
        |
        +--> DuckDB executable path
        |       |
        |       v
        |   loom_scan(path) -> SELECT projection/filter/count/sum/rows -> canonical CSV
        |
        +--> StarRocks-compatible contract path
                |
                v
            descriptor with table_uuid/schema_id/snapshot_id/artifact_sha256/row_count
                |
                v
            local validation of supported query shape and expected rows/aggregates
                |
                +--> optional StarRocks runtime smoke only if cluster env is present
```

### Recommended Project Structure

```text
crates/loom-dual-query-surface/
├── Cargo.toml                         # adapter-local dependency on loom-iceberg-binding + serde
├── src/lib.rs                         # exports bounded Phase 30 contract
├── src/query_surface.rs               # descriptor, canonical result, mismatch model
└── tests/query_surface_contract.rs    # accepted, mismatch, unsupported feature, boundary tests

scripts/
└── dual-query-surface-test.sh         # focused gate between Iceberg binding and DuckDB smoke

.planning/phases/30-starrocks-duckdb-dual-query-surface/
└── 30-DUAL-QUERY-SURFACE-REPORT.md    # implementation closeout report
```

### Pattern 1: Accepted Binding as Sole Trust Root

**What:** Phase 30 should call `bind_iceberg_ref_from_paths` and proceed only on `IcebergBindingStatus::Accepted`. [VERIFIED: grep]

**When to use:** Every DuckDB or StarRocks-compatible evidence record must start here. [VERIFIED: grep]

**Example:**
```rust
// Source: crates/loom-iceberg-binding/src/binding_contract.rs [VERIFIED: grep]
let accepted = bind_iceberg_ref_from_paths(metadata_path, sidecar_path, artifact_path)?;
let facts = accepted.report.facts.as_ref().expect("accepted facts");
```

### Pattern 2: Descriptor Not SQL Parser

**What:** Represent the query matrix as a typed enum or struct, then emit StarRocks-compatible SQL text as one field of the descriptor. [ASSUMED]

**When to use:** Projection, simple predicate, count, sum, and deterministic row materialization. [VERIFIED: grep]

**Example:**
```rust
// Source: StarRocks SELECT docs + Phase 30 context [CITED: https://docs.starrocks.io/docs/sql-reference/sql-statements/table_bucket_part_index/SELECT/]
struct DualQueryDescriptor {
    engine: QuerySurface,
    table_uuid: String,
    schema_id: i32,
    snapshot_id: i64,
    artifact_sha256: String,
    projection: Vec<String>,
    predicate: Option<SimplePredicate>,
    expected_rows_sha256: String,
    expected_count: u64,
    expected_sum: Option<i64>,
}
```

### Pattern 3: Optional Runtime Smoke Is Explicitly Non-Canonical

**What:** If StarRocks environment variables are present, run a smoke; otherwise print an explicit skip and keep the accepted proof as descriptor-only. [ASSUMED]

**When to use:** Local developer machines or CI jobs that intentionally provision StarRocks. [ASSUMED]

**Example:**
```bash
# Source: Phase 30 context + StarRocks Stream Load docs [CITED: https://docs.starrocks.io/docs/loading/StreamLoad/]
if [ -n "${STARROCKS_FE_HTTP:-}" ] && [ -n "${STARROCKS_MYSQL:-}" ]; then
  echo "[dual-query] optional StarRocks runtime smoke enabled"
else
  echo "[dual-query] optional StarRocks runtime smoke skipped; not accepted StarRocks runtime evidence"
fi
```

### Anti-Patterns to Avoid

- **Treating StarRocks runtime skip as a pass:** A skipped runtime smoke is only a portability diagnostic, not accepted StarRocks evidence. [VERIFIED: grep]
- **Generating query evidence from sidecar flags only:** Phase 29 proves sidecar flags are descriptive until independently checked. [VERIFIED: grep]
- **Adding StarRocks JDBC/ODBC/REST dependencies by default:** Official live paths require service and credential surfaces that the context forbids for the default phase. [CITED: https://docs.starrocks.io/docs/data_source/catalog/iceberg/iceberg_catalog/]
- **Changing `loom_scan(path)`:** Existing DuckDB smoke and user constraints require preserving the public SQL surface. [VERIFIED: grep]
- **Creating a generic engine framework in Phase 30:** Context prefers adapter-local descriptors until the second surface shape is validated. [VERIFIED: grep]

## DuckDB / StarRocks Query Surface Comparison

| Dimension | DuckDB Required Surface | StarRocks Phase 30 Surface | Planning Implication |
|-----------|-------------------------|-----------------------------|----------------------|
| Required runtime | Local DuckDB CLI and loadable extension | None by default | DuckDB is executable evidence; StarRocks is descriptor evidence unless optional env is configured. [VERIFIED: grep] |
| Public entry point | `loom_scan(path)` table function in SQL | Generated StarRocks-compatible `SELECT ... FROM catalog.db.table` descriptor | Do not add new public DuckDB or StarRocks routes. [VERIFIED: grep] |
| Import/load route | Existing Loom artifact path | Deferred; Stream Load is optional smoke only | Stream Load requires HTTP PUT and StarRocks privileges/network. [CITED: https://docs.starrocks.io/docs/loading/StreamLoad/] |
| Iceberg route | Phase 29 binding only, not DuckDB Iceberg extension | StarRocks real external Iceberg catalog deferred | Real StarRocks Iceberg catalogs require metastore/storage access and credentials. [CITED: https://docs.starrocks.io/docs/data_source/catalog/iceberg/iceberg_catalog/] |
| Query matrix | Projection, predicate-like SQL, count, sum, rows through `loom_scan` | Same matrix represented as typed descriptor + StarRocks SQL string | Compare canonical expected rows/aggregates rather than planner internals. [VERIFIED: grep] |
| Failure mode | DuckDB query mismatch fails focused gate | Descriptor mismatch or unsupported query fails closed | Both surfaces must preserve accepted Phase 29 identity. [VERIFIED: grep] |

## API Best Practices

- Keep Phase 30 public API local to `loom-dual-query-surface`; expose no C ABI and no CLI route. [VERIFIED: grep]
- Model query support as an allowlist enum, not freeform SQL; supported operations are projection, one simple filter-like predicate, count, sum, and deterministic rows. [VERIFIED: grep]
- Store both typed query intent and emitted SQL text in descriptors; validate typed intent first, then compare emitted SQL against expected fragments. [ASSUMED]
- Require descriptor identity fields: `table_uuid`, `schema_id`, `snapshot_id`, `artifact_sha256`, `row_count`, `projection`, `surface`, `query_kind`, and expected result digest. [VERIFIED: grep]
- Use deterministic ordering for row materialization; do not rely on engine output order unless the query includes an orderable key. [ASSUMED]
- Include explicit unsupported diagnostics for joins, nested types, nullable expansion, remote catalog routes, credentials, external table DDL, and predicate pushdown into kernels. [VERIFIED: grep]
- Gate optional StarRocks runtime smoke behind env vars such as `STARROCKS_MYSQL`, `STARROCKS_FE_HTTP`, `STARROCKS_USER`, and `STARROCKS_PASSWORD`; never prompt or store credentials. [ASSUMED]

## Dependency Risks

| Risk | Why It Matters | Mitigation |
|------|----------------|------------|
| StarRocks server/client dependency creep | Live StarRocks paths add FE/BE/CN lifecycle, JDBC/ODBC, HTTP, auth, storage, and catalog concerns. [CITED: https://docs.starrocks.io/docs/data_source/catalog/iceberg/iceberg_catalog/] | Default to offline descriptor; gate optional runtime by env and scans. |
| StarRocks 4.1.0 container issue | Official release notes warn container users not to upgrade to 4.1.0 and to wait for 4.1.1. [CITED: https://docs.starrocks.io/releasenotes/release-4.1/] | Do not require containers; if optional smoke uses StarRocks, require 4.1.1+ or a known-good pre-4.1 version. |
| SQL dialect mismatch | StarRocks says `SELECT` basically conforms to SQL92 but has dialect-specific clauses and privileges. [CITED: https://docs.starrocks.io/docs/sql-reference/sql-statements/table_bucket_part_index/SELECT/] | Keep generated SQL tiny and typed; no full SQL compatibility claim. |
| CSV null semantics mismatch | StarRocks local load docs use `\N` for CSV nulls while DuckDB smoke uses `COALESCE(..., 'NULL')` for display. [CITED: https://docs.starrocks.io/docs/loading/StreamLoad/] [VERIFIED: grep] | Descriptor evidence should compare typed values/digests, not display CSV null strings. |
| Existing dirty worktree | Source files were modified before research started. [VERIFIED: git status] | Write only `30-RESEARCH.md`; do not modify source code. |

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Artifact trust | A new query-surface verifier | `bind_iceberg_ref_from_paths` + `verify_artifact` evidence | Phase 29 already owns accepted bytes and mismatch failure. [VERIFIED: grep] |
| SQL parser | A StarRocks parser or transpiler | Typed allowlist descriptor + exact emitted SQL snippets | Full SQL grammar is out of scope; no official small offline parser was found. [ASSUMED] |
| StarRocks deployment | FE/BE/CN orchestration in release gate | Optional smoke only when external env is present | Live StarRocks requires cluster/network/service setup. [CITED: https://docs.starrocks.io/docs/loading/StreamLoad/] |
| DuckDB public surface | New `loom_scan_*` functions | Existing `loom_scan(path)` | Context locks DuckDB to the current public route. [VERIFIED: grep] |
| Engine framework | Generic query-engine abstraction in core | Adapter-local Phase 30 crate | Core/FFI/source-ingress must stay engine-neutral. [VERIFIED: grep] |

**Key insight:** Phase 30 is a trust-preserving equivalence proof, not a connector phase. The value is proving that one accepted artifact/binding identity can drive two query-surface descriptions without expanding runtime dependencies. [VERIFIED: grep]

## Common Pitfalls

### Pitfall 1: StarRocks Descriptor Accepted Without Binding Acceptance

**What goes wrong:** A descriptor records sidecar metadata but never proves accepted artifact bytes. [VERIFIED: grep]
**Why it happens:** Sidecar and metadata fields look complete enough to drive SQL text. [VERIFIED: grep]
**How to avoid:** Require `IcebergBindingStatus::Accepted` and accepted evidence before descriptor construction. [VERIFIED: grep]
**Warning signs:** Tests accept `manifest-only`, `sidecar-only`, or `verifier-status-only` fixtures. [VERIFIED: grep]

### Pitfall 2: Runtime Smoke Becomes Required

**What goes wrong:** CI starts depending on StarRocks FE/BE/CN, MySQL client, or credentials. [ASSUMED]
**Why it happens:** Optional runtime proof gets wired into the main gate as mandatory. [ASSUMED]
**How to avoid:** Make runtime smoke skip explicit and non-accepted; scan for StarRocks network/credential defaults. [VERIFIED: grep]
**Warning signs:** `scripts/mvp0-verify.sh` fails on machines without StarRocks. [ASSUMED]

### Pitfall 3: Query Output Comparison Uses Display CSV Semantics

**What goes wrong:** DuckDB displays `NULL` using `COALESCE`, while StarRocks load docs treat CSV nulls as `\N`; string comparison drifts. [CITED: https://docs.starrocks.io/docs/loading/StreamLoad/] [VERIFIED: grep]
**Why it happens:** Smoke tests naturally capture CSV. [VERIFIED: grep]
**How to avoid:** Compare canonical typed rows or SHA-256 digests derived from accepted artifact values. [VERIFIED: grep]
**Warning signs:** Tests fail only on null rendering or column-order display. [ASSUMED]

### Pitfall 4: Broad SQL Claims

**What goes wrong:** The report claims StarRocks compatibility for joins, external catalogs, pushdown, nested types, or distributed execution. [ASSUMED]
**Why it happens:** StarRocks SELECT supports a broad SQL surface, but Phase 30 only proves a small matrix. [CITED: https://docs.starrocks.io/docs/sql-reference/sql-statements/table_bucket_part_index/SELECT/]
**How to avoid:** State exact supported query kinds and reject everything else. [VERIFIED: grep]
**Warning signs:** Descriptor has freeform SQL accepted path. [ASSUMED]

## Code Examples

### StarRocks Descriptor Shape

```rust
// Source: Phase 30 context + StarRocks SELECT docs
// [VERIFIED: grep] [CITED: https://docs.starrocks.io/docs/sql-reference/sql-statements/table_bucket_part_index/SELECT/]
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StarRocksQueryDescriptor {
    pub status: QuerySurfaceStatus,
    pub table_uuid: String,
    pub table_name: String,
    pub schema_id: i32,
    pub snapshot_id: i64,
    pub artifact_sha256: String,
    pub row_count: u64,
    pub projection: Vec<String>,
    pub query_kind: QueryKind,
    pub starrocks_sql: String,
    pub expected_values_sha256: String,
}
```

### Focused Gate Ordering Check

```bash
# Source: scripts/iceberg-binding-test.sh gate pattern [VERIFIED: grep]
python3 - <<'PY'
from pathlib import Path
text = Path("scripts/mvp0-verify.sh").read_text()
order = [
    "scripts/iceberg-binding-test.sh",
    "scripts/dual-query-surface-test.sh",
    "scripts/duckdb-smoke-test.sh",
]
pos = [text.index(item) for item in order]
assert pos == sorted(pos), pos
PY
```

### DuckDB Executable Query Capture

```bash
# Source: scripts/duckdb-smoke-test.sh + DuckDB COPY docs
# [VERIFIED: grep] [CITED: https://duckdb.org/docs/current/sql/statements/copy.html]
"${DUCKDB_BIN}" -unsigned -c \
  "LOAD '${EXT_PATH}'; COPY (SELECT COUNT(*), SUM(id) FROM loom_scan('${payload}')) TO '${out}' (FORMAT CSV, HEADER FALSE);"
```

## State of the Art

| Old / Larger Approach | Current Phase 30 Approach | When Changed / Source | Impact |
|-----------------------|---------------------------|-----------------------|--------|
| Real StarRocks Iceberg catalog proof | Offline descriptor over accepted Phase 29 binding | Context accepted 2026-06-09. [VERIFIED: grep] | Keeps release gate deterministic and credential-free. |
| New DuckDB route for Iceberg-bound refs | Existing `loom_scan(path)` executable proof | Context accepted 2026-06-09. [VERIFIED: grep] | Avoids public SQL surface churn. |
| Full query-engine abstraction | Adapter-local descriptor crate | Context accepted 2026-06-09. [VERIFIED: grep] | Avoids freezing abstractions before second surface is validated. |
| StarRocks 4.1.0 container runtime | No required runtime; optional 4.1.1+ smoke if used | StarRocks release notes dated 2026-05-29. [CITED: https://docs.starrocks.io/releasenotes/release-4.1/] | Avoids known container startup instability in v4.1.0. |

**Deprecated/outdated:**
- Treating the DuckDB community `arrow` extension as the integration path remains out of scope; project stack says use Loom's extension and Arrow C Data boundary. [VERIFIED: grep]
- Treating Phase 30 as completed evidence is outdated in current planning state; `.planning/STATE.md` records it as skipped/deferred, and this research artifact does not implement it. [VERIFIED: grep]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | No small official offline StarRocks parser/contract library was found, so typed descriptors are preferable. | Standard Stack / Don't Hand-Roll | A missed official parser could simplify syntax validation, but would need dependency review. |
| A2 | Optional StarRocks smoke should be env-gated by conventional endpoint/user/password variables. | API Best Practices | Planner may choose different variable names; gate semantics remain the same. |
| A3 | Deterministic typed row digests are more stable than display CSV for cross-engine comparison. | Common Pitfalls | If implementation only covers non-null Int32, CSV may work initially but will not scale to null/string cases. |

## Resolved Questions For Current DuckDB Slice

1. **Should Phase 30 be reactivated despite `.planning/STATE.md` marking it skipped/deferred?**
   - What we know: The user explicitly requested Phase 30 research now; state says Phase 30 was skipped/deferred on 2026-06-09. [VERIFIED: grep]
   - Resolution: Reactivated only for the DuckDB executable evidence slice after the user cancelled full autonomous execution and asked to finish DuckDB real execution first. Roadmap/state now distinguish DuckDB evidence from incomplete full dual-surface evidence. [VERIFIED: implementation]

2. **Should the descriptor be fixture-only or a new crate?**
   - What we know: Context allows a Rust crate, fixture generator, or test-only module. [VERIFIED: grep]
   - Resolution: Use a small adapter-local crate, `loom-dual-query-surface`, because fixture generation, canonical query evidence, StarRocks-compatible descriptors, DuckDB SQL evidence, and focused shell gates all reuse the same typed contract. [VERIFIED: implementation]

3. **Which optional StarRocks version should runtime smoke allow?**
   - What we know: Latest StarRocks docs are `Latest-4.1`, and release notes warn against 4.1.0 containers while showing 4.1.1 on 2026-05-29. [CITED: https://docs.starrocks.io/releasenotes/release-4.1/]
   - Resolution for current slice: Not implemented. Live StarRocks runtime smoke remains pending/deferred to Plan 30-04/30-05 and is not required for DuckDB executable evidence. [VERIFIED: implementation]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust compiler | Adapter crate/tests | yes | `rustc 1.92.0` | none needed. [VERIFIED: command] |
| Cargo | Adapter crate/tests | yes | `cargo 1.92.0` | none needed. [VERIFIED: command] |
| DuckDB CLI | DuckDB executable smoke | no PATH binary | script downloads `v1.5.3` when `DUCKDB_CLI` is unset | Use existing script cache/download path. [VERIFIED: command] [VERIFIED: grep] |
| Docker | Optional StarRocks runtime smoke | yes | `29.2.0` | Runtime smoke remains optional. [VERIFIED: command] |
| MySQL client | Optional StarRocks SQL smoke | no | n/a | Skip runtime smoke unless user supplies a client or alternate command. [VERIFIED: command] |
| `curl` | Optional Stream Load smoke | yes | path present | Runtime smoke remains optional. [VERIFIED: command] |
| StarRocks FE/BE/CN | Optional runtime smoke | not detected | n/a | Offline descriptor proof is default. [VERIFIED: command] |
| Context7 docs CLI | Documentation lookup | no | n/a | Official docs were fetched directly. [VERIFIED: command] |

**Missing dependencies with no fallback:** none for the recommended default path. [VERIFIED: command]

**Missing dependencies with fallback:** DuckDB CLI is not on PATH, but the existing smoke script downloads/caches DuckDB `v1.5.3`; MySQL/StarRocks runtime is missing and should skip optional StarRocks smoke. [VERIFIED: command] [VERIFIED: grep]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no default | No StarRocks credentials in deterministic path. [VERIFIED: grep] |
| V3 Session Management | no | No web/session surface. [VERIFIED: grep] |
| V4 Access Control | yes for optional runtime | Require explicit user-provided StarRocks credentials and never store them. [CITED: https://docs.starrocks.io/docs/loading/StreamLoad/] |
| V5 Input Validation | yes | Typed descriptor allowlist; reject freeform query features. [ASSUMED] |
| V6 Cryptography | yes | Reuse existing SHA-256 artifact/evidence matching; do not implement custom crypto. [VERIFIED: grep] |

### Known Threat Patterns for This Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Forged sidecar or metadata acceptance | Spoofing/Tampering | Require accepted Phase 29 binding and recomputed hash/verifier/evidence checks. [VERIFIED: grep] |
| Credential leakage in optional StarRocks smoke | Information Disclosure | Keep runtime env-gated, no checked-in credentials, and scan for credential markers. [CITED: https://docs.starrocks.io/docs/data_source/catalog/iceberg/iceberg_catalog/] |
| Query-surface escalation | Elevation of Privilege | Reject remote/catalog/credential routes and unsupported SQL features. [VERIFIED: grep] |
| Report overclaiming skipped smoke | Repudiation | Report skipped runtime separately from accepted descriptor evidence. [VERIFIED: grep] |

## Sources

### Primary (HIGH confidence)

- `.planning/phases/30-starrocks-duckdb-dual-query-surface/30-CONTEXT.md` - locked decisions, scope, tradeoffs, and deferred ideas. [VERIFIED: grep]
- `.planning/phases/30-starrocks-duckdb-dual-query-surface/30-PATTERNS.md` - closest existing code patterns and recommended file shape. [VERIFIED: grep]
- `.planning/phases/29-iceberg-ref-table-binding/29-ICEBERG-BINDING-REPORT.md` - Phase 29 accepted binding contract and handoff. [VERIFIED: grep]
- `crates/loom-iceberg-binding/src/binding_contract.rs` - accepted binding implementation. [VERIFIED: grep]
- `scripts/duckdb-smoke-test.sh` and `scripts/iceberg-binding-test.sh` - executable query and focused gate patterns. [VERIFIED: grep]
- StarRocks Iceberg catalog docs - external catalog behavior, storage/metastore/credential requirements, query examples: https://docs.starrocks.io/docs/data_source/catalog/iceberg/iceberg_catalog/ [CITED: docs.starrocks.io]
- StarRocks Stream Load docs - HTTP PUT load behavior, privileges, network, null CSV behavior: https://docs.starrocks.io/docs/loading/StreamLoad/ [CITED: docs.starrocks.io]
- StarRocks SELECT docs - SELECT scope, privileges, and clauses: https://docs.starrocks.io/docs/sql-reference/sql-statements/table_bucket_part_index/SELECT/ [CITED: docs.starrocks.io]
- StarRocks 4.1 release notes - 4.1.1 release date and 4.1.0 container warning: https://docs.starrocks.io/releasenotes/release-4.1/ [CITED: docs.starrocks.io]
- DuckDB table function docs - table functions in `FROM`, projection pushdown APIs: https://duckdb.org/docs/current/clients/c/table_functions.html [CITED: duckdb.org]
- DuckDB SELECT/COPY docs - query and deterministic CSV export patterns: https://duckdb.org/docs/current/sql/statements/select.html and https://duckdb.org/docs/current/sql/statements/copy.html [CITED: duckdb.org]
- DuckDB C API overview - current stable C API version 1.5.3: https://duckdb.org/docs/current/clients/c/overview.html [CITED: duckdb.org]

### Secondary (MEDIUM confidence)

- Web search results for official StarRocks parser/library availability; no small official offline parser surface was identified. [ASSUMED]

### Tertiary (LOW confidence)

- None used as authoritative support.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - derived from local cargo metadata, Phase 29 code, and official DuckDB/StarRocks docs. [VERIFIED: cargo metadata] [CITED: duckdb.org] [CITED: docs.starrocks.io]
- Architecture: HIGH - constrained by Phase 30 context and existing adapter/gate patterns. [VERIFIED: grep]
- Pitfalls: MEDIUM - main risks are verified locally and in official docs; optional runtime env naming remains assumed. [VERIFIED: grep] [ASSUMED]

**Research date:** 2026-06-09
**Valid until:** 2026-06-16 for StarRocks runtime/version guidance; local adapter recommendations remain valid until Phase 30 context changes.
