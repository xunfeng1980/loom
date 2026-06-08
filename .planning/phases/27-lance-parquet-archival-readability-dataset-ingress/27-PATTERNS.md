# Phase 27: Lance + Parquet Archival Readability / Dataset Ingress - Pattern Map

**Mapped:** 2026-06-09  
**Files analyzed:** 24 required files plus Phase 26 handoff docs  
**Analogs found:** 13 / 13 likely deliverable groups

## Existing Patterns

Phase 27 should copy the Phase 26 adapter shape, not expand the generic contract. `loom-source-ingress` is dependency-light and owns only Loom vocabulary:

```rust
// crates/loom-source-ingress/src/lib.rs:1-4
//! Source-neutral external-source ingress contract for Loom.
//!
//! This crate intentionally owns only Loom contract vocabulary. Source-specific
//! SDKs and artifact verifier implementations stay in adapter crates.
```

The workspace isolates source SDKs in source-specific crates. `loom-vortex-ingress` is the current example:

```toml
# crates/loom-vortex-ingress/Cargo.toml:7-16
[dependencies]
arrow-schema = { workspace = true }
loom-core = { path = "../loom-core" }
loom-source-ingress = { path = "../loom-source-ingress" }
vortex-array = { workspace = true }
vortex-buffer = "=0.74.0"
vortex-file = "=0.74.0"
vortex-io = "=0.74.0"
vortex-layout = "=0.74.0"
vortex-session = "=0.74.0"
```

Do the same for Lance and Parquet: adapter crate(s) own source SDK dependencies; `loom-core`, `loom-ffi`, `loom-source-ingress`, DuckDB extension code, and public headers stay source-SDK free.

The source contract has a hard accepted/unsupported/rejected split:

```rust
// crates/loom-source-ingress/src/lib.rs:451-489
impl SourceIngressReport {
    pub fn accepted(
        facts: SourceFacts,
        emission_kind: SourceEmissionKind,
        emission_disposition: SourceEmissionDisposition,
        lowering_disposition: SourceLoweringDisposition,
        artifact_verification: SourceArtifactVerificationSummary,
        oracle_evidence: SourceOracleEvidence,
    ) -> Result<Self, SourceIngressReportError> {
        if !matches!(
            emission_kind,
            SourceEmissionKind::Lmp1 | SourceEmissionKind::Lmt1
        ) {
            return Err(SourceIngressReportError::MissingArtifactEmission);
        }

        if !artifact_verification.accepted
            || !artifact_verification.required
            || artifact_verification.artifact_byte_len.is_none()
        {
            return Err(SourceIngressReportError::ArtifactVerificationNotAccepted);
        }

        if !oracle_evidence.accepted {
            return Err(SourceIngressReportError::OracleEvidenceNotAccepted);
        }
```

Unsupported/rejected reports must be byte-free and oracle-free:

```rust
// crates/loom-source-ingress/src/lib.rs:491-521
pub fn unsupported(facts: Option<SourceFacts>, diagnostic: SourceDiagnostic) -> Self {
    ...
    emission_kind: SourceEmissionKind::None,
    emission_disposition: SourceEmissionDisposition::None,
    lowering_disposition: SourceLoweringDisposition::FailClosedDeferred,
    artifact_verification: SourceArtifactVerificationSummary::not_applicable(),
    oracle_evidence: None,
}

pub fn rejected(identity: SourceIdentity, diagnostic: SourceDiagnostic) -> Self {
    ...
    facts: None,
    emission_kind: SourceEmissionKind::None,
    emission_disposition: SourceEmissionDisposition::None,
    artifact_verification: SourceArtifactVerificationSummary::not_applicable(),
    oracle_evidence: None,
}
```

## Closest Analogs

