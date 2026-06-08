# Phase 28: Iceberg Ref/Table Binding - Research

**Researched:** 2026-06-09
**Domain:** Rust adapter-local Iceberg metadata/ref binding for verifier-backed Loom artifacts
**Confidence:** HIGH for dependency decision and local contract shape; MEDIUM for future Iceberg SDK integration timing

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

## Implementation Decisions

### Binding Surface
- Use a minimal adapter-local Iceberg binding crate or module as the first surface; do not add public SQL functions, public C ABI symbols, DuckDB routes, or StarRocks integration in Phase 28.
- Bind Loom artifacts as explicit sidecar/reference metadata associated with Iceberg table/ref identity rather than embedding Loom bytes into Iceberg manifests or Parquet footers.
- Represent Iceberg identity with bounded, source-neutral fields such as table UUID/name, snapshot ID, schema ID, manifest or metadata file location, and content/hash references where available.
- Keep the generic `loom-source-ingress` contract source-neutral; Iceberg-specific vocabulary belongs only in the Phase 28 adapter/reporting boundary.

### Evidence and Trust Model
- Accepted Iceberg-bound entries require an existing verifier-accepted Loom artifact and must carry the verifier summary forward as evidence, not as an unchecked trust token.
- Reuse Phase 26/27 semantics: accepted bindings require facts plus verifier acceptance plus oracle/equivalence evidence; unsupported valid Iceberg metadata may expose facts but no accepted Loom binding; malformed metadata must expose diagnostics only.
- Treat Iceberg schema/snapshot facts as descriptive until they are matched to verifier-accepted Loom artifact identity and source evidence.
- The binding should be fail-closed on stale snapshot/schema/artifact hash mismatches and on any attempt to treat manifest-only metadata as an accepted artifact proof.

### Scope and Dependency Boundaries
- Research current Iceberg Rust/project APIs before planning; choose dependencies only after primary-source verification.
- Isolate any Iceberg SDK dependency in a source-specific adapter crate; `loom-core`, `loom-ffi`, `loom-source-ingress`, public headers, CLI public route surfaces, and DuckDB host code must remain Iceberg-SDK-free unless explicitly planned as a private test-only guard.
- Local-file metadata/fixture handling is in scope. Remote catalogs, REST catalog auth, object-store credentials, warehouse configuration, production table commits, branch/tag mutation, and snapshot lifecycle management are out of scope.
- Do not broaden Lance/Parquet source compatibility, native kernels, predicate pushdown, split execution, nullable/nested semantic coverage, or full Vortex compatibility in this phase.

### Verification and Release Gate
- Produce a Phase 28 binding report, tentatively `28-ICEBERG-BINDING-REPORT.md`, that records binding schema, accepted/unsupported/rejected matrix, source evidence, verifier evidence, oracle/equivalence evidence, non-goals, and current-phase tradeoffs.
- Add a focused gate, tentatively `scripts/iceberg-binding-test.sh`, and wire it into `scripts/mvp0-verify.sh` only after the focused gate passes.
- The release gate should prove ordering after Phase 27 Lance/Parquet ingress and before any Phase 29 dual-query surface.
- Include negative tests for public-surface creep, object-store/catalog credential creep, unchecked manifest-only success, stale snapshot/schema/hash mismatch, and source SDK leakage.

### Current-Phase Tradeoffs
- Prefer sidecar/reference binding over embedding Loom bytes into Iceberg manifests. This is less integrated but avoids freezing writer internals or depending on manifest mutation semantics before the binding contract is proven.
- Prefer a local fixture and metadata proof over real catalog operations. This keeps Phase 28 deterministic and reviewable while deferring production catalog commit semantics.
- Prefer a narrow accepted primitive/table slice inherited from Phase 27 over broad Iceberg type coverage. This keeps the binding tied to verifier-backed Loom artifacts rather than source-format compatibility claims.
- Prefer adapter-local dependency isolation even if it duplicates some metadata mapping logic. This preserves the core/source-ingress neutrality established in Phase 26.

### the agent's Discretion
- Choose exact crate/module names, fixture formats, and report section names during planning, provided the binding remains narrow, local, verifier-backed, and dependency-isolated.
- Decide whether the first proof uses hand-authored Iceberg metadata fixtures, SDK-generated local metadata fixtures, or a combination, based on current primary-source API research and build reliability.
- Decide the exact mismatch dimensions to test, but include at minimum schema identity, snapshot identity, artifact hash/content identity, and verifier status mismatch.

### Deferred Ideas (OUT OF SCOPE)

## Deferred Ideas

- StarRocks and DuckDB dual query surfaces remain Phase 29.
- Full Vortex semantic compatibility remains Phase 30.
- Production Iceberg catalog commits, REST catalog auth, object-store credentials, branch/tag mutation, and remote warehouse semantics are deferred.
- Embedding Loom bytes into Iceberg manifests or Parquet footers is deferred until sidecar/reference binding is proven.
- Broad Iceberg type coverage, nested/nullable semantics, predicate pushdown, split execution, and new native kernels are deferred.
</user_constraints>

## Project Constraints (from AGENTS.md)

