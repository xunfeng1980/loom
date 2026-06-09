# Phase 29: Iceberg Ref/Table Binding - Pattern Map

**Mapped:** 2026-06-09
**Files analyzed:** 11 likely new/modified files
**Analogs found:** 11 / 11

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `Cargo.toml` | config | dependency isolation | `Cargo.toml` + adapter manifests | role-match |
| `crates/loom-iceberg-binding/Cargo.toml` | config | dependency isolation | `ingress/loom-parquet-ingress/Cargo.toml` / `ingress/loom-lance-ingress/Cargo.toml` | role-match |
| `crates/loom-iceberg-binding/src/lib.rs` | service | request-response / transform | `ingress/loom-parquet-ingress/src/source_contract.rs` | exact |
| `crates/loom-iceberg-binding/src/binding_contract.rs` | service | request-response / transform | `ingress/loom-lance-ingress/src/source_contract.rs` | exact |
| `crates/loom-iceberg-binding/tests/binding_contract.rs` | test | request-response | `ingress/loom-parquet-ingress/tests/source_ingress_contract.rs` | exact |
| `crates/loom-iceberg-binding/tests/binding_handoff.rs` | test | verifier-routed transform | `ingress/loom-parquet-ingress/tests/source_ingress_handoff.rs` | exact |
| `crates/loom-iceberg-binding/tests/dependency_boundary.rs` | test | dependency isolation | `ingress/loom-parquet-ingress/tests/dependency_boundary.rs` | exact |
| `crates/loom-iceberg-binding/tests/fixtures/*` | test fixture | file-I/O | `ingress/loom-parquet-ingress/tests/fixtures/legacy/*` + `ingress/loom-lance-ingress/tests/fixtures/legacy/*` | role-match |
| `.planning/phases/29-iceberg-ref-table-binding/29-ICEBERG-BINDING-REPORT.md` | report | batch / evidence | `.planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-ARCHIVAL-READABILITY-REPORT.md` | exact |
| `scripts/iceberg-binding-test.sh` | test gate | batch | `scripts/lance-parquet-ingress-test.sh` | exact |
| `scripts/mvp0-verify.sh` | release gate | batch | `scripts/mvp0-verify.sh` Phase 26/27 ordering block | exact |

## Pattern Assignments

### `crates/loom-iceberg-binding/src/lib.rs` / `src/binding_contract.rs` (service, request-response / transform)

**Analog:** `ingress/loom-parquet-ingress/src/source_contract.rs`

**Ownership pattern** (lines 1-4):
```rust
//! Source-neutral facts extracted from local Parquet files.
//!
//! Parquet SDK objects are adapter-private. Public helpers return only
//! `loom-source-ingress` contract data.
```

Copy the same boundary, but make it Iceberg-binding-specific: Iceberg SDK/types stay adapter-private, while generic outputs use Loom-owned structs and strings. Do not add Iceberg vocabulary to `loom-source-ingress`.

**Imports pattern** (lines 6-26):
```rust
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceCoverage, SourceDiagnostic, SourceDiagnosticCode,
    SourceEmissionDisposition, SourceEmissionKind, SourceFacts, SourceIdentity,
    SourceIngressAcceptedArtifact, SourceIngressReport, SourceIngressStatus, SourceLayoutFact,
    SourceLoweringDisposition, SourceOracleEvidence, SourceOracleStrategy, SourceSchemaFact,
    SourceSplitFact,
};
```

For Iceberg binding, keep the same `loom_core` verifier imports and `loom_source_ingress` evidence imports. Add any Iceberg crate import only in this adapter crate/module.