| Likely Phase 27 Deliverable | Role | Data Flow | Closest Existing Analog | Match Quality |
|---|---|---|---|---|
| `crates/loom-lance-ingress/Cargo.toml` | config/crate | dependency isolation | `crates/loom-vortex-ingress/Cargo.toml` | exact role |
| `crates/loom-parquet-ingress/Cargo.toml` | config/crate | dependency isolation | `crates/loom-vortex-ingress/Cargo.toml` | exact role |
| workspace `Cargo.toml` member additions | config | workspace registration | `Cargo.toml` lines 3-12 | exact role |
| Lance adapter public module exports | adapter/module | request-response/file-I/O | `crates/loom-vortex-ingress/src/lib.rs` lines 1-16 | exact role |
| Parquet adapter public module exports | adapter/module | request-response/file-I/O | `crates/loom-vortex-ingress/src/lib.rs` lines 1-16 | exact role |
| Lance source contract mapping | adapter/service | file-I/O -> transform | `crates/loom-vortex-ingress/src/source_contract.rs` lines 37-132 | exact flow |
| Parquet source contract mapping | adapter/service | file-I/O -> transform | `crates/loom-vortex-ingress/src/source_contract.rs` lines 37-132 | exact flow |
| Lance facts extraction | adapter/service | source SDK metadata -> facts | `crates/loom-vortex-ingress/src/lib.rs` lines 481-537 and 656-895 | role match |
| Parquet facts extraction | adapter/service | footer/schema/row-group facts -> facts | `crates/loom-vortex-ingress/src/lib.rs` lines 450-479 and 846-895 | role match |
| Arrow-compatible row canonicalization | adapter/utility | Arrow rows -> LMP1/LMT1 | `crates/loom-vortex-ingress/src/lib.rs` lines 964-1251 | exact flow |
| Artifact verifier handoff | adapter/service | candidate bytes -> accepted report | `crates/loom-vortex-ingress/src/source_contract.rs` lines 67-132 | exact flow |
| Focused adapter tests | test | accepted/unsupported/rejected | `crates/loom-vortex-ingress/tests/source_ingress_handoff.rs` lines 123-315 | exact flow |
| Phase gate script | script/config | batch gate | `scripts/source-ingress-contract-test.sh` lines 1-249 | exact role |

## Recommended File/Crate Map

Prefer two source-specific crates unless research finds a strong reason to share implementation:

| File / Directory | Role | Data Flow | Pattern To Copy |
|---|---|---|---|
| `crates/loom-lance-ingress/` | adapter crate | file-I/O, transform | `crates/loom-vortex-ingress/` |
| `crates/loom-lance-ingress/src/lib.rs` | adapter API | request-response | Export `source_contract` helpers like `loom-vortex-ingress/src/lib.rs:7-16`. |
| `crates/loom-lance-ingress/src/source_contract.rs` | mapping/service | source facts -> report | Copy function family shape from `source_contract.rs:37-132`. |
| `crates/loom-lance-ingress/tests/source_ingress_contract.rs` | test | contract mapping | Copy assertions from `loom-vortex-ingress/tests/source_ingress_contract.rs:75-158`, adjusted to Lance facts. |
| `crates/loom-lance-ingress/tests/source_ingress_handoff.rs` | test | verifier/oracle handoff | Copy `source_ingress_handoff.rs:123-315`. |
| `crates/loom-parquet-ingress/` | adapter crate | file-I/O, transform | Same crate isolation pattern as Vortex ingress. |
| `crates/loom-parquet-ingress/src/lib.rs` | adapter API | request-response | Export source-neutral helper names, source-specific internally. |
| `crates/loom-parquet-ingress/src/source_contract.rs` | mapping/service | source facts -> report | Copy Vortex report/facts/handoff structure. |
| `crates/loom-parquet-ingress/tests/source_ingress_contract.rs` | test | contract mapping | Copy status/coverage/facts assertions. |
| `crates/loom-parquet-ingress/tests/source_ingress_handoff.rs` | test | verifier/oracle handoff | Copy accepted LMP1/LMT1, unsupported, rejected tests. |
| `scripts/lance-parquet-ingress-test.sh` | script | batch verification | Copy `scripts/source-ingress-contract-test.sh` structure and add Lance/Parquet guard markers. |
| `.planning/phases/27-.../27-ARCHIVAL-READABILITY-REPORT.md` | docs/report | release evidence | Copy report sections from `26-SOURCE-INGRESS-REPORT.md:26-205`. |

Do not add Lance/Parquet symbols to `loom-source-ingress`; its own tests intentionally forbid source-specific public vocabulary:

```rust
// crates/loom-source-ingress/tests/source_ingress_contract.rs:95-112
fn contract_sources_do_not_contain_source_specific_public_vocabulary() {
    ...
    for marker in forbidden_source_markers() {
        assert!(
            !source.contains(&marker),
            "source contract API leaked marker {marker}"
        );
```

## Test Patterns

Accepted LMP1/LMT1 handoff should mirror the current Vortex tests:

```rust
// crates/loom-vortex-ingress/tests/source_ingress_handoff.rs:123-157
#[test]
fn accepted_single_column_handoff_is_verifier_routed_lmp1() {
    let vortex = vortex_file_bytes(buffer![7i32, -1, 42]);
    let accepted =
        emit_source_ingress_lmc1_from_vortex_buffer(&vortex).expect("accepted source handoff");

    assert!(!accepted.bytes.is_empty());
    assert_emitted_artifact_is_verifier_accepted(&accepted.bytes);
    assert_eq!(accepted.report.status, SourceIngressStatus::Accepted);
    assert_eq!(accepted.report.emission_kind, SourceEmissionKind::Lmp1);
    assert_eq!(
        accepted.report.emission_disposition,
        SourceEmissionDisposition::CanonicalRaw
    );
```

For table emission, copy the `LMT1` equivalent:

```rust
// crates/loom-vortex-ingress/tests/source_ingress_handoff.rs:160-193
#[test]
fn accepted_table_handoff_is_verifier_routed_lmt1() {
    let vortex = supported_table_bytes();
    let accepted =
        emit_source_ingress_lmc1_from_vortex_buffer(&vortex).expect("accepted source handoff");

    assert!(!accepted.bytes.is_empty());
    assert_emitted_artifact_is_verifier_accepted(&accepted.bytes);
    assert_eq!(accepted.report.status, SourceIngressStatus::Accepted);
    assert_eq!(accepted.report.emission_kind, SourceEmissionKind::Lmt1);
```

Oracle evidence must be explicit and row-equivalent. For Lance/Parquet, use `SourceOracleStrategy::ArrowScan` if the source reader yields Arrow batches; use `SourceOracleStrategy::SourceNativeScan` only if the adapter really scans through the source SDK:

```rust
// crates/loom-vortex-ingress/tests/source_ingress_handoff.rs:196-216
let oracle = accepted
    .report
    .oracle_evidence
    .as_ref()
    .expect("source oracle evidence");
assert_eq!(oracle.strategy, SourceOracleStrategy::SourceNativeScan);
assert!(oracle.accepted);
assert_eq!(oracle.row_count_checked, Some(3));
assert!(oracle.nulls_checked);
assert!(oracle.source_native_scan_used);
assert_eq!(decode_single_i32_values(&accepted.bytes), vec![7, -1, 42]);
```

Unsupported valid cases must retain facts but emit no bytes:

```rust
// crates/loom-vortex-ingress/tests/source_ingress_handoff.rs:244-270
let report = emit_source_ingress_lmc1_from_vortex_buffer(&vortex)
    .expect_err("unsupported source report");

assert_eq!(report.status, SourceIngressStatus::Unsupported);
assert!(report.facts.is_some());
assert_eq!(report.emission_kind, SourceEmissionKind::None);
assert_eq!(report.emission_disposition, SourceEmissionDisposition::None);
assert_eq!(
    report.artifact_verification,
    SourceArtifactVerificationSummary::not_applicable()
);
assert!(report.oracle_evidence.is_none());
```

Malformed/rejected cases must expose diagnostics only:

```rust
// crates/loom-vortex-ingress/tests/source_ingress_handoff.rs:299-315
let report = emit_source_ingress_lmc1_from_vortex_buffer(b"not a vortex file")
    .expect_err("malformed source report");

assert_eq!(report.status, SourceIngressStatus::Rejected);
assert!(report.facts.is_none());
assert_eq!(report.emission_kind, SourceEmissionKind::None);
assert_eq!(report.emission_disposition, SourceEmissionDisposition::None);
assert!(report.oracle_evidence.is_none());
```

Primitive/table equivalence should decode verified Loom output and compare to oracle rows. Copy the `single_column_to_loom` and `table_to_loom` pattern:

```rust
// crates/loom-vortex-ingress/tests/single_column_to_loom.rs:57-65
fn decode_lmc1(bytes: &[u8]) -> ArrayData {
    assert!(is_container_payload(bytes));
    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_artifact(bytes, &registry, &Default::default());
    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);

    let desc = decode_layout_payload_maybe_container(bytes).expect("decode LMC1 layout");
    decode_layout_to_array_data(&desc, &registry).expect("decode Loom layout")
}
```

```rust
// crates/loom-vortex-ingress/tests/table_to_loom.rs:135-156
let registry = L2KernelRegistry::default_for_mvp0();
let report = verify_artifact(&loom, &registry, &Default::default());
assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);

let table = decode_table_payload_maybe_container(&loom).expect("decode table");
assert_eq!(table.row_count, 3);
...
let arrays = decode_table_to_array_data(&table, &registry).expect("decode table arrays");
```

## Script/Gate Patterns

Copy the shell style from `scripts/source-ingress-contract-test.sh`: strict bash, repo root normalization, local helper functions, focused tests first, dependency/API creep scans second.

```bash
# scripts/source-ingress-contract-test.sh:1-23
#!/usr/bin/env bash
# source-ingress-contract-test.sh - Phase 26 source ingress contract gate.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"
...
info() { echo "${YLW}[source-ingress]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }
```

Add a Phase 27 script such as `scripts/lance-parquet-ingress-test.sh`. It should check:

- Phase 27 required docs exist, including `27-ARCHIVAL-READABILITY-REPORT.md`.
- `Cargo.toml` includes the new adapter crate members.
- focused tests pass for `loom-lance-ingress` and `loom-parquet-ingress`.
- `cargo test -p loom-source-ingress` still passes.
- `cargo test -p loom-core --test artifact_verifier` still passes.
- `loom-core`, `loom-ffi`, `loom-source-ingress`, DuckDB extension files, public headers, and CLI public surface contain no Lance/Parquet SDK dependency or route markers.
- report markers include support matrix, accepted/unsupported/rejected matrix, oracle evidence, dependency guards, tradeoffs, non-goals, and Phase 28 handoff.