- `loom-core` and `loom-ffi` must remain free of source-format SDKs such as Vortex, Lance, Parquet, and Iceberg except where previous phases explicitly isolated adapters. [VERIFIED: AGENTS.md / PROJECT.md]
- Rust decoder core uses arrow-rs, and the workspace currently pins `arrow`, `arrow-array`, `arrow-schema`, and `arrow-data` exactly to `=58.3.0`. [VERIFIED: Cargo.toml]
- C++ DuckDB integration remains a thin host wrapper over the Rust core via direct DataChunk population and the existing public `loom_scan(path)` surface. [VERIFIED: AGENTS.md / .planning/STATE.md / scripts/mvp0-verify.sh]
- The generic `loom-source-ingress` crate must remain source-neutral and free of source SDK vocabulary, host-engine handles, credentials, and Arrow stream ownership objects. [VERIFIED: crates/loom-source-ingress/src/lib.rs]
- Before file-changing work, use a GSD workflow; this research artifact is being produced inside the GSD research workflow requested for Phase 28. [VERIFIED: AGENTS.md]

## Summary

Phase 28 should implement a narrow adapter-local binding proof, not an Iceberg catalog implementation. The recommended surface is a new private crate such as `loom-iceberg-binding` that reads local Iceberg-style table metadata JSON plus a Loom sidecar/reference JSON, extracts bounded table/ref facts, verifies the referenced Loom artifact with `verify_artifact`, checks source-ingress/oracle evidence, and returns accepted/unsupported/rejected binding reports. [VERIFIED: .planning/phases/28-iceberg-ref-table-binding/28-CONTEXT.md] [VERIFIED: crates/loom-source-ingress/src/lib.rs] [VERIFIED: crates/loom-parquet-ingress/src/source_contract.rs]

The default implementation should not depend on the `iceberg` crate in Phase 28. `iceberg` 0.9.1 is official and current, but docs.rs and `cargo info` show it depends on Arrow/Parquet `^57.1`, while this workspace pins Arrow/Parquet `=58.3.0`; adding it would introduce parallel Arrow families and weaken the workspace's version-unification invariant. [CITED: https://docs.rs/crate/iceberg/latest] [VERIFIED: cargo info iceberg@0.9.1] [VERIFIED: Cargo.toml]