**Local metadata open/reject pattern** (lines 29-70):
```rust
pub fn parquet_source_facts_from_path(path: &Path) -> Result<SourceFacts, SourceIngressReport> {
    let file = File::open(path).map_err(|error| {
        rejected_report(
            path,
            diagnostic_with_detail(
                SourceDiagnosticCode::OpenFailed,
                "$.open",
                "local Parquet file could not be opened",
                error.to_string(),
            ),
        )
    })?;

    let builder = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|error| {
        rejected_report(
            path,
            diagnostic_with_detail(
                SourceDiagnosticCode::ReadFailed,
                "$.metadata",
                "local Parquet metadata could not be read",
                error.to_string(),
            ),
        )
    })?;
```

Use this shape for `iceberg_binding_facts_from_path` or equivalent. Malformed metadata must return `SourceIngressReport::rejected`-style diagnostics only. Valid-but-unsupported Iceberg metadata can expose facts, but no accepted binding.

**Byte-free report pattern** (lines 73-85):
```rust
pub fn source_ingress_report_from_parquet_path(path: &Path) -> SourceIngressReport {
    match parquet_source_facts_from_path(path) {
        Ok(facts) => {
            let diagnostic = diagnostic_for_facts(&facts);
            SourceIngressReport::unsupported(Some(facts), diagnostic)
        }
        Err(report) => report,
    }
}
```

Iceberg should provide a byte-free/facts-only path for valid metadata that is not accepted because verifier evidence, oracle evidence, or identity matching is missing.

**Verifier-routed accepted binding pattern** (lines 132-190):
```rust
pub fn emit_source_ingress_lmc1_from_parquet_path(
    path: &Path,
) -> Result<SourceIngressAcceptedArtifact, SourceIngressReport> {
    let facts = parquet_source_facts_from_path(path)?;
    let coverage = facts
        .coverage
        .as_ref()
        .expect("Parquet facts always include coverage");
    if coverage.support != SourceIngressStatus::Accepted {
        let diagnostic = diagnostic_for_facts(&facts);
        return Err(SourceIngressReport::unsupported(Some(facts), diagnostic));
    }

    let batches = parquet_arrow_oracle_batches_from_path(path)
        .map_err(|report| source_oracle_failed_report(&facts, report))?;
    let artifact_bytes = loom_artifact_from_batches(&batches)
        .map_err(|diagnostic| SourceIngressReport::unsupported(Some(facts.clone()), diagnostic))?;

    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_artifact(&artifact_bytes, &registry, &Default::default());
    if verification.status() != ArtifactVerificationStatus::Accepted {
        return Err(source_verification_failed_report(
            &facts,
            verification.status().as_str(),
        ));
    }
```

For Phase 29, accepted binding must take an existing Loom artifact sidecar/reference, verify it with `verify_artifact`, compare schema/snapshot/artifact hash facts, then construct accepted evidence. Never accept manifest-only metadata.

**Lance local-only guard pattern** (lines 26-48):
```rust
pub async fn lance_source_facts_from_path(path: &Path) -> Result<SourceFacts, SourceIngressReport> {
    let uri = path.to_str().ok_or_else(|| {
        rejected_report(
            path,
            SourceDiagnostic::new(
                SourceDiagnosticCode::OpenFailed,
                "$.open",
                "local Lance dataset path is not valid UTF-8",
            ),
        )
    })?;

    if uri.contains("://") {
        return Err(rejected_report(
            path,
            SourceDiagnostic::new(
                SourceDiagnosticCode::OpenFailed,
                "$.open",
                "only local Lance dataset paths are supported by this adapter",
            ),
        ));
    }
```

Reuse for Iceberg fixture metadata: reject remote/catalog/object-store URIs and credential-bearing paths/configs in Phase 29.

### `crates/loom-iceberg-binding/tests/binding_handoff.rs` (test, verifier-routed transform)

**Analog:** `ingress/loom-parquet-ingress/tests/source_ingress_handoff.rs`

**Verifier assertion pattern** (lines 68-72):
```rust
fn assert_emitted_artifact_is_verifier_accepted(bytes: &[u8]) {
    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_artifact(bytes, &registry, &Default::default());
    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
}
```

Phase 29 should assert that referenced sidecar Loom artifacts are verifier-accepted before any binding report is accepted.