The existing dependency/API guard pattern is directly reusable:

```bash
# scripts/source-ingress-contract-test.sh:166-199
info "Checking source dependency boundaries..."
source_dep_patterns=(
    "vort""ex"
    "fast""lanes"
    "lanc""e"
    "par""quet"
    "ice""berg"
    ...
)

check_cargo_tree_clean loom-core "${source_dep_patterns[@]}"
check_cargo_tree_clean loom-ffi "${source_dep_patterns[@]}"
check_cargo_tree_clean loom-source-ingress "${source_dep_patterns[@]}"
```

Wire the Phase 27 gate into `scripts/mvp0-verify.sh` after Phase 26 and before DuckDB smoke, mirroring the current late gate ordering:

```bash
# scripts/mvp0-verify.sh:124-133
# Keep late release gates ordered by contract dependency:
# Phase 24 DuckDB native integration -> Phase 25 native hardening ->
# Phase 26 source ingress contract -> DuckDB SQL smoke.
info "Running Phase 26 source ingress contract gate..."
bash scripts/source-ingress-contract-test.sh
ok "scripts/source-ingress-contract-test.sh"

info "Running DuckDB SQL smoke test..."
bash scripts/duckdb-smoke-test.sh
```

## Naming Rules

Use source-specific names only inside source-specific crates and tests:

- Good: `loom-lance-ingress`, `loom-parquet-ingress`, `lance_ingress_report_from_path`, `parquet_source_facts_from_path`.
- Good generic report fields: `SourceIdentity`, `SourceFacts`, `SourceCoverage`, `SourceOracleEvidence`, `SourceEmissionKind`.
- Avoid adding `Lance*` or `Parquet*` public types to `loom-source-ingress`.
- Avoid new public SQL/API names such as `loom_scan_lance`, `loom_scan_parquet`, `loom_ingest_lance`, or `loom_ingest_parquet`.
- Avoid exposing source SDK handles, readers, credentials, object-store state, Arrow stream ownership handles, or native dataset objects in generic reports.

Source identity should use plain strings. Copy `SourceIdentity::new(...).with_format_version(...).with_path_display(...)` from:

```rust
// crates/loom-source-ingress/src/lib.rs:35-59
impl SourceIdentity {
    pub fn new(source_kind: impl Into<String>, format: impl Into<String>) -> Self { ... }
    pub fn with_format_version(mut self, format_version: impl Into<String>) -> Self { ... }
    pub fn with_fingerprint(mut self, fingerprint: impl Into<String>) -> Self { ... }
    pub fn with_path_display(mut self, path_display: impl Into<String>) -> Self { ... }
}
```

## Risks

Public API creep: Phase 27 is adapter evidence, not host/query integration. Keep CLI, DuckDB, FFI headers, and generic contract free of Lance/Parquet route names. Copy the public-surface scans from `scripts/source-ingress-contract-test.sh:201-249`.

Dependency leakage: source SDK crates must not appear in `loom-core`, `loom-ffi`, or `loom-source-ingress` dependency trees. Use `cargo tree -p loom-core`, `cargo tree -p loom-ffi`, and `cargo tree -p loom-source-ingress` guards.

Over-claiming archival readability: accepted canonical raw/table emission proves row equivalence for the supported primitive/table slice only. It does not prove full Lance/Parquet semantic compatibility, nested types, nulls, indices, predicate pushdown, split execution, object-store behavior, or writer-internal compatibility.

Oracle misuse: source-native or Arrow scans are evidence, not the Loom decode path. Accepted reports still require `verify_artifact` acceptance first.

Unsupported partial output: valid but unsupported sources may expose facts, but must return no `.loom` bytes. Rejected malformed inputs must expose no trusted facts.

Legacy fixture brittleness: if old writer tooling is unstable, use checked-in or generated fixture evidence and record the gap in `27-ARCHIVAL-READABILITY-REPORT.md` rather than widening dependencies or requiring remote services.

Shared helper premature abstraction: Lance and Parquet can duplicate small mapping helpers initially. Introduce a shared helper only after both adapters have real identical code and it does not pull source SDK dependencies into generic crates.

## Metadata

**Analog search scope:** `Cargo.toml`, `crates/*/Cargo.toml`, `crates/loom-source-ingress`, `crates/loom-vortex-ingress`, `crates/loom-core`, `scripts`, Phase 26/27 planning docs.  
**Files scanned:** 24 required files/directories plus targeted grep over `crates`, `scripts`, `Cargo.toml`, and Phase 27 planning directory.  
**Pattern extraction date:** 2026-06-09.