**Primary recommendation:** Use hand-authored local metadata fixtures plus a typed `serde_json` parser in an adapter-local crate; treat the `iceberg` SDK as a quarantined optional cross-check only after a human accepts the Arrow 57/58 churn risk. [CITED: https://docs.rs/serde_json/latest/serde_json/] [VERIFIED: slopcheck 0.6.1 text output] [VERIFIED: cargo info iceberg@0.9.1]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Iceberg metadata/ref fact extraction | Adapter crate | Filesystem fixtures | The phase is local-file metadata proof only; no core, public FFI, DuckDB, or catalog surface owns Iceberg semantics. [VERIFIED: 28-CONTEXT.md] |
| Loom artifact trust decision | `loom-core` artifact verifier | Adapter crate report model | Existing accepted paths call `verify_artifact` before constructing accepted source reports; Phase 28 should preserve that trust gate. [VERIFIED: crates/loom-parquet-ingress/src/source_contract.rs] |
| Source-ingress evidence carry-forward | Adapter crate | `loom-source-ingress` plain-data types | Generic source facts are descriptive and source-neutral; Iceberg-specific vocabulary belongs only in the adapter/report boundary. [VERIFIED: crates/loom-source-ingress/src/lib.rs] |
| Public SQL / DuckDB / StarRocks query | Out of scope | Phase 29 | CONTEXT explicitly forbids public SQL, C ABI, DuckDB routes, and StarRocks work in Phase 28. [VERIFIED: 28-CONTEXT.md] |
| Release gate ordering | Shell scripts | `scripts/mvp0-verify.sh` | Phase 27's gate is already ordered after Phase 26 and before DuckDB smoke; Phase 28 should insert after Phase 27 and before DuckDB smoke or before any future Phase 29 gate. [VERIFIED: scripts/mvp0-verify.sh] |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Local `loom-iceberg-binding` crate/module | new workspace crate | Own Iceberg binding facts, sidecar parsing, accepted/unsupported/rejected reports, and dependency boundary tests | Matches Phase 26/27 adapter pattern and keeps core/source-ingress/public surfaces SDK-free. [VERIFIED: Cargo.toml] [VERIFIED: scripts/lance-parquet-ingress-test.sh] |
| `serde` | `=1.0.228` | Derive or implement typed fixture/sidecar structs | Already pinned in workspace dependencies and slopcheck returned OK. [VERIFIED: Cargo.toml] [VERIFIED: crates.io API] [VERIFIED: slopcheck 0.6.1 text output] |
| `serde_json` | `=1.0.150` | Parse local Iceberg metadata and Loom sidecar/reference JSON fixtures | docs.rs documents typed and untyped JSON parsing; registry and slopcheck checks passed. [CITED: https://docs.rs/serde_json/latest/serde_json/] [VERIFIED: cargo info serde_json@1.0.150] [VERIFIED: slopcheck 0.6.1 text output] |

### Supporting

| Library/Tool | Version | Purpose | When to Use |
|--------------|---------|---------|-------------|
| `iceberg` | `0.9.1` | Official Rust implementation and optional metadata API cross-check | Do not add by default; use only in a quarantined test crate/feature if the planner accepts Arrow 57 duplicate dependencies. [CITED: https://docs.rs/crate/iceberg/latest] [VERIFIED: cargo info iceberg@0.9.1] |
| `shasum` | system `/usr/bin/shasum` | Fixture hash verification in tests | Phase 27 already uses `shasum -a 256` in legacy readability tests; Phase 28 can reuse the pattern for sidecar/artifact hash checks. [VERIFIED: crates/loom-parquet-ingress/tests/legacy_readability.rs] [VERIFIED: command -v shasum] |
| `jq` | `1.8.1` | Optional shell-gate JSON sanity checks | Available locally; Rust tests should remain authoritative. [VERIFIED: jq --version] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Local typed parser | `iceberg = "=0.9.1"` | Stronger spec API and official Rust implementation, but pulls Arrow/Parquet 57.1 into an Arrow 58.3 workspace; this is not worth it for a sidecar binding proof. [CITED: https://docs.rs/crate/iceberg/latest] [VERIFIED: Cargo.toml] |
| Sidecar/reference JSON | Embed Loom bytes in Iceberg metadata/properties/manifests | More integrated packaging, but it freezes writer/manifest mutation semantics before the binding contract is proven and is explicitly deferred. [VERIFIED: 28-CONTEXT.md] |
| Local metadata fixture | MemoryCatalog/FileIO scan path | The docs.rs example proves catalog/table scan APIs exist, but Phase 28 excludes catalog operations, warehouse configuration, and query scans. [CITED: https://docs.rs/iceberg/latest/iceberg/] [VERIFIED: 28-CONTEXT.md] |

**Installation:**

```bash
# Recommended workspace dependency addition only if Phase 28 implementation needs direct JSON parsing:
# Add under [workspace.dependencies] in Cargo.toml:
serde_json = { version = "=1.0.150" }

# Do not add this by default in Phase 28:
# iceberg = { version = "=0.9.1", default-features = false }
```

**Version verification:** `cargo info iceberg@0.9.1 -v` reports `rust-version: 1.92` and Arrow/Parquet `57.1` dependencies; `cargo info serde_json@1.0.150 -v` reports current `serde_json` metadata; docs.rs lists `iceberg` 0.9.1 as latest on 2026-05-06. [VERIFIED: cargo info iceberg@0.9.1] [VERIFIED: cargo info serde_json@1.0.150] [CITED: https://docs.rs/crate/iceberg/latest]

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `serde` | crates.io | created 2014-12-05 | 1,061,595,816 total; 202,540,847 recent | `https://github.com/serde-rs/serde` | OK | Existing workspace dependency; approved. [VERIFIED: crates.io API] [VERIFIED: slopcheck 0.6.1 text output] |
| `serde_json` | crates.io | created 2015-08-07 | 961,698,532 total; 195,046,335 recent | `https://github.com/serde-rs/json` | OK | Approved if local JSON parser is implemented. [VERIFIED: crates.io API] [VERIFIED: slopcheck 0.6.1 text output] |
| `iceberg` | crates.io | created 2021-08-09 | 1,168,175 total; 650,935 recent | `https://github.com/apache/iceberg-rust` | OK | Not selected for default implementation because of Arrow/Parquet 57.1 dependency mismatch; optional quarantined cross-check only. [VERIFIED: crates.io API] [VERIFIED: cargo info iceberg@0.9.1] |

**Packages removed due to slopcheck [SLOP] verdict:** none. [VERIFIED: slopcheck 0.6.1 text output]
**Packages flagged as suspicious [SUS]:** none. [VERIFIED: slopcheck 0.6.1 text output]

Note: local `slopcheck 0.6.1` rejected the documented `--json` flag, so the audit used its supported text output. [VERIFIED: slopcheck install iceberg serde serde_json]

## Architecture Patterns

### System Architecture Diagram

```text
Local Iceberg metadata JSON
        |
        v
Adapter-local Iceberg parser
        |
        +--> malformed JSON / missing required identity -> rejected diagnostics only
        |
        v
Iceberg table/ref facts
        |
        +--> valid metadata but unsupported shape or missing sidecar/artifact evidence -> unsupported facts, no bytes
        |
        v
Loom sidecar/reference metadata
        |
        +--> stale schema/snapshot/artifact hash/verifier status mismatch -> rejected or unsupported, no accepted binding
        |
        v
Read referenced paired Loom artifact
        |
        v
loom_core::artifact_verifier::verify_artifact
        |
        +--> verifier rejected -> rejected/unsupported binding report, no accepted artifact
        |
        v
Check source-ingress and oracle/equivalence evidence
        |
        v
Accepted Iceberg binding report + SourceIngressAcceptedArtifact-style handoff
```

### Recommended Project Structure

```text
crates/
|-- loom-iceberg-binding/
|   |-- src/lib.rs                  # adapter-local binding reports and parser entry points
|   |-- tests/binding_contract.rs   # accepted/unsupported/rejected semantics
|   |-- tests/dependency_boundary.rs# SDK/public-surface leakage guards
|   `-- tests/fixtures/local/       # hand-authored metadata JSON, sidecars, paired .loom artifacts
scripts/
`-- iceberg-binding-test.sh         # focused Phase 28 gate
.planning/phases/28-iceberg-ref-table-binding/
`-- 28-ICEBERG-BINDING-REPORT.md    # final evidence report
```

### Pattern 1: Parse Metadata to Bounded Facts, Not SDK Types

**What:** Deserialize only the Iceberg fields needed for binding identity: `format-version`, `table-uuid`, `location`, `current-schema-id`, `current-snapshot-id`, `snapshots[].snapshot-id`, `snapshots[].manifest-list`, `snapshots[].schema-id`, `refs`, `properties`, and optional data file references when represented by fixture/sidecar evidence. [CITED: https://iceberg.apache.org/spec/] [CITED: https://docs.rs/iceberg/latest/iceberg/spec/struct.TableMetadata.html]

**When to use:** Use this in Phase 28's default local proof, where no catalog, scan, manifest Avro decoding, remote IO, or table commit is in scope. [VERIFIED: 28-CONTEXT.md]

**Example:**

```rust
// Source: Iceberg spec table metadata fields and serde_json docs.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LocalIcebergMetadata {
    format_version: u8,
    table_uuid: String,
    location: String,
    current_schema_id: i32,
    current_snapshot_id: Option<i64>,
    #[serde(default)]
    snapshots: Vec<LocalSnapshot>,
    #[serde(default)]
    refs: std::collections::BTreeMap<String, LocalSnapshotRef>,
    #[serde(default)]
    properties: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LocalSnapshot {
    snapshot_id: i64,
    manifest_list: Option<String>,
    schema_id: Option<i32>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LocalSnapshotRef {
    snapshot_id: i64,
    #[serde(rename = "type")]
    ref_type: String,
}
```

### Pattern 2: Accepted Binding Requires Independent Loom Verification

**What:** A sidecar claim such as `loom_artifact_sha256` or `loom_verifier_status=accepted` is descriptive until the adapter recomputes the artifact hash and runs `verify_artifact` on the referenced bytes. [VERIFIED: crates/loom-source-ingress/src/lib.rs] [VERIFIED: crates/loom-parquet-ingress/src/source_contract.rs]

**When to use:** Always use this before constructing an accepted Iceberg binding report. [VERIFIED: 28-CONTEXT.md]

**Example:**

```rust
// Source: crates/loom-parquet-ingress/src/source_contract.rs accepted handoff pattern.
let verification = verify_artifact(&loom_artifact_bytes, &registry, &Default::default());
if verification.status() != ArtifactVerificationStatus::Accepted {
    return Err(binding_rejected("loom artifact verifier rejected referenced bytes"));
}

let artifact_facts = verification
    .facts()
    .expect("accepted artifact verification exposes facts");
let verifier_summary = format!(
    "{} verifier accepted {}",
    artifact_facts.artifact_kind,
    artifact_facts.payload_kind.as_deref().unwrap_or("unknown payload")
);
```

### Anti-Patterns to Avoid

- **Adding `iceberg` to the workspace default graph:** It would introduce Arrow/Parquet 57.1 dependencies into a workspace whose core invariant is Arrow/Parquet 58.3.0 unification. [CITED: https://docs.rs/crate/iceberg/latest] [VERIFIED: Cargo.toml]
- **Manifest-only success:** Iceberg table metadata, manifest-list locations, or sidecar records must not be accepted without actual paired Loom artifact bytes and verifier/oracle evidence. [VERIFIED: 28-CONTEXT.md] [VERIFIED: 27-ARCHIVAL-READABILITY-REPORT.md]
- **Generic contract vocabulary creep:** Do not add Iceberg-specific names to `loom-source-ingress`; keep them in the adapter/report. [VERIFIED: crates/loom-source-ingress/src/lib.rs] [VERIFIED: 28-CONTEXT.md]
- **Public route creep:** Do not add `loom_scan_iceberg`, C ABI symbols, DuckDB code, CLI public routes, StarRocks code, object-store controls, or catalog credential options. [VERIFIED: 28-CONTEXT.md] [VERIFIED: scripts/source-ingress-contract-test.sh]

## Iceberg Metadata Fields for Binding

| Field | Use in Phase 28 | Trust Level |
|-------|-----------------|-------------|
| `format-version` | Accept only fixture-supported versions, likely v2/v3 table metadata JSON; unsupported valid versions expose facts only. | Descriptive metadata. [CITED: https://iceberg.apache.org/spec/] |
| `table-uuid` | Stable table identity in binding report and mismatch checks. | Descriptive until matched to sidecar and artifact evidence. [CITED: https://iceberg.apache.org/spec/] |
| table name / namespace | Local fixture/report label; not an Iceberg spec table metadata field itself unless provided by sidecar/catalog fixture. | Adapter-local descriptive field. [VERIFIED: 28-CONTEXT.md] |
| `location` / metadata file location | Provenance and path-display evidence; must not trigger remote fetch. | Descriptive local path only. [CITED: https://iceberg.apache.org/spec/] |
| `current-schema-id` | Required schema identity check against sidecar and source evidence. | Fail-closed mismatch dimension. [CITED: https://iceberg.apache.org/spec/] |
| `current-snapshot-id` | Required snapshot identity check when binding a current table state. | Fail-closed mismatch dimension. [CITED: https://iceberg.apache.org/spec/] |
| `snapshots[].snapshot-id` | Resolve current snapshot or named ref target. | Descriptive unless sidecar/artifact matches. [CITED: https://iceberg.apache.org/spec/] |
| `snapshots[].manifest-list` | Bind to manifest-list location as metadata provenance, not as artifact proof. | Facts only. [CITED: https://iceberg.apache.org/spec/] |
| `snapshots[].schema-id` | Cross-check snapshot schema identity; reject stale sidecar/schema mismatch. | Fail-closed mismatch dimension. [CITED: https://docs.rs/iceberg/latest/iceberg/spec/struct.Snapshot.html] |
| `refs` map | Named branch/tag-style snapshot reference facts; no mutation or lifecycle management. | Descriptive ref binding only. [CITED: https://iceberg.apache.org/spec/] |
| `properties` | Optional carrier for Loom reference keys in local fixtures; sidecar file remains preferred. | Descriptive; never trust-only. [CITED: https://iceberg.apache.org/spec/] |
| data file path / record count / file format | Optional fixture-side evidence when not decoding Avro manifests; useful for report row-count provenance. | Facts only unless backed by source-ingress/oracle evidence. [CITED: https://docs.rs/iceberg/latest/iceberg/spec/struct.DataFile.html] |

## Accepted / Unsupported / Rejected Semantics

| Case | Disposition | Required Behavior |
|------|-------------|-------------------|
| Local metadata + sidecar + paired Loom artifact; table UUID, snapshot ID, schema ID, artifact hash, verifier status, source-ingress report, and oracle/equivalence evidence all match | accepted | Produce accepted binding report and handoff bytes only after recomputing hash and rerunning `verify_artifact`. [VERIFIED: 28-CONTEXT.md] [VERIFIED: crates/loom-source-ingress/src/lib.rs] |
| Valid Iceberg metadata with unsupported version, nested/nullable schema, missing sidecar, missing Loom artifact, remote location, or no oracle evidence | unsupported | Expose bounded facts and diagnostics; emit no artifact bytes and no accepted binding. [VERIFIED: 26-SOURCE-INGRESS-CONTRACT.md] |
| Malformed JSON, missing required identity, invalid sidecar schema, unreadable local artifact, stale schema/snapshot/hash mismatch, verifier rejection, or manifest-only accepted claim | rejected or fail-closed unsupported | Expose diagnostics only for malformed/untrusted inputs; never construct accepted report. [VERIFIED: 28-CONTEXT.md] |
| Catalog/REST/object-store/credential/table commit/branch mutation request | out of scope / rejected | No implementation path or public control in Phase 28. [VERIFIED: 28-CONTEXT.md] |

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON parsing | Ad hoc string scanning for metadata fields | `serde_json` typed structs | Official docs support typed deserialization and better error reporting than loose indexing. [CITED: https://docs.rs/serde_json/latest/serde_json/] |
| Artifact trust | A custom verifier-status flag in sidecar | `loom_core::artifact_verifier::verify_artifact` | Existing accepted source adapters already rely on this gate before report acceptance. [VERIFIED: crates/loom-parquet-ingress/src/source_contract.rs] |
| SHA-256 fixture checks | New crypto crate by default | Existing `shasum -a 256` test pattern or small adapter helper if needed | Phase 27 already uses system `shasum` for fixture hash evidence; avoid adding a crypto crate for a local proof. [VERIFIED: crates/loom-parquet-ingress/tests/legacy_readability.rs] |
| Iceberg catalog/storage/scan | A custom catalog or object-store layer | No implementation in Phase 28 | Catalog, warehouse, remote IO, and table commits are out of scope. [VERIFIED: 28-CONTEXT.md] |
| Manifest Avro decoding | Partial Iceberg manifest reader | Sidecar/reference fixture for Phase 28 | The useful proof is binding identity and Loom evidence, not full Iceberg manifest semantics. [VERIFIED: 28-CONTEXT.md] |

**Key insight:** Iceberg metadata can identify a table/ref/snapshot/schema, but Loom trust still comes from a verifier-accepted artifact plus source/oracle evidence; table metadata is not an attestation. [VERIFIED: 28-CONTEXT.md] [VERIFIED: 26-SOURCE-INGRESS-CONTRACT.md]

## Common Pitfalls

### Pitfall 1: Arrow Family Drift

**What goes wrong:** Adding `iceberg` 0.9.1 pulls Arrow/Parquet 57.1 into the workspace while Loom pins Arrow/Parquet 58.3.0. [CITED: https://docs.rs/crate/iceberg/latest] [VERIFIED: Cargo.toml]
**Why it happens:** The official crate is current but not on the same Arrow family as this repo. [VERIFIED: cargo info iceberg@0.9.1]
**How to avoid:** Default to local JSON fixtures; if SDK cross-checks are needed, isolate them behind a test-only crate/feature and add a `cargo tree -d | rg 'arrow|parquet'` gate. [VERIFIED: scripts/lance-parquet-ingress-test.sh]
**Warning signs:** Duplicate `arrow-array`, `arrow-schema`, or `parquet` versions in `cargo tree -d`. [VERIFIED: Cargo.toml]

### Pitfall 2: Treating Iceberg Facts as Trust

**What goes wrong:** A sidecar or table property says a Loom artifact was accepted, and the adapter trusts that string. [VERIFIED: 28-CONTEXT.md]
**Why it happens:** Metadata looks authoritative but can be stale, copied, or malformed. [VERIFIED: 26-SOURCE-INGRESS-CONTRACT.md]
**How to avoid:** Recompute artifact hash, rerun `verify_artifact`, and check source/oracle evidence before acceptance. [VERIFIED: crates/loom-parquet-ingress/src/source_contract.rs]
**Warning signs:** Tests pass when the referenced `.loom` bytes are mutated or when `loom_verifier_status` is manually edited to `accepted`. [VERIFIED: 28-CONTEXT.md]

### Pitfall 3: Accidentally Building Phase 29

**What goes wrong:** The binding proof adds public SQL, DuckDB, StarRocks, or scan routes. [VERIFIED: 28-CONTEXT.md]
**Why it happens:** Iceberg naturally points toward engines and catalogs, but Phase 28 is only the metadata contract. [VERIFIED: .planning/ROADMAP.md]
**How to avoid:** Guard public headers, DuckDB extension code, CLI route files, and source-ingress crate for forbidden markers. [VERIFIED: scripts/lance-parquet-ingress-test.sh]
**Warning signs:** `loom_scan_iceberg`, `iceberg` in public headers, object-store credential keys, or StarRocks config appears outside the adapter. [VERIFIED: scripts/source-ingress-contract-test.sh]

### Pitfall 4: Over-reading Manifests

**What goes wrong:** The phase attempts full Avro manifest-list/data-file semantics instead of proving binding identity. [CITED: https://iceberg.apache.org/spec/] [VERIFIED: 28-CONTEXT.md]
**Why it happens:** Iceberg snapshots refer to manifest-list locations, and `DataFile` has useful file path/format/record count APIs. [CITED: https://docs.rs/iceberg/latest/iceberg/spec/struct.Snapshot.html] [CITED: https://docs.rs/iceberg/latest/iceberg/spec/struct.DataFile.html]
**How to avoid:** Record manifest-list/data-file refs as facts and use sidecar/reference fixtures for accepted Loom binding evidence. [VERIFIED: 28-CONTEXT.md]
**Warning signs:** New Avro readers, table scan APIs, or object-store storage crates appear in the plan. [CITED: https://docs.rs/crate/iceberg/latest]

## Code Examples

### Binding Report Invariant Sketch

```rust
// Source: crates/loom-source-ingress/src/lib.rs accepted report invariants.
pub enum IcebergBindingStatus {
    Accepted,
    Unsupported,
    Rejected,
}

pub struct IcebergBindingFacts {
    pub table_uuid: String,
    pub table_location: String,
    pub ref_name: Option<String>,
    pub snapshot_id: i64,
    pub schema_id: i32,
    pub metadata_location: String,
    pub manifest_list_location: Option<String>,
    pub loom_artifact_path: String,
    pub loom_artifact_sha256: String,
}

pub struct IcebergBindingReport {
    pub status: IcebergBindingStatus,
    pub facts: Option<IcebergBindingFacts>,
    pub source_ingress_report_summary: Option<String>,
    pub verifier_summary: Option<String>,
    pub oracle_summary: Option<String>,
    pub diagnostics: Vec<String>,
}
```

### Dependency Boundary Test Pattern

```rust
// Source: crates/loom-parquet-ingress/tests/dependency_boundary.rs.
#[test]
fn iceberg_sdk_dependency_is_adapter_only_if_present() {
    let root = workspace_root();
    let iceberg = format!("{}{}", "ice", "berg");
    let mut direct_iceberg_manifests = Vec::new();

    for entry in std::fs::read_dir(root.join("crates")).expect("read crates dir") {
        let manifest_path = entry.expect("crate entry").path().join("Cargo.toml");
        if !manifest_path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&manifest_path).expect("read manifest");
        if direct_dep_line_has(&text, &iceberg) {
            direct_iceberg_manifests.push(manifest_path);
        }
    }

    assert!(
        direct_iceberg_manifests.is_empty()
            || direct_iceberg_manifests
                == vec![root.join("crates/loom-iceberg-binding/Cargo.toml")]
    );
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Iceberg Rust core coupled to OpenDAL storage | Trait-based storage with `LocalFsStorage`, `MemoryStorage`, and OpenDAL moved to `iceberg-storage-opendal` | Iceberg Rust 0.9.0 release, 2026-03-10 blog | The core crate is lighter, but storage/catalog work is still unnecessary for Phase 28's local binding proof. [CITED: https://iceberg.apache.org/blog/apache-iceberg-rust-0.9.0-release/] |
| Direct SDK scan/write as first proof | Local sidecar/reference binding proof | Phase 28 project decision | Avoids Arrow 57/58 churn and avoids catalog/query scope creep. [VERIFIED: 28-CONTEXT.md] [CITED: https://docs.rs/crate/iceberg/latest] |

**Deprecated/outdated:**
- Treating Iceberg SDK adoption as harmless in this workspace is outdated for Phase 28 because the latest official crate depends on Arrow/Parquet 57.1 while the workspace pins 58.3.0. [VERIFIED: cargo info iceberg@0.9.1] [VERIFIED: Cargo.toml]

## Non-Goals and Current-Phase Tradeoffs

- No production Iceberg catalog commits, REST catalog auth, branch/tag mutation, snapshot lifecycle management, warehouse configuration, object-store credentials, or remote IO. [VERIFIED: 28-CONTEXT.md]
- No public SQL, public C ABI, DuckDB route, CLI public route, or StarRocks integration. [VERIFIED: 28-CONTEXT.md]
- No broad Iceberg type coverage, nested/nullable semantics, predicate pushdown, split execution, native kernels, or arbitrary Vortex compatibility. [VERIFIED: 28-CONTEXT.md]
- Tradeoff: local sidecar/reference metadata is less integrated than manifest/property embedding but avoids freezing writer internals before the binding contract is proven. [VERIFIED: 28-CONTEXT.md]
- Tradeoff: local parsing duplicates a tiny subset of Iceberg metadata mapping but avoids a known Arrow family mismatch. [VERIFIED: cargo info iceberg@0.9.1] [VERIFIED: Cargo.toml]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| - | No `[ASSUMED]` factual claims are used; recommended names such as `loom-iceberg-binding` are planning recommendations, not factual claims. | All | Planner can proceed without a user-confirmation gate for factual uncertainty. |

## Open Questions (RESOLVED)

1. **Exact sidecar key names**
   - What we know: Sidecar/reference binding is locked, and useful fields are table UUID/name, snapshot ID, schema ID, metadata/manifest locations, artifact path/hash, source-ingress summary, verifier summary, and oracle evidence. [VERIFIED: 28-CONTEXT.md]
   - What's unclear: Final key namespace, for example `loom.artifact.sha256` versus a standalone `loom-binding.json`.
   - RESOLVED default: Use standalone sidecar JSON for Phase 28 and mirror only non-authoritative reference keys in Iceberg `properties` if needed. [VERIFIED: 28-CONTEXT.md]

2. **Whether to include an optional SDK cross-check**
   - What we know: `iceberg` 0.9.1 is official and current but pulls Arrow/Parquet 57.1. [CITED: https://docs.rs/crate/iceberg/latest]
   - What's unclear: Whether a separate quarantined test-only crate is worth the compile/dependency overhead.
   - RESOLVED default: Do not include it in the first implementation plan; add a human checkpoint if a later plan wants SDK comparison. [VERIFIED: Cargo.toml]

3. **Hash source for final sidecar**
   - What we know: Phase 27 uses `shasum -a 256` for fixture hashes and existing runtime cache uses FNV-style internal digests for cache keys. [VERIFIED: crates/loom-parquet-ingress/tests/legacy_readability.rs] [VERIFIED: crates/loom-core/src/runtime_abi.rs]
   - What's unclear: Whether Phase 28 should use only SHA-256 fixture hashes or also record Loom runtime artifact digests.
   - RESOLVED default: Use SHA-256 for sidecar integrity evidence and keep runtime cache digests out of the binding trust model. [VERIFIED: 27-ARCHIVAL-READABILITY-REPORT.md]

4. **Source/oracle evidence artifact for accepted bindings**
   - What we know: Phase 28 requires accepted bindings to carry source/oracle evidence, and Phase 26/27 establish that accepted evidence cannot be a self-asserted trust token. [VERIFIED: 26-SOURCE-INGRESS-CONTRACT.md] [VERIFIED: 27-ARCHIVAL-READABILITY-REPORT.md]
   - What's unclear: Whether accepted binding should validate a Phase 27 source-ingress report, a decoded-row fixture, or both.
   - RESOLVED default: Require a concrete adapter-local decoded-row fixture or source-evidence JSON referenced by the sidecar, read it during binding, and independently verify row count, table UUID, schema ID, snapshot ID, artifact SHA-256, and decoded-row/oracle status before constructing accepted oracle evidence. A sidecar `oracle.accepted = true` flag is necessary only as descriptive input and is never sufficient by itself. [VERIFIED: 28-CONTEXT.md]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust | Workspace crate implementation/tests | yes | `rustc 1.92.0` | none needed. [VERIFIED: rustc --version] |
| Cargo | Dependency verification and tests | yes | `cargo 1.92.0` | none needed. [VERIFIED: cargo --version] |
| `shasum` | Fixture hash checks | yes | `/usr/bin/shasum` | Use existing Rust helper invoking `shasum`; no new crypto crate by default. [VERIFIED: command -v shasum] |
| `jq` | Optional shell JSON checks | yes | `jq-1.8.1` | Rust tests with `serde_json`. [VERIFIED: jq --version] |
| Context7 CLI | Documentation lookup fallback | no | not found | Used primary official docs.rs/Apache docs/web sources instead. [VERIFIED: command -v ctx7] |
| slopcheck | Package legitimacy audit | yes | `0.6.1` | Text mode used because `--json` unsupported locally. [VERIFIED: slopcheck --version] |

**Missing dependencies with no fallback:** none. [VERIFIED: local environment probes]

**Missing dependencies with fallback:** Context7 CLI is missing; primary official docs.rs, Apache Iceberg docs/blogs, GitHub, crates.io API, and `cargo info` covered the needed research. [VERIFIED: command -v ctx7] [CITED: https://docs.rs/crate/iceberg/latest]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | No auth, catalog credentials, REST catalog, or object-store credentials in Phase 28. [VERIFIED: 28-CONTEXT.md] |
| V3 Session Management | no | No sessions or engine service surface. [VERIFIED: 28-CONTEXT.md] |
| V4 Access Control | yes | Dependency and public-surface guards prevent accidental route/catalog/credential expansion. [VERIFIED: scripts/lance-parquet-ingress-test.sh] |
| V5 Input Validation | yes | Use typed `serde_json` parsing, bounded field extraction, stable diagnostics, and fail-closed malformed metadata handling. [CITED: https://docs.rs/serde_json/latest/serde_json/] |
| V6 Cryptography | yes | Recompute SHA-256 fixture/artifact hashes with existing `shasum` pattern; do not hand-roll crypto. [VERIFIED: crates/loom-parquet-ingress/tests/legacy_readability.rs] |

### Known Threat Patterns for Phase 28

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Tampered Loom artifact referenced by valid sidecar | Tampering | Recompute artifact hash and rerun `verify_artifact`; fail closed on mismatch. [VERIFIED: 28-CONTEXT.md] |
| Stale snapshot/schema binding | Tampering / Repudiation | Compare metadata `current-snapshot-id`, ref target, and schema ID to sidecar and source evidence. [CITED: https://iceberg.apache.org/spec/] |
| Metadata-only success claim | Spoofing | Reject accepted binding unless actual `.loom` bytes, verifier summary, and oracle/equivalence evidence are present. [VERIFIED: 26-SOURCE-INGRESS-CONTRACT.md] |
| Credential/config creep | Information Disclosure / Elevation | Gate source for object-store, AWS, secret, REST catalog, and storage option markers in public/host surfaces. [VERIFIED: scripts/lance-parquet-ingress-test.sh] |
| Dependency confusion or slopsquatting | Supply Chain | Use docs.rs/official docs plus slopcheck and registry checks before adding dependencies. [VERIFIED: slopcheck 0.6.1 text output] |

## Validation Architecture

Skipped because `.planning/config.json` sets `workflow.nyquist_validation` to `false`. [VERIFIED: .planning/config.json]

## Sources

### Primary (HIGH confidence)

- Local phase context: `.planning/phases/28-iceberg-ref-table-binding/28-CONTEXT.md` - locked binding, trust, scope, and gate decisions.
- Local source ingress contract: `crates/loom-source-ingress/src/lib.rs` and `26-SOURCE-INGRESS-CONTRACT.md` - accepted/unsupported/rejected invariants and plain-data verifier/oracle handoff.
- Local Phase 27 handoff: `27-ARCHIVAL-READABILITY-REPORT.md`, `crates/loom-parquet-ingress/src/source_contract.rs`, `crates/loom-lance-ingress/src/source_contract.rs`, and `scripts/lance-parquet-ingress-test.sh` - adapter-local accepted emission and gate pattern.
- Apache Iceberg spec: https://iceberg.apache.org/spec/ - table metadata, snapshots, refs, properties, manifest-list fields.
- docs.rs `iceberg` 0.9.1: https://docs.rs/iceberg/latest/iceberg/ and https://docs.rs/crate/iceberg/latest - official crate docs, dependencies, modules, TableMetadata/Snapshot/DataFile APIs.
- Apache Iceberg Rust 0.9.0 release blog: https://iceberg.apache.org/blog/apache-iceberg-rust-0.9.0-release/ - trait-based storage architecture and storage crate split.
- apache/iceberg-rust release: https://github.com/apache/iceberg-rust/releases/tag/v0.9.1 - v0.9.1 release date and official repo.
- docs.rs `serde_json`: https://docs.rs/serde_json/latest/serde_json/ - JSON parsing and typed deserialization APIs.

### Secondary (MEDIUM confidence)

- crates.io API and `cargo info` output - current registry metadata, dependency lists, rust-version, downloads, source repositories.
- Local command probes - Rust/Cargo/shasum/jq/slopcheck availability.

### Tertiary (LOW confidence)

- None. No WebSearch-only or community-only technical claims were used.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - local dependency constraints, docs.rs, crates.io, `cargo info`, and slopcheck all agree.
- Architecture: HIGH - locked CONTEXT and Phase 26/27 local code strongly constrain the shape.
- Pitfalls: HIGH for Arrow mismatch and scope creep; MEDIUM for future SDK timing because the Iceberg crate may move to Arrow 58+ in a later release.

**Research date:** 2026-06-09
**Valid until:** 2026-07-09 for local architecture; re-check `iceberg` crate dependencies before any SDK adoption because the crate is actively releasing.