**Accepted handoff evidence pattern** (lines 129-165):
```rust
#[test]
fn accepted_single_column_handoff_is_verifier_routed_lmp1() {
    let temp = TempDir::new().expect("tempdir");
    let path = single_i32_path(&temp);
    let accepted =
        emit_source_ingress_lmc1_from_parquet_path(&path).expect("accepted Parquet handoff");

    assert!(!accepted.bytes.is_empty());
    assert_emitted_artifact_is_verifier_accepted(&accepted.bytes);
    assert_eq!(accepted.report.status, SourceIngressStatus::Accepted);
    assert_eq!(accepted.report.emission_kind, SourceEmissionKind::Lmp1);
    assert_eq!(
        accepted.report.emission_disposition,
        SourceEmissionDisposition::CanonicalRaw
    );
    assert!(accepted.report.artifact_verification.required);
    assert!(accepted.report.artifact_verification.accepted);
    assert_eq!(
        accepted.report.artifact_verification.artifact_byte_len,
        Some(accepted.bytes.len())
    );
}
```

Adapt this into accepted Iceberg binding tests that require non-empty sidecar/ref, accepted verifier summary, table UUID/name, snapshot ID, schema ID, and artifact hash/content identity matches.

**Oracle evidence pattern** (lines 202-220):
```rust
#[test]
fn accepted_handoff_records_arrow_scan_oracle_evidence() {
    let accepted =
        emit_source_ingress_lmc1_from_parquet_path(&path).expect("accepted Parquet handoff");

    let oracle = accepted
        .report
        .oracle_evidence
        .as_ref()
        .expect("Arrow oracle evidence");
    assert_eq!(oracle.strategy, SourceOracleStrategy::ArrowScan);
    assert!(oracle.accepted);
    assert_eq!(oracle.row_count_checked, Some(3));
    assert!(oracle.nulls_checked);
}
```

Iceberg oracle/equivalence can be local fixture evidence, decoded-row fixture evidence, or Arrow scan evidence depending on the chosen fixture. The invariant is accepted binding requires accepted oracle/equivalence evidence.

### `crates/loom-iceberg-binding/tests/binding_contract.rs` (test, request-response)

**Analog:** `ingress/loom-source-ingress/src/lib.rs`

**Status and identity vocabulary** (lines 6-33):
```rust
pub enum SourceIngressStatus {
    Accepted,
    Unsupported,
    Rejected,
}

pub struct SourceIdentity {
    pub source_kind: String,
    pub format: String,
    pub format_version: Option<String>,
    pub fingerprint: Option<String>,
    pub path_display: Option<String>,
}
```

Use `source_kind = "iceberg"` or adapter-local equivalent in facts, but keep the generic crate unchanged. Binding-specific identity fields should live in the Iceberg adapter report structs or be encoded as bounded fact strings.

**Fact and evidence vocabulary** (lines 269-452):
```rust
pub struct SourceFacts {
    pub identity: SourceIdentity,
    pub row_count: u64,
    pub root_schema: Option<SourceSchemaFact>,
    pub schema_facts: Vec<SourceSchemaFact>,
    pub layout_facts: Vec<SourceLayoutFact>,
    pub segment_facts: Vec<SourceSegmentFact>,
    pub split_facts: Vec<SourceSplitFact>,
    pub coverage: Option<SourceCoverage>,
}

pub struct SourceArtifactVerificationSummary {
    pub required: bool,
    pub accepted: bool,
    pub artifact_byte_len: Option<usize>,
    pub summary: String,
}

pub struct SourceIngressAcceptedArtifact {
    pub bytes: Vec<u8>,
    pub report: SourceIngressReport,
}
```

Phase 29 should preserve accepted/unsupported/rejected semantics: accepted requires facts, verifier acceptance, source evidence, oracle/equivalence evidence, and matched Iceberg identity. Unsupported valid metadata exposes facts only. Rejected malformed metadata exposes diagnostics only.

### `crates/loom-iceberg-binding/tests/dependency_boundary.rs` (test, dependency isolation)

**Analog:** `ingress/loom-parquet-ingress/tests/dependency_boundary.rs`

**Direct dependency allowlist pattern** (lines 68-102):
```rust
#[test]
fn parquet_dependency_is_direct_only_in_parquet_adapter_manifest() {
    let root = workspace_root();
    let workspace_manifest = manifest(root.join("Cargo.toml"));

    assert!(direct_workspace_pin_has(&workspace_manifest, &parquet_name));

    let mut direct_parquet_manifests = Vec::new();
    for entry in std::fs::read_dir(root.join("crates")).expect("read crates dir") {
        let manifest_path = entry.expect("crate entry").path().join("Cargo.toml");
        if !manifest_path.exists() {
            continue;
        }
        let text = manifest(&manifest_path);
        if direct_dep_line_has(&text, &parquet_name) {
            direct_parquet_manifests.push(manifest_path);
        }
    }

    assert_eq!(
        direct_parquet_manifests,
        vec![root.join("ingress/loom-parquet-ingress/Cargo.toml")]
    );
}
```

Create the Iceberg version so direct Iceberg SDK dependency appears only in workspace pins and `crates/loom-iceberg-binding/Cargo.toml`.

**Generic contract leakage guard pattern** (lines 104-129):
```rust
#[test]
fn generic_source_ingress_contract_has_no_source_sdk_vocabulary() {
    let root = workspace_root();
    let source_files = [
        root.join("ingress/loom-source-ingress/Cargo.toml"),
        root.join("ingress/loom-source-ingress/src/lib.rs"),
        root.join("ingress/loom-source-ingress/tests/source_ingress_contract.rs"),
    ];
    let forbidden = [
        format!("{}{}", "par", "quet"),
        format!("{}{}", "Par", "quet"),
    ];

    for file in source_files {
        let text = manifest(&file);
        for marker in &forbidden {
            assert!(!text.contains(marker));
        }
    }
}
```

Add forbidden Iceberg markers for `loom-source-ingress`, `loom-core`, `loom-ffi`, public headers, DuckDB extension, and CLI.

### `scripts/iceberg-binding-test.sh` (test gate, batch)

**Analog:** `scripts/lance-parquet-ingress-test.sh`

**Script harness pattern** (lines 1-27):
```bash
#!/usr/bin/env bash
# lance-parquet-ingress-test.sh - Phase 27 Lance/Parquet closeout gate.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

info() { echo "${YLW}[lance-parquet-ingress]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

PHASE_DIR=".planning/phases/27-lance-parquet-archival-readability-dataset-ingress"
REPORT="${PHASE_DIR}/27-ARCHIVAL-READABILITY-REPORT.md"
```

Copy this for `[iceberg-binding]`, `PHASE_DIR=".planning/phases/29-iceberg-ref-table-binding"`, and `REPORT="${PHASE_DIR}/29-ICEBERG-BINDING-REPORT.md"`.

**Report marker gate pattern** (lines 166-195):
```bash
info "Checking Phase 27 planning and report artifacts..."
for file in \
    "${PHASE_DIR}/27-CONTEXT.md" \
    "${PHASE_DIR}/27-RESEARCH.md" \
    "${PHASE_DIR}/27-PATTERNS.md" \
    "${REPORT}"; do
    check_file "${file}"
done

for marker in \
    "Supported Slice" \
    "Unsupported and Rejected Matrix" \
    "Verifier Evidence" \
    "Oracle Evidence" \
    "Dependency and API Boundary" \
    "Current-Phase Tradeoffs" \
    "Non-Goals"; do
    check_marker "${marker}" "${REPORT}" "report section marker"
done
```

Phase 29 report markers should include: `Binding Schema`, `Accepted Unsupported Rejected Matrix`, `Source Evidence`, `Verifier Evidence`, `Oracle Evidence`, `Dependency and API Boundary`, `Current-Phase Tradeoffs`, `Non-Goals`, and `Phase 29 Handoff`.

**Focused test ordering pattern** (lines 210-221):
```bash
info "Running focused Phase 27 adapter and verifier tests..."
cargo test -p loom-source-ingress
cargo test -p loom-parquet-ingress --test dependency_boundary
cargo test -p loom-parquet-ingress --test source_ingress_contract
cargo test -p loom-parquet-ingress --test source_ingress_handoff
cargo test -p loom-core --test artifact_verifier
ok "focused Phase 27 tests"
```

Phase 29 should run the Iceberg binding crate tests plus `cargo test -p loom-core --test artifact_verifier`. Keep Phase 26/27 gates as prerequisites through `mvp0-verify.sh`, not by recursively invoking them inside the focused gate.

**Source dependency boundary gate pattern** (lines 223-257):
```bash
source_dep_patterns=(
    "lanc""e"
    "par""quet"
    "ice""berg"
    "m""cap"
    "z""arr"
    "object_""store"
    "object-""store"
)

check_cargo_tree_clean loom-core "${source_dep_patterns[@]}"
check_cargo_tree_clean loom-ffi "${source_dep_patterns[@]}"
check_cargo_tree_clean loom-source-ingress "${source_dep_patterns[@]}"
check_cargo_tree_clean loom-cli "${source_dep_patterns[@]}"
```

For Phase 29, allow Iceberg only in the Iceberg binding adapter. Continue to reject Iceberg/object-store/catalog credential markers in core, FFI, source-ingress, CLI, public headers, and DuckDB host code.

**Public-surface creep pattern** (lines 259-313):
```bash
api_surfaces=(
    crates/loom-ffi/include/loom.h
    crates/loom-ffi/include/loom_runtime.h
    crates/loom-ffi/include/loom_duckdb_internal.h
    duckdb-ext/loom_extension.cpp
    crates/loom-cli/src/main.rs
)

source_route_markers=(
    "loom_scan_""lance"
    "loom_scan_""parquet"
    "loom_ingest_""lance"
    "loom_ingest_""parquet"
    "loom_source_""sql"
)

check_no_fixed_patterns "route-specific source SQL/API" "${api_surfaces[@]}" -- "${source_route_markers[@]}"
```

Add Iceberg route markers such as `loom_scan_iceberg`, `loom_ingest_iceberg`, `iceberg_catalog`, `iceberg_rest`, `warehouse`, `branch`, `tag`, and credential markers. Also guard against manifest-only accepted language.

### `scripts/mvp0-verify.sh` (release gate, batch)

**Analog:** `scripts/mvp0-verify.sh`

**Ordered late-gate insertion pattern** (lines 124-138):
```bash
# Keep late release gates ordered by contract dependency:
# Phase 24 DuckDB native integration -> Phase 25 native hardening ->
# Phase 26 source ingress contract -> Phase 27 Lance/Parquet ingress ->
# DuckDB SQL smoke.
info "Running Phase 26 source ingress contract gate..."
bash scripts/source-ingress-contract-test.sh
ok "scripts/source-ingress-contract-test.sh"

info "Running Phase 27 Lance/Parquet ingress gate..."
bash scripts/lance-parquet-ingress-test.sh
ok "scripts/lance-parquet-ingress-test.sh"

info "Running DuckDB SQL smoke test..."
```

Insert Phase 29 after Phase 27 and before DuckDB SQL smoke:

```bash
info "Running Phase 29 Iceberg binding gate..."
bash scripts/iceberg-binding-test.sh
ok "scripts/iceberg-binding-test.sh"
```

## Shared Patterns

### Accepted / Unsupported / Rejected Semantics

**Source:** `ingress/loom-source-ingress/src/lib.rs` lines 6-21, 403-452

Apply to all Phase 29 binding APIs and tests:

- `accepted`: Iceberg table/ref facts, verifier-accepted Loom artifact, source evidence, oracle/equivalence evidence, and matched schema/snapshot/artifact identity.
- `unsupported`: valid Iceberg metadata or manifest facts, but no accepted Loom binding and no artifact bytes.
- `rejected`: malformed or unopenable metadata with diagnostics only.

### Verifier Handoff

**Source:** `ingress/loom-parquet-ingress/src/source_contract.rs` lines 151-173

Accepted binding must call `verify_artifact` on the referenced Loom artifact and copy a `SourceArtifactVerificationSummary::accepted(...)` style summary forward. The verifier summary is evidence, not a trust token; stale hash/schema/snapshot mismatches still fail closed.

### Oracle / Equivalence Evidence

**Source:** `ingress/loom-parquet-ingress/tests/source_ingress_handoff.rs` lines 202-220

Accepted binding tests must check the selected oracle/equivalence evidence is present and accepted. If the first Iceberg fixture is hand-authored metadata plus paired Loom artifact, use decoded-row fixture evidence rather than claiming source-native query integration.

### Dependency Isolation

**Source:** `ingress/loom-parquet-ingress/tests/dependency_boundary.rs` lines 68-129 and `scripts/lance-parquet-ingress-test.sh` lines 223-257

Iceberg dependencies belong only in `crates/loom-iceberg-binding` and workspace pins. `loom-core`, `loom-ffi`, `loom-source-ingress`, CLI, DuckDB host code, and public headers remain Iceberg-SDK-free.

### Public Surface Leakage Guards

**Source:** `scripts/lance-parquet-ingress-test.sh` lines 259-313

Phase 29 must not add public SQL functions, C ABI symbols, DuckDB table functions, CLI routes, remote catalog auth, object-store credentials, branch/tag mutation, or query engine integration. Gate this with fixed-string scans.

### Report Shape

**Source:** `.planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-ARCHIVAL-READABILITY-REPORT.md`

Copy the Phase 27 report shape: executive summary, implemented artifacts, supported slice, unsupported/rejected matrix, verifier evidence, oracle evidence, dependency/API boundary, current-phase tradeoffs, non-goals, release-gate evidence, and next-phase handoff. Replace legacy readability sections with Iceberg binding schema and schema/snapshot/hash mismatch matrix.

## Pitfalls To Preserve In Planning

- Do not modify `loom-source-ingress` to add Iceberg-specific public types unless the planner explicitly scopes that as generic source-neutral vocabulary. Phase 29 context says Iceberg vocabulary belongs only in the adapter/report boundary.
- Do not accept Iceberg manifest-only metadata as proof of a Loom artifact. Accepted binding requires an existing verifier-accepted sidecar/reference Loom artifact.
- Do not treat schema ID, snapshot ID, table UUID, or manifest location as trust by themselves. They are descriptive until matched to artifact hash/content identity and source evidence.
- Do not let object-store, REST catalog, warehouse, branch/tag, or credential dependencies leak into public/API surfaces.
- Do not wire `scripts/iceberg-binding-test.sh` into `scripts/mvp0-verify.sh` until the focused gate passes.
- Avoid broad Iceberg type coverage. Use the Phase 27 primitive non-null `LMC1(LMP1/LMT1)` slice unless a plan explicitly narrows otherwise.

## No Analog Found

No likely Phase 29 file lacks an analog. The Iceberg-specific metadata parser/binding structs have no exact existing Iceberg analog, but the service/report/test/gate pattern is covered by the Lance/Parquet/Vortex source adapters.

## Metadata

**Analog search scope:** `ingress/loom-source-ingress`, `ingress/loom-parquet-ingress`, `ingress/loom-lance-ingress`, `ingress/loom-vortex-ingress`, `scripts`, `.planning/phases/26-*`, `.planning/phases/27-*`
**Files scanned:** 10 required files plus adapter test/gate analogs
**Pattern extraction date:** 2026-06-09
